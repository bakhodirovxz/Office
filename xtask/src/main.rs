//! `cargo xt ...`: repository tooling (PLAN section 1.2). Std-only for now; later WPs add subcommands.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["register", "check"] => register_check(&repo_root()),
        _ => Err("usage: cargo xt register check".to_string()),
    };
    match result {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The repository root: the parent of the xtask crate directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask has a parent directory").to_path_buf()
}

const COLUMNS: [&str; 11] = [
    "id", "file", "subject", "files / +/\u{2212}", "category", "why", "measured effect", "upstream PR", "carried since", "last rebase",
    "drop when",
];
const CATEGORIES: [&str; 8] = ["mobile", "memory", "pdf", "brand", "i18n", "font", "ci-fix", "parked"];

/// Every `patches/<repo>/NNNN-*.patch` must have exactly one row in `patches/REGISTER.md` and the other way round.
fn register_check(root: &Path) -> Result<String, String> {
    let text = fs::read_to_string(root.join("patches/REGISTER.md")).map_err(|e| format!("patches/REGISTER.md: {e}"))?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut header_seen = false;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = line.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':') && !c.is_empty()) {
            continue; // separator row
        }
        if !header_seen {
            if cells.len() != COLUMNS.len() || cells[0] != "id" {
                return Err(format!("REGISTER.md:{}: header must have the {} fixed columns", i + 1, COLUMNS.len()));
            }
            header_seen = true;
            continue;
        }
        if cells.len() != COLUMNS.len() {
            return Err(format!("REGISTER.md:{}: {} cells, expected {}", i + 1, cells.len(), COLUMNS.len()));
        }
        for (c, name) in cells.iter().zip(COLUMNS) {
            if c.is_empty() {
                return Err(format!("REGISTER.md:{}: empty cell in column '{name}'", i + 1));
            }
        }
        if !CATEGORIES.contains(&cells[4].as_str()) {
            return Err(format!("REGISTER.md:{}: unknown category '{}'", i + 1, cells[4]));
        }
        rows.push(cells);
    }
    if !header_seen {
        return Err("REGISTER.md: no table found".into());
    }

    // rows -> files
    let mut row_files = BTreeSet::new();
    for r in &rows {
        let file = r[1].trim_matches('`').to_string();
        let Some((repo, num)) = r[0].split_once('/') else { return Err(format!("row id '{}' must be <repo>/<NNNN>", r[0])) };
        if !file.starts_with(num) {
            return Err(format!("row '{}': file '{file}' does not start with its number", r[0]));
        }
        let rel = format!("{repo}/{file}");
        let path = root.join("patches").join(&rel);
        let path = if path.exists() { path } else { root.join("patches").join(repo).join("parked").join(&file) };
        if !path.exists() {
            return Err(format!("row '{}' has no file: patches/{rel}", r[0]));
        }
        if !row_files.insert(rel.clone()) {
            return Err(format!("duplicate row for patches/{rel}"));
        }
    }

    // files -> rows
    let mut files = BTreeSet::new();
    let patches = root.join("patches");
    for repo in fs::read_dir(&patches).map_err(|e| e.to_string())? {
        let repo = repo.map_err(|e| e.to_string())?;
        if !repo.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let rn = repo.file_name().to_string_lossy().to_string();
        for dir in [repo.path(), repo.path().join("parked")] {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                let numbered = n.len() > 5 && n[..4].chars().all(|c| c.is_ascii_digit()) && n.as_bytes()[4] == b'-';
                if numbered && n.ends_with(".patch") {
                    files.insert(format!("{rn}/{n}"));
                }
            }
        }
    }
    for f in &files {
        if !row_files.contains(f) {
            return Err(format!("patch file has no row: patches/{f}"));
        }
    }
    Ok(format!("{} patches, {} rows, OK", files.len(), rows.len()))
}
