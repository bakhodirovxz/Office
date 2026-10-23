// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Version snapshots through the command hook (patch 0143), with a store in a temporary folder and XDFU off. The hook is process-wide,
//! so this file is its own test binary.
use gridcraft_engine::{Session, find_command};
use gridcraft_ui_egui::office_history;
use serde_json::json;
use suite_core::history::{Limits, Store};

fn store(root: &std::path::Path) -> Store {
    Store::new(root.join("history"), Limits { versions: 50, days: 365, max_bytes: 1 << 30, big_file_bytes: 50 << 20, big_file_versions: 5 })
}

#[test]
fn save_snapshot_list_restore_round_trip() {
    let dir = std::env::temp_dir().join(format!("office-history-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(office_history::install(Some(store(&dir)), None));
    assert!(find_command("office.history").is_some(), "the toolbar button needs the command");
    assert!(find_command("office.xdfu.insertGrif").is_none(), "XDFU is off");

    let mut s = Session::new();
    s.new_workbook();
    // no file yet: the command says so instead of opening a panel
    let e = s.execute("office.history", json!({})).unwrap_err().to_string();
    assert!(e.contains("Save the file first"), "{e}");

    let file = dir.join("book.xlsx");
    let path = file.to_string_lossy().to_string();
    let set = |s: &mut Session, v: i64| s.execute("range.setValues", json!({"range": "A1", "values": [[v]]})).unwrap();
    let reader = store(&dir); // a second handle on the same folder
    set(&mut s, 1);
    s.execute("file.saveAs", json!({"path": path})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 1, "save as snapshots the new file");
    let v1 = std::fs::read(&file).unwrap();

    set(&mut s, 2);
    s.execute("file.save", json!({"autosave": true})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 1, "an autosave right after a snapshot adds nothing");
    s.execute("file.save", json!({})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 2, "an explicit save always snapshots");
    set(&mut s, 3);
    s.execute("file.save", json!({})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 3);

    // a csv copy is not a version of the document
    s.execute("file.exportCsv", json!({"path": dir.join("copy.csv").to_string_lossy()})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 3);

    reader.restore(&file, 1).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), v1, "the file is v1 again");
    assert_eq!(reader.list(&file).unwrap().len(), 4, "restoring adds a version, it destroys none");
    assert!(reader.verify(&file).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
