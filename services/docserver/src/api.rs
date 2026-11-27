//! The session handlers of `/ds/session*` and the file route (PLAN phase 5, 3.1). Every one of them checks a token first: the
//! provider's config token on create, our session token on the others, our file token on the file route. A session token for
//! another session, and a session that does not exist, get the same 404 (no existence oracle, S24).
use crate::Docserver;
use crate::audit::sid_ref;
use crate::sessions::{SessErr, SessionRecord, sha256_hex, valid_sid};
use crate::tokens::ct_eq;
use axum::Extension;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use convert_api::jwt::now;
use convert_api::routes::{ApiError, Client};
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
            SessErr::NoRoom(code, m) => ApiError::new(StatusCode::INSUFFICIENT_STORAGE, code, m),
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

/// The request's client address, as the router's guard layer set it (behind a trusted proxy, the forwarded one). The internal listener has
/// no such layer: its callers count as one client.
pub(crate) type Cl = Option<Extension<Client>>;

pub(crate) fn client(c: Cl) -> Client {
    c.map_or_else(
        || Client {
            ip: "internal".into(),
            key: "internal".into(),
        },
        |c| c.0,
    )
}

/// A refused credential: audited and counted against the client (S15, S16). Over the limit the answer is 429 instead of `base`.
fn denied(ds: &Docserver, route: &str, c: &Client, why: &str, base: ApiError) -> ApiError {
    match ds.auth_failed(route, &c.ip, &c.key, why) {
        Ok(()) => base,
        Err(secs) => ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many failed attempts",
        )
        .retry(secs),
    }
}

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such session")
}

/// Runs blocking disk work off the async workers.
pub(crate) async fn blk<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
}

pub(crate) fn json_ok(v: Value) -> Res {
    Ok((StatusCode::OK, axum::Json(v)).into_response())
}

/// The session a request may use: a valid session token for `sid`, and the session on disk agreeing with it.
pub(crate) fn authorize(
    ds: &Docserver,
    h: &HeaderMap,
    sid: &str,
    allow_closed: bool,
    route: &str,
    cl: &Client,
) -> Result<SessionRecord, ApiError> {
    let t = now();
    let no = |why: &str| denied(ds, route, cl, why, unauthorized());
    let tok = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| t.len() <= 8192)
        .ok_or_else(|| no("no bearer token"))?;
    let c = ds
        .session_claims(tok, t)
        .ok_or_else(|| no("bad session token"))?;
    if c.sid != sid || !valid_sid(sid) {
        return Err(not_found());
    }
    let rec = ds.load_session(sid).ok_or_else(not_found)?;
    if rec.provider != c.prov || rec.user_id != c.sub || (rec.closed.is_some() && !allow_closed) {
        return Err(not_found());
    }
    if t > rec.expires {
        return Err(no("session expired"));
    }
    Ok(rec)
}

/// `POST /ds/session`, body `{"origin": "<event.origin of the config message>", "config": {..., "token": "<provider JWT>"}}`: only
/// the signed payload of the token is used, and `origin` must be a frame origin of the provider that signed it (403 otherwise).
async fn create(State(ds): Ds, cl: Cl, body: Bytes) -> Res {
    let cl = client(cl);
    if let Err(secs) = ds.create_allowed(&cl.ip, &cl.key) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many sessions opened from this address",
        )
        .retry(secs));
    }
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
        .ok_or_else(|| denied(&ds, "create", &cl, "no config token", unauthorized()))?
        .to_string();
    let origin = v
        .get("origin")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let t = now();
    let d = ds.clone();
    let (rec, token) = blk(move || {
        let rec = d.create_session(&tok, &origin, t)?;
        let token = d.mint_session_token(&rec, t);
        Ok::<_, SessErr>((rec, token))
    })
    .await?
    .map_err(|e| match e {
        SessErr::Unauthorized => denied(&ds, "create", &cl, "config token refused", e.into()),
        SessErr::Forbidden => denied(&ds, "create", &cl, "origin is not a frame origin", e.into()),
        e => e.into(),
    })?;
    ds.audit(
        "session.create",
        json!({"session": sid_ref(&rec.sid), "provider": rec.provider, "user": rec.user_id,
               "key_ref": &sha256_hex(rec.key.as_bytes())[..16], "mode": rec.mode, "perms": rec.perms,
               "client": cl.ip, "file": rec.title}),
    );
    // the provider's `ui_lang`, when set, is the editors' language whatever `editorConfig.lang` says
    let lang = ds.provider(&rec.provider).and_then(|p| p.ui_lang.clone());
    // `history`: whether the wrapper offers the History panel (the routes refuse it otherwise, history.rs `history_allowed`)
    let history = crate::history::history_allowed(&ds, &rec);
    json_ok(
        json!({"sid": rec.sid, "token": token, "editor": rec.editor, "mode": rec.mode,
                   "perms": rec.perms, "title": rec.title, "expires": rec.expires, "lang": lang, "history": history}),
    )
}

