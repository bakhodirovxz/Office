//! The server version store (PLAN phase 5, section 6, WP11): `VersionBackend` is what the routes talk to. Ours is `LocalBackend`
//! (suite-core's history store under `<data_dir>/history/`, keyed chain); a provider's own store (D6) is `ProviderBackend`, a stub that
//! answers "not available" until a provider offers one. A document's history is shared by every session of the same provider and
//! `document.key`, and the routes below only ever reach it through a valid token for one session (the same 404 rule as WP3).
//!
//! Routes (session token, `Authorization: Bearer`):
//!   `GET  /ds/session/{sid}/history`                 `{"versions":[...]}`, oldest first
//!   `GET  /ds/session/{sid}/history/{v}`             the bytes of version v (needs the `download` permission)
//!   `GET  /ds/session/{sid}/history/{a}/compare/{b}` the JSON of `docs-diff`/`sheets-diff` (schema 1) for a and b (needs `download`)
//!   `POST /ds/session/{sid}/history/{v}/restore`     v becomes a new version, and a new version of the session (needs `edit`)
//!   `POST /ds/session/{sid}/history/{v}/name`        body `{"name": "..."}`, an empty name clears it (needs `edit`)
use crate::Docserver;
use crate::api::{Cl, authorize, blk, client, json_ok};
use crate::sessions::{SessErr, SessionRecord, sha256_hex};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use convert_api::jwt::now;
use convert_api::routes::ApiError;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::io::ErrorKind;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use suite_core::history::{Limits, Store, Version};

#[derive(Debug)]
pub enum HistErr {
    /// no such document or version, or it was removed by the retention rules
    NotFound,
    /// the backend is not configured (the provider stub)
    Unavailable,
    Io(String),
}

impl From<std::io::Error> for HistErr {
    fn from(e: std::io::Error) -> HistErr {
        if e.kind() == ErrorKind::NotFound {
            HistErr::NotFound
        } else {
            HistErr::Io(e.to_string())
        }
    }
}

/// The store of record for the versions of a document, by document id.
pub trait VersionBackend: Send + Sync {
    /// Record `bytes` as a version by `author`; `None` when nothing changed. A backend that makes its own versions does nothing.
    fn snapshot(
        &self,
        _doc: &str,
        _bytes: &[u8],
        _author: &str,
    ) -> Result<Option<Version>, HistErr> {
        Ok(None)
    }
    fn list(&self, doc: &str) -> Result<Vec<Version>, HistErr>;
    fn read(&self, doc: &str, v: u64) -> Result<Vec<u8>, HistErr>;
    /// Version `v` becomes a new newest version; the caller moves the bytes to the live document.
    fn restore(&self, doc: &str, v: u64, author: &str) -> Result<Version, HistErr>;
    fn name(&self, doc: &str, v: u64, name: &str, author: &str) -> Result<(), HistErr>;
    /// Delete the document's history whole (the retention sweep); `false` when there was none. A backend we do not own keeps its own.
    fn forget(&self, _doc: &str) -> Result<bool, HistErr> {
        Ok(false)
    }
}

pub struct LocalBackend(Store);

impl LocalBackend {
    /// Last 50 versions or 90 days, whichever keeps more (Phase 4 PLAN 5.5, H8); 20 GiB in all; a file over 50 MB keeps 5.
    pub fn new(root: PathBuf, chain_key: &[u8]) -> LocalBackend {
        LocalBackend(
            Store::new(
                root,
                Limits {
                    versions: 50,
                    days: 90,
                    max_bytes: 20 << 30,
                    big_file_bytes: 50 << 20,
                    big_file_versions: 5,
                },
            )
            .with_key(chain_key),
        )
    }
}

