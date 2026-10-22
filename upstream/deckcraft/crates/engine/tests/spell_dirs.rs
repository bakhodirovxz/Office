// Added by the office shared-dictionaries patch. MIT OR Apache-2.0.
//! A folder added with `spell::add_search_dir` is searched, so the suite's shared dictionaries are found without environment variables.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_engine::spell;

#[test]
fn an_added_search_dir_is_searched() {
    let dir = std::env::temp_dir().join(format!("deckcraft-dicts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("xx_ZZ.aff"), "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzxjq\n").unwrap();
    std::fs::write(dir.join("xx_ZZ.dic"), "2\nsalom\ndunyo\n").unwrap();
    assert!(spell::dictionary("xx-ZZ").is_none(), "nothing yet");
    spell::add_search_dir(dir.clone());
    spell::add_search_dir(dir.clone());
    assert_eq!(spell::search_dirs().iter().filter(|d| **d == dir).count(), 1, "added once");
    // The cache keeps the miss of the first call, so look the pair up directly.
    assert!(spell::find_in(&spell::search_dirs(), "xx-ZZ").is_some());
    let _ = std::fs::remove_dir_all(dir);
}

/// The dictionaries the suite ships (`assets/dictionaries`, repository root) parse with this checker: common words pass, a typo does not and gets a suggestion.
#[test]
fn the_shipped_dictionaries_check_real_words() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../assets/dictionaries");
    if !dir.is_dir() {
        return; // an upstream-only checkout has no suite assets
    }
    let load = |stem: &str| {
        let (aff, dic) = (std::fs::read(dir.join(format!("{stem}.aff"))).unwrap(), std::fs::read(dir.join(format!("{stem}.dic"))).unwrap());
        spell::Dictionary::parse(&aff, &dic)
    };
    let en = load("en_US");
    assert!(en.check("presentation") && en.check("slides") && en.check("The"));
    assert!(!en.check("presentaiton"));
    assert!(en.suggest("presentaiton", 5).iter().any(|s| s == "presentation"));
    let ru = load("ru_RU");
    assert!(ru.check("привет") && ru.check("презентация") && ru.check("слайды"));
    assert!(!ru.check("превет"));
    assert!(ru.suggest("превет", 8).iter().any(|s| s == "привет"), "{:?}", ru.suggest("превет", 8));
}
