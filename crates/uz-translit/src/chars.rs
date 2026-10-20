//! Character classes (rules.md section 2.1).

/// The apostrophe set `A`.
pub(crate) fn is_apos(c: char) -> bool {
    matches!(
        c,
        '\u{2BB}' | '\u{2BC}' | '\u{2018}' | '\u{2019}' | '\'' | '`' | '\u{2BF}' | '\u{B4}'
    )
}
/// U+02BB/02BC/02BF are category Lm, so `is_alphabetic` is true for them: the exclusion is required.
pub(crate) fn is_letter(c: char) -> bool {
    match c {
        'a'..='z' | 'A'..='Z' | '\u{400}'..='\u{45F}' => true, // fast path: Latin and basic Cyrillic
        '\0'..='\u{7f}' => false,
        _ => c.is_alphabetic() && !is_apos(c),
    }
}
pub(crate) fn is_up(c: char) -> bool {
    match c {
        'A'..='Z' | '\u{400}'..='\u{42F}' => true,
        '\0'..='\u{7f}' | '\u{430}'..='\u{45F}' => false,
        _ => c.is_uppercase(),
    }
}
pub(crate) fn is_lo(c: char) -> bool {
    match c {
        'a'..='z' | '\u{430}'..='\u{45F}' => true,
        '\0'..='\u{7f}' | '\u{400}'..='\u{42F}' => false,
        _ => c.is_lowercase(),
    }
}
pub(crate) fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
}
pub(crate) fn is_foreign_latin(c: char) -> bool {
    matches!(c as u32, 0xC0..=0x24F | 0x1E00..=0x1EFF) && c.is_alphabetic()
}
pub(crate) fn lower(c: char) -> char {
    match c {
        'A'..='Z' => c.to_ascii_lowercase(),
        '\u{410}'..='\u{42F}' => char::from_u32(c as u32 + 0x20).unwrap_or(c),
        '\u{400}'..='\u{40F}' => char::from_u32(c as u32 + 0x50).unwrap_or(c),
        '\0'..='\u{7f}' | '\u{430}'..='\u{45F}' => c,
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}
pub(crate) fn upper(c: char) -> char {
    match c {
        'a'..='z' => c.to_ascii_uppercase(),
        '\u{430}'..='\u{44F}' => char::from_u32(c as u32 - 0x20).unwrap_or(c),
        '\u{450}'..='\u{45F}' => char::from_u32(c as u32 - 0x50).unwrap_or(c),
        '\0'..='\u{7f}' | '\u{400}'..='\u{42F}' => c,
        _ => c.to_uppercase().next().unwrap_or(c),
    }
}
pub(crate) fn is_vl(c: char) -> bool {
    matches!(lower(c), 'a' | 'e' | 'i' | 'o' | 'u')
}
pub(crate) fn is_vc(c: char) -> bool {
    matches!(
        lower(c),
        'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ў' | 'э' | 'ю' | 'я' | 'ы'
    )
}
pub(crate) fn is_cyr_consonant(c: char) -> bool {
    is_letter(c) && matches!(c as u32, 0x400..=0x52F) && !is_vc(c) && !matches!(lower(c), 'ъ' | 'ь')
}
pub(crate) fn is_og(c: char) -> bool {
    matches!(lower(c), 'o' | 'g')
}
/// Transparent characters (R16).
pub(crate) fn is_transparent(c: char) -> bool {
    matches!(
        c,
        '\u{AD}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
    )
}
pub(crate) fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x300..=0x36F)
}
/// Cyrillic combining pairs (R16): у+breve, и+breve, е+diaeresis.
pub(crate) fn compose(a: char, m: char) -> Option<char> {
    match (a, m) {
        ('у', '\u{306}') => Some('ў'),
        ('У', '\u{306}') => Some('Ў'),
        ('и', '\u{306}') => Some('й'),
        ('И', '\u{306}') => Some('Й'),
        ('е', '\u{308}') => Some('ё'),
        ('Е', '\u{308}') => Some('Ё'),
        _ => None,
    }
}
