//! The input scanner (PLAN 2.6 source 2, 5.5): reads the zip central directory only, never extracts and never parses XML.
//! It does two jobs from one pass. It rejects zip bombs and inconsistent archives (exit 9), and it counts the features that
//! the loss table (`data/losses.toml`) asks about. `shortcut: tracked changes and footnotes are not counted (that needs a
//! byte search inside word/document.xml); add them when a loss row needs them.`
use crate::exec::{Exit, Failure, fail};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_TOTAL_BYTES: u64 = 1 << 30;
pub const MAX_ENTRIES: usize = 10_000;
pub const MAX_RATIO: u64 = 100;
/// The ratio check applies to entries above this size only (small text parts compress very well and are harmless). The floor is
/// small on purpose (WP7 review M1): with a 10 MB floor, 100 entries of 9.9 MB of zeros cost a megabyte of upload and a gigabyte of
/// inflate. Entries under the floor are bounded by the entry count and by the caller's inflate budget.
pub const RATIO_FROM_BYTES: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub compressed: u64,
    /// where the entry's data starts, to catch overlapping or out-of-file entries
    pub start: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// false when the input is not a zip package (the loss table then treats every feature as possibly present)
    pub inspected: bool,
    pub features: BTreeMap<&'static str, u32>,
}

impl Scan {
    pub fn count(&self, feature: &str) -> u32 {
        self.features.get(feature).copied().unwrap_or(0)
    }
}

/// Which feature (if any) a package part name stands for.
fn feature_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.ends_with('/') {
        return None;
    }
    let base = n.rsplit('/').next().unwrap_or("");
    Some(if n.contains("/charts/chart") && n.ends_with(".xml") {
        "charts"
    } else if n.contains("/diagrams/data") {
        "diagrams"
    } else if n.contains("/embeddings/") {
        "embeddings"
    } else if (base.starts_with("comments") && base.ends_with(".xml")) || n.starts_with("ppt/comments/") {
        "comments"
    } else if base == "vbaproject.bin" {
        "macros"
    } else if (n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml") {
        "headers_footers"
    } else if n.contains("/media/") {
        "images"
    } else {
        return None;
    })
}

/// The pure core: limits first, then consistency, then the feature counts.
pub fn scan_entries(entries: &[Entry], file_len: u64) -> Result<Scan, String> {
    if entries.len() > MAX_ENTRIES {
        return Err(format!("the package holds {} parts, over the limit of {MAX_ENTRIES}", entries.len()));
    }
    let total: u64 = entries.iter().map(|e| e.size).sum();
    if total > MAX_TOTAL_BYTES {
        return Err(format!("the package expands to {} MB, over the limit of {} MB", total >> 20, MAX_TOTAL_BYTES >> 20));
    }
    for e in entries {
        if e.size > RATIO_FROM_BYTES && e.size / e.compressed.max(1) > MAX_RATIO {
            return Err(format!("part {} expands {}:1 (over {MAX_RATIO}:1), which looks like a zip bomb", e.name, e.size / e.compressed.max(1)));
        }
        if e.start.saturating_add(e.compressed) > file_len {
            return Err(format!("part {} lies beyond the end of the file (inconsistent central directory)", e.name));
        }
    }
    let mut spans: Vec<(u64, u64)> = entries.iter().filter(|e| e.compressed > 0).map(|e| (e.start, e.start + e.compressed)).collect();
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[1].0 < w[0].1) {
        return Err("parts overlap in the file (inconsistent central directory)".into());
    }
    let mut scan = Scan { inspected: true, features: BTreeMap::new() };
    for e in entries {
        if let Some(f) = feature_of(&e.name) {
            *scan.features.entry(f).or_insert(0) += 1;
        }
    }
    Ok(scan)
}

