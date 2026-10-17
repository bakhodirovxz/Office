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
