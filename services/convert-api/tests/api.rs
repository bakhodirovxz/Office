//! WP7 tests: the router in-process (tower `oneshot`) against a stand-in for the domain CLIs, so every security rule is tested
//! without the real engines. `tests/e2e.rs` runs the binary against the real tools.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use convert::exec::Config as ConvertConfig;
use convert::plan::ToolId;
use convert_api::app::App;
use convert_api::config::Settings;
use convert_api::jwt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef-secret";

struct H {
    dir: PathBuf,
    app: Arc<App>,
    router: Router,
    host: &'static str,
}

impl Drop for H {
    fn drop(&mut self) {
        self.app.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn sandbox(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("capi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fake_cfg() -> ConvertConfig {
    let mut c = ConvertConfig::from_env();
    for t in ToolId::ALL {
        c.tools
            .insert(t, env!("CARGO_BIN_EXE_convert-api-fake-tool").into());
    }
    c.sample_ms = 50;
    c
}

impl H {
    fn local(name: &str, f: impl FnOnce(&mut Settings)) -> H {
        let dir = sandbox(name);
        // the store shares the work directory here so that `work_entries` sees everything the server keeps
        let mut s = Settings {
            work_dir: Some(dir.join("work")),
            store_dir: Some(dir.join("work")),
            ..Settings::default()
        };
        f(&mut s);
        H::build(dir, s, "127.0.0.1:8787", false)
    }

    fn server(name: &str, f: impl FnOnce(&mut Settings)) -> H {
        let dir = sandbox(name);
        std::fs::write(dir.join("secret"), SECRET).unwrap();
        let mut s = Settings {
            auth: "jwt".into(),
            secret_file: Some(dir.join("secret")),
            tls_cert: Some(dir.join("cert.pem")),
            tls_key: Some(dir.join("key.pem")),
            public_url: Some("https://convert.test".into()),
            work_dir: Some(dir.join("work")),
            store_dir: Some(dir.join("work")),
            ..Settings::default()
        };
        f(&mut s);
        H::build(dir, s, "convert.test", true)
    }

    fn build(dir: PathBuf, s: Settings, host: &'static str, tls: bool) -> H {
        let app = App::new(s.resolve().unwrap(), fake_cfg(), tls).unwrap();
        H {
            dir,
            router: convert_api::routes::router(app.clone()),
            app,
            host,
        }
    }

    fn req(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        ct: Option<&str>,
        body: Vec<u8>,
    ) -> Request<Body> {
        let mut r = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", self.host);
        if let Some(t) = token {
            r = r.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = ct {
            r = r
                .header("content-type", c)
                .header("content-length", body.len());
        }
        let mut r = r.body(Body::from(body)).unwrap();
        r.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:5555".parse::<SocketAddr>().unwrap(),
        ));
        r
    }

    async fn send(&self, r: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let res = self.router.clone().oneshot(r).await.unwrap();
        let (st, h) = (res.status(), res.headers().clone());
        (
            st,
            h,
            axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
    }

    async fn json(&self, method: &str, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let (st, _, b) = self.send(self.req(method, uri, token, None, vec![])).await;
        (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    async fn post(
        &self,
        files: &[(&str, &[u8])],
        options: Value,
        token: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let (ct, body) = multipart(files, &options.to_string());
        let (st, h, b) = self
            .send(self.req("POST", "/api/v1/jobs", token, Some(&ct), body))
            .await;
        (st, h, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    async fn wait_done(&self, id: &str, token: Option<&str>) -> Value {
        let t = Instant::now();
        loop {
            let (st, v) = self.json("GET", &format!("/api/v1/jobs/{id}"), token).await;
            assert_eq!(st, StatusCode::OK, "{v}");
            if v["state"] == "done" {
                return v;
            }
            assert!(
                t.elapsed() < Duration::from_secs(60),
                "job never finished: {v}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn token(&self, sub: &str) -> String {
        jwt::sign(
            SECRET,
            &json!({"sub": sub, "exp": jwt::now() + 600, "scope": "convert"}),
        )
    }

    fn work_entries(&self) -> Vec<String> {
        std::fs::read_dir(self.dir.join("work"))
            .map(|r| {
                r.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn multipart(files: &[(&str, &[u8])], options: &str) -> (String, Vec<u8>) {
    let b = "----capi-test-boundary";
    let mut body = Vec::new();
    for (name, data) in files {
        body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n--{b}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={b}"), body)
}

const TXT: &[u8] = "Salom dunyo, bu oddiy matn.\n".as_bytes();

// ---- local mode ------------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn local_mode_works_end_to_end_with_no_token() {
    let h = H::local("local", |_| {});
    let (st, hd, v) = h
        .post(&[("note.txt", TXT)], json!({"to": "pdf"}), None)
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let id = v["job"].as_str().unwrap().to_string();
    assert_eq!(hd["location"], format!("/api/v1/jobs/{id}").as_str());
    assert_eq!(v["files"][0]["kind"], "txt");
    let done = h.wait_done(&id, None).await;
    assert_eq!(done["progress"], json!({"done": 1, "total": 1}));
    assert_eq!(done["files"][0]["state"], "done");
    assert_eq!(done["files"][0]["outputs"][0]["name"], "note.pdf");
    let (st, hd, body) = h
        .send(h.req(
            "GET",
            &format!("/api/v1/jobs/{id}/files/0/0"),
            None,
            None,
            vec![],
        ))
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.starts_with(b"%PDF-"));
    assert_eq!(hd["content-type"], "application/pdf");
    assert!(
        hd["content-disposition"]
            .to_str()
            .unwrap()
            .contains("filename*=UTF-8''note.pdf")
    );
    // the zip: the output plus report.json, and the report carries ids and names, never a server path
    let (st, _, zip) = h
        .send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![]))
        .await;
    assert_eq!(st, StatusCode::OK);
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert_eq!(names, ["note.pdf", "report.json"]);
    let mut rep = String::new();
    std::io::Read::read_to_string(&mut z.by_name("report.json").unwrap(), &mut rep).unwrap();
    let rep: Value = serde_json::from_str(&rep).unwrap();
    assert_eq!(rep[0]["input"], "note.txt");
    assert_eq!(rep[0]["outputs"], json!(["note.pdf"]));
    for needle in [
        h.dir.to_str().unwrap().replace('\\', "/"),
        h.dir.to_str().unwrap().replace('/', "\\"),
    ] {
        assert!(!rep.to_string().contains(&needle) && !done.to_string().contains(&needle));
    }
    // delete removes it now
    assert_eq!(
        h.json("DELETE", &format!("/api/v1/jobs/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(h.work_entries().is_empty(), "{:?}", h.work_entries());
}

#[tokio::test(flavor = "multi_thread")]
async fn formats_lists_the_matrix_and_the_limits() {
    let h = H::local("formats", |_| {});
    let (st, v) = h.json("GET", "/api/v1/formats", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["rows"].as_array().unwrap().len() > 50);
    assert_eq!(v["limits"]["maxBatchFiles"], 30);
    let (st, v) = h.json("GET", "/healthz", None).await;
    assert_eq!(
        (st, v["ok"].clone(), v["tools"]["docs-cli"].clone()),
        (StatusCode::OK, json!(true), json!(true))
    );
}

// ---- names -----------------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn hostile_file_names_become_plain_names_and_never_reach_the_disk_as_paths() {
    let h = H::local("names", |_| {});
    let cases: [(&str, &str); 4] = [
        ("..\\..\\x.txt", "_.._x.txt"),
        ("../../etc/x.txt", "_.._etc_x.txt"),
        ("NUL.txt", "_NUL.txt"),
        ("a\u{202E}fdp.txt", "a_fdp.txt"),
    ];
    for (raw, want) in cases {
        let (st, _, v) = h.post(&[(raw, TXT)], json!({"to": "pdf"}), None).await;
        assert_eq!(st, StatusCode::ACCEPTED, "{raw}: {v}");
        assert_eq!(v["files"][0]["name"], want, "{raw}");
        let id = v["job"].as_str().unwrap();
        let done = h.wait_done(id, None).await;
        let out = done["files"][0]["outputs"][0]["name"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(done["files"][0]["state"], "done", "{raw}: {done}");
        assert_eq!(out, format!("{}.pdf", want.trim_end_matches(".txt")));
        let (_, hd, body) = h
            .send(h.req(
                "GET",
                &format!("/api/v1/jobs/{id}/files/0/0"),
                None,
                None,
                vec![],
            ))
            .await;
        assert!(body.starts_with(b"%PDF-"));
        let cd = hd["content-disposition"].to_str().unwrap();
        assert!(
            cd.is_ascii() && !cd.contains(['/', '\\']) && !cd.contains("%E2%80%AE"),
            "{cd}"
        );
    }
    // nothing escaped the sandbox: the only things next to `work` are the sandbox's own
    let stray: Vec<_> = std::fs::read_dir(&h.dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(stray, ["work"]);
    // two uploads with the same name give two outputs with different names in the zip
    let (_, _, v) = h
        .post(
            &[("a.txt", TXT), ("a.txt", TXT)],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    let id = v["job"].as_str().unwrap();
    h.wait_done(id, None).await;
    let (_, _, zip) = h
        .send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![]))
        .await;
    let z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["a (2).pdf", "a.pdf", "report.json"]);
}

// ---- zip bombs, size, the batch --------------------------------------------------------------------------------------

/// The overlapping-entries bomb (the small-file/huge-expansion kind): one 9 MB part of zeros, and 499 more central-directory
/// records that point at the same compressed data. About 60 KB on disk, 4.5 GB declared, and no single part is over the ratio limit.
fn overlap_bomb() -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("word/document.xml", o).unwrap();
    for _ in 0..9 {
        w.write_all(&vec![0u8; 1 << 20]).unwrap();
    }
    let mut b = w.finish().unwrap().into_inner();
    let eocd = b.len() - 22;
    assert_eq!(&b[eocd..eocd + 4], b"PK");
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    let (cd_off, cd_size) = (
        u32_at(&b, eocd + 16) as usize,
        u32_at(&b, eocd + 12) as usize,
    );
    let first = b[cd_off..cd_off + cd_size].to_vec();
    assert_eq!(
        u16::from_le_bytes([first[28], first[29]]),
        17,
        "the name is word/document.xml"
    );
    let mut more = vec![];
    for i in 0..499 {
        let mut r = first.clone();
        r[46..46 + 17].copy_from_slice(format!("word/docu{i:04}.xml").as_bytes());
        more.extend_from_slice(&r);
    }
    b.truncate(eocd);
    b.extend_from_slice(&more);
    let mut tail = vec![0u8; 22];
    tail.copy_from_slice(&[b"PK".as_slice(), &[0; 18]].concat());
    tail[8..10].copy_from_slice(&500u16.to_le_bytes());
    tail[10..12].copy_from_slice(&500u16.to_le_bytes());
    tail[12..16].copy_from_slice(&((cd_size + more.len()) as u32).to_le_bytes());
    tail[16..20].copy_from_slice(&(cd_off as u32).to_le_bytes());
    b.extend_from_slice(&tail);
    b
}

/// One 12 MB part of zeros: over the 10 MB mark with a ratio near 1000:1.
fn ratio_bomb() -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    for _ in 0..12 {
        w.write_all(&vec![0u8; 1 << 20]).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[tokio::test(flavor = "multi_thread")]
async fn zip_bombs_are_rejected_per_file_and_the_rest_of_the_batch_runs() {
    let h = H::local("bomb", |_| {});
    let (overlap, ratio) = (overlap_bomb(), ratio_bomb());
    assert!(
        overlap.len() < 100_000,
        "a small file: {} bytes for 4.5 GB declared",
        overlap.len()
    );
    let t = Instant::now();
    let (st, _, v) = h
        .post(
            &[
                ("overlap.docx", &overlap),
                ("good.txt", TXT),
                ("ratio.docx", &ratio),
            ],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    eprintln!(
        "bombs: {} and {} bytes, rejected in {:?}",
        overlap.len(),
        ratio.len(),
        t.elapsed()
    );
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    for n in [0, 2] {
        assert_eq!(v["files"][n]["error"]["code"], 9, "{v}");
    }
    assert!(
        v["files"][2]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("zip bomb"),
        "{v}"
    );
    assert!(v["files"][1].get("error").is_none());
    let done = h.wait_done(v["job"].as_str().unwrap(), None).await;
    assert_eq!(
        (
            done["files"][0]["state"].clone(),
            done["files"][0]["code"].clone()
        ),
        (json!("failed"), json!(9))
    );
    assert_eq!(done["files"][1]["state"], "done");
    assert_eq!(done["files"][2]["code"], 9);
}

#[tokio::test(flavor = "multi_thread")]
async fn oversize_uploads_are_413_and_leave_nothing_behind() {
    let h = H::local("oversize", |s| {
        s.max_file_mb = 1;
        s.max_batch_mb = 3;
    });
    let two_mb = vec![b'a'; 2 << 20];
    let (st, _, v) = h
        .post(&[("big.txt", &two_mb)], json!({"to": "pdf"}), None)
        .await;
    assert_eq!(
        (st, v["error"]["code"].clone()),
        (StatusCode::PAYLOAD_TOO_LARGE, json!("file_too_large"))
    );
    // a body over the request limit is refused from its Content-Length, before anything is parsed
    let five_mb = vec![b'a'; 5 << 20];
    let (ct, body) = multipart(&[("a.txt", &five_mb)], "{}");
    let (st, _, _) = h
        .send(h.req("POST", "/api/v1/jobs", None, Some(&ct), body))
        .await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    // many small files add up to more than the batch
    let one = vec![b'a'; 900 * 1024];
    let (st, _, v) = h
        .post(
            &[
                ("1.txt", &one),
                ("2.txt", &one),
                ("3.txt", &one),
                ("4.txt", &one),
            ],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    assert_eq!(
        (st, v["error"]["code"].clone()),
        (StatusCode::PAYLOAD_TOO_LARGE, json!("batch_too_large"))
    );
    assert_eq!(h.app.queue_depth(), 0);
    assert!(h.work_entries().is_empty(), "{:?}", h.work_entries());
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_requests_are_400_and_unsupported_directions_are_rejected_per_file() {
    let h = H::local("bad", |_| {});
    assert_eq!(
        h.post(&[("a.txt", TXT)], json!({}), None).await.0,
        StatusCode::BAD_REQUEST
    ); // no `to`
    assert_eq!(
        h.post(&[("a.txt", TXT)], json!({"to": "exe"}), None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.post(&[("a.txt", TXT)], json!({"to": "pdf", "evil": 1}), None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.post(&[], json!({"to": "pdf"}), None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (st, _, v) = h
        .post(
            &[("a.txt", TXT), ("b.bin", &[0u8, 1, 2, 3, 0xFF][..])],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert_eq!(v["files"][1]["error"]["code"], 3);
    assert_eq!(
        h.wait_done(v["job"].as_str().unwrap(), None).await["files"][0]["state"],
        "done"
    );
    // a not-multipart body
    let (st, _, _) = h
        .send(h.req(
            "POST",
            "/api/v1/jobs",
            None,
            Some("application/json"),
            b"{}".to_vec(),
        ))
        .await;
    assert!(st.is_client_error());
}

// ---- auth ------------------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn tokens_expired_tampered_none_and_wrong_scope_are_401() {
    let h = H::server("auth", |_| {});
    let ok = h.token("alice");
    assert_eq!(
        h.json("GET", "/api/v1/formats", Some(&ok)).await.0,
        StatusCode::OK
    );
    let b64 = |v: Value| base64_url(v.to_string().as_bytes());
    let expired = jwt::sign(
        SECRET,
        &json!({"sub": "alice", "exp": 1, "scope": "convert"}),
    );
    let no_exp = jwt::sign(SECRET, &json!({"sub": "alice", "scope": "convert"}));
    let wrong_scope = jwt::sign(
        SECRET,
        &json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "admin"}),
    );
    let wrong_key = jwt::sign(
        b"another-secret-another-secret-another",
        &json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "convert"}),
    );
    let none = format!(
        "{}.{}.",
        b64(json!({"alg": "none", "typ": "JWT"})),
        b64(json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "convert"}))
    );
    let mut parts: Vec<String> = ok.split('.').map(String::from).collect();
    parts[1] = b64(json!({"sub": "mallory", "exp": jwt::now() + 600, "scope": "convert"}));
    let tampered = parts.join(".");
    for (what, t) in [
        ("expired", &expired),
        ("no exp", &no_exp),
        ("scope", &wrong_scope),
        ("wrong key", &wrong_key),
        ("alg none", &none),
        ("tampered", &tampered),
        ("garbage", &"x.y.z".to_string()),
    ] {
        let (st, hd, _) = h
            .send(h.req("GET", "/api/v1/formats", Some(t), None, vec![]))
            .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{what}");
        assert_eq!(hd["www-authenticate"], "Bearer");
    }
    assert_eq!(
        h.json("GET", "/api/v1/formats", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    // the token is never read from the URL
    assert_eq!(
        h.json("GET", &format!("/api/v1/formats?token={ok}"), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // an issuer rule
    let h2 = H::server("auth-iss", |s| s.iss = Some("idp.example".into()));
    assert_eq!(
        h2.json("GET", "/api/v1/formats", Some(&h2.token("alice")))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let with_iss = jwt::sign(
        SECRET,
        &json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "convert", "iss": "idp.example"}),
    );
    assert_eq!(
        h2.json("GET", "/api/v1/formats", Some(&with_iss)).await.0,
        StatusCode::OK
    );
}

fn base64_url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

#[tokio::test(flavor = "multi_thread")]
async fn another_users_job_is_404() {
    let h = H::server("owner", |_| {});
    let (alice, bob) = (h.token("alice"), h.token("bob"));
    let (st, _, v) = h
        .post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&alice))
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, Some(&alice)).await;
    for (m, p) in [
        ("GET", ""),
        ("GET", "/files/0/0"),
        ("GET", "/zip"),
        ("DELETE", ""),
    ] {
        assert_eq!(
            h.json(m, &format!("/api/v1/jobs/{id}{p}"), Some(&bob))
                .await
                .0,
            StatusCode::NOT_FOUND,
            "{m} {p}"
        );
    }
    // and bob's attempt did not delete it
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), Some(&alice))
            .await
            .0,
        StatusCode::OK
    );
    // an id that is not an id
    assert_eq!(
        h.json("GET", "/api/v1/jobs/..%2F..%2Fetc", Some(&alice))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rate_limits_per_user_and_per_ip_and_failed_logins() {
    let h = H::server("rate", |s| {
        s.rate_user_per_min = 2;
        s.per_user_active = 100;
    });
    let t = h.token("alice");
    for _ in 0..2 {
        assert_eq!(
            h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&t))
                .await
                .0,
            StatusCode::ACCEPTED
        );
    }
    let (st, hd, _) = h
        .post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&t))
        .await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    assert!(hd.contains_key("retry-after"));
    // another user is not affected
    assert_eq!(
        h.post(
            &[("a.txt", TXT)],
            json!({"to": "pdf"}),
            Some(&h.token("bob"))
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    // the IP limit
    let h = H::local("rate-ip", |s| {
        s.rate_ip_per_min = 1;
        s.per_user_active = 100;
    });
    assert_eq!(
        h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None)
            .await
            .0,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None)
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    // failed logins from one address are limited too
    let h = H::server("rate-auth", |_| {});
    let mut last = StatusCode::OK;
    for _ in 0..40 {
        last = h
            .json("GET", "/api/v1/formats", Some("bad.token.here"))
            .await
            .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test(flavor = "multi_thread")]
async fn host_and_origin_checks_stop_rebinding_and_cross_site_posts() {
    let h = H::local("host", |_| {});
    let mut r = h.req("GET", "/api/v1/formats", None, None, vec![]);
    r.headers_mut()
        .insert("host", "evil.example:8787".parse().unwrap());
    assert_eq!(h.send(r).await.0, StatusCode::MISDIRECTED_REQUEST);
    let mut r = h.req("GET", "/api/v1/formats", None, None, vec![]);
    r.headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    assert_eq!(h.send(r).await.0, StatusCode::FORBIDDEN);
    let mut r = h.req("GET", "/api/v1/formats", None, None, vec![]);
    r.headers_mut()
        .insert("origin", "http://127.0.0.1:8787".parse().unwrap());
    assert_eq!(h.send(r).await.0, StatusCode::OK);
    // a preflight gets no CORS headers
    let (_, hd, _) = h
        .send(h.req("OPTIONS", "/api/v1/jobs", None, None, vec![]))
        .await;
    assert!(!hd.keys().any(|k| k.as_str().starts_with("access-control-")));
}

// ---- queue, cancel ---------------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_full_queue_is_503_with_retry_after_and_a_user_is_limited_to_three_active_jobs() {
    let h = H::local("queue", |s| s.queue_max = 2);
    let slow = b"SLEEP".as_slice();
    let mut ids = vec![];
    for _ in 0..2 {
        let (st, _, v) = h.post(&[("s.txt", slow)], json!({"to": "pdf"}), None).await;
        assert_eq!(st, StatusCode::ACCEPTED, "{v}");
        ids.push(v["job"].as_str().unwrap().to_string());
    }
    let (st, hd, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!(
        (st, v["error"]["code"].clone()),
        (StatusCode::SERVICE_UNAVAILABLE, json!("queue_full"))
    );
    assert!(hd.contains_key("retry-after"));
    for id in &ids {
        h.json("DELETE", &format!("/api/v1/jobs/{id}"), None).await;
    }
    let h = H::local("queue-user", |_| {});
    let mut ids = vec![];
    for _ in 0..3 {
        ids.push(
            h.post(&[("s.txt", slow)], json!({"to": "pdf"}), None)
                .await
                .2["job"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let (st, _, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!(
        (st, v["error"]["code"].clone()),
        (StatusCode::TOO_MANY_REQUESTS, json!("too_many_active_jobs"))
    );
    for id in &ids {
        h.json("DELETE", &format!("/api/v1/jobs/{id}"), None).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_that_can_never_fit_the_memory_budget_is_409() {
    let h = H::local("mem", |s| s.mem_budget_mb = 100);
    let (st, _, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!(
        (st, v["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("too_large_for_server"))
    );
    assert_eq!(h.app.queue_depth(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_gate_serialises_jobs_that_do_not_fit_together() {
    // 300 MB budget, each file reserves 256 MB: the two files cannot run at once, yet both finish
    let h = H::local("memgate", |s| s.mem_budget_mb = 300);
    let (st, _, v) = h
        .post(
            &[("a.txt", TXT), ("b.txt", TXT)],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let done = h.wait_done(v["job"].as_str().unwrap(), None).await;
    assert!(
        done["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["state"] == "done"),
        "{done}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_cancels_and_kills_a_running_step() {
    let h = H::local("cancel", |_| {});
    let (_, _, v) = h
        .post(&[("s.txt", b"SLEEP")], json!({"to": "pdf"}), None)
        .await;
    let id = v["job"].as_str().unwrap().to_string();
    let t = Instant::now();
    loop {
        let (_, s) = h.json("GET", &format!("/api/v1/jobs/{id}"), None).await;
        if s["files"][0]["state"] == "running" {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(20), "never started: {s}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // the tool is asleep for 120 s; the job directory must be gone within seconds of the DELETE, which needs the step killed
    tokio::time::sleep(Duration::from_millis(500)).await;
    let t = Instant::now();
    assert_eq!(
        h.json("DELETE", &format!("/api/v1/jobs/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), None).await.0,
        StatusCode::NOT_FOUND
    );
    while !h.work_entries().is_empty() {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "left: {:?}",
            h.work_entries()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    eprintln!("cancel to clean work dir: {:?}", t.elapsed());
    assert!(t.elapsed() < Duration::from_secs(15));
    assert_eq!(h.app.queue_depth(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sweeper_removes_finished_jobs_after_retention() {
    let h = H::local("sweep", |s| s.retention_s = 1);
    let (_, _, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, None).await;
    h.app.sweep();
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), None).await.0,
        StatusCode::OK,
        "kept inside the retention time"
    );
    tokio::time::sleep(Duration::from_millis(1200)).await;
    h.app.sweep();
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(h.work_entries().is_empty(), "{:?}", h.work_entries());
}

// ---- headers ---------------------------------------------------------------------------------------------------------

fn check_headers(what: &str, hd: &HeaderMap, tls: bool) {
    let get = |k: &str| {
        hd.get(k)
            .unwrap_or_else(|| panic!("{what}: missing {k}"))
            .to_str()
            .unwrap()
            .to_string()
    };
    let csp = get("content-security-policy");
    for d in [
        "default-src 'self'",
        "script-src 'self'",
        "object-src 'none'",
        "frame-ancestors 'none'",
        "base-uri 'none'",
        "form-action 'self'",
    ] {
        assert!(
            csp.split("; ").any(|x| x == d),
            "{what}: CSP lacks {d}: {csp}"
        );
    }
    assert!(
        !csp.contains("unsafe-") && !csp.contains("http"),
        "{what}: {csp}"
    );
    assert_eq!(get("x-content-type-options"), "nosniff", "{what}");
    assert_eq!(get("referrer-policy"), "no-referrer", "{what}");
    assert_eq!(get("cache-control"), "no-store", "{what}");
    let pp = get("permissions-policy");
    for f in ["camera=()", "microphone=()", "geolocation=()"] {
        assert!(pp.contains(f), "{what}: {pp}");
    }
    assert_eq!(
        hd.contains_key("strict-transport-security"),
        tls,
        "{what}: HSTS"
    );
    if tls {
        assert_eq!(
            get("strict-transport-security"),
            "max-age=31536000",
            "{what}"
        );
    }
    assert!(
        !hd.keys().any(|k| k.as_str().starts_with("access-control-")),
        "{what}: CORS header"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn every_response_carries_the_security_headers() {
    for tls in [false, true] {
        let h = if tls {
            H::server("headers-s", |_| {})
        } else {
            H::local("headers-l", |_| {})
        };
        let t = tls.then(|| h.token("alice"));
        let t = t.as_deref();
        let (st, hd, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), t).await;
        assert_eq!(st, StatusCode::ACCEPTED);
        check_headers("202", &hd, tls);
        let id = v["job"].as_str().unwrap().to_string();
        h.wait_done(&id, t).await;
        let one = |what: &str, r: Request<Body>, want: StatusCode| {
            let h = &h;
            let what = what.to_string();
            async move {
                let (st, hd, _) = h.send(r).await;
                assert_eq!(st, want, "{what}");
                check_headers(&what, &hd, tls);
            }
        };
        let (g, base) = (
            |u: &str| h.req("GET", u, t, None, vec![]),
            format!("/api/v1/jobs/{id}"),
        );
        one("status", g(&base), StatusCode::OK).await;
        one("output", g(&format!("{base}/files/0/0")), StatusCode::OK).await;
        one("zip", g(&format!("{base}/zip")), StatusCode::OK).await;
        one("formats", g("/api/v1/formats"), StatusCode::OK).await;
        one(
            "healthz",
            h.req("GET", "/healthz", None, None, vec![]),
            StatusCode::OK,
        )
        .await;
        for p in [
            "/",
            "/app.js",
            "/app.css",
            "/i18n.json",
            "/icon.png",
            "/NotoSans-Regular-uz.ttf",
        ] {
            one(p, h.req("GET", p, None, None, vec![]), StatusCode::OK).await;
        }
        one("404", g("/api/v1/nothing"), StatusCode::NOT_FOUND).await;
        one(
            "404 job",
            g("/api/v1/jobs/00000000000000000000000000000000"),
            StatusCode::NOT_FOUND,
        )
        .await;
        one(
            "405",
            h.req("PUT", "/api/v1/formats", t, None, vec![]),
            StatusCode::METHOD_NOT_ALLOWED,
        )
        .await;
        one(
            "400",
            h.req(
                "POST",
                "/api/v1/jobs",
                t,
                Some("multipart/form-data; boundary=x"),
                b"--x--\r\n".to_vec(),
            ),
            StatusCode::BAD_REQUEST,
        )
        .await;
        one(
            "421",
            {
                let mut r = g("/api/v1/formats");
                r.headers_mut()
                    .insert("host", "evil.example".parse().unwrap());
                r
            },
            StatusCode::MISDIRECTED_REQUEST,
        )
        .await;
        one(
            "403",
            {
                let mut r = g("/api/v1/formats");
                r.headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
                r
            },
            StatusCode::FORBIDDEN,
        )
        .await;
        if tls {
            one(
                "401",
                h.req("GET", "/api/v1/formats", None, None, vec![]),
                StatusCode::UNAUTHORIZED,
            )
            .await;
        }
        one(
            "delete",
            h.req("DELETE", &base, t, None, vec![]),
            StatusCode::NO_CONTENT,
        )
        .await;
    }
}

// ---- data at rest ----------------------------------------------------------------------------------------------------

fn all_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let p = e.path();
        if p.is_dir() {
            all_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn server_mode_stores_nothing_readable_and_no_key_anywhere() {
    let h = H::server("rest", |_| {});
    let t = h.token("alice");
    let secret = "SALOM-MAXFIY-MATN-1234567890-SALOM-MAXFIY-MATN";
    // while the tool runs, the upload is on disk: it must be ciphertext, and no key may sit beside it (WP7 review M5)
    let data = format!("SLEEP{secret}\n").repeat(5000);
    let (st, _, v) = h
        .post(
            &[("note.txt", data.as_bytes())],
            json!({"to": "txt"}),
            Some(&t),
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let slow = v["job"].as_str().unwrap().to_string();
    let t0 = Instant::now();
    while h
        .json("GET", &format!("/api/v1/jobs/{slow}"), Some(&t))
        .await
        .1["files"][0]["state"]
        != "running"
    {
        assert!(t0.elapsed() < Duration::from_secs(20), "never started");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut files = vec![];
    all_files(&h.dir.join("work"), &mut files);
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("0") && f.parent().unwrap().ends_with("in")),
        "the upload is stored: {files:?}"
    );
    assert!(
        !files.iter().any(|f| f
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains("key"))),
        "a key file on disk: {files:?}"
    );
    // The tool's own scratch (`stage-<pid>-<hex>-<n>/`, not the `-api-` store or the `-pre-` check) is the documented
    // plaintext the tool reads while it runs (api.md, plaintext table); it is skipped here only, and the finished-job
    // check below proves it is gone afterwards. Owner decision, 2026-10-22.
    let tool_scratch = |f: &std::path::Path| {
        f.components().any(|c| {
            let c = c.as_os_str().to_string_lossy();
            c.starts_with("stage-") && !c.contains("-api-") && !c.contains("-pre-")
        })
    };
    for f in files.iter().filter(|f| !tool_scratch(f)) {
        let b = std::fs::read(f).unwrap();
        assert!(
            !b.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "plaintext in {}",
            f.display()
        );
    }
    h.json("DELETE", &format!("/api/v1/jobs/{slow}"), Some(&t))
        .await;
    let t0 = Instant::now();
    while !h.work_entries().is_empty() {
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "the cancelled job left: {:?}",
            h.work_entries()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // a finished job: the output decrypts through the API, the input is already gone, and nothing but the store is left
    let (_, _, v) = h
        .post(&[("note.txt", TXT)], json!({"to": "txt"}), Some(&t))
        .await;
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, Some(&t)).await;
    let mut files = vec![];
    all_files(&h.dir.join("work"), &mut files);
    assert!(
        files.iter().all(|f| !f.parent().unwrap().ends_with("in")),
        "inputs are deleted when their file has run: {files:?}"
    );
    assert!(
        files
            .iter()
            .all(|f| !f.to_string_lossy().contains("stage-")
                || f.to_string_lossy().contains("-api-")),
        "only the store remains: {files:?}"
    );
    let (_, _, body) = h
        .send(h.req(
            "GET",
            &format!("/api/v1/jobs/{id}/files/0/0"),
            Some(&t),
            None,
            vec![],
        ))
        .await;
    assert_eq!(body, b"stub\n"); // the stand-in tool's output, decrypted on the way out
    h.json("DELETE", &format!("/api/v1/jobs/{id}"), Some(&t))
        .await;
    let t1 = Instant::now();
    while !h.work_entries().is_empty() {
        assert!(
            t1.elapsed() < Duration::from_secs(20),
            "left: {:?}",
            h.work_entries()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_sweeps_what_a_dead_server_left() {
    let dir = sandbox("restart");
    let work = dir.join("work");
    // a store and a staging directory of a process that is gone (pid 4000000 cannot exist)
    for d in [
        "stage-4000000-api-0123456789abcdef0123456789abcdef/in",
        "stage-4000000-pre-0123456789abcdef0123456789abcdef-0",
        "job-4000000-1a2b3c-1/out",
        "stage-4000000-1a2b3c-2",
    ] {
        std::fs::create_dir_all(work.join(d)).unwrap();
        std::fs::write(work.join(d).join("f"), b"leftover").unwrap();
    }
    // an operator folder that only looks a little like ours must survive the sweep
    for keep in [
        "stage-2024-photos",
        "job-4000000-notes",
        "stage-4000000-api-short",
    ] {
        std::fs::create_dir_all(work.join(keep)).unwrap();
    }
    let s = Settings {
        work_dir: Some(work.clone()),
        ..Settings::default()
    };
    let _app = App::new(s.resolve().unwrap(), fake_cfg(), false).unwrap();
    let mut left: Vec<String> = std::fs::read_dir(&work)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "job-4000000-notes",
            "stage-2024-photos",
            "stage-4000000-api-short"
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- PDF operations (options.op) -----------------------------------------------------------------------------------------

/// A stand-in PDF the fake tool counts: `PAGES=n` is its page count.
fn pdf(pages: u32) -> Vec<u8> {
    format!("%PDF-1.4\n%% PAGES={pages}\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n").into_bytes()
}

async fn output(h: &H, id: &str, n: usize, k: usize, t: Option<&str>) -> (HeaderMap, Vec<u8>) {
    let (st, hd, b) = h
        .send(h.req(
            "GET",
            &format!("/api/v1/jobs/{id}/files/{n}/{k}"),
            t,
            None,
            vec![],
        ))
        .await;
    assert_eq!(st, StatusCode::OK);
    (hd, b)
}

#[tokio::test(flavor = "multi_thread")]
async fn combine_keeps_the_upload_order_and_makes_one_output() {
    let h = H::local("op-combine", |_| {});
    let (st, hd, v) = h
        .post(
            &[("b.pdf", &pdf(3)), ("a.pdf", &pdf(2))],
            json!({"op": "combine"}),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    check_headers("op 202", &hd, false);
    assert_eq!(v["files"][0]["name"], "b.pdf");
    assert_eq!(v["files"][1]["kind"], "pdf");
    let id = v["job"].as_str().unwrap().to_string();
    let done = h.wait_done(&id, None).await;
    assert_eq!(done["progress"], json!({"done": 1, "total": 1}), "{done}");
    assert_eq!(done["files"][0]["state"], "done", "{done}");
    assert_eq!(done["files"][0]["outputs"].as_array().unwrap().len(), 1);
    assert_eq!(done["files"][0]["outputs"][0]["name"], "combined.pdf");
    let (hd, body) = output(&h, &id, 0, 0, None).await;
    check_headers("op output", &hd, false);
    assert_eq!(hd["content-type"], "application/pdf");
    assert!(
        String::from_utf8_lossy(&body).contains("ORDER=3,2"),
        "the files were combined in the order sent"
    );
    let (st, hd, zip) = h
        .send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![]))
        .await;
    assert_eq!(st, StatusCode::OK);
    check_headers("op zip", &hd, false);
    let z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert_eq!(names, ["combined.pdf", "report.json"]);
    h.json("DELETE", &format!("/api/v1/jobs/{id}"), None).await;
    assert!(h.work_entries().is_empty(), "{:?}", h.work_entries());
}

#[tokio::test(flavor = "multi_thread")]
async fn split_and_extract_make_the_right_outputs() {
    let h = H::local("op-split", |_| {});
    let run = |opts: Value| {
        let h = &h;
        async move {
            let (st, _, v) = h.post(&[("x.pdf", &pdf(5))], opts, None).await;
            assert_eq!(st, StatusCode::ACCEPTED, "{v}");
            h.wait_done(v["job"].as_str().unwrap(), None).await
        }
    };
    let d = run(json!({"op": "split", "every": 2})).await;
    assert_eq!(d["files"][0]["state"], "done", "{d}");
    let outs = d["files"][0]["outputs"].as_array().unwrap();
    assert_eq!(outs.len(), 3);
    let names: std::collections::HashSet<_> =
        outs.iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 3, "distinct names: {names:?}");
    let d = run(json!({"op": "split", "before": [3, 5]})).await;
    assert_eq!(d["files"][0]["outputs"].as_array().unwrap().len(), 3, "{d}");
    let d = run(json!({"op": "extract", "pages": "1,3-4"})).await;
    assert_eq!(d["files"][0]["state"], "done", "{d}");
    assert_eq!(d["files"][0]["outputs"][0]["name"], "x-pages.pdf");
    // a page range beyond the document is a failed job with the tool's message, not a silent empty file
    let d = run(json!({"op": "extract", "pages": "2-9"})).await;
    assert_eq!(d["files"][0]["state"], "failed", "{d}");
    assert_eq!(d["files"][0]["outputs"], json!([]));
    assert!(
        d["files"][0]["message"]
            .as_str()
            .unwrap()
            .contains("beyond"),
        "{d}"
    );
    // a split point past the last page is a usage error from the planner
    let d = run(json!({"op": "split", "before": [9]})).await;
    assert_eq!(d["files"][0]["state"], "failed", "{d}");
    assert_eq!(d["files"][0]["code"], 2, "{d}");
}

#[tokio::test(flavor = "multi_thread")]
async fn pdf_operations_validate_their_options_and_files() {
    let h = H::local("op-bad", |_| {});
    let a = pdf(2);
    let one: &[(&str, &[u8])] = &[("a.pdf", &a)];
    let two: &[(&str, &[u8])] = &[("a.pdf", &a), ("b.pdf", &a)];
    type Files<'a> = &'a [(&'a str, &'a [u8])];
    let cases: Vec<(Files, Value)> = vec![
        (one, json!({"op": "combine"})),
        (two, json!({"op": "split", "every": 2})),
        (two, json!({"op": "extract", "pages": "1"})),
        (one, json!({"op": "shuffle"})),
        (one, json!({"op": "extract"})),
        (one, json!({"op": "extract", "pages": "0"})),
        (one, json!({"op": "extract", "pages": "3-1"})),
        (one, json!({"op": "extract", "pages": "a,b"})),
        (one, json!({"op": "extract", "pages": "1", "every": 2})),
        (one, json!({"op": "split"})),
        (one, json!({"op": "split", "every": 0})),
        (one, json!({"op": "split", "every": 2, "before": [3]})),
        (one, json!({"op": "split", "before": [1]})),
        (one, json!({"op": "split", "before": []})),
        (one, json!({"op": "split", "every": 2, "pages": "1"})),
        (two, json!({"op": "combine", "pages": "1"})),
        (two, json!({"op": "combine", "to": "docx"})),
        (two, json!({"op": "combine", "locale": "ru"})),
        (two, json!({"op": "combine", "translit": "cyrl"})),
        (one, json!({"to": "pdf", "every": 2})),
        (one, json!({"to": "pdf", "before": [2]})),
    ];
    for (files, opts) in cases {
        let (st, _, v) = h.post(files, opts.clone(), None).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{opts}: {v}");
        assert_eq!(v["error"]["code"], "bad_options", "{opts}");
    }
    assert!(
        h.work_entries().is_empty(),
        "nothing is left: {:?}",
        h.work_entries()
    );
    // `to` may say pdf
    let (st, _, v) = h
        .post(two, json!({"op": "combine", "to": "pdf"}), None)
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    // a non-PDF in a combine: told per file, and the whole operation fails with that file's code
    let (st, _, v) = h
        .post(
            &[("a.pdf", &a), ("n.txt", TXT)],
            json!({"op": "combine"}),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert!(v["files"][0].get("error").is_none());
    assert_eq!(v["files"][1]["error"]["code"], 3);
    let d = h.wait_done(v["job"].as_str().unwrap(), None).await;
    assert_eq!(d["files"][0]["state"], "failed", "{d}");
    assert_eq!(d["files"][0]["code"], 3);
    assert!(
        d["files"][0]["message"].as_str().unwrap().contains("PDF"),
        "{d}"
    );
    assert_eq!(d["files"][0]["outputs"], json!([]));
    // junk with a .pdf name is not a PDF either
    let (_, _, v) = h
        .post(
            &[("a.pdf", &a), ("b.pdf", &[0u8, 1, 2, 250][..])],
            json!({"op": "combine"}),
            None,
        )
        .await;
    assert_eq!(v["files"][1]["error"]["code"], 3, "{v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn pdf_operations_respect_the_caps() {
    let h = H::local("op-caps", |_| {});
    let run = |pages: u32, opts: Value| {
        let h = &h;
        async move {
            let (st, _, v) = h.post(&[("big.pdf", &pdf(pages))], opts, None).await;
            assert_eq!(st, StatusCode::ACCEPTED, "{v}");
            h.wait_done(v["job"].as_str().unwrap(), None).await
        }
    };
    // 501 pages is over max_pages_in (500); 300 one-page parts is over max_png_out (200)
    for (pages, opts, why) in [
        (501, json!({"op": "split", "every": 100}), "pages"),
        (300, json!({"op": "split", "every": 1}), "files"),
    ] {
        let d = run(pages, opts).await;
        assert_eq!(d["files"][0]["state"], "failed", "{why}: {d}");
        assert_eq!(d["files"][0]["code"], 9, "{why}: {d}");
        assert!(
            d["files"][0]["message"]
                .as_str()
                .unwrap()
                .contains("over the limit"),
            "{why}: {d}"
        );
        assert_eq!(d["files"][0]["outputs"], json!([]));
    }
    // more files than a batch allows is refused before anything runs
    let a = pdf(1);
    let many: Vec<(String, &[u8])> = (0..31)
        .map(|i| (format!("{i}.pdf"), a.as_slice()))
        .collect();
    let refs: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    let (st, _, v) = h.post(&refs, json!({"op": "combine"}), None).await;
    assert!(st.is_client_error(), "{st}: {v}");
    // oversize: a combine counts every file against the per-file limit
    let h = H::local("op-size", |s| s.max_file_mb = 1);
    let big = vec![b'x'; 2 << 20];
    let (st, _, _) = h
        .post(
            &[("a.pdf", &pdf(1)), ("b.pdf", &big)],
            json!({"op": "combine"}),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(h.work_entries().is_empty(), "{:?}", h.work_entries());
}

#[tokio::test(flavor = "multi_thread")]
async fn pdf_operations_need_a_token_in_server_mode_and_store_nothing_readable() {
    let h = H::server("op-auth", |_| {});
    let (a, b) = (pdf(1), pdf(4));
    let files: &[(&str, &[u8])] = &[("a.pdf", &a), ("b.pdf", &b)];
    let (st, hd, _) = h.post(files, json!({"op": "combine"}), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    check_headers("op 401", &hd, true);
    let t = h.token("alice");
    let (st, hd, v) = h.post(files, json!({"op": "combine"}), Some(&t)).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    check_headers("op 202", &hd, true);
    let id = v["job"].as_str().unwrap().to_string();
    // another user sees nothing
    let t2 = h.token("bob");
    assert_eq!(
        h.json("GET", &format!("/api/v1/jobs/{id}"), Some(&t2))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    h.wait_done(&id, Some(&t)).await;
    let mut stored = vec![];
    all_files(&h.dir.join("work"), &mut stored);
    assert!(!stored.is_empty());
    for f in &stored {
        let bytes = std::fs::read(f).unwrap();
        assert!(
            !bytes.windows(5).any(|w| w == b"%PDF-" || w == b"ORDER"),
            "plaintext in {}",
            f.display()
        );
    }
    assert!(
        stored.iter().all(|f| !f.parent().unwrap().ends_with("in")),
        "inputs are deleted: {stored:?}"
    );
    let (hd, body) = output(&h, &id, 0, 0, Some(&t)).await;
    check_headers("op output", &hd, true);
    assert!(String::from_utf8_lossy(&body).contains("ORDER=1,4"));
    // a refused operation (non-PDF) leaves no input behind either
    let (_, _, v) = h
        .post(
            &[("a.pdf", &a), ("n.txt", TXT)],
            json!({"op": "combine"}),
            Some(&t),
        )
        .await;
    h.wait_done(v["job"].as_str().unwrap(), Some(&t)).await;
    let mut stored = vec![];
    all_files(&h.dir.join("work"), &mut stored);
    assert!(
        stored.iter().all(|f| !f.parent().unwrap().ends_with("in")),
        "{stored:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn formats_lists_the_pdf_operations() {
    let h = H::local("op-formats", |_| {});
    let (_, v) = h.json("GET", "/api/v1/formats", None).await;
    let ops: Vec<&str> = v["pdfOps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["combine", "split", "extract"]);
    assert_eq!(v["pdfOps"][0]["files"]["min"], 2);
    assert_eq!(v["pdfOps"][2]["options"], json!(["pages"]));
}

// ---- warning arguments (the page fills {encoding}, {missing}, ... from them) ------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn warnings_carry_the_values_of_their_placeholders() {
    let h = H::local("warn-args", |_| {});
    // cp1251 text: "Привет, мир! Это обычный текст для проверки."
    let cp1251: &[u8] = &[
        0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2, 0x2c, 0x20, 0xec, 0xe8, 0xf0, 0x21, 0x20, 0xdd, 0xf2,
        0xee, 0x20, 0xee, 0xe1, 0xfb, 0xf7, 0xed, 0xfb, 0xe9, 0x20, 0xf2, 0xe5, 0xea, 0xf1, 0xf2,
        0x20, 0xe4, 0xeb, 0xff, 0x20, 0xef, 0xf0, 0xee, 0xe2, 0xe5, 0xf0, 0xea, 0xe8, 0x2e, 0x0a,
    ];
    let (_, _, v) = h
        .post(
            &[("old.txt", cp1251), ("memo.docx", TXT)],
            json!({"to": "pdf"}),
            None,
        )
        .await;
    let d = h.wait_done(v["job"].as_str().unwrap(), None).await;
    let w = |n: usize, code: &str| {
        d["files"][n]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["code"] == code)
            .unwrap_or_else(|| panic!("{code}: {d}"))
            .clone()
    };
    let enc = w(0, "text_transcoded");
    assert!(
        enc["args"]["encoding"]
            .as_str()
            .is_some_and(|e| !e.is_empty()),
        "{enc}"
    );
    assert_eq!(w(1, "kind_mismatch")["args"]["kind"], "txt");
}