impl VersionBackend for LocalBackend {
    fn snapshot(&self, doc: &str, bytes: &[u8], author: &str) -> Result<Option<Version>, HistErr> {
        Ok(self.0.snapshot_bytes(doc, bytes, author, None)?)
    }
    fn list(&self, doc: &str) -> Result<Vec<Version>, HistErr> {
        Ok(self.0.list_id(doc)?)
    }
    fn read(&self, doc: &str, v: u64) -> Result<Vec<u8>, HistErr> {
        Ok(self.0.read_id(doc, v)?)
    }
    fn restore(&self, doc: &str, v: u64, author: &str) -> Result<Version, HistErr> {
        Ok(self.0.restore_id(doc, v, author)?)
    }
    fn name(&self, doc: &str, v: u64, name: &str, author: &str) -> Result<(), HistErr> {
        Ok(self.0.name_id(doc, v, name, author)?)
    }
    fn forget(&self, doc: &str) -> Result<bool, HistErr> {
        Ok(self.0.forget_id(doc)?)
    }
}

/// A provider-side versions API (docs/phase5/PLAN.md §6): not built; every call answers "not available".
pub struct ProviderBackend;

impl VersionBackend for ProviderBackend {
    fn list(&self, _: &str) -> Result<Vec<Version>, HistErr> {
        Err(HistErr::Unavailable)
    }
    fn read(&self, _: &str, _: u64) -> Result<Vec<u8>, HistErr> {
        Err(HistErr::Unavailable)
    }
    fn restore(&self, _: &str, _: u64, _: &str) -> Result<Version, HistErr> {
        Err(HistErr::Unavailable)
    }
    fn name(&self, _: &str, _: u64, _: &str, _: &str) -> Result<(), HistErr> {
        Err(HistErr::Unavailable)
    }
}

impl From<HistErr> for ApiError {
    fn from(e: HistErr) -> ApiError {
        match e {
            HistErr::NotFound => {
                ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such version")
            }
            HistErr::Unavailable => ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "not_available",
                "version history is not available for this document",
            ),
            HistErr::Io(m) => ApiError::internal(m),
        }
    }
}

impl Docserver {
    /// The backend of record. Only ours exists today; a provider's is chosen here once configured.
    pub fn version_backend(&self) -> Box<dyn VersionBackend> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.server_secret).expect("any key length");
        mac.update(b"docserver history chain v1");
        let mut b = LocalBackend::new(self.data_dir.join("history"), &mac.finalize().into_bytes());
        if let Some(k) = &self.at_rest {
            b.0 = b.0.with_sealer(k.clone());
        }
        Box::new(b)
    }
}

/// The document id: one history per provider and `document.key`. `shortcut: a provider that rotates the key per editing session
/// gets a history per key, upgrade when the provider gives a stable document id.`
pub fn doc_id(rec: &SessionRecord) -> String {
    sha256_hex(format!("{}\0{}", rec.provider, rec.key).as_bytes())[..32].to_string()
}

/// Called by `put_version` for every stored version; a failure never fails the save (the session copy is the safety net).
pub(crate) fn record(ds: &Docserver, rec: &SessionRecord, bytes: &[u8], by: &str) {
    if let Err(e) = ds.version_backend().snapshot(&doc_id(rec), bytes, by) {
        eprintln!(
            "docserver: history: snapshot of session {} failed: {e:?}",
            crate::audit::sid_ref(&rec.sid)
        );
    }
}

/// Removes the plaintext compare copies a crash left behind: the `Scratch` guard runs on drop only, so a killed process leaves them in
/// the session folder (also with `encrypt_at_rest`). Called once at start, before any request.
pub(crate) fn sweep_compare_leftovers(data_dir: &FsPath) {
    let Ok(sessions) = std::fs::read_dir(data_dir.join("sessions")) else {
        return;
    };
    for e in sessions
        .flatten()
        .filter_map(|s| std::fs::read_dir(s.path()).ok())
        .flatten()
        .flatten()
    {
        if e.file_name().to_string_lossy().starts_with("compare-") {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// A permission the session lacks: 403, and one `perm.refused` audit line (the session by reference, never its token).
fn need(
    ds: &Docserver,
    rec: &SessionRecord,
    route: &str,
    perm: &str,
    ok: bool,
) -> Result<(), ApiError> {
    if !ok {
        ds.audit(
            "perm.refused",
            json!({"route": route, "perm": perm, "session": crate::audit::sid_ref(&rec.sid), "user": rec.user_id}),
        );
    }
    ok.then_some(()).ok_or_else(|| SessErr::Forbidden.into())
}

type Res = Result<Response, ApiError>;
type Ds = State<Arc<Docserver>>;

async fn list(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, true, "history.list", &client(cl))?;
    let vs = blk(move || ds.version_backend().list(&doc_id(&rec))).await??;
    json_ok(json!({"versions": vs.iter().map(Version::to_json).collect::<Vec<_>>()}))
}

async fn view(State(ds): Ds, cl: Cl, Path((sid, v)): Path<(String, u64)>, h: HeaderMap) -> Res {
    let cl = client(cl);
    let rec = authorize(&ds, &h, &sid, true, "history.view", &cl)?;
    need(&ds, &rec, "history.view", "download", rec.perms.download)?;
    let b = blk(move || {
        let b = ds.version_backend().read(&doc_id(&rec), v)?;
        ds.issue_copy(&rec, b, &cl.ip, "download")
            .map_err(ApiError::from)
    })
    .await??;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], b).into_response())
}

