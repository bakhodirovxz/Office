# Domain build proof (WP3, the W2 go/no-go)

Executed 2026-10-17 on branch `phase-1`, at the Phase 0 BASEs plus the WP2 queues (wordcraft +5, gridcraft +22, deckcraft and pdfcraft pristine).
Machine: windows-gnu, 11.8 GB RAM, `-j 4`, one build at a time (build lock, RAM gate, no other rustc.exe).

## Decision: **D-A (keep)** for all four apps
`domains/{docs,sheets,slides,pdf}` build the desktop GUI, the CLI and the web app (trunk) from our workspace. The package `[[bin]] path`
points at the unmodified `upstream/<app>/apps/<app>*/src/main.rs`. No fallback to D-B was needed. Step 5(a) (sheets desktop, debug) passed
on the first build (850 s). Workspace inheritance through path deps, deckcraft's git `[patch]` and pdfcraft's 8 vendored `[patch]` paths all resolve.

## What was built
- `cargo xt gen-domain [--check] <d|all>`, `cargo xt lockdiff <domain-lock> <upstream-lock>`, `cargo xt domain-upstream <d>` (`xtask/src/domain.rs`, `toml_edit`).
  `gen-domain --check all` -> `4 domains up to date`.
- Per domain: `domain.toml` (input), generated `Cargo.toml` + `apps/<d>-{desktop,web,cli}/Cargo.toml`, `Cargo.lock` seeded from upstream, our `build.rs`
  (upstream's with icon paths retargeted into `upstream/<app>/assets`), `index.html` (`data-bin` renamed, icon paths retargeted) and `Trunk.toml`
  (`dist = E:/office-target/p1/dist/<d>-web`).
- `scripts/build.sh <d|all> <desktop|web|cli|all> [--release]`: build lock, rustc/RAM gate (3 GB), `CARGO_TARGET_DIR=$OFFICE_TARGET/<d>`, `-j 4`, `--locked`,
  log `$OFFICE_TARGET/logs/build-<d>-<what>.log`, wall time to `times.tsv`.
- Bin names: desktop `docs|sheets|slides|pdf`, CLI `<d>-cli`, web `<d>-web`. Packages `<d>-desktop|web|cli`, version 0.1.0.

## lockdiff (domain lock vs upstream lock)
```
docs:   0 version changes, 29 removed (dev/xtask), 0 foreign additions
sheets: 0 version changes, 18 removed (dev/xtask), 0 foreign additions
slides: 0 version changes, 38 removed (dev/xtask), 0 foreign additions
pdf:    0 version changes, 21 removed (dev/xtask), 0 foreign additions
```
`cargo build --locked` passed for every target, so no lock needed rewriting after seeding.

## Results (release; wall seconds from `times.tsv`; "inv." = Phase 0 inventory)
| | docs (wordcraft) | sheets (gridcraft, +22 queue) | slides (deckcraft) | pdf (pdfcraft) |
|---|---|---|---|---|
| GUI build | 844 s (inv. 556) | 515 s (inv. 323) | 795 s (inv. 612) | 586 s on retry (inv. 2636, inflated); first try failed at build.rs after 1040 s |
| GUI exe | 82,260,799 B (inv. 82,316,979) | 42,338,227 (inv. 41,962,136) | 92,407,947 (inv. 92,392,362) | 69,217,524 (inv. 69,167,142) |
| GUI alive after 15 s | yes | yes | yes | yes |
| CLI build (after GUI) | 389 s (inv. 262) | 157 s (inv. 130) | 387 s (inv. 301) | 789 s (inv. 1400, inflated) |
| CLI exe | 46,378,503 (inv. 46,369,937) | 18,295,136 (inv. 17,574,860) | 65,374,556 (inv. 65,352,235) | 41,791,951 (inv. 41,783,522) |
| CLI `--version` | `wordcraft-cli 0.1.0` | `gridcraft-cli 0.1.0` | `deckcraft-cli 0.1.0` | `pdfcraft-cli 0.8.0` |
| Web build (trunk, incl. wasm-opt) | 1155 s (inv. 897) | 529 s | 1154 s | 1111 s |
| wasm raw | 42,561,682 (inv. 42,597,532) | 19,147,347 (inv. 18,973,730) | 35,126,706 (inv. 35,120,783) | 39,080,768 (inv. 39,076,254) |
| wasm gzip -9 | 15,872,496 (inv. 15,849,354) | 7,103,118 (inv. 7,041,161) | 13,904,510 (inv. 13,905,434) | 14,385,169 (inv. 14,379,872) |
| wasm brotli -q 11 | 9,448,978 (inv. 9,441,597) **+0.08 %** | 4,772,272 (inv. 4,731,289) **+0.87 %** | 9,309,372 (inv. 9,300,367) **+0.10 %** | 9,392,930 (inv. 9,390,759) **+0.02 %** |
| dist | html, js, wasm, ico | same | html, js, wasm, png | html, js, wasm |

The sheets +0.87 % brotli is the gridcraft queue (22 patches, +4,474/-330 lines), reported separately as the plan asks; the other three are within 0.5 %.
Build times are single runs; docs, sheets and slides took 1.1x to 1.6x the Phase 0 times (sccache wrapper overhead and machine load were not separated), pdf is far below the inflated Phase 0 figure.

Fonts: Phase 0 had `CRAFT_FONTS_DIR` only for deckcraft (its `.cargo/config.toml`; `upstream/craft-fonts` sat next to it). For like-for-like sizes `scripts/build.sh`
exports it **only for `slides`** (`OFFICE_FONTS=all` restores the plan's global `env.sh` setting). The size budgets (WP6a) must state which setting they use.

CLI smoke (same commands as Phase 0, working dir `E:/office-target/p1/smoke`):
- docs: `run --template sample --cmd 'select.text=...' --cmd format.bold --save out.docx` OK, `convert out.docx out.pdf` OK (75,932 B, `%PDF-`, same size as Phase 0).
- sheets: `run --sample sales --out s.xlsx` (11,163 B, same as Phase 0), `check`: 22 formulas, 22 matched; `eval '=SUM(1,2,3)'` = 6.
- slides: `run --sample --save s.deckcraft` OK, `convert` to .pdf OK (1,045,654 B).
- pdf: `info`, `check` on the 1,744 B `form.pdf` fixture OK (5 fields).

GUI `--version`: `docs` prints `wordcraft 0.1.0 (dev)`, `sheets` `GridCraft 0.1.0`, `slides` `deckcraft 0.1.0`, `pdf` `pdfcraft 0.8.0`.
Web: each `dist/<d>-web` has `index.html`, a `.js` and a `_bg.wasm` (`ls $OFFICE_TARGET/dist/*-web/*.wasm | wc -l` -> 4). Loading in a browser belongs to the WP6a web smoke and was not run here.

## sccache verdict: works for native crates, not for web; no help on cold builds
- First build (sheets debug, 250 requests): 198 misses, 0 write errors, 48 non-cacheable (crate-type: bins and proc-macros). Zero hits on a cold cache, as expected.
- Real rebuild (sheets debug, `cargo clean -p serde_json -p quick-xml -p zip -p image`, then build): **11 of 11 hits (100 %)**, 45 s.
  So sccache works with the windows-gnu rustc. Keep `RUSTC_WRAPPER=sccache` for native builds; its value is rebuilding after a target dir is cleaned.
- **wasm32 web builds fail under sccache**: `failed to spawn rustc ... (os error 206)`; the `web-sys` command line (hundreds of `--cfg feature=...`) exceeds the Windows limit.
  `scripts/build.sh` therefore unsets `RUSTC_WRAPPER` for `web` (recorded as `wrapper=none` in `times.tsv`). CI must do the same.

## Deviations and findings
1. **pdf desktop `build.rs`**: upstream's does `#[path = "src/windows_manifest.rs"] mod ...`. The first release build failed after compiling all dependencies (1040 s). Our build.rs now points at
   `upstream/pdfcraft/apps/pdfcraft/src/windows_manifest.rs`; the retry succeeded. WP7 must keep that path when it switches the icon to the overlay.
2. The domain root lists `crates/*` as a member only if `domains/<d>/crates` exists (cargo errors on a glob over a missing directory).
3. `[[test]]`, `[dev-dependencies]` are dropped (removed in lockdiff); target-gated dependency tables are kept so the lock matches.
4. `CARGO_PKG_VERSION` is `0.1.0` in every app (pdf-cli still prints `0.8.0`, it uses the engine's build_info). Anything keyed on upstream's version (web `ready` message, user-agent) changes; WP7 should set the brand version.
5. Trunk `dist` is an absolute `E:/office-target/p1/dist/<d>-web` in `Trunk.toml` (Trunk cannot expand environment variables there).
6. The build lock only serialises our own scripts; the rustc.exe wait loop covers foreign builds.
7. App tests are not run in the domain (pdfcraft `CARGO_MANIFEST_DIR` uses are test-only, plan risk 2); they run in the upstream workspace (WP6b).
8. Branding strings (descriptions, ProductName in build.rs, index.html titles) are still upstream's; WP7.

## W2 verdict: **GO**
All 12 targets build from our workspace with the upstream lock (0 version changes), the artefacts run, and web sizes match Phase 0 (0.02 % to 0.10 % brotli, +0.87 % for sheets from its queue).
