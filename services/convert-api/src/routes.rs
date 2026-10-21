//! The routes (PLAN 5.1), the auth and header middleware (5.5) and the error type.
use crate::app::{App, FileState, Job, StoredInput, estimate_mb, ip_key, run_file};
use crate::config::Auth;
use crate::jwt;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Multipart, Path as UrlPath, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use convert::exec::Exit;
use convert::job::InputSource;
use convert::kind::{self, DetectError};
use convert::names::safe_name;
use convert::plan::{
    self, Options, PageSpec, PdfEngine, PlanError, SheetSel, Target, Translit, TranslitDir,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::limit::RequestBodyLimitLayer;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }
    pub fn retry(mut self, secs: u64) -> ApiError {
        self.retry_after = Some(secs);
        self
    }
    pub fn internal(message: String) -> ApiError {
        eprintln!("internal error: {message}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
    }
    fn not_found() -> ApiError {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such job, file or output",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut r = (
            self.status,
            Json(json!({"error": {"code": self.code, "message": self.message}})),
        )
            .into_response();
        if let Some(s) = self.retry_after {
            r.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(s));
        }
        if self.status == StatusCode::UNAUTHORIZED {
            r.headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        r
    }
}

type Res<T = Response> = Result<T, ApiError>;

#[derive(Clone)]
pub struct Principal(pub String);
/// The client address behind any trusted proxy: the plain address (logs, the hooks) and the key the limits count by (an IPv6 /64).
#[derive(Clone)]
pub struct Client {
    pub ip: String,
    pub key: String,
}

const CSP: &str = "default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

/// Every response gets the same baseline headers (PLAN 5.5), errors and static files included. No CORS header is ever set.
pub fn baseline_headers(h: &mut axum::http::HeaderMap, hsts: bool) {
    for (k, v) in [
        ("content-security-policy", CSP),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        ("cache-control", "no-store"),
        // nobody else's page may embed or probe our responses
        ("cross-origin-resource-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
    ] {
        h.insert(k, HeaderValue::from_static(v));
    }
    if hsts {
        h.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000"),
        );
    }
}

/// Where a request really came from. A peer that is a trusted proxy may speak for the client in `X-Forwarded-For` (the rightmost
/// address that is not itself a trusted proxy counts) and `X-Forwarded-Proto`; from any other peer both headers are ignored.
struct Origin {
    peer: Option<IpAddr>,
    client: Option<IpAddr>,
    via_proxy: bool,
    /// the client's own connection was TLS: ours, or the proxy's according to `X-Forwarded-Proto`
    https: bool,
}

fn origin(app: &App, req: &Request) -> Origin {
    let peer = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_canonical());
    if !peer.is_some_and(|p| app.r.is_trusted_proxy(p)) {
        return Origin {
            peer,
            client: peer,
            via_proxy: false,
            https: app.tls,
        };
    }
    let joined = |name: &str| {
        req.headers()
            .get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut client = peer;
    for e in joined("x-forwarded-for").rsplit(',') {
        let Ok(ip) = e.trim().parse::<IpAddr>() else {
            break;
        };
        client = Some(ip.to_canonical());
        if !app.r.is_trusted_proxy(ip) {
            break;
        }
    }
    let https = joined("x-forwarded-proto")
        .rsplit(',')
        .next()
        .is_some_and(|p| p.trim().eq_ignore_ascii_case("https"));
    Origin {
        peer,
        client,
        via_proxy: true,
        https,
    }
}

async fn headers(State(app): State<Arc<App>>, req: Request, next: Next) -> Response {
    // HSTS goes only on answers the client received over TLS: ours, or the trusted proxy's
    let hsts = app.tls || (app.r.upstream_tls && origin(&app, &req).https);
    let mut res = next.run(req).await;
    baseline_headers(res.headers_mut(), hsts);
    res
}

