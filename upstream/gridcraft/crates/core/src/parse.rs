//! Parsing what people type into a cell (and numeric text in formulas).
//!
//! Recognises numbers with thousands separators, percentages, currency, scientific notation,
//! fractions (`1 1/2`), dates (`3/4/2026`, `2026-03-04`, `4-Mar-2026`, `Mar 4, 2026`), times
//! (`10:30`, `10:30:15 PM`) and date-times. Locale: en-US.

use crate::date::{DateSystem, MONTHS, serial_from_ymd, time_fraction};
use crate::locale::NumLocale;
use crate::value::{CellError, Value};

/// A parsed entry: its value and the number format Excel would apply automatically.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub value: Value,
    pub format: Option<&'static str>,
}

/// Interprets typed cell input (not formulas). Text that isn't a number, date, boolean or error
/// stays text. A leading apostrophe forces text.
pub fn parse_input(s: &str, sys: DateSystem) -> Parsed {
    if s.is_empty() {
        return Parsed { value: Value::Empty, format: None };
    }
    if let Some(rest) = s.strip_prefix('\'') {
        return Parsed { value: Value::text(rest), format: None };
    }
    let t = s.trim();
    if t.eq_ignore_ascii_case("TRUE") {
        return Parsed { value: Value::Bool(true), format: None };
    }
    if t.eq_ignore_ascii_case("FALSE") {
        return Parsed { value: Value::Bool(false), format: None };
    }
    if let Some(e) = CellError::parse(t) {
        return Parsed { value: Value::Error(e), format: None };
    }
    if let Some((n, f)) = parse_numeric(t, sys) {
        return Parsed { value: Value::number(n), format: f };
    }
    Parsed { value: Value::text(s), format: None }
}

/// [`parse_input`] for text typed under a locale: `1 234,5` is a number, `12.10.2026` is
/// 12 October and `12 окт 2026` a date. En-US gives exactly [`parse_input`]. The result is the
/// same value and format the en-US parser gives for [`to_invariant`] of the text.
pub fn parse_input_in(s: &str, sys: DateSystem, loc: NumLocale) -> Parsed {
    if loc == NumLocale::EnUs {
        return parse_input(s, sys);
    }
    parse_input(&to_invariant(s, sys, loc), sys)
}

/// Typed text under a locale, rewritten as the en-US input text that means the same: `1 234,5`
/// becomes `1,234.5`, `12.10.2026` becomes `10/12/2026`, `3 окт 2026` becomes `3-Oct-2026`.
/// The engine only ever sees this text. Text that the locale does not read as a number or date
/// but the en-US parser would (`1.234` in ru) gets a leading apostrophe, so it stays text and
/// never becomes a different number. En-US returns `s` unchanged.
pub fn to_invariant(s: &str, sys: DateSystem, loc: NumLocale) -> String {
    if loc == NumLocale::EnUs || s.is_empty() || s.starts_with('\'') {
        return s.to_string();
    }
    if let Some(out) = localize(s.trim(), sys, loc) {
        return out;
    }
    if matches!(parse_input(s, sys).value, Value::Number(_)) { format!("'{s}") } else { s.to_string() }
}

/// The en-US text for a locale number or date, checked to parse as a number.
fn localize(t: &str, sys: DateSystem, loc: NumLocale) -> Option<String> {
    if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        return localize(inner.trim(), sys, loc).map(|o| format!("({o})"));
    }
    let ok = |o: String| parse_numeric(&o, sys).is_some().then_some(o);
    if let Some(d) = localize_date(t, loc).and_then(ok) {
        return Some(d);
    }
    for (i, _) in t.match_indices(' ') {
        let (a, b) = (t.get(..i)?, t.get(i + 1..)?);
        if parse_time(b).is_some()
            && let Some(d) = localize_date(a, loc)
        {
            return ok(format!("{d} {b}"));
        }
    }
    if let Some(n) = localize_plain(t).and_then(ok) {
        return Some(n);
    }
    // Same meaning in every locale: times, ISO dates, fractions, `50%`, `$5`, `(100)`, `1E3`.
    let same = !t.contains(['.', ',']) && (!t.contains('/') || parse_fraction(t).is_some());
    if same && parse_numeric(t, sys).is_some() { Some(t.to_string()) } else { None }
}

