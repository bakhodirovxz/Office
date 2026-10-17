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
