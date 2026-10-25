#![forbid(unsafe_code)]
//! Fake HR backend (PLAN phase 5, WP1): the ONLYOFFICE integration of the HR backend as surveyed, for docserver tests.
//!
//! Faithful to `docs/phase0/hrm-current-state.md` (cited below as `[S:<line>]`, a line of that file) and to the backend work
//! order `docs/phase0/hr-backend-prompt.md` (`[P:<line>]`), not to the Phase 0 spike. Where the survey is silent the strict
//! behaviour is chosen and tagged `OPEN-n` (list: `OPEN_QUESTIONS`). Documents are synthetic bytes the test supplies, kept in
//! memory: the harness writes no file. Every secret is generated per `Config`.
pub mod jwt;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// Where the survey is silent. Each is answered by the HRM team or WP17; the harness is strict meanwhile.
pub const OPEN_QUESTIONS: [&str; 8] = [
    "OPEN-1 callback reply status for a rejected token / key / host: survey says only '401 JSON on a bad token' [P:850]; harness: 401 token, 403 key or host, body {\"error\":1}",
    "OPEN-2 `type` in a callback token: survey rejects `access`/`refresh` [S:281]; harness rejects any `type` claim",
    "OPEN-3 body `key` vs signed key: only PR-1 makes it strict [P:394-397]; harness always requires equality when the body has a key",
    "OPEN-4 does the allowlist compare the port: survey says 'a host in ...' [S:287]; harness follows PR-1 (bare entry = any port, host:port exact) [P:408-410]",
    "OPEN-5 extension to documentType map: survey gives the four types, not the extension list [S:252]; harness maps docx, xlsx, pptx, pdf only",
    "OPEN-6 a callback for a locked order act or letter: survey says writes are refused [S:294], not the reply; harness models no lock on callbacks",
    "OPEN-7 `exp` of the callback token and of the file token: survey is silent for callbacks; harness checks `exp` when present (file token: required, HRB sets +3600 s [S:256])",
    "OPEN-8 does the editor-config route need the user's login: surely, but the survey does not give the scheme; harness serves it unauthenticated",
];

/// The three resource kinds with a save callback in the pilot scope. Letter reports (`letter_report_`) are not modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    File,
    OrderAct,
    Letter,
}

impl Kind {
    /// route segment [S:259, S:272-275]
    pub fn segment(self) -> &'static str {
        match self {
            Kind::File => "files",
            Kind::OrderAct => "order-acts",
            Kind::Letter => "letters",
        }
    }
    /// key prefix `{kind}_{id}_` [S:254, S:283]
    pub fn key_prefix(self, id: u64) -> String {
        let k = match self {
            Kind::File => "file",
            Kind::OrderAct => "order_act",
            Kind::Letter => "letter",
        };
        format!("{k}_{id}_")
    }
    fn from_segment(s: &str) -> Option<Kind> {
        [Kind::File, Kind::OrderAct, Kind::Letter]
            .into_iter()
            .find(|k| k.segment() == s)
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// `BACKEND_DOCS_SECRET`: signs the editor config, verifies the callback token [S:265, S:280]
    pub ds_secret: String,
    /// `JWT_SECRET`: signs the file-route token [S:256]. Set equal to `ds_secret` to model the fallback [S:265]
    pub jwt_secret: String,
    /// `BACKEND_SELF_URL`: host of `document.url` and `callbackUrl` [S:226-227]. None = this server's own base URL
    pub internal_backend_url: Option<String>,
    /// `BACKEND_DOCS_URL`: the callback `url` host is rewritten to this [S:289-290]
    pub callback_internal_base: String,
    /// `BACKEND_CALLBACK_HOSTS`: `host` (any port) or `host:port` [S:287; OPEN-4]. Empty allows nothing
    pub callback_allowed_hosts: Vec<String>,
    /// PR-1: accept `Authorization: Bearer` (payload `{"payload": body}`) when the body has no token [P:399-401]. Off = as surveyed [S:279]
    pub bearer_fallback: bool,
    /// PR-3 optional commit: sign `exp` (now + 12 h) into the config token [P:513-514]. Off = as surveyed [S:265, P:829]
    pub include_exp: bool,
    /// `BACKEND_MOBILE_EDIT`: without it `device=mobile` forces view [S:264]
    pub mobile_edit: bool,
    /// origin of the document server, put into the host page's `api.js` URL
    pub docserver_public_url: String,
    /// `httpx.Client(timeout=30)` [S:291]
    pub download_timeout: Duration,
    /// Browser-test routes (`GET /__test/state`, `POST /__test/callback-secret`). Off by default: not part of the surveyed backend.
    pub test_api: bool,
}

