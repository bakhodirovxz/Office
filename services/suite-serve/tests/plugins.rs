//! WP8 (PLAN phase 7, section 8): the local web host's plugins. `/shell-config.json` lists the enabled, verified packages, `/plugins/<sha256>`
//! serves their wasm, the home page's CSP allows exactly the granted origins, and a package that changed since its record is refused.
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use convert::exec::Config as ConvertConfig;
use convert_api::app::App;
use convert_api::config::Settings;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use suite_core::plugins::{Policy, Root, State};
use suite_serve::{Layout, PluginSource, router_plugins};
use tower::ServiceExt;

const ORIGIN: &str = "https://api.example.org";

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn manifest(id: &str) -> String {
    format!(
        "id = \"{id}\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"Test\"\n[name]\nen = \"{id}\"\n[permissions]\ndocument = \"read\"\nnetwork = [\"{ORIGIN}\"]\n"
    )
}

struct Env {
    dir: PathBuf,
    plugins: PathBuf,
    app: Arc<App>,
    router: axum::Router,
}
impl Drop for Env {
    fn drop(&mut self) {
        self.app.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `with_record`: write `install.toml` next to alpha, so a change is detected.
fn env(
    name: &str,
    enabled: &[&str],
    grants: &[(&str, &[&str])],
    with_record: bool,
) -> (Env, String) {
    let dir = std::env::temp_dir().join(format!("suite-serve-plg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let plugins = dir.join("plugins");
    for (id, wasm) in [
        ("alpha", &b"\0asm-alpha"[..]),
        ("gamma", &b"\0asm-gamma"[..]),
    ] {
        let p = plugins.join(id).join("1.0.0");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("plugin.toml"), manifest(id)).unwrap();
        std::fs::write(p.join("plugin.wasm"), wasm).unwrap();
        if with_record && id == "alpha" {
            std::fs::write(
                p.join("install.toml"),
                format!(
                    "sha256 = \"{}\"\nmanifest_sha256 = \"{}\"\n",
                    sha(wasm),
                    sha(manifest(id).as_bytes())
                ),
            )
            .unwrap();
        }
    }
    let layout = Layout {
        shell: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/shell"),
        dist: dir.join("dist"),
        brand: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brand/brand.toml"),
        local_key: None,
    };
    let s = Settings {
        work_dir: Some(dir.join("work")),
        store_dir: Some(dir.join("store")),
        ..Settings::default()
    };
    let app = App::new(s.resolve().unwrap(), ConvertConfig::from_env(), false).unwrap();
    let st = State {
        enabled: enabled.iter().map(|s| s.to_string()).collect(),
        grants: grants
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect::<BTreeMap<_, _>>(),
    };
    let src = Arc::new(PluginSource {
        roots: vec![Root {
            dir: plugins.clone(),
            machine: true,
        }],
        now: Box::new(move || (st.clone(), Policy::default())),
        fixed: None,
    });
    let cfg = layout.shell_config().unwrap();
    let router = router_plugins(app.clone(), &layout, cfg, None, Some(src));
    (
        Env {
            dir,
            plugins,
            app,
            router,
        },
        sha(b"\0asm-alpha"),
    )
}

async fn get(e: &Env, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut r = Request::builder()
        .uri(uri)
        .header("host", "127.0.0.1:8787")
        .body(Body::empty())
        .unwrap();
    r.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5555".parse::<SocketAddr>().unwrap()));
    let res = e.router.clone().oneshot(r).await.unwrap();
    let (st, hd) = (res.status(), res.headers().clone());
    (
        st,
        hd,
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

fn csp(h: &HeaderMap) -> String {
    h.get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

#[tokio::test]
async fn enabled_plugins_are_listed_and_served_and_nothing_else() {
    let (e, a) = env("list", &["alpha"], &[("alpha", &[ORIGIN])], false);
    let (s, _, b) = get(&e, "/shell-config.json").await;
    assert_eq!(s, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(
        v["plugins"].as_array().unwrap().len(),
        1,
        "only the enabled one: {v}"
    );
    let d = &v["plugins"][0];
    assert_eq!(
        (d["id"].clone(), d["sha256"].clone(), d["wasm"].clone()),
        (
            "alpha".into(),
            a.clone().into(),
            format!("/plugins/{a}").into()
        )
    );
    assert_eq!(d["grant"], serde_json::json!([ORIGIN]));
    assert!(v["product"].is_string(), "the rest of the config is kept");
    let (s, h, b) = get(&e, &format!("/plugins/{a}")).await;
    assert_eq!(
        (s, b.as_slice(), h["content-type"].to_str().unwrap()),
        (StatusCode::OK, &b"\0asm-alpha"[..], "application/wasm")
    );
    // a package that is installed but not enabled is not served
    for u in [
        format!("/plugins/{}", sha(b"\0asm-gamma")),
        "/plugins/nothex".to_string(),
        format!("/plugins/{}", "0".repeat(64)),
    ] {
        assert_eq!(get(&e, &u).await.0, StatusCode::NOT_FOUND, "{u}");
    }
}

#[tokio::test]
async fn the_home_pages_csp_allows_exactly_the_granted_origins() {
    let (e, _) = env(
        "csp",
        &["alpha"],
        &[("alpha", &[ORIGIN, "https://evil.example.org"])],
        false,
    );
    let (_, h, _) = get(&e, "/").await;
    // the second origin is not in alpha's manifest, so it is not a grant at all
    assert!(
        csp(&h).ends_with(&format!("; connect-src 'self' {ORIGIN}")),
        "{}",
        csp(&h)
    );
    assert!(csp(&h).starts_with("default-src 'self'; script-src 'self';"));
    // enabled but not granted, or granted but not enabled: the baseline
    for (en, gr) in [
        (&["alpha"][..], &[][..]),
        (&[][..], &[("alpha", &[ORIGIN][..])][..]),
    ] {
        let (e, _) = env("csp2", en, gr, false);
        assert!(!csp(&get(&e, "/").await.1).contains("connect-src"));
    }
    // other pages keep the baseline
    assert!(!csp(&get(&e, "/shell/shell.js").await.1).contains("connect-src"));
}

#[tokio::test]
async fn a_package_that_changed_since_its_record_is_refused() {
    let (e, a) = env("changed", &["alpha"], &[], true);
    assert_eq!(get(&e, &format!("/plugins/{a}")).await.0, StatusCode::OK);
    std::fs::write(e.plugins.join("alpha/1.0.0/plugin.wasm"), b"\0asm-tampered").unwrap();
    assert_eq!(
        get(&e, &format!("/plugins/{a}")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&e, &format!("/plugins/{}", sha(b"\0asm-tampered")))
            .await
            .0,
        StatusCode::NOT_FOUND,
        "the new bytes have no record either"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&get(&e, "/shell-config.json").await.2).unwrap();
    assert_eq!(v["plugins"], serde_json::json!([]));
}
