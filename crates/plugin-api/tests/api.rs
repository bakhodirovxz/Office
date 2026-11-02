//! Tests of the plugin API v1: one per refusal of PLAN 4.1, the codecs (round trip, truncated and oversized input, mutation),
//! the permission check and the allowlist data. The allowlist test against the real Docs registry is in `docs-office`.

use plugin_api::action::*;
use plugin_api::allow;
use plugin_api::manifest::{self, DocAccess, Refusal};
use plugin_api::textmap::{self, Apply, Paragraph, Segment, TextMap};
use plugin_api::wire::{MAX_BLOBS, MAX_HEADER, MAX_MESSAGE, Message, WireError};
use serde_json::json;

const GOOD: &str = r#"
id = "uz-translit"
version = "1.0.0"
api = 1
apps = ["docs"]
publisher = "Example Publisher"
events = ["selectionChanged"]
name.en = "Transliteration (plugin)"
name.ru = "Транслитерация"
name."uz-Latn" = "Transliteratsiya"
name."uz-Cyrl" = "Транслитерация"

[permissions]
document = "write"
network = ["https://api.example.org", "https://example.org:8443"]
memory_mb = 32

[[command]]
id = "toCyrillic"
label.en = "Latin to Cyrillic"
reads = ["textmap"]
params = { scope = "auto" }

[[toolbar]]
command = "toCyrillic"
icon = "translit"
group.en = "Plugins"

[[menu]]
command = "toCyrillic"
place = "context.text"

[[panel]]
id = "fields"
title.en = "Template fields"

[settings]
mode = "strict"
"#;

fn with(from: &str, to: &str) -> String {
    assert!(GOOD.contains(from), "fixture lacks {from:?}");
    GOOD.replacen(from, to, 1)
}

fn refused(text: &str) -> Refusal {
    manifest::parse(text).expect_err("manifest should be refused")
}

#[test]
fn good_manifest_parses() {
    let m = manifest::parse(GOOD).unwrap();
    assert_eq!(m.id, "uz-translit");
    assert_eq!(m.permissions.document, DocAccess::Write);
    assert_eq!(m.permissions.memory_mb, 32);
    assert_eq!(m.commands.len(), 1);
    assert_eq!(m.commands[0].params["scope"], "auto");
    assert_eq!(
        manifest::registry_id(&m.id, &m.commands[0].id),
        "plugin.uz-translit.toCyrillic"
    );
}

#[test]
fn minimal_manifest_gets_defaults() {
    let m = manifest::parse("id = \"abc\"\nversion = \"0.1.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"p\"\nname.en = \"n\"\n").unwrap();
    assert_eq!(m.permissions, manifest::Permissions::default());
}

#[test]
fn refuses_unknown_keys() {
    for bad in [
        with("api = 1", "api = 1\nextra = 1"),
        with("memory_mb = 32", "memory_mb = 32\nroot = true"),
        with("id = \"fields\"", "id = \"fields\"\nx = 1"),
    ] {
        assert!(matches!(refused(&bad), Refusal::UnknownKey(_)), "{bad}");
    }
}

#[test]
fn events_after_a_table_header_are_refused_not_misread() {
    // `events` below `[[panel]]` would be a panel key, which the panel does not have.
    let bad = with("events = [\"selectionChanged\"]\n", "")
        .replace("[settings]", "events = [\"documentSaved\"]\n[settings]");
    assert!(matches!(refused(&bad), Refusal::UnknownKey(_)));
}

#[test]
fn refuses_syntax_errors_and_missing_keys() {
    assert!(matches!(refused("id = "), Refusal::Syntax(_)));
    assert!(matches!(
        refused(&with("api = 1\n", "")),
        Refusal::Syntax(_)
    ));
    assert!(matches!(
        refused(&with("api = 1", "api = \"1\"")),
        Refusal::Syntax(_)
    ));
    assert!(matches!(
        refused(&with("\"selectionChanged\"", "\"onTyping\"")),
        Refusal::Syntax(_)
    ));
    assert!(matches!(
        refused(&with("\"write\"", "\"root\"")),
        Refusal::Syntax(_)
    ));
}

#[test]
fn refuses_oversized_manifest() {
    let big = format!("{GOOD}\n# {}\n", "x".repeat(manifest::MAX_MANIFEST));
    assert!(matches!(refused(&big), Refusal::TooLarge { .. }));
}

#[test]
fn refuses_bad_ids() {
    for id in [
        "ab",
        "Upper",
        "has space",
        "-lead",
        "trail-",
        ".lead",
        "a..b",
        "ünï",
        &"a".repeat(65),
        "a/b",
    ] {
        assert!(
            matches!(
                refused(&with("\"uz-translit\"", &format!("{id:?}"))),
                Refusal::BadId(_)
            ),
            "{id}"
        );
    }
    assert!(manifest::parse(&with("\"uz-translit\"", "\"a.b-c.9\"")).is_ok());
    // command and panel ids: no dot (the registry id splits at the last one), letter first
    for id in ["to.Cyr", "1abc", "", "a b"] {
        assert!(
            matches!(
                refused(
                    &with("id = \"toCyrillic\"", &format!("id = {id:?}"))
                        .replace("command = \"toCyrillic\"", &format!("command = {id:?}"))
                ),
                Refusal::BadId(_)
            ),
            "{id}"
        );
    }
    assert!(matches!(
        refused(&with("id = \"fields\"", "id = \"a.b\"")),
        Refusal::BadId(_)
    ));
}

