// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! XDFU off means absent (patch 0143): no command, no check, no audit file, and the bytes of every copy are those of a run without the
//! layer. Its own test binary: the hook is process-wide, and the reference run has to happen before it is installed.
use deckcraft_engine::{Session, find_command};
use deckcraft_ui_egui::office_history;
use serde_json::json;

fn grif_deck() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s.execute("insert.textBox", &json!({"rect": [700.0, 10.0, 200.0, 40.0], "text": "XDFU"})).unwrap();
    s
}

fn copies(dir: &std::path::Path, tag: &str) -> Vec<Vec<u8>> {
    let mut s = grif_deck();
    let out = |name: &str| dir.join(format!("{tag}-{name}")).to_string_lossy().to_string();
    let mut v = vec![];
    for (file, format) in [("a.pdf", "pdf"), ("a.png", "png"), ("a.txt", "outline"), ("a.pptx", "pptx"), ("a.deckcraft", "deckcraft")] {
        s.execute("file.export", &json!({"path": out(file), "format": format})).unwrap();
        v.push(std::fs::read(out(file)).unwrap());
    }
    s.execute("file.export", &json!({"path": out("print.pdf"), "format": "pdf", "print": true})).unwrap();
    v.push(std::fs::read(out("print.pdf")).unwrap());
    let dl = s.execute("file.saveBytes", &json!({"format": "pptx"})).unwrap();
    v.push(deckcraft_engine::cmd::base64_decode(dl["data"].as_str().unwrap()).unwrap());
    v
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
    assert!(!after[2].windows(9).any(|w| w == b"XDFU copy"), "no stamp in the outline");
    assert!(!dir.join("xdfu-audit.jsonl").exists(), "no audit file");
    let mut s = grif_deck();
    assert!(s.execute("office.xdfu.insertGrif", &json!({})).is_err(), "the command does not exist");
    let _ = std::fs::remove_dir_all(&dir);
}
