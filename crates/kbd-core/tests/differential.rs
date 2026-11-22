//! PLAN section 6: every Latin cell of uz-translit's golden list, typed as keys, against `uz_translit` 1995 -> 2026.
mod common;
use std::collections::BTreeMap;
use uz_translit::{Direction, Options, convert};

fn engine(t: &str) -> String {
    let o = Options {
        latin_upgrade: true,
        ..Options::default()
    };
    convert(t, Direction::ToLatin, &o)
}

#[test]
fn golden_latin_cells_match_the_engine() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../uz-translit/data/golden.tsv"
    ))
    .unwrap();
    let (mut rows, mut same) = (0, 0);
    let mut diffs: BTreeMap<&str, String> = BTreeMap::new();
    for l in text
        .lines()
        .skip(1)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let c: Vec<&str> = l.split('\t').collect();
        if !c[5].starts_with("reviewed:") {
            continue;
        }
        rows += 1;
        let (got, want) = (common::sim(c[1]), engine(c[1]));
        if got == want {
            same += 1;
        } else {
            diffs.insert(c[0], format!("{:?}: keys {got:?}, engine {want:?}", c[1]));
        }
    }
    println!("rows {rows}, equal {same}, differing {diffs:#?}");
    // The complete list of expected differences in the golden list (nothing is skipped silently):
    // - SPN-004: the engine's acronym guard keeps ÖRQ-style all-caps ids 1995; a key press cannot see the word.
    // - QUO-001: a closing quote after "o" reads as an oʻ apostrophe to a keyboard (the engine's quote guard needs
    //   the opening quote, which the TIP does not see). Backspace undo is the escape (O3).
    assert_eq!(
        diffs.keys().copied().collect::<Vec<_>>(),
        ["QUO-001", "SPN-004"]
    );
    assert!(rows >= 1000 && same == rows - 2);
}

/// Differences the golden list does not contain, written down so they are decisions, not accidents.
#[test]
fn known_differences_outside_the_golden_list() {
    // O3: foreign words with ch/sh are converted by the keyboard (the engine's foreign-word guard keeps them).
    for (keys, key_out, eng_out) in [("check", "çeck", "check"), ("show", "şow", "show")] {
        assert_eq!(common::sim(keys), key_out);
        assert_eq!(engine(keys), eng_out);
    }
    // O7: no word-level s+h fixing. The engine has none either on this path (rules.md 4.4: a known input error), so
    // the keyboard and the engine agree: the separator form gives sh, the unseparated word gives ş in both.
    assert_eq!(common::sim("Isʼhoq"), "Ishoq");
    assert_eq!(engine("Isʼhoq"), "Ishoq");
    assert_eq!(common::sim("Ishoq"), "Işoq");
    assert_eq!(engine("Ishoq"), "Işoq");
}