#[test]
fn refuses_bad_versions() {
    for v in [
        "1",
        "1.0",
        "1.0.0.0",
        "01.0.0",
        "1.0.x",
        "v1.0.0",
        "1.0.0+build",
        "1.0.0-",
        "",
        "1.0.0-a b",
    ] {
        assert!(
            matches!(
                refused(&with("\"1.0.0\"", &format!("{v:?}"))),
                Refusal::BadVersion(_)
            ),
            "{v}"
        );
    }
    assert!(manifest::parse(&with("\"1.0.0\"", "\"1.2.3-rc.1\"")).is_ok());
}

#[test]
fn refuses_an_api_the_host_does_not_run() {
    for n in [0, 2, 99] {
        assert_eq!(
            refused(&with("api = 1", &format!("api = {n}"))),
            Refusal::UnsupportedApi(n)
        );
    }
    assert!(plugin_api::supports(1) && !plugin_api::supports(2));
}

#[test]
fn refuses_bad_apps() {
    for apps in [
        "[]",
        "[\"docs\", \"docs\"]",
        "[\"word\"]",
        "[\"docs\",\"sheets\",\"slides\",\"pdf\",\"docs\"]",
    ] {
        let r = refused(&with("[\"docs\"]", apps));
        assert!(
            matches!(r, Refusal::BadApps(_) | Refusal::Duplicate { .. }),
            "{apps}: {r}"
        );
    }
    assert!(manifest::parse(&with("[\"docs\"]", "[\"docs\", \"sheets\"]")).is_ok());
}

#[test]
fn refuses_bad_text() {
    let long = "x".repeat(65);
    for (from, to) in [
        (
            "name.en = \"Transliteration (plugin)\"",
            format!("name.en = {long:?}"),
        ),
        (
            "name.en = \"Transliteration (plugin)\"",
            "name.en = \"  \"".into(),
        ),
        (
            "name.en = \"Transliteration (plugin)\"",
            "name.en = \"a\\u202Eb\"".into(),
        ),
        (
            "name.en = \"Transliteration (plugin)\"",
            "name.en = \"a\\nb\"".into(),
        ),
        (
            "name.en = \"Transliteration (plugin)\"",
            "name.de = \"Hallo\"".into(),
        ),
        (
            "publisher = \"Example Publisher\"",
            "publisher = \"\"".into(),
        ),
        (
            "publisher = \"Example Publisher\"",
            format!("publisher = {:?}", "p".repeat(129)),
        ),
        (
            "label.en = \"Latin to Cyrillic\"",
            format!("label.en = {long:?}"),
        ),
        (
            "label.en = \"Latin to Cyrillic\"",
            "label.ru = \"x\"".into(),
        ),
        (
            "title.en = \"Template fields\"",
            format!("title.en = {long:?}"),
        ),
        ("group.en = \"Plugins\"", format!("group.en = {long:?}")),
    ] {
        assert!(
            matches!(refused(&with(from, &to)), Refusal::BadText { .. }),
            "{to}"
        );
    }
    // 64 characters of a multi-byte script is fine: the limit counts characters
    assert!(
        manifest::parse(&with(
            "name.ru = \"Транслитерация\"",
            &format!("name.ru = {:?}", "ж".repeat(64))
        ))
        .is_ok()
    );
}

#[test]
fn refuses_memory_out_of_range() {
    for mb in [0, 65, 4096] {
        assert!(matches!(
            refused(&with("memory_mb = 32", &format!("memory_mb = {mb}"))),
            Refusal::MemoryOutOfRange { .. }
        ));
    }
    assert!(manifest::parse(&with("memory_mb = 32", "memory_mb = 64")).is_ok());
}

#[test]
fn refuses_bad_origins() {
    for o in [
        "http://api.example.org",
        "https://api.example.org/",
        "https://api.example.org/path",
        "https://api.example.org?x=1",
        "https://user@api.example.org",
        "https://API.example.org",
        "https://example",
        "https://localhost",
        "https://a.localhost",
        "https://127.0.0.1",
        "https://10.0.0.1:8443",
        "https://[::1]",
        "https://api.example.org:",
        "https://api.example.org:0",
        "https://api.example.org:70000",
        "https://api.example.org:80a",
        "https://-bad.example.org",
        "https://bad..example.org",
        "https://exa mple.org",
        "https://exämple.org",
        "https://example.org\\@evil.org",
        "*",
        "",
    ] {
        assert!(
            matches!(
                refused(&with("\"https://api.example.org\"", &format!("{o:?}"))),
                Refusal::BadOrigin(_)
            ),
            "{o}"
        );
    }
    for o in [
        "https://api.example.org",
        "https://a-b.example.co.uk:8443",
        "https://xn--80ak6aa92e.example",
    ] {
        assert!(manifest::check_origin(o), "{o}");
    }
    assert!(matches!(
        refused(&with(
            "\"https://api.example.org\"",
            "\"https://example.org:8443\""
        )),
        Refusal::Duplicate { .. }
    ));
}

