//! The action executor (PLAN section 4.3, 5.1, 5.2): runs one invocation of a plugin against an [`Editor`].
//!
//! The rules of `docs-plugins::exec`, each with a test in `tests.rs`: every answer is checked against the plugin's grants and the
//! app's command allowlist before its first action runs; a plugin that may write never starts on a protected document; all changes
//! of one invocation are one undo step; an error in any round restores the snapshot taken before the first read (document,
//! selection, undo and redo); a text map is applied only when the document has not moved since it was read; at most 16 reads per
//! round, 16 rounds, 8 fetches and 10 s per invocation; three timeouts switch the plugin off for the session.
use std::collections::BTreeMap;
use std::sync::Arc;

use plugin_api::action::{
    Action, Answer, Call, CallKind, Denied, Grants, ReadReq, ReadWhat, Scope as ApiScope, Widget,
};
use plugin_api::manifest::{DocAccess, ReadKind};
use plugin_api::textmap::{Apply, TextMap};
use plugin_api::wire::{
    DISABLE_AFTER_TIMEOUTS, INVOCATION_MS, MAX_BLOBS, MAX_FETCHES, MAX_ROUNDS, Message,
};
use serde_json::{Map, Value, json};

use crate::{Editor, GuestSession, Plugin, now_ms};

/// The marker of every time-limit failure (the wasm epoch stop and the invocation deadline): [`invoke`] counts these per plugin.
const TIMED_OUT: &str = "time budget exceeded";

thread_local! {
    static DEADLINE_MS: std::cell::Cell<f64> = const { std::cell::Cell::new(INVOCATION_MS as f64) };
}

/// Tests shorten the invocation deadline (ms) on their own thread.
#[doc(hidden)]
pub fn set_invocation_ms(ms: f64) {
    DEADLINE_MS.with(|d| d.set(ms));
}

fn fail<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn scope_name(s: ApiScope) -> &'static str {
    match s {
        ApiScope::Auto => "auto",
        ApiScope::Selection => "selection",
        ApiScope::Document => "document",
    }
}

/// What one invocation is.
pub struct Invocation {
    pub plugin: Arc<Plugin>,
    pub kind: CallKind,
    pub command: Option<String>,
    pub widget: Option<String>,
    pub params: Map<String, Value>,
    pub lang: String,
    /// The reads the first call carries (the command's `reads`).
    pub reads: Vec<ReadKind>,
    /// The caller is inside the slot command's `Session::execute`.
    pub in_command: bool,
    pub label: String,
}

pub(crate) struct Run<M> {
    pub(crate) inv: Invocation,
    last_map: Option<(TextMap, M)>,
    /// The revision the text map was read at.
    read_rev: u64,
    panels: Vec<(String, Vec<Widget>)>,
    status: Option<String>,
    applied: usize,
    rounds: usize,
    deadline_ms: f64,
    fetches: usize,
}

