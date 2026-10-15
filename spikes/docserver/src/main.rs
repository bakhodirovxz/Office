//! Phase 0 spike 4: document server (throwaway). See docs/phase0/spike-plans.md section 4.
mod callback;
mod config;
mod fetch;
mod jwt;
mod session;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use fetch::FetchError;
use serde_json::{json, Value};
use session::{version_path, Session, Sessions};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

struct AppState {
    secret: String,
    public: String,
    allowed: Vec<String>,
    max_bytes: usize,
    data_dir: PathBuf,
    static_dir: PathBuf,
    client: reqwest::Client,
    sessions: Mutex<Sessions>,
}

type St = State<Arc<AppState>>;
type R<T> = Result<T, (StatusCode, String)>;

fn env(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

#[tokio::main]
async fn main() {
    let addr = env("DOCSERVER_ADDR", "127.0.0.1:8780");
    let max_bytes: usize = env("DOCSERVER_MAX_BYTES", "104857600").parse().expect("DOCSERVER_MAX_BYTES");
    let st = Arc::new(AppState {
        secret: env("DOCSERVER_JWT_SECRET", "spike4-dev-secret"),
        public: env("DOCSERVER_PUBLIC_URL", "http://localhost:8780").trim_end_matches('/').to_string(),
        allowed: env("DOCSERVER_ALLOWED_HOSTS", "127.0.0.1:8790").split(',').map(|s| s.trim().to_string()).collect(),
        max_bytes,
        data_dir: PathBuf::from(env("DOCSERVER_DATA_DIR", "E:/office-target/docserver/data")),
        static_dir: PathBuf::from(env("DOCSERVER_STATIC_DIR", "static")),
        client: fetch::client(),
        sessions: Mutex::new(Sessions::default()),
    });
    let wc_dist = env("WORDCRAFT_WEB_DIST", "E:/office-target/spike4/wc-stock/wordcraft-web-0.7.0");
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/api.js", get(api_js))
        .route("/editor", get(editor_html))
        .route("/api/session", post(create_session))
        .route("/api/session/{sid}/document", get(get_document))
        .route("/api/session/{sid}/save", post(save).layer(DefaultBodyLimit::max(max_bytes)))
        .route("/api/session/{sid}/autosave", post(autosave).layer(DefaultBodyLimit::max(max_bytes)))
        .route("/api/session/{sid}/close", post(close))
        .route("/files/{sid}/{ver}", get(get_file))
        .nest_service("/wc", ServeDir::new(wc_dist))
        .with_state(st.clone());
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    eprintln!("docserver listening on {addr}, public {}, allowed {:?}", st.public, st.allowed);
    axum::serve(listener, app).await.expect("serve");
}

async fn static_file(st: &AppState, name: &str, mime: &'static str) -> Response {
    match tokio::fs::read(st.static_dir.join(name)).await {
        Ok(b) => ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-store")], b).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}
async fn api_js(State(st): St) -> Response {
    static_file(&st, "api.js", "text/javascript; charset=utf-8").await
}
async fn editor_html(State(st): St) -> Response {
    static_file(&st, "editor.html", "text/html; charset=utf-8").await
}

fn fetch_status(e: &FetchError) -> (StatusCode, String) {
    match e {
        FetchError::Blocked(m) => (StatusCode::FORBIDDEN, m.clone()),
        FetchError::Upstream(m) => (StatusCode::BAD_GATEWAY, m.clone()),
    }
}

async fn create_session(State(st): St, Json(body): Json<Value>) -> R<Json<Value>> {
    let token = body
        .pointer("/config/token")
        .and_then(Value::as_str)
        .ok_or((StatusCode::UNAUTHORIZED, "missing config.token".to_string()))?;
    // Trust ONLY the verified payload, never the unsigned outer fields.
    let payload = jwt::verify(st.secret.as_bytes(), token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("invalid token: {e}")))?;
    let cfg: config::Config = serde_json::from_value(payload)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad config: {e}")))?;
    let doc_url = fetch::check_allowed(&cfg.document.url, &st.allowed).map_err(|e| fetch_status(&e))?;
    fetch::check_allowed(&cfg.editor_config.callback_url, &st.allowed).map_err(|e| fetch_status(&e))?;
    let bytes = fetch::fetch(&st.client, doc_url, st.max_bytes).await.map_err(|e| fetch_status(&e))?;
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err((StatusCode::BAD_GATEWAY, "document is not a zip/docx".into()));
    }
    let readonly = cfg.editor_config.mode != "edit" || cfg.document.permissions.edit == Some(false);
    if readonly {
        eprintln!("session is read-only (mode={}); not enforced in the spike", cfg.editor_config.mode);
    }
    let sid = uuid::Uuid::new_v4().to_string();
    let dir = st.data_dir.join(&sid);
    tokio::fs::create_dir_all(&dir).await.map_err(internal)?;
    tokio::fs::write(version_path(&st.data_dir, &sid, 0), &bytes).await.map_err(internal)?;
    let name = cfg.document.title.clone().unwrap_or_else(|| format!("document.{}", cfg.document.file_type));
    let s = Session {
        name: name.clone(),
        mode: cfg.editor_config.mode.clone(),
        readonly,
        key: cfg.document.key,
        callback_url: cfg.editor_config.callback_url,
        filetype: cfg.document.file_type,
        user_id: cfg.editor_config.user.id,
        last_version: 0,
    };
    let mode = s.mode.clone();
    st.sessions.lock().await.insert(sid.clone(), s);
    eprintln!("session {sid} created ({} bytes)", bytes.len());
    Ok(Json(json!({"sid": sid, "name": name, "mode": mode})))
}

fn internal<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

async fn latest(st: &AppState, sid: &str) -> R<(Session, Vec<u8>)> {
    let s = st.sessions.lock().await.get(sid).ok_or((StatusCode::NOT_FOUND, "no such session".to_string()))?;
    let b = tokio::fs::read(version_path(&st.data_dir, sid, s.last_version)).await.map_err(internal)?;
    Ok((s, b))
}

async fn get_document(State(st): St, Path(sid): Path<String>) -> R<Response> {
    let (_, b) = latest(&st, &sid).await?;
    Ok(([(header::CONTENT_TYPE, DOCX_MIME)], b).into_response())
}

async fn save(State(st): St, Path(sid): Path<String>, headers: HeaderMap, body: Bytes) -> R<Json<Value>> {
    if !body.starts_with(b"PK\x03\x04") {
        return Err((StatusCode::BAD_REQUEST, "body is not a zip/docx".into()));
    }
    let _ = headers.get("x-file-name"); // informational; the session name is authoritative
    let ver = st
        .sessions
        .lock()
        .await
        .next_version(&sid)
        .ok_or((StatusCode::NOT_FOUND, "no such session".to_string()))?;
    tokio::fs::write(version_path(&st.data_dir, &sid, ver), &body).await.map_err(internal)?;
    let s = st.sessions.lock().await.get(&sid).ok_or((StatusCode::NOT_FOUND, "no such session".to_string()))?;
    let ctx = callback::CallbackCtx {
        client: &st.client,
        secret: st.secret.as_bytes(),
        public: &st.public,
        allowed: &st.allowed,
    };
    let (cb, detail) = match callback::send(&ctx, &sid, &s, ver, 6).await {
        Ok(()) => ("ok", String::new()),
        Err(d) => ("failed", d),
    };
    eprintln!("save sid={sid} ver={ver} bytes={} callback={cb} {detail}", body.len());
    Ok(Json(json!({"version": ver, "callback": cb, "detail": detail})))
}

/// A recovery copy of unsaved work (the editor was hidden or lost its GPU context). It is not a
/// version and no callback is sent: the HRM never sees it unless the user comes back and saves.
async fn autosave(State(st): St, Path(sid): Path<String>, body: Bytes) -> R<Json<Value>> {
    if !body.starts_with(b"PK\x03\x04") {
        return Err((StatusCode::BAD_REQUEST, "body is not a zip/docx".into()));
    }
    st.sessions.lock().await.get(&sid).ok_or((StatusCode::NOT_FOUND, "no such session".to_string()))?;
    tokio::fs::write(st.data_dir.join(&sid).join("autosave.docx"), &body).await.map_err(internal)?;
    eprintln!("autosave sid={sid} bytes={}", body.len());
    Ok(Json(json!({"bytes": body.len()})))
}

/// The editor was closed: callback status 2 (closed, the last saved version is final) or status 4
/// (closed with no changes saved).
async fn close(State(st): St, Path(sid): Path<String>) -> R<Json<Value>> {
    let s = st.sessions.lock().await.get(&sid).ok_or((StatusCode::NOT_FOUND, "no such session".to_string()))?;
    let status = if s.last_version == 0 { 4 } else { 2 };
    let ctx = callback::CallbackCtx {
        client: &st.client,
        secret: st.secret.as_bytes(),
        public: &st.public,
        allowed: &st.allowed,
    };
    let (cb, detail) = match callback::send(&ctx, &sid, &s, s.last_version, status).await {
        Ok(()) => ("ok", String::new()),
        Err(d) => ("failed", d),
    };
    eprintln!("close sid={sid} ver={} status={status} callback={cb} {detail}", s.last_version);
    Ok(Json(json!({"version": s.last_version, "status": status, "callback": cb, "detail": detail})))
}

async fn get_file(
    State(st): St,
    Path((sid, ver)): Path<(String, u32)>,
    Query(q): Query<HashMap<String, String>>,
) -> R<Response> {
    let t = q.get("t").ok_or((StatusCode::UNAUTHORIZED, "missing t".to_string()))?;
    let p = jwt::verify(st.secret.as_bytes(), t).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    if p.get("sid").and_then(Value::as_str) != Some(&sid) || p.get("ver").and_then(Value::as_u64) != Some(ver as u64) {
        return Err((StatusCode::UNAUTHORIZED, "token does not match file".into()));
    }
    let b = tokio::fs::read(version_path(&st.data_dir, &sid, ver))
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "no such version".to_string()))?;
    Ok(([(header::CONTENT_TYPE, DOCX_MIME)], b).into_response())
}
