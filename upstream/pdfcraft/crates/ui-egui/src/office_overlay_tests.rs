// Added by the office overlay patch queue. MIT OR Apache-2.0.
//! Tests of the brand overlay. They assert the overlay's behaviour when it is active (`OFFICE_OVERLAY_DIR` set at
//! build time) and upstream's behaviour when it is not.

use crate::office_overlay as o;

#[test]
fn rebrand_follows_the_overlay() {
    if o::ACTIVE {
        assert_eq!(o::rebrand("About PdfCraft"), format!("About {}", o::app_name()));
        assert!(!o::rebrand("PdfCraft is drawing without a graphics processor").contains("Craft"));
        assert_eq!(o::rebrand("Open"), "Open");
        assert!(!o::attribution().is_empty());
    } else {
        assert_eq!(o::rebrand("About PdfCraft"), "About PdfCraft");
        assert_eq!(o::attribution(), "");
    }
}

#[test]
fn hidden_commands_follow_the_flags() {
    assert_eq!(o::hidden_command("help.discord"), !o::COMMUNITY);
    assert_eq!(o::hidden_command("help.github"), !o::UPSTREAM_LINKS);
    assert_eq!(o::hidden_command("help.check_updates"), !o::UPDATE_CHECK);
    assert!(!o::hidden_command("help.about"));
}

/// The WP8 test text: Uzbek Latin with U+02BB and U+02BC, Uzbek Cyrillic (ўқғҳ), and the ruble and numero signs.
const UZBEK_TEXT: &str = "Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №";

/// Every character of the test text is in the cmap of some face of every interface family, and the overlay's
/// fallback faces come last, so upstream's metrics are unchanged.
#[test]
fn fallback_font_covers_uzbek() {
    use skrifa::MetadataProvider as _;
    if !o::ACTIVE {
        return;
    }
    let defs = crate::theme::font_definitions();
    for (family, stack) in &defs.families {
        let last = stack.last().map(String::as_str);
        assert!(matches!(last, Some("office-fallback-sans" | "office-fallback-mono")), "{family:?} ends with {last:?}");
        for ch in UZBEK_TEXT.chars().filter(|c| !c.is_whitespace()) {
            let found = stack
                .iter()
                .filter_map(|name| defs.font_data.get(name))
                .any(|data| skrifa::FontRef::from_index(&data.font, data.index).is_ok_and(|font| font.charmap().map(ch).is_some()));
            assert!(found, "{family:?} has no glyph for {ch:?} (U+{:04X})", ch as u32);
        }
    }
}
