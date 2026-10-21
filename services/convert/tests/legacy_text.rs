//! WP9 fix 5: text in cp1251, cp1252 or UTF-16 is
//! transcoded to UTF-8 before the tool, with a `text_transcoded` warning that names the encoding. Hermetic: the tools are the fake one,
//! and a hook looks at the bytes the job holds when the tools would run.
use convert::exec::{Config, Exit, Failure};
use convert::hook::{JobCtx, JobHook, ToolRunner};
use convert::job::{self, Dest, FileReport};
use convert::plan::{Options, Target, ToolId};
use convert::report::Warning;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let p = std::env::temp_dir().join(format!("convert-legacy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Sandbox(p)
    }
    fn cfg(&self, seen: Option<Arc<Mutex<Vec<u8>>>>) -> Config {
        let mut c = Config::from_env();
        c.work_dir = self.0.join("work");
        for t in ToolId::ALL {
            c.tools.insert(t, env!("CARGO_BIN_EXE_convert-fake-tool").into());
        }
        c.sample_ms = 50;
        if let Some(seen) = seen {
            c.hooks.push(Arc::new(Spy(seen)));
        }
        c
    }
    fn convert(&self, name: &str, bytes: &[u8], to: Target) -> (FileReport, Vec<u8>) {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        let seen = Arc::new(Mutex::new(vec![]));
        let dest = Dest { out_dir: self.0.join("dest"), out_file: None };
        let r = job::convert_file(&self.cfg(Some(seen.clone())), &p, to, &Options::default(), &dest);
        let got = seen.lock().unwrap().clone();
        (r, got)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Records the job's input file as the tools would see it.
struct Spy(Arc<Mutex<Vec<u8>>>);

impl JobHook for Spy {
    fn before(&self, ctx: &JobCtx, _t: &dyn ToolRunner) -> Result<Vec<Warning>, Failure> {
        *self.0.lock().unwrap() = std::fs::read(&ctx.input).unwrap_or_default();
        Ok(vec![])
    }
}

fn codes(r: &FileReport) -> Vec<&str> {
    r.warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn cp1251_text_is_transcoded_and_named() {
    let s = Sandbox::new("cp1251");
    // "Буйруқ № 125-к: Тошкент шаҳри" has letters cp1251 lacks (қ, ҳ): they come out as '?' in Word too; the Cyrillic ones survive
    let bytes: &[u8] = b"\xC1\xF3\xE9\xF0\xF3? \xB9 125-\xEA\r\n\xD2\xEE\xF8\xEA\xE5\xED\xF2 \xF8\xE0?\xF0\xE8, 2026 \xE9\xE8\xEB 15 \xEE\xEA\xF2\xFF\xE1\xF0\xFC\r\n";
    for (name, to) in [("order.txt", Target::Pdf), ("order.csv", Target::Pdf), ("order.md", Target::Pdf)] {
        let (r, seen) = s.convert(name, bytes, to);
        assert_eq!(r.code, 0, "{name}: {}", r.message);
        assert_eq!(codes(&r), ["text_transcoded"], "{name}");
        assert!(r.warnings[0].text.contains("cp1251"), "{}", r.warnings[0].text);
        let text = String::from_utf8(seen).expect("the tool gets UTF-8");
        assert!(text.starts_with("\u{411}\u{443}\u{439}\u{440}\u{443}?"), "{text}");
        assert!(text.contains("\u{2116} 125-\u{43a}\r\n") && text.contains("2026 \u{439}\u{438}\u{43b} 15 \u{43e}\u{43a}\u{442}\u{44f}\u{431}\u{440}\u{44c}"), "{text}");
    }
}

#[test]
fn cp1252_text_is_transcoded_and_named() {
    let s = Sandbox::new("cp1252");
    let bytes: &[u8] = b"Le caf\xE9 du d\xE9jeuner co\xFBte 5 \x80; na\xEFve \xE0 la fa\xE7on, \xFCber, se\xF1or.\r\n";
    let (r, seen) = s.convert("menu.txt", bytes, Target::Pdf);
    assert_eq!(r.code, 0, "{}", r.message);
    assert_eq!(codes(&r), ["text_transcoded"]);
    assert!(r.warnings[0].text.contains("cp1252"), "{}", r.warnings[0].text);
    assert_eq!(String::from_utf8(seen).unwrap(), "Le caf\u{e9} du d\u{e9}jeuner co\u{fb}te 5 \u{20ac}; na\u{ef}ve \u{e0} la fa\u{e7}on, \u{fc}ber, se\u{f1}or.\r\n");
}

#[test]
fn utf16_with_a_bom_is_transcoded_in_both_byte_orders() {
    let s = Sandbox::new("utf16");
    let text = "Salom, dunyo \u{43c}\u{438}\u{440}\r\nIkkinchi qator\r\n";
    let le: Vec<u8> = [&[0xFF, 0xFE][..], &text.encode_utf16().flat_map(|c| c.to_le_bytes()).collect::<Vec<_>>()].concat();
    let be: Vec<u8> = [&[0xFE, 0xFF][..], &text.encode_utf16().flat_map(|c| c.to_be_bytes()).collect::<Vec<_>>()].concat();
    for (name, bytes, label) in [("le.txt", le.clone(), "UTF-16 LE"), ("be.txt", be, "UTF-16 BE"), ("le.csv", le, "UTF-16 LE")] {
        let (r, seen) = s.convert(name, &bytes, if name.ends_with("csv") { Target::Pdf } else { Target::Pdf });
        assert_eq!(r.code, 0, "{name}: {}", r.message);
        assert!(r.warnings[0].text.contains(label), "{name}: {}", r.warnings[0].text);
        assert_eq!(String::from_utf8(seen).unwrap(), text, "{name}");
    }
}

#[test]
fn utf8_text_is_not_touched_and_gets_no_warning() {
    let s = Sandbox::new("utf8");
    let text = "Salom dunyo, \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}\r\n";
    let (r, seen) = s.convert("a.txt", text.as_bytes(), Target::Pdf);
    assert_eq!((r.code, r.warnings.len()), (0, 0), "{}", r.message);
    assert_eq!(seen, text.as_bytes());
    // a legacy-encoded file with a binary extension is still refused, not read as text
    let (r, _) = s.convert("a.docx", b"\xC1\xD3\xC9\xD0\xD3", Target::Pdf);
    assert_eq!(r.code, Exit::Unsupported.code(), "{}", r.message);
}
