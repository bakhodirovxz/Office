//! The editor side of the web plugin bridge (Phase 7 WP4; docs/phase7/PLAN.md section 4.6, docs/plugins.md sections 9 and 9a).
//!
//! On the web the plugin runs in a Worker owned by the page, and the page (`web/plugins/host.js`) builds the calls, reads the answers,
//! does `ui.*` and `fetch`, and measures the Worker's time budget. The editor does what only it can: read the document, run the
//! editor actions (`run`, `textmap.apply`) with the allowlist and the grants, keep the undo step, the revision and the snapshot.
//!
//! A plugin registered from the page has a guest that [`Guest::remote`] marks; [`crate::exec::invoke`] hands such an invocation to
//! [`start`], which keeps it and returns `{"pending": call}` at once. [`answer`] takes the page's `pluginActions`, [`tick`] (once per
//! frame) ends invocations that got no answer in time or whose document was replaced, [`take_out`] hands out what is for the page.
//!
//! While a command or a panel click is pending the document is locked for input with Restrict Editing's read-only protection (the
//! engine's own lock: every command and keystroke goes through `Session::run`), lifted while an answer's actions run. Any failure,
//! also a timeout or an abort, restores the snapshot taken at the start, so a failed plugin leaves the document as it was.
//!
//! This module has no JS in it; `docs-office::plugin_web` turns the browser's messages into these calls.
use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use plugin_api::action::{Action, CallKind, ReadReq};
use plugin_api::manifest::Manifest;
use plugin_api::wire::Message;
use serde_json::{Map, Value, json};
use wordcraft_engine::{CmdError, CmdResult, EditSnapshot, Session};

use crate::exec::{Invocation, Remote};
use crate::{Guest, GuestSession, Plugin};

/// How long the editor waits for the page between two messages of an invocation: the Worker's load (10 s) and one round (5 s) with
/// room to spare. A page that sends `timeoutMs` overrides it.
pub const WAIT_MS: f64 = 20000.0;

/// Refusals the page shows in its own language (`plg.busy`, `plg.protected` in i18n/plugins.tsv; `web/plugins/host.js` maps the text).
pub const BUSY: &str = "another plugin is still running";
pub const PROTECTED: &str = "the document is protected";

/// How many ended string-call ids are remembered, so a late round of one cannot start a fresh invocation.
const ENDED_MAX: usize = 64;

/// A plugin that lives in the page. Nothing runs here: `start` only hands out a session that refuses to be called.
pub struct PageGuest;

struct PageSession;

impl Guest for PageGuest {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        Ok(Box::new(PageSession))
    }
    fn remote(&self) -> bool {
        true
    }
}

impl GuestSession for PageSession {
    fn round(&mut self, _: &Message) -> Result<Message, String> {
        Err("a plugin in the page is called through the bridge".into())
    }
}

