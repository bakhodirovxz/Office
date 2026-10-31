// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The office layer on the command hook (patch 0143): a version snapshot after every save of the document's file (PLAN WP10), the
//! `office.history` command behind the toolbar's Version History button, and the XDFU watermark (WP11).
//!
//! XDFU is watermark only (owner decision 2026-10-22): nothing is ever refused. When `xdfu = on` and the open workbook carries a grif
//! (XDFU, ХДФУ, ДСП in the first five rows of the first sheet or in its headers and footers), every copy made by print, PDF/CSV/HTML
//! export, save-as, or `file.saveBytes` (the web host's download) is stamped with the Resolution 766 cl. 12 line: copy number (the keyed
//! audit chain's next `seq`), network address, user, date and time. The working file is not a copy: a plain save is never stamped.
//! Formats with page setup (xlsx, json, pdf, print) get the line as the first line of the centre footer for the duration of the
//! command, restored afterwards; csv, tsv, txt and html get it as their first line. `edit.copy` and `edit.cut` are not blocked and
//! carry no mark (clipboard text has no place for one).
//! With `xdfu = off` nothing XDFU is registered, no check runs and no audit file is created.
//!
//! Desktop only; `init` installs the hook once, from the app's `main`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use gridcraft_engine::cmd::print::{codes_to_fields, join_header_footer, split_header_footer};
use gridcraft_engine::core::RangeRef;
use gridcraft_engine::model::Workbook;
use gridcraft_engine::office_hook::{self, Hook};
use gridcraft_engine::{EngineError, Result, Session, register_command};
use serde_json::{Value as Json, json};
use suite_core::history::{self, Store};
use xdfu_core::{LazyIssuer, Stamp};

/// An autosave (`file.save` with `autosave: true`, every ~2 s while dirty) snapshots at most this often per document. An explicit save
/// always snapshots (the store itself skips a file whose bytes did not change).
const AUTOSAVE_EVERY: Duration = Duration::from_secs(300);

static STORE: OnceLock<Option<Store>> = OnceLock::new();
static XDFU: OnceLock<LazyIssuer> = OnceLock::new();
static LAST_SNAPSHOT: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

struct Layer;

/// Reads the shared settings and installs the layer. Without the overlay it does nothing (upstream's behaviour).
pub fn init() {
    if crate::office_overlay::ACTIVE {
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
    register_command("office.history", "Version History", "{} (opens the History panel for the document's file)", history_cmd);
    if let Some((log, key)) = audit {
        let _ = XDFU.set(LazyIssuer::new(log, key));
        register_command(
            "office.xdfu.insertGrif",
            "Insert XDFU grif",
            "{grif?: \"XDFU\"|\"ХДФУ\"|\"ДСП\"} → {grif, copy}: the first sheet's header, right section, with \"Nusxa N\" from the audit counter",
            insert_grif,
        );
    }
    office_hook::install(Box::new(Layer))
}

fn bad(cmd: &str, msg: &str) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn history_cmd(s: &mut Session, _: &Json) -> Result<Json> {
    let path = s.doc()?.path.clone().ok_or_else(|| EngineError::Other("Save the file first, then open its version history.".into()))?;
    history::open_panel(Path::new(&path)).map_err(|e| EngineError::Internal("office.history".into(), e.to_string()))?;
    Ok(json!({"path": path}))
}

impl Hook for Layer {
    fn wrap(&self, s: &mut Session, id: &str, params: &Json, run: &mut dyn FnMut(&mut Session, Json) -> Result<Json>) -> Result<Json> {
        let Some(x) = XDFU.get() else { return run(s, params.clone()) };
        let Some((event, ext)) = copy_kind(s, id, params) else { return run(s, params.clone()) };
        if !has_grif(s) {
            return run(s, params.clone());
        }
        let name = s.active().map(|d| d.display_title()).unwrap_or_default();
        x.issue(event, &name, |stamp| make_copy(s, run, params, &ext, stamp))
    }

    fn after(&self, s: &Session, id: &str, params: &Json, result: &Result<Json>) {
        if !matches!(id, "file.save" | "file.saveAs") {
            return;
        }
        let (Ok(out), Some(Some(store)), Some(d)) = (result, STORE.get(), s.active()) else { return };
        // Only the document's own file is versioned: a csv/pdf copy, or a save that opened the Save As dialog, is not.
        let (Some(path), Some(saved)) = (d.path.as_deref(), out.get("path").and_then(Json::as_str)) else { return };
        if saved != path {
            return;
        }
        let autosave = params.get("autosave").and_then(Json::as_bool) == Some(true);
        // this module is desktop only (never built for wasm32), where `Instant::now` is fine
        #[allow(clippy::disallowed_methods)]
        let now = Instant::now();
        let mut last = LAST_SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner());
        let at = last.iter().position(|(p, _)| p == path);
        if autosave && at.is_some_and(|i| now.duration_since(last[i].1) < AUTOSAVE_EVERY) {
            return;
        }
        match at {
            Some(i) => last[i].1 = now,
            None => last.push((path.to_string(), now)),
        }
        drop(last);
        if let Err(e) = store.snapshot(Path::new(path), None) {
            eprintln!("office: version snapshot of {path} failed: {e}");
        }
    }
}

