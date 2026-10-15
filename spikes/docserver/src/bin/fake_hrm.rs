//! Fake HRM (spike 4): serves the page + fixtures, signs configs, receives and verifies callbacks.
#[path = "../jwt.rs"]
mod jwt;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{io::AsyncWriteExt, sync::Mutex};

struct Hrm {
    secret: String,
    /// Test-only override: secret used to VERIFY callbacks (simulates a secret mismatch
    /// between docserver and HRM without restarting servers). Set via /admin/callback-secret.
    cb_secret_override: Mutex<Option<String>>,
    docserver: String,
    fixtures: PathBuf,
    page: PathBuf,
    received: PathBuf,
    counter: Mutex<u32>,
    client: reqwest::Client,
}

type St = State<Arc<Hrm>>;

fn env(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

#[tokio::main]
async fn main() {
    let received = PathBuf::from(env("HRM_RECEIVED_DIR", "E:/office-target/docserver/received"));
    std::fs::create_dir_all(&received).expect("received dir");
    let st = Arc::new(Hrm {
        secret: env("DOCSERVER_JWT_SECRET", "spike4-dev-secret"),
        cb_secret_override: Mutex::new(None),
        docserver: env("HRM_DOCSERVER_URL", "http://localhost:8780"),
        fixtures: PathBuf::from(env("HRM_FIXTURES_DIR", "fixtures")),
        page: PathBuf::from(env("HRM_PAGE", "hrm/index.html")),
        received,
        counter: Mutex::new(0),
        client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().unwrap(),
    });
    let addr = env("HRM_ADDR", "127.0.0.1:8790");
    let app = Router::new()
        .route("/", get(index))
        .route("/files/{name}", get(file))
        .route("/config/{name}", get(config))
        .route("/callback", post(callback))
        .route("/received", get(|State(st): St| jsonl(st.received.join("received.jsonl"))))
        .route("/rejected", get(|State(st): St| jsonl(st.received.join("rejected.jsonl"))))
        .route("/admin/callback-secret", post(set_cb_secret))
        .with_state(st);
    let l = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    eprintln!("fake_hrm listening on {addr}");
    axum::serve(l, app).await.expect("serve");
}

async fn jsonl(p: PathBuf) -> Response {
    let t = tokio::fs::read_to_string(p).await.unwrap_or_default();
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], t).into_response()
}

async fn index(State(st): St) -> Response {
    match tokio::fs::read_to_string(&st.page).await {
        Ok(t) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")],
            t.replace("{{DOCSERVER}}", &st.docserver),
        )
            .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

fn safe(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && !name.contains("..")
}

async fn file(State(st): St, Path(name): Path<String>) -> Response {
    if !safe(&name) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match tokio::fs::read(st.fixtures.join(&name)).await {
        Ok(b) => ([(header::CONTENT_TYPE, "application/octet-stream")], b).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `?url=` overrides document.url (still correctly signed) so the SSRF test can sign a bad URL.
async fn config(State(st): St, Path(name): Path<String>, Query(q): Query<HashMap<String, String>>) -> Response {
    if !safe(&name) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let url = q.get("url").cloned().unwrap_or_else(|| format!("http://127.0.0.1:8790/files/{name}"));
    let cfg = json!({
        "documentType": "word",
        "document": {
            "fileType": "docx", "key": format!("{name}-{now_ms}"), "title": name, "url": url,
            "permissions": {"edit": true, "download": true}
        },
        "editorConfig": {
            "callbackUrl": "http://127.0.0.1:8790/callback?doc=42",
            "mode": "edit", "lang": "en", "user": {"id": "u1", "name": "Test User"}
        },
        "exp": jwt::now() + 300,
    });
    let token = jwt::sign(st.secret.as_bytes(), &cfg);
    let mut out = cfg;
    out.as_object_mut().unwrap().remove("exp");
    out["token"] = Value::String(token);
    ([(header::CACHE_CONTROL, "no-store")], Json(out)).into_response()
}

async fn set_cb_secret(State(st): St, Query(q): Query<HashMap<String, String>>) -> &'static str {
    *st.cb_secret_override.lock().await = q.get("value").filter(|v| !v.is_empty()).cloned();
    "ok"
}

async fn reject(st: &Hrm, why: &str, body: &Value) -> Response {
    let line = json!({"ts": jwt::now(), "reason": why, "key": body.get("key")}).to_string();
    append(&st.received.join("rejected.jsonl"), &line).await;
    eprintln!("callback rejected: {why}");
    (StatusCode::UNAUTHORIZED, Json(json!({"error": 1}))).into_response()
}

async fn append(p: &PathBuf, line: &str) {
    if let Ok(mut f) = tokio::fs::OpenOptions::new().create(true).append(true).open(p).await {
        let _ = f.write_all(format!("{line}\n").as_bytes()).await;
    }
}

async fn callback(State(st): St, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let secret = st.cb_secret_override.lock().await.clone().unwrap_or_else(|| st.secret.clone());
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(bearer) = bearer else { return reject(&st, "missing bearer", &body).await };
    let payload = match jwt::verify(secret.as_bytes(), bearer) {
        Ok(p) => p,
        Err(e) => return reject(&st, &format!("bearer {e}"), &body).await,
    };
    if payload.pointer("/payload/key") != body.get("key") || body.get("key").is_none() {
        return reject(&st, "payload.key != body.key", &body).await;
    }
    let status = body.get("status").and_then(Value::as_u64).unwrap_or(0);
    if status == 2 || status == 6 {
        let Some(url) = body.get("url").and_then(Value::as_str) else {
            return reject(&st, "no url", &body).await;
        };
        let bytes = match st.client.get(url).send().await {
            Ok(r) if r.status().is_success() => r.bytes().await.map(|b| b.to_vec()).unwrap_or_default(),
            Ok(r) => return reject(&st, &format!("download status {}", r.status()), &body).await,
            Err(e) => return reject(&st, &format!("download error {e}"), &body).await,
        };
        let key = body["key"].as_str().unwrap_or("nokey").replace(['/', '\\', ':'], "_");
        let n = {
            let mut c = st.counter.lock().await;
            *c += 1;
            *c
        };
        let file = st.received.join(format!("{key}-{n}.docx"));
        if tokio::fs::write(&file, &bytes).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": 1}))).into_response();
        }
        let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let line = json!({
            "ts": jwt::now(), "key": body["key"], "status": status, "bytes": bytes.len(),
            "sha256": sha, "file": file.to_string_lossy(),
        });
        append(&st.received.join("received.jsonl"), &line.to_string()).await;
        eprintln!("callback ok status={status} bytes={} -> {}", bytes.len(), file.display());
    }
    Json(json!({"error": 0})).into_response()
}
