//! The `postMessage` bridge to a parent page (`?host=parent`), for pages that embed WordCraft in
//! an `<iframe>` (a document server, a Nextcloud app). Messages are plain objects with a `type`:
//!
//! - `wordcraft:ready` (to the parent): `{ version }`, the app is listening; send a document now.
//! - `wordcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a document picked with File › Open or dropped on the window.
//!   `readOnly: true` opens it read only: the document is held under Restrict Editing's read-only
//!   protection (typing, paste and every editing command are refused, and Stop Protection doesn't
//!   lift it), and Save, Save As and the exports are refused. Absent or false opens it editable.
//!   `canDownload: false` withholds Save As and the exports, `canPrint: false` Print (greyed out,
//!   and their keyboard shortcuts refused); both default to true. The browser's own print command
//!   can't be blocked from a page, so this only removes the editor's own entries.
//!   `dirty: true` (office overlay, wordcraft/0146) opens it with unsaved changes: the parent hands back a
//!   `wordcraft:autosave` it kept (the page died), so it stays marked unsaved and keeps being autosaved.
//! - `wordcraft:opened` (to the parent): `{ name }`, a document has replaced the one shown.
//! - `wordcraft:requestSave` (from the parent): save now, as File › Save does.
//! - `wordcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every
//!   file WordCraft would otherwise download: Save, Save As and the exports. The parent stores it
//!   and reports failures itself with `wordcraft:saved`: the document stays marked as unsaved (no
//!   `dirty: false`) until every file sent has been acknowledged.
//! - `wordcraft:saved` (from the parent): `{ ok }`, the answer to one `wordcraft:save`, in order.
//!   `ok: true` lets the unsaved mark go (if nothing was edited since); `ok: false` keeps it, and
//!   marks a document that was clean after the save as unsaved again.
//! - `wordcraft:autosave` (to the parent): `{ name, bytes }`, the document with its unsaved changes,
//!   sent when the tab is hidden or the page goes away, and every `AUTOSAVE_MS` while the document
//!   has unsaved changes, so the parent holds the work if the browser then kills the page. It is
//!   not a save: the document stays dirty, and nothing is sent for a clean or read-only document.
//! - `wordcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `wordcraft:commands` (from the parent): list the commands. The answer, `wordcraft:commands` (to the
//!   parent), is `{ commands: [{ id, label, enabled, shortcut }] }`: the commands that need no required parameter,
//!   `label` in the editor's language, `shortcut` as the platform writes it (empty: none). A host page
//!   builds an accessible command palette from it (the canvas has no DOM for a screen reader).
//! - `wordcraft:run` (from the parent): `{ id }`, run that command as a button would. The answer,
//!   `wordcraft:ran` (to the parent), is `{ id, ok, error }`.
//! - `wordcraft:focus` (from the parent): put keyboard focus back on the document.
//! - `wordcraft:focusOut` (to the parent): `{ palette }`, the user pressed F6 (leave the editor, `palette`
//!   false) or F1 (open the command palette, `palette` true); a canvas swallows Tab, so these are the
//!   keyboard way out.
//! - `wordcraft:failed` (to the parent): `{ error }` when the app couldn't start or save, or lost
//!   its WebGL context (`webglcontextlost`: the page can't be drawn again, reload it).
//!
//! Only the parent window is heard or answered, and only on its origin: the page's own unless
//! `&parent_origin=` names another. In host mode the page starts with an empty document and the
//! parent guards leaving the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::json;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wordcraft_ui_egui::{Inbox, Services, WordApp};

/// The name a document gets when the parent opened none.
const DEFAULT_NAME: &str = "document.docx";

