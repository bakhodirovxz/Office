# Upstream: how to sync, carry and rebase

Upstream (wordcraft, gridcraft, deckcraft, pdfcraft, craft-libs; craft-fonts is fetched) is committed under `upstream/<repo>/` as a
**pristine snapshot plus our patch queue**. Never hand-edit `upstream/`. The source of truth for the snapshot is
`patches/<repo>/BASE` (sha, tree, date, describe, synced). There are no upstream PRs (gate decision B-6): every change is a **carried
patch** in `patches/<repo>/NNNN-*.patch`, with a row in `patches/REGISTER.md`. See PLAN section 1.4/1.5/5.

## Layout of history
For every BASE there are two commits per repo:
- **P** `vendor(<repo>): pristine <sha7>`: the subtree `upstream/<repo>` equals upstream's `<sha>^{tree}`.
- **Q** `patches(<repo>): apply queue (N patches) on <sha7>`: the subtree is the queue applied; `patches/<repo>/*.patch` and `BASE` change.

`git diff P_old P_new -- upstream/<repo>` is the pure upstream change; `git diff P Q` is exactly our delta.
`scripts/verify-queue.sh <repo|all>` (offline, seconds) proves `BASE tree + queue == HEAD:upstream/<repo>` and that nothing is hand-edited.

## Cache clones
`E:/office-upstream/<repo>` are full clones (history, spike branches, worktrees). All fetching, diffing and rebasing happens there.
Only tree objects enter the monorepo (`scripts/lib/import.sh`). Every sync leaves the rebased queue as the cache branch
`office/queue-<new7>`.

## The weekly sync
```bash
source scripts/env.sh
scripts/sync-upstream.sh --dry-run --gates quick all              # nothing in the monorepo changes
scripts/sync-upstream.sh --dry-run --survey all                   # list EVERY conflicting patch (file:line), do not stop at the first
scripts/sync-upstream.sh --dry-run --resolve --resolver sonnet gridcraft   # conflicts resolved in scratch, wall time recorded
scripts/sync-upstream.sh --apply --gates quick wordcraft deckcraft pdfcraft craft-libs   # partial apply by naming repos
```
The `quick` gate's native check also runs `cargo check --workspace --all-targets` on `upstream/<repo>`, so a new upstream test that no longer compiles against
our patches fails the sync (it was missed once: gridcraft `malformed.rs` reading `Formula` fields, patch 0103).

Options: `--to <sha>` (one repo), `--gates quick|full`, `--fmt` (regenerate the trailing rustfmt patch, below), `--resolver <name>` (recorded in the log). Exit codes: 0 ok, 1 conflict or gate
failure, 2 preflight or infrastructure.

What the script does (PLAN 5.1):
0. Preflight: `upstream/ patches/ domains/ scripts/` must be clean (other paths may be dirty; the script never touches them).
1. Fetch; NEW = `--to`, else the newest tag containing BASE, else `origin/HEAD`. Commit count and churn on files the queue touches.
2. Signals: `rust-version`, `[patch]`/`[profile]`/`[workspace.lints]`, wasm-bindgen and egui lock versions, new manifests. Read this
   section of the report first: a rust-version above our toolchain, or a new wasm-bindgen, needs action before the apply.
3. Rebase: cache worktree at NEW, `git am -3` for each patch (timed): `clean`, `3way` (auto-merged) or `conflict`. Without `--resolve` the
   run stops at the first conflict (a correct failure with a report); `--survey` carries on and lists all; `--resolve` waits (see below).
   The queue is regenerated with `git format-patch --zero-commit --binary --no-signature`; `git range-diff` gives the interdiff lines per patch.
4. Scratch monorepo worktree `$OFFICE_TARGET/sync/mono-<date>-<id>` on branch `sync/<date>`: commits P' and Q' per repo, `cargo xt gen-domain`,
   lock re-seeding (upstream lock copied into `domains/<d>/Cargo.lock`, `cargo metadata` adds our path packages), REGISTER last-rebase cells,
   `patches/rebase-log.tsv` rows.
   Patch files keep their registered names (patch k of the new queue takes the file name of patch k of the old queue; `format-patch` alone would renumber from 0001).
   **The trailing rustfmt patch.** A queue may end with `NNNN-<repo>-rustfmt.patch` (gridcraft: `0111`): `rustfmt --edition <ed>` over every `.rs` file
   the queue touches, as ONE patch (REGISTER: "mechanical formatting, regenerate on every sync"), so `scripts/ci.sh lint` (`cargo fmt --all --check`) is clean without
   formatting each patch. It is never rebased: the script skips it in the rebase and regenerates it on top of the rebased queue when `--fmt` is given or when the queue already
   has one. If nothing needs formatting any more the patch is dropped (the report says so; delete its REGISTER row). A new repo gets one with `--fmt`
   (number = last patch + 1, then add its REGISTER row). Never format the other patches by hand: they stay pure.
