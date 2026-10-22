//! `suite-serve`: one origin for the home shell (`/`), the converter (`/convert/`, `/api/v1`) and the four editors (`/docs/`,
//! `/sheets/`, `/slides/`, `/pdf/`), PLAN phase 4 sections 3.1-3.3.
//!
//! Security headers: every response gets the converter's baseline (`convert_api::routes::baseline_headers`), including the strict
//! CSP `script-src 'self'`. An editor route overrides it (F8): `'wasm-unsafe-eval'` plus the sha256 of each inline script and style
//! of that dist's `index.html`, read when the server starts, and `frame-ancestors 'self'` so only the shell may frame it. The shell
//! and the converter keep the strict CSP.
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use base64::Engine as _;
use convert_api::app::App;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

/// (route, trunk dist dir under `--dist`, key in `brand.toml [apps.*]`).
pub const EDITORS: [(&str, &str, &str); 4] = [
    ("docs", "docs-web", "docs"),
    ("sheets", "sheets-web", "sheets"),
    ("slides", "slides-web", "slides"),
    ("pdf", "pdf-web", "pdf"),
];

pub struct Layout {
    pub shell: PathBuf,
    pub dist: PathBuf,
    pub brand: PathBuf,
    /// Desktop mode (review M1): a per-launch secret, 64 hex digits. `GET /_k/<key>` sets it as an `HttpOnly` cookie and every
    /// other request needs that cookie, so another local user who finds the port gets 403. `None` (the web host) changes nothing.
    pub local_key: Option<String>,
}

impl Layout {
    /// The JSON the shell reads (`/shell-config.json`): the product name, the accent colours and each app's route, names and
    /// bridge prefix, all from `brand.toml`. No name is written in the shell itself.
    pub fn shell_config(&self) -> Result<String, String> {
        let text = std::fs::read_to_string(&self.brand)
            .map_err(|e| format!("{}: {e}", self.brand.display()))?;
        brand_json(&text, self.local_key.is_some())
    }
}

fn brand_json(text: &str, local: bool) -> Result<String, String> {
    let t: toml::Table = text.parse().map_err(|e| format!("brand.toml: {e}"))?;
    let need = |p: &[&str]| -> Result<serde_json::Value, String> {
        let mut v = t.get(p[0]);
        for k in &p[1..] {
            v = v.and_then(|v| v.get(k));
        }
        v.and_then(|v| serde_json::to_value(v).ok())
            .ok_or_else(|| format!("brand.toml: {} is missing", p.join(".")))
    };
    let mut apps = vec![];
    for (route, _, key) in EDITORS {
        apps.push(serde_json::json!({
            "id": key, "route": format!("/{route}/"),
            "prefix": need(&["apps", key, "upstream"])?, "names": need(&["apps", key, "names"])?,
        }));
    }
    Ok(serde_json::json!({
        "product": need(&["product", "name"])?,
        // the desktop host's server: the shell keeps no copies of documents in the browser it runs in (review M2)
        "local": local,
        "accent": {"light": need(&["colors", "accent_light"])?, "dark": need(&["colors", "accent_dark"])?},
        "apps": apps,
        "convert": {"route": "/convert/", "names": need(&["apps", "convert", "names"])?},
    })
    .to_string())
}

/// The contents of every inline `<tag>` (no `src=`) in `html`.
fn inline_blocks<'a>(html: &'a str, tag: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = vec![];
    let mut rest = html;
    while let Some(i) = rest.find(&open) {
        rest = &rest[i + open.len()..];
        let Some(gt) = rest.find('>') else { break };
        let (attrs, body) = (&rest[..gt], &rest[gt + 1..]);
        // the tag name must end here: `<scripts>` is not a script
        let named = attrs.is_empty() || attrs.starts_with(|c: char| c.is_whitespace());
        let Some(end) = body.find(&close) else { break };
        if named && !attrs.contains("src=") {
            out.push(&body[..end]);
        }
        rest = &body[end..];
    }
    out
}

/// The CSP of an editor page (F8).
pub fn editor_csp(index_html: &str) -> String {
    let hashes = |tag| {
        inline_blocks(index_html, tag)
            .iter()
            .map(|b| {
                format!(
                    " 'sha256-{}'",
                    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(b.as_bytes()))
                )
            })
            .collect::<String>()
    };
    format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'{}; style-src 'self'{}; img-src 'self' data: blob:; \
         object-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'self'",
        hashes("script"),
        hashes("style")
    )
}

struct Editor {
    bare: String,
    slash: String,
    csp: HeaderValue,
}

/// An editor route: the dist, precompressed `.br` files when present, its own CSP, and the cache rule: `index.html` is revalidated,
/// every other file of a trunk dist has a content hash in its name and never changes.
fn editor(route: &str, dir: &Path) -> Option<Router<Arc<App>>> {
    let html = std::fs::read_to_string(dir.join("index.html")).ok()?;
    let ed = Arc::new(Editor {
        bare: format!("/{route}"),
        slash: format!("/{route}/"),
        csp: HeaderValue::from_str(&editor_csp(&html)).ok()?,
    });
    Some(
        Router::new()
            .nest_service(&format!("/{route}"), ServeDir::new(dir).precompressed_br())
            .layer(middleware::from_fn_with_state(ed, editor_headers)),
    )
}

