//! The C ABI against the engine: the golden list through `uzt_convert`, split invariance through `uzt_convert_runs`,
//! and the error contract (bad args, bad UTF-16, null outputs).
use std::{fs, path::Path, ptr};

use uz_translit::{Direction, LatinAlphabet, Options, convert};
use uz_translit_ffi::*;

fn ffi(s: &str, to: u32, alphabet: u32, flags: u32) -> Result<String, i32> {
    let src: Vec<u16> = s.encode_utf16().collect();
    let (mut out, mut n) = (ptr::null_mut(), 0u32);
    let rc = unsafe {
        uzt_convert(
            src.as_ptr(),
            src.len() as u32,
            to,
            alphabet,
            flags,
            &mut out,
            &mut n,
        )
    };
    if rc != UZT_OK {
        assert!(out.is_null() && n == 0, "outputs stay empty on error");
        return Err(rc);
    }
    let res = if n == 0 {
        String::new()
    } else {
        String::from_utf16(unsafe { std::slice::from_raw_parts(out, n as usize) }).unwrap()
    };
    unsafe { uzt_free(out, n) };
    Ok(res)
}

fn ffi_runs(runs: &[(&str, bool)], to: u32, alphabet: u32) -> Result<Vec<String>, i32> {
    let mut text: Vec<u16> = Vec::new();
    let mut lens = Vec::new();
    for (t, _) in runs {
        let before = text.len();
        text.extend(t.encode_utf16());
        lens.push((text.len() - before) as u32);
    }
    let conv: Vec<u8> = runs.iter().map(|r| r.1 as u8).collect();
    let mut out_lens = vec![0u32; runs.len()];
    let (mut out, mut n) = (ptr::null_mut(), 0u32);
    let rc = unsafe {
        uzt_convert_runs(
            text.as_ptr(),
            lens.as_ptr(),
            conv.as_ptr(),
            runs.len() as u32,
            to,
            alphabet,
            0,
            &mut out,
            &mut n,
            out_lens.as_mut_ptr(),
        )
    };
    if rc != UZT_OK {
        return Err(rc);
    }
    let all: &[u16] = if n == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(out, n as usize) }
    };
    let mut at = 0usize;
    let res = out_lens
        .iter()
        .map(|&l| {
            let s = String::from_utf16(&all[at..at + l as usize]).unwrap();
            at += l as usize;
            s
        })
        .collect();
    assert_eq!(at, n as usize, "out_lens add up to out_len");
    unsafe { uzt_free(out, n) };
    Ok(res)
}

/// Reviewed rows of `golden.tsv`: (id, latin, cyrillic, dir).
fn golden() -> Vec<(String, String, String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../uz-translit/data/golden.tsv");
    let text = fs::read_to_string(&path).expect("golden.tsv");
    let mut lines = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let (id, lat, cyr, dir, st) = (
        col("id"),
        col("latin"),
        col("cyrillic"),
        col("dir"),
        col("status"),
    );
    lines
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f.get(st).is_some_and(|s| s.starts_with("reviewed:")))
        .map(|f| (f[id].into(), f[lat].into(), f[cyr].into(), f[dir].into()))
        .collect()
}

#[test]
fn golden_list_through_ffi() {
    let rows = golden();
    assert!(rows.len() >= 500, "golden list too short: {}", rows.len());
    let (mut n, mut fails) = (0, Vec::new());
    for (id, lat, cyr, dir) in &rows {
        if matches!(dir.as_str(), "both" | "l2c") {
            n += 1;
            let got = ffi(lat, UZT_TO_CYRILLIC, UZT_ALPHABET_1995, 0).unwrap();
            if got != *cyr {
                fails.push(format!("{id} l2c {lat:?} -> {got:?}, want {cyr:?}"));
            }
        }
        if matches!(dir.as_str(), "both" | "c2l") {
            n += 1;
            let got = ffi(cyr, UZT_TO_LATIN, UZT_ALPHABET_1995, 0).unwrap();
            if got != *lat {
                fails.push(format!("{id} c2l {cyr:?} -> {got:?}, want {lat:?}"));
            }
        }
    }
    assert!(
        fails.is_empty(),
        "{} mismatches\n{}",
        fails.len(),
        fails.join("\n")
    );
    println!("{n} golden conversions through the FFI");
}

