// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Tests of the brand overlay. They assert the overlay's behaviour when it is active (`OFFICE_OVERLAY_DIR` set at
//! build time) and upstream's behaviour when it is not.

use crate::i18n::Language;
use crate::office_overlay as o;

#[test]
fn rebrand_follows_the_overlay() {
    if o::ACTIVE {
        assert_eq!(o::rebrand("About GridCraft"), format!("About {}", o::app_name()));
        assert_eq!(o::rebrand("A spreadsheet. Part of ArtCraft."), "A spreadsheet.");
        assert_eq!(o::rebrand("Open"), "Open");
        assert!(!o::attribution().is_empty());
    } else {
        assert_eq!(o::rebrand("About GridCraft"), "About GridCraft");
        assert_eq!(o::attribution(), "");
    }
}

#[test]
fn the_label_lookup_shows_the_overlay_name() {
    let label = Language::En.tr("About\nGridCraft");
    assert_eq!(label, if o::ACTIVE { format!("About\n{}", o::app_name()) } else { "About\nGridCraft".to_string() });
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
    let defs = crate::theme::font_definitions_for_language(Language::En);
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

/// A sheet's text reaches the PDF export with a face that covers it (U+02BB included), also with no installed fonts.
#[test]
fn pdf_default_face_covers_uzbek() {
    if !o::ACTIVE {
        return;
    }
    let _ = crate::theme::font_definitions_for_language(Language::En);
    let font = gridcraft_engine::pdf_fonts::for_text(gridcraft_engine::pdf_fonts::resolve("Arial", false, false), UZBEK_TEXT);
    assert!(font.covers(UZBEK_TEXT));
}
