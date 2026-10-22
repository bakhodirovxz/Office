//! The real `suite.exe` as a process: the browser fallback when WebView2 is missing (forced by SUITE_FORCE_NO_WEBVIEW2) and the
//! single-instance hand-off (F14). Editors are stand-ins that log their arguments. No window is ever opened here.
#![cfg(windows)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Env {
    dir: PathBuf,
    id: String,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn env(name: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("suite-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    for b in ["suite-docs", "suite-sheets"] {
        std::fs::copy(
            env!("CARGO_BIN_EXE_suite-fake-editor"),
            dir.join("bin").join(format!("{b}.exe")),
        )
        .unwrap();
    }
    std::fs::write(dir.join("browser.cmd"), "@echo %1> \"%BROWSER_LOG%\"\r\n").unwrap();
    Env {
        dir,
        id: format!("t{name}{}", std::process::id()),
    }
}

impl Env {
    fn cmd(&self, files: &[&Path]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_suite"));
        c.arg("--dist")
            .arg(self.dir.join("dist"))
            .arg("--shell")
            .arg(repo().join("web/shell"));
        c.arg("--brand")
            .arg(repo().join("brand/brand.toml"))
            .arg("--bin-dir")
            .arg(self.dir.join("bin"));
        c.args(files);
        c.env("SUITE_DATA_DIR", self.dir.join("data"))
            .env("SUITE_INSTANCE", &self.id);
        c.env("SUITE_FORCE_NO_WEBVIEW2", "1")
            .env("SUITE_BROWSER", self.dir.join("browser.cmd"));
        c.env("BROWSER_LOG", self.dir.join("browser.log"))
            .env("FAKE_EDITOR_LOG", self.dir.join("editor.log"));
        c
    }
    fn file(&self, name: &str) -> PathBuf {
        let d = self.dir.join("Рабочий стол");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(name), b"x").unwrap();
        d.join(name)
    }
    fn wait(&self, name: &str) -> String {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(15) {
            if let Ok(s) = std::fs::read_to_string(self.dir.join(name))
                && !s.trim().is_empty()
            {
                return s;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("{name} did not appear")
    }
    fn port(&self) -> u16 {
        let v: serde_json::Value = serde_json::from_str(&self.wait("data/host.json")).unwrap();
        v["port"].as_u64().unwrap() as u16
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http_get(port: u16, path: &str) -> String {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn without_webview2_the_browser_opens_on_the_local_url_and_the_file_goes_to_its_editor() {
    let e = env("fallback");
    let doc = e.file("Буйруқ 1.docx");
    let mut c = e.cmd(&[&doc]);
    c.env("SUITE_TEST_EXIT_AFTER_MS", "4000");
    let mut host = Kill(c.spawn().unwrap());
    let port = e.port();
    assert_eq!(
        e.wait("browser.log").trim(),
        format!("http://127.0.0.1:{port}/")
    );
    // the file name arrives intact (spaces, Cyrillic)
    assert_eq!(e.wait("editor.log").trim(), doc.to_str().unwrap());
    // the shell config comes from the brand file, and the server answers on that port
    let r = http_get(port, "/shell-config.json");
    assert!(
        r.starts_with("HTTP/1.1 200") && r.contains("\"product\":\"Suite\""),
        "{r}"
    );
    // the desktop actions are not on HTTP
    assert!(http_get(port, "/ipc").starts_with("HTTP/1.1 404"));
    assert!(host.0.wait().unwrap().success());
}

#[test]
fn a_second_launch_hands_its_file_to_the_running_instance() {
    let e = env("single");
    let mut a = e.cmd(&[]);
    a.env("SUITE_TEST_EXIT_AFTER_MS", "20000");
    let mut first = Kill(a.spawn().unwrap());
    let _ = e.port();
    let sheet = e.file("Hisobot 2026.xlsx");
    let t = Instant::now();
    let second = e.cmd(&[&sheet]).status().unwrap();
    assert!(second.success(), "a second launch never fails");
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!(e.wait("editor.log").trim(), sheet.to_str().unwrap());
    assert!(
        first.0.try_wait().unwrap().is_none(),
        "the first instance is still running"
    );
}
