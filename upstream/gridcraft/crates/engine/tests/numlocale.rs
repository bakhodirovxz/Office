//! Office overlay (patches/gridcraft/0028, 0029): the number locale in the engine. What a person
//! sees (cell text, CSV, PDF, copy) follows `Session.prefs.num_locale`; what is stored (xlsx
//! parts, values, format codes) never does.
use std::io::Read;

use gridcraft_core::parse::to_invariant;
use gridcraft_core::{CellRef, DateSystem, NumLocale, Value};
use gridcraft_engine::{Session, io};
use serde_json::json;

const NB: &str = "\u{a0}";
const LOCALES: [NumLocale; 3] = [NumLocale::Ru, NumLocale::UzLatn, NumLocale::UzCyrl];

fn session(loc: NumLocale) -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.prefs.num_locale = loc;
    s
}

fn set(s: &mut Session, cell: &str, input: &str) {
    s.execute("cell.set", json!({"cell": cell, "input": input})).unwrap();
}

fn fmt(s: &mut Session, range: &str, code: &str) {
    s.execute("home.formatCells", json!({"range": range, "style": {"num_fmt": code}})).unwrap();
}

fn value(s: &Session, a: &str) -> Value {
    s.doc().unwrap().wb.sheet(0).unwrap().value(CellRef::parse(a).unwrap())
}

fn bytes(s: &mut Session, format: &str) -> Vec<u8> {
    let r = s.execute("file.saveBytes", json!({"format": format})).unwrap();
    io::base64_decode(r["base64"].as_str().unwrap()).unwrap()
}

/// Two columns: 3500000.5 in General, 3500000.5 as `#,##0.00`, and a date, a text.
fn book(loc: NumLocale) -> Session {
    let mut s = session(loc);
    set(&mut s, "A1", "3500000.5");
    set(&mut s, "B1", "3500000.5");
    fmt(&mut s, "B1", "#,##0.00");
    set(&mut s, "A2", "10/12/2026");
    set(&mut s, "B2", "x;y");
    s
}

#[test]
fn csv_save_per_locale() {
    let en = bytes(&mut book(NumLocale::EnUs), "csv");
    assert_eq!(String::from_utf8(en).unwrap(), "3500000.5,\"3,500,000.50\"\r\n10/12/2026,x;y\r\n");
    for loc in LOCALES {
        let got = String::from_utf8(bytes(&mut book(loc), "csv")).unwrap();
        // `;` between fields, `,` decimal, U+00A0 groups, the date in the short form, `;` in a field quoted
        assert_eq!(got, format!("3500000,5;3{NB}500{NB}000,50\r\n12.10.2026;\"x;y\"\r\n"), "{loc:?}");
    }
    // tab text keeps the tab and follows the locale
    let tsv = String::from_utf8(bytes(&mut book(NumLocale::Ru), "tsv")).unwrap();
    assert_eq!(tsv, format!("3500000,5\t3{NB}500{NB}000,50\r\n12.10.2026\tx;y\r\n"));
}

#[test]
fn a_comma_delimited_file_is_always_en_us() {
    // a CSV that came in with `,` keeps it and stays en-US text, whatever the session locale
    let csv = b"a,b\r\n1.5,\"2,500.25\"\r\n";
    let mut s = session(NumLocale::Ru);
    s.execute("file.open", json!({"name": "in.csv", "base64": io::base64_encode(csv)})).unwrap();
    assert_eq!(value(&s, "A2"), Value::Number(1.5));
    assert_eq!(value(&s, "B2"), Value::Number(2500.25));
    assert_eq!(String::from_utf8(bytes(&mut s, "csv")).unwrap(), "a,b\r\n1.5,\"2,500.25\"\r\n");
}

#[test]
fn csv_save_and_open_round_trips_in_a_locale() {
    for loc in LOCALES {
        let saved = bytes(&mut book(loc), "csv");
        let mut s = session(loc);
        s.execute("file.open", json!({"name": "back.csv", "base64": io::base64_encode(&saved)})).unwrap();
        assert_eq!(value(&s, "A1"), Value::Number(3_500_000.5), "{loc:?}");
        assert_eq!(value(&s, "B1"), Value::Number(3_500_000.5), "{loc:?}");
        // 10/12/2026 = 12 October = serial 46307
        assert_eq!(value(&s, "A2"), Value::Number(46307.0), "{loc:?}");
        assert_eq!(value(&s, "B2"), Value::text("x;y"));
        // and saving it again gives the same bytes
        assert_eq!(bytes(&mut s, "csv"), saved, "{loc:?}");
    }
    // an en-US session reads a `;` file written by Excel in en (decimal point)
    let mut s = session(NumLocale::EnUs);
    s.execute("file.open", json!({"name": "x.csv", "base64": io::base64_encode(b"1.5;2\r\n")})).unwrap();
    assert_eq!(value(&s, "A1"), Value::Number(1.5));
}

#[test]
fn sheet_read_formatted_and_copy_follow_the_locale() {
    let mut en = book(NumLocale::EnUs);
    let r = en.execute("sheet.read", json!({"range": "A1:B2", "formatted": true})).unwrap();
    assert_eq!(r["values"], json!([["3500000.5", "3,500,000.50"], ["10/12/2026", "x;y"]]));
    for loc in LOCALES {
        let mut s = book(loc);
        let r = s.execute("sheet.read", json!({"range": "A1:B2", "formatted": true})).unwrap();
        assert_eq!(r["values"], json!([["3500000,5", format!("3{NB}500{NB}000,50")], ["12.10.2026", "x;y"]]), "{loc:?}");
        // the plain value is never localized
        let v = s.execute("sheet.read", json!({"range": "A1:A2"})).unwrap();
        assert_eq!(v["values"], json!([[3_500_000.5], [46307.0]]));
        let c = s.execute("edit.copy", json!({"range": "A1:B1"})).unwrap();
        assert_eq!(c["text"], format!("3500000,5\t3{NB}500{NB}000,50\n"), "{loc:?}");
    }
}

