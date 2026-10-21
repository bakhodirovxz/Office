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

## The converter (`services/`)

A document converter on the four domain CLIs: docx, doc, odt, rtf, html, md, txt, xlsx, xls, xlsb, ods, csv, pptx, ppt and pdf to the
formats `convert formats` lists, plus Uzbek Latin/Cyrillic transliteration (`--translit`) and pdf combine/split/extract. It links no
engine: it runs the release CLIs as processes. Build: `scripts/build.sh <docs|sheets|slides|pdf> cli --release` for each domain, then
`cargo build --release --manifest-path services/Cargo.toml` (binaries `convert` and `convert-api`). Point it at the CLIs with
`CONVERT_DOCS_CLI`, `CONVERT_SHEETS_CLI`, `CONVERT_SLIDES_CLI`, `CONVERT_PDF_CLI` or `CONVERT_TOOLS_DIR`. Full reference: `docs/phase3/api.md`.

```bash
# CLI: one file or a batch; exit code and --json report say what happened (0 ok, 3 unsupported, 10 warnings under --strict)
convert report.docx --to pdf
convert letter.docx --to docx --translit cyrl            # Latin to Cyrillic, formatting kept
convert book.xlsx --to csv --sheet all --out-dir out/
convert scan.pdf --to png --pages 1-2 --dpi 150
convert pdf combine a.pdf b.pdf -o all.pdf
convert formats --json                                    # every supported direction;  convert check: which tools are found

# Local server: loopback only, no login, plain HTTP; the page is served at the same address
convert-api                                               # http://127.0.0.1:8787/

# Server mode: HTTPS only, bearer tokens (HS256), files encrypted at rest; refuses to start without its keys
convert-api --config convert-api.toml                     # auth = "jwt", tls_cert, tls_key, secret_file, key_file
```

Optional sidecars (Docker): `sidecars/pdf2docx` (layout-preserving pdf to docx, `--pdf-engine sidecar`, `CONVERT_PDF2DOCX_URL`) and
`sidecars/soffice` (legacy formats). Tests: `scripts/ci.sh convert`, `convert-e2e`, `sidecars` (see `docs/phase3/ci.md`).
