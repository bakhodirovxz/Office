//! The session handlers of `/ds/session*` and the file route (PLAN phase 5, 3.1). Every one of them checks a token first: the
//! provider's config token on create, our session token on the others, our file token on the file route. A session token for
//! another session, and a session that does not exist, get the same 404 (no existence oracle, S24).
use crate::Docserver;
use crate::sessions::{SessErr, SessionRecord, valid_sid};
use crate::tokens::ct_eq;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use convert_api::jwt::now;
use convert_api::routes::ApiError;
use serde_json::{Value, json};
use std::sync::Arc;

type Ds = State<Arc<Docserver>>;
type Res = Result<Response, ApiError>;

impl From<SessErr> for ApiError {
    fn from(e: SessErr) -> ApiError {
        match e {
            SessErr::Unauthorized => unauthorized(),
            SessErr::Bad(m) => ApiError::new(StatusCode::BAD_REQUEST, "bad_config", m),
            SessErr::NotFound => not_found(),
            SessErr::Forbidden => ApiError::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                "this session may not do that",
            ),
            SessErr::TooLarge => ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "the document is too large",
            ),
            SessErr::BadDocument(m) => {
                ApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, "bad_document", m)
            }
            SessErr::Upstream(m) => ApiError::new(StatusCode::BAD_GATEWAY, "fetch_failed", m),
            SessErr::Io(m) => ApiError::internal(m),
        }
    }
}

fn unauthorized() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "a valid token is required",
    )
}

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such session")
}

/// Runs blocking disk work off the async workers.
async fn blk<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
}

fn json_ok(v: Value) -> Res {
    Ok((StatusCode::OK, axum::Json(v)).into_response())
}

/// The session a request may use: a valid session token for `sid`, and the session on disk agreeing with it.
fn authorize(
    ds: &Docserver,
    h: &HeaderMap,
    sid: &str,
    allow_closed: bool,
) -> Result<SessionRecord, ApiError> {
    let t = now();
    let tok = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| t.len() <= 8192)
        .ok_or_else(unauthorized)?;
    let c = ds.session_claims(tok, t).ok_or_else(unauthorized)?;
    if c.sid != sid || !valid_sid(sid) {
        return Err(not_found());
    }
    let rec = ds.load_session(sid).ok_or_else(not_found)?;
    if rec.provider != c.prov || rec.user_id != c.sub || (rec.closed.is_some() && !allow_closed) {
        return Err(not_found());
    }
    if t > rec.expires {
        return Err(unauthorized());
    }
    Ok(rec)
}

/// `POST /ds/session`, body `{"origin": "<event.origin of the config message>", "config": {..., "token": "<provider JWT>"}}`: only
/// the signed payload of the token is used, and `origin` must be a frame origin of the provider that signed it (403 otherwise).
async fn create(State(ds): Ds, body: Bytes) -> Res {
    let v: Value = serde_json::from_slice(&body).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the body must be JSON",
        )
    })?;
    let tok = v
        .pointer("/config/token")
        .and_then(Value::as_str)
        .filter(|t| t.len() <= 16384)
        .ok_or_else(unauthorized)?
        .to_string();
    let origin = v
        .get("origin")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let t = now();
    let (rec, token) = blk(move || {
        let rec = ds.create_session(&tok, &origin, t)?;
        let token = ds.mint_session_token(&rec, t);
        Ok::<_, SessErr>((rec, token))
    })
    .await??;
    json_ok(
        json!({"sid": rec.sid, "token": token, "editor": rec.editor, "mode": rec.mode,
                   "perms": rec.perms, "title": rec.title, "expires": rec.expires}),
    )
}

async fn renew(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false)?;
    json_ok(json!({"token": ds.mint_session_token(&rec, now()), "expires": rec.expires}))
}

async fn document(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let mut rec = authorize(&ds, &h, &sid, false)?;
    if rec.versions.is_empty() {
        // the first call of a new session: fetch the provider's document (WP4)
        let (d, s) = (ds.clone(), sid.clone());
        blk(move || d.ensure_document(&s, now())).await??;
        rec = ds.load_session(&sid).ok_or_else(not_found)?;
    }
    let n = rec.versions.last().map(|v| v.n).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "no_document",
            "the document has not been loaded yet",
        )
    })?;
    let b = blk(move || ds.read_version(&sid, n))
        .await?
        .ok_or_else(not_found)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], b).into_response())
}