5. Gates (separate target dirs `$OFFICE_TARGET/sync/target-*`, build lock, RAM gate 3 GB, no foreign rustc.exe): verify-queue, register check,
   **i18n** (below), gen-check, lockdiff (must read `0 version changes ... 0 foreign additions`), cargo-deny, `cargo check` native (desktop + cli) and wasm32 (web, sccache off).
   `full` is a stub: run `scripts/build.sh <d> all --release` and the app tests after an apply.
6. `--dry-run` deletes the scratch worktree and branch. `--apply` (all gates PASS, no unresolved patch) commits the report to
   `docs/phase1/sync-reports/` and fast-forwards the monorepo. A failed gate leaves the pin where it is.
Reports: `$OFFICE_TARGET/sync/reports/<date>.md` and `<date>-rebase-log.tsv`.

## The i18n gate (catalog resync)
Our canonical catalogs (`i18n/*.tsv`, keyed by English) are extracted from the upstream catalogs and source in `upstream/`, so every sync can make them stale.
- `cargo xt i18n check-stale` re-extracts in memory and compares: keys added, keys removed (now `obsolete`), English changed, upstream `ru` changed. It exits 1 with a list
  of the keys and the fix when any exists; `where` (file:line) drift alone is not stale. `scripts/ci.sh i18n` runs it with `lint` and `roundtrip`.
- `scripts/sync-upstream.sh` runs it as gate `i18n-stale` after the import, then `i18n-roundtrip` and `i18n-lint`. A **dry run fails** the gate (with the key list in the
  report) when an upstream catalog changed; **`--apply` runs the merge** (`cargo xt i18n extract`), puts the key list in the report, re-checks, and commits `i18n/` with the sync.
- The merge keeps every uz and ru value and status and each row's placement. New keys arrive as `todo` (ru `upstream` where upstream has one). Keys that disappeared are
  marked `obsolete`, never deleted. Upstream `ru` replaces ours only where our `ru` is still `upstream`/`todo`. A key whose English changed is a new id (the id hashes
  the English): the old row's Uzbek is carried to the closest new row of the same scope and context (within 20% edit distance) as `draft:carried:<date>`, with a note
  `carried from <old id>; English changed: "<old>" -> "<new>"`. An old `reviewed:owner` status never survives: the row shows up as `draft-unreviewed` in the next review batch,
  and release exports skip it. `where` line numbers are refreshed only with `--refresh-where` (or when the file changed), so a sync does not rewrite every row.
- By hand: `cargo xt i18n extract --dry-run` (report only), `cargo xt i18n extract [--date YYYY-MM-DD]`, then `lint`, `roundtrip`, and commit `i18n/*.tsv`.

## Resolving a conflict (`--resolve`)
1. Run the script in the background. On a conflict it prints `RESOLVE NEEDED: worktree <path> patch <file> files: ...` and polls.
2. In that worktree (a cache worktree, mid `git am`): edit the files so that the patch's intent is kept on top of the new upstream, remove all
   markers, `cargo fmt` the touched files if the repo is rustfmt-clean at that spot, then `git -C <path> add <files>`.
3. The script runs `git am --continue` itself and records the wall time and the conflicted files.
4. Do not run `git am --continue` or `--abort` yourself while the script waits.
To work by hand instead: `git -C E:/office-upstream/<repo> worktree add --detach <dir> <new>` then `git am -3 patches/<repo>/*.patch`,
`git am --continue` after each fix, `git format-patch --zero-commit --binary --no-signature`.

## Carrying a patch
1. Work in a cache worktree on the current queue (`office/queue-<new7>` branch or `git am` the queue onto BASE).
2. Commit as a normal upstream-style commit (subject `area: what`, body says why). Keep touched files **rustfmt-clean** (CI lint).
3. `git format-patch --zero-commit --binary --no-signature -o patches/<repo>` for the new commit with the next free number
   (0001-0099 functional, 0100-0199 product overlay, 0900-0999 temporary CI fixes).
4. Add a row to `patches/REGISTER.md` (category, why, measured effect, "not sent", carried since, drop-when), run `cargo xt register check`.
5. Rebuild the Q commit: import the queue tree (`import_pristine`) and run `scripts/verify-queue.sh <repo>`.

## Measuring rebase cost (PLAN 5.4)
`patches/rebase-log.tsv` is append-only: one row per patch per sync (result, conflicted files, hunks, minutes, interdiff lines, resolver).
`docs/phase1/rebase-cost.md` derives minutes per 100 upstream commits, the clean share and the hot files. `minutes` is agent wall time, not
human time; the interdiff line count is the size of the resolution.

## Parking a patch
If a patch costs too much to rebase, move it to `patches/<repo>/parked/`, set the register category/status `parked: rebase cost X h`, and
report it as an owner decision (the B-6 revisit). `cargo xt register check` accepts files in `parked/`.
