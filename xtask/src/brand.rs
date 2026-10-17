//! `cargo xt overlay|brand ...`: brand config, overlay rendering, brand-leak checks (WP7, docs/phase1/designs/brand.md).
//!
//! Commands:
//!   overlay <docs|sheets|slides|pdf|all>   write $OFFICE_OVERLAY_ROOT/<app>/ (overlay.env, icons, fallback fonts)
//!   brand check                            parse and validate brand/brand.toml
//!   brand icons                            alias of `overlay all` (renders the placeholder icons)
//!   brand-grep                             source scan: working name only in brand.toml (rule W), no brand marks (rule M)
//!   brand-scan-bin [file...]               binary scan of release exes and web wasm (default: the Phase 1 build outputs)

use office_brand::Brand;
use std::fs;
use std::path::{Path, PathBuf};

const APPS: [&str; 4] = ["docs", "sheets", "slides", "pdf"];

fn overlay_root() -> PathBuf {
    PathBuf::from(std::env::var("OFFICE_OVERLAY_ROOT").unwrap_or_else(|_| "E:/office-target/p1/overlay".into()))
}

fn load(root: &Path) -> Result<Brand, String> {
    Brand::load(&root.join("brand/brand.toml"))
}

pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    match args {
        ["overlay", which] | ["brand", "icons", which] => overlay(root, which),
        ["brand", "icons"] => overlay(root, "all"),
        ["brand", "check"] => {
            let b = load(root)?;
            Ok(format!("brand.toml OK: product '{}' v{}, {} apps", b.id, b.version, b.apps.len()))
        }
        ["brand-grep"] => brand_grep(root),
        ["brand-scan-bin", files @ ..] => scan_bin(root, files),
        _ => Err("usage: cargo xt overlay <app|all> | brand check | brand icons | brand-grep | brand-scan-bin [file...]".into()),
    }
}

fn overlay(root: &Path, which: &str) -> Result<String, String> {
    let b = load(root)?;
    let apps: Vec<&str> = if which == "all" { APPS.to_vec() } else { vec![which] };
    let mut msgs = Vec::new();
    for a in apps {
        msgs.push(b.write_overlay(a, &root.join("assets/fonts"), &overlay_root().join(a))?);
    }
    Ok(msgs.join("\n"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" || name == "node_modules" {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whole-word, case-sensitive occurrence.
fn has_word(text: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let s = from + i;
        let e = s + word.len();
        let before = text[..s].chars().next_back().is_none_or(|c| !is_word_char(c));
        let after = text[e..].chars().next().is_none_or(|c| !is_word_char(c));
        if before && after {
            return true;
        }
        from = e;
    }
    false
}

fn brand_grep(root: &Path) -> Result<String, String> {
    let b = load(root)?;
    let mut names: Vec<String> = vec![b.name.clone()];
    for a in &b.apps {
        names.extend(a.names.values().cloned());
    }
    names.sort();
    names.dedup();
    let mut hits = Vec::new();
    let mut scanned = 0usize;

    // Rule W: the working name appears only in brand/brand.toml (never hard-coded).
    for dir in ["patches", "domains", "crates", "i18n", "scripts", "xtask"] {
        let mut files = Vec::new();
        walk(&root.join(dir), &mut files);
        for f in files {
            let r = rel(root, &f);
            // i18n/review/ carries the names to the owner by design; domains/*/apps/*-web/index.html is rendered from
            // index.html.in by scripts/build.sh (and git-ignored).
            if r.starts_with("i18n/review/") || (r.starts_with("domains/") && r.ends_with("/index.html")) {
                continue;
            }
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !["rs", "toml", "sh", "md", "html", "in", "json", "tsv", "patch", "yml", "txt", "env"].contains(&ext) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&f) else { continue };
            scanned += 1;
            for (i, line) in text.lines().enumerate() {
                if let Some(n) = names.iter().find(|n| has_word(line, n)) {
                    hits.push(format!("{r}:{}: working name '{n}' outside brand.toml [rule W]", i + 1));
                }
            }
        }
    }

    // Rule M: no include_bytes!/include_str! of the upstream brand marks, except under cfg(not(office_overlay)).
    let mut files = Vec::new();
    for dir in ["upstream", "domains", "crates", "xtask"] {
        walk(&root.join(dir), &mut files);
    }
    let marker = ["docs", "brand"].join("/");
    for f in files.iter().filter(|f| f.extension().and_then(|e| e.to_str()) == Some("rs")) {
        let Ok(text) = fs::read_to_string(f) else { continue };
        if !text.contains(&marker) {
            continue;
        }
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.contains(&marker) && (l.contains("include_bytes!") || l.contains("include_str!")) {
                let from = i.saturating_sub(15);
                let gated = lines[from..i].iter().any(|p| p.contains("cfg(not(office_overlay))"));
                if !gated {
                    hits.push(format!("{}:{}: brand mark included without cfg(not(office_overlay)) [rule M]", rel(root, f), i + 1));
                }
            }
        }
    }

    if hits.is_empty() {
        Ok(format!("brand-grep: {scanned} files scanned (rule W), rule M checked, 0 unallowlisted hits"))
    } else {
        for h in &hits {
            eprintln!("{h}");
        }
        Err(format!("brand-grep: {} hit(s)", hits.len()))
    }
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || hay.len() < needle.len() {
        return 0;
    }
    let (mut n, mut i) = (0, 0);
    while i + needle.len() <= hay.len() {
        match hay[i..].windows(needle.len()).position(|w| w == needle) {
            Some(p) => {
                n += 1;
                i += p + needle.len();
            }
            None => break,
        }
    }
    n
}