// ---------------------------------------------------------------- XDFU: which commands make a copy

/// `(audit event, output extension)` when the command makes a copy of the workbook, else `None`.
fn copy_kind(s: &Session, id: &str, p: &Json) -> Option<(&'static str, String)> {
    let path = p.get("path").and_then(Json::as_str).filter(|x| !x.is_empty());
    let ext = |x: &str| Path::new(x).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match id {
        "file.print" => Some(("print", "pdf".into())),
        "file.exportPdf" => Some(("export", "pdf".into())),
        "file.exportCsv" => Some(("export", "csv".into())),
        "file.exportHtml" => Some(("export", "html".into())),
        // The working file itself is not a copy; a save to another path (a first save, a pdf, a csv) is.
        "file.save" | "file.saveAs" => {
            let path = path?;
            if s.active()?.path.as_deref() == Some(path) { None } else { Some(("save_as", ext(path))) }
        }
        "file.saveBytes" => Some(("download", p.get("format").and_then(Json::as_str).unwrap_or("xlsx").to_ascii_lowercase())),
        _ => None,
    }
}

/// The first five rows of the first sheet and its headers and footers hold a grif.
fn has_grif(s: &Session) -> bool {
    let Some(sh) = s.active().and_then(|d| d.wb.sheet(0)) else { return false };
    let mut text = String::new();
    for (_, c) in sh.cells.iter_range(RangeRef::rows(0, 4)) {
        text.push_str(&c.input_text());
        text.push('\n');
    }
    let p = &sh.print;
    for spec in [&p.header, &p.footer, &p.first_header, &p.first_footer, &p.even_header, &p.even_footer] {
        for part in split_header_footer(spec) {
            // drop the `&X` codes that sit against the text (`&BXDFU`)
            let mut it = part.chars();
            while let Some(c) = it.next() {
                if c == '&' {
                    it.next();
                } else {
                    text.push(c);
                }
            }
            text.push('\n');
        }
    }
    xdfu_core::find_grif(&text).is_some()
}

/// Runs the copying command with the stamp in place and the working document back as it was afterwards.
fn make_copy(s: &mut Session, run: &mut dyn FnMut(&mut Session, Json) -> Result<Json>, params: &Json, ext: &str, stamp: &Stamp) -> Result<Json> {
    let line = if ext == "pdf" { stamp.ascii_line() } else { stamp.line() };
    let text = matches!(ext, "csv" | "tsv" | "txt" | "html" | "htm");
    let swapped = if text { None } else { stamp_footers(s, &line) };
    let r = run(s, params.clone());
    if let Some((orig, stamped)) = swapped
        && let Ok(d) = s.doc_mut()
    {
        if Arc::ptr_eq(&d.wb, &stamped) {
            d.wb = orig.clone();
        }
        // a save to a new xlsx makes the stamped workbook "the saved one": the working document stays clean
        if Arc::ptr_eq(&d.saved, &stamped) {
            d.saved = orig;
        }
    }
    let mut out = r?;
    if text {
        stamp_output(&mut out, if ext == "htm" { "html" } else { ext }, &line)?;
    }
    Ok(out)
}

/// The workbook with `line` at the top of every sheet's centre footer (in the variants the sheet uses). Returns (original, stamped).
fn stamp_footers(s: &mut Session, line: &str) -> Option<(Arc<Workbook>, Arc<Workbook>)> {
    let d = s.doc_mut().ok()?;
    let orig = d.wb.clone();
    let mut wb = (*orig).clone();
    for sh in &mut wb.sheets {
        let p = &mut Arc::make_mut(sh).print;
        p.footer = with_stamp(&p.footer, line);
        if p.different_odd_even {
            p.even_footer = with_stamp(&p.even_footer, line);
        }
        if p.different_first {
            p.first_footer = with_stamp(&p.first_footer, line);
        }
    }
    let stamped = Arc::new(wb);
    d.wb = stamped.clone();
    Some((orig, stamped))
}

/// The stamp as the first line of the centre section: footers sit on their last line, so the stamp is above anything on the sides.
fn with_stamp(footer: &str, line: &str) -> String {
    let mut sec = split_header_footer(footer);
    let line = line.replace('&', "&&");
    sec[1] = if !sec[1].is_empty() {
        format!("{line}\n{}", sec[1])
    } else if !sec[0].is_empty() || !sec[2].is_empty() {
        format!("{line}\n ")
    } else {
        line
    };
    join_header_footer(&sec)
}