/// Against DNS rebinding and cross-site requests to the local server: the `Host` must be one we serve, and an `Origin`, when
/// the browser sends one, must be this very host. With `tls = "upstream"` only the trusted proxy may talk to us, and a request it
/// received over plain HTTP is redirected to `public_url`.
async fn guard(State(app): State<Arc<App>>, mut req: Request, next: Next) -> Response {
    let o = origin(&app, &req);
    let ip = o
        .client
        .map_or_else(|| "unknown".to_string(), |ip| ip.to_string());
    let client = Client {
        key: o.client.map_or_else(|| "unknown".to_string(), ip_key),
        ip,
    };
    if app.r.upstream_tls {
        if !o.via_proxy {
            // a health probe on the loopback address of the container is the one exception; it learns only `{"ok":true}`
            if o.peer.is_some_and(|p| p.is_loopback()) && req.uri().path() == "/healthz" {
                req.extensions_mut().insert(client);
                return next.run(req).await;
            }
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "not_via_proxy",
                "this server answers its reverse proxy only",
            )
            .into_response();
        }
        if !o.https {
            let base = app
                .r
                .s
                .public_url
                .as_deref()
                .unwrap_or("")
                .trim_end_matches('/');
            let to = format!(
                "{base}{}",
                req.uri().path_and_query().map_or("/", |p| p.as_str())
            );
            return (
                StatusCode::PERMANENT_REDIRECT,
                [(
                    header::LOCATION,
                    HeaderValue::from_str(&to).unwrap_or_else(|_| HeaderValue::from_static("/")),
                )],
            )
                .into_response();
        }
    }
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_ascii_lowercase);
    if !app.allowed_hosts.is_empty()
        && !host.as_ref().is_some_and(|h| app.allowed_hosts.contains(h))
    {
        return ApiError::new(
            StatusCode::MISDIRECTED_REQUEST,
            "bad_host",
            "this host name is not served here",
        )
        .into_response();
    }
    if let Some(o) = req.headers().get(header::ORIGIN) {
        let origin = o.to_str().unwrap_or("").to_ascii_lowercase();
        let same = origin
            .split_once("://")
            .is_some_and(|(_, a)| Some(a) == host.as_deref());
        if !same {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "bad_origin",
                "cross-origin requests are not allowed",
            )
            .into_response();
        }
    }
    req.extensions_mut().insert(client);
    next.run(req).await
}

/// The user a request belongs to: `local` in local mode, else the `sub` of a valid bearer token.
fn authenticate(app: &App, h: &axum::http::HeaderMap) -> Option<String> {
    match &app.r.auth {
        Auth::None => Some("local".to_string()),
        Auth::Jwt { secret, rules } => {
            let tok = h
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .filter(|t| t.len() <= 8192);
            tok.and_then(|t| jwt::claims(secret, t, rules).ok())
        }
    }
}

async fn auth(State(app): State<Arc<App>>, mut req: Request, next: Next) -> Response {
    match authenticate(&app, req.headers()) {
        Some(sub) => {
            req.extensions_mut().insert(Principal(sub));
            next.run(req).await
        }
        None => {
            let key = req
                .extensions()
                .get::<Client>()
                .map(|c| c.key.clone())
                .unwrap_or_default();
            match app.auth_rl.hit(&key) {
                Err(s) => ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    "too many failed attempts",
                )
                .retry(s)
                .into_response(),
                Ok(()) => ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "unauthorized",
                    "a valid bearer token is required",
                )
                .into_response(),
            }
        }
    }
}

pub fn router(app: Arc<App>) -> Router {
    let post_limit = (app.r.s.max_batch_mb << 20) as usize + (1 << 20);
    let api = Router::new()
        .route("/formats", get(formats))
        // the limit layer rejects a too-large Content-Length before the handler runs, and counts a streamed body
        .route(
            "/jobs",
            post(create_job)
                .layer::<_, std::convert::Infallible>(DefaultBodyLimit::disable())
                .layer::<_, std::convert::Infallible>(RequestBodyLimitLayer::new(post_limit)),
        )
        .route("/jobs/{id}", get(job_status).delete(delete_job))
        .route("/jobs/{id}/files/{n}/{output}", get(download))
        .route("/jobs/{id}/zip", get(zip))
        .layer(middleware::from_fn_with_state(app.clone(), auth));
    Router::new()
        .nest("/api/v1", api)
        .route("/healthz", get(healthz))
        .route(
            "/",
            get(|| async { page("text/html; charset=utf-8", include_str!("../ui/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                page(
                    "text/javascript; charset=utf-8",
                    include_str!("../ui/app.js"),
                )
            }),
        )
        .route(
            "/app.css",
            get(|| async { page("text/css; charset=utf-8", include_str!("../ui/app.css")) }),
        )
        .route(
            "/i18n.json",
            get(|| async { page("application/json", include_str!("../ui/i18n.json")) }),
        )
        .route(
            "/icon.png",
            get(|| async { asset("image/png", include_bytes!("../ui/icon.png")) }),
        )
        // the one Noto Sans of the repo (assets/fonts/fallback): it has U+02BB, U+02BC and Cyrillic, which system fonts may lack
        .route(
            "/NotoSans-Regular-uz.ttf",
            get(|| async {
                asset(
                    "font/ttf",
                    include_bytes!("../../../assets/fonts/fallback/NotoSans-Regular-uz.ttf"),
                )
            }),
        )
        .fallback(|| async { ApiError::not_found() })
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .layer(middleware::from_fn_with_state(app.clone(), headers))
        .with_state(app)
}

fn page(ct: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, ct)], body).into_response()
}

