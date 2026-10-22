//! Legacy text encodings (owner decision, WP9 fixes). A `.txt`, `.csv`, `.tsv`, `.md` or `.html` file in cp1251, cp1252 or UTF-16
//! (a Word "Save as text" on a Russian Windows gives cp1251) is transcoded to UTF-8 in the job's private copy before any tool sees
//! it, and the report says so (`text_transcoded`). Detection order: a BOM, then valid UTF-8, then a cp1251-versus-cp1252 guess from
//! how the high bytes sit in words. `shortcut: only these two 8-bit code pages and UTF-16 with a BOM are guessed; a file in another
//! code page (cp1250, koi8-r) is read as cp1252 or cp1251 and comes out wrong, and UTF-16 without a BOM is not recognised. Add a
//! detector when such a file turns up.`
use crate::exec::{Exit, Failure, fail};
use crate::kind::Kind;
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1251, WINDOWS_1252};
use std::fs;
use std::path::Path;

/// The kinds that are plain text (read as UTF-8 by the tools).
pub fn is_text(k: Kind) -> bool {
    matches!(k, Kind::Txt | Kind::Md | Kind::Csv | Kind::Tsv | Kind::Html)
}

/// Bytes that are letters in cp1251 (A-ya, plus the ones outside that block: Yo, yo, short U, Ukrainian letters).
fn cyrillic_letter(b: u8) -> bool {
    b >= 0xC0
        || matches!(
            b,
            0xA1 | 0xA2 | 0xA8 | 0xAA | 0xAF | 0xB2 | 0xB3 | 0xB4 | 0xB8 | 0xBA | 0xBF
        )
}

/// cp1251 or cp1252 for 8-bit text. Cyrillic words are runs of high bytes (a, b, c all above 0xC0 in cp1251), while the accented
/// letters of a western language sit alone between ASCII letters (cafe with an accent). So: cp1251 when most of the high letters
/// are in runs of three or more.
fn eight_bit(b: &[u8]) -> &'static Encoding {
    let (mut high, mut in_runs, mut run) = (0u32, 0u32, 0u32);
    for &x in b.iter().chain(std::iter::once(&0u8)) {
        if cyrillic_letter(x) {
            high += 1;
            run += 1;
        } else {
            if run >= 3 {
                in_runs += run;
            }
            run = 0;
        }
    }
    if in_runs >= 3 && in_runs * 2 >= high {
        WINDOWS_1251
    } else {
        WINDOWS_1252
    }
}

/// The legacy encoding of `bytes`, or `None` when it is UTF-8 (with or without a BOM) or not text we recognise.
pub fn guess(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Some(UTF_16LE);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Some(UTF_16BE);
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        || std::str::from_utf8(bytes).is_ok()
        || bytes.contains(&0)
    {
        return None;
    }
    Some(eight_bit(bytes))
}

