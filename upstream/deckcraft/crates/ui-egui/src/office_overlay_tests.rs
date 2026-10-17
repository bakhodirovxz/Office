// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Tests of the brand overlay. They assert the overlay's behaviour when it is active (`OFFICE_OVERLAY_DIR` set at
//! build time) and upstream's behaviour when it is not.

use crate::office_overlay as o;

#[test]
fn rebrand_follows_the_overlay() {
    if o::ACTIVE {
        assert_eq!(o::rebrand("About DeckCraft"), format!("About {}", o::app_name()));
        assert_eq!(o::rebrand("Slides. Part of ArtCraft."), "Slides.");
        assert_eq!(o::rebrand("Open"), "Open");
        assert!(!o::attribution().is_empty());
    } else {
        assert_eq!(o::rebrand("About DeckCraft"), "About DeckCraft");
        assert_eq!(o::attribution(), "");
    }
}

#[test]
fn the_about_dialog_shows_the_product_version() {
    assert!(crate::dialogs::version_line().starts_with(&format!("Version {}", o::VERSION)));
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
    let defs = crate::theme::font_definitions(&[]);
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

/// Slides drawn from the font database find a face for U+02BB, U+02BC and Uzbek Cyrillic.
#[test]
fn slide_fonts_cover_okina() {
    if !o::ACTIVE {
        return;
    }
    o::register_document_fonts();
    for c in ['\u{02BB}', '\u{02BC}', 'ў', 'қ', 'ғ', 'ҳ', '₽'] {
        assert!(deckcraft_fonts::FontDb::global().fallback_for(c, 0).is_some(), "no face for {c:?}");
    }
}
