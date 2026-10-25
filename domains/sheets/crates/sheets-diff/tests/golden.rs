//! Golden diffs on synthetic pairs (PLAN phase 5, WP12 done-check). Each pair is built here from invented data, saved
//! with the upstream writer and read back, then diffed; the result must equal `tests/golden/<name>.json`.
//! `UPDATE_GOLDEN=1 cargo test -p sheets-diff` rewrites the files.
use std::path::PathBuf;
use std::sync::Arc;

use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Formula, Sheet, StyleId, Workbook};

const ITEMS: [(&str, f64, f64); 5] =
    [("Seeds", 4.0, 2.5), ("Compost", 2.0, 9.0), ("Gloves", 3.0, 4.0), ("Watering can", 1.0, 12.0), ("Twine", 6.0, 1.5)];

/// A row of the budget: item, quantity, price and `=qty*price`, starting at column `c0` (0-based) on row `r` (0-based).
fn row(s: &mut Sheet, r: u32, c0: u32, (item, qty, price): (&str, f64, f64)) {
    s.set_cell(CellRef::new(r, c0), Cell::value(Value::text(item)));
    s.set_cell(CellRef::new(r, c0 + 1), Cell::value(Value::number(qty)));
    s.set_cell(CellRef::new(r, c0 + 2), Cell::value(Value::number(price)));
    let (q, p) = (CellRef::new(r, c0 + 1).a1(), CellRef::new(r, c0 + 2).a1());
    s.set_cell(CellRef::new(r, c0 + 3), Cell::formula(Formula::new(&format!("={q}*{p}"))));
}

fn budget(items: &[(&str, f64, f64)]) -> Sheet {
    let mut s = Sheet::new("Budget");
    for (c, h) in ["Item", "Qty", "Price", "Total"].iter().enumerate() {
        s.set_cell(CellRef::new(0, c as u32), Cell::value(Value::text(*h)));
    }
    for (i, it) in items.iter().enumerate() {
        row(&mut s, i as u32 + 1, 0, *it);
    }
    s
}

fn notes(name: &str) -> Sheet {
    let mut s = Sheet::new(name);
    s.set_cell(CellRef::new(0, 0), Cell::value(Value::text("Plant after the last frost.")));
    s.set_cell(CellRef::new(1, 0), Cell::value(Value::text("Water in the morning.")));
    s
}

fn book(sheets: Vec<Sheet>) -> Workbook {
    let mut wb = Workbook::new();
    wb.sheets = sheets.into_iter().map(Arc::new).collect();
    wb
}

fn base() -> Workbook {
    book(vec![budget(&ITEMS), notes("Notes")])
}

fn edit(wb: &mut Workbook, i: usize, f: impl FnOnce(&mut Sheet)) {
    f(Arc::make_mut(&mut wb.sheets[i]));
}