impl<M: 'static> Run<M> {
    pub(crate) fn new(inv: Invocation, rev: u64) -> Run<M> {
        Run {
            inv,
            last_map: None,
            read_rev: rev,
            panels: Vec::new(),
            status: None,
            applied: 0,
            rounds: 0,
            deadline_ms: now_ms() + DEADLINE_MS.with(|d| d.get()),
            fetches: 0,
        }
    }

    fn check_time(&self) -> Result<(), String> {
        if now_ms() > self.deadline_ms {
            fail(format!(
                "plugin stopped: invocation {TIMED_OUT} ({INVOCATION_MS} ms)"
            ))
        } else {
            Ok(())
        }
    }

    /// Perform one read: the blob goes into `blobs`, its index under the read's name in `reads`.
    fn read<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
        req: &ReadReq,
        blobs: &mut Vec<Vec<u8>>,
        reads: &mut BTreeMap<String, u32>,
    ) -> Result<u32, String> {
        // Every read goes through here (command `reads`, `next.reads`, the `read` action): none without `document = read` or `write`.
        if self.inv.plugin.manifest.permissions.document < DocAccess::Read {
            return fail(
                "plugin action refused: reading the document needs the document permission",
            );
        }
        // A message carries 16 blobs at most; refuse the 17th read before it walks the document.
        if blobs.len() >= MAX_BLOBS {
            return fail("plugin asked for more than 16 reads in one round");
        }
        let (name, bytes) = match req.what {
            ReadWhat::Textmap => {
                let scope = match req.scope {
                    Some(s) => scope_name(s).to_string(),
                    None => self
                        .inv
                        .params
                        .get("scope")
                        .and_then(Value::as_str)
                        .unwrap_or("auto")
                        .to_string(),
                };
                let (map, at) = ed.read_map(&scope)?;
                let bytes = map.encode().map_err(|e| e.to_string())?;
                self.read_rev = ed.rev();
                self.last_map = Some((map, at));
                ("textmap", bytes)
            }
            ReadWhat::Selection => ("selection", ed.selection_text().into_bytes()),
            ReadWhat::Paragraph => {
                let Some(i) = req.index else {
                    return fail("read paragraph needs an index");
                };
                let Some((map, _)) = &self.last_map else {
                    return fail("read paragraph needs a text map read in this invocation");
                };
                let Some(p) = map.paragraphs.iter().find(|p| p.index == i) else {
                    return fail(format!("paragraph {i} is not in the text map"));
                };
                (
                    "paragraph",
                    p.segments
                        .iter()
                        .map(|g| g.text.as_str())
                        .collect::<String>()
                        .into_bytes(),
                )
            }
        };
        let at = blobs.len() as u32;
        blobs.push(bytes);
        reads.insert(name.to_string(), at);
        Ok(at)
    }

    fn action<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
        a: &Action,
        answer_blobs: &[Vec<u8>],
        blobs: &mut Vec<Vec<u8>>,
        reads: &mut BTreeMap<String, u32>,
    ) -> Result<Value, String> {
        self.check_time()?;
        match a {
            Action::Run { id, params } => ed.run(id, params),
            Action::TextmapApply { blob } => {
                let Some((map, at)) = self.last_map.take() else {
                    return fail("textmap.apply without a text map read in this invocation");
                };
                if ed.rev() != self.read_rev {
                    return fail(
                        "the document changed since the text map was read (stale revision)",
                    );
                }
                let bytes = answer_blobs
                    .get(*blob as usize)
                    .ok_or("blob index out of range")?;
                let ap = Apply::decode(bytes).map_err(|e| format!("plugin answer refused: {e}"))?;
                ap.validate(&map)
                    .map_err(|e| format!("plugin answer refused: {e}"))?;
                let n = ed.apply_map(&map, &at, &ap)?;
                self.applied += n;
                // a later apply in this invocation reads again: the document has moved
                self.read_rev = ed.rev();
                Ok(json!({"paragraphs": n}))
            }
            Action::Read(req) => Ok(json!({"blob": self.read(ed, req, blobs, reads)?})),
            Action::Fetch {
                url,
                method,
                headers,
                body,
            } => {
                if self.fetches >= MAX_FETCHES {
                    return fail(format!(
                        "plugin asked for more than {MAX_FETCHES} fetches in one invocation"
                    ));
                }
                self.fetches += 1;
                match &self.inv.plugin.fetcher {
                    Some(f) => Ok(f
                        .fetch(url, method, headers, body.as_deref())
                        .unwrap_or_else(|e| json!({"error": e}))),
                    None => Ok(json!({"error": "network is not available in this host"})),
                }
            }
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

    /// Check one answer's actions against the plugin's grants and the app's allowlist.
    fn check(&self, app: &str, actions: &[Action]) -> Result<(), String> {
        let p = &self.inv.plugin;
        let Some(list) = plugin_api::allow::list(app) else {
            return fail(format!("no plugin allowlist for {app}"));
        };
        let panels: Vec<String> = p.manifest.panels.iter().map(|x| x.id.clone()).collect();
        let grants = Grants {
            document: p.manifest.permissions.document,
            origins: &p.origins,
            panels: &panels,
        };
        plugin_api::action::check_actions_in(
            list,
            actions,
            &grants,
            self.inv.kind == CallKind::Event,
        )
        .map_err(denied)
    }

    /// Committed only when every round succeeded.
    fn finish(&mut self) -> Value {
        let mut ui = self.inv.plugin.ui.lock().unwrap_or_else(|e| e.into_inner());
        for (id, widgets) in std::mem::take(&mut self.panels) {
            if widgets.is_empty() {
                ui.panels.remove(&id);
            } else {
                ui.panels.insert(id, widgets);
            }
        }
        ui.last_error = None;
        json!({"plugin": self.inv.plugin.manifest.id, "rounds": self.rounds, "paragraphs": self.applied, "status": self.status})
    }
}

fn denied(d: Denied) -> String {
    format!("plugin action refused: {d:?}")
}

/// One invocation between its rounds, for a guest that answers at once (natively).
struct Machine<M> {
    run: Run<M>,
    call: Call,
    guest: Box<dyn GuestSession>,
    blobs: Vec<Vec<u8>>,
}

impl<M: 'static> Machine<M> {
    fn begin<E: Editor<Map = M>>(
        ed: &mut E,
        inv: Invocation,
    ) -> Result<(Machine<M>, Message), String> {
        let p = inv.plugin.clone();
        let guest = p
            .guest
            .start()
            .map_err(|e| format!("plugin could not start: {e}"))?;
        let run = Run::new(inv, ed.rev());
        let call = Call {
            kind: run.inv.kind,
            api: 1,
            api_minor: plugin_api::API_MINOR,
            app: ed.app().into(),
            lang: run.inv.lang.clone(),
            command: run.inv.command.clone(),
            widget: run.inv.widget.clone(),
            event: None,
            params: run.inv.params.clone(),
            settings: p.settings.clone(),
            results: Vec::new(),
            reads: BTreeMap::new(),
        };
        let mut m = Machine {
            run,
            call,
            guest,
            blobs: Vec::new(),
        };
        for r in m.run.inv.reads.clone() {
            let what = match r {
                ReadKind::Textmap => ReadWhat::Textmap,
                ReadKind::Selection => ReadWhat::Selection,
                ReadKind::None => continue,
            };
            m.run.read(
                ed,
                &ReadReq {
                    what,
                    scope: None,
                    index: None,
                },
                &mut m.blobs,
                &mut m.call.reads,
            )?;
        }
        let msg = m.next_message()?;
        Ok((m, msg))
    }

    fn next_message(&mut self) -> Result<Message, String> {
        self.run.rounds += 1;
        Message::new(&self.call, std::mem::take(&mut self.blobs))
            .map_err(|e| format!("call refused: {e}"))
    }

    /// The plugin's answer to the last call: check it, run its actions, and either build the next call or finish.
    fn resume<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
        out: &Message,
    ) -> Result<Option<Message>, String> {
        let ans: Answer = out
            .header_as()
            .map_err(|e| format!("plugin answer refused: {e}"))?;
        self.run.check(ed.app(), &ans.actions)?;
        let (mut results, mut reads) = (Vec::with_capacity(ans.actions.len()), BTreeMap::new());
        for a in &ans.actions {
            results.push(
                self.run
                    .action(ed, a, &out.blobs, &mut self.blobs, &mut reads)?,
            );
        }
        let Some(next) = ans.next else {
            return Ok(None);
        };
        if self.run.rounds >= MAX_ROUNDS {
            return fail("plugin asked for more than 16 rounds");
        }
        for r in &next.reads {
            self.run.read(ed, r, &mut self.blobs, &mut reads)?;
        }
        self.call.kind = CallKind::Resume;
        self.call.results = results;
        self.call.reads = reads;
        self.call.params = Map::new();
        Ok(Some(self.next_message()?))
    }
}