fn asset(ct: &'static str, body: &'static [u8]) -> Response {
    ([(header::CONTENT_TYPE, ct)], body).into_response()
}

async fn formats(State(app): State<Arc<App>>) -> Json<Value> {
    let s = &app.r.s;
    let mut v = convert::formats::formats_json();
    v["limits"] = json!({
        "maxFileMb": s.max_file_mb, "maxSheetFileMb": s.max_xlsx_mb, "maxBatchFiles": s.max_batch_files, "maxBatchMb": s.max_batch_mb,
        "maxPagesIn": app.cfg.caps.max_pages_in, "maxPngOut": app.cfg.caps.max_png_out, "retentionSeconds": s.retention_s,
        "activeJobsPerUser": s.per_user_active,
    });
    Json(v)
}

async fn healthz(State(app): State<Arc<App>>, h: axum::http::HeaderMap) -> Json<Value> {
    // a server on a network tells strangers only that it is up: no tool list, no sidecar state, no queue depth (WP7 review m4)
    if authenticate(&app, &h).is_none() {
        return Json(json!({"ok": true}));
    }
    let tools: serde_json::Map<String, Value> = plan::ToolId::ALL
        .iter()
        .map(|t| (t.name().to_string(), json!(app.cfg.resolve(*t).is_ok())))
        .collect();
    let sidecar = match app.cfg.pdf2docx_url.clone() {
        None => Value::Null,
        Some(url) => {
            let cached = app
                .health
                .lock()
                .unwrap()
                .filter(|(t, _)| t.elapsed() < Duration::from_secs(10))
                .map(|x| x.1);
            let ok = match cached {
                Some(ok) => ok,
                None => {
                    // `shortcut: a blocking probe on the blocking pool; cached 10 s so an open endpoint cannot be used to hammer the sidecar`
                    let ok = tokio::task::spawn_blocking(move || {
                        let agent: ureq::Agent = ureq::Agent::config_builder()
                            .proxy(None)
                            .timeout_global(Some(Duration::from_secs(2)))
                            .build()
                            .into();
                        agent
                            .get(format!("{}/healthz", url.trim_end_matches('/')))
                            .call()
                            .is_ok()
                    })
                    .await
                    .unwrap_or(false);
                    *app.health.lock().unwrap() = Some((std::time::Instant::now(), ok));
                    ok
                }
            };
            json!(ok)
        }
    };
    let all = tools.values().all(|v| v == &json!(true));
    Json(
        json!({"ok": all, "tools": tools, "sidecars": {"pdf2docx": sidecar}, "queue": {"active": app.queue_depth(), "max": app.r.s.queue_max}}),
    )
}

// ---- POST /jobs ---------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptionsIn {
    to: String,
    translit: Option<String>,
    alphabet: Option<String>,
    apostrophe: Option<String>,
    sheet: Option<String>,
    pages: Option<String>,
    dpi: Option<u32>,
    pdf_engine: Option<String>,
}

fn parse_options(o: OptionsIn) -> Result<(Target, Options), String> {
    let target = Target::parse(&o.to).ok_or(format!("unknown target '{}'", o.to))?;
    let mut opts = Options::default();
    if let Some(t) = &o.translit {
        let dir = match t.as_str() {
            "cyrl" => TranslitDir::Cyrl,
            "latn" => TranslitDir::Latn,
            "auto" => TranslitDir::Auto,
            _ => return Err(format!("translit takes cyrl, latn or auto, got '{t}'")),
        };
        let alphabet = match o.alphabet.as_deref().unwrap_or("1995") {
            "1995" => "1995",
            "2026" => "2026",
            "auto" => "auto",
            x => return Err(format!("alphabet takes 1995, 2026 or auto, got '{x}'")),
        };
        let apostrophe = match o.apostrophe.as_deref().unwrap_or("official") {
            "official" => "official",
            "ascii" => "ascii",
            x => return Err(format!("apostrophe takes official or ascii, got '{x}'")),
        };
        opts.translit = Some(Translit {
            dir,
            alphabet,
            apostrophe,
        });
    } else if o.alphabet.is_some() || o.apostrophe.is_some() {
        return Err("alphabet and apostrophe need translit".into());
    }
    opts.sheet = match o.sheet.as_deref() {
        None | Some("active") => SheetSel::Active,
        Some("all") => SheetSel::All,
        Some(n) => SheetSel::Name(n.into()),
    };
    if let Some(p) = &o.pages {
        opts.pages = Some(PageSpec::parse(p)?);
    }
    if let Some(d) = o.dpi {
        if !(72..=600).contains(&d) {
            return Err("dpi must be between 72 and 600".into());
        }
        opts.dpi = d;
    }
    opts.pdf_engine = match o.pdf_engine.as_deref() {
        None | Some("native") => PdfEngine::Native,
        Some("sidecar") => PdfEngine::Sidecar,
        Some(x) => return Err(format!("pdfEngine takes sidecar or native, got '{x}'")),
    };
    Ok((target, opts))
}

