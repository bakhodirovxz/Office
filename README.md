# Office suite on the Craft engines

Working notes for agents: `CLAUDE.md`. Plan and status: `docs/phase1/PLAN.md`, `docs/STATUS.md`. Upstream handling: `docs/UPSTREAM.md`.

## Clean checkout (Windows 10, Git Bash)
Prerequisites on the machine (`scripts/bootstrap.sh` only checks them, it installs nothing): rustup with the `stable-x86_64-pc-windows-gnu` toolchain (MSYS2 ucrt64 gcc),
the `wasm32-unknown-unknown` target, rustfmt and clippy; `cargo-deny`, `cargo-nextest`, `sccache`, `actionlint`, `trunk`, `brotli` on PATH (exact versions in
`scripts/tools.lock`; `~/.cargo/bin` and `E:/office-tools/bin` are added by `scripts/env.sh`); Node with Playwright for `web-smoke`; network access for the `craft-fonts`
fetch (or a cache clone in `E:/office-upstream/craft-fonts`).

```bash
git clone <repo> <dir> && cd <dir>                      # branch phase-1
export OFFICE_TARGET=E:/office-target/p1/clean/target   # optional: own target dir (default E:/office-target/p1); set it BEFORE sourcing env.sh
scripts/bootstrap.sh                                    # checks tools, creates E: dirs, fetches craft-fonts (upstream/craft-fonts is not in git), starts sccache
scripts/ci.sh full                                      # the whole gate, hours when cold. One job: scripts/ci.sh <job> [args] (see the header of scripts/ci.sh)
```
Notes:
- Run `scripts/ci.sh` with exclusive use of the machine: one rustc-heavy job at a time, a 3 GB free-RAM gate, and a build lock at `$OFFICE_TARGET/BUILD.lock`
  (remove a stale lock after a killed run).
- `bootstrap.sh` prints "sccache server did not start" when a server is already running; that is harmless.
- Results of a run: `$OFFICE_TARGET/ci/<run>/summary.md` (`--run <id>` names the run).
- The sccache cache is `E:/office-cache/sccache`, shared by all checkouts. It does not hit across checkouts at different paths (0 hits in the clean-checkout run).
