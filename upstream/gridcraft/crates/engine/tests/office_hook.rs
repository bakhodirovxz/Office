// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The command hook (patch 0141): a refusing hook disables print, the after-hook sees `file.save`, and the guarded ids all exist.
//! A hook is process-wide and cannot be removed, so this file is its own test binary; without a hook every other suite runs as before.
use std::sync::{Mutex, Once};

use gridcraft_engine::office_hook::{self, Hook};
use gridcraft_engine::{EngineError, Result, Session, command_specs};
use serde_json::{Value, json};

static SEEN: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

struct Policy;

impl Hook for Policy {
    fn before(&self, _: &Session, id: &str, _: &Value) -> std::result::Result<(), String> {
        if id == "file.print" { Err("printing is off by policy".into()) } else { Ok(()) }
    }
    fn after(&self, _: &Session, id: &str, _: &Value, result: &Result<Value>) {
        SEEN.lock().expect("log").push((id.to_string(), result.is_ok()));
    }
}

fn session() -> Session {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| assert!(office_hook::install(Box::new(Policy))));
    let mut s = Session::new();
    s.new_workbook();
    s
}

#[test]
fn hook_refuses_print() {
    let mut s = session();
    match s.execute("file.print", json!({})) {
        Err(EngineError::Disabled(id, why)) => assert_eq!((id.as_str(), why.as_str()), ("file.print", "printing is off by policy")),
        other => panic!("print was not refused: {other:?}"),
    }
    // a refused command never ran, so the after-hook did not see it; other commands are untouched
    assert!(!SEEN.lock().expect("log").iter().any(|(id, _)| id == "file.print"));
    assert!(s.execute("edit.copy", json!({"range": "A1"})).is_ok());
    assert!(!office_hook::install(Box::new(Policy)), "a second hook is refused");
}

#[test]
fn hook_sees_save() {
    let mut s = session();
    let path = std::env::temp_dir().join(format!("office-hook-{}.xlsx", std::process::id()));
    s.execute("file.saveAs", json!({"path": path})).expect("save as");
    s.execute("file.save", json!({})).expect("save");
    let _ = std::fs::remove_file(&path);
    let seen = SEEN.lock().expect("log");
    assert!(seen.contains(&("file.save".to_string(), true)), "after-hook missed file.save: {seen:?}");
}

/// The ids the policy and the snapshot hook guard (PLAN section 4) must exist in this registry, or a sync that renames one fails here.
#[test]
fn guarded_ids_exist() {
    let ids: Vec<&str> = command_specs().iter().map(|c| c.id).collect();
    for id in ["file.print", "file.exportPdf", "file.exportCsv", "file.exportHtml", "file.saveAs", "file.save", "file.saveBytes", "edit.copy", "edit.cut"] {
        assert!(ids.contains(&id), "{id} is not registered");
    }
    assert!(ids.iter().any(|id| id.starts_with("file.print")) && ids.iter().any(|id| id.starts_with("file.export")));
}
