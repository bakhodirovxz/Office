// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The XDFU watermark through the command hook (patch 0143): every copy of a grif presentation carries the Resolution 766 cl. 12 line
//! with a copy number from the keyed audit chain, and nothing is ever refused. Its own test binary (the hook is process-wide); the
//! tests share one audit log, so they run one at a time.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, Once};

use deckcraft_engine::{EngineError, Session, find_command};
use deckcraft_ui_egui::office_history;
use hayro_syntax::content::ops::TypedInstruction;
use hayro_syntax::object::{Dict, Object, Stream};
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

/// A one-slide deck with `first` as a text box at the top of the slide and "Budget" lower down.
fn deck(first: &str) -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s.execute("insert.textBox", &json!({"rect": [600.0, 10.0, 300.0, 40.0], "text": first})).unwrap();
    s.execute("insert.textBox", &json!({"rect": [60.0, 200.0, 300.0, 40.0], "text": "Budget"})).unwrap();
    s
}
fn path(name: &str) -> String {
    dir().join(name).to_string_lossy().to_string()
}
fn export(s: &mut Session, name: &str, extra: Value) -> Result<Value, EngineError> {
    let mut p = json!({"path": path(name)});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s.execute("file.export", &p)
}
fn has_stamp_shape(s: &Session) -> bool {
    s.doc().unwrap().doc.slides.iter().any(|sl| sl.shapes.iter().any(|sh| sh.name == "XDFU stamp"))
}

/// The text a PDF shows, read through each font's ToUnicode map.
fn pdf_text(bytes: Vec<u8>) -> String {
    let pdf = hayro_syntax::Pdf::new(bytes).unwrap();
    let mut all = String::new();
    for page in pdf.pages().iter() {
        let fonts = page.raw().get::<Dict<'_>>(b"Resources").unwrap().get::<Dict<'_>>(b"Font").unwrap();
        let mut maps: HashMap<Vec<u8>, HashMap<u16, String>> = HashMap::new();
        for key in fonts.keys() {
            let font = fonts.get::<Dict<'_>>(key.clone()).unwrap();
            let cmap = font.get::<Stream<'_>>(b"ToUnicode").unwrap().decoded().unwrap().into_owned();
            let cmap = std::str::from_utf8(&cmap).unwrap();
            let mut mapping = HashMap::new();
            let mut in_char_map = false;
            for line in cmap.lines() {
                if line.ends_with("beginbfchar") {
                    in_char_map = true;
                } else if line == "endbfchar" {
                    in_char_map = false;
                } else if in_char_map {
                    let parts: Vec<_> = line.split_whitespace().collect();
                    let cid = u16::from_str_radix(parts[0].trim_matches(['<', '>']), 16).unwrap();
                    let units: Vec<u16> = parts[1]
                        .trim_matches(['<', '>'])
                        .as_bytes()
                        .chunks(4)
                        .map(|c| u16::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                        .collect();
                    mapping.insert(cid, String::from_utf16(&units).unwrap());
                }
            }
            maps.insert(key.to_vec(), mapping);
        }
        let mut current: Vec<u8> = vec![];
        let mut ops = page.typed_operations();
        while let Some(op) = ops.next() {
            let mut show = |b: &[u8]| {
                if let Some(m) = maps.get(&current) {
                    for pair in b.chunks_exact(2) {
                        all.push_str(m.get(&u16::from_be_bytes([pair[0], pair[1]])).map_or("?", String::as_str));
                    }
                }
            };
            match op {
                TypedInstruction::TextFont(f) => current = f.0.to_vec(),
                TypedInstruction::ShowText(v) => show(v.0.as_bytes()),
                TypedInstruction::ShowTexts(vs) => {
                    for v in vs.0.iter::<Object<'_>>() {
                        if let Object::String(v) = v {
                            show(v.as_bytes());
                        }
                    }
                }
                _ => {}
            }
        }
        all.push('\n');
    }
    all
}

/// The copy number in the first `XDFU copy N | ` of `text`.
fn copy_no(text: &str) -> u64 {
    let at = text.find("XDFU copy ").expect("a stamp") + "XDFU copy ".len();
    text[at..].split(' ').next().unwrap().parse().expect("copy number")
}

#[test]
fn exported_pdf_carries_copy_number_and_user() {
    let _g = start();
    let mut s = deck("XDFU");
    let before = lines();
    export(&mut s, "a.pdf", json!({"format": "pdf"})).unwrap();
    let text = pdf_text(std::fs::read(path("a.pdf")).unwrap());
    let n = copy_no(&text);
    assert_eq!(n, before + 1, "the copy number is the audit counter: {text}");
    assert!(text.contains(&format!(" | {} | ", xdfu_core::os_user())), "the OS user is in the PDF text: {text}");
    assert_eq!(lines(), before + 1);
    assert!(!has_stamp_shape(&s), "the working presentation has no stamp left in it");
}

#[test]
fn print_of_a_grif_deck_succeeds_and_is_stamped() {
    let _g = start();
    let mut s = deck("\u{414}\u{421}\u{41f}");
    let before = lines();
    export(&mut s, "print.pdf", json!({"format": "pdf", "print": true})).unwrap();
    assert!(pdf_text(std::fs::read(path("print.pdf")).unwrap()).contains("XDFU copy "));
    assert_eq!(lines(), before + 1);
    assert!(std::fs::read_to_string(log()).unwrap().contains("\"print\""));
}

