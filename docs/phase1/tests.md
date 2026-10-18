# WP6b: test execution (debug, cargo-nextest, quarantine)

Run 2026-10-18 on branch `phase-1`, BASE as in `patches/*/BASE` (wordcraft eae2421, gridcraft f1894c6, deckcraft 1dc6972,
pdfcraft 924c4da). Machine: 12 cores, 11.8 GB RAM, Windows 10, one build at a time, `-j 4` (`--build-jobs 4 --test-threads 4`).
Logs of every run are in `docs/phase1/evidence/tests/` (`<app>-off` = overlay off, `-on` = overlay on).

## How it runs

`scripts/ci.sh test <app|all>` (and the `release-tests` job) does, per app, in the upstream workspace `upstream/<app>` (patched tree):

1. overlay prepare (`cargo xt overlay`, Uzbek catalog export), unless `OFFICE_NO_OVERLAY=1` (the pristine-like baseline);
2. `cargo xt quarantine check` (format, classes, no placeholder names);
3. `cargo xt quarantine apply <app>`: runs only the quarantined tests (nextest profile `quarantine`, JUnit) and reports
   `quarantined Q (still failing, passed-flaky, stale, missing)`. A deterministic entry that passes is **stale**; two stale runs
   in a row, or an entry that matches no test, fail the job (exit 1). A `flaky` entry may pass without being stale;
4. `cargo nextest run --workspace --profile ci -E 'not (<quarantined>)'`: every other failure fails the job;
5. `cargo test --doc --workspace` (all four apps have zero doctests; kept because the plan asks for it, it costs a rebuild).

Files: `ci/nextest.toml` (profiles `ci` no retries, fail-fast off, slow-timeout 120 s x 5; `quarantine`; `flaky`),
`ci/quarantine.toml`, `xtask/src/quarantine.rs`. Entries may carry `overlay = "on"|"off"`, `name = "*"` (whole binary cannot
run here) and an `env` label (`any`, else applied when it starts with a token of `OFFICE_TEST_ENV`, default `win10`).
With the overlay off, classes `brand-overlay` and `i18n-incomplete` are not applied. nextest's own store is redirected to the
target directory (otherwise it writes `upstream/<app>/target`).

Note: the upstream `dev`/`test` profile is `opt-level = 1` (deps 2), so "debug" is not an unoptimised build; compile times
are close to release.

## Results

Counts are nextest `passed / failed (not quarantined) / skipped`; skipped = ignored tests + quarantined tests (filtered out).
"Q" = quarantined entries applied in that mode, split into still failing / passed (flaky) / stale.

### Overlay ON (how we ship) - final runs, all green

| App | passed | failed | skipped | quarantined (failing / flaky-pass / stale) | exit |
|---|---|---|---|---|---|
| wordcraft | 2748 | 0 | 21 | 19 (19 / 0 / 0) | 0 |
| gridcraft | 1665 | 0 | 2 | 1 (1 / 0 / 0) | 0 |
| deckcraft | 1676 | 0 | 10 | 4 (4 / 0 / 0) | 0 |
| pdfcraft | 3491 | 0 | 44 | 28 (27 / 1 / 0) + 1 unrunnable binary (3 tests) | 0 |

### Overlay OFF (pristine-like baseline, same BASE, same patches, no brand/i18n overlay)

| App | passed | failed | skipped | notes |
|---|---|---|---|---|
| wordcraft | 2760 | 0 | 9 | 7 env entries still fail (5 Phase 0 + 2 craft-fonts) |
| gridcraft | 1666 | 0 | 1 | no quarantine needed |
| deckcraft | 1678 | 1 | 7 | the 1 is the timing-flaky `control_server` test (quarantined afterwards); `replacefonts_tests` fails 10 of 10 here (entry `overlay = "off"`) |
| pdfcraft | 3516 | 3 | 16 | the 3 are load-sensitive perf budgets (quarantined afterwards, `flaky`); all 12 Phase 0 entries pass now (stale, removed) |

The overlay-off run is not rerun after the entries were added: a rerun would only move the 1 resp. 3 tests from "failed" to
"quarantined". The overlay changes 5 wordcraft, 1 deckcraft, 21 pdfcraft tests (brand name, community chrome off, update check
off, fallback fonts, uz-latn gaps) and 1 gridcraft test; all are in the quarantine table below, none is a logic regression.