fn rounds<E: Editor>(ed: &mut E, inv: Invocation) -> Result<Value, String> {
    let (mut m, mut msg) = Machine::begin(ed, inv)?;
    loop {
        m.run.check_time()?;
        let out = m
            .guest
            .round(&msg)
            .map_err(|e| format!("plugin stopped: {e}"))?;
        match m.resume(ed, &out)? {
            Some(next) => msg = next,
            None => return Ok(m.run.finish()),
        }
    }
}

/// Count a timeout; the third one in a session switches the plugin off. Returns the error to report.
pub(crate) fn note_failure(plugin: &Plugin, e: String) -> String {
    plugin.fail(&e);
    if e.contains(TIMED_OUT) && plugin.timed_out() >= DISABLE_AFTER_TIMEOUTS {
        crate::set_enabled(&plugin.manifest.id, false);
        return format!(
            "{e}; plugin {} was switched off for this session: it ran out of time {DISABLE_AFTER_TIMEOUTS} times",
            plugin.manifest.id
        );
    }
    e
}

/// Run one invocation. Any error puts the document back as it was before the first read. A plugin that lives in the page (the web
/// bridge) returns `{"pending": call}` at once and finishes later ([`crate::bridge`]).
pub fn invoke<E: Editor>(ed: &mut E, inv: Invocation) -> Result<Value, String> {
    // Owner decision 2026-11-06: plugins stay refused on protected documents (as in Docs).
    if inv.plugin.mutates()
        && let Some(why) = ed.protected()
    {
        return fail(why);
    }
    if inv.plugin.guest.remote() {
        return crate::bridge::start(ed, inv);
    }
    let Some(snap) = ed.snapshot() else {
        return fail("there is no document");
    };
    let (plugin, label, in_command) = (inv.plugin.clone(), inv.label.clone(), inv.in_command);
    match rounds(ed, inv) {
        Ok(v) => {
            ed.commit(&snap, &label, in_command);
            Ok(v)
        }
        Err(e) => {
            ed.restore(snap);
            Err(note_failure(&plugin, e))
        }
    }
}

