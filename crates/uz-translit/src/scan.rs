//! R12: protected spans, found at token starts before word segmentation.
use crate::chars::*;
use crate::engine::{Item, PROT};
use alloc::vec::Vec;

const TLDS: [&str; 18] = [
    "uz", "ru", "com", "org", "net", "gov", "edu", "info", "io", "kz", "tj", "kg", "tm", "de",
    "uk", "us", "eu", "int",
];
const EXTS: [&str; 26] = [
    "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "txt", "csv", "odt", "ods", "odp", "rtf",
    "jpg", "jpeg", "png", "gif", "zip", "rar", "7z", "exe", "msi", "html", "htm", "xml", "json",
];

fn eq_ci(t: &[char], s: &str) -> bool {
    t.len() == s.chars().count()
        && t.iter()
            .zip(s.chars())
            .all(|(a, b)| a.to_ascii_lowercase() == b)
}
fn starts_ci(t: &[char], s: &str) -> bool {
    t.len() >= s.len() && eq_ci(&t[..s.len()], s)
}
fn in_list_ci(t: &[char], list: &[&str]) -> bool {
    list.iter().any(|s| eq_ci(t, s))
}
fn lab(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

/// Trailing trim shared by URL, e-mail and domain paths. `p` is the minimum kept length.
fn trim(t: &[char], p: usize, mut e: usize) -> usize {
    while e > p {
        let c = t[e - 1];
        if !matches!(
            c,
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '»' | '”' | '’' | ')' | ']' | '}'
        ) {
            break;
        }
        if c == ')' {
            let (o, cl) = (
                t[..e].iter().filter(|&&x| x == '(').count(),
                t[..e].iter().filter(|&&x| x == ')').count(),
            );
            if o >= cl {
                break;
            }
        }
        e -= 1;
    }
    e
}

fn url(t: &[char]) -> usize {
    let p = [
        "http://", "https://", "ftp://", "file://", "mailto:", "www.",
    ]
    .iter()
    .find(|s| starts_ci(t, s))
    .map_or(0, |s| s.len());
    if p == 0 {
        return 0;
    }
    let e = t[p..]
        .iter()
        .position(|&c| matches!(c, '<' | '>' | '"' | '«' | '»'))
        .map_or(t.len(), |x| p + x);
    let e = trim(t, p, e);
    if e > p { e } else { 0 }
}

/// Labels `[A-Za-z0-9-]+` joined by dots, from `from`: (end, label count, start of the last label).
fn labels(t: &[char], from: usize) -> (usize, usize, usize) {
    let (mut j, mut count, mut end, mut last) = (from, 0, from, from);
    loop {
        let st = j;
        while j < t.len() && lab(t[j]) {
            j += 1;
        }
        if j == st {
            break;
        }
        (count, end, last) = (count + 1, j, st);
        if j + 1 < t.len() && t[j] == '.' && lab(t[j + 1]) {
            j += 1;
        } else {
            break;
        }
    }
    (end, count, last)
}

fn email(t: &[char]) -> usize {
    let k = t
        .iter()
        .take_while(|&&c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
        .count();
    if k == 0 || t.get(k) != Some(&'@') {
        return 0;
    }
    let (end, count, _) = labels(t, k + 1);
    if count >= 2 { end } else { 0 }
}

fn domain(t: &[char]) -> usize {
    if !t[0].is_ascii_alphanumeric() {
        return 0;
    }
    let (end, count, last) = labels(t, 0);
    if count < 2
        || !in_list_ci(&t[last..end], &TLDS)
        || t.get(end).is_some_and(|c| c.is_alphanumeric())
    {
        return 0;
    }
    if t.get(end) == Some(&'/') {
        trim(t, end, t.len())
    } else {
        end
    }
}

fn mention(t: &[char]) -> usize {
    if t[0] != '@' {
        return 0;
    }
    let k = t[1..]
        .iter()
        .take_while(|&&c| c.is_ascii_alphanumeric() || c == '_')
        .count();
    if k > 0 { k + 1 } else { 0 }
}

fn path(t: &[char]) -> usize {
    let n = t.len();
    let drive = n >= 3 && t[0].is_ascii_alphabetic() && t[1] == ':' && matches!(t[2], '\\' | '/');
    let unc = n >= 2 && t[0] == '\\' && t[1] == '\\';
    let backslash = t.contains(&'\\');
    let rooted = [("/", 1), ("./", 2), ("../", 3), ("~/", 2)]
        .iter()
        .any(|&(p, l)| {
            starts_ci(t, p)
                && (t[l..].contains(&'/')
                    || t[l..]
                        .windows(2)
                        .any(|w| w[0] == '.' && w[1].is_alphanumeric()))
        });
    let run = t
        .iter()
        .take_while(|&&c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .count();
    let named = t[..run]
        .iter()
        .rposition(|&c| c == '.')
        .is_some_and(|d| d > 0 && in_list_ci(&t[d + 1..run], &EXTS));
    if drive || unc || backslash || rooted || named {
        n
    } else {
        0
    }
}

/// CODE-A: 1-4 upper-case letters (Latin O'/G' count as one), an optional separator, digits, more `[-/.]digits` groups.
fn code_a(t: &[char]) -> usize {
    let (mut k, mut letters) = (0, 0);
    while k < t.len() && is_up(t[k]) && is_letter(t[k]) {
        k += if matches!(t[k], 'O' | 'G') && t.get(k + 1).is_some_and(|&a| is_apos(a)) {
            2
        } else {
            1
        };
        letters += 1;
        if letters > 4 {
            return 0;
        }
    }
    if letters == 0 {
        return 0;
    }
    if t.get(k).is_some_and(|c| matches!(c, '-' | '–' | '‑' | '/')) {
        k += 1;
    }
    let digits = |k: usize| t[k..].iter().take_while(|c| c.is_ascii_digit()).count();
    let d = digits(k);
    if d == 0 {
        return 0;
    }
    k += d;
    while k + 1 < t.len() && matches!(t[k], '-' | '/' | '.') && t[k + 1].is_ascii_digit() {
        k += 1 + digits(k + 1);
    }
    k
}

/// CODE-B: an alphanumeric run in which 3+ digits touch an upper-case letter.
fn code_b(t: &[char]) -> usize {
    let run = t.iter().take_while(|c| c.is_alphanumeric()).count();
    let mut i = 0;
    while i < run {
        if t[i].is_ascii_digit() {
            let j = i + t[i..run].iter().take_while(|c| c.is_ascii_digit()).count();
            let touches = (i > 0 && is_up(t[i - 1])) || (j < run && is_up(t[j]));
            if j - i >= 3 && touches {
                return run;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    0
}

fn roman_value_ok(w: &[char]) -> bool {
    let (mut i, mut c) = (0, 0);
    while c < 3 && w.get(i) == Some(&'C') {
        (i, c) = (i + 1, c + 1);
    }
    if matches!((w.get(i), w.get(i + 1)), (Some('X'), Some('C' | 'L'))) {
        i += 2;
    } else {
        if w.get(i) == Some(&'L') {
            i += 1;
        }
        let mut x = 0;
        while x < 3 && w.get(i) == Some(&'X') {
            (i, x) = (i + 1, x + 1);
        }
    }
    if matches!((w.get(i), w.get(i + 1)), (Some('I'), Some('X' | 'V'))) {
        i += 2;
    } else {
        if w.get(i) == Some(&'V') {
            i += 1;
        }
        let mut x = 0;
        while x < 3 && w.get(i) == Some(&'I') {
            (i, x) = (i + 1, x + 1);
        }
    }
    i == w.len()
}

fn roman(t: &[char], para_start: bool) -> usize {
    let r = t.iter().take_while(|&&c| is_letter(c)).count();
    if r == 0
        || !t[..r]
            .iter()
            .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C'))
    {
        return 0;
    }
    if t.get(r).is_some_and(|&a| is_apos(a)) && t.get(r + 1).is_some_and(|&b| is_latin(b)) {
        return 0; // the word continues over a tutuq (2.2 b)
    }
    if r >= 2 {
        return if roman_value_ok(&t[..r]) { r } else { 0 };
    }
    if !matches!(t[0], 'I' | 'V' | 'X') {
        return 0;
    }
    if t.get(1) != Some(&'.') || para_start {
        1
    } else {
        0
    }
}

/// Mark protected spans (flags |= PROT). `roman` only for ->C. `seg_para_start`: the segment starts a paragraph.
pub(crate) fn mark(items: &mut [Item], roman_on: bool, seg_para_start: bool) {
    let n = items.len();
    for i in 1..n.saturating_sub(1) {
        if matches!(items[i].c, 'x' | 'X' | '×')
            && items[i - 1].c.is_ascii_digit()
            && items[i + 1].c.is_ascii_digit()
        {
            items[i].flags |= PROT; // multiplication sign
        }
    }
    let para_start = |items: &[Item], i: usize| {
        let mut j = i;
        while j > 0 {
            let p = items[j - 1].c;
            if matches!(p, '\n' | '\u{B}' | '\r' | '\u{2028}' | '\u{2029}') {
                return true;
            }
            if !p.is_whitespace() {
                return false;
            }
            j -= 1;
        }
        seg_para_start
    };
    let mut tok: Vec<char> = Vec::new();
    let mut i = 0;
    'outer: while i < n {
        let c = items[i].c;
        let token_start = !c.is_whitespace()
            && (i == 0
                || items[i - 1].c.is_whitespace()
                || matches!(items[i - 1].c, '(' | '[' | '{' | '«' | '"' | '“' | '„'));
        if token_start {
            let mut st = i;
            loop {
                // one pass over the token decides which scanners can possibly match
                let (mut at, mut dot, mut col, mut sl, mut dig) =
                    (false, false, false, false, false);
                let mut te = st;
                while te < n && !items[te].c.is_whitespace() {
                    match items[te].c {
                        '@' => at = true,
                        '.' => dot = true,
                        ':' => col = true,
                        '/' | '\\' => sl = true,
                        '0'..='9' => dig = true,
                        _ => {}
                    }
                    te += 1;
                }
                let first = items[st].c;
                let roman_first = roman_on && matches!(first, 'I' | 'V' | 'X' | 'L' | 'C');
                let mut best = 0;
                let mut cands = [0usize; 8];
                if at || dot || col || sl || dig || roman_first || first == '@' {
                    tok.clear();
                    tok.extend(items[st..te].iter().map(|it| it.c));
                    cands = [
                        if col || dot { url(&tok) } else { 0 },
                        if at { email(&tok) } else { 0 },
                        if dot { domain(&tok) } else { 0 },
                        mention(&tok),
                        if sl || dot || col { path(&tok) } else { 0 },
                        if dig { code_a(&tok) } else { 0 },
                        if dig { code_b(&tok) } else { 0 },
                        if roman_first {
                            roman(&tok, para_start(items, st))
                        } else {
                            0
                        },
                    ];
                }
                for len in cands {
                    if len > best {
                        best = len; // strict: on a tie the earlier row wins
                    }
                }
                if best > 0 {
                    items[st..st + best]
                        .iter_mut()
                        .for_each(|it| it.flags |= PROT);
                    i = st + best;
                    continue 'outer;
                }
                if st == i && items[i].c == '№' && i + 1 < n && !items[i + 1].c.is_whitespace() {
                    st += 1;
                } else {
                    break;
                }
            }
        }
        i += 1;
    }
}
