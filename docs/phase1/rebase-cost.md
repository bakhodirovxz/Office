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

## Derived numbers and caveats
- Share of clean patches: 27/27 (100 %). Hot files (conflicting in two consecutive syncs): none yet (first sync).
- Cost per sync at this distance is seconds of machine time and no resolver time; the real cost is the gate time (cold check of four domains about 51 min, warm 8 min).
- Projection of weekly cost = rate x weekly commits (701 to 993) is **not supportable** from this data: the rate is 0 conflicts per 48 commits, and the queue-touched files changed in only
  4 of 63 files (gridcraft) and 0 of 9 (wordcraft). The WP14 sync at about 7 days is the first real data point. Two points are not a trend.
- Agent minutes are not human minutes; with no conflict there is no resolver time to report.