/// A slot command ran: plugin command `command` of `p`, with the caller's params over the manifest's.
pub fn run_command<E: Editor>(
    ed: &mut E,
    p: &Arc<Plugin>,
    command: usize,
    v: &Value,
) -> Result<Value, String> {
    if crate::bridge::locked() {
        return fail(crate::bridge::BUSY);
    }
    if !p.enabled() {
        return fail("This plugin is not enabled");
    }
    let Some(c) = p.manifest.commands.get(command) else {
        return fail("unknown plugin command");
    };
    let mut params = c.params.clone();
    if let Some(o) = v.as_object() {
        params.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    let inv = Invocation {
        plugin: p.clone(),
        kind: CallKind::Command,
        command: Some(c.id.clone()),
        widget: None,
        params,
        lang: crate::lang(),
        reads: c.reads.clone(),
        in_command: true,
        label: crate::localized(&c.label, "en"),
    };
    invoke(ed, inv)
}

/// The editor's half of an invocation whose rounds the page runs (the web bridge): the page builds the calls and reads the answers;
/// the editor reads the document, runs the editor actions of each answer and keeps the text map and the revision.
pub(crate) struct Remote<M> {
    pub(crate) run: Run<M>,
}

impl<M: 'static> Remote<M> {
    pub(crate) fn new(rev: u64, inv: Invocation) -> Remote<M> {
        Remote {
            run: Run::new(inv, rev),
        }
    }

    /// `Session::execute` may touch the document after the slot command returned: the text map was read at this revision.
    pub(crate) fn rebase(&mut self, rev: u64) {
        self.run.read_rev = rev;
    }

    pub(crate) fn reads<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
        reqs: &[ReadReq],
    ) -> Result<Vec<Vec<u8>>, String> {
        let (mut blobs, mut names) = (Vec::new(), BTreeMap::new());
        for r in reqs {
            self.run.read(ed, r, &mut blobs, &mut names)?;
        }
        Ok(blobs)
    }

    /// The command's own reads (`reads` in the manifest), as (name, bytes).
    pub(crate) fn first_reads<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
    ) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
        let mut out = Vec::new();
        for r in self.run.inv.reads.clone() {
            let (name, what) = match r {
                ReadKind::Textmap => ("textmap", ReadWhat::Textmap),
                ReadKind::Selection => ("selection", ReadWhat::Selection),
                ReadKind::None => continue,
            };
            let mut b = self.reads(
                ed,
                &[ReadReq {
                    what,
                    scope: None,
                    index: None,
                }],
            )?;
            out.push((name, b.remove(0)));
        }
        Ok(out)
    }

    /// Check and run the actions of one answer; one result per action. `read` actions are not served: the page hands reads over as
    /// `next.reads`.
    pub(crate) fn actions<E: Editor<Map = M>>(
        &mut self,
        ed: &mut E,
        actions: &[Action],
        blobs: &[Vec<u8>],
    ) -> Result<Vec<Value>, String> {
        self.run.check(ed.app(), actions)?;
        let (mut results, mut sink, mut names) = (
            Vec::with_capacity(actions.len()),
            Vec::new(),
            BTreeMap::new(),
        );
        for a in actions {
            results.push(match a {
                Action::Read(_) => json!({"error": "ask for reads in `next.reads`"}),
                a => self.run.action(ed, a, blobs, &mut sink, &mut names)?,
            });
        }
        Ok(results)
    }

    pub(crate) fn finish(&mut self) -> Value {
        self.run.finish()
    }
}
