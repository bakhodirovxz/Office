//! The editor side of the web plugin bridge for Sheets and Slides (Phase 7 WP13; the protocol of `docs-plugins::bridge`,
//! docs/plugins.md sections 9 and 9a).
//!
//! On the web the plugin runs in a Worker owned by the page (`web/plugins/host.js`, shared by every editor), which builds the calls,
//! reads the answers, does `ui.*` and `fetch` and measures the Worker's time budget. The editor does what only it can: read the
//! document, run the editor actions with the allowlist and the grants, keep the undo step, the revision and the snapshot.
//!
//! While a command or a panel click is pending, input is locked: [`input_locked`] is true and the engine crate's command hook refuses
//! every command that changes the document (and `file.*`, `edit.*`, `plugin.*`). The lock is lifted while an answer's actions run.
//! Any failure, also a timeout or an abort, restores the snapshot taken at the start, so a failed plugin leaves the document as it was.
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use plugin_api::action::{Action, CallKind, ReadReq};
use plugin_api::manifest::Manifest;
use plugin_api::wire::Message;
use serde_json::{Map, Value, json};

use crate::exec::{Invocation, Remote};
use crate::{Editor, Guest, GuestSession, Plugin, now_ms};

/// How long the editor waits for the page between two messages of an invocation (the Worker's load and one round with room to
/// spare). A page that sends `timeoutMs` overrides it.
pub const WAIT_MS: f64 = 20000.0;

/// Refusals the page shows in its own language (`plg.busy`, `plg.protected`; `web/plugins/host.js` maps the text).
pub const BUSY: &str = "another plugin is still running";
pub const PROTECTED: &str = "the document is protected";

const ENDED_MAX: usize = 64;

/// A plugin that lives in the page. Nothing runs here.
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

/// What the editor tells the page. `call` is a number for the invocations the editor starts, a text (`h<n>`) for the page's own.
pub enum Out {
    Call {
        call: Value,
        plugin: String,
        command: String,
        params: Map<String, Value>,
        reads: Vec<(&'static str, Vec<u8>)>,
    },
    Results {
        call: Value,
        results: Vec<Value>,
        reads: Vec<Vec<u8>>,
        error: Option<String>,
    },
}

/// The engine half of a pending invocation: `Remote<E::Map>` and the snapshot, behind `Any` (one editor type per process).
struct Inner<S, M> {
    remote: Remote<M>,
    snap: S,
}

struct Pending {
    call: Value,
    plugin: Arc<Plugin>,
    doc: u64,
    /// The revision after the last answer's actions: if the document moved since, the next answer is stale.
    rev: u64,
    started: f64,
    first: Option<Out>,
    label: String,
    inner: Box<dyn Any>,
}

#[derive(Default)]
struct Bridge {
    next: u64,
    pending: Vec<Pending>,
    out: Vec<Out>,
    timeout_ms: f64,
    web: BTreeSet<String>,
    ended: VecDeque<Value>,
    /// Status lines for the editor's own UI (a failure the page may not hear of: its own timeout).
    status: Vec<String>,
}

thread_local! {
    static BRIDGE: RefCell<Bridge> = RefCell::new(Bridge::default());
}

/// The input lock the engines' command hooks read (a hook may be called from any thread, so not the thread-local).
static LOCK: AtomicBool = AtomicBool::new(false);

fn with<R>(f: impl FnOnce(&mut Bridge) -> R) -> R {
    BRIDGE.with(|b| f(&mut b.borrow_mut()))
}

fn relock() {
    LOCK.store(with(|b| !b.pending.is_empty()), Ordering::Relaxed);
}

/// A command or a panel click is waiting for the page.
pub fn locked() -> bool {
    with(|b| !b.pending.is_empty())
}

/// The engine's command hook refuses changes while this is true.
pub fn input_locked() -> bool {
    LOCK.load(Ordering::Relaxed)
}

/// Whether the lock refuses command `id` (`changes`: the engine's spec says it changes the document, `undoable`).
pub fn refuses(id: &str, changes: bool) -> bool {
    input_locked()
        && (changes
            || ["file.", "edit.", "plugin.", "office."]
                .iter()
                .any(|p| id.starts_with(p)))
}

fn fail<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

/// Begin an invocation of a plugin that lives in the page: a command returns `{"pending": call}` and the call goes out on the next
/// [`tick`]. The caller ([`crate::exec::invoke`]) has refused a protected document already.
pub fn start<E: Editor>(ed: &mut E, inv: Invocation) -> Result<Value, String> {
    if inv.kind != CallKind::Command {
        return Ok(Value::Null);
    }
    if locked() {
        return fail(BUSY);
    }
    let (Some(doc), Some(snap)) = (ed.doc_id(), ed.snapshot()) else {
        return fail("there is no document");
    };
    let (command, params, plugin, label) = (
        inv.command.clone().unwrap_or_default(),
        inv.params.clone(),
        inv.plugin.clone(),
        inv.label.clone(),
    );
    let mut remote: Remote<E::Map> = Remote::new(ed.rev(), inv);
    let reads = match remote.first_reads(ed) {
        Ok(r) => r,
        Err(e) => {
            ed.restore(snap);
            return Err(e);
        }
    };
    let call = with(|b| {
        b.next += 1;
        let call = json!(b.next);
        let first = Some(Out::Call {
            call: call.clone(),
            plugin: plugin.manifest.id.clone(),
            command,
            params,
            reads,
        });
        let inner: Box<dyn Any> = Box::new(Inner { remote, snap });
        b.pending.push(Pending {
            call: call.clone(),
            plugin,
            doc,
            rev: ed.rev(),
            started: now_ms(),
            first,
            label,
            inner,
        });
        call
    });
    relock();
    Ok(json!({"pending": call}))
}

fn inner<E: Editor>(p: &mut Pending) -> &mut Inner<E::Snap, E::Map> {
    #[allow(clippy::expect_used)]
    // one editor type per process: a pending invocation was made by the same `E`
    p.inner
        .downcast_mut::<Inner<E::Snap, E::Map>>()
        .expect("one editor type per process")
}

/// End `p` with a failure and put the document back (unless it was replaced).
fn fail_pending<E: Editor>(ed: &mut E, p: Pending, why: &str, replaced: bool) {
    end(&p.call);
    let why = crate::exec::note_failure(&p.plugin, why.to_string());
    if !replaced && let Ok(i) = p.inner.downcast::<Inner<E::Snap, E::Map>>() {
        ed.restore(i.snap);
    }
    with(|b| b.status.push(format!("Plugin stopped: {why}")));
    relock();
}

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
    let p = with(|b| {
        b.pending
            .iter()
            .position(|p| &p.call == call)
            .map(|i| b.pending.remove(i))
    });
    relock();
    p
}

