//! The job store, the queue with its two gates (a fair CPU gate and a memory budget), the quotas, the sweeper and the report
//! scrubbing (PLAN 5.2, 5.4). Everything is in memory; a restart loses the jobs (their files are swept at the next start).
use crate::config::Resolved;
use crate::crypt::{Codec, JobKey};
use crate::gate::Gate;
use crate::routes::ApiError;
use axum::http::StatusCode;
use convert::exec::{Class, Config as ConvertConfig, Exit, Failure, fail, limits};
use convert::job::{self, FileReport, InputSource, JobMeta, OutputSink};
use convert::kind::{Family, Kind};
use convert::plan::{Options, Target};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Read;
use std::net::IpAddr;
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
    /// the size of `in/<n>` while it is on disk (it is deleted as soon as the file's run ends)
    pub in_bytes: u64,
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
    /// the end of the whole job, for all its files together (PLAN 5.3: 30 minutes from admission)
    pub deadline: Instant,
    /// what this job holds in the store: a reservation of the batch limit while the upload runs, then the inputs still on disk,
    /// the outputs and the zip
    pub bytes: AtomicU64,
    active: AtomicBool,
    deleted: AtomicBool,
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
    /// The names a blob is bound to in the AAD (`crypt.rs`).
    pub fn in_role(n: usize) -> String {
        format!("in/{n}")
    }
    pub fn out_role(n: usize, k: usize) -> String {
        format!("out/{n}/{k}")
    }
    pub const ZIP_ROLE: &'static str = "zip";
    pub fn is_finished(&self) -> bool {
        self.st.lock().unwrap().finished_at.is_some()
    }
    pub fn is_deleted(&self) -> bool {
        self.deleted.load(Ordering::Relaxed)
    }
    pub fn add_bytes(&self, n: u64) {
        self.bytes.fetch_add(n, Ordering::Relaxed);
    }
    /// An input is never served back, so it goes as soon as its file has run (or failed its check): the store then holds outputs only.
    pub fn drop_input(&self, n: usize) {
        let _ = std::fs::remove_file(self.in_path(n));
        let freed = self
            .st
            .lock()
            .unwrap()
            .files
            .get_mut(n)
            .map_or(0, |f| std::mem::take(&mut f.in_bytes));
        let _ = self
            .bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |b| {
                Some(b.saturating_sub(freed))
            });
    }
}

/// The key of a client address for the rate limits and the connection caps: an IPv6 address counts by its /64, because a host on
/// a LAN owns the whole prefix (WP7 review M6).
pub fn ip_key(ip: IpAddr) -> String {
    match ip.to_canonical() {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => {
            let s = v.segments();
            format!("v6:{:x}:{:x}:{:x}:{:x}", s[0], s[1], s[2], s[3])
        }
    }
}

/// The fixed-window counter behind the rate limits: `hit` is Ok, or the seconds until the window ends. The map is capped: when it
/// is full of live windows, a key that is not in it is refused rather than evicting somebody else's.
pub struct Limiter {
    per_min: u32,
    m: Mutex<HashMap<String, (Instant, u32)>>,
}

const LIMITER_KEYS: usize = 16_384;