#[test]
fn refuses_too_many_of_everything() {
    let cmds = |n: usize| {
        (0..n)
            .map(|i| format!("[[command]]\nid = \"c{i}\"\nlabel.en = \"c\"\n"))
            .collect::<String>()
    };
    let base = "id = \"abc\"\nversion = \"0.1.0\"\napi = 1\napps = [\"docs\"]\npublisher = \"p\"\nname.en = \"n\"\n";
    assert!(manifest::parse(&format!("{base}{}", cmds(16))).is_ok());
    assert!(matches!(
        refused(&format!("{base}{}", cmds(17))),
        Refusal::TooMany {
            what: "commands",
            ..
        }
    ));
    let tb = |n: usize| {
        (0..n)
            .map(|_| "[[toolbar]]\ncommand = \"c0\"\n")
            .collect::<String>()
    };
    assert!(manifest::parse(&format!("{base}{}{}", cmds(1), tb(8))).is_ok());
    assert!(matches!(
        refused(&format!("{base}{}{}", cmds(1), tb(9))),
        Refusal::TooMany {
            what: "toolbar items",
            ..
        }
    ));
    let panels = |n: usize| {
        (0..n)
            .map(|i| format!("[[panel]]\nid = \"p{i}\"\ntitle.en = \"t\"\n"))
            .collect::<String>()
    };
    assert!(manifest::parse(&format!("{base}{}", panels(2))).is_ok());
    assert!(matches!(
        refused(&format!("{base}{}", panels(3))),
        Refusal::TooMany { what: "panels", .. }
    ));
    let menu = (0..9)
        .map(|_| "[[menu]]\ncommand = \"c0\"\nplace = \"tools\"\n")
        .collect::<String>();
    assert!(matches!(
        refused(&format!("{base}{}{menu}", cmds(1))),
        Refusal::TooMany {
            what: "menu items",
            ..
        }
    ));
    let origins = (0..9)
        .map(|i| format!("\"https://h{i}.example.org\""))
        .collect::<Vec<_>>()
        .join(",");
    assert!(matches!(
        refused(&format!("{base}[permissions]\nnetwork = [{origins}]\n")),
        Refusal::TooMany {
            what: "network origins",
            ..
        }
    ));
}

#[test]
fn refuses_duplicates_and_dangling_references() {
    let dup = with(
        "[[toolbar]]",
        "[[command]]\nid = \"toCyrillic\"\nlabel.en = \"x\"\n\n[[toolbar]]",
    );
    assert!(matches!(
        refused(&dup),
        Refusal::Duplicate {
            what: "command",
            ..
        }
    ));
    let dup = with(
        "[settings]",
        "[[panel]]\nid = \"fields\"\ntitle.en = \"x\"\n\n[settings]",
    );
    assert!(matches!(
        refused(&dup),
        Refusal::Duplicate { what: "panel", .. }
    ));
    assert!(matches!(
        refused(&with(
            "events = [\"selectionChanged\"]",
            "events = [\"documentSaved\", \"documentSaved\"]"
        )),
        Refusal::Duplicate { what: "event", .. }
    ));
    assert!(matches!(
        refused(&with(
            "command = \"toCyrillic\"\nicon",
            "command = \"nope\"\nicon"
        )),
        Refusal::UnknownCommand(_)
    ));
    assert!(matches!(
        refused(&with(
            "command = \"toCyrillic\"\nplace",
            "command = \"nope\"\nplace"
        )),
        Refusal::UnknownCommand(_)
    ));
}

#[test]
fn refuses_bad_icons_and_oversized_maps() {
    for i in ["../x", "Translit", "a b", "", &"i".repeat(33)] {
        assert!(
            matches!(
                refused(&with("icon = \"translit\"", &format!("icon = {i:?}"))),
                Refusal::BadIcon(_)
            ),
            "{i}"
        );
    }
    let many = (0..17)
        .map(|i| format!("k{i} = 1"))
        .collect::<Vec<_>>()
        .join(", ");
    assert!(matches!(
        refused(&with("{ scope = \"auto\" }", &format!("{{ {many} }}"))),
        Refusal::BadParams { .. }
    ));
    assert!(matches!(
        refused(&with(
            "{ scope = \"auto\" }",
            &format!("{{ s = {:?} }}", "x".repeat(2000))
        )),
        Refusal::BadParams { .. }
    ));
    assert!(matches!(
        refused(&with(
            "mode = \"strict\"",
            &format!("mode = {:?}", "x".repeat(5000))
        )),
        Refusal::BadParams { .. }
    ));
}

#[test]
fn refuses_reads_without_document_access() {
    let bad = with("document = \"write\"", "document = \"none\"");
    assert!(matches!(refused(&bad), Refusal::ReadsNeedDocument(_)));
    assert!(
        manifest::parse(
            &with("document = \"write\"", "document = \"none\"")
                .replace("reads = [\"textmap\"]", "reads = [\"none\"]")
        )
        .is_ok()
    );
}

#[test]
fn package_entries() {
    let ok = [
        ("plugin.toml", 900),
        ("plugin.wasm", 108_282),
        ("strings.tsv", 10),
    ];
    assert!(manifest::check_package(&ok).is_ok());
    let err = |e: &[(&str, u64)]| manifest::check_package(e).unwrap_err();
    for name in [
        "../plugin.wasm",
        "/etc/passwd",
        "dir/plugin.wasm",
        "plugin.wasm/",
        "evil.exe",
        "C:\\x",
        "plugin.toml\0",
        "PLUGIN.TOML",
    ] {
        assert!(
            matches!(
                err(&[("plugin.toml", 1), ("plugin.wasm", 1), (name, 1)]),
                Refusal::Package(_)
            ),
            "{name:?}"
        );
    }
    assert!(matches!(
        err(&[("plugin.toml", 1), ("plugin.toml", 1), ("plugin.wasm", 1)]),
        Refusal::Package(_)
    ));
    assert!(matches!(err(&[("plugin.toml", 1)]), Refusal::Package(_)));
    assert!(matches!(
        err(&[("plugin.toml", 1), ("plugin.wasm", (4 << 20) + 1)]),
        Refusal::Package(_)
    ));
    assert!(matches!(
        err(&[
            ("plugin.toml", 1),
            ("plugin.wasm", 1),
            ("strings.tsv", 8 << 20)
        ]),
        Refusal::Package(_)
    ));
    assert!(matches!(
        err(&[
            ("plugin.toml", 1),
            ("plugin.wasm", u64::MAX),
            ("SIGNATURE", u64::MAX)
        ]),
        Refusal::Package(_)
    ));
    let many: Vec<(&str, u64)> = (0..65).map(|_| ("plugin.toml", 1)).collect();
    assert!(matches!(err(&many), Refusal::Package(_)));
}

