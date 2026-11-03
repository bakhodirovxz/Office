//! PLAN phase 7 section 5.4: M1, M2, M6, M7, M8 against the real wasmtime host, with tiny modules written as WAT.
//! Plus the round loop (16-round cap) and a happy path.

use std::time::{Duration, Instant};

use plugin_api::textmap::{Apply, EDITABLE, Paragraph, Segment, TextMap};
use plugin_api::wire::Message;
use plugin_host::{Host, LoadError, RunError};

const BUDGET: Duration = Duration::from_millis(200);

fn bytes_wat(b: &[u8]) -> String {
    b.iter().map(|x| format!("\\{x:02x}")).collect()
}

/// A plugin that imports the host's memory (2 pages), has a bump allocator at 64 KiB, and whose `call` runs `body`.
/// `data` sits at offset 1024. `extra` is added to the module (more imports, helper functions).
fn module(imports: &str, extra: &str, body: &str, data: &[u8]) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
  (import "env" "memory" (memory 2))
  {imports}
  (global $bump (mut i32) (i32.const 65536))
  (data (i32.const 1024) "{}")
  (func (export "api_version") (result i32) (i32.const 1))
  (func (export "alloc") (param i32) (result i32) (local i32)
    global.get $bump local.set 1
    global.get $bump local.get 0 i32.add global.set $bump
    local.get 1)
  (func (export "free") (param i32 i32))
  {extra}
  (func (export "call") (param i32 i32) (result i64) {body}))"#,
        bytes_wat(data)
    ))
    .unwrap()
}

/// `call` returns the bytes `data` (placed at 1024) as its answer.
fn canned(data: &[u8]) -> Vec<u8> {
    module(
        "",
        "",
        &format!("(i64.const {})", (1024i64 << 32) | data.len() as i64),
        data,
    )
}

fn answer(header: &str, blobs: &[&[u8]]) -> Vec<u8> {
    Message {
        header: header.into(),
        blobs: blobs.iter().map(|b| b.to_vec()).collect(),
    }
    .encode()
    .unwrap()
}

fn call() -> Message {
    Message {
        header: r#"{"kind":"command"}"#.into(),
        blobs: vec![],
    }
}

fn host() -> Host {
    Host::new().unwrap()
}

fn load(wasm: &[u8]) -> Result<plugin_host::Plugin, LoadError> {
    host().load(wasm, 1, BUDGET)
}

fn round(wasm: &[u8]) -> Result<Message, RunError> {
    load(wasm).unwrap().start().unwrap().round(&call())
}

fn load_err(wasm: &[u8]) -> LoadError {
    match load(wasm) {
        Err(e) => e,
        Ok(_) => panic!("module was accepted"),
    }
}

#[test]
fn happy_path_and_log() {
    let data = answer(
        r#"{"actions":[{"type":"ui.status","text":"hi"}]}"#,
        &[b"blob"],
    );
    let n = data.len();
    // log(2048, 3) with "hey" stored at 2048 by a second data segment
    let wasm = module(
        r#"(import "plugin_v1" "log" (func $log (param i32 i32)))
           (data (i32.const 2048) "hey")"#,
        "",
        &format!(
            "(call $log (i32.const 2048) (i32.const 3)) (i64.const {})",
            (1024i64 << 32) | n as i64
        ),
        &data,
    );
    let mut s = load(&wasm).unwrap().start().unwrap();
    let m = s.round(&call()).unwrap();
    assert!(m.header.contains("ui.status"));
    assert_eq!(m.blobs, vec![b"blob".to_vec()]);
    assert_eq!(s.log(), ["hey"]);
    // a second round on the same session works
    s.round(&call()).unwrap();
}