fn scan_bin(root: &Path, files: &[&str]) -> Result<String, String> {
    let target = PathBuf::from(std::env::var("OFFICE_TARGET").unwrap_or_else(|_| "E:/office-target/p1".into()));
    let mut list: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    if list.is_empty() {
        let b = load(root)?;
        for d in APPS {
            let exe = target.join(d).join("release").join(format!("{d}.exe"));
            if exe.exists() {
                list.push(exe);
            }
            let mut dist = Vec::new();
            walk(&target.join("dist").join(format!("{d}-web")), &mut dist);
            list.extend(dist.into_iter().filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wasm")));
        }
        let _ = b;
    }
    if list.is_empty() {
        return Err("brand-scan-bin: no binaries found (build with scripts/build.sh first)".into());
    }
    // Signatures of the upstream brand marks: 48 bytes from the middle of each svg.
    let mut sigs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut svgs = Vec::new();
    for r in ["wordcraft", "gridcraft", "deckcraft", "pdfcraft"] {
        walk(&root.join("upstream").join(r).join("docs").join("brand"), &mut svgs);
    }
    for s in svgs.iter().filter(|p| p.extension().and_then(|e| e.to_str()) == Some("svg")) {
        if let Ok(bytes) = fs::read(s) {
            if bytes.len() > 160 {
                let mid = bytes.len() / 2;
                sigs.push((rel(root, s), bytes[mid..mid + 48].to_vec()));
            }
        }
    }
    let forbidden = ["discord.gg", "getartcraft", "github.com/storytold", "api.github.com/repos/storytold", "storyteller.ai"];
    let info = ["WordCraft", "GridCraft", "DeckCraft", "PdfCraft", "ArtCraft"];
    let mut fails = 0;
    let mut out = Vec::new();
    for f in &list {
        let bytes = fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let mut line = format!("{}: {} bytes;", f.display(), bytes.len());
        for t in forbidden {
            let n = count(&bytes, t.as_bytes());
            if n > 0 {
                fails += 1;
                line.push_str(&format!(" LEAK {t}={n};"));
            }
        }
        for (name, sig) in &sigs {
            if count(&bytes, sig) > 0 {
                fails += 1;
                line.push_str(&format!(" LEAK mark {name};"));
            }
        }
        let infos: Vec<String> = info.iter().map(|t| format!("{t}={}", count(&bytes, t.as_bytes()))).collect();
        line.push_str(&format!(" info[{}]", infos.join(" ")));
        out.push(line);
    }
    let report = out.join("\n");
    if fails > 0 {
        eprintln!("{report}");
        Err(format!("brand-scan-bin: {fails} leak(s) in {} file(s)", list.len()))
    } else {
        Ok(format!("{report}\nbrand-scan-bin: 0 leaks in {} file(s)", list.len()))
    }
}
