//! Office overlay (patches/gridcraft/0120): the transliteration commands of `cmd/translit.rs`.
use gridcraft_core::{CellRef, Value};
use gridcraft_engine::Session;
use serde_json::json;
use uz_translit::{Direction, Options, convert};

const CYRL: &str = "tools.translitCyrl";
const LATN: &str = "tools.translitLatn";

fn session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s
}

fn at(s: &Session, sheet: usize, a: &str) -> Value {
    s.doc().unwrap().wb.sheet(sheet).unwrap().value(CellRef::parse(a).unwrap())
}

fn text(s: &Session, a: &str) -> String {
    at(s, 0, a).as_text().unwrap_or_default().to_string()
}

fn cyr(t: &str) -> String {
    convert(t, Direction::ToCyrillic, &Options::default())
}

#[test]
fn the_ids_are_registered() {
    let s = session();
    for id in [CYRL, LATN, "tools.translitAuto"] {
        assert!(s.commands().iter().any(|c| c.id == id), "{id}");
    }
}

#[test]
fn a_selection_converts_only_the_selection_and_a_single_cell_the_sheet() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Toshkent shahri", "salom"], ["O'zbekiston", "yangi"]]})).unwrap();
    s.execute("selection.set", json!({"range": "A1:A2"})).unwrap();
    let r = s.execute(CYRL, json!({})).unwrap();
    assert_eq!((r["scope"].as_str(), r["cells"].as_u64(), r["direction"].as_str()), (Some("selection"), Some(2), Some("cyrl")));
    assert_eq!(text(&s, "A1"), cyr("Toshkent shahri"));
    assert_eq!(text(&s, "A2"), cyr("O'zbekiston"));
    assert_eq!(text(&s, "B1"), "salom");
    // a single cell selected: the used range of the sheet
    s.execute("selection.set", json!({"range": "B2"})).unwrap();
    let r = s.execute(CYRL, json!({})).unwrap();
    assert_eq!(r["scope"], "sheet");
    assert_eq!((text(&s, "B1"), text(&s, "B2")), (cyr("salom"), cyr("yangi")));
    // and back
    s.execute(LATN, json!({})).unwrap();
    assert_eq!(text(&s, "B1"), "salom");
}

#[test]
fn auto_follows_the_dominant_script() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["салом"], ["дунё"], ["ok"]]})).unwrap();
    let r = s.execute("tools.translitAuto", json!({})).unwrap();
    assert_eq!(r["direction"], "latn");
    assert_eq!(text(&s, "A1"), "salom");
}

#[test]
fn formulas_numbers_booleans_and_their_cached_values_are_untouched() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["salom", 42, true]]})).unwrap();
    s.execute("cell.set", json!({"cell": "D1", "input": "=\"salom\""})).unwrap();
    s.execute("cell.set", json!({"cell": "E1", "input": "=A1&\"-dunyo\""})).unwrap();
    s.execute("cell.set", json!({"cell": "F1", "input": "=IF(A1=\"salom\",1,0)"})).unwrap();
    let formulas = |s: &Session| {
        ["D1", "E1", "F1"]
            .map(|a| s.doc().unwrap().wb.sheet(0).unwrap().cell(CellRef::parse(a).unwrap()).unwrap().formula.as_ref().unwrap().text().to_string())
    };
    let before = formulas(&s);
    let r = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(formulas(&s), before);
    assert_eq!(r["cells"], 1);
    assert_eq!(r["skipped"]["formula"], 3);
    assert_eq!(r["formulasWithText"], 3, "all three hold a letter literal");
    assert_eq!((at(&s, 0, "B1"), at(&s, 0, "C1")), (Value::Number(42.0), Value::Bool(true)));
    assert_eq!(text(&s, "A1"), cyr("salom"));
    // the cached result of a literal formula stays; a formula reading the cell is recalculated as usual
    assert_eq!(text(&s, "D1"), "salom");
    assert_eq!(text(&s, "E1"), format!("{}-dunyo", cyr("salom")));
    assert_eq!(at(&s, 0, "F1"), Value::Number(0.0));
}

#[test]
fn a_table_header_is_skipped() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["yil", "oy"], ["yangi", "salom"]]})).unwrap();
    s.execute("insert.table", json!({"range": "A1:B2", "header": true})).unwrap();
    s.execute("cell.set", json!({"cell": "D1", "input": "=COUNTA(Table1[yil])"})).unwrap();
    let r = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(r["skipped"]["header"], 2);
    assert_eq!((text(&s, "A1"), text(&s, "B1")), ("yil".to_string(), "oy".to_string()));
    assert_eq!(text(&s, "A2"), cyr("yangi"));
    let f = s.execute("cell.get", json!({"cell": "D1"})).unwrap();
    assert_eq!(f["formula"], "=COUNTA(Table1[yil])");
    assert_eq!(f["value"], json!(1.0));
}

#[test]
fn one_command_is_one_undo_step_and_redo_works() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["a"], ["salom"], ["dunyo"]]})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "=A2"})).unwrap();
    let before = s.doc().unwrap().wb.sheet(0).unwrap().clone();
    let steps = s.doc().unwrap().undo.len();
    s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(s.doc().unwrap().undo.len(), steps + 1);
    let after = s.doc().unwrap().wb.sheet(0).unwrap().clone();
    assert_ne!(after, before);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().wb.sheet(0).unwrap(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().wb.sheet(0).unwrap(), after);
    // nothing left to convert: no history step
    s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(s.doc().unwrap().undo.len(), steps + 1);
}

