//! The executor: one subprocess per tool step, a cleared environment, a timeout, a memory watchdog and output validation
//! (PLAN 2.3-2.6). Never fails silently: every failure is a `Failure` with an exit code and a human-readable message.
use crate::hook::JobHook;
use crate::plan::{Outputs, ToolId, ToolStep};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(into = "u8")]
pub enum Exit {
    Ok = 0,
    ToolError = 1,
    Usage = 2,
    Unsupported = 3,
    Timeout = 4,
    Memory = 5,
    Crash = 6,
    Missing = 7,
    BadOutput = 8,
    Rejected = 9,
    Warnings = 10,
}

impl From<Exit> for u8 {
    fn from(e: Exit) -> u8 {
        e as u8
    }
}

impl Exit {
    pub fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone)]
pub struct Failure {
    pub exit: Exit,
    pub msg: String,
}

pub fn fail<T>(exit: Exit, msg: impl Into<String>) -> Result<T, Failure> {
    Err(Failure {
        exit,
        msg: msg.into(),
    })
}

/// What a step costs, for the timeout table. Derived from the tool and its first argument (`class_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Docs,
    SheetsConvert,
    SheetsPdf,
    Slides,
    Pdf,
    PdfNativeDocx,
}

/// One place for the per-class limits: timeout = base + per_mb x input MB + per_page x pages, capped at 15 min.
/// Values are WP1's census defaults (docs/phase3/census.md: 3x the measured p95, rounded); memory caps are PLAN 2.5.
/// pdf2docx and soffice keep the plan values (60 s + 3 s/page, 120 s) in their own steps when those sidecars land.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub base_s: f64,
    pub per_mb_s: f64,
    pub per_page_s: f64,
    pub mem_mb: u64,
}

pub const TIMEOUT_CAP_S: u64 = 15 * 60;
/// Census proposed 10 s; on a loaded 12-core box a 0.2 s docx->pdf still tripped it (WP3/4 runs), so 30 s.
const BASE_S: f64 = 30.0;

pub fn limits(c: Class) -> Limits {
    let l = |per_mb_s, per_page_s, mem_gb: u64| Limits {
        base_s: BASE_S,
        per_mb_s,
        per_page_s,
        mem_mb: mem_gb * 1024,
    };
    match c {
        Class::Docs => l(25.0, 0.0, 3),
        Class::SheetsConvert => l(3.0, 0.0, 6),
        Class::SheetsPdf => l(7.0, 0.0, 6),
        Class::Slides => l(10.0, 0.0, 3),
        Class::Pdf => l(3.0, 0.5, 2),
        Class::PdfNativeDocx => l(0.0, 1.0, 2),
    }
}

/// Put a freshly spawned tool into a Job Object with kill-on-close. `win32job` wraps the Win32 calls safely, so our crates keep
/// `unsafe_code = "forbid"`. The crate offers no per-process commit limit (only working-set limits, which page instead of
/// kill), so the memory cap stays with the watchdog. `shortcut: if the OS refuses (a parent job without nesting), the
/// watchdog alone applies and the tool can outlive us; the step still runs.`
#[cfg(windows)]
fn contain(child: &std::process::Child) -> Option<win32job::Job> {
    use std::os::windows::io::AsRawHandle;
    let job = win32job::Job::create().ok()?;
    let mut info = job.query_extended_limit_info().ok()?;
    info.limit_kill_on_job_close();
    job.set_extended_limit_info(&info).ok()?;
    job.assign_process(child.as_raw_handle() as isize).ok()?;
    Some(job)
}

#[cfg(not(windows))]
fn contain(_child: &std::process::Child) -> Option<()> {
    None
}

/// `sheets-cli run` is the pdf export (and translit); `pdf-cli run` is the native pdf to docx script.
pub fn class_of(tool: ToolId, argv: &[String]) -> Class {
    let first = argv.first().map(String::as_str);
    match (tool, first) {
        (ToolId::Docs, _) => Class::Docs,
        (ToolId::Sheets, Some("run")) => Class::SheetsPdf,
        (ToolId::Sheets, _) => Class::SheetsConvert,
        (ToolId::Slides, _) => Class::Slides,
        (ToolId::Pdf, Some("run")) => Class::PdfNativeDocx,
        (ToolId::Pdf, _) => Class::Pdf,
    }
}

