//! The Slides adapter of the plugin system (Phase 7 WP13; docs/phase7/PLAN.md WP13, docs/plugins.md section 6a).
//!
//! The app-neutral part (slots, executor, web bridge) is `plugin-adapter`; this crate is the deckcraft side of its [`Editor`]:
//!
//! - the text map is the built-in transliteration's walk (`deckcraft_engine::cmd::translit::{text_scope, text_pieces,
//!   write_pieces}`, patch deckcraft/0157): shapes, groups, table cells and speaker notes of the slide or the deck, or the selected
//!   shapes or text range; one paragraph per text paragraph, one segment per piece (a run, or the part of one inside or outside the
//!   selected range). A field, break or equation run is an object segment, text outside the range is marked outside the selection;
//!   language tags travel with the segment. The apply re-reads the pieces, checks them against the read and writes through the
//!   built-in's own write (`normalize`, the selection kept on the same text), so a plugin's result equals the built-in's;
//! - the undo step: a slot command is undoable in the engine (`register_command`), and commands run by a command are not steps of
//!   their own, so natively the slot's `Session::execute` is the one step; on the web the invocation's snapshot is folded into one;
//! - a presentation has no protection, so nothing is refused for it;
//! - the revision is the content's: the engine's own revision also moves for a selection change, which must not make a read stale.
use std::cell::RefCell;
use std::sync::Arc;

use deckcraft_engine::cmd::translit::{Piece, TextScope, text_pieces, text_scope, write_pieces};
use deckcraft_engine::model::Presentation;
use deckcraft_engine::{EngineError, History, HistoryEntry, Selection, Session, register_command};
use plugin_adapter::plugin_api::textmap::{Apply, EDITABLE, OBJECT, OUTSIDE_SELECTION, Paragraph, Segment, TextMap};
use plugin_adapter::{Editor, PARAMS, SLOTS, bridge};
use serde_json::Value;

/// The active session as the executor sees it.
pub struct Ed<'a>(pub &'a mut Session);

pub struct Snap {
    uid: u64,
    doc: Arc<Presentation>,
    selection: Selection,
    history: History,
    revision: u64,
}

thread_local! {
    /// (the document last seen, its content revision). The `Arc` is held, so its address cannot be reused by another document.
    static SEEN: RefCell<Option<(Arc<Presentation>, u64)>> = const { RefCell::new(None) };
}

impl Ed<'_> {
    fn doc_mut(&mut self, uid: u64) -> Option<&mut deckcraft_engine::DocState> {
        self.0.active_mut().filter(|d| d.uid == uid)
    }
}

fn pieces_of(s: &Session, t: &TextScope) -> Result<Vec<Vec<Piece>>, String> {
    text_pieces(s, t).map_err(|e| e.to_string())
}

