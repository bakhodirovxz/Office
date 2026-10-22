//! The shared fallback fonts (PLAN 5.3): one Noto Sans subset (and a mono one) that covers U+02BB, U+02BC and Cyrillic, which the
//! four apps and the shell use when a document's own font lacks a glyph. The file names come from `brand.toml` `[fonts]`.
//!
//! Where they are, first hit wins:
//! 1. `SUITE_FONTS_DIR` (a folder that has the `fallback/` folder; for tests and unusual installs),
//! 2. `fonts/` and `assets/fonts/` next to the running exe (the installer puts them there once, for all apps),
//! 3. `assets/fonts/` in the repository, found by walking up from the exe (a development build).

use std::path::{Path, PathBuf};

/// The sans fallback, relative to the fonts folder (`brand.toml` `fonts.fallback_sans`).
pub const SANS: &str = env!("SUITE_FONT_SANS");
/// The mono fallback, relative to the fonts folder (`brand.toml` `fonts.fallback_mono`).
pub const MONO: &str = env!("SUITE_FONT_MONO");

/// The folders to look in, best first.
pub fn roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = std::env::var_os("SUITE_FONTS_DIR").filter(|d| !d.is_empty()) {
        v.push(PathBuf::from(d));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        v.push(exe_dir.join("fonts"));
        v.push(exe_dir.join("assets/fonts"));
        v.extend(
            exe_dir
                .ancestors()
                .skip(1)
                .take(6)
                .map(|a| a.join("assets/fonts")),
        );
    }
    v
}

/// The first of `roots` that has `rel` as a file.
pub fn find_in(roots: &[PathBuf], rel: &str) -> Option<PathBuf> {
    roots.iter().map(|r| r.join(rel)).find(|p| p.is_file())
}

pub fn fallback_sans() -> Option<PathBuf> {
    find_in(&roots(), SANS)
}

pub fn fallback_mono() -> Option<PathBuf> {
    find_in(&roots(), MONO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::tests::tmp;

    fn repo_fonts() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")
    }

    #[test]
    fn the_names_from_brand_toml_are_the_files_in_the_repository() {
        for rel in [SANS, MONO] {
            let p = repo_fonts().join(rel);
            assert!(p.is_file(), "{} (brand.toml [fonts])", p.display());
            assert_eq!(find_in(&[repo_fonts()], rel), Some(p));
        }
    }

    #[test]
    fn the_sans_font_is_a_font_and_not_a_stub() {
        // The cmap is not parsed here: scripts/subset-fallback-font.sh fails if a letter (U+02BB, U+02BC, Cyrillic) is missing.
        let bytes = std::fs::read(repo_fonts().join(SANS)).unwrap();
        assert!(bytes.len() > 10_000, "{} bytes", bytes.len());
        assert!(
            matches!(&bytes[..4], [0, 1, 0, 0] | b"true" | b"OTTO"),
            "not a TrueType or OpenType file"
        );
    }

    #[test]
    fn the_first_root_with_the_file_wins() {
        let (a, b) = (tmp("font-a"), tmp("font-b"));
        std::fs::create_dir_all(b.join("fallback")).unwrap();
        std::fs::write(b.join(SANS), b"x").unwrap();
        assert_eq!(find_in(&[a.clone(), b.clone()], SANS), Some(b.join(SANS)));
        std::fs::create_dir_all(a.join("fallback")).unwrap();
        std::fs::write(a.join(SANS), b"y").unwrap();
        assert_eq!(find_in(&[a.clone(), b], SANS), Some(a.join(SANS)));
        assert_eq!(find_in(&[a], MONO), None);
    }
}
