//! The reformed Latin alphabet option (rules.md section 15): unit tests for both directions.
use uz_translit::{Direction, LatinAlphabet, Options, convert};

fn l26(s: &str) -> String {
    let o = Options {
        latin_alphabet: LatinAlphabet::Y2026,
        ..Options::default()
    };
    convert(s, Direction::ToLatin, &o)
}
fn l95(s: &str) -> String {
    convert(s, Direction::ToLatin, &Options::default())
}
fn c(s: &str) -> String {
    convert(s, Direction::ToCyrillic, &Options::default())
}

#[test]
fn default_is_1995() {
    assert_eq!(Options::default().latin_alphabet, LatinAlphabet::Y1995);
    assert_eq!(l95("Шаҳар Ўзбекистон"), "Shahar Oʻzbekiston");
}

#[test]
fn cyrillic_to_2026() {
    assert_eq!(l26("шаҳар чой ўзбек ғалаба"), "şahar çoy özbek ğalaba");
    assert_eq!(l26("Шаҳар Чой Ўзбек Ғалаба"), "Şahar Çoy Özbek Ğalaba");
    assert_eq!(l26("ШАҲАР ЧОЙ ЎЗБЕК ҒАЛАБА"), "ŞAHAR ÇOY ÖZBEK ĞALABA");
    assert_eq!(l26("Ш Ч Ў Ғ ш ч ў ғ щ"), "Ş Ç Ö Ğ ş ç ö ğ ş");
    assert_eq!(l26("ШШ ЧЧ"), "ŞŞ ÇÇ"); // a digraph never splits case
    assert_eq!(l26("Тошкент ЎзР"), "Toşkent ÖzR");
    assert_eq!(l26("тонг"), "tong"); // ng is not a unit
}

#[test]
fn separated_sh_has_no_separator() {
    assert_eq!(l95("Исҳоқ"), "Isʼhoq");
    assert_eq!(l26("Исҳоқ асҳоб ИСҲОҚ"), "Ishoq ashob ISHOQ");
    assert_eq!(l26("шоҳ"), "şoh");
}

#[test]
fn dictionary_and_acronyms_under_2026() {
    assert_eq!(l26("ЖШШИР МЧЖ"), "JŞŞIR MÇJ");
    assert_eq!(l26("Ўзбекконцерт"), "Özbekkonsert");
    assert_eq!(l26("сентябрь"), "sentabr");
    assert_eq!(c("özbekkonsert Özbekkonsert"), "ўзбекконцерт Ўзбекконцерт");
    assert_eq!(c("JŞŞIR MÇJ"), "ЖШШИР МЧЖ");
    assert_eq!(c("sentabrdan"), "сентябрдан");
}

#[test]
fn latin_accepts_both_alphabets() {
    assert_eq!(c("özbek ğalaba şahar çoy"), "ўзбек ғалаба шаҳар чой");
    assert_eq!(c("Özbek Ğalaba Şahar Çoy"), "Ўзбек Ғалаба Шаҳар Чой");
    assert_eq!(c("ÖZBEK ĞALABA ŞAHAR ÇOY"), "ЎЗБЕК ҒАЛАБА ШАҲАР ЧОЙ");
    // mixed text, and mixed inside one word
    assert_eq!(c("Oʻzbek Özbek Shahar Şahar"), "Ўзбек Ўзбек Шаҳар Шаҳар");
    assert_eq!(c("Oʻzbekiştan"), "Ўзбекиштан");
    assert_eq!(c("ŞAHAR SHAHAR"), "ШАҲАР ШАҲАР");
    // the 'e' after ö is a hard э, as after oʻ
    assert_eq!(c("öe oʻe ğe gʻe"), "ўэ ўэ ғэ ғэ");
}

#[test]
fn decomposed_forms_are_normalised() {
    assert_eq!(
        c("O\u{308}zbek g\u{306}alaba s\u{327}ahar c\u{327}oy"),
        "Ўзбек ғалаба шаҳар чой"
    );
    assert_eq!(
        c("O\u{308}ZBEK G\u{306}ALABA S\u{327}AHAR C\u{327}OY"),
        "ЎЗБЕК ҒАЛАБА ШАҲАР ЧОЙ"
    );
    assert_eq!(c("S\u{326}ahar"), "Шаҳар");
}

#[test]
fn foreign_guard_does_not_misfire_on_new_letters() {
    assert_eq!(c("Çiroq Şahar"), "Чироқ Шаҳар");
    assert_eq!(c("Windows Çiroq Facebook"), "Windows Чироқ Facebook");
    assert_eq!(c("Müller Ça"), "Müller Ча"); // a real foreign letter still guards its word
    assert_eq!(c("Café"), "Café");
}

#[test]
fn tutuq_next_to_new_letters() {
    assert_eq!(c("maʼşuq"), "маъшуқ");
    assert_eq!(c("Özbek taʼlim"), "Ўзбек таълим");
}

fn cy(s: &str, a: LatinAlphabet) -> String {
    let o = Options {
        latin_alphabet: a,
        ..Options::default()
    };
    convert(s, Direction::ToCyrillic, &o)
}

#[test]
fn explicit_2026_reads_2026_only() {
    use LatinAlphabet::{Auto, Y1995, Y2026};
    // sh, ch, o', g' are not digraphs; ş ç ö ğ are letters
    assert_eq!(cy("Ishoq asha", Y2026), "Исҳоқ асҳа");
    assert_eq!(cy("Ishoq", Y1995), "Ишоқ");
    assert_eq!(
        cy("Şahar Çoy Özbek Ğalaba", Y2026),
        "Шаҳар Чой Ўзбек Ғалаба"
    );
    assert_eq!(cy("taʼlim to'g'ri", Y2026), "таълим тоъгъри");
    assert_eq!(cy("Isʼhoq", Y1995), "Исҳоқ");
    // the guard treats c as foreign when ch is not a digraph
    assert_eq!(cy("cheque", Y2026), "cheque");
    // Auto: per paragraph
    assert_eq!(cy("Ishoq", Auto), "Ишоқ");
    assert_eq!(cy("Ishoq Çoy", Auto), "Исҳоқ Чой");
    assert_eq!(cy("Oʻzbek shahar", Auto), "Ўзбек шаҳар");
}

#[test]
fn separated_sh_round_trips_in_2026() {
    use LatinAlphabet::Y2026;
    let o = Options {
        latin_alphabet: Y2026,
        ..Options::default()
    };
    for t in ["Исҳоқ", "асҳоб", "ИСҲОҚ", "шоҳ Шаҳар"] {
        let l = convert(t, Direction::ToLatin, &o);
        assert_eq!(convert(&l, Direction::ToCyrillic, &o), t, "{l}");
    }
}