/// `pages == 0` means unknown; the per-page classes then assume 100 pages (`shortcut: generous, tighten when the pdf info
/// of the input is read before the step`).
pub fn timeout_for(c: Class, input_mb: f64, pages: u32) -> Duration {
    let l = limits(c);
    let pages = if pages == 0 && c == Class::PdfNativeDocx {
        100
    } else {
        pages
    };
    Duration::from_secs(
        ((l.base_s + l.per_mb_s * input_mb + l.per_page_s * pages as f64) as u64)
            .min(TIMEOUT_CAP_S),
    )
}

/// Known stderr patterns -> our exit code and message. Every domain CLI exits 1 for every error (census), so the text is the
/// only signal. First match wins; anything unknown stays exit 1 with the raw stderr. Patterns are lowercase substrings.
pub const STDERR_PATTERNS: &[(&str, Exit, &str)] = &[
    (
        "not a presentation",
        Exit::Unsupported,
        "the presentation cannot be read (legacy .ppt? re-save it as .pptx in PowerPoint)",
    ),
    (
        "no sheet named",
        Exit::Usage,
        "that sheet does not exist in the workbook",
    ),
    // exact upstream phrases (gridcraft io.rs PASSWORD_REQUIRED / WRONG_PASSWORD, deckcraft cmd/file.rs, craft-libs print): a bare
    // "password" would also match gridcraft's "The sheet is protected with a password", which is not an input problem
    (
        "is password-protected",
        Exit::Rejected,
        "the input is password-protected",
    ),
    (
        "password is incorrect",
        Exit::Rejected,
        "the input is password-protected",
    ),
    (
        "password required",
        Exit::Rejected,
        "the input is password-protected",
    ),
    ("it is encrypted", Exit::Rejected, "the input is encrypted"),
    (
        "not a readable pdf",
        Exit::Unsupported,
        "the input is not a readable PDF",
    ),
    (
        "unknown output format",
        Exit::Unsupported,
        "that output format is not supported by the tool",
    ),
];

pub fn classify(stderr: &str) -> Option<(Exit, &'static str)> {
    // `Warning:` lines are not the failure
    let low = stderr
        .lines()
        .filter(|l| !l.trim_start().to_ascii_lowercase().starts_with("warning:"))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    STDERR_PATTERNS
        .iter()
        .find(|(p, _, _)| low.contains(p))
        .map(|(_, e, m)| (*e, *m))
}

/// Cancellation and the overall job deadline (PLAN 5.1 `DELETE /jobs/{id}`, 5.3 job timeout). `run_tool` polls it, and
/// `run_plan` checks it between steps. Both end the job with exit 4.
#[derive(Clone, Default)]
pub struct Ctl {
    pub cancel: Option<Arc<AtomicBool>>,
    pub deadline: Option<Instant>,
}

impl Ctl {
    pub fn check(&self) -> Result<(), Failure> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return fail(Exit::Timeout, "the job was cancelled");
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return fail(Exit::Timeout, "the job exceeded its overall time limit");
        }
        Ok(())
    }
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }
}

pub struct Config {
    /// explicit tool paths; missing entries resolve through `CONVERT_<TOOL>_CLI`, `CONVERT_TOOLS_DIR`, then next to this exe
    pub tools: HashMap<ToolId, PathBuf>,
    pub work_dir: PathBuf,
    pub keep_work: bool,
    pub timeout_s: Option<u64>,
    pub max_mem_mb: Option<u64>,
    pub max_input_mb: u64,
    /// `--verify`: re-open every output with its own CLI (`info`) before it leaves the job directory
    pub verify: bool,
    /// the pdf2docx sidecar (`CONVERT_PDF2DOCX_URL`), e.g. `http://127.0.0.1:18080`
    pub pdf2docx_url: Option<String>,
    /// first wait after a busy (503) sidecar; doubles on each retry
    pub http_backoff_ms: u64,
    /// extra variables passed through (fonts); the rest of the environment is cleared
    pub pass_env: Vec<(String, String)>,
    /// Hooks around every job (WP15 XDFU, WP7 audit); none by default. See `hook.rs`.
    pub hooks: Vec<Arc<dyn JobHook>>,
    pub sample_ms: u64,
    /// overall time limit of one job (PLAN 5.3: 30 min)
    pub job_timeout_s: u64,
    /// stops every job of this process (a signal handler or the API sets it); a per-job flag goes into `JobMeta`
    pub cancel: Option<Arc<AtomicBool>>,
    /// Local mode: tools see the real `%LOCALAPPDATA%`, so per-user fonts (`Microsoft\Windows\Fonts`) are found like in a
    /// plain CLI run. Server mode (`CONVERT_LOCALAPPDATA=isolated`): it points into the job directory and fonts come from
    /// `CRAFT_FONTS_DIR` or the image only.
    pub share_localappdata: bool,
    /// limits that keep a document from fanning out into thousands of processes (checked by the expand steps, exit 9)
    pub caps: crate::plan::Caps,
}