#[test]
fn every_copy_is_stamped_and_the_working_file_is_not() {
    let _g = start();
    let mut s = deck("\u{425}.\u{414}.\u{424}.\u{423}.");
    // the first save of an untitled deck is a new copy, so it is stamped; the deck stays clean
    let n = lines();
    s.execute("file.saveAs", &json!({"path": path("work.pptx")})).unwrap();
    assert_eq!(lines(), n + 1);
    let mut t = Session::new();
    t.execute("file.open", &json!({"path": path("work.pptx")})).unwrap();
    let stamped = t.doc().unwrap().doc.slides[0].shapes.iter().any(|sh| sh.text.as_ref().is_some_and(|b| b.text().starts_with("XDFU copy ")));
    assert!(stamped, "the saved pptx carries the stamp");
    assert!(!s.doc().unwrap().is_dirty() && s.doc().unwrap().path.as_deref() == Some(path("work.pptx").as_str()));
    assert!(!has_stamp_shape(&s), "the working document keeps its own shapes");

    // the working file is saved plainly: no stamp, no audit line
    s.execute("insert.textBox", &json!({"rect": [60.0, 300.0, 100.0, 30.0], "text": "more"})).unwrap();
    s.execute("file.save", &json!({})).unwrap();
    s.execute("file.saveAs", &json!({"path": path("work.pptx")})).unwrap();
    assert_eq!(lines(), n + 1, "saving the working file writes no audit line");

    // png, jpeg, gif, pptx, deckcraft and outline exports, a template and two downloads: one audit line each
    for (file, format) in
        [("a.png", "png"), ("a.jpg", "jpeg"), ("a.gif", "gif"), ("a.pptx", "pptx"), ("a.deckcraft", "deckcraft"), ("a.txt", "outline")]
    {
        export(&mut s, file, json!({"format": format})).unwrap();
    }
    assert!(std::fs::read_to_string(path("a.txt")).unwrap().starts_with("XDFU copy "));
    s.execute("file.saveTemplate", &json!({"path": path("t.potx")})).unwrap();
    let dl = s.execute("file.saveBytes", &json!({"format": "outline"})).unwrap();
    assert!(deckcraft_engine::cmd::base64_decode(dl["data"].as_str().unwrap()).unwrap().starts_with(b"XDFU copy "));
    s.execute("file.saveBytes", &json!({"format": "pptx"})).unwrap();
    assert_eq!(lines(), n + 1 + 9);
    assert!(!has_stamp_shape(&s));
    // copy is not refused (it may have nothing selected, which is not a refusal)
    assert!(!matches!(s.execute("edit.copy", &json!({})), Err(EngineError::Disabled(..))));
    let all = std::fs::read_to_string(log()).unwrap();
    assert!(audit::verify(&log()).is_ok() && all.contains("\"export\"") && all.contains("save_as") && all.contains("download"));
}

#[test]
fn a_deck_without_a_grif_is_never_stamped() {
    let _g = start();
    // a grif low on slide 1 is not in its top quarter
    let mut s = deck("Quarterly report");
    s.execute("insert.textBox", &json!({"rect": [60.0, 450.0, 300.0, 40.0], "text": "XDFU"})).unwrap();
    let before = lines();
    export(&mut s, "plain.pdf", json!({"format": "pdf"})).unwrap();
    assert!(!pdf_text(std::fs::read(path("plain.pdf")).unwrap()).contains("XDFU copy"));
    export(&mut s, "plain.txt", json!({"format": "outline"})).unwrap();
    assert!(!std::fs::read_to_string(path("plain.txt")).unwrap().contains("XDFU copy"));
    export(&mut s, "plain.png", json!({"format": "png"})).unwrap();
    s.execute("file.saveBytes", &json!({})).unwrap();
    assert_eq!(lines(), before, "no audit line for a non-grif deck");
}

#[test]
fn insert_grif_adds_a_box_and_takes_a_copy_number() {
    let _g = start();
    assert!(find_command("office.xdfu.insertGrif").is_some());
    let mut s = deck("Budget");
    let before = lines();
    assert!(s.execute("office.xdfu.insertGrif", &json!({"grif": "nope"})).is_err());
    let r = s.execute("office.xdfu.insertGrif", &json!({"grif": "\u{425}\u{414}\u{424}\u{423}"})).unwrap();
    assert_eq!(r["copy"], before + 1);
    let text = |s: &Session| {
        s.doc().unwrap().doc.slides[0]
            .shapes
            .iter()
            .filter(|sh| sh.name == "XDFU grif")
            .map(|sh| sh.text.as_ref().unwrap().text())
            .collect::<Vec<_>>()
    };
    assert_eq!(text(&s), vec![format!("\u{425}\u{414}\u{424}\u{423}\nNusxa {}", before + 1)]);
    assert_eq!(lines(), before + 1);
    // the deck now carries a grif in its top quarter, so a PDF of it is stamped; inserting again replaces the box
    export(&mut s, "g.pdf", json!({"format": "pdf"})).unwrap();
    let pdf = pdf_text(std::fs::read(path("g.pdf")).unwrap());
    assert!(pdf.contains("XDFU copy ") && pdf.contains("Nusxa "), "{pdf}");
    s.execute("office.xdfu.insertGrif", &json!({})).unwrap();
    assert_eq!(text(&s).len(), 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(text(&s), vec![format!("\u{425}\u{414}\u{424}\u{423}\nNusxa {}", before + 1)], "undoable");
}
