//! The job hook seam (WP15 XDFU handling, WP7 audit and encryption). A hook sees every job twice: `before` any tool runs
//! (it can refuse, e.g. WP15 `refuse` mode, and add warnings) and `after` the outputs validated (it can rewrite them, add
//! warnings or veto). After the hooks, the outputs are listed again and validated again, so a watermark that writes a new file
//! next to the original can never leave the unmarked original as the result, and a hook that corrupts a file is exit 8.
//! No hook is installed by default.
use crate::exec::Failure;
use crate::kind::Kind;
use crate::plan::Options;
use crate::report::Warning;
use std::path::PathBuf;

/// What a hook may know about a job. `principal` and `client` come from the caller (the API: the authenticated user and
/// the client address; the CLI: both `None`).
#[derive(Clone, Debug)]
pub struct JobCtx {
    pub job_id: String,
    pub job_dir: PathBuf,
    /// the input inside the job directory (`in/input.<ext>`); empty for the PDF subcommands with several inputs
    pub input: PathBuf,
    pub kind: Option<Kind>,
    pub target: String,
    pub opts: Options,
    pub principal: Option<String>,
    pub client: Option<String>,
}

pub trait JobHook: Send + Sync {
    /// Before any tool runs. `Err` ends the job with that failure; the warnings join the report.
    fn before(&self, _ctx: &JobCtx) -> Result<Vec<Warning>, Failure> {
        Ok(vec![])
    }
    /// After every step succeeded and every output validated, before the outputs leave the job directory. `outputs` are the
    /// files in `out/`; the hook may rewrite them in place or add files there.
    fn after(&self, _ctx: &JobCtx, _outputs: &[PathBuf]) -> Result<Vec<Warning>, Failure> {
        Ok(vec![])
    }
}