// ---- codecs ----

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn ch(&mut self) -> char {
        const POOL: &[char] = &[
            'a',
            'Z',
            ' ',
            '\t',
            '0',
            'o',
            'g',
            '\u{2BB}',
            '\u{2BC}',
            '\u{2018}',
            '\u{301}',
            '\u{308}',
            '\u{200D}',
            'ў',
            'қ',
            'ғ',
            'ҳ',
            'ё',
            'Я',
            '\u{0}',
            '\u{1F600}',
            '\u{10FFFF}',
            '\u{FFFD}',
            '\u{D7FF}',
            '\u{E000}',
            '\u{FFFF}',
            '\u{10000}',
        ];
        match self.below(4) {
            0 => POOL[self.below(POOL.len() as u64) as usize],
            1 => char::from_u32(self.below(0x80) as u32).unwrap(),
            _ => loop {
                if let Some(c) = char::from_u32(self.below(0x110000) as u32) {
                    break c;
                }
            },
        }
    }
    fn text(&mut self, max: u64) -> String {
        (0..self.below(max + 1)).map(|_| self.ch()).collect()
    }
}

fn random_map(r: &mut Rng) -> TextMap {
    let mut m = TextMap::default();
    let mut index = r.below(3) as u32;
    for _ in 0..r.below(12) {
        let segs = (0..r.below(6))
            .map(|_| {
                let flags = match r.below(5) {
                    0..=2 => textmap::EDITABLE,
                    3 => textmap::OBJECT | textmap::FIELD_RESULT,
                    _ => textmap::OUTSIDE_SELECTION,
                };
                let lang = ["", "uz-Latn", "uz-Cyrl", "ru", "en-US"][r.below(5) as usize];
                Segment {
                    flags,
                    lang: m.lang_index(lang).unwrap(),
                    text: r.text(30),
                }
            })
            .collect();
        m.paragraphs.push(Paragraph {
            index,
            segments: segs,
        });
        index += 1 + r.below(4) as u32;
    }
    m
}

#[test]
fn textmap_round_trip_random() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..500 {
        let m = random_map(&mut r);
        let bytes = m.encode().unwrap();
        assert_eq!(TextMap::decode(&bytes).unwrap(), m);
        // an apply that changes some editable segments, as the plugin would send it
        let ap = Apply {
            paragraphs: m
                .paragraphs
                .iter()
                .map(|p| {
                    p.segments
                        .iter()
                        .map(|s| {
                            (s.editable() && r.below(2) == 0).then(|| {
                                r.text(40).replace(
                                    |c: char| {
                                        c.is_control() && c != '\t'
                                            || c == '\u{2028}'
                                            || c == '\u{2029}'
                                    },
                                    "",
                                )
                            })
                        })
                        .collect()
                })
                .collect(),
        };
        let ab = ap.encode().unwrap();
        let back = Apply::decode(&ab).unwrap();
        assert_eq!(back, ap);
        back.validate(&m).unwrap();
    }
}

#[test]
fn textmap_keeps_special_code_points_byte_exact() {
    let mut m = TextMap::default();
    let tag = m.lang_index("uz-Latn").unwrap();
    let texts = [
        "o\u{2BB}zbek g\u{2BB}isht",
        "o\u{2BC}",
        "e\u{301}\u{308}\u{323}",
        "Ўзбекистон қўшиқ ҳаво ғалаба",
        "\u{10FFFF}\u{10000}\u{FFFF}\u{D7FF}\u{E000}",
        "\u{0}",
        "",
    ];
    m.paragraphs.push(Paragraph {
        index: 0,
        segments: texts
            .iter()
            .map(|t| Segment {
                flags: textmap::EDITABLE,
                lang: tag,
                text: t.to_string(),
            })
            .collect(),
    });
    assert_eq!(TextMap::decode(&m.encode().unwrap()).unwrap(), m);
}

#[test]
fn apply_validation_refusals() {
    let mut m = TextMap::default();
    m.paragraphs.push(Paragraph {
        index: 4,
        segments: vec![
            Segment {
                flags: textmap::EDITABLE,
                lang: 0,
                text: "a".into(),
            },
            Segment {
                flags: textmap::OBJECT,
                lang: 0,
                text: "b".into(),
            },
        ],
    });
    let ok = Apply {
        paragraphs: vec![vec![Some("x".into()), None]],
    };
    ok.validate(&m).unwrap();
    let bad = |a: Apply| a.validate(&m).unwrap_err();
    assert!(matches!(
        bad(Apply { paragraphs: vec![] }),
        WireError::Textmap(_)
    ));
    assert!(matches!(
        bad(Apply {
            paragraphs: vec![vec![None]]
        }),
        WireError::Textmap(_)
    ));
    assert!(matches!(
        bad(Apply {
            paragraphs: vec![vec![None, None, None]]
        }),
        WireError::Textmap(_)
    ));
    assert!(
        matches!(
            bad(Apply {
                paragraphs: vec![vec![None, Some("x".into())]]
            }),
            WireError::Textmap(_)
        ),
        "changed a non-editable segment"
    );
    for t in [
        "a\nb",
        "a\rb",
        "a\u{2028}b",
        "a\u{2029}b",
        "a\u{0}b",
        "a\u{1b}b",
        "a\u{85}b",
    ] {
        assert!(
            matches!(
                bad(Apply {
                    paragraphs: vec![vec![Some(t.into()), None]]
                }),
                WireError::Textmap(_)
            ),
            "{t:?}"
        );
    }
    Apply {
        paragraphs: vec![vec![Some("tab\there".into()), None]],
    }
    .validate(&m)
    .unwrap();
}

