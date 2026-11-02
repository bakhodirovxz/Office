//! The text map (PLAN phase 7, section 4.4): the bulk read and the bulk apply, as blobs.
//!
//! Read blob (host to plugin):
//! `u32 lang_count, per lang (u32 len, UTF-8); u32 paragraph_count, per paragraph: u32 index, u32 segment_count,
//! per segment: u8 flags, u16 lang (index into the table; entry 0 is always ""), u32 len, UTF-8 text`.
//! Paragraph indexes strictly increase (a selection scope leaves gaps).
//!
//! Apply blob (plugin to host), positional against the read: `u32 paragraph_count, per paragraph: u32 segment_count,
//! per segment: u8 changed (0 or 1), then if 1: u32 len, UTF-8 text`.

use crate::wire::{MAX_MESSAGE, Rd, WireError, put_str, put_u32};

/// The segment may be changed. When clear, the reason bits below say why not.
pub const EDITABLE: u8 = 1;
pub const OUTSIDE_SELECTION: u8 = 2;
pub const OBJECT: u8 = 4;
pub const LOCKED_CONTROL: u8 = 8;
pub const FIELD_RESULT: u8 = 16;
const KNOWN_FLAGS: u8 = EDITABLE | OUTSIDE_SELECTION | OBJECT | LOCKED_CONTROL | FIELD_RESULT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub flags: u8,
    pub lang: u16,
    pub text: String,
}

impl Segment {
    pub fn editable(&self) -> bool {
        self.flags & EDITABLE != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    pub index: u32,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMap {
    /// Language tags; `langs[0]` is `""` (not set).
    pub langs: Vec<String>,
    pub paragraphs: Vec<Paragraph>,
}

impl Default for TextMap {
    fn default() -> Self {
        TextMap {
            langs: vec![String::new()],
            paragraphs: Vec::new(),
        }
    }
}

const E: fn(&'static str) -> WireError = WireError::Textmap;

impl TextMap {
    /// The index of `tag` in the table, adding it when new.
    pub fn lang_index(&mut self, tag: &str) -> Result<u16, WireError> {
        if let Some(i) = self.langs.iter().position(|l| l == tag) {
            return Ok(i as u16);
        }
        let i = u16::try_from(self.langs.len()).map_err(|_| E("more than 65535 language tags"))?;
        self.langs.push(tag.to_string());
        Ok(i)
    }

    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        if self.langs.first().map(String::as_str) != Some("") {
            return Err(E("language table entry 0 must be empty"));
        }
        let mut o = Vec::new();
        put_u32(&mut o, self.langs.len())?;
        for l in &self.langs {
            put_str(&mut o, l)?;
        }
        put_u32(&mut o, self.paragraphs.len())?;
        for p in &self.paragraphs {
            o.extend_from_slice(&p.index.to_le_bytes());
            put_u32(&mut o, p.segments.len())?;
            for s in &p.segments {
                if usize::from(s.lang) >= self.langs.len() {
                    return Err(E("segment language index outside the table"));
                }
                o.push(s.flags);
                o.extend_from_slice(&s.lang.to_le_bytes());
                put_str(&mut o, &s.text)?;
            }
            if o.len() > MAX_MESSAGE {
                return Err(WireError::TooLarge);
            }
        }
        Ok(o)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_MESSAGE {
            return Err(WireError::TooLarge);
        }
        let mut r = Rd::new(bytes);
        let n = r.count(4)?;
        if n > usize::from(u16::MAX) {
            return Err(E("more than 65535 language tags"));
        }
        let mut langs = Vec::with_capacity(n);
        for _ in 0..n {
            langs.push(r.str()?);
        }
        if langs.first().map(String::as_str) != Some("") {
            return Err(E("language table entry 0 must be empty"));
        }
        let np = r.count(8)?;
        let mut paragraphs = Vec::with_capacity(np);
        let mut prev: Option<u32> = None;
        for _ in 0..np {
            let index = r.u32()?;
            if prev.is_some_and(|p| index <= p) {
                return Err(E("paragraph indexes must strictly increase"));
            }
            prev = Some(index);
            let ns = r.count(7)?;
            let mut segments = Vec::with_capacity(ns);
            for _ in 0..ns {
                let flags = r.u8()?;
                if flags & !KNOWN_FLAGS != 0 {
                    return Err(E("unknown flag bits"));
                }
                if flags & EDITABLE != 0 && flags != EDITABLE {
                    return Err(E("an editable segment carries a reason bit"));
                }
                let lang = r.u16()?;
                if usize::from(lang) >= langs.len() {
                    return Err(E("segment language index outside the table"));
                }
                segments.push(Segment {
                    flags,
                    lang,
                    text: r.str()?,
                });
            }
            paragraphs.push(Paragraph { index, segments });
        }
        r.finish()?;
        Ok(TextMap { langs, paragraphs })
    }
}

/// New text per segment, `None` = unchanged; positional against the text map of the same invocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Apply {
    pub paragraphs: Vec<Vec<Option<String>>>,
}

/// Characters a segment may not gain: they would split or corrupt the paragraph (`\t` is allowed).
fn bad_char(c: char) -> bool {
    (c.is_control() && c != '\t') || c == '\u{2028}' || c == '\u{2029}'
}

impl Apply {
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let mut o = Vec::new();
        put_u32(&mut o, self.paragraphs.len())?;
        for p in &self.paragraphs {
            put_u32(&mut o, p.len())?;
            for s in p {
                match s {
                    None => o.push(0),
                    Some(t) => {
                        o.push(1);
                        put_str(&mut o, t)?;
                    }
                }
            }
            if o.len() > MAX_MESSAGE {
                return Err(WireError::TooLarge);
            }
        }
        Ok(o)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_MESSAGE {
            return Err(WireError::TooLarge);
        }
        let mut r = Rd::new(bytes);
        let np = r.count(4)?;
        let mut paragraphs = Vec::with_capacity(np);
        for _ in 0..np {
            let ns = r.count(1)?;
            let mut segs = Vec::with_capacity(ns);
            for _ in 0..ns {
                segs.push(match r.u8()? {
                    0 => None,
                    1 => Some(r.str()?),
                    _ => return Err(E("changed byte is not 0 or 1")),
                });
            }
            paragraphs.push(segs);
        }
        r.finish()?;
        Ok(Apply { paragraphs })
    }

    /// The host's check before it applies anything: same paragraph and segment counts as the read, only editable segments
    /// changed, no new text that splits or corrupts a paragraph.
    pub fn validate(&self, map: &TextMap) -> Result<(), WireError> {
        if self.paragraphs.len() != map.paragraphs.len() {
            return Err(E("paragraph count differs from the read"));
        }
        for (a, p) in self.paragraphs.iter().zip(&map.paragraphs) {
            if a.len() != p.segments.len() {
                return Err(E("segment count differs from the read"));
            }
            for (new, seg) in a.iter().zip(&p.segments) {
                if let Some(t) = new {
                    if !seg.editable() {
                        return Err(E("a segment that is not editable was changed"));
                    }
                    if t.chars().any(bad_char) {
                        return Err(E("new text holds a line break or control character"));
                    }
                }
            }
        }
        Ok(())
    }
}
