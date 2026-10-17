// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! The brand overlay: the product name, attribution, icon, chrome flags and fallback fonts that
//! `cargo xt overlay` renders from `brand/brand.toml`. Without `OFFICE_OVERLAY_DIR` at build time
//! every value is upstream's own, so a build without an overlay behaves like upstream.

include!(concat!(env!("OUT_DIR"), "/office_overlay_gen.rs"));

use std::collections::BTreeMap;
use std::sync::Mutex;

/// The interface language code, lowercase (`en`, `ru`, `uz-latn`, ...), as the overlay keys it.
fn lang_code() -> String {
    (crate::i18n::current().code()).to_ascii_lowercase()
}

fn by_lang(table: &'static [(&'static str, &'static str)], fallback: &'static str) -> &'static str {
    let code = lang_code();
    let find = |c: &str| table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v);
    find(&code).or_else(|| find("en")).unwrap_or(fallback)
}

/// The app's display name in the current interface language.
pub fn app_name() -> &'static str {
    by_lang(APP_NAMES, APP_NAME)
}

/// The "based on" line in the current interface language (empty without an overlay).
pub fn attribution() -> &'static str {
    by_lang(ATTRIBUTION, "")
}

/// Upstream sentences that advertise upstream's community; `rebrand` removes them.
const REMOVED: &[&str] = &["Part of the Crafting Apps from the ArtCraft team.", "Part of ArtCraft."];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replace whole-token occurrences of `from` in `s`.
fn replace_token(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len() + to.len());
    let mut rest = s;
    while let Some(i) = rest.find(from) {
        let (head, tail) = rest.split_at(i);
        let after = &tail[from.len()..];
        let before_ok = head.chars().next_back().is_none_or(|c| !is_word_char(c));
        let after_ok = after.chars().next().is_none_or(|c| !is_word_char(c));
        out.push_str(head);
        out.push_str(if before_ok && after_ok { to } else { from });
        rest = after;
    }
    out.push_str(rest);
    out
}

static INTERNED: Mutex<BTreeMap<String, &'static str>> = Mutex::new(BTreeMap::new());

/// Show `s` under the overlay's name: every upstream name token becomes `app_name()` and the upstream community
/// sentences are dropped. Returns `s` itself when there is no overlay or nothing matches. A changed string is
/// interned, so the cache holds one entry per distinct (language, source) pair.
pub fn rebrand(s: &str) -> &str {
    if !ACTIVE || UPSTREAM_NAMES.is_empty() || !(s.contains("Craft") || s.contains("Discord")) {
        return s;
    }
    let key = format!("{}\u{1f}{s}", lang_code());
    let mut cache = INTERNED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(done) = cache.get(&key) {
        return *done;
    }
    let mut out = s.to_string();
    for sentence in REMOVED {
        out = out.replace(sentence, "");
    }
    for name in UPSTREAM_NAMES {
        out = replace_token(&out, name, app_name());
    }
    if out.contains('\n') && out.contains("ArtCraft") {
        // A translation of an upstream "Part of ..." sentence: drop the lines that still name upstream's publisher.
        out = out.lines().filter(|l| !l.contains("ArtCraft")).collect::<Vec<_>>().join("\n");
    }
    let out = out.trim().to_string();
    debug_assert!(cache.len() < 4096, "rebrand cache is bounded by the catalog size");
    let leaked: &'static str = Box::leak(out.into_boxed_str());
    cache.insert(key, leaked);
    leaked
}

/// The window icon (PNG bytes) when an overlay is active; callers keep upstream's icon on `None`.
pub fn icon_png() -> Option<&'static [u8]> {
    ICON_PNG
}

/// Append the fallback faces after every other face of every family, never first, so upstream's metrics stay.
pub fn append_fallback_fonts(defs: &mut egui::FontDefinitions) {
    for (key, bytes) in FALLBACK_FONTS {
        defs.font_data.insert((*key).into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    for (family, list) in defs.families.iter_mut() {
        let key = if *family == egui::FontFamily::Monospace { "office-fallback-mono" } else { "office-fallback-sans" };
        if FALLBACK_FONTS.iter().any(|(k, _)| *k == key) && !list.iter().any(|n| n == key) {
            list.push(key.into());
        }
    }
}

/// The overlay icon drawn `size` points square (an empty response when there is no overlay icon).
pub fn logo(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let Some(bytes) = LOGO_RGBA else { return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover()) };
    let id = egui::Id::new("office_overlay_logo");
    let texture = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).unwrap_or_else(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied([LOGO_SIZE, LOGO_SIZE], bytes);
        let texture = ui.ctx().load_texture("office-logo", image, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    });
    ui.add(egui::Image::new((texture.id(), egui::vec2(size, size))).alt_text(APP_NAME))
}

/// Registry commands that the overlay hides (menus and palette) and disables (dispatch): upstream's community
/// links and its update check.
pub fn hidden_command(id: &str) -> bool {
    match id {
        "help.discord" => !COMMUNITY,
        "help.app_page" | "help.github" | "help.website" => !UPSTREAM_LINKS,
        "help.check_updates" => !UPDATE_CHECK,
        _ => false,
    }
}
