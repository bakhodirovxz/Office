//! PdfCraft in the browser. Build: `cd apps/pdfcraft-web && trunk build --release`
//! (trunk generates the JS loader; no handwritten JS — plan/execution-plan.md §1).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;
    use pdfcraft_ui_egui::PdfCraftApp;

    let options = eframe::WebOptions { renderer: eframe::Renderer::Glow, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("pdfcraft"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            return;
        };
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(|cc| {
                    let mut app = PdfCraftApp::new();
                    // "printcraft": settings saved under the app's former name, PrintCraft.
                    if let Some(json) = cc.storage.and_then(|s| s.get_string("pdfcraft").or_else(|| s.get_string("printcraft"))) {
                        app.restore(&json);
                    }
                    // `?theme=` (patches/pdfcraft/0120): the web shell's theme; without it the saved one stays.
                    app.set_theme_preference(pdfcraft_ui_egui::office_overlay::theme_at_start(app.theme_preference, query_param("theme").as_deref()));
                    // `?file=<url>` opens a PDF from a URL (same-origin or CORS-enabled).
                    if let Some(url) = query_param("file") {
                        let inbox = app.startup_inbox.clone();
                        let failed = app.failed_inbox.clone();
                        let ctx = cc.egui_ctx.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let name = url.rsplit('/').next().unwrap_or("document.pdf").split('?').next().unwrap_or("document.pdf").to_string();
                            match fetch_bytes(&url).await {
                                Ok(bytes) => {
                                    if let Ok(mut q) = inbox.lock() {
                                        q.push((name, bytes));
                                    }
                                }
                                Err(e) => {
                                    eframe::web_sys::console::error_1(&format!("PdfCraft: could not fetch {url}: {e}").into());
                                    // Shown in the app too, not only in the console (#173).
                                    if let Ok(mut q) = failed.lock() {
                                        q.push((name, e));
                                    }
                                }
                            }
                            ctx.request_repaint();
                        });
                    }
                    // eframe never raises a close request on a browser reload or tab close, so `guard_quit` never
                    // prompts; ask the browser to confirm instead while a document has unsaved work (#812).
                    warn_before_unload(app.unsaved_flag.clone());
                    // `?host=parent`: a same-origin page embedding PdfCraft in an iframe (such
                    // as the Nextcloud app) passes files in and takes saves back (`host` below).
                    let mut hosted = None;
                    if query_param("host").as_deref() == Some("parent")
                        && let Some(bridge) = host::Bridge::new()
                    {
                        // `?author=`: the host's name for the user signs their comments,
                        // unless they chose a name in PdfCraft (the web default is "Guest").
                        if let Some(author) = query_param("author").map(|a| a.trim().to_string()).filter(|a| !a.is_empty())
                            && app.comment_prefs.author == "Guest"
                        {
                            app.comment_prefs.author = author.chars().take(200).collect();
                        }
                        bridge.listen(cc.egui_ctx.clone());
                        app.host_save = Some(bridge.save_fn());
                        app.host_dirty = Some(bridge.dirty_fn());
                        bridge.post_ready();
                        hosted = Some(bridge);
                    } else if query_param("host").as_deref() == Some("parent") {
                        eframe::web_sys::console::warn_1(
                            &"PdfCraft: ?host=parent needs a framing page on the same origin; saves download instead".into(),
                        );
                    }
                    Ok(Box::new(host::Shell { app, bridge: hosted }))
                }),
            )
            .await;
        if let Err(e) = started {
            eframe::web_sys::console::error_1(&e);
            if query_param("host").as_deref() == Some("parent")
                && let Some(bridge) = host::Bridge::new()
            {
                bridge.post_failed(&e.as_string().unwrap_or_else(|| format!("{e:?}")));
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn warn_before_unload(unsaved: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use eframe::wasm_bindgen::{JsCast, closure::Closure};
    let Some(window) = web_sys::window() else { return };
    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        if unsaved.load(std::sync::atomic::Ordering::Relaxed) {
            event.prevent_default();
        }
    });
    if window.add_event_listener_with_callback("beforeunload", handler.as_ref().unchecked_ref()).is_ok() {
        handler.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn query_param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(key)
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use eframe::wasm_bindgen::JsCast;
    let window = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url)).await.map_err(|e| format!("{e:?}"))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response")?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// The `postMessage` bridge to a same-origin parent page. Messages are plain objects with a
