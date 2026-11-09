//! WP4 (PLAN phase 5, 3.3, 4 and 10): the SSRF-guarded fetch (S6, S7, S9), the callback client and its retry queue (S23), against the
//! HRB contract harness (`fake-hrm`) and against small raw servers for the cases a harness would not produce.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert_api::jwt::{now, sign};
use docserver::Docserver;
use fake_hrm::{Config, FakeHrm, Kind};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const SERVER_SECRET: &str = "ssssssssssssssssssssssssssssssssssssssss";
const AUDIT_KEY: &str = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk";
const PROV_SECRET: &str = "pppppppppppppppppppppppppppppppppppppppp";

fn package(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    for (n, b) in parts {
        w.start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn docx(tag: &str) -> Vec<u8> {
    package(&[
        ("[Content_Types].xml", b"x"),
        ("word/document.xml", tag.as_bytes()),
    ])
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("docserver-out-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A docserver over `d` (the same `d` twice is a restart). `allow` are TOML entries for both lists.
fn make(d: &Path, prov_secret: &str, allow: &str, internal_base: &str) -> Docserver {
    std::fs::write(d.join("server.key"), SERVER_SECRET).unwrap();
    std::fs::write(d.join("prov.key"), prov_secret).unwrap();
    std::fs::write(d.join("audit.key"), AUDIT_KEY).unwrap();
    let p = |f: &str| d.join(f).to_string_lossy().replace('\\', "/");
    let t: toml::Table = format!(
        r#"server_secret_file = "{}"
data_dir = "{}"
internal_listen = "127.0.0.1:19999"
internal_allow = ["127.0.0.0/8"]
audit_key_file = "{}"
rate_create_per_min = 100000
rate_auth_fail_per_min = 100000
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = [{allow}]
callback_allow = [{allow}]
frame_origins = ["https://app.test"]
"#,
        p("server.key"),
        p("data"),
        p("audit.key"),
        p("prov.key"),
    )
    .parse()
    .unwrap();
    let mut ds = Docserver::from_table(t, None).unwrap();
    ds.internal_url = Some(internal_base.into());
    ds
}

fn int(url: &str) -> String {
    format!("{{ url = \"{url}\", internal = true }}")
}

async fn call(
    r: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    call_from(r, None, method, uri, bearer, body).await
}

/// `client`: the address the router's guard layer would have set (none: the tests' single anonymous client).
async fn call_from(
    r: &Router,
    client: Option<&str>,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let mut req = b.body(Body::from(body)).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    if let Some(c) = client {
        req.extensions_mut().insert(convert_api::routes::Client {
            ip: c.into(),
            key: c.into(),
        });
    }
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

/// Our side of an HRB deployment: the fake backend, our internal listener serving the file route, and the public router.
struct Env {
    ds: Arc<Docserver>,
    r: Router,
    hrm: FakeHrm,
    d: PathBuf,
    base: String,
}

async fn env(name: &str, tweak: impl FnOnce(&mut Docserver)) -> Env {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let mut c = Config::generated(&base);
    c.include_exp = true;
    c.callback_allowed_hosts = vec![l.local_addr().unwrap().to_string()];
    let hrm = FakeHrm::start(c).await.unwrap();
    hrm.add_file(Kind::File, 7, "report.docx", docx("v1"));
    let d = dir(name);
    let mut ds = make(&d, &hrm.config().ds_secret, &int(hrm.base_url()), &base);
    tweak(&mut ds);
    let ds = Arc::new(ds);
    let app = ds
        .internal_router()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        let _ = axum::serve(l, app).await;
    });
    Env {
        r: ds.routes::<()>(),
        ds,
        hrm,
        d,
        base,
    }
}

impl Env {
    /// A new docserver process over the same data folder.
    fn restart(&mut self) {
        let ds = make(
            &self.d,
            &self.hrm.config().ds_secret,
            &int(self.hrm.base_url()),
            &self.base,
        );
        self.ds = Arc::new(ds);
        self.r = self.ds.routes::<()>();
    }

    /// `retry_due` blocks on the network: off the test runtime's only thread, which also serves the harness.
    async fn retry(&self, at: u64) -> usize {
        let ds = self.ds.clone();
        tokio::task::spawn_blocking(move || ds.retry_due(at))
            .await
            .unwrap()
    }

    /// Opens the document of the fake backend as its host page would: the HRB-signed config to `POST /ds/session`.
    async fn open(&self) -> (String, String) {
        let (sid, tok, _) = self.open_cfg().await;
        (sid, tok)
    }

    /// `open`, and the signed editor config the page was given.
    async fn open_cfg(&self) -> (String, String, Value) {
        self.open_q("").await
    }

    /// `open_cfg` with a query on the editor-config route (`mode=view`).
    async fn open_q(&self, q: &str) -> (String, String, Value) {
        let url = format!("{}/files/7/editor-config?{q}", self.hrm.base_url());
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
        let body = json!({"origin": "https://app.test", "config": cfg}).to_string();
        let (s, b) = call(&self.r, "POST", "/ds/session", None, body.into_bytes()).await;
        assert_eq!(s, StatusCode::OK, "{}", js(&b));
        let v = js(&b);
        (
            v["sid"].as_str().unwrap().into(),
            v["token"].as_str().unwrap().into(),
            cfg,
        )
    }

    async fn get_doc(&self, sid: &str, tok: &str) -> (StatusCode, Vec<u8>) {
        call(
            &self.r,
            "GET",
            &format!("/ds/session/{sid}/document"),
            Some(tok),
            vec![],
        )
        .await
    }

    async fn post(&self, sid: &str, tok: &str, route: &str, body: Vec<u8>) -> (StatusCode, Value) {
        let (s, b) = call(
            &self.r,
            "POST",
            &format!("/ds/session/{sid}/{route}"),
            Some(tok),
            body,
        )
        .await;
        (s, js(&b))
    }
}

#[tokio::test]
async fn open_save_close_reopen_against_the_hrb_harness() {
    let e = env("flow", |_| {}).await;
    let (sid, tok) = e.open().await;
    // open: the fetch (no version before the first read)
    assert!(e.ds.load_session(&sid).unwrap().versions.is_empty());
    let (s, b) = e.get_doc(&sid, &tok).await;
    assert_eq!((s, b), (StatusCode::OK, docx("v1")));
    // status 1 once
    let (s, v) = e.post(&sid, &tok, "dirty", vec![]).await;
    assert_eq!((s, v["callback"].as_str()), (StatusCode::OK, Some("ok")));
    // save: status 6; the harness downloads from OUR internal route and answers 0
    let v2 = docx("v2");
    let (s, v) = e.post(&sid, &tok, "save", v2.clone()).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        (v["callback"].as_str(), v["dirty"].as_bool()),
        (Some("ok"), Some(false)),
        "{v}"
    );
    let got = e.hrm.received();
    assert_eq!(
        (got.len(), got[0].status, got[0].bytes),
        (1, 6, v2.len()),
        "{:?} {:?}",
        got,
        e.hrm.rejected()
    );
    assert_eq!(e.hrm.object(Kind::File, 7), Some(v2.clone()));
    // close: status 2
    let (s, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(
        (s, v["status"].as_u64(), v["callback"].as_str()),
        (StatusCode::OK, Some(2), Some("ok")),
        "{v}"
    );
    assert_eq!(
        e.hrm
            .received()
            .iter()
            .map(|r| r.status)
            .collect::<Vec<_>>(),
        vec![6, 2]
    );
    assert!(
        e.ds.queued(&sid).is_none(),
        "an acknowledged close leaves nothing queued"
    );
    // a second close sends nothing
    e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(e.hrm.received().len(), 2);
    // reopen: a new session sees the saved version
    let (sid2, tok2) = e.open().await;
    assert_eq!(e.get_doc(&sid2, &tok2).await.1, v2);
    assert!(e.hrm.rejected().is_empty(), "{:?}", e.hrm.rejected());
}

/// PLAN 5, WP7: a view session (the backend's mode=view config has no callbackUrl) reads, cannot write, and sends the backend nothing.
#[tokio::test]
async fn a_view_session_reads_and_sends_no_callback() {
    let e = env("view", |_| {}).await;
    let (sid, tok, cfg) = e.open_q("mode=view").await;
    assert!(cfg["editorConfig"].get("callbackUrl").is_none());
    assert_eq!(e.get_doc(&sid, &tok).await, (StatusCode::OK, docx("v1")));
    for route in ["dirty", "save", "autosave"] {
        let (s, _) = e.post(&sid, &tok, route, docx("v2")).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{route}");
    }
    // closing is not a failure for the page: no status 4, no "failed"
    let (s, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(
        (
            s,
            v["status"].as_u64(),
            v["callback"].as_str(),
            v["dirty"].as_bool()
        ),
        (StatusCode::OK, Some(4), Some("ok"), Some(false)),
        "{v}"
    );
    assert!(e.hrm.received().is_empty() && e.hrm.rejected().is_empty());
    assert_eq!(e.hrm.object(Kind::File, 7), Some(docx("v1")));
}

#[tokio::test]
async fn a_failed_callback_keeps_the_document_dirty_and_the_retry_survives_a_restart() {
    let mut e = env("retry", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    e.hrm
        .set_callback_secret(Some("a-different-secret-of-some-length-1234".into()));
    let v2 = docx("v2");
    let (s, v) = e.post(&sid, &tok, "save", v2.clone()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (v["callback"].as_str(), v["dirty"].as_bool()),
        (Some("failed"), Some(true)),
        "{v}"
    );
    assert!(e.ds.load_session(&sid).unwrap().dirty());
    assert!(e.hrm.received().is_empty());
    // close: status 2 fails too and is queued on disk
    let (_, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(
        (
            v["status"].as_u64(),
            v["callback"].as_str(),
            v["dirty"].as_bool()
        ),
        (Some(2), Some("failed"), Some(true))
    );
    let q = e.ds.queued(&sid).expect("queued");
    assert_eq!((q.attempts, q.gave_up), (1, false));
    assert!(
        q.next >= now() + 55 && q.next <= now() + 65,
        "first retry after a minute"
    );
    // not due yet
    assert_eq!(e.retry(now()).await, 0);
    // a restart: the queue is on disk; the harness recovers
    e.restart();
    e.hrm.set_callback_secret(None);
    assert_eq!(e.retry(now() + 61).await, 1);
    assert!(e.ds.queued(&sid).is_none());
    assert!(!e.ds.load_session(&sid).unwrap().dirty());
    let got = e.hrm.received();
    assert_eq!((got.len(), got[0].status, got[0].bytes), (1, 2, v2.len()));
    assert_eq!(e.hrm.object(Kind::File, 7), Some(v2));
}

#[tokio::test]
async fn the_backoff_is_1_5_15_60_minutes_then_hourly_and_stops_after_a_day() {
    let e = env("backoff", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    e.hrm
        .set_callback_secret(Some("a-different-secret-of-some-length-1234".into()));
    e.post(&sid, &tok, "save", docx("v2")).await;
    e.post(&sid, &tok, "close", vec![]).await;
    // a simulated clock: each retry runs at the moment it fell due
    let mut gaps = vec![];
    for _ in 0..60 {
        let q = e.ds.queued(&sid).unwrap();
        if q.gave_up {
            break;
        }
        assert_eq!(e.retry(q.next).await, 1);
        let q2 = e.ds.queued(&sid).unwrap();
        if !q2.gave_up {
            gaps.push((q2.next - q.next) / 60);
        }
    }
    let q = e.ds.queued(&sid).unwrap();
    assert!(q.gave_up, "given up");
    // first retry after 1 min (checked in the test above); then 5, 15, 60 min, then hourly
    assert_eq!(&gaps[..4], &[5, 15, 60, 60], "{gaps:?}");
    assert!(gaps[3..].iter().all(|g| *g == 60), "{gaps:?}");
    assert!(q.next <= q.first + 24 * 3600 + 3600);
    assert_eq!(
        e.retry(q.next + 1).await,
        0,
        "a given-up entry is not tried again"
    );
    assert!(
        e.ds.load_session(&sid).unwrap().dirty(),
        "still dirty; the version is on disk"
    );
}

/// A raw server: `reply` gets the request head and body and returns the bytes to write (or hangs by sleeping).
fn raw(
    reply: impl Fn(&str, &mut TcpStream) + Send + Sync + 'static,
) -> (SocketAddr, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let (h, reply) = (hits.clone(), Arc::new(reply));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (h, reply) = (h.clone(), reply.clone());
            std::thread::spawn(move || {
                let mut s = s;
                s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut buf = vec![];
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                let want: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap_or(0))
                    })
                    .unwrap_or(0);
                let have = buf.len()
                    - buf
                        .windows(4)
                        .position(|w| w == b"\r\n\r\n")
                        .map_or(buf.len(), |p| p + 4);
                let mut left = want.saturating_sub(have);
                while left > 0 {
                    match s.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => left = left.saturating_sub(n),
                    }
                }
                h.fetch_add(1, Ordering::SeqCst);
                reply(&head, &mut s);
            });
        }
    });
    (addr, hits)
}

