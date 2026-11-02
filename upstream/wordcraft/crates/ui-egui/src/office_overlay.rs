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

/// What the domain's extension crate (docs-office) hooks into the ribbon, set once at start-up by the apps behind `office-ext`:
/// a per-frame callback (interface language code, catalog lookup) and a painter for the icons upstream has none of.
type FrameHook = fn(&mut wordcraft_engine::Session, &str, &dyn Fn(&str) -> String);
type IconHook = fn(&egui::Painter, egui::Rect, &str, egui::Color32) -> bool;
static EXT: std::sync::OnceLock<(FrameHook, IconHook)> = std::sync::OnceLock::new();

pub fn set_ext(frame: FrameHook, icon: IconHook) {
    let _ = EXT.set((frame, icon));
}

/// Called by the ribbon every frame; a no-op without an extension.
pub fn ribbon_frame(s: &mut wordcraft_engine::Session) {
    if let Some((frame, _)) = EXT.get() {
        frame(s, &lang_code(), &|k| crate::i18n::t(k).to_string());
    }
}

/// A print job's PDF: `(bytes, plan)` of `Session::print_pdf`.
pub type PrintJob = Result<(Vec<u8>, wordcraft_engine::print::PrintPlan), wordcraft_engine::CmdError>;
type PrintHook = fn(&mut wordcraft_engine::Session, &mut dyn FnMut(&mut wordcraft_engine::Session) -> PrintJob) -> PrintJob;
static PRINT: std::sync::OnceLock<PrintHook> = std::sync::OnceLock::new();

/// The extension sees every PDF that goes to a physical printer (XDFU watermark and audit line), like it sees an export.
pub fn set_print_hook(h: PrintHook) {
    let _ = PRINT.set(h);
}

/// `Session::print_pdf`, through the extension's print hook when there is one.
pub fn print_pdf(s: &mut wordcraft_engine::Session, params: &serde_json::Value) -> PrintJob {
    let mut run = |s: &mut wordcraft_engine::Session| s.print_pdf(params);
    match PRINT.get() {
        Some(h) => h(s, &mut run),
        None => run(s),
    }
}

/// The phone layout (wordcraft/0140): the extension decides per frame whether the window is a phone (`active`) and then draws the top row,
/// the contextual bottom toolbar and its sheets (`chrome`), which return the commands to run. Only the web app sets it.
type PhoneActive = fn(&egui::Context) -> bool;
type PhoneIcon<'a> = &'a dyn Fn(&egui::Painter, egui::Rect, &str, egui::Color32);
type PhoneChrome = fn(
    &mut wordcraft_engine::Session,
    &mut egui::Ui,
    &str,
    &[egui::Color32; 8],
    &dyn Fn(&str) -> String,
    PhoneIcon,
) -> Vec<(String, serde_json::Value)>;
static PHONE: std::sync::OnceLock<(PhoneActive, PhoneChrome)> = std::sync::OnceLock::new();
static PHONE_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CHROME_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PHONE_H: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// The view before phone mode took the Web Layout: restored when the window is a desktop window again.
static PHONE_VIEW: Mutex<Option<(wordcraft_engine::layout::ViewMode, bool)>> = Mutex::new(None);

pub fn set_phone(active: PhoneActive, chrome: PhoneChrome) {
    let _ = PHONE.set((active, chrome));
}

