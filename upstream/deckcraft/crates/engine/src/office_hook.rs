// Added by the office command-hook patch. MIT OR Apache-2.0.
//! The command-hook seam (PLAN phase 4 section 4): a policy (XDFU restrictions, the snapshot after a save, Phase 5 permissions)
//! attaches here and sees every command a user action runs. Nothing is attached by default, and then `Session::execute`
//! behaves exactly as upstream's does.
//!
//! Only the outermost command of an action is offered to the hook: commands that run other commands (`depth > 0`) are one
//! action, so a policy sees `file.export`, not the steps inside it.

use std::sync::{Arc, RwLock};

use serde_json::Value;

use crate::{Result, Session};

/// What a policy can do.
pub trait CommandHook: Send + Sync {
    /// A reason to refuse `id`, or `None` to allow it. Asked before the command runs, and by the UI to grey a control out.
    /// `id` may also be a UI-level name that is no registry command (`file.print`, for the PDF a print opens).
    fn before(&self, id: &str, s: &Session) -> Option<String> {
        let _ = (id, s);
        None
    }
    /// The outcome of command `id` once it ran (a failed one too).
    fn after(&self, id: &str, s: &Session, params: &Value, result: &Result<Value>) {
        let _ = (id, s, params, result);
    }
}

static HOOK: RwLock<Option<Arc<dyn CommandHook>>> = RwLock::new(None);

/// Attach a policy hook, or with `None` remove it.
pub fn set_hook(hook: Option<Arc<dyn CommandHook>>) {
    *HOOK.write().unwrap_or_else(|e| e.into_inner()) = hook;
}

fn hook() -> Option<Arc<dyn CommandHook>> {
    HOOK.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Why the hook refuses `id` right now (`None`: allowed, or no hook).
pub fn refusal(id: &str, s: &Session) -> Option<String> {
    hook()?.before(id, s)
}

pub(crate) fn after(id: &str, s: &Session, params: &Value, result: &Result<Value>) {
    if let Some(h) = hook() {
        h.after(id, s, params, result);
    }
}