/// csv, tsv, txt and html copies: the line goes on top of the produced file (`path`) or bytes (`base64`). Nothing is ever refused: a copy
/// that cannot be stamped is kept as it was produced and the reason goes to stderr (a documented limit of XDFU).
fn stamp_output(out: &mut Json, ext: &str, line: &str) -> Result<()> {
    if let Some(b64) = out.get("base64").and_then(Json::as_str) {
        let old = gridcraft_engine::io::base64_decode(b64).ok_or_else(|| EngineError::Internal("office.xdfu".into(), "bad base64".into()))?;
        match xdfu_core::stamp_text(&old, ext, line) {
            Ok(new) => {
                out["base64"] = json!(gridcraft_engine::io::base64_encode(&new));
                out["bytes"] = json!(new.len());
            }
            Err(e) => eprintln!("office: XDFU stamp of the {ext} download failed, the copy is left unstamped: {e}"),
        }
    } else if let Some(path) = out.get("path").and_then(Json::as_str).map(str::to_string) {
        let done = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|old| xdfu_core::stamp_text(&old, ext, line))
            .and_then(|new| std::fs::write(&path, &new).map(|()| new.len()).map_err(|e| e.to_string()));
        match done {
            Ok(n) => out["bytes"] = json!(n),
            Err(e) => eprintln!("office: XDFU stamp of {path} failed, the copy is left unstamped: {e}"),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- office.xdfu.insertGrif

fn insert_grif(s: &mut Session, p: &Json) -> Result<Json> {
    const CMD: &str = "office.xdfu.insertGrif";
    let Some(x) = XDFU.get() else { return Err(bad(CMD, "XDFU is off")) };
    let grif = match p.get("grif").and_then(Json::as_str) {
        None => "XDFU",
        Some(g) if ["XDFU", "ХДФУ", "ДСП"].contains(&g) => g,
        Some(g) => return Err(bad(CMD, &format!("`grif` must be XDFU, ХДФУ or ДСП, not `{g}`"))),
    };
    let (name, print) = {
        let d = s.doc()?;
        (d.display_title(), d.wb.sheet(0).ok_or_else(|| bad(CMD, "the workbook has no sheet"))?.print.clone())
    };
    x.issue("grif_inserted", &name, |st| {
        if st.copy == 0 {
            return Err(EngineError::Other("the XDFU audit log is unusable: no copy number".into()));
        }
        let mut pages = vec![("odd", &print.header)];
        if print.different_first {
            pages.push(("first", &print.first_header));
        }
        for (page, spec) in pages {
            // keep what the right section held, unless it is an earlier grif (then this replaces it)
            let old = codes_to_fields(&split_header_footer(spec)[2]);
            let keep = if xdfu_core::find_grif(&old).is_some() { String::new() } else { old };
            let mut text = format!("{grif}\nNusxa {}", st.copy);
            if !keep.is_empty() {
                text = format!("{text}\n{keep}");
            }
            s.execute("pageLayout.headerFooter", json!({"sheet": 0, "page": page, "part": "header", "section": "right", "text": text}))?;
        }
        Ok(json!({"grif": grif, "copy": st.copy}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_copy_that_cannot_be_stamped_is_kept_and_not_refused() {
        let dir = std::env::temp_dir().join(format!("office-stamp-output-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // no file to read
        let gone = dir.join("gone.csv");
        let mut out = json!({"path": gone.to_string_lossy(), "bytes": 3});
        assert!(stamp_output(&mut out, "csv", "XDFU copy 1").is_ok());
        assert_eq!(out["bytes"], 3);
        // an html copy with no body cannot carry the line
        let page = dir.join("p.html");
        std::fs::write(&page, "no markup").unwrap();
        let mut out = json!({"path": page.to_string_lossy(), "bytes": 9});
        assert!(stamp_output(&mut out, "html", "XDFU copy 1").is_ok());
        assert_eq!(std::fs::read_to_string(&page).unwrap(), "no markup");
        // download bytes
        let mut out = json!({"base64": gridcraft_engine::io::base64_encode(b"no markup"), "bytes": 9});
        assert!(stamp_output(&mut out, "html", "XDFU copy 1").is_ok());
        assert_eq!(out["bytes"], 9);
        // and the working case still stamps
        std::fs::write(dir.join("ok.csv"), "a,b\n").unwrap();
        let mut out = json!({"path": dir.join("ok.csv").to_string_lossy()});
        stamp_output(&mut out, "csv", "XDFU copy 1").unwrap();
        assert!(std::fs::read_to_string(dir.join("ok.csv")).unwrap().starts_with("\"XDFU copy 1\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