#[test]
fn textmap_decode_refusals() {
    let mut m = TextMap::default();
    m.paragraphs.push(Paragraph {
        index: 1,
        segments: vec![Segment {
            flags: textmap::EDITABLE,
            lang: 0,
            text: "a".into(),
        }],
    });
    m.paragraphs.push(Paragraph {
        index: 2,
        segments: vec![],
    });
    let good = m.encode().unwrap();
    assert!(TextMap::decode(&good).is_ok());
    let mut trailing = good.clone();
    trailing.push(0);
    assert_eq!(TextMap::decode(&trailing), Err(WireError::Trailing));
    let mut same = m.clone();
    same.paragraphs[1].index = 1;
    assert!(
        matches!(
            TextMap::decode(&same.encode().unwrap()),
            Err(WireError::Textmap(_))
        ),
        "repeated paragraph index"
    );
    for flags in [0x20u8, 0x80, textmap::EDITABLE | textmap::OBJECT] {
        let mut f = m.clone();
        f.paragraphs[0].segments[0].flags = flags;
        assert!(
            matches!(
                TextMap::decode(&f.encode().unwrap()),
                Err(WireError::Textmap(_))
            ),
            "flags {flags:#x}"
        );
    }
    let mut l = m.clone();
    l.paragraphs[0].segments[0].lang = 5;
    assert!(l.encode().is_err());
    let mut nolang = m.clone();
    nolang.langs[0] = "x".into();
    assert!(nolang.encode().is_err());
    // bad UTF-8 in a segment: the text starts after the 1+2+4 byte segment header; the only 'a' of the blob
    let at = good.iter().rposition(|b| *b == b'a').unwrap();
    let mut bad = good.clone();
    bad[at] = 0xFF;
    assert_eq!(TextMap::decode(&bad), Err(WireError::BadUtf8));
    // apply: changed byte other than 0/1
    assert!(matches!(
        Apply::decode(&[1, 0, 0, 0, 1, 0, 0, 0, 2]),
        Err(WireError::Textmap(_))
    ));
}

/// Parsers must answer every prefix of a valid input with `Err` (never a panic, never `Ok`) and survive any claimed length.
#[test]
fn truncated_inputs_are_errors() {
    let mut r = Rng(42);
    let m = random_map(&mut r);
    let tm = m.encode().unwrap();
    let ap = Apply {
        paragraphs: m
            .paragraphs
            .iter()
            .map(|p| {
                p.segments
                    .iter()
                    .map(|s| s.editable().then(|| "zz".to_string()))
                    .collect()
            })
            .collect(),
    }
    .encode()
    .unwrap();
    let msg = Message::new(&json!({"kind": "command"}), vec![tm.clone(), ap.clone()])
        .unwrap()
        .encode()
        .unwrap();
    for cut in 0..tm.len() {
        assert!(TextMap::decode(&tm[..cut]).is_err(), "textmap cut {cut}");
    }
    for cut in 0..ap.len() {
        assert!(Apply::decode(&ap[..cut]).is_err(), "apply cut {cut}");
    }
    for cut in 0..msg.len() {
        assert!(Message::decode(&msg[..cut]).is_err(), "message cut {cut}");
    }
    assert!(Message::decode(&msg).is_ok());
}

#[test]
fn oversized_claims_are_refused_without_allocating() {
    let max = u32::MAX.to_le_bytes();
    // header length, blob count, blob length, paragraph/segment/lang counts, string length: all claim 4 GiB
    assert!(Message::decode(&[&max[..], b"{}"].concat()).is_err());
    assert_eq!(
        Message::decode(&[&2u32.to_le_bytes()[..], b"{}", &max].concat()),
        Err(WireError::TooManyBlobs)
    );
    assert_eq!(
        Message::decode(&[&2u32.to_le_bytes()[..], b"{}", &1u32.to_le_bytes(), &max].concat()),
        Err(WireError::Truncated)
    );
    assert!(TextMap::decode(&max).is_err());
    assert!(
        TextMap::decode(&[&1u32.to_le_bytes()[..], &0u32.to_le_bytes(), &max].concat()).is_err()
    );
    assert!(
        TextMap::decode(
            &[
                &1u32.to_le_bytes()[..],
                &0u32.to_le_bytes(),
                &1u32.to_le_bytes(),
                &0u32.to_le_bytes(),
                &max
            ]
            .concat()
        )
        .is_err()
    );
    assert!(Apply::decode(&max).is_err());
    assert!(Apply::decode(&[&1u32.to_le_bytes()[..], &max].concat()).is_err());
    // 65536 language tags would not fit a u16 index
    let mut many = (65536u32).to_le_bytes().to_vec();
    many.extend(std::iter::repeat_n(0u8, 65536 * 4));
    assert!(matches!(TextMap::decode(&many), Err(WireError::Textmap(_))));
}

