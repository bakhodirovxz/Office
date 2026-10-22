//! Decimal and group separators by locale (office patch 0024). The rows are the locale-check
//! matrix (docs/phase1/locale-check.md section 2) with the Windows NLS separators; only the
//! text changes, never the value or the format code.
use gridcraft_core::{DateSystem, NumLocale, Value};
use gridcraft_numfmt::{NumberFormat, format_general_fit_in, format_value_in};

fn show(v: f64, code: &str, loc: NumLocale) -> String {
    format_value_in(&Value::Number(v), &NumberFormat::parse(code), DateSystem::D1900, loc).text
}

const NB: &str = "\u{a0}";

#[test]
fn ru_and_uz_use_comma_decimal_and_nbsp_groups() {
    for loc in [NumLocale::Ru, NumLocale::UzLatn, NumLocale::UzCyrl] {
        assert_eq!(show(3_500_000.5, "General", loc), "3500000,5");
        assert_eq!(show(0.123_456_7, "General", loc), "0,1234567");
        assert_eq!(show(3_500_000.0, "0.0", loc), "3500000,0");
        assert_eq!(show(3_500_000.5, "#,##0.0", loc), format!("3{NB}500{NB}000,5"));
        assert_eq!(show(3_500_000.5, "#,##0.00", loc), format!("3{NB}500{NB}000,50"));
        assert_eq!(show(0.255, "0.00%", loc), "25,50%");
        assert_eq!(show(12_345.0, "0.00E+00", loc), "1,23E+04");
        assert_eq!(show(3_500_000.5, "[$-419]#,##0.00", loc), format!("3{NB}500{NB}000,50"));
        assert_eq!(show(3_500_000.5, "[$-443]#,##0.00", loc), format!("3{NB}500{NB}000,50"));
        assert_eq!(show(3_500_000.5, "[$₽-419] #,##0.00", loc), format!("₽ 3{NB}500{NB}000,50"));
        assert_eq!(show(3_500_000.5, "#,##0.00 \"so'm\"", loc), format!("3{NB}500{NB}000,50 so'm"));
        assert_eq!(show(-1234.5, "#,##0.00;[Red]-#,##0.00", loc), format!("-1{NB}234,50"));
        assert_eq!(show(1_234_567.0, "#,##0,", loc), format!("1{NB}235"));
        // grouping inside a fraction's whole part and a bare "." literal-free code
        assert_eq!(show(1234.5, "#,##0 ?/?", loc), format!("1{NB}234 1/2"));
        // dates keep the stored code's own separators
        assert_eq!(show(46_307.0, "dd.mm.yyyy", loc), "12.10.2026");
        assert_eq!(show(46_307.0, "yyyy-mm-dd", loc), "2026-10-12");
        // sub-seconds follow the decimal
        assert_eq!(show(0.000_01, "mm:ss.0", loc), "00:00,9");
        // the TEXT() default and a number in General at width: same width as en-US
        assert_eq!(format_general_fit_in(123_456.789_012, 11, loc).as_deref(), Some("123456,789"));
        assert_eq!(format_general_fit_in(1.0 / 3.0, 11, loc).as_deref(), Some("0,333333333"));
        assert_eq!(format_general_fit_in(0.000_012_345_678_9, 11, loc).as_deref(), Some("1,23457E-05"));
        assert_eq!(format_general_fit_in(-5.0, 11, loc).as_deref(), Some("-5"));
    }
}

#[test]
fn en_us_keeps_its_separators() {
    let en = NumLocale::EnUs;
    assert_eq!(show(3_500_000.5, "General", en), "3500000.5");
    assert_eq!(show(3_500_000.5, "#,##0.00", en), "3,500,000.50");
    assert_eq!(show(0.000_01, "mm:ss.0", en), "00:00.9");
    assert_eq!(format_general_fit_in(123_456.789_012, 11, en), gridcraft_numfmt::format_general_fit(123_456.789_012, 11));
}

#[test]
fn general_text_and_checkbox_sections_follow_the_locale() {
    // The General keyword inside a code, and a number shown through a text section.
    assert_eq!(show(1.5, "General\" kg\"", NumLocale::Ru), "1,5 kg");
    assert_eq!(show(1.5, "General\" kg\"", NumLocale::EnUs), "1.5 kg");
    let text_section = NumberFormat::parse("0.0;0.0;0.0;\"[\"@\"]\"");
    let f = format_value_in(&Value::text("abc"), &text_section, DateSystem::D1900, NumLocale::Ru);
    assert_eq!(f.text, "[abc]");
}
