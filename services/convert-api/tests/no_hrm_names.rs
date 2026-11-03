//! PLAN phase 4, 5.6: the HR system's names never appear in our code. It is one configured provider (`[[providers]]`), described only in
//! docs. The list is the product and host names of `docs/phase0/hrm-current-state.md`; a name is a whole word, any case.
use std::path::Path;

const NAMES: [&str; 5] = ["hrm", "hrb", "xorgx", "oneid", "onlyoffice"];

/// `compat`: the embed scripts say in comments that they follow the public ONLYOFFICE API (clean-room note); that one name is
/// allowed on comment lines there. Code, strings and every other name still count.
fn scan(dir: &Path, compat: bool, hits: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan(&p, compat, hits);
        } else if p.extension().is_some_and(|x| {
            ["rs", "js", "html", "css", "json", "toml"]
                .iter()
                .any(|k| x == *k)
        }) {
            let text = std::fs::read_to_string(&p)
                .unwrap_or_default()
                .to_lowercase();
            for line in text.lines() {
                let t = line.trim_start();
                let comment = t.starts_with("//") || t.starts_with('*') || t.starts_with("/*");
                for w in line.split(|c: char| !c.is_alphanumeric()) {
                    if NAMES.contains(&w) && !(compat && comment && w == "onlyoffice") {
                        hits.push(format!("{}: {w}", p.display()));
                    }
                }
            }
        }
    }
}

#[test]
fn no_hrm_names_in_code() {
    let svc = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = svc.join("..");
    let mut hits = vec![];
    for (d, compat) in [
        (svc.join("docserver/static"), true),
        (svc.join("convert-api/src"), false),
        (svc.join("convert-api/ui"), false),
        (svc.join("suite-serve/src"), false),
        (svc.join("docserver/src"), false),
        (root.join("web/shell"), false),
        (root.join("crates"), false),
        (root.join("plugins"), false),
        (root.join("web/plugins"), false),
    ] {
        assert!(d.is_dir(), "{} moved: fix this test", d.display());
        scan(&d, compat, &mut hits);
    }
    assert!(hits.is_empty(), "{hits:#?}");
}
