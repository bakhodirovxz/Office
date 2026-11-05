//! The binary against the real tools. (1) `auth = "none"` off loopback and `xdfu` other than "off" refuse to start. (2) Every
//! direction of `GET /formats` runs once over HTTP on one synthetic file, then the server is killed hard and the next start
//! leaves `work/` empty. Skipped with a message when the WP1 inputs or the release tools are not on this machine; set
//! `CONVERT_API_E2E_REQUIRED=1` to make that a failure.
use serde_json::{Value, json};
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const INPUTS: &str = "E:/office-target/p3/census/in";
const BIN: &str = env!("CARGO_BIN_EXE_convert-api");

/// Where the release domain CLIs are built: `$OFFICE_TARGET`, default `E:/office-target/p1` (as in services/convert/tests).
fn tool(dom: &str) -> String {
    let target = std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into());
    format!("{target}/{dom}/release/{dom}-cli.exe")
}

fn sandbox(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("capi-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn bad_configurations_refuse_to_start() {
    let d = sandbox("refuse");
    for (what, toml, needle) in [
        (
            "auth none on all interfaces",
            "auth = \"none\"\nlisten = \"0.0.0.0:18787\"\n",
            "loopback",
        ),
        (
            "auth none on a LAN address",
            "listen = \"192.168.1.20:18787\"\n",
            "loopback",
        ),
        ("xdfu on", "xdfu = \"on\"\n", "watermark"),
        ("jwt without tls", "auth = \"jwt\"\n", "secret_file"),
        ("unknown key", "bogus = 1\n", "bogus"),
    ] {
        let cfg = d.join("c.toml");
        std::fs::write(
            &cfg,
            format!("work_dir = '{}'\n{toml}", d.join("work").display()),
        )
        .unwrap();
        let out = Command::new(BIN)
            .arg("--config")
            .arg(&cfg)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{what}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{what}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn start(dir: &Path, port: u16) -> Server {
        let cfg = dir.join("server.toml");
        std::fs::write(
            &cfg,
            format!(
                "listen = '127.0.0.1:{port}'\nwork_dir = '{}'\nstore_dir = '{}'\nper_user_active = 8\nrate_user_per_min = 100000\nrate_ip_per_min = 100000\nqueue_max = 100\n",
                dir.join("work").display(),
                dir.join("store").display()
            ),
        )
        .unwrap();
        let mut cmd = Command::new(BIN);
        cmd.arg("--config")
            .arg(&cfg)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (var, dom) in [
            ("DOCS", "docs"),
            ("SHEETS", "sheets"),
            ("SLIDES", "slides"),
            ("PDF", "pdf"),
        ] {
            cmd.env(format!("CONVERT_{var}_CLI"), tool(dom));
        }
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../upstream/craft-fonts");
        if fonts.is_dir() {
            cmd.env("CRAFT_FONTS_DIR", fonts.canonicalize().unwrap());
        }
        let child = cmd.spawn().unwrap();
        let t = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                t.elapsed() < Duration::from_secs(30),
                "the server did not start"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        Server { child, port }
    }
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill();
    }
}

struct Client {
    agent: ureq::Agent,
    base: String,
}

impl Client {
    fn new(port: u16) -> Client {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(600)))
            .build()
            .into();
        Client {
            agent,
            base: format!("http://127.0.0.1:{port}"),
        }
    }
    fn get(&self, path: &str) -> (u16, Vec<u8>) {
        let mut r = self
            .agent
            .get(format!("{}{path}", self.base))
            .call()
            .unwrap();
        (
            r.status().as_u16(),
            r.body_mut()
                .with_config()
                .limit(1 << 30)
                .read_to_vec()
                .unwrap(),
        )
    }
    fn json(&self, path: &str) -> Value {
        serde_json::from_slice(&self.get(path).1).unwrap()
    }
    fn post(&self, name: &str, data: &[u8], options: &Value) -> (u16, Value) {
        let b = "----e2e-boundary";
        let mut body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{b}\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n--{b}--\r\n").as_bytes());
        let mut r = self
            .agent
            .post(format!("{}/api/v1/jobs", self.base))
            .header("Content-Type", format!("multipart/form-data; boundary={b}"))
            .send(&body[..])
            .unwrap();
        (
            r.status().as_u16(),
            serde_json::from_slice(&r.body_mut().read_to_vec().unwrap()).unwrap_or(Value::Null),
        )
    }
    /// POST, poll to done, return the final status.
    fn run(&self, name: &str, data: &[u8], options: &Value) -> Value {
        let (st, v) = self.post(name, data, options);
        assert_eq!(st, 202, "{name} {options}: {v}");
        let id = v["job"].as_str().unwrap().to_string();
        let t = Instant::now();
        loop {
            let s = self.json(&format!("/api/v1/jobs/{id}"));
            if s["state"] == "done" {
                return s;
            }
            assert!(
                t.elapsed() < Duration::from_secs(600),
                "{name} {options} never finished: {s}"
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }
}

/// The synthetic input for each kind: the smallest WP1 file with that extension, or small generated text.
fn input_for(kind: &str) -> Option<(String, Vec<u8>)> {
    let text: &[u8] = match kind {
        "html" => b"<html><body><h1>Salom</h1><p>Dunyo</p><table><tr><td>1</td><td>2</td></tr></table></body></html>",
        "md" => b"# Salom

Dunyo **qalin** matn.

- bir
- ikki
",
        "txt" => b"Salom dunyo
Ikkinchi qator
",
        "csv" => b"ism,yosh
Ali,30
Vali,41
",
        "tsv" => b"ism	yosh
Ali	30
Vali	41
",
        _ => b"",
    };
    if !text.is_empty() {
        return Some((format!("sample.{kind}"), text.to_vec()));
    }
    let mut files: Vec<_> = std::fs::read_dir(INPUTS)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == kind))
        .collect();
    files.sort_by_key(|e| e.metadata().map_or(u64::MAX, |m| m.len()));
    let f = files.first()?.path();
    Some((
        f.file_name()?.to_string_lossy().into_owned(),
        std::fs::read(&f).ok()?,
    ))
}

