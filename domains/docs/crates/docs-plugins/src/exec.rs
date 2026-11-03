//! The action executor (PLAN section 4.3, 5.1, 6): runs one invocation of a plugin against a session.
//!
//! Rules, each with a test: every answer is checked against the plugin's grants and the command allowlist before its first action
//! runs; all mutating actions of one invocation are one undo step; an error in any round restores the snapshot taken before the
//! first action (the document, the selection and the undo stacks are as they were); a text map is applied only when the document
//! revision has not moved since it was read; an `event` invocation can only touch the plugin's own UI.
use std::collections::BTreeMap;
use std::sync::Arc;

use plugin_api::action::{Action, Answer, Call, CallKind, Denied, Grants, ReadReq, ReadWhat, Scope as ApiScope, Widget};
use plugin_api::manifest::{Event, ReadKind};
use plugin_api::textmap::{Apply, TextMap};
use plugin_api::wire::{MAX_ROUNDS, Message};
use serde_json::{Map, Value, json};
use wordcraft_engine::{CmdError, CmdResult, Session};

use crate::textmap::{self, Opts, Scope, Stats};
use crate::{GuestSession, Plugin};

fn fail<T>(s: impl Into<String>) -> Result<T, CmdError> {
    Err(CmdError::Failed(s.into()))
}

fn scope_of(v: Option<&str>) -> Scope {
    match v {
        Some("selection") => Scope::Selection,
        Some("document") => Scope::Document,
        _ => Scope::Auto,
    }
}

fn scope_api(s: ApiScope) -> Scope {
    match s {
        ApiScope::Auto => Scope::Auto,
        ApiScope::Selection => Scope::Selection,
        ApiScope::Document => Scope::Document,
    }
}

/// Read the text map of a scope: one paragraph per walked paragraph, one entry per live segment (tracked deletions are left out).
pub fn read_map(s: &mut Session, scope: Scope, field_results: bool) -> Result<TextMap, CmdError> {
    let mut map = TextMap::default();
    let (_, targets) = textmap::targets(s, scope);
    let Some(targets) = targets else { return Ok(map) };
    let mut err: Option<CmdError> = None;
    let mut index = 0u32;
    let tracked = s.doc.settings.track_changes;
    let mut st = Stats::default();
    let o = Opts { field_results, tracked, lang_ok: &|_| true };
    textmap::walk(s, &targets, &o, &mut st, &mut |s, p, segs| {
        let i = index;
        index += 1;
        let runs: Vec<_> = p.run_ranges().collect();
        let mut out = Vec::with_capacity(segs.len());
        for g in segs.iter().filter(|g| !g.deleted) {
            let tag = runs.get(g.run).and_then(|(_, props)| s.doc.styles.resolve_char(p.props.style.as_deref(), props).lang).unwrap_or_default();
            let lang = match map.lang_index(&tag) {
                Ok(l) => l,
                Err(e) => {
                    err = Some(CmdError::Failed(e.to_string()));
                    return false;
                }
            };
            let flags = if g.convert { plugin_api::textmap::EDITABLE } else if g.why != 0 { g.why } else { plugin_api::textmap::OBJECT };
            out.push(plugin_api::textmap::Segment { flags, lang, text: p.text[g.range.clone()].to_string() });
        }
        map.paragraphs.push(plugin_api::textmap::Paragraph { index: i, segments: out });
        false
    })?;
    match err {
        Some(e) => Err(e),
        None => Ok(map),
    }
}

