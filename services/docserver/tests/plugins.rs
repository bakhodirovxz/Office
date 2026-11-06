//! WP8 (PLAN phase 7, section 8 and M12): a provider's plugins over the session routes. Only the session provider's plugins are listed
//! and served, the config token cannot add one, a changed package is refused and audited, a grant is only what the admin wrote.
//! Synthetic providers and packages only.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
const SECRET_ONE: &str = "1111111111111111111111111111111111111111";
const SECRET_TWO: &str = "2222222222222222222222222222222222222222";
const ORIGIN: &str = "https://api.example.org";

fn manifest(id: &str, net: bool) -> String {
    let network = if net {
        format!("network = [\"{ORIGIN}\"]\n")
    } else {
        String::new()
    };
    format!(
        "id = \"{id}\"\nversion = \"1.0.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"Test\"\n[name]\nen = \"{id}\"\n[permissions]\ndocument = \"read\"\n{network}"
    )
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// The published test keys (ci/plugins/signing, TEST ONLY): a server takes signed packages only (O4).
fn seed(name: &str) -> Vec<u8> {
    let t = std::fs::read_to_string(format!(
        "{}/../../ci/plugins/signing/{name}.seed",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    (0..32)
        .map(|i| u8::from_str_radix(&t.trim()[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn public(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../ci/plugins/signing/{name}.pub",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .trim()
    .to_string()
}

/// Writes `SIGNATURE` into the package folder `p`, made with the test key `key`.
fn sign_pkg(p: &std::path::Path, key: &str) {
    let r = |n: &str| std::fs::read(p.join(n)).ok();
    let msg = suite_core::plugins::signed_message(
        &r("plugin.toml").unwrap(),
        &r("plugin.wasm").unwrap(),
        r("strings.tsv").as_deref(),
    );
    std::fs::write(
        p.join("SIGNATURE"),
        suite_core::plugins::signature_file(&seed(key), &msg).unwrap(),
    )
    .unwrap();
}

struct Env {
    d: PathBuf,
    alpha: String,
    beta: String,
}

impl Env {
    fn wasm(&self, id: &str) -> PathBuf {
        self.d.join("plugins").join(id).join("1.0.0/plugin.wasm")
    }
    fn audit(&self) -> String {
        std::fs::read_to_string(self.d.join("data/audit.jsonl")).unwrap_or_default()
    }
}

/// The folder: alpha (with a network origin) for provider one, beta for provider two, and the keys.
fn prepare(name: &str) -> Env {
    let d = std::env::temp_dir().join(format!("docserver-plg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut shas = vec![];
    for (id, net, body) in [
        ("alpha", true, &b"\0asm-alpha"[..]),
        ("beta", false, &b"\0asm-beta"[..]),
    ] {
        let p = d.join("plugins").join(id).join("1.0.0");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("plugin.toml"), manifest(id, net)).unwrap();
        std::fs::write(p.join("plugin.wasm"), body).unwrap();
        sign_pkg(&p, "test-a");
        shas.push(sha(body));
    }
    for (f, s) in [
        ("server.key", SERVER_SECRET),
        ("one.key", SECRET_ONE),
        ("two.key", SECRET_TWO),
    ] {
        std::fs::write(d.join(f), s).unwrap();
    }
    Env {
        d,
        alpha: shas[0].clone(),
        beta: shas[1].clone(),
    }
}

/// The server over the folder. `plugins_one` replaces the whole `plugins = [...]` line of provider one; `dir` writes `[plugins] dir`.
fn open(e: &Env, plugins_one: Option<&str>, dir: bool) -> Result<Arc<Docserver>, String> {
    let p = |f: &str| e.d.join(f).to_string_lossy().replace('\\', "/");
    let one = plugins_one.map(str::to_string).unwrap_or_else(|| {
        format!("plugins = [{{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{}\", grant = [\"{ORIGIN}\"], settings = {{ mode = \"x\" }} }}]", e.alpha)
    });
    let plugins_table = if dir {
        format!(
            "[plugins]\ndir = \"{}\"\ntrusted_keys = [\"{}\"]\n",
            p("plugins"),
            public("test-a")
        )
    } else {
        String::new()
    };
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
{plugins_table}
[[providers]]
name = "one"
issuer = "https://one.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
{one}
[[providers]]
name = "two"
issuer = "https://two.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
plugins = [{{ id = "beta", version = "1.0.0", sha256 = "{}" }}]
"#,
        p("server.key"),
        p("data"),
        p("one.key"),
        p("two.key"),
        e.beta
    )
    .parse()
    .unwrap();
    Docserver::from_table(t, None).map(Arc::new)
}

fn setup(
    name: &str,
    plugins_one: Option<&str>,
    dir: bool,
) -> (Env, Result<Arc<Docserver>, String>) {
    let e = prepare(name);
    let ds = open(&e, plugins_one, dir);
    (e, ds)
}

async fn call(
    r: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: &[u8],
) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::from(body.to_vec())).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    (
        s,
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

/// The session answer for provider `who` (1 or 2); `extra` is merged into `editorConfig`.
async fn session(r: &Router, who: u8, extra: Value) -> Value {
    let (secret, iss) = if who == 1 {
        (SECRET_ONE, "https://one.test")
    } else {
        (SECRET_TWO, "https://two.test")
    };
    let mut ec = json!({"mode": "edit", "user": {"id": "emp-1", "name": "U"}});
    for (k, v) in extra.as_object().unwrap() {
        ec[k] = v.clone();
    }
    let cfg = json!({"iss": iss, "documentType": "word",
        "document": {"fileType": "docx", "key": format!("k{who}"), "title": "a.docx", "url": "http://api:8000/f/1", "permissions": {"edit": true}},
        "editorConfig": ec, "plugins": {"pluginsData": ["https://evil.test/p.json"]}, "exp": now() + 3600});
    let body =
        json!({"origin": "https://app.test", "config": {"token": sign(secret.as_bytes(), &cfg)}});
    let (s, b) = call(r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    serde_json::from_slice(&b).unwrap()
}

fn ids(v: &Value) -> Vec<String> {
    v["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn only_the_session_providers_plugins_are_listed_and_served() {
    let (e, ds) = setup("list", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    let two = session(&r, 2, json!({})).await;
    assert_eq!(ids(&one), ["alpha"]);
    assert_eq!(ids(&two), ["beta"]);
    let a = &one["plugins"][0];
    assert_eq!(a["sha256"], e.alpha.as_str());
    assert_eq!(a["grant"], json!([ORIGIN]), "the admin's list, final");
    assert_eq!(a["settings"]["mode"], "x");
    assert_eq!(a["manifest"]["permissions"]["network"], json!([ORIGIN]));
    assert_eq!(
        two["plugins"][0]["grant"],
        json!([]),
        "listed with no grant: nobody is asked either"
    );
    // the descriptor's URL serves the file
    let url = a["wasm"].as_str().unwrap();
    assert!(url.starts_with(&format!("/ds/plugins/{}?s=", e.alpha)));
    let (s, b) = call(&r, "GET", url, None, b"").await;
    assert_eq!((s, b.as_slice()), (StatusCode::OK, &b"\0asm-alpha"[..]));
    // M12: a plugin of the other provider is 404 for this session, with the right hash and a live session
    let (sid1, sid2) = (one["sid"].as_str().unwrap(), two["sid"].as_str().unwrap());
    assert_eq!(
        call(
            &r,
            "GET",
            &format!("/ds/plugins/{}?s={sid1}", e.beta),
            None,
            b""
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &r,
            "GET",
            &format!("/ds/plugins/{}?s={sid2}", e.alpha),
            None,
            b""
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &r,
            "GET",
            &format!("/ds/plugins/{}?s={sid2}", e.beta),
            None,
            b""
        )
        .await
        .0,
        StatusCode::OK
    );
    // no session, a wrong one, a bad hash
    for u in [
        format!("/ds/plugins/{}", e.alpha),
        format!("/ds/plugins/{}?s={}", e.alpha, "0".repeat(32)),
        format!("/ds/plugins/zz?s={sid1}"),
    ] {
        assert_eq!(
            call(&r, "GET", &u, None, b"").await.0,
            StatusCode::NOT_FOUND,
            "{u}"
        );
    }
    let log = e.audit();
    assert!(
        log.contains("plugin.loaded") && log.contains("plugin.refused"),
        "{log}"
    );
    assert!(!log.contains(sid1), "the session id is not logged");
}

#[tokio::test]
async fn a_token_cannot_add_a_plugin() {
    let (e, ds) = setup("token", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    // editorConfig.plugins / pluginsData (ONLYOFFICE names), also at the top level of the config (session() adds one): all ignored
    let extra = json!({"plugins": {"autostart": ["asc.{x}"], "pluginsData": ["https://evil.test/p.json"]}, "pluginsData": ["https://evil.test/q.json"],
        "customization": {"plugins": ["https://evil.test/r.json"]}});
    let v = session(&r, 1, extra.clone()).await;
    assert_eq!(ids(&v), ["alpha"], "{v}");
    assert!(!v.to_string().contains("evil.test"));
    // a provider with no plugins stays with none, whatever the token says
    let (_e2, ds2) = setup("token2", Some(""), true);
    let r2 = ds2.unwrap().routes::<()>();
    assert_eq!(session(&r2, 1, extra).await["plugins"], json!([]));
    drop(e);
}

#[tokio::test]
async fn a_changed_package_is_refused_and_audited() {
    let (e, ds) = setup("changed", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    let url = one["plugins"][0]["wasm"].as_str().unwrap().to_string();
    assert_eq!(call(&r, "GET", &url, None, b"").await.0, StatusCode::OK);
    std::fs::write(e.wasm("alpha"), b"\0asm-tampered").unwrap();
    let (s, b) = call(&r, "GET", &url, None, b"").await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(!String::from_utf8_lossy(&b).contains("tampered"));
    assert!(e.audit().contains("changed on disk"), "{}", e.audit());
    // a server started over the changed file: the entry names another digest, so the plugin is left out and the refusal is audited
    let e2 = prepare("changed2");
    std::fs::write(e2.wasm("alpha"), b"\0asm-tampered").unwrap();
    // the entry still names the digest of the original file
    let line = format!(
        "plugins = [{{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{}\" }}]",
        e2.alpha
    );
    let again = open(&e2, Some(&line), true).unwrap();
    assert_eq!(
        session(&again.routes::<()>(), 1, json!({})).await["plugins"],
        json!([])
    );
    // the folder scan refuses it first: the publisher's signature no longer matches the changed file
    assert!(e2.audit().contains("does not match"), "{}", e2.audit());
}

/// O4: a server takes packages signed by a trusted key only; an unsigned one, one signed by another key and one whose signature does
/// not match are left out and audited, whatever the entry's SHA-256 says.
#[tokio::test]
async fn a_server_takes_only_packages_signed_by_a_trusted_key() {
    let e = prepare("unsigned");
    let dir = e.d.join("plugins/alpha/1.0.0");
    let entry = format!(
        "plugins = [{{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{}\" }}]",
        e.alpha
    );
    let listed =
        |ds: Arc<Docserver>| async move { ids(&session(&ds.routes::<()>(), 1, json!({})).await) };
    assert_eq!(
        listed(open(&e, Some(&entry), true).unwrap()).await,
        ["alpha"]
    );
    // unsigned
    std::fs::remove_file(dir.join("SIGNATURE")).unwrap();
    assert!(
        listed(open(&e, Some(&entry), true).unwrap())
            .await
            .is_empty()
    );
    assert!(
        e.audit().contains("not signed by a trusted publisher key"),
        "{}",
        e.audit()
    );
    // signed, by a key this server does not trust
    sign_pkg(&dir, "test-b");
    assert!(
        listed(open(&e, Some(&entry), true).unwrap())
            .await
            .is_empty()
    );
    // a signature that names the trusted key but was made by another one
    let forged = std::fs::read_to_string(dir.join("SIGNATURE"))
        .unwrap()
        .replace(&public("test-b"), &public("test-a"));
    std::fs::write(dir.join("SIGNATURE"), forged).unwrap();
    assert!(
        listed(open(&e, Some(&entry), true).unwrap())
            .await
            .is_empty()
    );
    assert!(e.audit().contains("does not match"), "{}", e.audit());
    // the admin trusts test-b too: its signature counts
    sign_pkg(&dir, "test-b");
    let p = |f: &str| e.d.join(f).to_string_lossy().replace('\\', "/");
    let both = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
[plugins]
dir = "{}"
trusted_keys = ["{}", "{}"]
[[providers]]
name = "one"
issuer = "https://one.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
{entry}
"#,
        p("server.key"),
        p("data"),
        p("plugins"),
        public("test-a"),
        public("test-b"),
        p("one.key"),
    );
    let ds = Docserver::from_table(both.parse().unwrap(), None)
        .map(Arc::new)
        .unwrap();
    assert_eq!(listed(ds).await, ["alpha"]);
    // a malformed key stops the start
    let bad = both.replace(&public("test-b"), "ABC");
    assert!(
        Docserver::from_table(bad.parse().unwrap(), None)
            .err()
            .unwrap()
            .contains("trusted_keys")
    );
}

/// Uninstall on a server is the admin's: drop the provider entry and the package folder, restart. A package removed while the server
/// runs is not served again (an editor that loaded it keeps the running copy until it reloads).
#[tokio::test]
async fn a_removed_package_is_not_served_and_a_restart_forgets_it() {
    let (e, ds) = setup("removed", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    let url = one["plugins"][0]["wasm"].as_str().unwrap().to_string();
    assert_eq!(call(&r, "GET", &url, None, b"").await.0, StatusCode::OK);
    std::fs::remove_dir_all(e.d.join("plugins/alpha")).unwrap();
    assert_ne!(call(&r, "GET", &url, None, b"").await.0, StatusCode::OK);
    // restarted with the entry removed (the admin's uninstall): nothing of it is listed, its grant is gone with the entry
    let again = open(&e, Some(""), true).unwrap();
    assert!(again.plugin_origins().is_empty());
    assert_eq!(
        session(&again.routes::<()>(), 1, json!({})).await["plugins"],
        json!([])
    );
}

#[tokio::test]
async fn bad_plugin_entries_stop_the_start() {
    let sha_a = sha(b"\0asm-alpha");
    let entry = |s: &str| {
        format!("plugins = [{{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{sha_a}\"{s} }}]")
    };
    for (what, plugins, dir, want) in [
        (
            "no folder",
            entry(""),
            false,
            "needs [docserver.plugins] dir",
        ),
        (
            "a bad grant",
            entry(", grant = [\"http://api.example.org\"]"),
            true,
            "not an https",
        ),
        (
            "an undeclared grant",
            entry(", grant = [\"https://other.example.org\"]"),
            true,
            "does not declare",
        ),
        (
            "a bad digest",
            "plugins = [{ id = \"alpha\", version = \"1.0.0\", sha256 = \"ABC\" }]".to_string(),
            true,
            "64 lower-case hex",
        ),
        (
            "a duplicate",
            format!(
                "plugins = [{{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{sha_a}\" }}, {{ id = \"alpha\", version = \"1.0.0\", sha256 = \"{sha_a}\" }}]"
            ),
            true,
            "listed twice",
        ),
    ] {
        let (_e, ds) = setup("bad", Some(&plugins), dir);
        let err = ds.err().unwrap_or_else(|| panic!("{what}: started"));
        assert!(err.contains(want), "{what}: {err}");
    }
    // a package that is not in the folder is left out, not a refusal to start
    let (e, ds) = setup(
        "missing",
        Some(&format!(
            "plugins = [{{ id = \"nope\", version = \"1.0.0\", sha256 = \"{sha_a}\" }}]"
        )),
        true,
    );
    let ds = ds.unwrap();
    assert_eq!(
        session(&ds.routes::<()>(), 1, json!({})).await["plugins"],
        json!([])
    );
    assert!(e.audit().contains("plugin.refused"));
}

#[tokio::test]
async fn the_page_reports_network_use_for_the_audit_log() {
    let (e, ds) = setup("event", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    let (sid, tok) = (one["sid"].as_str().unwrap(), one["token"].as_str().unwrap());
    let uri = format!("/ds/session/{sid}/plugin-event");
    let ev = |plugin: &str, kind: &str| {
        json!({"plugin": plugin, "kind": kind, "origin": ORIGIN}).to_string()
    };
    assert_eq!(
        call(&r, "POST", &uri, None, ev("alpha", "network").as_bytes())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &r,
            "POST",
            &uri,
            Some(tok),
            ev("alpha", "network").as_bytes()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &r,
            "POST",
            &uri,
            Some(tok),
            ev("alpha", "network.refused").as_bytes()
        )
        .await
        .0,
        StatusCode::OK
    );
    // another provider's plugin, an unknown kind, junk, a `network` line for an origin the plugin was not granted
    let other =
        json!({"plugin": "alpha", "kind": "network", "origin": "https://other.example.org"})
            .to_string();
    for body in [
        ev("beta", "network"),
        ev("alpha", "failed"),
        "x".into(),
        other,
    ] {
        assert_eq!(
            call(&r, "POST", &uri, Some(tok), body.as_bytes()).await.0,
            StatusCode::BAD_REQUEST,
            "{body}"
        );
    }
    assert_eq!(
        e.audit().matches("plugin.network").count(),
        2,
        "{}",
        e.audit()
    );
    assert!(!e.audit().contains("other.example.org"));
    assert_eq!(e.audit().matches(r#""source":"client""#).count(), 2);
    // a flood from one session is cut off (20 failures a minute by default; the two accepted above count)
    let mut last = StatusCode::OK;
    for _ in 0..30 {
        last = call(
            &r,
            "POST",
            &uri,
            Some(tok),
            ev("alpha", "network").as_bytes(),
        )
        .await
        .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        e.audit().matches("plugin.network").count() <= 20,
        "{}",
        e.audit()
    );
}

#[tokio::test]
async fn the_plugin_file_route_takes_only_a_sha_and_audits_little() {
    let (e, ds) = setup("shaflood", None, true);
    let ds = ds.unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    let sid = one["sid"].as_str().unwrap();
    let url = one["plugins"][0]["wasm"].as_str().unwrap().to_string();
    for _ in 0..3 {
        assert_eq!(call(&r, "GET", &url, None, b"").await.0, StatusCode::OK);
    }
    let junk = "a".repeat(5000);
    for bad in [junk, e.alpha.to_uppercase(), "ab".into()] {
        let u = format!("/ds/plugins/{bad}?s={sid}");
        assert_eq!(
            call(&r, "GET", &u, None, b"").await.0,
            StatusCode::NOT_FOUND
        );
    }
    // well-formed but unknown: 404, and many of them write one line
    for i in 0..10 {
        let u = format!("/ds/plugins/{i:064x}?s={sid}");
        assert_eq!(
            call(&r, "GET", &u, None, b"").await.0,
            StatusCode::NOT_FOUND
        );
    }
    let log = e.audit();
    assert_eq!(log.matches("plugin.loaded").count(), 1, "{log}");
    assert_eq!(log.matches("plugin.refused").count(), 1, "{log}");
    assert!(!log.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
}

#[tokio::test]
async fn the_wrapper_page_may_connect_to_the_granted_origins_and_nowhere_else() {
    let (_e, ds) = setup("csp", None, true);
    let csp = ds.unwrap().frame_csp();
    assert!(
        csp.contains(&format!("connect-src 'self' {ORIGIN}")) && !csp.contains('*'),
        "{csp}"
    );
    // no plugin with a grant: the baseline, unchanged
    let (_e, ds) = setup("csp2", Some(""), true);
    // provider two's beta has no grant either
    let csp = ds.unwrap().frame_csp();
    assert!(!csp.contains("connect-src"), "{csp}");
}

/// WP7 (PLAN phase 7, section 7): the template-fields plugin takes its field list from the provider entry. The package is the real
/// manifest with a stand-in binary; the names are synthetic.
#[tokio::test]
async fn template_fields_settings_come_from_the_provider_entry() {
    let e = prepare("fields");
    let body = b"\0asm-fields";
    let p = e.d.join("plugins/template-fields/1.0.0");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(
        p.join("plugin.toml"),
        include_str!("../../../plugins/template-fields/plugin.toml"),
    )
    .unwrap();
    std::fs::write(p.join("plugin.wasm"), body).unwrap();
    sign_pkg(&p, "test-a");
    let entry = format!(
        "plugins = [{{ id = \"template-fields\", version = \"1.0.0\", sha256 = \"{}\", settings = {{ fields = [{{ name = \"field_z\", label = \"Sample zeta\", kind = \"text\" }}, {{ name = \"mark_z\", label = \"Sample mark\", kind = \"bookmark\" }}] }} }}]",
        sha(body)
    );
    let ds = open(&e, Some(&entry), true).unwrap();
    let r = ds.routes::<()>();
    let one = session(&r, 1, json!({})).await;
    assert_eq!(ids(&one), ["template-fields"]);
    let d = &one["plugins"][0];
    assert_eq!(
        d["settings"]["fields"],
        json!([
            {"name": "field_z", "label": "Sample zeta", "kind": "text"},
            {"name": "mark_z", "label": "Sample mark", "kind": "bookmark"}
        ]),
        "the admin's list replaces the manifest default"
    );
    assert_eq!(d["manifest"]["settings"]["fields"][0]["name"], "field_a");
    assert_eq!(d["manifest"]["events"], json!(["selectionChanged"]));
}
