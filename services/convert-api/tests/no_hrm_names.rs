//! PLAN phase 4, 5.6: the HR system's names never appear in our code. It is one configured provider (`[[providers]]`), described only in
//! docs. The list is the product and host names of `docs/phase0/hrm-current-state.md`; a name is a whole word, any case.
use std::path::Path;

const NAMES: [&str; 5] = ["hrm", "hrb", "xorgx", "oneid", "onlyoffice"];

fn scan(dir: &Path, hits: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan(&p, hits);
        } else if p.extension().is_some_and(|x| {
            ["rs", "js", "html", "css", "json", "toml"]
                .iter()
                .any(|k| x == *k)
        }) {
            let text = std::fs::read_to_string(&p)
                .unwrap_or_default()
                .to_lowercase();
            for w in text.split(|c: char| !c.is_alphanumeric()) {
                if NAMES.contains(&w) {
                    hits.push(format!("{}: {w}", p.display()));
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
    for d in [
        svc.join("convert-api/src"),
        svc.join("convert-api/ui"),
        svc.join("suite-serve/src"),
        root.join("web/shell"),
        root.join("crates"),
    ] {
        assert!(d.is_dir(), "{} moved: fix this test", d.display());
        scan(&d, &mut hits);
    }
    assert!(hits.is_empty(), "{hits:#?}");
}
