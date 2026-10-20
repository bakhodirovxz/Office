# Phase 2 WP11: CI coverage of the transliteration

Date 2026-10-20. Local runs of `scripts/ci.sh` (one implementation for local and GitHub).

## What runs where

| Surface | Job | Profile |
|---|---|---|
| `uz-translit` unit, golden, round-trip property tests; `uz-translit-wasm` native tests | `test-ours` (root workspace, nextest) | full, nightly; quick when the diff touches the translit surface |
| `docs-translit` engine tests and the 10 docx preservation tests (`tests/preserve.rs`) | `test-ours` (`domains/docs` workspace) | same |
| gridcraft / deckcraft / wordcraft carried translit modules and menu tests | `test <app>` (patched upstream workspace) | quick (affected domain), full, nightly |
| npm package: build, size budget, node tests against the golden list, Chromium smoke | `translit-npm` | full, nightly; quick on a translit diff |
| xlsx / pptx formatting preservation (8 comparisons, both directions) | `translit-preserve` (new): builds the release sheets and slides CLIs, runs `scripts/translit/preserve-check.sh` | full, nightly; quick on a translit diff |
| 108-page docx, command to finished layout, budget 2 s | `translit-perf` (new): `cargo test --release -p docs-translit --test perf perf_108 -- --ignored`; the test asserts the median stays under `BUDGET_MS` = 2000 | **nightly only** (quick and full do not run it; the test is `#[ignore]`) |
| npm package size | `sizes` (`[npm.uz-translit]` in `web/budgets.toml`) | full, nightly |
| Catalog rows and exports | `i18n` (lint, roundtrip, check-stale), `queue` (`register check`) | every profile |
| UI in 4 languages | manual screenshots (`docs/phase2/evidence/ui/`) | not automated |

"Translit diff" = `translit_changed()` in `scripts/ci.sh`: `crates/uz-translit*`, `domains/*/crates/*-translit`, `domains/*/tests/translit-fixtures`, `ci/npm/uz-translit`, `scripts/build-npm.sh`, `scripts/translit/`.
`ci.sh nightly` = `full` plus `translit-perf`, `release-tests`, `fidelity`, `xlsx-bench`; so the gate evidence for the perf budget comes from a nightly run.

Workflow: `.github/workflows/ci.yml` gained the jobs `translit-preserve` (not in quick, Windows runner) and `translit-perf` (nightly, Windows runner). `translit-npm` was already there. Every `uses:` is pinned to a commit SHA (the only non-SHA `uses:` are the local `./.github/actions/setup`). `actionlint` 1.7.12: clean.

## ru export (WP7 gap)

`cargo xt i18n export` wrote `catalog-ru.tsv` only with `--with-ru`, so the ru rows (ours included) were never produced by the build pipeline. It now always writes it (upstream's Russian plus our rows; test `export_always_writes_ru_with_our_rows`). `docs/UPSTREAM.md` is updated.
**Still open:** no app hook reads `catalog-ru.tsv` yet; the `ru` entry of `LANGUAGES` embeds upstream's own `ru.tsv` (gridcraft: `ru.rs`), so a Russian UI still shows the nine new labels in English. Shipping them needs a follow-up of the 0110 patches (build_office.rs would embed the merged ru file) in wordcraft, gridcraft and deckcraft (deckcraft has no upstream ru at all: its export holds only our 7 rows). It is a carried-patch change in three apps with ui-egui rebuilds, so it was not done in WP11.

## Gate results (this machine, 2026-10-20)

| Gate | Result |
|---|---|
| `ci.sh quick` | **exit 0**, 18 jobs incl. `test pdfcraft` (3554 s), `test-ours`, `translit-npm`, `translit-preserve` (the diff touched the translit surface) |
| `ci.sh test-ours` | exit 0: root 173 passed; domain suites (docs, sheets, slides, pdf) all passed |
| `ci.sh translit-npm` | exit 0 (32 s): package 30,671 B wasm brotli against 49,152 |
| `ci.sh test wordcraft` | exit 0: 2746 passed, 23 skipped (quarantine pass: 21 expected failures) |
| `ci.sh test gridcraft` | exit 0: 1675 passed, 3 skipped |
| `ci.sh test deckcraft` | exit 0: 1684 passed, 11 skipped |
| `ci.sh lint` | first run failed on `cargo fmt --check` of my xtask edit (fixed with rustfmt); the rerun inside `quick` is exit 0, clippy ratchets all baselined |
| `ci.sh sizes` | first run **failed**: docs wasm brotli 9,523,141 B, 14,979 B over the budget (delta +146,051 B against `delta_br_max` 131,072). Phase 2 added `docs-translit`, the rules and dictionary, 0120/0121 and the catalog rows to the docs wasm. `delta_br_max` raised to 163,840 by the budgets.toml rule (measured delta + 10 %, rounded up to 16 KiB); rerun: exit 0 (docs headroom 17,789 B, sheets 1,972, slides 9,502, pdf 16,734, npm 18,481). **An owner-visible budget change.** The dist was built at 16:38; rebuild and re-measure at the WP12 gate |
| `translit-preserve` | exit 0: 8 of 8 IDENTICAL (2 xlsx and 2 pptx, both directions) |
| `translit-perf` (extra) | exit 0: command to finished layout **344 ms** (to Cyrillic) and **303 ms** (to Latin) against 2000 ms |
| `actionlint` | clean |

Not run here: `ci.sh full` and `nightly` as a whole (WP12), the GitHub workflow itself (no remote). One compiler warning remains in `docs-translit/tests/preserve.rs` (deprecated `unescape_value`); the domain workspaces are not clippy-gated.
