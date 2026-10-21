//! The job store, the queue with its two gates (CPU slots and a memory budget), the sweeper and the report scrubbing
//! (PLAN 5.2, 5.4). Everything is in memory; a restart loses the jobs (their files are swept at the next start).
use crate::config::Resolved;
use crate::crypt::{Codec, JobKey};
use crate::routes::ApiError;
use axum::http::StatusCode;
use convert::exec::{Class, Config as ConvertConfig, Exit, Failure, fail, limits};
use convert::job::{self, FileReport, InputSource, JobMeta, OutputSink};
use convert::kind::{Family, Kind};
use convert::plan::{Options, Target};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

pub struct OutputEntry {
    pub name: String,
    pub size: u64,
}

pub struct FileState {
    pub n: usize,
    pub name: String,
    pub kind: Option<String>,
    pub target: String,
    /// queued | running | done | failed
    pub state: &'static str,
    pub step: Option<String>,
    /// the scrubbed report (no server paths)
    pub report: Option<Value>,
    pub outputs: Vec<OutputEntry>,
}

#[derive(Default)]
pub struct JobState {
    pub files: Vec<FileState>,
    pub finished_at: Option<Instant>,
}

#[derive(Default)]
struct Life {
    finished: bool,
    deleted: bool,
}

pub struct Job {
    pub id: String,
    pub owner: String,
    pub dir: PathBuf,
    pub codec: Codec,
    pub cancel: Arc<AtomicBool>,
    pub st: Mutex<JobState>,
    pub zip_size: AtomicU64,
    pub zip_lock: Mutex<()>,
    active: AtomicBool,
    life: Mutex<Life>,
}

impl Job {
    pub fn in_path(&self, n: usize) -> PathBuf {
        self.dir.join("in").join(n.to_string())
    }
    pub fn out_path(&self, n: usize, k: usize) -> PathBuf {
        self.dir.join("out").join(n.to_string()).join(k.to_string())
    }
    pub fn zip_path(&self) -> PathBuf {
        self.dir.join("zip")
    }
    pub fn is_finished(&self) -> bool {
        self.st.lock().unwrap().finished_at.is_some()
    }
}

/// The fixed-window counter behind the rate limits: `hit` is Ok, or the seconds until the window ends.
pub struct Limiter {
    per_min: u32,
    m: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Limiter {
    pub fn new(per_min: u32) -> Limiter {
        Limiter { per_min, m: Mutex::new(HashMap::new()) }
    }
    pub fn hit(&self, key: &str) -> Result<(), u64> {
        let mut m = self.m.lock().unwrap();
        let now = Instant::now();
        if m.len() > 4096 {
            m.retain(|_, (t, _)| now.duration_since(*t) < Duration::from_secs(60));
        }
        let e = m.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(e.0) >= Duration::from_secs(60) {
            *e = (now, 0);
        }
        e.1 += 1;
        if e.1 > self.per_min { Err(60u64.saturating_sub(now.duration_since(e.0).as_secs()).max(1)) } else { Ok(()) }
    }
}

/// Removes server paths from text that goes to a client (PLAN 5.4): the configured roots first, then anything that still
/// looks like a drive path. `shortcut: Unix absolute paths are removed only when they sit under a configured root.`
pub struct Redactor {
    roots: Vec<String>,
}

impl Redactor {
    pub fn new(roots: &[&Path]) -> Redactor {
        let mut v = vec![];
        for r in roots {
            let s = r.display().to_string();
            if s.len() > 3 {
                v.push(s.replace('\\', "/"));
                v.push(s.replace('/', "\\"));
            }
        }
        v.sort_by_key(|s| std::cmp::Reverse(s.len()));
        v.dedup();
        Redactor { roots: v }
    }

    pub fn scrub(&self, s: &str) -> String {
        let mut out = s.to_string();
        for r in &self.roots {
            let rl = r.to_ascii_lowercase();
            while let Some(i) = out.to_ascii_lowercase().find(&rl) {
                out.replace_range(i..i + r.len(), "<server>");
            }
        }
        // a drive path that is not under a root: `X:\...` or `X:/...` up to a space or a quote
        let b: Vec<char> = out.chars().collect();
        let mut res = String::new();
        let mut i = 0;
        while i < b.len() {
            let drive = b[i].is_ascii_alphabetic() && b.get(i + 1) == Some(&':') && matches!(b.get(i + 2), Some('\\' | '/')) && (i == 0 || !b[i - 1].is_alphanumeric());
            if drive {
                res.push_str("<path>");
                i += 3;
                while i < b.len() && !b[i].is_whitespace() && !"\"'<>|)".contains(b[i]) {
                    i += 1;
                }
            } else {
                res.push(b[i]);
                i += 1;
            }
        }
        res
    }

