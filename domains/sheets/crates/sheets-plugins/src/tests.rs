//! The Sheets side with the real engine and a fake in-process guest: the slot commands, the undo step, the restore, the protected
//! workbook, the text map against formulas and table headers, the input lock, and the malicious-plugin rows that reach the engine
//! (M5 with a real `file.saveAs`, M8 against a live workbook). The executor's own rules are tested in `plugin-adapter`.
use std::sync::{Arc, Mutex, MutexGuard};

use gridcraft_engine::Session;
use gridcraft_engine::core::{CellRef, Value as CellValue};
use gridcraft_engine::office_hook::Hook;
use plugin_adapter::plugin_api::action::{Action, Answer, Call, Next};
use plugin_adapter::plugin_api::manifest;
use plugin_adapter::plugin_api::textmap::{Apply, TextMap};
use plugin_adapter::plugin_api::wire::Message;
use plugin_adapter::{Guest, GuestSession, Plugin, bridge};
use serde_json::{Value, json};

use crate::{Ed, Lock, register};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    plugin_adapter::set_app("sheets");
    plugin_adapter::clear();
    bridge::reset();
    g
}

const MANIFEST: &str = r#"
id = "sheet-test"
version = "1.0.0"
api = 1
apps = ["sheets"]
publisher = "Test"
name.en = "Sheet test"

[permissions]
document = "write"

[[command]]
id = "go"
label.en = "Go"
reads = ["textmap"]
"#;

type Handler = dyn FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send;
struct Fake(Arc<Mutex<Box<Handler>>>);
struct FakeSession(Arc<Mutex<Box<Handler>>>);

impl Guest for Fake {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        Ok(Box::new(FakeSession(self.0.clone())))
    }
}

impl GuestSession for FakeSession {
    fn round(&mut self, m: &Message) -> Result<Message, String> {
        let call: Call = m.header_as().map_err(|e| e.to_string())?;
        (self.0.lock().unwrap_or_else(|e| e.into_inner()))(&call, &m.blobs)
    }
}

fn answer(actions: Vec<Action>, next: Option<Next>, blobs: Vec<Vec<u8>>) -> Result<Message, String> {
    Message::new(&Answer { actions, next }, blobs).map_err(|e| e.to_string())
}

fn add(f: impl FnMut(&Call, &[Vec<u8>]) -> Result<Message, String> + Send + 'static) {
    plugin_adapter::add(Plugin::new(manifest::parse(MANIFEST).unwrap(), Arc::new(Fake(Arc::new(Mutex::new(Box::new(f))))))).unwrap();
    register();
}

fn set(cell: &str, input: &str) -> Action {
    Action::Run { id: "cell.set".into(), params: json!({"cell": cell, "input": input}) }
}

fn session(values: Value) -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": values})).unwrap();
    s.execute("selection.set", json!({"range": "A1"})).unwrap();
    s
}

fn text(s: &Session, a: &str) -> String {
    s.doc().unwrap().wb.sheet(0).unwrap().value(CellRef::parse(a).unwrap()).as_text().unwrap_or_default().to_string()
}

const GO: &str = "plugin.sheet-test.go";

fn upper(map: &TextMap) -> Apply {
    Apply { paragraphs: map.paragraphs.iter().map(|p| p.segments.iter().map(|g| Some(g.text.to_uppercase())).collect()).collect() }
}

#[test]
fn a_slot_command_is_one_undo_step_and_undo_restores() {
    let _t = turn();
    add(|_, _| answer(vec![set("A1", "X"), set("B1", "Y")], None, vec![]));
    let mut s = session(json!([["a", "b"]]));
    let (wb, depth) = (s.doc().unwrap().wb.clone(), s.doc().unwrap().undo.len());
    s.execute(GO, json!({})).unwrap();
    assert_eq!((text(&s, "A1"), text(&s, "B1")), ("X".into(), "Y".into()));
    assert_eq!(s.doc().unwrap().undo.len(), depth + 1, "two edits, one undo step");
    assert_eq!(s.doc().unwrap().undo.last().unwrap().label, "Go");
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().wb, *wb);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(text(&s, "B1"), "Y");
}