struct Upload {
    name: String,
    size: u64,
}

fn multipart_error(e: axum::extract::multipart::MultipartError) -> ApiError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "the upload is over the request limit",
        )
    } else {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_multipart",
            "the multipart body is malformed",
        )
    }
}

/// Fields and files, streamed to the job store (encrypted in server mode) with a byte counter: nothing is buffered whole.
async fn read_upload(
    app: &App,
    job: &crate::app::Job,
    mut mp: Multipart,
) -> Res<(Vec<Upload>, Option<String>)> {
    let s = &app.r.s;
    let (mut files, mut options, mut total) = (Vec::<Upload>::new(), None, 0u64);
    while let Some(mut field) = mp.next_field().await.map_err(multipart_error)? {
        match field.name() {
            Some("options") => {
                // read with a counter: a part is never held whole before its size is known (WP7 review M7)
                let too_big = || {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "bad_options",
                        "options must be one JSON object under 64 KiB",
                    )
                };
                let mut buf = Vec::new();
                while let Some(b) = field.chunk().await.map_err(multipart_error)? {
                    if buf.len() + b.len() > 64 * 1024 {
                        return Err(too_big());
                    }
                    buf.extend_from_slice(&b);
                }
                let t = String::from_utf8(buf).map_err(|_| too_big())?;
                if options.replace(t).is_some() {
                    return Err(too_big());
                }
            }
            Some("files" | "files[]") => {
                if files.len() >= s.max_batch_files {
                    return Err(ApiError::new(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "too_many_files",
                        format!("at most {} files per job", s.max_batch_files),
                    ));
                }
                let raw = field
                    .file_name()
                    .ok_or_else(|| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "bad_multipart",
                            "a file part needs a file name",
                        )
                    })?
                    .to_string();
                let name = safe_name(&raw, "upload");
                let ext = std::path::Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let cap = if matches!(
                    ext.as_str(),
                    "xlsx" | "xlsm" | "xls" | "xlsb" | "ods" | "csv" | "tsv"
                ) {
                    s.max_xlsx_mb
                } else {
                    s.max_file_mb
                } << 20;
                let n = files.len();
                let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(8);
                let (path, codec, role) = (job.in_path(n), job.codec.clone(), Job::in_role(n));
                let writer = tokio::task::spawn_blocking(move || -> std::io::Result<u64> {
                    let mut w = codec.writer(std::fs::File::create_new(path)?, &role)?;
                    while let Some(b) = rx.blocking_recv() {
                        w.write_all(&b)?;
                    }
                    w.finish()
                });
                let mut size = 0u64;
                let fed = loop {
                    match field.chunk().await {
                        Ok(Some(b)) => {
                            size += b.len() as u64;
                            total += b.len() as u64;
                            if size > cap {
                                break Err(ApiError::new(
                                    StatusCode::PAYLOAD_TOO_LARGE,
                                    "file_too_large",
                                    format!("a file is over the {} MB limit", cap >> 20),
                                ));
                            }
                            if total > s.max_batch_mb << 20 {
                                break Err(ApiError::new(
                                    StatusCode::PAYLOAD_TOO_LARGE,
                                    "batch_too_large",
                                    format!("the batch is over the {} MB limit", s.max_batch_mb),
                                ));
                            }
                            if tx.send(b).await.is_err() {
                                break Err(ApiError::internal("the store writer stopped".into()));
                            }
                        }
                        Ok(None) => break Ok(()),
                        Err(e) => break Err(multipart_error(e)),
                    }
                };
                drop(tx);
                let stored = writer
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                fed?;
                stored.map_err(|e| ApiError::internal(format!("cannot store an upload: {e}")))?;
                files.push(Upload { name, size });
            }
            _ => {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "unknown_field",
                    "only the fields files and options are accepted",
                ));
            }
        }
    }
    Ok((files, options))
}