async fn renew(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "renew", &client(cl))?;
    json_ok(json!({"token": ds.mint_session_token(&rec, now()), "expires": rec.expires}))
}

async fn document(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let mut rec = authorize(&ds, &h, &sid, false, "document", &client(cl))?;
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

/// A change (save, autosave, dirty) needs the edit permission, and the provider's document fetched first: otherwise a replayed config
/// token (one without `exp`, docs/phase5/dropin.md D1) could open a session and hand any bytes to the provider as the document.
pub(crate) fn editable(rec: &SessionRecord) -> Result<(), ApiError> {
    if !rec.perms.edit {
        return Err(SessErr::Forbidden.into());
    }
    if rec.versions.is_empty() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "no_document",
            "the document has not been loaded yet",
        ));
    }
    Ok(())
}

/// The body of a save: a document of the session's type within the size cap, a sound package of that type (S9).
fn check_body(ds: &Docserver, rec: &SessionRecord, b: &[u8]) -> Result<(), ApiError> {
    crate::ooxml::check(b, &rec.file_type, ds.max_document_mb << 20)
        .map_err(|e| Docserver::doc_err(e).into())
}

/// The request body, read only once the token and the session passed (gate review G1: buffered first, anonymous uploads filled the
/// memory), up to one byte past the cap so that `check_body` names it too large; a longer or broken body is refused the same way.
async fn read_body(ds: &Docserver, body: Body) -> Result<Bytes, ApiError> {
    axum::body::to_bytes(body, (ds.max_document_mb << 20) as usize + 1)
        .await
        .map_err(|_| SessErr::TooLarge.into())
}

async fn save(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap, body: Body) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "save", &client(cl))?;
    editable(&rec)?;
    let body = read_body(&ds, body).await?;
    check_body(&ds, &rec, &body)?;
    // the answer comes after the provider's reply (the async-save ticket, 3.3): ok, failed or unknown; the version is kept either way
    let (v, cb, dirty) = blk(move || {
        let v = ds.put_version(&sid, &body, &rec.user_id, true, now())?;
        ds.audit(
            "save",
            json!({"session": sid_ref(&sid), "version": v.n, "sha256": v.sha256, "size": v.size, "user": rec.user_id}),
        );
        let rec = ds.load_session(&sid).ok_or(SessErr::NotFound)?;
        let cb = ds.callback_saved(&rec, &v);
        let dirty = ds.load_session(&sid).is_some_and(|r| r.dirty());
        Ok::<_, SessErr>((v, cb, dirty))
    })
    .await??;
    json_ok(json!({"version": v.n, "sha256": v.sha256, "callback": cb.as_str(), "dirty": dirty}))
}

/// `POST /ds/session/{sid}/dirty`: the editor's first change; the status 1 callback, once.
async fn dirty(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "dirty", &client(cl))?;
    editable(&rec)?;
    let cb = blk(move || ds.callback_dirty(&sid)).await?;
    json_ok(json!({"callback": cb.map(|c| c.as_str())}))
}