impl Limiter {
    pub fn new(per_min: u32) -> Limiter {
        Limiter {
            per_min,
            m: Mutex::new(HashMap::new()),
        }
    }
    pub fn hit(&self, key: &str) -> Result<(), u64> {
        let mut m = self.m.lock().unwrap();
        let now = Instant::now();
        if m.len() >= 4096 {
            m.retain(|_, (t, _)| now.duration_since(*t) < Duration::from_secs(60));
        }
        if m.len() >= LIMITER_KEYS && !m.contains_key(key) {
            return Err(60);
        }
        let e = m.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(e.0) >= Duration::from_secs(60) {
            *e = (now, 0);
        }
        e.1 += 1;
        if e.1 > self.per_min {
            Err(60u64
                .saturating_sub(now.duration_since(e.0).as_secs())
                .max(1))
        } else {
            Ok(())
        }
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
            let drive = b[i].is_ascii_alphabetic()
                && b.get(i + 1) == Some(&':')
                && matches!(b.get(i + 2), Some('\\' | '/'))
                && (i == 0 || !b[i - 1].is_alphanumeric());
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
    pub cpu: Gate,
    pub mem: Arc<Semaphore>,
    pub user_rl: Limiter,
    pub ip_rl: Limiter,
    pub auth_rl: Limiter,
    pub redact: Redactor,
    pub tls: bool,
    pub allowed_hosts: Vec<String>,
    garbage: Mutex<Vec<PathBuf>>,
    pub(crate) health: Mutex<Option<(Instant, bool)>>,
    free: Mutex<Option<(Instant, Option<u64>)>>,
}

fn rand_hex() -> String {
    use aes_gcm::aead::{OsRng, rand_core::RngCore};
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn is_job_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Is `path` on a RAM-backed filesystem according to the text of `/proc/mounts`? `None` when the mount list does not say.
pub fn ram_backed(mounts: &str, path: &Path) -> Option<bool> {
    let p = path.to_string_lossy().replace('\\', "/");
    mounts
        .lines()
        .filter_map(|l| {
            let mut c = l.split_whitespace();
            let (_dev, mp, fs) = (c.next()?, c.next()?, c.next()?);
            let under = p == mp || p.starts_with(&format!("{}/", mp.trim_end_matches('/')));
            under.then_some((mp.len(), matches!(fs, "tmpfs" | "ramfs")))
        })
        .max_by_key(|x| x.0)
        .map(|x| x.1)
}

/// Server mode keeps plaintext in `work_dir` while a job runs, and a crash leaves it behind. On a RAM disk it dies with the process
/// or the power; on a disk it stays in free clusters (docs/phase3/api.md, "what is plaintext"). Say so loudly at every start.
fn warn_about_disks(r: &Resolved) {
    if !r.server_mode() {
        return;
    }
    let mounts = if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/mounts").ok()
    } else {
        None
    };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let bar = "*".repeat(78);
    for (what, dir, loud) in [
        ("work_dir", &r.work_dir, true),
        ("store_dir", &r.store_dir, false),
    ] {
        match mounts.as_deref().and_then(|m| ram_backed(m, &real(dir))) {
            Some(true) => {}
            Some(false) if loud => eprintln!(
                "convert-api: {bar}\nconvert-api: WARNING: {what} {} is NOT on tmpfs. Every upload is decrypted into it while it is converted, and a crash\nconvert-api: leaves that plaintext on the disk. Mount a tmpfs there (docs/phase3/api.md, \"Plaintext on disk\").\nconvert-api: {bar}",
                dir.display()
            ),
            None if loud => eprintln!(
                "convert-api: {bar}\nconvert-api: WARNING: cannot tell whether {what} {} is RAM-backed. On Windows put it on a RAM disk and turn on\nconvert-api: BitLocker for the system volume (page file, crash dumps); on Linux use a tmpfs (docs/phase3/api.md).\nconvert-api: {bar}",
                dir.display()
            ),
            _ => eprintln!(
                "convert-api: note: {what} {} is not on a RAM disk; it holds ciphertext only, which is fine, but size it for store_max_mb",
                dir.display()
            ),
        }
    }
}

/// The free space of the volume that holds `dir`, from the mount whose path is the longest prefix of it.
fn free_space(dir: &Path) -> Option<u64> {
    let canon = std::fs::canonicalize(dir).ok()?;
    let mut s = canon.to_string_lossy().replace('\\', "/");
    if let Some(x) = s.strip_prefix("//?/") {
        s = x.to_string();
    }
    let s = s.to_lowercase();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter_map(|d| {
            let m = d
                .mount_point()
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();
            s.starts_with(m.trim_end_matches('/'))
                .then(|| (m.len(), d.available_space()))
        })
        .max_by_key(|x| x.0)
        .map(|x| x.1)
}

impl App {
    /// Build the state and clean up after a crashed predecessor: directories of dead processes are removed before anything is served.
    pub fn new(r: Resolved, mut cfg: ConvertConfig, tls: bool) -> Result<Arc<App>, String> {
        for d in [&r.work_dir, &r.store_dir] {
            std::fs::create_dir_all(d)
                .map_err(|e| format!("cannot create {}: {e}", d.display()))?;
            job::sweep_stale(d);
        }
        warn_about_disks(&r);
        if r.xdfu != convert::xdfu::Mode::Off {
            // WP15: opt-in. The hook (detection, watermark, audit) is installed here and nowhere else, so with `off` nothing of it exists.
            let path =
                r.s.xdfu_audit_file
                    .clone()
                    .unwrap_or_else(convert::xdfu::default_audit_path);
            let log = convert::audit::AuditLog::open(&path, r.s.xdfu_redact)?;
            eprintln!(
                "convert-api: XDFU handling is ON ({:?}); audit log {}",
                r.xdfu,
                path.display()
            );
            if !r.server_mode() {
                eprintln!(
                    "convert-api: note: local mode on an uncertified computer can only watermark and warn; it does not make XDFU handling compliant (PLAN 10)"
                );
            }
            cfg.hooks.push(Arc::new(convert::xdfu::XdfuHook::new(
                r.xdfu,
                Arc::new(log),
                !r.server_mode(),
            )));
        }
        cfg.work_dir = r.work_dir.clone();
        cfg.max_input_mb = r.s.max_file_mb.max(r.s.max_xlsx_mb);
        cfg.job_timeout_s = r.s.job_timeout_s;
        // server mode: fonts come from the image, not from a user profile
        cfg.share_localappdata = !r.server_mode();
        let mut roots: Vec<PathBuf> = vec![
            r.work_dir.clone(),
            r.store_dir.clone(),
            std::env::temp_dir(),
        ];
        roots.extend(
            convert::plan::ToolId::ALL
                .iter()
                .filter_map(|t| cfg.resolve(*t).ok())
                .filter_map(|p| p.parent().map(Path::to_path_buf)),
        );
        // the sidecar's address is internal too: it must not reach a client in a message
        roots.extend(
            cfg.pdf2docx_url
                .iter()
                .map(|u| PathBuf::from(u.trim_end_matches('/'))),
        );
        let root_refs: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
        let host = |a: &str| a.to_ascii_lowercase();
        let mut allowed = vec![];
        if let Some(u) = &r.s.public_url {
            let auth = u
                .split_once("://")
                .map_or(u.as_str(), |x| x.1)
                .trim_end_matches('/');
            allowed.push(host(auth));
            if let Some(h) = auth.strip_suffix(":443") {
                allowed.push(host(h));
            }
        } else if !r.server_mode() {
            let p = r.listen.port();
            allowed.extend([
                format!("127.0.0.1:{p}"),
                format!("localhost:{p}"),
                format!("[::1]:{p}"),
                r.listen.to_string(),
            ]);
        }
        Ok(Arc::new(App {
            cpu: Gate::new(r.cpu_slots),
            mem: Arc::new(Semaphore::new(
                r.s.mem_budget_mb.min(u32::MAX as u64 / 2) as usize
            )),
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
            free: Mutex::new(None),
            r,
        }))
    }