/// A scan result bound to the content it was made from: the API scans an upload once at `POST` and hands the verdict to the job,
/// which re-hashes its staged copy and skips the second inflate when the hash is the same (WP7 review M1).
#[derive(Debug, Clone)]
pub struct Verdict {
    pub sha256: String,
    pub scan: Scan,
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Scan a file by content: anything that is not a zip package comes back as `inspected: false`.
pub fn scan(path: &Path) -> Result<Scan, Failure> {
    let mut unlimited = u64::MAX;
    scan_with(path, &mut unlimited)
}

/// `scan` that charges every measured inflated byte to `budget` (the API gives one budget to a whole upload). A file that would
/// overdraw it is rejected like a bomb, and so is every later file of the same upload.
pub fn scan_with(path: &Path, budget: &mut u64) -> Result<Scan, Failure> {
    let reject = |m: String| fail(Exit::Rejected, m);
    let Ok(mut f) = File::open(path) else { return Ok(Scan::default()) };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() || &magic != b"PK\x03\x04" {
        return Ok(Scan::default());
    }
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut z = match zip::ZipArchive::new(f) {
        Ok(z) => z,
        Err(e) => return reject(format!("the zip directory is unreadable or inconsistent: {e}")),
    };
    if z.len() > MAX_ENTRIES {
        return reject(format!("the package holds {} parts, over the limit of {MAX_ENTRIES}", z.len()));
    }
    let mut entries = Vec::with_capacity(z.len());
    for i in 0..z.len() {
        match z.by_index_raw(i) {
            Ok(e) => entries.push(Entry { name: e.name().map(|n| n.into_owned()).unwrap_or_default(), size: e.size(), compressed: e.compressed_size(), start: e.data_start().unwrap_or(u64::MAX) }),
            Err(e) => return reject(format!("part {i} of the zip directory is inconsistent: {e}")),
        }
    }
    // What the headers declare is checked first and costs nothing: a bomb that declares its size is refused before one byte is inflated.
    scan_entries(&entries, len).or_else(reject)?;
    // The headers are attacker-controlled, so the sizes that count are the ones measured while inflating (fail closed:
    // a part that cannot be inflated, fails its CRC, or outgrows its declared size rejects the file).
    let mut used = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    for (i, e) in entries.iter_mut().enumerate() {
        if e.name.ends_with('/') {
            continue;
        }
        let mut r = match z.by_index(i) {
            Ok(r) => r,
            Err(err) => return reject(format!("part {} cannot be opened: {err}", e.name)),
        };
        let mut n = 0u64;
        loop {
            let got = match r.read(&mut buf) {
                Ok(0) => break,
                Ok(g) => g as u64,
                Err(err) => return reject(format!("part {} is corrupt or its sizes lie: {err}", e.name)),
            };
            n += got;
            if n > e.size || used + n > MAX_TOTAL_BYTES {
                return reject(format!("part {} inflates beyond its declared size or the {} MB limit", e.name, MAX_TOTAL_BYTES >> 20));
            }
            if *budget < got {
                *budget = 0;
                return reject("the upload expands more than this server allows for one request (inflate budget used up)".into());
            }
            *budget -= got;
        }
        used += n;
        e.size = n;
    }
    scan_entries(&entries, len).or_else(reject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn e(name: &str, size: u64, compressed: u64, start: u64) -> Entry {
        Entry { name: name.into(), size, compressed, start }
    }

    #[test]
    fn limits_are_exit_9_material() {
        let ok = [e("word/document.xml", 1000, 300, 0)];
        assert!(scan_entries(&ok, 1000).is_ok());
        assert!(scan_entries(&[e("a", 600 << 20, 100 << 20, 0), e("b", 600 << 20, 100 << 20, 100 << 20)], 300 << 20).unwrap_err().contains("expands"));
        assert!(scan_entries(&[e("big.xml", 20 << 20, 100_000, 0)], 100_000).unwrap_err().contains("zip bomb"));
        // a small part with a huge ratio is harmless, one over the floor is not
        assert!(scan_entries(&[e("small.xml", 512 << 10, 100, 0)], 100).is_ok());
        assert!(scan_entries(&[e("mid.xml", 5 << 20, 100, 0)], 100).unwrap_err().contains("zip bomb"));
        let many: Vec<Entry> = (0..=MAX_ENTRIES as u64).map(|i| e(&format!("p{i}"), 1, 1, i)).collect();
        assert!(scan_entries(&many, 20_000).unwrap_err().contains("parts"));
        assert!(scan_entries(&[e("a", 10, 10, 0), e("b", 10, 10, 5)], 100).unwrap_err().contains("overlap"));
        assert!(scan_entries(&[e("a", 10, 10, 95)], 100).unwrap_err().contains("beyond"));
    }

    #[test]
    fn features_are_counted_by_part_name() {
        let names = [
            "word/document.xml", "word/comments.xml", "xl/comments1.xml", "ppt/comments/comment1.xml", "xl/charts/chart1.xml",
            "word/diagrams/data1.xml", "word/embeddings/oleObject1.bin", "word/vbaProject.bin", "word/header1.xml",
            "word/footer2.xml", "word/media/image1.png", "word/media/", "word/_rels/document.xml.rels",
        ];
        let v: Vec<Entry> = names.iter().enumerate().map(|(i, n)| e(n, 10, 10, i as u64 * 10)).collect();
        let s = scan_entries(&v, 1000).unwrap();
        let c = |f| s.count(f);
        assert_eq!((c("comments"), c("charts"), c("diagrams"), c("embeddings"), c("macros"), c("headers_footers"), c("images")), (3, 1, 1, 1, 1, 2, 1));
    }

    #[test]
    fn a_real_zip_and_a_broken_one() {
        let dir = std::env::temp_dir().join(format!("convert-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.docx");
        let mut w = zip::ZipWriter::new(File::create(&good).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for n in ["word/document.xml", "word/comments.xml"] {
            w.start_file(n, o).unwrap();
            w.write_all(b"<x/>").unwrap();
        }
        w.finish().unwrap();
        let s = scan(&good).unwrap();
        assert!(s.inspected);
        assert_eq!(s.count("comments"), 1);
        let bad = dir.join("bad.docx");
        std::fs::write(&bad, b"PK\x03\x04\x14\x00\x00\x00\x08\x00partial").unwrap();
        assert_eq!(scan(&bad).unwrap_err().exit, Exit::Rejected);
        // a bomb, honest (11 MB of zeros in a few KB) and lying (the central directory claims 1000 bytes)
        let bomb = dir.join("bomb.docx");
        let mut w = zip::ZipWriter::new(File::create(&bomb).unwrap());
        w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&vec![0u8; 11 << 20]).unwrap();
        w.finish().unwrap();
        assert!(scan(&bomb).unwrap_err().msg.contains("zip bomb"));
        let mut bytes = std::fs::read(&bomb).unwrap();
        let cd = bytes.windows(4).rposition(|w| w == b"PK").unwrap();
        bytes[cd + 24..cd + 28].copy_from_slice(&1000u32.to_le_bytes());
        let lie = dir.join("lie.docx");
        std::fs::write(&lie, &bytes).unwrap();
        assert_eq!(scan(&lie).unwrap_err().exit, Exit::Rejected);
        let txt = dir.join("a.txt");
        std::fs::write(&txt, b"hello").unwrap();
        assert!(!scan(&txt).unwrap().inspected);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
