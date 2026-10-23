//! The recent files, read from the four apps' own lists (PLAN 5.2, F9). The apps stay the owners of their lists; this module
//! only reads them (read-only, tolerant: a list that is missing, too big or does not parse is skipped with a warning, never fatal).
//! There is **no upstream change**: if a sync changes a list's shape, the contract tests below fail in CI.
//!
//! Where each list is, and its shape (all four verified against the BASEs of 2026-10-22):
//!
//! | app | file in the app's settings folder | shape |
//! |---|---|---|
//! | Docs (wordcraft) | `ui.json` | `{"recent": ["<path>", ...]}`, newest first |
//! | Sheets (gridcraft) | `ui.json` | the same |
//! | Slides (deckcraft) | `ui.json` | the same |
//! | PDF (pdfcraft) | `data/app.ron` (eframe's RON map of strings) | key `app` holds JSON: `{"recent": [{"name","path","pages","size"}, ...]}`, newest first |
//!
//! The settings folder of an app is `<config base>/<brand data dir>` once the install sets it up, else the upstream folder name
//! (`WordCraft`, `GridCraft`, `DeckCraft`, `PdfCraft`), and `WORDCRAFT_PROFILE_DIR` for wordcraft's isolated runs. The first
//! folder that has the file wins.
//!
//! The lists carry no per-file time. Within one app the order is the app's own. Across apps the entries get synthetic times:
//! the list file's modified time minus the position, so the app whose settings were saved last comes first and no app's own
//! order is ever reversed.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// A list file larger than this is not a recent list (pdfcraft's `app.ron` also holds the last session, so it is the big one).
const MAX_BYTES: u64 = 64 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum App {
    Docs,
    Sheets,
    Slides,
    Pdf,
}

impl App {
    pub const ALL: [App; 4] = [App::Docs, App::Sheets, App::Slides, App::Pdf];

    /// The id in `brand.toml` and in the host's replies.
    pub const fn id(self) -> &'static str {
        match self {
            App::Docs => "docs",
            App::Sheets => "sheets",
            App::Slides => "slides",
            App::Pdf => "pdf",
        }
    }

    /// The branded folder (`brand.toml` `data_dir`, for example `<Product>/Docs`).
    const fn brand_dir(self) -> &'static str {
        match self {
            App::Docs => env!("SUITE_DATA_DIR_DOCS"),
            App::Sheets => env!("SUITE_DATA_DIR_SHEETS"),
            App::Slides => env!("SUITE_DATA_DIR_SLIDES"),
            App::Pdf => env!("SUITE_DATA_DIR_PDF"),
        }
    }

    /// The folder the unmodified upstream app uses on Windows and macOS.
    const fn upstream_dir(self) -> &'static str {
        match self {
            App::Docs => "WordCraft",
            App::Sheets => "GridCraft",
            App::Slides => "DeckCraft",
            App::Pdf => "PdfCraft",
        }
    }
}

/// How an app stores its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    UiJson,
    PdfRon,
}

/// One list file to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub app: App,
    pub file: PathBuf,
    format: Format,
}

/// A recent file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recent {
    pub path: PathBuf,
    pub app: App,
    /// seconds since the epoch; synthetic (see the module comment), good for ordering only
    pub ts: u64,
}

/// The list files that exist under `base` (the config base) for each app. `profile` is wordcraft's `WORDCRAFT_PROFILE_DIR`.
pub fn sources_in(base: &Path, profile: Option<&Path>) -> Vec<Source> {
    let mut found = Vec::new();
    for app in App::ALL {
        let (format, tails): (Format, &[&str]) = match app {
            App::Pdf => (Format::PdfRon, &["data/app.ron", "app.ron"]),
            _ => (Format::UiJson, &["ui.json"]),
        };
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let (App::Docs, Some(p)) = (app, profile) {
            dirs.push(p.to_path_buf());
        }
        dirs.push(base.join(app.brand_dir()));
        dirs.push(base.join(app.upstream_dir()));
        let file = dirs
            .iter()
            .flat_map(|d| tails.iter().map(move |t| d.join(t)))
            .find(|f| f.is_file());
        if let Some(file) = file {
            found.push(Source { app, file, format });
        }
    }
    found
}

