//! Deterministic property-style tests (rules.md section 9): fixed xorshift seeds, no external crates.
use std::borrow::Cow;
mod common;
use uz_translit::{Direction, LatinAlphabet, Options, Run, convert, convert_runs};

const CASES: usize = 20_000;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

fn c(s: &str) -> String {
    convert(s, Direction::ToCyrillic, &Options::default())
}
fn l(s: &str) -> String {
    convert(s, Direction::ToLatin, &Options::default())
}

const CONS: [&str; 22] = [
    "b", "d", "f", "g", "h", "j", "k", "l", "m", "n", "p", "q", "r", "s", "t", "v", "x", "y", "z",
    "sh", "ch", "g\u{2BB}",
];
const VOW: [&str; 6] = ["a", "e", "i", "o", "u", "o\u{2BB}"];

/// 9.2 exclusions: no ts, no ye after a consonant, at least two Cyrillic letters.
fn generable(w: &str) -> bool {
    if w.contains("ts") {
        return false;
    }
    let cs: Vec<char> = w.chars().collect();
    for i in 1..cs.len() {
        if cs[i] == 'e' && cs[i - 1] == 'y' {
            let before = if i >= 2 { Some(cs[i - 2]) } else { None };
            let vowel_before = match before {
                None => true,
                Some('\u{2BB}') => i >= 3 && cs[i - 3] == 'o',
                Some(b) => "aeiou".contains(b),
            };
            if !vowel_before {
                return false;
            }
        }
    }
    c(w).chars().count() >= 2
}

fn gen_word(rng: &mut Rng) -> String {
    loop {
        let n = 2 + rng.below(4);
        let mut w = String::new();
        let mut vowel_last = false;
        for k in 0..n {
            let want_vowel = if k == 0 {
                rng.below(3) == 0
            } else {
                !vowel_last || rng.below(4) == 0
            };
            let u = if want_vowel {
                *rng.pick(&VOW)
            } else {
                *rng.pick(&CONS)
            };
            w.push_str(u);
            vowel_last = want_vowel;
            // tutuq after a/i/u/e and before a consonant other than y
            if want_vowel && !u.starts_with('o') && k + 1 < n && rng.below(8) == 0 {
                w.push('\u{2BC}');
                let next = loop {
                    let c = *rng.pick(&CONS);
                    if c != "y" {
                        break c;
                    }
                };
                w.push_str(next);
                vowel_last = false;
            }
        }
        if generable(&w) {
            return w;
        }
    }
}

fn with_class(w: &str, class: usize) -> String {
    match class {
        0 => w.to_string(),
        1 => {
            let mut cs = w.chars();
            let first = cs.next().unwrap().to_uppercase().collect::<String>();
            first + cs.as_str()
        }
        _ => w.to_uppercase(),
    }
}

const SPANS: [&str; 9] = [
    "https://lex.uz/docs/1",
    "info@lex.uz",
    "PF-1234",
    "XXI",
    "lex.uz",
    "@user_1",
    "C:\\x\\y.docx",
    "BI72373344",
    "3x4",
];

fn gen_sentence(rng: &mut Rng) -> String {
    let mut s = String::new();
    for k in 0..1 + rng.below(6) {
        if k > 0 {
            s.push_str(rng.pick(&[" ", " ", " ", ", ", "-", " 5 ", ". ", " (", ") "]));
        }
        if rng.below(12) == 0 {
            if !s.is_empty() && !s.ends_with(' ') {
                s.push(' '); // spans start at token starts
            }
            s.push_str(rng.pick(&SPANS));
            s.push(' ');
        }
        let w = gen_word(rng);
        s.push_str(&with_class(&w, rng.below(3)));
    }
    s
}

fn variants(rng: &mut Rng, s: &str) -> String {
    const A: [char; 8] = [
        '\u{2BB}', '\u{2BC}', '\u{2018}', '\u{2019}', '\'', '`', '\u{2BF}', '\u{B4}',
    ];
    s.chars()
        .map(|ch| {
            if ch == '\u{2BB}' || ch == '\u{2BC}' {
                *rng.pick(&A)
            } else {
                ch
            }
        })
        .collect()
}

fn split_runs(rng: &mut Rng, s: &str) -> Vec<String> {
    let cs: Vec<char> = s.chars().collect();
    let mut cuts: Vec<usize> = (0..rng.below(6)).map(|_| rng.below(cs.len() + 1)).collect();
    cuts.push(0);
    cuts.push(cs.len());
    cuts.sort_unstable();
    cuts.windows(2)
        .map(|w| cs[w[0]..w[1]].iter().collect())
        .collect()
}

fn runs_concat(parts: &[String], dir: Direction, o: &Options) -> String {
    let rs: Vec<Run> = parts
        .iter()
        .map(|t| Run {
            text: t,
            convert: true,
        })
        .collect();
    convert_runs(&rs, dir, o).iter().map(Cow::as_ref).collect()
}