/// How the report names an encoding.
pub fn label(e: &'static Encoding) -> &'static str {
    match e.name() {
        "windows-1251" => "cp1251 (windows-1251)",
        "windows-1252" => "cp1252 (windows-1252)",
        "UTF-16LE" => "UTF-16 LE",
        "UTF-16BE" => "UTF-16 BE",
        "UTF-8" => "UTF-8 (a few damaged bytes were replaced)",
        other => other,
    }
}

/// The decoded text and the encoding it was read as (`None`: it was UTF-8 already and `bytes` is unchanged).
pub fn to_utf8(bytes: &[u8]) -> Option<(String, &'static Encoding)> {
    let enc = guess(bytes)?;
    // UTF-8 with a stray bad byte (a paste, a cut multi-byte letter) is UTF-8, not cp1252: keep the letters, replace the bytes (m11)
    if enc == WINDOWS_1251 || enc == WINDOWS_1252 {
        let lossy = String::from_utf8_lossy(bytes);
        let high = bytes.iter().filter(|b| **b >= 0x80).count();
        let bad = lossy.chars().filter(|c| *c == '\u{fffd}').count();
        if high >= 16 && bad > 0 && bad * 50 <= high {
            return Some((lossy.into_owned(), UTF_8));
        }
    }
    // `decode` honours a BOM (and drops it); an 8-bit page never fails, a bad UTF-16 pair becomes U+FFFD
    let (text, _, _) = enc.decode(bytes);
    Some((text.into_owned(), enc))
}

/// Rewrite `path` as UTF-8 when it is text in a legacy encoding. Returns the encoding's label when it did.
pub fn normalise(path: &Path) -> Result<Option<&'static str>, Failure> {
    let io = |e: std::io::Error| Failure {
        exit: Exit::ToolError,
        msg: format!("cannot read the staged text file: {e}"),
    };
    let bytes = fs::read(path).map_err(io)?;
    let Some((text, enc)) = to_utf8(&bytes) else {
        return Ok(None);
    };
    if text.is_empty() {
        return fail(
            Exit::Rejected,
            "the file is empty (it holds only a byte order mark)",
        );
    }
    fs::write(path, text.as_bytes()).map_err(io)?;
    Ok(Some(label(enc)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str) -> Vec<u8> {
        [
            &[0xFF, 0xFE][..],
            &s.encode_utf16()
                .flat_map(|c| c.to_le_bytes())
                .collect::<Vec<_>>(),
        ]
        .concat()
    }

    #[test]
    fn utf8_is_left_alone_and_a_bom_wins() {
        assert!(
            guess("Salom dunyo, \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}".as_bytes()).is_none()
        );
        assert!(guess(b"plain ascii").is_none());
        assert!(guess(b"\xEF\xBB\xBFhello").is_none());
        assert_eq!(guess(&utf16le("a")).map(|e| e.name()), Some("UTF-16LE"));
        assert_eq!(
            guess(&[0xFE, 0xFF, 0, b'a']).map(|e| e.name()),
            Some("UTF-16BE")
        );
    }

    #[test]
    fn utf16_both_orders_decode() {
        assert_eq!(
            to_utf8(&utf16le("Salom, \u{43c}\u{438}\u{440}")).unwrap().0,
            "Salom, \u{43c}\u{438}\u{440}"
        );
        let be = [
            &[0xFE, 0xFF][..],
            &"Ok \u{43c}"
                .encode_utf16()
                .flat_map(|c| c.to_be_bytes())
                .collect::<Vec<_>>(),
        ]
        .concat();
        assert_eq!(to_utf8(&be).unwrap().0, "Ok \u{43c}");
    }

    #[test]
    fn cyrillic_is_cp1251_and_accented_latin_is_cp1252() {
        let (cyr, _, _) = WINDOWS_1251.encode("\u{411}\u{443}\u{439}\u{440}\u{443}\u{49b} 125-\u{43a}\n\u{422}\u{43e}\u{448}\u{43a}\u{435}\u{43d}\u{442} 2026");
        let (t, e) = to_utf8(&cyr).unwrap();
        assert_eq!(e.name(), "windows-1251");
        assert!(t.starts_with("\u{411}\u{443}\u{439}\u{440}\u{443}"), "{t}");
        // a Russian sentence with latin product names in it
        let (mixed, _, _) =
            WINDOWS_1251.encode("Приказ № 5 по HRM: премии за октябрь, see order 12");
        assert_eq!(to_utf8(&mixed).unwrap().1.name(), "windows-1251");
        let (fr, _, _) = WINDOWS_1252.encode("Le caf\u{e9} du d\u{e9}jeuner co\u{fb}te 5 \u{20ac}; na\u{ef}ve \u{e0} la fa\u{e7}on, \u{fc}ber, se\u{f1}or, \u{201c}quoted\u{201d}");
        let (t, e) = to_utf8(&fr).unwrap();
        assert_eq!(e.name(), "windows-1252");
        assert!(t.contains("caf\u{e9}") && t.contains('\u{20ac}'), "{t}");
    }

    #[test]
    fn utf8_with_one_bad_byte_stays_utf8() {
        let text = "Приказ № 5 по HRM: премии за октябрь, Ходимларни рағбатлантириш";
        let mut b = text.as_bytes().to_vec();
        b.insert(10, 0xFF);
        let (t, e) = to_utf8(&b).unwrap();
        assert_eq!(e.name(), "UTF-8");
        assert!(
            t.contains("премии за октябрь") && t.contains('\u{fffd}'),
            "{t}"
        );
        // real cp1251 is still cp1251
        let (cyr, _, _) = WINDOWS_1251.encode(text);
        assert_eq!(to_utf8(&cyr).unwrap().1.name(), "windows-1251");
    }

    #[test]
    fn normalise_rewrites_the_file_and_names_the_encoding() {
        let dir = std::env::temp_dir().join(format!("convert-text-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        let (cyr, _, _) = WINDOWS_1251.encode("Буйруқ тест, Ходимларни рағбатлантириш");
        fs::write(&p, &cyr).unwrap();
        assert_eq!(normalise(&p).unwrap(), Some("cp1251 (windows-1251)"));
        assert!(fs::read_to_string(&p).unwrap().starts_with("Буйру"));
        assert_eq!(normalise(&p).unwrap(), None); // now UTF-8
        fs::write(&p, [0xFF, 0xFE]).unwrap();
        assert_eq!(normalise(&p).unwrap_err().exit, Exit::Rejected);
        let _ = fs::remove_dir_all(&dir);
    }
}
