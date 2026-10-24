// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The XDFU watermark through the command hook (patch 0143): every copy of a grif workbook carries the Resolution 766 cl. 12 line with a
//! copy number from the keyed audit chain, and nothing is ever refused. Its own test binary (the hook is process-wide); the tests share
//! one audit log, so they run one at a time.
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, Once};

use gridcraft_engine::{Session, find_command};
use gridcraft_ui_egui::office_history;
use serde_json::{Value, json};
use xdfu_core::audit;

static LOCK: Mutex<()> = Mutex::new(());

fn dir() -> PathBuf {
    std::env::temp_dir().join(format!("office-xdfu-{}", std::process::id()))
}
fn log() -> PathBuf {
    dir().join("xdfu-audit.jsonl")
}
fn lines() -> u64 {
    if log().exists() { audit::verify(&log()).expect("the audit chain verifies").lines } else { 0 }
}

/// One test at a time, with the layer installed on first use.
fn start() -> MutexGuard<'static, ()> {
    static ONCE: Once = Once::new();
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ONCE.call_once(|| {
        let _ = std::fs::remove_dir_all(dir());
        std::fs::create_dir_all(dir()).unwrap();
        assert!(office_history::install(None, Some((log(), dir().join("xdfu-audit.key")))));
    });
    g
}

fn book(first: &str) -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[first], ["Budget"], [1.5]]})).unwrap();
    s
}
fn bytes(v: &Value) -> Vec<u8> {
    gridcraft_engine::io::base64_decode(v["base64"].as_str().expect("base64")).unwrap()
}
fn has(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}
/// The copy number in the first `XDFU copy N | ` of `hay`.
fn copy_no(hay: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(hay);
    let at = text.find("XDFU copy ").expect("a stamp") + "XDFU copy ".len();
    text[at..].split(' ').next().unwrap().parse().expect("copy number")
}
fn footer_of(p: &PathBuf) -> String {
    let mut t = Session::new();
    t.execute("file.open", json!({"path": p.to_string_lossy()})).unwrap();
    let f = t.doc().unwrap().wb.sheet(0).unwrap().print.footer.clone();
    f
}

#[test]
fn exported_pdf_carries_copy_number_and_user() {
    let _g = start();
    let mut s = book("XDFU");
    let before = lines();
    let out = bytes(&s.execute("file.exportPdf", json!({})).unwrap());
    let n = copy_no(&out);
    assert_eq!(n, before + 1, "the copy number is the audit counter");
    assert!(
        has(&out, &format!("(XDFU copy {n} | ")) && has(&out, &format!(" | {} | ", xdfu_core::os_user())),
        "number and OS user are in the PDF text"
    );
    assert_eq!(lines(), before + 1);
    // the working workbook is as it was: no footer left behind
    assert_eq!(s.doc().unwrap().wb.sheet(0).unwrap().print.footer, "");
}

#[test]
fn print_of_a_grif_workbook_succeeds_and_is_stamped() {
    let _g = start();
    let mut s = book("\u{414}\u{421}\u{41f}");
    s.execute("pageLayout.headerFooter", json!({"footer": "&LDept&RPage &P"})).unwrap();
    s.doc_mut().unwrap().saved = s.doc().unwrap().wb.clone();
    let out = bytes(&s.execute("file.print", json!({})).unwrap());
    assert!(has(&out, "(XDFU copy ") && has(&out, "(Dept) Tj") && has(&out, "(Page 1) Tj"), "the stamp joins the footer it found");
    assert!(!s.doc().unwrap().is_dirty());
    assert_eq!(s.doc().unwrap().wb.sheet(0).unwrap().print.footer, "&LDept&RPage &P");
}

