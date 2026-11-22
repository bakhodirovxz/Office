//! The PLAN 2.1 table, R10 case, Backspace undo, apostrophes, and exhaustive short sequences.
mod common;
use common::{BACK, run, sim};
use kbd_core::{Act, Key, decide, plain, shown};

#[test]
fn shown_follows_r10() {
    for (t, want) in [
        ("s", "s"),
        ("sh", "ş"),
        ("Sh", "Ş"),
        ("SH", "Ş"),
        ("sH", "ş"),
        ("ch", "ç"),
        ("Ch", "Ç"),
        ("CH", "Ç"),
        ("cH", "ç"),
        ("o'", "ö"),
        ("O'", "Ö"),
        ("g'", "ğ"),
        ("G'", "Ğ"),
        ("s'", "s'"),
        ("c'", "c'"),
        ("", ""),
        ("x", "x"),
    ] {
        assert_eq!(shown(t), want, "{t:?}");
    }
}

#[test]
fn every_apostrophe_of_set_a_converts_o_and_g() {
    for a in [
        '\'', '`', '\u{2BB}', '\u{2BC}', '\u{2018}', '\u{2019}', '\u{2BF}', '\u{B4}',
    ] {
        assert_eq!(
            sim(&format!("bo{a}lsa to{a}g{a}ri")),
            "bölsa töğri",
            "{a:?}"
        );
        assert_eq!(sim(&format!("Is{a}hoq")), "Ishoq", "{a:?}");
        assert_eq!(sim(&format!("o{a}{BACK}")), format!("o{a}"), "{a:?}");
    }
}

#[test]
fn plan_table_rows() {
    let ch = Key::Ch;
    // idle + s/c/o/g any case starts a composition; h and the apostrophe pass
    for c in "scogSCOG".chars() {
        assert_eq!(decide("", false, ch(c)), Some(Act::Start(c.to_string())));
    }
    assert_eq!(decide("", false, ch('h')), None);
    assert_eq!(decide("", false, ch('\'')), None);
    // pending s/c + h, pending o/g + apostrophe: Update (the composition now shows the letter)
    assert_eq!(decide("s", true, ch('h')), Some(Act::Update("sh".into())));
    assert_eq!(decide("C", true, ch('H')), Some(Act::Update("CH".into())));
    assert_eq!(
        decide("o", true, ch('\u{2BB}')),
        Some(Act::Update("o\u{2BB}".into()))
    );
    assert_eq!(decide("G", true, ch('`')), Some(Act::Update("G`".into())));
    // pending s + apostrophe, then h: plain sh, finished
    assert_eq!(decide("s", true, ch('\'')), Some(Act::Update("s'".into())));
    assert_eq!(decide("S'", true, ch('h')), Some(Act::Finish("Sh".into())));
    // Backspace: single letter removed; converted letter -> typed keys; s' -> s
    assert_eq!(
        decide("s", true, Key::Back),
        Some(Act::Finish(String::new()))
    );
    assert_eq!(
        decide("Sh", true, Key::Back),
        Some(Act::Finish("Sh".into()))
    );
    assert_eq!(decide("s'", true, Key::Back), Some(Act::Update("s".into())));
    assert_eq!(decide("", false, Key::Back), None);
    // any other key commits
    assert_eq!(decide("sh", true, Key::Other), Some(Act::Commit));
    assert_eq!(decide("s", true, ch('x')), Some(Act::Commit));
    assert_eq!(decide("sh", true, ch('h')), Some(Act::Commit));
    assert_eq!(decide("", false, Key::Other), None);
    // a new starter commits the old composition first
    assert_eq!(decide("sh", true, ch('c')), Some(Act::Start("c".into())));
    assert_eq!(decide("c", true, ch('\'')), Some(Act::Commit));
}

#[test]
fn cases() {
    for (typed, want) in [
        ("shahar ", "şahar "),
        ("Shahar ", "Şahar "),
        ("SHAHAR ", "ŞAHAR "),
        ("sHa ", "şa "),
        ("Sha ", "Şa "),
        ("choy ", "çoy "),
        ("Choy ", "Çoy "),
        ("CHOY ", "ÇOY "),
        ("cHoy ", "çoy "),
        ("o'zbek ", "özbek "),
        ("O'zbek ", "Özbek "),
        ("O'ZBEK ", "ÖZBEK "),
        ("g'isht ", "ğişt "),
        ("G'ani ", "Ğani "),
        ("to'g'ri ", "töğri "),
        ("Is'hoq ", "Ishoq "),
        ("Ishoq ", "Işoq "),
        ("Ish\u{8}oq ", "Ishoq "),
        ("o'\u{8}zbek ", "o'zbek "),
        ("a'lo ", "a'lo "),
        ("maktab ", "maktab "),
        ("as\u{8}b ", "ab "),
        ("ssh ", "sş "),
        ("tosh\n", "toş\n"),
        ("qush\u{8}\u{8}x ", "qusx "),
        ("Michael ", "Miçael "),
        ("s'h ", "sh "),
        ("S'H ", "SH "),
        ("c'h ", "c'h "),
        ("hh ", "hh "),
        ("sh'", "ş'"),
        ("ooo'", "ooö"),
    ] {
        assert_eq!(sim(typed), want, "typed {typed:?}");
    }
}

#[test]
fn backspace_undo_then_delete() {
    assert_eq!(sim("Sh\u{8}"), "Sh");
    assert_eq!(sim("Sh\u{8}\u{8}"), "S");
    assert_eq!(sim("SH\u{8}\u{8}\u{8}"), "");
    assert_eq!(sim("ch\u{8}\u{8}"), "c");
    assert_eq!(sim("G'\u{8}"), "G'");
    // once committed, a later Backspace deletes the shown letter like any char
    assert_eq!(sim("sha\u{8}"), "ş");
}

fn unconv(s: &str) -> String {
    s.replace('ş', "sh")
        .replace('ç', "ch")
        .replace('ö', "o'")
        .replace('ğ', "g'")
}

fn check(k: &str) {
    let (out, e) = run(k);
    if k.contains(BACK) {
        if k.chars().all(|c| c == BACK || c == 'x') {
            assert_eq!(out, plain(k), "keys {k:?}");
        }
        return;
    }
    let norm = k.replace("s'h", "sh");
    // nothing typed is lost: undoing the letters gives the keys (the s'h separator is dropped)
    assert_eq!(unconv(&out), norm, "keys {k:?}");
    // a committing key never changes the text
    assert_eq!(
        sim(&format!("{k} ")),
        format!("{out} "),
        "keys {k:?} + space"
    );
    // Backspace: right after a conversion the digraph returns; otherwise the last char goes
    let restored = e.composing() && shown(e.typed()) != e.typed();
    let want = if restored {
        out.strip_suffix(&shown(e.typed())).unwrap().to_string() + e.typed()
    } else {
        let mut o = out.clone();
        o.pop();
        o
    };
    assert_eq!(sim(&format!("{k}{BACK}")), want, "keys {k:?} + Back");
}

/// Keys of `{s c o g h ' x Back}`, up to length 6.
#[test]
fn exhaustive_short_sequences() {
    let alphabet = ['s', 'c', 'o', 'g', 'h', '\'', 'x', BACK];
    let (mut n, mut stack) = (0usize, vec![String::new()]);
    while let Some(k) = stack.pop() {
        check(&k);
        n += 1;
        if k.chars().count() < 6 {
            stack.extend(alphabet.iter().map(|a| format!("{k}{a}")));
        }
    }
    assert_eq!(n, (0..=6).map(|l| 8usize.pow(l)).sum::<usize>());
}
