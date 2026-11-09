//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! GridCraft in an `<iframe>` (such as a Nextcloud app or a document server). Messages are plain
//! objects with a `type`:
//!
//! - `gridcraft:ready` (to the parent): `{ version }`, the app is listening; send a file now.
//! - `gridcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. The workbook replaces the one shown. `readOnly: true` opens it read only:
//!   an edit is taken back at once (with a message) and nothing is ever sent as a save.
//!   `canDownload: false` withholds the exports (PDF, CSV, web page), `canPrint: false` Print;
//!   both default to true. Absent fields open it editable with both allowed.
//! - `gridcraft:opened` (to the parent): `{ name }`, the workbook is shown.
//! - `gridcraft:requestSave` (from the parent): save now, as File › Save does.
//! - `gridcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`): the
//!   workbook as .xlsx, from Save, Save As or `requestSave`. The workbook stays marked as unsaved
//!   (no `dirty: false`) until the parent has answered every save with `gridcraft:saved`. Other
//!   files (Export as PDF, CSV) are downloaded by the browser, not sent.
//! - `gridcraft:autosave` (to the parent): `{ name, bytes }`, the same .xlsx, `AUTOSAVE_IDLE_MS` (3 s) after the last edit while the
//!   workbook has unsaved changes and at least every `AUTOSAVE_MS` (30 s) during continuous editing, none when it is clean, read only, or a save is unanswered. It is no save:
//!   the workbook stays unsaved and nothing is downloaded; the parent keeps it as a recovery copy.
//! - `window.__hostFinalCopy()` (called by the parent, synchronously): the .xlsx `gridcraft:autosave` would send, a `Uint8Array`,
//!   when the workbook has changes not handed over yet; `null` otherwise. The parent calls it as it ends the session (its
//!   `pagehide` runs before this page's). It counts as a handover.
//! - `gridcraft:saved` (from the parent): `{ ok }`, the answer to one `gridcraft:save`, in order.
//!   `ok: false` marks the workbook unsaved again.
//! - `gridcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `gridcraft:failed` (to the parent): `{ error }` when the app couldn't start or open the file.
//!
//! Only the parent window on the page's own origin is heard or answered. In host mode the page
//! starts with an empty workbook instead of a sample, and the parent guards leaving the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;

/// Saving one of these hands the whole workbook to the parent, which stores it: the workbook
/// counts as saved under that name, as it would after Save on the desktop.
const WORKBOOK_EXTS: &[&str] = &["xlsx"];

/// The longest user name taken from `?author=`.
const MAX_AUTHOR_CHARS: usize = 200;

/// Shown when an edit of a read-only workbook was taken back, and when the host withholds a command.
const READ_ONLY: &str = "This workbook is open read-only: changes are not kept.";
const WITHHELD: &str = "This is not available in this session.";

/// The longest the unsaved workbook goes without an autosave to the parent while it is being edited.
const AUTOSAVE_MS: f64 = 30_000.0;
/// The quiet time after the last edit that brings the autosave (also the least time between two).
const AUTOSAVE_IDLE_MS: f64 = 3_000.0;

/// When the unsaved document is handed to the parent as a recovery copy: `AUTOSAVE_IDLE_MS` after the last edit, and at the
/// latest `AUTOSAVE_MS` after the first edit not yet handed over (continuous editing). The idle time is also the least time
/// between two copies. Nothing is sent for a document that has not changed since the last copy.
#[derive(Default)]
struct AutosaveClock {
    /// The edit signature seen at the last tick and the one last handed over (`None`: clean, or not counting).
    seen: Cell<Option<u64>>,
    sent: Cell<Option<u64>>,
    last_edit: Cell<f64>,
    first_unsent: Cell<Option<f64>>,
}