/// Apply a decoded `Apply` against the map read earlier. The same walk as the read, so paragraph and segment numbers agree; every
/// segment is checked again against the live text before anything is written, and `Apply::validate` has refused counts that differ
/// and changes to segments that are not editable.
pub fn apply_map(s: &mut Session, scope: Scope, field_results: bool, map: &TextMap, ap: &Apply) -> Result<Stats, CmdError> {
    ap.validate(map).map_err(|e| CmdError::Failed(format!("plugin answer refused: {e}")))?;
    let (_, targets) = textmap::targets(s, scope);
    let mut st = Stats::default();
    let Some(targets) = targets else {
        return if map.paragraphs.is_empty() { Ok(st) } else { fail("the selection changed since the text map was read") };
    };
    let mut err: Option<String> = None;
    let (mut index, mut cursor) = (0u32, 0usize);
    let tracked = s.doc.settings.track_changes;
    let o = Opts { field_results, tracked, lang_ok: &|_| true };
    textmap::walk(s, &targets, &o, &mut st, &mut |_, p, segs| {
        let i = index;
        index += 1;
        if err.is_some() {
            return false;
        }
        let Some(mp) = map.paragraphs.get(cursor).filter(|m| m.index == i) else { return false };
        let new = &ap.paragraphs[cursor];
        cursor += 1;
        let live: Vec<usize> = (0..segs.len()).filter(|&k| !segs[k].deleted).collect();
        if live.len() != mp.segments.len() {
            err = Some(format!("paragraph {i} has {} segments, the text map had {}", live.len(), mp.segments.len()));
            return false;
        }
        let mut changed = false;
        for (k, &g) in live.iter().enumerate() {
            let seen = &mp.segments[k];
            if p.text[segs[g].range.clone()] != seen.text || segs[g].convert != seen.editable() {
                err = Some(format!("paragraph {i} changed since the text map was read"));
                return false;
            }
            if let Some(t) = &new[k]
                && *t != seen.text
            {
                segs[g].new = Some(t.clone());
                changed = true;
            }
        }
        changed
    })?;
    if let Some(e) = err {
        return fail(e);
    }
    if cursor != map.paragraphs.len() {
        return fail("the document has fewer paragraphs than the text map had");
    }
    Ok(st)
}

/// The selected text, paragraphs joined by a line feed.
fn selection_text(s: &Session) -> String {
    let (_, t) = textmap::targets(s, Scope::Selection);
    t.map(|t| textmap::text_of(s, &t)).unwrap_or_default()
}

/// What one invocation is.
pub struct Invocation {
    pub plugin: Arc<Plugin>,
    pub kind: CallKind,
    pub command: Option<String>,
    pub widget: Option<String>,
    pub event: Option<Event>,
    pub params: Map<String, Value>,
    pub lang: String,
    /// The reads the first call carries (the command's `reads`).
    pub reads: Vec<ReadKind>,
    /// An asynchronous caller (the web bridge) names the revision its reads belong to; a moved revision refuses the invocation.
    pub expect_rev: Option<u64>,
    /// The caller is already inside a mutating `Session::run`, whose checkpoint is the undo step.
    pub in_command: bool,
    pub label: String,
}

struct Run {
    inv: Invocation,
    last_map: Option<(TextMap, Scope, bool)>,
    /// The revision the text map was read at.
    read_rev: u64,
    checkpointed: bool,
    panels: Vec<(String, Vec<Widget>)>,
    status: Option<String>,
    applied: Stats,
    rounds: usize,
}

impl Run {
    fn field_results(&self) -> bool {
        self.inv.params.get("fieldResults").and_then(Value::as_bool).unwrap_or(true)
    }

    /// Perform one read: the blob goes into `blobs`, its index under the read's name in `reads`.
    fn read(&mut self, s: &mut Session, req: &ReadReq, blobs: &mut Vec<Vec<u8>>, reads: &mut BTreeMap<String, u32>) -> Result<u32, CmdError> {
        let (name, bytes) = match req.what {
            ReadWhat::Textmap => {
                let scope = req.scope.map(scope_api).unwrap_or_else(|| scope_of(self.inv.params.get("scope").and_then(Value::as_str)));
                let fr = self.field_results();
                let map = read_map(s, scope, fr)?;
                let bytes = map.encode().map_err(|e| CmdError::Failed(e.to_string()))?;
                self.read_rev = s.rev();
                self.last_map = Some((map, scope, fr));
                ("textmap", bytes)
            }
            ReadWhat::Selection => ("selection", selection_text(s).into_bytes()),
            ReadWhat::Paragraph => {
                let Some(i) = req.index else { return fail("read paragraph needs an index") };
                let Some((map, ..)) = &self.last_map else { return fail("read paragraph needs a text map read in this invocation") };
                let Some(p) = map.paragraphs.iter().find(|p| p.index == i) else { return fail(format!("paragraph {i} is not in the text map")) };
                ("paragraph", p.segments.iter().map(|g| g.text.as_str()).collect::<String>().into_bytes())
            }
        };
        let at = blobs.len() as u32;
        blobs.push(bytes);
        reads.insert(name.to_string(), at);
        Ok(at)
    }

