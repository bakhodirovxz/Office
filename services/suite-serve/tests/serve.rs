//! The router in-process: routes, the CSP per route (F8), the cache rules, brotli precompression, and the shell catalog. The shell
//! files, the brand config and the catalog are the repo's own; the editor dists are small stand-ins written to a temp dir.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use base64::Engine as _;
use convert::exec::Config as ConvertConfig;
use convert_api::app::App;
use convert_api::config::Settings;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use suite_serve::{Layout, router};
use tower::ServiceExt;

const SCRIPT: &str = "import init from './x.js'; await init();";
const STRICT: &str = "default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct H {
    dir: PathBuf,
    router: Router,
    app: std::sync::Arc<App>,
}
impl Drop for H {
    fn drop(&mut self) {
        self.app.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn harness(name: &str) -> H {
    harness_with(name, None)
}

fn harness_with(name: &str, local_key: Option<&str>) -> H {
    let dir = std::env::temp_dir().join(format!("suite-serve-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let docs = dir.join("dist/docs-web");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("index.html"),
        format!("<html><head><script type=\"module\">{SCRIPT}</script><style>b{{}}</style></head></html>"),
    )
    .unwrap();
    std::fs::write(
        docs.join("docs-web-0123456789abcdef_bg.wasm"),
        b"\0asm\x01\0\0\0",
    )
    .unwrap();
    std::fs::write(
        docs.join("docs-web-0123456789abcdef_bg.wasm.br"),
        b"fake-brotli",
    )
    .unwrap();
    // sheets, slides and pdf are not built: their routes are absent
    let layout = Layout {
        shell: repo().join("web/shell"),
        dist: dir.join("dist"),
        brand: repo().join("brand/brand.toml"),
        local_key: local_key.map(str::to_string),
    };
    let s = Settings {
        work_dir: Some(dir.join("work")),
        store_dir: Some(dir.join("store")),
        ..Settings::default()
    };
    let app = App::new(s.resolve().unwrap(), ConvertConfig::from_env(), false).unwrap();
    let cfg = layout.shell_config().unwrap();
    H {
        router: router(app.clone(), &layout, cfg),
        app,
        dir,
    }
}

async fn get(h: &H, uri: &str, extra: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut r = Request::builder().uri(uri);
    if !extra.iter().any(|(k, _)| *k == "host") {
        r = r.header("host", "127.0.0.1:8787");
    }
    for (k, v) in extra {
        r = r.header(*k, *v);
    }
    let mut r = r.body(Body::empty()).unwrap();
    r.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5555".parse::<SocketAddr>().unwrap()));
    let res = h.router.clone().oneshot(r).await.unwrap();
    let (st, hd) = (res.status(), res.headers().clone());
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (st, hd, body.to_vec())
}

fn hv<'a>(h: &'a HeaderMap, k: &str) -> &'a str {
    h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("")
}

/// Every route carries the baseline headers; only the CSP (and the cache rule) differ per route.
fn baseline(h: &HeaderMap) {
    assert_eq!(hv(h, "x-content-type-options"), "nosniff");
    assert_eq!(hv(h, "referrer-policy"), "no-referrer");
    assert_eq!(hv(h, "cross-origin-resource-policy"), "same-origin");
    assert_eq!(hv(h, "cross-origin-opener-policy"), "same-origin");
    assert!(hv(h, "permissions-policy").contains("camera=()"));
    assert!(h.get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn shell_and_converter_keep_the_strict_csp() {
    let h = harness("strict");
    for uri in [
        "/",
        "/convert/",
        "/shell/shell.js",
        "/shell-config.json",
        "/api/v1/formats",
        "/healthz",
        "/app.js",
        "/nope",
    ] {
        let (st, hd, _) = get(&h, uri, &[]).await;
        assert_ne!(st, StatusCode::INTERNAL_SERVER_ERROR, "{uri}");
        assert_eq!(hv(&hd, "content-security-policy"), STRICT, "{uri}");
        baseline(&hd);
    }
    let (st, hd, body) = get(&h, "/", &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert!(hv(&hd, "content-type").starts_with("text/html"));
    assert!(String::from_utf8_lossy(&body).contains("shell/shell.js"));
    // the converter page moved to /convert/; the bare path redirects
    let (st, hd, body) = get(&h, "/convert/", &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("/app.js"));
    let (st, hd2, _) = get(&h, "/convert", &[]).await;
    assert_eq!(
        (st, hv(&hd2, "location")),
        (StatusCode::PERMANENT_REDIRECT, "/convert/")
    );
    let _ = hd;
}

#[tokio::test]
async fn editor_routes_get_their_own_csp() {
    let h = harness("editor");
    let hash =
        |s: &str| base64::engine::general_purpose::STANDARD.encode(Sha256::digest(s.as_bytes()));
    let (st, hd, _) = get(&h, "/docs/", &[]).await;
    assert_eq!(st, StatusCode::OK);
    let csp = hv(&hd, "content-security-policy");
    assert!(
        csp.contains("script-src 'self' 'wasm-unsafe-eval' 'sha256-")
            && csp.contains(&hash(SCRIPT)),
        "{csp}"
    );
    assert!(
        csp.contains(&format!("'sha256-{}'", hash("b{}"))),
        "inline style hash: {csp}"
    );
    assert!(
        csp.contains("frame-ancestors 'self'")
            && !csp.contains("'unsafe-inline'")
            && !csp.contains("'unsafe-eval'"),
        "{csp}"
    );
    assert_eq!(hv(&hd, "cache-control"), "no-cache");
    baseline(&hd);

    // a hashed file never changes; a client that takes brotli gets the .br next to it
    let (st, hd, body) = get(
        &h,
        "/docs/docs-web-0123456789abcdef_bg.wasm",
        &[("accept-encoding", "br")],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(hv(&hd, "content-type"), "application/wasm");
    assert_eq!(hv(&hd, "content-encoding"), "br");
    assert_eq!(body, b"fake-brotli");
    assert_eq!(
        hv(&hd, "cache-control"),
        "public, max-age=31536000, immutable"
    );
    assert!(hv(&hd, "content-security-policy").contains("'wasm-unsafe-eval'"));
    let (_, hd, body) = get(&h, "/docs/docs-web-0123456789abcdef_bg.wasm", &[]).await;
    assert!(hd.get("content-encoding").is_none() && body.starts_with(b"\0asm"));

    // no slash: redirect (the page's URLs are relative)
    let (st, hd, _) = get(&h, "/docs?host=parent", &[]).await;
    assert_eq!(
        (st, hv(&hd, "location")),
        (StatusCode::PERMANENT_REDIRECT, "/docs/?host=parent")
    );
    // path traversal stays inside the dist
    let (st, _, body) = get(&h, "/docs/..%2f..%2fCargo.toml", &[]).await;
    assert!(st != StatusCode::OK || !String::from_utf8_lossy(&body).contains("[package]"));

    // an editor that is not built is not served, and the answer is the strict one
    let (st, hd, _) = get(&h, "/sheets/", &[]).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(hv(&hd, "content-security-policy"), STRICT);
}

/// Review m3 and m5: odd names for the page are never cached for a year; the frame rule is per route; the Host/Origin guard covers the
/// routes of this crate and not only those of convert-api.
#[tokio::test]
async fn cache_frame_and_guard_rules_of_the_routes() {
    let h = harness("rules");
    for uri in ["/docs/", "/docs//", "/docs/INDEX.HTML", "/docs/index.html"] {
        let (st, hd, _) = get(&h, uri, &[]).await;
        assert!(
            !hv(&hd, "cache-control").contains("immutable"),
            "{uri} {st}: {}",
            hv(&hd, "cache-control")
        );
        if st == StatusCode::OK {
            assert_eq!(hv(&hd, "cache-control"), "no-cache", "{uri}");
            assert_eq!(hv(&hd, "x-frame-options"), "SAMEORIGIN", "{uri}");
        }
    }
    // a missing file is no cache entry either
    let (st, hd, _) = get(&h, "/docs/nope-0123456789abcdef.js", &[]).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert!(!hv(&hd, "cache-control").contains("immutable"));
    for uri in ["/", "/convert/", "/shell/shell.js", "/api/v1/formats"] {
        assert_eq!(
            hv(&get(&h, uri, &[]).await.1, "x-frame-options"),
            "DENY",
            "{uri}"
        );
    }
    // Host and Origin, on this crate's own routes
    let (st, ..) = get(&h, "/docs/", &[("host", "evil.example")]).await;
    assert_eq!(st, StatusCode::MISDIRECTED_REQUEST);
    for uri in ["/", "/shell-config.json"] {
        let (st, ..) = get(&h, uri, &[("host", "evil.example:8787")]).await;
        assert_eq!(st, StatusCode::MISDIRECTED_REQUEST, "{uri}");
    }
    let mut r = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header("host", "127.0.0.1:8787")
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    r.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5555".parse::<SocketAddr>().unwrap()));
    let res = h.router.clone().oneshot(r).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Review M1: with a key, nothing is served without the cookie; `/_k/<key>` hands the cookie out; without a key (the web host) the
/// paths behave as before.
#[tokio::test]
async fn desktop_key_gates_every_request() {
    let h = harness_with("key", Some(KEY));
    for uri in [
        "/",
        "/shell-config.json",
        "/api/v1/formats",
        "/healthz",
        "/docs/",
        "/nope",
    ] {
        assert_eq!(get(&h, uri, &[]).await.0, StatusCode::FORBIDDEN, "{uri}");
        let bad = [("cookie", "k=wrong; other=1")];
        assert_eq!(get(&h, uri, &bad).await.0, StatusCode::FORBIDDEN, "{uri}");
    }
    // a wrong key, or the key in a place other than the path, sets nothing
    let (st, hd, _) = get(&h, "/_k/wrong", &[]).await;
    assert!(st == StatusCode::FORBIDDEN && hd.get("set-cookie").is_none());
    let (st, ..) = get(&h, &format!("/?k={KEY}"), &[]).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // the key URL: a redirect home that sets an HttpOnly, SameSite=Strict cookie
    let (st, hd, _) = get(&h, &format!("/_k/{KEY}"), &[]).await;
    assert_eq!((st, hv(&hd, "location")), (StatusCode::SEE_OTHER, "/"));
    let c = hv(&hd, "set-cookie");
    assert_eq!(c, format!("k={KEY}; Path=/; HttpOnly; SameSite=Strict"));
    assert_eq!(hv(&hd, "referrer-policy"), "no-referrer");
    // with the cookie the normal answers (and their security headers) come back
    let ok = [("cookie", format!("a=b; k={KEY}"))];
    let ok: Vec<(&str, &str)> = ok.iter().map(|(k, v)| (*k, v.as_str())).collect();
    for uri in [
        "/",
        "/shell-config.json",
        "/api/v1/formats",
        "/healthz",
        "/docs/",
    ] {
        let (st, hd, _) = get(&h, uri, &ok).await;
        assert_eq!(st, StatusCode::OK, "{uri}");
        baseline(&hd);
    }
    // no key configured: the web host answers without a cookie, and `/_k/` is just an unknown path
    let web = harness("nokey");
    assert_eq!(get(&web, "/", &[]).await.0, StatusCode::OK);
    assert_eq!(
        get(&web, &format!("/_k/{KEY}"), &[]).await.0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn shell_config_comes_from_the_brand_file() {
    let l = Layout {
        shell: "x".into(),
        dist: "x".into(),
        brand: repo().join("brand/brand.toml"),
        local_key: None,
    };
    let c: serde_json::Value = serde_json::from_str(&l.shell_config().unwrap()).unwrap();
    let brand = std::fs::read_to_string(&l.brand).unwrap();
    assert!(brand.contains(&format!("name = {}", c["product"])));
    let ids: Vec<_> = c["apps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["docs", "sheets", "slides", "pdf"]);
    assert_eq!(c["apps"][0]["route"], "/docs/");
    assert!(
        c["apps"][0]["names"]["uz-Cyrl"].is_string() && c["convert"]["names"]["ru"].is_string()
    );
    // the desktop host's server tells the shell to keep no document copies (review M2); the web host does not
    assert_eq!(c["local"], false);
    let desk = Layout {
        local_key: Some(KEY.into()),
        shell: l.shell.clone(),
        dist: l.dist.clone(),
        brand: l.brand.clone(),
    };
    let c: serde_json::Value = serde_json::from_str(&desk.shell_config().unwrap()).unwrap();
    assert_eq!(c["local"], true);
    let bad = Layout {
        brand: "/nonexistent".into(),
        ..l
    };
    assert!(bad.shell_config().is_err());
}

/// The shell catalog: en is the source; ru and both Uzbek scripts have the same keys and placeholders; Uzbek is marked as an agent draft or an agent review, never an owner review.
#[test]
fn shell_catalog_is_complete() {
    let j: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo().join("web/shell/i18n.json")).unwrap())
            .unwrap();
    let ph = |s: &str| {
        let mut v: Vec<_> = s
            .match_indices('{')
            .map(|(i, _)| s[i..].split('}').next().unwrap().to_string())
            .collect();
        v.sort();
        v
    };
    let en = j["en"].as_object().unwrap();
    for lang in ["ru", "uz-Latn", "uz-Cyrl"] {
        let m = j[lang].as_object().unwrap();
        assert_eq!(
            m.keys().collect::<Vec<_>>(),
            en.keys().collect::<Vec<_>>(),
            "{lang}: keys"
        );
        for (k, v) in m {
            assert!(!v.as_str().unwrap().is_empty(), "{lang} {k}");
            assert_eq!(
                ph(v.as_str().unwrap()),
                ph(en[k].as_str().unwrap()),
                "{lang} {k}: placeholders"
            );
        }
    }
    for lang in ["uz-Latn", "uz-Cyrl"] {
        let s = j["_status"][lang].as_str().unwrap();
        assert!(
            (s.starts_with("draft:agent-") || s.starts_with("reviewed:agent-"))
                && !s.contains("owner"),
            "{lang}: {s}"
        );
    }
    // the Latin alphabet uses U+02BB, never an ASCII apostrophe, inside a word
    for v in j["uz-Latn"].as_object().unwrap().values() {
        let s = v.as_str().unwrap();
        assert!(
            !s.contains("o'") && !s.contains("g'") && !s.contains("O'"),
            "{s}"
        );
    }
}
