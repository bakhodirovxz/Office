//! Local version history (PLAN phase 4, section 5.5, WP10): a snapshot of a document each time it is saved.
//!
//! Layout under the store root (`%LOCALAPPDATA%\<product>\history`):
//!
//! ```text
//! objects/<sha256>     the file's bytes, content-addressed, written atomically (a temporary file renamed into place)
//! docs/<id>.jsonl      one line per event of one document, append-only and hash-chained
//! .lock                held while a process changes the store (a stale one is taken over after 30 s)
//! ```
//!
//! `<id>` is the first 32 hex digits of SHA-256 over the document's canonical path (lower-cased on Windows).
//! `shortcut: the id does not follow a rename (PLAN said path plus the volume file id; reading the id needs unsafe or a Win32 crate),
//! a renamed document starts a new chain, upgrade when the host can read file ids.`
//!
//! Each line is a JSON object with sorted keys. `prev` is the `hash` of the line before it (64 zeros for the first) and `hash` is
//! SHA-256 over the line's JSON text without `hash`, the chain of `xdfu_core::audit` unkeyed (this is local mode: whoever can write
//! the folder can rewrite the chain, so it only stops a naive edit). Lines:
//!
//! - `{"kind":"version","v":N,"parent":N-1|null,"sha256","size","time","author","path","message"?,"restored_from"?}`
//! - `{"kind":"name","v":N,"name","time","author"}`: names a version (the last one wins; an empty name clears it)
//! - `{"kind":"prune","drop":[N,...],"time","author"}`: the retention rules dropped these versions and their bytes
//!
//! A restore never rewrites anything: the current file is snapshotted first, then the old bytes are written to the file and recorded
//! as a new version. Pruning appends a `prune` line, so the chain keeps every event ever made; only the bytes go away. A named version is
//! never pruned, nor is the newest one of a document.
//!
//! Two APIs share one store: the path API (desktop: the id is `doc_id(path)`, the author is the OS user) and the id API (server:
//! `snapshot_bytes`, `list_id`, `read_id`, `restore_id`, `name_id`; no file, the author is given per call). With `with_key` the chain is
//! an HMAC-SHA256 under a server key, so whoever can write the folder but lacks the key cannot rewrite a history that still verifies.
//! A torn last line (power cut during an append) is cut off before the next append, and stale `.tmp` files are swept.
//!
//! Diffs between versions are Phase 5 (WP12); this module lists, reads, restores and names.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_NAME: usize = 200;
const MAX_MESSAGE: usize = 500;

/// Retention (settings `history.*`). Sizes are in bytes so tests can use small numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub versions: u32,
    pub days: u32,
    pub max_bytes: u64,
    pub big_file_bytes: u64,
    pub big_file_versions: u32,
}

impl From<&crate::settings::History> for Limits {
    fn from(h: &crate::settings::History) -> Limits {
        Limits {
            versions: h.versions,
            days: h.days,
            max_bytes: u64::from(h.max_mb) << 20,
            big_file_bytes: u64::from(h.big_file_mb) << 20,
            big_file_versions: h.big_file_versions,
        }
    }
}

/// One version of a document, as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub v: u64,
    pub parent: Option<u64>,
    pub sha256: String,
    pub size: u64,
    pub time: u64,
    pub author: String,
    pub message: Option<String>,
    pub name: Option<String>,
    pub restored_from: Option<u64>,
    pub path: String,
}

impl Version {
    pub fn to_json(&self) -> Value {
        json!({"v": self.v, "parent": self.parent, "sha256": self.sha256, "size": self.size, "time": self.time,
               "author": self.author, "message": self.message, "name": self.name, "restored_from": self.restored_from})
    }
}

/// A document that has a history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocSummary {
    pub id: String,
    pub path: String,
    pub versions: usize,
    pub last_time: u64,
    pub size: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Verified {
    pub lines: u64,
    pub last_hash: String,
}

/// Seals the stored document bytes (the server's encryption at rest, Phase 5 WP14). `name` is authenticated with the bytes. The index
/// files hold version metadata only and are not sealed.
pub trait Sealer: Send + Sync {
    fn seal(&self, name: &str, plain: &[u8]) -> io::Result<Vec<u8>>;
    fn open(&self, name: &str, sealed: &[u8]) -> io::Result<Vec<u8>>;
    /// bytes a sealed file is longer than its plaintext
    fn overhead(&self) -> usize;
}

pub struct Store {
    sealer: Option<std::sync::Arc<dyn Sealer>>,
    root: PathBuf,
    limits: Limits,
    author: String,
    /// the chain key; `None` is the unkeyed local chain
    key: Option<Vec<u8>>,
    clock: fn() -> u64,
}

/// A `.tmp` file older than this is a leftover of a crashed write.
const TMP_MAX_AGE_S: u64 = 600;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

fn bad(m: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.into())
}

/// The OS user: the version's author.
pub fn os_user() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into())
}

/// `%LOCALAPPDATA%\<product>` (macOS `~/Library/Application Support`, else `$XDG_DATA_HOME` or `~/.local/share`): per-user data that is not
/// settings.
pub fn data_dir() -> Option<PathBuf> {
    let dir = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        dir("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        dir("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        dir("XDG_DATA_HOME").or_else(|| dir("HOME").map(|h| h.join(".local/share")))
    };
    base.map(|b| b.join(crate::PRODUCT))
}

/// Where the editors keep the XDFU audit chain (its `seq` is the copy number) and the optional operator key beside it.
pub fn xdfu_audit_paths() -> Option<(PathBuf, PathBuf)> {
    let d = data_dir()?;
    Some((d.join("xdfu-audit.jsonl"), d.join("xdfu-audit.key")))
}

