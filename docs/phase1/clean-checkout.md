# Clean-checkout proof (WP14 step 1)

Exit-gate item: "a clean checkout builds desktop and web for all four apps; CI is green".
Clone: `git clone --branch phase-1 C:/Users/uge226/Desktop/office E:/office-target/p1/clean/office` (fresh directory), own target dir `E:/office-target/p1/clean/target`
(set with `OFFICE_TARGET`; see bug 1 and 2). sccache enabled with the shared cache: **0 hits of 1,020 compiles** (cache keys contain the checkout path). Machine: Win10, 12 cores, `-j 4`.
Documented steps followed: PLAN WP14 (`scripts/bootstrap.sh`, `scripts/ci.sh full`). Bootstrap took 5 s (craft-fonts 31 files OK from the cache clone).

## Result: not green. Every build, size, lint, test and check job passes in the clean clone after 6 script fixes, but `web-smoke` is red (spec never written) and 2 pdfcraft tests are flaky

`ci.sh full` did not run in one piece. The first runs exposed the bugs below. A session restart killed one run (rustc exit `0x40010004` = DBG_TERMINATE_PROCESS, nextest "infrastructure
error" while building the gridcraft-ui-egui tests): the restart, not code; the job was re-run and passed. The table gives, per job, the run in which it last passed in the clean
clone. All runs use the same target dir. `clean1` was the first run (cold); its root-workspace jobs shared the main checkout's `core` dir (bug 2), so they were repeated in `clean2`.

| job | result | wall time | run / note |
|---|---|---|---|
| queue | PASS | 540 s | clean2 (17 s in clean1) |
| lint | PASS | 1,443 s cold, 172 s warm | clean1, clean2; upstream clippy 1+2+2+1 warnings, all baselined |
| gen-check | PASS | 4 s | clean2 |
| deny | PASS | 26 s | clean2 |
| brand | PASS | 9 s | clean2 (binary scan: "no release binaries yet" at that point) |
| i18n | PASS | 5 s | clean2 |
| test-ours | PASS | 722 s | clean2 (3,791 s in clean1) |
| test wordcraft | PASS | 2,956 s | clean2, cold |
| test gridcraft | PASS | 1,148 s | clean3 (the clean2 attempt was killed by the session restart) |
| test deckcraft | PASS | 1,900 s | clean3 |
| test pdfcraft | PASS on re-run, FLAKY | 534 s warm (5,817 s cold run with 2 failures) | clean3 failed `combine_grid::a_long_file_expanded_renders_only_its_pages_in_view_within_three_threads` and `comments::a_released_pen_stroke_stays_drawn_until_the_page_raster_has_it` (3,489 of 3,491 passed); both passed in clean5. Load-sensitive, the same class as the quarantined pdfcraft `flaky` entries; not quarantined yet (needs more runs) |
| check docs, sheets, slides, pdf (native + wasm32) | PASS | 855 s | clean4 |
| bind | PASS | 535 s | clean3 |
| build desktop + CLI, release | PASS x4 | docs 31+306 s, sheets 505+133 s, slides 638+312 s, pdf 1,443+787 s | clean3 |
| build web, release (trunk) | PASS x4 | docs 1,405 s, sheets 581 s, slides 1,406 s, pdf 1,291 s | clean4 |
| sizes | PASS | 513 s | clean4; brotli headroom docs 26,112, sheets 23,668, slides 22,738, pdf 17,146 bytes; JS entry 20.8, 20.1, 19.6, 20.5 KB of 24.6 KB |
| actionlint | PASS | 1 s | clean3 |
| web-smoke | RED (infra, exit 2) | 0 s | `ci/web-smoke/playwright.config.ts` does not exist: the Playwright smoke spec was never written |

Not run: `release-tests`, `fidelity`, `xlsx-bench` (nightly profile only, not part of `full`).
The plan's line `GREEN: ... web-smoke x4` is therefore **not** reached.

## Bugs found and fixed in the main repo (each exposed by the clean checkout)
1. **`OFFICE_TARGET` could not be overridden.** `scripts/env.sh` assigned it unconditionally, although PLAN WP14 documents `OFFICE_TARGET=... ci.sh full`. Now `${OFFICE_TARGET:-...}` (8d749ae).
2. **Root-workspace builds shared the main checkout's target dir.** `.cargo/config.toml` hard-codes `E:/office-target/p1/core`, so xtask and the root tests of the clone wrote into the main checkout's `core`,
   and `ci.sh` could not find `$OFFICE_TARGET/core/debug/xtask.exe` (all four `test` jobs failed in 3 s). `env.sh` now exports `CARGO_BUILD_TARGET_DIR=$OFFICE_TARGET/core` (b12c208).
3. **`check <domain>` web step failed under sccache** ("filename or extension too long", os error 206) on the longer paths of a clone. The step now runs without `RUSTC_WRAPPER`, like the other wasm steps (b12c208).
4. **Deadlock: every `ci.sh build` job hung forever.** `ci.sh` holds the build lock and `scripts/build.sh` takes it again. `with_build_lock` is now re-entrant through `OFFICE_LOCK_HELD` (643ae33).
5. **`ci.sh check` used the wrong package name** (`docs` instead of `docs-desktop`): `exit 101` for all four domains (5ed9d41).
6. **Web builds failed or would have polluted the main checkout.** (a) `domains/*/apps/*-web/Trunk.toml` hard-code `dist = "E:/office-target/p1/dist/..."`; `build.sh` now passes
   `TRUNK_BUILD_DIST=$OFFICE_TARGET/dist/<d>-web` (7e50ec2). (b) trunk 0.21 mangles an absolute `E:/...` favicon href when the repo is on the same drive as the overlay
   (`error getting canonical path for "E:office-target\\..."`); the main checkout never saw it because it lives on C:. `build.sh` now writes a relative href on the same drive (2d44732).
   Re-verified: all four web builds pass in the clone.

## Documentation bugs (undocumented steps)
- There was **no README**. The clean-checkout steps existed only in PLAN WP14, which also named `E:/office-clean` and an `OFFICE_TARGET` override that did not work. Added `README.md`:
  prerequisites, command order, target-dir override, stale-lock and sccache notes.
- `bootstrap.sh` only checks tools and toolchain components against `scripts/tools.lock`; it installs nothing. Now stated in the README.
- Node and Playwright (needed by `web-smoke`) are not in `tools.lock`; moot until the spec exists.

## Still red or unverified
- `web-smoke` (spec missing). 2 flaky pdfcraft UI tests (not quarantined). `sync-upstream.sh --gates full` is still a stub.
  Update 98c0a02 (gate review 2026-10-19): spec written (`ci/web-smoke/`) but not executed yet; the 2 tests quarantined as `flaky`; `--gates full` implemented, never run.
- The `brand` job runs before `build` in the `full` profile, so its binary scan (`brand-scan-bin`) saw no binaries in clean2 and, in the final run, scans the binaries of the
  earlier runs (finding M3 in `docs/phase1/gate-review.md`).
- The evidence is stitched from runs clean1 to clean5 on one target dir, with the fixes applied between them. A single uninterrupted `ci.sh full` from zero was not run, and sccache gave no benefit.

## Final run (gate evidence): one uninterrupted `ci.sh full`, exit 0

- **Where:** clean clone `E:/office-target/p1/clean/office`.
- **Commit:** `3fe5f0c`, which includes the M3 fix (brand scan after the builds) and the 2 new pdfcraft `flaky` quarantines.
- **When:** 2026-10-20, 04:48 → 07:38.
- **Logs:** `E:/office-target/p1/clean/ci-final.log`; job logs in `E:/office-target/p1/clean/target/ci/final-run/`.

| job | result | wall |
|---|---|---|
| queue | ok | 361 s |
| lint (clippy: wordcraft 1, gridcraft 2, deckcraft 2, pdfcraft 1, all baselined) | ok | 3410 s |
| gen-check | ok | 32 s |
| deny | ok | 160 s |
| brand (source scan) | ok | 412 s |
| i18n | ok | 9 s |
| test-ours | ok | 790 s |
| test wordcraft / gridcraft / deckcraft / pdfcraft | ok / ok / ok / ok | 302 / 156 / 111 / 484 s |
| check docs / sheets / slides / pdf | ok ×4 | 40 / 34 / 68 / 72 s |
| bind | ok | 141 s |
| build docs / sheets / slides / pdf (desktop + CLI + web, release) | ok ×4 | 485 / 169 / 1620 / 417 s |
| sizes | ok | 526 s |
| brand (binary scan of this run's builds) | ok | 380 s |
| web-smoke (4 web dists, headless Chromium) | ok | 16 s |
| actionlint | ok | 1 s |

**Caveats**
- The target dir was warm, left over from the clean1–5 runs. This is not a cold build from zero.
- Only windows-gnu was built. The GitHub workflow has never run.
- Two load-sensitive pdfcraft tests failed in the attempts before this run and were quarantined as `flaky`: `e3b871d`, `3fe5f0c`.