impl Editor for Ed<'_> {
    type Snap = Snap;
    type Map = TextScope;

    fn app(&self) -> &'static str {
        "slides"
    }
    fn doc_id(&self) -> Option<u64> {
        self.0.active().map(|d| d.uid)
    }
    fn rev(&self) -> u64 {
        let Some(d) = self.0.active() else { return 0 };
        SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            match &*seen {
                Some((doc, n)) if Arc::ptr_eq(doc, &d.doc) => *n,
                other => {
                    let n = other.as_ref().map_or(1, |(_, n)| n + 1);
                    *seen = Some((d.doc.clone(), n));
                    n
                }
            }
        })
    }
    fn snapshot(&self) -> Option<Snap> {
        let d = self.0.active()?;
        Some(Snap { uid: d.uid, doc: d.doc.clone(), selection: d.selection.clone(), history: d.history.clone(), revision: d.revision })
    }
    fn restore(&mut self, s: Snap) {
        let Some(d) = self.doc_mut(s.uid) else { return };
        d.revision = d.revision.max(s.revision) + 1;
        d.doc = s.doc;
        d.selection = s.selection;
        d.history = s.history;
    }
    fn commit(&mut self, s: &Snap, label: &str, in_command: bool) {
        // Natively the slot command's own `Session::execute` makes the one step (the commands it runs are not steps).
        if in_command {
            return;
        }
        let Some(d) = self.doc_mut(s.uid) else { return };
        if Arc::ptr_eq(&d.doc, &s.doc) {
            return;
        }
        let mut h = s.history.clone();
        h.costs.insert(Arc::as_ptr(&s.doc) as usize, deckcraft_engine::undo_cost(&s.doc, &d.doc));
        h.undo.push(HistoryEntry { label: label.to_string(), doc: s.doc.clone(), selection: s.selection.clone() });
        h.redo.clear();
        // shortcut: the byte budget of `push_undo` is not re-applied here (one step is added); the next engine edit applies it.
        if h.undo.len() > h.limit.max(1) {
            h.undo.remove(0);
        }
        d.history = h;
    }
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        self.0.execute(id, params).map_err(|e| e.to_string())
    }
    fn protected(&self) -> Option<&'static str> {
        None
    }
    fn read_map(&mut self, scope: &str) -> Result<(TextMap, TextScope), String> {
        let asked = match scope {
            "document" => "deck",
            o => o,
        };
        let t = text_scope(self.0, asked).map_err(|e| e.to_string())?;
        let mut map = TextMap::default();
        for (i, p) in pieces_of(self.0, &t)?.into_iter().enumerate() {
            let mut segments = Vec::with_capacity(p.len());
            for g in p {
                let flags = if g.object {
                    OBJECT
                } else if g.inside {
                    EDITABLE
                } else {
                    OUTSIDE_SELECTION
                };
                let lang = map.lang_index(g.lang.as_deref().unwrap_or("")).map_err(|e| e.to_string())?;
                segments.push(Segment { flags, lang, text: g.text });
            }
            map.paragraphs.push(Paragraph { index: i as u32, segments });
        }
        Ok((map, t))
    }
    fn apply_map(&mut self, map: &TextMap, at: &TextScope, ap: &Apply) -> Result<usize, String> {
        let live = pieces_of(self.0, at)?;
        if live.len() != map.paragraphs.len() {
            return Err("the slides changed since the text map was read".into());
        }
        for (k, (l, m)) in live.iter().zip(&map.paragraphs).enumerate() {
            if l.len() != m.segments.len() || l.iter().zip(&m.segments).any(|(g, s)| g.text != s.text || g.inside != s.editable()) {
                return Err(format!("paragraph {k} changed since the text map was read"));
            }
        }
        let (paragraphs, _) = write_pieces(self.0, at, &ap.paragraphs).map_err(|e| e.to_string())?;
        Ok(paragraphs)
    }
    fn selection_text(&self) -> String {
        let Ok(t) = text_scope(self.0, "selection") else { return String::new() };
        let Ok(p) = pieces_of(self.0, &t) else { return String::new() };
        p.iter().map(|p| p.iter().filter(|g| g.inside).map(|g| g.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }
}

fn slot_run<const N: usize>(s: &mut Session, v: &Value) -> deckcraft_engine::Result<Value> {
    let (p, c) = plugin_adapter::slot(N).ok_or_else(|| EngineError::Other("This plugin is not enabled".into()))?;
    plugin_adapter::exec::run_command(&mut Ed(s), &p, c, v).map_err(EngineError::Other)
}

macro_rules! slot_fns {
    ($($n:literal),*) => { [$(slot_run::<$n> as deckcraft_engine::cmd::Run),*] };
}

const TABLE: [deckcraft_engine::cmd::Run; SLOTS] = slot_fns!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
    37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63
);

/// Register the slot commands of plugins added since the last call (each id once per process). Safe to call again.
pub fn register() {
    for (n, id, label) in plugin_adapter::bind() {
        register_command(id, label, PARAMS, TABLE[n]);
    }
}

/// The web input lock: while a plugin call waits for the page, every command that changes the presentation is refused.
pub struct Lock;

impl deckcraft_engine::office_hook::CommandHook for Lock {
    fn before(&self, id: &str, _: &Session) -> Option<String> {
        let changes = deckcraft_engine::find_command(id).is_some_and(|c| c.undoable);
        bridge::refuses(id, changes).then(|| bridge::BUSY.to_string())
    }
}

/// Start-up, once per process: the app id, and natively the user's enabled plugins (loaded on a thread, registered when ready).
pub fn init() {
    plugin_adapter::set_app("slides");
    #[cfg(not(target_arch = "wasm32"))]
    plugin_adapter::native::init("DECKCRAFT_NO_PREFS", register);
}

/// Start-up of the web app: the app id. The input lock is `Lock`, which the host's command hook (deckcraft-web, patch 0160) asks: the hook slot holds one policy.
pub fn init_web() -> bool {
    plugin_adapter::set_app("slides");
    true
}

/// The toolbar's Plugins menu in the interface language `lang`: per plugin its name and its commands as (label, registry id).
pub fn menu(lang: &str) -> Vec<(String, Vec<(String, &'static str)>)> {
    plugin_adapter::set_lang(lang);
    plugin_adapter::menu(lang)
}

/// The message a finished plugin command leaves for the status line (the plugin's `ui.status`).
pub fn status_of(result: &Value) -> Option<String> {
    result.get("status").and_then(Value::as_str).map(str::to_string)
}

/// The web host's per-frame call (patch deckcraft/0158): plugin messages from the page, then what goes back.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use js_sys::Array;
    use wasm_bindgen::JsValue;

    pub use plugin_adapter::web::incoming;

    /// Is `data` a `deckcraft:plugin*` message?
    pub fn is_plugin_message(data: &JsValue) -> bool {
        plugin_adapter::web::is_plugin_message("deckcraft", data)
    }

    /// Returns (keep drawing frames, status lines for the status bar).
    pub fn sync(s: &mut deckcraft_engine::Session, post: &dyn Fn(&JsValue, &Array) -> bool) -> (bool, Vec<String>) {
        let r = plugin_adapter::web::sync(&mut super::Ed(s), "deckcraft", post);
        super::register();
        r
    }
}

#[cfg(test)]
mod tests;
