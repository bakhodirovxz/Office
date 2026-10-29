//! WP10 and WP11 through the command hook: the snapshot on save, and the XDFU watermark. The layer and the hook are process-wide, so the
//! tests take turns (nextest also gives each its own process).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use hayro_syntax::content::ops::TypedInstruction;
use hayro_syntax::object::{Dict, Object, Stream};
use serde_json::json;
use suite_core::history::{Limits, Store};
use wordcraft_engine::Session;
use xdfu_core::audit;

use crate::layer;

pub(crate) static TURN: Mutex<()> = Mutex::new(());

struct T {
    dir: PathBuf,
    _g: MutexGuard<'static, ()>,
}

impl T {
    fn new(tag: &str) -> T {
        let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("docs-office-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        T { dir, _g: g }
    }
    fn p(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }
    fn log(&self) -> PathBuf {
        self.dir.join("xdfu-audit.jsonl")
    }
    fn audit(&self) -> Option<(PathBuf, PathBuf)> {
        Some((self.log(), self.dir.join("xdfu-audit.key")))
    }
    fn lines(&self) -> u64 {
        if self.log().exists() { audit::verify(&self.log()).expect("the audit chain verifies").lines } else { 0 }
    }
}

impl Drop for T {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn roomy() -> Limits {
    Limits { versions: 50, days: 90, max_bytes: 1 << 30, big_file_bytes: 1 << 29, big_file_versions: 5 }
}

/// A session that went through `install`, with `first` as the first paragraph and "Budget" below it.
fn session(first: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    crate::install(&mut s);
    s.run("text.insert", &json!({"text": first})).unwrap();
    s.run("text.newParagraph", &json!({})).unwrap();
    s.run("text.insert", &json!({"text": "Budget"})).unwrap();
    s
}

/// The text a PDF shows, read through each font's ToUnicode map.
fn pdf_text(path: &str) -> String {
    let pdf = hayro_syntax::Pdf::new(std::fs::read(path).unwrap()).unwrap();
    let mut all = String::new();
    for page in pdf.pages().iter() {
        let fonts = page.raw().get::<Dict<'_>>(b"Resources").unwrap().get::<Dict<'_>>(b"Font").unwrap();
        let mut maps: HashMap<Vec<u8>, HashMap<u16, String>> = HashMap::new();
        for key in fonts.keys() {
            let font = fonts.get::<Dict<'_>>(key.clone()).unwrap();
            let cmap = font.get::<Stream<'_>>(b"ToUnicode").unwrap().decoded().unwrap().into_owned();
            let cmap = std::str::from_utf8(&cmap).unwrap();
            let (mut mapping, mut in_char_map) = (HashMap::new(), false);
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
fn save_snapshot_list_restore_round_trip() {
    let t = T::new("hist");
    let store = Store::new(t.dir.join("store"), roomy());
    layer::configure(Some(Store::new(t.dir.join("store"), roomy())), None);
    let mut s = session("First");
    let f = t.p("a.docx");
    s.run("file.save", &json!({"path": f})).unwrap();
    // the history thread reads the file when it gets to it: a save before that shares the version (WP23)
    assert!(layer::flush_snapshots(SETTLE));
    s.run("text.insert", &json!({"text": " edited"})).unwrap();
    s.run("file.save", &json!({})).unwrap();
    let path = PathBuf::from(&f);
    assert!(layer::flush_snapshots(SETTLE));
    assert_eq!(store.list(&path).unwrap().len(), 2);
    // an autosave right after an explicit save makes no version, an explicit save does
    s.run("text.insert", &json!({"text": " more"})).unwrap();
    s.run("file.save", &json!({"autosave": true})).unwrap();
    assert!(layer::flush_snapshots(SETTLE));
    assert_eq!(store.list(&path).unwrap().len(), 2);
    s.run("file.save", &json!({})).unwrap();
    assert!(layer::flush_snapshots(SETTLE));
    assert_eq!(store.list(&path).unwrap().len(), 3);
    // a pdf copy is not a version of the document
    s.run("file.exportPdf", &json!({"path": t.p("a.pdf")})).unwrap();
    assert!(layer::flush_snapshots(SETTLE));
    assert_eq!(store.list(&path).unwrap().len(), 3);
    let v1 = store.read(&path, 1).unwrap();
    store.restore(&path, 1).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), v1);
    assert_eq!(store.list(&path).unwrap().len(), 4);
    assert_eq!(store.verify(&path).unwrap().lines, 4);
}

const SETTLE: std::time::Duration = std::time::Duration::from_secs(20);

/// WP23 "Save freezes the app": the snapshot ran inside `file.save` on the UI thread and waited for the store (its lock, then a prune
/// over every document's index). A store held by another process now delays only the version, never the save.
#[test]
fn a_busy_history_store_does_not_hold_up_the_save() {
    let t = T::new("busy");
    let store = Store::new(t.dir.join("store"), roomy());
    layer::configure(Some(Store::new(t.dir.join("store"), roomy())), None);
    std::fs::create_dir_all(t.dir.join("store")).unwrap();
    let lock = t.dir.join("store").join(".lock");
    std::fs::write(&lock, b"").unwrap();
    let mut s = session("First");
    let f = t.p("busy.docx");
    let started = std::time::Instant::now();
    s.run("file.save", &json!({"path": f})).unwrap();
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "the save waited {took:?} for the history store");
    std::fs::remove_file(&lock).unwrap();
    assert!(layer::flush_snapshots(SETTLE));
    assert_eq!(store.list(&PathBuf::from(&f)).unwrap().len(), 1, "the version is recorded once the store is free");
}

#[test]
fn xdfu_off_registers_nothing() {
    let t = T::new("off");
    layer::configure(None, None);
    let mut s = session("XDFU");
    assert!(!layer::xdfu_on());
    assert!(s.registry.get(layer::INSERT_GRIF).is_none());
    assert!(s.registry.all().iter().all(|c| !c.id.starts_with("office.xdfu")));
    // a grif document exports as it would without the layer: the same text, no stamp, no audit file
    s.run("file.exportPdf", &json!({"path": t.p("a.pdf")})).unwrap();
    let text = pdf_text(&t.p("a.pdf"));
    assert!(text.contains("Budget") && !text.contains("XDFU copy"), "{text}");
    s.run("file.saveAs", &json!({"path": t.p("a.txt")})).unwrap();
    assert!(!std::fs::read_to_string(t.p("a.txt")).unwrap().contains("XDFU copy"));
    assert_eq!(s.doc.settings.watermark, None);
    assert!(!t.log().exists());
}

#[test]
fn watermark_carries_copy_number_and_user_and_nothing_is_refused() {
    let t = T::new("wm");
    layer::configure(None, t.audit());
    assert!(layer::xdfu_on());
    let mut s = session("XDFU");
    assert!(s.registry.get(layer::INSERT_GRIF).is_some());
    let user = xdfu_core::os_user();
    // export PDF
    s.run("file.exportPdf", &json!({"path": t.p("a.pdf")})).unwrap();
    let text = pdf_text(&t.p("a.pdf"));
    assert!(text.contains("XDFU copy 1 | ") && text.contains(&user), "{text}");
    // print to PDF is the next copy
    s.run("file.printPdf", &json!({"path": t.p("p.pdf")})).unwrap();
    let printed = pdf_text(&t.p("p.pdf"));
    assert_eq!(copy_no(&printed), 2, "{printed}");
    assert!(printed.contains(&user));
    // save as txt: the first line; save as another docx: succeeds; the working document stays clean
    s.run("file.saveAs", &json!({"path": t.p("c.txt")})).unwrap();
    let txt = std::fs::read_to_string(t.p("c.txt")).unwrap();
    assert!(txt.starts_with("XDFU copy 3 | "), "{txt}");
    s.run("file.saveAs", &json!({"path": t.p("d.docx")})).unwrap();
    assert_eq!(s.doc.settings.watermark, None);
    assert_eq!(t.lines(), 4, "one audit line per copy, the chain verifies");
    // a plain save of the working file is not a copy
    s.run("file.save", &json!({})).unwrap();
    assert_eq!(t.lines(), 4);
    // copy is not blocked
    s.run("select.all", &json!({})).unwrap();
    s.run("edit.copy", &json!({})).unwrap();
    // a document without a grif is never stamped
    let mut plain = session("Hello");
    plain.run("file.exportPdf", &json!({"path": t.p("n.pdf")})).unwrap();
    assert!(!pdf_text(&t.p("n.pdf")).contains("XDFU copy"));
    assert_eq!(t.lines(), 4);
}

#[test]
fn printing_to_paper_carries_the_stamp_and_an_audit_line() {
    let t = T::new("paper");
    layer::configure(None, t.audit());
    let mut s = session("XDFU");
    // what the Print dialog's Submit does: the PDF through the print hook, not the `file.printPdf` command
    let (bytes, _) = layer::print_wrap(&mut s, &mut |s| s.print_pdf(&json!({}))).unwrap();
    std::fs::write(t.p("paper.pdf"), bytes).unwrap();
    let text = pdf_text(&t.p("paper.pdf"));
    assert_eq!(copy_no(&text), 1, "{text}");
    assert_eq!(t.lines(), 1);
    assert_eq!(s.doc.settings.watermark, None);
    // a job that fails is neither stamped nor counted as a copy, and the error comes through
    let r = layer::print_wrap(&mut s, &mut |_| Err(wordcraft_engine::CmdError::Failed("no".into())));
    assert!(r.is_err());
    // no grif: printed as it is
    let mut plain = session("Hello");
    layer::print_wrap(&mut plain, &mut |s| s.print_pdf(&json!({}))).unwrap();
    assert_eq!(t.lines(), 2, "the failed job left a print_failed line, the plain one nothing");
}

#[test]
fn insert_grif_writes_a_right_aligned_paragraph_with_the_copy_number() {
    let t = T::new("grif");
    layer::configure(None, t.audit());
    let mut s = session("Hello");
    let r = s.run(layer::INSERT_GRIF, &json!({"grif": "ДСП"})).unwrap();
    assert_eq!(r["copy"], 1);
    let body = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    assert!(body.starts_with("ДСП\nNusxa \u{2116} 1\nHello"), "{body:?}");
    assert!(s.run(layer::INSERT_GRIF, &json!({"grif": "secret"})).is_err());
    assert_eq!(t.lines(), 1);
    // now the document carries a grif: its next copy is number 2
    s.run("file.exportPdf", &json!({"path": t.p("a.pdf")})).unwrap();
    assert_eq!(copy_no(&pdf_text(&t.p("a.pdf"))), 2);
}

#[test]
fn version_history_opens_the_panel_only_for_a_saved_document() {
    let _t = T::new("vers");
    layer::configure(None, None);
    let mut s = session("x");
    // unsaved: upstream's in-memory list (an array); the on-disk History needs a file
    assert!(s.run("file.versions", &json!({})).unwrap().is_array());
    assert!(s.run("office.history", &json!({})).unwrap_err().to_string().contains("Save the file first"));
}
