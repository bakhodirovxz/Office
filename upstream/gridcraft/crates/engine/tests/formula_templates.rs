//! Formulas that share a tree (runs of formulas that are the same in R1C1 terms, see
//! `gridcraft_model::Formula`) behave exactly like formulas that each hold their own.
//!
//! Each test runs the same commands on two copies of a workbook: one as the XLSX reader gives it
//! (filled-down columns share trees), one with every formula held on its own (as before trees were
//! shared). After every command both must have the same cells: values, formula text and legacy
//! array ranges. Saving and reloading must agree too.

use std::collections::HashSet;
use std::sync::Arc;

use gridcraft_core::{CellRef, Value};
use gridcraft_engine::model::{Cell, Formula, Sheet, Workbook};
use gridcraft_engine::{DocState, Session};
use serde_json::{Value as Json, json};

fn at(a: &str) -> CellRef {
    CellRef::parse(a).unwrap()
}

/// `Lookup` (keys and factors), `Data` (values and filled-down formulas of several kinds, a
/// filled-right row and a legacy array), `Summary` (totals and a filled-down cross-sheet column).
fn source() -> Workbook {
    let mut wb = Workbook::new();
    let mut lookup = Sheet::new("Lookup");
    for i in 0..5u32 {
        lookup.set_cell(CellRef::new(i, 0), Cell::value(Value::text(format!("K{i}"))));
        lookup.set_cell(CellRef::new(i, 1), Cell::value(Value::number(1.0 + f64::from(i) / 4.0)));
    }
    let mut data = Sheet::new("Data");
    for r in 1..=30u32 {
        let n = r + 1;
        let x = f64::from((r * 37) % 23);
        data.set_cell(CellRef::new(r, 0), Cell::value(Value::text(format!("K{}", r % 5))));
        data.set_cell(CellRef::new(r, 2), Cell::value(Value::number(x)));
        data.set_cell(CellRef::new(r, 3), Cell::value(Value::number(f64::from(r % 7) + 0.5)));
        for (c, f) in [
            (4, format!("IF(C{n}>5,C{n}*D{n},0)")),
            (5, format!("VLOOKUP(A{n},Lookup!$A$1:$B$5,2,FALSE)")),
            (6, format!("SUM($C$2:C{n})")),
            (7, format!("E{n}+F{n}*$D$2")),
            (8, format!("IF(ROW()>3,I{}+1,0)", n - 1)),
        ] {
            data.set_cell(CellRef::new(r, c), Cell::formula(Formula::new(&f)));
        }
    }
    // Filled right.
    for c in 10..16u32 {
        let col = gridcraft_core::col_to_letters(c - 1);
        data.set_cell(CellRef::new(1, c), Cell::formula(Formula::new(&format!("{col}2*2"))));
    }
    let mut arr = Formula::new("C2:C4*2");
    arr.set_array(Some(gridcraft_core::RangeRef::parse("P2:P4").unwrap()));
    data.set_cell(at("P2"), Cell::formula(arr));
    let mut summary = Sheet::new("Summary");
    summary.set_cell(at("A1"), Cell::formula(Formula::new("SUM(Data!E2:E31)")));
    summary.set_cell(at("A2"), Cell::formula(Formula::new("SUMIFS(Data!C2:C31,Data!A2:A31,\"K1\")")));
    for r in 3..=20u32 {
        summary.set_cell(CellRef::new(r, 1), Cell::formula(Formula::new(&format!("Data!C{}+Data!G{}", r - 1, r - 1))));
    }
    wb.sheets = vec![Arc::new(lookup), Arc::new(data), Arc::new(summary)];
    wb.active_sheet = 1;
    // Through the file: the reader finds the runs.
    let bytes = gridcraft_xlsx::write_xlsx(&wb).unwrap();
    gridcraft_xlsx::read_xlsx(&bytes).unwrap().0
}

/// The same workbook with every formula holding its own tree and text.
fn unshared(wb: &Workbook) -> Workbook {
    let mut wb = wb.clone();
    for sheet in wb.sheets.iter_mut() {
        let sheet = Arc::make_mut(sheet);
        let cells: Vec<(CellRef, Arc<Formula>)> = sheet.cells.iter().filter_map(|(c, x)| x.formula.clone().map(|f| (c, f))).collect();
        for (c, f) in cells {
            let mut own = match f.expr() {
                Some(e) => Formula::from_expr(e),
                None => (*f).clone(),
            };
            own.set_array(f.array());
            assert!(own.template().is_none_or(|(_, dr, dc)| (dr, dc) == (0, 0)));
            if let Some(cell) = sheet.cells.get_mut(c) {
                cell.formula = Some(Arc::new(own));
            }
        }
    }
    wb
}

