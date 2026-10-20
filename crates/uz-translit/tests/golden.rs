//! Golden lists (WP3). Skipped when a file is absent; only `reviewed:*` rows are asserted.
use std::{collections::HashMap, fs, path::Path};
mod common;
use uz_translit::{Direction, LatinAlphabet, Options, convert};

fn table(name: &str) -> Vec<HashMap<String, String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(name);
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut lines = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .map(|h| h.split('\t').map(str::to_string).collect())
        .unwrap_or_default();
    lines
        .map(|l| {
            head.iter()
                .cloned()
                .zip(
                    l.split('\t')
                        .map(str::to_string)
                        .chain(std::iter::repeat(String::new())),
                )
                .collect()
        })
        .collect()
}

fn check(file: &str, input_col: &str) -> (usize, usize) {
    let (mut fwd, mut back) = (0, 0);
    let mut fails = Vec::new();
    for r in table(file)
        .into_iter()
        .filter(|r| r["status"].starts_with("reviewed:"))
    {
        let dir = r.get("dir").map_or("l2c", String::as_str);
        let (lat, cyr) = (&r[input_col], &r["cyrillic"]);
        let o = Options::default();
        if matches!(dir, "both" | "l2c") {
            fwd += 1;
        }
        if matches!(dir, "both" | "c2l") && file != "variants.tsv" {
            back += 1;
        }
        if matches!(dir, "both" | "l2c") && convert(lat, Direction::ToCyrillic, &o) != *cyr {
            fails.push(format!(
                "{} l2c {lat:?} -> {:?}, expected {cyr:?}",
                r["id"],
                convert(lat, Direction::ToCyrillic, &o)
            ));
        }
        if matches!(dir, "both" | "c2l") && file != "variants.tsv" {
            let want = r.get("latin").unwrap_or(lat);
            if convert(cyr, Direction::ToLatin, &o) != *want {
                fails.push(format!(
                    "{} c2l {cyr:?} -> {:?}, expected {want:?}",
                    r["id"],
                    convert(cyr, Direction::ToLatin, &o)
                ));
            }
        }
    }
    assert!(
        fails.is_empty(),
        "{file}: {} mismatches\n{}",
        fails.len(),
        fails.join("\n")
    );
    println!("{file}: {fwd} latin->cyrillic rows, {back} cyrillic->latin rows checked");
    (fwd, back)
}

#[test]
fn golden() {
    let (f, b) = check("golden.tsv", "latin");
    if f > 0 {
        assert!(
            f >= 500 && b >= 500,
            "plan: at least 500 reviewed both-direction rows"
        );
    }
}
#[test]
fn sentences() {
    check("sentences.tsv", "latin");
}
#[test]
fn variants() {
    check("variants.tsv", "input");
}

/// Section 15: the approved both-direction rows under the 2026 alphabet. The 2026 Latin is the golden Latin
/// passed through the 1995 -> 2026 letter substitution (`common::to_2026`) at test time.
#[test]
fn golden_2026() {
    let o95 = Options::default();
    let o26 = Options {
        latin_alphabet: LatinAlphabet::Y2026,
        ..o95
    };
    let (mut n, mut skipped, mut fails) = (0, 0, Vec::new());
    for r in table("golden.tsv")
        .into_iter()
        .filter(|r| r["status"].starts_with("reviewed:") && r["dir"] == "both")
    {
        let (lat, cyr) = (&r["latin"], &r["cyrillic"]);
        // R15 quote guard rows: the substitution would read a closing quote as oʻ. s'h cannot round-trip (2026 writes plain "sh", which reads as ш), and rows that keep Latin text in
        // the Cyrillic output (protected spans, foreign words) are not letter-substituted by the engine.
        if r["rule"].contains("R15")
            || common::has_separated_sh(lat)
            || cyr.chars().any(|c| c.is_ascii_alphabetic())
        {
            skipped += 1;
            continue;
        }
        n += 1;
        let l26 = common::to_2026(lat);
        let back = convert(cyr, Direction::ToLatin, &o26);
        if back != l26 {
            fails.push(format!(
                "{} c2l {cyr:?} -> {back:?}, expected {l26:?}",
                r["id"]
            ));
        }
        let fwd = convert(&l26, Direction::ToCyrillic, &o95);
        if fwd != *cyr {
            fails.push(format!(
                "{} l2c {l26:?} -> {fwd:?}, expected {cyr:?}",
                r["id"]
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "{} mismatches\n{}",
        fails.len(),
        fails.join("\n")
    );
    println!("golden_2026: {n} rows checked in both directions, {skipped} skipped");
    assert!(n >= 500);
}
