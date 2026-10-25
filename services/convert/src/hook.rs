//! The job hook seam (WP15 XDFU handling, WP7 audit and encryption). A hook sees every job twice: `before` any tool runs
//! (it can refuse, e.g. WP15 `refuse` mode, and add warnings) and `after` the outputs validated (it can rewrite them, add
//! warnings or veto). After the hooks, the outputs are listed again and validated again, so a watermark that writes a new file
//! next to the original can never leave the unmarked original as the result, and a hook that corrupts a file is exit 8.
//! `finish` closes the job for the hook (done or failed) and may hand back a JSON object for the report. No hook is installed by default.
//!
//! A hook that needs a domain CLI (WP15 reads the first page of a PDF and stamps documents) runs it through the `ToolRunner` it is
//! given, which applies the same cleared environment, timeout, memory watchdog, containment and cancel as a conversion step.
use crate::exec::Failure;
use crate::kind::Kind;
use crate::plan::{Options, ToolId};
use crate::report::Warning;
use std::path::PathBuf;

/// What a hook may know about a job. `principal` and `client` come from the caller (the API: the authenticated user and
/// the client address; the CLI: both `None`).
#[derive(Clone, Debug)]
pub struct JobCtx {
    pub job_id: String,
    pub job_dir: PathBuf,
    /// the first input inside the job directory (`in/input.<ext>`); a job with several inputs (PDF combine) has them all in `in/`
    pub input: PathBuf,
    pub kind: Option<Kind>,
    pub target: String,
    pub opts: Options,
    pub principal: Option<String>,
    pub client: Option<String>,
    /// the input as the report names it (the CLI: the path as typed; the API: the sanitised upload name)
    pub label: String,
    /// the caller's own id for this file (the API: `<job id>/<file number>`)
    pub reference: Option<String>,
}

/// Runs one domain CLI in the job directory (the tool's working directory, so paths are relative to it) and returns its stdout.
/// `input_mb` scales the timeout like a conversion step's does.
pub trait ToolRunner {
    fn run(&self, tool: ToolId, argv: &[String], input_mb: f64) -> Result<Vec<u8>, Failure>;
}

pub trait JobHook: Send + Sync {
    /// Before any tool runs. `Err` ends the job with that failure; the warnings join the report.
    fn before(&self, _ctx: &JobCtx, _tools: &dyn ToolRunner) -> Result<Vec<Warning>, Failure> {
        Ok(vec![])
    }
    /// After every step succeeded and every output validated, before the outputs leave the job directory. `outputs` are the
    /// files in `out/`; the hook may rewrite them in place or add files there.
    fn after(
        &self,
        _ctx: &JobCtx,
        _outputs: &[PathBuf],
        _tools: &dyn ToolRunner,
    ) -> Result<Vec<Warning>, Failure> {
        Ok(vec![])
    }
    /// The job's input carries the grif, or could not be checked for it: XDFU rules apply to it (no sidecar fallback).
    fn flagged(&self, _ctx: &JobCtx) -> bool {
        false
    }
    /// The job is over, whatever its result. The hook forgets the job; what it returns goes into the report as `xdfu`.
    fn finish(&self, _ctx: &JobCtx) -> Option<serde_json::Value> {
        None
    }
}
