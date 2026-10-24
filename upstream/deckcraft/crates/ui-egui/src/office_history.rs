// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The office layer on the command hook (patch 0143): a version snapshot after every save of the document's file (PLAN WP10), the
//! Version History button's handler, and the XDFU watermark (WP11).
//!
//! XDFU is watermark only (owner decision 2026-10-22): nothing is ever refused. When `xdfu = on` and the open presentation carries a
//! grif (XDFU, ХДФУ, ДСП in a text box in the top quarter of slide 1 or in its title), every copy made by print, export (PDF, PNG,
//! JPEG, GIF, PPTX, deckcraft, outline), save-as, save-as-template or `file.saveBytes` is stamped with the Resolution 766 cl. 12 line:
//! copy number (the keyed audit chain's next `seq`), network address, user, date and time. The stamp is a text box along the bottom
//! of every slide, present only while the command runs; the working presentation is put back afterwards. An outline copy gets the
//! line as its first line. A plain `file.save` of the working file is not a copy and is not stamped; `edit.copy` / `edit.cut` are not
//! blocked and carry no mark. With `xdfu = off` nothing XDFU is registered, no check runs and no audit file is created.
//!
//! Desktop only; `init` installs the hook once, from the app's `main`. The web build installs none.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use deckcraft_engine::cmd::file::format_for_path;
use deckcraft_engine::geom::Xfrm;
use deckcraft_engine::model::style::{ColorRef, Fill};
use deckcraft_engine::model::text::{Run, RunProps};
use deckcraft_engine::model::{Geom, Presentation, Rgba, Shape, ShapeId, TextBody};
use deckcraft_engine::office_hook::{self, CommandHook};
use deckcraft_engine::{EngineError, Result, Session, register_command};
use serde_json::{Value as Json, json};
use suite_core::history::{self, Store};
use xdfu_core::{LazyIssuer, Stamp};

use crate::SlideApp;

static STORE: OnceLock<Option<Store>> = OnceLock::new();
static XDFU: OnceLock<LazyIssuer> = OnceLock::new();

struct Layer;

/// Reads the shared settings and installs the layer. Without the overlay it does nothing (upstream's behaviour).
pub fn init() {
    if crate::office_overlay::ACTIVE {
        crate::office_toolbar::set_history_handler(open_history);
        let on = suite_core::settings::load().xdfu;
        install(Store::from_settings(), if on { history::xdfu_audit_paths() } else { None });
    }
}

/// Installs the layer once per process. `store` takes the snapshots (`None`: no history); `audit` is the XDFU audit log and key paths
/// (`None`: XDFU off, nothing of it is registered). Returns false when a layer is already installed.
pub fn install(store: Option<Store>, audit: Option<(PathBuf, PathBuf)>) -> bool {
    if STORE.set(store).is_err() {
        return false;
    }
    if let Some((log, key)) = audit {
        let _ = XDFU.set(LazyIssuer::new(log, key));
        register_command(
            "office.xdfu.insertGrif",
            "Insert XDFU grif",
            "{grif?: \"XDFU\"|\"ХДФУ\"|\"ДСП\"} → {grif, copy}: a text box at the top right of slide 1 with the grif and \"Nusxa N\" from the audit counter",
            insert_grif,
        );
    }
    office_hook::set_hook(Some(Arc::new(Layer)));
    true
}

/// The Version History button: the panel for the presentation's file, or a message when it has none yet.
fn open_history(app: &mut SlideApp) {
    let Some(path) = app.session.active().and_then(|d| d.path.clone()) else {
        app.notify(crate::i18n::t("Save the file first, then open its version history."));
        return;
    };
    if let Err(e) = history::open_panel(Path::new(&path)) {
        app.notify(e.to_string());
    }
}

impl CommandHook for Layer {
    fn wrap(&self, id: &str, s: &mut Session, params: &Json, run: &mut dyn FnMut(&mut Session) -> Result<Json>) -> Result<Json> {
        let Some(x) = XDFU.get() else { return run(s) };
        let Some((event, kind)) = copy_kind(s, id, params) else { return run(s) };
        if !has_grif(s) {
            return run(s);
        }
        let name = s.active().map(|d| d.title()).unwrap_or_default();
        x.issue(event, &name, |stamp| make_copy(s, run, &kind, stamp))
    }