#[test]
fn csv_html_saveas_and_download_are_stamped_and_the_working_file_is_not() {
    let _g = start();
    let mut s = book("\u{425}.\u{414}.\u{424}.\u{423}.");
    let d = dir();
    let work = d.join("work.xlsx");
    // the first save of an untitled workbook is a new copy, so it is stamped
    let before = lines();
    s.execute("file.saveAs", json!({"path": work.to_string_lossy()})).unwrap();
    assert_eq!(lines(), before + 1);
    assert!(footer_of(&work).contains("XDFU copy"), "the saved xlsx carries the stamp");
    assert!(!s.doc().unwrap().is_dirty() && s.doc().unwrap().path.as_deref() == work.to_str());
    assert_eq!(s.doc().unwrap().wb.sheet(0).unwrap().print.footer, "", "the working document keeps its own footer");

    // the working file is saved plainly: no stamp, no audit line
    let n = lines();
    s.execute("range.setValues", json!({"range": "B2", "values": [[7]]})).unwrap();
    s.execute("file.save", json!({})).unwrap();
    s.execute("file.saveAs", json!({"path": work.to_string_lossy()})).unwrap();
    assert_eq!(lines(), n, "saving the working file writes no audit line");

    let csv = d.join("a.csv");
    s.execute("file.exportCsv", json!({"path": csv.to_string_lossy()})).unwrap();
    let csv = std::fs::read(csv).unwrap();
    assert!(csv.starts_with(b"\"XDFU copy "), "{}", String::from_utf8_lossy(&csv));
    s.execute("file.exportHtml", json!({"path": d.join("a.html").to_string_lossy()})).unwrap();
    assert!(has(&std::fs::read(d.join("a.html")).unwrap(), "XDFU copy"));
    s.execute("file.saveAs", json!({"path": d.join("b.csv").to_string_lossy()})).unwrap();
    assert!(std::fs::read(d.join("b.csv")).unwrap().starts_with(b"\"XDFU copy "));
    s.execute("file.saveAs", json!({"path": d.join("c.pdf").to_string_lossy()})).unwrap();
    assert!(has(&std::fs::read(d.join("c.pdf")).unwrap(), "(XDFU copy "));
    let dl = bytes(&s.execute("file.saveBytes", json!({"format": "csv"})).unwrap());
    assert!(dl.starts_with(b"\"XDFU copy "));
    let dl = bytes(&s.execute("file.saveBytes", json!({"format": "xlsx"})).unwrap());
    let tmp = d.join("dl.xlsx");
    std::fs::write(&tmp, dl).unwrap();
    assert!(footer_of(&tmp).contains("XDFU copy"));
    assert_eq!(lines(), n + 6);
    // copy and cut are not blocked (and cannot carry a mark)
    assert!(s.execute("edit.copy", json!({"range": "A1"})).is_ok() && s.execute("edit.cut", json!({"range": "B2"})).is_ok());
    let all = std::fs::read_to_string(log()).unwrap();
    assert!(audit::verify(&log()).is_ok() && all.contains("\"export\"") && all.contains("save_as") && all.contains("download"));
}

#[test]
fn a_workbook_without_a_grif_is_never_stamped() {
    let _g = start();
    let mut s = book("Quarterly report");
    let before = lines();
    let out = bytes(&s.execute("file.exportPdf", json!({})).unwrap());
    assert!(!has(&out, "XDFU"));
    s.execute("file.exportCsv", json!({"path": dir().join("plain.csv").to_string_lossy()})).unwrap();
    assert_eq!(std::fs::read(dir().join("plain.csv")).unwrap(), b"Quarterly report\r\nBudget\r\n1.5\r\n");
    s.execute("file.print", json!({})).unwrap();
    s.execute("file.saveBytes", json!({})).unwrap();
    assert_eq!(lines(), before, "no audit line for a non-grif workbook");
}

#[test]
fn insert_grif_writes_the_header_and_takes_a_copy_number() {
    let _g = start();
    assert!(find_command("office.xdfu.insertGrif").is_some());
    let mut s = book("Budget");
    s.execute("pageLayout.headerFooter", json!({"header": "&LTitle&RPage &P"})).unwrap();
    let before = lines();
    assert!(s.execute("office.xdfu.insertGrif", json!({"grif": "nope"})).is_err());
    let r = s.execute("office.xdfu.insertGrif", json!({"grif": "\u{425}\u{414}\u{424}\u{423}"})).unwrap();
    assert_eq!(r["copy"], before + 1);
    let h = s.doc().unwrap().wb.sheet(0).unwrap().print.header.clone();
    assert_eq!(h, format!("&LTitle&R\u{425}\u{414}\u{424}\u{423}\nNusxa {}\nPage &P", before + 1));
    assert_eq!(lines(), before + 1);
    // a workbook that now carries a grif in its header is stamped on export
    let out = bytes(&s.execute("file.exportPdf", json!({})).unwrap());
    assert!(has(&out, "(XDFU copy ") && has(&out, "(Nusxa "));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.doc().unwrap().wb.sheet(0).unwrap().print.header, "&LTitle&RPage &P", "undoable");
}
