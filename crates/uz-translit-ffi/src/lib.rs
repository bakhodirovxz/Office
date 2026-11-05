//! C ABI of `uz-translit` for hosts that are not Rust (the Office COM add-in, docs/ms-addin/PLAN.md section 3).
//!
//! Contract, for every function:
//! * Strings are UTF-16 code units with an explicit length (no terminator needed). Lengths are `u32` on every target,
//!   so x86 and x64 callers declare the same signatures.
//! * Output buffers are allocated here and returned through `out`; the caller frees each one with [`uzt_free`],
//!   passing the length it got back. A zero-length result is a null pointer and needs no free.
//! * No panic crosses the boundary: a panic returns [`UZT_PANIC`] and leaves the outputs null/zero.
//! * Invalid UTF-16 (an unpaired surrogate) is rejected with [`UZT_BAD_UTF16`], never silently replaced, so the
//!   caller can leave that text untouched.
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use uz_translit::{Apostrophe, Direction, LatinAlphabet, Options, Run};

pub const UZT_OK: i32 = 0;
pub const UZT_BAD_ARG: i32 = 1;
pub const UZT_BAD_UTF16: i32 = 2;
pub const UZT_PANIC: i32 = 3;

/// `to`: the target script.
pub const UZT_TO_CYRILLIC: u32 = 0;
pub const UZT_TO_LATIN: u32 = 1;
/// `alphabet`: ->Latin writes it, ->Cyrillic reads it (uz_translit::LatinAlphabet).
pub const UZT_ALPHABET_AUTO: u32 = 0;
pub const UZT_ALPHABET_1995: u32 = 1;
pub const UZT_ALPHABET_2026: u32 = 2;
/// `flags` bits; any other bit is rejected.
pub const UZT_FLAG_ASCII_APOSTROPHE: u32 = 1;
pub const UZT_FLAG_NO_FOREIGN_GUARD: u32 = 2;
pub const UZT_FLAG_NO_ROMAN_NUMERALS: u32 = 4;

/// Bumped when a signature or a constant above changes.
pub const UZT_ABI_VERSION: u32 = 1;

fn params(to: u32, alphabet: u32, flags: u32) -> Option<(Direction, Options)> {
    let dir = match to {
        UZT_TO_CYRILLIC => Direction::ToCyrillic,
        UZT_TO_LATIN => Direction::ToLatin,
        _ => return None,
    };
    let latin_alphabet = match alphabet {
        UZT_ALPHABET_AUTO => LatinAlphabet::Auto,
        UZT_ALPHABET_1995 => LatinAlphabet::Y1995,
        UZT_ALPHABET_2026 => LatinAlphabet::Y2026,
        _ => return None,
    };
    if flags & !7 != 0 {
        return None;
    }
    let opts = Options {
        apostrophe: if flags & UZT_FLAG_ASCII_APOSTROPHE != 0 {
            Apostrophe::Ascii
        } else {
            Apostrophe::Official
        },
        latin_alphabet,
        foreign_latin_guard: flags & UZT_FLAG_NO_FOREIGN_GUARD == 0,
        roman_numerals: flags & UZT_FLAG_NO_ROMAN_NUMERALS == 0,
    };
    Some((dir, opts))
}

/// # Safety
/// `p` must point to `len` readable u16 (or be anything when `len` is 0).
unsafe fn units<'a>(p: *const u16, len: u32) -> Option<&'a [u16]> {
    if len == 0 {
        return Some(&[]);
    }
    if p.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees `len` readable units at `p`.
    Some(unsafe { std::slice::from_raw_parts(p, len as usize) })
}

/// Hands `v` to the caller; null for an empty result.
fn give(v: Vec<u16>) -> (*mut u16, u32) {
    if v.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let len = v.len() as u32;
    (Box::into_raw(v.into_boxed_slice()) as *mut u16, len)
}

fn guarded(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(UZT_PANIC)
}

/// The ABI version ([`UZT_ABI_VERSION`]); a host checks it before any other call.
#[unsafe(no_mangle)]
pub extern "C" fn uzt_abi_version() -> u32 {
    UZT_ABI_VERSION
}

/// The rules version (uz_translit::RULES_VERSION): static ASCII, not NUL-terminated, length in `*len`; never freed.
///
/// # Safety
/// `len` is writable or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uzt_rules_version(len: *mut u32) -> *const u8 {
    if !len.is_null() {
        // SAFETY: checked non-null; caller contract.
        unsafe { *len = uz_translit::RULES_VERSION.len() as u32 };
    }
    uz_translit::RULES_VERSION.as_ptr()
}

