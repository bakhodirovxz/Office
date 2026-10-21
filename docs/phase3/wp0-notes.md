# WP0 notes (2026-10-21, branch phase-3)

## 1. Reference re-check (PLAN §0–§4)
Upstream BASEs unchanged since the plan (patches/*/BASE): wordcraft `eae2421`, gridcraft `f1894c6`, deckcraft `1dc6972`,
pdfcraft `924c4da`. Every `file:line` reference was read at HEAD: **all confirmed, none moved**; the plan text was not
changed. Details of the less obvious ones:

- F5 `gridcraft-cli main.rs:269-284`: `convert`; the "unknown output format (use .xlsx, .csv, .tsv, .json or .html)" message is at `:275`.
- F7 `wordcraft-cli main.rs:63-71` is `open()`; `:111-113` is `system_region()` with `Region::from_locale_vars`.
- Exit codes: wordcraft `main.rs:560-566`, gridcraft `:62-92`, deckcraft `report()` `:105-111`, pdfcraft `:116-121`
  are all `ExitCode::FAILURE` (1) on error, as the plan says.
- pdfcraft `main.rs:880-909` is `run` with `--script` (`page_render` steps), as the plan says.
- F3, F4, F6, F8, wordcraft/gridcraft `io.rs` ranges, deckcraft `file.rs:340`, `docs-translit/src/lib.rs:28,100-130,119-128,203-214`
  and `spikes/fidelity/run.py:142-143` all matched their anchor text. Root `Cargo.toml:4` still excludes `services` (F12).

## 2. Build lock
`scripts/lib/lock.sh`: slots `BUILD.lock.1`, `BUILD.lock.2` (`OFFICE_LOCK_SLOTS`). A slot's `target` file holds the
cargo target dir (`$CARGO_TARGET_DIR`, else `$OFFICE_TARGET`), so two different target dirs build at once and one
target dir never builds twice. Stale slot = owner pid dead (`kill -0`) or no pid after 2 min; it is reclaimed. Callers
unchanged. RAM gate raised 3 -> 6 GB (`scripts/build.sh`, `CI_RAM_GB` in `scripts/ci.sh`). `build.sh` no longer calls
`wait_no_rustc` (it forced serial builds). `scripts/ci.sh:189` runs `with_build_lock env CARGO_TARGET_DIR=...`, so it
keys on `$OFFICE_TARGET`, not its own dir (conservative). Other scripts keep `wait_no_rustc` (still serial).
Self-check: `bash scripts/test-lock.sh` (~20 s): A+B overlap, C waits, same target serialised, stale slot reclaimed.

## 3. Docker
Docker Desktop 4.77.0, engine 29.5.3 (API 1.54, containerd v2.2.4, runc 1.3.5), client 29.5.3, linux/amd64, context
`desktop-linux`. The engine was already up (started by the parallel pdf2docx agent); `docker run --rm hello-world` printed
"Hello from Docker!".

## 4. CLI builds at HEAD (`scripts/build.sh <d> cli --release`)
| domain | wall | binary |
|---|---|---|
| docs | 177 s (ran in parallel with sheets) | `E:/office-target/p1/docs/release/docs-cli.exe` |
| sheets | 3 s (sccache/up to date, parallel with docs; no "waiting" line) | `E:/office-target/p1/sheets/release/sheets-cli.exe` |
| slides | 152 s (parallel with pdf) | `E:/office-target/p1/slides/release/slides-cli.exe` |
| pdf | 590 s (parallel with slides) | `E:/office-target/p1/pdf/release/pdf-cli.exe` |
Slides+pdf together took 594 s wall (sum would be 742 s): the two-slot lock works on real builds. Times are in
`E:/office-target/p1/logs/times.tsv`.

## 5. Brand
`brand/brand.toml` `[apps.convert]` (no engine: `upstream = ""`, `upstream_names = []`, pictogram `page`). `cargo xt brand check`:
"brand.toml OK ... 5 apps"; `cargo xt brand-grep` 0 hits; `cargo test -p office-brand` 4 passed (the app-count test now expects 5).