### Fixes (not quarantined)

- gridcraft 0103 (carried patch + REGISTER row): upstream's new `crates/xlsx/src/tests/malformed.rs:247` read `Formula.expr/.text`
  as fields; our template series made them private, so `gridcraft-xlsx` lib tests did not compile (E0615 x3). Switched to
  `expr()` / `text()`. A grep of the tree found no other field use.
- gridcraft 0104 (carried patch + REGISTER row): `theme::tests::fallback_faces_are_appended_to_every_family` failed with the overlay
  on because patch 0102 appends `office-fallback-*` faces after the script fallbacks; the test now skips those keys. This was a
  real breakage by our own patch, so it is fixed, not quarantined.
- `scripts/sync-upstream.sh --gates quick` now also runs `cargo check --workspace --all-targets` on the upstream tree, which
  would have caught 0103 (documented in `docs/UPSTREAM.md`).
- xtask clippy: 9 `collapsible_if` fixed (let chains); `cargo clippy -p xtask --all-targets -D warnings` clean.

## Timings (this PC, `-j 4`, persistent target `E:/office-target/p1/upstream-<app>`)

Compile and run are separated by nextest's own summary (run) against the step wall time (compile = step - run).
The first run of each app is cold (no test-profile artifacts); "overlay switch" is the rebuild when `OFFICE_OVERLAY_DIR` appears
or changes (the overlay hook reads files at build time). Phase 0 release-test totals were 0.9-2.4 h per app.

| App | cold compile (tests built) | test run, overlay off | test run, overlay on | overlay-switch rebuild | doctest step (compile only) |
|---|---|---|---|---|---|
| wordcraft | about 50 min | 245 s (2760 tests) | 154 s (2748) | about 30 min | 28 min cold, 3-7 min warm |
| gridcraft | about 40 min | 228 s (1666) | 231 s (1665) | about 25 min | 12 min cold, 1.5-2.5 min warm |
| deckcraft | about 29 min | 93 s (1678) | 91 s (1676) | about 20 min | 14 min cold, 5 min warm |
| pdfcraft | about 90 min | 333 s (3516) | 571 s (3491) | about 60 min | 43 min cold, 1-6 min warm |

So the run itself is 1.5-9.5 min per app, far below the Phase 0 release figures (those were dominated by release linking
and recompiles), but a cold debug build costs 0.5-1.5 h per app, 3.5 h for the four, plus a similar amount again for the first
overlay-on run. The PLAN target of "<= 30 min warm per app" holds for all four **once the target dir is warm and the overlay did not change**;
it does not hold for the first run after a sync that touches many crates.

Four causes of avoidable rebuilds were found and fixed in the scripts (all runs above that took far longer than their test time
were hit by them; after the fixes a warm deckcraft job is 141 s for the blocking pass):

1. running `cargo xt` (= `cargo run`) exports `CARGO_MANIFEST_DIR/CARGO_PKG_*/CARGO_BIN_NAME`, which made the nested nextest rebuild
   deckcraft and pdfcraft (72 min extra for pdfcraft). The xtask is now run as a built binary;
2. `quarantine apply` now sets the same `CARGO_TARGET_DIR/CARGO_BUILD_JOBS` as the blocking pass;
3. deckcraft's `crates/fonts/build.rs` prints `rerun-if-changed` for `fonts/latin-manifest.txt`, which our craft-fonts pin does
   not contain; cargo treats a missing file as always changed, so that build script and the 11 crates above it rebuilt on **every**
   cargo invocation (about 20 min each, also for `cargo test --doc`). The test job now uses a copy of the pin with an empty
   `latin-manifest.txt` in `E:/office-cache/craft-fonts-ci` (same embedded fonts). The same file is missing in release builds
   (`build.sh`), so every deckcraft build there also rebuilds that chain; worth the same fix (or adding the file to the pin) when
   the owner agrees;
4. `cargo xt overlay` rewrote every overlay file on each call, moving mtimes and triggering build-script reruns in the app crates;
   `office-brand` now writes a file only when its content changed.

sccache: `RUSTC_WRAPPER=sccache` was active for all runs. No clean A/B was made (a cold rebuild is 0.5-1.5 h per app, so it
was not worth the machine time). With persistent target directories cargo's own fingerprints already avoid recompiling, so
sccache can only help the cold start and the overlay-switch; the server counters at the end of the session (187 requests,
26 executed, 20 hits, 5 misses) show it is mostly bypassed (non-cacheable crate types, incremental off). Verdict: not
measurably helpful for the test job; keep it for cold starts on clean machines.

