//! Placeholder icon generator: a rounded square in the accent colour with a white pictogram, drawn from one vector
//! description at every size (strokes are pixel-snapped at 32 px and below). The art is ours, MIT OR Apache-2.0.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

/// Sizes written as `icon-<N>.png` (brand.md section 5).
pub const PNG_SIZES: [u32; 11] = [16, 20, 24, 32, 40, 48, 64, 128, 256, 512, 1024];
/// Sizes embedded in `.ico` files (BMP below 64 px, PNG from 64 px).
pub const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pictogram {
    Lines,
    Grid,
    Slide,
    Page,
}

impl Pictogram {
    pub fn parse(s: &str) -> Result<Pictogram, String> {
        match s {
            "lines" => Ok(Pictogram::Lines),
            "grid" => Ok(Pictogram::Grid),
            "slide" => Ok(Pictogram::Slide),
            "page" => Ok(Pictogram::Page),
            _ => Err(format!("unknown pictogram '{s}' (lines|grid|slide|page)")),
        }
    }
}

fn paint(c: [u8; 3]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], 255);
    p.anti_alias = true;
    p
}

struct Canvas<'a> {
    pm: &'a mut Pixmap,
    size: f32,
    /// Pictogram scale about the centre (1.0 normal, less for the maskable safe zone).
    scale: f32,
    snap: bool,
}

impl Canvas<'_> {
    fn map(&self, v: f32) -> f32 {
        let v = 0.5 + (v - 0.5) * self.scale;
        let px = v * self.size;
        if self.snap { px.round() } else { px }
    }

    /// Fill a rectangle given in unit coordinates (at least one pixel thick when snapped).
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3]) {
        let (a, b, mut cx, mut d) = (self.map(x0), self.map(y0), self.map(x1), self.map(y1));
        if self.snap {
            cx = cx.max(a + 1.0);
            d = d.max(b + 1.0);
        }
        if let Some(r) = Rect::from_ltrb(a, b, cx, d) {
            self.pm.fill_rect(r, &paint(c), Transform::identity(), None);
        }
    }

    fn polygon(&mut self, pts: &[(f32, f32)], c: [u8; 3]) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let (px, py) = (self.map(*x), self.map(*y));
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            self.pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
        }
    }

    fn pictogram(&mut self, p: Pictogram, fg: [u8; 3], bg: [u8; 3]) {
        match p {
            Pictogram::Lines => {
                for (i, right) in [0.76f32, 0.76, 0.76, 0.56].into_iter().enumerate() {
                    let y = 0.27 + 0.15 * i as f32;
                    self.rect(0.24, y, right, y + 0.08, fg);
                }
            }
            Pictogram::Grid if self.snap => {
                // Small sizes: whole-pixel cells with a one-pixel gap, centred.
                let cell = (((self.size * self.scale * 0.52) as u32).saturating_sub(2) / 3).max(1) as f32;
                let span = cell * 3.0 + 2.0;
                let start = ((self.size - span) / 2.0).floor();
                for r in 0..3 {
                    for c in 0..3 {
                        let (x, y) = (start + (cell + 1.0) * c as f32, start + (cell + 1.0) * r as f32);
                        if let Some(rc) = Rect::from_ltrb(x, y, x + cell, y + cell) {
                            self.pm.fill_rect(rc, &paint(fg), Transform::identity(), None);
                        }
                    }
                }
            }
            Pictogram::Grid => {
                for r in 0..3 {
                    for c in 0..3 {
                        let (x, y) = (0.24 + 0.18 * c as f32, 0.24 + 0.18 * r as f32);
                        self.rect(x, y, x + 0.16, y + 0.16, fg);
                    }
                }
            }
            Pictogram::Slide => {
                // A screen outline and a stand.
                self.rect(0.20, 0.26, 0.80, 0.34, fg);
                self.rect(0.20, 0.58, 0.80, 0.66, fg);
                self.rect(0.20, 0.34, 0.28, 0.58, fg);
                self.rect(0.72, 0.34, 0.80, 0.58, fg);
                self.rect(0.46, 0.66, 0.54, 0.76, fg);
                self.rect(0.34, 0.76, 0.66, 0.82, fg);
            }
            Pictogram::Page => {
                self.polygon(&[(0.30, 0.20), (0.58, 0.20), (0.72, 0.34), (0.72, 0.80), (0.30, 0.80)], fg);
                self.polygon(&[(0.58, 0.20), (0.58, 0.34), (0.72, 0.34)], bg);
            }
        }
    }
}

