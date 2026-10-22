//! Office overlay (patch 0027): `FormulaLocale::DecimalComma`, the formula entry of Russian and
//! Uzbek (`;` between arguments, `,` as the decimal, English function names).
use gridcraft_formula::{FormulaLocale, parse, parse_input, print, print_input};

const L: FormulaLocale = FormulaLocale::DecimalComma;

fn input(src: &str) -> String {
    print(&parse_input(src, L, |_| false).unwrap())
}

#[test]
fn separators_follow_excel_in_ru_and_uz() {
    assert_eq!((L.list_separator(), L.decimal_separator(), L.array_column_separator()), (';', ',', '\\'));
    let en = FormulaLocale::En;
    assert_eq!((en.list_separator(), en.decimal_separator(), en.array_column_separator()), (',', '.', ','));
    assert_eq!(L.function_name("SUM"), "SUM");
    assert_eq!(L.canonical_name("SUM"), "SUM");
}

#[test]
fn arguments_and_decimals_become_the_stored_syntax() {
    for (src, expected) in [
        ("=SUM(1;2)", "SUM(1,2)"),
        ("1,5+1", "1.5+1"),
        ("SUM(1,5;2,25)", "SUM(1.5,2.25)"),
        ("IF(A1>0;\"да\";\"нет\")", "IF(A1>0,\"да\",\"нет\")"),
        ("SUM((A1:A3;C1:C3))", "SUM((A1:A3,C1:C3))"),
        ("SUM({1,5\\2;3\\4};5)", "SUM({1.5,2;3,4},5)"),
        ("ROUND(2,675;2)", "ROUND(2.675,2)"),
        ("LAMBDA(x;x+1)(2)", "LAMBDA(x,x+1)(2)"),
    ] {
        assert_eq!(input(src), expected, "{src}");
    }
}

#[test]
fn strings_and_sheet_names_keep_their_separators() {
    for (src, expected) in [
        ("\"a;b,c\"&\"x\"\"y\"", "\"a;b,c\"&\"x\"\"y\""),
        ("'My;Sheet,1'!A1+1,5", "'My;Sheet,1'!A1+1.5"),
        ("CONCAT(\"1,5\";\"2;3\")", "CONCAT(\"1,5\",\"2;3\")"),
    ] {
        assert_eq!(input(src), expected, "{src}");
    }
}

#[test]
fn english_function_names_only_and_no_spanish_aliases() {
    assert_eq!(input("SUMA(1;2)"), "SUMA(1,2)");
    assert_eq!(input("VERDADERO"), "VERDADERO");
}

#[test]
fn a_comma_as_argument_separator_is_an_error_like_in_excel() {
    assert!(parse_input("SUM(1,2)", L, |_| false).is_ok(), "1,2 is the number 1.2 inside one argument list");
    assert_eq!(input("SUM(1,2)"), "SUM(1.2)");
    assert!(parse_input("SUM(1;;", L, |_| false).is_err());
}

#[test]
fn printing_for_editing_round_trips() {
    for stored in ["SUM(1.5,2.25)", "IF(A1>0,\"a;b\",2.5)", "SUM({1.5,2;3,4},5)", "SUM((A1:A3,C1:C3))", "A1+2.5%", "TEXT(A1,\"0.00\")"] {
        let expr = parse(stored).unwrap();
        let shown = print_input(&expr, L, |_| false).unwrap();
        assert_ne!(shown, stored, "{stored} should change in this locale");
        assert_eq!(print(&parse_input(&shown, L, |_| false).unwrap()), print(&expr), "{stored} via {shown}");
    }
    assert_eq!(print_input(&parse("SUM(1.5,2)").unwrap(), L, |_| false).unwrap(), "SUM(1,5;2)");
    assert_eq!(print_input(&parse("SUM({1,2;3,4})").unwrap(), L, |_| false).unwrap(), "SUM({1\\2;3\\4})");
}