## deckcraft ui-egui crash (STATUS_ACCESS_VIOLATION)

Phase 0: the `deckcraft-ui-egui` lib test binary died with 0xC0000005 when run multi-threaded (cargo test 2 of 2, direct 1 of 8),
never with `--test-threads 1`.

- **Under nextest it does not reproduce.** `cargo nextest run -E 'binary_id(=deckcraft-ui-egui)' --stress-count 10` (4 test
  processes at a time, 211 tests): 10 of 10 iterations completed with no crash (the only failure in each is the
  quarantined `replacefonts_tests` test, overlay off). The binary was also part of 3 full-suite nextest runs (overlay off, overlay on, and the final
  overlay-on run), again without a crash: 13 runs of the binary in total, 0 crashes. nextest's process-per-test isolation avoids the problem; no `gpu-serial` group is needed
  and the placeholder quarantine entry `UNIDENTIFIED` was removed.
- **The in-process crash still exists** (debug build, the same test binary run directly with default threads): 6 of 10 runs
  segfaulted (exit 139; PowerShell showed `C0000005`). It is therefore not a Phase 0 release-only artefact.
  Narrowed in about 40 minutes:
  - module `dialogs::` crashes (5 of 8 runs), `texture_limit_tests`, `slide_size_tests`, `icons::` do not (0 of 8);
  - inside `dialogs::` only `dialogs::spelling_tests` crashes (3 of 8 as a group; every other `dialogs::*` module 0 of 8);
  - each of its 4 tests alone: 0 of 12; the whole group with `--test-threads 1`: 0 of 10;
  - pairs with `--test-threads 2`: `squiggles_mark_misspelt_words_in_the_background` and
    `squiggles_in_table_cells_and_notes_and_on_the_ui_thread` crash with **any** other spelling test (3-5 of 12); the pair
    `notes_menu_language_dialog_and_repeated_words` + `spelling_dialog_changes_and_ignores_words` does not (0 of 12).
  - Both squiggles tests call `deckcraft_engine::spell::install(...)` and `set_user_dictionary_path(...)` (process-wide
    `OnceLock<Mutex<..>>` state in `crates/engine/src/spell.rs`, lines 556-612) and run `app.squiggles.settle` (a background
    thread). `deckcraft-ui-egui` has `#![forbid(unsafe_code)]`, so the access violation is below it (the spell checker's
    dependencies or the render stack).
  - Not resolved: which unsafe code is hit. Next steps for an opus agent: run the two squiggles tests together under
    `cdb`/WinDbg or with `RUSTFLAGS=-Zsanitizer=address` (needs nightly), or with the `spell` module's statics replaced by
    per-test dictionaries; check whether a release-profile `cargo test` still crashes.
- Consequence for CI: never run this lib with `cargo test` multi-threaded; `ci.sh` uses nextest everywhere. If a flaky-crash ever
  appears under nextest, quarantine the single test with class `flaky-crash` and put its binary in a `gpu-serial` group (stub in
  `ci/nextest.toml`).

## Quarantine table summary (`ci/quarantine.toml`, 54 entries, `cargo xt quarantine check` = OK)

| App | env | upstream-bug | flaky | flaky-crash | brand-overlay | i18n-incomplete | debug-only | total |
|---|---|---|---|---|---|---|---|---|
| wordcraft | 7 | 0 | 0 | 0 | 11 | 1 | 0 | 19 |
| gridcraft | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 |
| deckcraft | 1 | 0 | 1 | 0 | 3 | 0 | 0 | 5 |
| pdfcraft | 1 | 0 | 4 | 0 | 20 | 4 | 0 | 29 |

Re-validation of the seeds at the fresh BASE:

- **pdfcraft: 12 of the 14 Phase 0 entries are gone** (all 12 now pass: signing password, the 5 i18n catalog tests, the parity
  manifest, the 4 "unknown cause" tests, print dialog; upstream fixed or the failure was Phase 0 environment), 1 test no longer
  exists upstream (`a_save_that_does_not_reopen_leaves_the_file_as_it_was`), 1 (`primary_ui_fonts_cover_latin_and_cyrillic_catalogs`)
  passes with `CRAFT_FONTS_DIR` set as `scripts/env.sh` does. They were the `provisional` ones; none needs a diagnosis any more.
