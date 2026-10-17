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
Options: `--to <sha>` (one repo), `--gates quick|full`, `--resolver <name>` (recorded in the log). Exit codes: 0 ok, 1 conflict or gate
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
5. Gates (separate target dirs `$OFFICE_TARGET/sync/target-*`, build lock, RAM gate 3 GB, no foreign rustc.exe): verify-queue, register check,
   gen-check, lockdiff (must read `0 version changes ... 0 foreign additions`), cargo-deny, `cargo check` native (desktop + cli) and wasm32 (web, sccache off).
   `full` is a stub: run `scripts/build.sh <d> all --release` and the app tests after an apply.
6. `--dry-run` deletes the scratch worktree and branch. `--apply` (all gates PASS, no unresolved patch) commits the report to
   `docs/phase1/sync-reports/` and fast-forwards the monorepo. A failed gate leaves the pin where it is.
Reports: `$OFFICE_TARGET/sync/reports/<date>.md` and `<date>-rebase-log.tsv`.

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