    fn after(&self, id: &str, s: &Session, _params: &Json, result: &Result<Json>) {
        if !matches!(id, "file.save" | "file.saveAs") {
            return;
        }
        let (Ok(out), Some(Some(store)), Some(d)) = (result, STORE.get(), s.active()) else { return };
        // Only the document's own file is versioned: `needsPath` (the Save As dialog was requested) saved nothing. Slides has no
        // timed autosave of the file (AutoRecover writes its own folder), so every save here is an explicit one.
        let (Some(path), Some(saved)) = (d.path.as_deref(), out.get("path").and_then(Json::as_str)) else { return };
        if saved != path {
            return;
        }
        if let Err(e) = store.snapshot(Path::new(path), None) {
            eprintln!("office: version snapshot of {path} failed: {e}");
        }
    }
}

// ---------------------------------------------------------------- XDFU: which commands make a copy

/// What a copy is written as; decides the form of the stamp.
struct Kind {
    /// Rendered outputs (pdf, png, jpeg, gif) get the ASCII line: the fonts of the renderer are the ones the line must survive.
    ascii: bool,
    /// An outline is plain text: the line goes on top of the produced file or bytes.
    text: bool,
}

/// `(audit event, kind)` when the command makes a copy of the presentation, else `None`.
fn copy_kind(s: &Session, id: &str, p: &Json) -> Option<(&'static str, Kind)> {
    let path = p.get("path").and_then(Json::as_str).filter(|x| !x.is_empty());
    let own = s.active()?.path.as_deref();
    let format = |path: Option<&str>| -> String {
        p.get("format")
            .and_then(Json::as_str)
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| path.map(|x| format_for_path(x).to_string()).unwrap_or_else(|| "deckcraft".into()))
    };
    let (event, fmt) = match id {
        "file.export" => (if p.get("print").and_then(Json::as_bool) == Some(true) { "print" } else { "export" }, format(path)),
        // A save to another path (a first save, a copy under a new name, a template) is a copy; the working file itself is not.
        "file.saveAs" | "file.save" if path.is_some() && path != own => ("save_as", format(path)),
        "file.saveTemplate" if path.is_some() => ("save_as", format(path)),
        "file.saveBytes" => ("download", format(None)),
        _ => return None,
    };
    Some((event, Kind { ascii: matches!(fmt.as_str(), "pdf" | "png" | "jpeg" | "jpg" | "gif"), text: matches!(fmt.as_str(), "outline" | "txt") }))
}

/// Slide 1 carries a grif: in the title, or in a text box whose top edge is in the top quarter of the slide.
fn has_grif(s: &Session) -> bool {
    let Some(d) = s.active() else { return false };
    let Some(slide) = d.doc.slides.first() else { return false };
    let quarter = d.doc.slide_size.height / 4.0;
    let mut text = String::new();
    for sh in &slide.shapes {
        let (Some(body), false) = (&sh.text, sh.hidden) else { continue };
        let title = sh.ph.as_ref().is_some_and(|p| p.kind.is_title());
        if title || sh.xfrm.is_some_and(|x| x.y < quarter) {
            text.push_str(&body.text());
            text.push('\n');
        }
    }
    xdfu_core::find_grif(&text).is_some()
}

/// Runs the copying command with the stamp in place and the working presentation back as it was afterwards.
fn make_copy(s: &mut Session, run: &mut dyn FnMut(&mut Session) -> Result<Json>, kind: &Kind, stamp: &Stamp) -> Result<Json> {
    let line = if kind.ascii { stamp.ascii_line() } else { stamp.line() };
    let swapped = if kind.text { None } else { stamp_slides(s, &line) };
    let r = run(s);
    if let Some((orig, stamped)) = swapped
        && let Ok(d) = s.doc_mut()
    {
        if Arc::ptr_eq(&d.doc, &stamped) {
            d.doc = orig.clone();
        }
        // a save under a new name makes the stamped presentation "the saved one": the working document stays clean
        if Arc::ptr_eq(&d.saved_doc, &stamped) {
            d.saved_doc = orig;
        }
        d.revision += 1;
    }
    let mut out = r?;
    if kind.text {
        stamp_output(&mut out, &line)?;
    }
    Ok(out)
}

