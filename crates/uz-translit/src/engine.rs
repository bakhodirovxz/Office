//! Shared machinery: items (R16 view), words, case classes (R10), output pieces (R13) and the driver.
use crate::chars::*;
use crate::{Apostrophe, Direction, Options, c2l, l2c, scan};
use alloc::{string::String, vec::Vec};

pub(crate) const PROT: u8 = 1; // inside a protected span (R12): verbatim, acts as a boundary
pub(crate) const CLOSER: u8 = 2; // an apostrophe that closes a quotation (R15)

/// One logical character. Core chars are `orig[s..s+n]` (n = 2 for a composed Cyrillic pair);
/// attachments (transparent chars, combining marks) are `orig[s+n..e]`.
#[derive(Clone, Copy)]
pub(crate) struct Item {
    pub c: char,
    pub s: u32,
    pub e: u32,
    pub n: u8,
    pub flags: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Lower,
    Upper,
    Title,
    Mixed,
    Single,
}

#[derive(Clone, Copy)]
pub(crate) struct Word {
    pub ws: usize,
    pub we: usize,
    pub class: Class,
    pub first_upper: bool,
    pub allcaps: bool, // Single in an all-caps context
    pub attach: bool,  // contains attachments: skips the dictionary
}

impl Word {
    /// The class used for case transfer of dictionary output (5.4).
    pub fn effective(&self) -> Class {
        match self.class {
            Class::Single if self.allcaps => Class::Upper,
            Class::Single if self.first_upper => Class::Title,
            Class::Single => Class::Lower,
            c => c,
        }
    }
}

pub(crate) struct Out {
    pub s: String,
    /// (source char index, output byte offset) where each output piece starts; only when tracking.
    pub marks: Vec<(u32, u32)>,
    track: bool,
}

pub(crate) struct Seg<'a> {
    pub orig: &'a [char],
    pub items: Vec<Item>,
    pub words: Vec<Word>,
    pub out: Out,
    pub opts: Options,
    pub buf: String, // scratch for dictionary keys
}

fn build_items(orig: &[char]) -> Vec<Item> {
    let n = orig.len();
    let mut v = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let (c, len) = match orig.get(i + 1).and_then(|&m| compose(orig[i], m)) {
            Some(x) => (x, 2),
            None => (orig[i], 1),
        };
        let mut e = i + len;
        if is_letter(c) {
            while e < n && (is_transparent(orig[e]) || is_combining(orig[e])) {
                e += 1;
            }
        }
        v.push(Item {
            c,
            s: i as u32,
            e: e as u32,
            n: len as u8,
            flags: 0,
        });
        i = e;
    }
    v
}

impl<'a> Seg<'a> {
    pub fn piece(&mut self, src: usize) {
        if self.out.track {
            let o = self.out.s.len() as u32;
            self.out.marks.push((src as u32, o));
        }
    }

    /// Copy one item verbatim (every source char is its own piece, so spans split across runs correctly).
    pub fn verbatim(&mut self, i: usize) {
        let it = self.items[i];
        for k in it.s as usize..it.e as usize {
            self.piece(k);
            self.out.s.push(self.orig[k]);
        }
    }

    pub fn verbatim_range(&mut self, a: usize, b: usize) {
        for i in a..b {
            self.verbatim(i);
        }
    }

    /// Start a piece anchored at item `a`'s first source char.
    pub fn open(&mut self, a: usize) {
        let s = self.items[a].s as usize;
        self.piece(s);
    }

    /// Append the attachments of items a..=b (they follow the cluster output, 2.4).
    pub fn attachments(&mut self, a: usize, b: usize) {
        for k in a..=b {
            let it = self.items[k];
            for j in (it.s as usize + it.n as usize)..it.e as usize {
                self.out.s.push(self.orig[j]);
            }
        }
    }

    /// Output for a Latin apostrophe produced by ->L (R1 output forms; `Ascii` option).
    pub fn push_lat(&mut self, ch: char) {
        let ch = if self.opts.apostrophe == Apostrophe::Ascii && matches!(ch, '\u{2BB}' | '\u{2BC}')
        {
            '\''
        } else {
            ch
        };
        self.out.s.push(ch);
    }

    pub fn prev_letter(&self, i: usize, ws: usize) -> Option<usize> {
        (ws..i).rev().find(|&j| is_letter(self.items[j].c))
    }

    /// Dictionary output (8.3): `n` source items starting at `ws` get proportional slices of `target`.
    pub fn emit_split(&mut self, ws: usize, n: usize, target: &[char], to_lat: bool) {
        let m = target.len();
        let b = |k: usize| (k * m + n / 2) / n;
        for k in 0..n {
            self.open(ws + k);
            for &ch in &target[b(k)..b(k + 1)] {
                if to_lat {
                    self.push_lat(ch)
                } else {
                    self.out.s.push(ch)
                }
            }
        }
    }

