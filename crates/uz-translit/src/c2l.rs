//! Cyrillic -> Latin (rules.md 3.2, 4.x, case 5.2).
use crate::LatinAlphabet;
use crate::chars::*;
use crate::dict;
use crate::engine::{Class, Seg, Word};
use alloc::vec::Vec;

/// 1995 Latin text -> 2026 letters (case follows the first letter; sʼh -> sh, section 15).
fn to_2026(t: &[char]) -> Vec<char> {
    let mut o = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        let (c, n1, n2) = (
            t[i],
            t.get(i + 1).map(|&x| lower(x)),
            t.get(i + 2).map(|&x| lower(x)),
        );
        let new = match (lower(c), n1) {
            ('o', Some('\u{2BB}')) => Some(('ö', 2)),
            ('g', Some('\u{2BB}')) => Some(('ğ', 2)),
            ('s', Some('\u{2BC}')) if n2 == Some('h') => None,
            ('s', Some('h')) => Some(('ş', 2)),
            ('c', Some('h')) => Some(('ç', 2)),
            _ => None,
        };
        match new {
            Some((l, k)) => {
                o.push(if is_up(c) { upper(l) } else { l });
                i += k;
            }
            None if lower(c) == 's' && n1 == Some('\u{2BC}') && n2 == Some('h') => {
                o.extend([c, t[i + 2]]); // s + h: no separator, "ş" is distinct
                i += 3;
            }
            None => {
                o.push(c);
                i += 1;
            }
        }
    }
    o
}

pub(crate) fn word(seg: &mut Seg, wi: usize) {
    let w = seg.words[wi];
    let y26 = seg.opts.latin_alphabet == LatinAlphabet::Y2026;
    if !w.attach {
        let mut buf = core::mem::take(&mut seg.buf);
        buf.clear();
        buf.extend(seg.items[w.ws..w.we].iter().map(|t| t.c));
        // acronyms are matched with exact case; all of them contain a capital
        let acro = if w.class != Class::Lower {
            dict::acro(dict::ACRO_C, &buf)
        } else {
            None
        };
        let hit = if acro.is_some() {
            None
        } else {
            buf.clear();
            buf.extend(seg.items[w.ws..w.we].iter().map(|t| lower(t.c)));
            dict::find(dict::C2L, &buf)
        };
        seg.buf = buf;
        if let Some(l) = acro {
            let mut t: Vec<char> = l.chars().collect();
            if y26 {
                t = to_2026(&t);
            }
            seg.emit_split(w.ws, w.we - w.ws, &t, true);
            return;
        }
        if let Some(h) = hit {
            let mut t: Vec<char> = h.target.chars().collect();
            if y26 {
                t = to_2026(&t);
            }
            seg.recase(&w, h.n, &mut t);
            seg.emit_split(w.ws, h.n, &t, true);
            letters(seg, w.ws + h.n, &w);
            return;
        }
    }
    letters(seg, w.ws, &w);
}

fn map(lc: char, y26: bool) -> Option<&'static str> {
    if y26 {
        match lc {
            'ғ' => return Some("ğ"),
            'ў' => return Some("ö"),
            'ч' => return Some("ç"),
            'ш' | 'щ' => return Some("ş"),
            _ => {}
        }
    }
    Some(match lc {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'ғ' => "g\u{2BB}",
        'д' => "d",
        'ж' => "j",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'қ' => "q",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'т' => "t",
        'у' => "u",
        'ў' => "o\u{2BB}",
        'ф' => "f",
        'х' => "x",
        'ҳ' => "h",
        'ч' => "ch",
        'ш' | 'щ' => "sh",
        'ы' => "i",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        'ё' => "yo",
        _ => return None,
    })
}

fn letters(seg: &mut Seg, from: usize, w: &Word) {
    let (ws, we) = (w.ws, w.we);
    let y26 = seg.opts.latin_alphabet == LatinAlphabet::Y2026;
    for i in from..we {
        let c = seg.items[i].c;
        let lc = lower(c);
        let prev = (i > ws).then(|| seg.items[i - 1].c);
        let next = (i + 1 < we).then(|| seg.items[i + 1].c);
        let out: Option<&str> = match lc {
            'е' => Some(
                if prev.is_none_or(|p| is_vc(p) || matches!(lower(p), 'ъ' | 'ь')) {
                    "ye"
                } else {
                    "e"
                },
            ),
            'ц' => Some(if prev.is_some_and(is_vc) { "ts" } else { "s" }),
            'с' => Some(if !y26 && next.is_some_and(|n| lower(n) == 'ҳ') {
                "s\u{2BC}"
            } else {
                "s"
            }),
            'ъ' => Some(
                if prev.is_some_and(|p| lower(p) == 'ў')
                    || (prev.is_some_and(is_cyr_consonant)
                        && next.is_some_and(|n| matches!(lower(n), 'е' | 'ё' | 'ю' | 'я')))
                {
                    ""
                } else {
                    "\u{2BC}"
                },
            ),
            'ь' => Some(if next.is_some_and(|n| lower(n) == 'о') {
                "y"
            } else {
                ""
            }),
            _ => map(lc, y26),
        };
        let Some(s) = out else {
            seg.verbatim(i);
            continue;
        };
        let up = is_up(c);
        // 5.2: a multi-character output of a capital is all upper when its neighbour letter is a capital
        let all_up = up
            && s.chars().count() >= 2
            && (next.is_some_and(is_up)
                || (next.is_none() && prev.is_some_and(is_up))
                || w.allcaps);
        seg.open(i);
        for (k, ch) in s.chars().enumerate() {
            seg.push_lat(if up && (all_up || k == 0) {
                upper(ch)
            } else {
                ch
            });
        }
        seg.attachments(i, i);
    }
}