/// Formula cells holding a shifted copy of a shared tree, and the number of distinct trees.
fn sharing(wb: &Workbook) -> (usize, usize) {
    let mut shifted = 0;
    let mut trees = HashSet::new();
    for sheet in &wb.sheets {
        for (_, cell) in sheet.cells.iter() {
            if let Some((base, dr, dc)) = cell.formula.as_ref().and_then(|f| f.template()) {
                trees.insert(Arc::as_ptr(base) as usize);
                if (dr, dc) != (0, 0) {
                    shifted += 1;
                }
            }
        }
    }
    (shifted, trees.len())
}

fn session(wb: Workbook) -> Session {
    let mut s = Session::new();
    let i = s.add_document(DocState::new(wb, None, "book".into()));
    s.set_active(i);
    s
}

/// Every cell of every sheet: value, formula text, legacy array range.
fn cells(wb: &Workbook) -> Vec<(String, CellRef, Value, Option<String>, Option<gridcraft_core::RangeRef>)> {
    let mut v = Vec::new();
    for sheet in &wb.sheets {
        for (c, cell) in sheet.cells.iter() {
            let formula = cell.formula.as_ref();
            v.push((sheet.name.clone(), c, cell.value.clone(), formula.map(|f| f.text().into_owned()), formula.and_then(|f| f.array())));
        }
    }
    v
}

fn assert_same(a: &Workbook, b: &Workbook, step: &str) {
    let (x, y) = (cells(a), cells(b));
    if x != y {
        let first = x.iter().zip(&y).find(|(p, q)| p != q);
        panic!("after {step}: {} vs {} cells, first difference {first:?}", x.len(), y.len());
    }
    assert_eq!(a, b, "after {step}: workbooks compare equal");
}

/// Runs `steps` on a sharing and an unshared copy of `source()`, comparing after each; returns the
/// sharing session.
fn twin(steps: &[(&str, Json)]) -> Session {
    let wb = source();
    let (shifted, trees) = sharing(&wb);
    assert!(shifted > 150, "the reader shares the filled-down runs ({shifted} shifted copies, {trees} trees)");
    let plain = unshared(&wb);
    assert_eq!(sharing(&plain).0, 0);
    let mut a = session(wb);
    let mut b = session(plain);
    assert_same(&a.doc().unwrap().wb, &b.doc().unwrap().wb, "opening");
    for (id, p) in steps {
        let ra = a.execute(id, p.clone()).map_err(|e| e.to_string());
        let rb = b.execute(id, p.clone()).map_err(|e| e.to_string());
        assert_eq!(ra, rb, "{id} {p}");
        assert_same(&a.doc().unwrap().wb, &b.doc().unwrap().wb, &format!("{id} {p}"));
    }
    // Saved and reopened (as shared formulas where they share a tree): the same again.
    let wa = gridcraft_xlsx::write_xlsx(&a.doc().unwrap().wb).unwrap();
    let wb_ = gridcraft_xlsx::write_xlsx(&b.doc().unwrap().wb).unwrap();
    let ra = session(gridcraft_xlsx::read_xlsx(&wa).unwrap().0);
    let rb = session(gridcraft_xlsx::read_xlsx(&wb_).unwrap().0);
    assert_same(&ra.doc().unwrap().wb, &rb.doc().unwrap().wb, "saving and reopening");
    assert_same(&ra.doc().unwrap().wb, &a.doc().unwrap().wb, "saving and reopening (against the open workbook)");
    a
}

#[test]
fn reading_shares_runs_and_evaluates_the_same() {
    let s = twin(&[]);
    let wb = &s.doc().unwrap().wb;
    let data = wb.sheet(1).unwrap();
    assert_eq!(data.cell(at("E31")).unwrap().formula.as_ref().unwrap().text(), "IF(C31>5,C31*D31,0)");
    assert_eq!(data.cell(at("P2")).unwrap().formula.as_ref().unwrap().array(), gridcraft_core::RangeRef::parse("P2:P4"));
    // A run of 30 cells is one tree; the filled-right row one more.
    let (shifted, trees) = sharing(wb);
    assert!(trees < 20, "{trees} trees for {shifted} shifted copies");
}

#[test]
fn editing_a_cell_detaches_it_from_its_run() {
    let s = twin(&[("cell.set", json!({"cell": "E5", "input": "=C5*100"})), ("cell.set", json!({"cell": "C6", "input": "100"}))]);
    let wb = &s.doc().unwrap().wb;
    let data = wb.sheet(1).unwrap();
    let e5 = data.cell(at("E5")).unwrap().formula.clone().unwrap();
    let e6 = data.cell(at("E6")).unwrap().formula.clone().unwrap();
    assert_eq!(e5.text(), "C5*100");
    assert!(!Arc::ptr_eq(e5.template().unwrap().0, e6.template().unwrap().0));
    assert_eq!(data.value(at("E6")), Value::number(550.0));
}

