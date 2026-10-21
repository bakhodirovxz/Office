//! Server mode transport (PLAN 5.5): HTTPS only, TLS 1.3 and TLS 1.2 with AEAD suites, HSTS, and a plain-HTTP listener that only redirects.
use axum::body::Body;
use axum::http::Request;
use convert::exec::Config as ConvertConfig;
use convert::plan::ToolId;
use convert_api::app::App;
use convert_api::config::Settings;
use convert_api::{routes, tls};
use rustls::pki_types::ServerName;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use tower::ServiceExt;

/// A blocking HTTPS client pinned to some protocol versions: returns (response head, protocol version, cipher suite).
fn fetch(port: u16, cert_der: Vec<u8>, tls12: bool, path: &str) -> (String, String, String) {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert_der.into()).unwrap();
    let v = if tls12 { vec![&rustls::version::TLS12] } else { vec![&rustls::version::TLS13] };
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&v)
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut conn = rustls::ClientConnection::new(Arc::new(cfg), ServerName::try_from("localhost").unwrap()).unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut s = rustls::Stream::new(&mut conn, &mut tcp);
    write!(s, "GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n").unwrap();
    let mut body = Vec::new();
    let _ = s.read_to_end(&mut body); // the server closes without close_notify at times
    let ver = format!("{:?}", conn.protocol_version().unwrap());
    let suite = format!("{:?}", conn.negotiated_cipher_suite().unwrap().suite());
    (String::from_utf8_lossy(&body).into_owned(), ver, suite)
}

#[test]
fn every_offered_suite_is_aead() {
    let suites: Vec<String> = tls::provider().cipher_suites.iter().map(|s| format!("{:?}", s.suite())).collect();
    assert!(suites.len() >= 5, "{suites:?}");
    for s in &suites {
        assert!(s.contains("_GCM_") || s.contains("CHACHA20_POLY1305"), "{s}");
        assert!(!s.contains("CBC") && !s.contains("RC4") && !s.contains("3DES"), "{s}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn server_mode_speaks_tls13_and_tls12_aead_with_hsts_and_never_plain_http() {
    let dir = std::env::temp_dir().join(format!("capi-tls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(dir.join("cert.pem"), ck.cert.pem()).unwrap();
    std::fs::write(dir.join("key.pem"), ck.key_pair.serialize_pem()).unwrap();
    std::fs::write(dir.join("secret"), [b's'; 40]).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let s = Settings {
        auth: "jwt".into(),
        listen: format!("127.0.0.1:{port}"),
        secret_file: Some(dir.join("secret")),
        tls_cert: Some(dir.join("cert.pem")),
        tls_key: Some(dir.join("key.pem")),
        public_url: Some(format!("https://localhost:{port}")),
        work_dir: Some(dir.join("work")),
        store_dir: Some(dir.join("store")),
        ..Settings::default()
    };
    let cfg = tls::server_config(&dir.join("cert.pem"), &dir.join("key.pem")).unwrap();
    let mut conv = ConvertConfig::from_env();
    conv.tools.insert(ToolId::Docs, env!("CARGO_BIN_EXE_convert-api-fake-tool").into());
    let app = App::new(s.resolve().unwrap(), conv, true).unwrap();
    let router = routes::router(app.clone());
    let limits = tls::ConnLimits::from_settings(&app.r);
    tokio::spawn(tls::serve(tokio::net::TcpListener::from_std(listener).unwrap(), Some(cfg), router, limits, std::future::pending()));
    let der = ck.cert.der().to_vec();
    let go = |tls12: bool, path: &'static str| {
        let der = der.clone();
        tokio::task::spawn_blocking(move || fetch(port, der, tls12, path))
    };
    let (head, ver, suite) = go(false, "/healthz").await.unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(ver, "TLSv1_3", "{suite}");
    let low = head.to_ascii_lowercase();
    assert!(low.contains("strict-transport-security: max-age=31536000"), "{head}");
    assert!(low.contains("default-src 'self'"), "{head}");
    let (head, ver, suite) = go(true, "/healthz").await.unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(ver, "TLSv1_2");
    assert!(suite.starts_with("TLS_ECDHE_") && (suite.contains("_GCM_") || suite.contains("CHACHA20_POLY1305")), "{suite}");
    // the API needs a token over TLS as well
    let (head, _, _) = go(false, "/api/v1/formats").await.unwrap();
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    // plain HTTP on the TLS port gets no HTTP answer
    let plain = tokio::task::spawn_blocking(move || {
        let mut t = TcpStream::connect(("127.0.0.1", port)).unwrap();
        t.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let _ = t.write_all(format!("GET /healthz HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").as_bytes());
        let mut b = vec![];
        let _ = t.read_to_end(&mut b);
        String::from_utf8_lossy(&b).into_owned()
    })
    .await
    .unwrap();
    assert!(!plain.contains("HTTP/1.1 200") && !plain.contains("convert") && !plain.contains("tools"), "{plain:?}");
    app.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_plain_http_listener_only_redirects_to_the_configured_url() {
    let r = tls::redirect_router("https://convert.test/");
    let mut req = Request::builder().uri("/api/v1/jobs?x=1").header("host", "evil.example").body(Body::empty()).unwrap();
    req.extensions_mut().insert(axum::extract::ConnectInfo("203.0.113.9:1".parse::<SocketAddr>().unwrap()));
    let res = r.oneshot(req).await.unwrap();
    assert_eq!(res.status(), 308);
    assert_eq!(res.headers()["location"], "https://convert.test/api/v1/jobs?x=1");
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    assert!(res.headers()["content-security-policy"].to_str().unwrap().contains("frame-ancestors 'none'"));
}