impl AutosaveClock {
    /// `sig` changes with every edit and is `None` while there is nothing to hand over (clean, read only, a save unanswered).
    /// Returns the milliseconds until a copy is due (`<= 0`: due now, and counted as handed over), `None` when nothing is pending.
    fn tick(&self, sig: Option<u64>, now: f64) -> Option<f64> {
        let Some(sig) = sig else {
            self.seen.set(None);
            self.sent.set(None);
            self.first_unsent.set(None);
            return None;
        };
        if self.seen.replace(Some(sig)) != Some(sig) {
            self.last_edit.set(now);
            self.first_unsent.set(Some(self.first_unsent.get().unwrap_or(now)));
        }
        if self.sent.get() == Some(sig) {
            return None;
        }
        let due = (self.last_edit.get() + AUTOSAVE_IDLE_MS).min(self.first_unsent.get().unwrap_or(now) + AUTOSAVE_MS);
        if now >= due {
            self.sent.set(Some(sig));
            self.first_unsent.set(None);
        }
        Some(due - now)
    }
}

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// Names of the workbooks handed to the parent since the last frame.
    saved: Rc<RefCell<Vec<String>>>,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
    /// A file from the parent, opened at the next frame.
    to_open: Rc<RefCell<Option<(String, Vec<u8>)>>>,
    /// The name of the file the parent opened last.
    name: Rc<RefCell<String>>,
    /// `gridcraft:open { readOnly, canDownload, canPrint }`.
    read_only: Rc<Cell<bool>>,
    can_download: Rc<Cell<bool>>,
    can_print: Rc<Cell<bool>>,
    /// The parent asked for a save since the last frame.
    save_requested: Rc<Cell<bool>>,
    /// Saves sent that the parent hasn't answered with `gridcraft:saved` yet, and the answers since the last frame.
    pending: Rc<Cell<u32>>,
    acks: Rc<RefCell<Vec<bool>>>,
    /// A refusal to show at the next frame.
    refused: Rc<Cell<Option<&'static str>>>,
    /// When the next autosave is due.
    autosave_clock: Rc<AutosaveClock>,
}

impl Bridge {
    /// `None` when the page isn't framed (there is no parent to talk to).
    pub fn new() -> Option<Self> {
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = window.location().origin().ok()?;
        Some(Self {
            window,
            parent,
            origin,
            saved: Rc::default(),
            reported: Rc::default(),
            to_open: Rc::default(),
            name: Rc::new(RefCell::new("Book.xlsx".to_string())),
            read_only: Rc::default(),
            can_download: Rc::new(Cell::new(true)),
            can_print: Rc::new(Cell::new(true)),
            save_requested: Rc::default(),
            pending: Rc::default(),
            acks: Rc::default(),
            refused: Rc::default(),
            autosave_clock: Rc::default(),
        })
    }

