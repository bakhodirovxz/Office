//! Formulas sharing a tree: Excel shared formulas and filled-down runs are read as shifted copies
//! of one tree, and runs of them are written back as shared formulas.

use std::io::Read;
use std::sync::Arc;

use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Formula, Workbook};

use super::minimal;
use crate::{read_xlsx, write_xlsx};

fn at(a: &str) -> CellRef {
    CellRef::parse(a).unwrap()
}

fn text(wb: &Workbook, a: &str) -> String {
    wb.sheet(0).unwrap().cell(at(a)).unwrap().formula.as_ref().unwrap().text().into_owned()
}

fn tree(wb: &Workbook, a: &str) -> Arc<gridcraft_formula::Expr> {
    wb.sheet(0).unwrap().cell(at(a)).unwrap().formula.as_ref().unwrap().template().unwrap().0.clone()
}

fn sheet_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn excel_shared_formulas_share_the_masters_tree() {
    let body = r#"<sheetData>
        <row r="1"><c r="A1"><v>1</v></c><c r="B1"><f t="shared" ref="B1:C3" si="0">A1*2+$A$1</f></c><c r="C1"><f t="shared" si="0"/></c></row>
        <row r="2"><c r="A2"><v>2</v></c><c r="B2"><f t="shared" si="0"/></c></row>
        <row r="3"><c r="A3"><v>3</v></c><c r="B3"><f t="shared" si="0"/></c><c r="C3"><f t="shared" si="0"/></c></row>
        </sheetData>"#;
    let (wb, _) = read_xlsx(&minimal(body, &[], "", "")).unwrap();
    assert_eq!(text(&wb, "B1"), "A1*2+$A$1");
    assert_eq!(text(&wb, "C1"), "B1*2+$A$1");
    assert_eq!(text(&wb, "B3"), "A3*2+$A$1");
    assert_eq!(text(&wb, "C3"), "B3*2+$A$1");
    for a in ["C1", "B2", "B3", "C3"] {
        assert!(Arc::ptr_eq(&tree(&wb, "B1"), &tree(&wb, a)), "{a}");
    }
}

