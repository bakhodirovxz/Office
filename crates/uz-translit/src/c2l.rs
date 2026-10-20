//! Cyrillic -> Latin (rules.md 3.2, 4.x, case 5.2).
use crate::chars::*;
use crate::dict;
use crate::engine::{Class, Seg, Word};
use alloc::vec::Vec;

pub(crate) fn word(seg: &mut Seg, wi: usize) {
    let w = seg.words[wi];
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
            let t: Vec<char> = l.chars().collect();
            seg.emit_split(w.ws, w.we - w.ws, &t, true);
            return;
        }
        if let Some(h) = hit {
            let mut t: Vec<char> = h.target.chars().collect();
            seg.recase(&w, h.n, &mut t);
            seg.emit_split(w.ws, h.n, &t, true);
            letters(seg, w.ws + h.n, &w);
            return;
        }
    }
    letters(seg, w.ws, &w);
}

fn map(lc: char) -> Option<&'static str> {
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
            'с' => Some(if next.is_some_and(|n| lower(n) == 'ҳ') {
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
            _ => map(lc),
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
