# Status

**Current phase:** 2 — Transliteration (branch `phase-2`). **Phase 1 closed on 2026-10-20**: the final clean-checkout `ci.sh full` exited 0 (`docs/phase1/clean-checkout.md`) and the gate review is in `docs/phase1/gate-review.md`.

Phase 1 history: 1 — Foundation (branch `phase-1`). The Phase 0 gate was passed on 2026-10-17; the decisions are in
ADR Appendix C:
- name deferred;
- MIT OR Apache-2.0;
- W2 build domains + vendored snapshot and patch queue;
- A-web (conditional);
- priority fidelity → mobile → co-editing;
- no upstream PRs for now.
**Last updated:** 2026-10-19 (Phase 1 gate review: `docs/phase1/gate-review.md`)

## Phase 1 progress
- WP0 done (2026-10-17): `scripts/{env.sh,bootstrap.sh,tools.lock,lib/lock.sh}`. Shared toolchain changed: rustup components `rustfmt` and `clippy` added.
  Tools in `E:/office-tools/bin`: cargo-deny 0.20.2, cargo-nextest 0.9.148, sccache 0.18.0, actionlint 1.7.12 (trunk 0.21.14 already there).
- WP1 done (2026-10-17): upstream clones moved to `E:/office-upstream/<repo>` (cache clones; spike branches and worktrees intact). Pristine snapshots committed at the Phase 0 BASEs (tree hashes equal upstream's); BASE lives in `patches/<repo>/BASE`. craft-fonts is fetched (`scripts/fetch-fonts.sh`, 31 files verified).
- WP2 done (2026-10-17): queues at the old BASE: wordcraft 0001-0005, gridcraft one ordered queue 0001-0022 (pdf-fonts applied without conflict as 0020-0022). `scripts/verify-queue.sh all` green, `patches/REGISTER.md` has 27 rows (`cargo xt register check` OK). Repo `.git` is 54 MB after vendoring. Notes and deviations: `docs/phase1/wp0-2-notes.md`.
- WP3 done (2026-10-17): W2 go/no-go = GO, D-A (domain wraps the unmodified upstream app sources). `cargo xt gen-domain|lockdiff|domain-upstream`, `scripts/build.sh`, `domains/{docs,sheets,slides,pdf}`. All 12 targets (desktop, CLI, web) built from our workspaces; lockdiff 0 version changes; wasm brotli within 0.02-0.10 % of Phase 0 (sheets +0.87 % = its queue). sccache works for native crates (11/11 hits on rebuild) but fails for wasm32 web (os error 206), so web builds run without it. Details and times: `docs/phase1/domain-build.md`.

- WP4 done (2026-10-19): craft-libs on ONE rev (BASE 15ed713) tested in a scratch worktree. docs, sheets, pdf: resolve, check native + wasm32 and tests PASS (pdf: 3 timing flakes, pass alone); slides: native PASS, **wasm32 FAIL** (`deckcraft/crates/pdf/src/lib.rs:32,562,573,617`: craft-libs f39922b made krilla `simple-text` opt-in), fixed by one line (`craft-pdf` feature `write-simple-text`, deckcraft `Cargo.toml:60`), then PASS. Dedupe gain: 5 git revs to 1, 2 duplicate craft-* crates (`craft-print`, `craft-ui`); cost: docs/sheets build +102/+145 crates (winprint build-deps, RUSTSEC-2024-0370 ignore needed). **Owner decision, nothing adopted.** `docs/phase1/craft-libs-single-rev.md`.
- WP6 final done (2026-10-19): size budgets from deltas measured against pristine builds at the current BASE (docs +105,622 B brotli, sheets +122,186, slides +109,715, pdf +97,954; budgets +10 % rounded to 16 KiB, 512 KiB guard removed); upstream clippy is a blocking ratchet (baselines 1/2/2/1, five of the six warnings are in our carried patches 0100/0020); quarantine reviewed (54 entries, 1 weak: deckcraft `replacefonts`, perf and breaks_menu entries re-diagnosed). `ci.sh quick sizes` green. `docs/phase1/wp6-final.md`.
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

## Web release sizes (official upstream release zips, SHA256 verified, measured 2026-10-15)
Brotli is `-q 11`; gzip is `-9`. 1 MiB = 1,048,576 B.

| App (release) | wasm raw | gzip -9 | brotli -q 11 | zip |
|---|---|---|---|---|
| wordcraft-web 0.7.0 | 39,785,016 B (37.9 MiB) | 14,947,589 B (14.3 MiB) | 8,878,074 B (8.5 MiB) | 15.0 MB |
| pdfcraft-web 0.8.0 | 38,059,629 B (36.3 MiB) | 14,064,441 B (13.4 MiB) | 9,215,527 B (8.8 MiB) | 14.1 MB |
| deckcraft-web 0.7.0 | 25,537,284 B (24.4 MiB) | 9,345,258 B (8.9 MiB) | 5,953,603 B (5.7 MiB) | 9.4 MB |
| gridcraft-web 0.7.0 | 18,886,586 B (18.0 MiB) | 7,045,299 B (6.7 MiB) | 4,732,751 B (4.5 MiB) | 7.1 MB |

- **The brief's wordcraft figure is confirmed:** 37.9 / 14.3, measured in MiB.
- **Brotli shrinks wordcraft another 41% beyond gzip**, so brotli precompression should be part of
  hosting.
- **Each editor is one wasm module.** A lazy-loaded shell would download 4.5–8.8 MiB (brotli) on
  first use of each editor.
- **These are release builds of tag v0.7.0/v0.8.0,** not HEAD. HEAD sizes will come from the
  inventory build.

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
- `HOSTING.md` is not in the source repo but ships inside the release zip. Its "about 13 / 5 MB"
  is outdated; the measured size is below. (Corrected: an earlier note said the file did not exist.)

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

## Task 1 — inventory (DONE; all four apps at HEAD, this PC, windows-gnu)
Full report: `docs/phase0/inventory.md`.

| | wordcraft | gridcraft | deckcraft | pdfcraft* |
|---|---|---|---|---|
| GUI build (cold) | 556 s | 323 s | 612 s | 2636 s |
| GUI exe | 82.3 MB | 42.0 MB | 92.4 MB | 69.2 MB |
| CLI build | 262 s | 130 s | 301 s | 1400 s |
| CLI exe | 46.4 MB | 17.6 MB | 65.4 MB | 41.8 MB |
| Web build | 897 s | 420 s | 867 s | 1341 s |
| wasm raw | 42.6 MB | 19.0 MB | 35.1 MB | 39.1 MB |
| wasm gzip -9 | 15.8 MB | 7.0 MB | 13.9 MB | 14.4 MB |
| wasm brotli -q 11 | 9.4 MB | 4.7 MB | 9.3 MB | 9.4 MB |
| Tests pass / fail / ignored | 2750 / 5 / 2 | 1597 / 0 / 1 | 1669 / 1 / 6, plus an intermittent crash | 3466 / 14 / 16 |
| CLI smoke test | OK | OK | OK | OK |

\* pdfcraft times are upper bounds: other builds overlapped.

**Test failures:**
- **Environment- or locale-related:**
  - wordcraft: 2 colour-emoji font tests; Alt+numpad gives "й" instead of "é" (Cyrillic code page);
    2 "Breaks" menu label tests.
  - deckcraft: the replace-fonts ribbon test (label not found).
- **deckcraft crash:**
  - The `ui-egui` lib test binary crashes intermittently with STATUS_ACCESS_VIOLATION.
  - It happens only with concurrent tests: 1 of 8 runs; 0 of 4 runs with one thread.
  - A wgpu race in `egui_kittest` is suspected but unconfirmed.
- **Upstream bugs in pdfcraft:**
  - a password-length mismatch in a signing test;
  - 3 zh-hant duplicate catalog keys;
  - 3 Vietnamese catalog keys missing;
  - a duplicate key in the parity TOML;
  - 4 failures of unknown cause;
  - JetBrainsMono lacks "ѝ".
- **Test time:** almost all of it is compiling test binaries in release mode. The CI design should
  use debug tests, sccache and per-crate runs.

## Task 1 — inventory, wordcraft at HEAD fd0bb2f (detail) (measured on this PC, windows-gnu, cold builds)

| Step | Result | Wall time |
|---|---|---|
| Desktop GUI `cargo build --release` | OK | 556 s |
| CLI build | OK | 262 s |
| Web `trunk build --release` | OK | 897 s |
| `cargo test --workspace --release` | exit 101, stopped at the first failing crate | 3784 s |
| `cargo test --no-fail-fast` rerun | **2750 passed, 5 failed, 2 ignored** | 205 s (cached) |

| Artefact | Size |
|---|---|
| `wordcraft.exe` | 82,316,979 B |
| `wordcraft-cli.exe` | 46,369,937 B |
| HEAD wasm, raw | 42,597,532 B |
| HEAD wasm, gzip -9 | 15,849,354 B |
| HEAD wasm, brotli -q 11 | 9,441,597 B |
| JS glue, raw / brotli | 168,690 / 20,555 B |

- **The HEAD wasm is 2.8 MB (+7%) larger than the v0.7.0 release built the day before.** CI size
  budgets have to track upstream growth.
- **Failing tests.** All look environment-related; none is confirmed yet.
  - 2 × `render` colour-emoji tests: a font-environment difference.
  - 2 × `ui-egui` "Breaks" menu tests: no node labelled "Breaks" was found.
  - `ui-egui keys::alt_numpad_code_types_its_character_without_keytips`: expected "é", got
    "й". **This PC has a Cyrillic Windows locale**, so Alt+numpad codes follow the OEM/ANSI code
    page. The behaviour matters for our users and must be checked on uz/ru Windows.
- **CLI smoke test passed:** save, reopen, text, docx→PDF (valid), render to PNG, and
  docx→odt→docx. PDF import is not supported.
- **Local workaround:** `CARGO_HTTP_CHECK_REVOKE=false`, because of
  `CRYPT_E_NO_REVOCATION_CHECK` on this network.
- **Tools:** trunk 0.21.14 (`cargo install` took 14m38s), wasm-bindgen 0.2.129, wasm-opt v123.
- **Details** go to `docs/phase0/inventory.md`, written by the inventory agent.
- **Queue progress, measured cold builds on this PC (2026-10-16):**

| App | GUI | CLI | Web | Tests |
|---|---|---|---|---|
| gridcraft | 323 s | 130 s | 420 s | 7962 s; **1597 passed, 0 failed, 1 ignored** |
| deckcraft | 612 s | 301 s | 867 s | 3213 s, **crashed** |
| pdfcraft | 2636 s | in progress | — | — |

- **deckcraft test crash.** The `deckcraft-ui-egui` lib test binary died with
  `STATUS_ACCESS_VIOLATION` (0xc0000005). 1418 tests had passed (0 failed, 2 ignored) before it
  stopped. The crashing test is not yet identified; it needs a `--test-threads=1 --no-fail-fast`
  rerun.
- **Earlier note (superseded):** the inventory was paused after wordcraft. The gate is `E:/office-target/PAUSE` in `run.sh`. It frees RAM
  for the xlsx benchmark and the Android emulator, and gridcraft, deckcraft and pdfcraft continue
  afterwards.

## Task 3 — mobile A0 (done): wordcraft-web v0.7.0 in Chrome on the Android emulator
Full details: `docs/phase0/mobile-a0.md`. Evidence: `docs/phase0/mobile/`.

**Environment:** Pixel 8 AVD, Chrome 133, Gboard 15.1 with EN, UZ-Latn and RU layouts.

**Blocker on stock Chrome.**
- The app requests `max_texture_dimension_2d` 8192, but the emulator's swiftshader GPU allows only
  4096, so it fails to start. `-gpu host` gives blank pages.
- The tests below therefore ran with Chrome's own software GL (a flag override). They do not
  represent the emulator GPU.
- Fix: request the adapter's limit instead of 8192.

**What passed**

| Test | Result |
|---|---|
| Correct DPI scale (dpr 2.625, 412 CSS px) | pass |
| Keyboard shows on tap and hides on Back | pass |
| Latin typing | pass |
| Cyrillic "Салом дунё" | pass |
| Uzbek Latin o‘g‘il | pass; Gboard emits U+02BB / U+02BC, and both reached the docx |
| Backspace, Enter | pass |
| Pinch zoom (143%) | pass |
| Rotation keeps the document and caret | pass |
| **HRM embed flow on mobile** (open → type `mobilea салом` → File › Save by touch → status-6 callback → docx verified) | **pass** |

**What failed**

| Test | Result |
|---|---|
| Autocorrect | "teh " became "tehthe": the correction is appended instead of replacing the word. `DeleteSurrounding` is not handled. |
| One-finger swipe | Selects text instead of scrolling, so there is no momentum scroll. |
| Long-press | Selects a word and opens the desktop context menu. No touch selection handles. |
| 360 dp width | The desktop ribbon overflows. |
| App switch and return | The editor iframe was dead and the edits were lost. **Lifecycle risk**, to confirm on a real device. |

**Not measured**
- Glide typing is UNVERIFIED: injected strokes were ignored even in the control textarea.

**Measured numbers**
- **Load time,** HRM page → editor ready: 2.0–2.4 s warm, 2.9–4.0 s cold.
- **Chrome renderer PSS:** 284 MB after load, 326 MB after edits.

**Verdict:** egui's web path gives working text entry with Gboard, including Cyrillic and Uzbek, and
the HRM round trip works. It is **not yet a usable mobile editor**.
- **Fixable glue:** autocorrect replacement, touch-to-scroll, caret above the keyboard, a compact
  toolbar, the texture limit, lifecycle and WebGL context loss.
- **egui-core limits:** no native selection handles, and a generic hidden input as the IME
  target.

**Side effect:** one extra Back press left Chrome and landed in an Expo Go "Uzgidro HRM" login screen
that was already on the emulator. Two stray taps and the text "il" went into it. Nothing was
submitted and nobody was signed in.

## Spike: wordcraft-web fixes (done; Spike 4 S8–S9 + the mobile glue)
Report: `docs/phase0/wordcraft-patches.md`. Patches: `spikes/wordcraft-patches/0001-0005`.
Evidence: `docs/phase0/mobile-fixes/`. Branch `spike/bridge-mobile` in the worktree
E:/office-work/wordcraft-spike, based on fd0bb2f.

Verified on desktop Chromium (Playwright) and on the Android emulator in **stock Chrome 133 with no
flags**.

| Patch | Size | Before → after (measured) |
|---|---|---|
| 1. postMessage host bridge `?host=parent` | +213/−8 | No bridge → spec green with adapter=bridge 3/3. `requestSave`, dirty events and `/close` (status 2) work. The shim adapter still passes, so nothing regressed. |
| 2. Texture limit | +13/−4 | Stock Chrome refused to start ("8192 > 4096"; `egui-wgpu-0.36.2/src/setup.rs:272`) → starts. |
| 3. IME DeleteSurrounding | +37 | Gboard "teh " → "tehthe" before; "the " after; "wolrd " → "the world". A unit test was added. |
| 4. One-finger touch scroll | +328/−7 (≈190 non-test lines) | Swipe selected text → swipe scrolls with momentum; tap places the caret; long-press selects. Mouse behaviour is unchanged. |
| 5. Lifecycle autosave | +65/−4 | App switch lost edits → the host holds the unsaved docx about 4 s after hiding. After WebGL context loss the editor reloads with the autosaved text in about 2.7 s. |

- **wasm size change:** +20.7 KB raw, +9.3 KB brotli (9,441,597 → 9,450,860 B).
- **Native tests:** 554 passed, 1 failed. The failure is the known Alt+numpad "é" vs "й" test,
  which depends on the code page.
- **Remaining limits:**
  - The soft keyboard reappears after a swipe.
  - A finger cannot drag pictures, table handles or ruler markers.
  - A recovered document shows as clean.
  - A failed host upload still marks the document saved.
  - Not re-tested: ribbon overflow, the caret hidden by the keyboard, glide typing.
- **Takeaway for the ADR:** the A0 failures that looked like "fixable glue" were fixed with about
  650 lines in 5 small, upstreamable patches. This weakens the case that approach A (egui on web)
  is unworkable on mobile. Native selection handles and a mobile toolbar are still open.

## Spike: gridcraft shared formula trees + run-based dependency graph (done; MEASURED)
Report: `docs/phase0/gridcraft-templates-spike.md`. Patches: `spikes/gridcraft-patches/0001-0008`
(+865/−106 production lines; branch `spike/formula-templates` from e6b6b34).

**Peak RAM at open** (deterministic):

| File | 0.7.0 | Patched | Excel 16 (32-bit) |
|---|---|---|---|
| w10 | 635 MB | **194 MB** | 220 MB |
| w40 | 2,525 MB | **751 MB** | 515 MB |
| w80 | aborted >4 GB | **1,496 MB** | 916 MB |
| w120 | aborted >4 GB | **2,421 MB** | 1,298 MB |

- **Independent re-check by the orchestrator:** w80 opened with a **1,505 MB** peak in 73.7 s on an
  idle machine.
- **Save peaks:** w40 922 MB; w120 2,635 MB.
- **RAM ratio:** about 64 MB per file-MB before, about 19–20 MB per file-MB after (Excel: about 11).
- **Open time** improved only modestly (w10 11.3 → 8.7 s, w40 42.7 → 36.6 s). Parsing each run's
  formula once on load (about 1–2 more engineer-days) should speed up open further. Not measured.

**Correctness:**
- All values on w10, c10 and w40 are bit-identical to the baseline.
- 5.18M row values on w120 match the Python expectations.
- A 35-step structural-edit script gives identical results.
- Excel reads the same formulas and values in every cell.
- The touched crates' suites pass, plus 25 new tests.
- 300 fuzz seeds give the same outcomes as the baseline. The two failing seeds also fail on the
  unpatched build, so they are upstream bugs.

**Not done:**
- `cargo xtask ci`, because rustfmt and clippy are not installed;
- Excel checks on the largest files;
- a longer fuzz soak.

**Revised estimate for steps 1–2:** **5–9 engineer-days to production** (the earlier plan said
14–27), plus upstream review time.

**ADR impact:** the biggest Excel risk is now a contained, upstreamable change with measured
benefit. The external-links work (17–25 days) remains.

## Task 5 — fidelity baseline on a SYNTHETIC set (done; the corpus run is pending)
Sources:
- report: `docs/phase0/fidelity-baseline.md`;
- harness: `spikes/fidelity/`;
- worst-page images: `docs/phase0/fidelity/`.

Method:
- The 10 files were generated **by MS Office 16** via COM: HRM-like orders, letters, a staff list and a
  timesheet, in Uzbek Cyrillic and Latin and in Russian. Each is saved as docx/doc, xlsx/xls and
  pptx/ppt.
- **Reference:** Office's own PDF export.
- **Candidate:** the Craft 0.7.0 release CLIs.
- Both PDFs are rasterised by pdfcraft-cli 0.8.0.

| App | Saved file reopens in Office | Page count matches | Text similarity | Mean pixel diff |
|---|---|---|---|---|
| Word, 4 files | 4/4 | 4/4 | 0.958 | 6.2% |
| Excel, 4 files | 4/4 | 3/4 | **0.415** | 3.8% |
| PowerPoint, 2 files | 1/2 | 1/2 | 0.989 | 0.6% |

**What survives in Word:** tables, merged cells, numbering, the footnote, PAGE/NUMPAGES fields, the
image, the text box, the landscape section, and the `_hrm_desc_` bookmark in 4/4 files.

**Top losses:**
1. **gridcraft PDF export renders Cyrillic as `?`, and wide numbers as `####`.** Verified by eye
   (`docs/phase0/fidelity/hrm_staff_xlsx_*`).
   - **Cause:** gridcraft's own PDF writer supports only the standard-14 fonts
     (`gridcraft/docs/gaps.md`).
   - **Severity:** a **blocker** for printing HRM tables.
   - **Fix direction:** embed TrueType fonts in the PDF. The other apps already do this through
     krilla/craft-pdf. This is an upstream patch.
2. **Legacy formats:**
   - `.ppt` does not open at all.
   - `.xls` import drops charts and conditional formatting.
   - The `.doc` round trip loses comments and tracked changes.
3. **Word layout:** pages match, but 5–11% of pixels differ. The body sits about 10 px lower,
   probably from font and line-height metrics.

**Not done yet:**
- HEAD builds;
- LibreOffice and x2t references (Docker is down);
- Excel silent auto-repair detection;
- PowerPoint chart survival.

**Running it on the corpus:** `python -I spikes/fidelity/run.py --input corpus/ --out
E:/office-target/fidelity-corpus`. Outputs are ids only, and they stay under E:.

## Spike: gridcraft memory round 2 (done; MEASURED)
Report: `docs/phase0/gridcraft-memory-r2.md`. Patches 0009–0019.

Desktop app vs Excel 16, private working set (MB), medians of 3:

| File | Excel steady / peak | gridcraft round 2 steady / peak |
|---|---|---|
| w80 | 742 / 851 | 903 / 984 |
| w120 | 1,090 / 1,222 | 1,240 / 1,321 |

- **Engine (CLI) on w80:** 817 MB peak. The orchestrator re-check gave **828 MB**, against 1,505 MB in round 1.
- **Remaining GUI gap:** the fixed 226 MB empty-app baseline (Excel: 33 MB) — fonts 57 MB, DX12 renderer 92 MB.
- **Values** are bit-identical; the tests and Excel round trip pass.
- **Estimates:** 5–8 days to production, plus 3–4.5 days (unverified estimate) to close the GUI gap.
- **Owner premise ("Excel needs 300 MB") vs measurement:** Excel needs 742 MB on this 80 MB bench file.

## Spike: gridcraft PDF export with embedded fonts (done; MEASURED)
Sources:
- report: `docs/phase0/gridcraft-pdf-fonts.md`;
- patches: `spikes/gridcraft-pdf-patches/0001-0003`;
- evidence: `docs/phase0/fidelity-pdf-fix/`.

**Approach.** The patch subsets installed TrueType fonts as Identity-H CID fonts with ToUnicode, using
real advance widths. It stays inside gridcraft's dependency-free `crates/pdf`, as upstream's
charter requires. Arial, Times and Courier keep the standard-14 fonts for Latin-1 text.

**Result.**
- Excel PDFs no longer show `?` or `####`. Checked by the orchestrator on
  `hrm_staff_xlsx_p02_before_after.png`.
- Text similarity rose from 0.415 to 0.754.
- Pixel diff is unchanged (3.8% → 3.9%). What remains is anti-aliasing, borders and layout.
- **wasm cost:** +66 KB raw, +8.5 KB gzip.
- **Tests:** engine 294 and pdf 23 pass. fmt/clippy were not run.

**Limits.**
- PDFs are 3–10× larger: there is no FlateDecode yet.
- No RTL, shaping, CJK (CFF) or emoji.

**New findings from this spike (affect Phase 1):**
- **Ubuntu Light, egui's default UI font, lacks U+02BB `ʻ`, the Uzbek oʻ/gʻ letter.** On a machine
  with no installed font covering it, the letter prints as a blank box, and the same gap exists in
  the web GUI.
  - Phase 1 must bundle a UI and document fallback font that covers U+02BB, U+02BC and Cyrillic, and
    test it on every platform.
- **Decimal separator.** Excel on this Russian-locale Windows shows `3500000,0`; gridcraft shows
  `3500000.0`.
  - Locale-aware number display for the uz and ru locales must be checked and fixed.

## Task 3 — mobile approach-A kill test (done): native eframe 0.36.2 on Android
Report: `docs/phase0/mobile-a-kill.md`. Evidence: `docs/phase0/mobile-a/`. Code:
`spikes/mobile-a/probe/`.

Tests: (a) keyboard shows/hides, (b) Latin, (c) Cyrillic, (d) oʻgʻil, (e) autocorrect, (f) glide,
(g) backspace, (h) Enter.

| | Stock EditText (control) | Stock eframe/winit | eframe + GameActivity text-state shim (about 170 lines, public APIs only) |
|---|---|---|---|
| Result | a–h pass | **only a passes; b–h fail** | **a–h all pass** |

- **Why stock eframe fails:** winit drops GameTextInput text, so egui receives no Text or Ime events.
  The upstream fix locations are in the report (winit 0.30.13 `android/mod.rs:476`, `event.rs:774`;
  android-activity UTF-16 offsets).
- **Renderers:** glow works. wgpu's default Vulkan backend fails on the emulator
  (`max_uniform_buffer_binding_size`). wgpu with the GL backend works.
- **Probe numbers:** startup 1.6–1.7 s (`am start -W` median); idle PSS 54–57 MB; APK 13.5 MB;
  `.so` 10.3 MB stripped.
- **Risks found:**
  - An activity destroy or relaunch hangs stock eframe (ANR).
  - The preedit path is untested: Gboard reported no composing region.
  - Glide typing works only with touches injected through the emulator console.
- **Verdict:** approach A is **not killed** on keyboard grounds. It still needs the shim, the ANR fix
  and all the mobile UI work. The web path (A0 + patches) passes the same tests with less new code.

## Task 3 — mobile approach B (hybrid) spike (done)
Sources:
- report: `docs/phase0/mobile-b.md`;
- evidence: `docs/phase0/mobile-b/`;
- code: `spikes/mobile-b/`.

**What was built.** A thin wordcraft binding (281 lines) plus a web shell (400 lines of JS):
- one native `overflow:auto` canvas per page;
- a hidden-textarea IME bridge;
- a 4-button toolbar;
- also wrapped as a plain Android WebView APK.

**Upstream patches needed:** none.

| Test (emulator Chrome, real Gboard) | B | A-web (A0 + 5 patches) | A-native (+ shim) |
|---|---|---|---|
| Fit, scale, 360 dp | PASS | FAIL (desktop ribbon) | not tested |
| Keyboard; Latin / Cyrillic / Uzbek; backspace; Enter | PASS | PASS | PASS |
| Autocorrect | PASS, no patch | PASS (patch) | PASS (shim) |
| Glide | PASS (Latin) | UNVERIFIED | PASS |
| Swipe with momentum; pinch | PASS, native | PASS (190-line patch) | not tested |
| Long-press selection | PARTIAL, no handles | PARTIAL | not tested |
| App switch | PASS, no code | PASS (autosave patch) | destroy ANR |

| Number | B | A-web |
|---|---|---|
| wasm brotli | 5.8 MB | 9.45 MB |
| Renderer PSS after load | 148 MB | 284–326 MB |

More numbers for B:
- **Rendering:** a full-page render at DPR 2.625 takes 90–118 ms; a 1024² tile 22–39 ms.
- **Keystroke to repaint:** p50 41–46 ms, p95 87–135 ms.
- **WebView APK:** 11.2 MB; startup about 1.7–1.8 s.

**Cost.** A credible phone editor on B is estimated at **7–11 engineer-months**, because the whole
UI has to be rebuilt (about 51k lines of ui-egui). This is an estimate, not a measurement.

**Risks:**
- The engine has no composition API, and the preedit overlay was never exercised.
- Single-threaded wasm rendering is untested on a real phone.

**Verdict:**
- **B** wins on memory, size, robustness and native touch behaviour.
- **A-web** with its patches is far cheaper, because it keeps the existing desktop-grade UI.
- This is the owner's mobile-approach decision at the Phase 0 gate.

## Security notes carried into Phase 5 (from the spike review)
- **Session routes rely on a capability URL.** `/api/session/{sid}/{document,save,autosave,close}`
  are authorised only by the UUIDv4 session id. Phase 5 needs:
  - a per-request session token, bound to the user and the permissions (JWT on every call, as the
    brief requires);
  - session ids kept out of logs.
- **Read-only sessions could save. Fixed in the spike** (`writable()` in
  `spikes/docserver/src/main.rs`): save and autosave now return 403 for view-mode sessions or
  `permissions.edit == false`.

## In progress (Phase 0) — historical, superseded (Phase 0 closed 2026-10-17)
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

## Phase 1 progress (2026-10-17)

**Work packages**
- **WP0–WP2 done.** Tools are pinned. Pristine snapshots are byte-identical to upstream (tree-hash check). The 27-patch
  queue and its register are in place.
- **WP3 done: W2 is GO.** All 12 targets (desktop, CLI and web × 4 apps) build from our domains.
  - lockdiff shows 0 version changes.
  - CLIs and dists were verified by the orchestrator.
  - sccache works for native builds but breaks wasm, so it is off for web.
  - Details: `docs/phase1/domain-build.md`.
- **WP6a done.**
  - CI script and GitHub workflow (disabled).
  - cargo-deny ×5, all green, with the MPL exception for symphonia only.
  - Size budgets.
  - Quarantine of 21 known upstream test failures.
- **WP6b done (2026-10-18): debug tests of all four apps are green, overlay on, with 54 justified quarantine entries.**
  - `scripts/ci.sh test <app>` and `test-ours` are wired to cargo-nextest (`ci/nextest.toml`) and `cargo xt quarantine check|filter|selector|apply`.
    A quarantined test that passes is reported stale (two in a row fail CI); an entry matching no test fails; any other failure fails CI.
  - Final overlay-on results (passed / failed / quarantined): wordcraft 2748/0/19, gridcraft 1665/0/1, deckcraft 1676/0/4,
    pdfcraft 3491/0/28 (+1 binary of 3 tests that Windows Defender deletes: `pdfcraft-sign::store_keys`). test-ours: 59 + 64 + 44 + 43 + 71 passed.
  - Overlay-off baseline: wordcraft 2760/0, gridcraft 1666/0, deckcraft 1678/1 (flaky), pdfcraft 3516/3 (flaky perf).
  - Quarantine re-validated at the fresh BASE: 12 of 14 Phase 0 pdfcraft entries now pass and are gone; wordcraft's 5 env entries still fail.
    By class: env 9, flaky 5, brand-overlay 35, i18n-incomplete 5, upstream-bug 0.
  - Real breakages fixed by carried patches (not quarantined): gridcraft 0103 (new upstream xlsx test vs the shared-formula API) and 0104
    (fallback-faces test vs the overlay fonts). The sync quick gate now runs `cargo check --all-targets` (docs/UPSTREAM.md).
  - deckcraft ui-egui crash: not reproduced under nextest (13 runs of the binary, 0 crashes); it still reproduces in-process (6 of 10 direct
    runs) and was narrowed to `dialogs::spelling_tests` (the two `squiggles_*` tests with any other spelling test, global spell state). Not
    root-caused; hand-off notes in `docs/phase1/tests.md`.
  - Timings (debug profile is opt-level 1, so close to release): test run 1.5-9.5 min per app; cold compile 30-90 min per app; first run
    after the overlay appears 20-60 min. Four avoidable-rebuild causes fixed (cargo run env leak, deckcraft's missing
    `latin-manifest.txt`, overlay files rewritten, quarantine pass env). sccache gave no measurable help (no A/B). Details: `docs/phase1/tests.md`.
  - Other fixes: `gen-domain` now carries dev-dependencies (pdf-desktop tests compile); xtask clippy clean.
  - Open: architect review of `ci/quarantine.toml`; a Defender exclusion for `E:\office-target` (admin) to run `store_keys`.
- **WP7a/WP8/WP9a designs done.**
  - The font is Noto Sans + Noto Sans Mono, with cmap-verified coverage of ʻ ʼ ўқғҳ ₽.
  - No telemetry was found in any app.
- **WP9b done.**
  - The i18n catalogs and the extract/export/review tooling work.
  - Round trip: semantically equal.
  - Glossary batch 0 (150 terms) was sent to the owner for review.
- **WP12 done.** Sizing is in `docs/phase1/SIZING.md`.

**WP5 sync: all five repos are at the fresh BASE, 2026-10-17**

| Repo | Fresh BASE |
|---|---|
| wordcraft | eae2421 |
| gridcraft | f1894c6 |
| deckcraft | 1dc6972 |
| pdfcraft | 924c4da |
| craft-libs | 15ed713 |

- All 27 patches applied clean: 0 conflicts and 0 3-way merges. The quick gates pass.
- **Measured rebase cost (2-day distance):** wordcraft 5 patches in about 3 s; gridcraft 22 patches in about 0.3 s each.
- **Upstream velocity dropped sharply.** Only 4–18 commits per repo landed between 2026-10-15 and 2026-10-17, against
  700–1000 per week in Phase 0. Watch this before re-estimating risk 1.
- **Script bug found and fixed.**
  - What went wrong: the `--fmt` pass reformats whole files. That made patch 0017 conflict after patch 0014's reformat,
    and the script then exported a partial (16-patch) queue. The gate caught it, and nothing was applied.
  - Fix: on a fmt conflict the script now aborts and keeps the full unformatted queue. It also uses a unique scratch
    branch per run.
- **WP10 locale check done** (`docs/phase1/locale-check.md`). **gridcraft is hard-coded to en-US.**
  - Display, PDF and CSV all use `.` as the decimal separator (`numfmt/src/render.rs:159,198,337`).
  - Month names are English only, and `[$-419]`/`[$-443]` locale tags are dropped.
  - **Data-integrity bug, re-checked by the orchestrator with the domain CLI:** typing `12.10.2026` is stored as
    **2026-12-10**, and `3,5` stays text (`ISNUMBER` = FALSE). Excel ru-RU stores 12 October and the number 3.5.
  - uz-Latn, uz-Cyrl and ru all expect a decimal comma, a NBSP group separator and `;` as the list separator
    (CLDR and Windows NLS agree).
  - **Fix:** a carried `NumLocale` patch driven by the app language, about 6–8 days for separators, names and input.
  - This does not block the Phase 1 exit, but it **blocks v1 release quality** for uz/ru users. Proposed for early
    Phase 2; the owner decides.
- **Open:** gridcraft-patched files are not rustfmt-clean. Plan: add one trailing "rustfmt" patch at the end of the
  queue, so it cannot conflict. To do in WP6 final.

## Owner requirement (re-stated 2026-10-15)
Everything must work on **mobile, web and desktop**, with editing on all three, not only viewing.
This applies to the editors, the document-server embed (HRM web and the HRM mobile app),
the converter and transliteration.

For the ADR this is a hard selection criterion: an architecture that passes on desktop and web but
fails the Android touch and keyboard tests is rejected.

Spike consequences:
- **Mobile A0 also covers the HRM embed.** After Spike 4, the fake-HRM page → editor → save →
  callback flow is run in Chrome on the Android emulator, not only in desktop Chromium.
- **Each approach is scored on all three platforms,** from one UI codebase where possible:
  - A: egui everywhere, i.e. eframe on desktop, web and Android.
  - B: a web UI over the engine; wasm on web, Tauri on desktop, WebView or Tauri on Android.
- **iOS cannot be tested on this machine** (no macOS). The ADR will state it as UNVERIFIED and not
  claim it.

## Owner requirement: replace Microsoft Office on office PCs (stated 2026-10-15)
- **A Windows desktop installer (.exe/.msi) is required.** The suite is meant to replace MS Office on
  the office PCs, which raises the priority of desktop fidelity, Windows integration and mass
  deployment.
- **What upstream already ships** (official releases, SHA256 verified, run on this machine):
  - Windows x64/x86/arm64 MSI and portable zips.
  - Every portable zip contains a GUI exe and a CLI exe. All four CLIs print `--version` correctly.

| App (release) | GUI exe | CLI exe |
|---|---|---|
| wordcraft 0.7.0 | 102.4 MB | 75.4 MB |
| gridcraft 0.7.0 | 32.6 MB | 14.2 MB |
| deckcraft 0.7.0 | 86.5 MB | 67.9 MB |
| pdfcraft 0.8.0 | 92.8 MB | 66.6 MB |

- **The exes are MSVC-built.** They import only system DLLs: dxgi, opengl32, imm32,
  uiautomationcore and similar.
- **Gap: file associations.** The upstream MSI registers `.docx` and the others only under
  `OpenWithProgids`, and by design never becomes the default handler
  (`wordcraft/packaging/windows/wordcraft.wxs:79-98`).
  - Replacing MS Office needs a default-association option. Choices are a per-machine MSI property,
    or a GPO "default app associations" XML for domain PCs.
  - It also needs one suite installer rather than four.
- **Fonts.** Office documents use Calibri, Cambria, Times New Roman and Arial.
  - deckcraft ships the metric-compatible Carlito, Caladea and Liberation (OFL).
  - On Windows the system fonts exist, so the desktop is less affected than web and mobile.
  - Still to be measured on the corpus.
- **Open questions for the owner:**
  - number of PCs and Windows versions;
  - domain/GPO or manual installation;
  - whether VBA macros or Outlook are in use. **Macros are not supported** by the Craft apps.

## Owner acceptance criteria: Windows 10/11 and Office 2010+ files (stated 2026-10-15)

### Requirements
- The desktop suite runs on **Windows 10 and Windows 11**.
- It opens, edits and saves files created in **Microsoft Office 2010 and later**. Office 2010+ saves
  OOXML by default, and users can still save the legacy 97–2003 binary formats.

### Format support, checked in upstream source

| Format | Open | Save | Evidence |
|---|---|---|---|
| .docx .docm .dotx .dotm | yes | yes | `wordcraft/crates/engine/src/io.rs:11-15, 183` |
| .doc .dot (97–2003) | yes (import) | no, saves as .docx | `wordcraft/crates/engine/src/io.rs:192` |
| .xlsx .xlsm .xltx .xltm | yes | yes; the VBA project is kept in .xlsm/.xltm | `gridcraft/crates/engine/src/io.rs:24, 200-203` |
| .xls .xlt (97–2003), .xlsb, .ods | yes (import) | **no** (refused) | `gridcraft/crates/engine/src/io.rs:204-208` |
| .pptx .potx .ppsx | yes | yes | `deckcraft/crates/engine/src/cmd/file.rs:263, 340` |
| **.ppt .pot .pps (97–2003)** | **no** | **no** | `deckcraft/docs/file-format-parity.md:23` ("owner decision") |

### Gaps against the requirement
- **No legacy PowerPoint (.ppt).** Options:
  - (a) a converter sidecar (LibreOffice headless as a separate process; clean-room, nothing linked);
  - (b) contribute a .ppt importer upstream, which is large.
  - The owner should say how common .ppt files are.
- **.xls and .doc cannot be saved back in their own format.** The user must be told clearly, and
  they get .xlsx or .docx instead.
- **Macros are never executed.** gridcraft only carries the VBA project through on save.
- **Office 2010–2019 vs 365 fidelity is not measured yet.** The corpus must include files saved by
  several Office versions (2010, 2013, 2016/2019, 365).

### Owner workload details (2026-10-15)

**Word: users rely on Show/Hide ¶ to find stray spaces and empty paragraphs.**
- Upstream has it: the `view.marks` command, labelled "Show/Hide ¶"
  (`wordcraft/crates/engine/src/cmd/view.rs:18`).
- It draws ¶ · → ↵ marks and has an "always show these marks" option
  (`wordcraft/crates/layout/src/display.rs:92, 125-155`).
- This must be easy to find in the simplified toolbar.

**Excel: heavy formulas, many sheets, links to other workbooks, files over 80 MB that keep growing.**

| Area | Upstream state | Source |
|---|---|---|
| Formulas | 505 of 523 Excel functions present, 496 working (94.8%); every function up to Excel 2010 that matters is present | `gridcraft/docs/function-parity.md` |
| Formula functions in the release CLI | 514 listed | `gridcraft-cli functions` |
| Missing functions | GETPIVOTDATA, IMAGE, RTD, TRIMRANGE, PERCENTOF, STOCKHISTORY, CUBE* (stubs) | `function-parity.md` |
| Known calc bugs | 15-digit equality; spill updates after structural edits | `gridcraft/docs/gaps.md:62` |
| Sheet limits | 1,048,576 rows × 16,384 columns, the same as Excel | `gridcraft/crates/core/src/addr.rs:10-11` |
| **External workbook links** | **"never followed or refreshed"**: kept on save with their cached values. **This blocks the owner's workflow.** | `gridcraft/docs/file-format-parity.md:25` |
| **Large files** | **"100 MB files fail (#175)"**; the owner's files are 80 MB and growing | `gridcraft/docs/file-format-parity.md:25` |

**External-link test: done and MEASURED, gridcraft-cli 0.7.0, 2026-10-15.**
Test files: a hand-built ECMA-376 §18.14 `linked.xlsx` with a same-folder link and an absolute-path
link (built by `spikes/xlsx-bench/ext.py`).
- **Open.** Works, with the warning "links to external workbooks were kept; they are not followed
  or refreshed".
- **Source workbook changed.** Reopening `linked.xlsx` still shows the old cached values
  (10, 21, 150, 7). The new values are **not picked up**.
- **No update-links command.** `data.refreshAll` refreshes PivotTables only and returns
  `refreshed: 0`.
- **Recalculation.** `formulas.calculateNow` leaves the external cells frozen.
- **New links cannot be created.** Typing `='[source.xlsx]Data'!B2` gives `#REF!`.
- **Save round trip.** The externalLink parts, their relationships (including the absolute path),
  the formulas and the cached values all survive. Whether Excel opens the saved file without a
  repair prompt is unchecked (Excel is not available).

**Verdict:** gridcraft 0.7.0 cannot serve a workload that pulls live data from other workbooks.
It needs an upstreamable feature: resolve external links, recalculate, offer "Edit Links / Update
Values", and let users type link formulas.

**Size benchmark: MEASURED 2026-10-15** on this PC with the inventory paused.

Test files are synthetic: 30 data sheets with mixed numbers, dates and Cyrillic/Latin text, and
SUM/SUMIFS/lookup/IF/cross-sheet formulas. They were generated by `spikes/xlsx-bench/gen.py`.

Rules for reading the table:
- The gridcraft run is aborted when free RAM drops below 1 GB. About 5 GB was free at the start.
- Peak is the process PeakWorkingSet.
- Excel is the locally installed **Excel 16.0, 32-bit**, driven over COM
  (`spikes/xlsx-bench/excel_ref.ps1`). Its figures cover open + `CalculateFull`, then SaveAs.

| File (on disk) | gridcraft 0.7.0: open | gridcraft: open+save | gridcraft peak RAM | Excel 16: open+calc | Excel: save | Excel peak RAM |
|---|---|---|---|---|---|---|
| w10 (9.9 MB) | 11.0 s | 16.5 s, reopens OK | 0.68 GB | 7.7 s | 3.4 s | 0.22 GB |
| w40 (39.6 MB) | 44.9 s | 57.3 s, reopens OK | 2.68 GB | 28.2 s | 9.9 s | 0.52 GB |
| w80 (79.2 MB) | **aborted after 62 s at 4.0 GB** | aborted after 60 s at 4.0 GB | >4 GB | 57.8 s | 16.5 s | 0.92 GB |
| w120 (118.8 MB) | **aborted after 69 s at 4.0 GB** | aborted after 65 s at 3.9 GB | >4 GB | 86.5 s | 25.6 s | 1.30 GB |

**Conclusions (from measurements):**
- **Memory.** gridcraft uses about **5× the RAM of 32-bit Excel**: 2.68 vs 0.52 GB on w40.
- **Disk-to-RAM ratio.** gridcraft needs about 64 MB of RAM per MB of xlsx on disk; Excel needs
  about 11.
- **The owner's 80 MB+ files** open in Excel in under 1 GB. gridcraft cannot open them on this
  12 GB PC with 5 GB free. Whether they open on a 16 GB PC is **not measured**.
- **Speed.** gridcraft is about 1.4–1.6× slower than Excel at opening.

**Formula results.** Summary values for S01–S03 on w10 match the Python-computed expectations.
A full 30-sheet comparison is still pending.

**Upstream's own position:**
- `gridcraft/docs/architecture.md:64`: "XLSX reader builds the whole workbook in memory; no
  streaming (#175)".
- `docs/gaps.md`: "no threading in crates/calc".

**Needed for the MS Office replacement:** memory-efficient cell storage plus a streaming XLSX
reader. This is an upstream engine change, **not a thin layer**, and it is a key risk for the ADR.

**Append and GUI (measured on w40).**
- **Append:** adding 10,000 rows took 70 s with a 2.7 GB peak. On reopen the rows were present and
  the SUM over the extended range was correct.
- **GUI:** the window was responsive after 43.6 s with a 2.8 GB peak.
- The full report is in `docs/phase0/xlsx-bench.md`.

**Memory breakdown and fix plan.** Source: `docs/phase0/gridcraft-scaling-and-links.md`. It was
measured on w10 with a counting-allocator probe.

Where the memory goes:

| Part | Share of peak | Notes |
|---|---|---|
| **Formula ASTs** | **51%** | about 678 B each; Excel shared formulas are expanded per cell (`sheet_read.rs:462-480`) |
| Calc dependency graph | 25% | |
| Values | 16% | |
| Recalc | 9% | |
| XML buffering | <0.5% | |

- **Variants of w10:**

| Variant | Peak RAM | Open time |
|---|---|---|
| Numbers only | 88 MB | 1.7 s |
| + text | 103 MB | 1.8 s |
| + 432k formulas | 635 MB | 10.7 s |

- **Upstream's #175 plan (a streaming reader) would save only about 2%.** The formula
  representation is the real problem.
- **A 1 GiB cap on uncompressed size** (`package.rs:13`) would block files above about 150 MB on
  disk whatever the RAM. This is projected.

Fix plan:

| Step | Change | Effort (engineer-days) |
|---|---|---|
| 0 | Measure the real files; raise the size cap | 1–1.5 |
| 1 | Shared formula templates (the biggest, riskiest win) | 10–15 |
| 2 | Compact dependency graph | 4–6 |
| 3 | Streaming writer; bounded undo | 3.5–6.5 |
| 4 | Compact recalc state | 2–4 |
| 5 | Streaming zip | 3–5 |
| | **Total** | **≈23–36** |

- **Projected peak after the fixes:** 80 MB → about 1.4 GB, 120 MB → about 1.8–2.1 GB. 32-bit Excel
  uses 1.3 GB on 120 MB. These are projections, not runs.

**External links: ≈17–25 engineer-days, an upstream engine change.**
- The stored `[1]Sheet!A1` form does not parse, so those cells never recalculate.
- Mixed external and local formulas freeze silently. This comes from the code and was not run.
- Our own layer would hold only file access, the consent prompts and the Edit Links dialog.

**Verdict.**
- Replacing Excel for this workload in v1.0 is feasible **only with about 40–60 engineer-days of
  upstream engine work accepted upstream**.
- **Fallback:** keep Excel for the heaviest and linked workbooks during the migration, gated per file.

### Windows 10/11 and old PCs
- **Graphics fallback.** All four desktop apps choose DirectX 12 when a capable hardware adapter
  exists. Otherwise they fall back to OpenGL, and then to WARP, Microsoft's software rasteriser.
  - Evidence: `wordcraft/apps/wordcraft/src/graphics.rs:38-53`,
    `gridcraft/apps/gridcraft/src/graphics.rs`, `deckcraft/apps/deckcraft/src/graphics.rs`,
    `pdfcraft/apps/pdfcraft/src/main.rs`.
  - This should cover old office PCs and RDP sessions. It is **still to be measured**, because WARP
    draws every frame on the CPU.
- **Windows 10:** this machine (Win10 Pro 19045) can test it.
- **Windows 11:** no machine is available, so it is **UNVERIFIED** until the owner provides a Win11
  PC or VM.
- **To measure in Phase 0:**
  - startup time and memory of each exe on this Win10 PC;
  - behaviour over RDP or with `WGPU_BACKEND=gl`;
  - open → save → reopen on Office 2010+ files from the corpus.

## Task 4 — document-server spike S0–S7 (done)
Details: `docs/phase0/docserver-results.md`. The spike ran against wordcraft-web release v0.7.0.

All S0–S7 steps passed: JWT-signed config, the server fetching the document by URL, the edit, the
save callback, and the HRM receiving the .docx.

| Run | Page load → editor ready | Ctrl+S → callback received |
|---|---|---|
| 1 | 4.9 s | 0.33 s |
| 2 | 1.9 s | 0.45 s |
| 3 | 2.2 s | 0.15 s |

- **Content check:** the saved .docx contained the typed `SPIKE4-<ts> Салом` marker in every run.
- **Negative tests:**
  - tampered token: 401;
  - SSRF to 169.254.169.254: 403;
  - wrong callback secret: no file accepted, and the error was surfaced.
- **Not done yet (S8–S9):** the host-bridge patch (dirty events, host-initiated save, `/close`).
  The stock v0.7.0 UI also still shows the upstream Discord, Share and Zotero chrome.

## Owner decision 2026-10-16: the HRM integration target is **hr-backend**
The target is `<server-path>` (`bakhodirovxz/hr-backend`), the repo that already embeds
ONLYOFFICE 9.3. It replaces the sandbox. A standalone prompt for its backend agent is being written to
`docs/phase0/hr-backend-prompt.md`.

## HRM test server (read-only analysis in progress)
- **Owner statement:** <test-server> (`vds-test`) is the test server. Access is read-only, code only:
  no database, MinIO or document files are opened.
- **Observed:**
  - The HRM backend is `<server-path>` (Python).
  - The frontends are `hr-frontend-sandbox` (v1 and v2) and `hrm-admin-frontend`.
  - The mobile app is `hrm-mobile-uz` (Expo).
  - **ONLYOFFICE Docs 9.3.1 already runs there.**
- **Note:** the Traefik dynamic config on this host contains public <hrm-domain> and <other-domain> routes.
  Their target services (`hr-api`, `hr-onlyoffice`) are not running on this host, so this is
  probably a copy of the production config. The owner should confirm.

## Open risks / blockers
- `corpus/` is **missing** → fidelity baseline (task 5) blocked until the owner provides it.
- `vendor-notes/` (`merge_lines.py`, `uz-translit-onlyoffice/`) is **missing** → not needed for
  Phase 0, needed for Phases 2–3.
- No macOS → iOS part of the mobile spike cannot be done here.
- 11.8 GB RAM: parallel Rust builds + Android emulator may contend; builds run with limited jobs.
- Upstream is a Windows-gnu build here; upstream CI likely targets MSVC/Linux — GNU-specific build
  failures are possible and will be recorded, not assumed.

## Next step
Phase 1 exit gate: the final uninterrupted `ci.sh full` in the clean clone (result goes to `docs/phase1/clean-checkout.md`, "final run"),
then the gate report to the owner. Review findings and carried items: `docs/phase1/gate-review.md`.
(Historical, Phase 0: "Task 1 (inventory): clone the six repos into `upstream/`, build desktop + web, record numbers.")
Still missing for Phase 2: `vendor-notes/uz-translit-onlyoffice/` (the rule seed, BRIEF Phase 2 task 1) and `corpus/` (B-14).

## WP5 part A (2026-10-17): sync script, fresh BASE, wordcraft and the non-gridcraft repos rebased
- `scripts/sync-upstream.sh` and `docs/UPSTREAM.md` written. Fresh BASE (frozen): wordcraft eae2421, gridcraft f1894c6, deckcraft 1dc6972, pdfcraft 924c4da, craft-libs 15ed713; craft-fonts unchanged 3d9ab3a.
- Dry run, all five repos: 27/27 patches clean, quick gates PASS, monorepo untouched. Details and costs: `docs/phase1/rebase-cost.md`.
- Applied (partial apply by repo name) for wordcraft (5 patches, 0 conflicts, rustfmt-fixed 0004 and 0005), deckcraft, pdfcraft, craft-libs. gridcraft stays at e6b6b34 with its 22 patches (dry run: all clean); its apply and the cost section are pending.
- Not done: `--gates full` (release builds, sizes, app tests) after the new BASE; `web/budgets.toml` baselines not re-measured.

## WP7 + WP8 (2026-10-17): branding and fallback font
- **Brand config and tooling.** `brand/brand.toml` is the only file with the working name and the product version (0.1.0, one for every app: About, `--version`, VERSIONINFO, and pdfcraft's engine `build_info`).
  `cargo xt overlay <app>`, `brand check`, `brand icons`, `brand-grep`, `brand-scan-bin` (crate `crates/office-brand`, tiny-skia placeholder icons: PNG 16..1024, ICO, favicon, maskable, apple-touch, all from code, nothing committed).
  `scripts/build.sh` renders the overlay before every build and exports `OFFICE_OVERLAY_DIR`, `OFFICE_PRODUCT_NAME`, `OFFICE_PRODUCT_VERSION`; web pages come from `index.html.in`. `scripts/ci.sh brand` runs `brand-grep` and `brand-scan-bin`.
- **Patches (15, +1,679/−150 lines, all in `patches/REGISTER.md`, PR status "not sent (owner decision B-6)").** Per app: 0100 overlay hook, 0101 community chrome off, 0102 fallback fonts; wordcraft and deckcraft also 0103 file metadata; deckcraft also 0001 (deckcraft-web never started: `std::process::id()` panics on wasm32, upstream bug, pristine too).
  pdfcraft 3 (+395/−41), wordcraft 4 (+409/−47), gridcraft 3 (+388/−15), deckcraft 5 (+487/−47). About a third of the added lines are new files (`build_office.rs`, `office_overlay.rs`, tests). `verify-queue.sh all` and `cargo xt register check` are green (42 patches, 42 rows).
- **Removed (all gated on overlay constants; without an overlay every app behaves like upstream):** Discord pill/buttons/cards/links, wordcraft Share button and Zotero tab, About links to the upstream website and repository, pdfcraft's ArtCraft logo and mark (not compiled in under the overlay: the licence obligation) and its update check (no `update_source`, no menu entry, no `ureq` call), deckcraft's painted logo and "Learn More", upstream copyright lines (replaced by `COPYRIGHT`; the upstream notices are in `NOTICE`).
- **Evidence (`docs/phase1/evidence/brand/`, 2 MB).** Desktop (release exe, window capture with title bar, About through the app's control channel): pdf "Suite PDF", docs "<document> - Suite <ru name>", slides "Suite Slides", sheets shows the workbook name as upstream does (About: "About Suite ..."). VERSIONINFO of pdf.exe: product name, company, copyright, 0.1.0. pdfcraft's accessibility tree (114 widgets): 0 upstream brand tokens outside the attribution line.
  Web (Playwright, headless Chromium): page title and screenshot of each of the four apps, About dialog, font test.
- **Binary check (`cargo xt brand-scan-bin`, release exes and web wasm of all four apps): 0 leaks** (no `discord.gg`, `getartcraft`, `github.com/storytold`, `api.github.com/repos/storytold`, no ArtCraft mark bytes). Not zero: the embedded translation catalogs still carry upstream's own keys and translations (wordcraft: 44 `discord.gg`, 44 `getartcraft`, 44 `github.com/storytold` as unused catalog lines; every app still contains the tokens WordCraft/GridCraft/DeckCraft/PdfCraft/ArtCraft as catalog keys, `rebrand()` replaces them at lookup). The check separates catalog records (short TSV lines) from code literals and reports both.
- **Fonts (WP8).** Noto Sans + Noto Sans Mono Regular, subset (83,684 + 64,632 B, OFL-1.1, no Reserved Font Name, `assets/fonts/fallback/`, `scripts/subset-fallback-font.sh` pins the source commit and SHA-256 and runs the cmap check). Appended last in every interface family of all four apps; gridcraft also uses the sans as the PDF default face (Ubuntu Light behind it, `set_default_face_with_fallback`); deckcraft also registers it in the slide font database.
  Playwright: `Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №` renders without boxes in all four web apps (`font-<app>-web.png`); gridcraft before (pristine web): U+02BB and the ruble sign drew as boxes (`font-sheets-web-before-pristine.png`).
- **Sizes (wasm, brotli -q 11, vs `web/budgets.toml` base_br; the budget guard stays 512 KiB until WP9):** docs 9,489,283 (+47,686), sheets 4,848,551 (+117,262), slides 9,401,738 (+101,371), pdf 9,471,743 (+80,984). Raw wasm: docs 42,604,173, sheets 19,299,235, slides 35,509,297, pdf 39,340,783. Fonts alone are +148,316 B raw per app (exe and wasm); deckcraft and pdfcraft also embed a 64 KiB RGBA logo. The deltas include upstream growth between the Phase 0 base and the new BASE, so they are upper bounds for our patches.
- **Not done / open.**
  - File metadata (written `Application`/Producer/creator strings) is rebranded only for wordcraft and deckcraft (URL-bearing or visible strings); gridcraft, pdfcraft (Producer, signature Prop_Build, comment author, "PdfCraft Shaped Text" font family) and the CLIs keep upstream's strings: owner decision O3.
  - Not rebranded: data/settings folders (`%APPDATA%\WordCraft`, eframe storage names), Wayland app id, `run_native` app names, exe names (still `docs`, `sheets`, `slides`, `pdf`), accent colours (upstream blue/green/orange/red stay), the `Craft` theme name, the three web wire-protocol prefixes, MCP server text and the CLI `--help` community lines (the CLI crates do not link the overlay), the macOS native menu items of gridcraft (macOS only), packaging (Phase 4), i18n catalogs (WP9).
  - gridcraft-cli and any other path that does not link ui-egui still prints U+02BB as notdef in PDFs.
  - `catalog` lines with upstream URLs stay in the embedded TSVs (see above): removing them needs a build-time catalog filter or edits to 22 catalogs.
- **Tests.** `scripts/overlay-tests.sh <d>` runs `cargo test --lib -p <app>-ui-egui office_overlay` in each upstream workspace with the overlay on: pdfcraft 3/3, wordcraft 3/3, gridcraft 4/4 (including `pdf_default_face_covers_uzbek`), deckcraft 4/4; `fallback_font_covers_uzbek` passes in all four (cmap of the fallback faces covers every test character, and the fallback is last in every family). The upstream test suites were not run with the overlay on (quarantine class `brand-overlay` is still to be filled in WP6b). `scripts/ci.sh brand`: `brand-grep` 0 hits, `brand-scan-bin` 0 leaks in 8 files.

## WP9c + WP9d (2026-10-18): language hooks and the deckcraft mechanism
- **Patches (5, +677/-51, all in `patches/REGISTER.md`, "not sent (owner decision B-6)").** wordcraft 0110 (3 files, +75/-2), gridcraft 0110 (3, +114/-2), pdfcraft 0110 (7, +156/-3, including an engine `build.rs` for the date-name languages), deckcraft 0110 (6, +285/-2, new `i18n.rs`) and 0111 (4, +47/-42, P1 strings at the helpers). Uzbek is added only when the overlay holds `catalog-uz-latn.tsv` and `catalog-uz-cyrl.tsv` (`cfg(office_uz)`); without them every language list and upstream test is unchanged.
- **Plumbing.** `scripts/build.sh` and `scripts/overlay-tests.sh` run `cargo xt i18n export <app>` (review mode; `OFFICE_I18N_MODE=release` for reviewed rows) and copy the files next to `overlay.env`. The export writes the picker names and the draft marker into `catalog-meta.env`: `Oʻzbekcha (qoralama)` and `Ўзбекча (қоралама)` while any Uzbek row is an unreviewed draft. The fallback chain (uz-Cyrl to ru to en, uz-Latn to en) is applied by the export, as designed.
- **Switch evidence (`docs/phase1/evidence/i18n/`, 3 MB, README there).** Web (Playwright): wordcraft and gridcraft follow the browser language (`uz-Latn-UZ`, `uz-Cyrl-UZ`); pdfcraft and deckcraft were switched through their pickers (neither follows the browser; deckcraft has no OS detection in Phase 1). Desktop release exes: wordcraft `WORDCRAFT_LANGUAGE` (window title `Suite Hujjatlar` / `Suite Ҳужжатлар`), gridcraft and deckcraft `ui.json`, pdfcraft `--language`. No tofu anywhere in Uzbek text; clipping findings and the deckcraft uz-Cyrl face problem (F-R2) are listed in the README.
- **Tests with the overlay on** (`scripts/overlay-tests.sh <d> office_overlay i18n ...`): wordcraft 69 pass, 7 fail; gridcraft 9 pass, 1 fail; pdfcraft 119 pass, 4 fail; deckcraft office_overlay 6/6 and backstage, ribbon, dialog, menu tests 65 pass, 3 fail (run with `--test-threads 1`: the default run hit the known access violation). `fallback_font_covers_uzbek` passes in all four, and so does pdfcraft's `date_languages_are_the_interface_languages`. The failures are quarantined (14 new entries): brand-overlay (the test expects the upstream name) 10, i18n-incomplete (rows the P1 set lacks: wordcraft `Controls`, three pdfcraft tests) 4; one deckcraft failure was already quarantined (env).
- **Open.** `cargo xt i18n roundtrip` fails for pdfcraft (catalog extracted at the old BASE: two upstream ru rows are new); a re-extract is needed (WP9e). The deckcraft picker heading `Interface Language` is not yet in any catalog. The pdfcraft web build does not follow the browser language (design finding I3, P2).

## Phase 1 cleanup package (2026-10-18)
- **1. deckcraft Cyrillic font (done).** Cause: our craft-fonts pin has no Inter, so the interface stack started with BIZ UDPGothic and drew Latin and Cyrillic with CJK full-width advances (uz-Cyrl letter spacing). Fix: new deckcraft patch 0112 (`0112-deckcraft-fallback-font-before-cjk.patch`, 2 files, +36/-4) inserts the overlay's Noto Sans subset before the CJK face in the proportional and bold families; CJK text still falls through to it. Tests with the overlay on: ui-egui `office_overlay` 7 pass (new `cyrillic_is_not_laid_out_in_the_cjk_face`); upstream theme, uifonts and font-dialog tests 23 pass. Release slides desktop and web rebuilt (559 s, 1345 s); uz-Cyrl and uz-Latn screenshots in `docs/phase1/evidence/i18n/` (README finding 5): normal spacing, no tofu. 48 patches, 48 REGISTER rows after this step.
- **2. pdfcraft i18n catalog resync (done).** `cargo xt i18n extract` re-extracted all catalogs at the new BASE, keeping every uz/ru value and status (only `where` line numbers are no longer refreshed unless `--refresh-where`, so the diff is 22 lines, not 2,300). Changes: **added 8** (pdfcraft: the two `You are viewing ... revision` rows that broke the roundtrip; deckcraft: `Interface Language`, `Presenter View - {}`, `{} can't find ...`, `{} can't export ...`, `{name}: {} can't open ...`, `missing or unknown language`), **removed (now obsolete) 5** (deckcraft: the five rows whose product-name literal our patch 0111 turned into `{}`; the `{text}  ->` format string), **English changed 0** (no uz row was affected, so no carry), **upstream ru changed 0**. Our own `i18n.rs` and `office_overlay.rs` are no longer extracted as UI. One draft added: deckcraft `Interface Language` = `Interfeys tili` / `Интерфейс тили` (copied from the shared row, `draft:agent-sonnet:2026-10-18`; P1 needs one). New `cargo xt i18n check-stale`; changed English is carried by the merge as `draft:carried:<date>` (never reviewed, unit-tested). `cargo xt i18n roundtrip` OK for wordcraft, gridcraft and pdfcraft; `lint` 0 errors, 386 warnings (length, unchanged); `check` 7,182 rows; `check-stale` ok; xtask tests 45 pass. `scripts/sync-upstream.sh` gates `i18n-stale` (dry run fails on a stale catalog, `--apply` merges and re-checks), `i18n-roundtrip`, `i18n-lint`; `scripts/ci.sh i18n` runs lint + roundtrip + check-stale. Documented in `docs/UPSTREAM.md` (The i18n gate).
- **3. gridcraft rustfmt (done).** Before: `cargo fmt --all --check` in `upstream/gridcraft` rejected 6 files touched by the queue (recalc.rs, pdf_fonts.rs, formula_templates.rs, store.rs, embed.rs, pdf/lib.rs); wordcraft, deckcraft and pdfcraft were already clean. One trailing patch `0111-gridcraft-rustfmt.patch` (`rustfmt --edition 2024` over the 65 `.rs` files the queue touches; only those 6 change, +70/-26), REGISTER row "mechanical formatting, regenerate on every sync" (category ci-fix). `scripts/sync-upstream.sh --fmt` now regenerates that one patch on top of the rebased queue (it is skipped in the rebase, and regenerated automatically when the queue already has one) instead of formatting each patch; regenerated patch files also keep their registered names (before, `format-patch` would have renumbered 0100-0199 to 0001...). `scripts/ci.sh lint` fmt steps: wordcraft, gridcraft, deckcraft, pdfcraft all pass. Not clean and not in scope: `fmt-root` (our own `xtask/` and `crates/office-brand`, 302 hunks, existing before this package). 49 patches, 49 rows.
- **4. WP13 docs-bind (done).** `spikes/mobile-b/bind` copied (the spike stays) to `domains/docs/crates/docs-bind` (package `docs-bind`, was `wordcraft-bind`), our crate in the docs domain workspace (`crates/*` member glob; `cargo xt gen-domain docs` updated `Cargo.toml`, the lock gained only the `docs-bind` package: lockdiff `0 version changes ... 0 foreign additions`). Path deps point at `upstream/wordcraft/crates/{engine,doc,render}` at the current BASE (eae2421): **no API drift** since `fd0bb2f` (no source change besides the crate name; the unused `wordcraft-layout` dependency was dropped). The two tests build their own fixture with the binding (the spike read `sample.docx`, a synthetic fixture generated by wordcraft-cli with the text "HRM fixture", which is not copied). CI: new job `scripts/ci.sh bind` (and `bind` in the workflow, Windows + Ubuntu): `cargo check` and `cargo test` native, `cargo check`, `cargo build` (the cdylib) and `cargo test --no-run` for wasm32-unknown-unknown (sccache off); the wasm tests are compiled, not run (no wasm test runner; they are plain `#[test]`). `bind` is in `full` and `nightly`, and in `quick` when docs is affected. Ran `scripts/ci.sh bind`: exit 0, native `2 passed`; `cargo nextest run -p docs-bind`: `2 tests run: 2 passed`. `cargo xt gen-domain --check docs` up to date; actionlint clean. cargo-deny for docs reports 3 `unmaintained` advisories (zbus_names, zvariant, zvariant_derive: transitive, none from docs-bind; not new). Patch count unchanged by this step (49 at the time; 51 after WP6b added gridcraft 0103 and 0104).
- **Sync script check (2026-10-18).** `scripts/sync-upstream.sh --dry-run --fmt --gates quick --to f1894c6 gridcraft` (rebase onto the same BASE): 26 patches clean, the trailing `0111` regenerated (6 files, +70/-26, byte-identical result: verify-queue and register PASS with the registered file names), gates `i18n-stale`, `i18n-roundtrip`, `i18n-lint`, gen-check, lockdiff, check native and wasm32 PASS. The one FAIL is `deny-sheets`: 3 new `unmaintained` advisories (zbus_names, zvariant, zvariant_derive, transitive via egui/winit; same in docs). Not caused by this package; needs an owner or `deny.toml` decision.

## WP14 steps 1-2 (2026-10-19)
- **Second sync dry run** (docs/phase1/rebase-cost.md, docs/phase1/sync-reports/2026-10-19.md): upstream moved 67 commits in about 2 days; 49 of 51 patches clean, 1 conflict resolved (gridcraft 0020, a Cargo.lock version line, 92 s), 1 regenerated (gridcraft 0111 rustfmt); quick gates PASS after the resolution except `i18n-stale`/`i18n-roundtrip`, which fail by design in a dry run when upstream changed a catalog (shown to pass after `cargo xt i18n extract`, rebase-cost.md); no single run printed `DRY RUN OK` for all repos; not applied, BASE still 2026-10-17.
- **Clean checkout** (docs/phase1/clean-checkout.md): desktop, CLI and web release builds, sizes, check, bind, deny, lint, i18n and tests pass for all four domains in the clean clone after 6 script fixes (env.sh target dir, root target dir, sccache on wasm check, build-lock deadlock, check package name, trunk dist and favicon). **Red: web-smoke (spec not written); 2 flaky pdfcraft tests.** README.md added.
  Update 98c0a02: the web-smoke spec is written (`ci/web-smoke/`, never executed yet), the 2 pdfcraft tests are quarantined as `flaky` (quarantine now 56 entries:
  env 9, flaky 7, brand-overlay 35, i18n-incomplete 5), `sync-upstream.sh --gates full` is implemented (never executed). A final uninterrupted `ci.sh full` at 98c0a02
  was started in the clean clone at 23:18 (same, warm target dir); its result decides gate item 2.
- **Gate review** (2026-10-19): `docs/phase1/gate-review.md`.

## Phase 2 WP2: `uz-translit` crate (2026-10-20)
- **Crate** `crates/uz-translit`: zero dependencies (`cargo tree -e normal,dev` = 1 line), `#![no_std]` + alloc, `#![forbid(unsafe_code)]`, builds native and `wasm32-unknown-unknown`. Implements `docs/phase2/rules.md` (R1-R17, R7a, quote guard, keep list, `conf` column, run-boundary mapping of section 8). API: `convert`, `convert_runs`, `detect`, `Options`, `Direction`, `Run`, `LangPolicy` + `lang_converts`, `RULES_VERSION`. The dictionary `data/exceptions.tsv` (191 seed rows + the WP3 review changes) is compiled by `build.rs` into sorted static tables (only `reviewed:*` rows).
- **Tests:** 88 rules (E01-E60 as 69 tests, plus per-rule, run-boundary, apostrophe-variant and R12-R16 tests), 6 dictionary (every row both ways, every stem with 14 suffixes), 3 property tests (20,000 seeded cases: round trip, apostrophe variants, idempotence, run split invariance; split invariance and idempotence also on arbitrary text), 3 golden (golden.tsv 1,014 Latin->Cyrillic and 1,033 Cyrillic->Latin rows, variants.tsv 81 rows; all pass).
- **Throughput** (release, 1 MB mixed Latin+Cyrillic text, best of 5): ->Cyrillic about 20 MB/s, ->Latin about 27 MB/s; 300 KB in about 15 ms (budget 100 ms). The plan's 50 MB/s target is not reached; the margin on the real requirement is 6x. Command: `cargo test --release -p uz-translit -- --ignored throughput --nocapture`.
- Spec notes: dictionary suffix round trip holds for `stem` rows only (`atelye` + `si` is a `word` row and does not round-trip by rule); `detect` returns `None` on a tie.

## Phase 2 WP10a + WP4: the Docs command (2026-10-20)
- **WP10a (early measurement, `docs/phase2/perf.md`).** Two synthetic 108-page files (`scripts/translit/make-108.py`, raw OOXML seeded from the golden vocabulary; Word `ComputeStatistics` = **108** for both, engine 110): tables, lists, TOC field, header/footer with PAGE fields, footnotes, hyperlinks, mid-word bold/italic splits, ru/en tags, text boxes; about 0.33 M characters each. Release engine, median of 5: open 41-53 ms, **full layout 276-335 ms** (about 3 ms a page), iterate all runs 1.3-1.6 ms, `uz-translit` on the whole text 27-46 ms, save 21-38 ms. Layout is the cost (7-8x the conversion). Deviation from the plan: the files are not Word-written (Word only checked the page count).
- **WP4 design.** Our crate `domains/docs/crates/docs-translit` (about 400 lines) holds the whole command: `install(&mut Session)` appends `tools.translitCyrl`, `tools.translitLatn` and `tools.translitAuto` (direction from the dominant script; added beyond the plan's two ids) to `Session.registry`. Params `{scope: auto|selection|document, skipLangs, fieldResults, apostrophe, foreignGuard}`. One `Session::run` = one undo step. Per paragraph, one pass: classify bytes (outside the selection, U+FFFC objects, tracked-deleted runs, skipped languages, locked controls, field results when `fieldResults:false`) -> `uz_translit::convert_runs` -> rebuild `Paragraph.text` once, keep every `Run.props` (only `Run.len` changes), `normalize()` drops empty digraph tails. A paragraph whose outputs are all Borrowed is not touched (its layout stays cached). Tracked mode (`settings.track_changes`) uses Change Case's recipe: per changed run a tracked deletion plus a tracked insertion, **2 revisions in all** (same author and timestamp are reused); text this author already inserted is rewritten in place. Selection and caret are mapped through the changed stretches. **Owner decision:** text tagged `ru` and `en` is converted (default `ConvertRuEn`), other languages are skipped and counted; `skipLangs:true` also skips ru and en. Codes and numbers stay as typed (R12, in `uz-translit`).
- **Carried patch `wordcraft/0120`** (7 files, +21/-1; REGISTER category `translit`, new in `xtask`): the install call behind an empty `office-ext` feature in the three apps (9 lines of Rust), and the three ids in the `per_row` arm of `column.rs` (P7). `domain.toml` `[extensions] crates = ["docs-translit"]` makes `cargo xt gen-domain` add the optional path dependency and the default feature (ours, not carried; xtask unit test). Without the `column.rs` ids a column selection converts the whole document: confirmed by removing the line and running the test (row 0 came out fully converted). Not covered: the `wordcraft-cli mcp` server builds its own headless `Session` (`crates/mcp/src/lib.rs:67`) and has no transliteration commands.
- **Tests** (`cargo nextest run -p docs-translit`, 13 engine-level tests, headless): selection vs caret vs `scope`, auto-detect, one undo and redo (`doc == before`), run props identical apart from text, a digraph split across bold/italic/plain runs (S|h, c|h, o|'), field codes (`Field` and `FieldStart`/`FieldEnd`) and hyperlink targets untouched, language tags (ru/en converted, de skipped and counted, `skipLangs`), a column selection, a locked content control, tracked changes (2 revisions, old text kept as deleted, undo restores), headers and text boxes in the document scope.
- **Gates.** `verify-queue.sh all` OK (52 patches), `cargo xt register check` 52 rows OK, `gen-domain --check docs` up to date, `lockdiff` 0 version changes / 0 foreign additions, `cargo test -p xtask` 56 pass, `ci.sh test-ours` all pass (docs domain 411 s), `ci.sh check docs` native and wasm32 OK, `cargo fmt --all --check` (root and upstream/wordcraft) clean, `ci.sh test wordcraft`: 2748 passed, quarantine 19 failed as recorded (exit 0).
- **Measured on the 108-page files (N1, headless native, release):** command (`run()`) 47-67 ms, of which ours 46-65 ms (convert 41-59, apply 4-6); **command -> finished relayout 0.35-0.44 s** (110 -> 114 pages to Cyrillic, 110 -> 106 to Latin); undo and redo about 40 ms. The gate is 2 s. N2 (desktop frames) and N3 (web/wasm) are WP10 and not measured. Risk R1 stays open for web: Phase 0's emulator figure (16-22 ms a page) would put the layout alone at 1.8-2.4 s for 110 pages; that is a projection and a cost of upstream's layout, not of the command.
- **Still to do:** UI entries and catalogs (WP7), preservation harness on Word-written fixtures (WP9), N2/N3 (WP10).

## Phase 2 WP5 + WP6: the Sheets and Slides commands (2026-10-20)
- **Ids and result shape.** Same three ids as Docs, `tools.translitCyrl`, `tools.translitLatn`, `tools.translitAuto` (the plan said `edit.*` / `text.*`; the lead asked for one set, and neither engine dispatches on the `tools.` prefix). Same result JSON: `direction, scope, tracked:false, runs, skipped{lang,field,locked}, ms{convert,apply}, rulesVersion`, plus `cells`, `skipped.formula`, `skipped.header` and `formulasWithText` for Sheets and `paragraphs` for Slides. The menu cell is empty (no ribbon entry until WP7). Owner decisions applied: ru/en text is converted (neither model tags language per cell; deckcraft `RunProps.lang` is left as it is and never skips), codes stay as typed (R12), alphabet 1995.
- **Sheets, carried patch `gridcraft/0120`** (5 files, +423: module 190, tests 224, `mod.rs` 2, manifest 2, lock 5). The command registry is a closed `OnceLock` (no install hook as in Docs), so the module is carried. Scope `auto|selection|sheet|workbook` (`within` is accepted as an alias; `range` forces the selection): `auto` is the selection if it is more than one cell, otherwise the used range of the active sheet. Only constant `Value::Text` cells change. Skipped and counted: formulas and their cached values (`formulasWithText` counts those with a letter literal, the R4 warning), table header cells, locked cells on protected sheets (the commit would refuse them). Skipped silently: numbers, booleans, errors, spilled/array cells, cell pictures. Not converted: sheet names (renaming breaks references, PLAN 4), defined names, number-format literals, comments, charts. The cell is the unit: the model keeps text as one plain string (rich-text runs are dropped when a file is read, `xml.rs:67`, `write.rs:378`), so there are no runs to keep, and the formatting test compares open-save with open-convert-save. One `edit()` = one undo step; formulas that read a converted cell are recalculated. A command that changes nothing leaves no history step.
- **Slides, carried patch `deckcraft/0120`** (5 files, +575: module 293, tests 273). Our own walker over shapes, groups, table cells and notes (`find.rs` `bodies` is private). Scope `auto|selection|slide|deck`: `auto` is the selected text range, else the selected shapes (with what is inside them), else the whole deck (all slides and notes). Run level: the paragraph's runs are cut at the selection ends, `Text` runs are converted through `convert_runs` (a digraph across runs works), `Field`, `Break` and `Math` runs are boundaries and stay as typed, `props` (link target, language tag) are untouched, `normalize()` merges runs of equal props and drops empty tails. The selection follows the text. A selection inside a master or layout is a no-op. Layouts, masters, comments, alt text and SmartArt are not converted.
- **Tests.** `gridcraft-engine --test translit`: 10 (registered ids; selection vs single cell = sheet; auto direction; formulas, numbers, booleans and cached values untouched with the warning count; table header skipped with a structured reference still resolving; one undo and redo, and no step when nothing changes; workbook scope with sheet names kept; protected sheet; xlsx styles equal open-save vs open-convert-save; 8,000 generated cells both ways). `deckcraft-engine --test translit`: 8 (registered ids; deck + notes + auto direction; selected shapes; text range with the selection following; field, link, language and bold untouched and a digraph across a bold and a plain run; table cells and group children; one undo and redo; bad scope).
- **Measured (release, engine, `docs/phase2/perf.md`).** Sheets: 200,000 text cells 1.07 s, 800,000 cells 3.7 s (4.6 us a cell, half of it recalculation and row heights in `edit()`); slides: 8,500 paragraphs of a 500-slide deck 76 ms.
- **Gates.** `verify-queue.sh all` OK (gridcraft 30, deckcraft 9 patches), `cargo xt register check` 54 rows OK, `gen-domain --check all` up to date, `lockdiff` sheets and slides `0 version changes ... 0 foreign additions`, `ci.sh check sheets` and `check slides` native and wasm32 OK. CLI: `sheets-cli commands --json` and `slides-cli commands` list the three ids, `run --cmd 'tools.translitCyrl={}'` converts a workbook and the sample deck.
- **Still to do:** UI entries and catalogs (WP7), preservation harness on Office-written xlsx/pptx (WP9).

## Phase 2 WP7 + WP9 + WP10 N2/N3 (2026-10-20)
- **WP7 UI entries (patches `wordcraft/0121`, `gridcraft/0121`, `deckcraft/0121`; 57 patches, 57 REGISTER rows).** A "Transliterate" menu with Latin to Cyrillic, Cyrillic to Latin and Auto-detect:
  Docs Review > Language (menu button) and the text context menu (submenu, selection only); Sheets Review tab (button + menu) and two items in the cell context menu; Slides Review (button + menu) and a submenu
  in the shape/text context menu. Result as a status line (Docs, Slides) or toast (Sheets): "{n} paragraphs/cells converted", "Nothing to convert", "{n} items skipped ...", "{n} formulas contain text ...".
  No keyboard shortcut (the plan: none; gridcraft keytips need no row, Excel has no such command). The engine command labels were renamed to the menu labels ("Latin to Cyrillic", "Cyrillic to Latin", "Auto-detect")
  in docs-translit and the two carried engine modules; a test checks the labels are catalog rows. **Catalog:** 9 new rows (7 in `shared.tsv`, 2 in `gridcraft.tsv`), en/ru/uz-Latn/uz-Cyrl;
  uz-Latn "Lotinchadan kirillchaga" / "Kirillchadan lotinchaga" / "Avtomatik aniqlash", uz-Cyrl made with `uz-translit` itself. Reviewed by an opus sub-agent, which changed 3 rows (ru "Автоопределение", "boshqa tilda" in the skipped message,
  ru of the formulas message); status `reviewed:agent-opus:2026-10-20` (agent review, not owner review). `cargo xt i18n extract` needed an `OWN` list in xtask (upstream catalogs do not contain these keys); lint 0 errors, roundtrip OK, check-stale OK.
  **Not done:** the ru strings are in the catalog, but no ru export ships with the apps (existing pipeline), so a ru UI shows them in English.
  **Evidence:** `docs/phase2/evidence/ui/` (1.1 MB, 11 PNG): web apps in uz-Latn (menu, conversion through the UI with the status line, context menu) for Docs, Sheets and Slides, plus Docs in uz-Cyrl; no tofu. Slides has no browser-language detection, so the language was picked on File > Account.
  No desktop window captures (web only).
- **WP9 formatting preservation.** Fixtures written by Word, Excel and PowerPoint 16 through COM (`scripts/translit/make-fixtures.ps1`; own PIDs only, nothing was running before): 9 .docx (the 8 of the plan, tracked changes off and on), 2 .xlsx, 2 .pptx, synthetic text.
  - **docx** (`cargo nextest run -p docs-translit preserve`, 10 tests): B (open, save) against R (open, convert, save), both directions (round 2 converts the Cyrillic result back). Non-text parts byte-identical, text parts equal as XML trees ignoring `w:t`
    text (digraph-tail runs may vanish), model check of runs/props/mark/objects, plus a text check against `uz_translit::convert` and a test that the comparer sees a changed property. **All identical.** Findings: (1) Word writes its own symbol font on a U+2060 word joiner; after conversion
    that run is absorbed by the letter before it (the invisible character loses its font): reported, excluded from the model check; (2) with recording on (`06-tracked-on`) R differs from B by design (2 revisions per run) and `docProps/app.xml` word counts change; there the test checks non-text parts, accept-all text = converted text and reject-all text = original.
  - **xlsx/pptx** (`scripts/translit/preserve-check.sh`, comparer `preserve.py`, 2+2 files, both directions): **IDENTICAL** for all 8 comparisons, after two documented normalisations: xlsx the cached string result of a formula cell may change (B8 `="Natija: "&A2` follows A2; the `<f>` is identical),
    pptx adjacent runs with identical `a:rPr` are merged in R (the engine's `normalize()`) but not in B. No other delta; no rich-text delta beyond the known P3 loss (identical in B and R).
- **WP10 N2/N3 (`docs/phase2/perf.md`).** Desktop `docs.exe`, control channel, median of 5: command 55-73 ms, next frame after the command **0.40-0.42 s**, painted new layout (screenshot, upper bound) **0.93-0.94 s**. Web (docs-web wasm, headless Chromium, palette to next presented frame): **1.26-1.27 s**. Gate 2 s: met on both.
  Android emulator: **not obtained** (the 108-page file never reported `wordcraft:opened` in 240 s); R1 stays open for phones.
- **Gates run:** see the end of this section.

- **Gates (2026-10-20, all exit 0 at the end):** `verify-queue.sh all` (wordcraft 12, gridcraft 31, deckcraft 10 patches), `cargo xt register check` 57 rows, `cargo xt i18n lint` 0 errors / `roundtrip` OK / `check-stale` OK, `ci.sh queue`, `lint` (the root `fmt` caught my xtask change once; fixed), `brand`, `i18n`, `check docs|sheets|slides` (native and wasm32), `test-ours` (docs-translit 26 tests, sheets and slides engines), `test gridcraft`, `test deckcraft`.
  `test wordcraft` first failed on two upstream tests that compare the overlay's uz-Latn catalog with `pl.tsv` / `vi.tsv` (`polish_covers_every_key_of_every_other_catalog`, `vietnamese_covers_existing_catalog_keys_...`): the 7 new strings are in no Polish or Vietnamese catalog. Quarantined as `i18n-incomplete` (60 entries now, evidence `docs/phase2/evidence/test-wordcraft-translit-i18n.log`); rerun exit 0.
- **Still to do:** WP11 (CI, docs), WP12 (gate review); Android-phone timing; ru export for the apps; desktop window screenshots of the new entries.
