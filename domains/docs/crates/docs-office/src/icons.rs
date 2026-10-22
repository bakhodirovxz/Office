//! Fluent icons for the buttons that have none of upstream's (and the ones on our "Document" tab, so the tab reads as one set).
//!
//! The SVG loader (`egui_extras[svg]`, resvg) adds about 309 KB brotli to the wasm and the Docs budget has under 20 KB
//! (docs/phase4/icons-wasm.md), so the icons are rasterised at build time into 8-bit alpha masks (`assets/icons.bin`,
//! `scripts/gen-docs-icon-bitmaps.sh`) and drawn as tinted textures. Source: Fluent UI System Icons, MIT (assets/icons/fluent).

use std::sync::OnceLock;

use egui::{Color32, ColorImage, Painter, Rect, TextureHandle, TextureOptions, pos2};

const BIN: &[u8] = include_bytes!("../assets/icons.bin");
/// The raster sizes in `icons.bin`, in pixels, per icon in this order.
const SIZES: [usize; 3] = [20, 28, 40];

/// The icon name upstream derives from a command id (its last segment, `widgets::command_icon`) and the Fluent icon drawn for it.
pub const MAP: [(&str, &str); 11] = [
    ("print", "print"),
    ("exportPdf", "document_pdf"),
    ("orientation", "orientation"),
    ("pageSetup", "document_landscape"),
    ("spelling", "text_grammar_checkmark"),
    ("translitCyrl", "translate"),
    ("translitLatn", "translate"),
    ("translitAuto", "translate_auto"),
    ("history", "history"),
    ("ribbonFull", "window_header_horizontal"),
    ("ribbonSimple", "window_header_horizontal_off"),
];

type Masks = [&'static [u8]; SIZES.len()];

/// `icons.bin`: per icon `name\0` and then the masks of every size in `SIZES`.
fn table() -> &'static [(&'static str, Masks)] {
    static T: OnceLock<Vec<(&'static str, Masks)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = Vec::new();
        let mut rest = BIN;
        while let Some(z) = rest.iter().position(|b| *b == 0) {
            let name = std::str::from_utf8(&rest[..z]).unwrap_or("");
            rest = &rest[z + 1..];
            let mut masks: Masks = [&[]; SIZES.len()];
            for (m, s) in masks.iter_mut().zip(SIZES) {
                let (a, b) = rest.split_at(s * s);
                *m = a;
                rest = b;
            }
            out.push((name, masks));
        }
        out
    })
}

pub fn fluent_name(name: &str) -> Option<&'static str> {
    MAP.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}

/// Draw icon `name` into `r` in colour `c`. False when it is not one of ours (the caller draws upstream's).
pub fn paint_icon(p: &Painter, r: Rect, name: &str, c: Color32) -> bool {
    let Some(fluent) = fluent_name(name) else { return false };
    let Some((_, masks)) = table().iter().find(|(n, _)| *n == fluent) else { return false };
    let side = r.width().min(r.height());
    let px = (side * p.ctx().pixels_per_point()).ceil() as usize;
    let i = SIZES.iter().position(|s| *s >= px).unwrap_or(SIZES.len() - 1);
    let (size, mask) = (SIZES[i], masks[i]);
    let id = egui::Id::new(("docs-office-icon", fluent, size));
    let ctx = p.ctx();
    let tex = match ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        Some(t) => t,
        None => {
            let rgba: Vec<u8> = mask.iter().flat_map(|a| [255, 255, 255, *a]).collect();
            let t = ctx.load_texture(format!("docs-office/{fluent}/{size}"), ColorImage::from_rgba_unmultiplied([size, size], &rgba), TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };
    let rect = Rect::from_center_size(r.center(), egui::vec2(side, side));
    p.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), c);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mapped_icon_has_its_masks() {
        for (_, f) in MAP {
            let (_, masks) = table().iter().find(|(n, _)| *n == f).unwrap_or_else(|| panic!("`{f}` is not in icons.bin"));
            assert!(masks.iter().zip(SIZES).all(|(m, s)| m.len() == s * s));
            assert!(masks[0].iter().any(|a| *a > 128), "`{f}` is empty");
        }
        assert_eq!(table().len(), 10);
    }
}