fn http(status: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let mut v = format!(
        "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    v.extend_from_slice(body);
    v
}

fn config_for(fetch_url: &str, callback_url: &str) -> Value {
    json!({
        "documentType": "word",
        "document": {"fileType": "docx", "key": "file_7_v1_ab", "title": "a.docx", "url": fetch_url,
            "permissions": {"edit": true}},
        "editorConfig": {"mode": "edit", "callbackUrl": callback_url, "user": {"id": "emp-1", "name": "U"}},
        "exp": now() + 3600
    })
}

async fn open_cfg(r: &Router, cfg: &Value) -> (StatusCode, Value) {
    let body = json!({"origin": "https://app.test", "config": {"token": sign(PROV_SECRET.as_bytes(), cfg)}});
    let (s, b) = call(
        r,
        "POST",
        "/ds/session",
        None,
        body.to_string().into_bytes(),
    )
    .await;
    (s, js(&b))
}

fn plain(d: &Path, allow: &str, tweak: impl FnOnce(&mut Docserver)) -> (Arc<Docserver>, Router) {
    let mut ds = make(d, PROV_SECRET, allow, "http://127.0.0.1:19999");
    tweak(&mut ds);
    let ds = Arc::new(ds);
    let r = ds.routes::<()>();
    (ds, r)
}

#[tokio::test]
async fn a_callback_is_ok_only_for_error_0_and_a_silent_provider_is_unknown() {
    let (ok, _) = raw(|_, s| {
        s.write_all(&http(
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{"error":0}"#,
        ))
        .unwrap();
    });
    let (e1, _) = raw(|_, s| {
        s.write_all(&http("200 OK", "", br#"{"error":1}"#)).unwrap();
    });
    let (e500, _) = raw(|_, s| {
        s.write_all(&http("500 Internal Server Error", "", b"boom"))
            .unwrap();
    });
    let (red, _) = raw(|_, s| {
        s.write_all(&http("302 Found", "Location: http://127.0.0.1:1/\r\n", b""))
            .unwrap();
    });
    let (hang, _) = raw(|_, _| std::thread::sleep(Duration::from_secs(8)));
    let d = dir("cbk");
    let allow = [ok, e1, e500, red, hang]
        .map(|a| int(&format!("http://{a}")))
        .join(", ");
    let (ds, r) = plain(&d, &allow, |ds| ds.callback_timeout_s = 2);
    let mut seen = vec![];
    for (name, a) in [
        ("ok", ok),
        ("error1", e1),
        ("500", e500),
        ("redirect", red),
        ("hang", hang),
    ] {
        let cfg = config_for(&format!("http://{ok}/f"), &format!("http://{a}/cb"));
        let (s, v) = open_cfg(&r, &cfg).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let (sid, tok) = (
            v["sid"].as_str().unwrap().to_string(),
            v["token"].as_str().unwrap().to_string(),
        );
        ds.put_version(&sid, &docx("v1"), "one", false, now())
            .unwrap();
        let t = Instant::now();
        let (s, v) = call(
            &r,
            "POST",
            &format!("/ds/session/{sid}/save"),
            Some(&tok),
            docx("v2"),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let v = js(&v);
        seen.push((
            name,
            v["callback"].as_str().unwrap().to_string(),
            v["dirty"].as_bool().unwrap(),
        ));
        if name == "hang" {
            assert!(t.elapsed() < Duration::from_secs(6), "the timeout applies");
        }
    }
    let want = [
        ("ok", "ok", false),
        ("error1", "failed", true),
        ("500", "failed", true),
        ("redirect", "failed", true),
        ("hang", "unknown", true),
    ];
    let want: Vec<_> = want
        .iter()
        .map(|(a, b, c)| (*a, b.to_string(), *c))
        .collect();
    assert_eq!(seen, want);
}

#[tokio::test]
async fn the_callback_carries_both_tokens_and_the_file_url_of_our_internal_listener() {
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    let (a, _) = raw(move |head, s| {
        s2.lock().unwrap().push(head.to_string());
        s.write_all(&http("200 OK", "", br#"{"error":0}"#)).unwrap();
    });
    let d = dir("cbtok");
    let (ds, r) = plain(&d, &int(&format!("http://{a}")), |_| {});
    let (_, v) = open_cfg(
        &r,
        &config_for(&format!("http://{a}/f"), &format!("http://{a}/cb?x=1")),
    )
    .await;
    let (sid, tok) = (
        v["sid"].as_str().unwrap().to_string(),
        v["token"].as_str().unwrap().to_string(),
    );
    ds.put_version(&sid, &docx("v1"), "one", false, now())
        .unwrap();
    call(
        &r,
        "POST",
        &format!("/ds/session/{sid}/save"),
        Some(&tok),
        docx("v2"),
    )
    .await;
    let head = seen.lock().unwrap()[0].clone();
    assert!(head.starts_with("POST /cb?x=1 "), "{head}");
    assert!(
        head.to_ascii_lowercase()
            .contains("authorization: bearer eyj"),
        "{head}"
    );
}

// ---- SSRF (S6, S7) ----

/// A stub resolver: a name maps to a list of answers, one per lookup (the last repeats); an IP literal is itself.
fn stub(
    table: &[(&str, Vec<Vec<&str>>)],
) -> (
    docserver::outbound::Lookup,
    Arc<Mutex<HashMap<String, usize>>>,
) {
    let t: HashMap<String, Vec<Vec<IpAddr>>> = table
        .iter()
        .map(|(n, a)| {
            (
                n.to_string(),
                a.iter()
                    .map(|v| v.iter().map(|i| i.parse().unwrap()).collect())
                    .collect(),
            )
        })
        .collect();
    let calls = Arc::new(Mutex::new(HashMap::new()));
    let c = calls.clone();
    let f: docserver::outbound::Lookup = Arc::new(move |host, _| {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }
        let mut c = c.lock().unwrap();
        let n = c.entry(host.to_string()).or_insert(0);
        let answers = t
            .get(host)
            .ok_or_else(|| std::io::Error::other("no such name"))?;
        let a = answers[(*n).min(answers.len() - 1)].clone();
        *n += 1;
        Ok(a)
    });
    (f, calls)
}

async fn doc_status(r: &Router, ds: &Docserver, fetch_url: &str) -> (StatusCode, String) {
    let cb = format!(
        "{}/x",
        fetch_url.rsplit_once('/').map_or(fetch_url, |x| x.0)
    );
    let (s, v) = open_cfg(r, &config_for(fetch_url, &cb)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (sid, tok) = (v["sid"].as_str().unwrap(), v["token"].as_str().unwrap());
    let (s, b) = call(
        r,
        "GET",
        &format!("/ds/session/{sid}/document"),
        Some(tok),
        vec![],
    )
    .await;
    let _ = ds;
    (s, String::from_utf8_lossy(&b).to_string())
}

#[tokio::test]
async fn names_that_resolve_to_a_denied_range_are_refused_unless_the_entry_is_internal() {
    let denied = [
        "127.0.0.1",
        "10.0.0.5",
        "172.16.0.9",
        "192.168.0.9",
        "169.254.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "0.0.0.0",
        "224.0.0.1",
        "255.255.255.255",
        "::1",
        "fc00::1",
        "fe80::1",
        "fd00:ec2::254",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.5",
        "2002:7f00:1::",
        "64:ff9b::7f00:1",
        "::",
    ];
    let d = dir("ssrf");
    let (lookup, _) = stub(&[("deny.test", vec![vec!["127.0.0.1"]])]);
    let allow = "\"http://deny.test:80\", \"http://127.0.0.1:80\", \"http://[::ffff:127.0.0.1]:80\", \"http://2130706433:80\", \"http://169.254.169.254:80\"";
    let (ds, r) = plain(&d, allow, |ds| ds.lookup = lookup);
    // IP literals and the decimal form, through the same stub/system rules
    for url in [
        "http://127.0.0.1:80/f",
        "http://[::ffff:127.0.0.1]:80/f",
        "http://169.254.169.254:80/latest/meta-data",
    ] {
        let (s, m) = doc_status(&r, &ds, url).await;
        assert_eq!(s, StatusCode::BAD_GATEWAY, "{url}");
        assert!(m.contains("could not be fetched"), "{url}: {m}");
    }
    // every range, behind a name; the answer list is replaced for each case
    for ip in denied {
        let (lookup, _) = stub(&[("deny.test", vec![vec![ip]])]);
        let (ds, r) = plain(&d, allow, |ds| ds.lookup = lookup);
        let (s, m) = doc_status(&r, &ds, "http://deny.test:80/f").await;
        assert_eq!(s, StatusCode::BAD_GATEWAY, "{ip}");
        assert!(m.contains("could not be fetched"), "{ip}: {m}");
    }
    // the decimal form of 127.0.0.1 as getaddrinfo resolves it
    let (lookup, _) = stub(&[("2130706433", vec![vec!["127.0.0.1"]])]);
    let (ds, r) = plain(&d, allow, |ds| ds.lookup = lookup);
    assert_eq!(
        doc_status(&r, &ds, "http://2130706433:80/f").await.0,
        StatusCode::BAD_GATEWAY
    );
    // one denied answer among allowed ones refuses the whole name
    let (lookup, _) = stub(&[("deny.test", vec![vec!["93.184.216.34", "10.0.0.5"]])]);
    let (ds, r) = plain(&d, allow, |ds| ds.lookup = lookup);
    assert_eq!(
        doc_status(&r, &ds, "http://deny.test:80/f").await.0,
        StatusCode::BAD_GATEWAY
    );
}

#[tokio::test]
async fn an_internal_entry_may_reach_a_private_address_but_never_metadata_or_multicast() {
    let (a, hits) = raw(|_, s| {
        s.write_all(&http("200 OK", "", &docx("v1"))).unwrap();
    });
    let port = a.port();
    let d = dir("internal");
    let allow = [
        format!("http://ok.test:{port}"),
        format!("http://meta.test:{port}"),
        format!("http://mixed.test:{port}"),
        format!("http://rebind.test:{port}"),
    ]
    .iter()
    .map(|u| int(u))
    .collect::<Vec<_>>()
    .join(", ");
    let (lookup, calls) = stub(&[
        ("ok.test", vec![vec!["127.0.0.1"]]),
        ("meta.test", vec![vec!["169.254.169.254"]]),
        ("mixed.test", vec![vec!["127.0.0.1", "169.254.169.254"]]),
        // DNS rebinding: an allowed answer first, a denied one after
        (
            "rebind.test",
            vec![vec!["127.0.0.1"], vec!["169.254.169.254"]],
        ),
    ]);
    let (ds, r) = plain(&d, &allow, |ds| ds.lookup = lookup);
    assert_eq!(
        doc_status(&r, &ds, &format!("http://ok.test:{port}/f")).await,
        (
            StatusCode::OK,
            String::from_utf8_lossy(&docx("v1")).to_string()
        )
    );
    let before = hits.load(Ordering::SeqCst);
    for n in ["meta.test", "mixed.test"] {
        let (s, m) = doc_status(&r, &ds, &format!("http://{n}:{port}/f")).await;
        assert_eq!(s, StatusCode::BAD_GATEWAY, "{n}");
        assert!(m.contains("could not be fetched"), "{n}: {m}");
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        before,
        "no connection was made for a refused name"
    );
    // rebinding: the one lookup of the request decided the address that was used
    let (s, _) = doc_status(&r, &ds, &format!("http://rebind.test:{port}/f")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        calls.lock().unwrap()["rebind.test"],
        1,
        "the name is resolved once per request"
    );
    // the next request resolves again and now sees the denied answer
    let (s, _) = doc_status(&r, &ds, &format!("http://rebind.test:{port}/f")).await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn redirects_are_not_followed_and_urls_outside_the_list_are_not_accepted() {
    let (target, target_hits) = raw(|_, s| {
        s.write_all(&http("200 OK", "", &docx("evil"))).unwrap();
    });
    let (red, _) = raw(move |_, s| {
        s.write_all(&http(
            "302 Found",
            &format!("Location: http://{target}/f\r\n"),
            b"",
        ))
        .unwrap();
    });
    let d = dir("redirect");
    let (ds, r) = plain(&d, &int(&format!("http://{red}")), |_| {});
    let (s, m) = doc_status(&r, &ds, &format!("http://{red}/f")).await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
    assert!(m.contains("could not be fetched"), "{m}");
    assert_eq!(
        target_hits.load(Ordering::SeqCst),
        0,
        "the redirect target was never contacted"
    );
    // scheme, user info, wrong port, a host that is not listed
    for url in [
        format!("ftp://{red}/f"),
        format!("http://user:pw@{red}/f"),
        format!("http://127.0.0.1:{}/f", red.port().wrapping_add(1)),
        "http://evil.test:80/f".to_string(),
        format!("https://{red}/f"),
    ] {
        let (s, _) = open_cfg(&r, &config_for(&url, "http://cb.test:80/x")).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{url}");
    }
}

#[tokio::test]
async fn size_cap_zip_bombs_and_wrong_packages_are_refused_on_fetch() {
    let big_header = raw(|_, s| {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 62914560\r\nConnection: close\r\n\r\n");
        std::thread::sleep(Duration::from_secs(1));
    });
    // no Content-Length at all: the count while streaming decides (cap set to 1 MB below)
    let stream = raw(|_, s| {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
        let z = vec![b'P'; 64 << 10];
        for _ in 0..64 {
            if s.write_all(&z).is_err() {
                break;
            }
        }
    });
    let bomb = package(&[
        ("[Content_Types].xml", b"x"),
        ("word/document.xml", b"x"),
        ("word/media/a.bin", &vec![0u8; 40 << 20]),
    ]);
    let bomb_srv = raw(move |_, s| {
        s.write_all(&http("200 OK", "", &bomb)).unwrap();
    });
    let xlsx = package(&[("[Content_Types].xml", b"x"), ("xl/workbook.xml", b"x")]);
    let xlsx_srv = raw(move |_, s| {
        s.write_all(&http("200 OK", "", &xlsx)).unwrap();
    });
    let not_zip = raw(|_, s| {
        s.write_all(&http("200 OK", "", b"MZ not a package"))
            .unwrap();
    });
    let missing = raw(|_, s| {
        s.write_all(&http("404 Not Found", "", b"no")).unwrap();
    });
    let d = dir("size");
    let all = [
        &big_header,
        &stream,
        &bomb_srv,
        &xlsx_srv,
        &not_zip,
        &missing,
    ];
    let allow = all
        .iter()
        .map(|x| int(&format!("http://{}", x.0)))
        .collect::<Vec<_>>()
        .join(", ");
    let (ds, r) = plain(&d, &allow, |ds| ds.max_document_mb = 1);
    // (the lying/absent length cases run with a 1 MB cap; the 60 MB header is over even the default 50)
    let cases = [
        (&big_header, StatusCode::PAYLOAD_TOO_LARGE),
        (&stream, StatusCode::PAYLOAD_TOO_LARGE),
        (&bomb_srv, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (&xlsx_srv, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (&not_zip, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (&missing, StatusCode::BAD_GATEWAY),
    ];
    for (i, (srv, want)) in cases.iter().enumerate() {
        let (s, m) = doc_status(&r, &ds, &format!("http://{}/f", srv.0)).await;
        assert_eq!(s, *want, "case {i}: {m}");
    }
}

#[tokio::test]
async fn a_stalled_body_runs_into_the_total_timeout() {
    let (a, _) = raw(|_, s| {
        let _ = s.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nPK\x03\x04",
        );
        std::thread::sleep(Duration::from_secs(10));
    });
    let d = dir("slow");
    let (ds, r) = plain(&d, &int(&format!("http://{a}")), |ds| {
        ds.fetch_timeout_s = 2
    });
    let t = Instant::now();
    let (s, m) = doc_status(&r, &ds, &format!("http://{a}/f")).await;
    assert_eq!(s, StatusCode::BAD_GATEWAY, "{m}");
    assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());
}

#[tokio::test]
async fn save_bodies_are_capped_and_checked() {
    let (a, _) = raw(|_, s| {
        s.write_all(&http("200 OK", "", br#"{"error":0}"#)).unwrap();
    });
    let d = dir("savebody");
    let (ds, r) = plain(&d, &int(&format!("http://{a}")), |_| {});
    let (_, v) = open_cfg(
        &r,
        &config_for(&format!("http://{a}/f"), &format!("http://{a}/cb")),
    )
    .await;
    let (sid, tok) = (
        v["sid"].as_str().unwrap().to_string(),
        v["token"].as_str().unwrap().to_string(),
    );
    ds.put_version(&sid, &docx("v1"), "one", false, now())
        .unwrap();
    let save = |route: &'static str, b: Vec<u8>| {
        let (r, sid, tok) = (r.clone(), sid.clone(), tok.clone());
        async move {
            call(
                &r,
                "POST",
                &format!("/ds/session/{sid}/{route}"),
                Some(&tok),
                b,
            )
            .await
            .0
        }
    };
    // 51 MB: over the 50 MB cap, for save and autosave
    let mut big = docx("x");
    big.resize((51 << 20) + 1, 0);
    assert_eq!(
        save("save", big.clone()).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(save("autosave", big).await, StatusCode::PAYLOAD_TOO_LARGE);
    // a zip bomb (a 40 MB run of zeros in a few hundred bytes), a renamed spreadsheet, and plain junk
    let bomb = package(&[
        ("[Content_Types].xml", b"x"),
        ("word/document.xml", b"x"),
        ("word/media/a.bin", &vec![0u8; 40 << 20]),
    ]);
    let xlsx = package(&[("[Content_Types].xml", b"x"), ("xl/workbook.xml", b"x")]);
    for (n, b) in [
        ("bomb", bomb),
        ("xlsx", xlsx),
        ("junk", b"PK\x03\x04 junk".to_vec()),
    ] {
        assert_eq!(
            save("save", b.clone()).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{n}"
        );
        assert_eq!(
            save("autosave", b).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{n}"
        );
    }
    assert_eq!(
        ds.load_session(&sid).unwrap().versions.len(),
        1,
        "nothing was stored"
    );
}

#[tokio::test]
async fn callback_round_trip_on_loopback() {
    let e = env("rtt", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    let mut ms = vec![];
    for i in 0..20 {
        let t = Instant::now();
        let (_, v) = e.post(&sid, &tok, "save", docx(&format!("v{i}"))).await;
        assert_eq!(v["callback"], "ok", "{v}");
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!(
        "MEASURED save + status 6 callback + harness download, 20 runs on loopback: median {:.1} ms, max {:.1} ms",
        ms[10], ms[19]
    );
    assert!(ms[10] < 1000.0);
}

// ---- WP8: the audit log and the rate limits (S15, S16, S17) ----

fn audit_lines(d: &Path) -> Vec<Value> {
    std::fs::read_to_string(d.join("data/audit.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn the_audit_chain_covers_a_full_flow_verifies_and_holds_no_secret() {
    let e = env("audit", |_| {}).await;
    let (sid, tok, cfg) = e.open_cfg().await;
    // one refused call before the flow, from a named client
    let (s, _) = call_from(
        &e.r,
        Some("203.0.113.9"),
        "POST",
        &format!("/ds/session/{sid}/renew"),
        Some("not.a.token"),
        vec![],
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(e.get_doc(&sid, &tok).await.0, StatusCode::OK);
    assert_eq!(e.post(&sid, &tok, "dirty", vec![]).await.0, StatusCode::OK);
    assert_eq!(
        e.post(&sid, &tok, "save", docx("v2")).await.0,
        StatusCode::OK
    );
    assert_eq!(e.post(&sid, &tok, "close", vec![]).await.0, StatusCode::OK);

    // S16: the keyed chain verifies, the events are those of the flow, and a rewritten line fails
    let log = e.d.join("data/audit.jsonl");
    let key = convert::audit::load_key(&e.d.join("audit.key")).unwrap();
    let v = convert::audit::verify_keyed(&log, Some(&key)).unwrap();
    let l = audit_lines(&e.d);
    let events: Vec<_> = l.iter().map(|x| x["event"].as_str().unwrap()).collect();
    assert_eq!(v.lines as usize, events.len());
    assert_eq!(
        events,
        [
            "session.create",
            "auth.fail",
            "document.fetch",
            "callback",
            "save",
            "callback",
            "close",
            "callback"
        ]
    );
    assert!(
        convert::audit::verify(&log).is_err(),
        "an unkeyed check must refuse a keyed log"
    );
    let good = std::fs::read_to_string(&log).unwrap();
    std::fs::write(&log, good.replacen("\"status\":6", "\"status\":2", 1)).unwrap();
    assert!(
        convert::audit::verify_keyed(&log, Some(&key))
            .unwrap_err()
            .contains("line 6")
    );
    std::fs::write(&log, &good).unwrap();

    // what the lines say
    let find = |ev: &str| l.iter().find(|x| x["event"] == ev).unwrap().clone();
    let c = find("session.create");
    assert_eq!(
        (
            c["provider"].as_str(),
            c["user"].as_str(),
            c["mode"].as_str()
        ),
        (Some("one"), Some("emp-1"), Some("edit"))
    );
    assert_eq!(c["file"], "report.docx");
    assert_eq!(c["key_ref"].as_str().unwrap().len(), 16);
    assert_eq!(find("auth.fail")["client"], "203.0.113.9");
    let f = find("document.fetch");
    assert_eq!(
        (f["result"].as_str(), f["size"].as_u64()),
        (Some("ok"), Some(docx("v1").len() as u64))
    );
    assert_eq!(f["host"].as_str(), Some(e.hrm.base_url()));
    assert_eq!(f["sha256"].as_str().unwrap().len(), 64);
    let cbs: Vec<_> = l
        .iter()
        .filter(|x| x["event"] == "callback")
        .map(|x| (x["status"].as_u64().unwrap(), x["result"].as_str().unwrap()))
        .collect();
    assert_eq!(cbs, [(1, "ok"), (6, "ok"), (2, "ok")]);
    assert_eq!(find("save")["version"], 2);
    assert_eq!(find("close")["status"], 2);

    // S17: no token, secret, session id, query string, URL token or document byte in the log
    let text = std::fs::read_to_string(&log).unwrap();
    let file_token = cfg["document"]["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    for needle in [
        sid.as_str(),
        tok.as_str(),
        cfg["token"].as_str().unwrap(),
        file_token.as_str(),
        "not.a.token",
        PROV_SECRET,
        SERVER_SECRET,
        AUDIT_KEY,
        "Bearer",
        "/ds/files",
        "synthetic",
    ] {
        assert!(!text.contains(needle), "the audit log holds {needle}");
    }
    assert!(!text.contains('?'), "a query string in the audit log");
    assert!(text.contains(&docserver::audit::sid_ref(&sid)));
}

#[tokio::test]
async fn redact_leaves_the_title_out_of_the_audit_log() {
    let d = dir("redact");
    let log = convert::audit::AuditLog::open(&d.join("r.jsonl"), true).unwrap();
    log.append(
        "session.create",
        json!({"file": "secret-name.docx", "user": "u"}),
    )
    .unwrap();
    let text = std::fs::read_to_string(d.join("r.jsonl")).unwrap();
    assert!(!text.contains("secret-name") && text.contains("\"user\""));
}

#[tokio::test]
async fn session_creation_and_failed_auth_are_limited_per_client_address() {
    let e = env("limits", |ds| {
        let log = convert::audit::AuditLog::open(&ds.data_dir.join("limits.jsonl"), false).unwrap();
        ds.watch = Arc::new(docserver::audit::Watch::new(Some(log), 3, 3));
    })
    .await;
    let lines = |e: &Env| {
        std::fs::read_to_string(e.ds.data_dir.join("limits.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .collect::<Vec<_>>()
    };
    let (sid, tok, cfg) = e.open_cfg().await; // the anonymous client's first create
    let body = json!({"origin": "https://app.test", "config": cfg})
        .to_string()
        .into_bytes();
    // S15, create: 3 per minute per client; the 4th is 429, another client is not affected
    let mut codes = vec![];
    for _ in 0..4 {
        let c = Some("198.51.100.1");
        codes.push(
            call_from(&e.r, c, "POST", "/ds/session", None, body.clone())
                .await
                .0
                .as_u16(),
        );
    }
    assert_eq!(codes, [200, 200, 200, 429]);
    let c2 = Some("198.51.100.2");
    assert_eq!(
        call_from(&e.r, c2, "POST", "/ds/session", None, body.clone())
            .await
            .0,
        StatusCode::OK
    );
    // failed auth: 3 per minute per client, then 429 instead of 401; the same client's valid token still works (only failures count)
    let renew = format!("/ds/session/{sid}/renew");
    let bad = |c: &'static str| {
        let (r, uri) = (e.r.clone(), renew.clone());
        async move {
            call_from(&r, Some(c), "POST", &uri, Some("x.y.z"), vec![])
                .await
                .0
                .as_u16()
        }
    };
    let mut codes = vec![];
    for _ in 0..5 {
        codes.push(bad("192.0.2.7").await);
    }
    assert_eq!(codes, [401, 401, 401, 429, 429]);
    assert_eq!(bad("192.0.2.8").await, 401);
    let (s, _) = call_from(&e.r, Some("192.0.2.7"), "POST", &renew, Some(&tok), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    // the audit log: 3 auth.fail lines for .7, and ONE limit.hit per kind and client however many requests were refused
    let l = lines(&e);
    let hits: Vec<_> = l
        .iter()
        .filter(|x| x["event"] == "limit.hit")
        .map(|x| (x["kind"].as_str().unwrap(), x["client"].as_str().unwrap()))
        .collect();
    assert_eq!(hits, [("create", "198.51.100.1"), ("auth", "192.0.2.7")]);
    let fails = |c: &str| {
        l.iter()
            .filter(|x| x["event"] == "auth.fail" && x["client"] == c)
            .count()
    };
    assert_eq!((fails("192.0.2.7"), fails("192.0.2.8")), (3, 1));
}

#[tokio::test]
async fn a_refused_fetch_is_an_ssrf_block_in_the_audit_log() {
    let d = dir("ssrf-audit");
    let (ds, r) = plain(&d, "\"http://127.0.0.1:80\"", |_| {});
    let (s, _) = doc_status(&r, &ds, "http://127.0.0.1:80/f").await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
    let l = audit_lines(&d);
    let b = l
        .iter()
        .find(|x| x["event"] == "ssrf.block")
        .expect("an ssrf.block line");
    assert_eq!(
        (b["phase"].as_str(), b["host"].as_str()),
        (Some("fetch"), Some("http://127.0.0.1:80"))
    );
    let key = convert::audit::load_key(&d.join("audit.key")).unwrap();
    convert::audit::verify_keyed(&d.join("data/audit.jsonl"), Some(&key)).unwrap();
}

/// A crash between `close_session` and `callback_closed` leaves `closed` with `notified = false`: the next start sends status 2 once.
#[tokio::test]
async fn a_close_that_died_before_its_callback_sends_status_2_once_at_start() {
    let mut e = env("crash-close", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    let v2 = docx("v2");
    e.ds.put_version(&sid, &v2, "two", true, now()).unwrap();
    e.ds.close_session(&sid, now()).unwrap();
    assert!(e.hrm.received().is_empty());
    e.restart();
    let ds = e.ds.clone();
    let n = tokio::task::spawn_blocking(move || ds.recover_unnotified())
        .await
        .unwrap();
    assert_eq!(n, 1);
    let got = e.hrm.received();
    assert_eq!((got.len(), got[0].status, got[0].bytes), (1, 2, v2.len()));
    assert!(e.ds.queued(&sid).is_none());
    // a second start finds it notified and acknowledged
    let ds = e.ds.clone();
    let n = tokio::task::spawn_blocking(move || ds.recover_unnotified())
        .await
        .unwrap();
    assert_eq!((n, e.hrm.received().len()), (0, 1));
}

/// The page went away before its save (ONLYOFFICE assembles the last state at close): a recovery copy newer than the last version and
/// different from it becomes a version, and the close is a status 2 with exactly those bytes. Same bytes, or a save after it: as before.
#[tokio::test]
async fn a_close_promotes_a_newer_different_autosave_and_sends_status_2_with_it() {
    let e = env("promote", |_| {}).await;
    let statuses = |e: &Env| {
        e.hrm
            .received()
            .iter()
            .map(|r| r.status)
            .collect::<Vec<_>>()
    };
    // typed, autosaved, never saved: status 2 with the autosave's bytes
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    let typed = docx("typed, never saved");
    assert_eq!(
        e.post(&sid, &tok, "autosave", typed.clone()).await.0,
        StatusCode::OK
    );
    let (s, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(
        (
            s,
            v["status"].as_u64(),
            v["callback"].as_str(),
            v["dirty"].as_bool()
        ),
        (StatusCode::OK, Some(2), Some("ok"), Some(false)),
        "{v}"
    );
    assert_eq!(statuses(&e), [2]);
    let got = &e.hrm.received()[0];
    assert_eq!(got.sha256, docserver::sessions::sha256_hex(&typed));
    assert_eq!(e.hrm.object(Kind::File, 7), Some(typed.clone()));
    let rec = e.ds.load_session(&sid).unwrap();
    assert_eq!(
        rec.versions
            .iter()
            .map(|v| (v.n, v.by.as_str()))
            .collect::<Vec<_>>(),
        [(1, "one"), (2, rec.user_id.as_str())]
    );
    assert!(rec.saved_since_open && e.ds.queued(&sid).is_none());
    // the version is a real one: in the history too, and audited as a save from the autosave
    assert!(
        audit_lines(&e.d)
            .iter()
            .any(|l| l["event"] == "save" && l["from"] == "autosave"),
        "{:?}",
        audit_lines(&e.d)
    );

    // an autosave with the bytes of the last version: nothing changed, status 4
    let (sid, tok) = e.open().await;
    assert_eq!(e.get_doc(&sid, &tok).await.1, typed);
    e.post(&sid, &tok, "autosave", typed.clone()).await;
    let (_, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(
        (v["status"].as_u64(), v["callback"].as_str()),
        (Some(4), Some("ok")),
        "{v}"
    );
    // (the harness records the statuses that carry a file only)
    assert_eq!(statuses(&e), [2]);
    assert_eq!(e.ds.load_session(&sid).unwrap().versions.len(), 1);

    // an autosave older than the last save is not used: status 2 with the save
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    e.post(&sid, &tok, "autosave", docx("older")).await;
    let saved = docx("saved after the autosave");
    e.post(&sid, &tok, "save", saved.clone()).await;
    let (_, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(v["status"].as_u64(), Some(2), "{v}");
    assert_eq!(statuses(&e), [2, 6, 2]);
    assert_eq!(
        e.hrm.received()[2].sha256,
        docserver::sessions::sha256_hex(&saved)
    );
    assert_eq!(e.ds.load_session(&sid).unwrap().versions.len(), 2);
    assert!(e.hrm.rejected().is_empty(), "{:?}", e.hrm.rejected());
}

/// An edit session whose page vanished without a close (the host removed the frame) ends `close_idle_s` after its last activity, like
/// a close from the page. A ping keeps it open; a view session is never touched.
#[tokio::test]
async fn an_edit_session_without_its_page_is_closed_when_idle() {
    let e = env("idle", |_| {}).await;
    let idle = |at: u64| {
        let ds = e.ds.clone();
        async move {
            tokio::task::spawn_blocking(move || ds.close_idle(at))
                .await
                .unwrap()
        }
    };
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    let (vsid, vtok, _) = e.open_q("mode=view").await;
    e.get_doc(&vsid, &vtok).await;
    let typed = docx("typed, then the page went away");
    e.post(&sid, &tok, "autosave", typed.clone()).await;
    let t = e.ds.load_session(&sid).unwrap().last_activity;
    assert_eq!(e.ds.close_idle_s, 300);
    assert_eq!(idle(t + 299).await, 0);
    // a ping is activity: the 5 minutes count from it
    let (s, _) = e.post(&sid, &tok, "ping", vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let t = e.ds.load_session(&sid).unwrap().last_activity;
    assert_eq!(idle(t + 299).await, 0);
    assert!(e.hrm.received().is_empty());
    assert_eq!(idle(t + 300).await, 1);
    let got = e.hrm.received();
    assert_eq!((got.len(), got[0].status), (1, 2));
    assert_eq!(e.hrm.object(Kind::File, 7), Some(typed));
    // closed now: the page's late ping and close find nothing to do, and no second callback goes out
    assert_eq!(
        e.post(&sid, &tok, "ping", vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, v) = e.post(&sid, &tok, "close", vec![]).await;
    assert_eq!(v["status"].as_u64(), Some(2));
    assert_eq!(idle(t + 3600).await, 0);
    assert_eq!(e.hrm.received().len(), 1);
    // the view session: still open, nothing sent
    assert!(e.ds.load_session(&vsid).unwrap().closed.is_none());
    assert!(
        audit_lines(&e.d)
            .iter()
            .any(|l| l["event"] == "close" && l["reason"] == "idle")
    );
}

/// A crash after the idle close stored the promoted version and the close, before the callback: the next start sends status 2 with it.
#[tokio::test]
async fn an_idle_close_that_died_before_its_callback_sends_status_2_at_start() {
    let mut e = env("idle-crash", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    let typed = docx("autosaved before the crash");
    e.post(&sid, &tok, "autosave", typed.clone()).await;
    let t = e.ds.load_session(&sid).unwrap().last_activity;
    // what close_idle does before its callback
    let c = e.ds.close_if_idle(&sid, t + 300).unwrap();
    assert_eq!(c.status, 2);
    assert!(e.hrm.received().is_empty());
    e.restart();
    // the sweep at start keeps it (a status 2 is owed), whatever its age
    assert_eq!(e.ds.sweep(t + 400 * 86_400), 0);
    let ds = e.ds.clone();
    assert_eq!(
        tokio::task::spawn_blocking(move || ds.recover_unnotified())
            .await
            .unwrap(),
        1
    );
    let got = e.hrm.received();
    assert_eq!((got.len(), got[0].status), (1, 2));
    assert_eq!(e.hrm.object(Kind::File, 7), Some(typed));
}

/// The sweep never deletes an open edit session, however old: the idle close runs first and sends the callback, then the usual rules.
#[tokio::test]
async fn the_sweep_waits_for_the_idle_close() {
    let e = env("idle-sweep", |_| {}).await;
    let (sid, tok) = e.open().await;
    e.get_doc(&sid, &tok).await;
    e.post(&sid, &tok, "autosave", docx("left behind")).await;
    let later = e.ds.load_session(&sid).unwrap().last_activity + 30 * 86_400;
    assert_eq!(e.ds.sweep(later), 0);
    assert!(e.ds.load_session(&sid).is_some());
    let ds = e.ds.clone();
    assert_eq!(
        tokio::task::spawn_blocking(move || ds.close_idle(later))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        e.hrm
            .received()
            .iter()
            .map(|r| r.status)
            .collect::<Vec<_>>(),
        [2]
    );
    // a good close: kept keep_closed_hours (7 days) from the close, then gone
    assert_eq!(e.ds.sweep(later + 86_400), 0);
    assert_eq!(e.ds.sweep(later + 8 * 86_400), 1);
    assert!(e.ds.load_session(&sid).is_none());
}

/// `ui_lang` of the provider is handed to the wrapper in the session answer (it wins over `editorConfig.lang` there); unset: null.
#[tokio::test]
async fn the_session_answer_carries_the_providers_ui_lang() {
    for (lang, want) in [(Some("uz-Latn"), json!("uz-Latn")), (None, Value::Null)] {
        let e = env(&format!("lang-{}", lang.unwrap_or("none")), |ds| {
            ds.providers[0].ui_lang = lang.map(String::from)
        })
        .await;
        let url = format!("{}/files/7/editor-config", e.hrm.base_url());
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
        let body = json!({"origin": "https://app.test", "config": cfg}).to_string();
        let (s, b) = call(&e.r, "POST", "/ds/session", None, body.into_bytes()).await;
        assert_eq!((s, &js(&b)["lang"]), (StatusCode::OK, &want));
    }
}