/// What the page asked for in `pluginActions` (see `docs-plugins::bridge::Answer`).
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
    with(|b| {
        b.out.push(Out::Results {
            call,
            results: Vec::new(),
            reads: Vec::new(),
            error: Some(e),
        })
    });
}

/// The page's `pluginActions`. An unknown number is an invocation that already ended: answered with an error. An unknown text is a
/// panel click's first forwarded round: it starts the invocation (snapshot, undo step).
pub fn answer<E: Editor>(ed: &mut E, a: Answer) {
    let known = take(&a.call);
    if a.abort {
        if let Some(p) = known {
            fail_pending(ed, p, "the page stopped the plugin", false);
        }
        return;
    }
    let mut p = match known {
        Some(p) => p,
        None if a.call.is_string() => {
            let plugin = a.plugin.as_deref().and_then(|id| {
                crate::plugins()
                    .into_iter()
                    .find(|p| p.manifest.id == id && p.guest.remote())
            });
            let why = if with(|b| b.ended.contains(&a.call)) {
                Some("the invocation has ended")
            } else if locked() {
                Some(BUSY)
            } else if plugin.as_ref().is_some_and(|p| p.mutates()) && ed.protected().is_some() {
                Some(PROTECTED)
            } else {
                None
            };
            if let Some(why) = why {
                end(&a.call);
                with(|b| b.status.push(format!("Plugin not started: {why}")));
                return answer_error(a.call, why.into());
            }
            let Some(plugin) = plugin else {
                return answer_error(a.call, "unknown plugin".into());
            };
            let (Some(doc), Some(snap)) = (ed.doc_id(), ed.snapshot()) else {
                return answer_error(a.call, "there is no document".into());
            };
            let label = crate::localized(&plugin.manifest.name, "en");
            let inv = Invocation {
                plugin: plugin.clone(),
                kind: CallKind::Ui,
                command: None,
                widget: None,
                params: Map::new(),
                lang: crate::lang(),
                reads: Vec::new(),
                in_command: false,
                label: label.clone(),
            };
            let inner: Box<dyn Any> = Box::new(Inner {
                remote: Remote::<E::Map>::new(ed.rev(), inv),
                snap,
            });
            Pending {
                call: a.call.clone(),
                plugin,
                doc,
                rev: ed.rev(),
                started: now_ms(),
                first: None,
                label,
                inner,
            }
        }
        None => return answer_error(a.call, "the invocation has ended".into()),
    };
    let r = (|| {
        if ed.doc_id() != Some(p.doc) {
            return fail("the document was replaced");
        }
        if ed.rev() != p.rev {
            return fail("the document changed while the plugin ran (stale revision)");
        }
        let actions: Vec<Action> = serde_json::from_value(a.actions.clone())
            .map_err(|e| format!("plugin answer refused: {e}"))?;
        let reads: Vec<ReadReq> = if a.reads.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(a.reads.clone())
                .map_err(|e| format!("plugin answer refused: {e}"))?
        };
        let i = inner::<E>(&mut p);
        let results = i.remote.actions(ed, &actions, &a.blobs)?;
        Ok((results, i.remote.reads(ed, &reads)?))
    })();
    match r {
        Ok((results, reads)) => {
            if a.more {
                p.rev = ed.rev();
                p.started = now_ms();
                with(|b| b.pending.push(p));
                relock();
            } else {
                end(&a.call);
                let i = inner::<E>(&mut p);
                let _ = i.remote.finish();
                let label = std::mem::take(&mut p.label);
                if let Ok(i) = p.inner.downcast::<Inner<E::Snap, E::Map>>() {
                    ed.commit(&i.snap, &label, false);
                }
            }
            with(|b| {
                b.out.push(Out::Results {
                    call: a.call,
                    results,
                    reads,
                    error: None,
                })
            });
        }
        Err(e) => {
            fail_pending(ed, p, &e, false);
            answer_error(a.call, e);
        }
    }
}

