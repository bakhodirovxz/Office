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

/// Append the fallback faces after every other face of every family so upstream's metrics stay, with one exception: the
/// sans subset goes before the CJK interface face (BIZ UDPGothic, which the stack falls back to when Inter is not
/// embedded), because that face draws Latin and Cyrillic with CJK full-width advances (absurd letter spacing in uz-Cyrl).
/// CJK text still reaches the CJK face: the subset has no CJK glyphs.
pub fn append_fallback_fonts(defs: &mut egui::FontDefinitions) {
    for (key, bytes) in FALLBACK_FONTS {
        defs.font_data.insert((*key).into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    for (family, list) in defs.families.iter_mut() {
        let mono = *family == egui::FontFamily::Monospace || matches!(family, egui::FontFamily::Name(n) if n.to_lowercase().contains("mono"));
        let key = if mono { "office-fallback-mono" } else { "office-fallback-sans" };
        if FALLBACK_FONTS.iter().any(|(k, _)| *k == key) && !list.iter().any(|n| n == key) {
            match list.iter().position(|n| !mono && n.contains("BIZ UDPGothic")) {
                Some(at) => list.insert(at, key.into()),
                None => list.push(key.into()),
            }
        }
    }
}

/// Paint the overlay's icon into `r`. False when there is no overlay icon: the caller then draws upstream's mark.
pub fn paint_logo(p: &egui::Painter, r: egui::Rect) -> bool {
    let Some(bytes) = LOGO_RGBA else { return false };
    let id = egui::Id::new("office_overlay_logo");
    let texture = p.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).unwrap_or_else(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied([LOGO_SIZE, LOGO_SIZE], bytes);
        let texture = p.ctx().load_texture("office-logo", image, egui::TextureOptions::LINEAR);
        p.ctx().data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    });
    p.image(texture.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    true
}

/// Register the sans fallback in the slide font database too (once), so slides drawn from it show U+02BB and Uzbek
/// Cyrillic: the database asks every loaded face before it gives up on a character.
pub fn register_document_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Some((_, bytes)) = FALLBACK_FONTS.first() {
            deckcraft_fonts::FontDb::global().add_font(bytes.to_vec());
        }
        use_shared_dictionaries();
    });
}

static TOOLBAR_SIMPLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Should a window start with the simplified toolbar (`office_toolbar`)? The web build always does. Elsewhere it is the shared
/// `toolbar` setting (`simple` unless `settings.toml` says `full`), read once at the first call. This crate's own tests start
/// with the upstream ribbon, which is what upstream's UI tests drive; the toolbar tests choose the mode per window.
pub fn toolbar_default() -> bool {
    if cfg!(target_arch = "wasm32") {
        return true;
    }
    #[cfg(not(any(target_arch = "wasm32", test)))]
    {
        static LOAD: std::sync::Once = std::sync::Once::new();
        LOAD.call_once(|| set_toolbar_default(suite_core::settings::toolbar_mode() == suite_core::settings::ToolbarMode::Simple));
    }
    !cfg!(test) && TOOLBAR_SIMPLE.load(std::sync::atomic::Ordering::Relaxed)
}

fn set_toolbar_default(simple: bool) {
    TOOLBAR_SIMPLE.store(simple, std::sync::atomic::Ordering::Relaxed);
}

/// The user switched: remember it for this process and store it in the shared settings, where the other apps read it at their next start.
pub(crate) fn toolbar_changed(simple: bool) {
    set_toolbar_default(simple);
    #[cfg(not(any(target_arch = "wasm32", test)))]
    {
        use suite_core::settings::{ToolbarMode, set_toolbar_mode};
        if let Err(e) = set_toolbar_mode(if simple { ToolbarMode::Simple } else { ToolbarMode::Full }) {
            log::warn!("toolbar setting not saved: {e}");
        }
    }
}

/// Where the suite's shared Hunspell dictionaries are (PLAN 5.4), best first: the folder the `spell_dictionaries` setting names,
/// `dictionaries/` next to the exe (the installer puts them there once, for all apps), then `assets/dictionaries` of the repository
/// above the exe (a development build). Only folders that exist.
pub fn dictionary_dirs(setting: &str, exe_dir: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if !setting.trim().is_empty() {
        dirs.push(std::path::PathBuf::from(setting.trim()));
    }
    if let Some(exe) = exe_dir {
        dirs.push(exe.join("dictionaries"));
        dirs.extend(exe.ancestors().skip(1).take(6).map(|a| a.join("assets/dictionaries")));
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

/// Point the spell checker at the suite's dictionaries once. The web build has none (no folders) and the tests keep the checker's own.
fn use_shared_dictionaries() {
    #[cfg(not(any(target_arch = "wasm32", test)))]
    {
        let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(std::path::Path::to_path_buf));
        for dir in dictionary_dirs(&suite_core::settings::load().spell_dictionaries, exe.as_deref()) {
            deckcraft_engine::spell::add_search_dir(dir);
        }
    }
}

/// The shared `lang` setting (`settings.toml`, PLAN 5.1) as this app's language code (`en`, `ru`, `uz-latn`, `uz-cyrl`), read once.
/// `None` for `auto`, with no file, in the web build and in tests: the app keeps its own detection there. `uz-Latn-2026` has no
/// catalog in the apps yet (STATUS, deferred) and shows the `uz-latn` one.
#[cfg(not(any(target_arch = "wasm32", test)))]
pub fn shared_lang_code() -> Option<&'static str> {
    static CODE: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *CODE.get_or_init(|| code_of(suite_core::settings::load().lang))
}

#[cfg(any(target_arch = "wasm32", test))]
pub fn shared_lang_code() -> Option<&'static str> {
    None
}

/// The `language` setting at start: a fixed shared `lang` always wins over the app's saved one, `auto` (`None`) keeps it. A change
/// made in the app afterwards holds for the session only; the shell is the one writer of `settings.toml`.
pub fn start_language(own: &str) -> String {
    language_at_start(own, shared_lang_code())
}

pub(crate) fn language_at_start(own: &str, shared: Option<&str>) -> String {
    shared.and_then(crate::i18n::normalize_pref).map_or_else(|| own.to_string(), str::to_string)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn code_of(choice: suite_core::lang::LangChoice) -> Option<&'static str> {
    use suite_core::lang::{Lang, LangChoice};
    match choice {
        LangChoice::Auto => None,
        LangChoice::Fixed(Lang::En) => Some("en"),
        LangChoice::Fixed(Lang::Ru) => Some("ru"),
        LangChoice::Fixed(Lang::UzLatn | Lang::UzLatn2026) => Some("uz-latn"),
        LangChoice::Fixed(Lang::UzCyrl) => Some("uz-cyrl"),
    }
}
