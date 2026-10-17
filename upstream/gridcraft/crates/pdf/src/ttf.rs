//! A minimal, bounds-checked TrueType reader and glyph subsetter for PDF embedding.
//!
//! Supports `glyf`-outline fonts (`0x00010000` / `true` sfnt, and a face of a `.ttc`). Reads
//! `cmap` (formats 4 and 12), `hmtx`, `head`, `hhea`, `maxp`, `name`, `OS/2` and `post`; writes a
//! subset that keeps every glyph id (unused glyphs become empty), so PDF text can address glyphs
//! by their original id with `CIDToGIDMap /Identity`. Every read is checked: a damaged or hostile
//! font yields `None`, never a panic.

use std::collections::BTreeSet;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    let b = d.get(o..o.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}
fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    let b = d.get(o..o.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Most glyphs a font may declare (the format's own limit).
const MAX_GLYPHS: u32 = 65_535;
/// Most composite-glyph nesting followed when collecting components.
const MAX_COMPOSITE_DEPTH: u32 = 8;

#[derive(Clone, Copy, Debug)]
struct Table {
    off: usize,
    len: usize,
}

#[derive(Clone, Copy, Debug)]
enum CmapKind {
    Format4,
    Format12,
}

/// A parsed TrueType face. Owns the font file bytes.
#[derive(Debug)]
pub struct Face {
    data: Vec<u8>,
    tables: Vec<([u8; 4], Table)>,
    pub units_per_em: u16,
    pub num_glyphs: u16,
    loca_long: bool,
    num_hmetrics: u16,
    cmap: (usize, CmapKind),
    glyf: Table,
    loca: Table,
    hmtx: Table,
    /// `hhea` ascender / descender in font units.
    pub ascent: i16,
    pub descent: i16,
    /// `head` bounding box in font units: x_min, y_min, x_max, y_max.
    pub bbox: [i16; 4],
    pub bold: bool,
    pub italic: bool,
    /// `OS/2` cap height in font units (the ascender when the table lacks it).
    pub cap_height: i16,
    pub italic_angle: f32,
    /// The family name from the `name` table (typographic family when present).
    pub family: String,
    /// The PostScript name, restricted to PDF-name-safe ASCII.
    pub ps_name: String,
}

/// `(family, subfamily)` from a raw `name` table.
pub fn names_of_table(name: &[u8]) -> (String, String) {
    let get = |ids: &[u16]| ids.iter().find_map(|id| name_string(name, *id)).unwrap_or_default();
    (get(&[16, 1]), get(&[17, 2]))
}

fn name_string(name: &[u8], want: u16) -> Option<String> {
    let count = u16_at(name, 2)? as usize;
    let base = u16_at(name, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count.min(512) {
        let r = 6 + i * 12;
        let (pf, enc, lang, id) = (u16_at(name, r)?, u16_at(name, r + 2)?, u16_at(name, r + 4)?, u16_at(name, r + 6)?);
        if id != want {
            continue;
        }
        let (len, off) = (u16_at(name, r + 8)? as usize, u16_at(name, r + 10)? as usize);
        let start = base.checked_add(off)?;
        let raw = name.get(start..start.checked_add(len)?)?;
        let (rank, s) = match (pf, enc) {
            (3, 1) | (3, 10) | (0, _) => {
                let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                (if lang == 0x409 { 3 } else { 2 }, String::from_utf16_lossy(&units))
            }
            (1, 0) => (1, raw.iter().map(|b| if b.is_ascii() { *b as char } else { '?' }).collect()),
            _ => continue,
        };
        if !s.is_empty() && best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, s));
        }
    }
    best.map(|(_, s)| s)
}

/// The sfnt table directory entries of the face at `index` in `data`.
fn directory(data: &[u8], index: u32) -> Option<Vec<([u8; 4], Table)>> {
    let mut base = 0usize;
    if data.get(0..4)? == b"ttcf" {
        let n = u32_at(data, 8)?;
        if index >= n {
            return None;
        }
        base = u32_at(data, 12usize.checked_add((index as usize).checked_mul(4)?)?)? as usize;
    } else if index != 0 {
        return None;
    }
    if u32_at(data, base)? != 0x0001_0000 && data.get(base..base + 4)? != b"true" {
        return None;
    }
    let n = u16_at(data, base + 4)? as usize;
    let mut tables = Vec::with_capacity(n.min(64));
    for i in 0..n.min(64) {
        let r = base + 12 + i * 16;
        let tag: [u8; 4] = data.get(r..r + 4)?.try_into().ok()?;
        let off = u32_at(data, r + 8)? as usize;
        let len = u32_at(data, r + 12)? as usize;
        data.get(off..off.checked_add(len)?)?;
        tables.push((tag, Table { off, len }));
    }
    Some(tables)
}

