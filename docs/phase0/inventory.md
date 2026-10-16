# Phase 0 inventory

Machine: Windows 10, 12 cores, 11.8 GB RAM. rustc/cargo 1.97.1 stable-x86_64-pc-windows-gnu (MSYS2 gcc), no MSVC.
Targets: all build output under `E:/office-target/<repo>`; raw logs in `E:/office-target/logs/`.
Tools installed: `wasm32-unknown-unknown` target; `trunk 0.21.14` (`cargo install trunk --locked --root E:/office-tools/cargo`, 14m38s);
trunk itself downloaded wasm-bindgen 0.2.129 (matches all four Cargo.locks) and wasm-opt `version_123`.

Local workaround (all repos): `CARGO_HTTP_CHECK_REVOKE=false`. Without it cargo cannot reach crates.io:
`SSL connect error (schannel: ... CRYPT_E_NO_REVOCATION_CHECK (0x80092012))` / `download of config.json failed`.

Wall times were measured while nothing else of mine ran; the first (cold) build of each repo includes fetching git/crates deps.

## Summary table

| | wordcraft 0.7.0 | gridcraft 0.7.0 | deckcraft 0.7.0 | pdfcraft 0.8.0 |
|---|---|---|---|---|
| Desktop GUI release build (cold) | 556 s | 323 s | 612 s | 2636 s (*) |
| GUI exe size | 82.3 MB | 42.0 MB | 92.4 MB | 69.2 MB |
| CLI release build | 262 s | 130 s | 301 s | 1400 s (*) |
| CLI exe size | 46.4 MB | 17.6 MB | 65.4 MB | 41.8 MB |
| Web build (trunk --release) | 897 s | 420 s | 867 s | 1341 s (*) |
| wasm raw | 42,597,532 | 18,973,730 | 35,120,783 | 39,076,254 |
| wasm gzip -9 | 15,849,354 | 7,041,161 | 13,905,434 | 14,379,872 |
| wasm brotli -q 11 | 9,441,597 | 4,731,289 | 9,300,367 | 9,390,759 |
| JS glue raw / gzip / br | 168,690 / 24,727 / 20,555 | 164,096 / 23,947 / 20,036 | 158,813 / 23,504 / 19,665 | 168,396 / 24,478 / 20,487 |
| dist/web total (bytes) | 42,786,220 | 19,166,345 | 35,282,713 | 39,245,860 |
| First `cargo test --workspace --release` | exit 101, 3784 s | exit 0, 7962 s | exit 139, 3213 s | exit 101, 8810 s (*) |
| Tests passed / failed / ignored (full, `--no-fail-fast`) | 2750 / 5 / 2 | 1597 / 0 / 1 | 1669 / 1 / 6 (+ intermittent crash) | 3466 / 14 / 16 |
| CLI smoke test | OK (no PDF import) | OK (`convert` has no PDF target; PDF export works via `run --cmd file.exportPdf=…`, used by the fidelity harness) | OK (pptx, pdf) | OK |

(*) pdfcraft GUI/CLI/web/test timings overlapped with my own deckcraft `--no-fail-fast` test recompile and with the orchestrator's other work (memory spikes), so they are inflated; treat as upper bounds. Test wall times are dominated by compiling test binaries in release (e.g. gridcraft: cargo "Finished" after 130 min, execution itself is minutes), so a full workspace test is not a 40-min job on this machine; the first tests run was never split per crate.
Only wordcraft's docs state a wasm size (13 MB / 5 MB compressed); measured 42.6 / 15.8 (gz) / 9.4 (br) MB. No other repo documents a size.
Docs: none of the repos has docs/HOSTING.md; web build is `trunk build --release` in `apps/<name>-web` (README), plus `packaging/web/package.sh` (wordcraft).

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

## deckcraft (v0.7.0)

Commands: `cargo build --release -p deckcraft`, `-p deckcraft-cli`; web `cd apps/deckcraft-web && trunk build --release`; `cargo test --workspace --release`.

| Item | Result |
|---|---|
| Desktop GUI build (cold for this repo) | OK, 612 s; `deckcraft.exe` 92,392,362 B |
| CLI build (after GUI) | OK, 301 s; `deckcraft-cli.exe` 65,352,235 B |
| Web `trunk build --release` | OK, 867 s |
| wasm raw / gzip -9 / brotli -q 11 | 35,120,783 / 13,905,434 / 9,300,367 B |
| JS glue raw / gzip / brotli | 158,813 / 23,504 / 19,665 B |
| `dist/web` total | 35,282,713 B (4 files) |
| Docs size claim | none found in README/docs |
| Tests `cargo test --workspace --release` | exit 139 after 3213 s: the `deckcraft-ui-egui --lib` test binary died with `exit code: 0xc0000005, STATUS_ACCESS_VIOLATION` (cargo reports it; no test name). Re-run with `--no-fail-fast` (4801 s wall incl. a recompile; it overlapped with the pdfcraft GUI build) crashed the same way. All other binaries: 1466 passed, 0 failed, 2 ignored. Running that lib binary directly (`deckcraft_ui_egui-*.exe`): 3 runs (1 with `--test-threads=1`, 2 default) did NOT crash and gave 203 passed, 1 failed, 4 ignored. **Combined: 1669 passed, 1 failed, 6 ignored, plus an intermittent access violation in the multi-threaded ui-egui lib test run under `cargo test` (not diagnosed).** |