#[test]
fn every_direction_over_http_then_a_hard_kill_leaves_work_empty_after_restart() {
    let tools = ["docs", "sheets", "slides", "pdf"]
        .iter()
        .all(|d| Path::new(&tool(d)).is_file());
    if !tools || !Path::new(INPUTS).is_dir() {
        assert!(
            std::env::var("CONVERT_API_E2E_REQUIRED").is_err(),
            "tools or {INPUTS} missing"
        );
        eprintln!("SKIPPED: the release tools or {INPUTS} are not on this machine");
        return;
    }
    let dir = sandbox("matrix");
    let port = free_port();
    let mut server = Server::start(&dir, port);
    let c = Client::new(server.port);
    let formats = c.json("/api/v1/formats");
    let rows: Vec<(String, String)> = formats["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["from"].as_str().unwrap().into(),
                r["to"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(rows.len() >= 80, "{} rows", rows.len());
    assert!(
        c.json("/healthz")["ok"] == json!(true),
        "{}",
        c.json("/healthz")
    );

    let results = Mutex::new(Vec::<(String, String, String, u128, u64)>::new());
    let work: Mutex<Vec<(String, String)>> = Mutex::new(rows.clone());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let Some((from, to)) = work.lock().unwrap().pop() else {
                        break;
                    };
                    let t = Instant::now();
                    let Some((name, data)) = input_for(&from) else {
                        results
                            .lock()
                            .unwrap()
                            .push((from, to, "no synthetic input".into(), 0, 0));
                        continue;
                    };
                    let st = c.run(&name, &data, &json!({"to": to}));
                    let f = &st["files"][0];
                    let (mut state, mut bytes) = (f["state"].as_str().unwrap().to_string(), 0u64);
                    if state == "done" {
                        let o = &f["outputs"][0];
                        let (code, body) = c.get(&format!(
                            "/api/v1/jobs/{}/files/0/{}",
                            st["job"].as_str().unwrap(),
                            o["id"]
                        ));
                        bytes = body.len() as u64;
                        let magic_ok = match to.as_str() {
                            "pdf" => body.starts_with(b"%PDF-"),
                            "png" => body.starts_with(&[0x89, b'P', b'N', b'G']),
                            "docx" | "xlsx" | "pptx" | "odt" => body.starts_with(b"PK"),
                            "rtf" => body.starts_with(b"{\\rtf"),
                            _ => !body.is_empty(),
                        };
                        if code != 200
                            || bytes == 0
                            || bytes != o["size"].as_u64().unwrap()
                            || !magic_ok
                        {
                            state = format!("bad download: {code}, {bytes} bytes");
                        }
                    } else {
                        state = format!("{state}: code {} {}", f["code"], f["message"]);
                    }
                    results
                        .lock()
                        .unwrap()
                        .push((from, to, state, t.elapsed().as_millis(), bytes));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort();
    for (from, to, state, ms, bytes) in &results {
        println!("{from:>5} -> {to:<5} {state} {ms} ms {bytes} bytes");
    }
    let bad: Vec<_> = results.iter().filter(|r| r.2 != "done").collect();
    println!("{} directions, {} not done", results.len(), bad.len());
    assert!(bad.is_empty(), "directions that did not finish: {bad:#?}");

    // options reach the tools: a transliteration shows up in the report inside the zip
    let (name, data) = (
        "uz_latn.docx".to_string(),
        std::fs::read(Path::new(INPUTS).join("uz_latn.docx")).unwrap(),
    );
    let st = c.run(
        &name,
        &data,
        &json!({"to": "docx", "translit": "cyrl", "alphabet": "2026"}),
    );
    assert_eq!(st["files"][0]["state"], "done", "{st}");
    let (_, zip) = c.get(&format!("/api/v1/jobs/{}/zip", st["job"].as_str().unwrap()));
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut rep = String::new();
    z.by_name("report.json")
        .unwrap()
        .read_to_string(&mut rep)
        .unwrap();
    let rep: Value = serde_json::from_str(&rep).unwrap();
    assert!(
        rep[0]["translit"]["runs"].as_u64().unwrap_or(0) > 0,
        "{rep}"
    );
    assert!(
        !rep.to_string()
            .contains(&dir.display().to_string().replace('\\', "/")),
        "a server path in the report"
    );

    // a hard kill: the store of the finished jobs is still on disk; the next start must remove it
    let left = std::fs::read_dir(dir.join("store")).unwrap().count();
    println!("entries in store/ before the kill: {left}");
    assert!(
        left > 0,
        "the finished jobs are kept for the retention time"
    );
    server.kill();
    let server2 = Server::start(&dir, port);
    let after: Vec<_> = ["work", "store"]
        .iter()
        .flat_map(|d| {
            std::fs::read_dir(dir.join(d))
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.file_name())
        })
        .collect();
    println!("entries in work/ after the restart: {after:?}");
    assert!(after.is_empty(), "{after:?}");
    drop(server2);
    let _ = std::fs::remove_dir_all(&dir);
}