/// Show the History panel of the shell for the document at `doc`: starts the desktop host (the exe next to the running editor) with
/// `--history <doc>`; a host that is already running is handed the request (single instance). The editor never waits for it.
pub fn open_panel(doc: &Path) -> io::Result<()> {
    let exe = std::env::current_exe()?.with_file_name(format!(
        "{}{}",
        env!("SUITE_PRODUCT_ID"),
        std::env::consts::EXE_SUFFIX
    ));
    std::process::Command::new(exe)
        .arg("--history")
        .arg(doc)
        .spawn()
        .map(|_| ())
}

/// The id of the document at `path`.
pub fn doc_id(path: &Path) -> String {
    let p = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = p.to_string_lossy();
    let s = if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.into_owned()
    };
    sha(s.as_bytes())[..32].to_string()
}

/// HMAC-SHA256 (RFC 2104) by hand, so suite-core takes no new dependency.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |b: u8| k.iter().map(|x| x ^ b).collect::<Vec<u8>>();
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(msg)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .to_vec()
}

fn line_hash(key: Option<&[u8]>, body: &Map<String, Value>) -> String {
    let text = serde_json::to_string(body).expect("serialises");
    match key {
        Some(k) => hex(&hmac_sha256(k, text.as_bytes())),
        None => sha(text.as_bytes()),
    }
}

/// Ids name files under `docs/`: only a plain token, never a path.
fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

fn missing(m: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, m.into())
}

