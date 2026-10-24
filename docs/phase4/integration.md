# Phase 4 integration pass (2026-10-23)

Status: **green on HEAD.** Every job of the gate ran and passed (table below). Wall times are real, measured on this PC; the machine was shared with
other agents' builds for the first part of the day (see "Contention") and idle for the final run.

## Reds from the lane reports, fixed at the root

| Red | Root cause | Fix |
|---|---|---|
| `lockdiff-docs` | `suite-core` (toml_edit, ron, base64 0.21, ...) is in the domain lock; lockdiff counted its private deps as drift | `xtask/src/domain.rs`: packages only `suite-core` pulls in count as ours (a package any other package also needs is still drift); output "N ours via suite-core"; unit test. docs/sheets/slides 6 ours, 0 drift; pdf 0 (03626b3, 6ef6158) |
| `deny root` | `zip 2.6.1`, a `suite-core` dev-dependency: RUSTSEC-2026-0367 / 0368 | `zip` 9 (the services' major), two `.name()` call sites adapted. deny ok for root, 4 domains and services (03626b3) |
| `brand-grep` | a doc comment in `recent.rs:49` carried the name | reworded; after the rename 0 hits (03626b3) |
| `shell.spec.ts` "template cards", "lang uz-Latn-2026" | not reproducible on HEAD: 19 of 19 pass (int3b, again in int5). The failing run used a stale `dist/shell` copy, which only `scripts/shell-dist.sh` refreshes | no code change |
| cargo deny, new deps | ron, toml_edit, getrandom, suite-core path deps, dictionaries | root + 4 domains + services ok, also after the new `winresource` and `toml` build-dependencies of the host; dictionary licences are in NOTICE and `licences.md` |

Found by running the full gate, and fixed:
- **gridcraft test `cell_pictures::pictures_fit_current_geometry_and_selection_cannot_erase_them`** (the Sheets lane could not run `test gridcraft`): the simplified
  toolbar of patch 0140 draws its icons as textured meshes, so the test counted 2 meshes instead of the picture. New carried patch **gridcraft/0142** (1 file,
  +3/-1, the test's `image_meshes` keeps only meshes that touch the grid), REGISTER row, `verify-queue` ok (45 patches), `register check` ok (a890da1).
- **pdfcraft `about_dialog_has_contributors_and_models_tabs` and `simplified_chinese_about_tabs_and_credit_controls`; lint `layers-*`/`assets-*` os error 267.**
  One cause: sccache served build scripts and `xtask.exe` that were compiled in the deleted WP0 worktree (`E:/office-target/p1/sync/wp0-main/...`) with
  `env!("CARGO_MANIFEST_DIR")` baked in, so they looked for `contributors/` and the xtask workspace in a folder that no longer exists. Fixed in the build
  environment, not in the repo: `touch` the `build.rs` or xtask `main.rs` and rebuild that package once with `RUSTC_WRAPPER=` empty. After that both tests pass and
  `test pdfcraft` is green. The poisoned entries lived in the shared sccache store; **fixed at the root (WP22)**: `scripts/env.sh` now gives each checkout its own store (`E:/office-cache/sccache-<cksum of OFFICE_ROOT>`), so entries from a deleted worktree can never be served to another checkout, and the old shared `E:/office-cache/sccache` was deleted. Cost: a new checkout starts with a cold cache. Verified: `cargo xt register check` builds and runs `xtask` from a clean invocation (sccache reports the new location).
- `lint`: clippy `redundant_closure` in the new lockdiff code (6ef6158); `convert`: rustfmt in the host crates (068bbd8); the host test
  `without_webview2_...` raced a loaded machine with its 4 s exit hook, now 15 s (8f0f2c2).

## Final job table (HEAD a890da1 plus this docs commit; runs int5 and int6, `E:/office-target/p1/ci/int5` and `int6` `summary.md`)

| Job | Result | Wall | Notes |
|---|---|---|---|
| queue | ok | 880 s | (int2; includes a build-slot wait) |
| gen-check (gen-domain, lockdiff x4) | ok | 35 s | |
| deny (root + 4 domains) | ok | 35 s | services deny inside `convert` |
| build all (4 desktop, 4 web, 4 CLI; release, gnu) | ok | 4,219 s | docs web 844 s, slides web 713 s, pdf desktop 503 s; mostly cached |
| test-ours | ok | 1,903 s | root 38 s, domains docs 769 s, sheets 282 s, slides 468 s, pdf 341 s |
| test wordcraft | ok | 2,347 s | |
| test gridcraft | ok | 240 s (int6, after 0142) | int5: 1 failure, the one fixed by 0142 |
| test deckcraft | ok | 2,115 s | |
| test pdfcraft | ok | 3,247 s (int6) | int5: 2 failures, the stale build script above |
| check docs / sheets / slides / pdf | ok | 188 / 824 / 135 / 137 s | |
| bind | ok | 451 s | |
| convert (clippy, nextest 239/239, deny) | ok | 136 s | on a quiet machine; the earlier 3 timeouts were load |
| convert-e2e | ok | 369 s | |
| convert-ui | ok | 298 s | (int3b) |
| shell (suite-serve nextest, shell-dist, 19 Playwright, sizes) | ok | 493 s | |
| sizes | ok | 456 s | docs 9,852,663 / 9,871,648 B (headroom 18,985), sheets 4,941,891 / 4,966,771, slides 9,490,727 / 9,524,217, pdf 9,570,718 / 9,599,462, shell 48,443 / 65,536 |
| translit-npm / translit-preserve | ok | 21 s / 128 s | |
| web-smoke | ok | 14 s | |
| actionlint | ok | 1 s | |
| i18n | ok | 6 s | |
| brand (brand-grep + scan of the fresh binaries) | ok | 330 s | |
| lint (fmt, clippy -D warnings, layers, assets, upstream clippy ratchet) | ok | 349 s | upstream clippy: wordcraft 1, gridcraft 3, deckcraft 2, pdfcraft 1 warnings, all baselined |

Disk policy: E: never fell below 62 GB in the final runs, so the step "delete upstream test binaries older than 24 h" was not needed and nothing was deleted
(a watchdog loop ran during int5 and logged no action).

## Desktop checks with the rebuilt exes

- `scripts/smoke-desktop.ps1`: window title `Hoshiya`, start to window 3,582 ms, to server 3,040 ms, request without the key 403, editor started (`hoshiya-docs`),
  second launch handed to the running instance (`hoshiya-sheets`), no `hoshiya.exe` left behind. Host 39 MB, WebView2 291 MB (6 processes), tree with the editor 679 MB.
- `scripts/desktop-editors-check.mjs`: **16/16** for Docs, Sheets and Slides (Ctrl+P opens the editor's own print UI and no browser window, Ctrl+S writes exactly one file through the
  host's download handler, browser zoom stays put).
- **Shared toolbar setting across two apps** (`scripts/toolbar-share-check.py`, the real `%APPDATA%\Hoshiya\settings.toml`, which did not exist before and was deleted after;
  screenshots in `docs/phase4/evidence/toolbar-shared/`): Sheets starts simple; its "Full ribbon" button writes `toolbar = "full"` and the ribbon switches; Slides, started
  afterwards, opens with the full ribbon; its "Simple ribbon" button writes `toolbar = "simple"`; Sheets, started afterwards, opens simple again.
- **5 templates through `hoshiya.exe`** (`scripts/desktop-templates-check.mjs`, 8/8; screenshots in `docs/phase4/evidence/templates/desktop/`): each opens in its editor with no page
  error and no CSP violation. The editors run in the host's WebView2, as the shell does.

## Templates (web and desktop)

All five open in Docs, Sheets and Slides (web: `ci/web-smoke/templates.spec.ts` 5/5 on the fresh dists; desktop: above). Looked at by hand: Uzbek text with the oʻ, gʻ and apostrophe
forms renders with no tofu; frozen panes hold (tabel 3 columns and 4 rows, xodimlar 4 rows); COUNTIF totals compute (Sheets CLI on a copy with D5:H5 = I, I, K, T, S: AI..AM = 2, 16, 1, 1, 1,
Jami row the same; empty rows 0). One fix: the tabel legend was clipped at the frozen split, so it now starts in column D (a698257). Slides shows the editor's own "Arial isn't installed"
notice for taqdimot (any pptx that uses Arial does).

## WP12 (w80 GUI open time, idle machine)

3 runs, same protocol, same binary: open 62.9 / 60.2 / 60.2 s, steady private WS 903.9 / 903.8 / 904.8 MB (+0.1 %), peak 996.8 / 984.8 / 984.8 MB. At or slightly below round 2's slower cluster,
not on its faster one (39.8-40.4 s); that two-cluster spread was never explained. See `docs/phase4/excel-guard.md`.

## Hoshiya rename and logo

Commits 800ee09 (names, bins, ids, `hoshiya.exe`, `hoshiya-serve.exe`) and 126a782 (logo pack in `brand/logo/`, palette, icons, exe icon and version info, shell favicon, manifest and lockup).
Left as `suite-*` on purpose: crate and package names (`suite-core`, `suite-desktop`, `suite-serve`, `suite-winsec`, `suite-fake-editor`), the `SUITE_*` test-hook variables and the
`suite-host` event name. All are internal, none is brand.

## Contention (why the early times are long)

Until about 14:00 two `ci.sh quick` runs from other sessions held the `E:/office-target/p1` build slot for hours (the WP0 run's `test pdfcraft` took 14,663 s). At 14:34 E: filled up
(upstream test targets of 50-150 GB each); the coordinator freed 78 GB, and the machine was then idle for the w80 run and the final gate.

## WP22 final check (2026-10-24, HEAD after the "rustfmt: suite-serve test" commit; run `E:/office-target/p1/ci/final2/summary.md`)

`scripts/ci.sh quick convert convert-e2e shell sizes i18n deny` in one invocation (the multi-job quirk is fixed: every job named runs, also after a profile). Docs and Sheets web were rebuilt first (`build docs web`, `build sheets web`, both ok) because wordcraft/0124 and gridcraft/0144 touch their UI.

| Job | Result | Wall | Notes |
|---|---|---|---|
| queue | ok | 7 s | verify-queue 88 patches / 88 rows |
| lint | ok | 88 s | upstream clippy ratchet: only baselined warnings |
| gen-check | ok | 17 s | |
| deny | ok | 11 s | root + 4 domains; ctutils 0.4.4 |
| brand | ok | 305 s | |
| i18n | ok | 5 s | lint 0 errors, roundtrip OK, check-stale OK; shell 63 rows, 1 draft (`hist.beforerestore`, needs the opus review) |
| convert | ok | 57 s | the first run failed on rustfmt of the suite-serve test, fixed and rerun |
| convert-e2e | ok | 209 s | one `tls.spec.ts` "#token= flow" timeout in an earlier run (`#o-to` stayed disabled for 60 s) did not recur in this run; treat as a flake to watch, not explained |
| shell | ok | 539 s | 21 Playwright tests, including the new templates spec |
| sizes | ok | 457 s | |

`quick`'s own per-domain `check` and `test` jobs were not triggered (they follow the diff, and no domain source other than the two UI lines changed); `check docs` and `check sheets` were run by hand and are ok. Not run here: `desktop` and `msi` (new on-demand jobs; they need a desktop session and an hour of MSVC builds; syntax and actionlint only), and `uz-Latn-2026` in the apps (deferred, STATUS).