/// The paths in one list file, newest first.
fn read_list(src: &Source) -> Result<Vec<String>, String> {
    let meta = std::fs::metadata(&src.file).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("not a settings file".into());
    }
    let text = std::fs::read_to_string(&src.file).map_err(|e| e.to_string())?;
    let (value, key): (serde_json::Value, &str) = match src.format {
        Format::UiJson => (
            serde_json::from_str(&text).map_err(|e| e.to_string())?,
            "recent",
        ),
        Format::PdfRon => {
            let map: BTreeMap<String, String> = ron::from_str(&text).map_err(|e| e.to_string())?;
            let app = map.get("app").ok_or("no \"app\" entry")?;
            (
                serde_json::from_str(app).map_err(|e| e.to_string())?,
                "recent",
            )
        }
    };
    let items = value
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or("no \"recent\" list")?;
    Ok(items
        .iter()
        .filter_map(|i| match src.format {
            Format::UiJson => i.as_str(),
            Format::PdfRon => i.get("path").and_then(|p| p.as_str()),
        })
        .filter(|p| !p.is_empty() && !p.contains('\0'))
        .map(str::to_string)
        .collect())
}

/// The recent files of all `sources`, newest first, without duplicates, only those `exists` accepts, at most `limit`.
/// The second value holds one line per list that had to be skipped.
pub fn merge(
    sources: &[Source],
    limit: usize,
    exists: &dyn Fn(&Path) -> bool,
) -> (Vec<Recent>, Vec<String>) {
    let mut all: Vec<Recent> = Vec::new();
    let mut warnings = Vec::new();
    for src in sources {
        match read_list(src) {
            Ok(list) => {
                let saved = std::fs::metadata(&src.file)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                all.extend(list.into_iter().enumerate().map(|(i, p)| Recent {
                    path: PathBuf::from(p),
                    app: src.app,
                    ts: saved.saturating_sub(i as u64),
                }));
            }
            Err(e) => warnings.push(format!("{}: {e}; skipped", src.file.display())),
        }
    }
    all.sort_by(|a, b| b.ts.cmp(&a.ts).then(a.app.cmp(&b.app)));
    // Windows paths are not case-sensitive
    let key = |p: &Path| {
        if cfg!(windows) {
            p.to_string_lossy().to_lowercase()
        } else {
            p.to_string_lossy().into_owned()
        }
    };
    let mut seen = HashSet::new();
    let out = all
        .into_iter()
        .filter(|r| seen.insert(key(&r.path)) && exists(&r.path))
        .take(limit)
        .collect();
    (out, warnings)
}