/// How often the unsaved document is handed to the parent while it has unsaved changes
/// (ARCHITECTURE-DECISION 2.4 names no period; 30 s loses at most half a minute of typing and
/// costs one docx build per period, only while editing).
const AUTOSAVE_MS: f64 = 30_000.0;

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// The parent asked for a save since the last frame.
    save_requested: Rc<Cell<bool>>,
    /// The page was hidden or is going away since the last frame: hand over the unsaved document.
    autosave: Rc<Cell<bool>>,
    /// The parent opened the next document with unsaved changes (`wordcraft:open { dirty }`, wordcraft/0146).
    recovered: Rc<Cell<bool>>,
    /// The parent opened the document read only (`wordcraft:open { readOnly }`).
    read_only: Rc<Cell<bool>>,
    /// `wordcraft:open { canDownload }` and `{ canPrint }`: false withholds Save As and the exports, or Print.
    can_download: Rc<Cell<bool>>,
    can_print: Rc<Cell<bool>>,
    /// The parent asked for the command list since the last frame.
    list_requested: Rc<Cell<bool>>,
    /// Command ids the parent asked to run since the last frame.
    runs: Rc<RefCell<Vec<String>>>,
    /// Files sent as `wordcraft:save` that the parent hasn't answered with `wordcraft:saved` yet.
    pending: Rc<Cell<u32>>,
    /// Answers (`ok`) received since the last frame.
    acks: Rc<RefCell<Vec<bool>>>,
    /// When the unsaved document was last handed over by the periodic autosave (`None`: clean).
    last_autosave: Rc<Cell<Option<f64>>>,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
    /// The document shown when the parent was last told (`wordcraft:opened`).
    document: Rc<Cell<Option<u64>>>,
    /// The name of the document the parent opened last.
    name: Rc<RefCell<String>>,
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
            autosave: Rc::default(),
            recovered: Rc::default(),
            read_only: Rc::default(),
            can_download: Rc::new(Cell::new(true)),
            can_print: Rc::new(Cell::new(true)),
            list_requested: Rc::default(),
            runs: Rc::default(),
            pending: Rc::default(),
            acks: Rc::default(),
            last_autosave: Rc::default(),
            reported: Rc::default(),
            document: Rc::default(),
            name: Rc::new(RefCell::new(DEFAULT_NAME.to_string())),
        })
    }

    /// Send saved and exported files to the parent instead of downloading them, tell it when
    /// unsaved changes appear, and hear documents and save requests from it.
    pub fn connect(&self, services: &mut Services, inbox: Inbox, ctx: egui::Context) {
        self.listen_lifecycle(&ctx);
        self.listen_keys();
        self.listen(inbox, ctx);
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
            if bridge.read_only.get() {
                return Err("the document is open read only".to_string());
            }
            bridge.post_file("wordcraft:save", name, bytes)?;
            bridge.pending.set(bridge.pending.get() + 1);
            Ok(())
        }));
        let bridge = self.clone();
        services.blocked = Some(Box::new(move |id: &str| {
            (!bridge.can_download.get()
                && matches!(id, "file.saveAs" | "file.exportPdf" | "file.exportPng" | "backstage.saveAs" | "backstage.export"))
                || (!bridge.can_print.get()
                    && matches!(id, "file.print" | "file.printPdf" | "file.printMarkup" | "ui.print" | "ui.printSubmit" | "backstage.print"))
        }));
        // Keep the hook the web shell installed (the leave-page guard reads it).
        let bridge = self.clone();
        let previous = services.on_dirty.take();
        services.on_dirty = Some(Box::new(move |dirty| {
            if let Some(previous) = &previous {
                previous(dirty);
            }
            // A saved document stays unsaved until the parent has stored the file.
            if !dirty && bridge.pending.get() > 0 {
                return;
            }
            if bridge.reported.get() != Some(dirty) && bridge.send(&[("type", "wordcraft:dirty".into()), ("dirty", dirty.into())]) {
                bridge.reported.set(Some(dirty));
            }
        }));
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "wordcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "wordcraft:failed".into()), ("error", error.into())]);
    }

    /// After each frame: run a save the parent asked for, and tell it when another document is
    /// shown (office overlay, wordcraft/0142: not while it is still being shaped, `warming`).
    pub fn sync(&self, ctx: &egui::Context, app: &mut WordApp, warming: bool) {
        let id = app.session.document_id();
        // A recovered document is not the file on disk: it keeps its unsaved mark (wordcraft/0146). The open message is handled
        // between frames and the inbox is drained by this frame's logic, so a document other than the one last reported is it.
        if self.document.get() != Some(id) && self.recovered.replace(false) {
            app.session.dirty = true;
            ctx.request_repaint();
        }
        if !warming && self.document.replace(Some(id)).is_some_and(|shown| shown != id) {
            self.send(&[("type", "wordcraft:opened".into()), ("name", self.name.borrow().as_str().into())]);
        }
        // The phone layout's row is the only bar (wordcraft/0143): the shell hides its header while it is on, and Back is the shell's Home.
        #[cfg(feature = "office-ext")]
        {
            if let Some(on) = docs_office::phone::take_phone_change() {
                self.send(&[("type", "wordcraft:phone".into()), ("on", on.into())]);
            }
            if docs_office::phone::take_back() {
                self.send(&[("type", "wordcraft:back".into())]);
            }
        }
        // Read only is Restrict Editing's read-only protection, put back if the user lifts it or a
        // document replaced the one it was set on.
        if self.read_only.get() && app.session.doc.settings.protection.as_deref() != Some("readOnly") {
            app.session.doc.settings.protection = Some("readOnly".into());
            ctx.request_repaint();
        }
        if self.list_requested.replace(false) {
            let list = js_sys::Array::new();
            // A required parameter is written `"key":` in `params`, an optional one `"key"?:`: only the second kind runs from a bare id.
            for spec in app.session.registry.all().iter().filter(|s| !s.params.contains("\":")) {
                if let Some(item) = message(&[
                    ("id", spec.id.into()),
                    ("label", wordcraft_ui_egui::i18n::t(spec.label).into()),
                    ("enabled", wordcraft_ui_egui::widgets::enabled(app, spec.id).into()),
                    ("shortcut", wordcraft_ui_egui::widgets::shortcut_text(app, spec.id).as_str().into()),
                ]) {
                    list.push(&item);
                }
            }
            self.send(&[("type", "wordcraft:commands".into()), ("commands", list.into())]);
        }
        for id in self.runs.take() {
            let result = app.run(&id, json!({}));
            let error = result.as_ref().err().map(String::as_str).unwrap_or("");
            self.send(&[("type", "wordcraft:ran".into()), ("id", id.as_str().into()), ("ok", result.is_ok().into()), ("error", error.into())]);
            app.canvas.want_focus = true;
            ctx.request_repaint();
        }
        for ok in self.acks.take() {
            if self.pending.get() == 0 {
                break;
            }
            self.pending.set(self.pending.get() - 1);
            if !ok {
                app.session.dirty = true;
            }
            // `Services::on_dirty` reports the result in the next frame.
            ctx.request_repaint();
        }
        if self.save_requested.replace(false) && !self.read_only.get() {
            if let Err(e) = app.run("file.save", json!({})) {
                self.post_failed(&e);
            }
            // The saved document is clean now: a frame reports it (`Services::on_dirty`).
            ctx.request_repaint();
        }
        // The periodic autosave, only while there are unsaved changes.
        if app.session.dirty && !self.read_only.get() {
            let now = js_sys::Date::now();
            let last = self.last_autosave.get().unwrap_or(now);
            if now - last >= AUTOSAVE_MS {
                self.autosave.set(true);
                self.last_autosave.set(Some(now));
            } else {
                self.last_autosave.set(Some(last));
                // An idle page draws no frames: wake up when the period is over.
                ctx.request_repaint_after(std::time::Duration::from_millis((AUTOSAVE_MS - (now - last)) as u64 + 1));
            }
        } else {
            self.last_autosave.set(None);
        }
        // Not through `file.save`: the document stays dirty and the browser downloads nothing.
        if self.autosave.replace(false) && app.session.dirty {
            let name = self.name.borrow().clone();
            match wordcraft_ui_egui::office_overlay::autosave_bytes(app, &name).and_then(|b| self.post_file("wordcraft:autosave", &name, &b)) {
                Ok(()) => {}
                Err(e) => log::error!("couldn't hand {name} to the page for safekeeping: {e}"),
            }
        }
    }

    /// Hidden tabs and vanishing pages (`visibilitychange`, `pagehide`) may be the last chance to
    /// get the work out, and a lost WebGL context leaves a page that can't be drawn again. eframe
    /// keeps running the app's logic in a hidden tab (on a timer), so the flag set here is seen.
    fn listen_lifecycle(&self, ctx: &egui::Context) {
        let Some(document) = self.window.document() else { return };
        let on_hide = {
            let (autosave, ctx, document) = (self.autosave.clone(), ctx.clone(), document.clone());
            Closure::<dyn FnMut()>::new(move || {
                if document.hidden() {
                    autosave.set(true);
                    ctx.request_repaint();
                }
            })
        };
        let on_pagehide = {
            let (autosave, ctx) = (self.autosave.clone(), ctx.clone());
            Closure::<dyn FnMut()>::new(move || {
                autosave.set(true);
                ctx.request_repaint();
            })
        };
        let on_lost = {
            let (bridge, ctx) = (self.clone(), ctx.clone());
            Closure::<dyn FnMut()>::new(move || {
                bridge.autosave.set(true);
                bridge.post_failed("webglcontextlost");
                ctx.request_repaint();
            })
        };
        let listen = |target: &web_sys::EventTarget, event: &str, closure: &Closure<dyn FnMut()>| {
            let _ = target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
        };
        listen(&document, "visibilitychange", &on_hide);
        listen(&self.window, "pagehide", &on_pagehide);
        if let Some(canvas) = document.get_element_by_id("wordcraft_canvas") {
            listen(&canvas, "webglcontextlost", &on_lost);
        }
        // The listeners live as long as the page.
        on_hide.forget();
        on_pagehide.forget();
        on_lost.forget();
    }

    /// F6 leaves the editor and F1 opens the host's command palette (neither key is a command here).
    fn listen_keys(&self) {
        let bridge = self.clone();
        let on_key = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            let key = event.key();
            if (key == "F6" || key == "F1") && !(event.ctrl_key() || event.alt_key() || event.shift_key() || event.meta_key()) {
                event.prevent_default();
                bridge.send(&[("type", "wordcraft:focusOut".into()), ("palette", (key == "F1").into())]);
            }
        });
        let _ = self.window.add_event_listener_with_callback_and_bool("keydown", on_key.as_ref().unchecked_ref(), true);
        on_key.forget();
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

    fn listen(&self, inbox: Inbox, ctx: egui::Context) {
        let bridge = self.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != bridge.origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &bridge.parent)) {
                return;
            }
            let data = event.data();
            match string(&data, "type").as_deref() {
                Some("wordcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| DEFAULT_NAME.to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    bridge.read_only.set(bool_field(&data, "readOnly"));
                    bridge.recovered.set(bool_field(&data, "dirty"));
                    bridge.can_download.set(flag(&data, "canDownload"));
                    bridge.can_print.set(flag(&data, "canPrint"));
                    *bridge.name.borrow_mut() = name.clone();
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                }
                Some("wordcraft:requestSave") => bridge.save_requested.set(true),
                Some("wordcraft:commands") => bridge.list_requested.set(true),
                Some("wordcraft:run") => bridge.runs.borrow_mut().extend(string(&data, "id")),
                Some("wordcraft:focus") => {
                    if let Some(canvas) = bridge
                        .window
                        .document()
                        .and_then(|d| d.get_element_by_id("wordcraft_canvas"))
                        .and_then(|c| c.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = canvas.focus();
                    }
                }
                Some("wordcraft:saved") => bridge.acks.borrow_mut().push(bool_field(&data, "ok")),
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

fn bool_field(data: &JsValue, key: &str) -> bool {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// A boolean field that is true unless the parent sent `false`.
fn flag(data: &JsValue, key: &str) -> bool {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok().and_then(|v| v.as_bool()).unwrap_or(true)
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
