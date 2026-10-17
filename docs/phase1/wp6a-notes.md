# WP6a notes: CI skeleton

Executed 2026-10-17 on branch `phase-1`. Scripts and config only; no cargo build was run (cargo deny, cargo fmt --check,
actionlint and awk checks do not compile). Nothing is committed.

## Files

- `scripts/ci.sh`: jobs and profiles of PLAN 6.1 (`queue lint gen-check deny brand i18n test test-ours check build sizes
  web-smoke actionlint release-tests fidelity xlsx-bench sync-dry-run`; `quick|full|nightly`). Writes
  `$OFFICE_TARGET/ci/<run>/<job>.json`, `<job>.log` and `summary.md`. Exit 0 green, 1 blocking failure, 2 infrastructure.
  `--no-build` skips every compiling step (listed as skipped); this is the dry-run mode.
- `.github/workflows/ci.yml` (+ composite `.github/actions/setup/action.yml`): `workflow_dispatch` only, matrices, timeouts,
  concurrency, every step calls `scripts/ci.sh <job>`. Actions are listed by version in comments (pin by SHA on enabling).
- `deny.toml` (root workspace) and `ci/deny/{docs,sheets,slides,pdf}.toml`, one per workspace, modelled on
  `upstream/pdfcraft/deny.toml`, permissive policy of PLAN 6.6.
- `web/budgets.toml`: Phase 0 numbers per domain, `delta_br_max = 524288` guard, `js_br_max = 24576`, headroom rules in the header.
- `ci/quarantine.toml`: format documented in the header, 21 seed entries (wordcraft 5, deckcraft 2, pdfcraft 14), one reason each.

## Deviations and interfaces

- Plan says one `deny.toml` for all five lockfiles; the task asked for one config per workspace, so root has `deny.toml` and the
  domains have `ci/deny/<d>.toml` (the shared sections are identical; per-domain `ignore` lists differ).
- `cargo xt sizes` / `brand-grep` are xtask work (not in scope here). `ci.sh sizes` is implemented in shell with the `brotli` CLI
  (`brotli -q 11 --lgwin=24`, `/mingw64/bin/brotli`) and `gzip -9`; switch to the xtask when it exists. `brand` and `i18n` call the
  xtask only if its source mentions the subcommand (`xt_has`), else they are reported as stubs.
- Interfaces read from WP3 (not defined here): `domains/<d>/Cargo.toml` with packages `<d>`, `<d>-web`, `<d>-cli`;
  `scripts/build.sh <d> desktop|web|cli --release`; web dist in `$OFFICE_TARGET/dist/<d>-web` (override `CI_DIST_ROOT`).
  If `domains/<d>` is missing the jobs use `upstream/<app>` as a stand-in and say so.
- Domain `deny` configs set `wildcards = "warn"` (not deny): the upstream workspace crates are path deps without
  `publish = false`, which cargo-deny treats as public crates. Switch to `deny` when the generated domain manifests set it.
- Not written (out of the stated scope): `ci/nextest.toml` (WP6b; `test` returns exit 2 without it), `ci/web-smoke/`
  (`web-smoke` returns exit 2 without it), the xtask commands.
- Workflow limits: `scripts/env.sh` hard-codes E: paths and does not honour pre-set values, so the workflow cannot work on a
  runner until that changes (stated in the workflow header). Untested; only actionlint ran.

## cargo-deny findings (cargo-deny 0.20.2, advisory DB at E:/office-cache/advisory-db)

| Workspace | Licences | Advisories | Bans |
|---|---|---|---|
| root (core) | ok | ok | ok |
| docs (wordcraft) | ok after `epaint_default_fonts` exception | zip RUSTSEC-2026-0367/0368/0369 (ignored: `extract()` is never called), ttf-parser 0192 and rustybuzz 0206 unmaintained (ignored) | ok (wildcards warn) |
| sheets (gridcraft) | ok after the same exception | zip 0367/0368/0369 (ignored) | ok |
| slides (deckcraft) | **FAILS: 16 symphonia-* crates are MPL-2.0** (`deckcraft-media`, unconditional dependency) | zip x3, ttf-parser, rustybuzz (ignored) | ok |
| pdf (pdfcraft) | ok after exceptions `epaint_default_fonts`, `webpki-root-certs` (CDLA-Permissive-2.0) | RUSTSEC-2023-0071 (rsa, verification only), 2024-0370 (proc-macro-error) ignored, as upstream does | ok |

