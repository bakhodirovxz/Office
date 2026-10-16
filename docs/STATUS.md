# Status

**Current phase:** 0 — Evidence and architecture decision (branch `phase-0`)
**Last updated:** 2026-10-16

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

## Security notes carried into Phase 5 (from the spike review)
- **Session routes rely on a capability URL.** `/api/session/{sid}/{document,save,autosave,close}`
  are authorised only by the UUIDv4 session id. Phase 5 needs:
  - a per-request session token, bound to the user and the permissions (JWT on every call, as the
    brief requires);
  - session ids kept out of logs.
- **Read-only sessions could save. Fixed in the spike** (`writable()` in
  `spikes/docserver/src/main.rs`): save and autosave now return 403 for view-mode sessions or
  `permissions.edit == false`.

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
Task 1 (inventory): clone the six repos into `upstream/`, build desktop + web, record numbers.
