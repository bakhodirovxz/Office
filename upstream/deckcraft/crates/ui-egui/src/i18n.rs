// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Interface languages. Strings in the code stay English and are the lookup keys; a catalog per language (`context<TAB>English<TAB>translation`
//! lines, the format of WordCraft and PdfCraft) maps them to display text when the UI is drawn. A string without a translation shows in English,
//! so coverage grows one catalog entry at a time. The catalogs are not part of this tree: `cargo xt i18n export` writes them next to the
//! brand overlay and `build_office.rs` embeds them, so without an overlay English is the only language (upstream's behaviour).
//!
//! Command ids, document text, file names, the control channel and the CLI never see translated text.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The `language` setting that follows the default: English, or the overlay's default language once it has one.
pub const AUTO: &str = "auto";

/// One interface language.
pub struct LangInfo {
    /// BCP 47 code, lowercase (`en`, `uz-latn`, `uz-cyrl`). Also the `language` setting's value.
    pub code: &'static str,
    /// The language's name in itself, shown in the language picker.
    pub name: &'static str,
    /// Catalog contents (empty for the built-in English).
    source: &'static str,
    catalog: OnceLock<HashMap<String, String>>,
}

/// The registry. English first: it is the fallback and the source language. Uzbek is there only when the overlay supplies its catalogs.
pub static LANGUAGES: [LangInfo; 1 + crate::office_overlay::EXTRA_LANGS] = [
    LangInfo { code: "en", name: "English", source: "", catalog: OnceLock::new() },
    #[cfg(office_uz)]
    LangInfo { code: "uz-latn", name: crate::office_overlay::NAME_UZ_LATN, source: crate::office_overlay::CATALOG_UZ_LATN, catalog: OnceLock::new() },
    #[cfg(office_uz)]
    LangInfo { code: "uz-cyrl", name: crate::office_overlay::NAME_UZ_CYRL, source: crate::office_overlay::CATALOG_UZ_CYRL, catalog: OnceLock::new() },
];

impl LangInfo {
    fn catalog(&self) -> &HashMap<String, String> {
        self.catalog.get_or_init(|| {
            let (map, errors) = parse(self.source);
            for error in errors {
                log::warn!("{} interface catalog: {error}; that entry shows in English", self.code);
            }
            map
        })
    }
}

fn unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => return Err(format!("unknown escape \\{other}")),
            None => return Err("trailing backslash".into()),
        }
    }
    Ok(out)
}

/// The `{...}` placeholders of a template, sorted (translators may reorder them).
fn placeholders(text: &str) -> Vec<&str> {
    let mut names: Vec<&str> = text.split('{').skip(1).filter_map(|rest| rest.split_once('}').map(|(name, _)| name)).collect();
    names.sort_unstable();
    names
}

/// Read a catalog. A bad line (wrong columns, unknown escape, placeholders or trailing `…` unlike the English) is skipped and reported, so a
/// bad translation never breaks the UI. Only the plain entries (empty context) are used.
fn parse(text: &str) -> (HashMap<String, String>, Vec<String>) {
    let mut map = HashMap::new();
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let (Some(context), Some(source), Some(translation), None) = (cols.next(), cols.next(), cols.next(), cols.next()) else {
            errors.push(format!("line {}: expected `context<TAB>source<TAB>translation`", index + 1));
            continue;
        };
        if !context.is_empty() {
            continue; // contexts are not used by this app yet
        }
        match (unescape(source), unescape(translation)) {
            (Ok(s), Ok(t)) if s.is_empty() || t.is_empty() => errors.push(format!("line {}: empty source or translation", index + 1)),
            (Ok(s), Ok(t)) if placeholders(&s) != placeholders(&t) || s.ends_with('…') != t.ends_with('…') => {
                errors.push(format!("line {}: placeholders or trailing ellipsis differ from the English", index + 1));
            }
            (Ok(s), Ok(t)) => {
                map.entry(s).or_insert(t);
            }
            (Err(e), _) | (_, Err(e)) => errors.push(format!("line {}: {e}", index + 1)),
        }
    }
    (map, errors)
}

/// A language the UI can be shown in.
#[derive(Clone, Copy)]
pub struct Lang(&'static LangInfo);

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lang({})", self.0.code)
    }
}

impl PartialEq for Lang {
    fn eq(&self, other: &Self) -> bool {
        self.0.code == other.0.code
    }
}

impl Lang {
    pub const EN: Lang = Lang(&LANGUAGES[0]);

    pub fn code(self) -> &'static str {
        self.0.code
    }

    pub fn name(self) -> &'static str {
        self.0.name
    }

    /// Every registered language.
    pub fn all() -> impl Iterator<Item = Lang> {
        LANGUAGES.iter().map(Lang)
    }

    /// A language by a code or locale tag: the registered code (any case), `uz` and `uz-UZ` for Latin Uzbek, `uz-Cyrl-UZ` and
    /// `uz_UZ@cyrillic` for Cyrillic Uzbek. No tag is `auto`: that follows the default ([`from_pref`]).
    pub fn from_tag(tag: &str) -> Option<Lang> {
        let lower = tag.trim().to_ascii_lowercase().replace('_', "-");
        let uz = lower == "uz" || lower.starts_with("uz-") || lower.starts_with("uz@");
        let code = if uz && (lower.contains("cyrl") || lower.contains("@cyrillic")) {
            "uz-cyrl"
        } else if uz {
            "uz-latn"
        } else {
            lower.as_str()
        };
        Lang::all().find(|l| l.code() == code)
    }
}

/// Resolve the `language` setting: a language code, or [`AUTO`] (and anything unknown or not built in) for English.
pub fn from_pref(pref: &str) -> Lang {
    Lang::from_tag(pref).unwrap_or(Lang::EN)
}

/// The canonical `language` setting for user input: [`AUTO`] or a registered code. `None` for anything else, so the caller keeps the setting.
pub fn normalize_pref(pref: &str) -> Option<&'static str> {
    if pref.eq_ignore_ascii_case(AUTO) {
        return Some(AUTO);
    }
    Lang::from_tag(pref).map(Lang::code)
}

thread_local! {
    // Per thread: parallel test harnesses must not change each other's language.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Set the language the UI is drawn in (the app does this every frame from its setting).
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

/// The language the UI is drawn in.
pub fn current() -> Lang {
    CURRENT.get()
}

/// `s` in `lang`; a string without a translation comes back unchanged. The result shows the overlay's product name.
pub fn tr(lang: Lang, s: &str) -> &str {
    crate::office_overlay::rebrand(lang.0.catalog().get(s).map_or(s, String::as_str))
}

/// `s` in the current language.
pub fn t(s: &str) -> &str {
    tr(current(), s)
}

/// Does `lang` have an entry for this string? (English never does: it is the source.)
pub fn has(lang: Lang, s: &str) -> bool {
    lang.0.catalog().contains_key(s)
}