    /// Send saved workbooks to the parent instead of downloading them, and hear files, save
    /// requests and save answers from it.
    pub fn connect(&self, services: &mut Services, ctx: egui::Context) {
        self.listen(ctx);
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
            if !is_workbook(name) {
                if !bridge.can_download.get() {
                    return bridge.refused.set(Some(WITHHELD));
                }
                if let Err(e) = crate::web::download(name, bytes) {
                    log::error!("download of {name} failed: {e}");
                }
            } else if bridge.read_only.get() {
                bridge.refused.set(Some(READ_ONLY));
            } else if let Err(e) = bridge.post_save(name, bytes) {
                log::error!("couldn't hand {name} to the page: {e}");
                bridge.post_failed(&e);
            }
        }));
        let bridge = self.clone();
        let print = services.print.take();
        services.print = Some(Box::new(move |bytes: &[u8]| {
            if !bridge.can_print.get() {
                bridge.refused.set(Some(WITHHELD));
            } else if let Some(print) = &print {
                print(bytes);
            }
        }));
    }

    /// `?author=`: the host's name for the user signs new notes and comments.
    pub fn set_author(app: &mut SheetApp, author: &str) {
        let author: String = author.trim().chars().take(MAX_AUTHOR_CHARS).collect();
        if !author.is_empty() {
            app.session.prefs.user_name = author;
        }
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "gridcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "gridcraft:failed".into()), ("error", error.into())]);
    }

    /// After each frame: open a file the parent sent, apply the save answers, run a save it asked
    /// for, take back edits of a read-only workbook, and report the unsaved-changes state.
    pub fn sync(&self, app: &mut SheetApp, ctx: &egui::Context) {
        // Read only is the engine's own state: it refuses every command that changes the workbook, and the editors do not open.
        app.session.read_only = self.read_only.get();
        if let Some((name, bytes)) = self.to_open.borrow_mut().take() {
            app.read_only_notice = app.ui.language.tr(READ_ONLY).to_string();
            self.open(app, name, &bytes);
        }
        if self.save_requested.replace(false) && !self.read_only.get() {
            let name = xlsx_name(&self.name.borrow());
            let sent = app
                .run("file.saveBytes", json!({"format": "xlsx"}))
                .and_then(|r| {
                    r.get("base64").and_then(|b| b.as_str()).and_then(gridcraft_engine::io::base64_decode).ok_or_else(|| "no bytes".to_string())
                })
                .and_then(|bytes| self.post_save(&name, &bytes));
            if let Err(e) = sent {
                self.post_failed(&e);
            }
        }
        self.autosave(app, ctx);
        // A workbook handed to the parent counts as saved under the name it was saved as.
        let saved = std::mem::take(&mut *self.saved.borrow_mut());
        if let Some(name) = saved.last()
            && let Some(doc) = app.session.active_mut()
        {
            doc.saved = doc.wb.clone();
            if doc.path.is_none() {
                doc.title = name.clone();
            }
        }
        for ok in self.acks.take() {
            if self.pending.get() == 0 {
                break;
            }
            self.pending.set(self.pending.get() - 1);
            // Not stored by the parent: unsaved again (a copy of the workbook is another `Arc`).
            if !ok && let Some(doc) = app.session.active_mut() {
                doc.saved = Arc::new((*doc.wb).clone());
            }
        }
        // Read only: the engine and the editors refuse edits before they happen; this takes back whatever got through anyway.
        if self.read_only.get()
            && let Some(doc) = app.session.active_mut()
            && doc.is_dirty()
        {
            log::warn!("read-only safety net: the workbook changed and was taken back");
            doc.wb = doc.saved.clone();
            doc.calc.rebuild(&doc.wb);
            doc.undo.clear();
            doc.redo.clear();
            doc.revision += 1;
            self.refused.set(Some(READ_ONLY));
        }
        if let Some(why) = self.refused.take() {
            app.message = Some((gridcraft_ui_egui::office_overlay::app_name().into(), app.ui.language.tr(why).to_string()));
        }
        let dirty = app.session.documents().iter().any(|d| d.is_dirty());
        // A saved workbook stays unsaved until the parent has stored it.
        if (dirty || self.pending.get() == 0)
            && self.reported.get() != Some(dirty)
            && self.send(&[("type", "gridcraft:dirty".into()), ("dirty", dirty.into())])
        {
            self.reported.set(Some(dirty));
        }
    }

    /// The recovery copy: only while unsaved, editable and no save is unanswered. Not `file.save`: the workbook stays unsaved.
    fn autosave(&self, app: &mut SheetApp, ctx: &egui::Context) {
        match self.autosave_clock.tick(self.autosave_sig(app), js_sys::Date::now()) {
            None => return,
            // An idle page draws no frames: wake up when the autosave is due.
            Some(wait) if wait > 0.0 => return ctx.request_repaint_after(std::time::Duration::from_millis(wait as u64 + 1)),
            Some(_) => {}
        }
        let sent = xlsx_bytes(app).and_then(|bytes| self.post("gridcraft:autosave", &xlsx_name(&self.name.borrow()), &bytes));
        if let Err(e) = sent {
            log::error!("autosave failed: {e}");
        }
    }

    /// The edit signature of a workbook a recovery copy is due for (unsaved, editable, no save unanswered), else `None`.
    fn autosave_sig(&self, app: &SheetApp) -> Option<u64> {
        app.session
            .active()
            .filter(|d| d.is_dirty() && !self.read_only.get() && self.pending.get() == 0)
            .map(|d| (Arc::as_ptr(&d.wb) as usize as u64).wrapping_add(d.revision))
    }

    /// The parent's synchronous pull as it ends the session (`window.__hostFinalCopy`, see `expose_final_copy`): the workbook
    /// as `gridcraft:autosave` sends it when it has changes not handed over yet, else `None` (clean, read only, a save
    /// unanswered, or the last autosave holds them). It counts as a handover, so the autosave doesn't send the same state again.
    pub fn final_copy(&self, app: &mut SheetApp) -> Option<Vec<u8>> {
        let sig = self.autosave_sig(app)?;
        if self.autosave_clock.sent.replace(Some(sig)) == Some(sig) {
            return None;
        }
        self.autosave_clock.first_unsent.set(None);
        xlsx_bytes(app).map_err(|e| log::error!("couldn't hand the workbook to the page as it closed: {e}")).ok()
    }

    /// `window.__hostFinalCopy()` for the parent: `pull`'s bytes as a `Uint8Array`, or `null`. The parent calls it in its own
    /// `pagehide` or close, which runs before this page's: a `postMessage` from here would reach a parent already gone.
    pub fn expose_final_copy(&self, pull: impl Fn() -> Option<Vec<u8>> + 'static) {
        let f = Closure::<dyn Fn() -> JsValue>::new(move || pull().map_or(JsValue::NULL, |b| js_sys::Uint8Array::from(b.as_slice()).into()));
        if js_sys::Reflect::set(&self.window, &JsValue::from_str("__hostFinalCopy"), f.as_ref()).is_ok() {
            // It lives as long as the page.
            f.forget();
        }
    }

    /// The opened file replaces every workbook shown.
    fn open(&self, app: &mut SheetApp, name: String, bytes: &[u8]) {
        let before = app.session.documents().len();
        let b64 = gridcraft_engine::io::base64_encode(bytes);
        if let Err(e) = app.run("file.open", json!({"name": &name, "base64": b64})) {
            return self.post_failed(&e);
        }
        let shown = app.session.active_index();
        for i in (0..before).rev().filter(|&i| i != shown) {
            app.session.close_document(i);
        }
        *self.name.borrow_mut() = name.clone();
        self.send(&[("type", "gridcraft:opened".into()), ("name", name.as_str().into())]);
    }

    fn post_save(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        self.post("gridcraft:save", &name, bytes)?;
        self.saved.borrow_mut().push(name);
        self.pending.set(self.pending.get() + 1);
        Ok(())
    }

    /// `{ type, name, bytes }` to the parent, the bytes transferred.
    fn post(&self, kind: &str, name: &str, bytes: &[u8]) -> Result<(), String> {
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[("type", kind.into()), ("name", name.into()), ("bytes", buffer.clone().into())])
            .ok_or_else(|| "couldn't build the message".to_string())?;
        self.parent
            .post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer))
            .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))
    }

    fn listen(&self, ctx: egui::Context) {
        let bridge = self.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != bridge.origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &bridge.parent)) {
                return;
            }
            let data = event.data();
            match string(&data, "type").as_deref() {
                Some("gridcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Book.xlsx".to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    bridge.read_only.set(flag(&data, "readOnly", false));
                    bridge.can_download.set(flag(&data, "canDownload", true));
                    bridge.can_print.set(flag(&data, "canPrint", true));
                    *bridge.to_open.borrow_mut() = Some((name, bytes));
                }
                Some("gridcraft:requestSave") => bridge.save_requested.set(true),
                Some("gridcraft:saved") => bridge.acks.borrow_mut().push(flag(&data, "ok", false)),
                _ => return,
            }
            ctx.request_repaint();
        });
        if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// Post a message to the parent; false if it couldn't be sent.
    fn send(&self, fields: &[(&str, JsValue)]) -> bool {
        message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
    }
}

