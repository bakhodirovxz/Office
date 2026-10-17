# WP0-WP2 execution notes

Executed 2026-10-17 on branch `phase-1`. Evidence (commands and actual output) per WP; deviations at the end.

## WP0. Bootstrap, toolchain components and CI tools

Changes outside the repo (shared toolchain and tools):
- `rustup component add rustfmt clippy` (rustfmt 1.9.0-stable, clippy 0.1.97). `wasm32-unknown-unknown` was already installed.
- Prebuilt Windows (msvc build, standalone exe) binaries in `E:/office-tools/bin`: cargo-deny 0.20.2, cargo-nextest 0.9.148,
  sccache 0.18.0, actionlint 1.7.12. Versions, URLs and archive SHA-256 are in `scripts/tools.lock`. No `cargo install` was needed.
- trunk 0.21.14 was already in `E:/office-tools/cargo/bin`.

Done-check:
```
$ bash -c '. scripts/env.sh; cargo fmt --version; cargo clippy --version; cargo deny --version; cargo nextest --version; sccache --version; actionlint --version; trunk --version'; echo rc=$?
rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14)
clippy 0.1.97 (8bab26f4f6 2026-07-14)
cargo-deny 0.20.2
cargo-nextest 0.9.148 (cd1d6d546 2026-10-07)
release: 0.9.148
commit-hash: cd1d6d5467322dbc0d39a80cb671d000ac302798
commit-date: 2026-10-07
host: x86_64-pc-windows-msvc
sccache 0.18.0
1.7.12
installed by downloading from release page
built with go1.26.1 compiler for windows/amd64
trunk 0.21.14
rc=0
```
(`cargo nextest --version` and `actionlint --version` print several lines each, so there are more than seven lines; each of the seven tools reports once, exit 0, trunk is `trunk 0.21.14`.)

sccache note: a trivial `cargo build` of a hello-world bin with `RUSTC_WRAPPER=sccache` ran fine (server starts, build succeeds) but
sccache reported "Compile requests 6, executed 0", so cache effectiveness on windows-gnu is not yet proven. It must be
measured in WP3 on a real crate graph; if it does not cache, drop `RUSTC_WRAPPER` (PLAN WP0 risk) and record it.

## WP1. Cache clones and pristine snapshots at the old BASE

What was done:
- Before: every upstream clone had `git status --porcelain` empty; branch lists saved (`E:/office-cache/wp1/<repo>.branches.before`).
- Each clone (including craft-fonts) was copied with robocopy to `E:/office-upstream/<repo>` and the originals were moved to a
  temporary `.wp1-backup/` (git-excluded), not deleted. Checks before removing the backup: `git fsck --connectivity-only` (only
  pre-existing dangling commits reported), `git branch -a` byte-identical before and after for all six, file counts equal,
  `git worktree repair` for the four spike worktrees. Then the backup was removed.
- `core.autocrlf=false` set in every cache clone and in the monorepo.
- Spike worktrees still work: `git -C /e/office-work/gridcraft-spike rev-parse HEAD` -> 3884ffac384426016ec2a492bb1f7d2b06ab1c52,
  gridcraft-pdf 43b73b0f..., wordcraft-spike c0d3cbd6..., wordcraft-b fd0bb2fb... (all as before).
- `git add --renormalize .` produced no content change for existing docs (so no separate normalise commit was needed).
- Import used the plan's plumbing (`scripts/lib/import.sh`: `import_pristine`, `write_base`). Commit P = `9b23f4d`.
  Working tree is LF and byte-exact (`git ls-files --eol`: `i/lf w/lf` and `i/-text w/-text`, no CRLF).

Done-check (actual output):
```
wordcraft OK  tree=757fe786008f9657e0ff031d8623d747525c05c1
gridcraft OK  tree=53ce3a619d86dd8d0d205dc7b75fc27c481c099c
deckcraft OK  tree=3ebaa12b5d01c60df7b9221b02830d59a99e5b10
pdfcraft OK  tree=3ff3128b12409f6a9154b34a74a7b6289f2572eb
craft-libs OK  tree=a11d36f6ca38b0f617354f6b03f8d67b2f744c8e
$ git status --porcelain upstream | wc -l
0
$ scripts/fetch-fonts.sh --verify
craft-fonts 3d9ab3a: 31 files OK
$ git -C /e/office-work/gridcraft-spike rev-parse HEAD
3884ffac384426016ec2a492bb1f7d2b06ab1c52
```

## WP2. Spike queues and the register at the old BASE

What was done:
- `patches/wordcraft/0001-0005`: copied from `spikes/wordcraft-patches/` unchanged.
- gridcraft: one ordered queue in a cache worktree (`E:/office-target/p1/wt/gridcraft-q`, detached at `e6b6b34`):
  `git am` of `spikes/gridcraft-patches/0001-0019` (resulting tree `533cba33...` equals the Phase 0 `spike/memory-r2` tree, so the
  round-1 + round-2 stacking is reproduced exactly), then `git am -3` of `spikes/gridcraft-pdf-patches/0001-0003`.
  The overlap in `crates/engine/src/lib.rs` (the pdf-fonts series adds `pub mod pdf_fonts;` next to the `io`/`links`/`outline`/`paper` module lines)
  applied **without any conflict**: no manual resolution, so it is not a rebase-cost data point. The ordered queue is a single
  series, not two queues. Regenerated with `git format-patch --zero-commit --binary --no-signature` as `0001-0022`; 0001-0019 differ
  from the spike files only in the `From` hash line and the `[PATCH n/m]` subject counter, pdf-fonts became 0020-0022.
