//! Jobs: a job directory per file, plan execution, output collection and the report (PLAN 2.3, 2.6).
//!
//! The seams a caller plugs into (WP7 API, WP15 XDFU): `InputSource` (where the bytes come from), `OutputSink` (where the results
//! go; the CLI sink writes a temp name and renames), `JobHook` (before and after phases, see `hook.rs`), `JobMeta` (principal,
//! client address, a cancel flag) and `Ctl` (cancellation and the overall deadline, see `exec.rs`).
use crate::exec::{Config, Ctl, Exit, Failure, StepReport, fail, run_tool, validate_file, validate_step};
use crate::hook::JobCtx;
use crate::kind::{self, DetectError, Kind};
use crate::names::safe_name;
use crate::plan::{self, Expand, Options, PlanError, Step, Target, ToolId};
pub use crate::report::{FileReport, Warning};
use crate::report::{self, SCHEMA};
use crate::scan;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The exit code of a batch: any failure (1-9, the highest wins), else 10 if a job only warned under --strict, else 0.
pub fn batch_exit(reports: &[FileReport]) -> u8 {
    let fail = reports.iter().map(|r| r.code).filter(|c| (1..=9).contains(c)).max();
    fail.unwrap_or(if reports.iter().any(|r| r.code == 10) { 10 } else { 0 })
}

static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

/// `<pid>-<nanos>-<seq>`: unique per job, and the pid first so `sweep_stale` can tell whose directory it is.
fn new_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{}-{:x}-{}", std::process::id(), nanos, JOB_SEQ.fetch_add(1, Ordering::Relaxed))
}

pub struct JobDir(PathBuf, bool);

