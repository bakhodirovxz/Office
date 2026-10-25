//! The docserver mount (PLAN phase 5, WP2): nothing without the `[docserver]` section (S20), and the header rules of S13 with it:
//! `frame-ancestors` lists the configured origins on the wrapper page and the editors only, CORP is `cross-origin` on `api.js` only,
//! and every other route keeps the strict baseline.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use convert::exec::Config as ConvertConfig;
use convert_api::app::App;
use convert_api::config::Settings;
use docserver::Docserver;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use suite_serve::{Layout, router_ds};
use tower::ServiceExt;

const STRICT: &str = "default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";
const FRAMERS: &str = "'self' https://app.test http://localhost:3000";

struct H {
    dir: PathBuf,
    router: Router,
    app: Arc<App>,
}
impl Drop for H {
    fn drop(&mut self) {
        self.app.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn harness(name: &str, with_ds: bool) -> H {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("suite-serve-ds-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let docs = dir.join("dist/docs-web");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("index.html"),
        "<html><head><script type=\"module\">x()</script></head></html>",
    )
    .unwrap();
    for (f, b) in [("server", b's'), ("p", b'p')] {
        std::fs::write(dir.join(f), [b; 40]).unwrap();
    }
    let layout = Layout {
        shell: repo.join("web/shell"),
        dist: dir.join("dist"),
        brand: repo.join("brand/brand.toml"),
        local_key: None,
    };
    let ds = with_ds.then(|| {
        let p = |f: &str| dir.join(f).to_string_lossy().replace('\\', "/");
        let t: toml::Table = format!(
            r#"server_secret_file = "{}"
data_dir = "{}"
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = ["http://api:8000"]
frame_origins = ["https://app.test", "http://localhost:3000"]
"#,
            p("server"),
            p("data"),
            p("p")
        )
        .parse()
        .unwrap();
        Arc::new(Docserver::from_table(t, None).unwrap())
    });
    let s = Settings {
        work_dir: Some(dir.join("work")),
        store_dir: Some(dir.join("store")),
        ..Settings::default()
    };
    let app = App::new(s.resolve().unwrap(), ConvertConfig::from_env(), false).unwrap();
    let cfg = layout.shell_config().unwrap();
    H {
        router: router_ds(app.clone(), &layout, cfg, ds),
        app,
        dir,
    }
}

async fn send(h: &H, method: &str, uri: &str) -> (StatusCode, HeaderMap) {
    send_with(h, method, uri, &[]).await
}

async fn send_with(h: &H, method: &str, uri: &str, extra: &[(&str, &str)]) -> (StatusCode, HeaderMap) {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:8787");
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let mut r = b.body(Body::empty()).unwrap();
    r.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5555".parse::<SocketAddr>().unwrap()));
    let res = h.router.clone().oneshot(r).await.unwrap();
    (res.status(), res.headers().clone())
}

fn hv<'a>(h: &'a HeaderMap, k: &str) -> &'a str {
    h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("")
}

const DS_ROUTES: [(&str, &str); 9] = [
    ("GET", "/ds/api.js"),
    ("GET", "/web-apps/apps/api/documents/api.js"),
    ("GET", "/ds/editor"),
    ("POST", "/ds/session"),
    ("POST", "/ds/session/x/renew"),
    ("GET", "/ds/session/x/document"),
    ("POST", "/ds/session/x/save"),
    ("POST", "/ds/session/x/autosave"),
    ("POST", "/ds/session/x/close"),
];

#[tokio::test]
async fn no_providers_no_docserver_routes() {
    let h = harness("off", false);
    for (m, uri) in DS_ROUTES.iter().chain(&[("GET", "/ds/files/x/1")]) {
        let (st, hd) = send(&h, m, uri).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(hv(&hd, "content-security-policy"), STRICT, "{uri}");
    }
    // today's editor rule is unchanged
    let (st, hd) = send(&h, "GET", "/docs/").await;
    assert_eq!(st, StatusCode::OK);
    assert!(hv(&hd, "content-security-policy").contains("frame-ancestors 'self';"));
    assert_eq!(hv(&hd, "x-frame-options"), "SAMEORIGIN");
}