impl Config {
    pub fn generated(callback_internal_base: &str) -> Config {
        Config {
            ds_secret: rand_hex(32),
            jwt_secret: rand_hex(32),
            internal_backend_url: None,
            callback_internal_base: callback_internal_base.trim_end_matches('/').to_string(),
            callback_allowed_hosts: vec![],
            bearer_fallback: false,
            include_exp: false,
            mobile_edit: false,
            docserver_public_url: "http://localhost:0".into(),
            download_timeout: Duration::from_secs(30),
            test_api: false,
        }
    }
}

/// Test-grade randomness (std's randomly keyed hasher over the clock): enough for per-test secrets and key suffixes.
fn rand_hex(bytes: usize) -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut out = String::new();
    let mut n = 0u64;
    while out.len() < bytes * 2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(n);
        h.write_u128(nanos);
        out.push_str(&format!("{:016x}", h.finish()));
        n += 1;
    }
    out.truncate(bytes * 2);
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Body,
    Bearer,
}

/// A save that was accepted and stored.
#[derive(Clone, Debug)]
pub struct Received {
    pub kind: Kind,
    pub id: u64,
    pub status: u64,
    pub key: String,
    pub users: Vec<String>,
    pub bytes: usize,
    pub sha256: String,
    pub via: Via,
}

/// A callback that was refused or whose download failed, with the reason (never a URL query or a token).
#[derive(Clone, Debug)]
pub struct Rejected {
    pub reason: String,
    pub key: Option<String>,
}

struct Rec {
    name: String,
    object: String,
    updated_at: u64,
    can_edit: bool,
    locked: bool,
}

#[derive(Default)]
struct Inner {
    recs: HashMap<(Kind, u64), Rec>,
    objects: HashMap<String, Vec<u8>>,
    received: Vec<Received>,
    rejected: Vec<Rejected>,
    cb_secret_override: Option<String>,
}

struct Shared {
    cfg: Mutex<Config>,
    inner: Mutex<Inner>,
    base_url: String,
}

impl Shared {
    fn cfg(&self) -> Config {
        self.cfg.lock().unwrap().clone()
    }
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }
    fn reject(&self, status: StatusCode, reason: impl Into<String>, key: Option<&str>) -> Response {
        self.inner().rejected.push(Rejected {
            reason: reason.into(),
            key: key.map(str::to_string),
        });
        (status, Json(json!({"error": 1}))).into_response()
    }
}

const BUCKET: &str = "docs-test";

/// A running fake backend. Dropping it stops the server.
pub struct FakeHrm {
    shared: Arc<Shared>,
    addr: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeHrm {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeHrm {
    /// Bind `127.0.0.1:0` and serve.
    pub async fn start(cfg: Config) -> std::io::Result<FakeHrm> {
        Self::start_on("127.0.0.1:0", cfg).await
    }

    pub async fn start_on(addr: &str, cfg: Config) -> std::io::Result<FakeHrm> {
        let l = tokio::net::TcpListener::bind(addr).await?;
        let addr = l.local_addr()?;
        let shared = Arc::new(Shared {
            cfg: Mutex::new(cfg),
            inner: Mutex::default(),
            base_url: format!("http://{addr}"),
        });
        let app = Router::new()
            .route("/{seg}/{pk}/editor-config", get(editor_config)) // [S:238-240]
            .route("/{seg}/{pk}/save-callback", post(save_callback)) // [S:272-275]
            .route("/docs-file/{token}", get(file_route)) // [S:256]
            .route("/host", get(host_page))
            .route("/__test/state", get(test_state))
            .route("/__test/callback-secret", post(test_secret))
            .with_state(shared.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(l, app).await;
        });
        Ok(FakeHrm { shared, addr, task })
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }
    pub fn base_url(&self) -> &str {
        &self.shared.base_url
    }
    /// The host page under another site name (`localhost` vs `127.0.0.1`, or a name in the hosts file): the cross-site case.
    pub fn page_url(&self, site: &str, kind: Kind, id: u64) -> String {
        format!(
            "http://{site}:{}/host?kind={}&id={id}",
            self.addr.port(),
            kind.segment()
        )
    }
    pub fn config(&self) -> Config {
        self.shared.cfg()
    }
    pub fn update_config(&self, f: impl FnOnce(&mut Config)) {
        f(&mut self.shared.cfg.lock().unwrap());
    }
    /// Add or replace a synthetic document. The object name is `{kind}/{id}/{name}`.
    pub fn add_file(&self, kind: Kind, id: u64, name: &str, bytes: Vec<u8>) {
        let object = format!("{}/{id}/{name}", kind.segment());
        let mut g = self.shared.inner();
        g.objects.insert(object.clone(), bytes);
        g.recs.insert(
            (kind, id),
            Rec {
                name: name.into(),
                object,
                updated_at: jwt::now(),
                can_edit: true,
                locked: false,
            },
        );
    }
    /// Per-record switches that decide `editorConfig.mode` [S:258].
    pub fn set_access(&self, kind: Kind, id: u64, can_edit: bool, locked: bool) {
        if let Some(r) = self.shared.inner().recs.get_mut(&(kind, id)) {
            r.can_edit = can_edit;
            r.locked = locked;
        }
    }
    /// The stored bytes of a document (a saved callback overwrites them [S:292]).
    pub fn object(&self, kind: Kind, id: u64) -> Option<Vec<u8>> {
        let g = self.shared.inner();
        g.objects.get(&g.recs.get(&(kind, id))?.object).cloned()
    }
    pub fn received(&self) -> Vec<Received> {
        self.shared.inner().received.clone()
    }
    pub fn rejected(&self) -> Vec<Rejected> {
        self.shared.inner().rejected.clone()
    }
    /// Verify callbacks with another secret than `ds_secret` (None restores): the secret-mismatch case of spike S7 (iii).
    pub fn set_callback_secret(&self, secret: Option<String>) {
        self.shared.inner().cb_secret_override = secret;
    }
}

fn dt_for(ext: &str) -> Option<&'static str> {
    // OPEN-5
    match ext {
        "docx" => Some("word"),
        "xlsx" => Some("cell"),
        "pptx" => Some("slide"),
        "pdf" => Some("pdf"),
        _ => None,
    }
}