#[test]
fn a_failing_plugin_leaves_the_workbook_the_undo_stack_and_the_calculation_as_they_were() {
    let _t = turn();
    add(|c, _| {
        if c.results.is_empty() {
            answer(vec![set("A1", "5")], Some(Next { reads: vec![] }), vec![])
        } else {
            answer(vec![Action::Run { id: "cell.set".into(), params: json!({"cell": "not a cell", "input": "x"}) }], None, vec![])
        }
    });
    let mut s = session(json!([[1, "=A1*2"]]));
    let (wb, depth) = (s.doc().unwrap().wb.clone(), s.doc().unwrap().undo.len());
    assert!(s.execute(GO, json!({})).is_err());
    assert_eq!(*s.doc().unwrap().wb, *wb);
    assert_eq!(s.doc().unwrap().undo.len(), depth);
    // the dependency graph follows the workbook put back: B1 still recalculates from A1
    s.execute("cell.set", json!({"cell": "A1", "input": "7"})).unwrap();
    assert_eq!(s.doc().unwrap().wb.sheet(0).unwrap().value(CellRef::parse("B1").unwrap()), CellValue::Number(14.0));
}

#[test]
fn the_text_map_has_the_constant_text_cells_and_writes_through_the_builtin() {
    let _t = turn();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    add(move |c, blobs| {
        let map = TextMap::decode(&blobs[c.reads["textmap"] as usize]).unwrap();
        *seen2.lock().unwrap() = map.paragraphs.iter().map(|p| p.segments[0].text.clone()).collect::<Vec<_>>();
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![upper(&map).encode().unwrap()])
    });
    let mut s = session(json!([["salom", 3, "=\"x\"&A1"], ["dunyo", true, "ok"]]));
    s.execute(GO, json!({})).unwrap();
    assert_eq!(*seen.lock().unwrap(), ["salom", "dunyo", "ok"], "text cells only, row by row; numbers, booleans and formulas are not in it");
    assert_eq!((text(&s, "A1"), text(&s, "A2"), text(&s, "C2")), ("SALOM".into(), "DUNYO".into(), "OK".into()));
    let c1 = s.doc().unwrap().wb.sheet(0).unwrap().value(CellRef::parse("C1").unwrap());
    assert_eq!(c1, CellValue::from("xSALOM"), "a formula reading a written cell is recalculated");
}

#[test]
fn a_protected_sheet_or_structure_is_refused_and_a_reading_plugin_still_runs() {
    let _t = turn();
    add(|_, _| answer(vec![set("A1", "X")], None, vec![]));
    let mut s = session(json!([["a"]]));
    s.execute("review.protectSheet", json!({})).unwrap();
    let e = s.execute(GO, json!({})).unwrap_err().to_string();
    assert!(e.contains(bridge::PROTECTED), "{e}");
    assert_eq!(text(&s, "A1"), "a");
    let mut s = session(json!([["a"]]));
    s.execute("review.protectWorkbook", json!({})).unwrap();
    assert!(s.execute(GO, json!({})).is_err());
    plugin_adapter::clear();
    let toml = MANIFEST.replace("document = \"write\"", "document = \"read\"");
    plugin_adapter::add(Plugin::new(
        manifest::parse(&toml).unwrap(),
        Arc::new(Fake(Arc::new(Mutex::new(Box::new(|_: &Call, _: &[Vec<u8>]| answer(vec![Action::UiStatus { text: "read".into() }], None, vec![])))))),
    ))
    .unwrap();
    assert_eq!(s.execute(GO, json!({})).unwrap()["status"], "read");
}

#[test]
fn m5_a_save_as_is_refused_and_no_file_is_written() {
    let _t = turn();
    let path = std::env::temp_dir().join(format!("sheets-plugins-m5-{}.xlsx", std::process::id()));
    let p = path.to_string_lossy().to_string();
    for bad in [
        Action::Run { id: "file.saveAs".into(), params: json!({"path": p}) },
        Action::Run { id: "file.exportCsv".into(), params: json!({"path": p}) },
        Action::Run { id: "data.fromTextCsv".into(), params: json!({"path": p}) },
        Action::Run { id: "cell.set".into(), params: json!({"cell": "A1", "input": "=HYPERLINK(\"file:///C:/x\")"}) },
    ] {
        plugin_adapter::clear();
        let b = bad.clone();
        add(move |_, _| answer(vec![set("A1", "first"), b.clone()], None, vec![]));
        let mut s = session(json!([["a"]]));
        let e = s.execute(GO, json!({})).unwrap_err().to_string();
        assert!(e.contains("refused"), "{bad:?}: {e}");
        assert_eq!(text(&s, "A1"), "a");
        assert!(!path.exists(), "{bad:?} wrote a file");
    }
}