impl Config {
    pub fn from_env() -> Config {
        let pass = ["CRAFT_FONTS_DIR", "GRIDCRAFT_PDF_SYSTEM_FONTS"];
        Config {
            tools: HashMap::new(),
            work_dir: std::env::var_os("CONVERT_WORK_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("convert-work")),
            keep_work: false,
            timeout_s: None,
            max_mem_mb: None,
            max_input_mb: 512,
            verify: false,
            pdf2docx_url: std::env::var("CONVERT_PDF2DOCX_URL")
                .ok()
                .filter(|u| !u.is_empty()),
            http_backoff_ms: 1000,
            pass_env: pass
                .iter()
                .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
                .collect(),
            hooks: vec![],
            sample_ms: 100,
            job_timeout_s: 30 * 60,
            cancel: None,
            share_localappdata: std::env::var("CONVERT_LOCALAPPDATA")
                .map_or(true, |v| v != "isolated"),
            caps: crate::plan::Caps::default(),
        }
    }

    pub fn resolve(&self, tool: ToolId) -> Result<PathBuf, Failure> {
        let exe = format!("{}{}", tool.name(), std::env::consts::EXE_SUFFIX);
        let var = format!(
            "CONVERT_{}_CLI",
            tool.name().trim_end_matches("-cli").to_uppercase()
        );
        let mut tried: Vec<PathBuf> = vec![];
        let mut cands: Vec<PathBuf> = vec![];
        if let Some(p) = self.tools.get(&tool) {
            cands.push(p.clone());
        }
        if let Some(p) = std::env::var_os(&var) {
            cands.push(p.into());
        }
        if let Some(d) = std::env::var_os("CONVERT_TOOLS_DIR") {
            cands.push(PathBuf::from(d).join(&exe));
        }
        if let Some(d) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            cands.push(d.join(&exe));
        }
        for c in cands {
            if c.is_file() {
                return Ok(c);
            }
            tried.push(c);
        }
        fail(
            Exit::Missing,
            format!(
                "{} was not found (set {var} or CONVERT_TOOLS_DIR; looked at: {})",
                tool.name(),
                tried
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepReport {
    pub tool: String,
    pub exit: i64,
    pub ms: u64,
    pub peak_mb: u64,
    pub stderr_tail: String,
}

pub struct Ran {
    pub stdout: Vec<u8>,
    /// stderr lines that start with `Warning:` (any case; F8), verbatim
    pub warnings: Vec<String>,
}

fn drain<R: Read + Send + 'static>(mut r: R, cap: usize) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut keep = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            if keep.len() < cap {
                keep.extend_from_slice(&buf[..n.min(cap - keep.len())]);
            }
        }
        keep
    })
}

fn tail(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        return t.to_string();
    }
    let mut i = t.len() - max;
    while !t.is_char_boundary(i) {
        i += 1;
    }
    t[i..].to_string()
}

fn last_line(s: &str) -> String {
    // a Rust panic prints "thread .. panicked at ..:" and then the message; the last line is the RUST_BACKTRACE note
    if let Some(m) = s
        .lines()
        .position(|l| l.contains("panicked at"))
        .and_then(|i| {
            s.lines()
                .skip(i + 1)
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with("note:"))
        })
    {
        return m.chars().take(300).collect();
    }
    s.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no message")
        .chars()
        .take(300)
        .collect()
}

