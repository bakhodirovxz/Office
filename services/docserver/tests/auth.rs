//! WP3 (PLAN phase 5, 3.2 and 10): the three tokens on every call, the on-disk session, and one config produced by the fake HRB.
//! S1 (every route checks its token), S3 (exp rules), S5 (only the signed payload counts), S10 (a view session cannot save),
//! S11 (the file token's bindings), S24 (no existence oracle), and the restart test.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower::ServiceExt;

const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";
const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
const DOC: &[u8] = b"PK\x03\x04 synthetic docx v1";
const DOC2: &[u8] = b"PK\x03\x04 synthetic docx v2, saved";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("docserver-auth-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A docserver over `d` (the same `d` twice is a restart). `fetch` / `callback` are the providers' allowlists.
fn build(d: &Path, prov_secret: &str, require_exp: bool, fetch: &[&str], callback: &[&str]) -> Arc<Docserver> {
    std::fs::write(d.join("server.key"), SERVER_SECRET).unwrap();
    std::fs::write(d.join("prov.key"), prov_secret).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let list = |v: &[&str]| {
        v.iter()
            .map(|u| format!("{{ url = \"{u}\", internal = true }}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19999"
internal_allow = ["127.0.0.0/8"]
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{}]
callback_allow = [{}]
frame_origins = ["https://app.test"]
require_exp = {require_exp}
"#,
        p("server.key"),
        p("data"),
        p("prov.key"),
        list(fetch),
        list(callback)
    )
    .parse()
    .unwrap();
    Arc::new(Docserver::from_table(t, None).unwrap())
}

fn std_ds(d: &Path) -> Arc<Docserver> {
    build(d, PROV_SECRET, true, &["http://api:8000"], &["http://api:8000"])
}

fn config() -> Value {
    json!({
        "documentType": "word",
        "document": {"fileType": "docx", "key": "file_7_v1_ab", "title": "a.docx",
            "url": "http://api:8000/docs-file/secret-url-token",
            "permissions": {"edit": true, "download": true, "print": true, "review": false, "comment": true}},
        "editorConfig": {"mode": "edit", "callbackUrl": "http://api:8000/files/7/save-callback",
            "user": {"id": "emp-1", "name": "Synthetic User"}, "lang": "ru"},
        "exp": now() + 3600
    })
}

async fn call(r: &Router, method: &str, uri: &str, bearer: Option<&str>, body: &[u8]) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::from(body.to_vec())).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    (s, axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn js(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap_or(Value::Null)
}

/// POST /ds/session with a payload signed by `secret`.
async fn open_with(r: &Router, payload: &Value, secret: &str) -> (StatusCode, Value) {
    let body = json!({"origin": "https://app.test", "config": {"token": sign(secret.as_bytes(), payload)}});
    let (s, b) = call(r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    (s, js(&b))
}

async fn open(r: &Router, payload: &Value) -> (StatusCode, Value) {
    open_with(r, payload, PROV_SECRET).await
}

fn subkey(label: &str) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(SERVER_SECRET.as_bytes()).unwrap();
    m.update(label.as_bytes());
    m.finalize().into_bytes().to_vec()
}

fn payload_of(token: &str) -> Value {
    js(&B64.decode(token.split('.').nth(1).unwrap()).unwrap())
}

/// A session with version 1 in place (what WP4's fetch will do).
async fn session(ds: &Arc<Docserver>, r: &Router, payload: &Value) -> (String, String) {
    let (s, v) = open(r, payload).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let sid = v["sid"].as_str().unwrap().to_string();
    ds.put_version(&sid, DOC, "one", false, now()).unwrap();
    (sid, v["token"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn config_token_refusals() {
    let d = dir("cfg");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (s, v) = open(&r, &config()).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let sid = v["sid"].as_str().unwrap();
    assert_eq!(sid.len(), 32);
    assert!(sid.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_eq!((v["editor"].as_str(), v["mode"].as_str()), (Some("docs"), Some("edit")));
    let (_, v2) = open(&r, &config()).await;
    assert_ne!(v2["sid"], v["sid"], "session ids are fresh");

    let other = "oooooooooooooooooooooooooooooooooooooooo";
    let mut cases: Vec<(&str, StatusCode, Value, &str)> = vec![];
    let ok = StatusCode::OK;
    let no = StatusCode::UNAUTHORIZED;
    let with = |f: &dyn Fn(&mut Value)| {
        let mut c = config();
        f(&mut c);
        c
    };
    cases.push(("bad signature", no, config(), other));
    cases.push(("wrong issuer", no, with(&|c| c["iss"] = json!("https://evil.test")), PROV_SECRET));
    cases.push(("issuer given and right", ok, with(&|c| c["iss"] = json!("https://issuer.test")), PROV_SECRET));
    cases.push(("missing exp", no, with(&|c| { c.as_object_mut().unwrap().remove("exp"); }), PROV_SECRET));
    cases.push(("expired exp", no, with(&|c| c["exp"] = json!(now() - 300)), PROV_SECRET));
    cases.push(("too long exp", no, with(&|c| c["exp"] = json!(now() + 13 * 3600)), PROV_SECRET));
    cases.push(("11 h exp", ok, with(&|c| c["exp"] = json!(now() + 11 * 3600)), PROV_SECRET));
    cases.push(("exp is text", no, with(&|c| c["exp"] = json!("soon")), PROV_SECRET));
    cases.push(("nbf in the future", no, with(&|c| c["nbf"] = json!(now() + 3600)), PROV_SECRET));
    for (what, want, payload, secret) in cases {
        let (s, v) = open_with(&r, &payload, secret).await;
        assert_eq!(s, want, "{what}: {v}");
    }

    // alg none, and an alg that is not HS256, with a body that would otherwise be fine
    let payload = B64.encode(config().to_string());
    for alg in ["none", "HS512", "RS256"] {
        let head = B64.encode(json!({"alg": alg, "typ": "JWT"}).to_string());
        for sig in [String::new(), B64.encode(b"x")] {
            let body = json!({"origin": "https://app.test", "config": {"token": format!("{head}.{payload}.{sig}")}});
            let (s, _) = call(&r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
            assert_eq!(s, no, "alg {alg}");
        }
    }
    // no token at all, a token that is not text, a body that is not JSON
    for body in [json!({}), json!({"origin": "https://app.test", "config": {}}), json!({"origin": "https://app.test", "config": {"token": 5}}), json!({"token": "x"})] {
        let (s, _) = call(&r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
        assert_eq!(s, no, "{body}");
    }
    let (s, _) = call(&r, "POST", "/ds/session", None, b"not json").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // a session token is not a config token, nor a file token
    let (_, tok) = (0, v["token"].as_str().unwrap().to_string());
    let (s, _) = call(&r, "POST", "/ds/session", None, json!({"origin": "https://app.test", "config": {"token": tok}}).to_string().as_bytes()).await;
    assert_eq!(s, no);
}

#[tokio::test]
async fn config_field_refusals_and_modes() {
    let d = dir("fields");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let bad = |what: &str, f: &dyn Fn(&mut Value)| {
        let mut c = config();
        f(&mut c);
        (what.to_string(), c)
    };
    let cases = [
        bad("key with a space", &|c| c["document"]["key"] = json!("a b")),
        bad("key too long", &|c| c["document"]["key"] = json!("k".repeat(129))),
        bad("no key", &|c| { c["document"].as_object_mut().unwrap().remove("key"); }),
        bad("file type", &|c| c["document"]["fileType"] = json!("exe")),
        bad("document type mismatch", &|c| c["documentType"] = json!("cell")),
        bad("url off the list", &|c| c["document"]["url"] = json!("http://evil:8000/x")),
        bad("url port", &|c| c["document"]["url"] = json!("http://api:80/x")),
        bad("url scheme", &|c| c["document"]["url"] = json!("https://api:8000/x")),
        bad("url user info", &|c| c["document"]["url"] = json!("http://u@api:8000/x")),
        bad("url host trick", &|c| c["document"]["url"] = json!("http://api:8000@evil:1/x")),
        bad("url backslash", &|c| c["document"]["url"] = json!("http://evil\\@api:8000/x")),
        bad("no url", &|c| { c["document"].as_object_mut().unwrap().remove("url"); }),
        bad("callback off the list", &|c| c["editorConfig"]["callbackUrl"] = json!("http://evil:8000/x")),
        bad("no user", &|c| { c["editorConfig"].as_object_mut().unwrap().remove("user"); }),
        bad("mode", &|c| c["editorConfig"]["mode"] = json!("admin")),
        bad("permission type", &|c| c["document"]["permissions"]["edit"] = json!("yes")),
        bad("no document", &|c| { c.as_object_mut().unwrap().remove("document"); }),
    ];
    for (what, c) in cases {
        let (s, v) = open(&r, &c).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{what}: {v}");
        assert!(!v.to_string().contains("secret-url-token"), "{what}: a value in the error");
    }
    // S8: a callback URL that is only on the fetch list
    let d2 = dir("fields-s8");
    let ds2 = build(&d2, PROV_SECRET, true, &["http://api:8000", "http://cb:9000"], &["http://cb:9000"]);
    let r2 = ds2.routes::<()>();
    let (s, _) = open(&r2, &config()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "callback on the fetch list only");
    let mut c = config();
    c["editorConfig"]["callbackUrl"] = json!("http://cb:9000/cb");
    assert_eq!(open(&r2, &c).await.0, StatusCode::OK);

    // view: by mode, by permission, and by a missing callback; each one cannot save
    for (what, f) in [
        ("mode view", Box::new(|c: &mut Value| c["editorConfig"]["mode"] = json!("view")) as Box<dyn Fn(&mut Value)>),
        ("edit false", Box::new(|c| c["document"]["permissions"]["edit"] = json!(false))),
        ("no callback", Box::new(|c| { c["editorConfig"].as_object_mut().unwrap().remove("callbackUrl"); })),
    ] {
        let mut c = config();
        f(&mut c);
        let (sid, tok) = session(&ds, &r, &c).await;
        let (s, v) = open(&r, &c).await;
        assert_eq!((s, v["mode"].as_str(), v["perms"]["edit"].as_bool()), (StatusCode::OK, Some("view"), Some(false)), "{what}");
        for route in ["save", "autosave"] {
            let (s, _) = call(&r, "POST", &format!("/ds/session/{sid}/{route}"), Some(&tok), DOC2).await;
            assert_eq!(s, StatusCode::FORBIDDEN, "{what} {route}");
        }
        let (s, b) = call(&r, "GET", &format!("/ds/session/{sid}/document"), Some(&tok), b"").await;
        assert_eq!((s, b.as_slice()), (StatusCode::OK, DOC), "{what}: a view session still reads");
        let (s, b) = call(&r, "POST", &format!("/ds/session/{sid}/close"), Some(&tok), b"").await;
        assert_eq!((s, js(&b)["status"].as_u64()), (StatusCode::OK, Some(4)));
    }
}

#[tokio::test]
async fn config_message_origin_must_belong_to_the_signing_provider() {
    let d = dir("origin");
    std::fs::write(d.join("b.key"), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
    std::fs::write(d.join("server.key"), SERVER_SECRET).unwrap();
    std::fs::write(d.join("a.key"), PROV_SECRET).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
[[providers]]
name = "a"
issuer = "ia"
secret_file = "{}"
fetch_allow = ["http://api:8000"]
frame_origins = ["https://a.test"]
[[providers]]
name = "b"
issuer = "ib"
secret_file = "{}"
fetch_allow = ["http://api:8000"]
frame_origins = ["https://b.test", "https://ds.test"]
"#,
        p("server.key"), p("data"), p("a.key"), p("b.key")
    )
    .parse()
    .unwrap();
    let ds = Arc::new(Docserver::from_table(t, None).unwrap());
    let r = ds.routes::<()>();
    let mut c = config();
    c["editorConfig"].as_object_mut().unwrap().remove("callbackUrl");
    let go = |secret: &'static str, origin: Option<&'static str>| {
        let r = r.clone();
        let c = c.clone();
        async move {
            let mut body = json!({"config": {"token": sign(secret.as_bytes(), &c)}});
            if let Some(o) = origin {
                body["origin"] = json!(o);
            }
            call(&r, "POST", "/ds/session", None, body.to_string().as_bytes()).await.0
        }
    };
    let b_secret = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert_eq!(go(PROV_SECRET, Some("https://a.test")).await, StatusCode::OK);
    assert_eq!(go(b_secret, Some("https://b.test")).await, StatusCode::OK);
    // A's page with B's token, B's page with A's token: refused
    assert_eq!(go(b_secret, Some("https://a.test")).await, StatusCode::FORBIDDEN);
    assert_eq!(go(PROV_SECRET, Some("https://b.test")).await, StatusCode::FORBIDDEN);
    // our own origin only for the provider that lists it
    assert_eq!(go(b_secret, Some("https://ds.test")).await, StatusCode::OK);
    assert_eq!(go(PROV_SECRET, Some("https://ds.test")).await, StatusCode::FORBIDDEN);
    assert_eq!(go(PROV_SECRET, Some("null")).await, StatusCode::FORBIDDEN);
    assert_eq!(go(PROV_SECRET, None).await, StatusCode::FORBIDDEN, "no origin");
    // a bad token is still a 401, whatever the origin
    assert_eq!(go("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", Some("https://a.test")).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn outer_fields_are_ignored() {
    let d = dir("outer");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let mut signed = config();
    signed["editorConfig"]["mode"] = json!("view");
    let body = json!({"origin": "https://app.test", "config": {
        "token": sign(PROV_SECRET.as_bytes(), &signed),
        "document": {"url": "http://evil:1/x", "key": "outer", "fileType": "xlsx", "permissions": {"edit": true}},
        "editorConfig": {"mode": "edit", "callbackUrl": "http://evil:1/cb", "user": {"id": "root"}},
        "events": {"x": 1}, "type": "mobile"}});
    let (s, b) = call(&r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    let v = js(&b);
    assert_eq!((s, v["mode"].as_str(), v["editor"].as_str()), (StatusCode::OK, Some("view"), Some("docs")), "{v}");
    let rec = ds.load_session(v["sid"].as_str().unwrap()).unwrap();
    assert_eq!(rec.document_url, "http://api:8000/docs-file/secret-url-token");
    assert_eq!((rec.key.as_str(), rec.user_id.as_str()), ("file_7_v1_ab", "emp-1"));
    assert!(rec.callback_url.is_some());
}

#[tokio::test]
async fn every_route_checks_its_token() {
    let d = dir("s1");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (a, ta) = session(&ds, &r, &config()).await;
    let (b, tb) = session(&ds, &r, &config()).await;
    let t = now();
    let sess = |extra: Value| {
        let mut p = json!({"purpose": "session", "sid": a, "sub": "emp-1", "prov": "one", "exp": t + 600});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        sign(&subkey("session"), &p)
    };
    let mut tampered = ta.clone().into_bytes();
    let i = tampered.len() - 3;
    tampered[i] = if tampered[i] == b'A' { b'B' } else { b'A' };
    let file_tok = ds.mint_file_token(&a, 1, "00", t);
    let forged: Vec<(&str, String)> = vec![
        ("tampered", String::from_utf8(tampered).unwrap()),
        ("expired", sess(json!({"exp": t - 120}))),
        ("expired, inside the 60 s leeway", sess(json!({"exp": t - 10}))),
        ("lifetime too long", sess(json!({"exp": t + 86400}))),
        ("no exp", sign(&subkey("session"), &json!({"purpose": "session", "sid": a, "sub": "emp-1", "prov": "one"}))),
        ("purpose file under the session key", sess(json!({"purpose": "file"}))),
        ("a real file token", file_tok),
        ("the server secret itself", sign(SERVER_SECRET.as_bytes(), &json!({"purpose": "session", "sid": a, "sub": "emp-1", "prov": "one", "exp": t + 600}))),
        ("the provider secret", sign(PROV_SECRET.as_bytes(), &json!({"purpose": "session", "sid": a, "sub": "emp-1", "prov": "one", "exp": t + 600}))),
        ("a config token", sign(PROV_SECRET.as_bytes(), &config())),
        ("garbage", "a.b.c".into()),
    ];
    let routes = [("POST", "renew"), ("GET", "document"), ("POST", "save"), ("POST", "autosave"), ("POST", "close")];
    for (m, route) in routes {
        let uri = format!("/ds/session/{a}/{route}");
        let (s, _) = call(&r, m, &uri, None, DOC2).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{route}: no token");
        for (what, tok) in &forged {
            let (s, _) = call(&r, m, &uri, Some(tok), DOC2).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{route}: {what}");
        }
        // S24: another session's token, and a sid that does not exist, are the same answer
        let (s1, b1) = call(&r, m, &uri, Some(&tb), DOC2).await;
        let ghost = format!("/ds/session/{}/{route}", "0".repeat(32));
        let (s2, b2) = call(&r, m, &ghost, Some(&ta), DOC2).await;
        let (s3, b3) = call(&r, m, &format!("/ds/session/nope/{route}"), Some(&ta), DOC2).await;
        assert_eq!((s1, s2, s3), (StatusCode::NOT_FOUND, StatusCode::NOT_FOUND, StatusCode::NOT_FOUND), "{route}");
        assert!(b1 == b2 && b2 == b3, "{route}: the 404 bodies differ");
    }
    // the genuine token works on every route, and B's session is untouched by A's
    let (s, body) = call(&r, "GET", &format!("/ds/session/{a}/document"), Some(&ta), b"").await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, DOC));
    let (s, body) = call(&r, "POST", &format!("/ds/session/{a}/save"), Some(&ta), DOC2).await;
    assert_eq!((s, js(&body)["version"].as_u64()), (StatusCode::OK, Some(2)));
    let (_, body) = call(&r, "GET", &format!("/ds/session/{b}/document"), Some(&tb), b"").await;
    assert_eq!(body, DOC, "B still has its own document");
    let (s, body) = call(&r, "POST", &format!("/ds/session/{a}/renew"), Some(&ta), b"").await;
    assert_eq!(s, StatusCode::OK);
    let fresh = js(&body)["token"].as_str().unwrap().to_string();
    assert_eq!(payload_of(&fresh)["sid"], json!(a));
    // body checks on save: not a document, too large
    let (s, _) = call(&r, "POST", &format!("/ds/session/{a}/save"), Some(&ta), b"not a zip").await;
    assert_eq!(s, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let big = vec![b'P'; 51 << 20];
    let (s, _) = call(&r, "POST", &format!("/ds/session/{a}/save"), Some(&ta), &big).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    // a closed session is gone for every route but close (which repeats its answer)
    let (s, body) = call(&r, "POST", &format!("/ds/session/{a}/close"), Some(&ta), b"").await;
    assert_eq!((s, js(&body)["status"].as_u64()), (StatusCode::OK, Some(2)));
    for (m, route) in routes.iter().filter(|(_, r)| *r != "close") {
        let (s, _) = call(&r, m, &format!("/ds/session/{a}/{route}"), Some(&ta), DOC2).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{route} after close");
    }
    let (s, body) = call(&r, "POST", &format!("/ds/session/{a}/close"), Some(&ta), b"").await;
    assert_eq!((s, js(&body)["status"].as_u64()), (StatusCode::OK, Some(2)));
}

#[tokio::test]
async fn session_token_never_outlives_the_config() {
    let d = dir("cap");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let mut c = config();
    let end = now() + 120;
    c["exp"] = json!(end);
    let (_, v) = open(&r, &c).await;
    assert!(payload_of(v["token"].as_str().unwrap())["exp"].as_u64().unwrap() <= end);
    let sid = v["sid"].as_str().unwrap().to_string();
    let first_token = v["token"].as_str().unwrap().to_string();
    let (_, b) = call(&r, "POST", &format!("/ds/session/{sid}/renew"), v["token"].as_str(), b"").await;
    assert!(payload_of(js(&b)["token"].as_str().unwrap())["exp"].as_u64().unwrap() <= end);
    // a long config gives the 15 minute token
    let (_, v) = open(&r, &config()).await;
    let e = payload_of(v["token"].as_str().unwrap())["exp"].as_u64().unwrap();
    assert!((now() + 890..=now() + 905).contains(&e), "{e}");
    // a session whose absolute end has passed refuses its own, otherwise valid token
    let rec_path = d.join("data/sessions").join(&sid).join("session.json");
    let mut rec: Value = js(&std::fs::read(&rec_path).unwrap());
    rec["expires"] = json!(now() - 5);
    std::fs::write(&rec_path, rec.to_string()).unwrap();
    let (s, _) = call(&r, "POST", &format!("/ds/session/{sid}/renew"), Some(&first_token), b"").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn file_token_bindings() {
    let d = dir("file");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let fr = ds.internal_router();
    let (a, ta) = session(&ds, &r, &config()).await;
    let (b, _) = session(&ds, &r, &config()).await;
    call(&r, "POST", &format!("/ds/session/{a}/save"), Some(&ta), DOC2).await;
    let sha = |sid: &str, n: u64| ds.load_session(sid).unwrap().versions[n as usize - 1].sha256.clone();
    let t = now();
    let get = |sid: String, ver: u64, tok: String| {
        let fr = fr.clone();
        async move { call(&fr, "GET", &format!("/ds/files/{sid}/{ver}?t={tok}"), None, b"").await }
    };
    let good = ds.mint_file_token(&a, 2, &sha(&a, 2), t);
    let (s, body) = get(a.clone(), 2, good.clone()).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, DOC2));
    let (s, body) = get(a.clone(), 1, ds.mint_file_token(&a, 1, &sha(&a, 1), t)).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, DOC));
    // another ver, another sid, the wrong digest: 404; the token of version 1 on version 2
    assert_eq!(get(a.clone(), 1, good.clone()).await.0, StatusCode::NOT_FOUND, "other ver");
    assert_eq!(get(b.clone(), 2, good.clone()).await.0, StatusCode::NOT_FOUND, "other sid");
    assert_eq!(get(a.clone(), 3, good.clone()).await.0, StatusCode::NOT_FOUND, "no such ver");
    let wrong_sha = ds.mint_file_token(&a, 2, &sha(&a, 1), t);
    assert_eq!(get(a.clone(), 2, wrong_sha).await.0, StatusCode::NOT_FOUND, "other digest");
    // not a file token at all: 401
    let sess_tok = ta.clone();
    let expired = sign(&subkey("file"), &json!({"purpose": "file", "sid": a, "ver": 2, "sha256": sha(&a, 2), "exp": t - 120}));
    let long = sign(&subkey("file"), &json!({"purpose": "file", "sid": a, "ver": 2, "sha256": sha(&a, 2), "exp": t + 3600}));
    let session_purpose = sign(&subkey("file"), &json!({"purpose": "session", "sid": a, "ver": 2, "sha256": sha(&a, 2), "exp": t + 60}));
    let wrong_key = sign(&subkey("session"), &json!({"purpose": "file", "sid": a, "ver": 2, "sha256": sha(&a, 2), "exp": t + 60}));
    let provider_key = sign(PROV_SECRET.as_bytes(), &json!({"purpose": "file", "sid": a, "ver": 2, "sha256": sha(&a, 2), "exp": t + 60}));
    for (what, tok) in [("session token", sess_tok), ("expired", expired), ("10 min exceeded", long),
        ("purpose session", session_purpose), ("session key", wrong_key), ("provider key", provider_key), ("garbage", "x".into())] {
        assert_eq!(get(a.clone(), 2, tok).await.0, StatusCode::UNAUTHORIZED, "{what}");
    }
    let (s, _) = call(&fr, "GET", &format!("/ds/files/{a}/2"), None, b"").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "no t");
    // the file token is no session token
    let (s, _) = call(&r, "GET", &format!("/ds/session/{a}/document"), Some(&good), b"").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // the token lives 10 minutes
    let e = payload_of(&good)["exp"].as_u64().unwrap();
    assert_eq!(e, t + 600);
}

#[tokio::test]
async fn sessions_survive_a_restart() {
    let d = dir("restart");
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, &config()).await;
    let (s, _) = call(&r, "POST", &format!("/ds/session/{sid}/save"), Some(&tok), DOC2).await;
    assert_eq!(s, StatusCode::OK);
    drop((r, ds));
    // a new process: nothing but the data folder and the same configuration
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (s, body) = call(&r, "GET", &format!("/ds/session/{sid}/document"), Some(&tok), b"").await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, DOC2), "the saved version is there");
    let (s, _) = call(&r, "POST", &format!("/ds/session/{sid}/renew"), Some(&tok), b"").await;
    assert_eq!(s, StatusCode::OK);
    let rec = ds.load_session(&sid).unwrap();
    let on_disk = std::fs::read_to_string(d.join("data/sessions").join(&sid).join("session.json")).unwrap();
    for secret in [tok.as_str(), PROV_SECRET, SERVER_SECRET] {
        assert!(!on_disk.contains(secret), "a token or secret is on disk");
    }
    assert_eq!((rec.versions.len(), rec.saved_since_open), (2, true));
    let (s, body) = call(&r, "POST", &format!("/ds/session/{sid}/close"), Some(&tok), b"").await;
    assert_eq!((s, js(&body)["status"].as_u64()), (StatusCode::OK, Some(2)), "close after the restart is a status 2");
    // another restart: the close is remembered
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (s, body) = call(&r, "POST", &format!("/ds/session/{sid}/close"), Some(&tok), b"").await;
    assert_eq!((s, js(&body)["status"].as_u64()), (StatusCode::OK, Some(2)));
    // a session with nothing saved closes as a 4
    let (sid2, tok2) = session(&ds, &r, &config()).await;
    drop(ds);
    let ds = std_ds(&d);
    let r = ds.routes::<()>();
    let (_, body) = call(&r, "POST", &format!("/ds/session/{sid2}/close"), Some(&tok2), b"").await;
    assert_eq!(js(&body)["status"].as_u64(), Some(4));
}

mod hrb {
    use super::*;
    use fake_hrm::{Config, FakeHrm, Kind};
    use std::time::Duration;

    async fn editor_config(h: &FakeHrm, id: u64) -> Value {
        let url = format!("{}/files/{id}/editor-config", h.base_url());
        tokio::task::spawn_blocking(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .proxy(None)
                .timeout_global(Some(Duration::from_secs(20)))
                .build()
                .into();
            serde_json::from_slice(&agent.get(&url).call().unwrap().body_mut().read_to_vec().unwrap()).unwrap()
        })
        .await
        .unwrap()
    }

    async fn fetch(url: String) -> Vec<u8> {
        tokio::task::spawn_blocking(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder().proxy(None).build().into();
            agent.get(&url).call().unwrap().body_mut().read_to_vec().unwrap()
        })
        .await
        .unwrap()
    }

    async fn start(include_exp: bool) -> FakeHrm {
        let mut c = Config::generated("http://127.0.0.1:1");
        c.include_exp = include_exp;
        let h = FakeHrm::start(c).await.unwrap();
        h.add_file(Kind::File, 7, "report.docx", DOC.to_vec());
        h
    }

    #[tokio::test]
    async fn an_hrb_config_token_opens_a_session() {
        let h = start(true).await;
        let base = h.base_url().to_string();
        let d = dir("hrb");
        let ds = build(&d, &h.config().ds_secret, true, &[&base], &[&base]);
        let r = ds.routes::<()>();
        let cfg = editor_config(&h, 7).await;
        assert!(cfg["token"].is_string());
        let (s, b) = call(&r, "POST", "/ds/session", None, json!({"origin": "https://app.test", "config": cfg}).to_string().as_bytes()).await;
        let v = js(&b);
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!((v["mode"].as_str(), v["editor"].as_str(), v["title"].as_str()), (Some("edit"), Some("docs"), Some("report.docx")));
        assert_eq!(v["perms"]["edit"], json!(true));
        let rec = ds.load_session(v["sid"].as_str().unwrap()).unwrap();
        assert!(rec.key.starts_with("file_7_"), "{}", rec.key);
        assert_eq!(rec.user_id, "emp-1");
        assert!(rec.expires <= now() + 12 * 3600 + 5, "the session ends with the token's exp");
        // WP4 will do this fetch; here the document URL of the signed config is followed once to see it is the real file route
        let bytes = fetch(rec.document_url.clone()).await;
        assert_eq!(bytes, DOC);
        ds.put_version(&rec.sid, &bytes, "one", false, now()).unwrap();
        let tok = v["token"].as_str().unwrap();
        let (s, body) = call(&r, "GET", &format!("/ds/session/{}/document", rec.sid), Some(tok), b"").await;
        assert_eq!((s, body), (StatusCode::OK, DOC.to_vec()));
        // a view record of the same backend
        h.set_access(Kind::File, 7, false, false);
        let cfg = editor_config(&h, 7).await;
        assert!(cfg["editorConfig"].get("callbackUrl").is_none());
        let (s, b) = call(&r, "POST", "/ds/session", None, json!({"origin": "https://app.test", "config": cfg}).to_string().as_bytes()).await;
        assert_eq!((s, js(&b)["perms"]["edit"].as_bool()), (StatusCode::OK, Some(false)));
    }

    #[tokio::test]
    async fn an_hrb_token_without_exp_is_refused_unless_the_provider_allows_it() {
        let h = start(false).await; // as surveyed: no exp (F1)
        let base = h.base_url().to_string();
        let cfg = editor_config(&h, 7).await;
        let body = json!({"origin": "https://app.test", "config": cfg}).to_string();
        let strict = build(&dir("hrb-strict"), &h.config().ds_secret, true, &[&base], &[&base]);
        let (s, _) = call(&strict.routes::<()>(), "POST", "/ds/session", None, body.as_bytes()).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let lax = build(&dir("hrb-lax"), &h.config().ds_secret, false, &[&base], &[&base]);
        let (s, b) = call(&lax.routes::<()>(), "POST", "/ds/session", None, body.as_bytes()).await;
        assert_eq!(s, StatusCode::OK);
        // no exp: the session still ends, after 12 h, and so does its token
        let rec = lax.load_session(js(&b)["sid"].as_str().unwrap()).unwrap();
        assert!(rec.expires >= now() + 12 * 3600 - 5 && rec.expires <= now() + 12 * 3600 + 5);
        // a token of another backend is refused
        let other = start(true).await;
        let cfg2 = editor_config(&other, 7).await;
        let (s, _) = call(&strict.routes::<()>(), "POST", "/ds/session", None, json!({"origin": "https://app.test", "config": cfg2}).to_string().as_bytes()).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert!(h.rejected().is_empty() && h.received().is_empty());
    }
}