#[test]
fn inserting_and_deleting_rows_and_columns() {
    let s = twin(&[
        ("home.insertRows", json!({"rows": "5:6"})),
        ("home.deleteRows", json!({"rows": "10:10"})),
        ("home.insertColumns", json!({"cols": "B:B"})),
        ("home.deleteColumns", json!({"cols": "D:D"})),
        ("home.deleteRows", json!({"rows": "2:2"})),
        ("edit.undo", json!({})),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
    ]);
    // The rewritten runs still share trees.
    let (shifted, trees) = sharing(&s.doc().unwrap().wb);
    assert!(shifted > 150 && trees < 40, "{shifted} shifted copies, {trees} trees");
}

#[test]
fn inserting_and_deleting_cells() {
    twin(&[
        ("home.insertCells", json!({"range": "E7:E8", "shift": "down"})),
        ("home.deleteCells", json!({"range": "F3:G4", "shift": "up"})),
        ("home.insertCells", json!({"range": "C10:D10", "shift": "right"})),
        ("home.deleteCells", json!({"range": "B12:B12", "shift": "left"})),
    ]);
}

#[test]
fn sorting() {
    twin(&[("data.sortDescending", json!({"range": "A2:I31", "column": "C"})), ("data.sortAscending", json!({"range": "A2:I31", "column": "A"}))]);
}

#[test]
fn filling() {
    let s = twin(&[
        ("edit.fillDown", json!({"range": "E2:I45"})),
        ("edit.fillRight", json!({"range": "K2:T2"})),
        ("edit.fillUp", json!({"range": "H20:H31"})),
        ("range.fill", json!({"range": "R5:R40", "input": "=C5+D5"})),
    ]);
    let wb = &s.doc().unwrap().wb;
    let data = wb.sheet(1).unwrap();
    // Filled cells share the source's tree.
    let e2 = data.cell(at("E2")).unwrap().formula.clone().unwrap();
    let e45 = data.cell(at("E45")).unwrap().formula.clone().unwrap();
    assert_eq!(e45.text(), "IF(C45>5,C45*D45,0)");
    assert!(Arc::ptr_eq(e2.template().unwrap().0, e45.template().unwrap().0));
}

#[test]
fn cut_copy_and_paste() {
    twin(&[
        ("edit.copy", json!({"range": "E2:I6"})),
        ("edit.paste", json!({"at": "E40"})),
        ("edit.cut", json!({"range": "E10:H14"})),
        ("edit.paste", json!({"at": "M20"})),
        ("edit.copy", json!({"range": "E2:E3"})),
        ("edit.paste", json!({"at": "A1"})),
        ("sheet.activate", json!({"sheet": "Summary"})),
        ("edit.copy", json!({"range": "B4:B8"})),
        ("edit.paste", json!({"at": "D4"})),
    ]);
}

#[test]
fn cutting_to_another_sheet() {
    twin(&[("edit.cut", json!({"range": "E2:G9"})), ("sheet.activate", json!({"sheet": "Summary"})), ("edit.paste", json!({"at": "F2"}))]);
}

#[test]
fn renaming_and_deleting_sheets() {
    let s = twin(&[
        ("sheet.rename", json!({"sheet": "Data", "name": "Facts 2"})),
        ("sheet.rename", json!({"sheet": "Lookup", "name": "Keys"})),
        ("home.deleteSheet", json!({"sheet": "Keys"})),
    ]);
    let wb = &s.doc().unwrap().wb;
    let summary = wb.sheet(wb.sheet_index("Summary").unwrap()).unwrap();
    assert_eq!(summary.cell(at("B5")).unwrap().formula.as_ref().unwrap().text(), "'Facts 2'!C3+'Facts 2'!G3");
    let facts = wb.sheet(wb.sheet_index("Facts 2").unwrap()).unwrap();
    assert_eq!(facts.cell(at("F9")).unwrap().formula.as_ref().unwrap().text(), "VLOOKUP(A9,#REF!,2,FALSE)");
    // Renamed runs share trees again.
    let (shifted, _) = sharing(wb);
    assert!(shifted > 150, "{shifted}");
}

#[test]
fn a_run_reaching_off_the_sheet_by_copy() {
    // `I2` reads `I1`; copied up one row it reads off the sheet: `#REF!`, as with plain formulas.
    twin(&[
        ("edit.copy", json!({"range": "I2:I3"})),
        ("edit.paste", json!({"at": "I1"})),
        ("edit.copy", json!({"range": "H2"})),
        ("edit.paste", json!({"at": "A1"})),
    ]);
}