/// The recent files of this user: the four apps' lists in their usual folders, as many as `settings.recent.limit` allows.
pub fn recent() -> (Vec<Recent>, Vec<String>) {
    let limit = crate::settings::load().recent_limit as usize;
    let Some(base) = crate::config_base() else {
        return (Vec::new(), Vec::new());
    };
    let profile = std::env::var_os("WORDCRAFT_PROFILE_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    merge(&sources_in(&base, profile.as_deref()), limit, &|p| {
        p.is_file()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::tests::tmp;
    use std::fs;
    use std::time::{Duration, SystemTime};

    fn upstream(rel: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../upstream")
            .join(rel);
        fs::read_to_string(&p).unwrap_or_else(|e| {
            panic!(
                "{}: {e} (the contract tests read the vendored upstream sources)",
                p.display()
            )
        })
    }

    /// The text of `pub struct <name>` up to its closing brace.
    fn struct_text(src: &str, header: &str) -> String {
        let at = src
            .find(header)
            .unwrap_or_else(|| panic!("`{header}` not found"));
        // the attributes just above it belong to the contract (serde rename)
        let from = src[..at].rfind("\n\n").map_or(0, |i| i + 2);
        let end = src[at..].find("\n}\n").map_or(src.len(), |i| at + i);
        src[from..end].to_string()
    }

    fn write(dir: &Path, rel: &str, text: &str, age_secs: u64) -> PathBuf {
        let f = dir.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, text).unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_secs);
        fs::OpenOptions::new()
            .write(true)
            .open(&f)
            .unwrap()
            .set_modified(t)
            .unwrap();
        f
    }

    fn ui_json(paths: &[&Path]) -> String {
        let list: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        serde_json::json!({"tab": "Home", "zoom": 1.0, "recent": list}).to_string()
    }

    fn pdf_ron(paths: &[&Path]) -> String {
        let list: Vec<_> = paths
            .iter()
            .map(|p| serde_json::json!({"name": "x.pdf", "path": p.to_string_lossy(), "pages": 3, "size": 100}))
            .collect();
        let app = serde_json::json!({"recent": list, "reopen_last_session": false}).to_string();
        let map: BTreeMap<String, String> = [
            ("app".to_string(), app),
            ("egui".to_string(), "()".to_string()),
        ]
        .into();
        ron::to_string(&map).unwrap()
    }

    #[test]
    fn recent_reads_wordcraft() {
        let ui = upstream("wordcraft/crates/ui-egui/src/lib.rs");
        let st = struct_text(&ui, "pub struct UiState");
        assert!(
            st.contains("rename_all = \"camelCase\"") && st.contains("pub recent: Vec<String>,"),
            "UiState changed shape:\n{st}"
        );
        assert!(upstream("wordcraft/apps/wordcraft/src/main.rs").contains("join(\"ui.json\")"));
        let ck = upstream("wordcraft/crates/control-key/src/lib.rs");
        assert!(ck.contains("join(\"WordCraft\")") && ck.contains("WORDCRAFT_PROFILE_DIR"));
        let base = tmp("rc-wc");
        let doc = write(&base, "doc.docx", "x", 0);
        write(&base, "WordCraft/ui.json", &ui_json(&[&doc]), 10);
        let s = sources_in(&base, None);
        assert_eq!(s.len(), 1);
        let r = merge(&s, 10, &|_| true).0;
        assert_eq!((r[0].app, r[0].path.clone()), (App::Docs, doc));
        // the isolated-run override wins over both folders
        let prof = tmp("rc-wc-prof");
        write(&prof, "ui.json", &ui_json(&[]), 0);
        assert_eq!(sources_in(&base, Some(&prof))[0].file, prof.join("ui.json"));
    }

    #[test]
    fn recent_reads_gridcraft() {
        let st = struct_text(
            &upstream("gridcraft/crates/ui-egui/src/lib.rs"),
            "pub struct UiState",
        );
        assert!(
            st.contains("rename_all = \"camelCase\"") && st.contains("pub recent: Vec<String>,"),
            "UiState changed shape:\n{st}"
        );
        let main = upstream("gridcraft/apps/gridcraft/src/main.rs");
        assert!(main.contains("join(\"ui.json\")") && main.contains("join(\"GridCraft\")"));
        let base = tmp("rc-gc");
        let doc = write(&base, "book.xlsx", "x", 0);
        write(&base, "GridCraft/ui.json", &ui_json(&[&doc]), 10);
        assert_eq!(
            merge(&sources_in(&base, None), 10, &|_| true).0[0].app,
            App::Sheets
        );
    }

    #[test]
    fn recent_reads_deckcraft() {
        let st = struct_text(
            &upstream("deckcraft/crates/ui-egui/src/lib.rs"),
            "pub struct UiState",
        );
        assert!(
            st.contains("rename_all = \"camelCase\"") && st.contains("pub recent: Vec<String>,"),
            "UiState changed shape:\n{st}"
        );
        assert!(
            upstream("deckcraft/crates/ui-egui/src/settings.rs")
                .contains("UI_FILE: &str = \"ui.json\"")
        );
        assert!(upstream("deckcraft/apps/deckcraft/src/main.rs").contains("join(\"DeckCraft\")"));
        let base = tmp("rc-dc");
        let doc = write(&base, "deck.pptx", "x", 0);
        write(&base, "DeckCraft/ui.json", &ui_json(&[&doc]), 10);
        assert_eq!(
            merge(&sources_in(&base, None), 10, &|_| true).0[0].app,
            App::Slides
        );
    }

    #[test]
    fn recent_reads_pdfcraft() {
        let ui = upstream("pdfcraft/crates/ui-egui/src/lib.rs");
        let rf = struct_text(&ui, "pub struct RecentFile");
        assert!(
            rf.contains("pub path: String,") && rf.contains("pub name: String,"),
            "RecentFile changed shape:\n{rf}"
        );
        assert!(
            ui.contains("pub recent: Vec<RecentFile>,") && ui.contains("\"recent\": self.recent,")
        );
        let main = upstream("pdfcraft/apps/pdfcraft/src/main.rs");
        assert!(
            main.contains("d.join(\"app.ron\")")
                && main.contains("eframe::storage_dir(\"PdfCraft\")")
        );
        let base = tmp("rc-pdf");
        let doc = write(&base, "a.pdf", "x", 0);
        write(&base, "PdfCraft/data/app.ron", &pdf_ron(&[&doc]), 10);
        let (r, w) = merge(&sources_in(&base, None), 10, &|_| true);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!((r[0].app, r[0].path.clone()), (App::Pdf, doc));
    }

    #[test]
    fn the_four_lists_merge_newest_first_without_duplicates_or_missing_files() {
        let base = tmp("rc-merge");
        let d = |n: &str| write(&base, n, "x", 0);
        let (a, b, c, p) = (d("a.docx"), d("b.xlsx"), d("c.pptx"), d("p.pdf"));
        let gone = base.join("gone.docx");
        write(&base, "WordCraft/ui.json", &ui_json(&[&a, &gone, &b]), 40); // Docs saved last but one
        write(&base, "GridCraft/ui.json", &ui_json(&[&b, &c]), 30);
        write(&base, "DeckCraft/ui.json", &ui_json(&[&c]), 20);
        write(&base, "PdfCraft/data/app.ron", &pdf_ron(&[&p]), 10); // PDF saved last
        let (r, w) = merge(&sources_in(&base, None), 20, &|f| f.is_file());
        assert!(w.is_empty(), "{w:?}");
        let names: Vec<_> = r
            .iter()
            .map(|x| x.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // pdf first (newest list), then slides' c; gridcraft's b before docs' a, b dropped as a duplicate, gone.docx dropped
        assert_eq!(names, ["p.pdf", "c.pptx", "b.xlsx", "a.docx"]);
        assert_eq!(
            merge(&sources_in(&base, None), 2, &|f| f.is_file()).0.len(),
            2
        );
        assert_eq!(
            merge(&sources_in(&base, None), 0, &|f| f.is_file()).0.len(),
            0
        );
    }

    #[test]
    fn an_apps_own_order_is_never_reversed() {
        let base = tmp("rc-order");
        let files: Vec<PathBuf> = (0..5)
            .map(|i| write(&base, &format!("f{i}.docx"), "x", 0))
            .collect();
        let refs: Vec<&Path> = files.iter().map(|f| f.as_path()).collect();
        write(&base, "WordCraft/ui.json", &ui_json(&refs), 5);
        let (r, _) = merge(&sources_in(&base, None), 20, &|_| true);
        assert_eq!(r.iter().map(|x| x.path.clone()).collect::<Vec<_>>(), files);
    }

    #[test]
    fn a_broken_list_is_skipped_with_a_warning_and_the_others_still_load() {
        let base = tmp("rc-broken");
        let ok = write(&base, "ok.xlsx", "x", 0);
        write(&base, "WordCraft/ui.json", "{ not json", 5);
        write(&base, "PdfCraft/data/app.ron", "(((", 5);
        write(&base, "DeckCraft/ui.json", "{\"recent\": 5}", 5);
        write(&base, "GridCraft/ui.json", &ui_json(&[&ok]), 5);
        let (r, w) = merge(&sources_in(&base, None), 20, &|_| true);
        assert_eq!(r.len(), 1);
        assert_eq!(w.len(), 3, "{w:?}");
        // entries of the wrong type inside a good list are ignored too
        write(
            &base,
            "GridCraft/ui.json",
            &format!(
                "{{\"recent\": [1, null, \"\", {:?}]}}",
                ok.to_string_lossy()
            ),
            5,
        );
        assert_eq!(merge(&sources_in(&base, None), 20, &|_| true).0.len(), 1);
    }

    #[test]
    fn no_settings_folders_means_no_recent_files() {
        let base = tmp("rc-none");
        assert!(sources_in(&base, None).is_empty());
        assert_eq!(merge(&[], 20, &|_| true), (Vec::new(), Vec::new()));
    }

    #[test]
    fn the_branded_folder_wins_over_the_upstream_one() {
        let base = tmp("rc-brand");
        let a = write(&base, "a.docx", "x", 0);
        write(&base, "WordCraft/ui.json", &ui_json(&[]), 5);
        let branded = write(
            &base,
            &format!("{}/ui.json", App::Docs.brand_dir()),
            &ui_json(&[&a]),
            5,
        );
        assert_eq!(sources_in(&base, None)[0].file, branded);
    }
}