    pub fn scrub_value(&self, v: &mut Value) {
        match v {
            Value::String(s) => *s = self.scrub(s),
            Value::Array(a) => a.iter_mut().for_each(|x| self.scrub_value(x)),
            Value::Object(o) => o.values_mut().for_each(|x| self.scrub_value(x)),
            _ => {}
        }
    }
}

pub struct App {
    pub r: Resolved,
    pub cfg: Arc<ConvertConfig>,
    pub jobs: Mutex<HashMap<String, Arc<Job>>>,
    pub cpu: Arc<Semaphore>,
    pub mem: Arc<Semaphore>,
    pub user_rl: Limiter,
    pub ip_rl: Limiter,
    pub auth_rl: Limiter,
    pub redact: Redactor,
    pub tls: bool,
    pub allowed_hosts: Vec<String>,
    garbage: Mutex<Vec<PathBuf>>,
    pub(crate) health: Mutex<Option<(Instant, bool)>>,
}

fn rand_hex() -> String {
    use aes_gcm::aead::{OsRng, rand_core::RngCore};
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn is_job_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl App {
    /// Build the state and clean up after a crashed predecessor: directories of dead processes are removed before anything is served.
    pub fn new(r: Resolved, mut cfg: ConvertConfig, tls: bool) -> Result<Arc<App>, String> {
        for d in [&r.work_dir, &r.store_dir] {
            std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
            job::sweep_stale(d);
        }
        cfg.work_dir = r.work_dir.clone();
        cfg.max_input_mb = r.s.max_file_mb.max(r.s.max_xlsx_mb);
        cfg.job_timeout_s = r.s.job_timeout_s;
        // server mode: fonts come from the image, not from a user profile
        cfg.share_localappdata = !r.server_mode();
        let mut roots: Vec<PathBuf> = vec![r.work_dir.clone(), r.store_dir.clone(), std::env::temp_dir()];
        roots.extend(convert::plan::ToolId::ALL.iter().filter_map(|t| cfg.resolve(*t).ok()).filter_map(|p| p.parent().map(Path::to_path_buf)));
        let root_refs: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
        let host = |a: &str| a.to_ascii_lowercase();
        let mut allowed = vec![];
        if let Some(u) = &r.s.public_url {
            let auth = u.split_once("://").map_or(u.as_str(), |x| x.1).trim_end_matches('/');
            allowed.push(host(auth));
            if let Some(h) = auth.strip_suffix(":443") {
                allowed.push(host(h));
            }
        } else if !r.server_mode() {
            let p = r.listen.port();
            allowed.extend([format!("127.0.0.1:{p}"), format!("localhost:{p}"), format!("[::1]:{p}"), r.listen.to_string()]);
        }
        Ok(Arc::new(App {
            cpu: Arc::new(Semaphore::new(r.cpu_slots)),
            mem: Arc::new(Semaphore::new(r.s.mem_budget_mb.min(u32::MAX as u64 / 2) as usize)),
            user_rl: Limiter::new(r.s.rate_user_per_min),
            ip_rl: Limiter::new(r.s.rate_ip_per_min),
            auth_rl: Limiter::new(r.s.rate_ip_per_min.min(30)),
            redact: Redactor::new(&root_refs),
            cfg: Arc::new(cfg),
            jobs: Mutex::new(HashMap::new()),
            tls,
            allowed_hosts: allowed,
            garbage: Mutex::new(vec![]),
            health: Mutex::new(None),
            r,
        }))
    }

    /// Create a job record and its directory. Counted at once, so two simultaneous requests cannot both pass the limits.
    pub fn admit(&self, owner: &str) -> Result<Arc<Job>, ApiError> {
        let mut jobs = self.jobs.lock().unwrap();
        let (mut total, mut mine) = (0, 0);
        for j in jobs.values().filter(|j| j.active.load(Ordering::Relaxed)) {
            total += 1;
            mine += (j.owner == owner) as usize;
        }
        if total >= self.r.s.queue_max {
            return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "queue_full", "the job queue is full; try again shortly").retry(30));
        }
        if mine >= self.r.s.per_user_active {
            return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "too_many_active_jobs", format!("at most {} active jobs per user", self.r.s.per_user_active)).retry(10));
        }
        let id = rand_hex();
        let dir = self.r.store_dir.join(format!("stage-{}-api-{id}", std::process::id()));
        let (codec, key) = match self.r.operator_key {
            Some(op) => {
                let k = JobKey::generate();
                let wrapped = k.wrap(&op, &id);
                (Codec::Aes(k), Some(wrapped))
            }
            None => (Codec::Plain, None),
        };
        let io = |e: std::io::Error| ApiError::internal(format!("cannot create the job store: {e}"));
        std::fs::create_dir_all(dir.join("in")).map_err(io)?;
        std::fs::create_dir_all(dir.join("out")).map_err(io)?;
        if let Some(w) = key {
            std::fs::write(dir.join("key"), w).map_err(io)?;
        }
        let job = Arc::new(Job {
            id: id.clone(),
            owner: owner.to_string(),
            dir,
            codec,
            cancel: Arc::new(AtomicBool::new(false)),
            st: Mutex::new(JobState::default()),
            zip_size: AtomicU64::new(0),
            zip_lock: Mutex::new(()),
            active: AtomicBool::new(true),
            life: Mutex::new(Life::default()),
        });
        jobs.insert(id, job.clone());
        Ok(job)
    }

    pub fn get(&self, id: &str, owner: &str) -> Option<Arc<Job>> {
        if !is_job_id(id) {
            return None;
        }
        self.jobs.lock().unwrap().get(id).filter(|j| j.owner == owner).cloned()
    }

    pub fn queue_depth(&self) -> usize {
        self.jobs.lock().unwrap().values().filter(|j| j.active.load(Ordering::Relaxed)).count()
    }

    /// The key goes first, then the data: a leftover without its key file is unreadable.
    fn purge(&self, job: &Job) {
        let _ = std::fs::remove_file(job.dir.join("key"));
        if std::fs::remove_dir_all(&job.dir).is_err() && job.dir.exists() {
            // a download still holds a file (Windows): the sweeper retries
            self.garbage.lock().unwrap().push(job.dir.clone());
        }
    }

    /// The supervisor calls this when the last file task ends.
    pub fn finish(&self, job: &Job) {
        job.st.lock().unwrap().finished_at = Some(Instant::now());
        job.active.store(false, Ordering::Relaxed);
        let mut l = job.life.lock().unwrap();
        l.finished = true;
        if l.deleted {
            self.purge(job);
        }
    }

    /// `DELETE`: stop the running steps and remove the job (now, or when the last step has ended).
    pub fn delete(&self, job: &Arc<Job>) {
        self.jobs.lock().unwrap().remove(&job.id);
        job.cancel.store(true, Ordering::Relaxed);
        let mut l = job.life.lock().unwrap();
        l.deleted = true;
        if l.finished {
            self.purge(job);
        }
    }

    /// Expired results go, and directories that could not be removed before are tried again.
    pub fn sweep(&self) {
        let ttl = Duration::from_secs(self.r.s.retention_s);
        let old: Vec<Arc<Job>> = self.jobs.lock().unwrap().values().filter(|j| j.st.lock().unwrap().finished_at.is_some_and(|t| t.elapsed() >= ttl)).cloned().collect();
        for j in old {
            self.delete(&j);
        }
        let retry: Vec<PathBuf> = std::mem::take(&mut *self.garbage.lock().unwrap());
        for d in retry {
            let _ = std::fs::remove_file(d.join("key"));
            if std::fs::remove_dir_all(&d).is_err() && d.exists() {
                self.garbage.lock().unwrap().push(d);
            }
        }
    }

    /// Graceful stop: cancel and remove everything.
    pub fn shutdown(&self) {
        let all: Vec<Arc<Job>> = self.jobs.lock().unwrap().values().cloned().collect();
        for j in all {
            self.delete(&j);
        }
    }

    /// The report a client may see: ids and names, never a server path.
    pub fn public_report(&self, rep: &FileReport, outputs: &[OutputEntry]) -> Value {
        let mut v = serde_json::to_value(rep).expect("report serialises");
        v["outputs"] = json!(outputs.iter().map(|o| o.name.clone()).collect::<Vec<_>>());
        self.redact.scrub_value(&mut v);
        v
    }
}

