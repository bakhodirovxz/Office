//! Uzbek Latin <-> Cyrillic transliteration. The rules are specified in `docs/phase2/rules.md`
//! (summary in `RULES.md`); this crate implements them with no dependencies.
//!
//! * [`convert`]: a string in, a string out.
//! * [`convert_runs`]: the runs of one paragraph in, each run's new text out (rules.md section 8).
//! * [`detect`]: the dominant script of a text.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod c2l;
mod chars;
mod dict;
mod engine;
mod l2c;
mod scan;

use alloc::{borrow::Cow, string::String, vec::Vec};

/// Bumped with any output-changing rule or data change.
pub const RULES_VERSION: &str = "uz-translit-rules/2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    ToCyrillic,
    ToLatin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Latin,
    Cyrillic,
}

/// Form of the apostrophes the engine produces in Latin output (R1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Apostrophe {
    /// oʻ/gʻ = U+02BB, tutuq and the sʼh separator = U+02BC.
    Official,
    /// All of them U+0027.
    Ascii,
}

/// Latin alphabet (rules.md section 15). ->L writes it. ->C: `Y1995` reads both alphabets, `Y2026` reads
/// 2026 only (sh, ch, o', g' are not digraphs), `Auto` picks per paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LatinAlphabet {
    /// The alphabet in force (default): oʻ gʻ sh ch, and s + tutuq + h for сҳ.
    #[default]
    Y1995,
    /// The pending 2026 reform (not in force): ö ğ ş ç, and plain sh for сҳ.
    Y2026,
    /// ->Latin writes 1995. ->Cyrillic reads a paragraph as 2026 if it has any of ö ğ ş ç, else as 1995.
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub apostrophe: Apostrophe,
    /// ->Latin: which Latin alphabet to write (default: 1995).
    pub latin_alphabet: LatinAlphabet,
    /// ->Cyrillic: copy words with c (not in ch), w or diacritic Latin letters unchanged (R14).
    pub foreign_latin_guard: bool,
    /// ->Cyrillic: protect Roman numerals (R12 ROMAN).
    pub roman_numerals: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            apostrophe: Apostrophe::Official,
            latin_alphabet: LatinAlphabet::Y1995,
            foreign_latin_guard: true,
            roman_numerals: true,
        }
    }
}

/// Language-tag policy for adapters (rules.md 7.2, owner decision D1): which runs are passed with `convert: true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LangPolicy {
    /// Untagged, `uz*`, `ru*` and `en*` text is converted; other languages are skipped.
    #[default]
    ConvertRuEn,
    /// Untagged and `uz*` text is converted; every other language is skipped.
    SkipRuEn,
}

/// Should text with this BCP-47 tag (e.g. `ru-RU`, `uz-Latn`) be converted under `policy`?
pub fn lang_converts(tag: Option<&str>, policy: LangPolicy) -> bool {
    let Some(tag) = tag.filter(|t| !t.is_empty()) else {
        return true;
    };
    let primary = tag.split(['-', '_']).next().unwrap_or("");
    if primary.eq_ignore_ascii_case("uz") || primary.eq_ignore_ascii_case("und") {
        return true;
    }
    policy == LangPolicy::ConvertRuEn
        && (primary.eq_ignore_ascii_case("ru") || primary.eq_ignore_ascii_case("en"))
}

/// One run of a paragraph. `convert: false` runs are copied verbatim and act as hard boundaries.
#[derive(Clone, Copy, Debug)]
pub struct Run<'a> {
    pub text: &'a str,
    pub convert: bool,
}

/// Convert `text` in the given direction.
pub fn convert(text: &str, dir: Direction, opts: &Options) -> String {
    let chars: Vec<char> = text.chars().collect();
    engine::convert_segment(&chars, dir, opts, false, true).0
}

/// Converts the runs of ONE paragraph as one logical text and returns each run's new text.
/// `Cow::Borrowed` = unchanged. Invariant: `concat(convert_runs(split(t))) == convert(t)` for every split of `t`.
pub fn convert_runs<'a>(runs: &[Run<'a>], dir: Direction, opts: &Options) -> Vec<Cow<'a, str>> {
    let mut res: Vec<Cow<'a, str>> = runs.iter().map(|r| Cow::Borrowed(r.text)).collect();
    let mut i = 0;
    while i < runs.len() {
        if !runs[i].convert {
            i += 1;
            continue;
        }
        let j = i + runs[i..].iter().take_while(|r| r.convert).count();
        let mut chars: Vec<char> = Vec::new();
        let mut starts: Vec<usize> = Vec::with_capacity(j - i);
        for r in &runs[i..j] {
            starts.push(chars.len());
            chars.extend(r.text.chars());
        }
        let (out, marks) = engine::convert_segment(&chars, dir, opts, true, i == 0);
        let mut outs: Vec<String> = (i..j).map(|_| String::new()).collect();
        let mut r = 0;
        for (p, &(src, o)) in marks.iter().enumerate() {
            let end = marks.get(p + 1).map_or(out.len(), |m| m.1 as usize);
            while r + 1 < outs.len() && starts[r + 1] <= src as usize {
                r += 1;
            }
            outs[r].push_str(&out[o as usize..end]);
        }
        for (k, o) in outs.into_iter().enumerate() {
            if o != runs[i + k].text {
                res[i + k] = Cow::Owned(o);
            }
        }
        i = j;
    }
    res
}

/// The script with more letters (Latin a-z vs Cyrillic); `None` if there are no letters or the counts tie.
pub fn detect(text: &str) -> Option<Script> {
    let (mut lat, mut cyr) = (0usize, 0usize);
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            lat += 1;
        } else if matches!(c as u32, 0x400..=0x52F) && c.is_alphabetic() {
            cyr += 1;
        }
    }
    match lat.cmp(&cyr) {
        core::cmp::Ordering::Greater => Some(Script::Latin),
        core::cmp::Ordering::Less => Some(Script::Cyrillic),
        core::cmp::Ordering::Equal => None,
    }
}
