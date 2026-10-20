//! Latin -> Cyrillic (rules.md 4.x, cluster order 4.12).
use crate::chars::*;
use crate::dict;
use crate::engine::{CLOSER, Class, Item, PROT, Seg, Word};
use alloc::{string::String, vec::Vec};

/// R15 quote guard: marks apostrophes that close a single-quoted passage so that the o/g before them stays plain.
pub(crate) fn mark_closers(items: &mut [Item]) {
    let n = items.len();
    let opener = |items: &[Item], i: usize| {
        matches!(items[i].c, '\'' | '`' | '\u{2018}')
            && items[i].flags & PROT == 0
            && (i == 0
                || items[i - 1].c.is_whitespace()
                || matches!(items[i - 1].c, '(' | '[' | '{' | '«' | '"' | '“' | '„'))
            && items.get(i + 1).is_some_and(|x| is_letter(x.c))
    };
    let cand = |items: &[Item], j: usize| {
        matches!(items[j].c, '\'' | '`' | '\u{2019}')
            && items[j].flags & PROT == 0
            && j > 0
            && is_latin(items[j - 1].c)
            && items.get(j + 1).is_none_or(|x| !is_letter(x.c))
    };
    let mut i = 0;
    while i < n {
        if !opener(items, i) {
            i += 1;
            continue;
        }
        let (mut j, mut last_og) = (i + 1, None);
        while j < n && !opener(items, j) {
            if cand(items, j) {
                if is_og(items[j - 1].c) {
                    last_og = Some(j);
                } else {
                    last_og = None; // a non-o/g candidate closes the quote; o/g ones before it stay letters
                    break;
                }
            }
            j += 1;
        }
        if let Some(k) = last_og {
            items[k].flags |= CLOSER;
        }
        i = j.max(i + 1);
    }
}

fn at(seg: &Seg, k: usize, we: usize) -> Option<char> {
    (k < we).then(|| lower(seg.items[k].c))
}

fn cluster(seg: &mut Seg, a: usize, b: usize, ch: char, up: bool) {
    seg.open(a);
    seg.out.s.push(if up { upper(ch) } else { ch });
    seg.attachments(a, b);
}

fn foreign(seg: &Seg, w: &Word) -> bool {
    (w.ws..w.we).any(|i| {
        let it = seg.items[i];
        match it.c {
            'c' | 'C' => !at(seg, i + 1, w.we).is_some_and(|n| n == 'h'),
            'w' | 'W' => true,
            c if is_foreign_latin(c) => true,
            c => {
                is_latin(c)
                    && (it.s as usize + it.n as usize..it.e as usize)
                        .any(|j| is_combining(seg.orig[j]))
            }
        }
    })
}

pub(crate) fn word(seg: &mut Seg, wi: usize) {
    let mut buf = core::mem::take(&mut seg.buf);
    word_with(seg, wi, &mut buf);
    seg.buf = buf;
}

fn word_with(seg: &mut Seg, wi: usize, buf: &mut String) {
    let w = seg.words[wi];
    // keep and acro entries are matched with exact case; all of them contain a capital
    if !w.attach && w.class != Class::Lower {
        buf.clear();
        buf.extend(seg.items[w.ws..w.we].iter().map(|t| t.c));
        if dict::keep(buf) {
            seg.verbatim_range(w.ws, w.we);
            return;
        }
        if let Some(cy) = dict::acro(dict::ACRO_L, buf) {
            let t: Vec<char> = cy.chars().collect();
            seg.emit_split(w.ws, w.we - w.ws, &t, false);
            return;
        }
    }
    if seg.opts.foreign_latin_guard && foreign(seg, &w) {
        seg.verbatim_range(w.ws, w.we);
        return;
    }
    if !w.attach {
        buf.clear();
        let mut prev = ' ';
        for t in &seg.items[w.ws..w.we] {
            buf.push(if is_apos(t.c) {
                if is_og(prev) { '\u{2BB}' } else { '\u{2BC}' }
            } else {
                lower(t.c)
            });
            prev = t.c;
        }
        if let Some(h) = dict::find(dict::L2C, buf) {
            let mut t: Vec<char> = h.target.chars().collect();
            if w.ws + h.n < w.we && t.last() == Some(&'ь') {
                t.pop(); // D-11: a final soft sign is dropped before a suffix
            }
            seg.recase(&w, h.n, &mut t);
            seg.emit_split(w.ws, h.n, &t, false);
            letters(seg, w.ws + h.n, w.ws, w.we);
            return;
        }
    }
    letters(seg, w.ws, w.ws, w.we);
}