#[tokio::test]
async fn framing_and_corp_rules_with_the_docserver_on() {
    let h = harness("on", true);
    for (m, uri) in DS_ROUTES {
        let (st, hd) = send(&h, m, uri).await;
        // api.js and the wrapper page are stubs until WP5; the session routes check their token (WP3)
        let want = match uri {
            "/ds/session" => StatusCode::BAD_REQUEST,
            u if u.starts_with("/ds/session/") => StatusCode::UNAUTHORIZED,
            _ => StatusCode::NOT_IMPLEMENTED,
        };
        assert_eq!(st, want, "{uri}");
        // an error is never cached, api.js included
        assert_eq!(hv(&hd, "cache-control"), "no-store", "{uri}");
        // always, on every route
        assert_eq!(hv(&hd, "x-content-type-options"), "nosniff", "{uri}");
        assert_eq!(
            hv(&hd, "cross-origin-opener-policy"),
            "same-origin",
            "{uri}"
        );
        assert!(hd.get("access-control-allow-origin").is_none(), "{uri}");
        let (corp, xfo, csp) = (
            hv(&hd, "cross-origin-resource-policy"),
            hd.get("x-frame-options"),
            hv(&hd, "content-security-policy"),
        );
        if uri.ends_with("api.js") {
            assert_eq!(
                (corp, xfo.is_some(), csp),
                ("cross-origin", true, STRICT),
                "{uri}"
            );
        } else if uri == "/ds/editor" {
            let want = STRICT.replace(
                "frame-ancestors 'none'",
                &format!("frame-ancestors {FRAMERS}"),
            );
            assert_eq!(
                (corp, xfo.is_none(), csp),
                ("same-origin", true, want.as_str()),
                "{uri}"
            );
        } else {
            assert_eq!(
                (corp, xfo.map(|v| v.to_str().unwrap()), csp),
                ("same-origin", Some("DENY"), STRICT),
                "{uri}"
            );
        }
    }
    // the editor frames list the same origins and send no X-Frame-Options
    let (st, hd) = send(&h, "GET", "/docs/").await;
    assert_eq!(st, StatusCode::OK);
    assert!(hv(&hd, "content-security-policy").contains(&format!("frame-ancestors {FRAMERS};")));
    assert!(hd.get("x-frame-options").is_none());
    assert_eq!(hv(&hd, "cross-origin-resource-policy"), "same-origin");
    // the shell, the converter and the health check keep the strict baseline
    for uri in ["/", "/convert/", "/healthz", "/api/v1/formats", "/nope"] {
        let (_, hd) = send(&h, "GET", uri).await;
        assert_eq!(hv(&hd, "content-security-policy"), STRICT, "{uri}");
        assert_eq!(hv(&hd, "x-frame-options"), "DENY", "{uri}");
        assert_eq!(
            hv(&hd, "cross-origin-resource-policy"),
            "same-origin",
            "{uri}"
        );
    }
}

#[tokio::test]
async fn api_js_is_a_plain_script_load() {
    // WP5 note: `<script src>` sends no Origin header, so api.js loads; a request carrying one (fetch, a module script, a form)
    // from another site is refused by the converter's origin guard. api.js must stay a classic script, never `type=module` or CORS.
    let h = harness("origin", true);
    let (st, _) = send(&h, "GET", "/ds/api.js").await;
    assert_eq!(st, StatusCode::NOT_IMPLEMENTED, "no Origin reaches the route");
    let (st, _) = send_with(&h, "GET", "/ds/api.js", &[("origin", "https://app.test")]).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "Origin https://app.test is not our origin");
}
