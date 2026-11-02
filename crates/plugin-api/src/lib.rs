//! `plugin-api`: the plugin API v1 (PLAN phase 7, section 4 and 5.2). Pure data and codecs, no I/O, no unsafe code.
//!
//! - [`manifest`]: `plugin.toml` types and every refusal of section 4.1; the package entry rules.
//! - [`wire`]: the message envelope (section 4.2) and its limits.
//! - [`textmap`]: the bulk text read and apply blobs (section 4.4).
//! - [`action`]: calls, answers, actions and widgets (section 4.3) and the per-action permission check.
//! - [`allow`]: the command allowlist as data (section 5.2).
//!
//! The same code runs in the native host, in the editor wasm and (through the SDK) in plugins.
#![forbid(unsafe_code)]

pub mod action;
pub mod allow;
pub mod manifest;
pub mod textmap;
pub mod wire;

/// API majors this host runs (section 4.7). A change of meaning or a removal is a new major, run side by side for a release.
pub const HOST_APIS: &[u32] = &[1];
/// Additions inside the major (a new action, context field, widget or event); sent as `api_minor` in every call.
pub const API_MINOR: u32 = 0;

/// Does this host run plugins built for `api`?
pub fn supports(api: u32) -> bool {
    HOST_APIS.contains(&api)
}
