//! The spike plugin: uz-translit behind a core-wasm ABI (no imports). `p7_alloc`/`p7_free` manage guest memory,
//! `p7_bin` and `p7_json` take one batch of paragraphs and return (ptr << 32 | len) of the answer.
#[path = "../../wire.rs"]
mod wire;
use std::borrow::Cow;
use uz_translit::{Direction, LatinAlphabet, Options, Run, convert_runs};
use wire::*;

fn opts() -> Options {
    Options { latin_alphabet: LatinAlphabet::Y1995, ..Options::default() }
}

#[unsafe(no_mangle)]
pub extern "C" fn p7_alloc(len: u32) -> u32 {
    let mut v = Vec::<u8>::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn p7_free(ptr: u32, len: u32) {
    unsafe { drop(Vec::from_raw_parts(ptr as *mut u8, 0, len as usize)) }
}

fn give(mut out: Vec<u8>) -> u64 {
    out.shrink_to_fit();
    let (p, n) = (out.as_mut_ptr() as u64, out.len() as u64);
    std::mem::forget(out);
    (p << 32) | n
}

fn take(ptr: u32, len: u32) -> Vec<u8> {
    unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) }
}

#[unsafe(no_mangle)]
pub extern "C" fn p7_bin(ptr: u32, len: u32) -> u64 {
    let input = take(ptr, len);
    let mut r = Rd { b: &input, i: 0 };
    let dir = if r.u8() == 0 { Direction::ToCyrillic } else { Direction::ToLatin };
    let o = opts();
    let n = r.u32();
    let mut out = Vec::with_capacity(input.len() * 2);
    put_u32(&mut out, n);
    let mut runs = Vec::new();
    for _ in 0..n {
        runs.clear();
        let k = r.u32();
        for _ in 0..k {
            let convert = r.u8() == 1;
            runs.push(Run { text: r.str(), convert });
        }
        put_u32(&mut out, k);
        for c in convert_runs(&runs, dir, &o) {
            match c {
                Cow::Borrowed(_) => out.push(0),
                Cow::Owned(s) => {
                    out.push(1);
                    put_str(&mut out, &s);
                }
            }
        }
    }
    give(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn p7_json(ptr: u32, len: u32) -> u64 {
    let input = take(ptr, len);
    let v: serde_json::Value = serde_json::from_slice(&input).unwrap();
    let dir = if v["dir"] == "cyrl" { Direction::ToCyrillic } else { Direction::ToLatin };
    let o = opts();
    let mut paras = Vec::new();
    for p in v["paras"].as_array().unwrap() {
        let runs: Vec<Run> = p
            .as_array()
            .unwrap()
            .iter()
            .map(|r| Run { text: r["t"].as_str().unwrap(), convert: r["c"].as_bool().unwrap() })
            .collect();
        let outs: Vec<serde_json::Value> = convert_runs(&runs, dir, &o)
            .into_iter()
            .map(|c| match c {
                Cow::Borrowed(_) => serde_json::Value::Null,
                Cow::Owned(s) => serde_json::Value::String(s),
            })
            .collect();
        paras.push(serde_json::Value::Array(outs));
    }
    give(serde_json::to_vec(&paras).unwrap())
}

/// A hostile export for the sandbox checks: never returns.
#[unsafe(no_mangle)]
pub extern "C" fn p7_spin() -> u32 {
    let mut x: u32 = 1;
    loop {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        if x == 0 {
            return x;
        }
    }
}

/// A hostile export: grows memory until refused; returns pages obtained.
#[unsafe(no_mangle)]
pub extern "C" fn p7_hog() -> u32 {
    let mut got = 0;
    while core::arch::wasm32::memory_grow(0, 256) != usize::MAX {
        got += 256;
        if got > 70_000 {
            break;
        }
    }
    got as u32
}
