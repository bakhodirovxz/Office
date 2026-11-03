//! The web side of the plugin bridge (Phase 7 WP4; docs/phase7/PLAN.md section 4.6, docs/plugins.md "The web bridge").
//!
//! On the web the plugin runs in a Worker owned by the page, so a round trip is asynchronous while the executor is not. A plugin
//! registered from the page has a guest that [`Guest::remote`] marks; [`crate::exec::invoke`] hands such an invocation to [`start`],
//! which keeps the [`Machine`] and returns `{"pending": call}` at once. Messages for the page are queued in [`take_out`]; the page's
//! answer comes in through [`answer`]; [`tick`] (once per frame) finishes invocations that timed out or whose document was replaced.
//!
//! While a command or a panel click is pending the document is locked for input with Restrict Editing's read-only protection (the
//! engine's own lock: every command and keystroke goes through `Session::run`), lifted while the answer's actions run. Any failure,
//! also a timeout, restores the snapshot taken at the start, so a failed plugin leaves the document as it was. Event invocations
//! (`ui.*` actions only) neither lock nor snapshot.
//!
//! This module has no JS in it; `docs-office::plugin_web` turns the browser's messages into these calls.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use plugin_api::action::CallKind;
use plugin_api::wire::Message;
use serde_json::{Map, Value, json};
use wordcraft_engine::{CmdError, CmdResult, EditSnapshot, Session};

use crate::exec::{Invocation, Machine, Step};
use crate::{Guest, GuestSession, Plugin};

/// How long the page gets for one round before the editor gives up (the page's own Worker budget is 5 s, PLAN section 4.2).
pub const ROUND_TIMEOUT_MS: f64 = 7000.0;

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

/// What the editor tells the page.
pub enum Out {
    /// A call for the plugin: the header is the call's JSON, the blobs (the text map) are transferred, not copied into JSON.
    Call { call: u64, plugin: String, rev: u64, round: u32, header: String, blobs: Vec<Vec<u8>> },
    /// The invocation is over (`error` set: it failed, nothing changed; the page ends the Worker round).
    Done { call: u64, plugin: String, error: Option<String> },
}

struct Pending {
    call: u64,
    machine: Machine,
    snap: Option<EditSnapshot>,
    doc: u64,
    /// The revision after the last round's actions: if the document moved since, the answer is stale.
    rev: u64,
    /// The revision named in the last `Out::Call`; the page echoes it.
    sent_rev: u64,
    round: u32,
    started: f64,
    /// Not yet announced to the page (the revision settles when `Session::run` has returned).
    first: Option<Message>,
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
}

thread_local! {
    static BRIDGE: RefCell<Bridge> = RefCell::new(Bridge::default());
}

fn with<R>(f: impl FnOnce(&mut Bridge) -> R) -> R {
    BRIDGE.with(|b| f(&mut b.borrow_mut()))
}

fn locking(kind: CallKind) -> bool {
    kind != CallKind::Event
}

fn lock(s: &mut Session) {
    if s.doc.settings.protection.is_none() {
        s.doc.settings.protection = Some("readOnly".into());
        with(|b| b.lock_ours = true);
    }
}

fn unlock(s: &mut Session) {
    if with(|b| std::mem::take(&mut b.lock_ours)) && s.doc.settings.protection.as_deref() == Some("readOnly") {
        s.doc.settings.protection = None;
    }
}

/// A command or a panel click is waiting for the page: input is locked.
pub fn locked() -> bool {
    with(|b| b.pending.iter().any(|p| locking(p.machine.kind())))
}

fn now() -> f64 {
    wordcraft_layout::now_ms()
}

/// Begin an invocation of a plugin that lives in the page. Returns `{"pending": call}`; the first call goes out on the next [`tick`].
pub fn start(s: &mut Session, inv: Invocation) -> CmdResult {
    let lock_it = locking(inv.kind);
    if with(|b| {
        b.pending.iter().any(|p| {
            if lock_it {
                locking(p.machine.kind())
            } else {
                // An event is not queued behind itself: the plugin has not answered the last one yet.
                p.machine.event() == inv.event && Arc::ptr_eq(p.machine.plugin(), &inv.plugin)
            }
        })
    }) {
        return if lock_it { Err(CmdError::Failed("another plugin is still running".into())) } else { Ok(Value::Null) };
    }
    let snap = lock_it.then(|| s.edit_snapshot());
    let doc = s.document_id();
    let (machine, msg) = match Machine::begin(s, inv) {
        Ok(x) => x,
        Err(e) => {
            if let Some(snap) = snap {
                s.restore(snap);
            }
            return Err(e);
        }
    };
    if lock_it {
        lock(s);
    }
    let call = with(|b| {
        b.next += 1;
        let call = b.next;
        b.pending.push(Pending { call, machine, snap, doc, rev: s.rev(), sent_rev: 0, round: 0, started: now(), first: Some(msg) });
        call
    });
    Ok(json!({"pending": call}))
}

