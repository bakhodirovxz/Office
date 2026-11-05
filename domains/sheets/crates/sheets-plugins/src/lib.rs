//! The Sheets adapter of the plugin system (Phase 7 WP13; docs/phase7/PLAN.md WP13, docs/plugins.md section 6a).
//!
//! The app-neutral part (slots, executor, web bridge) is `plugin-adapter`; this crate is the gridcraft side of its [`Editor`]:
//!
//! - the text map is the built-in transliteration's cell list (`gridcraft_engine::cmd::translit::text_cells`, patch
//!   gridcraft/0157): one paragraph per constant text cell in scope, one editable segment each (a cell holds one plain string);
//!   formulas, table headers, spill and picture cells are not in it. The apply checks every cell against the read and writes the
//!   changed ones through `write_texts`, the built-in's own write, so a plugin's result equals the built-in's (`tests/equiv.rs`);
//! - the undo step: plugin slot commands are not undoable in the engine (`register_command`), so the invocation's snapshot is the
//!   step: the commands it ran are folded into one entry labelled with the command;
//! - a workbook with a protected sheet or a protected structure is protected: a plugin that may write is refused on it;
//! - on the web, a command hook refuses every change while a call waits for the page (the input lock).
use std::sync::Arc;

use gridcraft_engine::cmd::translit::{text_cells, write_texts};
use gridcraft_engine::core::{CellRef, Value as CellValue};
use gridcraft_engine::model::Workbook;
use gridcraft_engine::{EngineError, HistoryEntry, Selection, Session, register_command};
use plugin_adapter::plugin_api::textmap::{Apply, EDITABLE, Paragraph, Segment, TextMap};
use plugin_adapter::{Editor, PARAMS, SLOTS, bridge};
use serde_json::{Value, json};

/// gridcraft's own undo depth (`UNDO_LIMIT`, private in the engine).
const UNDO_LIMIT: usize = 100;

/// The active session as the executor sees it.
pub struct Ed<'a>(pub &'a mut Session);

pub struct Snap {
    uid: u64,
    wb: Arc<Workbook>,
    selection: Selection,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    revision: u64,
}

impl Ed<'_> {
    fn doc_mut(&mut self, uid: u64) -> Option<&mut gridcraft_engine::DocState> {
        self.0.active_mut().filter(|d| d.uid == uid)
    }
}

impl Editor for Ed<'_> {
    type Snap = Snap;
    type Map = Vec<(usize, CellRef)>;

    fn app(&self) -> &'static str {
        "sheets"
    }
    fn doc_id(&self) -> Option<u64> {
        self.0.active().map(|d| d.uid)
    }
    fn rev(&self) -> u64 {
        self.0.active().map_or(0, |d| d.revision)
    }
    fn snapshot(&self) -> Option<Snap> {
        let d = self.0.active()?;
        Some(Snap { uid: d.uid, wb: d.wb.clone(), selection: d.selection.clone(), undo: d.undo.clone(), redo: d.redo.clone(), revision: d.revision })
    }
    fn restore(&mut self, s: Snap) {
        let Some(d) = self.doc_mut(s.uid) else { return };
        let changed = !Arc::ptr_eq(&d.wb, &s.wb);
        d.revision = d.revision.max(s.revision) + 1;
        d.wb = s.wb;
        d.selection = s.selection;
        d.undo = s.undo;
        d.redo = s.redo;
        if changed {
            // the edits recalculated in place: the calculation state follows the workbook put back (as `Session::execute` after a panic)
            d.calc.rebuild(&d.wb);
        }
    }
    fn commit(&mut self, s: &Snap, label: &str, _in_command: bool) {
        let Some(d) = self.doc_mut(s.uid) else { return };
        if Arc::ptr_eq(&d.wb, &s.wb) {
            return;
        }
        d.undo = s.undo.clone();
        d.undo.push(HistoryEntry { label: label.to_string(), wb: s.wb.clone(), selection: s.selection.clone() });
        if d.undo.len() > UNDO_LIMIT {
            d.undo.remove(0);
        }
        d.redo.clear();
        d.revision = d.revision.max(s.revision + 1);
    }
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        self.0.execute(id, params.clone()).map_err(|e| e.to_string())
    }
    fn protected(&self) -> Option<&'static str> {
        let wb = &self.0.active()?.wb;
        (wb.protected_structure || wb.sheets.iter().any(|sh| sh.is_protected())).then_some(bridge::PROTECTED)
    }
    fn read_map(&mut self, scope: &str) -> Result<(TextMap, Self::Map), String> {
        let (_, cells) = text_cells(self.0, &json!({}), scope).map_err(|e| e.to_string())?;
        let mut map = TextMap::default();
        let mut at = Vec::with_capacity(cells.len());
        for (i, (si, c, t)) in cells.into_iter().enumerate() {
            map.paragraphs.push(Paragraph { index: i as u32, segments: vec![Segment { flags: EDITABLE, lang: 0, text: t.to_string() }] });
            at.push((si, c));
        }
        Ok((map, at))
    }
    fn apply_map(&mut self, map: &TextMap, at: &Self::Map, ap: &Apply) -> Result<usize, String> {
        let d = self.0.active().ok_or("there is no document")?;
        let mut changes = Vec::new();
        for (k, &(si, c)) in at.iter().enumerate() {
            let seen = &map.paragraphs[k].segments[0].text;
            let live = d.wb.sheet(si).and_then(|sh| sh.cell(c)).filter(|cell| cell.formula.is_none()).map(|cell| &cell.value);
            if !matches!(live, Some(CellValue::Text(t)) if **t == **seen) {
                return Err(format!("cell {} changed since the text map was read", c.a1()));
            }
            if let Some(Some(new)) = ap.paragraphs.get(k).and_then(|p| p.first())
                && new != seen
            {
                changes.push((si, c, new.clone()));
            }
        }
        let n = changes.len();
        if n > 0 {
            write_texts(self.0, changes).map_err(|e| e.to_string())?;
            if let Some(d) = self.0.active_mut() {
                d.revision += 1;
            }
        }
        Ok(n)
    }
    fn selection_text(&self) -> String {
        match text_cells(self.0, &json!({}), "selection") {
            Ok((_, cells)) => cells.iter().map(|(_, _, t)| t.as_ref()).collect::<Vec<_>>().join("\n"),
            Err(_) => String::new(),
        }
    }
}