/// The body of a save: a document of the session's type within the size cap, a sound package of that type (S9).
fn check_body(ds: &Docserver, rec: &SessionRecord, b: &[u8]) -> Result<(), ApiError> {
    crate::ooxml::check(b, &rec.file_type, ds.max_document_mb << 20)
        .map_err(|e| Docserver::doc_err(e).into())
}

async fn save(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap, body: Bytes) -> Res {
    let rec = authorize(&ds, &h, &sid, false)?;
    if !rec.perms.edit {
        return Err(SessErr::Forbidden.into());
    }
    check_body(&ds, &rec, &body)?;
    // the answer comes after the provider's reply (the async-save ticket, 3.3): ok, failed or unknown; the version is kept either way
    let (v, cb, dirty) = blk(move || {
        let v = ds.put_version(&sid, &body, &rec.user_id, true, now())?;
        let rec = ds.load_session(&sid).ok_or(SessErr::NotFound)?;
        let cb = ds.callback_saved(&rec, &v);
        let dirty = ds.load_session(&sid).is_some_and(|r| r.dirty());
        Ok::<_, SessErr>((v, cb, dirty))
    })
    .await??;
    json_ok(json!({"version": v.n, "sha256": v.sha256, "callback": cb.as_str(), "dirty": dirty}))
}

/// `POST /ds/session/{sid}/dirty`: the editor's first change; the status 1 callback, once.
async fn dirty(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false)?;
    if !rec.perms.edit {
        return Err(SessErr::Forbidden.into());
    }
    let cb = blk(move || ds.callback_dirty(&sid)).await?;
    json_ok(json!({"callback": cb.map(|c| c.as_str())}))
}

async fn autosave(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap, body: Bytes) -> Res {
    let rec = authorize(&ds, &h, &sid, false)?;
    if !rec.perms.edit {
        return Err(SessErr::Forbidden.into());
    }
    check_body(&ds, &rec, &body)?;
    blk(move || ds.put_autosave(&sid, &body, now())).await??;
    json_ok(json!({"ok": true}))
}

async fn close(State(ds): Ds, Path(sid): Path<String>, h: HeaderMap) -> Res {
    authorize(&ds, &h, &sid, true)?;
    let (c, cb, dirty) = blk(move || {
        let c = ds.close_session(&sid, now())?;
        // status 2 is queued and retried until the provider acknowledges it; 4 is one try
        let cb = ds.callback_closed(&sid);
        let dirty = ds.load_session(&sid).is_some_and(|r| r.dirty());
        Ok::<_, SessErr>((c, cb, dirty))
    })
    .await??;
    json_ok(json!({"status": c.status, "callback": cb.map(|c| c.as_str()), "dirty": dirty}))
}

/// `GET /ds/files/{sid}/{ver}?t=<file token>`: the bytes of one version, for the token bound to exactly that sid, ver and digest.
async fn file(State(ds): Ds, Path((sid, ver)): Path<(String, u64)>, req: Request) -> Res {
    let t = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("t=")))
        .filter(|t| t.len() <= 8192)
        .ok_or_else(unauthorized)?
        .to_string();
    let c = ds.file_claims(&t, now()).ok_or_else(unauthorized)?;
    let b = blk(move || {
        let rec = ds.load_session(&sid)?;
        let v = rec.versions.iter().find(|v| v.n == ver)?;
        if c.sid != sid || c.ver != ver || !ct_eq(c.sha256.as_bytes(), v.sha256.as_bytes()) {
            return None;
        }
        ds.read_version(&sid, ver)
    })
    .await?
    .ok_or_else(not_found)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], b).into_response())
}

impl Docserver {
    /// The session routes: they take their state from `self`, so they merge into any router.
    pub(crate) fn session_routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        let cap = DefaultBodyLimit::max((self.max_document_mb << 20) as usize + 1);
        Router::new()
            .route(
                "/ds/session",
                post(create).layer(DefaultBodyLimit::max(128 << 10)),
            )
            .route("/ds/session/{sid}/renew", post(renew))
            .route("/ds/session/{sid}/document", get(document))
            .route("/ds/session/{sid}/save", post(save).layer(cap))
            .route("/ds/session/{sid}/autosave", post(autosave).layer(cap))
            .route("/ds/session/{sid}/dirty", post(dirty))
            .route("/ds/session/{sid}/close", post(close))
            .with_state(self.clone())
    }

    pub(crate) fn file_route(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/ds/files/{sid}/{ver}", get(file))
            .with_state(self.clone())
    }
}