fn announce(b: &mut Bridge, p: &mut Pending, msg: Message, rev: u64) {
    p.round += 1;
    p.rev = rev;
    p.sent_rev = rev;
    p.started = now();
    b.out.push(Out::Call { call: p.call, plugin: p.machine.plugin().manifest.id.clone(), rev, round: p.round, header: msg.header, blobs: msg.blobs });
}

/// End `p` with a failure: lift the lock, put the document back and tell the page.
fn fail(s: &mut Session, p: Pending, why: &str, replaced: bool) {
    let plugin = p.machine.plugin().clone();
    if replaced {
        // The document the invocation was for is gone; the new one was never locked by us.
        with(|b| b.lock_ours = false);
    } else {
        unlock(s);
        if let Some(snap) = p.snap {
            s.restore(snap);
        }
    }
    plugin.ui.lock().unwrap_or_else(|e| e.into_inner()).last_error = Some(why.to_string());
    if locking(p.machine.kind()) {
        s.status = format!("Plugin stopped: {why}");
    }
    with(|b| b.out.push(Out::Done { call: p.call, plugin: plugin.manifest.id.clone(), error: Some(why.to_string()) }));
}

/// The page's answer to a call: the plugin's answer message, or why there is none (a trap, a refused message, the Worker's own
/// time budget). `rev` is the revision the call named, echoed. A `call` that is not pending (finished, timed out) is ignored.
pub fn answer(s: &mut Session, call: u64, rev: Option<u64>, result: Result<Message, String>) {
    let Some(mut p) = with(|b| b.pending.iter().position(|p| p.call == call).map(|i| b.pending.remove(i))) else { return };
    if locking(p.machine.kind()) {
        unlock(s);
    }
    let step = match result {
        Err(e) => Err(CmdError::Failed(format!("plugin stopped: {e}"))),
        Ok(_) if rev.is_some_and(|r| r != p.sent_rev) => Err(CmdError::Failed("the answer belongs to another revision (stale)".into())),
        Ok(_) if s.document_id() != p.doc => Err(CmdError::Failed("the document was replaced".into())),
        Ok(_) if s.rev() != p.rev => Err(CmdError::Failed("the document changed while the plugin ran (stale revision)".into())),
        Ok(m) => p.machine.resume(s, &m),
    };
    match step {
        Ok(Step::Call(msg)) => {
            if locking(p.machine.kind()) {
                lock(s);
            }
            let rev = s.rev();
            with(|b| {
                let mut p = p;
                announce(b, &mut p, msg, rev);
                b.pending.push(p);
            });
        }
        Ok(Step::Done(_)) => {
            let id = p.machine.plugin().manifest.id.clone();
            with(|b| b.out.push(Out::Done { call: p.call, plugin: id, error: None }));
        }
        Err(e) => {
            let why = e.to_string();
            fail(s, p, &why, false);
        }
    }
}

/// Once per frame: announce new invocations, end the ones that timed out or whose document was replaced. True while any is pending
/// (the caller keeps asking for frames so a timeout fires without input).
pub fn tick(s: &mut Session) -> bool {
    if with(|b| b.pending.is_empty()) {
        return false;
    }
    let (doc, rev, t) = (s.document_id(), s.rev(), now());
    // `Session::run` touches the document after the slot command returned: the revision settles here.
    with(|b| {
        let mut all = std::mem::take(&mut b.pending);
        for p in &mut all {
            if let Some(msg) = p.first.take() {
                p.machine.rebase(rev);
                announce(b, p, msg, rev);
            }
        }
        b.pending = all;
    });
    let timeout = with(|b| if b.timeout_ms > 0.0 { b.timeout_ms } else { ROUND_TIMEOUT_MS });
    let dead: Vec<(u64, bool)> = with(|b| b.pending.iter().filter(|p| p.doc != doc || t - p.started > timeout).map(|p| (p.call, p.doc != doc)).collect());
    for (call, replaced) in dead {
        if let Some(p) = with(|b| b.pending.iter().position(|p| p.call == call).map(|i| b.pending.remove(i))) {
            fail(s, p, if replaced { "the document was replaced" } else { "no answer in time" }, replaced);
        }
    }
    with(|b| !b.pending.is_empty())
}

/// The messages for the page since the last call.
pub fn take_out() -> Vec<Out> {
    with(|b| std::mem::take(&mut b.out))
}

/// One plugin the page registers: its `plugin.toml`, the settings the user changed, the network origins that were granted.
pub struct Remote {
    pub manifest: String,
    pub settings: Option<Map<String, Value>>,
    pub origins: Vec<String>,
    pub enabled: bool,
}

/// Make the plugins in the page exactly `list`: new ones are added, ones no longer listed are removed (an empty list unregisters
/// all). Returns `(ids registered, one message per refused entry)`. Refused entries are skipped, the rest still register.
pub fn set_plugins(list: Vec<Remote>, timeout_ms: Option<f64>) -> (Vec<String>, Vec<String>) {
    let (mut ids, mut errors) = (Vec::new(), Vec::new());
    for (i, r) in list.into_iter().enumerate() {
        let m = match plugin_api::manifest::parse(&r.manifest) {
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
        p.origins = r.origins;
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