/// Whether the phone layout was drawn this frame (the selection mini toolbar yields to the touch action bar there).
pub fn phone_on() -> bool {
    PHONE_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draw the phone chrome instead of the title bar and the ribbon; false (nothing drawn) for a desktop window. Entering the phone layout
/// shows the document in Web Layout (it reflows to the width); leaving restores the view the user had.
pub fn phone_chrome(app: &mut crate::WordApp, ui: &mut egui::Ui) -> bool {
    let Some((active, chrome)) = PHONE.get() else { return false };
    let on = active(ui.ctx());
    if on != PHONE_ON.swap(on, std::sync::atomic::Ordering::Relaxed) {
        use wordcraft_engine::layout::ViewMode;
        let before = (app.session.view.mode, app.session.view.read_mode);
        let was = PHONE_VIEW.lock().unwrap_or_else(|e| e.into_inner()).take();
        let go = if on {
            *PHONE_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(before);
            "view.webLayout"
        } else {
            match was {
                Some((_, true)) => "view.readMode",
                Some((ViewMode::Web, _)) => "view.webLayout",
                Some((ViewMode::Draft, _)) => "view.draft",
                _ => "view.printLayout",
            }
        };
        let _ = app.session.run(go, &serde_json::json!({}));
    }
    if !on {
        return false;
    }
    // The soft keyboard opening shrinks the window (`interactive-widget=resizes-content`): keep the caret's line in view (PLAN 5.3).
    let h = ui.ctx().content_rect().height();
    if f32::from_bits(PHONE_H.swap(h.to_bits(), std::sync::atomic::Ordering::Relaxed)) > h + 2.0 {
        app.canvas.scroll_to_caret = true;
    }
    let t = crate::theme::Tokens::get(ui.ctx());
    let colors = [t.ribbon, t.border, t.text, t.text_dim, t.accent, t.on_accent, t.pressed, t.menu];
    let icon = |p: &egui::Painter, r: egui::Rect, name: &str, c: egui::Color32| crate::icons::paint(p, r, name, c, t.accent);
    let title = app.title_stem();
    for (id, params) in chrome(&mut app.session, ui, &title, &colors, &|k| crate::i18n::t(k).to_string(), &icon) {
        let _ = app.run(&id, params);
    }
    // A finger that went down on a bar is the bar's until it lifts: egui hit-tests the page widget under it (the page would put the caret at the end of the
    // document), so `canvas::mouse` stands aside while this is set.
    let page = ui.available_rect_before_wrap();
    let (pressed_on_bar, active) = ui.input(|i| {
        let on_bar = i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { pos, pressed: true, .. } if !page.contains(*pos)));
        (on_bar, i.pointer.any_down() || i.pointer.any_released())
    });
    let before = CHROME_DOWN.load(std::sync::atomic::Ordering::Relaxed);
    CHROME_DOWN.store(pressed_on_bar || (before && active), std::sync::atomic::Ordering::Relaxed);
    true
}

/// A finger is down on the phone chrome (see `phone_chrome`).
pub fn chrome_down() -> bool {
    CHROME_DOWN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draw icon `name` if the extension has one; false lets upstream draw it.
pub fn paint_icon(p: &egui::Painter, r: egui::Rect, name: &str, c: egui::Color32) -> bool {
    EXT.get().is_some_and(|(_, icon)| icon(p, r, name, c))
}

/// Upstream's placeholder user name (`Session::new`).
pub const PLACEHOLDER_AUTHOR: &str = "WordCraft User";

/// office overlay (wordcraft/0137, WP23 finding): upstream's placeholder user name is not shown or written into files: the
/// system account's name replaces it until Options sets one, as Word takes the Windows user's. Not in tests; on the web there is
/// no account name, so the placeholder stays there.
pub fn replace_placeholder_author(author: &mut String) {
    #[cfg(not(any(target_arch = "wasm32", test)))]
    if author == PLACEHOLDER_AUTHOR
        && let Some(name) = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok().map(|n| n.trim().to_string()).filter(|n| !n.is_empty())
    {
        *author = name;
    }
    #[cfg(any(target_arch = "wasm32", test))]
    let _ = author;
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

/// The shared `theme` setting (`settings.toml`: `system`, `light` or `dark`; owner 2026-10-29). `None` in the web build and in tests:
/// the app keeps its own choice there.
#[cfg(not(any(target_arch = "wasm32", test)))]
pub fn shared_theme_code() -> Option<&'static str> {
    Some(suite_core::settings::load().theme.tag())
}

#[cfg(any(target_arch = "wasm32", test))]
pub fn shared_theme_code() -> Option<&'static str> {
    None
}

/// The interface theme at start (patches/wordcraft/0132): the shared `theme` always wins over the one saved in `ui.json`, `system`
/// included, so the shell is the one place to change it. A change made in the app afterwards holds for the session only.
pub fn start_theme(own: crate::theme::Appearance) -> crate::theme::Appearance {
    theme_at_start(own, shared_theme_code())
}

pub fn theme_at_start(own: crate::theme::Appearance, shared: Option<&str>) -> crate::theme::Appearance {
    shared.and_then(crate::theme::Appearance::from_code).unwrap_or(own)
}