    /// The free space of the store volume, looked up at most every 5 s. `None` when the volume cannot be found (no floor is applied).
    fn store_free(&self) -> Option<u64> {
        let mut c = self.free.lock().unwrap();
        if let Some((t, v)) = *c
            && t.elapsed() < Duration::from_secs(5)
        {
            return v;
        }
        let v = free_space(&self.r.store_dir);
        *c = Some((Instant::now(), v));
        v
    }

    /// Create a job record and its directory. Counted at once, so two simultaneous requests cannot both pass the limits.
    /// The refusals, cheapest first: the queue, the user's active jobs, the store quota, the user's retained data, the disk floor.
    pub fn admit(&self, owner: &str) -> Result<Arc<Job>, ApiError> {
        let s = &self.r.s;
        let reserve = s.max_batch_mb << 20;
        let free = self.store_free();
        let mut jobs = self.jobs.lock().unwrap();
        let (mut total, mut mine, mut stored, mut my_stored) = (0, 0, 0u64, 0u64);
        for j in jobs.values() {
            let b = j.bytes.load(Ordering::Relaxed);
            stored += b;
            if j.owner == owner {
                my_stored += b;
            }
            if j.active.load(Ordering::Relaxed) {
                total += 1;
                mine += (j.owner == owner) as usize;
            }
        }
        if total >= s.queue_max {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_full",
                "the job queue is full; try again shortly",
            )
            .retry(30));
        }
        if mine >= s.per_user_active {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_active_jobs",
                format!("at most {} active jobs per user", s.per_user_active),
            )
            .retry(10));
        }
        if stored.saturating_add(reserve) > s.store_max_mb << 20 {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "store_full",
                "the server's job store is full; try again later or delete finished jobs",
            )
            .retry(60));
        }
        if my_stored.saturating_add(reserve) > s.user_store_mb << 20 {
            return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "user_store_full", format!("you keep more than {} MB of results on the server; download and delete finished jobs first", s.user_store_mb)).retry(60));
        }
        if free.is_some_and(|f| f < (s.min_free_mb + s.max_batch_mb) << 20) {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "disk_low",
                "the server is low on disk space; try again later",
            )
            .retry(60));
        }
        let id = rand_hex();
        let dir = self
            .r
            .store_dir
            .join(format!("stage-{}-api-{id}", std::process::id()));
        let codec = if self.r.encrypt() {
            Codec::Aes(JobKey::generate(&id))
        } else {
            Codec::Plain
        };
        let io =
            |e: std::io::Error| ApiError::internal(format!("cannot create the job store: {e}"));
        std::fs::create_dir_all(dir.join("in")).map_err(io)?;
        std::fs::create_dir_all(dir.join("out")).map_err(io)?;
        let job = Arc::new(Job {
            id: id.clone(),
            owner: owner.to_string(),
            dir,
            codec,
            cancel: Arc::new(AtomicBool::new(false)),
            st: Mutex::new(JobState::default()),
            zip_size: AtomicU64::new(0),
            zip_lock: Mutex::new(()),
            deadline: Instant::now() + Duration::from_secs(s.job_timeout_s),
            bytes: AtomicU64::new(reserve),
            active: AtomicBool::new(true),
            deleted: AtomicBool::new(false),
            life: Mutex::new(Life::default()),
        });
        jobs.insert(id, job.clone());
        Ok(job)
    }

    pub fn get(&self, id: &str, owner: &str) -> Option<Arc<Job>> {
        if !is_job_id(id) {
            return None;
        }
        self.jobs
            .lock()
            .unwrap()
            .get(id)
            .filter(|j| j.owner == owner && !j.is_deleted())
            .cloned()
    }

    pub fn queue_depth(&self) -> usize {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| j.active.load(Ordering::Relaxed))
            .count()
    }

    /// What the store holds, reservations included (for the quota and the tests).
    pub fn stored_bytes(&self) -> u64 {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .map(|j| j.bytes.load(Ordering::Relaxed))
            .sum()
    }

    fn purge(&self, job: &Job) {
        if std::fs::remove_dir_all(&job.dir).is_err() && job.dir.exists() {
            // a download still holds a file (Windows): the sweeper retries
            self.garbage.lock().unwrap().push(job.dir.clone());
        }
        self.jobs.lock().unwrap().remove(&job.id);
    }

    /// The supervisor calls this when the last file task ends.
    pub fn finish(&self, job: &Job) {
        job.st.lock().unwrap().finished_at = Some(Instant::now());
        job.active.store(false, Ordering::Relaxed);
        let purge = {
            let mut l = job.life.lock().unwrap();
            l.finished = true;
            l.deleted
        };
        if purge {
            self.purge(job);
        }
    }

    /// `DELETE`: stop the running steps and remove the job (now, or when the last step has ended). Until then it still counts
    /// against the limits, because its work is still running (WP7 review m12), but it can no longer be fetched.
    pub fn delete(&self, job: &Arc<Job>) {
        job.cancel.store(true, Ordering::Relaxed);
        job.deleted.store(true, Ordering::Relaxed);
        let purge = {
            let mut l = job.life.lock().unwrap();
            l.deleted = true;
            l.finished
        };
        if purge {
            self.purge(job);
        }
    }

    /// Expired results go, and directories that could not be removed before are tried again.
    pub fn sweep(&self) {
        let ttl = Duration::from_secs(self.r.s.retention_s);
        let old: Vec<Arc<Job>> = self
            .jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| {
                j.st.lock()
                    .unwrap()
                    .finished_at
                    .is_some_and(|t| t.elapsed() >= ttl)
            })
            .cloned()
            .collect();
        for j in old {
            self.delete(&j);
        }
        let retry: Vec<PathBuf> = std::mem::take(&mut *self.garbage.lock().unwrap());
        for d in retry {
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
    /// the AAD role of the blob (`in/<n>`)
    pub role: String,
}

