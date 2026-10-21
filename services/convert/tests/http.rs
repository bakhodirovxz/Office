//! WP5 step 5: the `Http` step against a fake sidecar served from inside the test (no Docker). The real sidecar is exercised
//! by `real_sidecar_when_it_is_up` (skipped with a message when 127.0.0.1:18080 does not answer).
use convert::exec::{Config, Exit};
use convert::job::{self, Dest, FileReport};
use convert::plan::{Options, PdfEngine, Target};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

fn json_err(status: u16, code: &str, message: &str) -> Reply {
    Reply { status, headers: vec![("Content-Type", "application/json".into())], body: format!(r#"{{"code":"{code}","message":"{message}"}}"#).into_bytes() }
}

fn docx_bytes() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut w = zip::ZipWriter::new(&mut buf);
    w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(b"<w:document/>").unwrap();
    w.finish().unwrap();
    buf.into_inner()
}

fn ok(report: &str) -> Reply {
    Reply { status: 200, headers: vec![("X-Convert-Report", report.into())], body: docx_bytes() }
}

/// One canned reply per connection, in order; the last one repeats. Returns the base URL and the requests seen.
#[allow(clippy::type_complexity)] // shortcut: a test helper returning the request log as a tuple
fn serve(replies: Vec<Reply>) -> (String, Arc<Mutex<Vec<(String, Vec<(String, String)>, usize)>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    std::thread::spawn(move || {
        for (i, conn) in l.incoming().enumerate() {
            let Ok(mut c) = conn else { break };
            let (line, headers, body_len) = read_request(&mut c);
            log.lock().unwrap().push((line, headers, body_len));
            let r = &replies[i.min(replies.len() - 1)];
            let mut head = format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n", r.status, r.body.len());
            for (k, v) in &r.headers {
                head += &format!("{k}: {v}\r\n");
            }
            let _ = c.write_all(head.as_bytes()).and_then(|_| c.write_all(b"\r\n")).and_then(|_| c.write_all(&r.body));
        }
    });
    (url, seen)
}

fn read_request(c: &mut TcpStream) -> (String, Vec<(String, String)>, usize) {
    let mut buf = vec![];
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = c.read(&mut chunk).unwrap();
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        assert!(n > 0, "client closed early");
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let line = lines.next().unwrap().to_string();
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())).collect();
    let want: usize = headers.iter().find(|(k, _)| k == "content-length").map(|(_, v)| v.parse().unwrap()).unwrap_or(0);
    while buf.len() - head_end < want {
        let n = c.read(&mut chunk).unwrap();
        assert!(n > 0, "client closed early");
        buf.extend_from_slice(&chunk[..n]);
    }
    (line, headers, want)
}

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-http-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("in.pdf"), b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n").unwrap();
        Sandbox(p)
    }
    fn run(&self, url: Option<&str>) -> FileReport {
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        c.pdf2docx_url = url.map(String::from);
        c.http_backoff_ms = 10;
        let dest = Dest { out_dir: self.0.join("dest"), out_file: None };
        job::convert_file(&c, &self.0.join("in.pdf"), Target::Docx, &Options { pdf_engine: PdfEngine::Sidecar, ..Options::default() }, &dest)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn failed(r: &FileReport, exit: Exit, needle: &str) {
    assert_eq!(r.code, exit.code(), "wrong exit; message: {}", r.message);
    assert_eq!(r.status, "failed");
    assert!(r.message.to_lowercase().contains(needle), "message '{}' lacks '{needle}'", r.message);
    assert!(r.outputs.is_empty());
}

#[test]
fn success_sends_the_pdf_and_reports_the_sidecar() {
    let s = Sandbox::new("ok");
    let (url, seen) = serve(vec![ok(r#"{"pages":2,"merged":3,"paragraphs":9,"warnings":["pages without a text layer: [2]"]}"#)]);
    let r = s.run(Some(&url));
    assert_eq!((r.code, r.status), (0, "warn"), "{}", r.message);
    assert_eq!(r.warnings.len(), 1);
    assert_eq!((r.warnings[0].code.as_str(), r.warnings[0].text.as_str()), ("sidecar", "pages without a text layer: [2]"));
    assert_eq!(r.steps[0].tool, "pdf2docx");
    assert!(r.steps[0].stderr_tail.contains(r#""merged":3"#), "the sidecar report is in the step: {}", r.steps[0].stderr_tail);
    assert!(std::path::Path::new(&r.outputs[0]).exists());
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "POST /v1/pdf2docx?merge=1 HTTP/1.1");
    assert!(seen[0].1.contains(&("content-type".into(), "application/pdf".into())));
    assert_eq!(seen[0].2, std::fs::metadata(s.0.join("in.pdf")).unwrap().len() as usize);
}

#[test]
fn every_error_answer_maps_to_its_exit_code() {
    for (status, code, exit, needle) in [
        (400, "bad_pdf", Exit::Unsupported, "cannot be read"),
        (411, "length_required", Exit::ToolError, "content-length"),
        (413, "too_large", Exit::Rejected, "size limit"),
        (422, "encrypted", Exit::Rejected, "encrypted"),
        (422, "too_many_pages", Exit::Rejected, "too many pages"),
        (422, "no_text_layer", Exit::ToolError, "no text layer"),
        (504, "timeout", Exit::Timeout, "timed out"),
        (500, "worker_crashed", Exit::Crash, "crashed"),
    ] {
        let s = Sandbox::new(&format!("e{status}{code}"));
        let (url, seen) = serve(vec![json_err(status, code, "the sidecar says so")]);
        let r = s.run(Some(&url));
        failed(&r, exit, needle);
        assert!(r.message.contains("the sidecar says so"), "the sidecar's own message is passed on: {}", r.message);
        assert_eq!(seen.lock().unwrap().len(), 1, "{status} is not retried");
    }
}

#[test]
fn busy_is_retried_with_backoff_then_succeeds() {
    let s = Sandbox::new("busy");
    let (url, seen) = serve(vec![json_err(503, "busy", "b"), json_err(503, "busy", "b"), ok(r#"{"pages":1}"#)]);
    let r = s.run(Some(&url));
    assert_eq!(r.code, 0, "{}", r.message);
    assert_eq!(seen.lock().unwrap().len(), 3);
}

#[test]
fn busy_forever_ends_in_exit_1_after_four_attempts() {
    let s = Sandbox::new("busyforever");
    let (url, seen) = serve(vec![json_err(503, "busy", "all slots are busy")]);
    let r = s.run(Some(&url));
    failed(&r, Exit::ToolError, "stayed busy");
    assert_eq!(seen.lock().unwrap().len(), convert::http::ATTEMPTS as usize);
}

#[test]
fn unreachable_or_unconfigured_is_exit_7() {
    let s = Sandbox::new("down");
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    failed(&s.run(Some(&dead)), Exit::Missing, "did not answer");
    failed(&s.run(None), Exit::Missing, "CONVERT_PDF2DOCX_URL".to_lowercase().as_str());
}

#[test]
fn a_bad_answer_body_is_exit_8() {
    let s = Sandbox::new("badbody");
    let (url, _) = serve(vec![Reply { status: 200, headers: vec![], body: b"not a zip".to_vec() }]);
    failed(&s.run(Some(&url)), Exit::BadOutput, "rejected");
}

/// The real sidecar (`sidecars/pdf2docx/run.sh`, 127.0.0.1:18080), when it is up: a scanned PDF must give exit 1 and
/// the sidecar's own `no_text_layer` message. Set CONVERT_SCAN_PDF to an image-only PDF to run the second half.
#[test]
fn real_sidecar_when_it_is_up() {
    let url = "http://127.0.0.1:18080";
    let up = TcpStream::connect_timeout(&"127.0.0.1:18080".parse().unwrap(), std::time::Duration::from_millis(300)).is_ok();
    if !up {
        eprintln!("SKIPPED real_sidecar_when_it_is_up: nothing listens on 127.0.0.1:18080 (run sidecars/pdf2docx/run.sh)");
        return;
    }
    let Some(pdf) = std::env::var_os("CONVERT_TEST_TEXT_PDF") else {
        eprintln!("SKIPPED real_sidecar_when_it_is_up: set CONVERT_TEST_TEXT_PDF (and CONVERT_TEST_SCAN_PDF) to synthetic PDFs");
        return;
    };
    let s = Sandbox::new("real");
    std::fs::copy(pdf, s.0.join("in.pdf")).unwrap();
    let r = s.run(Some(url));
    assert_eq!(r.code, 0, "{}", r.message);
    assert!(r.steps[0].stderr_tail.contains("merged"), "{}", r.steps[0].stderr_tail);
    if let Some(scan) = std::env::var_os("CONVERT_TEST_SCAN_PDF") {
        std::fs::copy(scan, s.0.join("in.pdf")).unwrap();
        failed(&s.run(Some(url)), Exit::ToolError, "no text layer");
    }
}
