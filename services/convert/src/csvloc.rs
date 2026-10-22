//! B2 (gate review): a CSV or TSV written in the ru/uz style (`;` or tab delimiter, `dd.mm.yyyy` dates, decimal commas) that is read
//! in an en-US job swaps day and month, or keeps numbers as text, and the tool says nothing. This looks at the staged UTF-8 copy
//! and says whether the content looks like that style, so the job can warn (`csv_locale_mismatch`).
//! `shortcut: only the first 64 KB / 200 lines are looked at, and ru, uz-Latn and uz-Cyrl cannot be told apart (they agree on all three signals); a
//! file that mixes styles is judged by any one signal. Refine if a false warning is reported.`
use std::io::Read;
use std::path::Path;

/// The delimiter of the header line: the one of `,` `;` tab that occurs most outside quotes (ties go to `,`, then `;`).
fn delimiter(line: &str) -> char {
    let (mut best, mut n_best) = (',', 0);
    for d in [',', ';', '\t'] {
        let (mut q, mut n) = (false, 0);
        for c in line.chars() {
            if c == '"' {
                q = !q;
            } else if c == d && !q {
                n += 1;
            }
        }
        if n > n_best {
            (best, n_best) = (d, n);
        }
    }
    best
}

fn digits(s: &str, lo: usize, hi: usize) -> bool {
    (lo..=hi).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// `1.3.2019`, `01.03.2019`, `01.03.2019 14:30`
fn dotted_date(cell: &str) -> bool {
    let t = cell.split_whitespace().next().unwrap_or("");
    let p: Vec<&str> = t.split('.').collect();
    p.len() == 3 && digits(p[0], 1, 2) && digits(p[1], 1, 2) && digits(p[2], 4, 4)
}

/// `3500000,0`, `-12,5`, `1 200,50`; not `1,234` (a thousands group in en-US).
fn decimal_comma(cell: &str) -> bool {
    let t = cell.trim().trim_start_matches('-');
    let Some((int, frac)) = t.split_once(',') else {
        return false;
    };
    let int = int.replace([' ', '\u{a0}'], "");
    digits(&int, 1, 30) && digits(frac, 1, 12) && !(frac.len() == 3 && int.len() <= 3)
}

/// True when the text looks like a ru/uz-style CSV. `tsv`: a tab delimiter is that format's normal one, so it is no signal there.
pub fn looks_ru_uz(text: &str, tsv: bool) -> bool {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let Some(head) = lines.next() else {
        return false;
    };
    let d = delimiter(head);
    if d == ';' || (d == '\t' && !tsv) {
        return true;
    }
    std::iter::once(head).chain(lines.take(200)).any(|l| {
        l.split(d)
            .map(|c| c.trim().trim_matches('"'))
            .any(|c| dotted_date(c) || (d != ',' && decimal_comma(c)))
    })
}

/// The first 64 KB of the staged file (already UTF-8 after `text::normalise`), cut at a character boundary.
pub fn head(path: &Path) -> String {
    let mut b = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(64 * 1024).read_to_end(&mut b);
    }
    let s = String::from_utf8_lossy(&b);
    s.trim_end_matches('\u{fffd}').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excel_ru_csv_is_flagged() {
        assert!(looks_ru_uz("Date;Salary\n01.03.2019;3500000,0\n", false));
    }
    #[test]
    fn a_decimal_comma_or_dotted_date_in_a_tsv_is_flagged_but_the_tab_is_not() {
        assert!(looks_ru_uz("a\tb\n01.03.2019\t1\n", true));
        assert!(looks_ru_uz("a\tb\nx\t12,5\n", true));
        assert!(!looks_ru_uz("a\tb\nx\t12.5\n", true));
        assert!(looks_ru_uz("a\tb\nx\t12.5\n", false));
    }
    #[test]
    fn an_en_us_csv_is_not_flagged() {
        assert!(!looks_ru_uz(
            "Date,Salary,Name\n1/3/2019,\"3,500,000.50\",\"Smith, J\"\n2019-03-01,1,234,x\n",
            false
        ));
        assert!(!looks_ru_uz("", false));
    }
    #[test]
    fn quoted_decimal_commas_in_a_comma_csv_are_not_cells_of_their_own() {
        // "12,5" quoted in a comma file is ambiguous and en-US reads it as text anyway; only the dotted date counts there
        assert!(!looks_ru_uz("a,b\n\"12,5\",1\n", false));
        assert!(looks_ru_uz("a,b\n01.03.2019,1\n", false));
    }
}
