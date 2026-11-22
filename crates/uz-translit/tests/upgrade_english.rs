//! `Options::upgrade_skip_english`: a paragraph with >= 2 distinct English function words gets no Latin upgrade.
use std::{fs, path::Path};
use uz_translit::{Direction, Options, convert, looks_english};

fn opts(skip: bool) -> Options {
    Options {
        latin_upgrade: true,
        upgrade_skip_english: skip,
        ..Options::default()
    }
}
fn l(s: &str, skip: bool) -> String {
    convert(s, Direction::ToLatin, &opts(skip))
}

#[test]
fn english_paragraph_keeps_its_latin_uzbek_ones_are_upgraded() {
    let uz = "Men oʻqituvchiman, shahar markazida yashayman.";
    let cy = "Мен мактабга бораман.";
    let en = "The shop is open; she wishes to check the church. Run git push --force.";
    assert_eq!(l(uz, true), "Men öqituvçiman, şahar markazida yaşayman.");
    assert_eq!(l(cy, true), "Men maktabga boraman.");
    assert_eq!(l(en, true), en);
    // without the option the English words are mangled (the reason for it)
    assert_eq!(
        l(en, false),
        "The şop is open; şe wishes to check the çurç. Run git puş --force."
    );
    // Cyrillic inside an English paragraph is still converted
    assert_eq!(
        l("This is the shop: мактаб", true),
        "This is the shop: maktab"
    );
}

#[test]
fn one_function_word_is_not_enough() {
    let t = "The shop opens at nine; she asks for church tea.";
    assert!(!looks_english("The shop"));
    // "the" + "for" = 2 here; with only "the" the paragraph is treated as Uzbek and upgraded
    let one = "The shop";
    println!("1 word: {} -> {}", one, l(one, true));
    assert_eq!(l(one, true), "The şop");
    assert!(looks_english(t));
    assert_eq!(l(t, true), t);
    assert!(!looks_english("the the the"), "distinct words only");
}

#[test]
fn no_golden_uzbek_latin_is_taken_for_english() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut n = 0;
    let mut bad = Vec::new();
    for (file, col) in [
        ("golden.tsv", 1),
        ("variants.tsv", 2),
        ("exceptions.tsv", 0),
    ] {
        let txt = fs::read_to_string(dir.join(file)).unwrap();
        for row in txt.lines().skip(1).filter(|r| !r.starts_with('#')) {
            let Some(latin) = row.split('\t').nth(col) else {
                continue;
            };
            n += 1;
            if looks_english(latin) {
                bad.push(latin.to_string());
            }
        }
    }
    println!(
        "{n} Uzbek Latin entries checked, {} wrongly skipped: {bad:?}",
        bad.len()
    );
    assert!(bad.is_empty(), "{bad:?}");
}
