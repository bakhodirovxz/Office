use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::json;
use wordcraft_engine::{CmdError, CmdResult, Session};

use crate::hooks::{self, CommandHook};
use crate::mode::{self, Mode};
use crate::preset;

/// The mode and the hook are process-wide: tests that touch them take turns (nextest also gives each its own process).
static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    hooks::set_hook(None);
    mode::set_store(mode::Store { read: || Mode::Simple, write: |_| {} });
    g
}

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    docs_translit::install(&mut s);
    crate::install(&mut s);
    s
}

fn uz(key: &str) -> String {
    match key {
        "Document" => "Hujjat",
        "File" => "Fayl",
        "Page" => "Sahifa",
        "Spelling" => "Imlo",
        "Transliterate" => "Transliteratsiya",
        "History" => "Tarix",
        other => other,
    }
    .to_string()
}

fn ru(key: &str) -> String {
    if key == "Document" { "Документ".into() } else { key.to_string() }
}

fn tab_names(s: &Session) -> Vec<String> {
    s.ribbon.tabs().iter().filter(|t| !t.hidden).map(|t| t.name.clone()).collect()
}

/// Every id the preset and the QAT name is registered (the test that keeps the toolbar true at each upstream sync).
#[test]
fn office_toolbar_ids_exist() {
    let _g = turn();
    let s = session();
    for id in preset::ids() {
        assert!(s.registry.get(id).is_some(), "`{id}` is not registered");
    }
    for id in hooks::GUARDED {
        assert!(s.registry.get(id).is_some(), "guarded `{id}` is not registered");
    }
}

#[test]
fn first_run_applies_the_preset() {
    let _g = turn();
    let mut s = session();
    assert!(s.ribbon.ribbon_is_default());
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    assert!(preset::is_preset(&s.ribbon, &s.registry));
    assert_eq!(tab_names(&s), ["Home", "Insert", "Hujjat"]);
    let doc_tab = s.ribbon.tabs().iter().find(|t| t.custom).cloned().unwrap();
    let groups: Vec<_> = doc_tab.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(groups, ["Fayl", "Sahifa", "Imlo", "Transliteratsiya", "Tarix"]);
    assert_eq!(s.ribbon.tabs().iter().filter(|t| t.hidden).count(), 10);
    assert_eq!(s.ribbon.qat(), ["file.save", "edit.undo", "edit.redo", "view.marks", "office.ribbonFull"]);
}

#[test]
fn the_language_changes_the_names_only() {
    let _g = turn();
    let mut s = session();
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    crate::ribbon_frame(&mut s, "ru", &ru);
    assert_eq!(tab_names(&s), ["Home", "Insert", "Документ"]);
    assert!(preset::is_preset(&s.ribbon, &s.registry));
}

#[test]
fn a_users_own_layout_is_left_alone() {
    let _g = turn();
    let mut s = session();
    s.run("tools.customizeRibbon", &json!({"hide": "Draw"})).unwrap();
    let before = s.ribbon.clone();
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    assert_eq!(s.ribbon.tabs, before.tabs);
    // ...until the Simple ribbon button asks for the preset.
    s.run(crate::SIMPLE, &json!({})).unwrap();
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    assert!(preset::is_preset(&s.ribbon, &s.registry));
}

#[test]
fn the_toggle_round_trips_and_writes_the_setting() {
    let _g = turn();
    static WRITTEN: Mutex<Option<Mode>> = Mutex::new(None);
    mode::set_store(mode::Store { read: || Mode::Simple, write: |m| *WRITTEN.lock().unwrap() = Some(m) });
    let mut s = session();
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    let preset_tabs = s.ribbon.tabs.clone();

    s.run(crate::FULL, &json!({})).unwrap();
    assert_eq!(*WRITTEN.lock().unwrap(), Some(Mode::Full));
    assert!(s.ribbon.ribbon_is_default(), "the full ribbon is upstream's own");
    assert_eq!(s.ribbon.qat(), ["file.save", "edit.undo", "edit.redo", "view.marks", "office.ribbonSimple"]);
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    assert!(s.ribbon.ribbon_is_default(), "a frame in full mode leaves it");

    s.run(crate::SIMPLE, &json!({})).unwrap();
    assert_eq!(*WRITTEN.lock().unwrap(), Some(Mode::Simple));
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    assert_eq!(s.ribbon.tabs, preset_tabs);
    assert_eq!(s.ribbon.qat().last(), Some(&"office.ribbonFull"));
}

#[test]
fn a_saved_preset_is_reset_when_the_setting_says_full() {
    let _g = turn();
    let mut s = session();
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    // A new run of the app: the layout came back from ui.json (in another language), the setting says full.
    mode::set_store(mode::Store { read: || Mode::Full, write: |_| {} });
    crate::ribbon_frame(&mut s, "ru", &ru);
    assert!(s.ribbon.ribbon_is_default());
    assert_eq!(s.ribbon.qat().last(), Some(&"office.ribbonSimple"));
}

/// An id a version does not have is dropped, not fatal (toolbar-design section 9).
#[test]
fn an_unknown_id_is_dropped() {
    let _g = turn();
    let mut s = Session::new(wordcraft_doc::Document::new());
    // No transliteration crate: its three ids are unknown here.
    crate::install(&mut s);
    crate::ribbon_frame(&mut s, "uz-latn", &uz);
    let tab = s.ribbon.tabs().iter().find(|t| t.custom).cloned().unwrap();
    let all: Vec<&str> = tab.groups.iter().flat_map(|g| g.commands.iter().map(String::as_str)).collect();
    assert!(!all.iter().any(|c| c.starts_with("tools.translit")));
    assert!(all.contains(&"file.print"));
}