#[test]
fn filled_down_formulas_share_a_tree() {
    let mut rows = String::new();
    for r in 1..=6 {
        // Column B is filled down; C alternates between two formulas; D is the same text (all
        // absolute) in every row.
        let c = if r % 2 == 0 { format!("A{r}+1") } else { format!("A{r}-1") };
        let cells = format!(r#"<c r="B{r}"><f>A{r}*2</f></c><c r="C{r}"><f>{c}</f></c><c r="D{r}"><f>SUM($A$1:$A$6)</f></c>"#);
        rows.push_str(&format!(r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c>{cells}</row>"#));
    }
    let (wb, _) = read_xlsx(&minimal(&format!("<sheetData>{rows}</sheetData>"), &[], "", "")).unwrap();
    for r in 2..=6 {
        assert!(Arc::ptr_eq(&tree(&wb, "B1"), &tree(&wb, &format!("B{r}"))));
        assert!(Arc::ptr_eq(&tree(&wb, "D1"), &tree(&wb, &format!("D{r}"))));
        assert_eq!(text(&wb, &format!("B{r}")), format!("B{r}").replace('B', "A") + "*2");
    }
    // Different formulas never share.
    assert!(!Arc::ptr_eq(&tree(&wb, "C1"), &tree(&wb, "C2")));
    assert_eq!(text(&wb, "C2"), "A2+1");
    assert_eq!(text(&wb, "C3"), "A3-1");
}

#[test]
fn runs_are_written_as_shared_formulas() {
    let mut wb = Workbook::new();
    let master = Formula::new("A1*2+$A$1");
    {
        let sh = wb.sheet_mut(0).unwrap();
        for r in 0..12u32 {
            sh.cells.set(CellRef::new(r, 0), Cell::value(Value::Number(f64::from(r))));
        }
        for r in 0..6u32 {
            sh.cells.set(CellRef::new(r, 1), Cell::formula(master.copied(i64::from(r), 0).unwrap()));
        }
        // B7 holds the copy made for B10, moved up (as Delete Cells moves it): the same tree, but
        // not the master moved down to B7, so not in the group.
        sh.cells.set(at("B7"), Cell::formula(master.copied(9, 0).unwrap()));
        sh.cells.set(at("B8"), Cell::formula(master.copied(7, 0).unwrap()));
        sh.cells.set(at("B9"), Cell::formula(master.copied(8, 0).unwrap()));
        // Excel can't read a shared formula with a sheet-qualified reference: written per cell.
        let other = Formula::new("Sheet1!A1+1");
        for r in 0..3u32 {
            sh.cells.set(CellRef::new(r, 3), Cell::formula(other.copied(i64::from(r), 0).unwrap()));
        }
        // A copy reading off the sheet is written on its own.
        sh.cells.set(at("C2"), Cell::formula(master.copied(-1, 1).unwrap()));
        sh.cells.set(at("C3"), Cell::formula(master.copied(1, 1).unwrap()));
        // A legacy array formula is written on its own.
        let mut arr = master.copied(10, 0).unwrap();
        arr.set_array(Some(gridcraft_core::RangeRef::parse("B11:B11").unwrap()));
        sh.cells.set(at("B11"), Cell::formula(arr));
    }
    let bytes = write_xlsx(&wb).unwrap();
    let xml = sheet_xml(&bytes);
    assert!(xml.contains(r#"<c r="B1"><f t="shared" ref="B1:B6" si="0">A1*2+$A$1</f>"#), "{xml}");
    assert!(xml.contains(r#"<c r="B2"><f t="shared" si="0"/>"#), "{xml}");
    assert!(xml.contains(r#"<c r="B7"><f>A10*2+$A$1</f>"#), "{xml}");
    assert!(xml.contains(r#"<c r="B8"><f t="shared" ref="B8:B9" si="1">A8*2+$A$1</f>"#), "{xml}");
    assert!(xml.contains(r#"<c r="C2"><f>#REF!*2+$A$1</f>"#), "{xml}");
    assert!(xml.contains(r#"<f t="array" ref="B11">A11*2+$A$1</f>"#), "{xml}");
    assert!(xml.contains(r#"<c r="D2"><f>Sheet1!A2+1</f>"#), "{xml}");
    let (back, _) = read_xlsx(&bytes).unwrap();
    for (c, cell) in wb.sheet(0).unwrap().cells.iter() {
        let got = back.sheet(0).unwrap().cell(c).unwrap();
        let formula = |x: &Cell| x.formula.as_ref().map(|f| (f.text().into_owned(), f.array()));
        assert_eq!(formula(got), formula(cell), "{}", c.a1());
    }
    assert!(Arc::ptr_eq(&tree(&back, "B1"), &tree(&back, "B6")));
}

#[test]
fn inline_strings_with_the_same_short_text_share_one_string() {
    let long = "x".repeat(65);
    let mut rows = String::new();
    for r in 1..=4 {
        let code = if r % 2 == 0 { "K07" } else { "Приход" };
        rows.push_str(&format!(
            r#"<row r="{r}"><c r="A{r}" t="inlineStr"><is><t>{code}</t></is></c><c r="B{r}" t="inlineStr"><is><t>{long}</t></is></c><c r="C{r}" t="str"><f>A{r}</f><v>{code}</v></c></row>"#
        ));
    }
    let (wb, _) = read_xlsx(&minimal(&format!("<sheetData>{rows}</sheetData>"), &[], "", "")).unwrap();
    let s = wb.sheet(0).unwrap();
    let text = |a: &str| match &s.cell(at(a)).unwrap().value {
        Value::Text(t) => t.clone(),
        v => panic!("{a}: {v:?}"),
    };
    assert_eq!((&*text("A1"), &*text("A2"), &*text("C4")), ("Приход", "K07", "K07"));
    assert!(Arc::ptr_eq(&text("A1"), &text("A3")) && Arc::ptr_eq(&text("A2"), &text("A4")) && Arc::ptr_eq(&text("A2"), &text("C2")));
    assert!(!Arc::ptr_eq(&text("A1"), &text("A2")));
    // Long texts are not pooled.
    assert_eq!(&*text("B1"), long);
    assert!(!Arc::ptr_eq(&text("B1"), &text("B2")));
}

#[test]
fn a_workbook_calculated_on_save_is_reported_as_calculated() {
    let body = r#"<sheetData><row r="1"><c r="A1"><v>2</v></c><c r="B1"><f>A1*2</f><v>4</v></c></row></sheetData>"#;
    let spill = r#"<sheetData><row r="1"><c r="A1" cm="1"><f t="array" ref="A1:A2">SEQUENCE(2)</f><v>1</v></c></row><row r="2"><c r="A2"><v>2</v></c></row></sheetData>"#;
    for (sheet, wb_extra, calculated) in [
        (body, r#"<calcPr calcId="191029"/>"#, true),
        (body, r#"<calcPr calcId="124519" fullCalcOnLoad="1"/>"#, false),
        // No calcPr: no word on how the values were made.
        (body, "", false),
        // Spilled values are left out on read, so the file's values are incomplete.
        (spill, r#"<calcPr calcId="191029"/>"#, false),
    ] {
        let (wb, report) = read_xlsx(&minimal(sheet, &[], "", wb_extra)).unwrap();
        assert_eq!(report.calculated, calculated, "{wb_extra} {sheet}");
        assert!(wb.sheet(0).unwrap().cell(at("A1")).is_some());
    }
    // gridcraft's own files: calculated when every formula has its value.
    let mut wb = Workbook::new();
    let sh = wb.sheet_mut(0).unwrap();
    sh.cells.set(at("A1"), Cell::value(Value::Number(2.0)));
    sh.cells.set(at("B1"), Cell { value: Value::Number(4.0), ..Cell::formula(Formula::new("=A1*2")) });
    assert!(read_xlsx(&write_xlsx(&wb).unwrap()).unwrap().1.calculated);
    wb.sheet_mut(0).unwrap().cells.set(at("C1"), Cell::formula(Formula::new("=B1+1")));
    assert!(!read_xlsx(&write_xlsx(&wb).unwrap()).unwrap().1.calculated);
}

#[test]
fn reading_from_a_reader_gives_what_reading_the_bytes_gives() {
    let mut wb = Workbook::new();
    let sh = wb.sheet_mut(0).unwrap();
    for r in 0..300u32 {
        sh.cells.set(CellRef::new(r, 0), Cell::value(Value::Number(f64::from(r))));
        sh.cells.set(CellRef::new(r, 1), Cell::value(Value::from(if r % 3 == 0 { "K07" } else { "Приход" })));
        sh.cells.set(CellRef::new(r, 2), Cell { value: Value::Number(f64::from(r) * 2.0), ..Cell::formula(Formula::new(&format!("=A{}*2", r + 1))) });
    }
    let excel = r#"<sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><f t="shared" ref="B1:B3" si="0">A1*2</f><v>2</v></c></row><row r="2"><c r="B2"><f t="shared" si="0"/><v>0</v></c></row></sheetData>"#;
    for bytes in [write_xlsx(&wb).unwrap(), minimal(excel, &[], "", r#"<calcPr calcId="191029"/>"#)] {
        let from_bytes = read_xlsx(&bytes).unwrap();
        let from_reader = crate::read_xlsx_reader(std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(from_reader.0, from_bytes.0);
        assert_eq!(from_reader.1, from_bytes.1);
    }
}
