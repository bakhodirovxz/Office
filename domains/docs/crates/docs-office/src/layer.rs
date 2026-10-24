//! The office layer on the command hook (WP10 and WP11), desktop only: a version snapshot after every save of the document's own file,
//! `office.history` (and upstream's Version History, `file.versions`) opening the shell's History panel, and the XDFU watermark.
//!
//! XDFU is watermark only (owner decision 2026-10-22): nothing is ever refused. When `xdfu = on` in the shared settings and the open
//! document carries a grif (XDFU, ХДФУ, ДСП in the first 25 paragraphs of the body or at the top of a header, footer or text box), every
//! copy made by print to PDF, PDF/PNG export, Save As, Save as Template, or a save to another path is stamped with the Resolution 766
//! cl. 12 line: copy number (the keyed audit chain's next `seq`), network address, user, date and time. The line is the document's
//! diagonal watermark for the duration of the command (restored afterwards, so the working document stays clean); txt, md and html
//! copies also get it as their first line. A plain Save of the working file is not a copy and is never stamped. `edit.copy` and
//! `edit.cut` are not blocked and carry no mark (clipboard text has no place for one).
//! With `xdfu = off` nothing XDFU is registered, no check runs and no audit file is created.

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use suite_core::history::{self, Store};
use wordcraft_doc::{StoryRef, Watermark};
use wordcraft_engine::{CmdError, CmdResult, CommandSpec, Session};
use xdfu_core::{LazyIssuer, Stamp};

use crate::hooks::{self, CommandHook};

/// An autosave (`file.save` with `autosave: true`, every few seconds while dirty) snapshots at most this often per document. An explicit
/// save always snapshots (the store itself skips a file whose bytes did not change).
const AUTOSAVE_EVERY: Duration = Duration::from_secs(300);

pub const INSERT_GRIF: &str = "office.xdfu.insertGrif";

struct Config {
    store: Option<Store>,
    xdfu: Option<LazyIssuer>,
}

static CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);
static LAST_SNAPSHOT: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

fn config() -> Option<Arc<Config>> {
    CONFIG.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// True when XDFU is configured on: the commands `office.xdfu.*` are registered.
pub fn xdfu_on() -> bool {
    config().is_some_and(|c| c.xdfu.is_some())
}

/// Reads the shared settings and installs the layer, once per process. (Not in this crate's own tests: they call [`configure`].)
pub fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let on = suite_core::settings::load().xdfu;
        configure(Store::from_settings(), if on { history::xdfu_audit_paths() } else { None });
    });
}

/// Installs the layer: `store` takes the snapshots (`None`: no history); `audit` is the XDFU audit log and key paths (`None`: XDFU off,
/// nothing of it is registered). Sessions made afterwards get the commands.
pub fn configure(store: Option<Store>, audit: Option<(std::path::PathBuf, std::path::PathBuf)>) {
    let xdfu = audit.map(|(log, key)| LazyIssuer::new(log, key));
    *CONFIG.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Config { store, xdfu }));
    hooks::set_hook(Some(Arc::new(Layer)));
}

/// `office.history` and `file.versions`: show the History panel of the shell for the document's file.
pub fn open_history(s: &mut Session, _: &Value) -> CmdResult {
    let Some(path) = s.path.clone() else {
        return Err(CmdError::Failed("Save the file first, then open its version history.".into()));
    };
    history::open_panel(&path).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(json!({"path": path.to_string_lossy()}))
}

pub fn specs() -> Vec<CommandSpec> {
    if !xdfu_on() {
        return vec![];
    }
    vec![
        CommandSpec::new(INSERT_GRIF, "Insert XDFU grif", "Insert", insert_grif)
            .params(r#"{"grif"?: "XDFU"|"ХДФУ"|"ДСП"} → {grif, copy}: a right-aligned paragraph at the start of the body, with "Nusxa N" from the audit counter"#),
    ]
}

struct Layer;

impl CommandHook for Layer {
    fn wrap(&self, id: &str, s: &mut Session, params: &Value, run: &mut dyn FnMut(&mut Session) -> CmdResult) -> CmdResult {
        let Some(c) = config() else { return run(s) };
        let Some(x) = c.xdfu.as_ref() else { return run(s) };
        let Some((event, ext)) = copy_kind(s, id, params) else { return run(s) };
        if !has_grif(s) {
            return run(s);
        }
        let name = s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        x.issue(event, &name, |stamp| make_copy(s, run, params, &ext, stamp))
    }

    fn after(&self, id: &str, s: &Session, params: &Value, result: &CmdResult) {
        if !matches!(id, "file.save" | "file.saveAs") {
            return;
        }
        let Some(c) = config() else { return };
        let (Ok(out), Some(store), Some(path)) = (result, c.store.as_ref(), s.path.as_deref()) else { return };
        // Only the document's own file is versioned: a pdf or txt copy, or a save that opened the Save As dialog, is not.
        if out.get("saved") != Some(&json!(true)) || out.get("path").and_then(Value::as_str) != Some(&path.to_string_lossy()) {
            return;
        }
        let key = path.to_string_lossy().into_owned();
        let autosave = params.get("autosave").and_then(Value::as_bool) == Some(true);
        let now = Instant::now();
        let mut last = LAST_SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner());
        let at = last.iter().position(|(p, _)| *p == key);
        if autosave && at.is_some_and(|i| now.duration_since(last[i].1) < AUTOSAVE_EVERY) {
            return;
        }
        match at {
            Some(i) => last[i].1 = now,
            None => last.push((key.clone(), now)),
        }
        drop(last);
        if let Err(e) = store.snapshot(Path::new(&key), None) {
            eprintln!("office: version snapshot of {key} failed: {e}");
        }
    }
}