fn is_workbook(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| WORKBOOK_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

/// The workbook as .xlsx, as Save and the autosave send it (`file.saveBytes`: the workbook stays unsaved).
fn xlsx_bytes(app: &mut SheetApp) -> Result<Vec<u8>, String> {
    app.run("file.saveBytes", json!({"format": "xlsx"}))
        .and_then(|r| r.get("base64").and_then(|b| b.as_str()).and_then(gridcraft_engine::io::base64_decode).ok_or_else(|| "no bytes".to_string()))
}

/// `report.xls` and `report.csv` are saved as `report.xlsx`.
fn xlsx_name(opened: &str) -> String {
    let base = std::path::Path::new(opened).file_stem().map_or_else(|| "Book".to_string(), |s| s.to_string_lossy().to_string());
    format!("{base}.xlsx")
}

fn message(fields: &[(&str, JsValue)]) -> Option<js_sys::Object> {
    let m = js_sys::Object::new();
    for (key, value) in fields {
        js_sys::Reflect::set(&m, &JsValue::from_str(key), value).ok()?;
    }
    Some(m)
}

fn string(data: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?.as_string()
}

/// A boolean field, `default` when absent or not a boolean.
fn flag(data: &JsValue, key: &str, default: bool) -> bool {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok().and_then(|v| v.as_bool()).unwrap_or(default)
}

/// `bytes` as an `ArrayBuffer` or `Uint8Array`.
fn bytes(data: &JsValue) -> Option<Vec<u8>> {
    let bytes = js_sys::Reflect::get(data, &JsValue::from_str("bytes")).ok()?;
    if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
        Some(array.to_vec())
    } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
        Some(js_sys::Uint8Array::new(&bytes).to_vec())
    } else {
        None
    }
}
