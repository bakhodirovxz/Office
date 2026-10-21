//! Tests for the WP7 security-review fixes (docs/phase3/review-wp7.md): a dropped request, the store quotas, the zip scan budget, the
//! job deadline, the listener limits, trusted proxies, the token rules and the smaller items. Same shape as `tests/api.rs`: the
//! router in-process against a stand-in for the domain CLIs, and real sockets for the listener tests.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use convert::exec::Config as ConvertConfig;
use convert::plan::ToolId;
use convert_api::app::App;
use convert_api::config::Settings;
use convert_api::{jwt, routes, tls};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef-secret";
const TXT: &[u8] = "Salom dunyo, bu oddiy matn.\n".as_bytes();

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
    let d = std::env::temp_dir().join(format!("caph-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fake_cfg() -> ConvertConfig {
    let mut c = ConvertConfig::from_env();
    for t in ToolId::ALL {
        c.tools.insert(t, env!("CARGO_BIN_EXE_convert-api-fake-tool").into());
    }
    c.sample_ms = 50;
    c
}

impl H {
    fn local(name: &str, f: impl FnOnce(&mut Settings)) -> H {
        let dir = sandbox(name);
        let mut s = Settings { work_dir: Some(dir.join("work")), store_dir: Some(dir.join("store")), ..Settings::default() };
        f(&mut s);
        H::build(dir, s, "127.0.0.1:8787", false, fake_cfg())
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
            store_dir: Some(dir.join("store")),
            ..Settings::default()
        };
        f(&mut s);
        let upstream = s.tls == "upstream";
        if upstream {
            s.tls_cert = None;
            s.tls_key = None;
        }
        H::build(dir, s, "convert.test", !upstream, fake_cfg())
    }

    fn build(dir: PathBuf, s: Settings, host: &'static str, tls: bool, cfg: ConvertConfig) -> H {
        let app = App::new(s.resolve().unwrap(), cfg, tls).unwrap();
        H { dir, router: routes::router(app.clone()), app, host }
    }

    fn req(&self, method: &str, uri: &str, token: Option<&str>, ct: Option<&str>, body: Vec<u8>) -> Request<Body> {
        let mut r = Request::builder().method(method).uri(uri).header("host", self.host);
        if let Some(t) = token {
            r = r.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = ct {
            r = r.header("content-type", c).header("content-length", body.len());
        }
        let mut r = r.body(Body::from(body)).unwrap();
        r.extensions_mut().insert(ConnectInfo("203.0.113.9:5555".parse::<SocketAddr>().unwrap()));
        r
    }

    async fn send(&self, r: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let res = self.router.clone().oneshot(r).await.unwrap();
        let (st, h) = (res.status(), res.headers().clone());
        (st, h, axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec())
    }

    async fn json(&self, method: &str, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let (st, _, b) = self.send(self.req(method, uri, token, None, vec![])).await;
        (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    fn post_req(&self, files: &[(&str, &[u8])], options: Value, token: Option<&str>) -> Request<Body> {
        let (ct, body) = multipart(files, &options.to_string());
        self.req("POST", "/api/v1/jobs", token, Some(&ct), body)
    }

    async fn post(&self, files: &[(&str, &[u8])], options: Value, token: Option<&str>) -> (StatusCode, HeaderMap, Value) {
        let (st, h, b) = self.send(self.post_req(files, options, token)).await;
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
            assert!(t.elapsed() < Duration::from_secs(60), "job never finished: {v}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn wait_running(&self, id: &str, token: Option<&str>) {
        let t = Instant::now();
        while self.json("GET", &format!("/api/v1/jobs/{id}"), token).await.1["files"][0]["state"] != "running" {
            assert!(t.elapsed() < Duration::from_secs(20), "never started");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn token(&self, sub: &str) -> String {
        jwt::sign(SECRET, &json!({"sub": sub, "exp": jwt::now() + 600, "scope": "convert"}))
    }

    fn files_in(&self, sub: &str) -> Vec<PathBuf> {
        let mut v = vec![];
        all_files(&self.dir.join(sub), &mut v);
        v
    }

    async fn wait_clean(&self) {
        let t = Instant::now();
        while !(self.files_in("work").is_empty() && std::fs::read_dir(self.dir.join("store")).map_or(true, |r| r.count() == 0)) {
            assert!(t.elapsed() < Duration::from_secs(20), "left: {:?} {:?}", self.files_in("work"), self.files_in("store"));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

fn all_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            all_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn multipart(files: &[(&str, &[u8])], options: &str) -> (String, Vec<u8>) {
    let b = "----caph-test-boundary";
    let mut body = Vec::new();
    for (name, data) in files {
        body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n--{b}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={b}"), body)
}

fn sleeper(extra: usize) -> Vec<u8> {
    let mut v = b"SLEEP".to_vec();
    v.extend(std::iter::repeat_n(b'x', extra));
    v
}

// ---- B1: a dropped request --------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_request_dropped_while_it_waits_for_a_slot_leaks_nothing() {
    let h = H::local("b1", |s| {
        s.cpu_slots = Some(1);
        s.slot_wait_s = 60;
        s.per_user_active = 1;
    });
    let held = h.app.cpu.try_acquire().expect("the only slot");
    // the upload is read, then the handler waits for a slot for the upload check; the client goes away at that point
    let (router, req) = (h.router.clone(), h.post_req(&[("a.txt", TXT)], json!({"to": "pdf"}), None));
    let task = tokio::spawn(async move { router.oneshot(req).await });
    let t = Instant::now();
    while h.app.queue_depth() == 0 {
        assert!(t.elapsed() < Duration::from_secs(10), "never admitted");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await; // inside the wait now
    assert_eq!(h.app.queue_depth(), 1);
    assert!(!h.files_in("store").is_empty(), "the upload is on disk");
    task.abort();
    let _ = task.await;
    let t = Instant::now();
    while h.app.queue_depth() != 0 {
        assert!(t.elapsed() < Duration::from_secs(10), "the dropped request still counts as an active job");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    h.wait_clean().await;
    assert_eq!(h.app.stored_bytes(), 0);
    drop(held);
    // and the user, whose limit is one active job, can post again
    assert_eq!(h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await.0, StatusCode::ACCEPTED);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_wait_for_a_slot_is_bounded_for_the_upload_check_and_for_the_zip() {
    let h = H::local("b1-wait", |s| {
        s.cpu_slots = Some(1);
        s.slot_wait_s = 1;
    });
    // a finished job whose zip has not been built yet
    let (_, _, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, None).await;
    let held = h.app.cpu.try_acquire().expect("the only slot");
    let t = Instant::now();
    let (st, hd, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!((st, v["error"]["code"].clone()), (StatusCode::SERVICE_UNAVAILABLE, json!("busy")), "{v}");
    assert!(hd.contains_key("retry-after") && t.elapsed() < Duration::from_secs(10));
    // the refused job is gone, not leaked
    let t = Instant::now();
    while h.app.queue_depth() != 0 {
        assert!(t.elapsed() < Duration::from_secs(5), "leaked");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // building a zip needs a slot (M1); an already-built zip needs none
    let (st, _, _) = h.send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![])).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    drop(held);
    let (st, _, zip) = h.send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![])).await;
    assert_eq!(st, StatusCode::OK);
    assert!(zip.starts_with(b"PK"));
    let held = h.app.cpu.try_acquire().expect("the only slot");
    assert_eq!(h.send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), None, None, vec![])).await.0, StatusCode::OK, "a built zip is served without a slot");
    drop(held);
}

// ---- B2: the store is bounded -----------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn inputs_are_deleted_as_soon_as_their_file_has_run_or_failed_its_check() {
    let h = H::server("b2-in", |_| {});
    let t = h.token("alice");
    let (st, _, v) = h.post(&[("a.txt", TXT), ("b.bin", &[0u8, 1, 2, 3, 0xFF][..])], json!({"to": "pdf"}), Some(&t)).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert_eq!(v["files"][1]["error"]["code"], 3);
    // the rejected file's input goes at once (no wait for the run), the other when it has run
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, Some(&t)).await;
    let files = h.files_in("store");
    assert!(files.iter().all(|f| !f.parent().unwrap().ends_with("in")), "inputs left: {files:?}");
    assert!(files.iter().any(|f| f.to_string_lossy().contains("out")), "the output stays");
    // what is accounted is what is left: the (tiny) output, not the 2 uploads
    assert!(h.app.stored_bytes() < 1024, "{}", h.app.stored_bytes());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_store_quota_the_user_cap_and_the_disk_floor_each_refuse_a_new_job() {
    // total quota: 2 MB store, 1 MB batch reservation, 0.6 MB inputs that stay while their (sleeping) tool runs
    let h = H::local("b2-quota", |s| {
        s.store_max_mb = 2;
        s.max_batch_mb = 1;
        s.max_file_mb = 1;
        s.min_free_mb = 0;
        s.per_user_active = 10;
    });
    let big = sleeper(600 * 1024);
    let mut ids = vec![];
    for _ in 0..2 {
        let (st, _, v) = h.post(&[("s.txt", &big)], json!({"to": "pdf"}), None).await;
        assert_eq!(st, StatusCode::ACCEPTED, "{v}");
        ids.push(v["job"].as_str().unwrap().to_string());
    }
    assert!(h.app.stored_bytes() > 1_100_000, "{}", h.app.stored_bytes());
    let (st, hd, v) = h.post(&[("s.txt", &big)], json!({"to": "pdf"}), None).await;
    assert_eq!((st, v["error"]["code"].clone()), (StatusCode::SERVICE_UNAVAILABLE, json!("store_full")), "{v}");
    assert!(hd.contains_key("retry-after"));
    for id in &ids {
        h.json("DELETE", &format!("/api/v1/jobs/{id}"), None).await;
    }
    h.wait_clean().await;
    assert_eq!(h.app.stored_bytes(), 0);
    assert_eq!(h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await.0, StatusCode::ACCEPTED, "room again after the deletes");

    // per-user cap: alice may keep 1 MB; bob is not affected
    let h = H::server("b2-user", |s| {
        s.user_store_mb = 1;
        s.max_batch_mb = 1;
        s.max_file_mb = 1;
        s.min_free_mb = 0;
        s.per_user_active = 10;
    });
    let (alice, bob) = (h.token("alice"), h.token("bob"));
    let (st, _, v) = h.post(&[("s.txt", &big)], json!({"to": "pdf"}), Some(&alice)).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (st, _, v) = h.post(&[("s.txt", &big)], json!({"to": "pdf"}), Some(&alice)).await;
    assert_eq!((st, v["error"]["code"].clone()), (StatusCode::TOO_MANY_REQUESTS, json!("user_store_full")), "{v}");
    let (st, _, v) = h.post(&[("s.txt", &big)], json!({"to": "pdf"}), Some(&bob)).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");

    // the free-space floor: a floor no disk can meet
    let h = H::local("b2-floor", |s| s.min_free_mb = 1 << 30);
    let (st, hd, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!((st, v["error"]["code"].clone()), (StatusCode::SERVICE_UNAVAILABLE, json!("disk_low")), "{v}");
    assert!(hd.contains_key("retry-after"));
    assert_eq!(h.app.queue_depth(), 0);
}

// ---- M1: the scan ---------------------------------------------------------------------------------------------------

fn zip_of(parts: &[(&str, usize)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, size) in parts {
        w.start_file(*name, o).unwrap();
        w.write_all(&vec![0u8; *size]).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_ratio_rule_now_covers_parts_that_used_to_slip_under_ten_megabytes() {
    let h = H::local("m1-ratio", |_| {});
    // 100 such parts used to pass: each is under 10 MB. One is enough to be refused now (a few KB of upload, 9.9 MB of inflate)
    let bomb = zip_of(&[("word/document.xml", 9_900_000)]);
    assert!(bomb.len() < 50_000, "{}", bomb.len());
    let (st, _, v) = h.post(&[("bomb.docx", &bomb), ("ok.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert_eq!(v["files"][0]["error"]["code"], 9, "{v}");
    assert!(v["files"][0]["error"]["message"].as_str().unwrap().contains("zip bomb"), "{v}");
    assert!(v["files"][1].get("error").is_none());
}

// ---- M2: one deadline for the whole job ---------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_job_deadline_covers_all_its_files_together() {
    let h = H::local("m2", |s| {
        s.cpu_slots = Some(1);
        s.job_timeout_s = 3;
    });
    // three files that would each sleep for two minutes, one slot: per file that would be six minutes, per job it is three seconds
    let slow = sleeper(0);
    let t = Instant::now();
    let (st, _, v) = h.post(&[("1.txt", &slow), ("2.txt", &slow), ("3.txt", &slow)], json!({"to": "pdf"}), None).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let done = h.wait_done(v["job"].as_str().unwrap(), None).await;
    let took = t.elapsed();
    assert!(took < Duration::from_secs(20), "{took:?}");
    for f in done["files"].as_array().unwrap() {
        assert_eq!((f["state"].clone(), f["code"].clone()), (json!("failed"), json!(4)), "{done}");
    }
}

// ---- M3: the listeners ------------------------------------------------------------------------------------------------

async fn plain_server(lim: tls::ConnLimits) -> (u16, Arc<App>, PathBuf) {
    let dir = sandbox(&format!("m3-{}", lim.per_ip));
    let s = Settings { work_dir: Some(dir.join("work")), store_dir: Some(dir.join("store")), ..Settings::default() };
    let app = App::new(s.resolve().unwrap(), fake_cfg(), false).unwrap();
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(tls::serve(l, None, routes::router(app.clone()), lim, std::future::pending()));
    (port, app, dir)
}

fn limits() -> tls::ConnLimits {
    tls::ConnLimits { total: 1000, per_ip: 2, exempt: vec![], max_age: Duration::from_secs(60), max_requests: 100, head_timeout: Duration::from_secs(1) }
}

fn get(port: u16, s: &mut std::net::TcpStream) -> String {
    write!(s, "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1:8787\r\n\r\n").unwrap();
    let mut buf = vec![0u8; 8192];
    let n = s.read(&mut buf).unwrap_or(0);
    let _ = port;
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_listener_caps_connections_per_address_and_drops_slow_and_old_ones() {
    let (port, _app, dir) = plain_server(limits()).await;
    let r = tokio::task::spawn_blocking(move || {
        let conn = || {
            let s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s
        };
        let (mut a, mut b) = (conn(), conn());
        assert!(get(port, &mut a).starts_with("HTTP/1.1 200"));
        assert!(get(port, &mut b).starts_with("HTTP/1.1 200"));
        // the third connection from the same address is closed without an answer
        let mut c = conn();
        assert_eq!(get(port, &mut c), "", "a third connection from one address");
        // an idle connection (no request head within the timeout) is closed by the server: a slowloris slot does not last
        let t = Instant::now();
        let mut buf = [0u8; 16];
        let n = a.read(&mut buf).unwrap_or(0);
        assert_eq!(n, 0, "the idle keep-alive connection was closed");
        assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
        // a slot comes back when a connection ends
        drop(b);
        std::thread::sleep(Duration::from_millis(300));
        let mut d = conn();
        assert!(get(port, &mut d).starts_with("HTTP/1.1 200"));
        // a client that drips a partial request head is cut off too
        let mut e = conn();
        e.write_all(b"GET /healthz HT").unwrap();
        let t = Instant::now();
        assert_eq!(e.read(&mut buf).unwrap_or(0), 0);
        assert!(t.elapsed() < Duration::from_secs(4));
    });
    r.await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_is_closed_after_its_request_budget_so_keepalive_drip_ends() {
    let (port, _app, dir) = plain_server(tls::ConnLimits { max_requests: 3, head_timeout: Duration::from_secs(30), per_ip: 10, ..limits() }).await;
    tokio::task::spawn_blocking(move || {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let heads: Vec<String> = (0..3).map(|_| get(port, &mut s).to_ascii_lowercase()).collect();
        assert!(!heads[0].contains("connection: close") && !heads[1].contains("connection: close"), "{heads:?}");
        assert!(heads[2].contains("connection: close"), "{}", heads[2]);
        let mut buf = [0u8; 8];
        assert_eq!(s.read(&mut buf).unwrap_or(0), 0, "the server hung up after the last request");
    })
    .await
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- M6: proxies ---------------------------------------------------------------------------------------------------------

fn from(mut r: Request<Body>, peer: &str, headers: &[(&'static str, &str)]) -> Request<Body> {
    r.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    for (k, v) in headers {
        r.headers_mut().insert(*k, v.parse().unwrap());
    }
    r
}

#[tokio::test(flavor = "multi_thread")]
async fn x_forwarded_for_counts_only_from_a_trusted_proxy_and_ipv6_counts_by_64() {
    let h = H::server("m6", |s| {
        s.rate_ip_per_min = 2;
        s.per_user_active = 100;
        s.rate_user_per_min = 1000;
        s.trusted_proxies = vec!["10.1.0.0/16".into()];
    });
    let t = h.token("alice");
    let hr = &h;
    let post = |peer: &str, hs: &[(&'static str, &str)]| {
        let r = from(hr.post_req(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&t)), peer, hs);
        async move { hr.send(r).await.0 }
    };
    // an untrusted peer cannot choose its bucket: three different X-Forwarded-For values are still one address
    assert_eq!(post("203.0.113.9:1", &[("x-forwarded-for", "1.1.1.1")]).await, StatusCode::ACCEPTED);
    assert_eq!(post("203.0.113.9:1", &[("x-forwarded-for", "2.2.2.2")]).await, StatusCode::ACCEPTED);
    assert_eq!(post("203.0.113.9:1", &[("x-forwarded-for", "3.3.3.3")]).await, StatusCode::TOO_MANY_REQUESTS);
    // behind the trusted proxy each client has its own bucket; the rightmost address that is not a proxy is the client
    for _ in 0..2 {
        assert_eq!(post("10.1.2.3:1", &[("x-forwarded-for", "198.51.100.1")]).await, StatusCode::ACCEPTED);
    }
    assert_eq!(post("10.1.2.3:1", &[("x-forwarded-for", "198.51.100.1")]).await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(post("10.1.2.3:1", &[("x-forwarded-for", "198.51.100.2")]).await, StatusCode::ACCEPTED);
    // a client-supplied left part cannot hide the real client behind a chain of proxies
    assert_eq!(post("10.1.2.3:1", &[("x-forwarded-for", "9.9.9.9, 198.51.100.1, 10.1.9.9")]).await, StatusCode::TOO_MANY_REQUESTS);
    // IPv6: every address of one /64 is one client
    assert_eq!(post("[2001:db8:0:1::1]:1", &[]).await, StatusCode::ACCEPTED);
    assert_eq!(post("[2001:db8:0:1::ffff]:1", &[]).await, StatusCode::ACCEPTED);
    assert_eq!(post("[2001:db8:0:1:aaaa::3]:1", &[]).await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(post("[2001:db8:0:2::1]:1", &[]).await, StatusCode::ACCEPTED, "another /64 has its own bucket");
}

#[tokio::test(flavor = "multi_thread")]
async fn tls_upstream_answers_the_proxy_only_and_keeps_hsts_and_the_redirect_right() {
    let h = H::server("m6-up", |s| {
        s.tls = "upstream".into();
        s.trusted_proxies = vec!["10.1.0.0/16".into()];
    });
    let t = h.token("alice");
    let get = |peer: &str, uri: &str, hs: &[(&'static str, &str)]| from(h.req("GET", uri, Some(&t), None, vec![]), peer, hs);
    // a direct client, even one that claims to be forwarded, gets nothing
    let (st, hd, _) = h.send(get("203.0.113.9:1", "/api/v1/formats", &[("x-forwarded-proto", "https"), ("x-forwarded-for", "10.1.2.3")])).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(!hd.contains_key("strict-transport-security"), "HSTS is not sent for an answer that did not travel over TLS");
    // the proxy, with the client on https: served, with HSTS
    let (st, hd, _) = h.send(get("10.1.2.3:1", "/api/v1/formats", &[("x-forwarded-proto", "https")])).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(hd["strict-transport-security"], "max-age=31536000");
    // the proxy, with the client on plain http: a redirect to the configured URL, never to the Host header, and no HSTS
    let (st, hd, _) = h.send(get("10.1.2.3:1", "/api/v1/jobs/x?a=1", &[("x-forwarded-proto", "http"), ("host", "convert.test")])).await;
    assert_eq!(st, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(hd["location"], "https://convert.test/api/v1/jobs/x?a=1");
    assert!(!hd.contains_key("strict-transport-security"));
    // a missing X-Forwarded-Proto is not https
    assert_eq!(h.send(get("10.1.2.3:1", "/api/v1/formats", &[])).await.0, StatusCode::PERMANENT_REDIRECT);
    // the container's own health probe on the loopback address learns nothing
    let (st, _, b) = h.send(from(h.req("GET", "/healthz", None, None, vec![]), "127.0.0.1:1", &[])).await;
    assert_eq!((st, serde_json::from_slice::<Value>(&b).unwrap()), (StatusCode::OK, json!({"ok": true})));
    assert_eq!(h.send(from(h.req("GET", "/api/v1/formats", Some(&t), None, vec![]), "127.0.0.1:1", &[])).await.0, StatusCode::FORBIDDEN);
}

// ---- M7, m1, m4, m5, m8, m11, m12 -----------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_options_part_is_refused_while_it_streams() {
    let h = H::local("m7", |_| {});
    let big = format!("{{\"to\":\"pdf\",\"sheet\":\"{}\"}}", "x".repeat(200 * 1024));
    let (ct, body) = multipart(&[("a.txt", TXT)], &big);
    let (st, _, b) = h.send(h.req("POST", "/api/v1/jobs", None, Some(&ct), body)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&b).contains("bad_options"));
    h.wait_clean().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn token_rules_nbf_lifetime_audience_and_leeway_at_the_door() {
    let h = H::server("m1-jwt", |s| {
        s.aud = Some("convert-api".into());
        s.max_token_lifetime_s = 3600;
    });
    let tok = |extra: Value| {
        let mut p = json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "convert", "aud": "convert-api"});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        jwt::sign(SECRET, &p)
    };
    let status = |t: String| {
        let h = &h;
        async move { h.json("GET", "/api/v1/formats", Some(&t)).await.0 }
    };
    assert_eq!(status(tok(json!({}))).await, StatusCode::OK);
    assert_eq!(status(tok(json!({"exp": jwt::now() - 30}))).await, StatusCode::OK, "inside the leeway");
    assert_eq!(status(tok(json!({"exp": jwt::now() - 300}))).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(tok(json!({"nbf": jwt::now() + 600}))).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(tok(json!({"exp": jwt::now() + 100 * 365 * 86400}))).await, StatusCode::UNAUTHORIZED, "a token for a century");
    assert_eq!(status(tok(json!({"aud": "somebody-else"}))).await, StatusCode::UNAUTHORIZED);
    let no_aud = jwt::sign(SECRET, &json!({"sub": "alice", "exp": jwt::now() + 600, "scope": "convert"}));
    assert_eq!(status(no_aud).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread")]
async fn healthz_tells_a_stranger_only_that_the_server_is_up() {
    let h = H::server("m4-health", |_| {});
    let (st, v) = h.json("GET", "/healthz", None).await;
    assert_eq!((st, v), (StatusCode::OK, json!({"ok": true})));
    let (st, v) = h.json("GET", "/healthz", Some("bad.token.here")).await;
    assert_eq!((st, v), (StatusCode::OK, json!({"ok": true})));
    let (st, v) = h.json("GET", "/healthz", Some(&h.token("alice"))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["tools"].is_object() && v["queue"].is_object(), "{v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sidecar_address_never_reaches_a_client() {
    let dir = sandbox("m5");
    let mut cfg = fake_cfg();
    cfg.pdf2docx_url = Some("http://10.255.255.1:18080".into());
    cfg.timeout_s = Some(2);
    let s = Settings { work_dir: Some(dir.join("work")), store_dir: Some(dir.join("store")), ..Settings::default() };
    let h = H::build(dir, s, "127.0.0.1:8787", false, cfg);
    let pdf = b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n";
    let (st, _, v) = h.post(&[("a.pdf", pdf)], json!({"to": "docx", "pdfEngine": "sidecar"}), None).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let done = h.wait_done(v["job"].as_str().unwrap(), None).await;
    let all = done.to_string() + &v.to_string();
    assert!(all.contains("sidecar"), "the job did fail on the sidecar: {all}");
    assert!(!all.contains("10.255.255.1") && !all.contains("18080"), "{all}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_zip_part_left_by_a_crashed_build_does_not_break_downloads() {
    let h = H::server("m8", |_| {});
    let t = h.token("alice");
    let (_, _, v) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&t)).await;
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_done(&id, Some(&t)).await;
    let dir = h.files_in("store").iter().find_map(|f| f.ancestors().find(|a| a.file_name().is_some_and(|n| n.to_string_lossy().contains("-api-"))).map(|a| a.to_path_buf())).unwrap();
    std::fs::write(dir.join("zip.part"), b"half a zip").unwrap();
    let (st, _, zip) = h.send(h.req("GET", &format!("/api/v1/jobs/{id}/zip"), Some(&t), None, vec![])).await;
    assert_eq!(st, StatusCode::OK);
    assert!(!dir.join("zip.part").exists());
    assert!(zip.len() > 100);
}

fn check_headers(what: &str, hd: &HeaderMap, tls: bool) {
    let get = |k: &str| hd.get(k).unwrap_or_else(|| panic!("{what}: missing {k}")).to_str().unwrap().to_string();
    assert!(get("content-security-policy").contains("default-src 'self'"), "{what}");
    assert_eq!(get("x-content-type-options"), "nosniff", "{what}");
    assert_eq!(get("referrer-policy"), "no-referrer", "{what}");
    assert_eq!(get("cache-control"), "no-store", "{what}");
    assert_eq!(get("cross-origin-resource-policy"), "same-origin", "{what}");
    assert_eq!(get("cross-origin-opener-policy"), "same-origin", "{what}");
    assert_eq!(hd.contains_key("strict-transport-security"), tls, "{what}: HSTS");
    assert!(!hd.keys().any(|k| k.as_str().starts_with("access-control-")), "{what}: CORS header");
}

#[tokio::test(flavor = "multi_thread")]
async fn error_statuses_413_408_409_429_503_carry_the_security_headers_too() {
    // 413: a Content-Length over the limit, answered by the limit layer before the handler runs
    let h = H::local("m11-413", |s| s.max_batch_mb = 1);
    let (ct, body) = multipart(&[("a.txt", &vec![b'a'; 3 << 20])], "{}");
    let (st, hd, _) = h.send(h.req("POST", "/api/v1/jobs", None, Some(&ct), body)).await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    check_headers("413", &hd, false);
    // 408: a body that never ends
    let h = H::local("m11-408", |s| s.upload_timeout_s = 1);
    let mut r = Request::builder().method("POST").uri("/api/v1/jobs").header("host", "127.0.0.1:8787").header("content-type", "multipart/form-data; boundary=x").body(Body::from_stream(futures_pending())).unwrap();
    r.extensions_mut().insert(ConnectInfo("203.0.113.9:5555".parse::<SocketAddr>().unwrap()));
    let (st, hd, _) = h.send(r).await;
    assert_eq!(st, StatusCode::REQUEST_TIMEOUT);
    check_headers("408", &hd, false);
    // 409: a job that can never fit the memory budget
    let h = H::local("m11-409", |s| s.mem_budget_mb = 100);
    let (st, hd, _) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), None).await;
    assert_eq!(st, StatusCode::CONFLICT);
    check_headers("409", &hd, false);
    // 429 and 503, in server mode (HSTS on)
    let h = H::server("m11-429", |s| {
        s.rate_user_per_min = 1;
        s.queue_max = 1;
        s.per_user_active = 5;
    });
    let t = h.token("alice");
    let (st, hd, _) = h.post(&[("s.txt", b"SLEEP")], json!({"to": "pdf"}), Some(&t)).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    check_headers("202", &hd, true);
    let (st, hd, _) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&t)).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    check_headers("429", &hd, true);
    let (st, hd, _) = h.post(&[("a.txt", TXT)], json!({"to": "pdf"}), Some(&h.token("bob"))).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    check_headers("503", &hd, true);
}

/// A body stream that yields nothing and never ends.
fn futures_pending() -> futures_core_free::Pending {
    futures_core_free::Pending
}

mod futures_core_free {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    pub use tokio_stream::Stream;
    pub struct Pending;
    impl Stream for Pending {
        type Item = Result<axum::body::Bytes, std::io::Error>;
        fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Pending
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_job_still_counts_until_its_work_has_stopped() {
    let h = H::local("m12", |s| s.per_user_active = 1);
    let (_, _, v) = h.post(&[("s.txt", b"SLEEP")], json!({"to": "pdf"}), None).await;
    let id = v["job"].as_str().unwrap().to_string();
    h.wait_running(&id, None).await;
    assert_eq!(h.json("DELETE", &format!("/api/v1/jobs/{id}"), None).await.0, StatusCode::NO_CONTENT);
    // the job cannot be fetched any more ...
    assert_eq!(h.json("GET", &format!("/api/v1/jobs/{id}"), None).await.0, StatusCode::NOT_FOUND);
    // ... but its tool is still being killed, so POST then DELETE cannot be cycled past the per-user limit
    assert!(h.app.queue_depth() <= 1);
    let t = Instant::now();
    while h.app.queue_depth() != 0 {
        assert!(t.elapsed() < Duration::from_secs(15), "never stopped");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    h.wait_clean().await;
}