/// How much memory a conversion reserves before it starts (PLAN 5.2): 20 MB per input MB for sheets, 40 for the rest, plus a
/// base, capped at what the tool is allowed to use anyway.
pub fn estimate_mb(kind: Kind, size_bytes: u64) -> u64 {
    let mb = size_bytes.div_ceil(1 << 20);
    let (class, per_mb) = match kind.family() {
        Family::Docs => (Class::Docs, 40),
        Family::Sheets => (Class::SheetsConvert, 20),
        Family::Slides => (Class::Slides, 40),
        Family::Pdf => (Class::Pdf, 40),
    };
    (256 + per_mb * mb).min(limits(class).mem_mb)
}

pub struct StoredInput {
    pub name: String,
    pub path: PathBuf,
    pub codec: Codec,
}

impl InputSource for StoredInput {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn open(&self) -> std::io::Result<Box<dyn Read + '_>> {
        Ok(Box::new(self.codec.open(&self.path)?))
    }
    fn label(&self) -> String {
        self.name.clone()
    }
}

pub struct JobSink {
    pub dir: PathBuf,
    pub codec: Codec,
    pub outs: Mutex<Vec<OutputEntry>>,
}

/// `a.pdf`, `a (2).pdf`, ... against the names already taken.
pub fn dedupe(taken: &[&str], name: &str) -> String {
    let p = Path::new(name);
    let (stem, ext) = (p.file_stem().and_then(|s| s.to_str()).unwrap_or("output"), p.extension().and_then(|s| s.to_str()));
    (1..)
        .map(|n| match (n, ext) {
            (1, _) => name.to_string(),
            (_, Some(e)) => format!("{stem} ({n}).{e}"),
            (_, None) => format!("{stem} ({n})"),
        })
        .find(|c| !taken.contains(&c.as_str()))
        .expect("an unbounded range has a free name")
}