impl Face {
    /// Parses face `index` of `data` (a `.ttf`, or a `.ttc` collection).
    pub fn parse(data: Vec<u8>, index: u32) -> Option<Face> {
        let tables = directory(&data, index)?;
        let find = |tag: &[u8; 4]| tables.iter().find(|(t, _)| t == tag).map(|(_, t)| *t);
        let head = find(b"head")?;
        let hhea = find(b"hhea")?;
        let maxp = find(b"maxp")?;
        let hmtx = find(b"hmtx")?;
        let glyf = find(b"glyf")?;
        let loca = find(b"loca")?;
        let cmap_t = find(b"cmap")?;
        let units_per_em = u16_at(&data, head.off + 18)?;
        if !(16..=16_384).contains(&units_per_em) {
            return None;
        }
        let bbox = [i16_at(&data, head.off + 36)?, i16_at(&data, head.off + 38)?, i16_at(&data, head.off + 40)?, i16_at(&data, head.off + 42)?];
        let mac_style = u16_at(&data, head.off + 44)?;
        let loca_long = i16_at(&data, head.off + 50)? != 0;
        let num_glyphs = u16_at(&data, maxp.off + 4)?;
        if u32::from(num_glyphs) > MAX_GLYPHS || num_glyphs == 0 {
            return None;
        }
        let num_hmetrics = u16_at(&data, hhea.off + 34)?;
        if num_hmetrics == 0 || (num_hmetrics as usize) * 4 > hmtx.len {
            return None;
        }
        let ascent = i16_at(&data, hhea.off + 4)?;
        let descent = i16_at(&data, hhea.off + 6)?;
        // loca must cover every glyph.
        let need = (num_glyphs as usize + 1) * if loca_long { 4 } else { 2 };
        if loca.len < need {
            return None;
        }
        // cmap: prefer full Unicode (format 12), else BMP (format 4).
        let n_sub = u16_at(&data, cmap_t.off + 2)? as usize;
        let mut best: Option<(u8, usize, CmapKind)> = None;
        for i in 0..n_sub.min(64) {
            let r = cmap_t.off + 4 + i * 8;
            let (pf, enc) = (u16_at(&data, r)?, u16_at(&data, r + 2)?);
            let off = cmap_t.off.checked_add(u32_at(&data, r + 4)? as usize)?;
            let kind = match u16_at(&data, off)? {
                4 => CmapKind::Format4,
                12 => CmapKind::Format12,
                _ => continue,
            };
            let rank = match (pf, enc, kind) {
                (3, 10, CmapKind::Format12) => 5,
                (0, 4 | 6, CmapKind::Format12) => 4,
                (3, 1, CmapKind::Format4) => 3,
                (0, _, CmapKind::Format4) => 2,
                (_, _, CmapKind::Format12) => 1,
                _ => 0,
            };
            if rank > 0 && best.is_none_or(|(r, _, _)| rank > r) {
                best = Some((rank, off, kind));
            }
        }
        let (_, cmap_off, cmap_kind) = best?;
        let (mut cap_height, mut bold, mut italic) = (ascent, mac_style & 1 != 0, mac_style & 2 != 0);
        if let Some(os2) = find(b"OS/2") {
            if let Some(sel) = u16_at(&data, os2.off + 62) {
                italic |= sel & 1 != 0;
                bold |= sel & 0x20 != 0;
            }
            if u16_at(&data, os2.off).is_some_and(|v| v >= 2)
                && let Some(c) = i16_at(&data, os2.off + 88)
                && c > 0
            {
                cap_height = c;
            }
            if let Some(w) = u16_at(&data, os2.off + 4) {
                bold |= w >= 600;
            }
        }
        let italic_angle = find(b"post").and_then(|p| u32_at(&data, p.off + 4)).map(|v| (v as i32) as f32 / 65_536.0).unwrap_or(0.0);
        let (family, ps_name) = match find(b"name").and_then(|t| data.get(t.off..t.off + t.len)) {
            Some(nt) => {
                let ps: String = name_string(nt, 6).unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(60).collect();
                (names_of_table(nt).0, ps)
            }
            None => (String::new(), String::new()),
        };
        Some(Face {
            data,
            tables,
            units_per_em,
            num_glyphs,
            loca_long,
            num_hmetrics,
            cmap: (cmap_off, cmap_kind),
            glyf,
            loca,
            hmtx,
            ascent,
            descent,
            bbox,
            bold,
            italic,
            cap_height,
            italic_angle,
            family,
            ps_name,
        })
    }

