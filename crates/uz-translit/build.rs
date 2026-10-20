//! Compiles `data/exceptions.tsv` into sorted static tables (no parsing at run time, no_std friendly).
//! Only `reviewed:*` rows are compiled in (rules.md section 6.1).
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::{env, fs, path::Path};

type Table = BTreeMap<String, (String, bool)>; // key -> (value, is_stem)

fn put(t: &mut Table, key: String, val: &str, stem: bool, what: &str) {
    if t.insert(key.clone(), (val.to_string(), stem)).is_some() {
        panic!("exceptions.tsv: duplicate {what} key {key:?}");
    }
}

fn emit_kv(out: &mut String, name: &str, t: &Table) {
    writeln!(out, "pub(crate) static {name}: &[(&str, &str, Kind)] = &[").unwrap();
    for (k, (v, stem)) in t {
        let kind = if *stem { "Stem" } else { "Word" };
        writeln!(out, "    ({k:?}, {v:?}, Kind::{kind}),").unwrap();
    }
    out.push_str("];\n");
}

fn emit_pairs(out: &mut String, name: &str, t: &BTreeMap<String, String>) {
    writeln!(out, "pub(crate) static {name}: &[(&str, &str)] = &[").unwrap();
    for (k, v) in t {
        writeln!(out, "    ({k:?}, {v:?}),").unwrap();
    }
    out.push_str("];\n");
}

fn main() {
    println!("cargo:rerun-if-changed=data/exceptions.tsv");
    println!("cargo:rerun-if-changed=build.rs");
    let src = fs::read_to_string("data/exceptions.tsv").expect("data/exceptions.tsv");
    let (mut l2c, mut c2l) = (Table::new(), Table::new());
    let (mut acro_l, mut acro_c) = (BTreeMap::new(), BTreeMap::new());
    let mut keep = Vec::new();
    let mut header = false;
    for (n, line) in src.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if !header {
            assert!(
                f[0] == "latin" && f.len() >= 6,
                "exceptions.tsv: bad header"
            );
            header = true;
            continue;
        }
        assert!(
            f.len() >= 6,
            "exceptions.tsv:{}: expected 6+ columns",
            n + 1
        );
        let (lat, cyr, mt, dir, conf, status) = (f[0], f[1], f[2], f[3], f[4], f[5]);
        assert!(
            ["high", "medium", "low"].contains(&conf),
            "exceptions.tsv:{}: conf",
            n + 1
        );
        assert!(
            ["both", "l2c", "c2l", "variant"].contains(&dir),
            "exceptions.tsv:{}: dir",
            n + 1
        );
        if !status.starts_with("reviewed:") {
            continue;
        }
        let what = format!("line {}", n + 1);
        match mt {
            "word" | "stem" => {
                let stem = mt == "stem";
                if matches!(dir, "both" | "l2c" | "variant") {
                    put(&mut l2c, lat.to_string(), cyr, stem, &what);
                }
                if matches!(dir, "both" | "c2l") {
                    put(&mut c2l, cyr.to_string(), lat, stem, &what);
                    if stem && cyr.ends_with('ь') {
                        put(
                            &mut c2l,
                            cyr.trim_end_matches('ь').to_string(),
                            lat,
                            true,
                            &what,
                        );
                    }
                }
            }
            "acro" => {
                assert!(
                    acro_l.insert(lat.to_string(), cyr.to_string()).is_none(),
                    "{what}: duplicate acro"
                );
                acro_c.insert(cyr.to_string(), lat.to_string());
            }
            "keep" => keep.push(lat.to_string()),
            _ => panic!("exceptions.tsv:{}: bad match {mt:?}", n + 1),
        }
    }
    keep.sort();
    let mut out = String::new();
    emit_kv(&mut out, "L2C", &l2c);
    emit_kv(&mut out, "C2L", &c2l);
    emit_pairs(&mut out, "ACRO_L", &acro_l);
    emit_pairs(&mut out, "ACRO_C", &acro_c);
    writeln!(out, "pub(crate) static KEEP: &[&str] = &{keep:?};").unwrap();
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("dict_gen.rs"),
        out,
    )
    .unwrap();
}