async fn autosave(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap, body: Body) -> Res {
    let rec = authorize(&ds, &h, &sid, false, "autosave", &client(cl))?;
    editable(&rec)?;
    let body = read_body(&ds, body).await?;
    check_body(&ds, &rec, &body)?;
    blk(move || ds.put_autosave(&sid, &body, now())).await??;
    json_ok(json!({"ok": true}))
}

/// `POST /ds/session/{sid}/ping`: the editor page is still open (every minute); without it an edit session is closed when idle.
async fn ping(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap) -> Res {
    authorize(&ds, &h, &sid, false, "ping", &client(cl))?;
    blk(move || ds.touch(&sid, now())).await??;
    json_ok(json!({"ok": true}))
}

/// `POST /ds/session/{sid}/close`, with an optional body: the editor's final copy, the edits no autosave holds yet (the wrapper
/// sends it in the one keepalive request that closes, so it cannot arrive after the close). It is checked as an autosave body;
/// one the session cannot take (view session, too large, not a sound document of its type) is left out and the close goes on.
async fn close(State(ds): Ds, cl: Cl, Path(sid): Path<String>, h: HeaderMap, body: Body) -> Res {
    let rec = authorize(&ds, &h, &sid, true, "close", &client(cl))?;
    // one byte past the cap, so that `check_body` names it too large; a longer body stops there and is refused the same way
    let cap = (ds.max_document_mb << 20) as usize + 1;
    let sound = |b: &Bytes| {
        editable(&rec)
            .and_then(|()| check_body(&ds, &rec, b))
            .is_ok()
    };
    let (last, copy) = match axum::body::to_bytes(body, cap).await {
        Ok(b) if b.is_empty() => (None, "none"),
        Ok(b) if sound(&b) => (Some(b), "kept"),
        _ => (None, "refused"),
    };
    let (c, cb, dirty) = blk(move || {
        let c = ds.close_with(&sid, last.as_deref(), now())?;
        ds.audit(
            "close",
            json!({"session": sid_ref(&sid), "status": c.status, "user": rec.user_id, "final_copy": copy}),
        );
        // status 2 is queued and retried until the provider acknowledges it; 4 is one try
        let cb = ds.callback_closed(&sid);
        let dirty = ds.load_session(&sid).is_some_and(|r| r.dirty());
        Ok::<_, SessErr>((c, cb, dirty))
    })
    .await??;
    json_ok(json!({"status": c.status, "callback": cb.map(|c| c.as_str()), "dirty": dirty}))
}

/// `GET /ds/files/{sid}/{ver}?t=<file token>`: the bytes of one version, for the token bound to exactly that sid, ver and digest.
async fn file(State(ds): Ds, cl: Cl, Path((sid, ver)): Path<(String, u64)>, req: Request) -> Res {
    let cl = client(cl);
    let no = |why: &str| denied(&ds, "file", &cl, why, unauthorized());
    let t = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("t=")))
        .filter(|t| t.len() <= 8192)
        .ok_or_else(|| no("no file token"))?
        .to_string();
    let c = ds
        .file_claims(&t, now())
        .ok_or_else(|| no("bad file token"))?;
    // the provider's working copy is never stamped (owner, 2026-10-26): stamps would pile up as it is saved and reopened; only copies
    // that leave the system (a history download) are
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
        Router::new()
            .route(
                "/ds/session",
                post(create).layer(DefaultBodyLimit::max(128 << 10)),
            )
            .route("/ds/session/{sid}/renew", post(renew))
            .route("/ds/session/{sid}/document", get(document))
            .route("/ds/session/{sid}/save", post(save))
            .route("/ds/session/{sid}/autosave", post(autosave))
            .route("/ds/session/{sid}/dirty", post(dirty))
            .route("/ds/session/{sid}/ping", post(ping))
            .route("/ds/session/{sid}/close", post(close))
            .with_state(self.clone())
    }

    pub(crate) fn file_route(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/ds/files/{sid}/{ver}", get(file))
            .with_state(self.clone())
    }
}
