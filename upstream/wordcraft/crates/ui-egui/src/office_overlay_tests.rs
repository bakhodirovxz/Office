// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Tests of the brand overlay. They assert the overlay's behaviour when it is active (`OFFICE_OVERLAY_DIR` set at
//! build time) and upstream's behaviour when it is not.

use crate::office_overlay as o;

#[test]
fn rebrand_follows_the_overlay() {
    if o::ACTIVE {
        assert_eq!(o::rebrand("About WordCraft"), format!("About {}", o::app_name()));
        assert_eq!(
            o::rebrand("A free, open-source word processor.\nPart of the Crafting Apps from the ArtCraft team."),
            "A free, open-source word processor."
        );
        assert_eq!(o::rebrand("Open"), "Open");
        assert!(!o::attribution().is_empty());
    } else {
        assert_eq!(o::rebrand("About WordCraft"), "About WordCraft");
        assert_eq!(o::attribution(), "");
    }
}

#[test]
fn the_about_blurb_loses_its_upstream_sentence_through_the_catalog_lookup() {
    let blurb = crate::i18n::tr(crate::i18n::Lang::EN, "WordCraft");
    assert_eq!(blurb, if o::ACTIVE { o::app_name() } else { "WordCraft" });
}

/// The WP8 test text: Uzbek Latin with U+02BB and U+02BC, Uzbek Cyrillic (ўқғҳ), and the ruble and numero signs.
const UZBEK_TEXT: &str = "Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №";

/// Does a TrueType font's cmap (format 4 or 12) map `c`? A small reader, enough for the fallback subsets.
fn cmap_has(font: &[u8], c: char) -> Option<bool> {
    let u16at = |o: usize| font.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize);
    let u32at = |o: usize| font.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let record = (0..u16at(4)?).map(|i| 12 + i * 16).find(|&r| font.get(r..r + 4) == Some(b"cmap".as_slice()))?;
    let cmap = u32at(record + 8)?;
    let code = c as usize;
    for i in 0..u16at(cmap + 2)? {
        let sub = cmap + u32at(cmap + 4 + i * 8 + 4)?;
        match u16at(sub)? {
            4 => {
                let segs = u16at(sub + 6)? / 2;
                let (ends, starts, deltas, offsets) = (sub + 14, sub + 16 + 2 * segs, sub + 16 + 4 * segs, sub + 16 + 6 * segs);
                for s in 0..segs {
                    if code > u16at(ends + 2 * s)? {
                        continue;
                    }
                    let start = u16at(starts + 2 * s)?;
                    if code < start {
                        break;
                    }
                    let (delta, range) = (u16at(deltas + 2 * s)?, u16at(offsets + 2 * s)?);
                    let glyph = if range == 0 { (code + delta) & 0xFFFF } else { u16at(offsets + 2 * s + range + 2 * (code - start))? };
                    if glyph != 0 {
                        return Some(true);
                    }
                    break;
                }
            }
            12 => {
                for g in 0..u32at(sub + 12)? {
                    let at = sub + 16 + g * 12;
                    if (u32at(at)?..=u32at(at + 4)?).contains(&code) {
                        return Some(true);
                    }
                }
            }
            _ => {}
        }
    }
    Some(false)
}

/// The fallback faces cover every character of the test text, and they come last in every interface family,
/// so upstream's metrics are unchanged.
#[test]
fn fallback_font_covers_uzbek() {
    if !o::ACTIVE {
        return;
    }
    let defs = crate::theme::font_definitions(false, false);
    for (key, bytes) in o::FALLBACK_FONTS {
        for ch in UZBEK_TEXT.chars().filter(|c| !c.is_whitespace()) {
            assert_eq!(cmap_has(bytes, ch), Some(true), "{key} has no glyph for {ch:?} (U+{:04X})", ch as u32);
        }
    }
    for (family, stack) in &defs.families {
        let last = stack.last().map(String::as_str);
        assert!(matches!(last, Some("office-fallback-sans" | "office-fallback-mono")), "{family:?} ends with {last:?}");
    }
}

#[test]
fn uzbek_is_offered_only_with_the_overlay_catalogs() {
    use crate::i18n::{Lang, lang_from_tag, tr};
    let code = |tag: &str| lang_from_tag(tag).map(Lang::code);
    if cfg!(office_uz) {
        // Windows script tags, scriptless tags, and the Linux `@cyrillic` modifier.
        assert_eq!(code("uz-Latn-UZ"), Some("uz-latn"));
        assert_eq!(code("uz-Cyrl-UZ"), Some("uz-cyrl"));
        assert_eq!((code("uz"), code("uz-UZ"), code("uz_UZ.UTF-8")), (Some("uz-latn"), Some("uz-latn"), Some("uz-latn")));
        assert_eq!(code("uz_UZ@cyrillic"), Some("uz-cyrl"));
        let (latn, cyrl) = (Lang::from_code("uz-latn").expect("uz-latn"), Lang::from_code("uz-cyrl").expect("uz-cyrl"));
        assert!(latn.name().starts_with("O\u{2bb}zbekcha") && cyrl.name().starts_with("Ўзбекча"));
        assert_ne!(tr(latn, "Home"), "Home");
        assert_ne!(tr(cyrl, "Home"), "Home");
    } else {
        assert_eq!(code("uz-Latn-UZ"), None);
        assert!(Lang::from_code("uz-latn").is_none());
    }
}

#[test]
fn russian_gets_our_rows_and_keeps_upstreams() {
    use crate::i18n::{Lang, tr};
    let ru = Lang::from_code("ru").expect("ru");
    if o::CATALOG_RU.is_empty() {
        assert_eq!(tr(ru, "Transliterate"), "Transliterate");
        return;
    }
    assert_eq!(tr(ru, "Transliterate"), "Транслитерация");
    assert_eq!(tr(ru, "Latin to Cyrillic"), "С латиницы на кириллицу");
    // Upstream's own Russian wins: every plain, escape-free row of its file reads back unchanged.
    let mut checked = 0;
    for line in include_str!("i18n/ru.tsv").lines().filter(|l| !l.starts_with('#') && !l.contains("\\")) {
        if let [context, source, translation] = line.split('\t').collect::<Vec<_>>()[..] {
            if context.is_empty() {
                assert_eq!(tr(ru, source), o::rebrand(translation), "{source}");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked} upstream rows checked");
}
