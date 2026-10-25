//! D5: after the native pdf to docx the converter compares the letters of the PDF text layer with the letters of the docx. A
//! shortfall is redone by the sidecar when it answers, and warned about when it does not. Hermetic: `convert-fake-tool` plays
//! pdf-cli (its `text` prints the file, its native `run --script` writes the same text without the `<<...>>` spans, the "lost
//! column"), and a fake sidecar is served from inside the test.
use convert::exec::Config;
use convert::job::{self, Dest, FileReport};
use convert::plan::{Options, Target, ToolId};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

fn docx_with(text: &str) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut w = zip::ZipWriter::new(&mut buf);
    w.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    write!(
        w,
        "<w:document><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:document>"
    )
    .unwrap();
    w.finish().unwrap();
    buf.into_inner()
}

/// A sidecar that answers every POST with a docx holding `text`.
fn sidecar(text: &str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let body = docx_with(text);
    std::thread::spawn(move || {
        for c in l.incoming() {
            let Ok(mut c) = c else { break };
            let (mut buf, mut chunk) = (vec![], [0u8; 4096]);
            let head_end = loop {
                let n = c.read(&mut chunk).unwrap_or(0);
                buf.extend_from_slice(&chunk[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break p + 4;
                }
                if n == 0 {
                    break buf.len();
                }
            };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
            let want: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let mut got = buf.len() - head_end;
            while got < want {
                let n = c.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                got += n;
            }
            let _ = write!(
                c,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .and_then(|_| c.write_all(&body));
        }
    });
    url
}

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-guard-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Sandbox(p)
    }
    fn run(&self, pdf_text: &str, sidecar_url: Option<&str>) -> FileReport {
        self.run_with(pdf_text, sidecar_url, |c| c.pdf_fallback = true)
    }
    fn run_with(
        &self,
        pdf_text: &str,
        sidecar_url: Option<&str>,
        set: impl FnOnce(&mut Config),
    ) -> FileReport {
        std::fs::write(self.0.join("in.pdf"), format!("%PDF-1.4\n{pdf_text}")).unwrap();
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        c.pdf2docx_url = sidecar_url.map(String::from);
        set(&mut c);
        for t in ToolId::ALL {
            c.tools
                .insert(t, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        }
        let dest = Dest {
            out_dir: self.0.join("dest"),
            out_file: None,
        };
        job::convert_file(
            &c,
            &self.0.join("in.pdf"),
            Target::Docx,
            &Options::default(),
            &dest,
        )
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn docx_text(report: &FileReport) -> String {
    let mut z =
        zip::ZipArchive::new(std::fs::File::open(Path::new(&report.outputs[0])).unwrap()).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

const BODY: &str = "Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega ";
const COLUMN: &str = "<<Rahimov Karimov Aliyev>>";

#[test]
fn a_complete_native_docx_is_kept_quietly() {
    let s = Sandbox::new("whole");
    let r = s.run(BODY, Some("http://127.0.0.1:9"));
    assert_eq!(
        (r.code, r.status),
        (0, "ok"),
        "{}: {:?}",
        r.message,
        r.warnings
    );
}

#[test]
fn a_tiny_shortfall_under_the_threshold_is_not_reported() {
    let s = Sandbox::new("tiny");
    // 2 letters of 400 are not a lost column
    let r = s.run(&format!("{}<<ab>>{}", BODY.repeat(3), BODY), None);
    assert_eq!(r.status, "ok", "{:?}", r.warnings);
}

#[test]
fn a_lost_column_without_a_sidecar_warns_with_the_counts() {
    let s = Sandbox::new("nosidecar");
    let r = s.run(&format!("{BODY}{COLUMN} {BODY}"), None);
    assert_eq!((r.code, r.status), (0, "warn"), "{}", r.message);
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    let w = &r.warnings[0];
    let letters = |t: &str| t.chars().filter(|c| c.is_alphabetic()).count();
    let (lost, all) = (letters(COLUMN), letters(&format!("{BODY}{COLUMN}{BODY}")));
    assert_eq!(
        (w.code.as_str(), w.count as usize),
        ("pdf_text_missing", lost)
    );
    assert!(w.text.contains(&format!("{lost} of {all}")), "{}", w.text);
    // the native result stays
    assert!(!docx_text(&r).contains("Rahimov"));
}

#[test]
fn a_lost_column_is_redone_by_the_sidecar_when_it_answers() {
    let s = Sandbox::new("fallback");
    let url = sidecar(&format!("{BODY}Rahimov Karimov Aliyev {BODY}"));
    let r = s.run(&format!("{BODY}{COLUMN} {BODY}"), Some(&url));
    assert_eq!((r.code, r.status), (0, "warn"), "{}", r.message);
    let codes: Vec<_> = r.warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(codes, ["pdf_docx_fallback"]);
    assert!(
        docx_text(&r).contains("Rahimov"),
        "the sidecar's docx is the result"
    );
    assert!(r.steps.iter().any(|st| st.tool == "pdf2docx"));
}

#[test]
fn an_unreachable_sidecar_keeps_the_native_file_and_warns() {
    let s = Sandbox::new("down");
    // nothing listens on port 9
    let r = s.run(
        &format!("{BODY}{COLUMN} {BODY}"),
        Some("http://127.0.0.1:9"),
    );
    assert_eq!((r.code, r.status), (0, "warn"), "{}", r.message);
    let codes: Vec<_> = r.warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(codes, ["pdf_text_missing"]);
    assert!(
        !docx_text(&r).contains("Rahimov"),
        "the native docx was put back"
    );
}

#[test]
fn the_sidecar_fallback_is_an_operator_switch_and_never_serves_an_xdfu_job() {
    use convert::audit::AuditLog;
    use convert::xdfu::{Mode, XdfuHook};
    let s = Sandbox::new("switch");
    let url = sidecar(&format!("{BODY}Rahimov Karimov Aliyev {BODY}"));
    let lost = format!("{BODY}{COLUMN} {BODY}");
    let used = |r: &FileReport| r.steps.iter().any(|st| st.tool == "pdf2docx");
    // switch off (the default): the sidecar answers, but it is not asked
    let r = s.run_with(&lost, Some(&url), |c| assert!(!c.pdf_fallback));
    assert!(!used(&r), "{:?}", r.warnings);
    assert!(r.warnings.iter().any(|w| w.code == "pdf_text_missing"));
    // switch on, an ordinary file with the XDFU hook installed: the fallback runs
    let hook = |c: &mut Config| {
        c.pdf_fallback = true;
        c.hooks.push(std::sync::Arc::new(XdfuHook::new(
            Mode::Watermark,
            std::sync::Arc::new(AuditLog::open(&s.0.join("audit.jsonl"), false).unwrap()),
            true,
        )));
    };
    assert!(used(&s.run_with(&lost, Some(&url), hook)));
    // switch on, a file that carries the grif: never
    let r = s.run_with(&format!("XDFU {lost}"), Some(&url), hook);
    assert!(!used(&r), "{:?}", r.warnings);
    assert!(
        r.warnings.iter().all(|w| w.code != "pdf_docx_fallback"),
        "{:?}",
        r.warnings
    );
}
