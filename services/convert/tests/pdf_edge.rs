//! WP9 fixes 3 and 4, against the real pdf-cli (skipped loudly when it is not built; it is looked up under `$OFFICE_TARGET`, default
//! `E:/office-target/p1`, as `pdf/release/pdf-cli(.exe)`):
//! - a cut-off PDF converted to txt or docx carries `pdf_damaged` (and the engine's own message), a whole one does not;
//! - a PDF with no text layer converted to docx (native engine) carries `empty_text`, the same as to txt; a PDF with text does not.
//! The PDFs are built here, byte by byte, with a correct cross-reference table.
use convert::exec::Config;
use convert::job::{self, Dest, FileReport};
use convert::plan::{Options, Target, ToolId};
use std::path::PathBuf;

fn tool() -> Option<PathBuf> {
    let target = std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into());
    let p = PathBuf::from(target).join("pdf/release").join(format!("pdf-cli{}", std::env::consts::EXE_SUFFIX));
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("SKIPPED pdf edge cases: {} not built", p.display());
        None
    }
}

/// A one-page PDF; `content` is the page's content stream. The xref offsets are exact.
fn pdf(content: &str) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![];
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

const WITH_TEXT: &str = "BT /F1 18 Tf 20 100 Td (Hello converter, page one) Tj ET";
const NO_TEXT: &str = "0.5 g 20 20 200 100 re f";

struct Run {
    dir: PathBuf,
    cfg: Config,
}

fn run_env(name: &str, pdf_cli: PathBuf) -> Run {
    let dir = std::env::temp_dir().join(format!("convert-pdfedge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::from_env();
    cfg.work_dir = dir.join("work");
    cfg.tools.insert(ToolId::Pdf, pdf_cli);
    Run { dir, cfg }
}

impl Run {
    fn convert(&self, name: &str, bytes: &[u8], to: Target) -> FileReport {
        let p = self.dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        let dest = Dest { out_dir: self.dir.join("dest"), out_file: None };
        job::convert_file(&self.cfg, &p, to, &Options::default(), &dest)
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn codes(r: &FileReport) -> Vec<&str> {
    r.warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn a_cut_off_pdf_warns_and_a_whole_one_does_not() {
    let Some(t) = tool() else { return };
    let r = run_env("cut", t);
    let whole = pdf(WITH_TEXT);
    for to in [Target::Txt, Target::Docx] {
        let ok = r.convert("whole.pdf", &whole, to);
        assert_eq!(ok.code, 0, "{}: {}", to.ext(), ok.message);
        assert!(!codes(&ok).contains(&"pdf_damaged"), "a whole pdf to {}: {:?}", to.ext(), codes(&ok));
        // cut off before the cross-reference table, as an interrupted download or a half-copied file is
        let cut = &whole[..whole.windows(4).position(|w| w == b"xref").unwrap()];
        let bad = r.convert("cut.pdf", cut, to);
        assert!(codes(&bad).contains(&"pdf_damaged"), "a cut-off pdf to {}: code {} {:?} {}", to.ext(), bad.code, codes(&bad), bad.message);
        assert!(bad.warnings.iter().any(|w| w.code == "upstream" && w.text.contains("cross reference")), "{:?}", bad.warnings);
    }
}

#[test]
fn a_pdf_without_a_text_layer_warns_empty_text_to_docx_as_it_does_to_txt() {
    let Some(t) = tool() else { return };
    let r = run_env("scan", t);
    let scan = pdf(NO_TEXT);
    for to in [Target::Txt, Target::Docx] {
        let rep = r.convert("scan.pdf", &scan, to);
        assert_eq!(rep.code, 0, "{}: {}", to.ext(), rep.message);
        assert!(codes(&rep).contains(&"empty_text"), "a scan to {}: {:?}", to.ext(), codes(&rep));
        let withtext = r.convert("text.pdf", &pdf(WITH_TEXT), to);
        assert!(!codes(&withtext).contains(&"empty_text"), "a pdf with text to {}: {:?}", to.ext(), codes(&withtext));
    }
}