/// `type`:
///
/// - `pdfcraft:ready` (to the parent): the app is listening; send files now.
/// - `pdfcraft:open` (from the parent): `{ name, bytes }`, where `bytes` is an `ArrayBuffer` or
///   `Uint8Array` holding a PDF. It replaces the document the parent opened before (other tabs
///   stay). `readOnly: true` opens it read only: an edit is reverted at once (with a message) and
///   nothing is sent as a save. `canDownload: false` withholds Save As and the browser downloads
///   (exports, copies), `canPrint: false` Print; both default to true.
/// - `pdfcraft:opened` (to the parent): `{ name }`, the document is shown.
/// - `pdfcraft:requestSave` (from the parent): save that document now, as File › Save does.
/// - `pdfcraft:save` (to the parent): `{ name, saveAs, bytes }` when the user saves; `bytes` is
///   a transferred `ArrayBuffer`. The document stays marked as unsaved (no `dirty: false`) until
///   the parent has answered every save with `pdfcraft:saved`.
/// - `pdfcraft:saved` (from the parent): `{ ok }`, the answer to one `pdfcraft:save`, in order.
///   `ok: false` marks the document unsaved again.
/// - `pdfcraft:dirty` (to the parent): `{ dirty }` whenever unsaved work appears or goes away.
/// - `pdfcraft:failed` (to the parent): `{ error }` when the app couldn't start or open the file.
///
/// Only the parent window on the app's own origin is heard or answered. With a parent on
/// another origin there is no bridge: the app runs as if unframed and saves download.
#[cfg(target_arch = "wasm32")]
mod host {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::atomic::Ordering;

    use eframe::wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use pdfcraft_ui_egui::{Dialog, PdfCraftApp, SaveTarget};

    #[derive(Clone)]
    pub struct Bridge {
        window: web_sys::Window,
        parent: web_sys::Window,
        origin: String,
        /// A file from the parent, opened at the next frame.
        to_open: Rc<RefCell<Option<(String, Vec<u8>)>>>,
        /// The document the parent opened (once its tab exists), and the name it sent.
        doc: Rc<Cell<Option<pdfcraft_ui_egui::DocId>>>,
        name: Rc<RefCell<String>>,
        /// An open whose tab is not there yet (a password prompt): the tabs before it.
        awaiting: Rc<RefCell<Option<Vec<pdfcraft_ui_egui::DocId>>>>,
        /// `pdfcraft:open { readOnly, canDownload, canPrint }`.
        read_only: Rc<Cell<bool>>,
        can_download: Rc<Cell<bool>>,
        can_print: Rc<Cell<bool>>,
        /// The parent asked for a save since the last frame.
        save_requested: Rc<Cell<bool>>,
        /// Saves sent that the parent hasn't answered with `pdfcraft:saved` yet, and the answers since the last frame.
        pending: Rc<Cell<u32>>,
        acks: Rc<RefCell<Vec<bool>>>,
    }