struct Checked {
    kind: kind::Kind,
    est_mb: u64,
    /// the zip scan of this very content, handed to the job so that it is not inflated a second time
    verdict: Arc<convert::scan::Verdict>,
}

fn exit_of(e: &PlanError) -> (u8, String) {
    match e {
        PlanError::Unsupported(m) => (Exit::Unsupported.code(), m.clone()),
        PlanError::NeedsSidecar(m) => (Exit::Missing.code(), m.clone()),
        PlanError::Usage(m) => (Exit::Usage.code(), m.clone()),
        PlanError::Rejected(m) => (Exit::Rejected.code(), m.clone()),
    }
}

/// The per-file answer in the 202: detect the kind, scan the zip directory, plan. A file that fails here is rejected on its own
/// and the rest of the batch runs. In server mode this is the one extra plaintext copy (see docs/phase3/api.md).
fn precheck(
    app: &App,
    job: &crate::app::Job,
    n: usize,
    up: &Upload,
    target: Target,
    opts: &Options,
    budget: &mut u64,
) -> Result<Checked, (Option<kind::Kind>, u8, String)> {
    let dir = app
        .r
        .work_dir
        .join(format!("stage-{}-pre-{}-{n}", std::process::id(), job.id));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _guard = Cleanup(dir.clone());
    let io = |e: std::io::Error| {
        (
            None,
            Exit::ToolError.code(),
            format!("cannot read the upload: {e}"),
        )
    };
    std::fs::create_dir_all(&dir).map_err(io)?;
    let ext = std::path::Path::new(&up.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| safe_name(e, ""))
        .filter(|e| !e.is_empty());
    let file = dir.join(ext.map_or("input".to_string(), |e| format!("input.{e}")));
    let src = StoredInput {
        name: up.name.clone(),
        path: job.in_path(n),
        codec: job.codec.clone(),
        role: Job::in_role(n),
    };
    std::io::copy(
        &mut src.open().map_err(io)?,
        &mut std::fs::File::create(&file).map_err(io)?,
    )
    .map_err(io)?;
    let det = kind::detect(&file).map_err(|e| match e {
        DetectError::Io(e) => (None, Exit::Usage.code(), format!("cannot read the file: {e}")),
        DetectError::Empty => (None, Exit::Rejected.code(), convert::job::empty_file().msg),
        DetectError::Unknown => (None, Exit::Unsupported.code(), "unsupported input: the content is not a document, spreadsheet, presentation or PDF this converter reads".into()),
        DetectError::NotUtf8 => (None, Exit::Unsupported.code(), "unsupported input: the text is not UTF-8 (cp1251 or UTF-16?); re-save it as UTF-8".into()),
    })?;
    let k = det.kind;
    if det.password_protected {
        return Err((
            Some(k),
            Exit::Rejected.code(),
            "the file is password-protected; passwords are not supported".into(),
        ));
    }
    let sha256 = convert::scan::sha256_file(&file).map_err(io)?;
    let scan =
        convert::scan::scan_with(&file, budget).map_err(|f| (Some(k), f.exit.code(), f.msg))?;
    plan::plan(k, target, opts).map_err(|e| {
        let (c, m) = exit_of(&e);
        (Some(k), c, m)
    })?;
    Ok(Checked {
        kind: k,
        est_mb: estimate_mb(k, up.size),
        verdict: Arc::new(convert::scan::Verdict { sha256, scan }),
    })
}

/// A job that has been admitted but not handed to its supervisor yet. If the request is dropped before that (the client closed the
/// connection while the upload check waited for a slot, or the handler returned an error), this cancels and removes the job, so
/// nothing keeps counting against the limits (WP7 review B1).
struct Admitted {
    app: Arc<App>,
    job: Arc<Job>,
    armed: std::sync::atomic::AtomicBool,
}