/// Write `bytes` to `to` through a temporary file in the same folder.
pub fn write_atomic(to: &Path, bytes: &[u8]) -> io::Result<()> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = to.with_file_name(format!(
        ".{}.{}.{}.tmp",
        to.file_name().and_then(|n| n.to_str()).unwrap_or("f"),
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let r = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, to)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

struct Lock(PathBuf);

impl Lock {
    fn take(root: &Path) -> io::Result<Lock> {
        fs::create_dir_all(root)?;
        let p = root.join(".lock");
        for _ in 0..200 {
            match OpenOptions::new().write(true).create_new(true).open(&p) {
                Ok(_) => return Ok(Lock(p)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let old = fs::metadata(&p)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|a| a > Duration::from_secs(30));
                    if old {
                        let _ = fs::remove_file(&p);
                    } else {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the history store is locked by another process",
        ))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// One document's chain, read and checked.
struct Doc {
    lines: Vec<Map<String, Value>>,
    /// every version ever recorded, with `dropped` for the pruned ones
    all: BTreeMap<u64, (Version, bool)>,
}

impl Doc {
    fn live(&self) -> Vec<&Version> {
        self.all
            .values()
            .filter(|(_, d)| !d)
            .map(|(v, _)| v)
            .collect()
    }
    fn tail(&self) -> String {
        self.lines
            .last()
            .and_then(|l| l["hash"].as_str())
            .unwrap_or(GENESIS)
            .to_string()
    }
}

fn parse_doc(text: &str, key: Option<&[u8]>) -> Result<Doc, String> {
    let (mut lines, mut all) = (vec![], BTreeMap::new());
    let mut prev = GENESIS.to_string();
    for (i, raw) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let n = i + 1;
        let Ok(Value::Object(mut m)) = serde_json::from_str::<Value>(raw) else {
            return Err(format!("line {n}: not a JSON object"));
        };
        if m.get("prev").and_then(Value::as_str) != Some(prev.as_str()) {
            return Err(format!(
                "line {n}: the chain is broken (prev does not match)"
            ));
        }
        let hash = m
            .remove("hash")
            .and_then(|h| h.as_str().map(String::from))
            .ok_or(format!("line {n}: no hash"))?;
        if line_hash(key, &m) != hash {
            return Err(format!(
                "line {n}: the hash does not match the line (edited)"
            ));
        }
        let s = |k: &str| m.get(k).and_then(Value::as_str).map(String::from);
        let u = |k: &str| m.get(k).and_then(Value::as_u64);
        match m.get("kind").and_then(Value::as_str) {
            Some("version") => {
                let v = u("v").ok_or(format!("line {n}: no v"))?;
                let ver = Version {
                    v,
                    parent: u("parent"),
                    // joined under objects/: only a hash, never `..\` or an absolute path
                    sha256: s("sha256")
                        .filter(|h| {
                            h.len() == 64
                                && h.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
                        })
                        .ok_or(format!("line {n}: no sha256"))?,
                    size: u("size").unwrap_or(0),
                    time: u("time").unwrap_or(0),
                    author: s("author").unwrap_or_default(),
                    message: s("message"),
                    name: None,
                    restored_from: u("restored_from"),
                    path: s("path").unwrap_or_default(),
                };
                if all.insert(v, (ver, false)).is_some() {
                    return Err(format!("line {n}: version {v} is recorded twice"));
                }
            }
            Some("name") => {
                let v = u("v").unwrap_or(0);
                let name = s("name").filter(|x| !x.is_empty());
                if let Some((ver, _)) = all.get_mut(&v) {
                    ver.name = name;
                }
            }
            Some("prune") => {
                for d in m
                    .get("drop")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                {
                    if let Some((_, dropped)) = all.get_mut(&d) {
                        *dropped = true;
                    }
                }
            }
            _ => return Err(format!("line {n}: unknown kind")),
        }
        m.insert("hash".into(), hash.clone().into());
        prev = hash;
        lines.push(m);
    }
    Ok(Doc { lines, all })
}

impl Store {
    pub fn new(root: PathBuf, limits: Limits) -> Store {
        Store {
            sealer: None,
            root,
            limits,
            author: os_user(),
            key: None,
            clock: now,
        }
    }

    /// Chain the history under a server key (HMAC); a chain written without it, or with another key, fails verification.
    pub fn with_key(mut self, key: &[u8]) -> Store {
        self.key = Some(key.to_vec());
        self
    }

    /// Keep the version bytes sealed on disk (`objects/`).
    pub fn with_sealer(mut self, sealer: std::sync::Arc<dyn Sealer>) -> Store {
        self.sealer = Some(sealer);
        self
    }

    /// The stored bytes of an object, opened when the store is sealed (a file that does not open is an error, never plaintext).
    fn get_object(&self, sha: &str) -> io::Result<Vec<u8>> {
        let b = fs::read(self.object(sha))?;
        match &self.sealer {
            Some(s) => s.open(&format!("history/objects/{sha}"), &b),
            None => Ok(b),
        }
    }

    /// The clock the retention rules read (tests, or the server's own time source).
    pub fn with_clock(mut self, clock: fn() -> u64) -> Store {
        self.clock = clock;
        self
    }

    /// The store of the user's data folder with the retention of `settings.toml`.
    pub fn from_settings() -> Option<Store> {
        let s = crate::settings::load();
        Some(Store::new(
            data_dir()?.join("history"),
            Limits::from(&s.history),
        ))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn doc_file(&self, id: &str) -> PathBuf {
        self.root.join("docs").join(format!("{id}.jsonl"))
    }

    fn object(&self, sha: &str) -> PathBuf {
        self.root.join("objects").join(sha)
    }

    fn read_doc_id(&self, id: &str) -> io::Result<Doc> {
        if !valid_id(id) {
            return Err(bad("invalid document id"));
        }
        match fs::read_to_string(self.doc_file(id)) {
            Ok(t) => parse_doc(&t, self.key.as_deref()).map_err(bad),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Doc {
                lines: vec![],
                all: BTreeMap::new(),
            }),
            Err(e) => Err(e),
        }
    }

    /// `read_doc_id` for a change, under the lock: a torn last line (no newline after it) is cut off first.
    fn read_doc_for_write(&self, id: &str) -> io::Result<Doc> {
        if valid_id(id) {
            let f = self.doc_file(id);
            if let Ok(b) = fs::read(&f)
                && b.last().is_some_and(|c| *c != b'\n')
            {
                let keep = b.iter().rposition(|c| *c == b'\n').map_or(0, |i| i + 1);
                OpenOptions::new()
                    .write(true)
                    .open(&f)?
                    .set_len(keep as u64)?;
            }
        }
        self.read_doc_id(id)
    }

    fn append(
        &self,
        id: &str,
        doc: &mut Doc,
        kind: &str,
        author: &str,
        fields: Value,
    ) -> io::Result<()> {
        let mut body = match fields {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        body.insert("kind".into(), kind.into());
        body.insert("time".into(), (self.clock)().into());
        body.insert("author".into(), author.into());
        body.insert("prev".into(), doc.tail().into());
        let hash = line_hash(self.key.as_deref(), &body);
        body.insert("hash".into(), hash.into());
        let mut line = serde_json::to_string(&body).expect("serialises");
        line.push('\n');
        fs::create_dir_all(self.root.join("docs"))?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.doc_file(id))?;
        f.write_all(line.as_bytes())?;
        f.sync_data()?;
        // keep the in-memory view in step: re-parse the one line through the same code
        let again = parse_doc(
            &format!(
                "{}{}",
                doc.lines
                    .iter()
                    .map(|l| serde_json::to_string(l).unwrap() + "\n")
                    .collect::<String>(),
                line
            ),
            self.key.as_deref(),
        )
        .map_err(bad)?;
        *doc = again;
        Ok(())
    }

    fn put_object(&self, sha: &str, bytes: &[u8]) -> io::Result<()> {
        let p = self.object(sha);
        let extra = self.sealer.as_ref().map_or(0, |s| s.overhead());
        if p.is_file()
            && p.metadata()
                .is_ok_and(|m| m.len() == (bytes.len() + extra) as u64)
        {
            return Ok(());
        }
        fs::create_dir_all(self.root.join("objects"))?;
        match &self.sealer {
            Some(s) => write_atomic(&p, &s.seal(&format!("history/objects/{sha}"), bytes)?),
            None => write_atomic(&p, bytes),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_version(
        &self,
        id: &str,
        label: &str,
        bytes: &[u8],
        author: &str,
        message: Option<&str>,
        restored_from: Option<u64>,
        force: bool,
    ) -> io::Result<Option<Version>> {
        let _lock = Lock::take(&self.root)?;
        let mut doc = self.read_doc_for_write(id)?;
        let sha = sha(bytes);
        let latest = doc.live().last().map(|v| (v.v, v.sha256.clone()));
        if !force && latest.as_ref().is_some_and(|(_, h)| *h == sha) {
            return Ok(None);
        }
        self.put_object(&sha, bytes)?;
        let v = doc.all.keys().next_back().map_or(1, |l| l + 1);
        let parent = latest.map(|(p, _)| p);
        let msg: Option<String> = message.map(|m| {
            m.chars()
                .filter(|c| !c.is_control())
                .take(MAX_MESSAGE)
                .collect()
        });
        let mut f =
            json!({"v": v, "parent": parent, "sha256": sha, "size": bytes.len(), "path": label});
        if let Some(m) = msg.filter(|m| !m.is_empty()) {
            f["message"] = m.into();
        }
        if let Some(r) = restored_from {
            f["restored_from"] = r.into();
        }
        self.append(id, &mut doc, "version", author, f)?;
        let made = doc.all[&v].0.clone();
        self.prune_locked()?;
        Ok(Some(made))
    }

    /// Record the file as a new version. `None` when the file's bytes are those of the newest version (nothing changed).
    pub fn snapshot(&self, path: &Path, message: Option<&str>) -> io::Result<Option<Version>> {
        let bytes = fs::read(path)?;
        let full = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let label = full.to_string_lossy();
        self.add_version(
            &doc_id(path),
            &label,
            &bytes,
            &self.author,
            message,
            None,
            false,
        )
    }

    /// Id API: record `bytes` as a new version of document `id` by `author`. `None` when they are the newest version's bytes.
    pub fn snapshot_bytes(
        &self,
        id: &str,
        bytes: &[u8],
        author: &str,
        message: Option<&str>,
    ) -> io::Result<Option<Version>> {
        self.add_version(id, "", bytes, author, message, None, false)
    }

    /// The live versions of the document at `path`, oldest first.
    pub fn list(&self, path: &Path) -> io::Result<Vec<Version>> {
        self.list_id(&doc_id(path))
    }

    pub fn list_id(&self, id: &str) -> io::Result<Vec<Version>> {
        Ok(self.read_doc_id(id)?.live().into_iter().cloned().collect())
    }

    /// The bytes of version `v`, checked against its hash.
    pub fn read(&self, path: &Path, v: u64) -> io::Result<Vec<u8>> {
        self.read_id(&doc_id(path), v)
    }

    pub fn read_id(&self, id: &str, v: u64) -> io::Result<Vec<u8>> {
        let doc = self.read_doc_id(id)?;
        match doc.all.get(&v) {
            Some((ver, false)) => {
                let b = self.get_object(&ver.sha256)?;
                if sha(&b) != ver.sha256 {
                    return Err(bad(format!(
                        "version {v}: the stored bytes do not match their hash"
                    )));
                }
                Ok(b)
            }
            Some(_) => Err(missing(format!(
                "version {v} was removed by the retention rules"
            ))),
            None => Err(missing(format!("no version {v}"))),
        }
    }

    /// Restore version `v`: snapshot the current file (when it differs from the newest version), write the old bytes to the file and
    /// record them as a new version. Nothing already recorded changes.
    pub fn restore(&self, path: &Path, v: u64) -> io::Result<Version> {
        let old = self.read(path, v)?;
        let id = doc_id(path);
        let full = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let label = full.to_string_lossy();
        if let Ok(cur) = fs::read(path) {
            self.add_version(
                &id,
                &label,
                &cur,
                &self.author,
                Some("Before restore"),
                None,
                false,
            )?;
        }
        write_atomic(path, &old)?;
        let msg = format!("Restored from version {v}");
        self.add_version(&id, &label, &old, &self.author, Some(&msg), Some(v), true)?
            .ok_or_else(|| bad("restore recorded nothing"))
    }

    /// Id API restore: version `v` becomes a new newest version (nothing is rewritten). The caller puts the bytes where the
    /// document lives (the server: a new session version).
    pub fn restore_id(&self, id: &str, v: u64, author: &str) -> io::Result<Version> {
        let old = self.read_id(id, v)?;
        let msg = format!("Restored from version {v}");
        self.add_version(id, "", &old, author, Some(&msg), Some(v), true)?
            .ok_or_else(|| bad("restore recorded nothing"))
    }

    /// Give version `v` a name (kept for good, never pruned); an empty name clears it.
    pub fn name(&self, path: &Path, v: u64, name: &str) -> io::Result<()> {
        self.name_id(&doc_id(path), v, name, &self.author)
    }

    pub fn name_id(&self, id: &str, v: u64, name: &str, author: &str) -> io::Result<()> {
        let _lock = Lock::take(&self.root)?;
        let mut doc = self.read_doc_for_write(id)?;
        if !doc.all.get(&v).is_some_and(|(_, d)| !d) {
            return Err(missing(format!("no version {v}")));
        }
        let name: String = name
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME)
            .collect();
        self.append(
            id,
            &mut doc,
            "name",
            author,
            json!({"v": v, "name": name.trim()}),
        )
    }

    /// Id API: delete document `id` whole (its index file, then every object no other document uses). This is not a rewrite of the
    /// chain: the document is gone, and the other documents' chains are untouched. For a server whose sessions end; the path API never
    /// calls it. `false` when there was no such document.
    pub fn forget_id(&self, id: &str) -> io::Result<bool> {
        if !valid_id(id) {
            return Err(bad("invalid document id"));
        }
        if !self.doc_file(id).exists() {
            return Ok(false);
        }
        let _lock = Lock::take(&self.root)?;
        match fs::remove_file(self.doc_file(id)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            r => r?,
        }
        // the usual pass after a change: the retention rules, then the bytes nothing refers to any more
        self.prune_locked()?;
        Ok(true)
    }

    /// Check the chain of the document at `path`, and that every live version's bytes are there and match their hash.
    pub fn verify(&self, path: &Path) -> Result<Verified, String> {
        self.verify_id(&doc_id(path))
    }

    pub fn verify_id(&self, id: &str) -> Result<Verified, String> {
        self.verify_with(id, true)
    }

    /// Like `verify`, but a live version's bytes are checked by presence and size only (no hashing): cheap enough for a listing.
    /// shortcut: a same-size corruption passes; restore, copy and view still hash the bytes they read.
    pub fn verify_quick(&self, path: &Path) -> Result<Verified, String> {
        self.verify_with(&doc_id(path), false)
    }

    fn verify_with(&self, id: &str, hash: bool) -> Result<Verified, String> {
        if !valid_id(id) {
            return Err("invalid document id".into());
        }
        let text = fs::read_to_string(self.doc_file(id)).map_err(|e| e.to_string())?;
        let doc = parse_doc(&text, self.key.as_deref())?;
        for v in doc.live() {
            let missing = || format!("version {}: the stored bytes are missing", v.v);
            if hash {
                let b = self.get_object(&v.sha256).map_err(|_| missing())?;
                if sha(&b) != v.sha256 {
                    return Err(format!(
                        "version {}: the stored bytes do not match their hash",
                        v.v
                    ));
                }
            } else {
                let m = fs::metadata(self.object(&v.sha256)).map_err(|_| missing())?;
                let sealed = self.sealer.as_ref().map_or(0, |s| s.overhead() as u64);
                if m.len() != v.size + sealed {
                    return Err(format!(
                        "version {}: the stored bytes do not match their size",
                        v.v
                    ));
                }
            }
        }
        Ok(Verified {
            lines: doc.lines.len() as u64,
            last_hash: doc.tail(),
        })
    }

    /// Every document that has a history, most recently saved first.
    pub fn documents(&self) -> Vec<DocSummary> {
        let mut out: Vec<DocSummary> = fs::read_dir(self.root.join("docs"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".jsonl")?.to_string();
                let doc = self.read_doc_id(&id).ok()?;
                let live = doc.live();
                let last = live.last()?;
                Some(DocSummary {
                    id,
                    path: last.path.clone(),
                    versions: live.len(),
                    last_time: last.time,
                    size: last.size,
                })
            })
            .collect();
        out.sort_by(|a, b| b.last_time.cmp(&a.last_time).then(a.path.cmp(&b.path)));
        out
    }

    /// Delete the `.tmp` files a crashed write left in `objects/` and `docs/` (older than ten minutes: a live write is not touched).
    fn sweep_tmp(&self, t: u64) {
        for d in ["objects", "docs"] {
            for e in fs::read_dir(self.root.join(d))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                let age = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |m| t.saturating_sub(m.as_secs()));
                if e.file_name().to_string_lossy().ends_with(".tmp") && age > TMP_MAX_AGE_S {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }

    /// Apply the retention rules to every document, then delete the bytes no live version uses. Called after each new version.
    fn prune_locked(&self) -> io::Result<()> {
        let l = &self.limits;
        let t = (self.clock)();
        self.sweep_tmp(t);
        let mut docs: BTreeMap<String, Doc> = BTreeMap::new();
        // an index that cannot be read (torn last line, AV lock) still owns its objects: step 3 must not delete them
        let mut all_read = true;
        for e in fs::read_dir(self.root.join("docs"))?.filter_map(Result::ok) {
            if let Some(id) = e.file_name().to_string_lossy().strip_suffix(".jsonl") {
                match self.read_doc_id(id) {
                    Ok(d) => {
                        docs.insert(id.to_string(), d);
                    }
                    Err(_) => all_read = false,
                }
            }
        }
        // 1. per document: the last N versions, or those of the last D days, whichever keeps more; a big file keeps only a few
        let mut drops: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for (id, d) in &docs {
            let live = d.live();
            let Some(newest) = live.last() else { continue };
            let big = newest.size > l.big_file_bytes;
            let keep_n = if big { l.big_file_versions } else { l.versions } as usize;
            let cutoff = t.saturating_sub(u64::from(l.days) * 86_400);
            for (i, v) in live.iter().enumerate() {
                let by_count = live.len() - i <= keep_n;
                let by_age = !big && v.time > cutoff;
                if !(by_count || by_age || v.name.is_some()) {
                    drops.entry(id.clone()).or_default().insert(v.v);
                }
            }
        }
        // 2. all history together: the oldest unnamed version that is not a document's newest goes first
        let live_bytes =
            |docs: &BTreeMap<String, Doc>, drops: &BTreeMap<String, BTreeSet<u64>>| -> u64 {
                let mut seen: BTreeMap<&str, u64> = BTreeMap::new();
                for (id, d) in docs {
                    for v in d.live() {
                        if !drops.get(id).is_some_and(|s| s.contains(&v.v)) {
                            seen.insert(&v.sha256, v.size);
                        }
                    }
                }
                seen.values().sum()
            };
        while live_bytes(&docs, &drops) > l.max_bytes {
            let mut cand: Option<(u64, String, u64)> = None;
            for (id, d) in &docs {
                let live: Vec<&Version> = d
                    .live()
                    .into_iter()
                    .filter(|v| !drops.get(id).is_some_and(|s| s.contains(&v.v)))
                    .collect();
                for v in live
                    .iter()
                    .take(live.len().saturating_sub(1))
                    .filter(|v| v.name.is_none())
                {
                    if cand.as_ref().is_none_or(|c| v.time < c.0) {
                        cand = Some((v.time, id.clone(), v.v));
                    }
                }
            }
            let Some((_, id, v)) = cand else { break }; // only newest and named versions are left: they stay
            drops.entry(id).or_default().insert(v);
        }
        for (id, set) in drops.iter().filter(|(_, s)| !s.is_empty()) {
            let d = docs.get_mut(id).expect("listed above");
            self.append(
                id,
                d,
                "prune",
                &self.author,
                json!({"drop": set.iter().collect::<Vec<_>>()}),
            )?;
        }
        // 3. delete the bytes nothing refers to
        if !all_read {
            return Ok(());
        }
        let used: BTreeSet<String> = docs
            .values()
            .flat_map(|d| d.live().into_iter().map(|v| v.sha256.clone()))
            .collect();
        for e in fs::read_dir(self.root.join("objects"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.len() == 64 && !used.contains(&name) {
                let _ = fs::remove_file(e.path());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct T(PathBuf);
    impl T {
        fn new(tag: &str) -> T {
            let d =
                std::env::temp_dir().join(format!("suite-history-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(&d).unwrap();
            T(d)
        }
        fn store(&self, limits: Limits) -> Store {
            Store::new(self.0.join("store"), limits)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let p = self.0.join(name);
            fs::write(&p, bytes).unwrap();
            p
        }
    }
    impl Drop for T {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn roomy() -> Limits {
        Limits {
            versions: 50,
            days: 90,
            max_bytes: 1 << 30,
            big_file_bytes: 1 << 29,
            big_file_versions: 5,
        }
    }

    #[test]
    fn save_snapshot_list_restore_round_trip() {
        let t = T::new("rt");
        let s = t.store(roomy());
        let f = t.file("a.docx", b"one");
        let v1 = s.snapshot(&f, Some("first")).unwrap().unwrap();
        assert_eq!(
            (v1.v, v1.parent, v1.size, v1.author.as_str()),
            (1, None, 3, os_user().as_str())
        );
        assert_eq!(v1.sha256, sha(b"one"));
        assert!(
            s.snapshot(&f, None).unwrap().is_none(),
            "unchanged bytes make no version"
        );
        fs::write(&f, b"two").unwrap();
        s.snapshot(&f, None).unwrap().unwrap();
        fs::write(&f, b"three").unwrap();
        s.snapshot(&f, None).unwrap().unwrap();
        // restore v1: the file has it again and the history grew by one, nothing was rewritten
        let before = fs::read_to_string(s.doc_file(&doc_id(&f))).unwrap();
        let r = s.restore(&f, 1).unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"one");
        assert_eq!(
            (r.v, r.parent, r.restored_from, r.sha256.as_str()),
            (4, Some(3), Some(1), v1.sha256.as_str())
        );
        let after = fs::read_to_string(s.doc_file(&doc_id(&f))).unwrap();
        assert!(after.starts_with(&before), "append-only");
        let list = s.list(&f).unwrap();
        assert_eq!(list.iter().map(|v| v.v).collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert_eq!(s.read(&f, 3).unwrap(), b"three");
        assert_eq!(s.verify(&f).unwrap().lines, 4);
        assert_eq!(s.documents().len(), 1);
    }

    #[test]
    fn restore_snapshots_unsaved_work_first() {
        let t = T::new("pre");
        let s = t.store(roomy());
        let f = t.file("a.xlsx", b"v1");
        s.snapshot(&f, None).unwrap();
        fs::write(&f, b"edited, never snapshotted").unwrap();
        s.restore(&f, 1).unwrap();
        let l = s.list(&f).unwrap();
        assert_eq!(s.read(&f, l[1].v).unwrap(), b"edited, never snapshotted");
        assert_eq!(l[1].message.as_deref(), Some("Before restore"));
    }

    #[test]
    fn a_torn_index_keeps_its_objects_when_another_document_prunes() {
        let t = T::new("torn");
        let s = t.store(roomy());
        let (a, b) = (t.file("a.docx", b"a1"), t.file("b.docx", b"b1"));
        s.snapshot(&b, None).unwrap();
        // a power cut during append: half a line at the end of b's index
        let idx = s.doc_file(&doc_id(&b));
        let mut text = fs::read_to_string(&idx).unwrap();
        text.push_str("{\"kind\":\"vers");
        fs::write(&idx, text).unwrap();
        s.snapshot(&a, None).unwrap();
        assert!(
            s.object(&sha(b"b1")).is_file(),
            "b's bytes must survive a's prune"
        );
    }

    #[test]
    fn an_index_sha256_that_is_not_a_hash_is_refused() {
        // a well-chained line (its hash recomputed) whose sha256 would leave objects/
        let mut m = Map::new();
        for (k, v) in [
            ("kind", json!("version")),
            ("v", json!(1)),
            ("sha256", json!(r"..\..\secret")),
            ("prev", json!(GENESIS)),
        ] {
            m.insert(k.into(), v);
        }
        let h = line_hash(None, &m);
        m.insert("hash".into(), h.into());
        let err = parse_doc(&format!("{}\n", Value::Object(m)), None)
            .err()
            .unwrap();
        assert!(err.contains("no sha256"), "{err}");
    }

    #[test]
    fn a_tampered_line_is_detected() {
        let t = T::new("tamper");
        let s = t.store(roomy());
        let f = t.file("a.pptx", b"a");
        for b in [&b"b"[..], b"c", b"d"] {
            s.snapshot(&f, None).unwrap();
            fs::write(&f, b).unwrap();
        }
        s.snapshot(&f, None).unwrap();
        let file = s.doc_file(&doc_id(&f));
        let text = fs::read_to_string(&file).unwrap();
        // 1. a field edited
        fs::write(&file, text.replacen("\"size\":1", "\"size\":9", 1)).unwrap();
        assert!(s.verify(&f).unwrap_err().contains("line 1"));
        // 2. a line removed from the middle
        let cut: String = text
            .lines()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, l)| format!("{l}\n"))
            .collect();
        fs::write(&file, cut).unwrap();
        assert!(s.verify(&f).unwrap_err().contains("line 2"));
        // 3. stored bytes replaced
        fs::write(&file, &text).unwrap();
        fs::write(s.object(&sha(b"a")), b"x").unwrap();
        assert!(s.verify(&f).unwrap_err().contains("do not match"));
        assert!(s.read(&f, 1).is_err());
        // the listing's quick check sees a wrong size or a missing file, not a same-size swap
        assert!(s.verify_quick(&f).is_ok());
        fs::write(s.object(&sha(b"a")), b"xy").unwrap();
        assert!(s.verify_quick(&f).unwrap_err().contains("size"));
        fs::remove_file(s.object(&sha(b"a"))).unwrap();
        assert!(s.verify_quick(&f).unwrap_err().contains("missing"));
    }

    #[test]
    fn pruning_keeps_named_and_newest_versions() {
        let t = T::new("prune");
        // days = 0: age keeps nothing, so only the count rule (2) and the name apply
        let s = t.store(Limits {
            versions: 2,
            days: 0,
            max_bytes: 1 << 30,
            big_file_bytes: 1 << 29,
            big_file_versions: 5,
        });
        let f = t.file("a.docx", b"a");
        for i in 0..6u8 {
            fs::write(&f, [b'a' + i]).unwrap();
            s.snapshot(&f, None).unwrap();
            if i == 0 {
                s.name(&f, 1, "signed off").unwrap();
            }
        }
        let l = s.list(&f).unwrap();
        assert_eq!(
            l.iter().map(|v| v.v).collect::<Vec<_>>(),
            [1, 5, 6],
            "the name and the last two stay"
        );
        assert_eq!(l[0].name.as_deref(), Some("signed off"));
        assert!(
            !s.object(&sha(b"b")).exists(),
            "the bytes of a pruned version are deleted"
        );
        assert!(s.object(&sha(b"a")).exists());
        assert!(s.read(&f, 3).unwrap_err().to_string().contains("retention"));
        assert!(s.verify(&f).is_ok(), "the chain keeps the prune lines");
    }

    #[test]
    fn a_big_file_keeps_only_a_few_versions_whatever_their_age() {
        let t = T::new("big");
        let s = t.store(Limits {
            versions: 50,
            days: 90,
            max_bytes: 1 << 30,
            big_file_bytes: 4,
            big_file_versions: 2,
        });
        let f = t.file("big.xlsx", b"");
        for i in 0..5u8 {
            fs::write(&f, vec![i; 10]).unwrap();
            s.snapshot(&f, None).unwrap();
        }
        assert_eq!(
            s.list(&f).unwrap().iter().map(|v| v.v).collect::<Vec<_>>(),
            [4, 5]
        );
    }

    #[test]
    fn the_size_cap_drops_the_oldest_first_and_keeps_names_and_newest() {
        let t = T::new("cap");
        // stands in for "2 GB with 80 MB workbooks": cap 250 units, files of 80 units
        let s = t.store(Limits {
            versions: 50,
            days: 90,
            max_bytes: 250,
            big_file_bytes: 1 << 29,
            big_file_versions: 5,
        });
        let (a, b) = (t.file("a.xlsx", &[1; 80]), t.file("b.xlsx", &[9; 80]));
        s.snapshot(&a, None).unwrap();
        s.name(&a, 1, "keep me").unwrap();
        for i in 2..=4u8 {
            fs::write(&a, vec![i; 80]).unwrap();
            s.snapshot(&a, None).unwrap();
        }
        s.snapshot(&b, None).unwrap();
        let total: u64 = s.documents().iter().map(|d| d.size).sum();
        assert_eq!(total, 160, "newest of each document is listed");
        let live: u64 = [&a, &b]
            .iter()
            .flat_map(|p| s.list(p).unwrap())
            .map(|v| v.size)
            .sum();
        assert!(live <= 250, "{live}");
        let av: Vec<u64> = s.list(&a).unwrap().iter().map(|v| v.v).collect();
        assert!(
            av.contains(&1) && av.contains(&4),
            "named and newest stay: {av:?}"
        );
        assert_eq!(s.list(&b).unwrap().len(), 1);
        let on_disk: u64 = fs::read_dir(s.root.join("objects"))
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        assert_eq!(on_disk, live);
    }

    #[test]
    fn names_clear_and_unknown_versions_are_refused() {
        let t = T::new("name");
        let s = t.store(roomy());
        let f = t.file("a.docx", b"x");
        s.snapshot(&f, None).unwrap();
        s.name(&f, 1, "  final ").unwrap();
        assert_eq!(s.list(&f).unwrap()[0].name.as_deref(), Some("final"));
        s.name(&f, 1, "").unwrap();
        assert_eq!(s.list(&f).unwrap()[0].name, None);
        assert!(s.name(&f, 7, "x").is_err() && s.read(&f, 7).is_err());
    }

    fn at(ts: u64) -> fn() -> u64 {
        // fn pointers cannot capture: three fixed clocks are enough for the age tests
        match ts {
            0 => || 1_000_000,
            1 => || 1_000_000 + 100 * 86_400,
            _ => now,
        }
    }

    #[test]
    fn restore_is_a_new_version_server() {
        let t = T::new("idrt");
        let s = t.store(roomy()).with_key(b"server key");
        let id = "0123456789abcdef0123456789abcdef";
        let v1 = s
            .snapshot_bytes(id, b"one", "alice", Some("first"))
            .unwrap()
            .unwrap();
        assert_eq!(
            (v1.v, v1.author.as_str(), v1.path.as_str()),
            (1, "alice", "")
        );
        assert!(s.snapshot_bytes(id, b"one", "bob", None).unwrap().is_none());
        s.snapshot_bytes(id, b"two", "bob", None).unwrap().unwrap();
        let before = fs::read_to_string(s.doc_file(id)).unwrap();
        let r = s.restore_id(id, 1, "carol").unwrap();
        assert_eq!(
            (r.v, r.parent, r.restored_from, r.author.as_str()),
            (3, Some(2), Some(1), "carol")
        );
        assert_eq!(s.read_id(id, 3).unwrap(), b"one");
        assert!(
            fs::read_to_string(s.doc_file(id))
                .unwrap()
                .starts_with(&before),
            "append-only"
        );
        assert_eq!(s.list_id(id).unwrap().len(), 3);
        s.name_id(id, 2, " keep ", "dave").unwrap();
        assert_eq!(s.list_id(id).unwrap()[1].name.as_deref(), Some("keep"));
        assert_eq!(s.verify_id(id).unwrap().lines, 4);
        assert_eq!(
            s.read_id(id, 9).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(
            s.snapshot_bytes("../x", b"a", "a", None).is_err(),
            "an id is never a path"
        );
    }

    #[test]
    fn torn_index_is_repaired() {
        let t = T::new("repair");
        let s = t.store(roomy());
        let id = "aa11";
        s.snapshot_bytes(id, b"one", "a", None).unwrap();
        let idx = s.doc_file(id);
        let mut text = fs::read_to_string(&idx).unwrap();
        text.push_str("{\"kind\":\"vers");
        fs::write(&idx, text).unwrap();
        assert!(
            s.list_id(id).is_err() && s.verify_id(id).is_err(),
            "a reader sees the damage"
        );
        let v = s.snapshot_bytes(id, b"two", "a", None).unwrap().unwrap();
        assert_eq!(v.v, 2);
        assert_eq!(s.verify_id(id).unwrap().lines, 2);
        assert_eq!(s.read_id(id, 1).unwrap(), b"one");
    }

    #[test]
    fn keyed_chain_rewrite_fails() {
        let t = T::new("keyed");
        let id = "bb22";
        let s = t.store(roomy()).with_key(b"k1");
        s.snapshot_bytes(id, b"one", "a", None).unwrap();
        s.snapshot_bytes(id, b"two", "a", None).unwrap();
        assert!(s.verify_id(id).is_ok());
        // an attacker without the key re-chains the file with the plain SHA-256 rule after editing a line
        let text = fs::read_to_string(s.doc_file(id)).unwrap();
        let mut prev = GENESIS.to_string();
        let forged: String = text
            .lines()
            .map(|l| {
                let Value::Object(mut m) = serde_json::from_str::<Value>(l).unwrap() else {
                    panic!()
                };
                m.remove("hash");
                m.insert("prev".into(), prev.clone().into());
                m.insert("author".into(), "mallory".into());
                prev = line_hash(None, &m);
                m.insert("hash".into(), prev.clone().into());
                format!(
                    "{}
",
                    Value::Object(m)
                )
            })
            .collect();
        fs::write(s.doc_file(id), forged).unwrap();
        assert!(s.verify_id(id).is_err());
        // the same file read without any key verifies, so the key is what protects it; and another key fails too
        assert!(t.store(roomy()).verify_id(id).is_ok());
        assert!(t.store(roomy()).with_key(b"k2").verify_id(id).is_err());
        // an untouched keyed chain fails under the wrong key
        let id2 = "cc33";
        s.snapshot_bytes(id2, b"x", "a", None).unwrap();
        assert!(t.store(roomy()).verify_id(id2).is_err());
        assert!(t.store(roomy()).with_key(b"k1").verify_id(id2).is_ok());
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn retention_by_count_and_by_age_server() {
        let t = T::new("ret");
        let lim = |versions, days| Limits {
            versions,
            days,
            ..roomy()
        };
        // count: days = 0 keeps only the last 3 (and a named one)
        let s = t.store(lim(3, 0));
        for i in 0..6u8 {
            s.snapshot_bytes("d1", &[i], "a", None).unwrap();
            if i == 0 {
                s.name_id("d1", 1, "named", "a").unwrap();
            }
        }
        assert_eq!(
            s.list_id("d1")
                .unwrap()
                .iter()
                .map(|v| v.v)
                .collect::<Vec<_>>(),
            [1, 4, 5, 6]
        );
        // age: 50 versions allowed, but the clock moves 100 days on, so old ones go once a new one is saved
        let s = t.store(lim(1, 90)).with_clock(at(0));
        for i in 0..3u8 {
            s.snapshot_bytes("d2", &[i], "a", None).unwrap();
        }
        assert_eq!(
            s.list_id("d2").unwrap().len(),
            3,
            "within 90 days and over the count: kept by age"
        );
        let s = t.store(lim(1, 90)).with_clock(at(1));
        s.snapshot_bytes("d2", &[9], "a", None).unwrap();
        assert_eq!(
            s.list_id("d2")
                .unwrap()
                .iter()
                .map(|v| v.v)
                .collect::<Vec<_>>(),
            [4],
            "only the newest is left"
        );
        assert!(s.verify_id("d2").is_ok());
    }

    #[test]
    fn forget_id_removes_the_document_and_only_its_own_bytes() {
        let t = T::new("forget");
        let s = t.store(roomy());
        s.snapshot_bytes("d1", b"shared", "a", None).unwrap();
        s.snapshot_bytes("d1", b"only-d1", "a", None).unwrap();
        s.snapshot_bytes("d2", b"shared", "a", None).unwrap();
        assert!(s.forget_id("d1").unwrap());
        assert!(s.list_id("d1").unwrap().is_empty());
        assert!(!s.object(&sha(b"only-d1")).exists(), "its own bytes go");
        assert!(s.object(&sha(b"shared")).exists(), "bytes d2 uses stay");
        assert!(s.verify_id("d2").is_ok());
        assert!(!s.forget_id("d1").unwrap(), "a second forget finds nothing");
        assert!(s.forget_id("../x").is_err());
    }

    #[test]
    fn stale_tmp_files_are_swept() {
        let t = T::new("tmp");
        let s = t.store(roomy());
        s.snapshot_bytes("d1", b"a", "a", None).unwrap();
        let (old, fresh) = (
            s.root.join("objects/.x.1.0.tmp"),
            s.root.join("objects/.y.1.1.tmp"),
        );
        fs::write(&old, b"half").unwrap();
        fs::write(&fresh, b"half").unwrap();
        let f = fs::OpenOptions::new().write(true).open(&old).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(2 * TMP_MAX_AGE_S))
            .unwrap();
        drop(f);
        s.snapshot_bytes("d1", b"b", "a", None).unwrap();
        assert!(!old.exists() && fresh.exists(), "only the stale one goes");
    }
}
