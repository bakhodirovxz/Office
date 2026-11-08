// Added by the office command-hook patch. MIT OR Apache-2.0.
//! The command-hook seam: a refusing hook stops a command, the after-hook sees a save, and with no hook (or one that allows
//! everything) a command's result is byte-identical. The hook is process-wide, so the tests take turns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use deckcraft_engine::office_hook::{self, CommandHook};
use deckcraft_engine::{Result, Session};
use serde_json::{Value, json};

static TURN: Mutex<()> = Mutex::new(());

struct Recorder {
    refuse: &'static [&'static str],
    seen: Mutex<Vec<(String, bool)>>,
}

impl CommandHook for Recorder {
    fn before(&self, id: &str, _: &Session) -> Option<String> {
        self.refuse.contains(&id).then(|| "refused by policy".to_string())
    }
    fn after(&self, id: &str, _: &Session, _: &Value, result: &Result<Value>) {
        self.seen.lock().unwrap().push((id.to_string(), result.is_ok()));
    }
}

fn recorder(refuse: &'static [&'static str]) -> Arc<Recorder> {
    Arc::new(Recorder { refuse, seen: Mutex::default() })
}

fn temp(name: &str) -> String {
    std::env::temp_dir().join(format!("deckcraft-hook-{}-{name}", std::process::id())).to_string_lossy().to_string()
}

#[test]
fn hook_refuses_print() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let hook = recorder(&["file.export"]);
    office_hook::set_hook(Some(hook.clone()));
    let mut s = Session::with_new();
    let path = temp("print.pdf");
    let err = s.execute("file.export", &json!({"path": path, "format": "pdf"})).unwrap_err();
    assert!(err.to_string().contains("refused by policy"), "{err}");
    assert!(!std::path::Path::new(&path).exists(), "the refused export wrote a file");
    assert_eq!(office_hook::refusal("file.export", &s).as_deref(), Some("refused by policy"));
    // Other commands run, and the hook saw them.
    s.execute("slide.new", &json!({})).unwrap();
    assert!(hook.seen.lock().unwrap().iter().any(|(id, ok)| id == "slide.new" && *ok));
    office_hook::set_hook(None);
    assert!(office_hook::refusal("file.export", &s).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn hook_sees_save() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let hook = recorder(&[]);
    office_hook::set_hook(Some(hook.clone()));
    let mut s = Session::with_new();
    let path = temp("save.pptx");
    s.execute("file.save", &json!({"path": path})).unwrap();
    office_hook::set_hook(None);
    assert!(hook.seen.lock().unwrap().iter().any(|(id, ok)| id == "file.save" && *ok), "{:?}", hook.seen.lock().unwrap());
    assert!(std::path::Path::new(&path).exists());
    let _ = std::fs::remove_file(path);
}

#[test]
fn without_a_hook_nothing_changes() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let bytes = |s: &mut Session| s.execute("file.saveBytes", &json!({"format": "pptx"})).unwrap();
    let mut s = Session::with_new();
    s.execute("insert.textBox", &json!({"text": "Salom"})).ok();
    office_hook::set_hook(None);
    let plain = bytes(&mut s);
    office_hook::set_hook(Some(recorder(&[])));
    let hooked = bytes(&mut s);
    office_hook::set_hook(None);
    assert_eq!(plain, hooked);
}

#[test]
fn hook_refuses_a_pointer_gesture_that_edits_without_a_command() {
    use deckcraft_engine::{Mods, PointerEvent, PointerKind};
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let ev = |kind, x, y| PointerEvent { kind, x, y, mods: Mods::default(), tol: 4.0, pressure: None };
    let fresh = || {
        let mut s = Session::with_new();
        s.execute("shape.insert", &json!({"preset": "rect", "rect": [100.0, 100.0, 60.0, 40.0]})).unwrap();
        s.select(|_, sel| sel.shapes.clear()).unwrap();
        s
    };
    // Without the policy the drag moves the shape (so the check below is not vacuous).
    let mut free = fresh();
    let was = free.active().unwrap().doc.clone();
    free.pointer(ev(PointerKind::Down, 120.0, 120.0)).unwrap();
    free.pointer(ev(PointerKind::Drag, 220.0, 220.0)).unwrap();
    free.pointer(ev(PointerKind::Up, 220.0, 220.0)).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&was, &free.active().unwrap().doc), "the control drag changed nothing");
    let mut s = fresh();
    let before = s.active().unwrap().doc.clone();
    office_hook::set_hook(Some(recorder(&[office_hook::EDIT])));
    // Press on the shape and drag it: selecting is allowed, moving is not, and the document never changes.
    s.pointer(ev(PointerKind::Down, 120.0, 120.0)).unwrap();
    assert!(s.pointer(ev(PointerKind::Drag, 220.0, 220.0)).is_err());
    s.pointer(ev(PointerKind::Up, 220.0, 220.0)).unwrap();
    assert!(std::sync::Arc::ptr_eq(&before, &s.active().unwrap().doc), "the drag moved the shape");
    // A drawing tool is refused from the press.
    s.set_tool(deckcraft_engine::ToolKind::TextBox);
    assert!(s.pointer(ev(PointerKind::Down, 10.0, 10.0)).is_err());
    assert!(s.tool.preview.is_none());
    office_hook::set_hook(None);
}