impl Admitted {
    fn disarm(&self) {
        self.armed
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for Admitted {
    fn drop(&mut self) {
        if self.armed.load(std::sync::atomic::Ordering::Relaxed) {
            self.app.delete(&self.job);
            self.app.finish(&self.job);
        }
    }
}

async fn create_job(
    State(app): State<Arc<App>>,
    Extension(p): Extension<Principal>,
    Extension(c): Extension<Client>,
    mp: Multipart,
) -> Res {
    for (l, key) in [(&app.user_rl, &p.0), (&app.ip_rl, &c.key)] {
        l.hit(key).map_err(|s| {
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many requests; slow down",
            )
            .retry(s)
        })?;
    }
    // admission before the body is read: a full queue costs no disk
    let job = app.admit(&p.0)?;
    let admitted = Admitted {
        app: app.clone(),
        job: job.clone(),
        armed: std::sync::atomic::AtomicBool::new(true),
    };
    create_inner(&app, &job, &c.ip, mp, &admitted).await
}

async fn create_inner(
    app: &Arc<App>,
    job: &Arc<Job>,
    client: &str,
    mp: Multipart,
    admitted: &Admitted,
) -> Res {
    let (uploads, options) = tokio::time::timeout(
        Duration::from_secs(app.r.s.upload_timeout_s),
        read_upload(app, job, mp),
    )
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::REQUEST_TIMEOUT,
            "upload_timeout",
            "the upload took too long",
        )
    })??;
    // the reservation made at admission gives way to what was really stored
    let uploaded: u64 = uploads.iter().map(|u| u.size).sum();
    job.bytes
        .store(uploaded, std::sync::atomic::Ordering::Relaxed);
    let bad = |m: String| ApiError::new(StatusCode::BAD_REQUEST, "bad_options", m);
    if uploads.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "no_files",
            "send at least one part named files",
        ));
    }
    let parsed: OptionsIn =
        serde_json::from_str(&options.ok_or_else(|| bad("the options field is required".into()))?)
            .map_err(|e| bad(format!("options: {e}")))?;
    let (target, opts) = parse_options(parsed).map_err(bad)?;

    // the prechecks read and inflate the upload, so they wait for a CPU slot like a conversion does, but not for ever
    let _cpu = tokio::time::timeout(
        Duration::from_secs(app.r.s.slot_wait_s),
        app.cpu.acquire(&job.owner),
    )
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "all conversion slots are busy; try again shortly",
        )
        .retry(10)
    })?;
    // one inflate budget for the whole request: 50 times what was uploaded, between 256 MiB and 2 GiB (WP7 review M1)
    let mut budget = (uploaded.saturating_mul(50)).clamp(256 << 20, 2 << 30);
    let checks = {
        let (app, job, opts) = (app.clone(), job.clone(), opts.clone());
        let names: Vec<(String, u64)> = uploads.iter().map(|u| (u.name.clone(), u.size)).collect();
        tokio::task::spawn_blocking(move || {
            names
                .into_iter()
                .enumerate()
                .map(|(n, (name, size))| {
                    precheck(
                        &app,
                        &job,
                        n,
                        &Upload { name, size },
                        target,
                        &opts,
                        &mut budget,
                    )
                })
                .collect::<Vec<_>>()
        })
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    };
    drop(_cpu);
    if let Some((n, c)) = checks.iter().enumerate().find_map(|(n, c)| {
        c.as_ref()
            .ok()
            .filter(|c| c.est_mb > app.r.s.mem_budget_mb)
            .map(|c| (n, c))
    }) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "too_large_for_server",
            format!(
                "file {n} needs about {} MB of memory, more than this server's budget of {} MB",
                c.est_mb, app.r.s.mem_budget_mb
            ),
        ));
    }

    let mut answer = vec![];
    let mut runnable = vec![];
    let mut rejected = vec![];
    {
        let mut st = job.st.lock().unwrap();
        for (n, (up, chk)) in uploads.iter().zip(&checks).enumerate() {
            let mut f = FileState {
                n,
                name: up.name.clone(),
                kind: None,
                target: target.ext().into(),
                state: "queued",
                step: None,
                report: None,
                outputs: vec![],
                in_bytes: up.size,
            };
            match chk {
                Ok(c) => {
                    f.kind = Some(c.kind.ext().into());
                    answer.push(json!({"n": n, "name": up.name, "kind": c.kind.ext(), "target": target.ext()}));
                    runnable.push((n, c.est_mb, c.verdict.clone()));
                }
                Err((k, code, msg)) => {
                    f.kind = k.map(|k| k.ext().to_string());
                    f.state = "failed";
                    let mut rep =
                        crate::app::convert_failed(&up.name, target, Exit::ToolError, msg);
                    rep.code = *code;
                    rep.kind = f.kind.clone();
                    f.report = Some(app.public_report(&rep, &[]));
                    answer.push(json!({"n": n, "name": up.name, "kind": f.kind, "error": {"code": code, "message": app.redact.scrub(msg)}}));
                    rejected.push(n);
                }
            }
            st.files.push(f);
        }
    }
    // a rejected upload is never converted, so it does not stay on disk until the retention time is over (WP7 review B2)
    for n in rejected {
        job.drop_input(n);
    }
    let (a, j, client) = (app.clone(), job.clone(), client.to_string());
    tokio::spawn(async move {
        let mut set = tokio::task::JoinSet::new();
        for (n, est, verdict) in runnable {
            set.spawn(run_file(
                a.clone(),
                j.clone(),
                n,
                target,
                opts.clone(),
                est,
                client.clone(),
                verdict,
            ));
        }
        while set.join_next().await.is_some() {}
        a.finish(&j);
    });
    admitted.disarm();
    let mut res = (
        StatusCode::ACCEPTED,
        Json(json!({"job": job.id, "files": answer})),
    )
        .into_response();
    res.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("/api/v1/jobs/{}", job.id)).expect("hex id"),
    );
    Ok(res)
}

