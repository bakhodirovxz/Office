//! The ONLYOFFICE drop-in (docs/phase5/dropin.md): the HR backend harness exactly as surveyed (no `exp`, the callback token in the
//! body only, a host-name allowlist and the host rewrite of the saved-file URL) against a provider with `compat = "docsapi"`, an
//! internal listener reached by a host name, an `internal_allow` that names the backend instead of its address, and `edit_types`.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use docserver::Docserver;
use fake_hrm::{Config, FakeHrm, Kind};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;

fn package(main: &str, tag: &str) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    for (n, b) in [("[Content_Types].xml", "x"), (main, tag)] {
        w.start_file(n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

async fn call(
    r: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::from(body)).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    let res = r.clone().oneshot(req).await.unwrap();
    let s = res.status();
    let b = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_backend_as_surveyed_saves_through_the_host_rewrite() {
    // our internal listener; the backend knows it only as `localhost` (the old container's alias, here a name that resolves)
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let rewrite = format!("http://localhost:{port}");
    let mut c = Config::generated(&rewrite); // BACKEND_DOCS_URL
    assert!(
        !c.include_exp && !c.bearer_fallback,
        "the harness defaults are HRB as surveyed"
    );
    c.callback_allowed_hosts = vec!["localhost".into()]; // host name only, any port
    c.ds_secret = "twenty-byte-secret!!".into(); // shorter than our own 32-byte rule
    let hrm = FakeHrm::start(c).await.unwrap();
    hrm.add_file(
        Kind::File,
        7,
        "report.docx",
        package("word/document.xml", "v1"),
    );
    hrm.add_file(
        Kind::File,
        8,
        "sheet.xlsx",
        package("xl/workbook.xml", "s1"),
    );

    let d = std::env::temp_dir().join(format!("docserver-dropin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("server.key"), [b's'; 40]).unwrap();
    std::fs::write(d.join("prov.key"), hrm.config().ds_secret).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:{port}"
internal_allow = ["localhost"]
internal_url = "{rewrite}"
[[providers]]
name = "docsapi"
issuer = "docsapi"
compat = "docsapi"
edit_types = ["docx"]
secret_file = "{}"
fetch_allow = [{{ url = "{b}", internal = true }}]
callback_allow = [{{ url = "{b}", internal = true }}]
frame_origins = ["https://app.test"]
"#,
        p("server.key"),
        p("data"),
        p("prov.key"),
        b = hrm.base_url()
    )
    .parse()
    .unwrap();
    let ds = Arc::new(Docserver::from_table(t, None).unwrap());
    let app = ds
        .internal_router()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        let _ = axum::serve(l, app).await;
    });
    let r = ds.routes::<()>();

    let open = async |id: u64| {
        let url = format!("{}/files/{id}/editor-config", hrm.base_url());
        let cfg: Value = tokio::task::spawn_blocking(move || {
            let a: ureq::Agent = ureq::Agent::config_builder().proxy(None).build().into();
            serde_json::from_slice(
                &a.get(&url)
                    .call()
                    .unwrap()
                    .body_mut()
                    .read_to_vec()
                    .unwrap(),
            )
            .unwrap()
        })
        .await
        .unwrap();
        let (s, v) = call(
            &r,
            "POST",
            "/ds/session",
            None,
            json!({"origin": "https://app.test", "config": cfg})
                .to_string()
                .into_bytes(),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        (
            v["sid"].as_str().unwrap().to_string(),
            v["token"].as_str().unwrap().to_string(),
            v["mode"].as_str().unwrap().to_string(),
        )
    };

    // a config token without exp opens an edit session
    let (sid, tok, mode) = open(7).await;
    assert_eq!(mode, "edit");
    // nothing may be saved before the backend's document was fetched (a replayed token cannot push bytes as the document)
    for route in ["save", "autosave", "dirty"] {
        let (s, v) = call(
            &r,
            "POST",
            &format!("/ds/session/{sid}/{route}"),
            Some(&tok),
            package("word/document.xml", "x"),
        )
        .await;
        assert_eq!(
            (s, v["error"]["code"].as_str()),
            (StatusCode::CONFLICT, Some("no_document")),
            "{route} {v}"
        );
    }
    let (s, _) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/document"),
        Some(&tok),
        vec![],
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // save: status 6; the backend checks the url host by name, rewrites it to its BACKEND_DOCS_URL and downloads from us
    let v2 = package("word/document.xml", "v2");
    let (s, v) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/save"),
        Some(&tok),
        v2.clone(),
    )
    .await;
    assert_eq!(
        (s, v["callback"].as_str()),
        (StatusCode::OK, Some("ok")),
        "{v} {:?}",
        hrm.rejected()
    );
    let (s, v) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/close"),
        Some(&tok),
        vec![],
    )
    .await;
    assert_eq!(
        (s, v["status"].as_u64(), v["callback"].as_str()),
        (StatusCode::OK, Some(2), Some("ok")),
        "{v}"
    );
    let got = hrm.received();
    assert_eq!(got.iter().map(|g| g.status).collect::<Vec<_>>(), [6, 2]);
    assert!(
        got.iter().all(|g| g.via == fake_hrm::Via::Body),
        "the body token is the one HRB reads"
    );
    let want: String = Sha256::digest(&v2)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(got[1].sha256, want, "status 2 carries the saved bytes");
    assert_eq!(hrm.object(Kind::File, 7), Some(v2));
    assert!(hrm.rejected().is_empty(), "{:?}", hrm.rejected());

    // an xlsx edit config opens as a view (edit_types = ["docx"]): its document is shown, nothing reaches the backend
    let (sid, tok, mode) = open(8).await;
    assert_eq!(mode, "view");
    let (s, _) = call(
        &r,
        "GET",
        &format!("/ds/session/{sid}/document"),
        Some(&tok),
        vec![],
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/save"),
        Some(&tok),
        package("xl/workbook.xml", "s2"),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(hrm.received().len(), 2);

    // the internal listener answers the backend's name only: from an address that is not `localhost` the gate refuses
    assert!(ds.internal_allows("127.0.0.1".parse().unwrap()));
    assert!(!ds.internal_allows("10.9.8.7".parse().unwrap()));
    let _ = std::fs::remove_dir_all(&d);
}
