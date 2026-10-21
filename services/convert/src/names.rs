//! File-name safety (PLAN 5.5), a property of the crate so that no caller has to remember it: a name that comes from a client
//! upload or from document content (a sheet name) can never be a path, a Windows device, or carry control or bidi characters.

const RESERVED: [&str; 24] = [
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3", "COM4", "COM5",
    "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
    "LPT9",
];
const MAX_CHARS: usize = 200;

fn bad(c: char) -> bool {
    c.is_control()
        || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        || matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// A single path component made from `s`: no separators or `..`, no control or bidi characters, no Windows reserved
/// base name (also with an extension or trailing dots/spaces), at most 200 characters; `fallback` when nothing is left.
pub fn safe_name(s: &str, fallback: &str) -> String {
    let mapped: String = s.chars().map(|c| if bad(c) { '_' } else { c }).collect();
    let trimmed = mapped.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let mut out: String = trimmed.chars().take(MAX_CHARS).collect();
    out = out
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();
    if out.is_empty() || out.chars().all(|c| c == '_') {
        return fallback.to_string();
    }
    // Windows also treats the superscript digits as device numbers (`COM¹`)
    let base: String = out
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .chars()
        .map(|c| match c {
            '\u{B9}' => '1',
            '\u{B2}' => '2',
            '\u{B3}' => '3',
            c => c.to_ascii_uppercase(),
        })
        .collect();
    if RESERVED.contains(&base.as_str()) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_names_are_made_safe() {
        let s = |x: &str| safe_name(x, "output");
        assert_eq!(s("report"), "report");
        assert_eq!(s("Buyruq \u{2116}5 (2026)"), "Buyruq \u{2116}5 (2026)");
        assert_eq!(s(r"..\..\x"), "_.._x");
        for evil in ["..", "...", "", "   ", "/", r"\\", r"..\.."] {
            let n = s(evil);
            assert!(
                !n.contains(['/', '\\']) && n != ".." && !n.is_empty(),
                "{evil:?} -> {n:?}"
            );
        }
        assert_eq!(s("NUL"), "_NUL");
        assert_eq!(s("con.txt"), "_con.txt");
        assert_eq!(s("COM1 ."), "_COM1");
        assert_eq!(s("Lpt9"), "_Lpt9");
        assert_eq!(s("a\u{202E}b\0c"), "a_b_c");
        assert_eq!(s(&"x".repeat(500)).chars().count(), 200);
        assert_eq!(s("???"), "output");
        assert_eq!(s("CONSOLE"), "CONSOLE");
        // invisible and bidi characters (ALM, zero-width, word joiner) and the device names Windows adds to the list
        assert_eq!(s("a\u{061C}b\u{200B}c\u{200D}d\u{2060}e"), "a_b_c_d_e");
        assert_eq!(s("COM\u{B9}.txt"), "_COM\u{B9}.txt");
        assert_eq!(s("lpt\u{B3}"), "_lpt\u{B3}");
        assert_eq!(s("CONIN$"), "_CONIN$");
        assert_eq!(s("conout$.log"), "_conout$.log");
    }
}
