// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Version snapshots through the command hook (patch 0143), with a store in a temporary folder and XDFU off. The hook is process-wide,
//! so this file is its own test binary.
use deckcraft_engine::{Session, find_command};
use deckcraft_ui_egui::office_history;
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
    assert!(find_command("office.xdfu.insertGrif").is_none(), "XDFU is off");

    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    let file = dir.join("deck.pptx");
    let path = file.to_string_lossy().to_string();
    let add = |s: &mut Session, t: &str| s.execute("insert.textBox", &json!({"rect": [40.0, 300.0, 300.0, 40.0], "text": t})).unwrap();
    let reader = store(&dir); // a second handle on the same folder
    add(&mut s, "one");
    s.execute("file.saveAs", &json!({"path": path})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 1, "save as snapshots the new file");
    let v1 = std::fs::read(&file).unwrap();

    add(&mut s, "two");
    s.execute("file.save", &json!({})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 2, "an explicit save snapshots");
    s.execute("file.save", &json!({})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 2, "the same bytes again add nothing");
    add(&mut s, "three");
    s.execute("file.save", &json!({})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 3);

    // an export is not a version of the document
    s.execute("file.export", &json!({"path": dir.join("copy.txt").to_string_lossy(), "format": "outline"})).unwrap();
    assert_eq!(reader.list(&file).unwrap().len(), 3);

    reader.restore(&file, 1).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), v1, "the file is v1 again");
    assert_eq!(reader.list(&file).unwrap().len(), 4, "restoring adds a version, it destroys none");
    assert!(reader.verify(&file).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
