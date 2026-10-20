//! The worked examples of docs/phase2/rules.md section 10 (E01-E60) plus per-rule extras.
use std::borrow::Cow;
use uz_translit::{Apostrophe, Direction, Options, Run, convert, convert_runs};

fn c(s: &str) -> String {
    convert(s, Direction::ToCyrillic, &Options::default())
}
fn l(s: &str) -> String {
    convert(s, Direction::ToLatin, &Options::default())
}
fn runs(rs: &[&str], dir: Direction) -> Vec<String> {
    let rs: Vec<Run> = rs
        .iter()
        .map(|t| Run {
            text: t,
            convert: true,
        })
        .collect();
    convert_runs(&rs, dir, &Options::default())
        .into_iter()
        .map(Cow::into_owned)
        .collect()
}

macro_rules! to_c {
    ($($name:ident: $i:expr => $o:expr;)*) => { $( #[test] fn $name() { assert_eq!(c($i), $o); } )* };
}
macro_rules! to_l {
    ($($name:ident: $i:expr => $o:expr;)*) => { $( #[test] fn $name() { assert_eq!(l($i), $o); } )* };
}

to_c! {
    e01: "shahar" => "шаҳар";
    e02: "Shahar" => "Шаҳар";
    e03: "SHAHAR" => "ШАҲАР";
    e04: "choy" => "чой";
    e05: "o\u{2BB}zbek" => "ўзбек";
    e06: "o'zbek" => "ўзбек";
    e07: "O\u{2018}zbekiston" => "Ўзбекистон";
    e08: "g\u{2BC}alaba" => "ғалаба";
    e09: "to`g`ri" => "тўғри";
    e10: "bo\u{B4}lim" => "бўлим";
    e11: "ma\u{2BC}no" => "маъно";
    e12: "san'at" => "санъат";
    e13: "mas\u{2019}ul" => "масъул";
    e14: "e\u{2BC}lon" => "эълон";
    e15: "ma\u{2BC}yus" => "маъюс";
    e16: "Is'hoq" => "Исҳоқ";
    e17: "yer" => "ер";
    e18: "ekin" => "экин";
    e19: "poeziya" => "поэзия";
    e20: "sotsial-ekonomik" => "социал-экономик";
    e21: "yo\u{2BB}q" => "йўқ";
    e22: "dunyo" => "дунё";
    e23: "yangi" => "янги";
    e24: "tayyor" => "тайёр";
    e25: "ketsa" => "кетса";
    e26: "operatsiya" => "операция";
    e27: "litsey" => "лицей";
    e28: "funksiyasi" => "функцияси";
    e29: "vaksinatsiya" => "вакцинация";
    e30: "sentabr" => "сентябрь";
    e31: "1-sentabrdan" => "1-сентябрдан";
    e32: "SENTABR" => "СЕНТЯБРЬ";
    e33: "kompyuter" => "компьютер";
    e34: "obyekt" => "объект";
    e35: "mo\u{2BB}jiza" => "мўъжиза";
    e36: "PF-1234-son farmon" => "PF-1234-сон фармон";
    e37: "Manba: https://lex.uz/docs/5013007." => "Манба: https://lex.uz/docs/5013007.";
    e38: "C:\\Hujjatlar\\hisobot.docx" => "C:\\Hujjatlar\\hisobot.docx";
    e39: "XXI asr, I bob" => "XXI аср, I боб";
    e40: "Buyruqni X. Rahimov imzoladi" => "Буйруқни Х. Раҳимов имзолади";
    e41: "Microsoft Word" => "Microsoft Word";
    e42: "PDF fayl, BMT" => "PDF файл, БМТ";
    e43: "\u{2018}Bog\u{2019} ko\u{2019}chasi\u{2019}" => "\u{2018}Боғ кўчаси\u{2019}";
    e44: "\u{2018}radio\u{2019} dedi" => "\u{2018}радио\u{2019} деди";
    e45: "'olma'" => "'олма'";
    e46: "va/yoki 3x4" => "ва/ёки 3x4";
}

to_l! {
    e48a: "ШАҲАР" => "SHAHAR";
    e48b: "Шаҳар" => "Shahar";
    e49: "Ўзбекистон тўғри" => "Oʻzbekiston toʻgʻri";
    e50: "маъно масъул Исҳоқ" => "maʼno masʼul Isʼhoq";
    e51: "ер қаер кел эл Европа" => "yer qayer kel el Yevropa";
    e52: "объект съезд маъюс мўъжиза" => "obyekt syezd maʼyus moʻjiza";
    e53: "цирк операция концерт абзац" => "sirk operatsiya konsert abzats";
    e54a: "сентябрь" => "sentabr";
    e54b: "сентябрда" => "sentabrda";
    e54c: "сентябрьда" => "sentabrda";
    e54d: "октябрь" => "oktabr";
    e55: "компьютер фильм почтальон пьеса" => "kompyuter film pochtalyon pyesa";
    e56a: "дунё йўқ" => "dunyo yoʻq";
    e56b: "ЯНГИ Ёзги" => "YANGI Yozgi";
    e57a: "Ш. Рашидов" => "Sh. Rashidov";
    e57b: "Ш. РАШИДОВ" => "SH. RASHIDOV";
    e58: "ТошДУ ЎзР АҚШ МЧЖ" => "ToshDU OʻzR AQSH MChJ";
    e59a: "ПФ-5050-сон" => "ПФ-5050-son";
    e59b: "XXI аср" => "XXI asr";
    e59c: "щётка" => "shyotka";
}

#[test]
fn e47_digraph_split_across_runs() {
    assert_eq!(
        runs(&["S", "h", "ahar"], Direction::ToCyrillic),
        ["Ш", "", "аҳар"]
    );
}

#[test]
fn e60_runs_and_ascii() {
    assert_eq!(runs(&["Ш", "аҳар"], Direction::ToLatin), ["Sh", "ahar"]);
    assert_eq!(runs(&["сент", "ябрь"], Direction::ToLatin), ["sent", "abr"]);
    let o = Options {
        apostrophe: Apostrophe::Ascii,
        ..Options::default()
    };
    assert_eq!(
        convert("Ўзбекистон ғ ъ", Direction::ToLatin, &o),
        "O'zbekiston g' '"
    );
    assert_eq!(convert("Исҳоқ", Direction::ToLatin, &o), "Is'hoq");
}

// Section 1.3: the expected disagreements with the owner's plugin.
to_c! {
    d1: "koeffitsiyent" => "коэффициент";
}
to_l! {
    d2: "маъюс" => "maʼyus";
    d3: "Ш. РАШИДОВ" => "SH. RASHIDOV";
    d4: "почтальон" => "pochtalyon";
}

// R1 / R2 / R4 apostrophe variants: every member of A gives the same output (R15 closers excepted).
#[test]
fn apostrophe_variants() {
    let all = [
        '\u{2BB}', '\u{2BC}', '\u{2018}', '\u{2019}', '\'', '`', '\u{2BF}', '\u{B4}',
    ];
    for a in all {
        let t = |w: &str| w.replace('*', &a.to_string());
        assert_eq!(c(&t("o*zbek")), "ўзбек", "{a:?}");
        assert_eq!(c(&t("G*alaba")), "Ғалаба", "{a:?}");
        assert_eq!(c(&t("to*g*ri")), "тўғри", "{a:?}");
        assert_eq!(c(&t("ma*no")), "маъно", "{a:?}");
        assert_eq!(c(&t("MA*NO")), "МАЪНО", "{a:?}");
        assert_eq!(c(&t("as*hob")), "асҳоб", "{a:?}");
        assert_eq!(c(&t("tog*")), "тоғ", "{a:?}");
    }
}

#[test]
fn apostrophe_edge_cases() {
    assert_eq!(c("bog''"), "боғ'");
    assert_eq!(c("'olma'"), "'олма'");
    assert_eq!(c("a'"), "а'");
    assert_eq!(c("5'6"), "5'6");
    assert_eq!(c("Ishoq"), "Ишоқ");
    assert_eq!(c("Qat\u{2BC}iy"), "Қатъий");
    assert_eq!(c("Men 'yo'q' dedim"), "Мен 'йўқ' дедим");
    assert_eq!(c("MA\u{2BC}NO ma\u{2BC}No"), "МАЪНО маъНо");
}

#[test]
fn case_rules() {
    assert_eq!(c("sH"), "ш");
    assert_eq!(c("Ch CH cH"), "Ч Ч ч");
    assert_eq!(c("Yo Yu YA ye"), "Ё Ю Я е");
    assert_eq!(c("O\u{2BB}zR"), "ЎзР");
    assert_eq!(l("ЎзР ТошДУ"), "OʻzR ToshDU");
    assert_eq!(l("ТОШКЕНТ Шаҳри"), "TOSHKENT Shahri");
    assert_eq!(l("ЁЗ Ёз"), "YOZ Yoz");
    assert_eq!(l("ЦИРК Цирк"), "SIRK Sirk");
}

#[test]
fn ng_is_two_letters() {
    assert_eq!(c("yangi tong ming NG Ng"), "янги тонг минг НГ Нг");
    assert_eq!(l("ТОНГ Тонг"), "TONG Tong");
}

#[test]
fn ye_yo_yu_ya() {
    assert_eq!(c("Yevropa kliyent qayer"), "Европа клиент қаер");
    assert_eq!(
        c("aktyor iyun Yusuf choy qiyin seyf"),
        "актёр июнь Юсуф чой қийин сейф"
    );
    assert_eq!(l("актёр объём компьютер"), "aktyor obyom kompyuter");
    assert_eq!(c("aeroport 3-etaj"), "аэропорт 3-этаж");
    assert_eq!(c("Toshkent kel el"), "Тошкент кел эл");
}

#[test]
fn ts_rules() {
    assert_eq!(c("otsiz aytsin ketsa"), "отсиз айтсин кетса");
    assert_eq!(
        c("konstitutsiya investitsiya militsiya sotsial ratsion patsiyent initsiativa ofitsiant"),
        "конституция инвестиция милиция социал рацион пациент инициатива официант"
    );
    assert_eq!(
        l("концерт функция сцена кальций лицей абзац цирк"),
        "konsert funksiya ssena kalsiy litsey abzats sirk"
    );
}

#[test]
fn dictionary_extras() {
    assert_eq!(
        c("Sentabr oktabrda yanvardan"),
        "Сентябрь октябрда январдан"
    );
    assert_eq!(c("sirk sirka"), "цирк сирка");
    assert_eq!(c("MChJ YaTT JShShIR YaIM"), "МЧЖ ЯТТ ЖШШИР ЯИМ");
    assert_eq!(l("МЧЖ ЯТТ ЖШШИР ЯИМ"), "MChJ YaTT JShShIR YaIM");
    assert_eq!(c("it id IT ID"), "ит ид IT ID");
    assert_eq!(c("Google YouTube Excel"), "Google YouTube Excel");
    assert_eq!(c("premyer-ministr"), "премьер-министр");
    assert_eq!(c("Polsha"), "Польша");
}

#[test]
fn protected_spans() {
    assert_eq!(
        c("lex.uz my.gov.uz IJRO.GOV.UZ @user_1 info@lex.uz"),
        "lex.uz my.gov.uz IJRO.GOV.UZ @user_1 info@lex.uz"
    );
    assert_eq!(
        c("(https://lex.uz/a_(b)) www.lex.uz,"),
        "(https://lex.uz/a_(b)) www.lex.uz,"
    );
    assert_eq!(
        c("PF-1234 PQ-4545 O\u{2BB}RQ-587 A4 BI72373344 AA1234567"),
        "PF-1234 PQ-4545 O\u{2BB}RQ-587 A4 BI72373344 AA1234567"
    );
    assert_eq!(c("100ta 2024yil MODDA-5"), "100та 2024йил МОДДА-5");
    assert_eq!(
        l("ПФ-5050 ПҚ-4545 ЎРҚ-587 ВИ7237"),
        "ПФ-5050 ПҚ-4545 ЎРҚ-587 ВИ7237"
    );
    assert_eq!(c("1920x1080 3x4"), "1920x1080 3x4");
    assert_eq!(
        c("/tmp/x ./a.txt report.xlsx ~/dir/x"),
        "/tmp/x ./a.txt report.xlsx ~/dir/x"
    );
    assert_eq!(c("\u{2116}PF-12"), "\u{2116}PF-12");
    // roman numerals: values 1-399; initials convert
    assert_eq!(c("XXI IV XCIX CCCXCIX"), "XXI IV XCIX CCCXCIX");
    assert_eq!(c("IIV DXX"), "ИИВ ДХХ");
    assert_eq!(c("I. Karimov\nI. Umumiy"), "I. Каримов\nI. Умумий"); // paragraph start: a section number
    assert_eq!(c("Rais I. Karimov"), "Раис И. Каримов");
    assert_eq!(c("Bobur X. Rahimov"), "Бобур Х. Раҳимов");
}

#[test]
fn foreign_guard() {
    assert_eq!(
        c("Windows Facebook M\u{FC}ller McDonald's"),
        "Windows Facebook M\u{FC}ller McDonald's"
    );
    assert_eq!(c("Chilonzor choy"), "Чилонзор чой");
    let o = Options {
        foreign_latin_guard: false,
        ..Options::default()
    };
    assert_eq!(convert("Word", Direction::ToCyrillic, &o), "Ворд");
}

#[test]
fn quote_guard() {
    assert_eq!(
        c("\u{2018}radio\u{2019} dedi"),
        "\u{2018}радио\u{2019} деди"
    );
    assert_eq!(c("tog' va bog'"), "тоғ ва боғ");
    assert_eq!(c("'bog' bor"), "'бог' бор"); // ambiguous by spec: the last apostrophe is read as the closing quote
    assert_eq!(c("(\u{2018}ko\u{2019}p\u{2019})"), "(\u{2018}кўп\u{2019})");
}

#[test]
fn r16_normalisation() {
    // combining pairs compose; soft hyphen inside a digraph; marks follow their letter
    assert_eq!(l("у\u{306}з"), "oʻz");
    assert_eq!(l("е\u{308}з"), "yoz");
    assert_eq!(c("s\u{AD}hahar"), "ш\u{AD}аҳар");
    assert_eq!(l("ша\u{301}ҳар"), "sha\u{301}har");
    assert_eq!(c("sentabr\u{AD}"), "сентабр\u{AD}"); // words with transparent chars skip the dictionary
}

#[test]
fn unmapped_text_passes_through() {
    assert_eq!(l("Hello мир, щ ы і ө"), "Hello mir, sh i і ө");
    assert_eq!(c("привет 42 «1»"), "привет 42 «1»");
    assert_eq!(c(""), "");
}

#[test]
fn run_boundaries() {
    // 2 -> 1 and 1 -> 2
    assert_eq!(
        runs(&["O", "\u{2BB}zbek"], Direction::ToCyrillic),
        ["Ў", "збек"]
    );
    assert_eq!(
        runs(&["s", "\u{2BC}", "hoq"], Direction::ToCyrillic),
        ["с", "", "ҳоқ"]
    );
    assert_eq!(runs(&["Ш", "аҳар"], Direction::ToLatin), ["Sh", "ahar"]);
    // case across runs: the word class is computed on the joined text
    assert_eq!(runs(&["S", "HAHAR"], Direction::ToCyrillic), ["Ш", "АҲАР"]);
    assert_eq!(runs(&["Ш", "АҲАР"], Direction::ToLatin), ["SH", "AHAR"]);
    assert_eq!(runs(&["Ш", "аҳар"], Direction::ToLatin), ["Sh", "ahar"]);
    // dictionary output is spread proportionally
    assert_eq!(
        runs(&["sen", "tabr"], Direction::ToCyrillic),
        ["сен", "тябрь"]
    );
    // a bold word and its plain neighbour
    assert_eq!(
        runs(
            &["Tosh", "kent", " va ", "Sam", "arqand"],
            Direction::ToCyrillic
        ),
        ["Тош", "кент", " ва ", "Сам", "арқанд"]
    );
}

#[test]
fn convert_false_runs_are_boundaries() {
    let rs = [
        Run {
            text: "S",
            convert: true,
        },
        Run {
            text: "h",
            convert: false,
        },
        Run {
            text: "ahar",
            convert: true,
        },
    ];
    let out = convert_runs(&rs, Direction::ToCyrillic, &Options::default());
    assert_eq!(
        out.iter().map(|c| c.as_ref()).collect::<Vec<_>>(),
        ["С", "h", "аҳар"]
    );
    assert!(matches!(out[1], Cow::Borrowed(_)));
}

#[test]
fn unchanged_runs_are_borrowed() {
    let rs = [
        Run {
            text: "123 ",
            convert: true,
        },
        Run {
            text: "shahar ",
            convert: true,
        },
        Run {
            text: "https://x.uz",
            convert: true,
        },
    ];
    let out = convert_runs(&rs, Direction::ToCyrillic, &Options::default());
    assert!(matches!(out[0], Cow::Borrowed(_)));
    assert!(matches!(out[1], Cow::Owned(_)));
    assert!(matches!(out[2], Cow::Borrowed(_)));
}

#[test]
fn para_start_roman_initial() {
    // the first non-whitespace char of runs[0] is a paragraph start: "V." stays; a later run in the same segment is not
    assert_eq!(
        runs(&["  V. Aliyev"], Direction::ToCyrillic),
        ["  V. Алиев"]
    );
    assert_eq!(
        runs(&["Bu ", "V. Aliyev"], Direction::ToCyrillic),
        ["Бу ", "В. Алиев"]
    );
}