#[test]
fn round_trip_variants_and_splits() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for case in 0..CASES {
        let x = gen_sentence(&mut rng);
        let cy = c(&x);
        assert_eq!(
            l(&cy),
            x,
            "latin->cyrillic->latin, case {case}: {x:?} -> {cy:?}"
        );
        let v = variants(&mut rng, &x);
        assert_eq!(c(&v), cy, "apostrophe variants, case {case}: {v:?}");
        assert_eq!(c(&cy), cy, "idempotence ->C, case {case}: {x:?}");
        assert_eq!(l(&l(&cy)), l(&cy), "idempotence ->L, case {case}: {x:?}");
        let parts = split_runs(&mut rng, &x);
        assert_eq!(
            runs_concat(&parts, Direction::ToCyrillic, &Options::default()),
            cy,
            "run split ->C, case {case}: {parts:?}"
        );
        let parts = split_runs(&mut rng, &cy);
        assert_eq!(
            runs_concat(&parts, Direction::ToLatin, &Options::default()),
            l(&cy),
            "run split ->L, case {case}: {parts:?}"
        );
    }
}

#[test]
fn cyrillic_round_trip() {
    // 9.3: Cyrillic -> Latin -> Cyrillic for text that came out of the generator (so it satisfies the exclusions).
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for case in 0..CASES / 4 {
        let y = c(&gen_sentence(&mut rng));
        assert_eq!(
            c(&l(&y)),
            y,
            "cyrillic->latin->cyrillic, case {case}: {y:?}"
        );
    }
}

#[test]
fn run_split_invariance_on_arbitrary_text() {
    const ALPHABET: [&str; 52] = [
        "a", "o", "g", "s", "h", "c", "y", "e", "t", "'", "\u{2BB}", "\u{2019}", "\u{2018}", " ",
        "-", "5", "x", "S", "H", "Ш", "ш", "ў", "ъ", "ь", "е", "ц", "с", "ҳ", "(", ")", ".", "/",
        "@", "A", "O", "X", "I", "V", "\n", "`", "\u{AD}", "\u{306}", "у", "Ё", "ö", "Ş", "ç", "Ğ",
        "\u{308}", "\u{327}", "щ", "ч",
    ];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for case in 0..CASES {
        let n = 1 + rng.below(40);
        let s: String = (0..n).map(|_| *rng.pick(&ALPHABET)).collect();
        for (dir, alphabet) in [
            (Direction::ToCyrillic, LatinAlphabet::Y1995),
            (Direction::ToLatin, LatinAlphabet::Y1995),
            (Direction::ToLatin, LatinAlphabet::Y2026),
        ] {
            let o = Options {
                latin_alphabet: alphabet,
                ..Options::default()
            };
            let whole = convert(&s, dir, &o);
            assert_eq!(
                convert(&whole, dir, &o),
                whole,
                "idempotence, case {case}, {dir:?} {alphabet:?}: {s:?}"
            );
            let parts = split_runs(&mut rng, &s);
            assert_eq!(
                runs_concat(&parts, dir, &o),
                whole,
                "split invariance, case {case}, {dir:?} {alphabet:?}: {s:?} {parts:?}"
            );
        }
    }
}

#[test]
fn reformed_alphabet_properties() {
    // Section 15: ->L in 2026 is the 1995 output through the letter substitution; ->C reads it back, also
    // when each word is randomly written in either alphabet; ->C is unchanged by the option.
    let o26 = Options {
        latin_alphabet: LatinAlphabet::Y2026,
        ..Options::default()
    };
    let mut rng = Rng(0xA5A5_1234_DEAD_BEEF);
    for case in 0..CASES {
        let x = gen_sentence(&mut rng);
        let cy = c(&x);
        let y = convert(&cy, Direction::ToLatin, &o26);
        assert_eq!(
            y,
            common::to_2026(&l(&cy)),
            "2026 output, case {case}: {cy:?}"
        );
        assert_eq!(c(&y), cy, "2026 round trip, case {case}: {y:?}");
        assert_eq!(
            convert(&y, Direction::ToCyrillic, &o26),
            cy,
            "option is ignored by ->C, case {case}"
        );
        let mixed: Vec<String> = x
            .split(' ')
            .map(|w| {
                if rng.below(2) == 0 {
                    common::to_2026(w)
                } else {
                    w.to_string()
                }
            })
            .collect();
        assert_eq!(
            c(&mixed.join(" ")),
            cy,
            "mixed alphabets, case {case}: {mixed:?}"
        );
        assert_eq!(
            convert(&y, Direction::ToLatin, &o26),
            y,
            "idempotence ->L 2026, case {case}"
        );
        let parts = split_runs(&mut rng, &cy);
        assert_eq!(
            runs_concat(&parts, Direction::ToLatin, &o26),
            y,
            "run split ->L 2026, case {case}: {parts:?}"
        );
    }
}