async fn restore(State(ds): Ds, cl: Cl, Path((sid, v)): Path<(String, u64)>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "history.restore", &client(cl))?;
    need(&ds, &rec, "history.restore", "edit", rec.perms.edit)?;
    let out = blk(move || {
        let b = ds.version_backend();
        let id = doc_id(&rec);
        let bytes = b.read(&id, v)?;
        let made = b.restore(&id, v, &rec.user_id)?;
        // the open session gets the restored bytes as its newest version (the snapshot hook sees identical bytes and adds nothing)
        let sv = ds
            .put_version(&rec.sid, &bytes, &rec.user_id, true, now())
            .map_err(|e| match e {
                SessErr::NotFound => HistErr::NotFound,
                e => HistErr::Io(format!("{e:?}")),
            })?;
        Ok::<_, HistErr>((made, sv.n))
    })
    .await??;
    json_ok(json!({"version": out.0.to_json(), "session_version": out.1}))
}

async fn name(
    State(ds): Ds,
    cl: Cl,
    Path((sid, v)): Path<(String, u64)>,
    h: HeaderMap,
    body: Bytes,
) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "history.name", &client(cl))?;
    need(&ds, &rec, "history.name", "edit", rec.perms.edit)?;
    let name = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|j| j.get("name")?.as_str().map(str::to_string))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "bad_request",
                "body: {\"name\": \"...\"}",
            )
        })?;
    blk(move || {
        ds.version_backend()
            .name(&doc_id(&rec), v, &name, &rec.user_id)
    })
    .await??;
    json_ok(json!({"ok": true}))
}

/// The tool's JSON is read up to this many bytes; a longer answer is refused (a diff of two huge workbooks must not fill the memory).
const DIFF_MAX_OUT: u64 = 16 << 20;

fn compare_err(status: StatusCode, code: &'static str, m: impl Into<String>) -> ApiError {
    ApiError::new(status, code, m)
}

/// A scratch folder inside the session folder, removed on drop (also on an error or a panic).
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `exe A B --json`: exit 0 or 1 is a result, anything else (or a timeout, or too much output) is an error. The child is killed on
/// the deadline.
fn run_diff(exe: &FsPath, a: &FsPath, b: &FsPath, secs: u64) -> Result<Value, ApiError> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.arg(a)
        .arg(b)
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| {
        eprintln!("docserver: cannot start the compare tool: {e}");
        compare_err(
            StatusCode::BAD_GATEWAY,
            "compare_failed",
            "the compare tool could not be started",
        )
    })?;
    // read both pipes to the end (a full pipe would stall the tool) but keep only the first `cap` bytes
    fn drain<R: Read + Send + 'static>(r: Option<R>, cap: u64) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut keep = vec![];
            if let Some(mut r) = r {
                let _ = r.by_ref().take(cap + 1).read_to_end(&mut keep);
                let _ = std::io::copy(&mut r, &mut std::io::sink());
            }
            keep
        })
    }
    let out = drain(child.stdout.take(), DIFF_MAX_OUT);
    let err = drain(child.stderr.take(), 2048);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if std::time::Instant::now() < end => {
                std::thread::sleep(std::time::Duration::from_millis(25))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(compare_err(
                    StatusCode::GATEWAY_TIMEOUT,
                    "compare_timeout",
                    "the compare tool took too long",
                ));
            }
            Err(e) => return Err(ApiError::internal(e.to_string())),
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    if !matches!(status.code(), Some(0 | 1)) {
        // the tool's stderr may name server paths: it goes to the log (session id hidden), not to the browser (F8)
        let mut text = String::from_utf8_lossy(&err).trim().to_string();
        if let Some(dir) = a.parent().and_then(|p| p.parent()) {
            text = text.replace(&dir.display().to_string(), "<session>");
        }
        eprintln!("docserver: compare tool failed: {text}");
        return Err(compare_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "compare_failed",
            "the compare tool could not compare these versions",
        ));
    }
    if out.len() as u64 > DIFF_MAX_OUT {
        return Err(compare_err(
            StatusCode::BAD_GATEWAY,
            "compare_too_large",
            "the result of the comparison is too large",
        ));
    }
    serde_json::from_slice(&out).map_err(|_| {
        compare_err(
            StatusCode::BAD_GATEWAY,
            "compare_failed",
            "the compare tool gave no result",
        )
    })
}