type PrintJob = Result<(Vec<u8>, wordcraft_engine::print::PrintPlan), CmdError>;

/// A print to a physical printer (the Print dialog's Submit; upstream renders its PDF with `Session::print_pdf`, not the `file.printPdf`
/// command): with XDFU on and a grif in the document the pages carry the stamp and the audit log gets a `print` line. Never refuses.
pub fn print_wrap(s: &mut Session, run: &mut dyn FnMut(&mut Session) -> PrintJob) -> PrintJob {
    let Some(x) = config().and_then(|c| c.xdfu.is_some().then_some(c)) else { return run(s) };
    if !has_grif(s) {
        return run(s);
    }
    let name = s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    x.xdfu.as_ref().expect("checked").issue("print", &name, |stamp| {
        let before = s.doc.settings.watermark.clone();
        s.doc.settings.watermark = Some(Watermark { text: stamp.ascii_line().chars().take(200).collect(), ..Watermark::default() });
        let r = run(s);
        s.doc.settings.watermark = before;
        r
    })
}

/// `(audit event, output extension)` when the command makes a copy of the document, else `None`.
fn copy_kind(s: &Session, id: &str, p: &Value) -> Option<(&'static str, String)> {
    let path = p.get("path").and_then(Value::as_str).filter(|x| !x.is_empty())?;
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match id {
        "file.printPdf" => Some(("print", "pdf".into())),
        "file.exportPdf" => Some(("export", "pdf".into())),
        "file.exportPng" => Some(("export", "png".into())),
        "file.saveAs" | "file.saveTemplate" => Some(("save_as", ext)),
        // The working file itself is not a copy; a save to another path is.
        "file.save" if s.path.as_deref() != Some(Path::new(path)) => Some(("save_as", ext)),
        _ => None,
    }
}

/// The top of the document holds a grif: the first 25 body paragraphs, and the first 5 lines of every other story (headers, footers,
/// text boxes).
fn has_grif(s: &Session) -> bool {
    let d = &s.doc;
    let mut text = String::new();
    for path in d.para_paths(StoryRef::Body).iter().take(25) {
        if let Some(p) = d.para(StoryRef::Body, path) {
            text.push_str(&p.plain_text().to_string());
            text.push('\n');
        }
    }
    for k in d.parts.keys() {
        for line in d.plain_text(StoryRef::Part(*k)).lines().take(5) {
            text.push_str(line);
            text.push('\n');
        }
    }
    xdfu_core::find_grif(&text).is_some()
}

/// Runs the copying command with the stamp as the watermark and the document's own watermark back afterwards.
fn make_copy(s: &mut Session, run: &mut dyn FnMut(&mut Session) -> CmdResult, params: &Value, ext: &str, stamp: &Stamp) -> CmdResult {
    let line = if matches!(ext, "pdf" | "png") { stamp.ascii_line() } else { stamp.line() };
    let text = matches!(ext, "txt" | "md" | "html" | "htm");
    let before = s.doc.settings.watermark.clone();
    if !text {
        s.doc.settings.watermark = Some(Watermark { text: line.chars().take(200).collect(), ..Watermark::default() });
    }
    let r = run(s);
    s.doc.settings.watermark = before;
    let out = r?;
    if text && let Some(path) = params.get("path").and_then(Value::as_str) {
        let ext = if ext == "htm" { "html" } else { ext };
        // a copy that cannot be stamped is deleted rather than left unmarked
        let done = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|old| xdfu_core::stamp_text(&old, ext, &line))
            .and_then(|new| std::fs::write(path, new).map_err(|e| e.to_string()));
        if let Err(e) = done {
            let _ = std::fs::remove_file(path);
            return Err(CmdError::Failed(format!("XDFU stamp: {e}")));
        }
    }
    Ok(out)
}

fn insert_grif(s: &mut Session, p: &Value) -> CmdResult {
    let Some(x) = config().filter(|c| c.xdfu.is_some()) else { return Err(CmdError::Disabled("XDFU is off".into())) };
    let grif = match p.get("grif").and_then(Value::as_str) {
        None => "XDFU",
        Some(g) if ["XDFU", "ХДФУ", "ДСП"].contains(&g) => g,
        Some(g) => return Err(CmdError::Params(format!("`grif` must be XDFU, ХДФУ or ДСП, not `{g}`"))),
    };
    let name = s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    x.xdfu.as_ref().expect("checked").issue("grif_inserted", &name, |st| {
        if st.copy == 0 {
            return Err(CmdError::Failed("the XDFU audit log is unusable: no copy number".into()));
        }
        let start = s.doc.start_of(StoryRef::Body);
        s.run("select.range", &json!({"anchor": start, "focus": start}))?;
        s.run("text.insert", &json!({"text": grif}))?;
        s.run("text.lineBreak", &json!({}))?;
        s.run("text.insert", &json!({"text": format!("Nusxa \u{2116} {}", st.copy)}))?;
        s.run("text.newParagraph", &json!({}))?;
        s.run("select.range", &json!({"anchor": start, "focus": start}))?;
        s.run("para.alignRight", &json!({}))?;
        Ok(json!({"grif": grif, "copy": st.copy}))
    })
}
