//! Embedded TrueType faces: a process-wide registry of parsed faces, per-character fallback and
//! the PDF objects (subset font file, `Identity-H` CID font, `ToUnicode` map) written for each
//! face a document uses.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{Arc, RwLock};

use crate::{Font, ttf};

/// A registered face and the face that supplies glyphs it lacks.
pub(crate) struct Entry {
    pub face: ttf::Face,
    pub fallback: Option<Font>,
}

/// Most faces one process may register (a hostile caller cannot grow the registry without bound).
const MAX_FACES: usize = 512;
/// Fallback hops followed when a face lacks a character.
const MAX_FALLBACK: usize = 4;

static REGISTRY: RwLock<Vec<Arc<Entry>>> = RwLock::new(Vec::new());

/// Registers face `index` of the TrueType file `data` and returns the [`Font`] that draws with
/// it. `fallback` (another registered face) supplies characters this face has no glyph for.
/// Returns `None` for a file that is not a parseable `glyf`-outline TrueType font.
pub fn register_face(data: Vec<u8>, index: u32, fallback: Option<Font>) -> Option<Font> {
    let face = ttf::Face::parse(data, index)?;
    let fallback = fallback.filter(|f| matches!(f, Font::Face(_)));
    let mut reg = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    if reg.len() >= MAX_FACES {
        return None;
    }
    reg.push(Arc::new(Entry { face, fallback }));
    Some(Font::Face((reg.len() - 1) as u16))
}

pub(crate) fn entry(id: u16) -> Option<Arc<Entry>> {
    REGISTRY.read().unwrap_or_else(|e| e.into_inner()).get(id as usize).cloned()
}

/// Characters that are never drawn or measured (zero-width controls and joiners).
fn invisible(c: char) -> bool {
    (c < ' ' && c != '\t') || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' | '\u{FE00}'..='\u{FE0F}' | '\u{00AD}')
}

/// Face id, glyph id and fallback-resolved font for one character.
fn pick(start: u16, c: char) -> (u16, u16) {
    let c = if c == '\t' { ' ' } else { c };
    let mut id = start;
    for _ in 0..=MAX_FALLBACK {
        let Some(e) = entry(id) else { break };
        let g = e.face.glyph(c);
        if g != 0 {
            return (id, g);
        }
        match e.fallback {
            Some(Font::Face(next)) => id = next,
            _ => break,
        }
    }
    (start, 0)
}

/// Consecutive characters set in one face: `(face id, [(glyph id, char)])`.
pub(crate) fn runs(id: u16, text: &str) -> Vec<(u16, Vec<(u16, char)>)> {
    let mut out: Vec<(u16, Vec<(u16, char)>)> = Vec::new();
    for c in text.chars().filter(|c| !invisible(*c)) {
        let (face, gid) = pick(id, c);
        match out.last_mut() {
            Some((f, v)) if *f == face => v.push((gid, c)),
            _ => out.push((face, vec![(gid, c)])),
        }
    }
    out
}

/// Width of `text` in 1/1000 em when set in face `id` (with its fallbacks).
pub(crate) fn width_units(id: u16, text: &str) -> f32 {
    let mut total = 0.0f32;
    for (face, glyphs) in runs(id, text) {
        let Some(e) = entry(face) else { continue };
        let k = 1000.0 / f32::from(e.face.units_per_em);
        total += glyphs.iter().map(|(g, _)| f32::from(e.face.advance(*g)) * k).sum::<f32>();
    }
    total
}

/// Width of `text` in device pixels when every glyph advance is rounded to whole pixels at `ppem`
/// pixels per em, the way Windows GDI lays out text at 100% zoom.
pub(crate) fn width_px_snapped(id: u16, text: &str, ppem: f32) -> f32 {
    let mut total = 0.0f32;
    for (face, glyphs) in runs(id, text) {
        let Some(e) = entry(face) else { continue };
        let k = ppem / f32::from(e.face.units_per_em);
        total += glyphs.iter().map(|(g, _)| (f32::from(e.face.advance(*g)) * k).round()).sum::<f32>();
    }
    total
}

/// True when the face (or a fallback) has a glyph for every visible character of `text`.
pub(crate) fn covers(id: u16, text: &str) -> bool {
    text.chars().filter(|c| !invisible(*c)).all(|c| pick(id, c).1 != 0)
}