    impl Bridge {
        /// `None` when the page isn't framed (there is no parent to talk to), or when the parent
        /// is on another origin: messages only go to this origin, so the browser would drop
        /// every save without an error while the document showed as saved (#2167).
        pub fn new() -> Option<Self> {
            let window = web_sys::window()?;
            let parent = window.parent().ok().flatten()?;
            if js_sys::Object::is(&parent, &window) {
                return None;
            }
            let origin = window.location().origin().ok()?;
            // Reading another origin's location throws, which is `None` too.
            if parent.location().origin().ok()? != origin {
                return None;
            }
            Some(Self {
                window,
                parent,
                origin,
                to_open: Rc::default(),
                doc: Rc::default(),
                name: Rc::new(RefCell::new("document.pdf".to_string())),
                awaiting: Rc::default(),
                read_only: Rc::default(),
                can_download: Rc::new(Cell::new(true)),
                can_print: Rc::new(Cell::new(true)),
                save_requested: Rc::default(),
                pending: Rc::default(),
                acks: Rc::default(),
            })
        }

        fn post(&self, message: &js_sys::Object, transfer: Option<&JsValue>) -> Result<(), String> {
            let sent = match transfer {
                Some(t) => self.parent.post_message_with_transfer(message, &self.origin, &js_sys::Array::of1(t)),
                None => self.parent.post_message(message, &self.origin),
            };
            sent.map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))
        }

        fn send(&self, fields: &[(&str, JsValue)]) {
            if let Some(m) = message(fields) {
                let _ = self.post(&m, None);
            }
        }

        pub fn post_ready(&self) {
            self.send(&[("type", "pdfcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
        }

        /// The app couldn't start (no WebGL, for example) or open the file: let the parent say so.
        pub fn post_failed(&self, error: &str) {
            self.send(&[("type", "pdfcraft:failed".into()), ("error", error.into())]);
        }

        /// Hear files, save requests and save answers from the parent.
        pub fn listen(&self, ctx: egui::Context) {
            let bridge = self.clone();
            let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                if event.origin() != bridge.origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &bridge.parent)) {
                    return;
                }
                let data = event.data();
                match string(&data, "type").as_deref() {
                    Some("pdfcraft:open") => {
                        let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "document.pdf".to_string());
                        let Ok(bytes) = js_sys::Reflect::get(&data, &JsValue::from_str("bytes")) else { return };
                        let bytes = if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
                            array.to_vec()
                        } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
                            js_sys::Uint8Array::new(&bytes).to_vec()
                        } else {
                            return;
                        };
                        bridge.read_only.set(flag(&data, "readOnly", false));
                        bridge.can_download.set(flag(&data, "canDownload", true));
                        bridge.can_print.set(flag(&data, "canPrint", true));
                        pdfcraft_ui_egui::HOST_WITHHOLDS_DOWNLOADS.store(!bridge.can_download.get(), Ordering::Relaxed);
                        *bridge.to_open.borrow_mut() = Some((name, bytes));
                    }
                    Some("pdfcraft:requestSave") => bridge.save_requested.set(true),
                    Some("pdfcraft:saved") => bridge.acks.borrow_mut().push(flag(&data, "ok", false)),
                    _ => return,
                }
                ctx.request_repaint();
            });
            if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
                // The listener lives as long as the page.
                on_message.forget();
            }
        }

        pub fn save_fn(&self) -> pdfcraft_ui_egui::HostSaveFn {
            let bridge = self.clone();
            Box::new(move |name: &str, bytes: &[u8], save_as: bool| {
                // read only, or Save As (a copy) while the parent withholds downloads
                if bridge.read_only.get() || (save_as && !bridge.can_download.get()) {
                    return Err(pdfcraft_ui_egui::i18n::t("Not allowed").to_string());
                }
                let buffer = js_sys::Uint8Array::from(bytes).buffer();
                let m =
                    message(&[("type", "pdfcraft:save".into()), ("name", name.into()), ("saveAs", save_as.into()), ("bytes", buffer.clone().into())])
                        .ok_or_else(|| "couldn't build the message".to_string())?;
                bridge.post(&m, Some(&buffer))?;
                bridge.pending.set(bridge.pending.get() + 1);
                Ok(())
            })
        }

        pub fn dirty_fn(&self) -> pdfcraft_ui_egui::HostDirtyFn {
            let bridge = self.clone();
            let mut last = false;
            Box::new(move |dirty: bool| {
                // A read-only document is reverted after this frame's logic (`sync`): it never counts as unsaved.
                let dirty = dirty && !bridge.read_only.get();
                // A saved document stays unsaved until the parent has stored it.
                if dirty != last && (dirty || bridge.pending.get() == 0) {
                    last = dirty;
                    bridge.send(&[("type", "pdfcraft:dirty".into()), ("dirty", dirty.into())]);
                }
            })
        }

        /// After each frame's logic: open a file the parent sent, apply the save answers, run a save
        /// it asked for, revert edits of a read-only document, and close a Print dialog the parent withholds.
        fn sync(&self, ctx: &egui::Context, app: &mut PdfCraftApp) {
            if let Some((name, bytes)) = self.to_open.borrow_mut().take() {
                let before: Vec<_> = app.views.iter().map(|v| v.id).collect();
                *self.name.borrow_mut() = name.clone();
                match app.open_bytes(&name, None, bytes) {
                    Ok(()) => *self.awaiting.borrow_mut() = Some(before),
                    Err(e) => self.post_failed(&e),
                }
            }
            // The new tab (now, or after a password prompt): it replaces the document the parent opened before.
            let shown = self.awaiting.borrow().as_ref().and_then(|before| app.views.iter().map(|v| v.id).find(|id| !before.contains(id)));
            if let Some(id) = shown {
                *self.awaiting.borrow_mut() = None;
                if let Some(old) = self.doc.replace(Some(id))
                    && let Some(i) = app.views.iter().position(|v| v.id == old)
                {
                    app.close_tab(i);
                }
                let name = self.name.borrow().clone();
                self.send(&[("type", "pdfcraft:opened".into()), ("name", name.as_str().into())]);
            }
            let index = self.doc.get().and_then(|id| app.views.iter().position(|v| v.id == id));
            for ok in self.acks.take() {
                if self.pending.get() == 0 {
                    break;
                }
                self.pending.set(self.pending.get() - 1);
                // Not stored by the parent: unsaved again.
                if !ok && let Some(id) = self.doc.get() {
                    let path = app.session.get(id).and_then(|d| d.path.clone());
                    app.session.mark_recovered(id, path);
                }
                ctx.request_repaint();
            }
            if self.save_requested.replace(false)
                && !self.read_only.get()
                && let Some(i) = index
            {
                app.save_view(i, SaveTarget::InPlace);
            }
            // Read only: an edit of the parent's document is taken back (File › Revert).
            if self.read_only.get()
                && let Some(i) = index
                && app.active == Some(i)
                && let Some(id) = self.doc.get()
                && app.session.get(id).is_some_and(|d| d.dirty)
            {
                app.revert_active();
                app.notify_fmt("{name} is read-only", &[("name", &self.name.borrow())]);
            }
            if !self.can_print.get() && app.dialog == Some(Dialog::Print) {
                app.dialog = None;
                app.notify_tr("Not allowed");
            }
        }
    }

    /// The app with the bridge's per-frame work after its own.
    pub struct Shell {
        pub app: PdfCraftApp,
        pub bridge: Option<Bridge>,
    }

    impl eframe::App for Shell {
        fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
            self.app.raw_input_hook(ctx, raw);
        }

        fn save(&mut self, storage: &mut dyn eframe::Storage) {
            self.app.save(storage);
        }

        fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            self.app.logic(ctx, frame);
            if let Some(bridge) = &self.bridge {
                bridge.sync(ctx, &mut self.app);
            }
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            self.app.ui(ui, frame);
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

    /// A boolean field, `default` when absent or not a boolean.
    fn flag(data: &JsValue, key: &str, default: bool) -> bool {
        js_sys::Reflect::get(data, &JsValue::from_str(key)).ok().and_then(|v| v.as_bool()).unwrap_or(default)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("pdfcraft-web targets wasm32: run `trunk serve` or `trunk build --release` in apps/pdfcraft-web");
}