fn detail(status: StatusCode, d: &str) -> Response {
    (status, Json(json!({"detail": d}))).into_response()
}

/// `GET /{files|order-acts|letters}/{pk}/editor-config?mode&theme&device` [S:238-244, S:248-265]
async fn editor_config(
    State(st): State<Arc<Shared>>,
    Path((seg, pk)): Path<(String, u64)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(kind) = Kind::from_segment(&seg) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cfg = st.cfg();
    let (name, object, updated_at, can_edit, locked) = {
        let g = st.inner();
        let Some(r) = g.recs.get(&(kind, pk)) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        (
            r.name.clone(),
            r.object.clone(),
            r.updated_at,
            r.can_edit,
            r.locked,
        )
    };
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e).to_lowercase();
    let Some(doc_type) = dt_for(&ext) else {
        return detail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_document_type",
        ); // [S:252]
    };
    let mobile = q.get("device").is_some_and(|d| d == "mobile");
    // [S:258, S:264]: locked or no right -> view; mobile -> view unless BACKEND_MOBILE_EDIT
    let edit = q.get("mode").is_none_or(|m| m != "view")
        && can_edit
        && !locked
        && !(mobile && !cfg.mobile_edit);
    let backend = cfg
        .internal_backend_url
        .clone()
        .unwrap_or_else(|| st.base_url.clone());
    let backend = backend.trim_end_matches('/');
    let file_token = jwt::sign(
        cfg.jwt_secret.as_bytes(),
        &json!({"purpose": "/docs-file/", "bucket": BUCKET, "object": object, "exp": jwt::now() + 3600}),
    ); // [S:256]
    let mut editor = json!({
        "mode": if edit { "edit" } else { "view" },
        "lang": "ru", // [S:261]
        "user": {"id": "emp-1", "name": "Synthetic User"}, // [S:260]
        "customization": {
            "forcesave": edit, // [S:262]
            "uiTheme": if q.get("theme").is_some_and(|t| t == "dark") { "theme-dark" } else { "theme-classic-light" }, // [S:263]
        },
    });
    if edit {
        editor["callbackUrl"] = json!(format!("{backend}/{seg}/{pk}/save-callback")); // omitted for view [S:259]
    }
    let mut c = json!({
        "documentType": doc_type,
        "document": {
            "fileType": ext,
            "key": format!("{}v{updated_at}_{}", kind.key_prefix(pk), rand_hex(4)), // fresh suffix on every open [S:254]
            "title": name,
            "url": format!("{backend}/docs-file/{file_token}"),
            "permissions": {"edit": edit, "download": q.get("download").is_none_or(|v| v != "0"), "print": q.get("print").is_none_or(|v| v != "0"), "review": false, "comment": edit}, // [S:257]
        },
        "editorConfig": editor,
    });
    if mobile {
        c["type"] = json!("mobile");
    }
    // the token covers the whole config, signed before `token` is added [S:265]
    let mut signed = c.clone();
    if cfg.include_exp {
        signed["exp"] = json!(jwt::now() + 12 * 3600);
    }
    c["token"] = json!(jwt::sign(cfg.ds_secret.as_bytes(), &signed));
    ([(header::CACHE_CONTROL, "no-store")], Json(c)).into_response()
}

