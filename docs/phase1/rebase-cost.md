# Rebase cost (PLAN 5.4), WP5 part A

Measured 2026-10-17 with `scripts/sync-upstream.sh` (see `docs/UPSTREAM.md`). Log: `patches/rebase-log.tsv` (applied repos);
full reports: `docs/phase1/sync-reports/2026-10-17.md`, and in `E:/office-target/p1/sync/reports/` (`*-dryrun1-cold.md`, `*-dryrun2-warm.md`).

## Fresh BASE (frozen for Phase 1)
| repo | old BASE | new BASE | upstream date | commits crossed |
|---|---|---|---|---|
| wordcraft | fd0bb2f | **eae2421** (eae242176108d328c5dd81fbc10370c51ab90706) | 2026-10-15 | 8 |
| gridcraft | e6b6b34 | **f1894c6** (f1894c6da64d4326022ae644c767581816a23c2c) | 2026-10-15 | 6 |
| deckcraft | 9859da3 | **1dc6972** (1dc69720db64323370dda061226becb09ad79fd4) | 2026-10-15 | 15 |
| pdfcraft | afa8e18 | **924c4da** (924c4da4ac2879ac9107c6c82e20ac6be5570f0a) | 2026-10-16 | 19 |
| craft-libs | 8f5a283 | **15ed713** (15ed713b8fc622302a7f2920f710cec7d261fa9a) | 2026-10-16 | 9 |
| craft-fonts | 3d9ab3a | unchanged | 2026-10-11 | 0 |

The old BASEs were taken only one to two days before the new HEADs, so the distance is 6 to 19 commits per repo, not the 4 to 7 days
(701 to 993 commits a week) that PLAN F6/D4 assume. Treat this as a **low-distance data point**, not a weekly one.

## Dry run (all five repos, `--dry-run --survey --gates quick`)
- 27 of 27 patches **clean** (plain apply or offset only; 0 three-way, 0 conflicts). Gates quick PASS for all four domains:
  verify-queue, register, gen-check, lockdiff (0 version changes, 0 foreign additions in each), cargo-deny, `cargo check` native + wasm32.
- Monorepo untouched: guard on `upstream/ patches/ domains/ scripts/` unchanged, scratch worktree and branch removed, exit 0.
  (A first run printed `BUG: the dry run changed the monorepo` only because another agent committed to HEAD meanwhile; the guard now compares path content, not HEAD.)
- Wall time: 3,065 s cold (cargo check of four domains: docs 642, sheets 523, slides 690, pdf 1,065 s), 510 s warm.
- Lock/domain regeneration: wordcraft changes `domains/docs/Cargo.lock` (-76 +x lines: the lock moved with upstream) and the domain manifests; slides, pdf and sheets change only
  the generated manifests (BASE sha7 comment). No version change in any lock versus upstream.
- Signals: wordcraft `rust-version` 1.90 -> 1.95 (our rustc is 1.97.1, fine), wordcraft `[patch]/[profile]/[workspace.lints]` changed (domain generator copied it; gen-check passes),
  craft-libs gained `crates/ai-core`. No wasm-bindgen or egui bump in any repo.

## wordcraft (applied)
Upstream: 8 commits, 10 files, +241/-58. Churn on the 9 files touched by the queue: none.
| patch | result | seconds | conflicted files | hunks | interdiff lines | retest |
|---|---|---|---|---|---|---|
| 0001 bridge | clean | 0.44 | 0 | 0 | 0 | check pass |
| 0002 GPU texture | clean | 0.29 | 0 | 0 | 0 | check pass |
| 0003 IME | clean | 1.06 | 0 | 0 | 0 | check pass |
| 0004 touch scroll | clean | 0.37 | 0 | 0 | 0 | check pass |
| 0005 hand over | clean | 0.31 | 0 | 0 | 0 | check pass |
(seconds from the cold dry run; the applied run measured 3.7 s for the whole repo.)

- Total rebase wall time: **about 3 s (0.05 min)**, conflicts **0**, resolution lines **0**. Minutes per 100 upstream commits: 0.6 (3.7 s / 8 commits x 100 = 46 s).
- Gates after rebase: verify-queue, register check, gen-check, lockdiff, deny, `cargo check` native (desktop + cli) and wasm32 web: all PASS (92 s warm). Release builds, sizes and
  app tests were **not** run (`--gates full` is not automated in scratch yet; WP6b).