async fn compare(
    State(ds): Ds,
    cl: Cl,
    Path((sid, a, b)): Path<(String, u64, u64)>,
    h: HeaderMap,
) -> Res {
    let rec = authorize(&ds, &h, &sid, true, "history.compare", &client(cl))?;
    // a comparison shows the text of both versions, so it is a download as far as the permission goes
    need(&ds, &rec, "history.compare", "download", rec.perms.download)?;
    let app = match rec.editor.as_str() {
        app @ ("docs" | "sheets") => app.to_string(),
        _ => {
            return Err(compare_err(
                StatusCode::NOT_IMPLEMENTED,
                "not_supported",
                "comparing versions is not supported for this kind of file",
            ));
        }
    };
    let exe = ds
        .diff_bin_dir
        .as_ref()
        .map(|d| d.join(format!("{app}-diff{}", std::env::consts::EXE_SUFFIX)))
        .filter(|p| p.is_file())
        .ok_or_else(|| {
            compare_err(
                StatusCode::NOT_IMPLEMENTED,
                "not_configured",
                "the compare tool is not installed on this server",
            )
        })?;
    let out = blk(move || {
        // `read` opens the sealed objects: the plaintext copies live in the session folder only, and go when this scope ends
        let backend = ds.version_backend();
        let id = doc_id(&rec);
        let (ba, bb) = (backend.read(&id, a)?, backend.read(&id, b)?);
        let base = ds.session_dir(&rec.sid).ok_or(HistErr::NotFound)?;
        let ext: String = rec
            .file_type
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let mut rnd = [0u8; 8];
        getrandom::getrandom(&mut rnd).map_err(|e| HistErr::Io(e.to_string()))?;
        let dir = Scratch(base.join(format!("compare-{}", crate::sessions::hex(&rnd))));
        std::fs::create_dir_all(&dir.0)?;
        let (fa, fb) = (
            dir.0.join(format!("a.{ext}")),
            dir.0.join(format!("b.{ext}")),
        );
        std::fs::write(&fa, ba)?;
        std::fs::write(&fb, bb)?;
        let diff = run_diff(&exe, &fa, &fb, ds.diff_timeout_s);
        Ok::<_, HistErr>(diff.map(|d| json!({"ok": true, "a": a, "b": b, "app": app, "diff": d})))
    })
    .await??;
    json_ok(out?)
}

impl Docserver {
    pub(crate) fn history_routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        Router::new()
            .route("/ds/session/{sid}/history", get(list))
            .route("/ds/session/{sid}/history/{v}", get(view))
            .route("/ds/session/{sid}/history/{a}/compare/{b}", get(compare))
            .route("/ds/session/{sid}/history/{v}/restore", post(restore))
            .route(
                "/ds/session/{sid}/history/{v}/name",
                post(name).layer(DefaultBodyLimit::max(4 << 10)),
            )
            .with_state(self.clone())
    }
}
