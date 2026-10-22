//! `suite-core`: the services the four apps and the desktop host share (PLAN phase 4, section 5). No GUI, no unsafe code, no network.
//!
//! - [`settings`]: the per-user `settings.toml` (typed keys, atomic writes, a corrupt file is set aside, never fatal).
//! - [`lang`]: the five UI languages and how `lang = "auto"` resolves.
//! - [`recent`]: the recent files, read from the four apps' own lists (they stay the owners; nothing is written to them).
//! - [`fonts`]: where the shared Noto fallback fonts are.
//! - [`templates`]: the catalogue of the starter documents (`web/shell/templates.json`).
//!
//! Every name comes from `brand/brand.toml` at build time (`build.rs`); none is written here.
#![forbid(unsafe_code)]

pub mod fonts;
pub mod lang;
pub mod recent;
pub mod settings;
pub mod templates;

/// The product name from `brand/brand.toml`; the folder under the user's settings base is named after it.
pub const PRODUCT: &str = env!("SUITE_PRODUCT");

/// The base folder for per-user settings: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, else
/// `$XDG_CONFIG_HOME` or `~/.config`.
pub(crate) fn config_base() -> Option<std::path::PathBuf> {
    use std::{env::var_os, path::PathBuf};
    let dir = |k: &str| var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        dir("APPDATA")
    } else if cfg!(target_os = "macos") {
        dir("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        dir("XDG_CONFIG_HOME").or_else(|| dir("HOME").map(|h| h.join(".config")))
    }
}
