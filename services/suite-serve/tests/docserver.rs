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
    harness_with(name, with_ds, "")
}

/// `extra`: lines of the `[docserver]` table (before the provider).
fn harness_with(name: &str, with_ds: bool, extra: &str) -> H {
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
{extra}
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

async fn send_with(
    h: &H,
    method: &str,
    uri: &str,
    extra: &[(&str, &str)],
) -> (StatusCode, HeaderMap) {
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
        // the session routes check their token (WP3)
        let want = match uri {
            "/ds/session" => StatusCode::BAD_REQUEST,
            u if u.starts_with("/ds/session/") => StatusCode::UNAUTHORIZED,
            _ => StatusCode::OK,
        };
        assert_eq!(st, want, "{uri}");
        // only a good answer from api.js is cached; an error is never cached
        let cached = uri.ends_with("api.js");
        assert_eq!(
            hv(&hd, "cache-control"),
            if cached {
                "public, max-age=300"
            } else {
                "no-store"
            },
            "{uri}"
        );
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

/// Every route's answer, headers and body, in one mode.
async fn snapshot(h: &H) -> Vec<(String, StatusCode, Vec<(String, String)>, Vec<u8>)> {
    let mut out = vec![];
    for (m, uri) in DS_ROUTES.iter().chain(&[
        ("GET", "/ds/editor.js"),
        ("GET", "/docs/"),
        ("GET", "/"),
        ("GET", "/healthz"),
    ]) {
        let mut r = Request::builder()
            .method(*m)
            .uri(*uri)
            .header("host", "127.0.0.1:8787")
            .body(Body::empty())
            .unwrap();
        r.extensions_mut()
            .insert(ConnectInfo("127.0.0.1:5555".parse::<SocketAddr>().unwrap()));
        let res = h.router.clone().oneshot(r).await.unwrap();
        let mut hd: Vec<(String, String)> = res
            .headers()
            .iter()
            // the only values that change between two runs
            .filter(|(k, _)| {
                !["date", "last-modified", "etag", "x-request-id"].contains(&k.as_str())
            })
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect();
        hd.sort();
        let st = res.status();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        out.push((uri.to_string(), st, hd, body));
    }
    out
}

/// docs/phase5/dropin.md D3b. Off (the default, or written out): every answer is the same, byte for byte, as without the key.
#[tokio::test]
async fn allow_opaque_origin_off_changes_nothing() {
    let a = snapshot(&harness("opq-default", true)).await;
    let b = snapshot(&harness_with(
        "opq-false",
        true,
        "allow_opaque_origin = false",
    ))
    .await;
    assert_eq!(a, b);
}

/// On: the wrapper page and the editors send no `frame-ancestors` and no `X-Frame-Options`, `editor.js` is served with `OPAQUE` on,
/// and every other header and route keeps today's value.
#[tokio::test]
async fn allow_opaque_origin_on_drops_the_frame_rule_of_the_framed_pages_only() {
    let off = snapshot(&harness("opq-off", true)).await;
    let on = snapshot(&harness_with("opq-on", true, "allow_opaque_origin = true")).await;
    for ((uri, s0, h0, b0), (_, s1, h1, b1)) in off.iter().zip(&on) {
        assert_eq!(s0, s1, "{uri}");
        let framed = uri == "/ds/editor" || uri == "/docs/";
        let csp = |h: &[(String, String)]| {
            h.iter()
                .find(|(k, _)| k == "content-security-policy")
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        if framed {
            let (c0, c1) = (csp(h0), csp(h1));
            assert!(!c1.contains("frame-ancestors"), "{uri}: {c1}");
            assert_eq!(
                c0.replace(&format!("frame-ancestors {FRAMERS}; "), ""),
                c1,
                "{uri}: only the directive goes"
            );
            assert!(h1.iter().all(|(k, _)| k != "x-frame-options"), "{uri}");
            let rest = |h: &[(String, String)]| {
                h.iter()
                    .filter(|(k, _)| k != "content-security-policy")
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(rest(h0), rest(h1), "{uri}");
        } else if uri == "/ds/editor.js" {
            // `true` is one byte shorter than `false`
            let len = |h: &[(String, String)]| {
                h.iter()
                    .filter(|(k, _)| k != "content-length")
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(len(h0), len(h1), "{uri}");
        } else {
            assert_eq!(h0, h1, "{uri}");
        }
        if uri == "/ds/editor.js" {
            let (t0, t1) = (String::from_utf8_lossy(b0), String::from_utf8_lossy(b1));
            assert!(t0.contains("var OPAQUE = false;") && t1.contains("var OPAQUE = true;"));
            assert_eq!(t1.replace("var OPAQUE = true;", "var OPAQUE = false;"), t0);
        } else {
            assert_eq!(b0, b1, "{uri}");
        }
    }
}

#[tokio::test]
async fn api_js_is_a_plain_script_load() {
    // WP5 note: `<script src>` sends no Origin header, so api.js loads; a request carrying one (fetch, a module script, a form)
    // from another site is refused by the converter's origin guard. api.js must stay a classic script, never `type=module` or CORS.
    let h = harness("origin", true);
    let (st, _) = send(&h, "GET", "/ds/api.js").await;
    assert_eq!(st, StatusCode::OK, "no Origin reaches the route");
    let (st, _) = send_with(&h, "GET", "/ds/api.js", &[("origin", "https://app.test")]).await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "Origin https://app.test is not our origin"
    );
}

#[tokio::test]
async fn embed_files_are_served_with_the_right_types() {
    let h = harness("files", true);
    for (uri, ct) in [
        ("/ds/api.js", "text/javascript"),
        ("/web-apps/apps/api/documents/api.js", "text/javascript"),
        ("/ds/editor.js", "text/javascript"),
        ("/ds/editor", "text/html"),
    ] {
        let (st, hd) = send(&h, "GET", uri).await;
        assert_eq!(st, StatusCode::OK, "{uri}");
        assert!(hv(&hd, "content-type").starts_with(ct), "{uri}");
    }
    // the scripts the wrapper page loads are plain same-origin files under the strict script CSP: no inline script, no CORP opening
    let (_, hd) = send(&h, "GET", "/ds/editor.js").await;
    assert_eq!(hv(&hd, "cross-origin-resource-policy"), "same-origin");
    assert_eq!(hv(&hd, "content-security-policy"), STRICT);
    // the host-facing script never mentions a product name: it takes its origin from where it was loaded
    let api = include_str!("../../docserver/static/api.js");
    assert!(!api.to_lowercase().contains("hoshiya"));
}

/// The shared layout (docs/phase5/dropin.md D2b): Traefik's route and the backend's saved-file download reach one port. Only
/// `internal_allow` gets past the file route's peer gate (Traefik and any other peer get 403, whatever headers they send); the
/// public routes keep the public listener's rules for every peer (`tls = "upstream"`: the proxy only).
#[tokio::test]
async fn one_shared_port_keeps_the_file_route_to_internal_allow() {
    let dir = std::env::temp_dir().join(format!("suite-serve-ds-shared-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("dist")).unwrap();
    for (f, b) in [("server", b's'), ("p", b'p'), ("conv", b'c')] {
        std::fs::write(dir.join(f), [b; 40]).unwrap();
    }
    let p = |f: &str| dir.join(f).to_string_lossy().replace('\\', "/");
    let (proxy, backend) = ("10.0.0.9", "10.0.0.2");
    let s = Settings {
        listen: "0.0.0.0:80".into(),
        auth: "jwt".into(),
        secret_file: Some(dir.join("conv")),
        tls: "upstream".into(),
        trusted_proxies: vec![proxy.into()],
        public_url: Some("https://docs.test".into()),
        work_dir: Some(dir.join("work")),
        store_dir: Some(dir.join("store")),
        ..Settings::default()
    };
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "0.0.0.0:80"
internal_allow = ["{backend}"]
internal_url = "http://docs-server:80"
[[providers]]
name = "one"
issuer = "one"
compat = "docsapi"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
"#,
        p("server"),
        p("data"),
        p("p")
    )
    .parse()
    .unwrap();
    let ds = Arc::new(Docserver::from_table(t, s.listen.parse().ok()).unwrap());
    assert!(
        ds.bind_internal().unwrap().is_none(),
        "no second socket in the shared layout"
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let layout = Layout {
        shell: repo.join("web/shell"),
        dist: dir.join("dist"),
        brand: repo.join("brand/brand.toml"),
        local_key: None,
    };
    let app = App::new(s.resolve().unwrap(), ConvertConfig::from_env(), false).unwrap();
    let router = router_ds(
        app.clone(),
        &layout,
        layout.shell_config().unwrap(),
        Some(ds),
    );
    let get = |peer: &str, uri: &str| {
        let mut r = Request::builder()
            .uri(uri)
            .header("host", "docs.test")
            .header("x-forwarded-proto", "https")
            .header("x-forwarded-for", backend)
            .body(Body::empty())
            .unwrap();
        r.extensions_mut().insert(ConnectInfo(
            format!("{peer}:4000").parse::<SocketAddr>().unwrap(),
        ));
        let router = router.clone();
        async move { router.oneshot(r).await.unwrap().status() }
    };
    const API: &str = "/web-apps/apps/api/documents/api.js";
    // through Traefik: the public routes work, the saved-file route is 403 (an X-Forwarded-For naming the backend changes nothing)
    assert_eq!(get(proxy, API).await, StatusCode::OK);
    assert_eq!(get(proxy, "/ds/api.js").await, StatusCode::OK);
    for uri in ["/ds/files/s/1", "/ds/files/s/1?t=x"] {
        assert_eq!(get(proxy, uri).await, StatusCode::FORBIDDEN, "{uri}");
    }
    // another shape under /ds/files/ is the public router's: no such route
    assert_eq!(get(proxy, "/ds/files/s").await, StatusCode::NOT_FOUND);
    // the backend: on to the file token check (401 without a valid one)
    assert_eq!(
        get(backend, "/ds/files/s/1").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(backend, "/ds/files/s/1?t=x").await,
        StatusCode::UNAUTHORIZED
    );
    // ... and the public routes answer it as the public listener always did: the proxy only (no new way in for it)
    assert_eq!(get(backend, API).await, StatusCode::FORBIDDEN);
    assert_eq!(get(backend, "/ds/%66iles/s/1").await, StatusCode::FORBIDDEN);
    // any other peer (the Docker gateway of a host publish, a neighbour, the loopback): 403 on both
    for peer in ["172.18.0.1", "10.0.0.7", "127.0.0.1"] {
        assert_eq!(
            get(peer, "/ds/files/s/1").await,
            StatusCode::FORBIDDEN,
            "{peer}"
        );
        assert_eq!(get(peer, API).await, StatusCode::FORBIDDEN, "{peer}");
    }
    app.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