    /// The glyph id for `c`, or 0 (`.notdef`) when the font has none.
    pub fn glyph(&self, c: char) -> u16 {
        let (off, kind) = self.cmap;
        let d = &self.data;
        let cp = c as u32;
        let gid = match kind {
            CmapKind::Format12 => (|| {
                let n = u32_at(d, off + 12)? as usize;
                let (mut lo, mut hi) = (0usize, n.min(1 << 20));
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    let r = off + 16 + mid * 12;
                    let (start, end, g) = (u32_at(d, r)?, u32_at(d, r + 4)?, u32_at(d, r + 8)?);
                    if cp < start {
                        hi = mid;
                    } else if cp > end {
                        lo = mid + 1;
                    } else {
                        return g.checked_add(cp - start);
                    }
                }
                None
            })(),
            CmapKind::Format4 => (|| {
                if cp > 0xFFFF {
                    return None;
                }
                let seg = (u16_at(d, off + 6)? / 2) as usize;
                let ends = off + 14;
                let starts = ends + seg * 2 + 2;
                let deltas = starts + seg * 2;
                let ranges = deltas + seg * 2;
                let (mut lo, mut hi) = (0usize, seg);
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    let end = u32::from(u16_at(d, ends + mid * 2)?);
                    let start = u32::from(u16_at(d, starts + mid * 2)?);
                    if cp > end {
                        lo = mid + 1;
                    } else if cp < start {
                        hi = mid;
                    } else {
                        let delta = u16_at(d, deltas + mid * 2)?;
                        let ro = u16_at(d, ranges + mid * 2)? as usize;
                        if ro == 0 {
                            return Some(u32::from((cp as u16).wrapping_add(delta)));
                        }
                        let g = u16_at(d, ranges + mid * 2 + ro + (cp - start) as usize * 2)?;
                        return if g == 0 { None } else { Some(u32::from(g.wrapping_add(delta))) };
                    }
                }
                None
            })(),
        };
        match gid {
            Some(g) if g < u32::from(self.num_glyphs) => g as u16,
            _ => 0,
        }
    }

    /// Advance width of glyph `gid` in font units.
    pub fn advance(&self, gid: u16) -> u16 {
        let i = gid.min(self.num_hmetrics.saturating_sub(1)) as usize;
        u16_at(&self.data, self.hmtx.off + i * 4).unwrap_or(0)
    }

    fn lsb(&self, gid: u16) -> i16 {
        let g = gid as usize;
        let nh = self.num_hmetrics as usize;
        let o = if g < nh { self.hmtx.off + g * 4 + 2 } else { self.hmtx.off + nh * 4 + (g - nh) * 2 };
        if o + 2 <= self.hmtx.off + self.hmtx.len { i16_at(&self.data, o).unwrap_or(0) } else { 0 }
    }

    fn glyph_range(&self, gid: u16) -> Option<(usize, usize)> {
        let g = gid as usize;
        let (a, b) = if self.loca_long {
            (u32_at(&self.data, self.loca.off + g * 4)? as usize, u32_at(&self.data, self.loca.off + g * 4 + 4)? as usize)
        } else {
            (u16_at(&self.data, self.loca.off + g * 2)? as usize * 2, u16_at(&self.data, self.loca.off + g * 2 + 2)? as usize * 2)
        };
        if b < a || b > self.glyf.len {
            return None;
        }
        Some((self.glyf.off + a, self.glyf.off + b))
    }

    fn glyph_bytes(&self, gid: u16) -> &[u8] {
        self.glyph_range(gid).and_then(|(a, b)| self.data.get(a..b)).unwrap_or(&[])
    }

    fn components(&self, gid: u16, depth: u32, out: &mut BTreeSet<u16>) {
        if depth > MAX_COMPOSITE_DEPTH || (!out.insert(gid) && depth > 0) {
            return;
        }
        let g = self.glyph_bytes(gid);
        if i16_at(g, 0).is_none_or(|n| n >= 0) {
            return;
        }
        let mut o = 10usize;
        for _ in 0..256 {
            let (Some(flags), Some(child)) = (u16_at(g, o), u16_at(g, o + 2)) else { return };
            if child < self.num_glyphs {
                self.components(child, depth + 1, out);
            }
            o += 4 + if flags & 1 != 0 { 4 } else { 2 };
            if flags & 0x8 != 0 {
                o += 2;
            } else if flags & 0x40 != 0 {
                o += 4;
            } else if flags & 0x80 != 0 {
                o += 8;
            }
            if flags & 0x20 == 0 {
                return;
            }
        }
    }

    fn table(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        let t = self.tables.iter().find(|(t, _)| t == tag)?.1;
        self.data.get(t.off..t.off + t.len)
    }

    /// A TrueType file holding only `used` glyphs (plus `.notdef` and composite components), with
    /// the original glyph ids. `None` if the font is too damaged to subset.
    pub fn subset(&self, used: &BTreeSet<u16>) -> Option<Vec<u8>> {
        let mut keep = BTreeSet::new();
        keep.insert(0u16);
        for g in used {
            if *g < self.num_glyphs {
                self.components(*g, 0, &mut keep);
            }
        }
        let n = *keep.iter().next_back()? as usize + 1;
        let mut glyf: Vec<u8> = Vec::new();
        let mut loca: Vec<u8> = Vec::with_capacity((n + 1) * 4);
        let mut hmtx: Vec<u8> = Vec::with_capacity(n * 4);
        for g in 0..n as u16 {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if keep.contains(&g) {
                glyf.extend_from_slice(self.glyph_bytes(g));
                while glyf.len() % 4 != 0 {
                    glyf.push(0);
                }
            }
            hmtx.extend_from_slice(&self.advance(g).to_be_bytes());
            hmtx.extend_from_slice(&self.lsb(g).to_be_bytes());
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut head = self.table(b"head")?.get(..54)?.to_vec();
        head[8..12].copy_from_slice(&[0; 4]);
        head[50..52].copy_from_slice(&1i16.to_be_bytes());
        let mut hhea = self.table(b"hhea")?.get(..36)?.to_vec();
        hhea[34..36].copy_from_slice(&(n as u16).to_be_bytes());
        let mut maxp = self.table(b"maxp")?.to_vec();
        maxp.get_mut(4..6)?.copy_from_slice(&(n as u16).to_be_bytes());
        let mut out_tables: Vec<([u8; 4], Vec<u8>)> =
            vec![(*b"glyf", glyf), (*b"head", head), (*b"hhea", hhea), (*b"hmtx", hmtx), (*b"loca", loca), (*b"maxp", maxp)];
        for tag in [b"cvt ", b"fpgm", b"prep"] {
            if let Some(t) = self.table(tag) {
                out_tables.push((*tag, t.to_vec()));
            }
        }
        out_tables.sort_by_key(|(t, _)| *t);
        Some(write_sfnt(out_tables))
    }
}