impl JobDir {
    pub fn create(cfg: &Config, id: &str) -> Result<JobDir, Failure> {
        let root = cfg.work_dir.join(format!("job-{id}"));
        for d in ["in", "mid", "out", "tmp", "home"] {
            fs::create_dir_all(root.join(d)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create the job directory {}: {e}", root.display()) })?;
        }
        Ok(JobDir(root, cfg.keep_work))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// The cleanup guard: the job directory goes away on success, failure and panic alike (unless `--keep-work`). A kill or Ctrl-C
/// skips `Drop`; `sweep_stale` catches those at the next start.
impl Drop for JobDir {
    fn drop(&mut self) {
        if !self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

/// The pid in the name of a directory this crate or the API makes, and only then (WP7 review m9: an operator folder such as
/// `stage-2024-photos` in a shared directory must never match). The shapes: `job-<pid>-<hex>-<n>`, `job-<pid>-check`,
/// `stage-<pid>-<hex>-<n>`, `stage-<pid>-api-<32 hex>`, `stage-<pid>-pre-<32 hex>-<n>`.
fn owner_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("job-").or_else(|| name.strip_prefix("stage-"))?;
    let parts: Vec<&str> = rest.split('-').collect();
    let hex = |s: &str, len: Option<usize>| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit()) && len.is_none_or(|l| s.len() == l);
    let num = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let ok = match parts.as_slice() {
        [_, "check"] => true,
        [_, "api", id] => hex(id, Some(32)),
        [_, "pre", id, n] => hex(id, Some(32)) && num(n),
        [_, h, n] => hex(h, None) && num(n),
        _ => false,
    };
    ok.then(|| parts[0].parse().ok()).flatten()
}

/// Remove `job-<pid>-*` and `stage-<pid>-*` directories whose process is gone (a crash, a kill or Ctrl-C left them, with
/// plaintext copies of inputs). Returns how many were removed. A pid that a later process reused does not keep a directory
/// alive: the owner must have started before the directory was last written.
pub fn sweep_stale(work_dir: &Path) -> usize {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let Ok(rd) = fs::read_dir(work_dir) else { return 0 };
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    let me = std::process::id();
    let mut n = 0;
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = owner_pid(&name) else { continue };
        let written = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(u64::MAX, |d| d.as_secs());
        let alive = sys.process(Pid::from_u32(pid)).is_some_and(|p| p.start_time() <= written);
        if pid != me && !alive && fs::remove_dir_all(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

fn plan_failure(e: PlanError) -> Failure {
    match e {
        PlanError::Unsupported(m) => Failure { exit: Exit::Unsupported, msg: m },
        PlanError::NeedsSidecar(m) => Failure { exit: Exit::Missing, msg: m },
        PlanError::Usage(m) => Failure { exit: Exit::Usage, msg: m },
        PlanError::Rejected(m) => Failure { exit: Exit::Rejected, msg: m },
    }
}

#[derive(Default)]
struct Progress {
    steps: Vec<StepReport>,
    warnings: Vec<Warning>,
    translit: Option<serde_json::Value>,
    verified: Option<bool>,
}

impl Progress {
    /// A warning whose text is the catalog text of its code.
    fn warn(&mut self, code: &str, count: u32) {
        self.warnings.push(report::warn(code, count));
    }
    /// An engine message, verbatim. The same message again (`--sheet all` on an .xls repeats the import warning per sheet) raises the count.
    fn upstream(&mut self, text: String) {
        match self.warnings.iter_mut().find(|w| w.code == "upstream" && w.text == text) {
            Some(w) => w.count += 1,
            None => self.warnings.push(Warning { code: "upstream".into(), count: 1, text }),
        }
    }
    /// The transliteration command's result JSON goes into the report. Nothing converted, or text inside formulas, is a warning.
    fn note_translit(&mut self, r: serde_json::Value) {
        if r.get("runs").and_then(|n| n.as_u64()) == Some(0) {
            self.warn("translit_nothing", 1);
        }
        let in_formulas = r.get("formulasWithText").and_then(|n| n.as_u64()).unwrap_or(0);
        if in_formulas > 0 {
            self.warn("translit_formula_text", in_formulas as u32);
        }
        self.translit = Some(r);
    }
}

/// The result object of a `tools.translit*` command in a tool's stdout. The three CLIs print it differently
/// (`id: {..}`, `id -> {..}`, `{"command":..,"result":{..}}`), so take the first JSON object on a line that has `rulesVersion`.
pub fn translit_result(stdout: &[u8]) -> Option<serde_json::Value> {
    String::from_utf8_lossy(stdout).lines().find_map(|l| {
        let v: serde_json::Value = serde_json::from_str(&l[l.find('{')?..]).ok()?;
        let r = v.get("result").unwrap_or(&v);
        r.get("rulesVersion").is_some().then(|| r.clone())
    })
}

/// Does the docx at `path` hold a single non-blank `<w:t>` run? A file that cannot be read says "yes": that is the validator's to report.
fn docx_has_text(path: &Path) -> bool {
    let Ok(f) = fs::File::open(path) else { return true };
    let Ok(mut z) = zip::ZipArchive::new(f) else { return true };
    let Ok(part) = z.by_name("word/document.xml") else { return true };
    let mut xml = Vec::new();
    if part.take(scan::PART_CAP).read_to_end(&mut xml).is_err() {
        return true;
    }
    let xml = String::from_utf8_lossy(&xml);
    xml.match_indices("<w:t").any(|(i, _)| {
        let rest = &xml[i + 4..];
        // `<w:t>` or `<w:t xml:space="preserve">`, not `<w:tbl>`, `<w:tc>` or `<w:tab/>`
        matches!(rest.as_bytes().first(), Some(b'>' | b' '))
            && rest.split_once('>').and_then(|(_, body)| body.split_once("</w:t>")).is_some_and(|(text, _)| !text.trim().is_empty())
    })
}

/// The regular files of `out/`. A symlink, a directory or anything else is reported by name.
fn list_out(job: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = vec![];
    for e in fs::read_dir(job.join("out")).map(|rd| rd.filter_map(Result::ok).collect::<Vec<_>>()).unwrap_or_default() {
        let p = e.path();
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_file() => files.push(p),
            _ => return fail(Exit::BadOutput, format!("the tool left something that is not a regular file in out/: {}", e.file_name().to_string_lossy())),
        }
    }
    files.sort();
    Ok(files)
}

/// `--verify`: re-open every output with the CLI that owns its format (`info`). A file that does not open is exit 8.
fn verify(cfg: &Config, ctl: &Ctl, job: &Path, steps: &mut Vec<StepReport>) -> Result<(), Failure> {
    for f in list_out(job)? {
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let tool = match ext.as_str() {
            "docx" | "odt" | "rtf" | "html" | "md" | "txt" => ToolId::Docs,
            "xlsx" => ToolId::Sheets,
            "pptx" => ToolId::Slides,
            "pdf" => ToolId::Pdf,
            _ => continue, // png and csv have no CLI that opens them
        };
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        run_tool(cfg, ctl, tool, &["info".into(), format!("out/{name}")], job, None, 0.0, 0, steps).map_err(|e| match e.exit {
            Exit::ToolError | Exit::Crash => Failure { exit: Exit::BadOutput, msg: format!("--verify: {} cannot re-open {name}: {}", tool.name(), e.msg) },
            _ => e,
        })?;
    }
    Ok(())
}

/// Run the plan. Returns every file a step declared and validated (`collect` accepts nothing else from `out/`).
fn run_plan(cfg: &Config, ctl: &Ctl, job: &Path, steps: Vec<Step>, input_mb: f64, p: &mut Progress) -> Result<HashSet<PathBuf>, Failure> {
    let mut queue: VecDeque<Step> = steps.into();
    let mut pages = 0u32; // from the last info; scales the per-page timeouts
    let mut declared = HashSet::new();
    while let Some(s) = queue.pop_front() {
        ctl.check()?;
        match s {
            Step::Tool(t) => {
                for (rel, body) in &t.write_files {
                    fs::write(job.join(rel), body).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot write {rel}: {e}") })?;
                }
                let sink = t.stdout_to.as_ref().map(|r| job.join(r));
                let ran = run_tool(cfg, ctl, t.tool, &t.argv, job, sink.as_deref(), input_mb, pages, &mut p.steps)?;
                for w in ran.warnings {
                    p.upstream(w);
                }
                if let Some(r) = translit_result(&ran.stdout) {
                    p.note_translit(r);
                }
                let (files, mut empty) = validate_step(job, &t)?;
                declared.extend(files);
                if let (false, true, Some(s)) = (empty, t.allow_empty, &sink) {
                    // a scanned page gives a newline or a form feed, not nothing
                    empty = fs::read(s).map(|b| String::from_utf8_lossy(&b).trim().is_empty()).unwrap_or(false);
                }
                if empty {
                    p.warn("empty_text", 1);
                }
            }
            Step::Http(h) => {
                if let Some(rep) = crate::http::run_http(cfg, ctl, job, &h, &mut p.steps)? {
                    for w in rep["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                        p.warnings.push(Warning { code: "sidecar".into(), count: 1, text: w.to_string() });
                    }
                }
                declared.insert(job.join(&h.output));
            }
            Step::Expand(e) => {
                let (tool, argv): (ToolId, Vec<String>) = match &e {
                    Expand::CsvSheets { input, .. } => (ToolId::Sheets, vec!["info".into(), input.clone(), "--json".into()]),
                    Expand::PdfPages { pdf, .. } | Expand::PdfSplit { pdf, .. } | Expand::PdfCheck { pdf } => (ToolId::Pdf, vec!["info".into(), pdf.clone()]),
                    Expand::SlidePages { deck, .. } => (ToolId::Slides, vec!["info".into(), deck.clone()]),
                };
                let ran = run_tool(cfg, ctl, tool, &argv, job, None, input_mb, 0, &mut p.steps)?;
                let info: serde_json::Value = serde_json::from_slice(&ran.stdout)
                    .map_err(|e| Failure { exit: Exit::BadOutput, msg: format!("{} info did not print JSON: {e}", tool.name()) })?;
                for w in ran.warnings {
                    p.upstream(w);
                }
                // pdf-cli info lists what it could not read (a damaged page, skipped content, a cross-reference table it had to rebuild)
                // in `warnings`: each is shown as the engine said it, and the file as a whole is flagged `pdf_damaged`
                let damage: Vec<&str> = info.get("warnings").and_then(|w| w.as_array()).into_iter().flatten().filter_map(|w| w.as_str()).collect();
                for w in &damage {
                    p.upstream(format!("Warning: {w}"));
                }
                if tool == ToolId::Pdf && !damage.is_empty() {
                    p.warn("pdf_damaged", damage.len() as u32);
                }
                if matches!(e, Expand::PdfPages { .. } | Expand::PdfSplit { .. } | Expand::PdfCheck { .. }) {
                    pages = info.get("pages").and_then(|p| p.as_u64()).unwrap_or(0) as u32;
                }
                let more = match &e {
                    Expand::CsvSheets { input, only } => plan::expand_csv(input, only.as_deref(), &info, &cfg.caps),
                    Expand::PdfPages { pdf, spec, dpi } => plan::expand_pages(pdf, spec, *dpi, &info, &cfg.caps),
                    Expand::SlidePages { deck, spec, dpi } => plan::expand_slides(deck, spec, *dpi, &info, &cfg.caps),
                    Expand::PdfSplit { pdf, by } => plan::expand_split(pdf, by, &info, &cfg.caps),
                    Expand::PdfCheck { .. } => Ok(plan::Expanded { steps: vec![], warnings: vec![] }),
                }
                .map_err(plan_failure)?;
                for (code, n) in more.warnings {
                    p.warn(code, n);
                }
                for s in more.steps.into_iter().rev() {
                    queue.push_front(s);
                }
            }
        }
    }
    Ok(declared)
}

pub struct Dest {
    pub out_dir: PathBuf,
    /// `-o FILE`: valid for single-output jobs only
    pub out_file: Option<PathBuf>,
}

/// Where a job's results go. The CLI's sink is `Dest` (files, written under a temp name and renamed, so a failure never
/// leaves a real-looking file); the WP7 sink encrypts and keeps them under a job id.
pub trait OutputSink: Sync {
    /// Called once with the number of outputs, before the first `put`. Refuse here when the sink cannot take them.
    fn begin(&self, count: usize, origins: &[PathBuf]) -> Result<(), Failure>;
    /// Store `src` as `name`; returns the locator for the report.
    fn put(&self, index: usize, name: &str, src: &Path) -> Result<String, Failure>;
    /// A `put` failed: remove what was placed so far.
    fn discard(&self, placed: &[String]);
}

/// Where a job's input comes from. The executor copies it into a private staging file once, and everything after (size limit,
/// kind detection, scan, conversion) works on that copy.
pub trait InputSource {
    /// the file name with its extension (kind detection uses the extension as a tie-break)
    fn name(&self) -> String;
    fn open(&self) -> std::io::Result<Box<dyn Read + '_>>;
    /// shown in the report
    fn label(&self) -> String;
    /// a local path the caller owns, for the "-o is an input" check
    fn origin(&self) -> Option<PathBuf> {
        None
    }
}

impl InputSource for PathBuf {
    fn name(&self) -> String {
        Path::file_name(self).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
    fn open(&self) -> std::io::Result<Box<dyn Read + '_>> {
        let f = fs::File::open(self)?;
        if !f.metadata()?.is_file() {
            return Err(std::io::Error::other("not a file"));
        }
        Ok(Box::new(f))
    }
    fn label(&self) -> String {
        self.display().to_string()
    }
    fn origin(&self) -> Option<PathBuf> {
        Some(self.clone())
    }
}

/// `<stem>.<ext>`, `<stem>-p01.png`, `<stem>-<sheet>.csv`, `<stem>-p1-3.pdf` (PLAN 2.2).
fn final_name(stem: &str, file: &Path) -> String {
    let fs_ = file.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("");
    let sheet = fs_.strip_prefix('s').and_then(|r| r.split_once('-')).filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    let mid = if fs_ == "output" {
        String::new()
    } else if let Some((_, name)) = sheet {
        format!("-{name}")
    } else if let Some(rest) = fs_.strip_prefix("input-") {
        format!("-{rest}")
    } else if let Some(n) = fs_.strip_prefix("slide-") {
        format!("-p{n}")
    } else {
        format!("-{fs_}")
    };
    format!("{stem}{mid}.{ext}")
}

/// Reserve a name that does not exist yet: `a.pdf`, `a (2).pdf`, ... (create_new is atomic, so `-j` jobs cannot collide).
fn reserve(dir: &Path, name: &str) -> Result<PathBuf, Failure> {
    let p = Path::new(name);
    let (stem, ext) = (p.file_stem().and_then(|s| s.to_str()).unwrap_or("out"), p.extension().and_then(|s| s.to_str()));
    for n in 1..10_000 {
        let cand = match (n, ext) {
            (1, _) => dir.join(name),
            (_, Some(e)) => dir.join(format!("{stem} ({n}).{e}")),
            (_, None) => dir.join(format!("{stem} ({n})")),
        };
        match fs::OpenOptions::new().write(true).create_new(true).open(&cand) {
            Ok(_) => return Ok(cand),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return fail(Exit::ToolError, format!("cannot write {}: {e}", cand.display())),
        }
    }
    fail(Exit::ToolError, format!("no free name for {name} in {}", dir.display()))
}

impl OutputSink for Dest {
    fn begin(&self, count: usize, origins: &[PathBuf]) -> Result<(), Failure> {
        if let Some(t) = &self.out_file {
            if count != 1 {
                return fail(Exit::Usage, format!("-o names one file but the job produced {count} (use --out-dir)"));
            }
            let same = |a: &Path, b: &Path| matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y);
            if origins.iter().any(|s| same(s, t)) {
                return fail(Exit::Usage, "-o is the input file; inputs are never overwritten");
            }
        }
        let dir = match &self.out_file {
            Some(f) => f.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| ".".into()),
            None => self.out_dir.clone(),
        };
        fs::create_dir_all(&dir).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create {}: {e}", dir.display()) })
    }

    fn put(&self, _index: usize, name: &str, src: &Path) -> Result<String, Failure> {
        let (to, placeholder) = match &self.out_file {
            Some(f) => (f.clone(), false),
            None => (reserve(&self.out_dir, name)?, true),
        };
        let part = to.with_file_name(format!(".{}.{}.part", to.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
        let done = fs::copy(src, &part).and_then(|_| fs::rename(&part, &to));
        if let Err(e) = done {
            let _ = fs::remove_file(&part);
            if placeholder {
                let _ = fs::remove_file(&to);
            }
            return fail(Exit::ToolError, format!("cannot write {}: {e}", to.display()));
        }
        Ok(to.display().to_string())
    }

    fn discard(&self, placed: &[String]) {
        for p in placed {
            let _ = fs::remove_file(p);
        }
    }
}

/// Hooks run domain CLIs through this: the same environment, limits and cancel as a conversion step.
struct Runner<'a> {
    cfg: &'a Config,
    ctl: &'a Ctl,
    job: &'a Path,
}

impl crate::hook::ToolRunner for Runner<'_> {
    fn run(&self, tool: ToolId, argv: &[String], input_mb: f64) -> Result<Vec<u8>, Failure> {
        run_tool(self.cfg, self.ctl, tool, argv, self.job, None, input_mb, 0, &mut vec![]).map(|r| r.stdout)
    }
}

fn collect(cfg: &Config, ctl: &Ctl, jctx: &JobCtx, declared: &HashSet<PathBuf>, spec: &JobSpec, stem: &str, p: &mut Progress) -> Result<Vec<String>, Failure> {
    let mut files = list_out(&jctx.job_dir)?;
    if files.is_empty() {
        return fail(Exit::BadOutput, "the conversion finished but produced no output file");
    }
    if let Some(extra) = files.iter().find(|f| !declared.contains(*f)) {
        return fail(Exit::BadOutput, format!("the tool left an unexpected file in out/: {}", extra.file_name().unwrap_or_default().to_string_lossy()));
    }
    if !cfg.hooks.is_empty() {
        let runner = Runner { cfg, ctl, job: &jctx.job_dir };
        for h in &cfg.hooks {
            p.warnings.extend(h.after(jctx, &files, &runner)?);
        }
        // the hooks may have rewritten or added files: list again and validate again
        files = list_out(&jctx.job_dir)?;
        if files.is_empty() {
            return fail(Exit::BadOutput, "a hook removed every output");
        }
        for f in &files {
            let txt = f.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"));
            validate_file(f, txt).map_err(|m| Failure { exit: Exit::BadOutput, msg: format!("output rejected after the hooks: {m}") })?;
        }
    }
    spec.dest.begin(files.len(), &spec.origins)?;
    let mut done: Vec<String> = vec![];
    for (i, f) in files.iter().enumerate() {
        match spec.dest.put(i, &final_name(stem, f), f) {
            Ok(loc) => done.push(loc),
            Err(e) => {
                spec.dest.discard(&done);
                return Err(e);
            }
        }
    }
    Ok(done)
}

/// Who and what a job is for, beyond the file: handed to the hooks, and the way to cancel one job.
#[derive(Clone, Default)]
pub struct JobMeta {
    pub principal: Option<String>,
    pub client: Option<String>,
    /// set it to stop this job (exit 4)
    pub cancel: Option<Arc<AtomicBool>>,
    /// the end of the whole API job, shared by all its files (default: `job_timeout_s` from the start of this file)
    pub deadline: Option<Instant>,
    /// a scan the caller already made of this very content: used when the staged copy has the same SHA-256
    pub verdict: Option<Arc<scan::Verdict>>,
    /// the caller's own id for this file, for the audit log (the API: `<job id>/<file number>`)
    pub reference: Option<String>,
}

pub struct JobSpec<'a> {
    pub label: String,
    pub kind: Option<String>,
    pub target: String,
    /// (staged file, name inside the job directory)
    pub inputs: Vec<(PathBuf, String)>,
    /// the caller's own paths of the inputs, for the "-o is an input" check (the job converts private copies)
    pub origins: Vec<PathBuf>,
    pub steps: Result<Vec<Step>, Failure>,
    pub warnings: Vec<Warning>,
    /// the output name stem; made safe inside `run_job` (no separators, no device names, 200 characters)
    pub stem: String,
    pub dest: &'a dyn OutputSink,
    pub opts: Options,
    pub meta: JobMeta,
}

impl<'a> JobSpec<'a> {
    pub fn new(dest: &'a dyn OutputSink) -> JobSpec<'a> {
        JobSpec {
            label: String::new(),
            kind: None,
            target: String::new(),
            inputs: vec![],
            origins: vec![],
            steps: Ok(vec![]),
            warnings: vec![],
            stem: "output".into(),
            dest,
            opts: Options::default(),
            meta: JobMeta::default(),
        }
    }
}

pub fn run_job(cfg: &Config, spec: JobSpec) -> FileReport {
    let mut p = Progress { warnings: spec.warnings.clone(), ..Progress::default() };
    let id = new_id();
    let ctl = Ctl { cancel: spec.meta.cancel.clone().or_else(|| cfg.cancel.clone()), deadline: Some(spec.meta.deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(cfg.job_timeout_s))) };
    let stem = safe_name(&spec.stem, "output");
    let mut ctx_for_hooks: Option<JobCtx> = None;
    let result = (|| {
        let steps = spec.steps.clone()?;
        let job = JobDir::create(cfg, &id)?;
        let mut mb = 0.0;
        for (src, name) in &spec.inputs {
            fs::copy(src, job.path().join(name)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot copy the input into the job: {e}") })?;
            mb += fs::metadata(src).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
        }
        let jctx = JobCtx {
            job_id: id.clone(),
            job_dir: job.path().to_path_buf(),
            input: spec.inputs.first().map(|(_, n)| job.path().join(n)).unwrap_or_default(),
            kind: spec.kind.as_deref().and_then(|k| Kind::ALL.into_iter().find(|x| x.ext() == k)),
            target: spec.target.clone(),
            opts: spec.opts.clone(),
            principal: spec.meta.principal.clone(),
            client: spec.meta.client.clone(),
            label: spec.label.clone(),
            reference: spec.meta.reference.clone(),
        };
        ctx_for_hooks = Some(jctx.clone());
        let runner = Runner { cfg, ctl: &ctl, job: job.path() };
        for h in &cfg.hooks {
            p.warnings.extend(h.before(&jctx, &runner)?);
        }
        let declared = run_plan(cfg, &ctl, job.path(), steps, mb, &mut p)?;
        // pdf -> docx with the native engine has no stdout to look at (pdf -> txt does): a scan gives a docx without a single word
        if spec.kind.as_deref() == Some("pdf") && spec.target == "docx" && spec.opts.pdf_engine == plan::PdfEngine::Native && !docx_has_text(&job.path().join("out/output.docx")) {
            p.warn("empty_text", 1);
        }
        if cfg.verify {
            verify(cfg, &ctl, job.path(), &mut p.steps)?;
            p.verified = Some(true);
        }
        collect(cfg, &ctl, &jctx, &declared, &spec, &stem, &mut p)
    })();
    let (code, message, outputs) = match result {
        Ok(o) => (Exit::Ok, "ok".to_string(), o),
        Err(f) => (f.exit, f.msg, vec![]),
    };
    // the hooks forget the job, and one of them may have something for the report (WP15: `xdfu`)
    let xdfu = ctx_for_hooks.and_then(|c| cfg.hooks.iter().filter_map(|h| h.finish(&c)).next());
    FileReport {
        schema: SCHEMA,
        input: spec.label,
        kind: spec.kind,
        target: spec.target,
        status: if code != Exit::Ok { "failed" } else if p.warnings.is_empty() { "ok" } else { "warn" },
        code: code.code(),
        message,
        steps: p.steps,
        warnings: p.warnings,
        translit: p.translit,
        verified: if code == Exit::Ok { p.verified } else { None },
        xdfu,
        outputs,
    }
}

fn failed(label: &str, target: &str, kind: Option<String>, f: Failure) -> FileReport {
    FileReport {
        schema: SCHEMA,
        input: label.to_string(),
        kind,
        target: target.into(),
        status: "failed",
        code: f.exit.code(),
        message: f.msg,
        steps: vec![],
        warnings: vec![],
        translit: None,
        verified: None,
        xdfu: None,
        outputs: vec![],
    }
}

/// Run one job body so that a panic in it becomes a `Crash` report instead of taking the whole `-j` batch (and every other
/// job's report) down with it.
pub fn guarded(label: &str, target: &str, f: impl FnOnce() -> FileReport) -> FileReport {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|e| {
        let what = e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default();
        failed(label, target, None, Failure { exit: Exit::Crash, msg: format!("internal error in the converter: {what}") })
    })
}

/// A private copy of an input. The size limit, the kind detection and the scan all read this copy, and the job converts this
/// copy, so a file that changes on a sync folder or a share between the checks and the conversion cannot slip through.
struct Staged {
    dir: PathBuf,
    file: PathBuf,
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stage(cfg: &Config, src: &dyn InputSource) -> Result<Staged, Failure> {
    let max = cfg.max_input_mb << 20;
    let input = src.open().map_err(|e| Failure { exit: Exit::Usage, msg: format!("{} is not a readable file ({e})", src.label()) })?;
    let dir = cfg.work_dir.join(format!("stage-{}", new_id()));
    fs::create_dir_all(&dir).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create {}: {e}", dir.display()) })?;
    // the file name is the client's: keep the extension (a tie-break for detection), make the rest safe
    let name = src.name();
    let ext = Path::new(&name).extension().and_then(|e| e.to_str()).map(|e| safe_name(e, "")).filter(|e| !e.is_empty());
    let file = dir.join(match ext {
        Some(e) => format!("input.{e}"),
        None => "input".into(),
    });
    let staged = Staged { dir, file };
    let mut out = fs::File::create(&staged.file).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot stage the input: {e}") })?;
    // max + 1 bytes: one more than allowed means "over the limit" without copying a huge file in full
    let n = std::io::copy(&mut input.take(max + 1), &mut out).map_err(|e| Failure { exit: Exit::Usage, msg: format!("cannot read {}: {e}", src.label()) })?;
    if n > max {
        return fail(Exit::Rejected, format!("the file is over the {} MB input limit", cfg.max_input_mb));
    }
    if n == 0 {
        return Err(empty_file());
    }
    Ok(staged)
}

/// A 0-byte file is rejected whatever its extension claims: it is never read as an empty text file.
pub fn empty_file() -> Failure {
    Failure { exit: Exit::Rejected, msg: "the file is empty (0 bytes), so there is nothing to convert".into() }
}

fn detect_failure(label: &str, e: DetectError) -> Failure {
    match e {
        DetectError::Io(e) => Failure { exit: Exit::Usage, msg: format!("cannot read {label}: {e}") },
        DetectError::Empty => empty_file(),
        DetectError::Unknown => Failure { exit: Exit::Unsupported, msg: "unsupported input: the content is not a document, spreadsheet, presentation or PDF this converter reads".into() },
        DetectError::NotUtf8 => Failure { exit: Exit::Unsupported, msg: "unsupported input: the text is not UTF-8 (cp1251 or UTF-16?); re-save it as UTF-8".into() },
    }
}

/// Detect, plan, run, collect: one input file to one target.
pub fn convert_file(cfg: &Config, input: &Path, target: Target, opts: &Options, dest: &Dest) -> FileReport {
    convert_source(cfg, &input.to_path_buf(), target, opts, dest, &JobMeta::default())
}

/// The same for any `InputSource` and `OutputSink` (the API).
pub fn convert_source(cfg: &Config, input: &dyn InputSource, target: Target, opts: &Options, dest: &dyn OutputSink, meta: &JobMeta) -> FileReport {
    let t = target.ext();
    let label = input.label();
    let staged = match stage(cfg, input) {
        Ok(s) => s,
        Err(f) => return failed(&label, t, None, f),
    };
    let det = match kind::detect(&staged.file) {
        Ok(d) => d,
        Err(e) => return failed(&label, t, None, detect_failure(&label, e)),
    };
    let k: Kind = det.kind;
    let kind = Some(k.ext().to_string());
    if det.password_protected {
        return failed(&label, t, kind, Failure { exit: Exit::Rejected, msg: "the file is password-protected; passwords are not supported".into() });
    }
    // staging, detection and the scan run before any `Ctl` exists, so the cancel flag is looked at here (WP7 review m12)
    if meta.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
        return failed(&label, t, kind, Failure { exit: Exit::Timeout, msg: "the job was cancelled".into() });
    }
    // text in cp1251, cp1252 or UTF-16 becomes UTF-8 in the private copy, before the scan and before any tool (owner decision, WP9 fixes)
    let transcoded = if crate::text::is_text(k) {
        match crate::text::normalise(&staged.file) {
            Ok(e) => e,
            Err(f) => return failed(&label, t, kind, f),
        }
    } else {
        None
    };
    let known = meta.verdict.as_ref().filter(|v| scan::sha256_file(&staged.file).is_ok_and(|h| h == v.sha256));
    let scan = match known {
        Some(v) => v.scan.clone(),
        None => match scan::scan(&staged.file) {
            Ok(s) => s,
            Err(f) => return failed(&label, t, kind, f),
        },
    };
    let mut warnings = vec![];
    if let Some(enc) = transcoded {
        warnings.push(Warning { code: "text_transcoded".into(), count: 1, text: report::text_with("text_transcoded", &[("encoding", enc)]) });
    }
    if det.ext_mismatch {
        warnings.push(Warning { code: "kind_mismatch".into(), count: 1, text: report::text_with("kind_mismatch", &[("kind", k.ext())]) });
    }
    let name = input.name();
    let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    warnings.extend(crate::losses::warnings(&scan, k, target));
    let steps = plan::plan(k, target, opts).map_err(plan_failure);
    run_job(
        cfg,
        JobSpec {
            label,
            kind,
            target: t.into(),
            inputs: vec![(staged.file.clone(), plan::input_path(k))],
            origins: input.origin().into_iter().collect(),
            steps,
            warnings,
            stem,
            dest,
            opts: opts.clone(),
            meta: meta.clone(),
        },
    )
}

/// `convert pdf combine|split|extract`. The inputs must be PDFs by content.
pub fn pdf_op(cfg: &Config, op: &str, inputs: &[PathBuf], steps: Vec<Step>, dest: &Dest) -> FileReport {
    let label = inputs.first().map(|p| p.display().to_string()).unwrap_or_default();
    let mut staged = vec![];
    for i in inputs {
        let st = match stage(cfg, i) {
            Ok(s) => s,
            Err(f) => return failed(&i.display().to_string(), "pdf", None, f),
        };
        match kind::detect(&st.file) {
            Ok(d) if d.kind == Kind::Pdf => {}
            Ok(_) | Err(DetectError::Unknown | DetectError::NotUtf8) => {
                return failed(&i.display().to_string(), "pdf", None, Failure { exit: Exit::Unsupported, msg: format!("{} is not a PDF", i.display()) });
            }
            Err(e) => return failed(&i.display().to_string(), "pdf", None, detect_failure(&i.display().to_string(), e)),
        }
        staged.push(st);
    }
    let named: Vec<(PathBuf, String)> = if op == "combine" {
        staged.iter().enumerate().map(|(n, p)| (p.file.clone(), format!("in/input{}.pdf", n + 1))).collect()
    } else {
        staged.iter().map(|p| (p.file.clone(), "in/input.pdf".to_string())).collect()
    };
    let stem = inputs.first().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or("output").to_string();
    run_job(
        cfg,
        JobSpec { label, kind: Some("pdf".into()), target: "pdf".into(), inputs: named, origins: inputs.to_vec(), steps: Ok(steps), stem, ..JobSpec::new(dest) },
    )
}

/// Run `f` over `items` on `n` threads; results keep the input order.
pub fn par_map<T: Sync, R: Send>(items: &[T], n: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..n.clamp(1, items.len().max(1)) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = f(item);
                    out.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.expect("every item ran")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn docx(name: &str, body: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("convert-hastext-{}-{name}.docx", std::process::id()));
        let mut w = zip::ZipWriter::new(fs::File::create(&p).unwrap());
        w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(body.as_bytes()).unwrap();
        w.finish().unwrap();
        p
    }

    #[test]
    fn a_docx_has_text_when_one_run_is_not_blank() {
        let yes = docx("yes", "<w:document><w:body><w:p><w:r><w:t>Salom</w:t></w:r></w:p></w:body></w:document>");
        let preserved = docx("pres", "<w:p><w:r><w:t xml:space=\"preserve\"> a </w:t></w:r></w:p>");
        // a scan: tables, cells, tabs, empty and blank runs, no words
        let no = docx("no", "<w:tbl><w:tr><w:tc><w:p><w:r><w:tab/><w:t></w:t><w:t xml:space=\"preserve\">  </w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>");
        assert!(docx_has_text(&yes) && docx_has_text(&preserved));
        assert!(!docx_has_text(&no));
        // what cannot be read is not called empty (the validator reports it)
        assert!(docx_has_text(&std::env::temp_dir().join("convert-hastext-missing.docx")));
        for p in [yes, preserved, no] {
            let _ = fs::remove_file(p);
        }
    }
}