- **wordcraft: the 5 Phase 0 env entries still fail** (emoji x2, Alt+numpad, Breaks x2: locale/font of this PC). New: 2 `env`
  entries (`wordcraft-fonts` tests assume no bundled Thai font, false with `CRAFT_FONTS_DIR`), 11 `brand-overlay`, 1 `i18n-incomplete`.
- **deckcraft:** `replacefonts_tests` fails only with the overlay off (10 of 10) and passes with it on, so it became `overlay = "off"`;
  new: 1 `flaky` (`control_server::...blocked_ui_times_out...`, 1 of 30 nextest runs fails: a 50 ms sleep race), 3 `brand-overlay`.
- **gridcraft:** 1 `brand-overlay`; the fallback-faces failure is fixed by patch 0104, not quarantined.
- **pdfcraft new:** 20 `brand-overlay` (product name, community chrome off, update check off, fallback fonts), 4 `i18n-incomplete`
  (uz-latn gaps), 4 `flaky` (3 frame/time budgets that fail under 4 parallel processes and pass alone, 1 poll-with-timeout test),
  1 `env` with `name = "*"`: **`pdfcraft-sign::store_keys` cannot run on this PC** - Windows Defender deletes the test .exe right
  after the link (`Trojan:Win64/Aotera.CP!MTB`, a heuristic false positive; detections 11:59 and 12:07). nextest then cannot list the
  binary and the whole run fails (exit 104), so the binary is excluded and its 3 tests are NOT run here. A Defender exclusion for
  `E:\office-target` (needs admin) fixes it; the owner/admin must decide.
- No `upstream-bug`, `debug-only` or `flaky-crash` entry remains. No entry covers a failure of our own patches other than classes
  `brand-overlay` and `i18n-incomplete`; every `brand-overlay` entry is an upstream test that asserts the upstream product name or chrome.
  The architect review of `quarantine.toml` for honesty (PLAN WP6b) is still to be done.

Review triggers: every entry has `review` (default "every sync"); `brand-overlay` and `i18n-incomplete` shrink when WP9e adds the
uz rows and when upstream tests read the brand config; `store_keys` goes when an exclusion exists.

## test-ours (root workspace and domain crates)

`scripts/ci.sh test-ours`: root 59 passed; domain docs 64, sheets 44, slides 43, pdf 71 passed, 0 failed. The domain crates compile
in debug for the first time here (docs 32 min, sheets 20, slides 30, pdf 40 min cold). Two findings, both fixed:

- `gen-domain` did not carry the upstream app's `[dev-dependencies]`, so `pdf-desktop` (the upstream `main.rs` with its tests) did not
  compile its tests (`pollster`). `xtask/src/domain.rs` now copies `dev-dependencies` (also the target-specific ones); the domain
  locks changed by 2 lines each and `lockdiff` still reports 0 version changes.
- Three pdfcraft `main.rs` tests (`egui_wgpu_carries_the_surface_size_fit`, `egui_wgpu_returns_a_surface_it_cannot_configure_as_an_error`,
  `winit_carries_the_windows_11_monitor_scale_fix`) read `vendor/` and `Cargo.lock` relative to `CARGO_MANIFEST_DIR/../..`, a path that
  only exists in the upstream workspace. They are excluded from the pdf domain run in `ci.sh` (comment there) and are covered by the
  upstream suite, where they pass.

## Upstream clippy (from the lint run)

Upstream clippy warnings counted by `ci.sh lint` (baseline "none"): wordcraft 1, gridcraft 2, deckcraft 2, pdfcraft 1 (the order in the
lint log). PLAN WP6 does not define the ratchet baseline file, so they are only recorded here; `ci.sh lint` keeps reporting counts as advisory.

## Not done / open

- Architect review of `ci/quarantine.toml`.
- Defender exclusion for the `store_keys` binary (3 tests are not run here).
- A rerun of wordcraft/gridcraft/pdfcraft after the final script fixes (3 and 4 above) was not done; their last runs are the green ones listed. Only deckcraft was rerun after fix 3 (green, 141 s blocking pass).
- The `release-tests` job is wired to the same function (`--release`), but its quarantined pass does not yet pass `--release`; not run.
