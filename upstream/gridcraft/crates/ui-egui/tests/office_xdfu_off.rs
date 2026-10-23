// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! XDFU off means absent (patch 0143): no command, no check, no audit file, and the bytes of every copy are those of a run without the
//! layer. Its own test binary: the hook is process-wide, and the reference run has to happen before it is installed.
use gridcraft_engine::{Session, find_command};
use gridcraft_ui_egui::office_history;
use serde_json::json;

fn grif_book() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["XDFU"], ["Budget"], [1.5]]})).unwrap();
    s.execute("pageLayout.headerFooter", json!({"footer": "&LDept&RPage &P"})).unwrap();
    s
}

fn copies(dir: &std::path::Path, tag: &str) -> Vec<Vec<u8>> {
    let mut s = grif_book();
    let b64 = |v: serde_json::Value| gridcraft_engine::io::base64_decode(v["base64"].as_str().unwrap()).unwrap();
    let csv = dir.join(format!("{tag}.csv"));
    s.execute("file.exportCsv", json!({"path": csv.to_string_lossy()})).unwrap();
    let html = dir.join(format!("{tag}.html"));
    s.execute("file.exportHtml", json!({"path": html.to_string_lossy()})).unwrap();
    let pdf_path = dir.join(format!("{tag}.pdf"));
    s.execute("file.exportPdf", json!({"path": pdf_path.to_string_lossy()})).unwrap();
    vec![
        b64(s.execute("file.exportPdf", json!({})).unwrap()),
        b64(s.execute("file.print", json!({})).unwrap()),
        b64(s.execute("file.saveBytes", json!({"format": "xlsx"})).unwrap()),
        b64(s.execute("file.saveBytes", json!({"format": "csv"})).unwrap()),
        std::fs::read(csv).unwrap(),
        std::fs::read(html).unwrap(),
        std::fs::read(pdf_path).unwrap(),
    ]
}

#[test]
fn xdfu_off_registers_nothing() {
    let dir = std::env::temp_dir().join(format!("office-xdfu-off-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let before = copies(&dir, "before");
    assert_eq!(before, copies(&dir, "again"), "the copies are reproducible, so they can be compared");

    assert!(office_history::install(None, None));
    assert!(find_command("office.xdfu.insertGrif").is_none());
    let after = copies(&dir, "after");
    assert_eq!(before, after, "bytes of every copy are identical with the layer installed");
    assert!(!after[0].windows(9).any(|w| w == b"XDFU copy"), "no stamp");
    assert!(!dir.join("xdfu-audit.jsonl").exists(), "no audit file");
    let mut s = grif_book();
    assert!(s.execute("office.xdfu.insertGrif", json!({})).is_err(), "the command does not exist");
    let _ = std::fs::remove_dir_all(&dir);
}