- Commits: P = `9b23f4d` (vendor, WP1); Q wordcraft = `46d75d5`; Q gridcraft = `af2700c` (queue tree imported with the same plumbing as P).
- `scripts/verify-queue.sh` implements PLAN 5.2 rules (a), (b), (c). Negative tests run: a hand-edited upstream file gives
  `FAIL deckcraft: hand-edited`, a removed patch gives `FAIL wordcraft: tree != BASE+queue`.
- `patches/REGISTER.md`: 27 rows, generated once from the patch headers and the Phase 0 reports; `cargo xt register check`
  (std-only xtask) fails on a file without a row, a row without a file, an empty cell, an unknown category (negative tests run).
- Root `Cargo.toml` (workspace `core`: member `xtask`, excludes upstream/domains/spikes/services), `.cargo/config.toml`, `Cargo.lock`.

Done-check (actual output):
```
$ scripts/verify-queue.sh all; echo rc=$?
wordcraft: 5 patches, tree OK
gridcraft: 22 patches, tree OK
deckcraft: 0 patches, tree OK
pdfcraft: 0 patches, tree OK
craft-libs: 0 patches, tree OK
rc=0
$ cargo xt register check
27 patches, 27 rows, OK
$ git diff --stat 9b23f4d af2700c -- upstream/gridcraft | tail -1
 63 files changed, 4474 insertions(+), 330 deletions(-)
$ git diff --stat 9b23f4d 46d75d5 -- upstream/wordcraft | tail -1
 9 files changed, 651 insertions(+), 18 deletions(-)
```

## Repo size after vendoring

`git gc` then `git count-objects -vH`: pack 53.26 MiB, `.git` 54 MB (23 MB before WP1; the plan projected about 60 MB).
`upstream/` working tree: 131 MB (craft-fonts, fetched and ignored, is 41.8 MB of that).

## Deviations

1. `.gitattributes`: the plan's line `*.ttf *.otf *.png ... binary` is not valid git syntax (git rejects `*.otf is not a valid attribute name`).
   Replaced by one `binary` line per extension.
2. P commit message. WP1 asks for one commit holding all five P subtrees, but PLAN 1.5 rule (a) looks for `vendor(<repo>): pristine <sha7>`.
   The single commit is titled `vendor: pristine snapshots at Phase 0 BASEs (wordcraft fd0bb2f, gridcraft e6b6b34, ...)` and
   `verify-queue.sh` accepts both forms (regex on `<repo> <sha7>` in the message).
3. Cache clones were copied with robocopy (not `/MOVE`) and the originals renamed to a git-excluded `.wp1-backup/` until fsck, branch lists and file counts
   matched; then the backup was removed. craft-fonts was moved to the cache too and is re-extracted from it by `fetch-fonts.sh`.
4. The tools are standalone prebuilt MSVC-built executables (the Rust toolchain is windows-gnu); no `cargo install` was needed. nextest is pinned to its
   GitHub release URL (`cargo-nextest-0.9.148`); the `get.nexte.st/latest` download had the identical SHA-256.
5. `.cargo/config.toml` also sets `[build] target-dir = "E:/office-target/p1/core"` so that `cargo xt` never writes to C:. Domain and upstream builds
   override it with `CARGO_TARGET_DIR` (an environment variable beats config); `scripts/build.sh` in WP3 must set it per workspace.
6. gridcraft diffstat: PLAN WP2 expects "about 64 files and +3,170 lines". Measured P to Q is 63 files, +4,474 / -330 (the sum of both series'
   additions; the plan's +3,170 looks like a net figure). Not a defect.
7. REGISTER.md "last rebase" cell reads `initial import 2026-10-17 @ <sha7>: no rebase yet` (the cell must be non-empty); WP5 replaces it.
   The register category for 0020-0022 is `pdf` (the `font` category is for the product font overlay).
8. `rustfmt.toml`, `deny.toml`, `LICENSE-*`, `NOTICE` (listed in the layout tree) were not created; they belong to later WPs (WP6a and the licence task), not to WP0-WP2.

## Open points for WP3

- sccache with the windows-gnu toolchain is unproven: stats after two small builds show `Non-cacheable calls`, no hits
  (only a trivial bin crate was compiled). Measure on the first real domain build; if no hits, unset `RUSTC_WRAPPER` in `scripts/env.sh`.
- Wrapper scripts have LF endings (`.gitattributes` `eol=lf`), but the working copy of older files such as `docs/STATUS.md` is still CRLF
  from before `core.autocrlf` was turned off. Git sees no difference; edit tools should preserve either ending.
- The cache worktrees `E:/office-target/p1/wt/{wordcraft-q,gridcraft-q}` (queue applied, detached) are kept for WP5's `range-diff` of the old queue.