#[test]
fn m1_foreign_imports_refuse_the_load() {
    let wasm = wat::parse_str(
        r#"(module
  (import "env" "memory" (memory 2))
  (import "wasi_snapshot_preview1" "path_open" (func (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_open" (func))
  (import "env" "other" (func)))"#,
    )
    .unwrap();
    match load_err(&wasm) {
        LoadError::Imports(v) => {
            assert_eq!(
                v,
                [
                    "wasi_snapshot_preview1.path_open",
                    "wasi_snapshot_preview1.fd_write",
                    "wasi_snapshot_preview1.sock_open",
                    "env.other"
                ]
            );
        }
        e => panic!("{e:?}"),
    }
    // the product-name look-alike namespace is not the ABI either, and env.memory must be a memory
    let wasm = wat::parse_str(r#"(module (import "env" "memory" (memory 2)) (import "plugin_v2" "log" (func (param i32 i32))))"#).unwrap();
    assert!(matches!(load_err(&wasm), LoadError::Imports(_)));
    let wasm = wat::parse_str(r#"(module (import "env" "memory" (func)))"#).unwrap();
    assert!(matches!(load_err(&wasm), LoadError::Imports(_)));
}

#[test]
fn m2_own_or_exported_memory_refuses_the_load() {
    // re-exports the imported memory
    let wasm = wat::parse_str(
        r#"(module (import "env" "memory" (memory 2)) (export "memory" (memory 0)))"#,
    )
    .unwrap();
    assert!(matches!(load_err(&wasm), LoadError::ExportsMemory(n) if n == "memory"));
    // defines and exports its own memory instead of importing the host's
    let wasm = wat::parse_str(r#"(module (memory (export "memory") 2))"#).unwrap();
    assert_eq!(load_err(&wasm), LoadError::NoMemoryImport);
    // imports the host's memory but also defines a second one (multi-memory) and exports that
    let wasm =
        wat::parse_str(r#"(module (import "env" "memory" (memory 2)) (memory (export "own") 1))"#)
            .unwrap();
    assert!(matches!(
        load_err(&wasm),
        LoadError::ExportsMemory(_) | LoadError::Compile(_)
    ));
}

#[test]
fn m2b_a_second_unexported_memory_refuses_the_load() {
    // imports the host's capped memory and defines its own, never exported: multi-memory is off, so it does not compile
    let wasm = wat::parse_str(r#"(module (import "env" "memory" (memory 2)) (memory 1))"#).unwrap();
    assert!(matches!(load_err(&wasm), LoadError::Compile(_)));
    let wasm = module("", "(memory 1)", "(i64.const 0)", b"");
    assert!(matches!(load_err(&wasm), LoadError::Compile(_)));
}

#[test]
fn deep_recursion_traps_cleanly_on_a_small_stack() {
    let wasm = module(
        "",
        "(func $r (param i32) (result i32) local.get 0 i32.const 1 i32.add call $r)",
        "(drop (call $r (i32.const 0))) (i64.const 0)",
        b"",
    );
    // the wasm stack was 512 KiB and the guest ran on the caller's thread: a 300 KiB thread lost the process. `round` now has its own thread.
    let mut s = load(&wasm).unwrap().start().unwrap();
    let e = std::thread::Builder::new()
        .stack_size(100 << 10)
        .spawn(move || s.round(&call()).unwrap_err())
        .unwrap()
        .join()
        .unwrap();
    assert!(matches!(e, RunError::Trap(_)), "{e:?}");
}

#[test]
fn limit_round_budget_cuts_a_round_short() {
    let wasm = module("", "", "(loop $l (br $l)) (i64.const 0)", b"");
    let mut s = host()
        .load(&wasm, 1, Duration::from_secs(30))
        .unwrap()
        .start()
        .unwrap();
    s.limit_round_budget(Duration::from_millis(100));
    let t = Instant::now();
    assert!(matches!(s.round(&call()), Err(RunError::Timeout)));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

#[test]
fn load_checks_memory_limit_exports_and_version() {
    let ok = canned(b"x");
    assert!(matches!(
        host().load(&ok, 0, BUDGET),
        Err(LoadError::MemoryLimit(0))
    ));
    assert!(matches!(
        host().load(&ok, 65, BUDGET),
        Err(LoadError::MemoryLimit(65))
    ));
    // starts above the cap: 17 pages = 1 MiB + 64 KiB, cap 1 MB = 16 pages
    let big = wat::parse_str(r#"(module (import "env" "memory" (memory 17)))"#).unwrap();
    assert!(matches!(load_err(&big), LoadError::MemoryType(_)));
    // missing exports
    let none = wat::parse_str(r#"(module (import "env" "memory" (memory 2)))"#).unwrap();
    assert!(matches!(load_err(&none), LoadError::Exports(_)));
    // api_version 2
    let v2 = String::from_utf8(wat_text()).unwrap().replace(
        "(i32.const 1))\n  (func (export \"alloc\")",
        "(i32.const 2))\n  (func (export \"alloc\")",
    );
    assert_eq!(
        load_err(&wat::parse_str(v2).unwrap()),
        LoadError::ApiVersion(2)
    );
    assert!(matches!(load_err(b"not wasm"), LoadError::Compile(_)));
}

fn wat_text() -> Vec<u8> {
    br#"(module
  (import "env" "memory" (memory 2))
  (func (export "api_version") (result i32) (i32.const 1))
  (func (export "alloc") (param i32) (result i32) (i32.const 65536))
  (func (export "free") (param i32 i32))
  (func (export "call") (param i32 i32) (result i64) (i64.const 0)))"#
        .to_vec()
}

#[test]
fn m6_infinite_loop_is_stopped_at_the_budget() {
    let wasm = module("", "", "(loop $l (br $l)) (i64.const 0)", b"");
    let mut s = load(&wasm).unwrap().start().unwrap();
    let t = Instant::now();
    let e = s.round(&call()).unwrap_err();
    let took = t.elapsed();
    assert!(matches!(e, RunError::Timeout), "{e:?}");
    assert!(took < Duration::from_secs(2), "{took:?}"); // 200 ms budget, 50 ms tick
    // the session is dead afterwards
    assert!(matches!(s.round(&call()), Err(RunError::Dead)));
    // an infinite loop in the start function is stopped too (load probes the module)
    let wasm = wat::parse_str(
        r#"(module (import "env" "memory" (memory 2)) (func $s (loop $l (br $l))) (start $s)
        (func (export "api_version") (result i32) (i32.const 1)) (func (export "alloc") (param i32) (result i32) (i32.const 0))
        (func (export "free") (param i32 i32)) (func (export "call") (param i32 i32) (result i64) (i64.const 0)))"#,
    )
    .unwrap();
    assert!(matches!(load_err(&wasm), LoadError::Exports(m) if m.contains("time budget")));
}

#[test]
fn m7_memory_growth_is_capped_at_memory_mb() {
    // grow one page at a time until refused, remember how many pages were granted, then answer normally
    let data = answer(r#"{"actions":[]}"#, &[]);
    let hog = |then: &str| {
        module(
            "",
            "",
            &format!(
                "(loop $l (if (i32.ne (memory.grow (i32.const 1)) (i32.const -1)) (then (br $l)))) {then} (i64.const {})",
                (1024i64 << 32) | data.len() as i64
            ),
            &data,
        )
    };
    let mut s = load(&hog("")).unwrap().start().unwrap();
    s.round(&call()).unwrap();
    assert!(s.memory_denied());
    assert_eq!(s.memory_pages(), 16, "1 MB cap = 16 pages"); // the cap, exactly
    // a plugin that traps after being refused is reported as a memory stop
    let mut s = load(&hog("unreachable")).unwrap().start().unwrap();
    assert!(matches!(s.round(&call()), Err(RunError::Memory)));
    // one huge grow request is refused at once; the plugin sees -1
    let wasm = module(
        "",
        "",
        "(if (i32.ne (memory.grow (i32.const 60000)) (i32.const -1)) (then unreachable)) (i64.const 0)",
        b"",
    );
    let e = round(&wasm).unwrap_err();
    assert!(matches!(e, RunError::BadOutput(_)), "{e:?}"); // reached the end, answer 0/0 is not a message
}

#[test]
fn m8_malformed_answers_are_refused() {
    let good = answer(r#"{"actions":[]}"#, &[]);
    let refused = |bytes: &[u8]| -> String {
        let mut s = load(&canned(bytes)).unwrap().start().unwrap();
        let m = s.round(&call());
        let m = match m {
            Err(RunError::BadOutput(e)) => return e,
            Err(e) => panic!("{e:?}"),
            Ok(m) => m,
        };
        // the message decoded; the header must then fail as an Answer
        let r = s.run(call(), |_, _, _| Ok(None));
        let _ = m;
        match r {
            Err(RunError::BadOutput(e)) => e,
            other => panic!("{other:?}"),
        }
    };
    // truncated
    assert!(refused(&good[..good.len() - 3]).contains("truncated"));
    // header is not JSON
    refused(&answer("not json", &[]));
    // header is JSON but not an answer (unknown field)
    refused(&answer(r#"{"actions":[],"evil":1}"#, &[]));
    // header length field claims 1 GB
    let mut huge = good.clone();
    huge[..4].copy_from_slice(&(1u32 << 30).to_le_bytes());
    assert!(refused(&huge).contains("1 MiB"));
    // invalid UTF-8 in the header
    let mut bad = good.clone();
    bad[4] = 0xff;
    assert!(refused(&bad).contains("UTF-8"));
    // trailing bytes
    let mut tail = good.clone();
    tail.push(0);
    assert!(refused(&tail).contains("after the end"));
    // blob count 1 GB
    let mut blobs = good.clone();
    let n = blobs.len();
    blobs[n - 4..].copy_from_slice(&(1u32 << 30).to_le_bytes());
    assert!(refused(&blobs).contains("blobs"));

    // the answer pointer/length packed by the plugin: length 1 GB, and a range outside memory
    for packed in [
        (1024i64 << 32) | (1 << 30),
        (131000i64 << 32) | 4096,
        0x7fff_ffff_0000_0010,
    ] {
        let wasm = module("", "", &format!("(i64.const {packed})"), b"");
        assert!(
            matches!(round(&wasm), Err(RunError::BadOutput(_))),
            "{packed:x}"
        );
    }
}

#[test]
fn m8_text_map_apply_with_an_extra_segment_fails_validation() {
    // The plugin answers with a well-formed message whose apply blob has one segment too many. The wire layer accepts the
    // blob (it is opaque there); the adapter's check (Apply::validate) must refuse it. Proven here end to end.
    let map = TextMap {
        langs: vec![String::new()],
        paragraphs: vec![Paragraph {
            index: 0,
            segments: vec![Segment {
                flags: EDITABLE,
                lang: 0,
                text: "abc".into(),
            }],
        }],
    };
    let evil = Apply {
        paragraphs: vec![vec![Some("x".into()), Some("y".into())]],
    }
    .encode()
    .unwrap();
    let data = answer(
        r#"{"actions":[{"type":"textmap.apply","blob":0}]}"#,
        &[&evil],
    );
    let m = round(&canned(&data)).unwrap();
    let apply = Apply::decode(&m.blobs[0]).unwrap();
    assert!(apply.validate(&map).is_err());
}

#[test]
fn round_loop_stops_at_16_rounds_and_passes_blobs() {
    let again = answer(r#"{"actions":[],"next":{"reads":[]}}"#, &[]);
    let mut s = load(&canned(&again)).unwrap().start().unwrap();
    let mut seen = 0;
    let e = s
        .run(call(), |r, _, _| {
            seen = r;
            Ok(Some(call()))
        })
        .unwrap_err();
    assert!(matches!(e, RunError::TooManyRounds), "{e:?}");
    assert_eq!(seen, 16);

    // `next` with a step that declines ends the invocation; no `next` ends it even if the step offers a call
    let mut s = load(&canned(&again)).unwrap().start().unwrap();
    s.run(call(), |_, _, _| Ok(None)).unwrap();
    let done = answer(r#"{"actions":[]}"#, &[]);
    let mut s = load(&canned(&done)).unwrap().start().unwrap();
    let mut n = 0;
    s.run(call(), |_, _, _| {
        n += 1;
        Ok(Some(call()))
    })
    .unwrap();
    assert_eq!(n, 1);
    // a refusing step is an error and kills the session
    let mut s = load(&canned(&done)).unwrap().start().unwrap();
    assert!(matches!(
        s.run(call(), |_, _, _| Err("no".into())),
        Err(RunError::Step(_))
    ));
    assert!(matches!(s.round(&call()), Err(RunError::Dead)));
}

#[test]
fn background_load_and_fresh_state_per_session() {
    let h = host();
    let p = h
        .load_in_background(canned(&answer(r#"{"actions":[]}"#, &[])), 1, BUDGET)
        .join()
        .unwrap()
        .unwrap();
    let (a, b) = (p.start().unwrap(), p.start().unwrap());
    assert_eq!(a.memory_pages(), 2);
    assert_eq!(b.memory_pages(), 2);
}