MPL-2.0 is not allowed (CLAUDE.md, PLAN 6.6), so no exception was added. This needs an owner decision: replace symphonia in
deckcraft (carried patch), accept an MPL-2.0 exception explicitly (file-level copyleft, would be a policy change), or drop audio
decoding from the slides web build. `multiple-versions = "warn"` reports many duplicates (not failures).

## Other findings from the dry runs

- `lint` fails on formatting: our patched upstream files are not `cargo fmt`-clean (wordcraft 3 files, gridcraft 6 files, all
  touched by patches in `patches/`), and root `xtask/src/{domain,main}.rs` (WP3 work in progress). deckcraft and pdfcraft are clean.
- `brand` fails the logo-include check: `upstream/pdfcraft/crates/ui-egui/src/widgets.rs:506,508,516` include `docs/brand/*`.
  That is what WP7 removes; the check works.

## Done-checks (commands and actual output)

```
$ scripts/ci.sh deny
=== job deny
ok root
ok docs
ok sheets
FAIL slides: 16 error(s); advisories ok, bans ok, licenses FAILED, sources ok
         16 error[rejected]: failed to satisfy license requirements
ok pdf
=== deny: fail (exit 1)
rc=1
```
Four `ok` lines and one blocked by the MPL-2.0 finding above (not the "five ok, exit 0" the plan wants).

```
$ scripts/ci.sh --no-build sizes        # no dist exists: Phase 0 baseline against the budgets
domain  (src)              raw         gzip       brotli       budget     headroom js_br/max
docs    phase0        42597532     15849354      9441597      9965885       524288 20555/24576
sheets  phase0        18973730      7041161      4731289      5255577       524288 20036/24576
slides  phase0        35120783     13905434      9300367      9824655       524288 19665/24576
pdf     phase0        39076254     14379872      9390759      9915047       524288 20487/24576
=== sizes: ok (exit 0)
```
The measured path (real dists) was exercised with fake 3 MB wasm files: the table was built from `brotli`/`gzip` output and a
forced tiny budget gave `FAIL docs: wasm brotli 3000008 exceeds budget 524388 ...`, exit 1. Real numbers need the WP3 web builds.

```
$ actionlint .github/workflows/ci.yml; echo rc=$?          (E:/office-tools/bin)
rc=0
$ scripts/ci.sh actionlint
=== actionlint: ok (exit 0)
```

```
$ scripts/ci.sh --no-build queue lint gen-check
=== queue: ok-with-skips (exit 0)      # verify-queue: 5 repos tree OK; quarantine format "21 entries, 0 problems"; register check skipped (compiles xtask)
=== lint: fail (exit 1)                # fmt findings above; clippy/layers/assets skipped
=== gen-check: ok-with-skips (exit 0)  # xtask steps skipped
rc=1
```
`queue` and `gen-check` pass in dry-run; `lint` fails for the real formatting reasons listed above, so the plan's
`queue lint gen-check exits 0` is not met yet.

## Deferred until builds exist

`cargo clippy` (root and the upstream ratchet, baselines go in `ci/clippy-baseline/<app>.count`), `cargo xtask layers|assets`,
`cargo xt register check|gen-domain|lockdiff|brand-grep|i18n lint`, `test`, `test-ours`, `check`, `build`, real `sizes`
(including re-measuring the pristine BASE with `--measure-base`), `web-smoke`, and the nightly/weekly jobs. shellcheck is not
installed, so `ci.sh` was checked with `bash -n` only.