// ---- status, outputs, zip, delete ------------------------------------------------------------------------------------

fn lookup(app: &App, id: &str, p: &Principal) -> Res<Arc<crate::app::Job>> {
    app.get(id, &p.0).ok_or_else(ApiError::not_found)
}

async fn job_status(
    State(app): State<Arc<App>>,
    Extension(p): Extension<Principal>,
    UrlPath(id): UrlPath<String>,
) -> Res<Json<Value>> {
    let job = lookup(&app, &id, &p)?;
    let st = job.st.lock().unwrap();
    let finished = |f: &FileState| matches!(f.state, "done" | "failed");
    let done = st.files.iter().filter(|f| finished(f)).count();
    let state = if done == st.files.len() && st.finished_at.is_some() {
        "done"
    } else if done > 0 || st.files.iter().any(|f| f.state == "running") {
        "running"
    } else {
        "queued"
    };
    let files: Vec<Value> = st
        .files
        .iter()
        .map(|f| {
            let rep = f.report.as_ref();
            let mut v = json!({
                "n": f.n, "name": f.name, "kind": f.kind, "target": f.target, "state": f.state, "step": f.step,
                "code": rep.map(|r| r["code"].clone()), "message": rep.map(|r| r["message"].clone()),
                "warnings": rep.map(|r| r["warnings"].clone()).unwrap_or(json!([])),
                "outputs": f.outputs.iter().enumerate().map(|(k, o)| json!({"id": k, "name": o.name, "size": o.size})).collect::<Vec<_>>(),
            });
            // only when the XDFU hook found a grif: with `xdfu = "off"` the answer is what it always was
            if let Some(x) = rep.and_then(|r| r.get("xdfu")) {
                v["xdfu"] = x.clone();
            }
            v
        })
        .collect();
    Ok(Json(
        json!({"job": job.id, "state": state, "progress": {"done": done, "total": st.files.len()}, "files": files}),
    ))
}

async fn delete_job(
    State(app): State<Arc<App>>,
    Extension(p): Extension<Principal>,
    UrlPath(id): UrlPath<String>,
) -> Res {
    let job = lookup(&app, &id, &p)?;
    app.delete(&job);
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// RFC 5987 `filename*` plus an ASCII fallback.
fn disposition(name: &str) -> HeaderValue {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut enc = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b) {
            enc.push(b as char);
        } else {
            enc.push_str(&format!("%{b:02X}"));
        }
    }
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{enc}"
    ))
    .expect("ascii header")
}

/// A fixed type per extension; markup types are served as opaque bytes so a downloaded page can never run in our origin.
fn content_type(name: &str) -> &'static str {
    match std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "txt" | "md" | "csv" => "text/plain; charset=utf-8",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Decrypt on the blocking pool and stream; the response is as long as the plaintext.
fn stream(
    codec: crate::crypt::Codec,
    path: PathBuf,
    role: String,
    name: &str,
    size: u64,
) -> Response {
    use zeroize::Zeroize;
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(4);
    tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 64 * 1024];
        let sent = (|| -> std::io::Result<()> {
            let mut r = codec.open(&path, &role)?;
            loop {
                match std::io::Read::read(&mut r, &mut buf)? {
                    0 => return Ok(()),
                    n if tx
                        .blocking_send(Ok(Bytes::copy_from_slice(&buf[..n])))
                        .is_err() =>
                    {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        })();
        buf.zeroize();
        if let Err(e) = sent {
            let _ = tx.blocking_send(Err(e));
        }
    });
    let mut res = Response::new(Body::from_stream(
        tokio_stream::wrappers::ReceiverStream::new(rx),
    ));
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(name)),
    );
    h.insert(header::CONTENT_DISPOSITION, disposition(name));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
    res
}

