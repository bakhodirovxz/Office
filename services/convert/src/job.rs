//! Jobs: a job directory per file, plan execution, output collection and the report (PLAN 2.3, 2.6).
use crate::exec::{Config, Exit, Failure, StepReport, fail, run_tool, validate_step};
pub use crate::report::{FileReport, Warning};
use crate::report::{self, SCHEMA};
use crate::scan;
use crate::kind::{self, DetectError, Kind};
use crate::plan::{self, Expand, Options, PlanError, Step, Target, ToolId};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// The exit code of a batch: any failure (1-9, the highest wins), else 10 if a job only warned under --strict, else 0.
pub fn batch_exit(reports: &[FileReport]) -> u8 {
    let fail = reports.iter().map(|r| r.code).filter(|c| (1..=9).contains(c)).max();
    fail.unwrap_or(if reports.iter().any(|r| r.code == 10) { 10 } else { 0 })
}

pub struct JobDir(PathBuf, bool);

static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

impl JobDir {
    pub fn create(cfg: &Config) -> Result<JobDir, Failure> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let root = cfg.work_dir.join(format!("job-{}-{:x}-{}", std::process::id(), nanos, JOB_SEQ.fetch_add(1, Ordering::Relaxed)));
        for d in ["in", "mid", "out", "tmp", "home"] {
            fs::create_dir_all(root.join(d)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create the job directory {}: {e}", root.display()) })?;
        }
        Ok(JobDir(root, cfg.keep_work))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// The cleanup guard: the job directory goes away on success, failure and panic alike (unless `--keep-work`).
impl Drop for JobDir {
    fn drop(&mut self) {
        if !self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn plan_failure(e: PlanError) -> Failure {
    match e {
        PlanError::Unsupported(m) => Failure { exit: Exit::Unsupported, msg: m },
        PlanError::NeedsSidecar(m) => Failure { exit: Exit::Missing, msg: m },
        PlanError::Usage(m) => Failure { exit: Exit::Usage, msg: m },
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

/// `--verify`: re-open every output with the CLI that owns its format (`info`). A file that does not open is exit 8.
fn verify(cfg: &Config, job: &Path, steps: &mut Vec<StepReport>) -> Result<(), Failure> {
    let mut files: Vec<PathBuf> = fs::read_dir(job.join("out")).map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    files.sort();
    for f in files.iter().filter(|p| p.is_file()) {
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let tool = match ext.as_str() {
            "docx" | "odt" | "rtf" | "html" | "md" | "txt" => ToolId::Docs,
            "xlsx" => ToolId::Sheets,
            "pptx" => ToolId::Slides,
            "pdf" => ToolId::Pdf,
            _ => continue, // png and csv have no CLI that opens them
        };
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        run_tool(cfg, tool, &["info".into(), format!("out/{name}")], job, None, 0.0, 0, steps).map_err(|e| match e.exit {
            Exit::ToolError | Exit::Crash => Failure { exit: Exit::BadOutput, msg: format!("--verify: {} cannot re-open {name}: {}", tool.name(), e.msg) },
            _ => e,
        })?;
    }
    Ok(())
}

fn run_plan(cfg: &Config, job: &Path, steps: Vec<Step>, input_mb: f64, p: &mut Progress) -> Result<(), Failure> {
    let mut queue: VecDeque<Step> = steps.into();
    let mut pages = 0u32; // from the last pdf info; scales the per-page timeouts
    while let Some(s) = queue.pop_front() {
        match s {
            Step::Tool(t) => {
                for (rel, body) in &t.write_files {
                    fs::write(job.join(rel), body).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot write {rel}: {e}") })?;
                }
                let sink = t.stdout_to.as_ref().map(|r| job.join(r));
                let ran = run_tool(cfg, t.tool, &t.argv, job, sink.as_deref(), input_mb, pages, &mut p.steps)?;
                for w in ran.warnings {
                    p.upstream(w);
                }
                if let Some(r) = translit_result(&ran.stdout) {
                    p.note_translit(r);
                }
                let (_, mut empty) = validate_step(job, &t)?;
                if let (false, true, Some(s)) = (empty, t.allow_empty, &sink) {
                    // a scanned page gives a newline or a form feed, not nothing
                    empty = fs::read(s).map(|b| String::from_utf8_lossy(&b).trim().is_empty()).unwrap_or(false);
                }
                if empty {
                    p.warn("empty_text", 1);
                }
            }
            Step::Expand(e) => {
                let (tool, argv): (ToolId, Vec<String>) = match &e {
                    Expand::CsvSheets { input, .. } => (ToolId::Sheets, vec!["info".into(), input.clone(), "--json".into()]),
                    Expand::PdfPages { pdf, .. } => (ToolId::Pdf, vec!["info".into(), pdf.clone()]),
                };
                let ran = run_tool(cfg, tool, &argv, job, None, input_mb, 0, &mut p.steps)?;
                let info: serde_json::Value = serde_json::from_slice(&ran.stdout)
                    .map_err(|e| Failure { exit: Exit::BadOutput, msg: format!("{} info did not print JSON: {e}", tool.name()) })?;
                for w in ran.warnings {
                    p.upstream(w);
                }
                // pdf-cli info lists what it could not read in `warnings`
                for w in info.get("warnings").and_then(|w| w.as_array()).into_iter().flatten().filter_map(|w| w.as_str()) {
                    p.upstream(format!("Warning: {w}"));
                }
                if matches!(e, Expand::PdfPages { .. }) {
                    pages = info.get("pages").and_then(|p| p.as_u64()).unwrap_or(0) as u32;
                }
                let more = match &e {
                    Expand::CsvSheets { input, only } => plan::expand_csv(input, only.as_deref(), &info),
                    Expand::PdfPages { pdf, spec, dpi } => plan::expand_pages(pdf, spec, *dpi, &info),
                }
                .map_err(plan_failure)?;
                for s in more.into_iter().rev() {
                    queue.push_front(s);
                }
            }
        }
    }
    Ok(())
}

pub struct Dest {
    pub out_dir: PathBuf,
    /// `-o FILE`: valid for single-output jobs only
    pub out_file: Option<PathBuf>,
}

/// `<stem>.<ext>`, `<stem>-p01.png`, `<stem>-<sheet>.csv`, `<stem>-p1-3.pdf` (PLAN 2.2).
fn final_name(stem: &str, file: &Path) -> String {
    let fs_ = file.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mid = if fs_ == "output" {
        String::new()
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

fn collect(cfg: &Config, job: &Path, stem: &str, dest: &Dest, sources: &[PathBuf]) -> Result<Vec<PathBuf>, Failure> {
    let mut files: Vec<PathBuf> = fs::read_dir(job.join("out"))
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file()).collect())
        .unwrap_or_default();
    files.sort();
    if files.is_empty() {
        return fail(Exit::BadOutput, "the conversion finished but produced no output file");
    }
    for h in &cfg.post_output {
        h.after(job, &files)?;
    }
    let targets: Vec<PathBuf> = match &dest.out_file {
        Some(f) if files.len() == 1 => vec![f.clone()],
        Some(_) => return fail(Exit::Usage, format!("-o names one file but the job produced {} (use --out-dir)", files.len())),
        None => vec![],
    };
    if let Some(t) = targets.first() {
        let same = |a: &Path, b: &Path| matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y);
        if sources.iter().any(|s| same(s, t)) {
            return fail(Exit::Usage, "-o is the input file; inputs are never overwritten");
        }
    }
    fs::create_dir_all(&dest.out_dir).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create {}: {e}", dest.out_dir.display()) })?;
    let mut done = vec![];
    for (i, f) in files.iter().enumerate() {
        let to = match targets.get(i) {
            Some(t) => t.clone(),
            None => reserve(&dest.out_dir, &final_name(stem, f))?,
        };
        fs::copy(f, &to).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot write {}: {e}", to.display()) })?;
        done.push(to);
    }
    Ok(done)
}

pub struct JobSpec<'a> {
    pub label: String,
    pub kind: Option<String>,
    pub target: String,
    /// (source file, name inside the job directory)
    pub inputs: Vec<(PathBuf, String)>,
    /// the caller's own paths of the inputs, for the "-o is an input" check (the job converts private copies)
    pub origins: Vec<PathBuf>,
    pub steps: Result<Vec<Step>, Failure>,
    pub warnings: Vec<Warning>,
    pub stem: String,
    pub dest: &'a Dest,
}

pub fn run_job(cfg: &Config, spec: JobSpec) -> FileReport {
    let mut p = Progress { warnings: spec.warnings.clone(), ..Progress::default() };
    let result = (|| {
        let steps = spec.steps.clone()?;
        let job = JobDir::create(cfg)?;
        let mut mb = 0.0;
        for (src, name) in &spec.inputs {
            fs::copy(src, job.path().join(name)).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot copy the input into the job: {e}") })?;
            mb += fs::metadata(src).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
        }
        run_plan(cfg, job.path(), steps, mb, &mut p)?;
        if cfg.verify {
            verify(cfg, job.path(), &mut p.steps)?;
            p.verified = Some(true);
        }
        collect(cfg, job.path(), &spec.stem, spec.dest, &spec.origins)
    })();
    let (code, message, outputs) = match result {
        Ok(o) => (Exit::Ok, "ok".to_string(), o.iter().map(|p| p.display().to_string()).collect()),
        Err(f) => (f.exit, f.msg, vec![]),
    };
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
        outputs,
    }
}

fn failed(label: &Path, target: &str, kind: Option<String>, f: Failure) -> FileReport {
    FileReport {
        schema: SCHEMA,
        input: label.display().to_string(),
        kind,
        target: target.into(),
        status: "failed",
        code: f.exit.code(),
        message: f.msg,
        steps: vec![],
        warnings: vec![],
        translit: None,
        verified: None,
        outputs: vec![],
    }
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

fn stage(cfg: &Config, src: &Path) -> Result<Staged, Failure> {
    use std::io::Read;
    let max = cfg.max_input_mb << 20;
    let mut input = match fs::File::open(src) {
        Ok(f) if f.metadata().is_ok_and(|m| m.is_file()) => f,
        _ => return fail(Exit::Usage, format!("{} is not a readable file", src.display())),
    };
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = cfg.work_dir.join(format!("stage-{}-{:x}-{}", std::process::id(), nanos, JOB_SEQ.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&dir).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot create {}: {e}", dir.display()) })?;
    let file = dir.join(src.file_name().unwrap_or_default());
    let staged = Staged { dir, file };
    let mut out = fs::File::create(&staged.file).map_err(|e| Failure { exit: Exit::ToolError, msg: format!("cannot stage the input: {e}") })?;
    // max + 1 bytes: one more than allowed means "over the limit" without copying a huge file in full
    let n = std::io::copy(&mut (&mut input).take(max + 1), &mut out).map_err(|e| Failure { exit: Exit::Usage, msg: format!("cannot read {}: {e}", src.display()) })?;
    if n > max {
        return fail(Exit::Rejected, format!("the file is over the {} MB input limit", cfg.max_input_mb));
    }
    Ok(staged)
}

fn detect_failure(src: &Path, e: DetectError) -> Failure {
    match e {
        DetectError::Io(e) => Failure { exit: Exit::Usage, msg: format!("cannot read {}: {e}", src.display()) },
        DetectError::Unknown => Failure { exit: Exit::Unsupported, msg: "unsupported input: the content is not a document, spreadsheet, presentation or PDF this converter reads".into() },
        DetectError::NotUtf8 => Failure { exit: Exit::Unsupported, msg: "unsupported input: the text is not UTF-8 (cp1251 or UTF-16?); re-save it as UTF-8".into() },
    }
}

/// Detect, plan, run, collect: one input file to one target.
pub fn convert_file(cfg: &Config, input: &Path, target: Target, opts: &Options, dest: &Dest) -> FileReport {
    let t = target.ext();
    let staged = match stage(cfg, input) {
        Ok(s) => s,
        Err(f) => return failed(input, t, None, f),
    };
    let det = match kind::detect(&staged.file) {
        Ok(d) => d,
        Err(e) => return failed(input, t, None, detect_failure(input, e)),
    };
    let k: Kind = det.kind;
    let kind = Some(k.ext().to_string());
    if det.password_protected {
        return failed(input, t, kind, Failure { exit: Exit::Rejected, msg: "the file is password-protected; passwords are not supported".into() });
    }
    let scan = match scan::scan(&staged.file) {
        Ok(s) => s,
        Err(f) => return failed(input, t, kind, f),
    };
    let mut warnings = vec![];
    if det.ext_mismatch {
        warnings.push(Warning { code: "kind_mismatch".into(), count: 1, text: report::text_with("kind_mismatch", &[("kind", k.ext())]) });
    }
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    warnings.extend(crate::losses::warnings(&scan, k, target));
    let steps = plan::plan(k, target, opts).map_err(plan_failure);
    run_job(cfg, JobSpec { label: input.display().to_string(), kind, target: t.into(), inputs: vec![(staged.file.clone(), plan::input_path(k))], origins: vec![input.to_path_buf()], steps, warnings, stem, dest })
}

/// `convert pdf combine|split|extract`. The inputs must be PDFs by content.
pub fn pdf_op(cfg: &Config, op: &str, inputs: &[PathBuf], steps: Vec<Step>, dest: &Dest) -> FileReport {
    let label = inputs.first().map(|p| p.display().to_string()).unwrap_or_default();
    let mut staged = vec![];
    for i in inputs {
        let st = match stage(cfg, i) {
            Ok(s) => s,
            Err(f) => return failed(i, "pdf", None, f),
        };
        match kind::detect(&st.file) {
            Ok(d) if d.kind == Kind::Pdf => {}
            Ok(_) | Err(DetectError::Unknown | DetectError::NotUtf8) => {
                return failed(i, "pdf", None, Failure { exit: Exit::Unsupported, msg: format!("{} is not a PDF", i.display()) });
            }
            Err(e) => return failed(i, "pdf", None, detect_failure(i, e)),
        }
        staged.push(st);
    }
    let named: Vec<(PathBuf, String)> = if op == "combine" {
        staged.iter().enumerate().map(|(n, p)| (p.file.clone(), format!("in/input{}.pdf", n + 1))).collect()
    } else {
        staged.iter().map(|p| (p.file.clone(), "in/input.pdf".to_string())).collect()
    };
    let stem = inputs.first().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or("output").to_string();
    run_job(cfg, JobSpec { label, kind: Some("pdf".into()), target: "pdf".into(), inputs: named, origins: inputs.to_vec(), steps: Ok(steps), warnings: vec![], stem, dest })
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
