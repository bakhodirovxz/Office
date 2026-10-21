//! The `Http` step (PLAN 2.3, WP5 step 5): one POST to the pdf2docx sidecar (docs/phase3/PLAN.md 3.3), its answer mapped to our
//! exit codes. The sidecar URL comes from `CONVERT_PDF2DOCX_URL` (like `CONVERT_*_CLI`); the client has no TLS and no proxy
//! (the sidecar sits on an internal network, and the environment is cleared on principle).
//!
//! Sidecar answer -> exit code:
//!   200                         ok; `X-Convert-Report` goes into the step report, its `warnings` become `sidecar` warnings
//!   400 bad_pdf / bad_request   3  the PDF cannot be read
//!   411                         1  our client always sends Content-Length, so this is a bug report
//!   413 too_large               9  over the sidecar's size limit
//!   422 encrypted | too_many_pages   9
//!   422 no_text_layer           1  scanned PDF: the message says OCR is not supported (PLAN WP5 done-check)
//!   503 busy                    retried with backoff (1, 2, 4 s), then 1
//!   504 timeout                 4
//!   500 worker_crashed          6
//!   connection refused / no answer   7  unreachable;  our own client timeout   4
use crate::exec::{Config, Ctl, Exit, Failure, StepReport, fail, validate_file};
use crate::plan::HttpStep;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// The attempts at a busy (503) sidecar: the first try plus three retries.
pub const ATTEMPTS: u32 = 4;

/// Map a sidecar error answer (`{"code","message"}`) to a failure. Pure, so the table is unit-tested.
pub fn map_error(status: u16, code: &str, message: &str) -> Failure {
    let (exit, what) = match (status, code) {
        (400, _) => (Exit::Unsupported, "the PDF cannot be read"),
        (411, _) => (Exit::ToolError, "the request was rejected (no Content-Length)"),
        (413, _) => (Exit::Rejected, "the PDF is over the PDF converter's size limit"),
        (422, "encrypted") => (Exit::Rejected, "the PDF is encrypted"),
        (422, "too_many_pages") => (Exit::Rejected, "the PDF has too many pages for the PDF converter"),
        (422, "no_text_layer") => (Exit::ToolError, "the PDF has no text layer"),
        (503, _) => (Exit::ToolError, "the PDF converter stayed busy"),
        (504, _) => (Exit::Timeout, "the PDF converter timed out"),
        (500, _) => (Exit::Crash, "the PDF converter's worker crashed"),
        _ => (Exit::ToolError, "the PDF converter failed"),
    };
    Failure { exit, msg: format!("{what} (pdf2docx sidecar {status} {code}): {message}") }
}

fn timeout(cfg: &Config, ctl: &Ctl) -> Duration {
    // the sidecar's own job timeout is 300 s; wait a little longer so its 504 arrives before our client gives up
    let t = Duration::from_secs(cfg.timeout_s.unwrap_or(330));
    ctl.remaining().map_or(t, |left| t.min(left))
}

/// Run one Http step. Returns the sidecar's `X-Convert-Report` JSON, if it sent one.
pub fn run_http(cfg: &Config, ctl: &Ctl, job: &Path, h: &HttpStep, steps: &mut Vec<StepReport>) -> Result<Option<serde_json::Value>, Failure> {
    let Some(base) = cfg.pdf2docx_url.as_deref() else {
        return fail(Exit::Missing, "the pdf2docx sidecar is not configured (set CONVERT_PDF2DOCX_URL, e.g. http://127.0.0.1:18080), or use --pdf-engine native");
    };
    let query = h.query.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
    let url = format!("{}{}{}{}", base.trim_end_matches('/'), h.route, if query.is_empty() { "" } else { "?" }, query);
    let body = std::fs::read(job.join(&h.input)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot read {}: {e}", h.input) })?;
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).proxy(None).timeout_global(Some(timeout(cfg, ctl))).build().into();
    let start = Instant::now();
    let note = |status: i64, text: String, steps: &mut Vec<StepReport>| {
        steps.push(StepReport { tool: "pdf2docx".into(), exit: status, ms: start.elapsed().as_millis() as u64, peak_mb: 0, stderr_tail: text.chars().take(2048).collect() });
    };
    // shortcut: a blocking request is not interrupted by a cancel; the job deadline caps it, and cancel is checked between attempts
    for attempt in 0..ATTEMPTS {
        ctl.check()?;
        let mut resp = match agent.post(&url).header("Content-Type", "application/pdf").send(&body[..]) {
            Ok(r) => r,
            Err(e) => {
                note(-1, e.to_string(), steps);
                let timed_out = matches!(e, ureq::Error::Timeout(_));
                return fail(if timed_out { Exit::Timeout } else { Exit::Missing }, format!("the pdf2docx sidecar at {base} did not answer: {e}"));
            }
        };
        let status = resp.status().as_u16();
        let header = |n: &str| resp.headers().get(n).and_then(|v| v.to_str().ok()).map(String::from);
        let report_header = header("x-convert-report");
        let bytes = resp.body_mut().with_config().limit(1 << 30).read_to_vec().map_err(|e| Failure { exit: Exit::Missing, msg: format!("the pdf2docx sidecar answer was cut off: {e}") })?;
        if status == 200 {
            let report = report_header.as_deref().and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok());
            note(200, report_header.unwrap_or_default(), steps);
            let out = job.join(&h.output);
            std::fs::File::create(&out).and_then(|mut f| f.write_all(&bytes)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot write {}: {e}", h.output) })?;
            validate_file(&out, false).map_err(|m| Failure { exit: Exit::BadOutput, msg: format!("pdf2docx sidecar output rejected: {m}") })?;
            return Ok(report);
        }
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        let (code, message) = (v["code"].as_str().unwrap_or("").to_string(), v["message"].as_str().map(String::from).unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(200).collect()));
        note(status as i64, format!("{code}: {message}"), steps);
        if status == 503 && attempt + 1 < ATTEMPTS {
            std::thread::sleep(Duration::from_millis(cfg.http_backoff_ms << attempt));
            continue;
        }
        return Err(map_error(status, &code, &message));
    }
    unreachable!("the loop returns on its last attempt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_table() {
        let e = |s, c| map_error(s, c, "m").exit;
        assert_eq!(e(400, "bad_pdf"), Exit::Unsupported);
        assert_eq!(e(411, "length_required"), Exit::ToolError);
        assert_eq!(e(413, "too_large"), Exit::Rejected);
        assert_eq!(e(422, "encrypted"), Exit::Rejected);
        assert_eq!(e(422, "too_many_pages"), Exit::Rejected);
        assert_eq!(e(422, "no_text_layer"), Exit::ToolError);
        assert_eq!(e(503, "busy"), Exit::ToolError);
        assert_eq!(e(504, "timeout"), Exit::Timeout);
        assert_eq!(e(500, "worker_crashed"), Exit::Crash);
        assert_eq!(e(418, "x"), Exit::ToolError);
    }
}
