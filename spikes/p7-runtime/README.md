# p7-runtime: plugin runtime spike (Phase 7 planning, throwaway)

Question: which wasm runtime hosts plugins in the native editors, what does the plugin boundary cost for uz-translit,
and does the browser path (dedicated Worker) keep the same output and speed. Not product code; nothing here ships.

## What it is
- `plugin/`: uz-translit (`crates/uz-translit`, unchanged) behind a core-wasm ABI with **no imports**: `p7_alloc`, `p7_free`,
  `p7_bin` (binary batch, `wire.rs`), `p7_json` (the same batch as JSON), plus two hostile exports (`p7_spin` never returns,
  `p7_hog` grows memory until refused). Built `--profile plugin --target wasm32-unknown-unknown` (opt-level s, LTO): 108,282 B.
  A second build with `RUSTFLAGS="-C link-arg=--import-memory -C link-arg=--max-memory=4294967296"` imports `env.memory`.
- `host/`: the bench. Corpus: 2,577 paragraphs, 12,555 runs, 334,029 B UTF-8 (about the 0.33 M characters of the Phase 2
  108-page files, `docs/STATUS.md:971`), runs of 1 to 5 words, every 7th run a keep run. Native `convert_runs` per paragraph is
  the reference; every plugin answer is compared with it (`identical=true` = equal per run).
- `size-base`, `size-wasmtime`, `size-wasmi`: the same tiny program (std + serde_json) without a runtime, and loading + calling one
  export through each runtime. The size difference is the runtime's cost in an exe.
- `web/`: `bench.html` + `worker.js` + `run.mjs` (headless Chromium through Playwright): the same module and input on the main
  thread and in a dedicated Worker served with `Content-Security-Policy: default-src 'none'`.

## Run (this PC)
```
export CARGO_TARGET_DIR=E:/office-target/p7-spike CARGO_HTTP_CHECK_REVOKE=false
cargo build --profile plugin -p p7-translit-plugin --target wasm32-unknown-unknown
cargo build --release -p p7-host -p p7-size-base -p p7-size-wasmtime -p p7-size-wasmi     # about 6 min (codegen-units 1)
p7-host.exe plugin.wasm 330000 <data dir>        # also writes input.bin + native.bin for the web bench
node web/run.mjs <data dir with plugin.wasm, plugin-imp.wasm, input.bin, native.bin> <path to playwright>
cargo deny --manifest-path Cargo.toml --config ../../deny.toml check
```

## Results (2026-11-03, Windows 10, release, x86_64-pc-windows-gnu, wasmtime 50.0.1, wasmi 2.0.0, Chromium 141)

Timing: median of 7 rounds per run, two runs each (ranges are across runs).

| Path | Time for the whole batch | vs native | Output |
|---|---|---|---|
| native `convert_runs` (reference; Phase 2 N1 measured "convert 41-59 ms") | **40.5-41.1 ms** | 1.0x | reference |
| wasmtime, binary batch (encode + copy in + convert + copy out + decode) | **65.5-73.3 ms** | 1.6-1.8x | identical |
| wasmtime, JSON batch | 104.0-106.9 ms | 2.6x | identical |
| wasmi (interpreter, fuel on), binary batch | **474.9-645.8 ms** | 12-16x | identical |
| wasmi, JSON batch | 572.0-581.3 ms | 14x | identical |
| Chromium, main thread (copy in + call + copy out) | 44.8-57.5 ms | 1.1-1.4x | identical |
| Chromium, dedicated Worker round trip (postMessage in, call, transfer out) | **44.8-61.5 ms** | 1.1-1.5x | identical |

- Host encoding alone: binary 0.1-0.8 ms, JSON 23.5-28.8 ms for the same batch. The JSON envelope costs about 35-40 ms here.
- Start-up: wasmtime Cranelift compile of the 108 KB module **150-185 ms** (cold, once per process), instantiate 0.05-0.12 ms.
  wasmi translate 1.0-1.3 ms. Chromium `WebAssembly.compile` 1.8-2.3 ms (lazy Liftoff), Worker start + instantiate 7.3-22.9 ms.
- The Worker hop costs under 1 ms for 0.4 MB: the Worker round trip equals the main-thread call within noise.

| Exe size (stripped release) | raw | gzip -9 | xz -9 |
|---|---|---|---|
| baseline (std + serde_json) | 973,824 | 452,747 | 355,328 |
| + wasmtime (cranelift, runtime, std; no WASI, no component model) | 13,352,960 (**+12.4 MB**) | 4,699,429 (+4.2 MB) | 3,295,924 (+2.9 MB) |
| + wasmi | 3,271,680 (+2.3 MB) | 1,251,537 (+0.8 MB) | 939,436 (+0.6 MB) |

For scale: the release `docs.exe` is 87,830,277 B (`E:/office-target/p1/docs/release/docs.exe`, gnu, 2026-10-24) and the MSI
211,734,528 B (`E:/office-target/p1/msi/hoshiya-0.1.0-x64.msi`, 2026-10-31).

Sandbox behaviour seen:
- wasmtime: `p7_spin` stopped by the epoch deadline after 50-51 ms (a ticker thread bumps the epoch every 50 ms); `p7_hog` got 768
  pages (48 MiB) more and was then refused under a 64 MiB `StoreLimits` cap. The module has **0 imports** (`module.imports()`).
- Chromium: a spinning plugin in a Worker leaves the page responsive (a 300 ms timer fired after 305.8-308.5 ms) and
  `terminate()` stops it. `fetch` from the CSP'd worker script fails (`TypeError`).
- Chromium, **exported memory: the hog reached 4,279,304,192 B (65,280 pages) inside a Worker of the page**. With the
  import-memory build and a host-made `WebAssembly.Memory({maximum: 1024})` the hog stopped at 768 pages (51,445,760 B).
  So on the web the cap only holds if the plugin **imports** its memory from the host: an ABI rule, not an option.
- A non-shared `WebAssembly.Memory` cannot be posted to a Worker (`DataCloneError`); the worker creates it from a page count.

Licences: `cargo deny check` with the root `deny.toml` unchanged: **bans ok, licenses ok, sources ok, advisories ok** for the whole
graph (wasmtime 50.0.1 and wasmi 2.0.0 together; 104 crates under the wasmtime probe, 21 under the wasmi probe). The wasmtime crates
are `Apache-2.0 WITH LLVM-exception` (26) or `MIT OR Apache-2.0`; that licence is already on the allow list (`deny.toml:17`).

## Not measured here
- MSVC build of wasmtime (gnu only); Android System WebView (the emulator was not used); a real editor bridge hop
  (iframe to parent, same structured-clone mechanism as the Worker hop); Wasmtime's `winch` and `pulley` back ends; the
  Docs wasm size of the editor-side code (that is WP4's number).