/// Run one tool process in `job_dir` (its cwd). `stdout_to` streams stdout into a file instead of memory. The step report
/// is pushed to `steps` whether the step succeeds or not.
#[allow(clippy::too_many_arguments)] // shortcut: one call shape for every step; fold into a struct when a 9th arrives
pub fn run_tool(
    cfg: &Config,
    ctl: &Ctl,
    tool: ToolId,
    argv: &[String],
    job_dir: &Path,
    stdout_to: Option<&Path>,
    input_mb: f64,
    pages: u32,
    steps: &mut Vec<StepReport>,
) -> Result<Ran, Failure> {
    let exe = cfg.resolve(tool)?;
    let name = tool.name();
    let class = class_of(tool, argv);
    ctl.check()?;
    let mut timeout = cfg
        .timeout_s
        .map(Duration::from_secs)
        .unwrap_or_else(|| timeout_for(class, input_mb, pages));
    let mut deadline_hit = false;
    if let Some(left) = ctl.remaining().filter(|l| *l < timeout) {
        timeout = left;
        deadline_hit = true;
    }
    let mem_cap = cfg.max_mem_mb.unwrap_or(limits(class).mem_mb);

    let abs = |p: &str| job_dir.join(p);
    for d in ["tmp", "home"] {
        let _ = std::fs::create_dir_all(abs(d));
    }
    let mut cmd = Command::new(&exe);
    cmd.args(argv)
        .current_dir(job_dir)
        .env_clear()
        .stdin(Stdio::null());
    // PLAN 2.4: only these variables reach the tool
    let tools_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    cmd.env("PATH", &tools_dir)
        .env("TEMP", abs("tmp"))
        .env("TMP", abs("tmp"))
        .env("TMPDIR", abs("tmp"))
        .env("HOME", abs("home"))
        .env("USERPROFILE", abs("home"))
        .env("APPDATA", abs("home"));
    // per-user fonts live under the real %LOCALAPPDATA% (wordcraft fontdb.rs, gridcraft pdf_fonts.rs); see Config::share_localappdata
    match std::env::var_os("LOCALAPPDATA").filter(|_| cfg.share_localappdata) {
        Some(real) => cmd.env("LOCALAPPDATA", real),
        None => cmd.env("LOCALAPPDATA", abs("home")),
    };
    cmd.env("LANG", "uz_UZ.UTF-8")
        .env("LC_ALL", "uz_UZ.UTF-8")
        .env("LC_PAPER", "uz_UZ.UTF-8")
        .env("WORDCRAFT_NO_PREFS", "1");
    for k in ["SystemRoot", "windir"] {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
        }
    }
    for (k, v) in &cfg.pass_env {
        cmd.env(k, v);
    }
    match stdout_to {
        Some(p) => {
            let f = File::create(p).map_err(|e| Failure {
                exit: Exit::ToolError,
                msg: format!("cannot create {}: {e}", p.display()),
            })?;
            cmd.stdout(Stdio::from(f));
        }
        None => {
            cmd.stdout(Stdio::piped());
        }
    }
    cmd.stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| Failure {
        exit: Exit::Missing,
        msg: format!("{name} could not be started ({}): {e}", exe.display()),
    })?;
    let out_t = child.stdout.take().map(|s| drain(s, 8 * 1024 * 1024));
    let err_t = child.stderr.take().map(|s| drain(s, 1024 * 1024));

    // Windows: the tool runs in a Job Object that kills it when we die or drop the job.
    let _job = contain(&child);
    let pid = Pid::from_u32(child.id());
    let mut sys = System::new();
    let mut peak: u64 = 0;
    let mut last_sample: Option<Instant> = None;
    let mut verdict: Option<Failure> = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {}
            Err(e) => {
                verdict = Some(Failure {
                    exit: Exit::Crash,
                    msg: format!("{name}: cannot wait for the process: {e}"),
                });
                break None;
            }
        }
        if let Err(f) = ctl.check() {
            verdict = Some(f);
            break None;
        }
        if start.elapsed() > timeout {
            let why = if deadline_hit {
                "the job ran out of time while"
            } else {
                ""
            };
            verdict = Some(Failure {
                exit: Exit::Timeout,
                msg: format!(
                    "{why} {name} did not finish within {} s and was stopped",
                    timeout.as_secs()
                )
                .trim()
                .to_string(),
            });
            break None;
        }
        if last_sample.is_none_or(|t| t.elapsed() >= Duration::from_millis(cfg.sample_ms)) {
            last_sample = Some(Instant::now());
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_memory(),
            );
            if let Some(p) = sys.process(pid) {
                // commit (private bytes), not the working set: Windows trims working sets under pressure, so that reads low
                // exactly when the box is short of memory
                let used = p.memory().max(p.virtual_memory());
                peak = peak.max(used);
                if used / (1024 * 1024) > mem_cap {
                    verdict = Some(Failure {
                        exit: Exit::Memory,
                        msg: format!(
                            "{name} used more than {mem_cap} MB of memory (saw {} MB) and was stopped",
                            used / (1024 * 1024)
                        ),
                    });
                    break None;
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    drop(_job);
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = out_t
        .map(|t| t.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = String::from_utf8_lossy(
        &err_t
            .map(|t| t.join().unwrap_or_default())
            .unwrap_or_default(),
    )
    .into_owned();
    let ms = start.elapsed().as_millis() as u64;
    let peak_mb = peak / (1024 * 1024);
    let exit_num = status.and_then(|s| s.code()).map(i64::from).unwrap_or(-1);
    let report = StepReport {
        tool: name.to_string(),
        exit: exit_num,
        ms,
        peak_mb,
        stderr_tail: tail(&stderr, 2048),
    };
    steps.push(report);
    if let Some(v) = verdict {
        return Err(v);
    }
    let st = status.expect("status is set when no verdict");
    match st.code() {
        Some(0) => {}
        Some(101) => {
            return fail(
                Exit::Crash,
                format!("{name} crashed (panic, exit 101): {}", last_line(&stderr)),
            );
        }
        Some(c) if (c as u32) >= 0xC000_0000 => {
            return fail(
                Exit::Crash,
                format!(
                    "{name} crashed (exception 0x{:08X}): {}",
                    c as u32,
                    last_line(&stderr)
                ),
            );
        }
        Some(c) => {
            return Err(match classify(&stderr) {
                Some((exit, msg)) => Failure {
                    exit,
                    msg: format!("{msg} ({name}: {})", last_line(&stderr)),
                },
                None => Failure {
                    exit: Exit::ToolError,
                    msg: format!("{name} failed (exit {c}): {}", last_line(&stderr)),
                },
            });
        }
        None => {
            return fail(
                Exit::Crash,
                format!("{name} was killed by a signal: {}", last_line(&stderr)),
            );
        }
    }
    let warnings = stderr
        .lines()
        .map(str::trim)
        .filter(|l| {
            l.get(..8)
                .is_some_and(|h| h.eq_ignore_ascii_case("warning:"))
        })
        .map(String::from)
        .collect();
    Ok(Ran { stdout, warnings })
}

fn read_head_tail(path: &Path) -> std::io::Result<(Vec<u8>, Vec<u8>, u64)> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut head = vec![0u8; len.min(64 * 1024) as usize];
    f.read_exact(&mut head)?;
    let t = len.min(1024);
    f.seek(SeekFrom::Start(len - t))?;
    let mut tailb = vec![0u8; t as usize];
    f.read_exact(&mut tailb)?;
    Ok((head, tailb, len))
}

/// Check one output file: exists, non-empty, right magic, not truncated. `Ok(true)` = valid but empty text (a warning).
pub fn validate_file(path: &Path, allow_empty: bool) -> Result<bool, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let shown = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let meta = std::fs::metadata(path).map_err(|_| format!("the tool wrote no file {shown}"))?;
    if meta.len() == 0 {
        return if allow_empty {
            Ok(true)
        } else {
            Err(format!("the tool wrote an empty file {shown}"))
        };
    }
    let (head, tail, _len) =
        read_head_tail(path).map_err(|e| format!("cannot read {shown}: {e}"))?;
    let bad = |what: &str| Err(format!("{shown} is not a valid .{ext} file: {what}"));
    match ext.as_str() {
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => {
            if !head.starts_with(b"PK") {
                return bad("missing the zip signature");
            }
            if File::open(path)
                .ok()
                .and_then(|f| zip::ZipArchive::new(f).ok())
                .is_none()
            {
                return bad("truncated (the zip directory cannot be read)");
            }
        }
        "pdf" => {
            if !head.starts_with(b"%PDF-") {
                return bad("missing the %PDF- header");
            }
            if !tail.windows(5).any(|w| w == b"%%EOF") {
                return bad("truncated (no %%EOF)");
            }
        }
        "png" => {
            if !head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
                return bad("missing the PNG signature");
            }
            if tail.len() < 12 || &tail[tail.len() - 8..tail.len() - 4] != b"IEND" {
                return bad("truncated (no IEND chunk)");
            }
        }
        "rtf" => {
            if !head.starts_with(b"{\\rtf") {
                return bad("missing the {\\rtf header");
            }
            if tail.iter().rev().find(|b| !b.is_ascii_whitespace()) != Some(&b'}') {
                return bad("truncated (unbalanced end)");
            }
        }
        "txt" | "md" | "html" | "csv" | "tsv" | "json" => {
            let h = head.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&head);
            // an incomplete character cut at the end of the 64 KB window is fine
            if std::str::from_utf8(h).is_err_and(|e| e.error_len().is_some()) {
                return bad("not valid UTF-8 text");
            }
        }
        _ => {}
    }
    Ok(false)
}

