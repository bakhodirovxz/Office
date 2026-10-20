//! Test helper: the 1995 -> 2026 Latin letter substitution (rules.md section 15.2).
#![allow(dead_code)]

fn is_apos(c: char) -> bool {
    matches!(
        c,
        '\u{2BB}' | '\u{2BC}' | '\u{2018}' | '\u{2019}' | '\'' | '`' | '\u{2BF}' | '\u{B4}'
    )
}

/// oʻ gʻ sh ch -> ö ğ ş ç (case of the first letter); s + apostrophe + h -> sh (no separator).
pub fn to_2026(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut o = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let lc = c.to_ascii_lowercase();
        let n1 = cs.get(i + 1).copied();
        let l1 = n1.map(|x| x.to_ascii_lowercase());
        let l2 = cs.get(i + 2).map(|x| x.to_ascii_lowercase());
        let letter = |lo: char| {
            if c.is_ascii_uppercase() {
                lo.to_uppercase().next().unwrap()
            } else {
                lo
            }
        };
        match (lc, n1) {
            ('o', Some(a)) if is_apos(a) => {
                o.push(letter('ö'));
                i += 2;
            }
            ('g', Some(a)) if is_apos(a) => {
                o.push(letter('ğ'));
                i += 2;
            }
            ('s', Some(a)) if is_apos(a) && l2 == Some('h') => {
                o.push(c);
                o.push(cs[i + 2]);
                i += 3;
            }
            ('s', _) if l1 == Some('h') => {
                o.push(letter('ş'));
                i += 2;
            }
            ('c', _) if l1 == Some('h') => {
                o.push(letter('ç'));
                i += 2;
            }
            _ => {
                o.push(c);
                i += 1;
            }
        }
    }
    o
}

/// Does the text contain `s`, an apostrophe and `h` (the separated сҳ spelling)?
pub fn has_separated_sh(s: &str) -> bool {
    let cs: Vec<char> = s.to_lowercase().chars().collect();
    cs.windows(3)
        .any(|w| w[0] == 's' && is_apos(w[1]) && w[2] == 'h')
}