fn units(face: &ttf::Face, v: i32) -> i32 {
    let u = i32::from(face.units_per_em);
    (v * 1000 + u / 2).div_euclid(u)
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut o = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    o.extend_from_slice(data);
    o.extend_from_slice(b"\nendstream");
    o
}

fn to_unicode(map: &BTreeMap<u16, char>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = map.iter().filter(|(g, _)| **g != 0).collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (g, c) in chunk {
            let mut buf = [0u16; 2];
            let units = c.encode_utf16(&mut buf);
            let _ = write!(s, "<{g:04X}> <");
            for u in units.iter() {
                let _ = write!(s, "{u:04X}");
            }
            s.push_str(">\n");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// A short stable subset tag (six capital letters) for `/BaseFont`.
fn subset_tag(id: u16, gids: &BTreeSet<u16>) -> String {
    let mut h: u32 = 0x811C_9DC5 ^ u32::from(id);
    for g in gids {
        h = (h ^ u32::from(*g)).wrapping_mul(0x0100_0193);
    }
    (0..6)
        .map(|_| {
            let c = (b'A' + (h % 26) as u8) as char;
            h /= 26;
            if h == 0 {
                h = 0x9E37_79B9;
            }
            c
        })
        .collect()
}

/// Writes the objects for every face in `used` (face id → glyph id → character), numbering from
/// `*next`. Returns the page-resource font entries (`/E<id> <obj> 0 R `).
pub(crate) fn write_objects(used: &BTreeMap<u16, BTreeMap<u16, char>>, next: &mut u32, objects: &mut Vec<(u32, Vec<u8>)>) -> String {
    let mut dict = String::new();
    for (id, glyphs) in used {
        let Some(e) = entry(*id) else { continue };
        let face = &e.face;
        let gids: BTreeSet<u16> = glyphs.keys().copied().collect();
        let (type0, cid, desc, file, tou) = (*next, *next + 1, *next + 2, *next + 3, *next + 4);
        *next += 5;
        let name = if face.ps_name.is_empty() { "Embedded" } else { face.ps_name.as_str() };
        let base = format!("{}+{}", subset_tag(*id, &gids), name);
        let _ = write!(dict, "/E{id} {type0} 0 R ");
        objects.push((
            type0,
            format!("<< /Type /Font /Subtype /Type0 /BaseFont /{base} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {tou} 0 R >>")
                .into_bytes(),
        ));
        let mut w = String::new();
        for g in &gids {
            let _ = write!(w, "{g} [{}] ", units(face, i32::from(face.advance(*g))));
        }
        objects.push((
            cid,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor {desc} 0 R /CIDToGIDMap /Identity /DW 1000 /W [{w}] >>"
            )
            .into_bytes(),
        ));
        let b = face.bbox.map(|v| units(face, i32::from(v)));
        let mut flags = 4;
        if face.italic {
            flags |= 64;
        }
        if face.bold {
            flags |= 1 << 18;
        }
        objects.push((
            desc,
            format!(
                "<< /Type /FontDescriptor /FontName /{base} /Flags {flags} /FontBBox [{} {} {} {}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV {} /FontFile2 {file} 0 R >>",
                b[0],
                b[1],
                b[2],
                b[3],
                face.italic_angle,
                units(face, i32::from(face.ascent)),
                units(face, i32::from(face.descent)),
                units(face, i32::from(face.cap_height)),
                if face.bold { 140 } else { 80 },
            )
            .into_bytes(),
        ));
        let font_file = face.subset(&gids).unwrap_or_default();
        objects.push((file, stream(&format!("/Length1 {}", font_file.len()), &font_file)));
        objects.push((tou, stream("", to_unicode(glyphs).as_bytes())));
    }
    dict
}

/// Hex string `<…>` of two-byte glyph ids.
pub(crate) fn glyph_string(glyphs: &[(u16, char)]) -> String {
    let mut s = String::with_capacity(glyphs.len() * 4 + 2);
    s.push('<');
    for (g, _) in glyphs {
        let _ = write!(s, "{g:04X}");
    }
    s.push('>');
    s
}