fn rounded_square(pm: &mut Pixmap, size: f32, c: [u8; 3]) {
    let r = size * 0.22;
    let k = r * 0.5523;
    let mut pb = PathBuilder::new();
    pb.move_to(r, 0.0);
    pb.line_to(size - r, 0.0);
    pb.cubic_to(size - r + k, 0.0, size, r - k, size, r);
    pb.line_to(size, size - r);
    pb.cubic_to(size, size - r + k, size - r + k, size, size - r, size);
    pb.line_to(r, size);
    pb.cubic_to(r - k, size, 0.0, size - r + k, 0.0, size - r);
    pb.line_to(0.0, r);
    pb.cubic_to(0.0, r - k, r - k, 0.0, r, 0.0);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
    }
}

/// A rounded square in `accent` with the white pictogram.
pub fn render_icon(p: Pictogram, size: u32, accent: [u8; 3], fg: [u8; 3]) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("icon size is non-zero");
    rounded_square(&mut pm, size as f32, accent);
    let mut cv = Canvas { pm: &mut pm, size: size as f32, scale: 1.0, snap: size <= 32 };
    cv.pictogram(p, fg, accent);
    pm
}

/// Full-bleed accent background, pictogram inside the central 80 % (maskable icons keep a safe zone).
pub fn render_maskable(p: Pictogram, size: u32, accent: [u8; 3], fg: [u8; 3]) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("icon size is non-zero");
    pm.fill(tiny_skia::Color::from_rgba8(accent[0], accent[1], accent[2], 255));
    let mut cv = Canvas { pm: &mut pm, size: size as f32, scale: 0.8, snap: false };
    cv.pictogram(p, fg, accent);
    pm
}

/// Straight-alpha RGBA bytes (for hosts that cannot decode a PNG, such as a painter-only logo).
pub fn rgba_bytes(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.pixels().len() * 4);
    for px in pm.pixels() {
        let c = px.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

pub fn png_bytes(pm: &Pixmap) -> Result<Vec<u8>, String> {
    pm.encode_png().map_err(|e| format!("png: {e}"))
}

/// An `.ico` with one image per size: 32-bit BMP below 64 px (old shells), PNG from 64 px.
pub fn ico_bytes(p: Pictogram, sizes: &[u32], accent: [u8; 3], fg: [u8; 3]) -> Result<Vec<u8>, String> {
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
    for &s in sizes {
        let pm = render_icon(p, s, accent, fg);
        let data = if s >= 64 { png_bytes(&pm)? } else { bmp_dib(&pm) };
        images.push((s, data));
    }
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (s, data) in &images {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in images {
        out.extend_from_slice(&data);
    }
    Ok(out)
}

/// BITMAPINFOHEADER + bottom-up BGRA pixels + an all-zero AND mask (alpha carries the transparency).
fn bmp_dib(pm: &Pixmap) -> Vec<u8> {
    let (w, h) = (pm.width(), pm.height());
    let mask_row = w.div_ceil(32) * 4;
    let mut d = Vec::new();
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&w.to_le_bytes());
    d.extend_from_slice(&(h * 2).to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&32u16.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&(w * h * 4 + mask_row * h).to_le_bytes());
    d.extend_from_slice(&[0; 16]);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = pm.pixel(x, y).expect("in bounds").demultiply();
            d.extend_from_slice(&[c.blue(), c.green(), c.red(), c.alpha()]);
        }
    }
    d.resize(d.len() + (mask_row * h) as usize, 0);
    d
}
