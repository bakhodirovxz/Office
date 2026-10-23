# Phase 4 integration pass (2026-10-23)

Status: **stopped, not green.** E: reached 0 bytes free during `build all` (the stop rule is 5 GB), so the full gate did not finish.
This file records what ran, the reds fixed at their root, and what is still owed. Wall times are on a machine that other agents' builds
shared the whole time (see "Contention").

## Reds from the lane reports, fixed

| Red | Root cause | Fix (commit) |
|---|---|---|
| `lockdiff-docs` | `suite-core` (toml_edit, ron, base64 0.21, ...) is in the domain lock; lockdiff counted its private deps as drift | `xtask/src/domain.rs`: packages only `suite-core` pulls in are counted as ours (a package any other package also needs is still drift); output says "N ours via suite-core"; unit test. docs/sheets/slides: 6 ours, 0 drift (03626b3, clippy fix in a later commit) |
| `deny root` | `zip 2.6.1` (suite-core dev-dependency): RUSTSEC-2026-0367 / 0368 | `zip` 9 in `crates/suite-core` (the same major as services), two `.name()` call sites adapted; `cargo deny` root, docs, sheets, slides, pdf and services all ok (03626b3, 126a782 for the services lock) |
| `brand-grep` | a doc comment in `recent.rs:49` carried the working name | comment reworded (03626b3); after the Hoshiya rename 0 hits |
| `shell.spec.ts` "template cards", "lang uz-Latn-2026" | not reproducible on HEAD: 19 of 19 pass against the current shell, the reviewed templates and fresh `shell-dist` output. The failing run had a stale `dist/shell` copy (it is only refreshed by `scripts/shell-dist.sh`) | no code change; `ci.sh shell` run int3b |
| cargo deny for the new deps | ron, toml_edit, getrandom, the suite-core path deps and the dictionaries | `deny` job ok for root + 4 domains (13 s, 35 s); services `ci/deny/services.toml` ok (also after the new `winresource`/`toml` build-dependencies); dictionary licences were already in NOTICE (`docs/phase4/licences.md`) |

Found and fixed while running the gate:
- `lint`: clippy `redundant_closure` in my lockdiff code (fixed). `layers-*`/`assets-*` for wordcraft, gridcraft and deckcraft failed with
  "os error 267" because sccache served an `xtask.exe` built in the deleted WP0 sync worktree (the path is baked in with `env!("CARGO_MANIFEST_DIR")`).
  Rebuilt the three xtasks without sccache (`touch upstream/<app>/xtask/src/main.rs`, `RUSTC_WRAPPER= cargo build -p xtask`); no repo change. Not yet re-run.
- `convert` fmt: 6 rustfmt hunks in `services/suite-desktop` and `suite-serve` (fixed).
- `suite-desktop` host test `without_webview2_...` raced a loaded machine (exit hook 4 s): now 15 s.

## Job table (this tree, runs int1 to int4)

| Job | Result | Wall | Note |
|---|---|---|---|
| queue | ok | 880 s (int2); 10,279 s (int3) | verify-queue and register check take seconds; the rest was waiting for a build slot behind another agent's `ci.sh quick` |
| gen-check (gen-domain, lockdiff x4) | ok | 35 s | |
| deny (root + 4 domains) | ok | 13-35 s | services deny ok inside `convert` |
| brand (brand-grep, brand-scan-bin) | ok | 631 s | run before the logo/rename commits' binaries; rerun owed after `build all` |
| convert-ui (services release build, converter spec, TLS spec) | ok | 298 s | |
| shell (suite-serve nextest, shell-dist, 19 Playwright, sizes) | ok | 1,572 s | stale editor dists (built before the rename); rerun owed on fresh dists |
| sizes | ok | 719 s (inside shell) | docs 9,853,705 / 9,871,648 B; sheets 4,943,443 / 4,966,771; slides 9,492,465 / 9,524,217; pdf 9,569,362 / 9,599,462; shell 48,443 / 65,536 |
| convert | **fail** (first run: fmt, fixed) | 1,076 s; rerun 813 s | rerun: clippy ok; nextest 236/239, the 3 failures are load timeouts (`containment::killing_convert_kills_the_tool_it_started`, `empty_file` 41 s, the host test above); all three passed in the int3b-era run (927 s). Rerun owed on a quiet machine |
| build all (4 desktop, web, CLI) | **fail: disk full** | 2,285 s | `build-docs-desktop` ok (1,557 s); everything after died with os error 112 |
| test-ours, test x4, check x4, bind, convert-e2e, translit-npm, translit-preserve, web-smoke, actionlint, i18n, lint, brand (after builds) | **not run** | - | blocked by the disk |

## Hoshiya rename and logo (commits 800ee09, 126a782)

See the report: `brand/brand.toml` names, bins, app ids, aumids, data dirs; `hoshiya.exe` and `hoshiya-serve.exe`; logo pack in `brand/logo/`
(owner's brand, `LICENSE.txt`, excluded from the MIT/Apache claim in NOTICE); `[colors]` accent_light #1E4FA3, accent_dark #6C9BF0,
accent_margin #E9A23B, ink #101828 with two new contrast checks; app icons with the margin line (`docs/phase4/evidence/brand/`); exe icon and
version info from the brand file (checked on the built `hoshiya.exe`); shell favicon, manifest and header lockup.

## Templates (web, through the shell, real editors)

`ci/web-smoke/templates.spec.ts` (new) opens each of the 5 starters from the home screen and reaches the editor's open message with no page
error and no CSP violation; screenshots in `docs/phase4/evidence/templates/`. Looked at by hand: Docs (buyruq, xizmat xati) and Slides (taqdimot)
show the Uzbek text with oʻ, gʻ and the apostrophe forms and no tofu. Sheets: frozen panes hold in tabel (3 columns, 4 rows) and xodimlar (4 rows);
the tabel legend was clipped at the frozen split, so it now starts in column D (`scripts/gen-templates.py`, `tabel.xlsx` regenerated). COUNTIF totals
computed with the Sheets CLI on a copy with D5:H5 = I,I,K,T,S: AI..AM = 2, 16, 1, 1, 1 and the Jami row the same; empty rows show 0.
Slides shows the editor's "Arial isn't installed" notice (taqdimot uses Arial; that is the web editor's normal message for any pptx).
**Desktop through the shell was not run** (needs the rebuilt `hoshiya.exe` and editors).

## Owed (not done, with the reason)

- Full gate on HEAD (disk).
- `scripts/smoke-desktop.ps1`, `scripts/desktop-editors-check.mjs` (Slides included), the shared-toolbar-setting check across two apps, the desktop template run: need the rebuilt exes.
- WP12 w80 GUI open-time re-run: the machine was never idle (other agents built throughout), and E: is full. `docs/phase4/excel-guard.md` is unchanged.

## Contention (why times are long)

Two `ci.sh quick` runs from other sessions held the p1 build slot for hours (WP0 run 20261023-023719 from 02:37; its `test pdfcraft` took
14,663 s), and p4 clean-checkout nextest runs were active. The lock rule "one build per target dir" serialises everything on `E:/office-target/p1`.
Upstream test targets are very large: `upstream-gridcraft` 152 GB, `upstream-pdfcraft` 88 GB, `upstream-deckcraft` 87 GB, `upstream-wordcraft` 51 GB
(per-test debug executables of about 540 MB each), which is what filled E:.