fn single(lc: char) -> Option<char> {
    Some(match lc {
        'a' => 'а',
        'b' => 'б',
        'c' => 'к',
        'd' => 'д',
        'f' => 'ф',
        'g' => 'г',
        'h' => 'ҳ',
        'i' => 'и',
        'j' => 'ж',
        'k' => 'к',
        'l' => 'л',
        'm' => 'м',
        'n' => 'н',
        'o' => 'о',
        'p' => 'п',
        'q' => 'қ',
        'r' => 'р',
        's' => 'с',
        't' => 'т',
        'u' => 'у',
        'v' => 'в',
        'w' => 'в',
        'x' => 'х',
        'y' => 'й',
        'z' => 'з',
        _ => return None,
    })
}

/// The letter rules over items `from..we` of the word starting at `ws` (the part before `from` is the dictionary stem).
fn letters(seg: &mut Seg, from: usize, ws: usize, we: usize) {
    let mut i = from;
    while i < we {
        let c = seg.items[i].c;
        let lc = lower(c);
        let up = is_up(c);
        let (n1, n2, n3) = (at(seg, i + 1, we), at(seg, i + 2, we), at(seg, i + 3, we));
        let apos = |x: Option<char>| x.is_some_and(is_apos);
        if matches!(lc, 'o' | 'g') && apos(n1) {
            cluster(seg, i, i + 1, if lc == 'o' { 'ў' } else { 'ғ' }, up);
            i += 2;
        } else if lc == 's' && apos(n1) && n2 == Some('h') {
            cluster(seg, i, i + 1, 'с', up);
            i += 2;
        } else if (lc == 's' || lc == 'c') && n1 == Some('h') {
            cluster(seg, i, i + 1, if lc == 's' { 'ш' } else { 'ч' }, up);
            i += 2;
        } else if lc == 'y' {
            let (ch, len) = match n1 {
                Some('o') if apos(n2) => ('й', 1),
                Some('o') => ('ё', 2),
                Some('u') => ('ю', 2),
                Some('a') => ('я', 2),
                Some('e') => ('е', 2),
                _ => ('й', 1),
            };
            cluster(seg, i, i + len - 1, ch, up);
            i += len;
        } else if lc == 't'
            && n1 == Some('s')
            && n2 == Some('i')
            && matches!(n3, Some('y' | 'a' | 'o' | 'e'))
            && seg
                .prev_letter(i, ws)
                .is_some_and(|p| is_vl(seg.items[p].c))
        {
            cluster(seg, i, i + 1, 'ц', up); // R7a
            i += 2;
        } else if lc == 'e' {
            let e_hard = match seg.prev_letter(i, ws) {
                None => true,
                Some(p) => is_vl(seg.items[p].c),
            } || (i > ws && is_apos(seg.items[i - 1].c));
            cluster(seg, i, i, if e_hard { 'э' } else { 'е' }, up);
            i += 1;
        } else if is_apos(c) && i > ws && i + 1 < we {
            let both_up = is_up(seg.items[i - 1].c) && is_up(seg.items[i + 1].c);
            cluster(seg, i, i, 'ъ', both_up);
            i += 1;
        } else if let Some(ch) = single(lc) {
            cluster(seg, i, i, ch, up);
            i += 1;
        } else {
            seg.verbatim(i);
            i += 1;
        }
    }
}
