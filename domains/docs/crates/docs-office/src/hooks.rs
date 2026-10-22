//! The command-hook seam (PLAN section 4, F2): XDFU restrictions, the snapshot after a save and Phase 5 permissions attach here.
//! Nothing is attached in Phase 4 WP4; WP10 and WP11 call `set_hook`.
//!
//! The guarded ids are re-registered with wrappers. `Registry::new` keeps the last spec of an id, so the wrapper replaces
//! upstream's handler, and the original `enabled` and `run` pointers are kept in a table the wrappers index by a const
//! generic (a `fn` pointer cannot capture them). With no hook set a wrapper only calls the original, so behaviour is unchanged.

use std::sync::{Arc, OnceLock, RwLock};

use serde_json::Value;
use wordcraft_engine::{CmdResult, CommandSpec, Registry, Session};

/// What a policy can do. `before` runs inside the command's `enabled`, so the UI greys the control out and `Session::run`
/// refuses with the reason. `after` sees the outcome of a command that ran (also a failed one).
pub trait CommandHook: Send + Sync {
    fn before(&self, id: &str, s: &Session) -> Option<&'static str> {
        let _ = (id, s);
        None
    }
    fn after(&self, id: &str, s: &Session, params: &Value, result: &CmdResult) {
        let _ = (id, s, params, result);
    }
}

static HOOK: RwLock<Option<Arc<dyn CommandHook>>> = RwLock::new(None);

/// Attach (or, with `None`, remove) the policy hook.
pub fn set_hook(h: Option<Arc<dyn CommandHook>>) {
    *HOOK.write().unwrap_or_else(|e| e.into_inner()) = h;
}

fn hook() -> Option<Arc<dyn CommandHook>> {
    HOOK.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The ids a policy can refuse or observe. Registered ids only: a name upstream drops is skipped (and the test notices).
pub const GUARDED: [&str; 9] = [
    "file.print",
    "file.printPdf",
    "file.exportPdf",
    "file.exportPng",
    "file.saveAs",
    "file.save",
    "edit.copy",
    "edit.cut",
    "file.saveTemplate",
];

type Original = (fn(&Session) -> Option<&'static str>, fn(&mut Session, &Value) -> CmdResult);
static ORIGINALS: OnceLock<[Option<Original>; GUARDED.len()]> = OnceLock::new();

fn enabled<const N: usize>(s: &Session) -> Option<&'static str> {
    let (orig, _) = ORIGINALS.get()?[N]?;
    orig(s).or_else(|| hook().and_then(|h| h.before(GUARDED[N], s)))
}

fn run<const N: usize>(s: &mut Session, v: &Value) -> CmdResult {
    let Some((_, orig)) = ORIGINALS.get().and_then(|o| o[N]) else { return Err(wordcraft_engine::CmdError::Unknown(GUARDED[N].into())) };
    let result = orig(s, v);
    if let Some(h) = hook() {
        h.after(GUARDED[N], s, v, &result);
    }
    result
}

macro_rules! wrappers {
    ($($n:literal),*) => { [$(($n, enabled::<$n> as fn(&Session) -> Option<&'static str>, run::<$n> as fn(&mut Session, &Value) -> CmdResult)),*] };
}

/// The wrapped copies of the guarded commands `reg` has. The first call records the originals; they are upstream's
/// handlers, identical for every session, so a later call (a second session) sees the same table.
pub fn wrapped(reg: &Registry) -> Vec<CommandSpec> {
    let table = wrappers!(0, 1, 2, 3, 4, 5, 6, 7, 8);
    ORIGINALS.get_or_init(|| std::array::from_fn(|i| reg.get(GUARDED[i]).map(|c| (c.enabled, c.run))));
    table
        .iter()
        .filter_map(|(n, en, rn)| {
            reg.get(GUARDED[*n]).map(|c| CommandSpec { enabled: *en, run: *rn, ..c.clone() })
        })
        .collect()
}