Failing test: `replacefonts_tests::ribbon_button_opens_the_dialog_and_replaces` (`crates/ui-egui/src/replacefonts_tests.rs:46`: assertion left None, right Some("replaceFonts"); the ribbon button was not found, likely the same localized-label issue as wordcraft's Breaks test; unverified).

Smoke test (`deckcraft-cli`, log `E:/office-target/logs/deckcraft-smoke.log`): subcommands info, render, render-transition, convert, run, commands (321), describe, app, mcp, bench, links. No sample deck except `assets/app-icon/icon-source.deckcraft`; `run --sample --save s.deckcraft` OK; `info s.deckcraft` JSON OK; re-open + `run --in s.deckcraft --cmd 'slide.new=...' --save s2.deckcraft` OK; convert to .pptx (38,324 B) and back to .deckcraft OK; convert to .pdf OK (1,071,584 B, valid %PDF). (`info --in` is not a valid form: `info` takes a positional FILE.)

### deckcraft crash investigation

`E:/office-target/deckcraft/release/deps/deckcraft_ui_egui-*.exe` (the `deckcraft-ui-egui --lib` test binary), run directly:
- default threads: both `cargo test` runs crashed (`0xc0000005 STATUS_ACCESS_VIOLATION`); of 8 direct runs, 1 crashed (`Segmentation fault`, rc 139) and 7 finished with 203 passed / 1 failed / 4 ignored.
- `--test-threads=1`: 4 runs, never crashed.
So the crash needs concurrent tests. The last completed tests in the crashed direct run were `icons::tests::triangulates_concave_polygon` and `eyedropper_tests::a_click_on_the_slide_takes_the_colour_under_it_and_nothing_else_happens` (75 tests ok at that point); the crashing test itself cannot be named from libtest's default output (it only prints on completion). The crate's dev-dependency is `egui_kittest` with the `wgpu` and `snapshot` features, so a wgpu/software-GPU race is plausible, but this is not confirmed. Logs: `E:/office-target/logs/deckcraft-uilib-*.log`.

## pdfcraft (v0.8.0)

Commands: `cargo build --release -p pdfcraft`, `-p pdfcraft-cli`; web `cd apps/pdfcraft-web && trunk build --release` (Trunk.toml); `cargo test --workspace --release`.

| Item | Result |
|---|---|
| Desktop GUI build | OK, 2636 s (inflated, see note); `pdfcraft.exe` 69,167,142 B |
| CLI build | OK, 1400 s (inflated); `pdfcraft-cli.exe` 41,783,522 B |
| Web `trunk build --release` | OK, 1341 s (inflated) |
| wasm raw / gzip -9 / brotli -q 11 | 39,076,254 / 14,379,872 / 9,390,759 B |
| JS glue raw / gzip / brotli | 168,396 / 24,478 / 20,487 B |
| `dist/web` total | 39,245,860 B (3 files: html, js, wasm) |
| Tests `cargo test --workspace --release` | exit 101 after 8810 s (stopped at `pdfcraft-automation --test automation`). `--no-fail-fast` rerun: 465 s, **3466 passed, 14 failed, 16 ignored** |

Failing tests (reason; env-related?):
1. `automation`: `a_save_that_does_not_reopen_leaves_the_file_as_it_was`: `unwrap_err()` on an Ok value, the save of a damaged PDF succeeded (automation.rs:359). Unknown; possibly Windows path/verify behaviour.
2. `automation`: `signing_with_translated_labels_through_tools`: `sign_id_create ... the password must have at least 8 characters`; the test uses "secret1" (7 chars): upstream test/code mismatch, not environment.
3-8. `ui-egui` lib `i18n::tests::bundled_catalogs_are_consistent`, `lithuanian_covers_bundled_catalog_keys_including_dynamic_dialog_labels`, `vietnamese_covers_bundled_catalog_keys`: zh-hant catalog has duplicate keys (lines 790, 1325). `vietnamese_covers_commands_and_catalogue`, `vietnamese_covers_ui_literals`, `signature_appearance_labels_are_in_every_catalog`: Vietnamese catalog lacks entries. Upstream data bugs, not environment.
9. `ui-egui` lib `canvas::tests::explicit_search_jumps_record_history_and_keep_match_destination`: assertion left "1" (canvas.rs:5118); unknown, likely logic/timing.
10. `ui-egui` tests/editing `files_dropped_beside_the_pages_panel_still_open_as_documents`: assertion left 1 (editing.rs:1781); unknown.
11. tests/fonts `primary_ui_fonts_cover_latin_and_cyrillic_catalogs`: `bg: JetBrainsMono lacks ['ѝ']`; font asset gap (maybe an optional fonts dir `CRAFT_FONTS_DIR` is absent); likely env/asset.
12. tests/print `print_dialog_shows_the_printer_driver_options`: expected printer options differ (left [("InputSlot","Tray2"),("media","Letter")]); print/CUPS-oriented, likely environment (Windows).
13. tests/view `single_page_swipe_that_reaches_the_edge_does_not_turn_but_a_fresh_one_does`: page 2 starts at -195 not at top; layout/timing, unknown.
14. `xtask` `parity::tests::committed_manifest_is_well_formed`: committed parity manifest TOML has a duplicate key (upstream bug).

Smoke test (`pdfcraft-cli`, log `E:/office-target/logs/pdfcraft-smoke.log`): subcommands info, render, text, edit, combine, extract, split, check, audit, batch, tools, run, mcp, ui. Used `crates/ui-egui/tests/data/form.pdf` (1,744 B, 5 form fields): `info` JSON OK, `text` OK (empty), `edit --rotate 1:90 --title Smoke --full` OK, `render --page 1 --dpi 96` OK (534x400 PNG), `combine`, `extract`, `check` (1 ok), `audit` (0 findings) OK. The fixture is tiny; no larger sample PDF ships in the repo.
