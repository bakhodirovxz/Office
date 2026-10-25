//! Golden diffs on synthetic pairs (PLAN phase 5, WP12 done-check). Each pair is built here from invented text, saved
//! with the upstream writer and read back (so the reader's tracked-change parsing is in the path), then diffed; the
//! result must equal `tests/golden/<name>.json`. `UPDATE_GOLDEN=1 cargo test -p docs-diff` rewrites the files.
use std::path::PathBuf;

use wordcraft_doc::{Block, CharProps, Document, Paragraph, Revision, RevisionKind, Run, Table, para_block};

const BASE: [&str; 6] = [
    "Annual garden report",
    "The quick brown fox jumps over the lazy dog.",
    "Tomatoes grew well this year, and the beans were late.",
    "Oʻzbekiston bogʻlarida olma va oʻrik pishdi.",
    "The pond was cleaned twice, in April and in September.",
    "Next year we plant more pumpkins.",
];

fn p(t: &str) -> Paragraph {
    Paragraph::with_text(t, CharProps::default())
}

fn doc(paras: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    d.body = paras.into_iter().map(para_block).collect();
    d
}

fn base() -> Vec<Paragraph> {
    BASE.iter().map(|t| p(t)).collect()
}

/// Through the writer and the reader, as a saved version would be.
fn saved(d: &Document) -> Document {
    let bytes = wordcraft_docx::write(d).expect("write");
    docs_diff::read(&bytes).expect("read")
}

fn golden(name: &str, a: Document, b: Document) -> serde_json::Value {
    let out = docs_diff::diff(&saved(&a), &saved(&b));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.json"));
    let text = serde_json::to_string_pretty(&out).expect("json") + "\n";
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
        std::fs::write(&path, &text).expect("write golden");
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} missing (UPDATE_GOLDEN=1)", path.display()));
    assert_eq!(text.replace("\r\n", "\n"), want.replace("\r\n", "\n"), "{name}: diff differs from the golden");
    out
}

fn ops(v: &serde_json::Value) -> Vec<String> {
    v["changes"].as_array().expect("changes").iter().map(|c| c["op"].as_str().unwrap_or("?").to_string()).collect()
}

#[test]
fn identical() {
    let v = golden("identical", doc(base()), doc(base()));
    assert!(ops(&v).is_empty());
}

#[test]
fn insert() {
    let mut b = base();
    b.insert(3, p("A new paragraph about the apple harvest."));
    let v = golden("insert", doc(base()), doc(b));
    assert_eq!(ops(&v), ["insert"]);
    assert_eq!(v["changes"][0]["b"], 3);
}

#[test]
fn delete() {
    let mut b = base();
    b.remove(2);
    let v = golden("delete", doc(base()), doc(b));
    assert_eq!(ops(&v), ["delete"]);
    assert_eq!(v["changes"][0]["a"], 2);
}

#[test]
fn moved() {
    let mut b = base();
    let x = b.remove(4);
    b.insert(1, x);
    let v = golden("move", doc(base()), doc(b));
    assert_eq!(ops(&v), ["move"]);
    assert_eq!((v["changes"][0]["a"].as_u64(), v["changes"][0]["b"].as_u64()), (Some(4), Some(1)));
}

#[test]
fn modify_words() {
    let mut b = base();
    b[1] = p("The quick red fox jumps over the sleeping dog.");
    let v = golden("modify", doc(base()), doc(b));
    assert_eq!(ops(&v), ["modify"]);
    let spans = v["changes"][0]["spans"].to_string();
    assert!(spans.contains(r#"{"del":"brown"}"#) && spans.contains(r#"{"ins":"red"}"#), "{spans}");
}

#[test]
fn format_only() {
    let mut b = base();
    // "quick" made bold (a run split mid-paragraph), and the title turned into a heading
    let t = BASE[1];
    let bold = CharProps { bold: Some(true), ..CharProps::default() };
    b[1].runs = vec![Run { len: 4, props: CharProps::default() }, Run { len: 5, props: bold }, Run { len: t.len() - 9, props: CharProps::default() }];
    b[0] = p(BASE[0]).styled("Heading1");
    let v = golden("format", doc(base()), doc(b));
    assert_eq!(ops(&v), ["format", "format"]);
    assert_eq!(v["changes"][1]["format"]["runs"][0]["text"], "quick");
    assert_eq!(v["changes"][0]["format"]["para"]["style"][1], "Heading1");
}

#[test]
fn blame_from_tracked_changes() {
    let mut b = base();
    let mut d = Document::new();
    d.revisions = vec![
        Revision::new(RevisionKind::Insert, "Alice Example", "2026-10-01T09:00:00Z"),
        Revision::new(RevisionKind::Delete, "Bob Example", "2026-10-02T09:00:00Z"),
    ];
    // a tracked insertion at the end of paragraph 2
    let extra = " Peppers too.";
    let t = format!("{}{extra}", BASE[2]);
    b[2] = p(&t);
    b[2].runs = vec![
        Run { len: BASE[2].len(), props: CharProps::default() },
        Run { len: extra.len(), props: CharProps { ins: Some(0), ..CharProps::default() } },
    ];
    // paragraph 4 deleted with tracking on: text and mark
    let del = CharProps { del: Some(1), ..CharProps::default() };
    b[4].runs = vec![Run { len: BASE[4].len(), props: del.clone() }];
    b[4].mark = del;
    d.body = b.into_iter().map(para_block).collect();
    let v = golden("blame", doc(base()), d);
    assert_eq!(ops(&v), ["modify", "delete"]);
    assert_eq!(v["changes"][0]["authors"][0], "Alice Example");
    assert_eq!(v["changes"][1]["authors"][0], "Bob Example");
}

#[test]
fn table_cell_edit() {
    let table = |cell: &str| {
        let mut t = Table::new(2, 2, 400.0);
        for (r, row) in t.rows.iter_mut().enumerate() {
            for (c, x) in row.cells.iter_mut().enumerate() {
                *x = wordcraft_doc::Cell::with_text(if (r, c) == (1, 1) { cell } else { ["Crop", "Kilograms", "Beans", ""][r * 2 + c] });
            }
        }
        std::sync::Arc::new(Block::Table(t))
    };
    let mut a = doc(base());
    a.body.insert(3, table("12"));
    let mut b = doc(base());
    b.body.insert(3, table("15"));
    let v = golden("table", a, b);
    assert_eq!(ops(&v), ["modify"]);
    assert_eq!(v["changes"][0]["path"], "3/r1c1/0");
}

#[test]
fn rewrite_is_delete_then_insert() {
    let v = golden("rewrite", doc(base()), doc(vec![p("Completely different words here."), p("Nothing in common at all.")]));
    assert!(ops(&v).iter().all(|o| o == "delete" || o == "insert"));
}
