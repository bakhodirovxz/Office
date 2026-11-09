//! Telling Uzbek and Russian text apart from English, so a document whose runs are tagged
//! `en-US` (what many generators write) isn't covered in red underlines (as Word's "detect
//! language automatically" does). Like [`crate::vi`], the test is orthographic and runs on the
//! paragraph's text only.
//!
//! - **Cyrillic.** English has no Cyrillic letters, so a paragraph mostly in Cyrillic is Russian
//!   ([`ProofLang::Ru`]); one with a letter only Uzbek Cyrillic uses (`ў қ ғ ҳ`) is Uzbek, which
//!   has no proofing tools here ([`ProofLang::Other`]: no marks).
//! - **Latin.** Uzbek Latin writes `oʻ` and `gʻ` (the modifier letter or an apostrophe after `o`
//!   or `g`), unlike English except `o'clock`, and uses a few frequent words no English text
//!   has (`va`, `bilan`, `uchun`, `hujjat`…); see [`looks_uzbek_latin`] and [`has_uzbek_word`].

use crate::lang::ProofLang;

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}')
}

/// Letters only the Uzbek Cyrillic alphabet has among the Cyrillic ones used with Russian.
fn uzbek_cyrillic_letter(c: char) -> bool {
    matches!(c, 'ў' | 'Ў' | 'қ' | 'Қ' | 'ғ' | 'Ғ' | 'ҳ' | 'Ҳ')
}

/// The language of a paragraph written mostly in Cyrillic: Russian, or Uzbek (no tools) when it
/// has an Uzbek-only letter. `None` when Latin letters are at least as many (English with a
/// Russian word in it stays English).
pub fn cyrillic_lang(text: &str) -> Option<ProofLang> {
    let (mut cyr, mut other) = (0usize, 0usize);
    let mut uzbek = false;
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        if is_cyrillic(c) {
            cyr += 1;
            uzbek |= uzbek_cyrillic_letter(c);
        } else {
            other += 1;
        }
    }
    (cyr > other).then_some(if uzbek { ProofLang::Other } else { ProofLang::Ru })
}

/// Is `c` an apostrophe-like mark that Uzbek Latin writes after `o` and `g`?
fn uzbek_mark(c: char) -> bool {
    matches!(c, '\u{02BB}' | '\u{02BC}' | '\u{2018}' | '\u{2019}' | '\'' | '`' | '\u{00B4}')
}

/// Frequent Uzbek Latin words (and the vocabulary of office documents) that are not English words.
const UZBEK_WORDS: &[&str] = &[
    "va",
    "bu",
    "bir",
    "uchun",
    "bilan",
    "yoki",
    "emas",
    "kerak",
    "haqida",
    "sifatida",
    "qilish",
    "ushbu",
    "biz",
    "siz",
    "ular",
    "qanday",
    "nima",
    "bugun",
    "sana",
    "ism",
    "familiya",
    "ariza",
    "buyruq",
    "hujjat",
    "xodim",
    "lavozim",
    "mehnat",
    "tashkilot",
    "raqam",
    "imzo",
    "manzil",
];

fn is_uzbek_word(w: &str) -> bool {
    let w = w.to_lowercase();
    UZBEK_WORDS.contains(&w.as_str())
}

fn words_of(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || uzbek_mark(c))).filter(|w| !w.is_empty())
}

/// Does the paragraph have at least one frequent Uzbek word (`va`, `bilan`, `hujjat`…)?
pub fn has_uzbek_word(text: &str) -> bool {
    words_of(text).any(is_uzbek_word)
}

/// Does a paragraph read as Uzbek Latin? At least two of its words carry `oʻ`/`gʻ` or are frequent
/// Uzbek words (any, in a text of up to three words), and they are at least a sixth of its words:
/// an English paragraph with one `o'clock` or a name like `Sana` stays English.
pub fn looks_uzbek_latin(text: &str) -> bool {
    let (mut total, mut marked) = (0usize, 0usize);
    for w in words_of(text) {
        total += 1;
        let cs: Vec<char> = w.chars().collect();
        // `o'` or `g'` followed by a letter.
        if is_uzbek_word(w)
            || cs.windows(3).any(|t| matches!(t, [a, m, b] if matches!(a, 'o' | 'O' | 'g' | 'G') && uzbek_mark(*m) && b.is_alphabetic()))
        {
            marked += 1;
        }
    }
    marked > 0 && (marked >= 2 || total <= 3) && marked * 6 >= total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrillic_paragraphs() {
        assert_eq!(cyrillic_lang("Это тестовый документ для проверки"), Some(ProofLang::Ru));
        assert_eq!(cyrillic_lang("Бу ўзбекча ҳужжат намунаси"), Some(ProofLang::Other));
        assert_eq!(cyrillic_lang("The report на русском is ready"), None);
        assert_eq!(cyrillic_lang("Plain English text"), None);
    }

    #[test]
    fn uzbek_latin_cues() {
        assert!(looks_uzbek_latin("Bu hujjat oʻzbek tilida, goʻzal va oʻqishli"));
        assert!(looks_uzbek_latin("Bu o'zbek tilidagi g'oya haqida"));
        assert!(looks_uzbek_latin("Oʻzbekiston"));
        assert!(!looks_uzbek_latin("The meeting starts at nine o'clock in the morning at the main office"));
        assert!(!looks_uzbek_latin("Plain English text with don't and it's"));
        // Frequent words: the paragraph, a form label, but not one name inside English.
        assert!(looks_uzbek_latin("Bugun biz yangi hujjat tayyorladik va uni tekshirdik"));
        assert!(looks_uzbek_latin("Familiya, ism"));
        assert!(!looks_uzbek_latin("The report was written by Sana for the quarterly meeting of the board"));
        assert!(has_uzbek_word("Toshkent shahri va tumani") && !has_uzbek_word("Toshkent shahri"));
    }
}