async fn download(
    State(app): State<Arc<App>>,
    Extension(p): Extension<Principal>,
    UrlPath((id, n, k)): UrlPath<(String, usize, usize)>,
) -> Res {
    let job = lookup(&app, &id, &p)?;
    let (name, size) = {
        let st = job.st.lock().unwrap();
        let o = st
            .files
            .get(n)
            .filter(|f| f.state == "done")
            .and_then(|f| f.outputs.get(k))
            .ok_or_else(ApiError::not_found)?;
        (o.name.clone(), o.size)
    };
    Ok(stream(
        job.codec.clone(),
        job.out_path(n, k),
        Job::out_role(n, k),
        &name,
        size,
    ))
}

/// Every output plus `report.json`, built once into the job directory (encrypted in server mode), then served.
fn build_zip(job: &crate::app::Job) -> Result<(), ApiError> {
    let part = job.dir.join("zip.part");
    let built = build_zip_part(job, &part);
    if built.is_err() {
        // a failed build must not leave the part behind: the next request starts clean
        let _ = std::fs::remove_file(&part);
    }
    built
}

fn build_zip_part(job: &crate::app::Job, part: &std::path::Path) -> Result<(), ApiError> {
    let _one = job.zip_lock.lock().unwrap();
    if job.zip_path().exists() {
        return Ok(());
    }
    // copy what is needed and release the lock: status polls must not wait for the zip
    type Item = (usize, Vec<String>, Option<Value>); // file n, output names, report
    let (total, files): (u64, Vec<Item>) = {
        let st = job.st.lock().unwrap();
        (
            st.files
                .iter()
                .flat_map(|f| &f.outputs)
                .map(|o| o.size)
                .sum(),
            st.files
                .iter()
                .map(|f| {
                    (
                        f.n,
                        f.outputs.iter().map(|o| o.name.clone()).collect(),
                        f.report.clone(),
                    )
                })
                .collect(),
        )
    };
    if total > convert::scan::MAX_TOTAL_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "zip_too_large",
            "the outputs are too large to pack into one zip; download them one by one",
        ));
    }
    let internal = |e: &dyn std::fmt::Display| ApiError::internal(format!("zip: {e}"));
    // a part left by a crashed build is overwritten, not an error
    let w = job
        .codec
        .writer(
            std::fs::File::create(part).map_err(|e| internal(&e))?,
            Job::ZIP_ROLE,
        )
        .map_err(|e| internal(&e))?;
    let mut z = zip::ZipWriter::new_stream(w);
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut names: Vec<String> = vec![];
    let mut reports = vec![];
    for (n, outs, report) in files {
        for (k, name) in outs.iter().enumerate() {
            let taken: Vec<&str> = names.iter().map(String::as_str).collect();
            let entry = crate::app::dedupe(&taken, name);
            z.start_file(entry.clone(), opt).map_err(|e| internal(&e))?;
            std::io::copy(
                &mut job
                    .codec
                    .open(&job.out_path(n, k), &Job::out_role(n, k))
                    .map_err(|e| internal(&e))?,
                &mut z,
            )
            .map_err(|e| internal(&e))?;
            names.push(entry);
        }
        reports.extend(report);
    }
    z.start_file("report.json", opt).map_err(|e| internal(&e))?;
    z.write_all(&serde_json::to_vec_pretty(&reports).expect("json"))
        .map_err(|e| internal(&e))?;
    let sw = z.finish().map_err(|e| internal(&e))?;
    let size = sw.into_inner().finish().map_err(|e| internal(&e))?;
    job.zip_size
        .store(size, std::sync::atomic::Ordering::Relaxed);
    std::fs::rename(part, job.zip_path()).map_err(|e| internal(&e))?;
    job.add_bytes(size);
    Ok(())
}

async fn zip(
    State(app): State<Arc<App>>,
    Extension(p): Extension<Principal>,
    UrlPath(id): UrlPath<String>,
) -> Res {
    let job = lookup(&app, &id, &p)?;
    if !job.is_finished() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "not_done",
            "the job has not finished yet",
        ));
    }
    if !job.zip_path().exists() {
        // deflating up to a gigabyte is conversion-sized work: it takes a CPU slot like one (WP7 review M1)
        let slot = tokio::time::timeout(
            Duration::from_secs(app.r.s.slot_wait_s),
            app.cpu.acquire(&job.owner),
        )
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "busy",
                "all conversion slots are busy; try again shortly",
            )
            .retry(10)
        })?;
        let j = job.clone();
        tokio::task::spawn_blocking(move || {
            let _slot = slot;
            build_zip(&j)
        })
        .await
        .map_err(|e| ApiError::internal(e.to_string()))??;
    }
    Ok(stream(
        job.codec.clone(),
        job.zip_path(),
        Job::ZIP_ROLE.to_string(),
        "converted.zip",
        job.zip_size.load(std::sync::atomic::Ordering::Relaxed),
    ))
}