/// `,` is the decimal and space, U+00A0 or U+202F between digits are group separators. Text with a
/// `.` is not a plain number (it goes to the date rules).
fn localize_plain(t: &str) -> Option<String> {
    if t.contains('.') || t.matches(',').count() > 1 {
        return None;
    }
    let cs: Vec<char> = t.chars().collect();
    let digit = |i: Option<usize>| i.and_then(|i| cs.get(i)).is_some_and(char::is_ascii_digit);
    let mut out = String::with_capacity(t.len());
    for (i, &c) in cs.iter().enumerate() {
        out.push(match c {
            ',' => '.',
            ' ' | '\u{a0}' | '\u{202f}' if digit(i.checked_sub(1)) && digit(Some(i + 1)) => ',',
            c => c,
        });
    }
    parse_plain(&out).map(|_| out)
}

/// Month `1..=12` named by `tok`: the locale's full or abbreviated name (exact, case-insensitive,
/// optional trailing `.`), else an English name as [`month_from_name`] reads it.
fn month_in(tok: &str, loc: NumLocale) -> Option<usize> {
    let tok = tok.trim_end_matches('.');
    let l = tok.to_lowercase();
    let same = |names: &[&str; 12]| names.iter().position(|n| n.to_lowercase() == l);
    same(loc.months()).or_else(|| same(loc.months_abbr())).map(|i| i + 1).or_else(|| month_from_name(tok).map(|m| m as usize))
}

/// `d.m.y` (also `/` and `-`), `d.m`, `m.yyyy`, `3 окт 2026`, `окт 2026`, as en-US text.
fn localize_date(t: &str, loc: NumLocale) -> Option<String> {
    let num = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = t.split(['/', '-', '.']).map(str::trim).collect();
    if parts.iter().all(|p| num(p)) {
        return match parts.as_slice() {
            [a, _, _] if a.len() == 4 => Some(t.to_string()),
            [d, m, y] => Some(format!("{m}/{d}/{y}")),
            [m, y] if y.len() == 4 => Some(format!("{m}/{y}")),
            [d, m] => Some(format!("{m}/{d}")),
            _ => None,
        };
    }
    let words: Vec<&str> = t.split([' ', ',', '-', '/', '.']).filter(|w| !w.is_empty()).collect();
    let abbr = |m: usize| NumLocale::EnUs.months_abbr().get(m.wrapping_sub(1)).copied();
    match words.as_slice() {
        [d, m, y] if num(d) && num(y) => Some(format!("{d}-{}-{y}", abbr(month_in(m, loc)?)?)),
        [m, d, y] if num(d) && num(y) => Some(format!("{d}-{}-{y}", abbr(month_in(m, loc)?)?)),
        [d, m] if num(d) => Some(format!("{d}-{}", abbr(month_in(m, loc)?)?)),
        [m, y] if num(y) => Some(format!("{}-{y}", abbr(month_in(m, loc)?)?)),
        _ => None,
    }
}

/// Numeric text as arithmetic coerces it ("1,000", "50%", "$3", "1/2/2020", "10:00").
pub fn parse_number_text(s: &str) -> Option<f64> {
    parse_number_text_in(s, DateSystem::D1900)
}

/// Like [`parse_number_text`], with date text read in the given workbook date system.
pub fn parse_number_text_in(s: &str, sys: DateSystem) -> Option<f64> {
    parse_numeric(s.trim(), sys).map(|(n, _)| n)
}

fn parse_numeric(t: &str, sys: DateSystem) -> Option<(f64, Option<&'static str>)> {
    if t.is_empty() {
        return None;
    }
    // Parenthesised negatives: (100) = -100.
    if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        return parse_numeric(inner.trim(), sys).map(|(n, f)| (-n, f));
    }
    if let Some(r) = parse_plain(t) {
        return Some(r);
    }
    if let Some(r) = parse_fraction(t) {
        return Some((r, Some("# ?/?")));
    }
    if let Some(r) = parse_datetime(t, sys) {
        return Some(r);
    }
    None
}