/// Once per frame: announce new invocations, end the ones that got no answer in time or whose document was replaced. True while any
/// is pending (the caller keeps asking for frames so a timeout fires without input).
pub fn tick<E: Editor>(ed: &mut E) -> bool {
    if !locked() {
        return false;
    }
    let (doc, rev, t) = (ed.doc_id(), ed.rev(), now_ms());
    let first: Vec<Value> = with(|b| {
        b.pending
            .iter()
            .filter(|p| p.first.is_some())
            .map(|p| p.call.clone())
            .collect()
    });
    for call in first {
        if let Some(mut p) = take(&call) {
            inner::<E>(&mut p).remote.rebase(rev);
            p.rev = rev;
            p.started = t;
            let o = p.first.take();
            with(|b| {
                b.out.extend(o);
                b.pending.push(p);
            });
            relock();
        }
    }
    let timeout = with(|b| {
        if b.timeout_ms > 0.0 {
            b.timeout_ms
        } else {
            WAIT_MS
        }
    });
    let dead: Vec<(Value, bool)> = with(|b| {
        b.pending
            .iter()
            .filter(|p| Some(p.doc) != doc || t - p.started > timeout)
            .map(|p| (p.call.clone(), Some(p.doc) != doc))
            .collect()
    });
    for (call, replaced) in dead {
        if let Some(p) = take(&call) {
            fail_pending(
                ed,
                p,
                if replaced {
                    "the document was replaced"
                } else {
                    "no answer in time"
                },
                replaced,
            );
        }
    }
    locked()
}

/// The messages for the page since the last call.
pub fn take_out() -> Vec<Out> {
    with(|b| std::mem::take(&mut b.out))
}

/// Status lines for the editor's own UI since the last call.
pub fn take_status() -> Vec<String> {
    with(|b| std::mem::take(&mut b.status))
}

/// One plugin the page registers: its manifest (the descriptor's JSON), the settings in force, whether it is enabled.
pub struct Registration {
    pub manifest: Value,
    pub settings: Option<Map<String, Value>>,
    pub enabled: bool,
}

/// Make the plugins in the page exactly `list` (an empty list unregisters all). Returns `(ids registered, one message per refused
/// entry)`. The engine crate binds and registers the new slots afterwards.
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
        let gone = b
            .web
            .difference(&ids.iter().cloned().collect())
            .cloned()
            .collect();
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
    LOCK.store(false, Ordering::Relaxed);
}
