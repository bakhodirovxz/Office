//! Golden lists (WP3). Skipped when a file is absent; only `reviewed:*` rows are asserted.
use std::{collections::HashMap, fs, path::Path};
use uz_translit::{Direction, Options, convert};

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