#[test]
fn the_editor_text_of_a_number_or_date_types_back_to_itself() {
    for loc in LOCALES {
        let mut s = book(loc);
        s.execute("cell.set", json!({"cell": "C1", "input": &to_invariant("12 окт 2026", DateSystem::D1900, loc)})).unwrap();
        let d = s.doc().unwrap();
        let sh = d.wb.sheet(0).unwrap();
        for a in ["A1", "B1", "A2", "C1"] {
            let c = CellRef::parse(a).unwrap();
            let shown = gridcraft_engine::display::edit_text_in(&d.wb, sh, c, loc);
            let back = to_invariant(&shown, DateSystem::D1900, loc);
            let p = gridcraft_core::parse::parse_input(&back, DateSystem::D1900);
            assert_eq!(p.value, sh.value(c), "{a} {shown:?} in {loc:?}");
        }
        assert_eq!(gridcraft_engine::display::edit_text_in(&d.wb, sh, CellRef::parse("A2").unwrap(), loc), "12.10.2026");
        assert_eq!(gridcraft_engine::display::edit_text_in(&d.wb, sh, CellRef::parse("A1").unwrap(), loc), "3500000,5");
    }
}

#[test]
fn pasted_text_is_typed_in_the_session_locale() {
    for loc in LOCALES {
        let mut s = session(loc);
        s.execute("edit.paste", json!({"text": "12.10.2026\t1 234,5\t1.234\r\n", "at": "A1"})).unwrap();
        assert_eq!(value(&s, "A1"), Value::Number(46307.0), "{loc:?}");
        assert_eq!(value(&s, "B1"), Value::Number(1234.5));
        assert_eq!(value(&s, "C1"), Value::text("1.234"));
    }
}

/// The zip parts of an xlsx except `docProps/*` (timestamps and the app name differ per save).
fn parts(xlsx: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(xlsx)).unwrap();
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        if name.starts_with("docProps/") {
            continue;
        }
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        out.push((name, b));
    }
    out
}

/// The same entries, once as typed in a locale and once in en-US.
const SCRIPT: [(&str, &str, &str); 12] = [
    ("A1", "12.10.2026", "10/12/2026"),
    ("A2", "3,5", "3.5"),
    ("A3", "1 234,5", "1,234.5"),
    ("A4", "25,5%", "25.5%"),
    ("A5", "1.234", "'1.234"),
    ("A6", "12.10.2026 15:00", "10/12/2026 15:00"),
    ("A7", "-1 234,50", "-1,234.50"),
    ("A8", "hello", "hello"),
    ("A9", "2026-10-12", "2026-10-12"),
    ("B1", "=SUM(A2:A4)", "=SUM(A2:A4)"),
    ("B2", "10:30", "10:30"),
    ("B3", "TRUE", "TRUE"),
];

#[test]
fn xlsx_parts_are_byte_equal_across_locales() {
    let mut en = session(NumLocale::EnUs);
    for (cell, _, invariant) in SCRIPT {
        set(&mut en, cell, invariant);
    }
    let want = parts(&bytes(&mut en, "xlsx"));
    assert!(want.len() > 5);
    for loc in LOCALES {
        let mut s = session(loc);
        for (cell, typed, _) in SCRIPT {
            set(&mut s, cell, &to_invariant(typed, DateSystem::D1900, loc));
        }
        // the locale is also set while saving: it must not reach the file
        let got = parts(&bytes(&mut s, "xlsx"));
        assert_eq!(got.iter().map(|p| &p.0).collect::<Vec<_>>(), want.iter().map(|p| &p.0).collect::<Vec<_>>(), "{loc:?}");
        for ((name, a), (_, b)) in got.iter().zip(&want) {
            assert!(a == b, "{name} differs in {loc:?}");
        }
        assert_eq!(value(&s, "A1"), Value::Number(46307.0));
        assert_eq!(value(&s, "A5"), Value::text("1.234"));
    }
}

#[test]
fn builtin_date_ids_show_the_locale_forms_and_the_stored_code_is_unchanged() {
    // measured on ru-RU Excel: ids 14, 15, 16, 17 and 22 of 12 October 2026 15:04:05
    let want = ["12.10.2026", "12.окт.26", "12.окт", "окт.26", "12.10.2026 15:04"];
    let codes = ["m/d/yyyy", "d-mmm-yy", "d-mmm", "mmm-yy", "m/d/yyyy h:mm"];
    let mut ru = session(NumLocale::Ru);
    let mut en = session(NumLocale::EnUs);
    for (i, code) in codes.iter().enumerate() {
        for s in [&mut ru, &mut en] {
            set(s, &format!("A{}", i + 1), "46307.62783564815");
            fmt(s, &format!("A{}", i + 1), code);
        }
    }
    let r = ru.execute("sheet.read", json!({"range": "A1:A5", "formatted": true})).unwrap();
    assert_eq!(r["values"], json!(want.map(|w| [w])));
    let e = en.execute("sheet.read", json!({"range": "A1:A5", "formatted": true})).unwrap();
    assert_eq!(e["values"], json!([["10/12/2026"], ["12-Oct-26"], ["12-Oct"], ["Oct-26"], ["10/12/2026 15:04"]]));
    // the codes are what the file stores
    assert_eq!(
        ru.doc().unwrap().wb.styles.get(ru.doc().unwrap().wb.sheet(0).unwrap().style_id(CellRef::parse("A2").unwrap())).num_fmt.as_str(),
        "d-mmm-yy"
    );
}
