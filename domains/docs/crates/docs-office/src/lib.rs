//! The simplified Docs ribbon, its simple/full toggle and the command-hook seam (Phase 4 WP2 and WP4; docs/phase4/toolbar-design.md).
//!
//! `install(&mut Session)` appends the commands this layer owns and re-registers the guarded ones with wrappers. The apps call it
//! through the carried hook `patches/wordcraft/0120` and `0122`; the ribbon itself is upstream's Customize Ribbon, set to a
//! preset by `ribbon_frame` (called once per frame by the carried hook, cheap when nothing changed).

use serde_json::{Value, json};
use wordcraft_engine::{CmdResult, CommandSpec, Registry, Session};

pub mod hooks;
pub mod icons;
pub mod levels;
pub mod mode;
pub mod preset;
#[cfg(not(target_arch = "wasm32"))]
mod settings;

pub use icons::paint_icon;
pub use mode::Mode;
pub use preset::{FULL, SIMPLE, sync as ribbon_frame};

pub const EXPORT_PDF: &str = "office.exportPdf";
pub const HISTORY: &str = "office.history";

fn full(s: &mut Session, _: &Value) -> CmdResult {
    preset::to_full(s);
    Ok(json!({"toolbar": "full"}))
}

fn simple(s: &mut Session, _: &Value) -> CmdResult {
    preset::to_simple(s);
    Ok(json!({"toolbar": "simple"}))
}

fn desktop_only(_: &Session) -> Option<&'static str> {
    (!mode::toggle_available()).then_some("The web version always shows the simple ribbon")
}

/// File > Print draws the "save a copy" formats, PDF first, below the print settings (design 3.4 item 2). Upstream's own
/// `file.exportPdf {}` opens a save dialog instead; switching to it is a one-line change here.
fn export_pdf(s: &mut Session, _: &Value) -> CmdResult {
    s.ui_requests.push(json!({"open": "print"}));
    Ok(json!({"exported": false}))
}

/// Opens the shell's History panel (WP10 fills it in). Until then the request is one the UI ignores.
fn history(s: &mut Session, _: &Value) -> CmdResult {
    s.ui_requests.push(json!({"open": "history"}));
    Ok(json!({"opened": false}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(FULL, "Full ribbon", "Quick Access Toolbar", full).pure().when(desktop_only),
        CommandSpec::new(SIMPLE, "Simple ribbon", "Quick Access Toolbar", simple).pure().when(desktop_only),
        CommandSpec::new(EXPORT_PDF, "Export PDF", "File › Export", export_pdf).pure(),
        CommandSpec::new(HISTORY, "Version History", "File › Info", history).pure(),
    ]
}

/// The registry with our commands added and the guarded ones wrapped. Safe to call twice.
pub fn extend(reg: &Registry) -> Option<Registry> {
    if reg.get(FULL).is_some() {
        return None;
    }
    let mut all = reg.all().to_vec();
    all.extend(specs());
    all.extend(hooks::wrapped(reg));
    all.extend(levels::wrapped(reg));
    Some(Registry::new(all))
}

/// Add the commands to `s.registry`. Safe to call twice.
pub fn install(s: &mut Session) {
    #[cfg(not(target_arch = "wasm32"))]
    mode::default_store(settings::store());
    if let Some(r) = extend(&s.registry) {
        s.registry = std::sync::Arc::new(r);
    }
}


#[cfg(test)]
mod tests;