    /// The first mutation of an invocation outside a command opens the undo step; every later one joins it.
    fn mutating(&mut self, s: &mut Session) {
        if !self.checkpointed {
            s.checkpoint(&self.inv.label);
            self.checkpointed = true;
        }
        s.join_next_undo();
    }

    fn action(&mut self, s: &mut Session, a: &Action, answer_blobs: &[Vec<u8>], blobs: &mut Vec<Vec<u8>>, reads: &mut BTreeMap<String, u32>) -> CmdResult {
        match a {
            Action::Run { id, params } => {
                if s.registry.get(id).is_some_and(|c| c.mutates) {
                    self.mutating(s);
                }
                s.run(id, params)
            }
            Action::TextmapApply { blob } => {
                let Some((map, scope, fr)) = self.last_map.take() else { return fail("textmap.apply without a text map read in this invocation") };
                if s.rev() != self.read_rev {
                    return fail("the document changed since the text map was read (stale revision)");
                }
                let bytes = answer_blobs.get(*blob as usize).ok_or_else(|| CmdError::Failed("blob index out of range".into()))?;
                let ap = Apply::decode(bytes).map_err(|e| CmdError::Failed(format!("plugin answer refused: {e}")))?;
                self.mutating(s);
                let st = apply_map(s, scope, fr, &map, &ap)?;
                s.touch();
                self.applied.paragraphs += st.paragraphs;
                self.applied.runs += st.runs;
                Ok(json!({"paragraphs": st.paragraphs, "runs": st.runs}))
            }
            Action::Read(req) => Ok(json!({"blob": self.read(s, req, blobs, reads)?})),
            Action::Fetch { url, method, headers, body } => match &self.inv.plugin.fetcher {
                Some(f) => Ok(f.fetch(url, method, headers, body.as_deref()).unwrap_or_else(|e| json!({"error": e}))),
                None => Ok(json!({"error": "network is not available in this host"})),
            },
            Action::UiPanel { panel, widgets } => {
                self.panels.push((panel.clone(), widgets.clone()));
                Ok(Value::Null)
            }
            Action::UiStatus { text } => {
                self.status = Some(text.clone());
                Ok(Value::Null)
            }
        }
    }
}

fn denied(d: Denied) -> CmdError {
    CmdError::Failed(format!("plugin action refused: {d:?}"))
}

/// One invocation between its rounds. The synchronous path ([`rounds`]) loops over it with a guest that answers at once; the web bridge
/// ([`crate::bridge`]) keeps it while the page runs the plugin in a Worker and calls [`Machine::resume`] when the answer comes.
pub struct Machine {
    run: Run,
    call: Call,
    guest: Box<dyn GuestSession>,
    blobs: Vec<Vec<u8>>,
}

pub enum Step {
    /// The next call to the plugin.
    Call(Message),
    /// Every round succeeded: the result of the command.
    Done(Value),
}

impl Machine {
    /// Start the guest, do the command's reads and build the first call.
    pub fn begin(s: &mut Session, inv: Invocation) -> Result<(Machine, Message), CmdError> {
        if let Some(r) = inv.expect_rev
            && s.rev() != r
        {
            return fail("the document changed before the plugin was called (stale revision)");
        }
        let p = inv.plugin.clone();
        let guest = p.guest.start().map_err(|e| CmdError::Failed(format!("plugin could not start: {e}")))?;
        let mut run = Run { inv, last_map: None, read_rev: s.rev(), checkpointed: false, panels: Vec::new(), status: None, applied: Stats::default(), rounds: 0 };
        run.checkpointed = run.inv.in_command;
        let call = Call {
            kind: run.inv.kind,
            api: 1,
            api_minor: plugin_api::API_MINOR,
            app: "docs".into(),
            lang: run.inv.lang.clone(),
            command: run.inv.command.clone(),
            widget: run.inv.widget.clone(),
            event: run.inv.event.map(|e| e.name().to_string()),
            params: run.inv.params.clone(),
            settings: p.settings.clone(),
            results: Vec::new(),
            reads: BTreeMap::new(),
        };
        let mut m = Machine { run, call, guest, blobs: Vec::new() };
        for r in m.run.inv.reads.clone() {
            let what = match r {
                ReadKind::Textmap => ReadWhat::Textmap,
                ReadKind::Selection => ReadWhat::Selection,
                ReadKind::None => continue,
            };
            m.run.read(s, &ReadReq { what, scope: None, index: None }, &mut m.blobs, &mut m.call.reads)?;
        }
        let msg = m.next_message()?;
        Ok((m, msg))
    }

