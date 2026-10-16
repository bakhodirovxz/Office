# Phase 0 inventory (in progress)

Machine: Windows 10, 12 cores, 11.8 GB RAM. rustc/cargo 1.97.1 stable-x86_64-pc-windows-gnu (MSYS2 gcc), no MSVC.
Targets: all build output under `E:/office-target/<repo>`; raw logs in `E:/office-target/logs/`.
Tools installed: `wasm32-unknown-unknown` target; `trunk 0.21.14` (`cargo install trunk --locked --root E:/office-tools/cargo`, 14m38s);
trunk itself downloaded wasm-bindgen 0.2.129 (matches all four Cargo.locks) and wasm-opt `version_123`.

Local workaround (all repos): `CARGO_HTTP_CHECK_REVOKE=false`. Without it cargo cannot reach crates.io:
`SSL connect error (schannel: ... CRYPT_E_NO_REVOCATION_CHECK (0x80092012))` / `download of config.json failed`.

Wall times were measured while nothing else of mine ran; the first (cold) build of each repo includes fetching git/crates deps.

## wordcraft (v0.7.0)

Official commands (README / CLAUDE.md): `cargo build --release -p wordcraft`, `-p wordcraft-cli`; web: `cd apps/wordcraft-web && trunk build --release` (Trunk.toml: dist `dist/web`, `data-wasm-opt="z"`); tests: `cargo xtask ci` runs fmt/clippy/tests/assets/layers/wasm; measured plain `cargo test --workspace --release`.

| Item | Result |
|---|---|
| Desktop GUI `cargo build --release -p wordcraft` (cold) | OK, 556 s; `wordcraft.exe` 82,316,979 B |
| CLI `cargo build --release -p wordcraft-cli` (after GUI, shared deps warm) | OK, 262 s; `wordcraft-cli.exe` 46,369,937 B |
| Web `trunk build --release` (cold) | OK, 897 s (cargo wasm build 7m37s, wasm-opt -Oz ~6m34s) |
| wasm | raw 42,597,532 B; gzip -9 15,849,354 B; brotli -q 11 9,441,597 B |
| JS glue | raw 168,690 B; gzip 24,727 B; brotli 20,555 B |
| dist/web total | 42,786,220 B (4 files: html, ico, js, wasm) |
| Docs claim (`packaging/web/README.md`) | "about 13 MB, or 5 MB with compression" -> measured is 3.3x raw, ~3.2x gzip, ~1.9x brotli larger than the claim (claim is stale or built with different inputs) |
| Tests `cargo test --workspace --release` | stops at first failing crate: 3784 s (incl. cold test compile ~55 min). Re-run with `--no-fail-fast` (binaries cached): 205 s, 77 result lines: **2750 passed, 5 failed, 2 ignored** |

Failing tests (probably environment-related, not verified):
- `wordcraft-render` `tests_color_glyphs::emoji_are_drawn_in_colour` and `emoji_keep_their_colours_on_a_dark_page`: `both emoji use the colour emoji font: left 1, right 2` (no/other colour emoji font on this Windows box).
- `wordcraft-ui-egui` `keys::tests::alt_numpad_code_types_its_character_without_keytips`: left "й" right "é" (Alt-code decoding depends on the machine's ANSI code page / Cyrillic layout).
- `wordcraft-ui-egui` tests/breaks_menu `page_break_from_the_menu_adds_a_page`, `section_break_from_the_menu_splits_sections`: `No nodes found matching the query: label "Breaks"` (ribbon label probably localized to the system language).

Smoke test (`wordcraft-cli`, with `WORDCRAFT_NO_PREFS=1`, log `E:/office-target/logs/wordcraft-smoke.log`): `--help` lists convert, info, text, inspect, render, run, commands (629 commands), parity, mcp, zotero. No sample docx ships in the repo, so one was generated: `run --template sample --cmd 'select.text={"text":"Membership"}' --cmd format.bold --save out.docx` OK (2 pages, 41 paragraphs); `run --file out.docx --save out2.docx` (re-open + save) OK; `text` OK; `convert out2.docx out.pdf` OK (75,932 B, valid %PDF); `render ... p1.png` OK (1224x1584); docx -> odt -> docx OK. PDF import is unsupported: `unsupported format .pdf`. (layoutMs 4115 for first doc was measured while another test build was running.)

## gridcraft (v0.7.0)

Commands: `cargo build --release -p gridcraft`, `-p gridcraft-cli`; web `cd apps/gridcraft-web && trunk build --release` (README); `cargo test --workspace --release`.

| Item | Result |
|---|---|
| Desktop GUI build (cold for this repo) | OK, 323 s; `gridcraft.exe` 41,962,136 B |
| CLI build (after GUI) | OK, 130 s; `gridcraft-cli.exe` 17,574,860 B |
| Web `trunk build --release` | OK, 420 s |
| wasm raw / gzip -9 / brotli -q 11 | 18,973,730 / 7,041,161 / 4,731,289 B |
| JS glue raw / gzip / brotli | 164,096 / 23,947 / 20,036 B |
| `dist/web` total | 19,166,345 B (4 files) |
| Docs size claim | none found in README / packaging/web |
| Tests `cargo test --workspace --release` | OK (exit 0), 7962 s wall (cargo reports "Finished release in 130m 05s", i.e. almost all of it is compiling the test binaries in release; test execution itself was short). 91 result lines: **1597 passed, 0 failed, 1 ignored** |

Smoke test (`gridcraft-cli`, log `E:/office-target/logs/gridcraft-smoke.log`): `--help` lists info, convert, eval, cat, check, run, commands (341), functions (514), mcp, send, version. No sample workbook in the repo, so `run --sample sales --out out.xlsx` produced one (11,163 B, table + 2 charts). `info` OK; `cat --range A1:D5 --csv` OK; `check out.xlsx` (calc oracle + XLSX round-trip): 22 formulas, 22 matched, round trip equal; `convert` xlsx -> csv -> xlsx OK; `eval '=SUM(1,2,3)'` = 6. PDF output is unsupported: `unknown output format (use .xlsx, .csv, .tsv, .json or .html)`. (Passing a bogus `--cmd x=1` correctly errors `unknown command x`.)
