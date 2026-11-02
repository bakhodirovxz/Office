//! The simplified Docs ribbon, its simple/full toggle and the command-hook seam (Phase 4 WP2 and WP4; docs/phase4/toolbar-design.md).
//!
//! `install(&mut Session)` appends the commands this layer owns and re-registers the guarded ones with wrappers. The apps call it
//! through the carried hook `patches/wordcraft/0120` and `0122`; the ribbon itself is upstream's Customize Ribbon, set to a
//! preset by `ribbon_frame` (called once per frame by the carried hook, cheap when nothing changed).

use serde_json::{Value, json};
use wordcraft_engine::{CmdResult, CommandSpec, Registry, Session};

pub mod hooks;
pub mod icons;
#[cfg(not(target_arch = "wasm32"))]
pub mod layer;
pub mod levels;
pub mod mode;
pub mod phone;
pub mod preset;
#[cfg(not(target_arch = "wasm32"))]
mod settings;

pub use icons::paint_icon;
pub use mode::Mode;
#[cfg(not(target_arch = "wasm32"))]
pub use layer::print_wrap;
#[cfg(not(target_arch = "wasm32"))]
pub use layer::flush_snapshots;
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

/// Upstream's `file.exportPdf {}` (no path) opens the PDF save dialog. Run through the registry, so the guarded wrapper
/// (`before`: a policy can refuse; `after`: the XDFU watermark, the snapshot) sees it.
fn export_pdf(s: &mut Session, _: &Value) -> CmdResult {
    s.run("file.exportPdf", &json!({}))
}

/// Opens the shell's History panel for the document's file (desktop). The web build has no panel: the request is one the UI ignores.
fn history(s: &mut Session, v: &Value) -> CmdResult {
    #[cfg(not(target_arch = "wasm32"))]
    return layer::open_history(s, v);
    #[cfg(target_arch = "wasm32")]
    {
        let _ = v;
        s.ui_requests.push(json!({"open": "history"}));
        Ok(json!({"opened": false}))
    }
}

/// Upstream's Version History keeps copies in memory (`save`, `restore`). Without those parameters the Backstage entry opens our
/// on-disk History instead, so staff see one feature; an unsaved document falls back to upstream's list.
static VERSIONS: std::sync::OnceLock<fn(&mut Session, &Value) -> CmdResult> = std::sync::OnceLock::new();

fn versions(s: &mut Session, v: &Value) -> CmdResult {
    let orig = VERSIONS.get().copied().expect("recorded by extend");
    #[cfg(not(target_arch = "wasm32"))]
    if s.path.is_some() && v.get("save").is_none() && v.get("restore").is_none() {
        return layer::open_history(s, v);
    }
    orig(s, v)
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
    #[cfg(not(target_arch = "wasm32"))]
    all.extend(layer::specs());
    if let Some(c) = reg.get("file.versions") {
        VERSIONS.get_or_init(|| c.run);
        all.push(CommandSpec { run: versions, ..c.clone() });
    }
    all.extend(hooks::wrapped(reg));
    all.extend(levels::wrapped(reg));
    Some(Registry::new(all))
}

/// Add the commands to `s.registry`. Safe to call twice.
pub fn install(s: &mut Session) {
    #[cfg(not(target_arch = "wasm32"))]
    mode::default_store(settings::store());
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    layer::init();
    if let Some(r) = extend(&s.registry) {
        s.registry = std::sync::Arc::new(r);
    }
}


#[cfg(test)]
mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod layer_tests;
