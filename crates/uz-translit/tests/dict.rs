//! The exceptions dictionary against the rules (rules.md 6, 9.2): every reviewed row, with and without suffixes.
use uz_translit::{Direction, Options, convert};

const TSV: &str = include_str!("../data/exceptions.tsv");
const SUFFIXES: [&str; 14] = [
    "da", "dan", "ga", "ni", "ning", "lar", "larni", "lari", "i", "si", "imiz", "ingiz", "dagi",
    "gacha",
];

fn c(s: &str) -> String {
    convert(s, Direction::ToCyrillic, &Options::default())
}
fn l(s: &str) -> String {
    convert(s, Direction::ToLatin, &Options::default())
}

struct Row<'a> {
    latin: &'a str,
    cyr: &'a str,
    kind: &'a str,
    dir: &'a str,
}

fn rows() -> Vec<Row<'static>> {
    TSV.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("latin\t") && !l.is_empty())
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f[5].starts_with("reviewed:"))
        .map(|f| Row {
            latin: f[0],
            cyr: f[1],
            kind: f[2],
            dir: f[3],
        })
        .collect()
}

#[test]
fn seed_counts() {
    let all = TSV
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("latin\t") && !l.is_empty())
        .count();
    assert!(all >= 191, "154 words + 4 acronyms + 33 keep entries");
    assert!(rows().len() >= 185);
}

#[test]
fn every_row_in_both_directions() {
    for r in rows() {
        match r.kind {
            "keep" => assert_eq!(c(r.latin), r.latin, "keep {}", r.latin),
            _ => {
                if r.dir != "c2l" {
                    assert_eq!(c(r.latin), r.cyr, "l2c {}", r.latin);
                    if r.kind != "acro" {
                        assert_eq!(
                            c(&r.latin.to_uppercase()).to_lowercase(),
                            r.cyr,
                            "upper {}",
                            r.latin
                        );
                    }
                }
                if matches!(r.dir, "both" | "l2c") && r.kind != "acro" {
                    assert_eq!(l(r.cyr), r.latin, "c2l by rule or dictionary {}", r.cyr);
                }
                if r.kind == "acro" {
                    assert_eq!(l(r.cyr), r.latin, "acro {}", r.cyr);
                }
            }
        }
    }
}

#[test]
fn stems_round_trip_with_suffixes() {
    for r in rows()
        .into_iter()
        .filter(|r| r.kind == "stem" && r.dir != "variant")
    {
        for s in SUFFIXES {
            let lat = format!("{}{}", r.latin, s);
            let cy = c(&lat);
            assert!(cy.starts_with(r.cyr.trim_end_matches('ь')), "{lat} -> {cy}");
            assert_eq!(l(&cy), lat, "round trip {lat} -> {cy}");
        }
    }
}

#[test]
fn variants_are_one_way() {
    assert_eq!(c("sentyabr"), "сентябрь");
    assert_eq!(l("сентябрь"), "sentabr");
}

#[test]
fn case_transfer() {
    assert_eq!(c("Sentabr"), "Сентябрь");
    assert_eq!(c("SENTABRDAN"), "СЕНТЯБРДАН");
    assert_eq!(l("СЕНТЯБРЬ Сентябрь"), "SENTABR Sentabr");
    assert_eq!(c("kompyuterlar"), "компьютерлар");
    assert_eq!(c("Kompyuterlar KOMPYUTER"), "Компьютерлар КОМПЬЮТЕР");
}

#[test]
fn wp3_review_changes() {
    assert_eq!(c("medalyon medal medallar"), "медальон медаль медаллар");
    assert_eq!(c("pochtalon"), "почтальон");
    assert_eq!(l("почтальон"), "pochtalyon");
    assert_eq!(c("bilyard mayonez sink"), "бильярд майонез цинк");
}