    fn next_message(&mut self) -> Result<Message, CmdError> {
        self.run.rounds += 1;
        Message::new(&self.call, std::mem::take(&mut self.blobs)).map_err(|e| CmdError::Failed(format!("call refused: {e}")))
    }

    /// The plugin's answer to the last call: check it, run its actions, and either build the next call or finish.
    pub fn resume(&mut self, s: &mut Session, out: &Message) -> Result<Step, CmdError> {
        let p = self.run.inv.plugin.clone();
        let panels: Vec<String> = p.manifest.panels.iter().map(|x| x.id.clone()).collect();
        let grants = Grants { document: p.manifest.permissions.document, origins: &p.origins, panels: &panels };
        let ans: Answer = out.header_as().map_err(|e| CmdError::Failed(format!("plugin answer refused: {e}")))?;
        plugin_api::action::check_actions(&ans.actions, &grants, self.run.inv.kind == CallKind::Event).map_err(denied)?;
        let (mut results, mut reads) = (Vec::with_capacity(ans.actions.len()), BTreeMap::new());
        for a in &ans.actions {
            results.push(self.run.action(s, a, &out.blobs, &mut self.blobs, &mut reads)?);
        }
        let Some(next) = ans.next else { return Ok(Step::Done(self.finish(s))) };
        if self.run.rounds >= MAX_ROUNDS {
            return fail("plugin asked for more than 16 rounds");
        }
        for r in &next.reads {
            self.run.read(s, r, &mut self.blobs, &mut reads)?;
        }
        self.call.kind = CallKind::Resume;
        self.call.results = results;
        self.call.reads = reads;
        self.call.params = Map::new();
        Ok(Step::Call(self.next_message()?))
    }

    /// Committed only when every round succeeded.
    fn finish(&mut self, s: &mut Session) -> Value {
        let run = &mut self.run;
        {
            let mut ui = run.inv.plugin.ui.lock().unwrap_or_else(|e| e.into_inner());
            for (id, widgets) in std::mem::take(&mut run.panels) {
                if widgets.is_empty() {
                    ui.panels.remove(&id);
                } else {
                    ui.panels.insert(id, widgets);
                }
            }
            ui.last_error = None;
        }
        if let Some(t) = &run.status {
            s.status = t.clone();
        }
        json!({"plugin": run.inv.plugin.manifest.id, "rounds": run.rounds, "paragraphs": run.applied.paragraphs, "runs": run.applied.runs, "status": run.status})
    }

    /// What the invocation is, for the web bridge.
    pub fn kind(&self) -> CallKind {
        self.run.inv.kind
    }

    pub fn plugin(&self) -> &Arc<Plugin> {
        &self.run.inv.plugin
    }

    pub fn event(&self) -> Option<Event> {
        self.run.inv.event
    }

    /// `Session::run` touches the document after the slot command returned: the text map was read at this revision, not the old one.
    pub fn rebase(&mut self, rev: u64) {
        self.run.read_rev = rev;
    }
}

fn rounds(s: &mut Session, inv: Invocation) -> CmdResult {
    let (mut m, mut msg) = Machine::begin(s, inv)?;
    loop {
        let out = m.guest.round(&msg).map_err(|e| CmdError::Failed(format!("plugin stopped: {e}")))?;
        match m.resume(s, &out)? {
            Step::Call(next) => msg = next,
            Step::Done(v) => return Ok(v),
        }
    }
}

/// Run one invocation. Any error puts the session back as it was before the first action. A plugin that lives in the page (the web
/// bridge) returns `{"pending": call}` at once and finishes later ([`crate::bridge`]).
pub fn invoke(s: &mut Session, inv: Invocation) -> CmdResult {
    if inv.plugin.guest.remote() {
        return crate::bridge::start(s, inv);
    }
    let plugin = inv.plugin.clone();
    let snap = s.edit_snapshot();
    let r = rounds(s, inv);
    if let Err(e) = &r {
        s.restore(snap);
        plugin.ui.lock().unwrap_or_else(|e| e.into_inner()).last_error = Some(e.to_string());
    }
    r
}

