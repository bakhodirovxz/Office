//! Phase 7: the real `plugins/uz-translit` package, installed into a temp suite-core store, loaded by [`plugin_native::load`] and run
//! through the native guest, ends with the same document as the built-in command. The package comes from
//! `cargo xt plugin pack uz-translit`; `UZ_TRANSLIT_PLUGIN_ZIP` names it (default: `$OFFICE_TARGET/plugins/uz-translit-1.0.0.zip`).

use std::path::PathBuf;

use serde_json::json;
use suite_core::plugins::Store;
use wordcraft_engine::{Session, io};

use crate::plugin_native::load;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs-translit/tests/fixtures");

fn zip() -> Option<PathBuf> {
    let p = std::env::var_os("UZ_TRANSLIT_PLUGIN_ZIP")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OFFICE_TARGET").map(|t| PathBuf::from(t).join("plugins/uz-translit-1.0.0.zip")))
        .filter(|p| p.is_file());
    if p.is_none() {
        eprintln!("UZ_TRANSLIT_PLUGIN_ZIP is not set: run `cargo xt plugin pack uz-translit`; test skipped");
    }
    p
}

fn store(dir: &std::path::Path) -> Store {
    Store::new(dir.join("plugins"), None, dir.to_path_buf())
}

fn open(name: &str, bytes: &[u8]) -> Session {
    let (doc, _) = io::open_bytes_with(name, bytes, None).unwrap();
    let mut s = Session::new(doc);
    docs_translit::install(&mut s);
    docs_plugins::register(&mut s, "en");
    s
}

#[test]
fn the_stored_plugin_gives_the_builtin_result_and_a_changed_package_is_refused() {
    let Some(zip) = zip() else { return };
    let dir = std::env::temp_dir().join(format!("office-plugin-native-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let st = store(&dir);
    st.install_zip(&zip).unwrap();
    // installed but not enabled: nothing loads
    docs_plugins::clear();
    assert!(load(&st).is_empty());
    st.set_enabled("uz-translit", true).unwrap();
    assert_eq!(load(&st), ["uz-translit"]);
    assert_eq!(docs_plugins::plugins().len(), 1);

    for file in ["01-split-runs.docx", "../uz108-latn.docx"] {
        let bytes = std::fs::read(format!("{FIXTURES}/word/{file}")).unwrap();
        let (mut a, mut b) = (open(file, &bytes), open(file, &bytes));
        let before = a.doc.clone();
        let p = json!({"scope": "document"});
        a.run("tools.translitCyrl", &p).unwrap();
        b.run("plugin.uz-translit.toCyrillic", &p).unwrap();
        assert!(a.doc != before, "{file}: the built-in changed nothing");
        assert!(a.doc == b.doc, "{file}: the plugin's document differs from the built-in's");
    }

    // a plugin.wasm that no longer matches the install record is refused and the plugin goes away
    let wasm = std::fs::read_dir(dir.join("plugins/uz-translit")).unwrap().flatten().next().unwrap().path().join("plugin.wasm");
    let mut bytes = std::fs::read(&wasm).unwrap();
    bytes.push(0);
    std::fs::write(&wasm, bytes).unwrap();
    assert!(load(&st).is_empty());
    assert!(docs_plugins::plugins().is_empty());
    docs_plugins::clear();
    let _ = std::fs::remove_dir_all(&dir);
}
