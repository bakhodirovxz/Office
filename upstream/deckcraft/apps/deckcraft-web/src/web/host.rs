//! The `postMessage` bridge to a parent page (`?host=parent`), for pages that embed DeckCraft in
//! an `<iframe>` (the web shell, a document server). The messages are WordCraft's, with the
//! `deckcraft:` prefix:
//!
//! - `deckcraft:ready` (to the parent): `{ version }`, the app is listening; send a file now.
//! - `deckcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. The presentation replaces the one shown. `readOnly: true` opens it read only:
//!   every change is refused before it reaches the presentation (typing, paste, drags, commands; a message in the status bar;
//!   the per-frame revert stays as a last resort) and nothing is ever sent as a save.
//!   `canDownload: false` withholds the exports (PDF, pictures, outline), `canPrint: false` Print;
//!   both default to true.
//! - `deckcraft:opened` (to the parent): `{ name }`, it is shown.
//! - `deckcraft:requestSave` (from the parent): save now.
//! - `deckcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`): the
//!   presentation, always as a `.pptx`. It stays marked as unsaved (no `dirty: false`) until the
//!   parent has answered every save with `deckcraft:saved`. Exports and the PDF a print makes
//!   are downloaded by the browser, not sent.
//! - `deckcraft:saved` (from the parent): `{ ok }`, the answer to one `deckcraft:save`, in order.
//!   `ok: false` marks the presentation unsaved again.
//! - `deckcraft:autosave` (to the parent): `{ name, bytes }`, the same `.pptx` a save sends, `AUTOSAVE_IDLE_MS` (3 s) after the last edit while the
//!   presentation has unsaved changes, and at least every `AUTOSAVE_MS` (30 s) during continuous editing, so the parent holds the
//!   work if the page is lost. It is not a save: the presentation
//!   stays unsaved, and nothing is sent when it is clean, read only, or has a save the parent hasn't answered.
//! - `deckcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `deckcraft:failed` (to the parent): `{ error }` when the app couldn't start, open or save.
//!
//! Only the parent window is heard or answered, and only on its origin: the page's own unless
//! `&parent_origin=` names another. In host mode the page starts with a blank presentation.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use deckcraft_ui_egui::{Services, SlideApp};
use serde_json::json;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;

/// The longest the unsaved presentation goes without a recovery copy for the parent while it is being edited.
const AUTOSAVE_MS: f64 = 30_000.0;
/// The quiet time after the last edit that brings the recovery copy (also the least time between two copies).
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

/// The name a presentation gets when the parent opened none.
const DEFAULT_NAME: &str = "presentation.pptx";

/// Shown when an edit of a read-only presentation was taken back, and when the host withholds a command.
const READ_ONLY: &str = "This presentation is open read-only: changes are not kept.";
const WITHHELD: &str = "This is not available in this session.";

/// `deckcraft:open { canDownload, canPrint }`, read by the command hook (the app asks it before an export or a print).
static CAN_DOWNLOAD: AtomicBool = AtomicBool::new(true);
static CAN_PRINT: AtomicBool = AtomicBool::new(true);
/// Set once a read-only presentation is open: the hook then refuses every change before it reaches the presentation.
static LOCKED: AtomicBool = AtomicBool::new(false);

/// Refuses Export and Print when the host withholds them, and every change while the presentation is read-only; the app shows
/// the reason and greys the commands out.
struct HostPolicy;

impl deckcraft_engine::office_hook::CommandHook for HostPolicy {
    fn before(&self, id: &str, _s: &deckcraft_engine::Session) -> Option<String> {
        if LOCKED.load(Ordering::Relaxed) && changes_or_replaces(id) {
            return Some(deckcraft_ui_egui::i18n::t(READ_ONLY).to_string());
        }
        let withheld = match id {
            "file.export" => !CAN_DOWNLOAD.load(Ordering::Relaxed),
            "file.print" => !CAN_PRINT.load(Ordering::Relaxed),
            _ => false,
        };
        withheld.then(|| deckcraft_ui_egui::i18n::t(WITHHELD).to_string())
    }
}

