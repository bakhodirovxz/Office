# Phase 3 WP12: CI for the converter

Date 2026-10-21. One implementation: `scripts/ci.sh <job>`; `.github/workflows/ci.yml` mirrors the jobs (SHA-pinned, still `workflow_dispatch` only, not enabled; `actionlint` clean).

## Jobs

| Job | What it runs | Profiles |
|---|---|---|
| `convert` | `cargo clippy -D warnings` and `cargo nextest run` on `services/Cargo.toml` (both crates, `ci` profile), `cargo deny` with `ci/deny/services.toml` | full, nightly; quick when the diff touches `services/**` |
| `convert-e2e` | release build of the services and of the four domain CLIs; `convert-api --test e2e` (every `GET /formats` row over HTTP, hard kill, empty `work/`) with `CONVERT_API_E2E_REQUIRED=1`; `services/convert/tests/directions.sh`; the Playwright spec `ci/web-smoke/converter.spec.ts` | full, nightly |
| `convert-ui` | release build of the services + the Playwright spec only (`CONVERT_API_BIN`, `CONVERT_API_FAKE_TOOL` are set by the job) | quick when `i18n/converter.tsv` or `services/convert-api/ui/**` changed (the `i18n` job runs in every profile) |
| `sidecars` | host-side `fetch-wheels.sh` (only when `requirements.lock` changed), `docker build` and `test.sh` for `sidecars/pdf2docx` and `sidecars/soffice`, with `MSYS_NO_PATHCONV=1`. No Docker running: exit 2 with a message, never a skip | nightly; quick when `sidecars/**` changed |
| `fidelity-directions` | `python -I scripts/fidelity/run.py --directions all`, then `scripts/fidelity/silent_baseline.py check`: fails on any loss without warning not in `ci/fidelity/silent-baseline.json` | nightly |

Quick triggers are `diff_has` in `scripts/ci.sh` (diff against `CI_DIFF_BASE`, default `HEAD~1`).

Inputs: the e2e tests use the synthetic WP1 set in `E:/office-target/p3/census/in` (override `CONVERT_INPUTS`; the Rust tests hard-code that path). When a file is missing, `convert-e2e` regenerates the set with `scripts/fidelity/census.ps1 -Phase prep` (Office 16 COM, only PIDs it started). Nothing comes from `corpus/`.

Silent-loss baseline: `ci/fidelity/silent-baseline.json`, keys `direction|fixture|feature`, seeded from `summary.json` of 2026-10-21 (73 entries). When a loss is fixed the job prints "gone"; shrink the file with `python -I scripts/fidelity/silent_baseline.py seed <out>/summary.json`. x2t is optional (`X2T_URL`); `CI_FID_NO_OFFICE=1` drops the Office reference.

Known gap: `services/` is not rustfmt-clean (about 500 hunks), so `convert` has no fmt check yet.

## Wall times (this PC)

| Job | Wall time | Notes |
|---|---|---|
| `convert` | 32 s warm (clippy cached, 135 tests in 25 s); 10 min 15 s cold | cold: clippy 6 min, then the test build |
| `convert-e2e` | 19 min 14 s | 12 min of it rebuilt the sheets/slides/pdf CLIs (their sources had changed); then api-e2e 39 s, directions.sh 13 s, `npm ci` 21 s, Playwright 15 s (7 passed, 1 screenshot test skipped without `UI_SHOTS`). Without builds: about 1.5 min |
| `convert-ui` | about 40 s with a built converter | build services + `npm ci` + 15 s of Playwright |
| `sidecars` | 6 min 19 s (two image builds with a cold layer cache, 9 + 9 tests) | with Docker stopped: exit 2 and "Docker is not running" at once |
| `fidelity-directions` | 12 min 18 s | the run itself 530 s (38 directions, 65 cases, MS Office 16 reference, no soffice/x2t); 73 silent losses = the baseline, 0 new |

Negative check: removing one key from a copy of the baseline makes `silent_baseline.py check` print `NEW silent loss: ...` and exit 1.

## Observed while testing (not caused by WP12)

- At commit 3f699b6 `convert` failed one test, `convert-api::e2e every_direction_over_http_then_a_hard_kill_leaves_work_empty_after_restart` (assertion `the finished jobs are kept for the retention time`, `e2e.rs:244`); with the services builder's working tree it passes.
- `ci.sh quick` could not go green on a shared working tree: `lint` stopped at `gridcraft-numfmt` (`en_us_golden`, a test that did not compile mid-change by the NumLocale work).
