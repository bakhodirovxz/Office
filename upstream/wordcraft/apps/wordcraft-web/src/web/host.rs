//! The `postMessage` bridge to a parent page (`?host=parent`), for pages that embed WordCraft in
//! an `<iframe>` (a document server, a Nextcloud app). Messages are plain objects with a `type`:
//!
//! - `wordcraft:ready` (to the parent): `{ version }`, the app is listening; send a document now.
//! - `wordcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a document picked with File › Open or dropped on the window.
//! - `wordcraft:opened` (to the parent): `{ name }`, a document has replaced the one shown.
//! - `wordcraft:requestSave` (from the parent): save now, as File › Save does.
//! - `wordcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every
//!   file WordCraft would otherwise download: Save, Save As and the exports. The parent stores it
//!   and reports failures itself.
//! - `wordcraft:autosave` (to the parent): `{ name, bytes }`, the document with its unsaved changes,
//!   sent when the tab is hidden or the page goes away, so the parent holds the work if the
//!   browser then kills the page. It is not a save: the document stays dirty.
//! - `wordcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
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

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// The parent asked for a save since the last frame.
    save_requested: Rc<Cell<bool>>,
    /// The page was hidden or is going away since the last frame: hand over the unsaved document.
    autosave: Rc<Cell<bool>>,
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
            reported: Rc::default(),
            document: Rc::default(),
            name: Rc::new(RefCell::new(DEFAULT_NAME.to_string())),
        })
    }

    /// Send saved and exported files to the parent instead of downloading them, tell it when
    /// unsaved changes appear, and hear documents and save requests from it.
    pub fn connect(&self, services: &mut Services, inbox: Inbox, ctx: egui::Context) {
        self.listen_lifecycle(&ctx);
        self.listen(inbox, ctx);
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| bridge.post_file("wordcraft:save", name, bytes)));
        // Keep the hook the web shell installed (the leave-page guard reads it).
        let bridge = self.clone();
        let previous = services.on_dirty.take();
        services.on_dirty = Some(Box::new(move |dirty| {
            if let Some(previous) = &previous {
                previous(dirty);
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
    /// shown.
    pub fn sync(&self, ctx: &egui::Context, app: &mut WordApp) {
        let id = app.session.document_id();
        if self.document.replace(Some(id)).is_some_and(|shown| shown != id) {
            self.send(&[("type", "wordcraft:opened".into()), ("name", self.name.borrow().as_str().into())]);
        }
        if self.save_requested.replace(false) {
            if let Err(e) = app.run("file.save", json!({})) {
                self.post_failed(&e);
            }
            // The saved document is clean now: a frame reports it (`Services::on_dirty`).
            ctx.request_repaint();
        }
        // Not through `file.save`: the document stays dirty and the browser downloads nothing.
        if self.autosave.replace(false) && app.session.dirty {
            let name = self.name.borrow().clone();
            let password = app.session.password.as_ref().map(wordcraft_engine::Password::as_str);
            let options = wordcraft_engine::cmd::file::export_options(&app.session, &json!({}));
            match wordcraft_engine::io::save_bytes_with(&name, &app.session.doc, password, &options)
                .and_then(|b| self.post_file("wordcraft:autosave", &name, &b))
            {
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
                    *bridge.name.borrow_mut() = name.clone();
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                }
                Some("wordcraft:requestSave") => bridge.save_requested.set(true),
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