#[test]
fn the_new_commands_only_ask_the_ui() {
    let _g = turn();
    let mut s = session();
    s.run("office.exportPdf", &json!({})).unwrap();
    assert_eq!(s.ui_requests.pop(), Some(json!({"open": "exportPdf"})), "the PDF save dialog, not the Print page");
    s.run("file.print", &json!({})).unwrap();
    assert_eq!(s.ui_requests.pop(), Some(json!({"open": "print"})));
    assert!(!s.dirty);
}

struct Probe {
    seen: Mutex<Vec<String>>,
}

impl CommandHook for Probe {
    fn before(&self, id: &str, _: &Session) -> Option<&'static str> {
        (id == "file.print").then_some("restricted document")
    }
    fn after(&self, id: &str, _: &Session, _: &serde_json::Value, r: &CmdResult) {
        self.seen.lock().unwrap().push(format!("{id}:{}", r.is_ok()));
    }
}

#[test]
fn hook_refuses_print() {
    let _g = turn();
    let mut s = session();
    let probe = Arc::new(Probe { seen: Mutex::new(Vec::new()) });
    hooks::set_hook(Some(probe.clone()));
    let spec = s.registry.get("file.print").unwrap();
    assert_eq!((spec.enabled)(&s), Some("restricted document"), "the UI greys the control out");
    match s.run("file.print", &json!({})) {
        Err(CmdError::Disabled(why)) => assert!(why.contains("restricted document"), "{why}"),
        other => panic!("print was not refused: {other:?}"),
    }
    assert!(s.ui_requests.is_empty(), "the original never ran");
    assert!(probe.seen.lock().unwrap().is_empty());
}

#[test]
fn hook_sees_save() {
    let _g = turn();
    let mut s = session();
    let probe = Arc::new(Probe { seen: Mutex::new(Vec::new()) });
    hooks::set_hook(Some(probe.clone()));
    let dir = std::env::temp_dir().join(format!("docs-office-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.docx");
    s.run("file.save", &json!({"path": path.to_str().unwrap()})).unwrap();
    assert_eq!(*probe.seen.lock().unwrap(), ["file.save:true"]);
    std::fs::remove_dir_all(&dir).ok();
}

/// With no hook the wrapped ids behave like upstream's: same specs, same result, same side effects.
#[test]
fn without_a_hook_nothing_changes() {
    let _g = turn();
    let mut plain = Session::new(wordcraft_doc::Document::new());
    let mut wrapped = session();
    for id in hooks::GUARDED {
        let key = |s: &Session| s.registry.get(id).map(|c| (c.label, c.shortcut, c.mutates));
        assert_eq!(key(&plain), key(&wrapped), "{id}");
    }
    let a = plain.run("file.print", &json!({})).unwrap();
    let b = wrapped.run("file.print", &json!({})).unwrap();
    assert_eq!(a, b);
    assert_eq!(plain.ui_requests, wrapped.ui_requests);
}

fn level_texts(s: &Session, num: u64) -> Vec<String> {
    let n = s.doc.numbering.nums.iter().find(|n| u64::from(n.id) == num).unwrap();
    s.doc.numbering.abstracts.iter().find(|a| a.id == n.abstract_id).unwrap().levels.iter().take(2).map(|l| l.text.clone()).collect()
}

/// The preset's level text follows the interface language (HRM orders: "1-bob", "I modda", "1.01-boʻlim").
#[test]
fn multilevel_presets_write_the_interface_language() {
    let _g = turn();
    for (lang, article, chapter) in [("uz-latn", ["%1 modda.", "%1.%2-boʻlim"], "%1-bob"), ("ru", ["Статья %1.", "Раздел %1.%2"], "Глава %1"), ("en", ["Article %1.", "Section %1.%2"], "Chapter %1")] {
        let mut s = session();
        crate::ribbon_frame(&mut s, lang, &uz);
        let r = s.run("list.multilevelPreset", &json!({"preset": "article"})).unwrap();
        assert_eq!(level_texts(&s, r["num"].as_u64().unwrap()), article, "{lang}");
        let r = s.run("list.multilevelPreset", &json!({"preset": "chapter"})).unwrap();
        assert_eq!(level_texts(&s, r["num"].as_u64().unwrap())[0], chapter, "{lang}");
        let r = s.run("list.multilevelPreset", &json!({"preset": "headings"})).unwrap();
        assert_eq!(level_texts(&s, r["num"].as_u64().unwrap()), ["%1", "%1.%2"], "{lang}");
    }
}

/// Export PDF from our button goes through the guarded `file.exportPdf`: a policy refuses it and the after-hook sees it.
#[test]
fn export_pdf_button_is_guarded() {
    let _g = turn();
    let mut s = session();
    let probe = Arc::new(Probe { seen: Mutex::new(Vec::new()) });
    hooks::set_hook(Some(probe.clone()));
    s.run("office.exportPdf", &json!({})).unwrap();
    assert_eq!(*probe.seen.lock().unwrap(), ["file.exportPdf:true"]);
    struct Refuse;
    impl CommandHook for Refuse {
        fn before(&self, id: &str, _: &Session) -> Option<&'static str> {
            (id == "file.exportPdf").then_some("restricted document")
        }
    }
    hooks::set_hook(Some(Arc::new(Refuse)));
    s.ui_requests.clear();
    assert!(matches!(s.run("office.exportPdf", &json!({})), Err(CmdError::Disabled(_))));
    assert!(s.ui_requests.is_empty());
}