#[test]
fn message_limits() {
    let big = vec![0u8; MAX_MESSAGE + 1];
    assert_eq!(Message::decode(&big), Err(WireError::TooLarge));
    let msg = Message {
        header: "x".repeat(MAX_HEADER + 1),
        blobs: vec![],
    };
    assert_eq!(msg.encode(), Err(WireError::HeaderTooLarge));
    let mut raw = ((MAX_HEADER + 1) as u32).to_le_bytes().to_vec();
    raw.extend(vec![b' '; MAX_HEADER + 1]);
    raw.extend(0u32.to_le_bytes());
    assert_eq!(Message::decode(&raw), Err(WireError::HeaderTooLarge));
    let many = Message {
        header: "{}".into(),
        blobs: vec![vec![]; MAX_BLOBS + 1],
    };
    assert_eq!(many.encode(), Err(WireError::TooManyBlobs));
    let huge = Message {
        header: "{}".into(),
        blobs: vec![vec![0u8; MAX_MESSAGE / 2 + 1]; 2],
    };
    assert_eq!(huge.encode(), Err(WireError::TooLarge));
    // the largest legal message round-trips
    let edge = Message {
        header: "{}".into(),
        blobs: vec![vec![7u8; MAX_MESSAGE - 8 - 2 - 4]],
    };
    let enc = edge.encode().unwrap();
    assert_eq!(enc.len(), MAX_MESSAGE);
    assert_eq!(Message::decode(&enc).unwrap(), edge);
    // not UTF-8 in the header
    let mut bad = 2u32.to_le_bytes().to_vec();
    bad.extend([0xC3, 0x28]);
    bad.extend(0u32.to_le_bytes());
    assert_eq!(Message::decode(&bad), Err(WireError::BadUtf8));
    let m = Message {
        header: "{not json".into(),
        blobs: vec![],
    };
    assert!(matches!(m.header_as::<Call>(), Err(WireError::Json(_))));
}

#[test]
fn random_mutations_never_panic() {
    let mut r = Rng(7);
    let m = random_map(&mut r);
    let seeds = [
        m.encode().unwrap(),
        Apply {
            paragraphs: vec![vec![Some("é".into()), None], vec![]],
        }
        .encode()
        .unwrap(),
        Message::new(&json!({"actions": []}), vec![vec![1, 2, 3]])
            .unwrap()
            .encode()
            .unwrap(),
    ];
    for _ in 0..20_000 {
        let mut b = seeds[r.below(3) as usize].clone();
        for _ in 0..1 + r.below(4) {
            match r.below(3) {
                0 if !b.is_empty() => {
                    let i = r.below(b.len() as u64) as usize;
                    b[i] = r.next() as u8;
                }
                1 if !b.is_empty() => b.truncate(r.below(b.len() as u64) as usize),
                _ => b.push(r.next() as u8),
            }
        }
        let _ = TextMap::decode(&b);
        let _ = Apply::decode(&b);
        let _ = Message::decode(&b);
    }
}

// ---- calls, answers, actions ----