/// Converts `text`. On success `*out`/`*out_len` hold the result (free it with [`uzt_free`]).
///
/// # Safety
/// `text` points to `len` u16; `out` and `out_len` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uzt_convert(
    text: *const u16,
    len: u32,
    to: u32,
    alphabet: u32,
    flags: u32,
    out: *mut *mut u16,
    out_len: *mut u32,
) -> i32 {
    if out.is_null() || out_len.is_null() {
        return UZT_BAD_ARG;
    }
    // SAFETY: checked non-null above; the caller guarantees they are writable.
    unsafe {
        *out = ptr::null_mut();
        *out_len = 0;
    }
    guarded(|| {
        let Some((dir, opts)) = params(to, alphabet, flags) else {
            return UZT_BAD_ARG;
        };
        // SAFETY: caller contract.
        let Some(src) = (unsafe { units(text, len) }) else {
            return UZT_BAD_ARG;
        };
        let Ok(s) = String::from_utf16(src) else {
            return UZT_BAD_UTF16;
        };
        let (p, n) = give(
            uz_translit::convert(&s, dir, &opts)
                .encode_utf16()
                .collect(),
        );
        // SAFETY: checked non-null at entry.
        unsafe {
            *out = p;
            *out_len = n;
        }
        UZT_OK
    })
}

/// Converts the `n` runs of ONE paragraph as one text (uz_translit::convert_runs: the output does not depend on where
/// the runs are split). The runs are given concatenated in `text`, their lengths in `lens[n]`; `convert[i] == 0`
/// copies run i verbatim and makes it a hard boundary. On success `*out`/`*out_len` hold the concatenated new texts
/// and `out_lens[n]` each run's new length. Every run must be valid UTF-16 on its own.
///
/// # Safety
/// `text` points to `sum(lens)` u16; `lens` and `convert` point to `n` readable items; `out_lens` to `n` writable u32;
/// `out` and `out_len` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uzt_convert_runs(
    text: *const u16,
    lens: *const u32,
    convert: *const u8,
    n: u32,
    to: u32,
    alphabet: u32,
    flags: u32,
    out: *mut *mut u16,
    out_len: *mut u32,
    out_lens: *mut u32,
) -> i32 {
    if out.is_null()
        || out_len.is_null()
        || (n > 0 && (lens.is_null() || convert.is_null() || out_lens.is_null()))
    {
        return UZT_BAD_ARG;
    }
    // SAFETY: checked non-null above.
    unsafe {
        *out = ptr::null_mut();
        *out_len = 0;
    }
    guarded(|| {
        let Some((dir, opts)) = params(to, alphabet, flags) else {
            return UZT_BAD_ARG;
        };
        let n = n as usize;
        let (lens, conv): (&[u32], &[u8]) = if n == 0 {
            (&[], &[])
        } else {
            // SAFETY: caller contract: `n` readable items each.
            unsafe {
                (
                    std::slice::from_raw_parts(lens, n),
                    std::slice::from_raw_parts(convert, n),
                )
            }
        };
        let total: u64 = lens.iter().map(|&l| u64::from(l)).sum();
        let Ok(total) = u32::try_from(total) else {
            return UZT_BAD_ARG;
        };
        // SAFETY: caller contract: `sum(lens)` units at `text`.
        let Some(src) = (unsafe { units(text, total) }) else {
            return UZT_BAD_ARG;
        };
        let mut owned: Vec<String> = Vec::with_capacity(n);
        let mut at = 0usize;
        for &l in lens {
            let Ok(s) = String::from_utf16(&src[at..at + l as usize]) else {
                return UZT_BAD_UTF16;
            };
            owned.push(s);
            at += l as usize;
        }
        let runs: Vec<Run> = owned
            .iter()
            .zip(conv)
            .map(|(t, &c)| Run {
                text: t,
                convert: c != 0,
            })
            .collect();
        let mut buf: Vec<u16> = Vec::with_capacity(total as usize);
        let mut new_lens: Vec<u32> = Vec::with_capacity(n);
        for r in uz_translit::convert_runs(&runs, dir, &opts) {
            let before = buf.len();
            buf.extend(r.encode_utf16());
            new_lens.push((buf.len() - before) as u32);
        }
        if u32::try_from(buf.len()).is_err() {
            return UZT_BAD_ARG;
        }
        let (p, len) = give(buf);
        // SAFETY: checked non-null at entry; `out_lens` has `n` slots.
        unsafe {
            ptr::copy_nonoverlapping(new_lens.as_ptr(), out_lens, n);
            *out = p;
            *out_len = len;
        }
        UZT_OK
    })
}

/// Frees a buffer returned by this library. Null is a no-op.
///
/// # Safety
/// `p`/`len` must be exactly a pointer and length this library returned, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uzt_free(p: *mut u16, len: u32) {
    if p.is_null() {
        return;
    }
    let _ = catch_unwind(|| {
        // SAFETY: caller contract: `p` came from `give` with this length.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(p, len as usize)) });
    });
}
