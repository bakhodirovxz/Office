//! The pre-open guard for workbooks (PLAN phase 4, section 6, item 3; F18). Before the host starts Sheets on an `.xlsx` or `.xlsm`
//! it reads the zip central directory with the Phase 3 scanner (`convert::scan::directory`, nothing is inflated) and decides
//! whether the open is safe to start:
//! - `Wall`: Sheets cannot open the workbook yet (its parts add up to over 1 GiB, or one part is over 512 MiB, the limits of
//!   `gridcraft/crates/xlsx/src/package.rs`). The host does not start the editor and says so; it offers Excel.
//! - `Links`: `xl/externalLinks/` is present. Sheets opens the file and keeps the links, but never refreshes them; the host says so.
//!
//! Everything else (not a zip, a damaged directory, a bomb-like ratio) is left to the editor: it already refuses those with its own
//! message. The host never reads a part, so the check costs one directory read even for a 150 MB file.
use std::path::{Path, PathBuf};

use convert::scan;

/// The most one part may inflate to in Sheets (`MAX_PART` in gridcraft's `package.rs`).
pub const MAX_PART: u64 = 512 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Clear,
    /// External links are present: kept, never updated.
    Links,
    /// Too large for Sheets today.
    Wall,
}

impl Verdict {
    /// The code the page translates (`guard.<code>` in `web/shell/i18n.json`).
    pub fn code(self) -> Option<&'static str> {
        match self {
            Verdict::Clear => None,
            Verdict::Links => Some("links"),
            Verdict::Wall => Some("wall"),
        }
    }
}

/// The English texts the host's own dialogs show (the page shows its translations of the same sentences).
pub const WALL_TEXT: &str = "This workbook is larger than Sheets can open yet. Open it in Excel.";
pub const LINKS_TEXT: &str = "Links to other workbooks are kept but not updated.";

/// The decision from a zip directory alone. The wall counts what the parts add up to once inflated, as the editor does.
pub fn verdict_of(entries: &[scan::Entry]) -> Verdict {
    let total: u64 = entries.iter().map(|e| e.size).sum();
    if total > scan::MAX_TOTAL_BYTES || entries.iter().any(|e| e.size > MAX_PART) {
        Verdict::Wall
    } else if entries
        .iter()
        .any(|e| e.name.to_ascii_lowercase().starts_with("xl/externallinks/"))
    {
        Verdict::Links
    } else {
        Verdict::Clear
    }
}

/// Only Sheets' zip workbooks are looked at.
pub fn check(app: &str, path: &Path) -> Verdict {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if app != "sheets" || !matches!(ext.as_str(), "xlsx" | "xlsm") {
        return Verdict::Clear;
    }
    scan::directory(path).map_or(Verdict::Clear, |(entries, _)| verdict_of(&entries))
}

/// Excel's executable, when it is installed: the `App Paths` entry that Windows keeps for `excel.exe`. Debug builds honour
/// `SUITE_EXCEL` (a test stand-in), like the host's other `SUITE_*` hooks.
pub fn excel_exe() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("SUITE_EXCEL") {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    #[cfg(windows)]
    {
        let reg = Path::new(&std::env::var_os("SystemRoot")?)
            .join("System32")
            .join("reg.exe");
        for root in ["HKLM", "HKCU"] {
            for view in ["/reg:64", "/reg:32"] {
                let key = format!(
                    r"{root}\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe"
                );
                let out = std::process::Command::new(&reg)
                    .args(["query", &key, "/ve", view])
                    .output()
                    .ok()?;
                // "    (Default)    REG_SZ    C:\Program Files\...\EXCEL.EXE"
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(p) = text.lines().find_map(|l| {
                    l.split_once("REG_SZ")
                        .map(|(_, v)| v.trim().trim_matches('"'))
                }) && Path::new(p).is_file()
                {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn entry(name: &str, size: u64) -> scan::Entry {
        scan::Entry {
            name: name.into(),
            size,
            compressed: 1,
            start: 0,
        }
    }

    fn many(size: u64, n: usize) -> Vec<scan::Entry> {
        (0..n)
            .map(|i| entry(&format!("xl/worksheets/sheet{i}.xml"), size))
            .collect()
    }

    #[test]
    fn the_wall_is_the_editors_own_limit() {
        let part = 300 << 20;
        assert_eq!(
            verdict_of(&many(part, 3)),
            Verdict::Clear,
            "900 MiB is below the wall"
        );
        assert_eq!(
            verdict_of(&many(part, 4)),
            Verdict::Wall,
            "1.2 GiB is over it"
        );
        assert_eq!(
            verdict_of(&[entry("xl/worksheets/sheet1.xml", MAX_PART + 1)]),
            Verdict::Wall,
            "one part over 512 MiB"
        );
        assert_eq!(
            verdict_of(&[entry("xl/worksheets/sheet1.xml", MAX_PART)]),
            Verdict::Clear
        );
        assert_eq!(
            verdict_of(&[entry("xl/externalLinks/externalLink1.xml", 3 << 30)]),
            Verdict::Wall,
            "the wall wins over links"
        );
    }

    #[test]
    fn external_links_are_a_warning() {
        let one = [
            entry("xl/workbook.xml", 10),
            entry("xl/externalLinks/externalLink1.xml", 10),
        ];
        assert_eq!(verdict_of(&one), Verdict::Links);
        assert_eq!(
            verdict_of(&[entry("XL/ExternalLinks/_rels/externalLink1.xml.rels", 5)]),
            Verdict::Links,
            "case does not matter"
        );
        assert_eq!(
            verdict_of(&[
                entry("xl/workbook.xml", 10),
                entry("xl/media/externalLinks.png", 10)
            ]),
            Verdict::Clear
        );
    }

    fn zip_with(path: &Path, parts: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).expect("create"));
        for (name, bytes) in parts {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .expect("start");
            z.write_all(bytes).expect("write");
        }
        z.finish().expect("finish");
    }

    #[test]
    fn a_real_file_is_read_from_its_directory_only() {
        let dir = std::env::temp_dir().join(format!("guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let linked = dir.join("linked.xlsx");
        zip_with(
            &linked,
            &[
                ("xl/workbook.xml", b"<workbook/>"),
                ("xl/externalLinks/externalLink1.xml", b"<externalLink/>"),
            ],
        );
        let plain = dir.join("plain.xlsx");
        zip_with(&plain, &[("xl/workbook.xml", b"<workbook/>")]);
        assert_eq!(check("sheets", &linked), Verdict::Links);
        assert_eq!(check("sheets", &plain), Verdict::Clear);
        assert_eq!(
            check("docs", &linked),
            Verdict::Clear,
            "only Sheets is guarded"
        );
        let csv = dir.join("a.csv");
        std::fs::write(&csv, "a,b").expect("csv");
        assert_eq!(
            check("sheets", &csv),
            Verdict::Clear,
            "not a workbook package"
        );
        std::fs::write(dir.join("broken.xlsx"), b"PK\x03\x04 not a zip").expect("broken");
        assert_eq!(
            check("sheets", &dir.join("broken.xlsx")),
            Verdict::Clear,
            "a damaged package is the editor's to refuse"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