fn golden(name: &str, a: &Workbook, b: &Workbook) -> serde_json::Value {
    let saved = |wb: &Workbook| sheets_diff::read(&gridcraft_xlsx::write_xlsx(wb).expect("write")).expect("read");
    let out = sheets_diff::diff(&saved(a), &saved(b));
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

#[test]
fn identical() {
    let v = golden("identical", &base(), &base());
    assert!(v["sheets"].as_array().is_some_and(Vec::is_empty));
}

#[test]
fn value_change() {
    let mut b = base();
    edit(&mut b, 0, |s| s.set_cell(CellRef::new(2, 1), Cell::value(Value::number(7.0))));
    let v = golden("value", &base(), &b);
    let cells = &v["sheets"][0]["cells"];
    assert_eq!(cells.as_array().map(Vec::len), Some(1));
    assert_eq!((cells[0]["b"].as_str(), cells[0]["old"]["value"].as_str(), cells[0]["new"]["value"].as_str()), (Some("B3"), Some("2"), Some("7")));
}

#[test]
fn formula_change() {
    let mut b = base();
    edit(&mut b, 0, |s| s.set_cell(CellRef::new(3, 3), Cell::formula(Formula::new("=B4*C4*1.1"))));
    let v = golden("formula", &base(), &b);
    let c = &v["sheets"][0]["cells"][0];
    assert_eq!((c["old"]["formula"].as_str(), c["new"]["formula"].as_str()), (Some("=B4*C4"), Some("=B4*C4*1.1")));
}

#[test]
fn row_insert_is_one_change() {
    let mut items = ITEMS.to_vec();
    items.insert(2, ("Rake", 1.0, 15.0));
    let v = golden("row-insert", &base(), &book(vec![budget(&items), notes("Notes")]));
    let s = &v["sheets"][0];
    assert_eq!(s["rows"]["inserted"], serde_json::json!([[4, 1]]));
    // only the new row's own cells, none of the rows it pushed down
    let cells = s["cells"].as_array().expect("cells");
    assert_eq!(cells.len(), 4);
    assert!(cells.iter().all(|c| c["a"].is_null() && c["b"].as_str().is_some_and(|b| b.ends_with('4'))), "{cells:?}");
}

#[test]
fn row_delete() {
    let mut items = ITEMS.to_vec();
    items.remove(0);
    let v = golden("row-delete", &base(), &book(vec![budget(&items), notes("Notes")]));
    assert_eq!(v["sheets"][0]["rows"]["deleted"], serde_json::json!([[2, 1]]));
    let cells = v["sheets"][0]["cells"].as_array().expect("cells").clone();
    assert!(cells.len() == 4 && cells.iter().all(|c| c["b"].is_null() && c["a"].as_str().is_some_and(|a| a.ends_with('2'))), "{cells:?}");
}

#[test]
fn column_insert_is_one_change() {
    // a "Category" column inserted before Qty: every formula moves right, as Excel would rewrite it
    let mut s = Sheet::new("Budget");
    for (c, h) in ["Item", "Category", "Qty", "Price", "Total"].iter().enumerate() {
        s.set_cell(CellRef::new(0, c as u32), Cell::value(Value::text(*h)));
    }
    for (i, it) in ITEMS.iter().enumerate() {
        let r = i as u32 + 1;
        row(&mut s, r, 1, *it);
        s.set_cell(CellRef::new(r, 0), Cell::value(Value::text(it.0)));
        s.set_cell(CellRef::new(r, 1), Cell::value(Value::text("Garden")));
    }
    let v = golden("col-insert", &base(), &book(vec![s, notes("Notes")]));
    let sh = &v["sheets"][0];
    assert_eq!(sh["cols"]["inserted"], serde_json::json!([["B", 1]]));
    // only the new column's own cells, none of the cells it pushed right
    let cells = sh["cells"].as_array().expect("cells");
    assert!(cells.len() == 6 && cells.iter().all(|c| c["b"].as_str().is_some_and(|b| b.starts_with('B'))), "{cells:?}");
}

#[test]
fn sheets_added_removed_renamed() {
    let mut extra = notes("Extra");
    extra.set_cell(CellRef::new(5, 2), Cell::value(Value::number(1.0)));
    let b = book(vec![budget(&ITEMS), notes("Hints"), extra]);
    let mut a = base();
    a.sheets.push(Arc::new(Sheet::new("Old")));
    let v = golden("sheets", &a, &b);
    let ops: Vec<(&str, &str)> =
        v["sheets"].as_array().expect("sheets").iter().map(|s| (s["op"].as_str().unwrap_or(""), s["name"].as_str().unwrap_or(""))).collect();
    assert_eq!(ops, [("rename", "Hints"), ("insert", "Extra"), ("delete", "Old")]);
}

#[test]
fn format_only() {
    let mut b = base();
    let mut bold = b.styles.get(StyleId::DEFAULT).clone();
    bold.font.bold = true;
    let id = b.styles.intern(bold);
    edit(&mut b, 0, |s| s.set_cell(CellRef::new(0, 0), Cell { value: Value::text("Item"), formula: None, style: id }));
    let v = golden("format", &base(), &b);
    let c = &v["sheets"][0]["cells"][0];
    assert_eq!((c["b"].as_str(), c["format"].as_bool()), (Some("A1"), Some(true)));
    assert_eq!(c["old"], c["new"]);
}

#[test]
fn row_insert_next_to_an_edit() {
    let mut items = ITEMS.to_vec();
    items.insert(2, ("Rake", 1.0, 15.0));
    items[3].1 = 5.0; // Gloves, now one row lower: 3 -> 5
    let v = golden("row-insert-edit", &base(), &book(vec![budget(&items), notes("Notes")]));
    let s = &v["sheets"][0];
    assert_eq!(s["rows"]["inserted"], serde_json::json!([[4, 1]]));
    let c = &s["cells"][0];
    assert_eq!((c["a"].as_str(), c["b"].as_str(), s["cells"].as_array().map(Vec::len)), (Some("B4"), Some("B5"), Some(5)));
}