/// What the editor tells the page. `call` is the invocation's id as the page knows it: a number for the ones the editor starts, a text
/// (`h<n>`) for the ones the page starts (panel clicks).
pub enum Out {
    /// A plugin command ran: the page calls the plugin. `reads` are the command's reads by name (the text map is transferred, not JSON).
    Call { call: Value, plugin: String, command: String, params: Map<String, Value>, reads: Vec<(&'static str, Vec<u8>)> },
    /// The answer to `pluginActions`: one result per action, one buffer per requested read; `error` set: the invocation failed and the
    /// document is as it was.
    Results { call: Value, results: Vec<Value>, reads: Vec<Vec<u8>>, error: Option<String> },
    /// An event of a plugin that listens (`selectionChanged`, `documentOpened`, `documentSaved`).
    Event { event: String },
}

struct Pending {
    call: Value,
    remote: Remote,
    snap: Option<EditSnapshot>,
    doc: u64,
    /// The revision after the last answer's actions: if the document moved since, the next answer is stale.
    rev: u64,
    started: f64,
    /// The `Call` message waits for the next tick: the revision settles when `Session::run` has returned.
    first: Option<Out>,
}

#[derive(Default)]
struct Bridge {
    next: u64,
    pending: Vec<Pending>,
    out: Vec<Out>,
    timeout_ms: f64,
    /// Ids of the plugins the page registered.
    web: BTreeSet<String>,
    /// We set the read-only protection, so we lift it.
    lock_ours: bool,
    /// Ids of the page's (string) calls that ended or were refused, oldest first.
    ended: VecDeque<Value>,
}

thread_local! {
    static BRIDGE: RefCell<Bridge> = RefCell::new(Bridge::default());
}

fn with<R>(f: impl FnOnce(&mut Bridge) -> R) -> R {
    BRIDGE.with(|b| f(&mut b.borrow_mut()))
}

fn lock(s: &mut Session) {
    if s.doc.settings.protection.is_none() {
        s.doc.settings.protection = Some("readOnly".into());
        with(|b| b.lock_ours = true);
        wordcraft_engine::io::set_save_filter(Some(without_lock));
    }
}

/// Every save, autosave and recovery snapshot goes through `io::save_bytes_opts`, which calls this: our temporary protection is never
/// written. The lock is only ever put on a document that had no protection, so the user's own protection is never touched.
fn without_lock(d: &wordcraft_doc::Document) -> Option<wordcraft_doc::Document> {
    if !with(|b| b.lock_ours) || d.settings.protection.as_deref() != Some("readOnly") {
        return None;
    }
    let mut d = d.clone();
    d.settings.protection = None;
    Some(d)
}

fn unlock(s: &mut Session) {
    if with(|b| std::mem::take(&mut b.lock_ours)) && s.doc.settings.protection.as_deref() == Some("readOnly") {
        s.doc.settings.protection = None;
    }
}

/// A command or a panel click is waiting for the page: input is locked.
pub fn locked() -> bool {
    with(|b| !b.pending.is_empty())
}

fn now() -> f64 {
    wordcraft_layout::now_ms()
}

fn fail<T>(s: impl Into<String>) -> Result<T, CmdError> {
    Err(CmdError::Failed(s.into()))
}

/// Begin an invocation of a plugin that lives in the page. A command returns `{"pending": call}` and the call goes out on the next
/// [`tick`]; an event is queued for the page; a panel click is the page's own business (it sends `pluginActions` when it needs the editor).
pub fn start(s: &mut Session, inv: Invocation) -> CmdResult {
    match inv.kind {
        CallKind::Event => {
            if let Some(e) = inv.event {
                with(|b| {
                    if !b.out.iter().any(|o| matches!(o, Out::Event { event } if event == e.name())) {
                        b.out.push(Out::Event { event: e.name().to_string() });
                    }
                });
            }
            Ok(Value::Null)
        }
        CallKind::Command => {
            if with(|b| !b.pending.is_empty()) {
                return fail(BUSY);
            }
            // Our lock only goes on a document without protection; with the user's own (comments, forms, tracked changes) the user could
            // edit during the call and a stale restore would lose it.
            if s.doc.settings.protection.is_some() {
                return fail(PROTECTED);
            }
            let snap = s.edit_snapshot();
            let (command, params, plugin) = (inv.command.clone().unwrap_or_default(), inv.params.clone(), inv.plugin.manifest.id.clone());
            let doc = s.document_id();
            let mut remote = Remote::new(s, inv);
            let reads = match remote.first_reads(s) {
                Ok(r) => r,
                Err(e) => {
                    s.restore(snap);
                    return Err(e);
                }
            };
            lock(s);
            let call = with(|b| {
                b.next += 1;
                let call = json!(b.next);
                let first = Some(Out::Call { call: call.clone(), plugin, command, params, reads });
                b.pending.push(Pending { call: call.clone(), remote, snap: Some(snap), doc, rev: s.rev(), started: now(), first });
                call
            });
            Ok(json!({"pending": call}))
        }
        _ => Ok(Value::Null),
    }
}

/// End `p` with a failure: lift the lock and put the document back. `status` is for the user (an abort comes from a page that said it).
fn fail_pending(s: &mut Session, p: Pending, why: &str, replaced: bool) {
    end(&p.call);
    if replaced {
        // The document the invocation was for is gone; the new one was never locked by us.
        with(|b| b.lock_ours = false);
    } else {
        unlock(s);
        if let Some(snap) = p.snap {
            s.restore(snap);
        }
    }
    p.remote.plugin().ui.lock().unwrap_or_else(|e| e.into_inner()).last_error = Some(why.to_string());
    s.status = format!("Plugin stopped: {why}");
}

/// Remember that a page call is over.
fn end(call: &Value) {
    if call.is_string() {
        with(|b| {
            if b.ended.len() >= ENDED_MAX {
                b.ended.pop_front();
            }
            b.ended.push_back(call.clone());
        });
    }
}

fn take(call: &Value) -> Option<Pending> {
    with(|b| b.pending.iter().position(|p| &p.call == call).map(|i| b.pending.remove(i)))
}

/// What the page asked for in `pluginActions`: the actions of the answer (the page keeps `ui.*` and `fetch`), its blobs, the reads the
/// next call needs, whether another round follows, and whether to give up. `plugin` names the plugin of an invocation the page started.
pub struct Answer {
    pub call: Value,
    pub plugin: Option<String>,
    pub actions: Value,
    pub blobs: Vec<Vec<u8>>,
    pub reads: Value,
    pub more: bool,
    pub abort: bool,
}

fn answer_error(call: Value, e: String) {
    with(|b| b.out.push(Out::Results { call, results: Vec::new(), reads: Vec::new(), error: Some(e) }));
}

/// The page's `pluginActions`. An unknown number is an invocation that already ended (timed out, replaced): answered with an error so the
/// page stops. An unknown text is a panel click's first forwarded round: it starts the invocation (snapshot, undo step).
pub fn answer(s: &mut Session, a: Answer) {
    let known = take(&a.call);
    if a.abort {
        if let Some(p) = known {
            fail_pending(s, p, "the page stopped the plugin", false);
        }
        return;
    }
    let mut p = match known {
        Some(p) => p,
        None if a.call.is_string() => {
            let why = if with(|b| b.ended.contains(&a.call)) {
                Some("the invocation has ended")
            } else if with(|b| !b.pending.is_empty()) {
                Some(BUSY)
            } else if s.doc.settings.protection.is_some() {
                Some(PROTECTED)
            } else {
                None
            };
            if let Some(why) = why {
                end(&a.call);
                s.status = format!("Plugin not started: {why}");
                return answer_error(a.call, why.into());
            }
            let plugin = a.plugin.as_deref().and_then(|id| crate::plugins().into_iter().find(|p| p.manifest.id == id && p.guest.remote()));
            let Some(plugin) = plugin else { return answer_error(a.call, "unknown plugin".into()) };
            let inv = Invocation {
                plugin: plugin.clone(),
                kind: CallKind::Ui,
                command: None,
                widget: None,
                event: None,
                params: Map::new(),
                lang: crate::lang(),
                reads: Vec::new(),
                expect_rev: None,
                in_command: false,
                label: crate::localized(&plugin.manifest.name, "en"),
            };
            Pending { call: a.call.clone(), remote: Remote::new(s, inv), snap: Some(s.edit_snapshot()), doc: s.document_id(), rev: s.rev(), started: now(), first: None }
        }
        None => return answer_error(a.call, "the invocation has ended".into()),
    };
    unlock(s);
    let r = (|| {
        if s.document_id() != p.doc {
            return fail("the document was replaced");
        }
        if s.rev() != p.rev {
            return fail("the document changed while the plugin ran (stale revision)");
        }
        let actions: Vec<Action> = serde_json::from_value(a.actions.clone()).map_err(|e| CmdError::Failed(format!("plugin answer refused: {e}")))?;
        let reads: Vec<ReadReq> = if a.reads.is_null() { Vec::new() } else { serde_json::from_value(a.reads.clone()).map_err(|e| CmdError::Failed(format!("plugin answer refused: {e}")))? };
        let results = p.remote.actions(s, &actions, &a.blobs)?;
        Ok((results, p.remote.reads(s, &reads)?))
    })();
    match r {
        Ok((results, reads)) => {
            if !a.more {
                end(&a.call);
            }
            if a.more {
                lock(s);
                p.rev = s.rev();
                p.started = now();
                with(|b| b.pending.push(p));
            }
            with(|b| b.out.push(Out::Results { call: a.call, results, reads, error: None }));
        }
        Err(e) => {
            let why = e.to_string();
            fail_pending(s, p, &why, false);
            answer_error(a.call, why);
        }
    }
}

/// Once per frame: announce new invocations, end the ones that got no answer in time or whose document was replaced. True while any is
/// pending (the caller keeps asking for frames so a timeout fires without input).
pub fn tick(s: &mut Session) -> bool {
    if with(|b| b.pending.is_empty()) {
        return false;
    }
    let (doc, rev, t) = (s.document_id(), s.rev(), now());
    // `Session::run` touches the document after the slot command returned: the revision settles here.
    with(|b| {
        for p in &mut b.pending {
            if let Some(o) = p.first.take() {
                p.remote.rebase(rev);
                p.rev = rev;
                p.started = t;
                b.out.push(o);
            }
        }
    });
    let timeout = with(|b| if b.timeout_ms > 0.0 { b.timeout_ms } else { WAIT_MS });
    let dead: Vec<(Value, bool)> = with(|b| b.pending.iter().filter(|p| p.doc != doc || t - p.started > timeout).map(|p| (p.call.clone(), p.doc != doc)).collect());
    for (call, replaced) in dead {
        if let Some(p) = take(&call) {
            fail_pending(s, p, if replaced { "the document was replaced" } else { "no answer in time" }, replaced);
        }
    }
    with(|b| !b.pending.is_empty())
}

/// The messages for the page since the last call.
pub fn take_out() -> Vec<Out> {
    with(|b| std::mem::take(&mut b.out))
}

/// One plugin the page registers: its manifest (the descriptor's JSON), the settings in force, whether it is enabled.
pub struct Registration {
    pub manifest: Value,
    pub settings: Option<Map<String, Value>>,
    pub enabled: bool,
}

/// Make the plugins in the page exactly `list`: new ones are added, ones no longer listed are removed (an empty list unregisters
/// all). Returns `(ids registered, one message per refused entry)`. Refused entries are skipped, the rest still register.
pub fn set_plugins(list: Vec<Registration>, timeout_ms: Option<f64>) -> (Vec<String>, Vec<String>) {
    let (mut ids, mut errors) = (Vec::new(), Vec::new());
    for (i, r) in list.into_iter().enumerate() {
        let m: Manifest = match serde_json::from_value(r.manifest) {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("plugin {i}: {e}"));
                continue;
            }
        };
        let id = m.id.clone();
        let mut p = Plugin::new(m, Arc::new(PageGuest));
        if let Some(st) = r.settings {
            p.settings.extend(st);
        }
        if let Err(e) = crate::add(p) {
            errors.push(format!("{id}: {e}"));
            continue;
        }
        crate::set_enabled(&id, r.enabled);
        ids.push(id);
    }
    let gone: Vec<String> = with(|b| {
        if let Some(t) = timeout_ms.filter(|t| *t > 0.0) {
            b.timeout_ms = t;
        }
        let gone = b.web.difference(&ids.iter().cloned().collect()).cloned().collect();
        b.web = ids.iter().cloned().collect();
        gone
    });
    for id in gone {
        crate::remove(&id);
    }
    (ids, errors)
}

/// Test hook: forget pending invocations, queued messages and registrations.
pub fn reset() {
    BRIDGE.with(|b| *b.borrow_mut() = Bridge::default());
}