#[test]
fn m8_an_apply_after_the_cell_changed_or_with_the_wrong_shape_is_refused() {
    let _t = turn();
    add(|_, blobs| {
        let map = TextMap::decode(&blobs[0]).unwrap();
        let mut ap = upper(&map);
        ap.paragraphs[0].push(Some("extra".into()));
        answer(vec![Action::TextmapApply { blob: 0 }], None, vec![ap.encode().unwrap()])
    });
    let mut s = session(json!([["a", "b"]]));
    let wb = s.doc().unwrap().wb.clone();
    assert!(s.execute(GO, json!({})).is_err());
    assert_eq!(*s.doc().unwrap().wb, *wb);
    // the live cell no longer matches the read (direct check of the apply)
    let mut ed = Ed(&mut s);
    let (map, at) = plugin_adapter::Editor::read_map(&mut ed, "sheet").unwrap();
    ed.0.execute("cell.set", json!({"cell": "A1", "input": "moved"})).unwrap();
    let e = plugin_adapter::Editor::apply_map(&mut ed, &map, &at, &upper(&map)).unwrap_err();
    assert!(e.contains("A1 changed"), "{e}");
    assert_eq!(text(&s, "B1"), "b", "nothing was written");
}

#[test]
fn a_disabled_plugin_is_not_run_and_the_menu_lists_the_enabled_ones() {
    let _t = turn();
    add(|_, _| answer(vec![set("A1", "X")], None, vec![]));
    let mut s = session(json!([["a"]]));
    assert_eq!(crate::menu("en")[0].1[0].1, GO);
    plugin_adapter::set_enabled("sheet-test", false);
    assert!(s.execute(GO, json!({})).is_err());
    assert!(crate::menu("en").is_empty());
    assert_eq!(text(&s, "A1"), "a");
}

#[test]
fn the_web_lock_refuses_changes_but_not_moves_while_a_call_waits() {
    let _t = turn();
    let mut p = Plugin::new(manifest::parse(MANIFEST).unwrap(), Arc::new(bridge::PageGuest));
    p.origins = vec![];
    plugin_adapter::add(p).unwrap();
    register();
    let mut s = session(json!([["a"]]));
    let r = s.execute(GO, json!({})).unwrap();
    assert!(r["pending"].is_number());
    assert!(bridge::tick(&mut Ed(&mut s)));
    assert!(matches!(bridge::take_out()[0], bridge::Out::Call { .. }));
    for (id, refused) in [("cell.set", true), ("home.bold", true), ("edit.undo", true), ("file.save", true), (GO, true), ("selection.set", false), ("sheet.next", false)] {
        assert_eq!(Lock.before(&s, id, &json!({})).is_err(), refused, "{id}");
    }
    bridge::answer(
        &mut Ed(&mut s),
        bridge::Answer { call: json!(1), plugin: None, actions: json!([{"type": "run", "id": "cell.set", "params": {"cell": "A1", "input": "W"}}]), blobs: vec![], reads: Value::Null, more: false, abort: false },
    );
    assert!(Lock.before(&s, "cell.set", &json!({})).is_ok(), "unlocked when the answer is in");
    assert_eq!(text(&s, "A1"), "W");
    assert_eq!(s.doc().unwrap().undo.last().unwrap().label, "Go");
}

/// PLAN 5.2 for sheets: every exception and read of the allowlist is a real command, no allowed command's params speak of a path,
/// file, URL, bytes or a link, and the refused families stay refused.
#[test]
fn the_allowlist_matches_the_registry() {
    use plugin_adapter::plugin_api::allow::{SHEETS, check_run_in};
    let specs = gridcraft_engine::command_specs();
    let has = |id: &str| specs.iter().any(|c| c.id == id);
    for (id, _) in SHEETS.exceptions {
        assert!(has(id), "exception {id} is not a command");
        assert!(SHEETS.prefixes.iter().any(|p| id.starts_with(p)), "exception {id} is outside the prefixes");
    }
    for id in SHEETS.reads {
        assert!(has(id), "read {id} is not a command");
    }
    let mut allowed = 0;
    for c in specs {
        let ok = check_run_in(&SHEETS, c.id, &json!({}));
        if ok.is_ok() {
            allowed += 1;
            let p = c.params.split('→').next().unwrap_or_default().to_ascii_lowercase();
            for w in ["path", "url", "file", "base64", "bytes", "href", "link", "folder"] {
                assert!(!p.contains(w), "{} is allowed but its params mention {w}: {}", c.id, c.params);
            }
        }
        if !SHEETS.reads.contains(&c.id) && ["file.", "edit.", "tools.", "data.", "view.", "office.", "plugin.", "automate.", "review.", "insert.", "formulas."].iter().any(|f| c.id.starts_with(f)) {
            assert!(ok.is_err(), "{} must not be runnable by a plugin", c.id);
        }
    }
    println!("sheets plugin allowlist: {allowed} of {} registry commands", specs.len());
    assert!(allowed > 20, "only {allowed} commands are allowed");
}
