# Status

**Current phase:** 0 — Evidence and architecture decision (branch `phase-0`)
**Last updated:** 2026-10-15

## Done
- Repo initialised; `CLAUDE.md`, `docs/BRIEF.md`, `docs/STATUS.md` created.
- Environment checked (see below).

## Measured
| What | Value | How |
|---|---|---|
| Upstream HEADs reachable | all 6 | `git ls-remote`, 2026-10-15 |
| Upstream velocity (commits in last 7 days) | wordcraft 701, gridcraft 993, deckcraft 859, pdfcraft 823 (≈100–140/day each) | `git log --since='7 days ago'` |
| Total commits | wordcraft 727, gridcraft 1015, deckcraft 884, pdfcraft 1165, craft-libs 116, craft-fonts 10 | `git rev-list --count HEAD` |
| Toolchain floor | edition 2024 everywhere; rust-version 1.90 (wordcraft, gridcraft) / 1.95 (deckcraft, pdfcraft, craft-libs) | `Cargo.toml` |
| UI framework | egui/eframe 0.36 in all four apps | `Cargo.toml` `[workspace.dependencies]` |

## Task 2 — layer maps (done, static read)
Files: `docs/phase0/layer-map-{wordcraft,gridcraft,deckcraft,pdfcraft-craftlibs}.md`.

| App | UI replaceable | Command surface | Renderer w/o egui | Input/IME location | i18n |
|---|---|---|---|---|---|
| wordcraft | 4/5 | 629 ids, `Session::run(id, json)` | yes, display list → RGBA | hit-test in layout; **pointer→selection + IME in ui-egui** | TSV catalogs, 23 langs (ui-egui) |
| gridcraft | 3/5 | ~340 `cmd!`, `Session::execute(id, json)` | charts yes, **grid no** | **editor, hit-test, keys in ui-egui** | Rust tables, 11 langs (ui-egui) |
| deckcraft | 4/5 | 320, `Session::execute(id, json)` | yes, vello_cpu → RGBA | **engine** (keys, pointer, IME compose) | none |
| pdfcraft | 4/5 | 200 static ids + ~229 JSON automation tools; `Edit` enum not serialisable | yes, hayro → RGBA | ui-egui | `tl!` catalogs, 23 langs (ui-egui) |

Brief corrections:
- wordcraft has 629 command ids, not 433. The docs say 422 and are stale.
- `HOSTING.md` does not exist in any repo or in wordcraft's git history. The "13 / 5 MB" claim
  cannot be traced.

Cross-cutting findings:
- Every app enforces "nothing below the UI layer depends on egui" with `xtask layers`.
- No app has an FFI or wasm-bindgen API over the engine. Remote access is limited to stdio MCP and a
  TCP JSON-lines control channel.
- pdfcraft-web already supports `?file=<url>` fetch and a postMessage host bridge for Nextcloud
  embedding. gridcraft-web has `?host=parent`. wordcraft-web and deckcraft-web have neither.
- craft-libs is a git dependency pinned to **5 different revs** across the apps.
  craft-settings, craft-atomic and craft-cms are unused, and the apps re-implement atomic writes
  locally.
- deckcraft always compiles 8 video and audio codecs and symphonia into the engine, wasm included.
  There is no feature gate.
- The i18n mechanisms differ in 3 apps, and deckcraft has none.
- gridcraft PDF export uses only the standard-14 fonts. This is a Cyrillic fidelity risk to measure.

## Task 1b — one-workspace spike (done; resolve only, no compile)
Details: `docs/phase0/workspace-conflicts.md` and `spikes/workspace/`.

**Verdict: feasible with explicit pins, but each app's root `[workspace]` has to be removed.**
- Upstream crates cannot join a parent workspace while their own root `Cargo.toml` declares one
  ("member of the wrong workspace").
- That rules out plain git submodules plus one root workspace unless each root manifest is patched
  or deleted.
- Result: 97 members and 923 packages in the lockfile. Nothing was compiled, so API breakage after
  the version unification is **not yet verified**.

Conflicts:
- **Version mismatch.** pdfcraft is at 0.8.0, the others at 0.7.0.
- **Two incompatible hayro forks.** pdfcraft's vendored copy and craft-libs' vendored copy differ in
  2, 15 and 10 files across three crates. Only one fork can win a shared `[patch]`.
- **pdfcraft's `[patch]` set** (winit, egui-wgpu, vello_common, lopdf) would apply to all four apps.
- **krilla** comes from three different sources.
- **craft-libs** is pinned to 3–4 different revs.
- **Smaller ones.** skrifa 0.44 vs 0.47; one shared lint set and one shared profile set; rust-version
  1.90 → 1.95.

Duplicated crate names (2+ versions in the tree):

| Scope | Count |
|---|---|
| merged workspace | 32 |
| wordcraft alone | 15 |
| gridcraft alone | 7 |
| deckcraft alone | 17 |
| pdfcraft alone | 27 |

Of the crates that share a role across apps, only `metafile` is a real copy (wordcraft and deckcraft
share byte-identical files). geom, pdf and render share names only.

## In progress (Phase 0)
- Task 1 inventory: desktop and web builds plus tests, run one at a time
  (→ `docs/phase0/inventory.md`).
- Spike design for tasks 3 and 4 by architect/opus (→ `docs/phase0/spike-plans.md`).
- Pending: task 3 mobile spike, task 4 document-server spike (after builds), task 5 fidelity
  baseline (blocked on `corpus/`), task 6 ADR.

Upstream HEADs at start of Phase 0:

| Repo | HEAD |
|---|---|
| wordcraft | fd0bb2fb1ec081e3eddc4d01917637fef0cafdfe |
| gridcraft | e6b6b349708ab4978cb3f061f6bd6ecbc25667cb |
| deckcraft | 9859da39255c0262cd592bb57101d7f2fa961f72 |
| pdfcraft | afa8e1843cfed613f3d403119f6c2649bce824c9 |
| craft-libs | 8f5a2831614e632f91ad367f7af2a6e5b61b9529 |
| craft-fonts | 3d9ab3a132ed0eff457e424226a435a901da30d3 |

## Environment
- Windows 10, 12 cores, 11.8 GB RAM, Intel UHD 630 (integrated GPU only).
- rustc/cargo 1.97.1, `stable-x86_64-pc-windows-gnu` (MSYS2 ucrt64 gcc). No MSVC toolchain.
- Node 24.13, Python 3.10.11, Docker 29.5.3.
- Android SDK + NDK 27.1, emulator 37.1, AVDs `HRM_Dev_33`, `HRM_Pixel_8`.
- Disk: C: ~34 GB free (too small for all build output); E: ~618 GB free → build output on E:.
- No macOS → iOS simulator testing is **not possible** on this machine.
- Not installed: LibreOffice (`soffice`), wasm-pack/trunk/wasm-opt (to be added as needed or run
  in Docker).

## Open risks / blockers
- `corpus/` is **missing** → fidelity baseline (task 5) blocked until the owner provides it.
- `vendor-notes/` (`merge_lines.py`, `uz-translit-onlyoffice/`) is **missing** → not needed for
  Phase 0, needed for Phases 2–3.
- No macOS → iOS part of the mobile spike cannot be done here.
- 11.8 GB RAM: parallel Rust builds + Android emulator may contend; builds run with limited jobs.
- Upstream is a Windows-gnu build here; upstream CI likely targets MSVC/Linux — GNU-specific build
  failures are possible and will be recorded, not assumed.

## Next step
Task 1 (inventory): clone the six repos into `upstream/`, build desktop + web, record numbers.