#[test]
fn answers_are_strict_calls_are_lenient() {
    let ok: Answer = serde_json::from_str(r#"{"actions":[{"type":"textmap.apply","blob":0},{"type":"ui.status","text":"done"}],"next":{"reads":[{"what":"textmap","scope":"auto"}]}}"#).unwrap();
    assert_eq!(ok.actions.len(), 2);
    for bad in [
        r#"{"actions":[{"type":"format.all"}]}"#,
        r#"{"actions":[{"type":"run","id":"text.insert","extra":1}]}"#,
        r#"{"actions":[],"surprise":1}"#,
        r#"{"actions":[{"type":"textmap.apply"}]}"#,
        r#"{"actions":[{"type":"read","what":"files"}]}"#,
        r#"{"actions":[{"type":"ui.panel","panel":"p","widgets":[{"type":"iframe"}]}]}"#,
    ] {
        assert!(serde_json::from_str::<Answer>(bad).is_err(), "{bad}");
    }
    // a newer host adds fields and an unknown kind of field: an old plugin keeps working
    let call: Call = serde_json::from_str(r#"{"kind":"command","api":1,"api_minor":3,"app":"docs","lang":"en","command":"c","future":{"x":1}}"#).unwrap();
    assert_eq!(call.api_minor, 3);
    let back: Call = serde_json::from_str(&serde_json::to_string(&call).unwrap()).unwrap();
    assert_eq!(back, call);
}

fn grants(doc: DocAccess) -> (Vec<String>, Vec<String>, DocAccess) {
    (
        vec!["https://api.example.org".into()],
        vec!["fields".into()],
        doc,
    )
}

fn check(a: Action, doc: DocAccess, event: bool) -> Result<(), Denied> {
    let (o, p, d) = grants(doc);
    check_actions(
        &[a],
        &Grants {
            document: d,
            origins: &o,
            panels: &p,
        },
        event,
    )
}

fn run(id: &str, params: serde_json::Value) -> Action {
    Action::Run {
        id: id.into(),
        params,
    }
}

#[test]
fn run_needs_allowlist_and_document_access() {
    assert_eq!(
        check(
            run("text.insert", json!({"text": "x"})),
            DocAccess::Write,
            false
        ),
        Ok(())
    );
    assert!(matches!(
        check(run("text.insert", json!({})), DocAccess::Read, false),
        Err(Denied::NeedsDocument {
            need: DocAccess::Write
        })
    ));
    assert!(matches!(
        check(run("text.insert", json!({})), DocAccess::None, false),
        Err(Denied::NeedsDocument { .. })
    ));
    assert_eq!(
        check(run("document.text", json!({})), DocAccess::Read, false),
        Ok(())
    );
    assert!(matches!(
        check(run("document.text", json!({})), DocAccess::None, false),
        Err(Denied::NeedsDocument {
            need: DocAccess::Read
        })
    ));
    for id in [
        "file.save",
        "file.open",
        "edit.copy",
        "tools.macro",
        "mailings.finish",
        "insert.textFromFile",
        "insert.picture",
        "link.open",
        "view.zoom",
        "readAloud.start",
        "ui.dialog",
        "office.mode",
        "plugin.other.cmd",
        "review.acceptAll",
        "review.rejectAll",
        "document.setText",
        "",
        "text",
        "textual.x",
        "TEXT.insert",
    ] {
        assert!(
            matches!(
                check(run(id, json!({})), DocAccess::Write, false),
                Err(Denied::Command { .. })
            ),
            "{id}"
        );
    }
}

#[test]
fn run_refuses_path_like_params_at_any_depth() {
    for p in [
        json!({"path": "C:/x"}),
        json!({"Path": "x"}),
        json!({"a": {"b": [{"url": "https://x"}]}}),
        json!({"dir": "x"}),
        json!({"FILE": 1}),
        json!({"path": [1, "x"]}),
        json!({"path": [-1]}),
        json!({"path": {"0": 1}}),
        json!({"href": "file:///c:/x"}),
    ] {
        assert!(
            matches!(
                check(run("text.insert", p.clone()), DocAccess::Write, false),
                Err(Denied::Command { .. })
            ),
            "{p}"
        );
    }
    // a node path (array of non-negative integers) is data, not a file
    assert_eq!(
        check(
            run("document.paragraph", json!({"path": [3, 0]})),
            DocAccess::Read,
            false
        ),
        Ok(())
    );
    let mut deep = json!(1);
    for _ in 0..40 {
        deep = json!({ "k": deep });
    }
    assert!(matches!(
        check(run("text.insert", deep), DocAccess::Write, false),
        Err(Denied::Command { .. })
    ));
}

#[test]
fn allowlist_data_is_consistent() {
    for (id, why) in allow::EXCEPTIONS {
        assert!(
            allow::PREFIXES.iter().any(|p| id.starts_with(p)),
            "{id} is an exception to no prefix"
        );
        assert!(!why.is_empty());
        assert!(allow::check_run(id, &json!({})).is_err(), "{id}");
    }
    for r in allow::READS {
        assert_eq!(allow::check_run(r, &json!({})), Ok(DocAccess::Read), "{r}");
    }
    for p in allow::PREFIXES {
        assert!(
            p.ends_with('.') && p.chars().all(|c| c.is_ascii_alphabetic() || c == '.'),
            "{p}"
        );
        for denied in [
            "file.",
            "edit.",
            "tools.",
            "mailings.",
            "link.",
            "view.",
            "readAloud.",
            "ui.",
            "office.",
            "plugin.",
        ] {
            assert_ne!(*p, denied);
        }
    }
}

#[test]
fn textmap_apply_read_and_event_rules() {
    assert_eq!(
        check(Action::TextmapApply { blob: 0 }, DocAccess::Write, false),
        Ok(())
    );
    assert!(matches!(
        check(Action::TextmapApply { blob: 0 }, DocAccess::Read, false),
        Err(Denied::NeedsDocument { .. })
    ));
    assert!(matches!(
        check(Action::TextmapApply { blob: 16 }, DocAccess::Write, false),
        Err(Denied::Invalid(_))
    ));
    let read = Action::Read(ReadReq {
        what: ReadWhat::Textmap,
        scope: Some(Scope::Auto),
        index: None,
    });
    assert_eq!(check(read.clone(), DocAccess::Read, false), Ok(()));
    assert!(matches!(
        check(read.clone(), DocAccess::None, false),
        Err(Denied::NeedsDocument { .. })
    ));
    // events: only ui.*
    assert_eq!(
        check(Action::UiStatus { text: "x".into() }, DocAccess::None, true),
        Ok(())
    );
    assert_eq!(
        check(read, DocAccess::Write, true),
        Err(Denied::EventOnlyUi)
    );
    assert_eq!(
        check(run("text.insert", json!({})), DocAccess::Write, true),
        Err(Denied::EventOnlyUi)
    );
    let (o, p, d) = grants(DocAccess::Write);
    let many = vec![Action::UiStatus { text: "x".into() }; MAX_ACTIONS + 1];
    assert_eq!(
        check_actions(
            &many,
            &Grants {
                document: d,
                origins: &o,
                panels: &p
            },
            false
        ),
        Err(Denied::TooManyActions)
    );
}

fn fetch(url: &str) -> Action {
    Action::Fetch {
        url: url.into(),
        method: "GET".into(),
        headers: Default::default(),
        body: None,
    }
}

#[test]
fn fetch_needs_a_granted_origin() {
    for u in [
        "https://api.example.org",
        "https://api.example.org/v1/x?q=1#f",
        "https://api.example.org?x",
    ] {
        assert_eq!(check(fetch(u), DocAccess::None, false), Ok(()), "{u}");
    }
    for u in [
        "https://other.example.org/",
        "http://api.example.org/",
        "https://api.example.org:443/",
        "https://API.example.org/",
        "https://api.example.org@evil.example.org/",
        "https://evil.example.org\\@api.example.org/",
        "https://api.example.org\\.evil.example.org/",
        "https://api.example.org%2f@evil.example.org/",
        "https://user:pw@api.example.org/",
        "https://api.example.org/a b",
        "https://api.example.org/\n",
        "https://api.example.org.evil.org/",
        "https://evil.org/https://api.example.org/",
        "file:///c:/x",
        "data:text/plain,x",
        "//api.example.org/",
        "",
    ] {
        assert!(
            matches!(
                check(fetch(u), DocAccess::None, false),
                Err(Denied::Network(_))
            ),
            "{u}"
        );
    }
    assert_eq!(
        url_origin("https://api.example.org/x"),
        Some("https://api.example.org")
    );
    assert_eq!(
        url_origin(&format!("https://api.example.org/{}", "a".repeat(3000))),
        None
    );
    let with = |method: &str, hk: &str| Action::Fetch {
        url: "https://api.example.org/".into(),
        method: method.into(),
        headers: [(hk.to_string(), "v".to_string())].into(),
        body: None,
    };
    assert_eq!(
        check(with("POST", "X-Api-Key"), DocAccess::None, false),
        Ok(())
    );
    for hk in [
        "Host",
        "cookie",
        "Sec-Fetch-Mode",
        "Proxy-Authorization",
        "Content-Length",
        "bad header",
        "",
    ] {
        assert!(
            matches!(
                check(with("GET", hk), DocAccess::None, false),
                Err(Denied::Invalid(_))
            ),
            "{hk}"
        );
    }
    assert!(matches!(
        check(with("CONNECT", "X-A"), DocAccess::None, false),
        Err(Denied::Invalid(_))
    ));
    assert!(matches!(
        check(with("get", "X-A"), DocAccess::None, false),
        Err(Denied::Invalid(_))
    ));
    // no origin granted at all: nothing passes
    let r = check_actions(
        &[fetch("https://api.example.org/")],
        &Grants {
            document: DocAccess::Write,
            origins: &[],
            panels: &[],
        },
        false,
    );
    assert!(matches!(r, Err(Denied::Network(_))));
}

#[test]
fn panels_and_widgets() {
    let w = |ws: Vec<Widget>| {
        check(
            Action::UiPanel {
                panel: "fields".into(),
                widgets: ws,
            },
            DocAccess::None,
            false,
        )
    };
    let full = vec![
        Widget::Heading { text: "H".into() },
        Widget::Text {
            text: "a\nb".into(),
        },
        Widget::List {
            items: vec![ListItem {
                id: Some("i1".into()),
                text: "x".into(),
            }],
        },
        Widget::Button {
            id: "go".into(),
            label: "Go".into(),
        },
        Widget::TextField {
            id: "q".into(),
            value: String::new(),
            placeholder: Some("p".into()),
        },
        Widget::Select {
            id: "s".into(),
            options: vec![SelectOption {
                value: "1".into(),
                label: "one".into(),
            }],
            value: Some("1".into()),
        },
    ];
    assert_eq!(w(full.clone()), Ok(()));
    assert!(matches!(
        check(
            Action::UiPanel {
                panel: "undeclared".into(),
                widgets: vec![]
            },
            DocAccess::Write,
            false
        ),
        Err(Denied::UnknownPanel(_))
    ));
    assert!(
        w(vec![Widget::Heading {
            text: "x".repeat(1025)
        }])
        .is_err()
    );
    assert!(
        w(vec![Widget::Heading {
            text: "a\nb".into()
        }])
        .is_err()
    );
    assert!(
        w(vec![Widget::Text {
            text: "a\u{202E}b".into()
        }])
        .is_err()
    );
    assert!(
        w(vec![Widget::Button {
            id: "a b".into(),
            label: "x".into()
        }])
        .is_err()
    );
    assert!(
        w(vec![Widget::Button {
            id: String::new(),
            label: "x".into()
        }])
        .is_err()
    );
    assert!(
        w((0..65)
            .map(|_| Widget::Heading { text: "x".into() })
            .collect())
        .is_err()
    );
    assert!(
        w(vec![Widget::List {
            items: (0..257)
                .map(|_| ListItem {
                    id: None,
                    text: "x".into()
                })
                .collect()
        }])
        .is_err()
    );
    // the wire shape of a widget
    let v = serde_json::to_value(&full[4]).unwrap();
    assert_eq!(v["type"], "textField");
    assert!(
        check(
            Action::UiStatus {
                text: "x".repeat(257)
            },
            DocAccess::None,
            false
        )
        .is_err()
    );
    assert!(
        check(
            Action::UiStatus {
                text: "a\u{0}".into()
            },
            DocAccess::None,
            false
        )
        .is_err()
    );
}

#[test]
fn spike_style_round_trip_through_a_message() {
    let mut m = TextMap::default();
    let l = m.lang_index("uz-Latn").unwrap();
    m.paragraphs.push(Paragraph {
        index: 0,
        segments: vec![Segment {
            flags: textmap::EDITABLE,
            lang: l,
            text: "salom dunyo".into(),
        }],
    });
    let call = Call {
        kind: CallKind::Command,
        api: 1,
        api_minor: plugin_api::API_MINOR,
        app: "docs".into(),
        lang: "uz-Latn".into(),
        command: Some("toCyrillic".into()),
        widget: None,
        event: None,
        params: Default::default(),
        settings: Default::default(),
        results: vec![],
        reads: [("textmap".to_string(), 0)].into(),
    };
    let bytes = Message::new(&call, vec![m.encode().unwrap()])
        .unwrap()
        .encode()
        .unwrap();
    let got = Message::decode(&bytes).unwrap();
    assert_eq!(got.header_as::<Call>().unwrap(), call);
    assert_eq!(TextMap::decode(&got.blobs[0]).unwrap(), m);
}