/// The 2026 alphabet and Auto through the FFI give exactly what the library gives (the library's own tests own
/// the expected 2026 values: tests/alphabet2026.rs, golden.rs::golden_2026).
#[test]
fn alphabets_match_library() {
    let cases = [
        (UZT_ALPHABET_AUTO, LatinAlphabet::Auto),
        (UZT_ALPHABET_1995, LatinAlphabet::Y1995),
        (UZT_ALPHABET_2026, LatinAlphabet::Y2026),
    ];
    let mut n = 0;
    for (_, lat, cyr, _) in golden() {
        for (a, la) in cases {
            let o = Options {
                latin_alphabet: la,
                ..Options::default()
            };
            assert_eq!(
                ffi(&cyr, UZT_TO_LATIN, a, 0).unwrap(),
                convert(&cyr, Direction::ToLatin, &o)
            );
            assert_eq!(
                ffi(&lat, UZT_TO_CYRILLIC, a, 0).unwrap(),
                convert(&lat, Direction::ToCyrillic, &o)
            );
            n += 2;
        }
    }
    // The 2026 letters, both ways.
    assert_eq!(
        ffi(
            "Ўзбек тили. Шаҳар, ғалла, чой",
            UZT_TO_LATIN,
            UZT_ALPHABET_2026,
            0
        )
        .unwrap(),
        "Özbek tili. Şahar, ğalla, çoy"
    );
    assert_eq!(
        ffi(
            "Özbek tili. Şahar, ğalla, çoy",
            UZT_TO_CYRILLIC,
            UZT_ALPHABET_AUTO,
            0
        )
        .unwrap(),
        "Ўзбек тили. Шаҳар, ғалла, чой"
    );
    assert_eq!(
        ffi(
            "Ўзбек",
            UZT_TO_LATIN,
            UZT_ALPHABET_1995,
            UZT_FLAG_ASCII_APOSTROPHE
        )
        .unwrap(),
        "O'zbek"
    );
    println!("{n} conversions match the library");
}

/// Splitting a paragraph into runs anywhere never changes the result (the Word adapter relies on it: it splits runs
/// at bookmarks, comments and control characters).
#[test]
fn runs_split_invariance() {
    let mut n = 0;
    for (_, lat, cyr, _) in golden().iter().take(400) {
        for (s, to) in [(lat, UZT_TO_CYRILLIC), (cyr, UZT_TO_LATIN)] {
            let whole = ffi(s, to, UZT_ALPHABET_AUTO, 0).unwrap();
            let idx: Vec<usize> = s.char_indices().map(|(i, _)| i).skip(1).collect();
            for &k in &idx {
                let parts =
                    ffi_runs(&[(&s[..k], true), (&s[k..], true)], to, UZT_ALPHABET_AUTO).unwrap();
                assert_eq!(parts.concat(), whole, "split {s:?} at {k}");
                n += 1;
            }
        }
    }
    // A verbatim run is copied and is a hard boundary.
    let r = ffi_runs(
        &[("shahar ", true), ("KEEP sh", false), (" shahar", true)],
        UZT_TO_CYRILLIC,
        UZT_ALPHABET_AUTO,
    )
    .unwrap();
    assert_eq!(r, ["шаҳар ", "KEEP sh", " шаҳар"]);
    assert_eq!(
        ffi_runs(&[], UZT_TO_CYRILLIC, UZT_ALPHABET_AUTO).unwrap(),
        Vec::<String>::new()
    );
    println!("{n} two-run splits checked");
}

#[test]
fn error_contract() {
    assert_eq!(uzt_abi_version(), UZT_ABI_VERSION);
    let mut len = 0u32;
    let p = unsafe { uzt_rules_version(&mut len) };
    let v = std::str::from_utf8(unsafe { std::slice::from_raw_parts(p, len as usize) }).unwrap();
    assert_eq!(v, uz_translit::RULES_VERSION);

    assert_eq!(ffi("", UZT_TO_LATIN, 0, 0), Ok(String::new()));
    assert_eq!(ffi("a", 7, 0, 0), Err(UZT_BAD_ARG));
    assert_eq!(ffi("a", UZT_TO_LATIN, 9, 0), Err(UZT_BAD_ARG));
    assert_eq!(ffi("a", UZT_TO_LATIN, 0, 8), Err(UZT_BAD_ARG));

    let lone = [0x0441u16, 0xD800, 0x0430]; // с, unpaired high surrogate, а
    let (mut out, mut n) = (ptr::null_mut(), 0u32);
    let rc = unsafe { uzt_convert(lone.as_ptr(), 3, UZT_TO_LATIN, 0, 0, &mut out, &mut n) };
    assert_eq!((rc, out.is_null(), n), (UZT_BAD_UTF16, true, 0));

    let rc = unsafe { uzt_convert(ptr::null(), 3, UZT_TO_LATIN, 0, 0, &mut out, &mut n) };
    assert_eq!(rc, UZT_BAD_ARG);
    let rc = unsafe {
        uzt_convert(
            lone.as_ptr(),
            1,
            UZT_TO_LATIN,
            0,
            0,
            ptr::null_mut(),
            &mut n,
        )
    };
    assert_eq!(rc, UZT_BAD_ARG);
    let rc = unsafe {
        uzt_convert_runs(
            lone.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            0,
            0,
            0,
            &mut out,
            &mut n,
            ptr::null_mut(),
        )
    };
    assert_eq!(rc, UZT_BAD_ARG);
    unsafe { uzt_free(ptr::null_mut(), 0) };
}