impl OutputSink for JobSink {
    fn begin(&self, _count: usize, _origins: &[PathBuf]) -> Result<(), Failure> {
        std::fs::create_dir_all(&self.dir).or_else(|e| fail(Exit::ToolError, format!("cannot create the output store: {e}")))
    }
    fn put(&self, index: usize, name: &str, src: &Path) -> Result<String, Failure> {
        let size = self.codec.store_file(src, &self.dir.join(index.to_string())).or_else(|e| fail(Exit::ToolError, format!("cannot store the output: {e}")))?;
        let mut outs = self.outs.lock().unwrap();
        let taken: Vec<&str> = outs.iter().map(|o| o.name.as_str()).collect();
        let name = dedupe(&taken, name);
        outs.push(OutputEntry { name, size });
        Ok(index.to_string())
    }
    fn discard(&self, _placed: &[String]) {
        self.outs.lock().unwrap().clear();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One file of a job: wait for a CPU slot, then for its memory, then convert.
pub async fn run_file(app: Arc<App>, job: Arc<Job>, n: usize, target: Target, opts: Options, est_mb: u64, client: String) {
    let set = |f: &dyn Fn(&mut FileState)| f(&mut job.st.lock().unwrap().files[n]);
    // CPU first, then memory: a holder of both always finishes, so the two gates cannot deadlock
    let cpu = app.cpu.clone().acquire_owned().await;
    let mem = app.mem.clone().acquire_many_owned(est_mb as u32).await;
    let name = job.st.lock().unwrap().files[n].name.clone();
    if job.cancel.load(Ordering::Relaxed) || cpu.is_err() || mem.is_err() {
        let rep = convert_failed(&name, target, Exit::Timeout, "the job was cancelled");
        set(&|f| {
            f.state = "failed";
            f.report = Some(app.public_report(&rep, &[]));
        });
        return;
    }
    set(&|f| f.state = "running");
    let sink = Arc::new(JobSink { dir: job.dir.join("out").join(n.to_string()), codec: job.codec.clone(), outs: Mutex::new(vec![]) });
    let (a, j, s) = (app.clone(), job.clone(), sink.clone());
    let t0 = Instant::now();
    let rep = tokio::task::spawn_blocking(move || {
        let input = StoredInput { name: a.name_of(&j, n), path: j.in_path(n), codec: j.codec.clone() };
        let meta = JobMeta { principal: Some(j.owner.clone()), client: Some(client), cancel: Some(j.cancel.clone()) };
        job::guarded(&input.name, target.ext(), || job::convert_source(&a.cfg, &input, target, &opts, &*s, &meta))
    })
    .await
    .unwrap_or_else(|e| convert_failed(&name, target, Exit::Crash, &format!("internal error: {e}")));
    let outs = std::mem::take(&mut *sink.outs.lock().unwrap());
    let ok = rep.code == 0 || rep.code == 10;
    let public = app.public_report(&rep, &outs);
    eprintln!("job={} sub={} file={n} kind={} to={} code={} ms={}", job.id, job.owner, rep.kind.as_deref().unwrap_or("?"), rep.target, rep.code, t0.elapsed().as_millis());
    set(&|f| {
        f.state = if ok { "done" } else { "failed" };
        f.step = rep.steps.last().map(|s| s.tool.clone());
        f.report = Some(public.clone());
    });
    if ok {
        job.st.lock().unwrap().files[n].outputs = outs;
    }
}

impl App {
    fn name_of(&self, job: &Job, n: usize) -> String {
        job.st.lock().unwrap().files[n].name.clone()
    }
}

pub fn convert_failed(label: &str, target: Target, exit: Exit, msg: &str) -> FileReport {
    FileReport {
        schema: convert::report::SCHEMA,
        input: label.to_string(),
        kind: None,
        target: target.ext().into(),
        status: "failed",
        code: exit.code(),
        message: msg.to_string(),
        steps: vec![],
        warnings: vec![],
        translit: None,
        verified: None,
        outputs: vec![],
    }
}
