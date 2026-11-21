//! Phase 7 WP6, PLAN section 6.3 (output): for every fixture x direction x {tracked off, on} x {document, selection, caret}, the
//! plugin run (the real wasm module in the native host) and the built-in run start from the same opened document and must end with
//! an equal in-memory document, an equal selection and a byte-identical saved docx; one undo restores the document.
//! Direction "to Latin" takes the built-in's Cyrillic result as its input (as `preserve.rs` does), so it runs on real Cyrillic text.
mod common;

use serde_json::json;
use wordcraft_doc::{Pos, StoryRef};
use wordcraft_engine::{Selection, Session, io};

const WORD: &[&str] = &[
    "01-split-runs",
    "02-hyperlinks",
    "03-fields",
    "04-tables",
    "05-parts",
    "06-tracked-off",
    "06-tracked-on",
    "07-languages",
    "08-controls-codes",
];

fn open(name: &str, bytes: &[u8], tracked: bool) -> Session {
    let (doc, _) = io::open_bytes_with(name, bytes, None).unwrap();
    let mut s = Session::new(doc);
    docs_translit::install(&mut s);
    docs_plugins::register(&mut s, "en");
    s.doc.settings.track_changes = tracked;
    s
}

/// From a few characters into the first paragraph to the end of the middle one; `None` when the body has no paragraph.
fn selection(s: &Session) -> Option<Selection> {
    let paths = s.doc.para_paths(StoryRef::Body);
    let (first, mid) = (paths.first()?, &paths[paths.len() / 2]);
    let a = s.doc.para(StoryRef::Body, first)?;
    let off = a.text.char_indices().nth(2).map_or(0, |(i, _)| i);
    let end = s.doc.para(StoryRef::Body, mid)?.len();
    Some(Selection {
        anchor: Pos {
            story: StoryRef::Body,
            path: first.clone(),
            off,
        },
        focus: Pos {
            story: StoryRef::Body,
            path: mid.clone(),
            off: end,
        },
    })
}

#[derive(Default)]
struct Tally {
    same: usize,
    noise: usize,
    diff: Vec<String>,
}

struct Case<'a> {
    label: &'a str,
    name: &'a str,
    bytes: &'a [u8],
    builtin: &'a str,
    plugin: &'a str,
    tracked: bool,
    scope: &'a str,
}

/// One comparison: (the open noise flag, what differs).
fn once(c: &Case) -> (bool, Vec<String>) {
    let (mut a, mut b) = (
        open(c.name, c.bytes, c.tracked),
        open(c.name, c.bytes, c.tracked),
    );
    if c.scope == "selection"
        && let Some(sel) = selection(&a)
    {
        a.sel = sel.clone();
        b.sel = sel;
    }
    let open_noise = a.doc != b.doc;
    let before = b.doc.clone();
    let params = json!({ "scope": if c.scope == "document" { "document" } else { "selection" } });
    let (ra, rb) = (a.run(c.builtin, &params), b.run(c.plugin, &params));
    let mut why = Vec::new();
    if ra.is_err() != rb.is_err() {
        why.push(format!(
            "results differ: builtin ok={} plugin ok={}",
            ra.is_ok(),
            rb.is_ok()
        ));
    }
    // Opening the same bytes twice must give equal documents; some fixtures carry a random id (the glossary part) that it does not.
    // Such a case still has to save identical bytes, but the in-memory comparison is not meaningful and is counted apart.
    if !open_noise && a.doc != b.doc {
        why.push("in-memory document differs".to_string());
    }
    if a.sel != b.sel {
        why.push("selection differs".to_string());
    }
    let (sa, sb) = (
        io::save_bytes("out.docx", &a.doc).unwrap(),
        io::save_bytes("out.docx", &b.doc).unwrap(),
    );
    if sa != sb {
        why.push(format!(
            "saved docx differs ({} vs {} bytes)",
            sa.len(),
            sb.len()
        ));
        if let Some(dir) = std::env::var_os("UZ_DUMP_DIFFS") {
            let d = std::path::Path::new(&dir);
            std::fs::write(d.join("builtin.docx"), &sa).unwrap();
            std::fs::write(d.join("plugin.docx"), &sb).unwrap();
        }
    }
    if rb.is_ok() {
        b.run("edit.undo", &json!({})).unwrap();
        if b.doc != before {
            why.push("undo does not restore the document".to_string());
        }
    }
    (open_noise, why)
}

fn secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A tracked run stamps its revisions with the wall clock to the second, so two runs that straddle a second boundary legitimately differ:
/// such a comparison is repeated (up to 5 times) until both runs fall in the same second.
fn case(t: &mut Tally, c: &Case) {
    let (mut noise, mut why) = (false, Vec::new());
    for _ in 0..5 {
        let s0 = secs();
        (noise, why) = once(c);
        if why.is_empty() || secs() == s0 {
            break;
        }
    }
    t.noise += usize::from(noise);
    if why.is_empty() {
        t.same += 1;
    } else {
        t.diff.push(format!(
            "{} {} tracked={} scope={}: {}",
            c.label,
            c.builtin,
            c.tracked,
            c.scope,
            why.join("; ")
        ));
    }
}

fn all(t: &mut Tally, label: &str, name: &str, bytes: &[u8], to_cyrl: bool) {
    let (builtin, plugin) = if to_cyrl {
        ("tools.translitCyrl", "plugin.uz-translit.toCyrillic")
    } else {
        ("tools.translitLatn", "plugin.uz-translit.toLatin")
    };
    for tracked in [false, true] {
        for scope in ["document", "selection", "caret"] {
            case(
                t,
                &Case {
                    label,
                    name,
                    bytes,
                    builtin,
                    plugin,
                    tracked,
                    scope,
                },
            );
        }
    }
    case(
        t,
        &Case {
            label,
            name,
            bytes,
            builtin: "tools.translitAuto",
            plugin: "plugin.uz-translit.auto",
            tracked: false,
            scope: "document",
        },
    );
}

#[test]
fn plugin_output_is_identical_to_the_builtin() {
    let Some(wasm) = common::wasm() else { return };
    common::install(&wasm);
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut t = Tally::default();
    for n in WORD {
        let name = format!("{n}.docx");
        let bytes = std::fs::read(format!("{dir}/word/{name}")).unwrap();
        all(&mut t, &name, &name, &bytes, true);
        // the way back, on the built-in's Cyrillic result
        let mut s = open(&name, &bytes, false);
        s.run("tools.translitCyrl", &json!({"scope": "document"}))
            .unwrap();
        let cyr = io::save_bytes("cyr.docx", &s.doc).unwrap();
        all(&mut t, &format!("{name} (cyrl)"), "cyr.docx", &cyr, false);
    }
    for n in ["uz108-latn.docx", "uz108-cyrl.docx"] {
        let bytes = std::fs::read(format!("{dir}/{n}")).unwrap();
        all(&mut t, n, n, &bytes, n.contains("latn"));
    }
    println!(
        "OUTPUT MATRIX: identical {}, differing {} (in-memory comparison skipped by open noise: {})",
        t.same,
        t.diff.len(),
        t.noise
    );
    for d in &t.diff {
        println!("DIFF {d}");
    }
    docs_plugins::clear();
    assert!(
        t.diff.is_empty(),
        "{} of {} cases differ",
        t.diff.len(),
        t.same + t.diff.len()
    );
}