/// Signs, currency, thousands separators, percent, exponent.
fn parse_plain(t: &str) -> Option<(f64, Option<&'static str>)> {
    let mut s = t;
    let mut neg = false;
    let mut currency = false;
    let mut percent = false;
    if let Some(r) = s.strip_prefix('-') {
        neg = true;
        s = r.trim_start();
    } else if let Some(r) = s.strip_prefix('+') {
        s = r.trim_start();
    }
    if let Some(r) = s.strip_prefix('$') {
        currency = true;
        s = r.trim_start();
        if !neg && let Some(r2) = s.strip_prefix('-') {
            neg = true;
            s = r2;
        }
    }
    if let Some(r) = s.strip_suffix('%') {
        percent = true;
        s = r.trim_end();
    }
    if s.is_empty() {
        return None;
    }
    let has_sep = s.contains(',');
    let digits: String = if has_sep {
        // Thousands separators must be in groups of three before the decimal point.
        let (int, frac) = s.split_once('.').map_or((s, None), |(a, b)| (a, Some(b)));
        let groups: Vec<&str> = int.split(',').collect();
        let first = groups.first()?;
        if first.is_empty() || first.len() > 3 || groups.iter().skip(1).any(|g| g.len() != 3) {
            return None;
        }
        let mut d = groups.concat();
        if let Some(f) = frac {
            d.push('.');
            d.push_str(f);
        }
        d
    } else {
        s.to_string()
    };
    let b = digits.as_bytes();
    if !b.iter().all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-')) {
        return None;
    }
    if !b.first().is_some_and(|c| c.is_ascii_digit() || *c == b'.') {
        return None;
    }
    let mut n: f64 = digits.parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    let sci = digits.contains(['e', 'E']);
    if percent {
        n /= 100.0;
    }
    if neg {
        n = -n;
    }
    let fmt = if currency {
        Some(if digits.contains('.') { "\"$\"#,##0.00" } else { "\"$\"#,##0" })
    } else if percent {
        Some(if digits.contains('.') { "0.00%" } else { "0%" })
    } else if sci {
        Some("0.00E+00")
    } else if has_sep {
        Some(if digits.contains('.') { "#,##0.00" } else { "#,##0" })
    } else {
        None
    };
    Some((n, fmt))
}

/// `1/2` (only with a whole part, else it's a date), `1 1/2`, `0 3/4`.
fn parse_fraction(t: &str) -> Option<f64> {
    let (whole, frac) = t.split_once(' ')?;
    let neg = whole.starts_with('-');
    let w: f64 = whole.trim_start_matches('-').parse().ok()?;
    let (a, b) = frac.trim().split_once('/')?;
    let a: f64 = a.parse().ok()?;
    let b: f64 = b.parse().ok()?;
    if b == 0.0 || !w.is_finite() {
        return None;
    }
    let v = w + a / b;
    Some(if neg { -v } else { v })
}

fn month_from_name(s: &str) -> Option<u32> {
    let l = s.to_ascii_lowercase();
    if l.len() < 3 {
        return None;
    }
    MONTHS.iter().position(|m| m.to_ascii_lowercase().starts_with(&l)).map(|i| i as u32 + 1)
}

