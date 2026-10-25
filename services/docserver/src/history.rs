//! The server version store (PLAN phase 5, section 6, WP11): `VersionBackend` is what the routes talk to. Ours is `LocalBackend`
//! (suite-core's history store under `<data_dir>/history/`, keyed chain); a provider's own store (D6) is `ProviderBackend`, a stub that
//! answers "not available" until a provider offers one. A document's history is shared by every session of the same provider and
//! `document.key`, and the routes below only ever reach it through a valid token for one session (the same 404 rule as WP3).
//!
//! Routes (session token, `Authorization: Bearer`):
//!   `GET  /ds/session/{sid}/history`                 `{"versions":[...]}`, oldest first
//!   `GET  /ds/session/{sid}/history/{v}`             the bytes of version v (needs the `download` permission)
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
use std::path::PathBuf;
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
        Box::new(LocalBackend::new(
            self.data_dir.join("history"),
            &mac.finalize().into_bytes(),
        ))
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
            rec.sid
        );
    }
}

fn need(ok: bool) -> Result<(), ApiError> {
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
    let rec = authorize(&ds, &h, &sid, true, "history.view", &client(cl))?;
    need(rec.perms.download)?;
    let b = blk(move || ds.version_backend().read(&doc_id(&rec), v)).await??;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], b).into_response())
}

async fn restore(State(ds): Ds, cl: Cl, Path((sid, v)): Path<(String, u64)>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "history.restore", &client(cl))?;
    need(rec.perms.edit)?;
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
    need(rec.perms.edit)?;
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

impl Docserver {
    pub(crate) fn history_routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        Router::new()
            .route("/ds/session/{sid}/history", get(list))
            .route("/ds/session/{sid}/history/{v}", get(view))
            .route("/ds/session/{sid}/history/{v}/restore", post(restore))
            .route(
                "/ds/session/{sid}/history/{v}/name",
                post(name).layer(DefaultBodyLimit::max(4 << 10)),
            )
            .with_state(self.clone())
    }
}