/// `GET /docs-file/{jwt}`: public, purpose-checked [S:256]
async fn file_route(State(st): State<Arc<Shared>>, Path(token): Path<String>) -> Response {
    let cfg = st.cfg();
    let Ok(p) = jwt::verify(cfg.jwt_secret.as_bytes(), &token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if p.get("exp").is_none() || p.get("purpose").and_then(Value::as_str) != Some("/docs-file/")
    {
        return StatusCode::FORBIDDEN.into_response(); // OPEN-7
    }
    let (Some(bucket), Some(object)) = (
        p.get("bucket").and_then(Value::as_str),
        p.get("object").and_then(Value::as_str),
    ) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    match st.inner().objects.get(object).filter(|_| bucket == BUCKET) {
        Some(b) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            b.clone(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Test-only (`Config::test_api`): what the backend holds and has accepted, for a browser test that cannot reach the in-process API.
async fn test_state(State(st): State<Arc<Shared>>) -> Response {
    if !st.cfg().test_api {
        return StatusCode::NOT_FOUND.into_response();
    }
    let g = st.inner();
    let digest = |b: &[u8]| {
        Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
    };
    Json(json!({
        "received": g.received.iter().map(|r| json!({"kind": r.kind.segment(), "id": r.id, "status": r.status, "bytes": r.bytes, "sha256": r.sha256})).collect::<Vec<_>>(),
        "rejected": g.rejected.iter().map(|r| r.reason.clone()).collect::<Vec<_>>(),
        "objects": g.objects.iter().map(|(k, v)| (k.clone(), digest(v))).collect::<HashMap<_, _>>(),
    }))
    .into_response()
}

/// Test-only: the body is the secret callbacks are verified with from now on; an empty body restores `ds_secret`.
async fn test_secret(State(st): State<Arc<Shared>>, body: String) -> Response {
    if !st.cfg().test_api {
        return StatusCode::NOT_FOUND.into_response();
    }
    st.inner().cb_secret_override = (!body.is_empty()).then_some(body);
    StatusCode::NO_CONTENT.into_response()
}

async fn host_page(State(st): State<Arc<Shared>>) -> Response {
    let html = include_str!("host.html").replace(
        "{{DOCSERVER}}",
        st.cfg().docserver_public_url.trim_end_matches('/'),
    );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        html,
    )
        .into_response()
}

/// `key` at the top level or under `payload` [S:283]
fn str_key(p: &Value) -> Option<&str> {
    p.get("key")
        .or_else(|| p.get("payload")?.get("key"))
        .and_then(Value::as_str)
}

/// `POST /{files|order-acts|letters}/{pk}/save-callback` [S:271-300]
async fn save_callback(
    State(st): State<Arc<Shared>>,
    Path((seg, pk)): Path<(String, u64)>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(kind) = Kind::from_segment(&seg) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cfg = st.cfg();
    let secret = st
        .inner()
        .cb_secret_override
        .clone()
        .unwrap_or_else(|| cfg.ds_secret.clone());
    let body_key = body.get("key").and_then(Value::as_str);
    let bad = |s: StatusCode, why: &str| st.reject(s, why, body_key);

    // token: the body `token` field [S:279]; the header only in PR-1 mode, when the body has none [P:399-401]
    let body_tok = body.get("token").and_then(Value::as_str);
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|_| cfg.bearer_fallback);
    let via = if body_tok.is_some() {
        Via::Body
    } else {
        Via::Bearer
    };
    let mut signed: Vec<Value> = vec![];
    for (tok, from_header) in [(body_tok, false), (bearer, true)] {
        let Some(tok) = tok else { continue };
        match jwt::verify(secret.as_bytes(), tok) {
            // the header JWT's payload is `{"payload": body}` [P:401]
            Ok(p) => signed.push(if from_header {
                p.get("payload").cloned().unwrap_or(Value::Null)
            } else {
                p
            }),
            Err(e) => return bad(StatusCode::UNAUTHORIZED, &format!("token {e:?}")), // HS256 only [S:280]
        }
    }
    let Some(p) = signed.first() else {
        return bad(StatusCode::UNAUTHORIZED, "no token");
    };
    if signed.len() == 2 && str_key(&signed[0]) != str_key(&signed[1]) {
        return bad(StatusCode::FORBIDDEN, "body and header keys differ"); // [P:401-402]
    }
    // a replayed editor or session token [S:281-282]; checked at both levels (the header form nests the body under `payload`)
    for scope in [Some(p), p.get("payload")].into_iter().flatten() {
        for claim in ["type", "editorConfig", "documentType"] {
            if scope.get(claim).is_some() {
                return bad(StatusCode::FORBIDDEN, &format!("forbidden claim {claim}")); // OPEN-2
            }
        }
    }
    let prefix = kind.key_prefix(pk);
    let Some(skey) = str_key(p).filter(|k| k.starts_with(&prefix)) else {
        return bad(StatusCode::FORBIDDEN, "key prefix"); // [S:283]
    };
    if body_key.is_some_and(|k| k != skey) {
        return bad(StatusCode::FORBIDDEN, "body key != signed key"); // OPEN-3
    }
    let skey = skey.to_string();

    // only 2 and 6 act; everything else is {"error":0} [S:284-285]
    let status = body.get("status").and_then(Value::as_u64).unwrap_or(0);
    if status != 2 && status != 6 {
        return Json(json!({"error": 0})).into_response();
    }
    let Some(url) = body.get("url").and_then(Value::as_str) else {
        return st.reject(StatusCode::OK, "no url", Some(&skey)); // [S:298]
    };
    let Some(rest) = allowed_rest(url, &cfg.callback_allowed_hosts) else {
        return st.reject(
            StatusCode::FORBIDDEN,
            "url refused (scheme or host)",
            Some(&skey),
        ); // [S:287]; OPEN-1
    };
    let object = st.inner().recs.get(&(kind, pk)).map(|r| r.object.clone()); // the guard must not outlive this line
    let Some(object) = object else {
        return st.reject(StatusCode::NOT_FOUND, "unknown record", Some(&skey));
    };
    let target = format!("{}{rest}", cfg.callback_internal_base); // host rewritten, path and query kept [S:289-290]
    let timeout = cfg.download_timeout;
    let bytes = match tokio::task::spawn_blocking(move || download(&target, timeout)).await {
        Ok(Ok(b)) => b,
        // {"error":1} so the document server retries [S:298]
        Ok(Err(e)) => {
            return st.reject(StatusCode::OK, format!("download failed: {e}"), Some(&skey));
        }
        Err(_) => return st.reject(StatusCode::OK, "download task failed", Some(&skey)),
    };
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let users = body
        .get("users")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut g = st.inner();
    let n = bytes.len();
    g.objects.insert(object, bytes); // same object name, overwritten [S:292]
    g.received.push(Received {
        kind,
        id: pk,
        status,
        key: skey,
        users,
        bytes: n,
        sha256,
        via,
    });
    Json(json!({"error": 0})).into_response()
}

/// `http(s)` URL with no userinfo and a host on the allowlist ([S:287]; entries `host` = any port, `host:port` exact, OPEN-4).
/// Returns path + query for the rewrite. shortcut: bracketed IPv6 hosts are refused, upgrade if a test needs them.
fn allowed_rest(url: &str, allowed: &[String]) -> Option<String> {
    let (scheme, tail) = url.split_once("://")?;
    let default_port = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    if url.chars().any(|c| c.is_control() || c == ' ' || c == '\\') {
        return None;
    }
    let end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
    let (auth, rest) = tail.split_at(end);
    if auth.is_empty() || auth.contains(['@', '[']) {
        return None;
    }
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (auth, default_port),
    };
    let host = host.to_ascii_lowercase();
    allowed
        .iter()
        .any(|a| {
            let a = a.to_ascii_lowercase();
            a == host || a == format!("{host}:{port}")
        })
        .then(|| {
            let rest = rest.split('#').next().unwrap_or("");
            if rest.starts_with('?') {
                format!("/{rest}")
            } else {
                rest.to_string()
            }
        })
}

/// 30 s total, no redirects, no proxy, 200 only [S:291; P:411-417]. The body is capped at 256 MB.
fn download(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut r = agent.get(url).call().map_err(|e| e.to_string())?;
    if r.status().as_u16() != 200 {
        return Err(format!("status {}", r.status().as_u16()));
    }
    r.body_mut()
        .with_config()
        .limit(256 << 20)
        .read_to_vec()
        .map_err(|e| e.to_string())
}
