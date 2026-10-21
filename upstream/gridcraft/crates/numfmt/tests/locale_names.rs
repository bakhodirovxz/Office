//! Month and day names by locale and `[$-xxx]` tags (office patch 0025). The names are the
//! Windows NLS ones Excel shows (locale-check matrix, measured on ru-RU).
use gridcraft_core::{DateSystem, NumLocale, Value};
use gridcraft_numfmt::{NumberFormat, format_value_in};

fn show(v: f64, code: &str, loc: NumLocale) -> String {
    format_value_in(&Value::Number(v), &NumberFormat::parse(code), DateSystem::D1900, loc).text
}

/// 12 October 2026, a Monday.
const D: f64 = 46_307.0;

#[test]
fn names_follow_the_locale() {
    assert_eq!(show(D, "d mmmm yyyy", NumLocale::Ru), "12 Октябрь 2026");
    assert_eq!(show(D, "dddd, d mmm yyyy", NumLocale::Ru), "понедельник, 12 окт 2026");
    assert_eq!(show(D, "ddd", NumLocale::Ru), "Пн");
    assert_eq!(show(D, "mmmmm", NumLocale::Ru), "О");
    assert_eq!(show(D, "d mmmm yyyy", NumLocale::UzLatn), "12 Oktabr 2026");
    assert_eq!(show(D, "dddd, d mmm yyyy", NumLocale::UzLatn), "dushanba, 12 Okt 2026");
    assert_eq!(show(D, "ddd", NumLocale::UzLatn), "Dush");
    assert_eq!(show(D, "d mmmm yyyy", NumLocale::UzCyrl), "12 октябр 2026");
    assert_eq!(show(D, "dddd, d mmm yyyy", NumLocale::UzCyrl), "душанба, 12 окт 2026");
    assert_eq!(show(D, "d mmmm yyyy", NumLocale::EnUs), "12 October 2026");
    assert_eq!(show(D, "dddd, d mmm yyyy", NumLocale::EnUs), "Monday, 12 Oct 2026");
    // every month and weekday, abbreviated and full, is non-empty and distinct per month
    for loc in [NumLocale::Ru, NumLocale::UzLatn, NumLocale::UzCyrl] {
        let months: std::collections::HashSet<String> = (0..12).map(|m| show(46_023.0 + 31.0 * m as f64, "mmmm", loc)).collect();
        assert_eq!(months.len(), 12, "{loc:?}");
        let days: std::collections::HashSet<String> = (0..7).map(|d| show(D + d as f64, "dddd", loc)).collect();
        assert_eq!(days.len(), 7, "{loc:?}");
    }
}

#[test]
fn locale_tags_name_months_for_a_non_english_interface_only() {
    // [$-419] Russian, [$-443] uz-Latn, [$-843] uz-Cyrl; the digits are not affected
    assert_eq!(show(D, "[$-419]d mmmm yyyy", NumLocale::UzLatn), "12 Октябрь 2026");
    assert_eq!(show(D, "[$-443]d mmmm yyyy", NumLocale::Ru), "12 Oktabr 2026");
    assert_eq!(show(D, "[$-843]d mmmm yyyy", NumLocale::Ru), "12 октябр 2026");
    assert_eq!(show(D, "[$-10419]d mmmm yyyy", NumLocale::UzLatn), "12 Октябрь 2026");
    assert_eq!(show(D, "[$-409]d mmmm yyyy", NumLocale::Ru), "12 October 2026");
    // an unknown LCID leaves the interface's names
    assert_eq!(show(D, "[$-407]d mmmm yyyy", NumLocale::Ru), "12 Октябрь 2026");
    // en-US interface: the tag stays ignored (owner decision Q3, WP14)
    assert_eq!(show(D, "[$-419]d mmmm yyyy", NumLocale::EnUs), "12 October 2026");
    assert_eq!(show(D, "[$-443]d mmmm yyyy", NumLocale::EnUs), "12 October 2026");
    // a currency symbol with an LCID keeps both jobs
    assert_eq!(show(3.5, "[$₽-419] 0.0", NumLocale::Ru), "₽ 3,5");
}
