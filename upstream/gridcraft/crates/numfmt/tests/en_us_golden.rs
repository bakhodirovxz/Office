//! The en-US display must not change with the locale work (office patch 0024): every built-in
//! format id and a set of custom codes, applied to 25 values, equal the output recorded from the
//! tree before the locale parameter existed (`en_us_golden.tsv`).
//!
//! `GOLD_OUT=<path> cargo test --test en_us_golden` rewrites the file (only meant to be run
//! against a tree whose en-US output is known to be right).
#![allow(clippy::approx_constant)]
use gridcraft_core::{DateSystem, NumLocale, Value};
use gridcraft_numfmt::{NumberFormat, builtin_format, format_value_in};

const VALUES: [f64; 25] = [
    0.0,
    1.0,
    -1.0,
    0.5,
    -0.5,
    1234.5678,
    -1234.5678,
    1e-5,
    1e15,
    12_345_678_901.0,
    3_500_000.5,
    0.255,
    0.123_456_7,
    45_000.75,
    46_307.0,
    0.999_999,
    59.0,
    60.0,
    61.0,
    44_927.5,
    -3.14159,
    1e10,
    123_456_789_012_345.0,
    0.041_666_67,
    2.5,
];

const CUSTOM: [&str; 24] = [
    "yyyy-mm-dd",
    "dd.mm.yyyy",
    "d mmmm yyyy",
    "dddd, d mmm yyyy",
    "mmm d, yyyy hh:mm:ss",
    "[h]:mm:ss",
    "mm:ss.0",
    "h:mm:ss.000 AM/PM",
    "#,##0.0",
    "0.0",
    "0.00%",
    "#,##0.00;[Red]-#,##0.00",
    "#,##0 \"so'm\"",
    "[$-419]d mmmm yyyy",
    "[$-443]dddd",
    "[$₽-419] #,##0.00",
    "_-* #,##0.00_-;-* #,##0.00_-;_-* \"-\"??_-;_-@_-",
    "0.00E+00",
    "# ??/??",
    "#,##0,",
    "General\"x\"",
    "[<1]0.00;[>=100]#,##0;0",
    "@",
    "00000",
];

fn rows() -> Vec<(String, String)> {
    let codes: Vec<String> = (0..=49).filter_map(builtin_format).map(str::to_string).chain(CUSTOM.iter().map(|c| c.to_string())).collect();
    let mut out = Vec::new();
    for code in &codes {
        let fmt = NumberFormat::parse(code);
        for v in VALUES {
            let f = format_value_in(&Value::Number(v), &fmt, DateSystem::D1900, NumLocale::EnUs);
            out.push((format!("{code}\t{v:?}"), format!("{:?}", f.text)));
        }
    }
    out
}

#[test]
fn en_us_output_equals_the_recorded_golden() {
    let rows = rows();
    let tsv: String = rows.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
    if let Ok(path) = std::env::var("GOLD_OUT") {
        std::fs::write(path, &tsv).expect("write golden");
        return;
    }
    let golden = include_str!("en_us_golden.tsv");
    assert_eq!(golden.lines().count(), rows.len(), "row count");
    for (g, (k, v)) in golden.lines().zip(&rows) {
        assert_eq!(g, format!("{k}\t{v}"), "en-US display changed");
    }
}