fn slot_run<const N: usize>(s: &mut Session, v: &Value) -> gridcraft_engine::Result<Value> {
    let (p, c) = plugin_adapter::slot(N).ok_or_else(|| EngineError::Other("This plugin is not enabled".into()))?;
    plugin_adapter::exec::run_command(&mut Ed(s), &p, c, v).map_err(EngineError::Other)
}

macro_rules! slot_fns {
    ($($n:literal),*) => { [$(slot_run::<$n> as gridcraft_engine::cmd::Run),*] };
}

const TABLE: [gridcraft_engine::cmd::Run; SLOTS] = slot_fns!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
    37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63
);

/// Register the slot commands of plugins added since the last call (each id once per process). Safe to call again.
pub fn register() {
    for (n, id, label) in plugin_adapter::bind() {
        register_command(id, label, PARAMS, TABLE[n]);
    }
}

/// The web input lock: while a plugin call waits for the page, every command that changes the workbook is refused.
struct Lock;

impl gridcraft_engine::office_hook::Hook for Lock {
    fn before(&self, _: &Session, id: &str, _: &Value) -> Result<(), String> {
        let changes = gridcraft_engine::find_command(id).is_some_and(|c| c.undoable);
        if bridge::refuses(id, changes) { Err(bridge::BUSY.into()) } else { Ok(()) }
    }
}

/// Start-up, once per process: the app id, and natively the user's enabled plugins (loaded on a thread, registered when ready).
pub fn init() {
    plugin_adapter::set_app("sheets");
    #[cfg(not(target_arch = "wasm32"))]
    plugin_adapter::native::init("GRIDCRAFT_NO_PREFS", register);
}

/// Start-up of the web app: the app id and the input lock. Returns false when another command hook holds the seam (then no plugin
/// is registered from the page, so none can run without the lock).
pub fn init_web() -> bool {
    plugin_adapter::set_app("sheets");
    gridcraft_engine::office_hook::install(Box::new(Lock))
}

/// The toolbar's Plugins menu in the interface language `lang` (`en`, `ru`, `uz-Latn`, `uz-Cyrl`): per plugin its name and its
/// commands as (label, registry id). Empty when no plugin is loaded.
pub fn menu(lang: &str) -> Vec<(String, Vec<(String, &'static str)>)> {
    plugin_adapter::set_lang(lang);
    plugin_adapter::menu(lang)
}

/// The message a finished plugin command leaves for the status line (the plugin's `ui.status`).
pub fn status_of(result: &Value) -> Option<String> {
    result.get("status").and_then(Value::as_str).map(str::to_string)
}

/// The web host's per-frame call (patch gridcraft/0158, `host.rs`): plugin messages from the page, then what goes back.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use js_sys::Array;
    use wasm_bindgen::JsValue;

    pub use plugin_adapter::web::incoming;

    /// Is `data` a `gridcraft:plugin*` message?
    pub fn is_plugin_message(data: &JsValue) -> bool {
        plugin_adapter::web::is_plugin_message("gridcraft", data)
    }

    /// Returns (keep drawing frames, status lines for the toast).
    pub fn sync(s: &mut gridcraft_engine::Session, post: &dyn Fn(&JsValue, &Array) -> bool) -> (bool, Vec<String>) {
        let r = plugin_adapter::web::sync(&mut super::Ed(s), "gridcraft", post);
        super::register();
        r
    }
}

#[cfg(test)]
mod tests;
