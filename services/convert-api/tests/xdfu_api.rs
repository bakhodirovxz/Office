//! WP15 through the API: `xdfu = "off" | "watermark" | "refuse"` in the server config, against the stand-in tool (text outputs, so
//! the watermark is a first line that the test can read back). The real-tool matrix lives in `services/convert/tests/xdfu.rs`.
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use convert::audit;
use convert::exec::Config as ConvertConfig;
use convert::plan::ToolId;
use convert_api::app::App;
use convert_api::config::Settings;
use convert_api::{jwt, routes};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef-secret";
const GRIF: &[u8] = "ДСП\n\nSalom dunyo, bu sinov matni.\n".as_bytes();
const PLAIN: &[u8] = "Salom dunyo, bu oddiy matn.\n".as_bytes();

struct H {
    dir: PathBuf,
    app: Arc<App>,
    router: Router,
    host: &'static str,
    audit: PathBuf,
}

impl Drop for H {
    fn drop(&mut self) {
        self.app.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
    fn new(name: &str, server: bool, f: impl FnOnce(&mut Settings)) -> H {
        let dir = std::env::temp_dir().join(format!("capix-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let audit = dir.join("audit").join("xdfu.jsonl");
        let mut s = Settings {
            work_dir: Some(dir.join("work")),
            store_dir: Some(dir.join("store")),
            xdfu_audit_file: Some(audit.clone()),
            ..Settings::default()
        };
        if server {
            std::fs::write(dir.join("secret"), SECRET).unwrap();
            s.auth = "jwt".into();
            s.secret_file = Some(dir.join("secret"));
            s.tls_cert = Some(dir.join("cert.pem"));
            s.tls_key = Some(dir.join("key.pem"));
            s.public_url = Some("https://convert.test".into());
        }
        f(&mut s);
        let app = App::new(s.resolve().unwrap(), fake_cfg(), server).unwrap();
        H {
            dir,
            router: routes::router(app.clone()),
            app,
            host: if server {
                "convert.test"
            } else {
                "127.0.0.1:8787"
            },
            audit,
        }
    }

    fn token(sub: &str) -> String {
        jwt::sign(
            SECRET,
            &json!({"sub": sub, "exp": jwt::now() + 600, "scope": "convert"}),
        )
    }

    async fn send(&self, mut r: Request<Body>, peer: &str) -> (StatusCode, Vec<u8>) {
        r.extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let res = self.router.clone().oneshot(r).await.unwrap();
        (
            res.status(),
            axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
    }

    fn req(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<(String, Vec<u8>)>,
    ) -> Request<Body> {
        let mut r = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", self.host);
        if let Some(t) = token {
            r = r.header("authorization", format!("Bearer {t}"));
        }
        match body {
            Some((ct, b)) => r
                .header("content-type", ct)
                .header("content-length", b.len())
                .body(Body::from(b))
                .unwrap(),
            None => r.body(Body::empty()).unwrap(),
        }
    }

    /// Post one file, wait for the job, and return (the status JSON, the first output's bytes if it has one).
    async fn convert(
        &self,
        name: &str,
        data: &[u8],
        to: &str,
        token: Option<&str>,
        peer: &str,
    ) -> (Value, Option<Vec<u8>>) {
        let b = "----capix";
        let mut body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{b}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{{\"to\":\"{to}\"}}\r\n--{b}--\r\n").as_bytes());
        let (st, v) = self
            .send(
                self.req(
                    "POST",
                    "/api/v1/jobs",
                    token,
                    Some((format!("multipart/form-data; boundary={b}"), body)),
                ),
                peer,
            )
            .await;
        assert_eq!(st, StatusCode::ACCEPTED, "{}", String::from_utf8_lossy(&v));
        let id = serde_json::from_slice::<Value>(&v).unwrap()["job"]
            .as_str()
            .unwrap()
            .to_string();
        let t = Instant::now();
        let status = loop {
            let (st, v) = self
                .send(
                    self.req("GET", &format!("/api/v1/jobs/{id}"), token, None),
                    peer,
                )
                .await;
            assert_eq!(st, StatusCode::OK);
            let v: Value = serde_json::from_slice(&v).unwrap();
            if v["state"] == "done" {
                break v;
            }
            assert!(t.elapsed() < Duration::from_secs(60), "never finished: {v}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let out = if status["files"][0]["state"] == "done" {
            Some(
                self.send(
                    self.req("GET", &format!("/api/v1/jobs/{id}/files/0/0"), token, None),
                    peer,
                )
                .await
                .1,
            )
        } else {
            None
        };
        (status, out)
    }

    fn lines(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.audit)
            .map(|t| {
                t.lines()
                    .map(|l| serde_json::from_str(l).unwrap())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn watermark_marks_a_grif_document_logs_it_and_leaves_a_plain_one_alone() {
    let h = H::new("wm", true, |s| s.xdfu = "watermark".into());
    let t = H::token("alice");
    let (st, out) = h
        .convert("buyruq.txt", GRIF, "txt", Some(&t), "203.0.113.9:4000")
        .await;
    assert_eq!(st["files"][0]["state"], "done", "{st}");
    let out = String::from_utf8(out.unwrap()).unwrap();
    let mut lines = out.lines();
    let mark = lines.next().unwrap();
    assert!(
        mark.starts_with("XDFU copy 1 | 203.0.113.9 | alice | 20") && mark.ends_with(" UTC"),
        "{mark}"
    );
    assert_eq!(
        lines.next(),
        Some("stub"),
        "the converted text follows the mark"
    );
    // the report says so, with the evidence
    let x = &st["files"][0]["xdfu"];
    assert_eq!(
        (x["detected"].clone(), x["copy"].clone(), x["mode"].clone()),
        (json!(true), json!(1), json!("watermark")),
        "{st}"
    );
    assert_eq!(x["evidence"][0]["marker"], "ДСП");
    assert!(
        st["files"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "xdfu_copy"),
        "{st}"
    );
    // the audit log: who, from where, which job and file, which copy; no content
    let log = h.lines();
    assert_eq!(log.len(), 1, "{log:?}");
    let l = &log[0];
    assert_eq!(
        (
            l["user"].clone(),
            l["client"].clone(),
            l["file"].clone(),
            l["seq"].clone(),
            l["event"].clone()
        ),
        (
            json!("alice"),
            json!("203.0.113.9"),
            json!("buyruq.txt"),
            json!(1),
            json!("copy")
        ),
        "{l}"
    );
    assert!(
        l["job"].as_str().unwrap().len() > 32 && l["ts"].as_str().unwrap().ends_with('Z'),
        "{l}"
    );
    assert!(
        !std::fs::read_to_string(&h.audit)
            .unwrap()
            .contains("Salom dunyo")
    );
    // a plain document: converted as ever, no mark, no key in the status, no new audit line
    let (st, out) = h
        .convert("oddiy.txt", PLAIN, "txt", Some(&t), "203.0.113.9:4000")
        .await;
    assert_eq!(out.unwrap(), b"stub\n");
    assert!(st["files"][0].get("xdfu").is_none(), "{st}");
    assert_eq!(h.lines().len(), 1);
    // a second grif document is copy 2, another user and address are recorded as such
    let (_, out) = h
        .convert(
            "boshqa.txt",
            GRIF,
            "txt",
            Some(&H::token("bob")),
            "198.51.100.7:1",
        )
        .await;
    assert!(
        String::from_utf8(out.unwrap())
            .unwrap()
            .starts_with("XDFU copy 2 | 198.51.100.7 | bob | ")
    );
    assert_eq!(audit::verify(&h.audit).unwrap().lines, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn refuse_vetoes_the_job_with_a_clear_message_and_cannot_carry_is_refused_too() {
    let h = H::new("refuse", true, |s| s.xdfu = "refuse".into());
    let t = H::token("alice");
    let (st, out) = h
        .convert("buyruq.txt", GRIF, "txt", Some(&t), "203.0.113.9:1")
        .await;
    assert!(out.is_none());
    assert_eq!(
        (
            st["files"][0]["state"].clone(),
            st["files"][0]["code"].clone()
        ),
        (json!("failed"), json!(9)),
        "{st}"
    );
    assert!(
        st["files"][0]["message"]
            .as_str()
            .unwrap()
            .contains("refuse"),
        "{st}"
    );
    assert_eq!(st["files"][0]["xdfu"]["refused"], "mode");
    assert_eq!(h.lines()[0]["event"], "refused");
    assert_eq!(
        h.convert("oddiy.txt", PLAIN, "txt", Some(&t), "203.0.113.9:1")
            .await
            .1
            .unwrap(),
        b"stub\n"
    );
    // watermark mode, an output that cannot carry a mark: refused before any tool runs
    let h = H::new("nocarry", true, |s| s.xdfu = "watermark".into());
    let t = H::token("alice");
    let (st, out) = h
        .convert("buyruq.txt", GRIF, "odt", Some(&t), "203.0.113.9:1")
        .await;
    assert!(out.is_none());
    assert_eq!(st["files"][0]["code"], 3, "{st}");
    assert!(
        st["files"][0]["message"]
            .as_str()
            .unwrap()
            .contains("watermark"),
        "{st}"
    );
    assert_eq!(h.lines()[0]["reason"], "cannot_carry");
}

#[tokio::test(flavor = "multi_thread")]
async fn off_changes_nothing_and_writes_nothing() {
    let h = H::new("off", true, |_| {});
    let (st, out) = h
        .convert(
            "buyruq.txt",
            GRIF,
            "txt",
            Some(&H::token("alice")),
            "203.0.113.9:1",
        )
        .await;
    assert_eq!(out.unwrap(), b"stub\n", "no mark");
    assert!(
        st["files"][0].get("xdfu").is_none()
            && st["files"][0]["warnings"].as_array().unwrap().is_empty(),
        "{st}"
    );
    assert!(
        !h.audit.exists() && !h.audit.parent().unwrap().exists(),
        "no audit file, not even its directory"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn redact_keeps_the_file_name_out_and_local_mode_names_the_os_user_and_this_computer() {
    let h = H::new("local", false, |s| {
        s.xdfu = "watermark".into();
        s.xdfu_redact = true;
    });
    let (_, out) = h
        .convert("maxfiy-nom.txt", GRIF, "txt", None, "127.0.0.1:5000")
        .await;
    let out = String::from_utf8(out.unwrap()).unwrap();
    let mark = out.lines().next().unwrap();
    let os_user = convert::xdfu::os_user();
    assert!(
        mark.contains(&format!("| {os_user} |")),
        "{mark} (the OS user is {os_user})"
    );
    assert!(
        !mark.contains("| 127.0.0.1 |") && !mark.contains("| local |"),
        "{mark}"
    );
    let text = std::fs::read_to_string(&h.audit).unwrap();
    assert!(
        !text.contains("maxfiy-nom") && !text.contains("\"file\""),
        "{text}"
    );
    assert_eq!(h.lines()[0]["user"], json!(os_user));
}

#[test]
fn the_config_checks_the_mode_and_wants_an_audit_file_in_server_mode() {
    let e = Settings {
        xdfu: "on".into(),
        ..Settings::default()
    }
    .resolve()
    .err()
    .unwrap();
    assert!(
        e.contains("off") && e.contains("watermark") && e.contains("refuse"),
        "{e}"
    );
    let dir = std::env::temp_dir().join(format!("capix-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("secret"), [b's'; 40]).unwrap();
    let server = |audit: Option<PathBuf>| {
        Settings {
            auth: "jwt".into(),
            secret_file: Some(dir.join("secret")),
            tls_cert: Some("c.pem".into()),
            tls_key: Some("k.pem".into()),
            public_url: Some("https://convert.test".into()),
            xdfu: "refuse".into(),
            xdfu_audit_file: audit,
            ..Settings::default()
        }
        .resolve()
        .err()
    };
    assert!(server(None).unwrap().contains("xdfu_audit_file"));
    assert!(server(Some(dir.join("a.jsonl"))).is_none());
    // local mode may use the per-user default
    assert!(
        Settings {
            xdfu: "watermark".into(),
            ..Settings::default()
        }
        .resolve()
        .is_ok()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