/// A command that changes the presentation (every undoable one, a text composition, Undo/Redo), a gesture that edits it without a
/// command, or one that swaps it for another file. Selecting, scrolling, zooming, Copy and the like stay allowed.
fn changes_or_replaces(id: &str) -> bool {
    use deckcraft_engine::office_hook::EDIT;
    matches!(
        id,
        EDIT | "edit.undo"
            | "edit.redo"
            | "text.compose"
            | "text.commit"
            | "text.autocorrect"
            | "file.new"
            | "file.open"
            | "file.openBytes"
            | "file.adopt"
            | "file.revert"
            | "file.recovery.open"
    ) || deckcraft_engine::cmd::find_command(id).is_some_and(|c| c.undoable)
}

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// The parent asked for a save (or the app tried to download a `.deckcraft`) since the last frame.
    save_requested: Rc<Cell<bool>>,
    /// A file from the parent, opened at the next frame.
    to_open: Rc<RefCell<Option<(String, Vec<u8>)>>>,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
    /// The name of the file the parent opened last.
    name: Rc<RefCell<String>>,
    /// `deckcraft:open { readOnly }`.
    read_only: Rc<Cell<bool>>,
    /// Saves sent that the parent hasn't answered with `deckcraft:saved` yet, and the answers since the last frame.
    pending: Rc<Cell<u32>>,
    acks: Rc<RefCell<Vec<bool>>>,
    /// When the next recovery copy is due.
    autosave_clock: Rc<AutosaveClock>,
    /// A refusal to show at the next frame.
    refused: Rc<Cell<Option<&'static str>>>,
}

impl Bridge {
    /// `None` when the page isn't framed (there is no parent to talk to).
    pub fn new(parent_origin: Option<String>) -> Option<Self> {
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = match parent_origin.filter(|o| !o.is_empty()) {
            Some(origin) => origin,
            None => window.location().origin().ok()?,
        };
        Some(Self {
            window,
            parent,
            origin,
            save_requested: Rc::default(),
            to_open: Rc::default(),
            reported: Rc::default(),
            name: Rc::new(RefCell::new(DEFAULT_NAME.to_string())),
            read_only: Rc::default(),
            pending: Rc::default(),
            acks: Rc::default(),
            autosave_clock: Rc::default(),
            refused: Rc::default(),
        })
    }