/// Validate a step's declared outputs under `job_dir`. Returns the files and whether any empty-text warning applies.
pub fn validate_step(job_dir: &Path, st: &ToolStep) -> Result<(Vec<PathBuf>, bool), Failure> {
    let files: Vec<PathBuf> = match &st.outputs {
        Outputs::Files(v) => v.iter().map(|p| job_dir.join(p)).collect(),
        Outputs::Dir { dir, ext, expect } => {
            let mut v: Vec<PathBuf> = std::fs::read_dir(job_dir.join(dir))
                .map(|rd| {
                    rd.filter_map(Result::ok)
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            if v.is_empty() {
                return fail(
                    Exit::BadOutput,
                    format!("{} wrote no .{ext} files", st.tool.name()),
                );
            }
            if let Some(n) = expect.filter(|n| *n != v.len()) {
                return fail(
                    Exit::BadOutput,
                    format!(
                        "{} wrote {} .{ext} files, expected {n}",
                        st.tool.name(),
                        v.len()
                    ),
                );
            }
            v
        }
    };
    let mut empty = false;
    for f in &files {
        match validate_file(f, st.allow_empty) {
            Ok(e) => empty |= e,
            Err(m) => {
                return fail(
                    Exit::BadOutput,
                    format!("{} output rejected: {m}", st.tool.name()),
                );
            }
        }
    }
    Ok((files, empty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_patterns_classify() {
        let c = |s: &str| classify(s).map(|x| x.0);
        assert_eq!(
            c("deckcraft-cli: x.ppt: not a presentation DeckCraft can read"),
            Some(Exit::Unsupported)
        );
        assert_eq!(c("gridcraft-cli: no sheet named `X`"), Some(Exit::Usage));
        assert_eq!(
            c("book.xlsx is password-protected. Enter its password to open it."),
            Some(Exit::Rejected)
        );
        assert_eq!(
            c("The sheet is protected with a password. Unprotect it first"),
            None
        );
        assert_eq!(c("Warning: the file is password-protected"), None);
        assert_eq!(c("something new went wrong"), None);
    }

    #[test]
    fn timeouts_follow_the_census_table() {
        let s = |c, mb, p| timeout_for(c, mb, p).as_secs();
        assert_eq!(s(Class::Docs, 1.0, 0), 55);
        assert_eq!(s(Class::SheetsConvert, 10.0, 0), 60);
        assert_eq!(s(Class::SheetsPdf, 10.0, 0), 100);
        assert_eq!(s(Class::Slides, 2.0, 0), 50);
        assert_eq!(s(Class::Pdf, 1.0, 10), 38);
        assert_eq!(s(Class::PdfNativeDocx, 5.0, 20), 50);
        assert_eq!(s(Class::Docs, 1000.0, 0), TIMEOUT_CAP_S);
        assert_eq!(class_of(ToolId::Sheets, &["run".into()]), Class::SheetsPdf);
    }
}