- **rustfmt**: the unrebased queue was not fmt-clean at the new BASE (edition 2024, repo `rustfmt.toml`): `host.rs`, `canvas.rs`, `lib.rs` and the new `touch_scroll.rs` region. The script's `--fmt`
  pass (`git rebase -x rustfmt`) reformatted patches 0004 (+2 lines) and 0005 (+2 lines); all seven touched `.rs` files are `rustfmt --check` clean afterwards. This is a content change,
  not a conflict resolution, and is not counted as resolution lines.

## deckcraft, pdfcraft, craft-libs (applied)
No patches, so only the snapshot moved (commits P'). 15, 19 and 9 upstream commits. deckcraft and pdfcraft domains regenerated and checked (slides 72 s, pdf 92 s warm); craft-libs has no domain
(the apps fetch it by git rev; see WP4).

## gridcraft: **dry run only, not rebased** (queue reserved for the next agent)
6 upstream commits. Churn on the 63 queue-touched files: 4 files, +78/-10 (`crates/engine/src/cmd/edit.rs`, `crates/formula/src/tests.rs`, `crates/ui-egui/src/grid.rs`, `docs/file-format-parity.md`).
Dry run: **all 22 patches apply clean, zero conflicts** (0001-0019 and the pdf-fonts 0020-0022), 15 s total; gates quick PASS for sheets (check 523 s cold). So the plan's expected gridcraft
hot spot (0002, 27 files; `recalc.rs`) did not conflict at this BASE. Real rebase, apply and measurement are **pending**:
`scripts/sync-upstream.sh --apply --fmt --gates quick gridcraft` (cache branch `office/queue-f1894c6` already holds the rebased queue; fmt-cleanliness of the 22 patches is not checked yet).
(Update, gate review 2026-10-19: done later the same day. gridcraft was applied at f1894c6, 22 of 22 clean, see `docs/phase1/sync-reports/2026-10-17.md` and `patches/gridcraft/BASE`;
fmt-cleanliness was handled by the trailing patch 0111.)

## Derived numbers and caveats
- Share of clean patches: 27/27 (100 %). Hot files (conflicting in two consecutive syncs): none yet (first sync).
- Cost per sync at this distance is seconds of machine time and no resolver time; the real cost is the gate time (cold check of four domains about 51 min, warm 8 min).
- Projection of weekly cost = rate x weekly commits (701 to 993) is **not supportable** from this data: the rate is 0 conflicts per 48 commits, and the queue-touched files changed in only
  4 of 63 files (gridcraft) and 0 of 9 (wordcraft). The WP14 sync at about 7 days is the first real data point. Two points are not a trend.
- Agent minutes are not human minutes; with no conflict there is no resolver time to report.

# Second dry run, 2026-10-19 (WP14 step 2; nothing applied, BASE still 2026-10-17)

Owner decision 2026-10-18 (option A): run now against the current upstream HEAD, no 7-day wait. Full reports: `docs/phase1/sync-reports/2026-10-19.md`.
`--gates full` was still a stub in the script at the time (implemented in 98c0a02 afterwards, never executed yet), so `quick` was used (verify-queue, register, i18n, gen-check, lockdiff, deny, `cargo check` native incl. `--workspace --all-targets`
of the upstream tree, and wasm32 web). Release builds, sizes and tests are covered by the clean-checkout run (`docs/phase1/clean-checkout.md`), not by this dry run.

Upstream **did move, but little**: 67 commits in about 2 days (newest 2026-10-18). Again a low-distance point, not a weekly one.

| repo | BASE (2026-10-17) | NEW | commits | queue patches | churn on queue-touched files | clean | 3-way | conflict | rebase wall |
|---|---|---|---|---|---|---|---|---|---|
| wordcraft | eae2421 | f2e15cf | 3 | 10 | 1 file, +2/-2 | 10 | 0 | 0 | 5.2 s |
| gridcraft | f1894c6 | 823c565 | 3 | 29 | 1 file, +16/-16 | 27 (+ 0111 regenerated) | 0 | 1 (0020, `Cargo.lock`), resolved | 12.1 s survey, 102.3 s resolved |
| deckcraft | 1dc6972 | 22db926 | 10 | 8 | 1 file, +4/-1 | 8 | 0 | 0 | 4.7 s |
| pdfcraft | 924c4da | 645016a | 29 | 4 | 3 files, +7/-4 | 4 | 0 | 0 | 4.2 s |
| craft-libs | 15ed713 | ae2f796 | 22 | 0 | - | - | - | - | - |
| total | | | 67 | 51 | | 49 (+1 regenerated) | 0 | 1 | |

- **Patch outcomes: 49 of 51 clean (96 %), 0 three-way, 1 conflicting (resolved), 1 regenerated (gridcraft 0111 rustfmt, never rebased by design).**
  (Corrected by the gate review: the earlier text said "50 of 51 clean"; the report totals are `clean 49 ... conflict 1` in run 1 and `clean 27, resolved 1` plus `0111 regenerated` in run 2.) The conflict is a generated file, not source: gridcraft 0020 (pdf font embedding) adds a dependency line to
  `Cargo.lock` under `gridcraft-pdf`, and upstream bumped that crate `0.7.0 -> 0.8.0` in the same lock stanza. Resolution: keep `0.8.0` plus the `epaint_default_fonts` dependency.
  1 hunk, 4 interdiff lines, **92 s** agent wall time (about 1.5 min). This is a recurring-conflict candidate: every upstream version bump of a crate whose lock stanza a patch edits
  collides here. Cheaper fix than resolving each time: have the patch not edit `Cargo.lock` (the domain lock is re-seeded by the script anyway), tracked as a follow-up for Phase 2.
- Hot files (conflicting in two consecutive syncs): none yet; `Cargo.lock` (gridcraft) is the first candidate, to be confirmed by the next sync.
- The brand, i18n, font and overlay patches (wordcraft 0100-0110, gridcraft 0100-0110, deckcraft 0100-0112, pdfcraft 0100-0110) rebased clean (the only conflict is the gridcraft lock file above).
  The R10 risk (brand patches in hot files) is not contradicted, but the sample is 2 days and the queue-touched files changed in only 6 of 142 files.
- **Gates (quick): all PASS after the resolution except the i18n gates** (run 2 for gridcraft; run 1 for the others; run 1 ended `DRY RUN INCOMPLETE` with `i18n-stale` and
  `i18n-roundtrip` FAIL, see the next bullet; no single invocation printed `DRY RUN OK` for all five repos): verify-queue, register (51 rows), gen-check x4, lockdiff x4 (0 version changes, 0 foreign
  additions), deny x4, `cargo check` native + wasm32: docs 172 s, slides 126 s, pdf 244 s, sheets 106 s.
- **i18n gate in a dry run fails by design** when an upstream catalog changed: pdfcraft added 1 key (`the name is not a plain file name`) and wordcraft changed 1 upstream `ru` value
  ("Accessibility"). `--apply` runs the catalog merge. In run 1 `i18n-roundtrip` also failed while the stale catalogs were not merged (gridcraft was still unrebased in that scratch tree);
  in the gridcraft-only run 2 the stale, roundtrip and lint gates passed.
  **i18n-roundtrip finding (reproduced 2026-10-19, not a tool bug).** Rebuilt run 1's scratch tree (HEAD plus the rebased wordcraft f2e15cf, pdfcraft 645016a and deckcraft 22db926 queues, gridcraft still old)
  and ran the gates by hand: `check-stale` FAIL (same 1 added key, 1 ru change), `roundtrip` FAIL with exactly `wordcraft: semantic equal: false` (2571 vs 2571 entries, the changed `ru` value) and
  `pdfcraft: semantic equal: false` (upstream ru 2317 entries, exported 2316: the new key is missing); gridcraft passed, and the Uzbek-empty checks passed for all three. `roundtrip` compares the committed
  catalog (`i18n/*.tsv`) exported as `ru` with upstream's own `ru` file, so it fails whenever the catalog is stale; it is the same condition as `i18n-stale`, reported a second time. After `cargo xt i18n extract`
  (what `--apply` runs; it changed `i18n/pdfcraft.tsv` +2/-1 and `i18n/wordcraft.tsv` +1/-1) on that tree: `check-stale` PASS, `roundtrip` PASS ("round trip OK"), `lint` PASS (1879 rows, 0 errors).
  So `--apply` merges the catalogs and then passes both gates; nothing to fix in `xtask`.
- Wall time: run 1 763 s (12 m 47 s) for five repos, run 2 271 s. Rebase machine time was 26 s for run 1 (23 s per 100 upstream commits); the cost is the gates (2 to 4 min of cargo check per domain, sccache warm).
- **Rebase cost per PLAN 5.4** (this sync): resolver time 1.5 min for 67 commits, i.e. about **2.3 min per 100 upstream commits**; 49 of 51 patches clean (1 resolved, 1 regenerated); 4 resolution lines.
  Together with the first sync (0 conflicts per 57 commits) that is 2 points over 124 commits and 1 conflict: **not a trend**, still no basis for a weekly projection (701 to 993 commits a week
  in PLAN F6/D4). The weekly measurement is Phase 2 work (owner decision above).
- Not applied: BASE for the gate stays 2026-10-17. The cache branches `office/queue-<new7>` hold the rebased queues; the scratch worktrees were removed by the script.