#[test]
fn workbook_scope_reaches_every_sheet_and_leaves_sheet_names() {
    let mut s = session();
    s.execute("cell.set", json!({"cell": "A1", "input": "salom"})).unwrap();
    s.execute("home.insertSheet", json!({"name": "Ma'lumot"})).unwrap();
    s.execute("cell.set", json!({"cell": "A1", "input": "dunyo"})).unwrap();
    let on_sheet = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(on_sheet["cells"], 1);
    assert_eq!(at(&s, 0, "A1"), Value::from("salom"));
    let r = s.execute(CYRL, json!({"scope": "workbook"})).unwrap();
    assert_eq!((r["cells"].as_u64(), r["scope"].as_str()), (Some(1), Some("workbook")));
    assert_eq!(at(&s, 0, "A1"), Value::from(cyr("salom")));
    assert_eq!(at(&s, 1, "A1"), Value::from(cyr("dunyo")));
    let names: Vec<String> = s.doc().unwrap().wb.sheets.iter().map(|sh| sh.name.clone()).collect();
    assert_eq!(names, ["Sheet1", "Ma'lumot"]);
}

#[test]
fn a_locked_cell_on_a_protected_sheet_is_skipped_not_an_error() {
    let mut s = session();
    s.execute("cell.set", json!({"cell": "A1", "input": "salom"})).unwrap();
    s.execute("review.protectSheet", json!({})).unwrap();
    let r = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!((r["cells"].as_u64(), r["skipped"]["locked"].as_u64()), (Some(0), Some(1)));
    assert_eq!(text(&s, "A1"), "salom");
}

/// gridcraft drops rich text on save (xlsx xml.rs:67, write.rs:378), so the check is open->save against open->convert->save.
#[test]
fn formatting_survives_and_the_xlsx_matches_open_save_apart_from_the_text() {
    use gridcraft_xlsx::{read_xlsx, write_xlsx};
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Toshkent", "o'rik"], ["salom", 7]]})).unwrap();
    s.execute("selection.set", json!({"range": "A1:B1"})).unwrap();
    s.execute("home.bold", json!({})).unwrap();
    s.execute("selection.set", json!({"range": "A2"})).unwrap();
    s.execute("home.italic", json!({})).unwrap();
    let wb = (*s.doc().unwrap().wb).clone();
    let (a, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    let (b, _) = read_xlsx(&write_xlsx(&s.doc().unwrap().wb).unwrap()).unwrap();
    let (sa, sb) = (a.sheet(0).unwrap(), b.sheet(0).unwrap());
    for addr in ["A1", "B1", "A2", "B2"] {
        let c = CellRef::parse(addr).unwrap();
        assert_eq!(a.styles.get(sa.style_id(c)), b.styles.get(sb.style_id(c)), "{addr} style");
        assert_eq!(sa.cell(c).map(|x| x.formula.is_some()), sb.cell(c).map(|x| x.formula.is_some()), "{addr}");
    }
    assert_eq!(sb.value(CellRef::parse("A1").unwrap()), Value::from(cyr("Toshkent")));
    assert_eq!(sb.value(CellRef::parse("B2").unwrap()), Value::Number(7.0));
    assert_eq!((sa.cells.len(), sa.merges.clone(), sa.cols.clone()), (sb.cells.len(), sb.merges.clone(), sb.cols.clone()));
}

const WORDS: [&str; 6] = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi", "ish haqi"];

fn big(rows: usize, cols: usize) -> Session {
    let mut s = session();
    let mut r0 = 0;
    while r0 < rows {
        let n = (rows - r0).min(5_000);
        let values: Vec<Vec<serde_json::Value>> = (0..n)
            .map(|i| (0..cols).map(|c| if c % 5 == 4 { json!(i as f64) } else { json!(WORDS[(r0 + i + c) % WORDS.len()]) }).collect())
            .collect();
        s.execute("range.setValues", json!({"range": format!("A{}", r0 + 1), "values": values})).unwrap();
        r0 += n;
    }
    s
}

#[test]
fn a_generated_sheet_converts_in_both_directions() {
    let mut s = big(2_000, 5);
    let r = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
    assert_eq!(r["cells"], 2_000 * 4);
    assert_eq!(text(&s, "A1"), cyr(WORDS[0]));
    let r = s.execute(LATN, json!({"scope": "sheet"})).unwrap();
    assert_eq!(r["direction"], "latn");
    assert_eq!(r["cells"], 2_000 * 4);
}

/// Release timing for docs/phase2/perf.md: `cargo test --release -p gridcraft-engine --test translit -- --ignored --nocapture`.
#[test]
#[ignore]
fn timing_on_a_large_sheet() {
    for (rows, cols) in [(50_000, 5), (200_000, 5)] {
        let mut s = big(rows, cols);
        let t = std::time::Instant::now();
        let r = s.execute(CYRL, json!({"scope": "sheet"})).unwrap();
        let total = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        s.execute("edit.undo", json!({})).unwrap();
        println!(
            "{rows}x{cols}: {} cells, total {total:.0} ms, convert {:.0} ms, apply+recalc {:.0} ms, undo {:.1} ms",
            r["cells"],
            r["ms"]["convert"],
            r["ms"]["apply"],
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