    /// Re-case a dictionary target by the source word's class (5.4).
    pub fn recase(&self, w: &Word, n: usize, target: &mut [char]) {
        let m = target.len();
        match w.effective() {
            Class::Lower | Class::Single => {}
            Class::Title => target[0] = upper(target[0]),
            Class::Upper => target.iter_mut().for_each(|c| *c = upper(*c)),
            Class::Mixed => {
                let b = |k: usize| (k * m + n / 2) / n;
                for k in 0..n {
                    if is_up(self.items[w.ws + k].c) {
                        for t in &mut target[b(k)..b(k + 1)] {
                            *t = upper(*t);
                        }
                    }
                }
            }
        }
    }

    fn classify(&self, ws: usize, we: usize) -> (Class, bool) {
        let (mut letters, mut up, mut low, mut first_upper) = (0, 0, 0, false);
        for it in &self.items[ws..we] {
            if is_letter(it.c) {
                if letters == 0 {
                    first_upper = is_up(it.c);
                }
                letters += 1;
                if is_up(it.c) {
                    up += 1;
                } else if is_lo(it.c) {
                    low += 1;
                }
            }
        }
        let class = if letters == 1 {
            Class::Single
        } else if up == 0 {
            Class::Lower
        } else if low == 0 {
            Class::Upper
        } else if first_upper && up == 1 {
            Class::Title
        } else {
            Class::Mixed
        };
        (class, first_upper)
    }

    fn find_words(&mut self, to_cyr: bool) {
        let n = self.items.len();
        let mut i = 0;
        while i < n {
            let it = self.items[i];
            if it.flags & PROT != 0 || !is_letter(it.c) {
                i += 1;
                continue;
            }
            let ws = i;
            i += 1;
            while i < n && self.continues(i, to_cyr) {
                i += 1;
            }
            let (class, first_upper) = self.classify(ws, i);
            let attach = self.items[ws..i].iter().any(|t| t.e > t.s + t.n as u32);
            self.words.push(Word {
                ws,
                we: i,
                class,
                first_upper,
                allcaps: false,
                attach,
            });
        }
        for k in 0..self.words.len() {
            let w = self.words[k];
            if w.class == Class::Single && w.first_upper {
                let up = |j: Option<usize>| j.is_some_and(|j| self.words[j].class == Class::Upper);
                let ctx = up(k.checked_sub(1)) || up((k + 1 < self.words.len()).then_some(k + 1));
                self.words[k].allcaps = ctx;
            }
        }
    }

    /// Does item `i` (outside a protected span) continue a word whose previous item is `i-1`? (2.2 a, b)
    fn continues(&self, i: usize, to_cyr: bool) -> bool {
        let it = self.items[i];
        if it.flags & PROT != 0 {
            return false;
        }
        if is_letter(it.c) {
            return true;
        }
        if !(to_cyr && is_apos(it.c) && it.flags & CLOSER == 0) {
            return false;
        }
        let prev = self.items[i - 1].c;
        is_latin_ext(prev)
            && (is_og(prev)
                || self
                    .items
                    .get(i + 1)
                    .is_some_and(|nx| nx.flags & PROT == 0 && is_latin_ext(nx.c)))
    }

    fn run(&mut self, dir: Direction) {
        let (n, mut wi, mut i) = (self.items.len(), 0, 0);
        while i < n {
            if wi < self.words.len() && self.words[wi].ws == i {
                let we = self.words[wi].we;
                match dir {
                    Direction::ToCyrillic => l2c::word(self, wi),
                    Direction::ToLatin => c2l::word(self, wi),
                }
                i = we;
                wi += 1;
            } else {
                self.verbatim(i);
                i += 1;
            }
        }
    }
}

/// Convert one segment. Returns the output text and (when `track`) the piece marks.
pub(crate) fn convert_segment(
    orig: &[char],
    dir: Direction,
    opts: &Options,
    track: bool,
    para_start: bool,
) -> (String, Vec<(u32, u32)>) {
    let mut seg = Seg {
        orig,
        items: build_items(orig),
        words: Vec::new(),
        out: Out {
            s: String::with_capacity(orig.len() + orig.len() / 4),
            marks: Vec::new(),
            track,
        },
        opts: *opts,
        buf: String::new(),
    };
    let to_cyr = dir == Direction::ToCyrillic;
    scan::mark(&mut seg.items, to_cyr && opts.roman_numerals, para_start);
    if to_cyr {
        l2c::mark_closers(&mut seg.items);
    }
    seg.find_words(to_cyr);
    seg.run(dir);
    (seg.out.s, seg.out.marks)
}