    /// Send saved files to the parent instead of downloading them, and hear files, save requests
    /// and save answers from it.
    pub fn connect(&self, services: &mut Services, ctx: egui::Context) {
        self.listen(ctx.clone());
        deckcraft_engine::office_hook::set_hook(Some(Arc::new(HostPolicy)));
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
            let lower = name.to_ascii_lowercase();
            // The web app's own Save of an unnamed presentation downloads the native format; the
            // parent wants a .pptx, so the next frame saves that instead.
            if lower.ends_with(".deckcraft") || lower.ends_with(".pptx") {
                bridge.save_requested.set(true);
                ctx.request_repaint();
            } else if let Err(e) = super::download(name, bytes) {
                log::error!("download of {name} failed: {e}");
            }
        }));
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "deckcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "deckcraft:failed".into()), ("error", error.into())]);
    }

    /// After each frame: open a file the parent sent, run a save it asked for, apply the save
    /// answers, take back edits of a read-only presentation, and report the unsaved-changes state.
    pub fn sync(&self, ctx: &egui::Context, app: &mut SlideApp) {
        if let Some((name, bytes)) = self.to_open.borrow_mut().take() {
            self.open(app, name, &bytes);
        }
        if self.save_requested.replace(false) {
            if self.read_only.get() {
                self.refused.set(Some(READ_ONLY));
            } else {
                self.save(app);
            }
            ctx.request_repaint();
        }
        for ok in self.acks.take() {
            if self.pending.get() == 0 {
                break;
            }
            self.pending.set(self.pending.get() - 1);
            // Not stored by the parent: unsaved again (a copy of the presentation is another `Arc`).
            if !ok && let Some(d) = app.session.active_mut() {
                d.saved_doc = Arc::new((*d.doc).clone());
            }
        }
        // Read only, last resort: the command hook refuses changes before they happen; whatever still got through is taken back.
        if self.read_only.get()
            && let Some(d) = app.session.active_mut()
            && d.is_dirty()
        {
            log::warn!("read-only safety net: an edit reached the presentation and was taken back");
            d.doc = d.saved_doc.clone();
            d.history.undo.clear();
            d.history.redo.clear();
            let slide = d.selection.slide.min(d.doc.slides.len().saturating_sub(1));
            d.selection = deckcraft_engine::Selection { slide, ..Default::default() };
            d.interaction = None;
            d.composition = None;
            d.revision += 1;
            self.refused.set(Some(READ_ONLY));
        }
        // The recovery copy: only while there are unsaved changes, never read only, never with a save unanswered.
        let sig = app
            .session
            .active()
            .filter(|d| d.is_dirty() && !self.read_only.get() && self.pending.get() == 0)
            .map(|d| Arc::as_ptr(&d.doc) as usize as u64);
        if let Some(wait) = self.autosave_clock.tick(sig, js_sys::Date::now()) {
            if wait <= 0.0 {
                self.autosave(app);
            } else {
                // An idle page draws no frames: wake up when the copy is due.
                ctx.request_repaint_after(std::time::Duration::from_millis(wait as u64 + 1));
            }
        }
        if let Some(why) = self.refused.take() {
            app.notify(deckcraft_ui_egui::i18n::t(why).to_string());
        }
        let dirty = app.session.documents().iter().any(|d| d.is_dirty());
        // A saved presentation stays unsaved until the parent has stored it.
        if (dirty || self.pending.get() == 0)
            && self.reported.get() != Some(dirty)
            && self.send(&[("type", "deckcraft:dirty".into()), ("dirty", dirty.into())])
        {
            self.reported.set(Some(dirty));
        }
    }

    /// The opened file replaces whatever was shown.
    fn open(&self, app: &mut SlideApp, name: String, bytes: &[u8]) {
        let before = app.session.documents().len();
        let data = deckcraft_engine::cmd::base64_encode(bytes);
        LOCKED.store(false, Ordering::Relaxed);
        if let Err(e) = app.run("file.openBytes", json!({"name": name, "data": data})) {
            return self.post_failed(&e);
        }
        LOCKED.store(self.read_only.get(), Ordering::Relaxed);
        for _ in 0..before {
            app.session.close_document(0);
        }
        *self.name.borrow_mut() = name.clone();
        self.send(&[("type", "deckcraft:opened".into()), ("name", name.as_str().into())]);
    }

    /// Save the shown presentation as a `.pptx` named after the file the parent opened.
    fn save(&self, app: &mut SlideApp) {
        let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
        let name = pptx_name(&self.name.borrow());
        let sent = deckcraft_engine::cmd::file::save_bytes(&doc, "pptx").map_err(|e| e.to_string()).and_then(|bytes| self.post_save(&name, &bytes));
        match sent {
            Ok(()) => {
                self.pending.set(self.pending.get() + 1);
                // Clean now (the parent is told once it has stored it).
                if let Some(d) = app.session.active_mut() {
                    d.saved_doc = d.doc.clone();
                    d.revision += 1;
                }
            }
            Err(e) => self.post_failed(&e),
        }
    }

    /// Hand the shown presentation to the parent as a recovery copy: it stays unsaved and counts as no pending save.
    fn autosave(&self, app: &SlideApp) {
        let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
        let name = pptx_name(&self.name.borrow());
        let sent = deckcraft_engine::cmd::file::save_bytes(&doc, "pptx")
            .map_err(|e| e.to_string())
            .and_then(|bytes| self.post_file("deckcraft:autosave", &name, &bytes));
        if let Err(e) = sent {
            log::error!("couldn't hand {name} to the page for safekeeping: {e}");
        }
    }

    fn post_save(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.post_file("deckcraft:save", path, bytes)
    }

    fn post_file(&self, kind: &str, path: &str, bytes: &[u8]) -> Result<(), String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[("type", kind.into()), ("name", name.as_str().into()), ("bytes", buffer.clone().into())])
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
                Some("deckcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| DEFAULT_NAME.to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    bridge.read_only.set(flag(&data, "readOnly", false));
                    CAN_DOWNLOAD.store(flag(&data, "canDownload", true), Ordering::Relaxed);
                    CAN_PRINT.store(flag(&data, "canPrint", true), Ordering::Relaxed);
                    *bridge.to_open.borrow_mut() = Some((name, bytes));
                }
                Some("deckcraft:requestSave") => bridge.save_requested.set(true),
                Some("deckcraft:saved") => bridge.acks.borrow_mut().push(flag(&data, "ok", false)),
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

/// `report.ppt` and `report.odp` are saved as `report.pptx`.
fn pptx_name(opened: &str) -> String {
    let base = std::path::Path::new(opened).file_stem().map_or_else(|| "presentation".to_string(), |s| s.to_string_lossy().to_string());
    format!("{base}.pptx")
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