fn checksum(d: &[u8]) -> u32 {
    let mut sum = 0u32;
    for c in d.chunks(4) {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(w));
    }
    sum
}

fn write_sfnt(tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n * 16 - pow * 16).to_be_bytes());
    let mut off = 12 + tables.len() * 16;
    let mut head_at = None;
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(off as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_at = Some(off);
        }
        off += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        if let Some(slot) = out.get_mut(h + 8..h + 12) {
            slot.copy_from_slice(&adj.to_be_bytes());
        }
    }
    out
}

/// Reads `(family, subfamily)` of face `index` from the head of a font file without loading the
/// glyphs: `read(offset, len)` returns file bytes. Lets a host catalogue installed fonts cheaply.
pub fn probe_names(index: u32, mut read: impl FnMut(u64, usize) -> Option<Vec<u8>>) -> Option<(String, String)> {
    let head = read(0, 16)?;
    let base = if head.get(0..4)? == b"ttcf" {
        if index >= u32_at(&head, 8)? {
            return None;
        }
        u64::from(u32_at(&read(12 + u64::from(index) * 4, 4)?, 0)?)
    } else if index == 0 {
        0
    } else {
        return None;
    };
    let hdr = read(base, 12)?;
    if u32_at(&hdr, 0)? != 0x0001_0000 && hdr.get(0..4)? != b"true" {
        return None;
    }
    let n = (u16_at(&hdr, 4)? as usize).min(64);
    let dir = read(base + 12, n * 16)?;
    for i in 0..n {
        if dir.get(i * 16..i * 16 + 4)? == b"name" {
            let off = u64::from(u32_at(&dir, i * 16 + 8)?);
            let len = u32_at(&dir, i * 16 + 12)? as usize;
            if len > 1 << 20 {
                return None;
            }
            return Some(names_of_table(&read(off, len)?));
        }
    }
    None
}
