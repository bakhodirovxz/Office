// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! One seam on `Session::execute` for the office layer: a hook that sees every command before it runs (and may refuse it)
//! and after it ran. Nothing is installed by default, and without a hook `execute` takes exactly its former path.
//! The policy (XDFU) and the version snapshot on save (docs/phase4/PLAN.md section 4) are written in our own crates and
//! installed once at start-up; this file only carries the call.

use std::sync::OnceLock;

use serde_json::Value as Json;

use crate::{Result, Session};

/// Installed with [`install`]. Both methods run on the thread that executes the command, so keep them quick.
pub trait Hook: Send + Sync {
    /// Called before the command runs. `Err(reason)` refuses it: the caller gets `EngineError::Disabled(id, reason)`.
    fn before(&self, _session: &Session, _id: &str, _params: &Json) -> std::result::Result<(), String> {
        Ok(())
    }
    /// Called after the command ran (also when it failed), with its result.
    fn after(&self, _session: &Session, _id: &str, _params: &Json, _result: &Result<Json>) {}
}

static HOOK: OnceLock<Box<dyn Hook>> = OnceLock::new();

/// Installs the process-wide hook. Returns `false` (and drops `hook`) when one is already installed.
pub fn install(hook: Box<dyn Hook>) -> bool {
    HOOK.set(hook).is_ok()
}

pub(crate) fn installed() -> Option<&'static dyn Hook> {
    HOOK.get().map(|h| &**h)
}
