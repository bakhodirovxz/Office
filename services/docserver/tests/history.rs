//! WP11 (PLAN phase 5, section 6): the server version store behind the session routes.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";
const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
/// A minimal docx package: saves pass the S9 package check (WP4).
fn docx(tag: &str) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    for (n, b) in [("[Content_Types].xml", "x"), ("word/document.xml", tag)] {
        w.start_file(n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn build(name: &str) -> (Arc<Docserver>, PathBuf) {
    let d = std::env::temp_dir().join(format!("docserver-hist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("server.key"), SERVER_SECRET).unwrap();
    std::fs::write(d.join("prov.key"), PROV_SECRET).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19998"
internal_allow = ["127.0.0.0/8"]
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{{ url = "http://api:8000", internal = true }}]
callback_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
require_exp = true
"#,
        p("server.key"),
        p("data"),
        p("prov.key")
    )
    .parse()
    .unwrap();
    (Arc::new(Docserver::from_table(t, None).unwrap()), d)
}

fn config(key: &str, edit: bool) -> Value {
    json!({"documentType": "word",
        "document": {"fileType": "docx", "key": key, "title": "a.docx", "url": "http://api:8000/f/1",
            "permissions": {"edit": edit, "download": true}},
        "editorConfig": {"mode": "edit", "callbackUrl": "http://api:8000/cb", "user": {"id": "emp-1", "name": "U"}},
        "exp": now() + 3600})
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

fn js(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap_or(Value::Null)
}

/// A session for `key` with its document stored as version 1 (what WP4's fetch will do).
async fn session(ds: &Arc<Docserver>, r: &Router, key: &str, edit: bool) -> (String, String) {
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), &config(key, edit))}});
    let (s, b) = call(r, "POST", "/ds/session", None, body.to_string().as_bytes()).await;
    assert_eq!(s, StatusCode::OK);
    let v = js(&b);
    let sid = v["sid"].as_str().unwrap().to_string();
    ds.put_version(&sid, &docx("history v1"), "provider", false, now())
        .unwrap();
    (sid, v["token"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn saves_list_restore_and_name_over_the_routes() {
    let (ds, d) = build("flow");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (s, _) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/save"),
        Some(&tok),
        &docx("history v2"),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let h = format!("/ds/session/{sid}/history");
    let (s, b) = call(&r, "GET", &h, Some(&tok), b"").await;
    let vs = js(&b)["versions"].as_array().unwrap().clone();
    assert_eq!(
        (s, vs.len(), vs[1]["author"].as_str()),
        (StatusCode::OK, 2, Some("emp-1"))
    );
    assert_eq!(
        call(&r, "GET", &format!("{h}/1"), Some(&tok), b"").await.1,
        docx("history v1")
    );

    // restore: always a new version, in the store and in the session
    let (s, b) = call(&r, "POST", &format!("{h}/1/restore"), Some(&tok), b"").await;
    let j = js(&b);
    assert_eq!(
        (
            s,
            j["version"]["v"].as_u64(),
            j["version"]["restored_from"].as_u64()
        ),
        (StatusCode::OK, Some(3), Some(1))
    );
    assert_eq!(j["session_version"].as_u64(), Some(3));
    let (_, b) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/document"),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(b, docx("history v1"));
    assert_eq!(
        js(&call(&r, "GET", &h, Some(&tok), b"").await.1)["versions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let (s, _) = call(
        &r,
        "POST",
        &format!("{h}/2/name"),
        Some(&tok),
        br#"{"name":"signed"}"#,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = call(&r, "GET", &h, Some(&tok), b"").await;
    assert_eq!(js(&b)["versions"][1]["name"].as_str(), Some("signed"));
    assert_eq!(
        call(&r, "GET", &format!("{h}/99"), Some(&tok), b"").await.0,
        StatusCode::NOT_FOUND
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn history_routes_check_the_session_token_and_permissions() {
    let (ds, d) = build("auth");
    let r = ds.routes::<()>();
    let (sid, tok) = session(&ds, &r, "k1", true).await;
    let (sid2, tok2) = session(&ds, &r, "k2", false).await;
    let h = format!("/ds/session/{sid}/history");
    assert_eq!(
        call(&r, "GET", &h, None, b"").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&r, "GET", &h, Some("junk"), b"").await.0,
        StatusCode::UNAUTHORIZED
    );
    // another session's token: the same 404 as a session that does not exist
    let other = call(&r, "GET", &h, Some(&tok2), b"").await;
    let none = call(
        &r,
        "GET",
        &format!("/ds/session/{}/history", "0".repeat(32)),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(
        (other.0, none.0),
        (StatusCode::NOT_FOUND, StatusCode::NOT_FOUND)
    );
    assert_eq!(other.1, none.1);
    // a view session reads but cannot restore or name
    let h2 = format!("/ds/session/{sid2}/history");
    assert_eq!(
        call(&r, "GET", &h2, Some(&tok2), b"").await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&r, "POST", &format!("{h2}/1/restore"), Some(&tok2), b"")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &r,
            "POST",
            &format!("{h2}/1/name"),
            Some(&tok2),
            br#"{"name":"x"}"#
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // histories of different documents do not mix
    assert_eq!(
        js(&call(&r, "GET", &h, Some(&tok), b"").await.1)["versions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn a_rewritten_server_history_fails_verification() {
    let (ds, d) = build("chain");
    let r = ds.routes::<()>();
    let (_sid, _tok) = session(&ds, &r, "k1", true).await;
    let idx = std::fs::read_dir(d.join("data/history/docs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&idx).unwrap();
    std::fs::write(&idx, text.replace("provider", "mallory")).unwrap();
    let (sid, tok) = session(&ds, &r, "k1", true).await; // the damaged chain refuses a new append
    let _ = sid;
    let (s, _) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/history"),
        Some(&tok),
        b"",
    )
    .await;
    assert_eq!(
        s,
        StatusCode::INTERNAL_SERVER_ERROR,
        "a history that does not verify is not served"
    );
    let _ = std::fs::remove_dir_all(d);
}