/// `docs-web-1a2b3c4d5e6f7a8b_bg.wasm`: `-` and 15 or 16 hex digits (trunk's hash), then `.` or `_`.
fn hashed(file: &str) -> bool {
    file.split('-').skip(1).any(|p| {
        let n = p.bytes().take_while(u8::is_ascii_hexdigit).count();
        (15..=16).contains(&n) && p[n..].starts_with(['.', '_'])
    })
}

async fn editor_headers(State(ed): State<Arc<Editor>>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if path == ed.bare {
        // the page's relative URLs need the slash
        let q = req
            .uri()
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        return Redirect::permanent(&format!("{}{q}", ed.slash)).into_response();
    }
    // only a file with a trunk content hash in its name is immutable; every other name that may reach `index.html` (`/docs//`,
    // `/docs/INDEX.HTML`, an NTFS stream) is revalidated, 304 included (review m3)
    let immutable = hashed(path.rsplit('/').next().unwrap_or(""));
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::CONTENT_SECURITY_POLICY, ed.csp.clone());
    h.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    if matches!(res.status(), StatusCode::OK | StatusCode::NOT_MODIFIED) {
        let cc = if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(cc));
    }
    res
}

async fn no_cache(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}

/// Equal in time that does not depend on where the first difference is (the length is not secret).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// The desktop gate (M1). It sits outside the Host/Origin guard, so its answers carry only the headers set here.
async fn local_gate(State(key): State<Arc<str>>, req: Request, next: Next) -> Response {
    let plain = [
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ];
    if let Some(k) = req.uri().path().strip_prefix("/_k/") {
        if req.method() == Method::GET && same(k, &key) {
            let cookie = format!("k={key}; Path=/; HttpOnly; SameSite=Strict");
            return (
                StatusCode::SEE_OTHER,
                plain,
                [
                    (header::LOCATION, "/".to_string()),
                    (header::SET_COOKIE, cookie),
                ],
            )
                .into_response();
        }
        return (StatusCode::FORBIDDEN, plain).into_response();
    }
    let has_key = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().strip_prefix("k="))
        .any(|v| same(v, &key));
    if has_key {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, plain).into_response()
    }
}

/// The converter API router plus the shell and the editors. An editor whose dist is not built is not mounted (404).
pub fn router(app: Arc<App>, layout: &Layout, shell_config: String) -> Router {
    let mut extra: Router<Arc<App>> = Router::new();
    for (route, dist, _) in EDITORS {
        let dir = layout.dist.join(dist);
        match editor(route, &dir) {
            Some(r) => extra = extra.merge(r),
            None => eprintln!(
                "suite-serve: /{route}/ is not served: {}/index.html is missing",
                dir.display()
            ),
        }
    }
    let shell = Router::new()
        .route_service(
            "/",
            ServeFile::new(layout.shell.join("index.html")).precompressed_br(),
        )
        .nest_service("/shell", ServeDir::new(&layout.shell).precompressed_br())
        .route(
            "/shell-config.json",
            get(move || {
                let c = shell_config.clone();
                async move { ([(header::CONTENT_TYPE, "application/json")], c) }
            }),
        )
        .route(
            "/convert",
            get(|| async { Redirect::permanent("/convert/") }),
        )
        .route(
            "/convert/",
            get(|| async { convert_api::routes::converter_page() }),
        )
        .layer(middleware::from_fn(no_cache));
    let router = convert_api::routes::router_with(app, extra.merge(shell));
    match &layout.local_key {
        Some(k) => {
            assert!(
                k.len() >= 32 && k.bytes().all(|c| c.is_ascii_alphanumeric()),
                "local_key must be 32 or more letters and digits"
            );
            router.layer(middleware::from_fn_with_state(
                Arc::<str>::from(k.as_str()),
                local_gate,
            ))
        }
        None => router,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_trunk_hashed_names_are_immutable() {
        for f in [
            "docs-web-1a2b3c4d5e6f7a8b_bg.wasm",
            "docs-web-1a2b3c4d5e6f7a8.js",
            "x-ABCDEF0123456789.css",
        ] {
            assert!(hashed(f), "{f}");
        }
        for f in [
            "index.html",
            "",
            "INDEX.HTML",
            "index.html::$DATA",
            "docs-web-abc_bg.wasm",
            "docs-web-1a2b3c4d5e6f7a8b9c.js",
            "a1b2c3d4e5f6a7b8.js",
        ] {
            assert!(!hashed(f), "{f}");
        }
    }

    #[test]
    fn hashes_inline_script_and_style_only() {
        let html = r#"<script src="a.js"></script><script type="module">x()</script><style>b{}</style><link rel="x">"#;
        assert_eq!(inline_blocks(html, "script"), ["x()"]);
        assert_eq!(inline_blocks(html, "style"), ["b{}"]);
        let csp = editor_csp(html);
        assert!(
            csp.contains("'wasm-unsafe-eval' 'sha256-") && csp.contains("frame-ancestors 'self'")
        );
    }
}
