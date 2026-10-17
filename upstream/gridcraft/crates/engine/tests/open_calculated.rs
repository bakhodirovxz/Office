//! A workbook whose values come from a full calculation (an XLSX file with a `calcPr` that asks
//! for no calculation on open, as Excel saves it) opens with its values as they are, the way Excel
//! opens it; Calculate Now (F9) calculates everything. A file that asks for a calculation on open
//! (`fullCalcOnLoad`) is calculated when it opens, as before.

use gridcraft_core::{CellRef, Value};
use gridcraft_engine::Session;
use gridcraft_engine::model::{Cell, Formula, Workbook};
use serde_json::json;

fn at(a: &str) -> CellRef {
    CellRef::parse(a).unwrap()
}

fn value(s: &Session, a: &str) -> Value {
    s.active().unwrap().wb.sheet(0).unwrap().value(at(a))
}

/// The bytes of a workbook with A1 = 2 and B1 = A1*2 whose cached value is `cached`.
fn file(cached: f64, full_calc_on_load: bool) -> Vec<u8> {
    let mut wb = Workbook::new();
    let sh = wb.sheet_mut(0).unwrap();
    sh.cells.set(at("A1"), Cell::value(Value::Number(2.0)));
    sh.cells.set(at("B1"), Cell { value: Value::Number(cached), ..Cell::formula(Formula::new("=A1*2")) });
    let bytes = gridcraft_engine::io::save_bytes(&wb, "book.xlsx").unwrap();
    if !full_calc_on_load {
        return bytes;
    }
    // The same package with `fullCalcOnLoad` in its `calcPr`.
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "xl/workbook.xml" {
            let text = String::from_utf8(data).unwrap();
            assert!(text.contains("<calcPr ") && !text.contains("fullCalcOnLoad"), "{text}");
            data = text.replacen("<calcPr ", "<calcPr fullCalcOnLoad=\"1\" ", 1).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn open(bytes: &[u8], name: &str) -> Session {
    let path = std::env::temp_dir().join(format!("gridcraft-open-calculated-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let mut s = Session::new();
    s.execute("file.open", json!({"path": path.to_str().unwrap()})).unwrap();
    let _ = std::fs::remove_file(&path);
    s
}

#[test]
fn a_calculated_file_opens_with_its_values() {
    // A stale cached value shows it: the file's 5 is kept where a calculation gives 4.
    let mut s = open(&file(5.0, false), "kept.xlsx");
    assert_eq!(value(&s, "B1"), Value::Number(5.0));
    assert!(!s.active().unwrap().is_dirty());
    // Edits recalculate their dependents, as after any open.
    s.execute("cell.set", json!({"cell": "A1", "input": "3"})).unwrap();
    assert_eq!(value(&s, "B1"), Value::Number(6.0));
    let mut s = open(&file(5.0, false), "f9.xlsx");
    s.execute("formulas.calculateNow", json!({})).unwrap();
    assert_eq!(value(&s, "B1"), Value::Number(4.0));
}

#[test]
fn a_file_that_asks_for_a_calculation_is_calculated_on_open() {
    let s = open(&file(5.0, true), "full.xlsx");
    assert_eq!(value(&s, "B1"), Value::Number(4.0));
}