/// Time part: `h:mm`, `h:mm:ss`, `h:mm:ss.000`, `h AM`, with optional AM/PM.
fn parse_time(t: &str) -> Option<(f64, bool, bool)> {
    let t = t.trim();
    let lower = t.to_ascii_lowercase();
    let (body, ampm) = if let Some(b) = lower.strip_suffix("am").or_else(|| lower.strip_suffix('a')) {
        (b.trim().to_string(), Some(false))
    } else if let Some(b) = lower.strip_suffix("pm").or_else(|| lower.strip_suffix('p')) {
        (b.trim().to_string(), Some(true))
    } else {
        (lower.clone(), None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() == 1 && ampm.is_none() {
        return None;
    }
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let mut h: f64 = parts.first()?.parse().ok()?;
    let m: f64 = parts.get(1).map_or(Some(0.0), |p| p.parse().ok())?;
    let s: f64 = parts.get(2).map_or(Some(0.0), |p| p.parse().ok())?;
    if m >= 60.0 || s >= 60.0 || h < 0.0 || m < 0.0 || s < 0.0 {
        return None;
    }
    if let Some(pm) = ampm {
        if !(0.0..=12.0).contains(&h) {
            return None;
        }
        if h == 12.0 {
            h = 0.0;
        }
        if pm {
            h += 12.0;
        }
    } else if h >= 10000.0 {
        return None;
    }
    Some((time_fraction(h, m, s), parts.len() == 3, ampm.is_some()))
}

fn parse_date(t: &str, sys: DateSystem) -> Option<(f64, &'static str)> {
    let t = t.trim().trim_end_matches(',');
    let num = |s: &str| s.trim().parse::<i64>().ok();
    // ISO yyyy-mm-dd or yyyy/mm/dd
    let seps: &[char] = &['/', '-', '.'];
    let parts: Vec<&str> = t.split(seps).collect();
    if parts.len() == 3 {
        let (a, b, c) = (parts[0].trim(), parts[1].trim(), parts[2].trim());
        if a.len() == 4
            && let (Some(y), Some(m), Some(d)) = (num(a), num(b), num(c))
        {
            return valid_ymd(sys, y, m, d).map(|s| (s, "yyyy-mm-dd"));
        }
        if let (Some(m), Some(d), Some(y)) = (num(a), num(b), num(c)) {
            let y = if c.len() <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
            return valid_ymd(sys, y, m, d).map(|s| (s, "m/d/yyyy"));
        }
        // 4-Mar-2026
        if let (Some(d), Some(m), Some(y)) = (num(a), month_from_name(b), num(c)) {
            let y = if c.len() <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
            return valid_ymd(sys, y, m as i64, d).map(|s| (s, "d-mmm-yy"));
        }
    }
    if parts.len() == 2 {
        let (a, b) = (parts[0].trim(), parts[1].trim());
        // m/d in the current year is ambiguous without a clock; use month/day of 2026? Excel uses
        // the current year. We accept d-mmm / mmm-d only with month names, and m/d → current year.
        if let (Some(m), Some(d)) = (num(a), num(b)) {
            let y = current_year();
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return valid_ymd(sys, y, m, d).map(|s| (s, "d-mmm"));
            }
            // m/yyyy
            if (1..=12).contains(&m) && b.len() == 4 {
                return valid_ymd(sys, d, m, 1).map(|s| (s, "mmm-yy"));
            }
        }
        if let (Some(d), Some(m)) = (num(a), month_from_name(b)) {
            return valid_ymd(sys, current_year(), m as i64, d).map(|s| (s, "d-mmm"));
        }
        if let (Some(m), Some(y)) = (month_from_name(a), num(b)) {
            let y = if b.len() <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
            return valid_ymd(sys, y, m as i64, 1).map(|s| (s, "mmm-yy"));
        }
    }
    // "Mar 4, 2026" / "March 4 2026" / "4 March 2026"
    let words: Vec<&str> = t.split([' ', ',']).filter(|w| !w.is_empty()).collect();
    if words.len() == 3 {
        // Two-digit years follow the same 2029 rule as the dashed forms above.
        let expand = |w: &str, y: i64| if w.len() <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
        if let (Some(m), Some(d), Some(y)) = (month_from_name(words[0]), num(words[1]), num(words[2])) {
            let y = expand(words[2], y);
            return valid_ymd(sys, y, m as i64, d).map(|s| (s, "d-mmm-yy"));
        }
        if let (Some(d), Some(m), Some(y)) = (num(words[0]), month_from_name(words[1]), num(words[2])) {
            let y = expand(words[2], y);
            return valid_ymd(sys, y, m as i64, d).map(|s| (s, "d-mmm-yy"));
        }
    }
    None
}

fn valid_ymd(sys: DateSystem, y: i64, m: i64, d: i64) -> Option<f64> {
    // Dates before 1900 stay text (`DATE()` would add 1900 to such a year).
    if !(1..=12).contains(&m) || d < 1 || y < 1900 {
        return None;
    }
    let max = crate::date::days_in_month_in(sys, y as i32, m as u32) as i64;
    if d > max {
        return None;
    }
    serial_from_ymd(sys, y, m, d)
}

/// The year used for dates typed without one, from the system clock (browser clock on wasm).
/// Falls back to 2026 only when no clock is available.
pub fn current_year() -> i64 {
    if let Some(ms) = crate::date::unix_now_ms() {
        let days = (ms / 86_400_000.0).floor() as i64;
        return crate::date::civil_from_days(days).0 as i64;
    }
    2026
}

fn parse_datetime(t: &str, sys: DateSystem) -> Option<(f64, Option<&'static str>)> {
    if let Some((tf, secs, ampm)) = parse_time(t) {
        let f = match (secs, ampm) {
            // 24 hours or more: elapsed hours, so 25:00 shows as 25:00:00, not 1:00.
            _ if tf >= 1.0 => "[h]:mm:ss",
            (false, false) => "h:mm",
            (true, false) => "h:mm:ss",
            (false, true) => "h:mm AM/PM",
            (true, true) => "h:mm:ss AM/PM",
        };
        return Some((tf, Some(f)));
    }
    if let Some((d, f)) = parse_date(t, sys) {
        return Some((d, Some(f)));
    }
    // date + time: split at the last space(s) before a time-looking tail.
    for (i, _) in t.match_indices(' ') {
        let (a, b) = (t.get(..i)?, t.get(i + 1..)?);
        if let (Some((d, _)), Some((tf, _, _))) = (parse_date(a, sys), parse_time(b)) {
            return Some((d + tf, Some("m/d/yyyy h:mm")));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Parsed {
        parse_input(s, DateSystem::D1900)
    }

    #[test]
    fn month_name_short_year() {
        for s in ["Mar 4, 26", "Mar 4 26", "March 4 26", "4 March 26", "Mar 4, 2026"] {
            assert_eq!(p(s).value, Value::Number(46085.0), "{s}");
        }
        assert_eq!(p("Mar 4, 99").value, p("Mar 4, 1999").value);
        assert_eq!(p("Mar 4, 1776").value, Value::text("Mar 4, 1776"));
        assert_eq!(p("Feb 30, 26").value, Value::text("Feb 30, 26"));
    }

    #[test]
    fn numbers() {
        assert_eq!(p("42").value, Value::Number(42.0));
        assert_eq!(p("-3.5").value, Value::Number(-3.5));
        assert_eq!(p("1,234").format, Some("#,##0"));
        assert_eq!(p("1,234").value, Value::Number(1234.0));
        assert_eq!(p("1,23").value, Value::text("1,23"));
        assert_eq!(p("12%").value, Value::Number(0.12));
        assert_eq!(p("$1,200.50").value, Value::Number(1200.5));
        assert_eq!(p("$1,200.50").format, Some("\"$\"#,##0.00"));
        assert_eq!(p("(100)").value, Value::Number(-100.0));
        assert_eq!(p("1e3").value, Value::Number(1000.0));
        assert_eq!(p("1 1/2").value, Value::Number(1.5));
        assert_eq!(p(".5").value, Value::Number(0.5));
    }

    #[test]
    fn texts_and_specials() {
        assert_eq!(p("hello").value, Value::text("hello"));
        assert_eq!(p("'123").value, Value::text("123"));
        assert_eq!(p("true").value, Value::Bool(true));
        assert_eq!(p("#N/A").value, Value::Error(CellError::NA));
        assert_eq!(p("e").value, Value::text("e"));
        assert_eq!(p("-").value, Value::text("-"));
        assert_eq!(p("1.2.3.4").value, Value::text("1.2.3.4"));
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(p("10/7/2026").value, Value::Number(46302.0));
        assert_eq!(p("2026-10-07").value, Value::Number(46302.0));
        assert_eq!(p("7-Oct-2026").value, Value::Number(46302.0));
        assert_eq!(p("Oct 7, 2026").value, Value::Number(46302.0));
        assert_eq!(p("2/30/2026").value, Value::text("2/30/2026"));
        assert_eq!(p("7/4/1776").value, Value::text("7/4/1776"));
        assert_eq!(p("1776-07-04").value, Value::text("1776-07-04"));
        assert_eq!(p("4-Jul-1776").value, Value::text("4-Jul-1776"));
        assert_eq!(p("12/31/1899").value, Value::text("12/31/1899"));
        assert_eq!(p("1/1/1900").value, Value::Number(1.0));
        assert_eq!(p("12:00").value, Value::Number(0.5));
        assert_eq!(p("6:00 PM").value, Value::Number(0.75));
        assert_eq!(p("12:00 AM").value, Value::Number(0.0));
        assert_eq!(p("10/7/2026 18:00").value, Value::Number(46302.75));
        assert_eq!(p("25:00").value, Value::Number(25.0 / 24.0));
        assert_eq!(p("25:00").format, Some("[h]:mm:ss"));
        assert_eq!(p("23:59").format, Some("h:mm"));
    }

    #[test]
    fn fictitious_1900_02_29() {
        // Excel accepts 29 February 1900 (serial 60) and keeps 1 March 1900 at 61.
        assert_eq!(p("2/29/1900").value, Value::Number(60.0));
        assert_eq!(p("1900-02-29").value, Value::Number(60.0));
        assert_eq!(p("29-Feb-1900").value, Value::Number(60.0));
        assert_eq!(p("2/28/1900").value, Value::Number(59.0));
        assert_eq!(p("3/1/1900").value, Value::Number(61.0));
        assert_eq!(p("2/29/1901").value, Value::text("2/29/1901"));
        assert_eq!(p("2/30/1900").value, Value::text("2/30/1900"));
        // The 1904 system starts after it.
        assert_eq!(parse_input("2/29/1900", DateSystem::D1904).value, Value::text("2/29/1900"));
        assert_eq!(parse_input("2/29/1904", DateSystem::D1904).value, Value::Number(59.0));
    }

    fn pl(s: &str, loc: NumLocale) -> Parsed {
        parse_input_in(s, DateSystem::D1900, loc)
    }

    const LOCALES: [NumLocale; 3] = [NumLocale::Ru, NumLocale::UzLatn, NumLocale::UzCyrl];

    #[test]
    fn en_us_is_parse_input() {
        for s in ["3.5", "1,234.5", "12/10/2026", "Mar 4, 2026", "x", "", "'1", "1 1/2", "10:30"] {
            assert_eq!(pl(s, NumLocale::EnUs), p(s), "{s}");
        }
    }

    #[test]
    fn locale_numbers() {
        for l in LOCALES {
            assert_eq!(pl("3,5", l).value, Value::Number(3.5));
            assert_eq!(pl("1 234,5", l).value, Value::Number(1234.5));
            assert_eq!(pl("1 234,5", l).format, Some("#,##0.00"));
            assert_eq!(pl("1\u{a0}234\u{202f}567", l).value, Value::Number(1_234_567.0));
            assert_eq!(pl("-1 234,50", l).value, Value::Number(-1234.5));
            assert_eq!(pl("(1 234)", l).value, Value::Number(-1234.0));
            assert_eq!(pl("25,5%", l).value, Value::Number(0.255));
            assert_eq!(pl("1,5E3", l).value, Value::Number(1500.0));
            assert_eq!(pl("1,234", l).value, Value::Number(1.234));
            // en-US habits do not become other numbers: they stay text
            assert_eq!(pl("1.234", l).value, Value::text("1.234"));
            assert_eq!(pl("1,234,567", l).value, Value::text("1,234,567"));
            assert_eq!(pl("1 23 456", l).value, Value::text("1 23 456"));
            assert_eq!(pl("10:30", l).value, Value::Number(10.5 / 24.0));
            assert_eq!(pl("1 1/2", l).value, Value::Number(1.5));
            assert_eq!(pl("TRUE", l).value, Value::Bool(true));
            assert_eq!(pl("Iyu", l).value, Value::text("Iyu"));
            assert_eq!(pl("abc", l).value, Value::text("abc"));
        }
    }

    #[test]
    fn locale_dates() {
        let by_locale: [(NumLocale, &[&str]); 3] = [
            (NumLocale::Ru, &["12 окт 2026", "12 октябрь 2026", "12 ОКТ. 2026", "окт 12 2026"]),
            (NumLocale::UzLatn, &["12 Oktabr 2026", "12 oktabr 2026", "12 Okt 2026"]),
            (NumLocale::UzCyrl, &["12 октябр 2026", "12 окт 2026"]),
        ];
        for (l, names) in by_locale {
            // 12 October 2026 is serial 46307 (not 10 December = 46366)
            for s in ["12.10.2026", "12/10/2026", "12-10-2026", "12.10.26", "2026-10-12", "12 Oct 2026"].iter().chain(names) {
                assert_eq!(pl(s, l).value, Value::Number(46307.0), "{s} in {l:?}");
            }
            assert_eq!(pl("12.10.2026", l).format, Some("m/d/yyyy"));
            assert_eq!(pl("12.10.2026 15:00", l).value, Value::Number(46307.0 + 15.0 / 24.0));
            assert_eq!(pl("12.10.2026 15:00", l).format, Some("m/d/yyyy h:mm"));
            assert_eq!(pl("12.10", l).format, Some("d-mmm"));
            assert_eq!(pl("12.10", l).value, p(&format!("10/12/{}", current_year())).value);
            assert_eq!(pl("3.5", l).value, p(&format!("5/3/{}", current_year())).value);
            assert_eq!(pl("10.2026", l).value, Value::Number(46296.0));
            assert_eq!(pl("окт 2026", NumLocale::Ru).value, Value::Number(46296.0));
            // the forms the built-in ids 15-17 show in ru (Excel ru: 12.окт.26, 12.окт, окт.26)
            assert_eq!(pl("12.окт.26", NumLocale::Ru).value, Value::Number(46307.0));
            assert_eq!(pl("окт.26", NumLocale::Ru).value, Value::Number(46296.0));
            assert_eq!(pl("12.окт", NumLocale::Ru).value, p(&format!("10/12/{}", current_year())).value);
            // an impossible day or month stays text, also where en-US would read it
            assert_eq!(pl("31.02.2026", l).value, Value::text("31.02.2026"));
            assert_eq!(pl("10.13.2026", l).value, Value::text("10.13.2026"));
            assert_eq!(pl("10/13/2026", l).value, Value::text("10/13/2026"));
        }
    }

    #[test]
    fn the_data_integrity_case() {
        assert_eq!(p("10/12/2026").value, Value::Number(46307.0));
        assert_eq!(p("12.10.2026").value, Value::Number(46366.0), "en-US reads it as 10 December");
        let inv = |s, l| to_invariant(s, DateSystem::D1900, l);
        assert_eq!(inv("12.10.2026", NumLocale::Ru), "10/12/2026");
        assert_eq!(inv("12.10.2026 15:00", NumLocale::Ru), "10/12/2026 15:00");
        assert_eq!(inv("1 234,5", NumLocale::UzLatn), "1,234.5");
        assert_eq!(inv("25,5%", NumLocale::UzLatn), "25.5%");
        assert_eq!(inv("3 окт 2026", NumLocale::Ru), "3-Oct-2026");
        assert_eq!(inv("1.234", NumLocale::Ru), "'1.234");
        assert_eq!(inv("hello", NumLocale::Ru), "hello");
        assert_eq!(inv("'1,5", NumLocale::Ru), "'1,5");
        assert_eq!(inv("12.10.2026", NumLocale::EnUs), "12.10.2026");
    }

    /// Every date 1900..=2100 shown as dd.mm.yyyy reads back as the same serial and format.
    #[test]
    fn every_date_round_trips_in_every_locale() {
        let sys = DateSystem::D1900;
        for serial in 1..=73050 {
            let Some(dt) = crate::date::datetime_from_serial(sys, serial as f64) else { continue };
            let text = format!("{:02}.{:02}.{:04}", dt.day, dt.month, dt.year);
            for l in LOCALES {
                let got = parse_input_in(&text, sys, l);
                assert_eq!(got.value, Value::Number(serial as f64), "{text} in {l:?}");
                assert_eq!(got.format, Some("m/d/yyyy"));
            }
        }
    }

    /// Shown numbers (`#,##0.00` and General) read back as the same f64.
    #[test]
    fn shown_numbers_read_back_equal() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..10_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let k = x % 1_000_000_000_000;
            let (int, frac) = (k / 100, k % 100);
            let digits = int.to_string();
            let grouped: Vec<String> = digits.as_bytes().rchunks(3).rev().map(|c| String::from_utf8_lossy(c).into_owned()).collect();
            let want = Value::Number(k as f64 / 100.0);
            for l in LOCALES {
                assert_eq!(pl(&format!("{},{frac:02}", grouped.join("\u{a0}")), l).value, want, "{k}");
                assert_eq!(pl(&format!("{int},{frac:02}"), l).value, want, "{k}");
            }
        }
    }
}