impl InputSource for StoredInput {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn open(&self) -> std::io::Result<Box<dyn Read + '_>> {
        Ok(Box::new(self.codec.open(&self.path, &self.role)?))
    }
    fn label(&self) -> String {
        self.name.clone()
    }
}

pub struct JobSink {
    pub dir: PathBuf,
    pub codec: Codec,
    pub outs: Mutex<Vec<OutputEntry>>,
    /// the file number, for the AAD role of each output
    pub n: usize,
}

/// `a.pdf`, `a (2).pdf`, ... against the names already taken.
pub fn dedupe(taken: &[&str], name: &str) -> String {
    let p = Path::new(name);
    let (stem, ext) = (
        p.file_stem().and_then(|s| s.to_str()).unwrap_or("output"),
        p.extension().and_then(|s| s.to_str()),
    );
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
        std::fs::create_dir_all(&self.dir).or_else(|e| {
            fail(
                Exit::ToolError,
                format!("cannot create the output store: {e}"),
            )
        })
    }
    fn put(&self, index: usize, name: &str, src: &Path) -> Result<String, Failure> {
        let size = self
            .codec
            .store_file(
                src,
                &self.dir.join(index.to_string()),
                &Job::out_role(self.n, index),
            )
            .or_else(|e| fail(Exit::ToolError, format!("cannot store the output: {e}")))?;
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

/// Deletes `in/<n>` when the file's task ends, however it ends (done, failed, cancelled, panicked).
struct DropInput(Arc<Job>, usize);
impl Drop for DropInput {
    fn drop(&mut self) {
        self.0.drop_input(self.1);
    }
}

/// One file of a job: wait for a CPU slot (fairly, by user), then for its memory, then convert. Waiting ends at the job's deadline.
#[allow(clippy::too_many_arguments)] // shortcut: one call shape for the supervisor; fold into a struct when a 9th arrives
pub async fn run_file(
    app: Arc<App>,
    job: Arc<Job>,
    n: usize,
    target: Target,
    opts: Options,
    est_mb: u64,
    client: String,
    verdict: Arc<convert::scan::Verdict>,
) {
    let _input = DropInput(job.clone(), n);
    let set = |f: &dyn Fn(&mut FileState)| f(&mut job.st.lock().unwrap().files[n]);
    let name = job.st.lock().unwrap().files[n].name.clone();
    let give_up = |exit: Exit, why: &str| {
        let rep = convert_failed(&name, target, exit, why);
        set(&|f| {
            f.state = "failed";
            f.report = Some(app.public_report(&rep, &[]));
        });
    };
    let by = tokio::time::Instant::from_std(job.deadline);
    // CPU first, then memory: a holder of both always finishes, so the two gates cannot deadlock
    let Ok(cpu) = tokio::time::timeout_at(by, app.cpu.acquire(&job.owner)).await else {
        return give_up(
            Exit::Timeout,
            "the job waited for a free slot until its time limit ran out",
        );
    };
    let Ok(mem) =
        tokio::time::timeout_at(by, app.mem.clone().acquire_many_owned(est_mb as u32)).await
    else {
        return give_up(
            Exit::Timeout,
            "the job waited for memory until its time limit ran out",
        );
    };
    if job.cancel.load(Ordering::Relaxed) || mem.is_err() {
        return give_up(Exit::Timeout, "the job was cancelled");
    }
    set(&|f| f.state = "running");
    let sink = Arc::new(JobSink {
        dir: job.dir.join("out").join(n.to_string()),
        codec: job.codec.clone(),
        outs: Mutex::new(vec![]),
        n,
    });
    let (a, j, s) = (app.clone(), job.clone(), sink.clone());
    let t0 = Instant::now();
    let rep = tokio::task::spawn_blocking(move || {
        let input = StoredInput {
            name: a.name_of(&j, n),
            path: j.in_path(n),
            codec: j.codec.clone(),
            role: Job::in_role(n),
        };
        let meta = JobMeta {
            principal: Some(j.owner.clone()),
            client: Some(client),
            cancel: Some(j.cancel.clone()),
            deadline: Some(j.deadline),
            verdict: Some(verdict),
            reference: Some(format!("{}/{}", j.id, n)),
        };
        job::guarded(&input.name, target.ext(), || {
            job::convert_source(&a.cfg, &input, target, &opts, &*s, &meta)
        })
    })
    .await
    .unwrap_or_else(|e| {
        convert_failed(&name, target, Exit::Crash, &format!("internal error: {e}"))
    });
    drop((cpu, mem));
    let outs = std::mem::take(&mut *sink.outs.lock().unwrap());
    let ok = rep.code == 0 || rep.code == 10;
    let public = app.public_report(&rep, &outs);
    // `sub` is quoted (Debug) so that a hostile value cannot add fields to the line
    eprintln!(
        "job={} sub={:?} file={n} kind={} to={} code={} ms={}",
        job.id,
        job.owner,
        rep.kind.as_deref().unwrap_or("?"),
        rep.target,
        rep.code,
        t0.elapsed().as_millis()
    );
    if ok {
        job.add_bytes(outs.iter().map(|o| o.size).sum());
    }
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
        xdfu: None,
        outputs: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_counts_by_its_64_and_the_limiter_map_is_capped() {
        let k = |s: &str| ip_key(s.parse().unwrap());
        assert_eq!(k("2001:db8:1:2:aaaa:bbbb:cccc:dddd"), k("2001:db8:1:2::1"));
        assert_ne!(k("2001:db8:1:2::1"), k("2001:db8:1:3::1"));
        assert_eq!(k("::ffff:203.0.113.9"), "203.0.113.9");
        let l = Limiter::new(1000);
        for i in 0..LIMITER_KEYS {
            l.hit(&format!("k{i}")).unwrap();
        }
        assert_eq!(
            l.hit("one-more"),
            Err(60),
            "a full map refuses a new key instead of evicting a live one"
        );
        assert!(l.hit("k0").is_ok());
    }

    #[test]
    fn ram_backed_follows_the_longest_mount_prefix() {
        let m = "overlay / overlay rw 0 0\ntmpfs /work tmpfs rw 0 0\n/dev/sda1 /work/disk ext4 rw 0 0\n";
        assert_eq!(ram_backed(m, Path::new("/work")), Some(true));
        assert_eq!(ram_backed(m, Path::new("/work/a/b")), Some(true));
        assert_eq!(ram_backed(m, Path::new("/work/disk/x")), Some(false));
        assert_eq!(
            ram_backed(m, Path::new("/workshop")),
            Some(false),
            "a sibling with the same prefix is on /"
        );
        assert_eq!(ram_backed("", Path::new("/work")), None);
    }
}