/// A red bold text box of `size` pt along the bottom of the slide.
fn stamp_shape(doc: &mut Presentation, text: &str, size: f64) -> Option<Shape> {
    let (w, h) = (doc.slide_size.width, doc.slide_size.height);
    let mut body = TextBody::from_text(text);
    body.body.wrap = Some(true);
    let props = RunProps { size: Some(size), bold: Some(true), fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(0xB0, 0, 0)))), ..Default::default() };
    for para in &mut body.paragraphs {
        para.runs = vec![Run::with(para.text(), props.clone())];
    }
    Some(Shape {
        id: ShapeId(doc.alloc_id()?),
        name: "XDFU stamp".into(),
        xfrm: Some(Xfrm::new(10.0, h - 22.0, w - 20.0, 18.0)),
        text_box: true,
        text: Some(body),
        geom: Geom::preset("rect"),
        ..Default::default()
    })
}

/// The presentation with the stamp on every slide, made the active one. Returns (original, stamped).
fn stamp_slides(s: &mut Session, line: &str) -> Option<(Arc<Presentation>, Arc<Presentation>)> {
    let d = s.doc_mut().ok()?;
    let orig = d.doc.clone();
    let mut doc = (*orig).clone();
    for i in 0..doc.slides.len() {
        let shape = stamp_shape(&mut doc, line, 8.0)?;
        Arc::make_mut(&mut doc.slides[i]).shapes.push(shape);
    }
    let stamped = Arc::new(doc);
    d.doc = stamped.clone();
    d.revision += 1;
    Some((orig, stamped))
}

/// An outline copy: the line goes on top of the produced file (`path`) or bytes (`data`, base64). A copy that cannot be stamped is
/// deleted rather than left unmarked.
fn stamp_output(out: &mut Json, line: &str) -> Result<()> {
    let fail = |m: String| EngineError::Internal("office.xdfu".into(), m);
    if let Some(b64) = out.get("data").and_then(Json::as_str) {
        let old = deckcraft_engine::cmd::base64_decode(b64).ok_or_else(|| fail("bad base64".into()))?;
        let new = xdfu_core::stamp_text(&old, "txt", line).map_err(fail)?;
        out["data"] = json!(deckcraft_engine::cmd::base64_encode(&new));
        out["bytes"] = json!(new.len());
    } else if let Some(path) = out.get("path").and_then(Json::as_str).map(str::to_string) {
        let done = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|old| xdfu_core::stamp_text(&old, "txt", line))
            .and_then(|new| std::fs::write(&path, &new).map(|()| new.len()).map_err(|e| e.to_string()));
        match done {
            Ok(n) => out["bytes"] = json!(n),
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                return Err(fail(e));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- office.xdfu.insertGrif

fn insert_grif(s: &mut Session, p: &Json) -> Result<Json> {
    const CMD: &str = "office.xdfu.insertGrif";
    let bad = |msg: &str| EngineError::BadParams { cmd: CMD.into(), msg: msg.into() };
    let Some(x) = XDFU.get() else { return Err(bad("XDFU is off")) };
    let grif = match p.get("grif").and_then(Json::as_str) {
        None => "XDFU",
        Some(g) if ["XDFU", "ХДФУ", "ДСП"].contains(&g) => g,
        Some(g) => return Err(bad(&format!("`grif` must be XDFU, ХДФУ or ДСП, not `{g}`"))),
    };
    let (name, width) = {
        let d = s.doc()?;
        if d.doc.slides.is_empty() {
            return Err(bad("the presentation has no slide"));
        }
        (d.title(), d.doc.slide_size.width)
    };
    x.issue("grif_inserted", &name, |st| {
        if st.copy == 0 {
            return Err(EngineError::Other("the XDFU audit log is unusable: no copy number".into()));
        }
        let text = format!("{grif}\nNusxa {}", st.copy);
        s.edit(|doc, _| {
            let mut shape = stamp_shape(doc, &text, 14.0).ok_or_else(|| bad("no free shape id"))?;
            shape.name = "XDFU grif".into();
            shape.xfrm = Some(Xfrm::new(width - 170.0, 10.0, 160.0, 40.0));
            let slide = Arc::make_mut(&mut doc.slides[0]);
            // a grif inserted before is replaced, not stacked
            slide.shapes.retain(|sh| sh.name != "XDFU grif");
            slide.shapes.push(shape);
            Ok(())
        })?;
        Ok(json!({"grif": grif, "copy": st.copy}))
    })
}