/// A slot command ran (`Session::run` has opened the undo step already).
pub fn run_command(s: &mut Session, p: &std::sync::Arc<Plugin>, command: usize, v: &Value) -> CmdResult {
    let Some(c) = p.manifest.commands.get(command) else { return fail("unknown plugin command") };
    let mut params = c.params.clone();
    if let Some(o) = v.as_object() {
        params.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    let inv = Invocation {
        plugin: p.clone(),
        kind: CallKind::Command,
        command: Some(c.id.clone()),
        widget: None,
        event: None,
        params,
        lang: crate::lang(),
        reads: c.reads.clone(),
        expect_rev: None,
        in_command: true,
        label: crate::localized(&c.label, "en"),
    };
    invoke(s, inv)
}

/// A click or edit in a plugin's panel. `value` is the text of a field or the chosen option.
pub fn ui_event(s: &mut Session, p: &Arc<Plugin>, widget: &str, value: Option<&str>) -> CmdResult {
    let mut params = Map::new();
    if let Some(v) = value {
        params.insert("value".into(), json!(v));
    }
    let inv = Invocation {
        plugin: p.clone(),
        kind: CallKind::Ui,
        command: None,
        widget: Some(widget.to_string()),
        event: None,
        params,
        lang: crate::lang(),
        reads: Vec::new(),
        expect_rev: None,
        in_command: false,
        label: crate::localized(&p.manifest.name, "en"),
    };
    invoke(s, inv)
}

/// Fire `event` at every enabled plugin that subscribed to it. Failures are recorded on the plugin and never reach the user.
pub fn emit(s: &mut Session, event: Event, lang: &str) {
    for p in crate::plugins() {
        if !p.enabled() || !p.manifest.events.contains(&event) {
            continue;
        }
        let inv = Invocation {
            plugin: p.clone(),
            kind: CallKind::Event,
            command: None,
            widget: None,
            event: Some(event),
            params: Map::new(),
            lang: lang.to_string(),
            reads: Vec::new(),
            expect_rev: None,
            in_command: false,
            label: String::new(),
        };
        let _ = invoke(s, inv);
    }
}

/// What the frame hook remembers to turn session changes into events.
#[derive(Default)]
struct Seen {
    doc_id: Option<u64>,
    dirty: bool,
    sel: Option<wordcraft_engine::Selection>,
    sel_changed_ms: f64,
    sel_sent: bool,
}

thread_local! {
    static SEEN: std::cell::RefCell<Seen> = std::cell::RefCell::new(Seen::default());
}

/// `selectionChanged` fires once the selection has been still for this long (PLAN section 4.3).
pub const SELECTION_DEBOUNCE_MS: f64 = 200.0;

/// Per frame: `documentOpened` when another document is open, `documentSaved` when the dirty flag fell, `selectionChanged` debounced.
pub fn events_frame(s: &mut Session, lang: &str) {
    if !crate::plugins().iter().any(|p| p.enabled() && !p.manifest.events.is_empty()) {
        return;
    }
    let now = wordcraft_layout::now_ms();
    let mut fire = Vec::new();
    SEEN.with(|seen| {
        let mut sn = seen.borrow_mut();
        if sn.doc_id != Some(s.document_id()) {
            sn.doc_id = Some(s.document_id());
            sn.dirty = s.dirty;
            sn.sel = Some(s.sel.clone());
            sn.sel_sent = true;
            fire.push(Event::DocumentOpened);
        } else {
            if sn.dirty && !s.dirty {
                fire.push(Event::DocumentSaved);
            }
            sn.dirty = s.dirty;
            if sn.sel.as_ref() != Some(&s.sel) {
                sn.sel = Some(s.sel.clone());
                sn.sel_changed_ms = now;
                sn.sel_sent = false;
            } else if !sn.sel_sent && now - sn.sel_changed_ms >= SELECTION_DEBOUNCE_MS {
                sn.sel_sent = true;
                fire.push(Event::SelectionChanged);
            }
        }
    });
    for e in fire {
        emit(s, e, lang);
    }
}

/// Test hook: forget what the frame hook saw.
pub fn reset_events() {
    SEEN.with(|s| *s.borrow_mut() = Seen::default());
}


