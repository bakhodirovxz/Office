# Status

**Current phase:** 4 — Unified shell (branch `phase-4`, plan `docs/phase4/PLAN.md`). **Phase 3 closed on 2026-10-22** (owner "go"): `ci.sh full` 28/28, the silent-loss ratchet 0 new, gate review and close-out in `docs/phase3/gate-review.md` (§10 carry-forward list). O8 not answered: the code default stays the native PDF → DOCX engine with the D5 fallback.

**WP12 full-CI reds, fixes (2026-11-22).** (1) `sizes` Slides: growth since the WP13 budget is the phase-5 Tinos serif face (deckcraft/0186, +40,015 B brotli; raw data section +73,924 B = the 74,024 B font, code section -3 KB) plus the merged phase-4..6 fixes; the `Navigator` web-sys feature adds no bytes. No cut without a fidelity loss (narrowing the Tinos subset to drop Latin Extended-B and the non-Central-Asian Cyrillic extensions would save about 15 KB brotli; those letters would draw in the sans fallback: owner option). `delta_br_max` 311,296 -> 360,448 by the rule (delta +321,064); `ci.sh sizes` on the phase-7 dist (E:/office-target/phase-7/dist, 9,647,196 B) ok, slides headroom 40,861 B. phase-5 was over too (9,558,929 B, by 1,944 B): raised there to 262,144 on `p5-langtest` (keep phase-7's own value when merging forward). (2) `test wordcraft` 0163 test: premise wrong since it was added (`Document::from_text` is en-US through `StyleSheet::builtin()`), fixed by wordcraft/0189 on `p5-langtest`. (3) `deny`: rustix 1.1.6 (pdfcraft/0910 lock patch + domains/pdf lock) and the chrono RUSTSEC-2026-0543 ignore (slides, pdf) on `p4-deny`; the zip 3.0.0 advisories RUSTSEC-2026-0545/0548 are still red pending an owner decision (neither is reachable: no `aes-crypto` feature, no `ZipStreamReader`; zip 9 is an API break in three engines).

**Phase 7 WP12 full CI, 2026-11-21 (branch p7-ci2 from phase-7 00203229; logs E:/office-target/tmp/p7-full-*.log, runs under E:/office-target/phase-7/ci/p7full*).** First pass on the phase-7 head: queue ok 95 s, lint ok 1,106 s, brand ok, i18n ok. Red and fixed on p7-ci2: gen-check (slides-web Cargo.toml drifted from gen-domain: the Navigator web-sys feature; plugin-commands failed because docs-office lost `suite-core` from [dependencies] in the phase-6 merge c633cd8f, so every native docs build and test failed), deny (root: wasmtime 48.0.8 had 8 advisories RUSTSEC-2026-0566..0573, fixed by plugin-host `=48.0.9` and the root and domain locks), convert (no_hrm_names: three comments in services/docserver/src named the reference server; reworded), docs-translit tests rustfmt (separate commit). Second pass from p7-ci2 (OFFICE_TARGET phase-7, fonts fetched): gen-check ok 857 s, notices ok 475 s, test-ours ok 3,923 s, test gridcraft ok, test deckcraft ok, check x4 ok, bind ok 714 s, convert ok 419 s, convert-e2e ok 1,621 s, shell ok 797 s, docserver ok 3,123 s (the first pass failed 3 Playwright cases under load: plugin wrapper x2, xlsx 300 ms close; green when the machine was idle), plugins ok 1,919 s on wasmtime 48.0.9, build x4 ok, translit-npm ok, translit-preserve ok, brand ok, web-smoke ok, actionlint ok. Docker drop-in rehearsal (build.sh, rehearse-dropin.sh, both scenarios): 67 passed, 0 failed. STILL RED, not fixed here: (1) `ci.sh sizes`: Slides wasm brotli 9,648,673 B, delta 321,064 > delta_br_max 311,296 (over by 9,768 B; no budget raised, needs an owner/maintainer decision); (2) `test wordcraft`: ribbon::tests::status_bar_language_of_an_untagged_document_follows_an_uzbek_interface (patch 0163) fails at ribbon.rs:3351 `caret_lang_tag(&app).is_none()` because StyleSheet::builtin() sets default_chr.lang = en-US, root cause not settled; (3) `deny` in the domains: zip 3.0.0 (RUSTSEC-2026-0545/0548, upstream wordcraft-docx, fix is zip >= 9), chrono 0.4.45 unmaintained (RUSTSEC-2026-0543, no fix, slides and pdf), rustix 1.1.5 xattr unsound (RUSTSEC-2026-0486, pdf lock, fix 1.1.6); (4) pdfcraft `drag_selects_text_and_copy_returns_it` failed once under load (1.9 s, `text layer loaded`), 3 of 3 green alone (0.06 s). `release-tests` is nightly only and was not run.
**2026-11-22: Cyrillic to Latin with nothing selected (Docs, Sheets, Slides) now upgrades 1995 Latin too, as in the add-in; a paragraph/cell that looks English (`uz-translit` `upgrade_skip_english`, cherry-picked from `office-addin` def1acaa) keeps its Latin. A real selection is unchanged. Patches gridcraft/0189, deckcraft/0189; Docs in `docs-translit`.**
**Phase 7 WP12 full-CI reds that start on phase-5 (2026-11-22, branch `p5-langtest` from phase-5 36fa69aa).** (1) `test wordcraft`: `ribbon::tests::status_bar_language_of_an_untagged_document_follows_an_uzbek_interface` (0163) failed at ribbon.rs:3351 because its "untagged" document came from `Document::from_text`, whose `StyleSheet::builtin()` has set `default_chr.lang = en-US` since the pristine snapshot (styles.rs:475); the test is `cfg(office_uz)` and the WP12 run was its first with the overlay on. A .docx with no `w:lang` reads into `StyleSheet::empty()` (docx read/mod.rs:793), so the product behaviour was right and the test premise wrong: **wordcraft/0189** clears the default language in the test (unfixed: fails on phase-5 and phase-7; fixed: passes with the overlay). (2) `sizes` Slides: phase-5 itself was over budget: release build of 36fa69aa with craft fonts 9,558,929 B brotli, delta +231,320 > 229,376 (by 1,944 B). Cause: the Tinos serif face (deckcraft/0186, +40,015 B, left 4,607 B headroom on 2026-11-10) plus the merged-forward fixes since. No cheap cut without a fidelity loss (Tinos is already unhinted, no glyph names; dropping Latin Extended-B and the non-Central-Asian Cyrillic extensions from the Tinos subset would save about 15 KB brotli but those letters would draw in the sans fallback), so `delta_br_max` 229,376 -> 262,144 by the rule (ceil(1.10 x delta / 16 KiB) x 16 KiB); `ci.sh sizes` slides headroom 30,824 B.

**Owner MSI test feedback (2026-11-06), Docs pointer "stays white, hard to find": NOT reproduced, no fix.** Code review: the page canvas requests `CursorIcon::Text` over text and `Default` only in the selection bar (`upstream/wordcraft/crates/ui-egui/src/canvas.rs:1743-1753`; headless kittest hover over text returns `Text`, so the I-beam is requested); no custom cursor images anywhere in our layer or wordcraft; egui-winit maps to the standard OS cursors. Owner scheme is "Windows Aero" with the default (inverting) I-beam (`HKCUControl PanelCursors`). Live capture was impossible (workstation was locked). Needs a real screenshot or the owner's cursor scheme/size to continue.

**Author of HRM blank files, 2026-11-10 (branch p5-author).** HRM makes blanks with python-docx/openpyxl/python-pptx, so File > Info showed the generator as author. The docserver now rewrites only `docProps/core.xml` of a stored version (every other zip entry raw-copied) in edit sessions: `dc:creator` becomes the session user (`editorConfig.user.name`, capped 255, escaped) when empty or a generator name (real names kept), and a save or promoted autosave also sets `cp:lastModifiedBy`. One point: `put_version` and the autosave promotion in sessions.rs, before hashing; the promotion compares the stamped form, so no spurious version. Engines round-trip creator (wordcraft, gridcraft, deckcraft read and write it); gridcraft does not write `lastModifiedBy`, the server re-adds it at save. Tests: services/docserver/tests/author.rs. Limit: the first editor, not the HRM creator, becomes the author of an untouched blank.

**Docs in the document-server iframe, 2026-11-09 (branch p5-hdocs, HRM test server screenshots).** (1) Full ribbon on the web: `suite_core::settings::toolbar_mode()` on wasm32 = the saved choice (`localStorage` `suite.toolbar`) > URL `?toolbar=full|simple` > simple (`resolve_toolbar`, unit tested); docs-office installs the store and offers the toggle on the web too. (2) `?lang=` already won over `navigator.language` through the docs `index.html` shim; verified in Chromium with ru, uz-Cyrl, uz-Latn against a browser locale of uz-UZ, en-US, ru-RU. (3) Autosave (wordcraft/0160): 3 s after the last edit, at least every 30 s of continuous editing, never twice within 3 s; `bridge-docs.spec.ts` 13/13 (cases 5 and 5b). (4) wordcraft/0161: a host-opened document starts at 100 % (Page Width only when the page is wider than the view). (5) wordcraft/0162: Uzbek (Latin, Cyrillic) and Russian text under an en-US tag is not flagged as English (proof::uz, the Vietnamese rule extended); wordcraft/0163: the status bar names the interface language for a document with no language in an Uzbek interface. Docs wasm brotli 9,860,812 B, budget 9,871,648 B, headroom 10,836 B (suite-core on the web cost +1.4 KB); js 20,768/24,576.

**Phase 4 WP0 (upstream sync) done 2026-10-22** (`docs/phase4/sync-report.md`): wordcraft v0.9.0 `2e6db0b`, gridcraft v0.10.0 `2ebdf16`, deckcraft v0.9.0 `fe510bd`, pdfcraft v0.11.0 `3e30645`, craft-libs `1b9cf19` (craft-fonts unchanged). 73 patches: 53 clean, 5 3-way, 13 hand-merged, 1 amended (gridcraft 0130), wordcraft 0003 dropped (upstream does it), pdfcraft 0111 new (D7). NumLocale re-threaded through upstream value_text_as; tests and the WP14 matrix unchanged (byte-identical). 68 new i18n rows with uz drafts for review. Gates: verify-queue, register, ci quick, release builds x4, convert-e2e (43/0), translit-preserve, sizes (base_br re-measured, our delta inside every delta_br_max) green. Open: wordcraft/gridcraft origin/HEAD are 562/605 commits further (next sync); the sync gate cannot pass new P1 strings without drafts.

**Previous:** 3 — Converter (branch `phase-3`). **Gate close-out 2026-10-22: ready for the owner's "go"** (`docs/phase3/gate-review.md` section 10). **Phase 2 closed on 2026-10-21** (owner "go"): clean-checkout `ci.sh full` 24/24 green (`docs/phase2/clean-checkout.md`), follow-ups D3/D2/m4 done, M3 carried to Phase 4 (`docs/phase2/gate-review.md`).
Phase 3 owner decisions: this PC only (WP11 deferred), no .ppt, limits accepted, accounts optional (local mode with no login is the default, server mode optional with an operator-configured token issuer), NumLocale in scope (WP14).

**Owner requirement, 2026-10-21: GitHub-like version history and authorship, independent of the HRM.** The brief's
version history (Phase 5) relies on the HRM's `/versions` API. Accounts are optional, so we also need our own store.
- **Version store.** Built into our docserver and content-addressed, so identical blobs are stored once. Every save
  becomes a version holding: author (the token `sub`, or the OS user in local mode), time, an optional message,
  parent, size and hash. The history is append-only and hash-chained, so it cannot be edited quietly.
- **Who changed what.**
  - Docs: a version diff at paragraph and run level, with the author per changed paragraph (a "blame" view), built on
    the tracked-changes model wordcraft already has (`w:ins`/`w:del` with authors).
  - Sheets: a cell-level diff showing changed and added cells, sheets and formulas.
  - Slides: a per-slide diff. PDF: a page diff.
- **Actions.** List versions, view one, compare any two, restore (a new version, never a rewrite), download an old
  version, and name a version (like a tag).
- **Backends.**
  - Our own store is the default.
  - When the HRM is configured, its `/versions` is used instead, through the same interface.
  - Local mode, optional: snapshots kept under app data, the same as the store with one user.
- **Not in scope:** git branches or merges of documents. Real-time co-editing (Phase 9) adds per-change attribution
  on top.
- **Phases:**
  - Phase 4 (shell): the History panel UI, local snapshots.
  - Phase 5: the server store, diff engines, restore, HRM backend.
  - Phase 9: live attribution.
- **Rough size:** about +8–12 stream-days over SIZING 5.3. To be sized in the Phase 4/5 plans.

**Previous:** 2 — Transliteration (branch `phase-2`). **Phase 1 closed on 2026-10-20**: the final clean-checkout `ci.sh full` exited 0 (`docs/phase1/clean-checkout.md`) and the gate review is in `docs/phase1/gate-review.md`.

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

**Side effect:** one extra Back press left Chrome and landed in an Expo Go "<org> HRM" login screen
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
The target is `<server-path>` (`<hrm-backend-repo>`), the repo that already embeds
ONLYOFFICE 9.3. It replaces the sandbox. A standalone prompt for its backend agent is being written to
`docs/phase0/hr-backend-prompt.md`.

## HRM test server (read-only analysis in progress)
- **Owner statement:** <test-server> (`<test-host>`) is the test server. Access is read-only, code only:
  no database, MinIO or document files are opened.
- **Observed:**
  - The HRM backend is `<server-path>` (Python).
  - The frontends are `<hrm-app>` (v1 and v2) and `<hrm-app>`.
  - The mobile app is `<hrm-app>` (Expo).
  - **ONLYOFFICE Docs 9.3.1 already runs there.**
[redacted: HRM security detail]
  Their target services (`backend-api`, `docs-alias`) are not running on this host, so this is
[redacted: HRM security detail]

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
- **Tests:** 88 rules (E01-E60 as 69 tests, plus per-rule, run-boundary, apostrophe-variant and R12-R16 tests), 6 dictionary (every row both ways, every stem with 14 suffixes), 3 property tests (20,000 seeded cases: round trip, apostrophe variants, idempotence, run split invariance; split invariance and idempotence also on arbitrary text), 3 golden (golden.tsv 1,014 Latin->Cyrillic and 1,033 Cyrillic->Latin rows, variants.tsv 81 rows; all pass). *At the gate review (2026-10-20, after the source check and the 2026 alphabet): golden.tsv 1,032 / 1,051 rows checked, `golden_2026` 952 rows, 4 golden and 4 property tests; `sentences.tsv` does not exist, so that test checks 0 rows (docs/phase2/gate-review.md).*
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
- **WP11 CI (2026-10-20, `docs/phase2/ci.md`).** New jobs `translit-preserve` (xlsx/pptx `preserve-check.sh`, builds the release CLIs) and `translit-perf` (nightly: `perf_108` in release, the test now asserts command to layout under 2000 ms; measured 344 / 303 ms); `quick` adds `test-ours`, `translit-npm`, `translit-preserve` when the diff touches the translit surface; `full` adds `translit-preserve`; workflow mirrored, actionlint clean, all `uses:` pinned. `cargo xt i18n export` now always writes `catalog-ru.tsv` (test added); **no app reads it yet**, so the ru UI still shows the 9 new labels in English (needs a 0110 follow-up in three apps; open). `ci.sh sizes` failed on docs (+146,051 B delta against 131,072): `delta_br_max` raised to 163,840 by the rule measured + 10 % (re-measure at WP12). Gates: quick, test-ours, translit-npm, test wordcraft/gridcraft/deckcraft, lint (in quick), sizes (after the budget), all exit 0.
- **Still to do:** WP12 report; Android-phone timing; desktop window screenshots of the new entries.
## WP12 part A (2026-10-21): Russian rows in the apps, gate-review fixes, clean-checkout CI
- **ru labels (carried patches `wordcraft/0111`, `gridcraft/0112`, `deckcraft/0113`; 62 patches, 62 REGISTER rows).** wordcraft: `build_office.rs` adds `CATALOG_RU` (the export: upstream's ru rows plus ours, in upstream's format) and the `ru` `LangInfo` embeds it instead of `ru.tsv` when present; the test reads every escape-free upstream ru row back unchanged (over 1,000) and checks our two labels. gridcraft: `translate_ru_extra()` is generated from the rows that `src/i18n/ru.rs` does not spell (size budget) and `Language::tr` asks upstream's table first, so upstream wins by construction. deckcraft has no upstream Russian: `ru` (Русский) is a new registry entry under `cfg(office_ru)` holding our 7 rows only; every other label stays English there (screenshot shows it). Tests with the overlay on: docs `russian_gets_our_rows_and_keeps_upstreams`, sheets same name, slides `russian_is_offered_only_with_the_overlay_catalog`: pass (the other failures in those binaries are the quarantined ones). Screenshots `docs/phase2/evidence/ui/{docs,sheets,slides}-web-ru-transliterate-menu.png` (headless Chromium, locale ru-RU; slides: language picked on File > Account); looked at: the three menus read "С латиницы на кириллицу / С кириллицы на латиницу / Автоопределение", the button "Транслитерация".
- **Gate-review fixes** (`docs/phase2/gate-review.md` section 6): M1 okina before a closing apostrophe, M2 `latinAlphabet` param in Docs (domain crate) and Sheets/Slides (`gridcraft/0122`, `deckcraft/0122`), D1 Sheets context menu converts the selection, D4 golden harness fails on missing files, m1 `sirk`/`sirka` stems, m5 npm `private`. `RULES_VERSION` is `uz-translit-rules/3`. D3 (`deckcraft/0123`), D2 (comparer tolerances and all stories; no bug found) and m4 (tracked deletions left out of the converter input) were closed afterwards; open: M3 only (Phase 4).
- **Clean-checkout `ci.sh full`: all 24 jobs green** after three config/data fixes; table, perf (command to layout 392 / 323 ms of 2,000) and sizes in `docs/phase2/clean-checkout.md`.
- **Owner-visible:** (1) the four web wasm budgets were raised again (docs 196,608, sheets 196,608, slides 163,840, pdf 163,840 B delta) because the P2 Uzbek catalog import added +21..+33 KB brotli per app; (2) RUSTSEC-2026-0382/0383 (rustls-webpki, webpki-root-certs, emSign) are ignored in `ci/deny/pdf.toml` until the next upstream sync, which carries the fixed versions.
- **Phase 2 follow-ups (2026-10-21, 64 patches, 64 REGISTER rows).** `deckcraft/0123` language tags in Slides (D3); `deckcraft/0124` six plural and list fragments (Columns, Slides, cell alignment tips, missing-fonts toast, damaged-media notice) as whole `{n}`/`{a}` templates with 13 new catalog rows (`reviewed:agent-sonnet:2026-10-21`; the old fragment rows are obsolete; `i18n/p2/code-fixes-needed.md` is empty); glossary: AutoCorrect = *Avtomatik tuzatish*, Command Palette = *Buyruqlar paneli*, Content control = *Boshqaruv elementi* (the reviewed P1 form, old form as `alt-*`). Budgets were not changed.
- **Verification of the follow-ups (2026-10-21).** `ci.sh quick` exit 0 in 159 min (queue, lint, gen-check, deny, brand, i18n, check docs/sheets/slides/pdf, test wordcraft/gridcraft/deckcraft/pdfcraft, bind); `test-ours` 0 (root 27 s, domains docs 422 s, sheets 192 s, slides 239 s, pdf 224 s); `translit-preserve` 0 (8 of 8 IDENTICAL); `build slides all` 0 (desktop 344 s, web 758 s, cli 132 s); `sizes` 0, slides brotli 9,440,021 of 9,466,604 B (headroom 26,583). No budget raised.

## Phase 3 WP14 (2026-10-21): gridcraft NumLocale
- **Built** as carried patches `gridcraft/0023`-`0030` and `0130` (9 patches, 81 gridcraft rows in total in REGISTER). The UI language (ru, uz-Latn, uz-Cyrl; every other language keeps en-US) sets the locale of displayed and typed numbers and dates. Typing `12.10.2026` is 12 October (it was 10 December), `3,5` and `1 234,5` are numbers, `=SUM(1;2)` is accepted. Stored data does not depend on the locale: xlsx parts are byte-equal across en, ru, uz-Latn and uz-Cyrl (engine test, and Excel reads all four files cell for cell identically).
- **Measured** (`docs/phase3/numlocale.md` section 13): matrix 5 of 24 rows equal to Excel ru before, 21 of 24 after (the 3 left are calc, en-US on purpose); 27 typed inputs agree with Excel ru except two deliberate leniencies; ids 14-17 and 22 match. w80 release CLI: en-US median 65.9 s against 71.3 s before (inside the spread), peak 817 against 818 MB.
- **Owner:** Q4 (a uz Windows culture to measure uz in Excel) and Q5 (`;` or `,` as the CSV default for uz and ru, one constant `LOCALE_CSV_LIST_SEPARATOR`) are still open. Excel on this PC writes a plain space as the CSV group character (its Windows thousands separator is U+0020), so Q5 and the group byte need a decision with the real HRM importers.

## Phase 3 WP10 (2026-10-21): fidelity runs and report
- `docs/phase3/fidelity.md`. All 80 `convert formats --json` rows measured: the CI harness set (65 cases) plus 52 rows added through
  `docs/phase3/evidence/fidelity_extra.py`. References: Office 16 COM, LibreOffice 7.4.7 (soffice sidecar), ONLYOFFICE x2t 8.2.3 (local
  reference, needs `ALLOW_PRIVATE_IP_ADDRESS=true`). JSON under `E:/office-target/p3/fidelity`; summary in
  `docs/phase3/evidence/fidelity-summary.json`.
- CI set: 1 silent loss (D1), 0 new against the baseline. The rows outside the CI set: 22 more in 12 directions, plus 2 that `run.py`
  does not check. They are `defects.md` D2-D6, all open, fixes in `services/convert`:
  - D2: same-format odt/rtf conversions carry no warnings;
  - D3: rtf header and revision rows missing;
  - D4: html to html pictures, md to txt tables;
  - D5: native pdf to docx drops a narrow table column;
  - D6: uz/ru CSV dates swapped because `GRIDCRAFT_NUM_LOCALE` is not set.
- O8 input: native as the default once D5 is guarded, the sidecar optional. The pdf2docx "one paragraph per line" weakness did not appear
  on Word-made PDFs. Word PDF reflow hangs headless (not measurable). Corpus (O9) still missing: every number is synthetic.

## Phase 3 WP16 (2026-10-21): the 2026 Latin alphabet law
- **Signed 2026-10-21, not in force.** It enters into force 6 months after official publication. On 2026-10-21 the text was
  not on lex.uz, and no fetched source gives the number OʻRQ-1184 (it comes from the brief). The diff against rules.md
  section 15 used the official announcement, written in the new alphabet, the Senate's page and the press
  (rules.md section 15.0). Result: letters, case pairs, code points, 28 letters + 1 tutuq, and ng (dropped as a unit,
  no transliteration effect) **all match**. The tutuq code point is not stated anywhere; output stays U+02BC (D-02).
- **Crate:** no rule change. The whole official 2026 paragraph converts correctly and round-trips. Added golden
  SRC-017..025 and `official_2026_text_reads_and_round_trips`.
- **Default switch (O13):** one line, `DEFAULT_LATIN_ALPHABET` in `crates/uz-translit/src/lib.rs`, was `Y1995`, **now `Y2026` (see the 2026-10-29 entry)**. The
  library, docs-translit, wasm, the converter CLI/API (`convert::plan::DEFAULT_ALPHABET`) and the page (via
  `/api/v1/formats` `defaults.alphabet`) follow it. The tests pin 1995 themselves.
  - **Not measured:** a run of the suite with the switch flipped was not done; the agent's permission check refused even a
    temporary flip.
- **uz-Latn-2026 converter catalog:** derived at export time, `cargo xt i18n export converter --format json`. Each value
  is the row's own trusted uz-Cyrl value through `uz-translit` `Y2026`, never the ru fallback; there is no new TSV column.
  - `i18n lint` checks the derived values too: 0 errors.
  - All 123 page strings equal the 1995 strings under the letter substitution, 0 differences.
  - The page menu offers "Oʻzbekcha (1995)" and "Oʻzbekcha (2026)". The app catalogs are not derived: four carried 0110
    patches would need a third locale.
- **Font:** Noto Sans Uz (subset) has Ş ş Ç ç Ö ö Ğ ğ. Screenshots are in
  `docs/phase3/evidence/ui/converter-uz-Latn-2026*.png`; no tofu. The converter Playwright spec passed 9 of 9.
- **Reviewed:** `warning.pdf_text_missing`, `warning.pdf_docx_fallback` and `warning.csv_one_sheet` (uz-Latn, uz-Cyrl, ru).
- **Found, not fixed:** `services/convert/tests/warnings.rs` fails depending on test order. `sheets_honours_locale` caches the
  probe per tool binary, so the first test's `honour-locale` marker decides for all the others. It fails
  single-threaded too. This came from the D6 commits; WP16 does not cause it.

## Phase 3 gate review, WP13 (2026-10-22)
Full report: `docs/phase3/gate-review.md`. Architect tier, at HEAD `d67ed47`. **Verdict: not ready for "go" yet.** There are two
blockers. Everything else measured green.
- **`ci.sh full`: exit 0, all 28 jobs ok** in 5 h 17 min (run `wp13-full`). The services passed nextest 171 of 171, with the
  two formerly flaky tests fixed in `15f542b`, `a119148` and `d67ed47`. `convert-e2e`: the API e2e over every `formats`
  row, `directions.sh` 43/43, Playwright 8 passed. `translit-preserve` 8 of 8 IDENTICAL. The silent-loss ratchet
  (`fidelity-directions`, run separately) gives 90 directions, 117 cases and 4 silent losses = the D1 baseline, 0 new.
- **Never silent, broken on purpose** through the CLI, the API and the page: a missing CLI, panic and access-violation
  crashes, a hang (tool timeout and job deadline), the memory cap, the sidecar down, not configured, 503, 504 and 500,
  a real 422 (scan and encrypted), a corrupt docx and pdf, an empty file, a password docx/doc/pdf and a 1.5 GiB zip bomb.
  No case was silent. Message-quality defects are m1-m4 (password .doc and .pdf give exit 1 instead of 9; a garbled OS
  text when the sidecar is down; a cut-off docx is called "unsupported"; a container path in a sidecar message).
- **Blockers:**
  - **B1: PDF combine, split and extract are CLI-only.** There is no API route and no UI control, and the brief lists
    them. Owner: scope them CLI-only for Phase 3, or 1-2 d to add them.
  - **B2: a `;` CSV with `01.03.2019` read in an en-US job stores 3 January.** Status ok, no warning. That happens with
    the API default `locale`, the page in English, or the CLI on a PC with an English display language: the CLI default
    is the Windows UI language, not the regional format. This is D6 returning; the planned interim warning was never
    added. About 0.5 d.
- **Major:**
  - M1: the page shows `{encoding}`, `{missing} of {total}` and `{copy}` literally.
  - M2: the next upstream sync is a hand-merge. The dry run gives 53 clean, 6 3-way and 13 conflicts, among them
    NumLocale 0028 and 0029 (upstream `value_text_as`). Every CLI flag and command id that `convert` uses still exists.
  - M3-M5 (XDFU, off by default; they block switching XDFU on, not the gate): the docx/xlsx watermark replaces the
    document's own header (letterhead and grif lost); detection misses headers of unscanned kinds, two-column
    letterheads and image-only PDFs; the audit chain has no key.
- **XDFU is off by default** in the CLI, the API config and the UI (measured: a grif document converts with no `xdfu`
  object and no audit file).
- **Licences signed off** (`docs/phase3/licences.md`): PyMuPDF 1.26.7 (AGPL) only in the isolated container; the LGPL
  FFmpeg in the opencv wheel is fine in the container (name its exact source before any image is distributed);
  LibreOffice and x2t are test references only, never called by the product. `cargo deny` passes for the root, the 4
  domains and services. The new crates (win32job, ureq, toml, encoding_rs, sys-locale, png, aes-gcm, rustls/ring) are
  permissive. Clean-room grep is clean.
- **Matrix:** 80 `formats` rows cover the brief's docs, sheets and slides directions. `.ppt` is excluded by O2 (exit 3,
  "re-save as .pptx"). The PDF operations are the gap (B1).
- **Owner decisions:** B1 scope; O8, the default PDF to DOCX engine (WP10 recommends native); O9 corpus (every number
  is still synthetic); O10 before XDFU is switched on anywhere.
- **Housekeeping:** C: is at 6.7 GB free. A stray `C:\c\Users\...\scratchpad` (850 KB, synthetic outputs of the review's
  API runs) could not be removed by the agent; delete it by hand.

## Phase 3 close-out (2026-10-22)
Full report: `docs/phase3/gate-review.md` section 10. Architect tier, at HEAD `d7b9582`. **Verdict: READY for the owner's "go".**
- **Blockers fixed.**
  - **B1:** `POST /api/v1/jobs` takes `options.op` = combine, split or extract (`73d8e67`), and the page has an
    Operation select with order buttons.
  - Measured with the real tools: a combine through the API, and through the page in Chromium after reordering, gives one
    4-page PDF in the order the user set.
  - **B2:** a ru/uz-style CSV read in an en-US job warns `csv_locale_mismatch` on the CLI, the API and the page (`827eced`).
    The CLI default now comes from the Windows regional format, and the page has a Number format select that follows its
    language (`6cc3623`).
- **Majors.** Fixed:
  - M1 (warning placeholders filled);
  - M3 (the XDFU stamp no longer replaces headers);
  - M4 (wider detection; an image-only PDF is refused);
  - M5 (HMAC-SHA256 audit chain with an operator key).

  M2 (the upstream sync hand-merge) goes to Phase 4.
- **Minors.**
  - Fixed: m1-m4, m7, m8, m11, m12.
  - Partly fixed: m5 and m13.
  - m6: resolved (engine.rs follows `DEFAULT_LATIN_ALPHABET`; test `auto_to_latin_follows_default`).
  - Deferred: m9, m10, m14.
  - New: n1-n4. All are small: a stale api.md line, the file table's status in combine mode, `reg` by bare name, and no en-US
    csv case in the ratchet.
- **Never silent, re-run:** a password `.doc` and PDF now give exit 9; a cut-off docx is "damaged", exit 9, and the API
  rejects it at POST; sidecar down gives a clean sentence with no OS text and no path; a real sidecar error carries no
  container path through the API. 20 cases, none silent.
- **`ci.sh full`: all 28 jobs ok** (run `p3close-full`, 1 h 34 min). Services nextest 206 of 206. `convert-e2e`:
  `directions.sh` 43/43 and Playwright 12 passed. `translit-preserve` 8 of 8.
- **Ratchet** (`fidelity-directions`): 90 directions, 117 cases, 4 silent losses = the D1 baseline, 0 new.
- **Q4 decisions** (owner-delegated, taken by the orchestrator, `numlocale.md` section 12):
  - Q4-a: uz ids 15-17 keep the `.` forms.
  - Q4-b: uz AM/PM goes to Phase 8 as D8.
  - Q4-c: uz auto formats go to Phase 8 as D9, a `parse_input` design item.
- **Carried to Phase 4:**
  - the upstream sync (M2, 13 conflicts, gridcraft 0028/0029 against `value_text_as`);
  - D7 (pdfcraft ru "Move down");
  - D1, D8, D9 and m9 to Phase 8;
  - the deferred minors;
  - Phase 2 M3 (Android phone open);
  - server mode over TLS in a real browser, not exercised end to end;
  - the corpus (O9).
- **Owner decisions still open:** O8 (the default PDF to DOCX engine; the code default is native), O9 (corpus), O10
  (before XDFU is switched on).
- **Housekeeping:** `C:\c` (850 KB, synthetic) still exists; delete it by hand. C: has 17 GB free.

## Phase 4 WP7: web shell and `suite-serve` (2026-10-22)
- `services/suite-serve` mounts the Phase 3 converter router (`convert_api::routes::router_with`) and adds `/` (shell, `web/shell/`), `/convert/` (the converter page) and `/docs/ /sheets/ /slides/ /pdf/` (the trunk dists, `.br` precompressed, hashed files immutable). Editor routes get `'wasm-unsafe-eval'` plus the sha256 of each inline script and style of that dist's `index.html` (computed when the server starts) and `frame-ancestors 'self'`; the shell and the converter keep the strict CSP.
- Shell: plain HTML/JS/CSS, catalog `web/shell/i18n.json` (en, ru; uz-Latn and uz-Cyrl drafted `agent-sonnet:2026-10-22`, reviewed `reviewed:agent-opus:2026-10-23`; not in the xtask TSV pipeline yet). Recent files in IndexedDB (200 MB cap, oldest dropped). Names and colours come from `brand/brand.toml` through `/shell-config.json`.
- Budget: `[shell]` 45,417 B brotli measured, `br_max` 65,536. Editors' absolute budgets are `base_br + delta_br_max` (unchanged; headroom 19-27 KB). Spec: `ci/web-smoke/shell.spec.ts` (11 tests, `ci.sh shell`).
- Open: templates (manifest support is in, `templates.json` is empty), the Slides bridge (WP8: until then Slides opens blank and a file times out with a message), shell catalog into the xtask i18n pipeline.

## Phase 4 WP5 + WP0b: shared services and the reference re-check (2026-10-23)
- **WP0b:** every upstream `file:line` in `docs/phase4/PLAN.md` and `toolbar-design.md` was re-checked at the new BASEs and the moved ones updated in place (about 80 moved, the rest confirmed). All 139 command ids of toolbar-design Appendix A are found at their updated lines; none is missing. One behaviour change: wordcraft v0.9.0 `file.exportPdf` without a `path` now opens the PDF export dialog (#2313), so it can sit in a custom group and the `office.exportPdf` workaround is optional.
- **`crates/suite-core`** (MIT OR Apache-2.0, `forbid(unsafe_code)`; deps `toml_edit`, `serde_json`, `ron`): the shared `settings.toml` at `%APPDATA%\<product>` (typed keys `lang` incl. `uz-Latn-2026`, `latin_alphabet`, `toolbar`, `spell_dictionaries`, `xdfu`, `recent.limit`, `history.*`, `server.url`; atomic writes; a corrupt file is set aside as `settings.toml.corrupt`), the language choice, the recent files read from the four apps' own lists (contract tests against the vendored sources; gridcraft and deckcraft keep theirs in `ui.json`, not `prefs.json` as the plan said), the Noto fallback lookup, and the template catalogue. API for the lanes: `settings::load()`, `toolbar_mode()`, `set_toolbar_mode()`; see `docs/phase4/shared-services.md`.
- **Templates:** five synthetic Uzbek starters in `web/shell/templates/` (buyruq, xizmat xati, tabel, xodimlar ro'yxati, taqdimot) made by `scripts/gen-templates.py`; the names and text were drafted `agent-sonnet:2026-10-23` and reviewed `reviewed:agent-opus:2026-10-23`.
- **Desktop host and shell:** `settings_get`/`settings_set` read and write the shared file; the page may set `lang`, `toolbar` and `latin_alphabet` only, with value checks. The shell treats `lang = auto` as "follow the browser" and shows `uz-Latn-2026` with the `uz-Latn` catalog until WP16.
- Open: the apps' own start-up reading of the shared values (the per-app lanes and WP6), the installer's single font copy, a settings page in the shell.

### Phase 4 Slides lane (WP3, WP4, WP6, WP8; 2026-10-23)
- **Patches** deckcraft 0002 (web host bridge), 0140 (simplified toolbar, Fluent icons as 19 alpha masks), 0141 (command hook), 0142 (shared dictionaries folder); REGISTER rows added, `verify-queue.sh deckcraft` and `cargo xt register check` green.
- **Bridge:** `deckcraft:ready/open/opened/requestSave/save/dirty/failed`, parent-only and same-origin checked; `ci/web-smoke/bridge-slides.spec.ts` 9/9 three runs in a row; shell Slides round trip passes.
- **Wasm:** slides brotli 9,492,465 B; the SVG loader (+309 KB) cannot fit 19,944 B of headroom, so bitmaps. `delta_br_max` 163,840 to 196,608 (measured delta +164,856 + 10%, rule of budgets.toml): `docs/phase4/icons-slides.md`.
- **Spell crash:** 0 of 25 GUI runs crashed, quarantine stays: `docs/phase4/spell-crash.md`. Dictionaries en_US, ru_RU shipped (`assets/dictionaries`, `docs/phase4/licences.md`).
- Not run: full `ci.sh quick` and `ci.sh test deckcraft` (build slots busy); only the new tests ran (14 of 14 pass).

### Phase 4 Sheets lane (WP3, WP4, WP12; 2026-10-23)
- **Patches** gridcraft 0140 (simple toolbar: new `office_toolbar.rs`, 25 Fluent icons as alpha masks in `office_icons.bin`, the Full ribbon / Simple ribbon toggle through suite-core's `toolbar` key; 20 added and 7 removed lines in pre-existing upstream files besides Cargo.lock) and 0141 (command hook on `Session::execute`, no-op until installed); REGISTER rows added, `verify-queue.sh gridcraft` ok (44 patches).
- **Wasm:** the SVG loader (+309 KB, `docs/phase4/icons-wasm.md`) cannot fit 24,492 B of headroom, so bitmaps (5 KB brotli). Sheets brotli 4,943,443 B (+33,932 B); `delta_br_max` 196,608 to 229,376 (measured delta +206,048 + 10 %, the budgets.toml rule). My own with/without-loader build did not reproduce the +309 KB and is not cited: `docs/phase4/sheets-toolbar-wasm.md`.
- **Width thresholds measured**, not the design's 1,500 / 1,100 estimates: labels from 1,640 px (en) to 1,820 px (ru, uz-Cyrl), every item in the row from about 1,320 px.
- **Keytips:** Alt,H,F,F / F,S / H / F,C / N reach the five picker controls in simple mode (kittest, `tests/office_toolbar.rs`).
- **WP12:** memory gate passed (w40 573, w80 904.5, w120 1,241 MB steady, all within +1.3 % of round 2); open time not clean (machine busy), idle re-run owed. The shell host now stops a workbook over the 1 GiB wall before Sheets starts, warns about external links, and offers Excel. The GUI's own message at the wall is clear, so no carried message fix: `docs/phase4/excel-guard.md`.
- Hook bypass list for Sheets: `docs/phase4/hooks-sheets.md` (WP11 must also guard `file.saveBytes`).
- Catalog: five new gridcraft rows and five ru gaps filled (drafted `agent-sonnet:2026-10-23`); the shell `guard.*` rows are in `web/shell/i18n.json` with no `_draft` block. All of them, and the five templates' text, were reviewed `reviewed:agent-opus:2026-10-23` (commit 02f1364).
- Not done: the simple toolbar's "Version History" button appears only once WP10 registers `office.history`; no Alt-letter badges on the new "Full ribbon" toggle.

### Phase 4 integration pass (2026-10-23)
- Reds fixed at the root (lockdiff own-roots, zip 9, brand-grep, shell spec not reproducible on HEAD), product renamed to Hoshiya, logo pack integrated; **the full gate is not green yet: E: filled up (upstream test targets of 50-150 GB each) during `build all`**. Job table, owed items and the contention notes: `docs/phase4/integration.md`.

### Phase 4 history, XDFU, MSI, phone evidence, accessibility (WP10, WP11, WP14, WP16, WP18; 2026-10-24)
- **WP10 version history (done, tests green; real-window use not yet watched by the owner).** `crates/suite-core/src/history.rs`: content-addressed, hash-chained local snapshots (view, restore as a new version, save a copy, name). The desktop host and the shell History panel (`web/shell/history.js`, IPC `history*`) are in; Docs gets the layer in `domains/docs/crates/docs-office/src/layer.rs`, Sheets and Slides get it through patches gridcraft/0143 and deckcraft/0143 (office hook `wrap`, `office.history`); wordcraft/0123 marks AutoSave so an autosave makes at most one version in 5 minutes. Details and the bypass list: `docs/phase4/hooks-docs.md` (WP10 and WP11 section), `docs/phase4/desktop-host.md`.
- **WP11 XDFU watermark (done, opt-in, off by default).** `crates/xdfu-core` (grif finder, stamp, keyed audit chain; `LazyIssuer` so a copy number is issued only when a copy is made) is shared by the converter and the three editors. With `xdfu = on`, copies (print, PDF/PNG/CSV/HTML export, save as, `file.saveBytes`) carry the cl. 12 stamp and nothing is refused; off means no command, no audit file, byte-identical output (test `xdfu_off_registers_nothing` in each editor). Known limits (web path has no layer, clipboard and screenshots are outside): `docs/phase4/hooks-docs.md`.
- **WP14/WP15 MSI (built, install test passed on Win10).** `docs/phase4/windows-verification.md`: `hoshiya-0.1.0-x64.msi` 175.8 MB, WiX 5.0.2 pinned, statically checked (decompile, registry values, 5 shortcuts, MajorUpgrade). **Re-run 2026-10-25 with the post-gate-fix MSI** (201.7 MB, SHA-256 `2430CB40…A97E`, notices included), elevated by the owner: 0 failed checks (install, 27 extension registrations / 4 ProgIDs / 4 Capabilities / 5 shortcuts, no default changed, Open with x4, spaces+Cyrillic 4/4, single instance x4, uninstall), residue 0 of 402 installed registry lines, 0 files left; log `docs/phase4/evidence/openwith/msi-test.log`. "Always" is not driven by the script: owed as a manual check. Windows 11 and the domain PC are not run. Windows Sandbox is not available on this PC.
- **WP16 phone-sized evidence (done as a proxy; no phone, B-10).** `docs/phase4/mobile-evidence.md`: shell and four editors at 360x800 and 412x915, touch, CPU throttling up to 20x, no horizontal overflow. M3: on desktop the stalling stage of the 108-page docx is whole-document layout in one synchronous call after the open command (51 s at 20x); the emulator's 240 s is not reproduced and its cause is not proven. Recommendation: keep A-web conditional, decide B-3 when a phone exists.
- **WP18 accessibility baseline (done, partly not run).** `docs/phase4/a11y-baseline.md`: shell and converter pass the keyboard walk, focus and contrast (shell names fixed in fa108dd); the web editors in the iframe **fail** (Tab does not leave the iframe, no accessible tree inside the canvas); the desktop apps expose unnamed interactive nodes (UIA: Docs 44, Sheets 31, Slides 19, PDF 4). **Not run:** real key walks of the desktop apps and Narrator (the session was locked).
- **REGISTER:** gridcraft/0143 is 12 files +659/-14 and deckcraft/0143 is 11 files +720/-4 (the rows said +700 and +750); 88 patches, 88 rows.
- **WP22 / WP17 (this pass):** ctutils 0.4.4 fixes RUSTSEC-2026-0385 (carried lock patches wordcraft/0900, pdfcraft/0900); sccache is per checkout; the shell catalog is `i18n/shell.tsv` (`cargo xt i18n export shell --format json`); Version History's engine message goes through the catalog (wordcraft/0124, gridcraft/0144); the History panel shows the store's two English messages in the page language; CI has the templates spec in `shell`, on-demand `desktop` and `msi` jobs, a disk guard, and `ci.sh` runs every job named. **uz-Latn-2026 in the four apps is deferred:** it needs a third language entry in each `0110` patch and a size decision (an embedded catalog is an estimated 60-90 KB brotli against 19-27 KB of headroom; runtime derivation costs about 30 KB where uz-translit is not linked), measured nowhere yet. The shell still shows uz-Latn for it. Details: `docs/phase4/integration.md`.
- **WP24 gate review (done; fixes landed):** review 7f1b3f1; fixes 5905293 (paper print stamp, audit takeover, Version History message, converter picked file), 39e77a4 (Slides title, History View copy read-only, restore text, WebView2 overrides), bdb77d6, e719099 (print hook desktop only); **brief task 5 closed in 6413f7a (code d3deade)** (owner 2026-10-24): the four apps read the shared `lang` at start (wordcraft/0126, gridcraft/0145, deckcraft/0145, pdfcraft/0112), applied when the app's own language is `auto` (Sheets has no `auto`: a fixed shared `lang` wins at each start); an in-app change does not write back to `settings.toml`; Slides' `auto` stays English; uz-Latn-2026 stays shell-only. Details: `docs/phase4/shared-services.md` section 6.1. 94 patches, 94 rows.
  **F10 fixed** (mapped drive / UNC): the host trusts a file it already knows (Recent, History) as the user's own, and matches a drive-letter path to a stored `\?\UNC\...` document by id; test `a_document_on_a_share_or_mapped_drive_keeps_its_history` (run on a real `net use` drive). Before: History said "no saved versions", Recent reopen was refused.
- **WP23 kit ready (2026-10-25), sessions pending (IT staff, owner).** Protocol and results table `docs/phase4/usability.md`; printable cards uz-Latn/ru, observer sheet and the T4 PDF in `docs/phase4/wp23/` (abecb11; wording re-reviewed 483317f: the intro said "you must not make mistakes"); `scripts/wp23-reset.ps1` clears the current user's product folders between participants (dry run by default, `-Yes` deletes; tested on a fake profile only). Kit for the owner: `E:\office-target\wp23-kit\`, with the MSI rebuilt for the F10 fix (SHA-256 `80E7C9B0…C317`; it differs from the install-tested `2430CB40…A97E` build only in `hoshiya.exe`, so the install test was not re-run). Limits: the web session runs only on the owner's PC (local mode binds loopback; LAN needs server mode with TLS + token), the T4 converter download lands in Downloads, and the click paths of T4/T5 are read from code, not clicked.
- **WP23 findings 1-2 (owner's MSI use, 2026-10-27; fixed on branch `p4-recovery`, 2026-10-29).** (1) Docs opened File › Recover at every start with stale copies, also of other files: upstream opens the page whenever a copy exists and removes a copy only when the still-open document is saved, so Don't Save, Save + Open/Close in one step and "Continue without restoring" left copies that came back forever. New carried patch `wordcraft/0129` (Office's AutoRecover model): the page opens once per copy (an `offered` marker; a newer generation clears it, so a later crash is offered again); a clean exit (`on_exit`) removes the window's own copies; a document replaced with no unsaved changes releases its copy; an offered copy older than 7 days is removed (never one not yet offered, never one the window holds). (2) Save froze the window: the docs-office version snapshot ran inside `file.save` on the UI thread and pruned the whole history store, 2,672 documents in the owner's profile, nearly all temp outputs of converter test runs (the converter's sandboxed CLI passes the real `%LOCALAPPDATA%` for fonts, so it recorded history). Fix in `docs-office/layer.rs`: snapshots run on one background thread (`flush_snapshots` at exit, 10 s at most), and a `WORDCRAFT_NO_PREFS` run (the converter's) keeps no history. Measured with the built release exe over the control port, on a copy of that store: save blocked the UI 29.8 s / 50.6 s (local disk), 44.5 s (from the Recover page), 51.8 s (mapped drive `net use` to the local admin share), next saves 1.0-4.2 s; after: 0.12-0.65 s in every case. Recovery run with the built exe (isolated profile): before, the page opened at starts 2, 3 and 5 and Don't Save + exit kept the draft; after, only at start 2 (after the kill), and Don't Save + exit removes it. PDF (different mechanism, already cleans on clean quit) gets the same 7-day expiry for a dismissed dialog (`pdfcraft/0113`; the dialog is its only way to the snapshots, so a dismissed one is still offered again within the week); Slides reopens leftovers as tabs (resolved by Save / Don't Save) and Sheets has no crash recovery: not changed. Tests (overlay on): wordcraft engine `recovery` 18/18 (4 new), ui-egui `recovery` 3/3 (new `recovery_page_opens_once_for_the_same_copies`), docs-office 21/21 (new `a_busy_history_store_does_not_hold_up_the_save`), pdfcraft ui-egui `--test recovery` 13/13 (new `a_snapshot_offered_a_week_ago_expires_and_a_newer_one_is_offered_again`); `verify-queue.sh wordcraft pdfcraft` OK, `cargo xt register check` 96/96. Full workspace suites and clippy not run (E: was down to 4 GB free). Not done: the owner's own history store still holds the 2,672 test documents (their data folder; `scripts/wp23-reset.ps1 -Yes` clears it); the upstream docx save itself still runs on the UI thread (0.1-0.4 s here; a multi-MB document not measured). 96 patches, 96 rows.
- **i18n pass (branch `p4-i18n`, 2026-10-29), from the exploratory uz-Latn/ru test of the Phase 4 build.** Docs and `i18n/wordcraft.tsv` untouched (a Docs pass follows).
  - **Slides, deckcraft/0146:** about 400 widget calls drew an English literal although the catalog had the row (only the ribbon, menu, dialog and backstage helpers went through `i18n::t`). They are wrapped now; `big_button_ex`/`small_button_ex` translate their label; `Slide {} of {}`, `{} of {}`, `Version {}` are whole templates (`status::slide_of`); `Present` is `t("Present")` inside its padding; status line and proofing language names are translated. New rows (xtask `OWN_DECK_0146`): starter names, paper names, `Slides:`. Evidence `docs/phase4/evidence/i18n/slides-*-uz-latn.png` (main window, Home, Print, Preferences: no English left).
  - **Sheets, gridcraft/0146:** the print preview formats cells with the interface number locale (`cell_text_in`, as the PDF does); sample workbook names go through `lang.tr`. The print page wording and the `Options` context `Editing` had no catalog rows (xtask `OWN_GRID_0146`, 19 rows copied from the Docs rows, marked `reviewed:agent-sonnet:2026-10-29`). Evidence `sheets-print-uz-latn.png`.
  - **PDF, pdfcraft/0114:** no milestone banner, no Ready/Mxx badges; tools that are not built are not listed (panel, palette); a planned command run by id says it is not available. The brand name: the code already goes through `rebrand`, but the Uzbek rows glued suffixes to the name (`PdfCraftga`), so the token was not replaced; 14 rows now read `PdfCraft dasturiga` (lint product-token rule keeps the name whole). Evidence `pdf-edit-uz-latn.png`, `pdf-home-uz-latn.png`.
  - **Web, deckcraft/0147 + shell:** the shell opens each editor frame with `?lang=<shell language>`; the four editor pages (`domains/*/apps/*-web/index.html.in`) answer `navigator.language(s)` with it, Slides also reads it in Rust (its `auto` is English). Changing the language in the shell drops idle frames (an editor with unsaved work keeps its language until reopened). Playwright `ci/web-smoke/shell.spec.ts`: 5 new tests, run against a dist assembled from the p1 editors plus the new index.html and shell (the editors' wasm was not rebuilt): shell home, language switch, docs/sheets/slides frame URL and `navigator.language`, pdf page `navigator.language` pass.
  - **Open:** English ru rows. Empty `ru` column (active rows): deckcraft 1118 of 1138, gridcraft 447 of 763, pdfcraft 312 of 2478, shared 11 of 593 (wordcraft 43, not touched). Still English by design: the ribbon tooltip descriptions and engine messages that are not UI literals; the Uzbek rows with `ʻ` vs `’` untouched.
- **Language rule unified, crypto-bigint (2026-10-24):** one rule in Docs, PDF, Slides and Sheets: a fixed shared `lang` always wins at start (`office_overlay::start_language`, called in each app's `main.rs` after the saved prefs; Sheets in `load_prefs`), the app's saved language applies only when the shared `lang` is `auto`, an in-app change is session-only and never written back; tests `shared_lang_wins_over_the_saved_language_at_start` (wordcraft/0126, pdfcraft/0112, deckcraft/0145 rewritten; web unaffected). `ci.sh deny` pdf: RUSTSEC-2026-0387 fixed by crypto-bigint 0.7.5 to 0.7.6 (needs ctutils 0.4.5) in the carried lock patch `pdfcraft/0900-pdfcraft-crypto-bigint-0.7.6.patch` (renamed from the ctutils one) and `domains/pdf/Cargo.lock`; lockdiff unchanged (0 version changes); third-party notices regenerated. 94 patches, 94 rows.

## Phase 5 (started early, in parallel; branch `phase-5`, worktree `E:/office-wt/phase-5`)
- **Plan written 2026-10-25:** `docs/phase5/PLAN.md` (17 findings, WP0-WP18, security checklist S1-S25, owner decisions Q1-Q11; about 42.5 stream-days likely, 24.5 on the gate path) and the field table `docs/phase5/onlyoffice-compat.md` (skeleton, nothing verified yet). Pilot flow **assumed** HRB `files` docx (owner may change; affects WP1, WP9, WP17 only). The HRM test server is unchanged since Phase 0 (owner, 2026-10-25), so HRB PR-0 … PR-3 are not deployed yet: they are the external critical path. Implementation starts with WP0, then WP2 and WP1.

### Phase 5 WP0: baseline and merge-forward rule (2026-10-25)
- **Merge-forward rule:** after every Phase 4 fix, run `git merge phase-4` on `phase-5` (never the reverse: nothing from `phase-5` goes into `phase-4`). Run `scripts/verify-queue.sh` after a merge that touches `patches/`. Merge `phase-4` before starting WP6 or WP15.
- **Baseline before any Phase 5 change (`phase-5` at f89e7b6, OFFICE_TARGET `E:/office-target/p5`):** `scripts/ci.sh quick` exit 0, 41 min 56 s, 19 steps all exit 0 (queue, register check, clippy root, upstream fmt/layers/assets for four trees, gen-domain, lockdiff x4, brand-grep, i18n x3; upstream clippy: 1/3/2/1 warnings, all baselined). The `convert` job did not run (no `services/` diff). Services: `cargo test --workspace --exclude suite-desktop` 213 passed, 0 failed, 14 min 24 s (cold build included). `suite-desktop` tests were not run in the baseline.
- No baseline failure.

### Phase 5 WP2: docserver crate, config, mounting (2026-10-25)
- `services/docserver` (member of the services workspace, `forbid(unsafe_code)`): `[docserver]` config with every refusal (`config.rs`), the `/ds/` stub routes (501 until WP3+) and the internal router (`routes.rs`). Mounted by `suite-serve` (`router_ds`, `spawn_internal`) only when the config file has a `[docserver]` section; without it nothing changes.
- Header rules (F5): `convert-api` `baseline_headers` accepts a route-set `Cross-Origin-Resource-Policy` only when it is exactly `cross-origin` (only the `api.js` routes set it); an empty `X-Frame-Options` from a route (`NO_FRAME_OPTIONS`) means "none" and is honoured only beside a CSP whose `frame-ancestors` is not `'none'`, else it becomes DENY. The wrapper page and, with the docserver on, the editor frames get `frame-ancestors 'self' <all providers' frame_origins>`.
- Tests: docserver 10 (config refusals, internal listener), suite-serve `docserver.rs` 2 (`no_providers_no_docserver_routes`, S13 framing/CORP table), convert-api +1 (header rules). Services total now 226 passed, 0 failed (`--exclude suite-desktop`); `cargo clippy --workspace --all-targets -- -D warnings` clean; `ci.sh --no-build convert` (fmt, cargo-deny incl. linux target) ok. No new dependency.

### Phase 5 WP10: deployment image and local rehearsal (2026-10-25)
- `packaging/docserver/` (Dockerfile, build.sh, compose.yml, rehearse.sh, Traefik config) and `docs/phase5/deploy.md`. Image `hoshiya-docserver:rehearsal`: 49.7 MB content / 193 MB on disk, non-root (uid 10002), read-only rootfs, healthcheck; `cargo deny` (services policy, linux target): all four checks ok. Rehearsal on `backend_net_rehearsal` (Traefik with a throwaway CA, server, fake-hrm container, internal listener 403 from a foreign container and unreachable from the host): open, edit, save, close, reopen, download 3 of 3 green (`ci/web-smoke/docserver-compose.spec.ts`, Chromium on the host). One earlier run timed out at 30 s under heavy disk load; see deploy.md.

### Phase 5 WP12: diff engines and the History Compare view (2026-10-25)
- `crates/seq-diff` (patience + bounded Myers + similarity pairing), `domains/docs/crates/docs-diff`, `domains/sheets/crates/sheets-diff` (domain binaries, JSON schema 1: `docs/phase5/PLAN.md` 6.1-6.2). Golden tests on synthetic pairs: docs 9 (identical, insert, delete, move, modify, format, blame, table cell, rewrite), sheets 9 (identical, value, formula, row insert, row insert next to an edit, row delete, column insert, sheets added/removed/renamed, format), seq-diff 6; clippy `-D warnings` clean.
- Measured (PLAN 6.3): 108-page docx 65-106 ms wall (diff 10-44 ms); w40 (5.18 M cells) 30-31 s wall, of which reading both 28-30 s and the diff 1.4-1.5 s (6.5 s when 576,000 cells differ); process start 6-8 ms.
- Desktop History panel: compare any two versions (`history_compare` IPC, suite-desktop test `history_compare_checks`); 28 shell strings (uz-Latn, uz-Cyrl, ru) `reviewed:agent-opus:2026-10-25`; the old "comparing arrives in Phase 5" note removed.
- Open: the server compare route and the web panel, the tools in the MSI, headers/footers/notes in docs-diff, number formats in sheets-diff values (PLAN 6.4).

### Phase 5 WP16: `ci.sh docserver`, compat re-verification (2026-10-25)
- **Job `docserver`** in `scripts/ci.sh` (also `.github/workflows/ci.yml`, `needs: [convert]`, not in `quick`): fmt, clippy `-D warnings` and `cargo test -p docserver -p fake-hrm -p suite-serve` in `services/`, then the docserver and bridge-docs Playwright specs (binary called as `node_modules/.bin/playwright` from `ci/web-smoke`). The Docker rehearsal (`packaging/docserver/build.sh` + `rehearse.sh`, 3 runs) runs only with `ci.sh docserver rehearsal` or `CI_DOCKER_REHEARSAL=1`; otherwise the job prints "skipped: docker rehearsal not requested". `quick` runs it when the diff touches `services/{docserver,fake-hrm,suite-serve}`, its specs/configs or `packaging/docserver/`; `full` and `nightly` always.
- **Measured (OFFICE_TARGET `E:/office-target/p5`, warm build):** without Docker 2 min 07 s (clippy 1 s warm / 43 s cold, tests 18 s warm / 134 s cold, Playwright docserver 17 s, bridge-docs 86 s). With the rehearsal 5 min 57 s (image build 155 s, compose + isolation probe + 3 browser runs 74 s). All exit 0; 70 Rust tests in the three crates, 6 docserver specs, 5 bridge-docs specs, 3 of 3 rehearsal runs green. First run also made `shell/` in `dist/` (shell-dist.sh, 137 s, one-off).
- **Compat file re-verified** against 7f10ad8: every row now names a test or says "no test". Corrected: the callback sends no `filetype` and no `actions` (hr-backend's need for `actions` is open), `exp` is required by default but `require_exp = false` exists for HRB as surveyed, titles are validated not sanitised, `onAppReady` is not asserted by any test. Newly credited: onWarning (S23 spec), status 1/2/4/6, retries, SSRF rows, view-session rows.
- **Open:** field names are not re-checked against the public DocsAPI pages (no network step); `ci.yml` job is untested (workflow disabled, `actionlint` not installed here).

### Phase 5: permissions.download enforced on the server, compare route (2026-10-25)
- **download:false / print:false.** Version download and the new compare route return 403 without `download` in edit sessions too, with a `perm.refused` audit line (also for restore/name without `edit`); `/document` is untouched. The editors' own export and print UI cannot be hidden without a bridge flag (a carried wordcraft/gridcraft patch): recorded as a gap in the compat file, no patch written. Tests: `services/docserver/tests/history.rs::download_false_is_refused_in_an_edit_session_and_audited`.
- **Compare route.** `GET /ds/session/{sid}/history/{a}/compare/{b}`, `[docserver] diff_bin_dir`, fake tool `src/bin/fake_diff.rs`; tests for a result, exit 2, oversize, timeout kill, missing binary (501), slides (501), 403, and encrypted at rest (plaintext only in the session folder, removed after). Not done: a web History panel (none exists; see PLAN 6.4) and the two binaries in the Docker image.

### Phase 5: download / print flags reach the Docs editor (2026-10-25)
- **Patch `wordcraft/0128`** (4 files, +41/-1; REGISTER row): `wordcraft:open {canDownload?, canPrint?}`, default true. False withholds Save As and the PDF/image exports, or Print: greyed out (ribbon, Backstage pages) and refused by `run` (Ctrl+P, F12, Ctrl+Shift+S); a Backstage page left open on one falls back to Home. New `Services::blocked` hook in ui-egui; scripts (`execute`) and Save are unaffected. The browser's own print command cannot be blocked from a page (same as ONLYOFFICE); PLAN section 5 says so. Docs only: Sheets, Slides and PDF ignore the flags.
- **Wiring:** `services/docserver/static/editor.js` sends `perms.download` / `perms.print` in the open message; `fake-hrm` takes `?download=0&print=0` through its host page.
- **Tests:** bridge-docs.spec.ts cases 6-8 (default reachable: Print page luma shift 17.4, Save As 1 message; blocked: shift 0.08, 0 messages, Save still sent; flags reset on the next open), docserver.spec.ts `WP18 edit session` control and download:false/print:false. Docs web release rebuilt into `E:/office-target/p5/dist/docs-web`.

### Phase 5: web History panel in the docserver wrapper (2026-10-25)
- **Panel.** `services/docserver/static/editor.js` (a "History" button in a slim bar above the editor; side panel, Esc closes, focus returns to the button) lists the versions of the session's document and offers Save a copy and Compare (needs `download`) and Restore and Name (needs `edit`); controls the session may not use are not drawn, the server still answers 403. Compare shows the server's text for 501/422/502/504. Strings: `hist.*` plus one new `hist.close` (uz-Latn, uz-Cyrl, ru, en; Uzbek `reviewed:agent-sonnet:2026-10-25`); the wrapper takes the language from the session config `lang`.
- **Shared code.** `web/shell/history-ui.js` (version rows, compare selectors, docs/sheets diff renderers) is loaded by the desktop panel (`history.js`, 82 lines fewer) and by the wrapper from `/shell/` (no new origin, no inline script; `shell.css` and `history.css` are reused). `ci.sh sizes` counts the new file.
- **Tests.** `ci/web-smoke/docserver.spec.ts` "History panel" x2: list after two saves, name, download (file name and bytes), restore with cancel, Esc and focus, compare 501 `not_configured` shown, then a restarted server with `diff_bin_dir` -> `fake_diff` draws the change; `download:false` has no download or compare controls; a view session has no restore or name. `fake_diff` now prints one `changes` entry when the files differ; `fake-hrm` takes `?lang=`.
- **Restore reloads the editor** (5cc8f86): after a restore the wrapper sends the restored version to the editor, so its next save cannot overwrite it; the History panel spec checks the reload.

### Phase 5: diff tools shipped in the MSI and the image (2026-10-25)
- **MSI** (`E:/office-target/p5/msi/hoshiya-0.1.0-x64.msi`, not installed): `docs-diff.exe` (2.9 MB) and `sheets-diff.exe` (2.0 MB) next to the app exes; both are in the File table. 214,167,552 bytes (204.2 MB), sha256 `A589C894209302EB84B33EE19FF7EE7E714D5D5AA44842C0B0405BED5C2439A2`; the WP23 kit MSI is 211,542,016 bytes (+2.6 MB in all, which also holds the new docserver code in `hoshiya-serve.exe` and the fresh docs dist, not only the two tools). Notices regenerated (`gen-notices.sh`: docs-diff 56 crates, sheets-diff 44). `msi-test.ps1` (elevated, owner) now also checks the two files.
- **Image:** `hoshiya-docserver:rehearsal` 59.7 -> 62.3 MB content (+2.5 MB), 213 -> 222 MB on disk. Vendored apart: `E:/office-target/p5/docs-vendor`, `sheets-vendor`. Compose rehearsal x3 passed, including History > Compare drawing a real docs diff.
- Not done: the elevated MSI install test and a Compare in the real WebView2 window (owner's runs).

### Phase 5: owner decision Q12 (2026-10-26)
- The provider's working copy (the callback file route) is no longer XDFU-stamped; only history downloads are, so stamps do not pile up on save and reopen. With xdfu on, pdf copies stay refused. Test `wp14.rs::a_download_of_a_grif_document_carries_the_stamp_the_callback_copy_does_not`; docserver tests green, clippy clean.

### Phase 5: History panel visual review (2026-10-26)
- Reviewed screenshots (360 and 1366 px, light/dark, uz-Latn/ru; `DS_SHOTS=<dir>` on the docserver spec): the Compare selects overflowed the panel at 360 px; fixed in `history.css` (labels stack, selects shrink). Shell budget 58394 / 65536.

### Phase 5 WP18 rehearsal: security checklist and attacker review (2026-10-30; branch `p5-secreview`)
- [docs/phase5/security-rehearsal.md](phase5/security-rehearsal.md): §10 is 19 green, 3 partial (S14, S19, S21), S18 missing (no retention sweep: nothing stored is ever deleted, F1, high, needs a design), S22 not run (no cargo-deny/audit here), S25 owner/HRM.
- Fixed with tests: plaintext compare copies left by a crash (F2), session ids in two error log lines (F3, S17), a frame origin with its default port refused every session (F6); S1 test gap closed. Open: F1, F4 (rate-limit key behind a front proxy), F5 (history shared per key), F7/F8 (error texts).

### Phase 5: retention sweep (F1, S18) and the proxy-chain deployment fix (F4) (2026-10-30; branch `p5-retention`)
- `services/docserver/src/retention.rs`: sweep at start and hourly. A session folder goes 24 h after a close the provider
  acknowledged, 7 days after the last activity otherwise, never while a status 2 is queued. `[docserver] keep_closed_hours = 24`,
  `keep_abandoned_days = 7`. Its history document and unshared blobs go with it (new `suite_core::history::Store::forget_id`) unless
  another session of the key is on disk or the provider has `history = "keep"` (default `"session"`; PLAN §4 "Retention").
  One `retention.sweep` audit line with counts only.
- F4: `traefik.yml` `forwardedHeaders.trustedIPs` (operator value, loopback in the rehearsal); `deploy.md` "Client addresses behind
  proxies": `trusted_proxies` must list Traefik and every front proxy in `trustedIPs`.
- Measured: services `cargo test -p docserver -p fake-hrm -p suite-serve -p convert-api` 169 passed, 0 failed (was 165; 4 new sweep
  tests); `cargo test -p suite-core` 49 passed (1 new); `cargo test -p suite-desktop` 26 passed; clippy `-D warnings` on docserver clean;
  Playwright `playwright.docserver.config.ts` (`DIST=E:/office-target/p5/dist`) 17 passed, 1 skipped (2.2 min).
- Not checked: the sweep thread in a running server over hours (the tests inject the clock); the real proxy chain on `<test-host>` (WP17);
  sessions closed before this change have `acked` false and wait the 7 days.

### Phase 5: security rehearsal, open items closed (2026-10-30; branch `p5-sec2`)
- Crash gap (PLAN R3): `callback.rs` `recover_unnotified`, run at start by `spawn_retry_loop`. A session closed with status 2 and
  `notified = false`, `acked = false` (and no queue entry) gets its status 2 through the normal queue. At-least-once: a crash after
  the provider acted but before `acked` is written can repeat the same version.
- S14 (Playwright, foreign frame posts `<prefix>:save` etc.), S21 (`docserver/static/` scanned, `onlyoffice` allowed on comment
  lines only), S19 built-in TLS half (`tls.spec.ts`: `/ds/editor` HSTS, plain HTTP 308). F7 and F8: fixed texts to the browser,
  detail in the log without ids. S rows: 22 green, 1 partial (S19), 1 not run (S22), 1 owner/HRM (S25).
- Measured: services tests 170 passed, 0 failed (was 169; 1 new); clippy `-D warnings` on docserver clean; Playwright docserver
  18 passed, 1 skipped (was 17+1; 1 new); Playwright tls (debug build) 6 passed.
- Not checked: S19 behind Traefik (Docker, TLS-intercepting proxy); S22 (tools missing); the crash recovery in a real process kill
  (tested by a record written to disk, then a restart); F5 unchanged by design.

### Phase 5: ONLYOFFICE drop-in, no HRB change (2026-11-07; branch `p5-dropin`, owner decision of the same day)
- Plan and gap table: [docs/phase5/dropin.md](phase5/dropin.md); `hrb-handoff.md` marked superseded. Server: provider
  `compat = "docsapi"` (no `exp`, secret floor 16 bytes with a warning under 32), `edit_types` (other types open as views),
  `internal_allow` and `trusted_proxies` may name containers (re-resolved every 15 s), save/autosave/dirty refused (409) before the
  provider's document was fetched (bounds a replayed no-`exp` token). Wrapper: Sheets/Slides/PDF count as open once handed over.
  Packaging: env-driven entrypoint (ONLYOFFICE `JWT_*` names + `DOCSERVER_*`), all four dists in the image, `install.sh` (dry run,
  rerun, rollback, self-rollback on a failed health check), `profiles/hrm.env`, `rehearse-dropin.sh`.
- Measured: `ci.sh docserver` exit 0 (fmt, clippy `-D warnings`, 96 tests passed 0 failed for docserver + fake-hrm + suite-serve,
  Playwright docserver 18 passed 1 skipped, bridge-docs 8 passed); `cargo test -p convert-api` 77 passed 0 failed; new tests
  `dropin.rs` (HRB as surveyed: no exp, body token, host-name allowlist, host rewrite to the internal listener named by host; 409
  before fetch; xlsx edit config gives a view) and `config.rs::the_docsapi_compat_mode`. Drop-in rehearsal on this PC (stand-in
  container with the old aliases and labels, Traefik v3.6 docker provider, fake HRB as `api`): **18 of 18 checks** (dry run changes
  nothing and prints no secret; install healthy under the old name; old container stopped, `restart=no`, disconnected; our api.js on
  the old Traefik route; rerun idempotent; stranger gets 403 on `docs-server:80`, port 80 unpublished; browser docx open/edit/
  save/close: statuses 6 and 2, stored hash = downloaded hash; xlsx opens view-only in Sheets, nothing sent; no secret/token in our
  log; rollback restores name, aliases, restart policy and the stand-in's api.js; data volume kept). It found and we fixed: upstream
  TLS refused a names-only `trusted_proxies`; the Sheets page never reported ready. Old compose rehearsal still green (1 run).
  Image 403 MB on disk (was 193 MB).
- Not checked (needs the HRM test server, owner OK): the real ONLYOFFICE container's env/labels/aliases and HRB's real env names
  (CORS list, allowlist); public URL auto-derivation from a real `Host()` rule; uid 10002 file modes on Linux (the rehearsal ran
  install.sh from Git Bash, chown skipped); a real HRB save; a Traefik file-provider setup; HRM's compose recreating the old
  service. Owner decision open: non-docx editing, `dropin.md` D5 (b) view-only now vs (a) pass-through to the old ONLYOFFICE.

### Phase 5 R6: xlsx, pptx and pdf edit through the document server (2026-11-08; branch `p5-r6`, owner decision of the same day)
- Done: [docs/phase5/r6.md](phase5/r6.md). Round trip measured first (open, one edit, save, reopen; independent readers plus Office 16's
  own counts), then the Sheets, Slides and PDF host bridges got what the Docs bridge has: read-only open honoured (an edit is taken
  back at once, with a message), `opened`, `requestSave`, the `saved {ok}` acknowledgement (unsaved until the host stored it),
  `canDownload`/`canPrint`; carried patches gridcraft/0160, deckcraft/0160, pdfcraft/0122 (numbers free on phase-5/6/7). Server
  unchanged but for a comment; the wrapper no longer fakes `opened` for non-Docs editors. Installer and image entrypoint default
  `edit_types` = docx,xlsx,pptx,pdf (`--edit-types docx` keeps the old view-only behaviour). `dropin.md` D5 superseded.
- Measured: round trips with no lost content: xlsx 8 files / 1,917 cells (one auto-fit row height 15.75 -> 16.5 pt; calcChain and a
  hidden `_xlnm._FilterDatabase` not rewritten; Excel opened 16/16, counts equal; `sheets-cli check` 0 of 8 differ), pptx 6 files /
  20 slides / 40 shapes (thumbnail and one printerSettings part dropped; PowerPoint opened 12/12, counts equal), pdf 7 files (incremental
  update: original bytes a prefix in 7/7; signatures "changes permitted"). `ci.sh docserver` exit 0: fmt, clippy `-D warnings`, 97
  tests passed 0 failed (was 96; new `dropin.rs::xlsx_pptx_and_pdf_edit_and_save_when_edit_types_has_them`), Playwright docserver
  24 passed 1 skipped (was 18+1; 6 new R6 cases: edit/save/close with status 6 then 2 and the stored hash, and a view session, per
  type), bridge-docs 8, bridge-sheets 7 (new), bridge-slides 11 (was 9; 2 new, 1 changed for the acknowledgement), bridge-pdf 6
  (new). Page load to onDocumentReady / requestSave to onSaved: xlsx 1,445 / 930 ms, pptx 1,543 / 908 ms, pdf 1,431 / 465 ms (docx
  2,445 / 923 ms in the same run). `verify-queue.sh` gridcraft 59, deckcraft 32, pdfcraft 17 patches OK; `cargo xt register check`
  138 rows OK; `cargo xt i18n check-stale` OK (3 new rows, uz-Latn/uz-Cyrl/ru `reviewed:agent-opus:2026-11-08`).
  Web wasm brotli (`ci.sh sizes` exit 0, no budget raised): Sheets 4,959,688 (budget 4,966,771, headroom 7,083), Slides 9,515,833
  (9,524,217; 8,384), PDF 9,569,050 (9,599,462; 30,412); Docs unchanged (its dist copied from E:/office-target/p5/dist).
  Drop-in rehearsal (`rehearse-dropin.sh`, rebuilt images): **18 of 18 checks**; the installer printed `edit types
  docx,xlsx,pptx,pdf`; browser flow 3 passed: docx as before, and an xlsx and a pptx edit config each edited, saved (status 6) and
  closed (status 2) with the backend's stored object changed. Image 355 MB on disk.
- Not checked: real HRM files (corpus missing, O9) and the HRM test server; pivot tables, SmartArt, animations, XFA forms in the round
  trip; the browser save of the larger feature files (CI uses the repo fixtures); the Sheets and PDF export refusals (canDownload) in
  a browser (Print is checked; the export paths are reached only through the File tab with the mouse); no autosave every 30 s for the
  three new bridges (Docs has it; the task did not list it); size delta against a pristine phase-5 build not measured (absolute budgets
  only).

### Phase 5: drop-in fixes for the HRM test server's shape (2026-11-08; branch `p5-srvfix` from phase-5 80488c6)
- Why: the owner's `install.sh --dry-run` on the test server refused twice: no Traefik labels (a file-provider route to
  `http://docs-alias:80`), then a published 443 (loopback only). The backend downloads saved files from port 80 too.
- Done (`dropin.md` D2b, D3b, D7): **shared layout** (`internal_listen` = `listen`: the file route behind the internal peer gate merged
  into the public port, outside the proxy guard); entrypoint `DOCSERVER_PUBLIC_PORT`, health check on it. Installer: picks the
  shared layout when the route's port equals the rewrite target's; finds the file route itself (Traefik's file-provider files via
  `docker cp`, read only, hits printed, ambiguity refused); maps published 80 on any address, drops a loopback-only 443 with a note,
  still refuses 443 elsewhere; carries the memory limit over; takes a `docker save` tarball and checks the free space of Docker's
  data root first (4 x tar + 2048 MB); prints a `docker-compose.override.yml` (`profiles: ["disabled"]`) at the end of a real install.
  `profiles/hrm.env`: `ORIGIN_ENVS=FRONTEND_URL`. Opaque (`null`) iOS origin: rejected, D3b says what breaks and what it would take.
- Measured: `ci.sh docserver` exit 0 (fmt, clippy `-D warnings`, 98 tests passed 0 failed for docserver + fake-hrm + suite-serve, was
  97; new `suite-serve/tests/docserver.rs::one_shared_port_keeps_the_file_route_to_internal_allow`: through the proxy api.js 200 and
  `/ds/files/` 403 even with an `X-Forwarded-For` naming the backend, the backend 401 on the file route and 403 on the public routes,
  three other peers 403 on both; `config.rs` shared/colliding cases); Playwright docserver 24 passed 1 skipped, bridges docs 8, sheets
  7, slides 11, pdf 6. `cargo test -p convert-api` 77 passed 0 failed; clippy convert-api clean. (`outbound.rs::callback_round_trip_on_loopback`
  failed once under a parallel Docker compile: its median 889 ms against the 1000 ms bound; green in the CI run.)
  Drop-in rehearsal, two scenarios, rebuilt image (355 MB, R6 dists from `E:/office-target/p5-r6/dist`): **54 of 54** (labels 23,
  file 31; each browser run 3 of 3: docx save round trip with the stored hash, xlsx and pptx edit/save/close). The file scenario
  found the route and the public URL with no flags, chose the shared layout, gave 403 on `/ds/files/` through Traefik, from a
  stranger and on the loopback publish, saved through port 80, kept 512 MB, published only `127.0.0.1:18082 -> 80`, left Traefik's
  files byte-identical. A first run used the Oct 25 dists of `E:/office-target/p5/dist` and the xlsx case hung at "loading" in both
  scenarios (pre-R6 Sheets bridge, not the layout).
- Not checked: the real `/dynamic` (one `Host()` per route file or not decides whether `--public-url` is needed), `df` on a real Linux
  data root and a tarball load there (the branch ran in Git Bash: Docker's VM, not measurable), uid 10002 file modes, compose's
  reaction to the printed override, a real HRB save, the iOS opaque-origin case (no macOS/iOS).

### Phase 5 R6 follow-up: read-only blocked before it shows, 30 s autosave for Sheets, Slides and PDF (2026-11-08; branch `p5-ro`, owner request)
- Done: [docs/phase5/r6.md](phase5/r6.md) section 5. A read-only session can no longer change the document even for one frame: Sheets
  (gridcraft 0161 engine `Session::read_only` refuses before running and reports the commands disabled, 0162 no cell editor, no drag,
  no header/footer or text box editor), Slides (deckcraft 0161 `Session::edit` and the drawing tools ask the command hook, notes/outline
  fields inert; 0162 the host policy refuses every change), PDF (pdfcraft 0123 engine view-only document: `editable()` false, `apply`/undo/
  redo/revert/unlock refuse; 0124 set before the first draw), Docs (wordcraft 0152: the read-only protection of 0127 let ranges everyone
  may edit take typing and always allowed Stop Protection and Undo; now refused before they run). The R6 per-frame take-back stays only
  as a safety net that logs `read-only safety net`; no test saw it fire. Autosave: gridcraft 0163, deckcraft 0162, pdfcraft 0124 post
  `<prefix>:autosave` every 30 s while dirty, not read-only and no save pending (Docs' logic; the wrapper and server unchanged).
- Measured: `ci.sh docserver`: fmt, clippy, 97 cargo tests passed (unchanged), Playwright docserver 24 passed 1 skipped, bridge-sheets
  47 passed (was 7), bridge-slides 32 passed (was 11), bridge-pdf 35 passed (was 6), bridge-docs 10 passed (was 8; case 10 failed in the
  CI run on the status bar's refusal text being counted as a pixel change, the spec now compares the frame above the status bar and the
  whole spec passed on rerun). Every read-only input path has an editable control case that does change pixels and dirty. Engine tests:
  gridcraft `a_read_only_session_refuses_edits_but_allows_view_commands`, deckcraft `office_hook` 4/4 (1 new), pdfcraft
  `view_only_refuses_every_change_and_keeps_viewing`, wordcraft `protect::tests` 23/23 (1 new). `verify-queue.sh` all OK (wordcraft 31,
  gridcraft 62, deckcraft 34, pdfcraft 19); `cargo xt register check` 146 rows OK; `cargo xt i18n check-stale` OK (no new UI strings).
  Web wasm brotli (`ci.sh sizes` exit 0, no budget raised): Sheets 4,961,381 (+1,693 since R6; headroom 5,390), Slides 9,519,321
  (+3,488; headroom 4,896), PDF 9,576,628 (+7,578; headroom 22,834), Docs 9,858,596 (headroom 13,052). Sheets and Slides are now within
  6 KB of their budgets: the next code added to either will need a budget raise under the web/budgets.toml rule.
- Not checked: browser cases for the Sheets comment, header/footer and text box editors, the Slides outline view, drawing-tool preview and
  side-pane drafts (alt text, SmartArt lines, comment replies still take typed text into the field; nothing reaches the presentation), PDF
  page operations by menu, bookmarks, attachments, properties, security, redaction, OCR, flatten, Combine and links (all end in the refused
  `apply` or a false `allows_*`), Docs Stop Protection in a browser (engine test only); real HRM files; read only inside the docker
  image (the rehearsal's browser flow edits).
- Merged 2026-11-08: phase-5 0dff431, phase-6 741a88f (wordcraft/0152 regenerated on the bridge's `recovered` line), phase-7 12a0ef4 +
  c43a384 (gridcraft 0162/0163 and deckcraft 0162 regenerated: the plugin bridge's `sync` returns its wait flag and takes `ctx`; the
  Slides host policy refuses read-only first, then falls through to the plugin lock). Docker rehearsal on phase-5 with a rebuilt image
  (dists of `E:/office-target/p5-ro/dist`): **54 of 54** (labels and file scenarios). The test server's earlier dry-run tarball
  (sha256 2eabb2b7...) predates this work: build a new one before a real install. Docs still
  autosaves while a save is pending (the three new bridges skip that period).

### Phase 5 D3b: opaque (`null`) host origin switch, off by default (2026-11-08; branch `p5-opaque`, owner decision of the same day)
- Why: the HRM mobile app ships on iOS too and its code is not changed for us; its WKWebView may give the host page an opaque origin,
  which shows an empty editor today. So a server-side switch, set at install (`dropin.md` D3b).
- Done: `[docserver] allow_opaque_origin` (default false; `config.rs`, start warning when on); entrypoint `DOCSERVER_ALLOW_OPAQUE_ORIGIN`
  (1|0, anything else refused); `install.sh --allow-opaque-origin` / `--no-allow-opaque-origin`, an `opaque origin` line in the plan,
  a plain rerun keeps the running container's value. When on, exactly three changes: no `frame-ancestors`/`X-Frame-Options` on
  `/ds/editor` and the four editor pages (`routes.rs` `frame_csp`, `suite-serve` `editor_csp_for`; convert-api's baseline now honours
  the empty `X-Frame-Options` beside a route CSP without `frame-ancestors`); `editor.js` served with `OPAQUE = true` (accepts `#o=null`,
  hears only `window.parent` at `"null"`, posts with `*`); `null` a frame origin of every provider at session create, after the
  signature. What the mode weakens is listed in D3b (frame check off for every page, leaked config tokens usable from any page,
  standalone editors frameable, `*` messages to the parent; the token signature stays the gate, no cookie or ambient authority).
- Measured: cargo (docserver + fake-hrm + suite-serve) 102 passed 0 failed (was 98): `auth.rs::an_opaque_origin_needs_the_switch_and_still_a_signed_config`
  (off 403; on 200 with a good token, 401 with a wrong signature, expired or unsigned config, 403 for other origins and `NULL`/`null `,
  no `Set-Cookie`, session routes 401 without the bearer), `suite-serve/tests/docserver.rs::allow_opaque_origin_off_changes_nothing`
  (every route's headers and body identical to a config without the key), `..._on_drops_the_frame_rule_of_the_framed_pages_only`,
  `routes.rs` CSP and editor.js unit tests, `config.rs` default; convert-api header test. Playwright `docserver.spec.ts` "D3b"
  (Chromium as the WKWebView stand-in; host as a `data:` frame and as a sandboxed `allow-scripts allow-same-origin` frame): off,
  "Refused to frame ... frame-ancestors", no `/ds/session`, no session, host stays `loading`; on, open, edit, save (status 6, stored hash),
  close (status 2), config origin `"null"`. Measured once: a `sandbox="allow-scripts"` srcdoc host fails in both modes (the wrapper
  inherits the sandbox, CORP refuses its own files). Entrypoint mapping checked in a throwaway container (unset/0 -> false, 1 -> true,
  `maybe` refused). `ci.sh docserver` (one run): clippy `-D warnings` clean, cargo 102 passed, bridges docs 10, sheets 47, slides 32, pdf 35 passed; the
  docserver spec 24 passed 1 failed: fmt was not yet applied, and the D3b close was polled for 5 s under the parallel load. Both fixed (close
  poll 30 s); then fmt clean, clippy (docserver, fake-hrm, suite-serve, convert-api) clean, the docserver spec alone **25 passed 1 skipped**
  (was 24 + 1), D3b close -> onClosed 405 / 390 ms. convert-api 77 passed (the baseline header test has the new case).
- Not checked: WebKit/WKWebView itself (no macOS, no iOS device): whether the HRM app's page is opaque and whether WKWebView then frames
  our pages is the WP17 device check. The installer path (`rehearse-dropin.sh` file scenario, new `opaque` step: plan line, container
  env, headers through Traefik, a rerun keeps it, `--no-allow-opaque-origin` undoes it) is written but not run: another agent's
  rehearsal held the shared `dropin-*` names and the `hoshiya-docserver:rehearsal` tag. Found on the way: `--edit-types` is not kept
  by a rerun (it goes back to the default); the switch is. Fixed after the merge: a rerun keeps it too (rehearsal check).
### Phase 5: no edit lost when the host page goes away (2026-11-09; branch `p5-save`, from phase-5 4a8969df)
- Why (measured on the HRM test server): the editor wasm took 14-70 s over the owner's link, the host's 15 s not-ready timer offered
  Retry and restarted the download; later a Save followed 1.4 s by Back was aborted (Traefik 499), no callback went out, the edit was lost.
  The server kept a recovery copy (`autosave.bin`) but never used it, and a session whose frame vanished never ended.
- Done: a close promotes a recovery copy that is newer than the last version and differs from it (sha256, the save-body checks, sealed
  at rest) to a version, then sends status 2 with it (same bytes: 4). An edit session pings every 60 s; one idle for `close_idle_minutes`
  (5) is closed by the server the same way (the 15 s retry loop); the sweep never deletes an open edit session first; a crash between
  promotion and callback is the owed status 2 at start. The wrapper closes on `pagehide` with a keepalive request, sends saves <= 60 KiB
  as keepalive (and then leaves the close to the idle close), emits `onAppReady` when the session and app frame exist, passes
  `&lang=` and `&toolbar=full` to the app. Provider `ui_lang` (installer `--ui-lang`, kept on a rerun; the host profile: `uz-Latn`)
  overrides `editorConfig.lang`. History is a 26 px icon button over the app (no row). No new UI strings (the button uses `hist.nav`).
- Measured: `ci.sh docserver` Rust tests all green (outbound 23, incl. promotion, idle close, restart, sweep ordering, ui_lang), clippy
  clean; docserver.spec 31 passed + 1 fixed afterwards (the History opener had no text any more; the History cases rerun 3/3); bridge
  docs 12, sheets 53, slides 36, pdf 39. Host removes the frame without a save: status 2 with the bridge's autosave (first autosave
  29,971 ms after dirty), the typed text in v2. Save then remove at 4 KB/s upload: status 6 arrives; with keepalive off the same case
  loses the save (checked). onAppReady 8.3 s before a held-back (8 s) editor download ends. History button: the pixels under it are flat
  (spread 0) in docs, sheets, slides and pdf, uz-Latn and ru, 1366x768 and 1600x900. Docker drop-in rehearsal (rebuilt image 404 MB):
  **67 of 67**, incl. the profile's uz-Latn on install, `--ui-lang ru` kept by a plain rerun, the drop-in wrapper speaking uz-Latn.
- Not checked: the apps honouring `&lang=`/`&toolbar=full` (their teams' branches; the dists used here are p5-ro's); a host calling
  `destroyEditor()` or `close()` while a save is still on its way can still close before the save is stored (the save then fails);
  a save over 60 KiB is not keepalive (only the last recovery copy survives a leave); the real HRM page.

### Phase 5: an edit made just before the host's Back is kept (2026-11-10; branch `p5-flush`, from phase-5 a54fcd10)
- Why (measured on the HRM test server with Playwright): HRM's Back runs `destroyEditor()` and empties the container at once. Type, Back
  after 300 ms: callback status 4, the edit lost (the debounced autosave comes 3 s after the last edit, at most 30 s apart while typing).
  The wrapper's `pagehide` closed the session; the app's own handover (a postMessage) went to a parent already being torn down.
- Done: each web bridge sets `window.__hostFinalCopy()` (wordcraft 0164, gridcraft 0174, deckcraft 0177, pdfcraft 0131): the bytes the
  autosave would send when there are changes not handed over yet, else null (clean, read only, already handed over); a pull counts as a
  handover. The wrapper (`editor.js` `closeSession`) pulls it synchronously in `pagehide`, `docs:close` and `close()` (or takes an
  autosave whose request is still on its way) and sends it in the keepalive `/close` itself. Over 60 KiB: a live close sends an awaited
  `/autosave` first; `pagehide` closes without it (code comment `shortcut:`, the 64 KiB keepalive budget). The server stores a close body
  as the recovery copy and closes in one locked step (`close_with`): status 2 with it, the same bytes as the last version give 4; the
  autosave checks apply (edit permission, size cap, package/PDF check); a refused body is left out and the close goes on. Audit: `close
  {final_copy: none|kept|refused}`, the version `save {from: "close"}`.
- Measured: docserver.spec (local, two sites, real TLS): "HRM Back 300 ms after typing" and "tab close 300 ms after typing" (Docs:
  callback status 2, v2 holds the text typed 300 ms before, no `/autosave` request), "HRM Back 300 ms after an edit" for xlsx (the typed
  cell kept), pptx, pdf: status 2 each. Control with the pull disabled in editor.js: both Docs cases fail (no status 2 callback, the edit
  lost), as on the test server. Playwright `page.close()` in headless Chromium runs no `pagehide` at all (probed): the tab-close case uses
  CDP `Page.close`. Pull time in the app (headless Chromium, swiftshader, median of 3, two runs; bridge-docs.spec `13`): 10 KB copy 2.7
  ms, 42 KB docx 15.4 ms, 140 KB 59.5-70.3 ms, 518 KB 144-153 ms. Rust: outbound.rs
  `a_close_with_the_final_copy_keeps_it_and_leaves_out_a_bad_one` (status 2 with the bytes, same bytes 4, damaged and over-cap bodies
  refused with the close kept, view session ignored, no content in the audit), author.rs
  `a_close_with_the_final_copy_stamps_the_last_modifier` (after the p5-author merge). Final CI after merging phase-5 f6428f02: `ci.sh
  queue lint i18n` ok; `shell sizes` ok (brotli/budget: docs 9,858,891/9,871,648, sheets 4,997,623/5,032,307, slides 9,552,352/9,556,985,
  pdf 9,573,509/9,599,462; no budget changed); docserver job: Rust all green (docserver lib 25, auth 12, author 4, dropin 2, history 8,
  outbound 24), clippy clean, bridges docs 15, sheets 55, slides 38, pdf 40; docserver.spec in the job 36 passed 1 failed: the xlsx Back
  case typed R6's `zz` into the cell R6 had just stored, so its bytes equalled the last version and the close was rightly status 4; the
  case now types its own value, then docserver.spec 37 passed 1 skipped. Docker was not running for the job (infra); started, rehearsal
  images rebuilt and `rehearse.sh 3`: 3 of 3 runs green (an earlier run on the pre-merge tree failed once with "api.js failed to load"
  before any edit; not reproduced). `outbound.rs::callback_round_trip_on_loopback` (median < 1000 ms) failed twice under load on the way
  (1,277 and 1,576 ms; 785 ms alone).
- Not checked: the real HRM page and the test server (the fix is measured on the local two-site harness only); a copy over 60 KiB on
  `pagehide` (docx above ~60 KB: only the last autosave, at most 30 s old, is kept; `fetchLater()` would lift it); the over-60-KiB path of
  a live `close()`/`docs:close` (awaited `/autosave`, then close) has no test; Sheets: text still in the cell editor (not committed with
  Enter) is not in the workbook, so neither the autosave nor the pull has it; Slides: the pull serializes `d.doc` as the autosave does (an
  uncommitted text composition is not in it); pull times for xlsx/pptx/pdf were not measured; the pull blocks the closing page for its
  time (144 ms at 518 KB here, more on slow phones).

### Phase 5: the idle close no longer overwrites a newer save (2026-11-11; branch `p5-idle`, from phase-5 7487c56b)
- Why: `rehearse.sh 5` failed on run 4 (the CI rehearsal on run 3): the compose spec's reopened session saved (status 6) and was never
  closed; the server's idle close, 5 min later, sent status 2 with that same copy into the next run, after the next session had saved
  (docserver log `6, 2, 2`). Real case: save in one tab, leave it, edit the file in a new session (HRM: a new `key` per open); the old
  session's idle close overwrote the newer file. Data loss.
- Done: the idle close (`sessions.rs` `close_locked`, `idle`) sends 2 only with data the provider has not acknowledged (a promoted
  recovery copy, or a save whose 6 was not answered `{"error":0}`), else 4. The explicit close keeps 2 after a save: the public callback
  page does not say what follows a forcesave with no change, and its "Saving file" page keeps forcesave versions out of the history, so
  the close's 2 is the version of record (compat doc, status 2 row). The compose spec closes its reopened session (6, 2 asserted).
  `callback_round_trip_on_loopback` bounds the callback alone (status 6 + harness download + answer), the whole save is reported only:
  timed inside the handler under load, the store (synced version, session, history, audit writes) took 0.35-2.8 s per save.
- Measured: outbound.rs `the_idle_close_never_resends_a_copy_the_provider_already_has` (fails without the fix: status 2, not 4);
  docserver Rust tests all green (lib 25, auth 12, author 4, dropin 2, history 8, outbound 25, wp14 6; fake-hrm 14; suite-serve 16),
  clippy -D warnings and fmt clean; docserver.spec 37 passed 1 skipped; round trip: callback median 52-62 ms, whole save 360-500 ms
  (3 runs). Images rebuilt, `rehearse.sh 5` 5 of 5, and again with KEEP=1: 30 callbacks (1, 6, 2 for each of 10 sessions), none more
  after 400 s idle.
- Not checked: the real ONLYOFFICE server's status after a forcesave with no change (no instance here; clean-room); the idle close's 4
  in the rehearsal (only the Rust test drives it); an old session with unacknowledged data still sends 2 when idle, and an explicit
  close of an old tab after a newer session saved still sends its 2 (both as ONLYOFFICE with a key per open; we cannot tell two
  sessions are on one file).

### Phase 5 R6 follow-up 2: the "Not checked" list of read-only closed (2026-11-09; branch `p5-ro2`, from phase-5 5e34f7d7)
- Done ([docs/phase5/r6.md](phase5/r6.md) section 5, follow-up paragraph): deckcraft 0163 (Slides: the Format Shape, Background, Animation and
  SmartArt Text panes drawn disabled while the hook bars edits; the comment reply box, selection-pane and section renames inert; every
  `widgets::spinner` number box inert through a per-frame context flag; the reply box no longer widens its resizable pane on every repaint,
  which had moved the slide under the tests) and gridcraft 0164 (Sheets: a form dialog whose command a read-only session refuses is not
  opened, chart/table-name fields and the reply box inert, no sheet-tab rename, no header/footer hover frame or "Add header" hint). PDF needed
  no change (bookmark rename and its menu are offered only when editable; Prepare a form refuses with its own message). `0950` of both
  engines regenerated for the new lines. No new UI strings.
- Measured (dists built from this branch; docs and pdf dists unchanged from p5-ro): bridge-sheets 53 passed (was 47), bridge-slides 36
  (was 32), bridge-pdf 39 (was 35), bridge-docs 12 (was 10); every read-only case has an editable control. Against the p5-ro dists the new
  Slides comment-reply and Format Shape cases and the Sheets note-dialog and header cases fail (so they test something); the Sheets text box
  and the Slides outline and tool cases pass on both (0161/0162 already held them). Docker rehearsal rebuilt image (374 MB, these dists):
  **54 of 54**, the browser flow now also opens a docx, an xlsx and a pptx with `mode=view` (labels and file scenarios, 6 of 6 each): no
  dirty mark, no save callback, the backend's object unchanged. `verify-queue.sh` deckcraft 35, gridcraft 63 OK; `cargo xt register check`
  148 rows OK. Web wasm brotli (`ci.sh sizes` exit 0, no budget raised): Sheets 4,954,975 (headroom 11,796), Slides 9,519,606 (headroom
  4,611), Docs 9,858,596, PDF 9,576,628.
- Not checked: the SmartArt Text pane in a browser (the web UI has no way to open it; inert by code only); the Slides Alt Text and Size and
  Position dialogs, which still open in a read-only session (whether their fields show typing is unchecked; nothing reaches the presentation);
  the Sheets name box; PDF page operations by menu, attachments, security, redaction, OCR, flatten, Combine (end in the refused `apply`);
  real HRM files.

## Phase 4 fix (2026-10-29): O13 taken, the default Latin alphabet is 2026
- Owner decision: the Cyrillic to Latin default is the new (2026) alphabet. The 2026 law is **not yet in force**; the earlier
  plan was to switch on its entry-into-force date, and the owner chose now (rules.md 15.4).
- `DEFAULT_LATIN_ALPHABET` = `Y2026`; docs-translit, wasm/npm, converter CLI/API/page follow it. `suite-core` settings default
  is 2026; a stored "1995" stays. m6 closed. Explicit `1995` still works everywhere. Branch `p4-alphabet`, merged (9123a0e).
- **Corrected the same day.** `Y2026` as the default also made ->Cyrillic read 2026 only: "Toshkent shahri, Oʻzbekiston,
  choy" became "Тосҳкент сҳаҳри, Оъзбекистон, choy" (Docs, converter, npm; never in an MSI). Now `DEFAULT_LATIN_ALPHABET` =
  `Auto` and the new `LATIN_WRITE_DEFAULT` = `Y2026`: ->Latin writes 2026, ->Cyrillic reads each paragraph as 2026 if it
  has ö ğ ş ç, else 1995 (test `the_default_reads_1995_and_2026_latin_and_writes_2026`).
- Sheets and Slides wrote 1995: `tools.translit*` took `Y1995` when `latinAlphabet` was left out, and their menus never
  pass it. Fixed by gridcraft/0147 and deckcraft/0148 (the suite default; a test each).
- Owner, 2026-10-29: "Doim 2026 rejimida ishlasin. 1995 kerak emas." The converter page's Latin alphabet menu is removed
  (its 4 catalog rows too); the CLI/API `alphabet` parameter stays for scripts. The suite-core `latin_alphabet` setting
  is read by no app (found here); left as it is.

### Phase 4 fix: PDF to Word moved the text (owner report, 2026-10-29; branch `p4-pdf2docx`)
- **Which pipeline made what.** The owner's three files (a 103-page document; nothing of it is in the repo). The "Downloads" docx is **Microsoft Word's own PDF Reflow**
  (`docProps/custom.xml` Converter = SolidFramework, the source PDF from PDF-XChange), not ours. `orig.pdf` is **our Docs
  export** of that docx (byte-identical when re-exported with the p1 build). The "Desktop" docx is **our native engine**
  (`pdfcraft_export::docx`, all 103 pages in the positioned layout: one VML text box per paragraph, 4,125 boxes). Its
  title is the PDF's file name, so it came from the **PDF app's Export to Word**. The converter (`convert --to docx`,
  CLI, API and the shell page) makes the same document apart from the title "input"; the pdf2docx sidecar was not used.
- **Where it breaks: in the docx itself**, and our rendering makes it worse. The boxes were exactly as wide as the PDF
  text and had no font. Our Docs fell back to Arial and Word to Calibri, which rewrapped the rows. Measured with
  `scripts/fidelity/fid/positions.py` (source words matched to the words of the docx printed again; pypdf, measurement only).
  103 pages, 20,991 words; before -> after:

  | printed by | words moved >3 pt | rows starting >3 pt off | source rows split | printed rows merged | words over words | dy p90 |
  |---|---|---|---|---|---|---|
  | our Docs | 43.4% -> 0.0% | 51.1% -> 0.1% | 59.0% -> 0.0% | 27.7% -> 0.0% | 18.3% -> 0.3% | 18.2 -> 0.02 pt |
  | Word 16 | 94.8% -> 0.0% | 43.8% -> 0.9% | 26.6% -> 0.7% | 25.5% -> 0.1% | 8.0% -> 0.2% | 17.5 -> 0.85 pt |

  Pages with any class over 5%: ours 102 -> 0, Word 101 -> 2. Control: Word against our Docs printing the source docx moves 2.5%
  of the words and splits 1.4% of the rows (the measurement floor).
- **Failure classes (before).** (1) No font in the runs, so the reader's default font is used (Arial in our Docs, Calibri in Word,
  where the PDF set Times New Roman, Calibri, Verdana and Arial). (2) Boxes as wide as the PDF text: a wider font wraps the row onto the row below, which gives the split
  rows and the text over text. (3) The rows of a multi-row paragraph were rewrapped at the reader's line height: merged
  rows, and table rows 10 and 11 run together. (4) The first baseline sat about 3 pt low in Word (single spacing, not the
  PDF's ascent).
- **Fix: carried patch `pdfcraft/0001`** (5 files, +227/-19; REGISTER row). edit: `ReadingBlocks` also returns its lines.
  engine `compare.rs`: export blocks carry the font family and each visual row's text and box. export: `family()`
  (`ABCDEF+TimesNewRomanPS-BoldMT` becomes Times New Roman; symbol fonts are left out) and `w:rFonts` on every run (flow pages too).
  A new `upright_text_box` writes one box per paragraph with one paragraph per row: left indent = the row's x, exact line
  height = the distance between row bottoms (Word and our layout keep the descent below the line), and the box reaches the
  page's right edge. Rotated text keeps the old path.
- **Tests:** export `font_family_comes_from_the_pdf_font_name` and `word_positioned_rows_keep_their_font_place_and_line`
  (31 of 31 pass); engine `office_export_keeps_a_paragraphs_rows_and_font_family` (14 of 14 `export` tests pass);
  `test_fid.py` Positions self-test.
- **New fixture `p2d_letter`** (`gen_pdf2docx.ps1`, Word-made, synthetic: a two-row centred title, an 11-row numbered
  table, Calibri, Verdana and Arial paragraphs, a stamp over the signature). It reproduces the owner's classes. Before ->
  after, our Docs: moved 20.4% -> 0, rows split 60% -> 0, merged 43.5% -> 0, text over text 15.3% -> 0. Word: moved 4.5% -> 0,
  split 10% -> 0. `p2d_overimage`, our Docs: moved 15.4% -> 0, split 40% -> 0; Word: split 23.1% -> 0.
  `run.py` now records `positions` for every pdf2docx output. Flow-mode fixtures (latn, cyrl, widths): the paragraph and
  table counts, text similarity and column widths are unchanged; only `w:rFonts` is added.
- **Remains:** (a) vector graphics (table rules, lines) are not exported, so positioned pages lose their table borders;
  (b) cells joined into one row ("10" and the cell text) get one space instead of the column gap; (c) the release `pdf.exe` /
  MSI were not rebuilt (only `pdf-cli` at `E:/office-target/p4-pdf2docx/pdf/release`); (d) LibreOffice not measured
  (Docker daemon down); (e) the owner should re-run Export to Word on the real file after the next MSI.
- **Docs tables made in Word (owner finding 2026-10-29, branch `p4-tables`, not merged):** `corpus/` is still missing (O9), so Word 16 built 29 synthetic table documents through COM (`scripts/fidelity/gen_word_tables.ps1`; the files stay on E:) and `scripts/fidelity/tables.py` compared our rendering with Word's PDF (table rule positions, pixels) and the saved docx with the source (markup counts, Word's re-render). Before: 35 of 237 column rules and 18 of 257 row rules more than 3 pt off; after: 2 and 9 (cell spacing, one letter). Fixed by **wordcraft/0130** (AutoFit: spanning cells share by grid width; compatibility mode <= 14 percentage/AutoFit room adds the cell margins; a table with its own `w:tblW` past 1.5x the column is no longer squeezed) and **wordcraft/0131** (`w:tblCellSpacing` kept on save; partial `w:tblCellMar` sides keep the style's/table's instead of 0). Word's re-render of our saved files: 21 of 23 at 0.00 % before, 29 of 29 after. Golden check without Office: `tables.py --golden ci/fidelity/tables-golden.json` (11 hand-written fixtures; before 7 fail, after 8 ok + 3 known gaps), in the nightly `fidelity-directions` job. Not fixed: cell spacing layout, Word's split for `tblW` auto tables past the column, and **the web editor's Times New Roman substitute (Source Serif 4): rows taller, a 60-row table 4 to 5 pages** (owner decision: metric-compatible fonts do not fit the docs headroom). Not measured: `.doc`/`.rtf` (Word's SaveAs to them hangs headless here). An owner file (best a `.doc`) is asked for. Details: `docs/phase4/tables.md`.

### Phase 4: one theme setting for the shell and the four apps (owner, 2026-10-29; branch `p4-theme`)
- **Owner decision (design-system.md, Revised):** each app keeps its own look; one global theme, changed in one place; our
  own additions follow the host app's design language.
- **Setting:** `settings.toml` `theme = "system" | "light" | "dark"`, default `system` (`crates/suite-core/src/settings.rs`,
  `Theme`). The desktop host lets the page read and set it (`PAGE_SETTINGS`). The shell header has a select next to the
  language; the shell page applies it at once (`<html data-theme>`, logo, accent; `system` follows the OS live). On the web
  the browser keeps it (`localStorage` `sh.theme`).
- **Apps (read once at start, native only, never in tests):** Docs wordcraft/0132 (`Appearance`), Sheets gridcraft/0148
  (`system_theme`/`dark`), Slides deckcraft/0149 (first frame; Slides has no system mode, so `system` = the OS answer at
  start, not live), PDF pdfcraft/0115 (`ThemePreference`). Every value wins over the app's saved one, `system` included;
  an in-app change holds for the session. With no file the default `system` applies, so Slides and PDF (upstream default
  Light) now follow Windows. Details: shared-services.md 6.2.
- **Our additions:** inventory in design-system.md section 10. The simple toolbars, the Docs tab and icons, translit menus,
  logos/attribution already use the host's `Tokens`, radii and fonts; History opens the shell page, which now follows the
  setting. Nothing in the apps needed a change.
- **Not done:** the web editors do not get the setting (the shell passes only `?lang=`; Docs/Sheets follow the browser,
  Slides/PDF their own default). Phase 5's web History panel (`services/docserver/static/editor.js`,
  `web/shell/history-ui.js`) was not touched; it needs: `data-theme` on its page root from the setting (shell.css keeps an
  OS fallback for a page without it, so a merge forward does not break it), its hard-coded status colour `#555`
  (`editor.js:21`) moved to `var(--muted)`, and, to follow the Docs look rather than the shell's, Docs' accent and
  radii (wordcraft `theme.rs`: accent `#3B5BDB`, widget radius 4, window radius 8, body 12.5 px) instead of the shell's
  brand blue and 16 px type.
- **Checked:** suite-core 42 tests (new `theme_defaults_to_system_and_a_bad_value_costs_only_the_theme`), suite-desktop lib 22,
  clippy clean on both; overlay tests with the overlay ON: Docs 8/8 (+22 upstream theme tests), Sheets 8/8, Slides 12/12
  (+ upstream `preferences_cancel_restores_the_settings_done_keeps_them`), PDF 7/7; shell smoke 16 passed, 11 skipped (no
  editor dists here). Real debug windows, isolated profile (APPDATA in a temp folder), Windows set to light: Docs `ui.theme`
  = dark with `theme = "dark"`, and light with `theme = "light"` over a `ui.json` that saved dark (its `uiScale` 150 was read,
  so the file was loaded); PDF `ui.state` Dark/Dark and System/Light; Sheets and Slides screenshots dark, light/system.
  Screenshots: `docs/phase4/evidence/theme/` (11, synthetic, no user or file names). Docs was built without `office-ext`
  (the domain build failed on a transient file error under disk pressure), so its Hujjat tab is not in the picture.
- **Not checked:** clippy on gridcraft/deckcraft/pdfcraft (the hooks mirror the Docs code, which is clean); a Windows switch
  to dark while `system` is chosen (the owner's PC; upstream follows it live in Docs, Sheets and PDF); the MSI.
### Phase 4 fix (2026-10-29): clear print pages, Times New Roman 12 for new documents, A4 for uz/ru (branch `p4-print`)
- **Owner decisions taken** (`design-system.md`, "Revised"; Q6): each app keeps its own look; new documents are Times New
  Roman 12 pt (Slides: typeface only); A4 is the first paper for uz/ru, Letter stays for en-US. Plus the WP23 print finding
  (`usability.md` finding 3).
- **Docs (wordcraft/0135, 0136).** `blank_document` sets the docDefaults run font and size and the body theme font to Times
  New Roman 12 (so Design › Fonts and Style Sets keep it); templates and opened files keep theirs. `Session::set_interface_a4`
  (set by the UI from the interface language, like the editing language) makes new documents A4 for uz/ru whatever the
  system region says; a region chosen in Options still wins; the untouched start document is resized at once. Print has its
  own page (it shared `export_page` with Export): Print (accent, first) and Save as PDF, then printer/copies, Pages with a
  labelled "Odd or even pages" list, Orientation, Scale to paper size, Print Markup and the page count; the 11 format buttons
  stay on Export only. ru rows (upstream `ru.tsv`) and uz rows for the three untranslated settings, the Recover page and the
  title bar's account tip. The placeholder author "WordCraft User" becomes the Windows account name (`USERNAME`) until
  Options sets one, as Word does (web: no account name, the placeholder stays).
- **Sheets (gridcraft/0151, 0152).** A new workbook's Normal font is Times New Roman 12 pt with 21 px (15.75 pt) default
  rows, the height Excel gives that font (Calibri 11: 20 px / 15 pt); its sheets are A4 when the number locale (= the
  interface language) is ru/uz; the desktop Book1 is made after the prefs so it knows the language; Insert Sheet takes the
  active sheet's paper and the Normal font's row height; autofit compares sizes with the workbook's Normal font instead of
  a fixed 11 pt (so 12 pt text keeps the default row; 11 pt workbooks unchanged). Consequence: 43 rows per Letter page
  instead of 45 (three print tests updated). The Print page shows the sheet's paper and orientation above Page Setup.
- **Slides (deckcraft/0152, 0153).** `file.new` without a theme: the default theme with Times New Roman major/minor Latin
  fonts, sizes unchanged; a chosen theme/template keeps its fonts. Notes/handouts print on A4 for uz/ru; a dim line explains
  why paper and orientation are greyed for full-page slides.
- **PDF (pdfcraft/0118, 0119).** Add Text's default style and new text boxes, typewriter text and callouts are standard-14
  Times 12 (`/TiRo`, nothing embedded); a box keeps the font its /DA names on colour/size changes, and the appearance,
  rich text and refit measure with that font (Helvetica boxes are measured exactly as before). The Print dialog's first
  paper and Create › Blank page are A4 for uz/ru (a picked paper is kept for the session).
- **Font fallback where Times New Roman is missing (not changed, no font files added).** Docs web: the bundled Source Serif 4
  (not metric-compatible; see the tables entry above). Sheets: Liberation Serif or Tinos when installed (PDF export maps it
  to std-14 Times); the web grid draws with the bundled UI faces. Slides web: only Inter is embedded, so new decks show a sans
  face on the web. PDF: std-14 Times is drawn by hayro's own standard-font substitutes on every platform.
- **Tests (overlay on, debug).** wordcraft engine lib 787/787 (new `new_documents_are_times_new_roman_12_on_the_interface_paper`),
  doc 173/173, ui-egui lib 720 pass + 12 failing that are all in `ci/quarantine.toml` (brand overlay, catalog coverage);
  `the_print_button_shows_only_with_a_print_hook` rewritten (no export list on Print, labelled odd/even, Print Markup).
  gridcraft engine lib 306/306 (new `new_workbooks_are_times_new_roman_12_on_the_interface_paper`; three print tests moved to
  43 rows a Letter page and `/Times-Bold`) plus all 24 engine integration suites and model 26/26; ui-egui lib 103 pass + 1
  quarantined (brand). The gridcraft ui-egui integration suites were stopped: their debug binaries filled 30 GB of E: with
  under 7 GB left. deckcraft engine lib 332/332; ui-egui `backstage` 18 pass + 1 quarantined (new
  `new_decks_are_times_new_roman_and_print_on_the_interface_paper`). pdfcraft annot lib 46/46 (new `new_text_boxes_are_times_and_old_ones_keep_helvetica`; four /Helv asserts moved to /TiRo), ui-egui `print_ui`+`content_ui` 9/9 (new `first_print_uses_the_interface_paper_and_added_text_is_times`). `verify-queue` for the four
  apps, `cargo xt register check` (117/117 after the rebase on the p4-theme merge), `i18n lint` (0 errors), `check-stale` and `roundtrip` pass.
- **Evidence** (`docs/phase4/evidence/print/`, isolated profile, account name set to a neutral word): Docs print page uz-Latn
  and ru, new Docs document (Times New Roman 12 in the font box, A4 ruler), Sheets print page uz-Latn and ru ("A4 · Kitob"
  above Page Setup), new Sheets workbook (Times New Roman 12). Sheets ru still shows "Grayscale" in English (a gridcraft ru
  catalog gap, not touched here). Slides and PDF were not built or photographed (E: fell under 7 GB while other agents built; their defaults are covered by the tests above).
- **Not done / open:** (default column width: answered by gridcraft/0154, see the last section); sheets made by analysis tools (pivot, solver,
  forecast) keep Letter and 20 px rows; A4 follows the interface language, not the units (an en-US Windows with a uz
  interface still shows inches in Docs).
- **MSI rebuilt from phase-4 `2bb7c0d` (2026-10-29)**, with the alphabet (always 2026), recovery, tables, PDF->Word,
  i18n, shared theme and TNR 12 / A4 / print page fixes: `E:\office-target\p1\msi\hoshiya-0.1.0-x64.msi`, 201.8 MB,
  SHA-256 `139DA156…A548` (web release dists, MSVC release builds, shell dist, WiX all rc 0). Not install-tested here.
  Same version 0.1.0 and ProductCode as the earlier builds, so the installed copy must be uninstalled first.
  Merge checks: all four apps `cargo check --tests` on p4-print after its rebase; phase-5 suite-core 48/0 and
  suite-desktop 23+3/0 after the theme merge; wordcraft/0136 regenerated on the phase-5 queue (context lines only).
  Open: the Docs Recover page names an unsaved copy "Untitled draft" in English (engine `recovery/desktop.rs`, no i18n).
- **WP23: Docs Document Recovery pane (branch `p4-recover-pane`, 2026-10-29; wordcraft/0137, 118 patches, 118 rows).** After a crash Docs no
  longer opens File > Recover over the window: it opens the document as usual and shows a Word-style "Document Recovery" pane on
  the left (`panes.rs`): the app name from the overlay, one card per copy (name, "Version created from the last AutoSave", time in
  local time `dd.mm.yyyy HH:MM`, Open, Discard... with the old confirm), Close. Clicking the name or Open restores; the pane is
  marked offered when shown (0129 unchanged: once per copy, 7-day expiry, clean exit removes own copies). File > Recover has the
  same cards on the normal backstage background (no black page). The engine stores an empty name for an untitled document;
  the UI shows the translated "Document1" (old copies holding "Untitled draft" map to it). Local time: new crate
  `crates/local-stamp` (the one OS call, Windows `GetTimeZoneInformation` / unix `localtime_r`; app crates keep forbidding
  unsafe code; web shows UTC); it adds one package to the wordcraft and docs-domain locks (`lockdiff` 0 foreign). 3 new strings
  (ru in the carried `ru.tsv`, uz in `i18n/wordcraft.tsv`, `reviewed:agent-sonnet:2026-10-29`).
  Tests (overlay on, debug): ui-egui `recovery` 4/4 (the three old ones rewritten for the pane, new
  `recovery_names_and_times_are_shown_for_people`), engine `recovery` 18/18, `local-stamp` 3/3; `verify-queue.sh wordcraft`,
  `register check`, `i18n check-stale` and `lint` (0 errors) pass. Evidence `docs/phase4/evidence/recover/` (built debug exe,
  isolated profile, synthetic text, crash by kill then restart; uz-Latn and ru, light): pane and File > Recover page.
  Not done / not checked: the full ui-egui suite and clippy; Sheets/Slides/PDF unchanged; two or more cards and a named
  (saved-file) card were not photographed; the local zone's offset is read as of now (a copy from the other half of the year
  across a DST change is off by one hour); the web build has no pane test and shows UTC; dd.mm.yyyy is used for every language
  (no per-locale date order); the pane is not shown if the window starts with unsaved changes (as 0129 required for the page);
  Android and the web were not built.
- **MSI rebuilt from phase-4 `ab62c5a` (2026-10-29, late)** with the Document Recovery pane (wordcraft/0137):
  `E:\office-target\p1\msi\hoshiya-0.1.0-x64.msi`, 201.8 MB, SHA-256 `9B804E90…0646` (Docs web + MSVC rebuilt, shell
  dist, WiX all rc 0). It replaces the `139DA156…A548` build. Not install-tested here. wordcraft/0137 regenerated on the
  phase-5 queue; wordcraft `cargo check --tests` on phase-5 OK.
### Phase 4: the web editors follow the shell theme (owner, 2026-10-30; branch `p4-webtheme`)
- **Shell:** `web/shell/shell.js` opens each editor frame with `?host=parent&lang=..&theme=<system|light|dark>`. A theme change
  drops the idle editor frames (as a language change does), so the next open starts in the new theme; a frame with unsaved work
  keeps its theme until reopened (no live switch: the editors read `?theme=` once at start).
- **Editors (carried patches, category `brand`; 122 patches, 122 rows):** Docs wordcraft/0138 (`Appearance`, after the saved
  prefs), Sheets gridcraft/0153 (`system_theme`/`dark`, replaces the browser-colour default), Slides deckcraft/0154 (feeds the
  0149 first-frame hook; `system` = the browser's answer at start), PDF pdfcraft/0120 (`ThemePreference`, after the saved
  state). A missing or unknown value keeps today's behaviour. `theme_at_start` made `pub` in three overlays.
- **Tests (overlay ON, native):** Docs 9/9, Sheets 9/9, Slides 13/13, PDF 8/8 in `office_overlay_tests` (new: three
  `web_theme_param_keeps_own_on_garbage`, one `web_theme_param_is_validated`). Shell Playwright, new
  `the editor frame follows the shell theme`: with the existing language and theme tests 9/9 (`-g "theme|language"`; the full
  spec was not re-run). `verify-queue.sh all` and `cargo xt register check` pass.
- **Web wasm brotli (release, `ci.sh sizes`, budget = base_br + delta_br_max):** Docs 9,862,707 (budget 9,871,648, headroom
  8,941), Sheets 4,940,175 (4,966,771; 26,596), PDF 9,570,483 (9,599,462; 28,979), Slides 9,495,548 with the fonts (headroom
  about 28,700; the first Slides build lacked `upstream/craft-fonts` in this worktree and is not comparable). The p1 dists are
  older trees: Docs 9,862,588 -> 9,862,707 (+119 B), Sheets 4,942,108 -> 4,940,175, PDF 9,569,970 -> 9,570,483 (+513 B),
  Slides 9,482,519 -> 9,495,548 (includes the print patches merged since). The change itself is a few hundred bytes per app.
- **Evidence:** `docs/phase4/evidence/theme/web-{docs-dark,docs-light,sheets-dark,slides-dark}-via-shell.png` (blank
  documents; the Docs light shot ran with a dark browser scheme, so the param beat the browser). PDF web was not
  screenshotted (it has no blank document).
- **Not checked:** a live switch in an open editor; the MSI; a real-device browser.

### Phase 4: gate-review carry-over (branch `p4-carry`, 2026-10-31; F11, F16, F17, F20, F21)
- **F20 MSI upgrade:** `UpgradeCode` frozen in `brand.toml` (`{2EA291EA-35AE-59D4-AB3E-09D1AE815A49}`, the value the MSI the owner runs already has); `ProductCode="*"` and `MajorUpgrade AllowSameVersionUpgrades="yes"`, so the next build upgrades the installed 0.1.0 without an uninstall. Chosen over a growing version because the same-version flag also upgrades the copy installed today (same 0.1.0). Release versions still bump `brand.toml version`. Built `hoshiya-0.1.0-x64.msi` (201.8 MB, from the p1 MSVC outputs; not an install test); its ProductCode differs from the shipped one, UpgradeCode equal. `wix msi validate`: ICE43/ICE57 errors on five shortcut components (open), ICE61 warning (expected). Signing store default now `CurrentUser`; `msi-test.ps1` uninstalls in a `finally`.
- **F16/F17/F11/F21:** pipe squatter no longer stops a plain launch (test); cookie name per launch (tests); History IPC on a worker and a size-only listing (test); converter listeners before the first await (Playwright 13 passed, 1 skipped).
- **Tests:** `suite-desktop` 23 lib + 3 host + others, `suite-serve` 26, `suite-core history` 9, `office-brand` 4, converter spec 13/13 (+1 skipped screenshots), all green.
- **Not checked:** an install or upgrade of the MSI (elevation); a real WebView window for F11/F16; a 2 GB History document; the cause of the F21 host-run losses.
### Phase 4: translation gaps in Sheets, Slides and PDF, Slides tooltips, a test that was not broken (branch `p4-l10n`, 2026-10-31)
- **Cause of the Russian gaps.** Slides has no upstream Russian, Sheets only 398 upstream rows and PDF 2,318: every other row
  had an empty `ru` and showed English. Filled in `i18n/*.tsv` (status `reviewed:agent-sonnet:2026-10-31`, never an owner
  review; drafted by sonnet sub-agents against the glossary, MS Office ru terms): Sheets 447 rows, Slides 1,118 rows (the 15
  obsolete rows left empty), PDF 312 rows (incl. 8 three-form plurals). Placeholders and `\n` checked per row by script.
  Docs not asked for (43 rows still empty). uz-Latn and uz-Cyrl had no gaps (`cargo xt i18n coverage`: 1 todo row per app,
  none visible); the four Slides strings that say "DeckCraft" stay uz todo (brand text).
- **"Grayscale" in Sheets ru (and 493 other shared rows).** The shared row had ru only for Docs (`ru_apps = wordcraft`), a
  restriction that exists to keep the upstream round trip exact. `export.rs::resolve` now applies `ru_apps` only under the
  owner-only policy used by `roundtrip`; real builds lend the row to every app that has it (Sheets ru export 398 to 996 rows).
- **Slides tooltips.** Not in the catalog problem: the shared widget sinks (`icon_toggle`, `color_split`, swatches, gallery,
  shape/freeform/icon/warp/table-style tiles, selection pane buttons, thesaurus, corner hint) passed literals unchanged.
  Carried patch `deckcraft/0155` wraps them in `i18n::t`. Computed labels (preset names, icon names) translate only where the
  catalog has the text; not audited one by one.
- **`backstage::tests::every_page_renders` is a Slides (deckcraft ui-egui) test, not Docs, and nothing broke.** It passes with
  the overlay off (19/19 `backstage::tests`) and fails with the overlay on because it looks for the hard-coded "About
  DeckCraft" while the overlay shows the product name. It is the existing quarantine entry (`ci/quarantine.toml`,
  class brand-overlay, since 2026-10-18); the test is what is wrong, left as is.
- **Checks.** `cargo xt i18n lint` 0 errors, `roundtrip` OK, `check-stale` OK, `extract --dry-run` 0 changes, `check` OK,
  xtask i18n unit tests 59/59. Slides `office_overlay_tests` + `russian_*` 13/13 (overlay on).
- **Not checked.** Screenshots in ru/uz, quality of the 1,877 Russian strings by a native reader, PDF overlay tests, size
  budgets (no web app rebuilt), Docs.
## Phase 5: web History panel follows the host app's look (branch `p5-histlook`, 2026-10-30)
- Owner decision 2026-10-29 (each app keeps its design language), the to-do of `design-system.md` section 10. The wrapper (`services/docserver/static/editor.js`) sets `<html data-theme>` (`?theme=light|dark` on the wrapper URL, else `prefers-color-scheme`, followed live) and `<html data-app>` (`docs|sheets|slides|pdf`, the session's editor). `web/shell/history.css` holds one small variable set per app and scheme (`--accent --bg --fg --muted --card --line --on-accent`, `--r-widget`, `--r-window`, 12.5 px body) taken from upstream `theme*.rs` (Docs `#3B5BDB`/`#7A93F0`, radius 4/8; Sheets `#107C41`/`#5CC587`, 4/10; Slides `#D95A1A`/`#F26B1D`, 5/10; PDF `#1B63E0`/`#4B8BF5`, 6/10). Status text `#555` -> `var(--muted)`; the panel shadow is `--shadow` (.25 light, .6 dark). The shell's own History page has no `data-app`, so it is unchanged.
- Test: `docserver.spec.ts` "History panel takes the host app look and the theme": `?theme=` and OS fallback on the bare wrapper; over Docs and Sheets, light and dark, panel background, accent, widget radius and 12.5 px match the tokens. Whole spec 11 passed, 1 skipped (screenshots, needs `DS_SHOTS`); the screenshots test also passes. Screenshots: `docs/phase5/evidence/history-look/panel-{docs,sheets}-{light,dark}.png` (synthetic fixtures).
- Not done: Slides and PDF values are in the CSS but not run in a browser (no session for them in this spec); nothing in the embed path passes `?theme=` to the wrapper (`api.js` frames it with only `#o=`), so a host page gets the OS scheme; the shell/desktop does not forward its `theme` setting to the wrapper yet. Sheets editor did not send `ready` on the p5 dist in this run, so the test does not wait for it there. Slides accent `#D95A1A` on white is upstream's own (about 3.9:1 as link text on ghost buttons).
- **HRM theme (2026-10-30, after the merge of `p4-webtheme`).** The wrapper reads the host's `editorConfig.customization.uiTheme`
  (ONLYOFFICE names: `-dark` -> dark, `-light` -> light, wins over `?theme=` and the OS; any other name keeps them), applies it to
  its own pages and opens the editor with `?theme=` (the param the editors read since wordcraft/0138, gridcraft/0153,
  deckcraft/0154, pdfcraft/0120). `onlyoffice-compat.md` row `uiTheme` now says so. Test: the look test runs one session per
  theme through the fake HRM's `?theme=` with the OS set to the opposite, and checks `data-theme` and the editor frame's
  `?theme=`. Whole `docserver.spec.ts` 11 passed, 1 skipped (`DS_SHOTS`), with `DIST=E:/office-target/p5/dist` after
  `CI_DIST_ROOT=E:/office-target/p5/dist scripts/shell-dist.sh` (the default `p1/dist` shell has no `history-ui.js`: 9 tests
  fail there on its 404). Note: the fake HRM sends `theme-classic-light` unless asked for dark; if the real HRM does the same,
  its users get light whatever the OS, as in ONLYOFFICE. Not checked: the editor inside the wrapper actually drawing the theme
  (the p5 dist editors predate the four patches); the real HRM's value.

### Phase 5 WP9 web half: `_hrm_*` bookmarks and translit in the embed (2026-10-30)
- **Done.** `scripts/hrm-bookmarks/hrm_bookmarks.py --check ORIG SAVED [B1|T1]` reuses the CLI half's checker (name, zero-length vs
  non-empty, paragraph anchors, range text; T1 needs Cyrillic; it also fails when the saved file does not show the edit).
  `ci/web-smoke/docserver.spec.ts` serves the synthetic decree, letter and report as `files/3..5` of the fake HRM and runs, per
  document, B1 (Ctrl+End, type outside every bookmark, `requestSave`) and T1 (Alt+Q command search, `tools.translitCyrl`, save).
  The saved file is the server's `v2.bin`, asserted byte-equal (sha256) to the object the fake HRM holds after its callback.
- **Result: no bookmark lost.** 6/6 new cases pass (all 7 distinct names: `_hrm_desc_`, `_plan_`, `_sig_`, `_agree_`, `_rsmry_`,
  `_rtask_`, `_rcont_`; zero-length ones included, also in tables); 4-7 s each. Whole `docserver.spec.ts`: 17 passed, 1 skipped
  (`DS_SHOTS`), `DIST=E:/office-target/p5/dist`. No upstream patch needed.
- **Not checked:** real HRM decrees (H12); header/footer bookmarks (HRB uses body only); typing inside a bookmark range; the
  p5 dist docs editor predates the latest patches (0137, 0138); `ci.sh` job wiring (the spec runs inside the existing `docserver` job).
- **MSI rebuilt from phase-4 `a638aef` (2026-10-31)** with p4-carry and p4-l10n: `E:\office-target\p1\msi\hoshiya-0.1.0-x64.msi`,
  201.9 MB, SHA-256 `A5F72131…A6E2`. Overlay, web release, `ci.sh sizes`, MSVC release, shell dist and WiX all rc 0.
  Size headroom (brotli, under budget): Docs 10,005 B, Sheets 14,246 B, Slides 9,377 B (was about 28 KB before the Russian
  rows), PDF 30,230 B, shell 9,901 B. Slides now has under 10 KB left for the next catalog growth. UpgradeCode is the frozen
  one, the same as the owner's installed `9B804E90…0646` build, so it should upgrade in place; the in-place upgrade is not
  tested here.

### Phase 5: gate finding F14, history lock and "delete this document's history" (2026-10-31, branch `p5-f14`)
- Lock ownership (token in `.lock`, `Drop` and takeover never remove another process's lock), `.tmp` sweep already present (WP11), and a desktop `history_forget` message plus a confirm-guarded "Delete history" button in the shell History panel (reuses `Store::forget_id`). Details and tests: `docs/phase4/gate-review.md` F14.
### Phase 4: Russian for the Docs gaps (branch `p4-ru-docs`, 2026-10-31)
- `i18n/wordcraft.tsv`: the 43 active rows with an empty `ru` are filled (status `reviewed:agent-sonnet:2026-10-31`, never an
  owner review). 17 are names that stay as written (citation styles APA, MLA, IEEE..., paper sizes such as "Letter   8.50" ×
  11.00""); the rest are short Word-style Russian (levels, spelling/grammar hints, bookmark and page prompts, GOST sort modes).
  Empty active wordcraft `ru` rows now: 0. Uzbek and English rows untouched.
- **Checks.** `cargo xt i18n lint` 0 errors, `roundtrip` OK, `check-stale` OK, `extract --dry-run` 0 changes, `check` OK
  (8,236 rows), xtask i18n unit tests 59/59.
- **Not checked.** No native speaker has read these rows; no screenshots in ru.
- **MSI gate leftovers (F20), branch `p4-msi` (2026-10-31).** `wix msi validate` now has no ICE errors (was ICE43 x5, ICE57 x5): the five Start
  menu shortcuts moved to own components with an HKCU KeyPath. File and OpenWith components are byte-identical in the tables to
  the a638aef MSI, UpgradeCode frozen, `RemoveExistingProducts` right after `InstallValidate`, so an in-place upgrade should work
  (static check only). `build-msi.ps1` now checks the VC runtime imports with dumpbin on every build and has `-Out`. `msi-test.ps1`:
  recommended-handlers filter, `AppActivate` check, tracked PDF fixture. Test MSI (not installed) `E:\office-target\p4-msi-test\`,
  SHA-256 `DEC278B6...A887`. Not run: install, upgrade, the msi-test changes.
### Phase 5 XDFU gate fixes F18, F19 (2026-10-31, branch `p5-xdfu`, `ca62fce`)
- **Done.** Docs Save As onto the working path is no copy (case-insensitive on Windows); text stamping never refuses or deletes a copy (Docs, gridcraft 0143, deckcraft 0143 amended in place); Docs stamps the reported output path; deckcraft logs `<event>_unstamped` when no shape id is left; `mailings.finish` and the in-window editors added to the limits in `docs/phase4/hooks-docs.md`. XDFU stays off by default; F4, F5 and m13 still gate switching it on.
- **Not checked:** Sheets and Slides still compare the working path exactly (case); a real Windows drive-case run of Save As.

### Phase 4 fix: the Docs/Sheets window goes dark for a moment (owner MSI test, 2026-11-06; branch `p4-flicker`)
- **Cause.** All four editors return from `ui()` without drawing while new fonts are pending (named font families would
  panic), and eframe presents that pass in its default clear colour (12, 12, 12): Docs two frames at start, Sheets one at start
  and one after every font rebind (font scan done; each new style that adds a font name, e.g. font, bold, fill), Slides and PDF
  one at start. Not the renderer: DX12 on the UHD 630, no swapchain recreation, no window-mode change found. Fix: the gates call
  `ctx.request_discard` (egui redoes the pass at once with the fonts active): wordcraft/0140, gridcraft/0156, deckcraft/0156,
  pdfcraft/0121. Tests: wordcraft and gridcraft ui-egui `tests/no_blank_frames.rs` fail before (empty first frame; 1 shape after
  a font change) and pass after; gridcraft-ui-egui suite 726 pass, 1 quarantined fail; wordcraft-ui-egui 731 pass, 14
  quarantined fails plus `recovery_restore_uses_save_changes...` failing once under the parallel suite (passes alone 3 of 3).
- **Measured** (PrintWindow capture of the window, about 25 grabs/s, the PC locked, CPU busy with other builds; fixtures only):
  continuous near-black window after it is shown, before -> after: Docs 2.0-5.7 s (6 runs) -> 0 (6), Sheets 0.1-4.9 s (4) -> 0
  (3), Slides 0.2-0.4 s (3) -> 0 (3), PDF 0-14.6 s (5) -> 0 (5). Not seen: a mid-session dark frame on screen, before or after
  (30 s runs with 100-177 typed/clicked/font inputs); the test is the evidence there. Open: right after the window is shown
  the capture sees one black grab and then stalls 1.7-2.4 s in 3 of 6 Docs runs, while the app answers frames every ~0.1 s
  (control port): the screen in that gap is unmeasured; whole-screen effects and other GPUs not checked.
- **Follow-up (branch `p4-overlay`, 2026-11-06): the whole SCREEN goes dark, Docs/Sheets only; white I-beam everywhere, also over
  the ribbon.** Not settled; it needs the owner at an unlocked desktop. Measured on the locked PC: MPO is on (dxdiag "MPO
  MaxPlanes 3", RGB/BGRA8 formats; no `OverlayTestMode` value), `CheckHardwareCompositionSupport` = FULLSCREEN|WINDOWED on both
  outputs, `SupportsOverlays` true on DISPLAY1 (HDMI, 59 Hz) and false on DISPLAY2 (Dell, DP, 60 Hz); per-window composition
  mode not measurable (ETW needs admin, the app's swapchain is in-process). egui asks for the arrow over the title bar and
  ribbon and the I-beam over the page canvas (`spikes/p4-overlay/pointer_sweep.out.txt`), so an I-beam over the ribbon is not
  what the app requests. Backend switch exists without a rebuild (`WGPU_BACKEND`, `graphics.rs` `configure`): GL runs Docs
  and Sheets here; Vulkan crashes in instance creation (segfault). The 5-12 s DX12 probe at start was CPU starvation from
  parallel builds: 12-107 ms in-app and 60 ms in a spike when the PC is less loaded. Owner kit:
  `E:\office-target\p4-overlay\owner-test\` (DX12/GL launchers for Docs and Sheets, fixtures, README). MPO off
  (`OverlayTestMode`) is the owner's IT call, listed as an optional diagnostic only.

## Sheets default column width follows the Normal font (gridcraft/0154, p4-colwidth)
- Answers the "stays 64 px" open item. Rule (ECMA-376 18.3.1.81): default width = `baseColWidth` (8) characters of the Normal
  font's maximum digit width plus 5 px padding, rounded up to 8 px. Digit width: Times New Roman 0.5 em (12 pt = 16 px gives 8 px);
  every other font keeps the 7 px the model already assumes (Calibri 11, Excel's value). No other font table was invented.
- Calibri 11: 64 px before and after. Times New Roman 12: 64 px before, 72 px after (new workbooks, and xlsx files in that font
  without `defaultColWidth`).
- Stored widths (`defaultColWidth` and `<col width>`, read and write) use that same digit width (ECMA-376 18.3.1.13), so a
  Times New Roman 12 column and the default agree; Calibri 11 files convert exactly as before (7 px). Note a stored width includes
  the padding: 9.0 reads as 72 px in Times New Roman 12, while 8.43 reads as 67 px. `.xls` column widths still use 7 px. The
  writer omits `defaultColWidth` when the width equals what a reader derives from the Normal font.
- Tests: engine lib (new-workbook assert in `new_workbooks_are_times_new_roman_12_on_the_interface_paper`), xlsx lib
  `default_col_width_follows_the_normal_font`. Not checked: on-screen look, Excel opening a saved TNR 12 file.
### Phase 5: owner answers (2026-10-31)
- **Q10 retention:** 7 days for both. `keep_closed_hours` default 24 → 168 (`services/docserver/src/config.rs`), docs updated
  (`deploy.md`, `PLAN.md`, `retention.rs`); `keep_abandoned_days` stays 7.
- **S22 dependency check:** the agent's choice. The pinned `cargo-deny` 0.20.2 was already in `E:/office-tools/bin` (off the
  PATH). `ci.sh deny` ok for root and the four domains; the services workspace ok on the host and Linux targets (advisories,
  bans, licenses, sources; duplicate warnings only). `cargo-audit` is not added: deny's advisories check reads the same RustSec
  database. Security rehearsal now: 23 green, 1 partial (S19, Docker), 1 HRM team (S25).
- **Q3:** a Claude Code session on the HRM server does HRB PR-0 … PR-3 with `docs/phase0/hr-backend-prompt.md`.
- **Q8:** SSO and desktop sign-in after the pilot gate. **R6** (Sheets/Slides/PDF bridges) after the gate: confirmed.

### Phase 6: kick-off (2026-11-01)
- **Merge-forward rule:** `phase-4` -> `phase-5` -> `phase-6`, never the other way. Phase 4 and 5 fixes go on their own branches and are merged
  forward; `phase-6` was at `2b72bb1` (merge of `phase-5`), nothing newer on `phase-5` at kick-off. Rig work: branch `p6-rig` (worktree `E:/office-wt/p6-rig`).
- **Baseline `scripts/ci.sh quick` on `phase-6` (run `p6-wp0`, `E:/office-target/p1/ci/p6-wp0/`): exit 1.** ok: queue, gen-check, deny, i18n.
  Red before any Phase 6 change: `lint` (fmt drift in root 8 hunks, wordcraft 11, gridcraft 11, deckcraft 21, pdfcraft 9; `clippy-root`:
  `crates/suite-core/src/history.rs:314,316` `if_same_then_else`; upstream clippy pdfcraft: 1 new warning not in the baseline) and `brand`
  (`brand-grep`: 4 hits, the word of the working name in the "shared theme at start" patches `patches/{deckcraft/0149,gridcraft/0148,pdfcraft/0115,wordcraft/0132}`, rule W).
  These are carried debts, not caused by the rig; no `test`/`check` job was reached in this run.
- **Baseline `scripts/ci.sh sizes`** (dist under `E:/office-target/p1/dist`, whatever commit was last built there, not rebuilt for this run; brotli, headroom = budget - brotli):

| domain | brotli | budget | headroom | js_br / max |
|---|---|---|---|---|
| docs | 9,861,643 | 9,871,648 | **10,005 B** | 20,767 / 24,576 |
| sheets | 4,952,525 | 4,966,771 | 14,246 B | 20,054 / 24,576 |
| slides | 9,514,840 | 9,524,217 | **9,377 B** | 19,922 / 24,576 |
| pdf | 9,569,232 | 9,599,462 | 30,230 B | 20,543 / 24,576 |
| shell (src) | 60,559 | 65,536 | 4,977 B | |
| npm | 30,817 | 49,152 | 18,335 B | 3,142 / 16,384 |

  `sizes`: exit 0 (all within budget).

  Docs and Slides have about 10 KB left: any Phase 6 patch that adds wasm or JS needs a budget decision (plan section 9).
- **Browser and WebView on both emulators** (API 36 Google Play image, x86_64, Gboard 15.1.08.726012951): Chrome **133.0.6943.137**;
  Android System WebView **133.0.6943.137** (`com.google.android.webview`, the active provider). `HRM_Pixel_8` 1080x2400 @ 420 dpi = 412 dp;
  `HRM_360` 1080x2400 @ 480 dpi = 360 dp. Chrome 133 on the emulator (swiftshader_indirect) starts the Docs editor with no command-line override (the Phase 0 4096 texture limit did not bite).

### Phase 6 WP1: emulator rig `scripts/mobile/` (2026-11-01, branch `p6-rig`)
- `node scripts/mobile/rig.mjs <cmd>`: `create360` (writes the 360 dp AVD under `E:/office-target/p6/avd`, no avdmanager in this SDK; boots, verified 1080x2400 @ 480),
  `start|stop <avd>` (`-gpu swiftshader_indirect -no-snapshot-save`, `emu kill` of that serial only), `versions`, `gboard [--enable]`, `forward` (CDP on 9222),
  `shot`, `glide <avd> <word>` (emulator-console `event send`, 14 ms steps), `run <checklist> --avd <avd> [--dist <dist>]` (JSON + one `screenrecord` per item, named
  `<app>-<item>-<dp>.mp4` under `E:/office-target/p6/evidence/`, SHA-256 list in `docs/phase6/evidence/SHA256SUMS`, JSON and screenshots in `docs/phase6/evidence/rig/`).
  `scripts/mobile-m3.mjs --cdp [url] [--serial s] [--dist d]` drives the emulator's Chrome over CDP (adb reverse, logcat excerpt for renderer kills); 1-page control on `HRM_Pixel_8`: `opened` 7.2 s.
- Gboard: English, Russian and Uzbek (Latin) are enabled through the Android IME subtype setting (`enabled_input_methods`, hashes from `dumpsys input_method`); after that Gboard's own
  `dumpsys` list shows all three. Stylus-handwriting onboarding is switched off (it popped up over the editor). Gboard's internal language list is private: the glide and key
  coordinates in `lib.mjs` (`EN_LAYOUT`) are calibrated for 1080x2400 @ 420 dpi only; the 360 dp AVD and the RU/UZ layouts need their own map (`--layout`/`layout` option, read off `rig.mjs shot`).
- Limits found: Chrome's own download to `/sdcard/Download` fails on this image ("1 download failed"), so the rig reads the saved bytes from the blob the shell passes to the download; the shell status line
  ("Saved: <name>") still comes from the real touch path (File > Save). Editor taps are fixed device pixels (canvas, no accessibility nodes, plan section 8). The glide path is imprecise (stray `o`/`f` around `hello world` in the document).
- **Done-check** (`HRM_Pixel_8`, shell + Docs, synthetic `01-split-runs.docx`, dist `E:/office-target/p5/dist`): `shell-home` 3.1 s, `open` 9.2 s, `type` 10.6 s (five key taps + glide), `save` 7.9 s: all pass;
  the saved docx contains `hello` and `world`. JSON `docs/phase6/evidence/rig/2026-11-01T09-10-48-shell-docs-412.json`; recordings `docs/phase6/evidence/rig/docs-*-412.mp4` (copies; originals on E:).

### Phase 6 WP2 step 1: M3 measurement (2026-11-02, branch `p6-m3`)
- Emulator `opened` (median of 3): 412 dp 17.5 s (1 page) vs 24.7 s (108 pages); 360 dp 26.6 s vs 26.4 s. The emulator noise exceeds the document effect; the stall is not reproduced as one 240 s task.
- Native layout of the 108-page file (110 pages): 255 ms cold, 38 ms warm cache: shaping is 85 %, pagination plus all passes 15 %. Web Layout is not cheaper (233 ms).
- Recommendation: (a) always, then (c) slice the shaping warm-up; (b) refuted, (d) not needed unless pagination matters on a phone. Details `docs/phase6/m3.md`.

### Phase 6 WP4: WebView embed proof (2026-11-02, branch `p6-webview`)
- Test-only APK `ci/android-harness/` (aapt2, javac, d8, apksigner; no Gradle, no downloads) does what MOB does: inline HTML, `loadDataWithBaseURL(baseUrl = docserver)`, `new DocsAPI.DocEditor`. Run on AVD `HRM_Dev_33` (API 33, WebView 109, Gboard 12.4): open, type Latin (`salom`) and Cyrillic (`привет`) with Gboard taps, save (callback status 6, received once), close (status 2); the saved docx holds both texts.
- Origin: with `baseUrl` = the Document Server the page origin is the Document Server's own origin (`location.origin`, `ancestorOrigins`), so the operator must list the Document Server's own public origin in `frame_origins` besides the SPA origin (session create answers 403 otherwise; verified). WebGL2 works in all three nested frames and the editor canvas is WebGL2. Details and limits `docs/phase6/webview-embed.md`.
### Phase 6 WP2 step 2: M3 fix (a) + (c) (2026-11-02, branch `p6-m3fix`)
- (a) The shell covers the Docs/Slides editor with "Opening <file>…" (new `ed.opening`, reviewed:agent-opus:2026-11-02) from the hand-over until
  `<prefix>:opened`; before, the status cleared at `ready` and the blank or frozen editor showed for 3-25 s. Cache headers: already immutable for hashed files (curl-checked), no change.
- (c) Carried patch `wordcraft/0142` (layout `warm`, engine `Session::warm_layout`, wordcraft-web 25 ms slices per frame, `opened` after them; 0950 regenerated).
  Native: layout after the warm-up 64 ms vs 290 ms cold. Desktop 6x CPU, 108 pages: longest task 5.6-6.7 s → 3.8-4.1 s; `opened` about equal (12.0 s idle);
  **the "108-page `opened` ≤ 1.5x the 1-page" target is not met** (12.0 s vs 9.9 s); no task > 200 ms in the 10 s after `opened` (5 of 6 runs; one 207 ms under host load).
  Emulator `HRM_Pixel_8`: opens, page 1 shows, a tap places the caret and a key types there (recorded). The rest of the `opened` frame is mostly UI work, not
  layout (status bar walks the whole body every frame via `Document::start_of`/`end_of`); next steps in `docs/phase6/m3.md` section 5.
### Phase 6 WP7: keyboard, IME and viewport (2026-11-02, branch `p6-kbd`)
- Full write-up and evidence: `docs/phase6/keyboard.md`, `docs/phase6/evidence/kbd/`. `HRM_360`, Chrome 133, Gboard 15.1, dist from `phase-6` HEAD. The emulator needed `wm size 540x1200` + `wm density 240` (still 360 dp, DPR 1.5): at 1080x2400 software GL starved the guest (ANR dialogs, lost taps).
- A0 keyboard matrix on HEAD: 7 of 7 rows pass (Uzbek U+02BB/U+02BC, Latin, autocorrect `wolrd ` -> `world ` (F8 resolved), glide, Backspace, Cyrillic with `ё`, Enter makes paragraphs); each with a recording and the saved docx text.
- F5 "keyboard reappears after a swipe": reproduced (4 runs); root is Chrome re-raising the keyboard after a fully `preventDefault`ed touch sequence over a focused text input (reproduced on a plain page; eframe's refocus calls are a second path). Fixed in page code only: `index.html.in` of the four editors (agent blurred when the visual viewport grows; focus/blur no-ops while a touch has moved). No upstream patch, no eframe fork. After: two swipes keep the keyboard hidden, a tap raises it. WebView not tested.
- Caret above the keyboard at the page bottom: PASS with the current viewport meta; `interactive-widget=resizes-content` tried and not shipped (leaves 120 px of editor with the desktop ribbon); re-test with WP2.
- Cost: wasm/JS 0 B; `index.html` +718 B brotli per editor. New rig pieces: `kbd-matrix/-swipe/-caret/-probe.mjs`, `session.mjs`, RU/UZ maps, layout fingerprints, `RIG_SCALE`.

### Phase 6 WP2 step 3: `start_of`/`end_of` without the full walk (2026-11-02, branch `p6-docwalk`)
- Carried patch `wordcraft/0144` (`Document::start_of`/`end_of` stop at the first/last paragraph via `edge_para`; identical results, test on tables, nested tables,
  text box and footnote stories; `wordcraft-doc` 174/174; 0950 regenerated, byte-identical). Docs wasm brotli 9,855,009 B (-762 B).
- Native: the two removed walks cost 0.6 ms per frame (uz108), not seconds. Desktop 6x CPU: 108-page `opened` 13.1 → 12.6 s median (noise-level), 1 page 6.1 → 6.0 s;
  **the 1.5x target is still not met (2.1x)**; no task > 200 ms in the 10 s after `opened` (6 of 6, max 134 ms). The `opened` frame's remaining cost is elsewhere
  (candidates: ribbon style gallery, first word count 5 ms native); details and next measurement in `docs/phase6/m3.md` section 6.
- Shell Playwright spec run for the first time with the WP2 cover checks: 25 of 28 pass (both round trips); the 3 failures are stale copied sheets/slides/pdf dists without the `?lang=` shim, identical on the p6-m3fix dist.

### Phase 6 WP6: Docs phone layout and contextual bottom toolbar (2026-11-02, branch `p6-phone`)
- Full write-up: `docs/phase6/phone-layout.md`; evidence `docs/phase6/evidence/phone-layout/` (Chromium 360/412 dp, emulator HRM_Pixel_8 412 dp and HRM_360 360 dp). Carried patch **wordcraft/0140** (6 files, +132/-2; `lib.rs` +5, `canvas.rs` +4, `mini_toolbar.rs` +4) plus `domains/docs/crates/docs-office/src/phone.rs`. Docs only; the hook is set only by the web app, desktop untouched (ui-egui tests 733 pass, the same 7 translit i18n-audit failures as before).
- Trigger: width at most 600 CSS px and the last pointer a finger (`?phone=on`/`off` forces); Web Layout while on; top row 48 dp (name, dirty dot, Undo, Redo, More) and a contextual 48 dp bottom toolbar: caret (Format, List, Insert, Keyboard, More), text selected (Bold, Italic, Underline, Highlight, More), table (Row, Column, Delete, Format, Table), picture (Wrap Text, Size, Delete, More); sheets at most 60 % of the window.
- `interactive-widget=resizes-content` is shipped for Docs only (the shell switches the viewport meta while Docs is shown): without it the keyboard covers the toolbar. Android WebView not tested.
- Found and fixed: egui did not hit-test the bar's widgets on the web and a bar press also moved the page's caret (taps are read from pointer events; `chrome_down` makes `canvas::mouse` stand aside). Desktop Chromium at DPR above 1 maps touches at the wrong scale (not from this work; the emulator is right); Playwright's `touchscreen.tap` is missed by the page.
- Dialogs: Find and Replace, Split Cells and Size fit 360 dp; Print is not offered (it opens the Backstage, whose 200 px rail clips the page); no Language item. Three new catalog rows (Keyboard, More, Desktop layout), Uzbek `reviewed:agent-sonnet:2026-11-02`.
- **Size: Docs wasm brotli 9,875,353 B, +19,582 B on phase-6 HEAD (9,855,771 B); the budget (9,871,648 B) is exceeded by 3,705 B, `ci.sh sizes` fails until the owner raises Docs `delta_br_max` (measured + 10 % rounded: 212,992 B) or the code is slimmed.** Budgets were not edited. JS 20,803 B.
- Rig: recordings now go to `E:/office-target/p6/evidence/<run id>/` (no overwriting); new `phone-docs` checklist, `phone-shots.mjs`, `phone-dialogs.mjs`. The 360 dp run on this PC showed "System UI isn't responding" because two emulators ran beside a build.
- **Budget re-baselined on merge (2026-11-02):** `ci.sh sizes` (CI_DIST_ROOT = the p6-phone dist) measured Docs 9,875,353 B, delta +200,313 B, FAIL by 3,705 B; Docs `delta_br_max` 196,608 to 229,376 by the `web/budgets.toml` rule (measured + 10 %, rounded up to 16 KiB), as PLAN section 9 asks. Reason: the phone chrome is the WP6 deliverable; slimming it would not recover 3.7 KB without dropping sheets. The merged tree (adds 0144, -762 B) was not rebuilt. New Docs headroom about 29.8 KB.

### Phase 6 WP6 revision: read first and one bar (2026-11-02, branch `p6-mobileux`)
- Owner review of the WP6 screenshots ("close to WPS Office or ONLYOFFICE mobile, or else very simple"). Full write-up: `docs/phase6/phone-layout.md` section 9; evidence `docs/phase6/evidence/phone-ux/` (Chromium 360x640 and 412x915, DPR 1, light and dark, uz-Latn and ru, `scripts/mobile/phone-ux.mjs`; no emulator was started).
- **One bar:** in phone mode the editor's row is the only bar (Back arrow = the shell's Home, file name, Undo, Redo, More); the shell hides its header while the editor says phone mode (`wordcraft:phone`, `wordcraft:back`; `web/shell/shell.js`). Desktop and the other editors keep the header.
- **Read first:** a document opens for reading (no caret, no soft keyboard, a tap does nothing, no bottom toolbar, Undo and Redo hidden), the page is the whole screen; a round 56 dp Edit button bottom right starts editing (the contextual toolbar of WP6 appears; More > Done editing returns). No pencil in a session that cannot edit (read-only protection, view sessions, PLAN O7). A document that replaces the open one starts in reading again.
- **Sheets:** grab handle, header with title and close, Format with two tabs (Text; Paragraph with the styles drawn in their own size and weight), drawn keyboard, back, pencil and check glyphs; the paper colour fills the page panel.
- Carried patch **wordcraft/0143** (5 files, +45/-9; hot files `canvas.rs` +3/-2, `lib.rs` +3/-1), `0950-wordcraft-rustfmt.patch` regenerated (7 files, +53/-38); 0140 unchanged. One new catalog row ("Done editing", `reviewed:agent-sonnet:2026-11-02`).
- Tests: docs-office 39 pass (5 new); ui-egui 733 pass with the same 7 i18n failures as before; `verify-queue.sh wordcraft`, `cargo xt register check`, `cargo xt i18n check-stale` OK. Docs wasm brotli 9,878,844 B (+3,491 B on WP6), headroom 25,572 B (`ci.sh sizes`).
- Not verified: a real phone or emulator (touch feel, the soft keyboard on the first tap in editing, Android WebView). A new blank document also opens in reading (one more tap before typing): owner decision.

### Phase 6 WP11: accessibility minimum for Docs (2026-11-03, branch `p6-a11y`)
- Full write-up: `docs/phase6/a11y-palette.md`. PLAN section 8 option (b): a DOM command palette plus a keyboard exit, Docs only. `a11y-baseline.md` A1 (partly) and A2 (Docs) updated.
- Carried patch **wordcraft/0141** `bridge-commands-focusout` (`host.rs` +60, `Cargo.toml` +1, no hot file): `wordcraft:commands` (id, translated label, enabled, shortcut of the commands without a required parameter), `wordcraft:run { id }` / `wordcraft:ran`, `wordcraft:focus`, and a capture-phase keydown that posts `wordcraft:focusOut { palette }` for F6 (leave the editor) and F1 (open the palette); the editor binds neither key. `0950` regenerated (7 files, +59/-39).
- Page side: `web/shell/palette.js` (app-neutral; `<dialog>` + combobox + listbox, ARIA 1.2), used by `web/shell/shell.js` (`CMD_APPS = ['docs']`) and the docserver wrapper (`services/docserver/static/editor.js`, `addBar`). The Commands button is visible only while focused; F1 opens the palette on the page too. Eight `pal.*` catalog rows in `i18n/shell.tsv` (Uzbek `reviewed:agent-sonnet:2026-11-03`).
- Evidence: Playwright `ci/web-smoke/a11y-palette.spec.ts` (shell: Tab reaches the button, `ariaSnapshot` names the options, Bold from the palette on a selection makes the saved docx bolder, Esc and F6; uz-Latn, uz-Cyrl, ru) and a case in `docserver.spec.ts` (the wrapper). Results: shell spec 28 pass, docserver spec 19 pass + 1 skipped (incl. the new palette case), `a11y-palette.spec.ts` 4 pass (Bold from the palette changes the saved docx's bold runs 34 to 23: it toggled an already mostly bold selection). Sizes: Docs wasm brotli 9,882,041 B (budget 9,904,416, headroom 22,375); shell 64,609 B of 65,536 (headroom 927; palette.js 1,457 B, the rest of the +2,458 B over phase-6 is shell.js/css/i18n). `verify-queue wordcraft` (36 patches), `register check` (138), `i18n check-stale` OK.
- **Not verified:** TalkBack/NVDA speech (the emulator is held by WP8). **Option (c) stated:** screen-reader users of the document body should use the desktop apps; **(a)** AccessKit web adapter stays an upstream (egui) issue to file when upstream PRs are allowed.

### Phase 6 WP8: Android app, thin WebView (2026-11-03, branch `p6-android`)
- `packaging/android/` (one Activity, no Gradle, no androidx, no INTERNET permission); the shell and the four dists from APK assets on a virtual https origin; VIEW/SEND/picker in, write-back to the `content://` URI or Save as out; `brand.toml [android]` (provisional id `app.hoshiya.suite`, O5 open). Docs: `docs/phase6/android-app.md`, `android-licences.md` (empty dependency list).
- APK **53,200,442 bytes** (50.7 MiB; wasm 45.2 + 19.7 + 36.0 + 39.9 MB deflated), debug-signed with a local key (not committed).
- AVD `HRM_Pixel_8`: Gboard EN + RU typing, save back, reopen, read-only to Save as, rotation all pass (`docs/phase6/evidence/android/`, recordings in SHA256SUMS). Not run: a real share sheet, process death (WP12), a phone.
### Phase 6 WP6 follow-up: edit first and a Saved toast (2026-11-03, branch `p6-editfirst`)
- Owner decision 2026-11-03: on a phone every document opens in **edit mode** (the reading mode stays: More > Done editing, then the pencil brings editing back; a view session still opens in reading with no pencil). `docs-office` `phone.rs` now keeps the id of the document being read, so any other document opens in editing. New patch **wordcraft/0145** (+4/-1, `office_overlay.rs`) keeps the page from taking focus when a document opens, so the soft keyboard waits for the first tap (reasoned from the code and the Chromium focus check, not seen on a device).
- The "Saved: <name>" line is in the header that phone mode hides: the shell shows it as a 2 s toast above the bottom toolbar on a phone (`shell.js` `toast`, `shell.css` `#toast`, `role=status`, existing `ed.saved` string); desktop unchanged, errors still bring the header back.
- Tests: docs-office 39 pass; `verify-queue.sh wordcraft` (35 patches), `cargo xt register check` (137), `cargo xt i18n check-stale` OK. Docs wasm br 9,879,738 B (headroom 24,678 B); shell 61,589 of 65,536 B (headroom 3,947 B). Evidence `docs/phase6/evidence/phone-ux/360-light-uz-Latn-editfirst-*.png`. Details: `docs/phase6/phone-layout.md` section 9.

### Phase 6 WP12 lifecycle and WP10a other-editors survey (2026-11-03, branch `p6-lifecycle`)
- **WP12** (`docs/phase6/lifecycle.md`, rig `scripts/mobile/lifecycle.mjs`; Docs read-first build of `p6-mobileux/dist`, Chrome 133 and the WP8 APK, emulator `HRM_Pixel_8`): app switch and rotation keep the edit (Chrome and app); `am kill` while backgrounded, a renderer crash and a WebGL context loss lose it (Chrome tab reloads to the shell home; app restarts on the shell home; a renderer crash **kills the whole app**, `MainActivity` has no `onRenderProcessGone`). Loss window as shipped: everything since the last Save.
- **The 0005/0127 autosave does not reach the APK host:** the editor sends `wordcraft:autosave` 0.3-0.9 s after hide, with the WebGL-loss failure, and every 30 s while dirty (measured), but `web/shell/shell.js:142-160` has no `autosave` case and `SuiteHost` has no autosave method. The 5,752 B payload has the typed text but no picture part (cause not isolated). Not patched: proposal in `lifecycle.md` (shell keeps the bytes and re-opens after a GL loss, `SuiteHost.autosave` to an app-private file, `onRenderProcessGone`).
- **WP10a** (`docs/phase6/other-editors.md`, evidence `docs/phase6/evidence/other-editors/`): **Sheets and Slides fail to start** on the emulator's GL (`max_texture_dimension_2d 8192 is better than allowed 4096`, the failure `wordcraft/0002` fixed for Docs; no clamp in `gridcraft-web` or `deckcraft-web`), so their matrix rows are BLOCKED. PDF runs with the desktop layout: unusable as opened at 412 dp (three panes, 13 %), pinch zoom and long-press selection pass, one-finger drag selects instead of scrolling. O4 recommendation: Docs only for the gate; a small clamp WP for Sheets and Slides, then a real survey; PDF is the best second candidate.
- Recordings: `E:/office-target/p6/evidence/<run>/` (SHA-256 lines marked WP12 in `docs/phase6/evidence/SHA256SUMS`); the WP10a survey is screenshots only.


### Phase 6 WP10a2: texture-limit clamp for Sheets and Slides (2026-11-03, branch `p6-texclamp`)
- Carried patches `gridcraft/0156` and `deckcraft/0156` (REGISTER rows added; `verify-queue.sh gridcraft deckcraft` and `cargo xt register check` pass; the new lines are rustfmt-clean, no change to the 0950 patches): the web apps request the adapter's real `max_texture_dimension_2d` (same as `wordcraft/0002`), so they start where `MAX_TEXTURE_SIZE` is 4096.
- Emulator (`HRM_360`, no shim): both start and draw; Sheets scrolls and selects by touch; Slides zooms. Neither is phone-ready (desktop layout). Desktop Chromium: both unchanged. Details and matrix: `docs/phase6/other-editors.md` (update section, O4 after the clamp).
- Sizes (brotli wasm, budgets unchanged): Sheets 4,954,751 (headroom 12,020), Slides 9,512,548 (headroom 11,669). Dist: `E:/office-target/p6-texclamp/dist`.
- Not tested: a texture wider than 4096 px on a 4096 GPU; a real phone; pinch, typing, long-press, save in Sheets and Slides.
- Rig: `scripts/mobile/lib.mjs` `forwardCdp` reads `RIG_CDP_PORT` (default 9222) so two surveys on two emulators do not share a forward.
### Phase 6 WP12 recovery: the picture-less autosave and recovery of unsaved work (2026-11-03, branch `p6-recover`)
- **Picture-less autosave, root cause** (`docs/phase6/lifecycle.md` "WP12 recovery"): not a writer bug. Autosave and Save serialise the same document
  with the same writer (now one function, `office_overlay::autosave_bytes`); the picture had been deleted from the document by the rig's taps on the
  emulator: the Edit-button tap left the picture selected, the next tap moved it to the start of the document, and the first key replaced it (reproduced
  step by step, `E:/office-target/p6/evidence/2026-11-02T23-10-picture-rootcause/`). **Explicit Save keeps pictures**: test
  `autosave_and_save_keep_the_picture_of_an_opened_document` (wordcraft/0146) passes; every saved docx after a recovery holds `image1.png` (49,270 B).
  **Open risk, not fixed:** on a phone a tap can reach the page through the Edit button and a tap can move a selected picture (emulator only for the move);
  with a key after them a picture is deleted out of sight (Undo brings it back). Needs its own WP.
- **Recovery built:** shell keeps each `<prefix>:autosave` (Android: app-private file through `SuiteHost.autosave`; plain web host: IndexedDB; names
  mode: memory only), reloads the editor with it after a WebGL loss, reopens it at every start, unsaved and **without a question** (a first version asked
  with `confirm()`; relaunched Chrome answered it by itself with Cancel, unseen, and the copy was deleted: measured, `lifecycle.md`), and deletes it only
  after a Save in its own format or when the user discards the changes. App: `onRenderProcessGone` (the app no longer dies with the renderer), atomic
  temp + fsync + rename. wordcraft/0146 (`host.rs` +13/-5, `office_overlay.rs` +8, test +77): `wordcraft:open { dirty: true }`. One string (`rec.done`),
  Uzbek `reviewed:agent-opus:2026-11-03`. Shell 65,274 B brotli of 65,536 (headroom 262, no budget change); Docs wasm 9,876,954 B (headroom 27,462).
- **Desktop Chromium** (412x860 touch): WebGL loss, reload after the 30 s autosave, renderer crash after it: each reopens the edit unsaved with the picture
  (the saved docx then has `image1.png`, 49,270 B, and the text); the Save drops the kept copy.
- **Emulator re-run** (`HRM_Pixel_8`, Chrome 133 and the rebuilt APK, `lifecycle.mjs --edit-first`): WebGL loss recovered in place (Chrome, app; page back
  in about 2.5 s); background `am kill` recovered (Chrome, app; copy written 0.6 s after hide); renderer crash 30 s after typing recovered (Chrome, app; the
  **app now survives** the crash); renderer crash right after typing still **lost** (no autosave yet). Every kept copy: 55,495 B with the text and the
  picture. New loss window: none for WebGL loss and for a kill later than about 1 s after hide; **up to 30 s** (`AUTOSAVE_MS`) for a foreground renderer
  crash. Recordings `E:/office-target/p6/evidence/2026-11-03T0*/` (SHA-256 lines marked "WP12 recovery").
- Checks: `verify-queue.sh wordcraft` (37 patches), `cargo xt register check` (139), `cargo xt i18n check-stale` and `check` OK; the new ui-egui test passes;
  Playwright `bridge-docs` 8/8 (incl. autosave) on the new Docs dist; `shell` 25 pass, 3 fail (Sheets/Slides/PDF frame language: the same 3 fail with
  phase-6's `shell.js`; those dists were copied from `p6-mobileux`, not built here).
  Rig fixes: `lib.mjs tapText` no longer scales display coordinates by `RIG_SCALE` (it tapped half-way off at 540x1200); `lifecycle.mjs --edit-first`,
  `--before`, kept-copy pull.
- **Not verified:** a real phone, Android's own low-memory kill, Chrome tab discard, the picture-move code path, two shell tabs sharing one copy,
  private windows (memory only), the desktop host. Shell headroom is 262 B.

### Phase 6 WP12: the tap-through that deleted a picture (2026-11-03, branch `p6-tapfix`)
- **Root cause and fix** (`docs/phase6/lifecycle.md` "Tap-through fix"): on a touch phone the first pointer press after a quiet second arrives in a frame without a
  touch event, `phone::active` read it as a mouse and switched the phone layout **off for that frame**, so the press of a tap on the floating Edit button
  reached the page (selected the picture under it, began an object drag); the release met the closed gate and the drag stayed; the next tap on a heading was taken
  as that drag's pointer (the picture moved, the document went dirty) and the first key replaced the selected picture. Fixed in our code
  (`docs-office` `phone::touched_after`: a finger device never reads a pointer press as a mouse) and by wordcraft/0149 (`canvas::mouse`'s gate also drops
  `canvas.obj_drag`, +2 lines; `objects.rs` +49 test lines). REGISTER row added (mobile); `verify-queue.sh wordcraft` (38 patches) OK, `cargo xt register check` (142) OK.
- **Emulator before and after** (`HRM_Pixel_8`, Chrome 133, `scripts/mobile/tapfix.mjs`, screenshots in `docs/phase6/evidence/tapfix/`): before, the Edit tap selects the
  picture, the heading tap moves it, typing replaces it, the autosave is 5,750 B **without** `word/media/image1.png` and the layout toggled 9 times; after,
  the Edit tap only starts editing, nothing moves, the autosave is 55,490 B **with** `image1.png` and the layout toggled once.
- **Tests:** ui-egui `--lib` 737 passed, the same 7 i18n failures as before (cs, hu, hr, pl, tr, vi catalogs, `untranslated_interface_text`); docs-office 40 passed (new:
  `a_coarse_pointer_device_keeps_the_phone_layout_on_a_pointer_press_without_a_touch`); new ui-egui `a_touch_tap_on_text_does_not_move_the_selected_picture`
  (passes without the fix too: the headless harness has no phone chrome, it pins the behaviour). Mouse and desktop paths unchanged (`phone_on` false).
- **Size** (`CI_DIST_ROOT=E:/office-target/p6-tapfix/dist-final bash scripts/ci.sh sizes`): Docs wasm 9,875,600 B brotli of 9,904,416 (headroom 28,816; was 27,462); shell
  not touched (65,235 B of 65,536 as measured from `web/shell`, headroom 301).
- **Not verified:** a real finger (the rig's `input tap` is a 1 ms tap), the Android app's WebView (same page code, not re-run), the second tap of the "after" run
  selects a word through 0139's touch select (not investigated; it is not a move).

## Phase 7 (started early, in parallel; branch `phase-7` from `phase-6` b89c6de, worktree `E:/office-wt/phase-7`)
- Owner, 2026-11-03: "keep going; when Phase 6 is done, move to Phase 7". Plan only so far: `docs/phase7/PLAN.md`. Merge forward phase-6 -> phase-7 only; gates stay 4 -> 5 -> 6 -> 7.
- Spike `spikes/p7-runtime` (throwaway, 2026-11-03): uz-translit as a 108 KB core wasm module on a 334 KB / 12,555-run batch: native 40.5-41.1 ms, wasmtime 50.0.1 65.5-73.3 ms (binary batch; JSON 104-107 ms), wasmi 2.0.0 475-646 ms, Chromium 141 Worker 44.8-61.5 ms; all outputs identical. wasmtime costs +12.4 MB raw (+4.2 MB gzip) per exe, Cranelift compile 150-185 ms once. `cargo deny` with the root `deny.toml` unchanged: ok. In a browser Worker an exported memory grew to 4.28 GB; an imported, host-capped memory stopped at the cap, so the ABI requires an imported memory.
- Decisions proposed: wasmtime natively, the browser's WebAssembly in a Worker on web and Android, one core-module plugin binary, effects-as-data rounds, a binary text map for bulk text. Owner decisions P7-O1..O8 in PLAN section 13.4.
- WP1 `crates/plugin-api` (2026-11-03, branch `p7-api`): manifest types and every PLAN 4.1 refusal, message envelope, text map codec, actions + permission check, command allowlist as data, API version rules; deps `serde`, `serde_json`, `toml` only (root `Cargo.lock` gains `toml` 0.8.23 and `serde_spanned`); native and `wasm32-unknown-unknown`. 34 integration tests (every refusal; 500-case random round trip incl. U+02BB/U+02BC, combining marks, Cyrillic, U+10FFFF; every prefix of valid inputs; 4 GiB length claims; 20,000 mutations), clippy `-D warnings`, fmt, `cargo deny` pass. Allowlist-vs-registry test (3 tests, `domains/docs/crates/docs-office/src/plugin_allowlist_tests.rs`, dev-dep on `plugin-api`): 306 of 633 Docs commands allowed; it found nine path/file-taking commands the plan had not excepted (now in `allow::EXCEPTIONS`). `docs/plugins.md` sections 1-8 drafted. PLAN 4.1 fixed (the `events` key must precede the first table header) and extended with the WP1 validation additions.
- WP2 `crates/plugin-host` (2026-11-03, branch `p7-host`): wasmtime `=50.0.1` (`cranelift`, `runtime`, `std`; no WASI) behind the PLAN 4.2 ABI: import/export check (only `env.memory` + `plugin_v1.log`, exported memory refused), host-made imported memory capped at `memory_mb`, `StoreLimits`-style limiter, epoch ticker (50 ms) with a per-round deadline, background compile, 16-round loop, bounded messages. Tests `cargo test -p plugin-host`: 10 pass on gnu and on `stable-x86_64-pc-windows-msvc` (M1, M2, M6, M7, M8 + round cap); clippy `-D warnings` and fmt clean. `ci.sh deny` green (root, docs, sheets, slides, pdf; docs graph with `plugin-host` linked also ok, checked on a temporary dep). It needed a new ignore RUSTSEC-2026-0429 (zip `extract()`, never called) in the docs/sheets/slides configs. **Size: a minimal release exe (thin LTO, cgu 16, stripped, gnu) linking plugin-host is 11,461,120 B against 285,696 B for the same exe linking only plugin-api: +11.2 MB raw (+3.7 MB gzip -9), about +13 % on the 87.8 MB `docs.exe`; the real `docs.exe` delta is measured when WP3 links it.** Wasmtime LTS: unverified (no network check possible here); from memory every 12th major is LTS (24, 36, 48), so 50.0.1 is not on an LTS line: pin `=` and bump by review. `fetch` and its private-address filter are not in this crate (TODO under PLAN WP3).
- Wasmtime pinned to the LTS line `=48.0.8` (was `=50.0.1`, supported 2 months only): the release policy (docs.wasmtime.dev/stability-release.html, read 2026-11-03) supports majors that are multiples of 12 for 24 months. `cargo test -p plugin-host` 10/10 on gnu and MSVC, clippy `-D warnings` clean, root `cargo deny` ok. Bump by review to the next LTS (60).

### Phase 7 WP3: Docs adapter `docs-plugins` (2026-11-03, branch `p7-docs`)
- **Step 1 (f498ab9):** the docs-translit walker moved to `docs-plugins::textmap` with no behaviour change. docs-translit tests before/after identical (16 command + 13 preservation, 2 ignored perf; `docs/phase7/evidence/wp3-translit-{before,after}.log`, order-insensitive diff = timings only). `preserve-check.sh` (xlsx/pptx; those CLIs do not link docs-translit, p1 binaries) identical (`wp3-preserve-*.log`). The ignored `perf_108` was not re-run (nightly job).
- **Step 2:** 64 fixed slot commands re-registered on load, enable/disable and language change; executor (allowlist, one undo step, `restore` on any error, stale `rev` refused); events (selectionChanged debounced 200 ms, documentOpened, documentSaved); contributions (preset "Plugins" group, text context items, palette via the registry, egui side panel through the new carried patch `wordcraft/0147`, 7 hot-file lines). 20 unit tests with a fake guest + 1 equivalence test (a plugin converting through the text map equals the built-in, tracked on/off, selection, skipLangs, undo), docs-office 43 tests (39 + 3 allowlist + 1 new preset test). Guest and `fetch` are traits; real wasmtime linking, the host-side `fetch` private-address check, the full-ribbon "Plugins" tab and web (WP4/5) are not done here.

### Phase 7 WP5: web page host (2026-11-03, branch `p7-webhost`)
- `web/plugins/{host.js,worker.js,host.css,i18n.json}` (ours, no patch): lazy (the shell and the docserver wrapper fetch `host.js` only when `plugins` is non-empty in `shell-config.json` / the session answer; Playwright: no plugin, no request), one Worker per plugin, import/export check, host-made memory with `maximum`, SHA-256 of the wasm, 5 s round budget with `terminate()`, grant prompt (local: once per origin; server: descriptor list only), DOM panel (side panel, bottom sheet at 600 px or less), strict answer checks. Descriptor, bridge shapes assumed and CSP/fetch caveat: `docs/plugins.md` section 9a.
- Worker CSP `default-src 'none'; script-src 'wasm-unsafe-eval'` on `/shell/plugins/*.js`: suite-serve (`WORKER_CSP`, test `plugin_scripts_get_the_worker_csp`) and `MainActivity.java` (same constant; code only, **not run on a device**). Open for WP8: a granted `fetch` is stopped by the page's own `connect-src 'self'`, so it needs a per-provider CSP header or a proxy route.
- Strings: new catalog scope `plugins` (`i18n/plugins.tsv`, 8 rows, uz `reviewed:agent-sonnet:2026-11-03`) exported to `web/plugins/i18n.json`, not into `web/shell/i18n.json`, because the shell has 809 B of headroom (`cargo xt i18n export plugins --format json`; test `the_committed_plugins_catalog_is_current`).
- Budgets (`ci.sh sizes`, brotli): shell 64,727 of 65,536 B (+118 B for the loader line and the message hook; headroom 809 B); new `[plugins]` row 9,820 B measured, `br_max` 16,384 (measured + 10 %, rounded up to 16 KiB).
- `ci/web-smoke/plugins-host.spec.ts` (13 tests, ~12 s, stub editor, fixture plugin `ci/plugins/webhost-test`, WAT generated by `gen.mjs`): lazy rule, load, panel, M3, M6 (page timer late by under 150 ms while the loop ran), M7 (memory exactly `memory_mb`), M9, M11 (with a control script outside `plugins/` that does reach the server), M1/M2 import and export refusals, checksum refusal, editor action round trip.

### Phase 7 WP4: web plugin bridge, editor side (2026-11-03, branch `p7-bridge`)
- Editor end of the WP5 page-host shapes (docs/plugins.md section 9; PLAN 4.6 rewritten): `docs-plugins::bridge` (pending invocation, input lock = read-only protection lifted while actions run, wait timeout 20 s default, snapshot restore on error/abort/stale/replaced document, one undo step) and `docs-office::plugin_web` (the JS end, transferred ArrayBuffers); carried patch `wordcraft/0148` is +10 lines in `host.rs` (4 doc, 4 `sync`, 2 match arm), no hot file. `exec.rs` gained `Remote` (the editor half of an invocation) next to the synchronous `Machine`; `invoke` routes a `Guest::remote()` plugin to the bridge.
- `web/plugins/host.js` changed in two places (additive): `plugins` also carries `plugins: [{manifest, settings}]` (the editor builds slots from the manifests) and `pluginActions` carries `plugin` (an editor call it did not start needs it).
- Tests: docs-plugins 29 pass (9 new `web_bridge::*`); Playwright `ci/web-smoke/bridge-plugins.spec.ts` 8 pass against the real `host.js` + fixture plugin (edit over two rounds, hello) and a stub (register/refuse/unregister, text map as buffer, timeout, input lock, abort); `verify-queue.sh wordcraft` (39 patches), `cargo xt register check` (143) OK.
- **Size: Docs wasm brotli 9,958,372 B (`ci.sh sizes`, p7-bridge dist), 53,956 B over the old budget; `delta_br_max` re-baselined 229,376 -> 327,680 (delta +283,332 B, +10 %, 16 KiB) by the budgets.toml rule.** This is the whole plugin system on top of phase-6 (9,879,738 B at WP6 follow-up); the bridge's own share was not measured separately (no phase-7 baseline build).
- Not done / unverified: `fetch` runs in the page only; a `read` action is not served by the editor (the page hands reads over as `next.reads`); the stale-revision path through a real browser is covered by the native test (undo may be refused by the lock); a save during a pending plugin would write the read-only protection into the file (shell does not block saves).

### Phase 7 WP8: install, enable, admin control (2026-11-03, branch `p7-install`)
- Built to the recommendations O3 (server: admin only; local: user may install from a file, machine policy can forbid install and grants), O4 (SHA-256 pin and record, changed package refused; `signature` is a reserved, unread field), O7 (config files, no admin UI). Detail: `docs/plugins.md` 9b.
- `suite-core::plugins` (store, record, policy, descriptors; 5 tests), desktop IPC `plugins_list/install/enable/grant` with the `open` path rules (1 test, 26 in the lib), `suite-serve` `[plugins]` + `/plugins/<sha256>` + per-request `connect-src` (3 tests), docserver `[docserver.plugins]`, provider entries, session answer, `/ds/plugins/<sha256>`, `plugin-event`, audit lines (6 tests, M12 among them), `onlyoffice-compat.md` row, Plugins page `web/plugins/page.js` (Playwright `plugins-page.spec.ts`, fake desktop host).
- Loose ends closed: a granted `fetch` is no longer stopped by the page CSP (home page and wrapper page get `connect-src 'self' <granted origins>`); the descriptors are made by the servers from verified, enabled packages.
- Size (brotli, `ci.sh sizes` method): shell 65,452 of 65,536 B (+99 B over 65,353: the nav button, its 4 strings, one hook line and one CSS selector; headroom 84 B, so the next shell string needs a budget decision); plugins 12,847 of 16,384 B (page.js and its strings are 3,027 B of it). No budget raised.
- Not done / unverified: signature check (O4), Android install (WP9), uninstall, the file dialog and the WebView2 window of the Plugins page on a real desktop (the host code compiles and the IPC is tested; the dialog branch is not run), the docserver wrapper against a real browser with a granted `fetch` (WP10).
- 2026-11-03 p7-savelock: the plugin bridge's input lock is kept out of saves and autosaves (wordcraft/0150 `io::set_save_filter` + 2 docs-plugins tests); web smoke on a fresh suite-serve and docs dist: plugins-host 13/13, bridge-plugins 8/8, shell 21 passed 7 skipped (no sheets/slides/pdf dist); docs wasm.br 9,960,807 B.
### Phase 7 WP6: plugin #1 (uz-translit) and the Rust SDK (2026-11-03, branch `p7-translit`)
- Built: `crates/plugin-sdk` (ABI glue over `plugin-api`: `export!`, `Reply`, `read`; no proc macros, ~120 lines), `plugins/uz-translit` (`toCyrillic`, `toLatin`, `auto`; params, `lang_converts` per segment, `detect`, `convert_runs`, summary to `ui.status` in the built-in's words, four languages from the existing reviewed catalog rows: `plugins/uz-translit/strings.tsv`, no new strings), `cargo xt plugin build|pack <name>` (stored zip, fixed order/time/mode, SHA-256 printed; same inputs give the same bytes: `4de205c9...8271`, run twice), `scripts/ci.sh plugins` (pack, unit tests, native matrix) and the pack/wasm env in `translit-perf`. The plugin is the same `crates/uz-translit` the built-in links. lockdiff: `plugin-api` and `plugin-host` are own roots (`OWN_ROOTS`); with `plugin-api` the docs lockdiff is green again for the toml/serde_spanned entries it already had at the phase-7 base; **2 foreign additions remain (`futures-channel`, `futures-sink`: feature unification on `futures-util` through wasmtime, dev-dependency of `docs-translit`), so `ci.sh gen-check` stays red for docs until lockdiff learns to ignore edges the upstream lock lacks.**
- **Package:** `uz-translit-1.0.0.zip` 252,303 B raw, 83,262 B brotli (module 250,138 B raw, 82,869 B brotli, `plugin` profile: LTO, panic=abort, strip; wasm-opt -O3 gave no speed-up, not used). Not in a domain budget. Docs wasm not rebuilt by this WP (no editor code changed).
- **Output (PLAN 6.3), native** (`domains/docs/crates/docs-translit/tests/plugin_matrix.rs`, the real wasm module in the wasmtime host): 9 Word fixtures x {to Cyrillic, back to Latin on the built-in's result} x {tracked off, on} x {document, selection, caret} + `auto`, and the two 108-page files (one direction each, same 6 cases + `auto`) = **140 cases, 140 identical, 0 differing**: saved docx byte-identical, selection equal, one undo restores the document, in-memory document equal. Two honesty notes: (1) in 14 cases (all `08-controls-codes`) opening the same bytes twice already gives different documents (a random glossary id), so only the saved docx, selection and undo were compared there; (2) a tracked run stamps revisions with the wall clock to the second, so a comparison whose two runs straddle a second boundary is repeated (this made one 108-page tracked case look different by 3 bytes).
- **Output, web** (`ci/web-smoke/plugins-translit.spec.ts`: the real page host and Worker, the editor bridge, the docs-web dist of `p7-bridge`, Chromium): 9 fixtures x 2 directions x tracked off/on + the two 108-page files x tracked off/on = **40 of 40 identical**, document scope only (`wordcraft:run` carries no params, caret collapsed: scope auto). Compared per zip entry with ISO timestamps blanked, because the two runs are seconds apart. **Not run on the web:** selection and caret scopes.
- **N1 (headless native, release, alternating built-in/plugin in one process, median of 5, `perf_108_plugin`, 108-page files):** guest instantiate 0.1 ms; guest round 51-80 ms. Five runs of the final build on a PC that other builds shared (CPU 11-39 %): command part built-in 62-69 / 47-50 ms, plugin +39..+45 / +32..+39 ms (latn / cyrl) = **pass (<= +50 ms)**; command -> finished relayout +9.2 % / +9.9 % median of the five runs (individual runs -1.1 to +14.3 % and -2.3 to +11.6 %) = **marginal against +10 %**: 5 of the 10 per-run verdicts failed, 5 passed. Three earlier runs under heavier load (35-46 %) gave +61..+69 ms and up to +35 %. The overhead is a near-constant 35-45 ms (wasm vs native converter, the text map read and the second walk of the apply), about 10 % of the 340-440 ms total. Both far under the 2 s gate. Rows: `perf_108_plugin` prints PASS/FAIL per file.
- **N2 (desktop docs.exe over the control channel): not run.** The desktop app has no wasmtime guest installed yet (that is WP8: IPC, install, enable), so the plugin cannot run in `docs.exe` today.
- **N3 (web, `scripts/translit/perf-web.mjs --plugin`, new): run, inconclusive.** 24 rounds, software GL, PC loaded by other builds: the same command's frame time ranged 1.5-15 s between rounds (Phase 2 measured on an idle PC), far more than the expected +5-20 ms; medians of 5 (latn 2.1 s built-in / 3.4 s plugin, cyrl 6.6 s / 5.4 s) are noise. Re-run on an idle PC before quoting anything. The docserver wrapper variant and N4 (Android) were not run.
- Not done: N2, N3 on an idle PC, N3 in the docserver wrapper, N4, web selection/caret scopes. Docs: `docs/plugins.md` section 10 (short); the full guide and the template are WP11.

### Phase 7 WP8 security fixes (2026-11-03, branch `p7-wp8fix`)
- Review findings H1, M1-M4, L1-L4 fixed: machine folder trusted only when owned by Administrators/SYSTEM with no other writer (`suite-winsec::folder_is_admin_only`; MSI `icacls` custom actions in `scripts/build-msi.ps1`, not built or installed here); `user_install = false` leaves the user root out; docserver `/ds/plugins/<sha>` strict and audit-limited, `plugin-event` checked against the grant, marked `source: client`, rate-limited; web host list computed once, optional `[plugins.sha256]` pins; reinstall with a new manifest resets consent; `plugins_install` no longer takes a path from the page. Detail: `docs/plugins.md` 9b, `docs/phase5/deploy.md`.

### Phase 7 WP7: plugin #2, template fields (2026-11-03, branch `p7-fields`)
- Built: `plugins/template-fields` (panel with filter box and an Insert button per field, `developer.plainText` + `developer.properties` / bookmark kind, `selectionChanged` -> `next.reads` selection -> highlight, "Check fields" through `document.inspect`; `strings.tsv` in four languages, uz `reviewed:agent-sonnet:2026-11-03`), docs in `docs/plugins.md` section 11. The field list is `settings.fields` only; the manifest default is synthetic (`field_a`, `Sample date`, ...). All commands used are on the allowlist (`developer.`, `text.`, `caret.`, `insert.` families and the read `document.inspect`); none needed a change.
- Settings: manifest default, admin per provider (docserver test), user in local mode (new: `[plugins.settings.<id>]` in `settings.toml` and the web host config -> `Enabled.settings` -> descriptor; suite-core test). `no_hrm_names` now scans `plugins/` and `web/plugins/`.
- Two small host changes: an event refreshes an open panel and never opens one (`web/plugins/host.js`, `docs-plugins::exec::finish`); `scripts/plugins/manifest-json.mjs` reads multi-line arrays of inline tables.
- Tests: plugin unit tests 8; native `docs-translit/tests/fields.rs` (real wasm in wasmtime: filter, insert, undo, bookmark, highlight, event does not open a panel, `w:sdt`/`w:tag` in the saved docx, byte-identical second save, check, settings); Playwright `ci/web-smoke/plugins-fields.spec.ts` (real page host, insert, highlight, check, round trip); docserver `template_fields_settings_come_from_the_provider_entry`.
- Package `template-fields-1.0.0.zip`: 226,870 B raw, 73,452 B brotli (module 222,131 B raw, 72,837 B brotli; serde_json dominates). Not on the desktop: the app run waits for the native guest of `p7-native`.
- Limits: the selection read is text only (empty for a collapsed caret), so the highlight needs the placeholder selected; the filter resets on the next selection event; check does not see tables, headers or footers.
- Results: plugin unit tests 8 pass; `docs-translit` `fields` 2 pass (native wasmtime); docs-plugins 31 pass; Playwright `plugins-fields` 1 pass (the egui editor needs a frame after a selection change for the 200 ms debounce, so the spec moves the mouse once); docserver `plugins` 8 pass; suite-core `plugins` 7 pass; suite-serve `plugins` 3 pass. `no_hrm_names` fails on `services/docserver/src/plugins.rs` ("onlyoffice" in two comment/string places, present at the phase-7 base, not from this work); `plugins/` and `web/plugins/` are clean. The docs workspace `Cargo.lock` at the base lacks the `plugin-host` entries (`gen-check` was already red), so this branch does not touch it.
### Phase 7 native plugins in the desktop Docs app (2026-11-03, branch `p7-native`)
- Built: `docs-office::plugin_native` (wasmtime `plugin-host` as `docs_plugins::Guest`, `load(&Store)` registers the enabled, verified plugins of the suite-core store with their grants; `init()` runs it once on a thread from `install`, not under `WORDCRAFT_NO_PREFS`), and `plugin-host::net` (native `fetch`: granted origin, plain https, resolve then refuse loopback/private/link-local/CGNAT/unspecified/multicast/reserved and the v6 and v4-in-v6 forms, connect to the checked address, same-origin redirects only with a re-check per hop, rustls + ring + OS roots). A refused package is said on stderr and skipped. The set is read at app start only (the app does not watch settings; a plugin enabled in the shell shows up at the next start of `docs.exe`).
- Tests: `plugin-host` 6 net tests (address table of 51 rows, resolver-behind-public-name, origin/https, redirects, targets, responses) + 1 ignored live GET (run by hand: passed against example.org) + the 10 existing; `docs-office` `plugin_native_tests` (real `plugins/uz-translit` package installed into a temp store, enabled, loaded, `toCyrillic` gives a document equal to the built-in's on a Word fixture and the 108-page file, a changed `plugin.wasm` is refused and the plugin goes away); `xtask` lockdiff test for the allowance below.
- **N2 (PLAN 6.3, desktop `docs.exe` release, control channel, `scripts/translit/perf-desktop-plugin.py`, built-in and plugin alternating, fresh process each, median of 5 per run, 3 runs = 15 timed rounds per mode, 108-page files):** pooled medians, built-in -> plugin: to Cyrillic `cmd` 75 -> 121 ms, next frame (`inspect`) 485 -> 562 ms (+16.0 %), painted new layout (`shot`, the end-to-end upper bound) 1041 -> 1136 ms (**+9.1 %**); to Latin `cmd` 54 -> 88 ms, `inspect` 454 -> 495 ms (+9.1 %), `shot` 1037 -> 1019 ms (**-1.7 %**). Per-run `shot` deltas: to Cyrillic -4.7, +12.7, +10.6 %; to Latin -6.4, +3.8, -1.4 %. Machine: other agents' builds shared the PC (CPU 100 % with 15 rust processes in run 1, 21-29 % in runs 2-3), so single runs swing by +-13 %. Reading: the plugin adds a steady 30-50 ms to the command (wasm convert is 1.6-1.8x native); **pass on `shot` (within +10 %), but to Cyrillic is marginal and `inspect` is over +10 %**; repeat on an idle PC before quoting it as final. Raw rows: `E:/office-target/tmp/p7-native/n2*.json` (not committed).
- gen-check (`lockdiff`): was red for `docs` (`futures-channel`, `futures-sink`). Cause: the lock resolver follows wasmtime's optional `futures` dependency, which widens `futures-util`'s lock entry; no build links them (`cargo tree -i futures-sink --target all -e all` prints nothing in `domains/docs`, and `cargo tree -p docs-web --target wasm32-unknown-unknown` has no wasmtime, rustls, ring, plugin-host or suite-core). Recorded in `xtask/src/domain.rs` `LOCK_ONLY_VIA_WASMTIME` with name and version, so a new or changed entry still fails. New private deps of `plugin-host` (rustls, ring, rustls-native-certs, schannel and friends) are counted as ours through the existing `OWN_ROOTS`. `cargo deny` docs config and root: ok (licences, bans, advisories, sources). `gen-domain --check` was also red for `docs-web/Cargo.toml` (stale since Phase 6 WP11: the web-sys feature `KeyboardEvent` that upstream's manifest has was missing); regenerated, a feature flag only, the Docs wasm size was not re-measured. The domain lock also gained `zip 9.3.0` and `typed-path` (suite-core's own dependency, missing from the committed domain lock).
- Docs wasm size: unchanged by construction (no edit to any wasm-reachable code; docs-office's new dependencies are `cfg(not(wasm32))`); not rebuilt, so `ci.sh sizes` was not run.
- Panel path checked in code: `apps/wordcraft/src/main.rs:440` `set_plugin_ui(docs_office::plugin_panel, plugin_context)` (wordcraft/0147) -> `panes::show` -> `docs_plugins::panel`; drawn headless by the docs-plugins test `the_panel_and_the_context_items_draw`. No first-party plugin declares a panel, so no panel was seen in a live window.
- Not done: the admin switch that allows private fetch targets (always refused); reload while the app runs; a live TLS fetch from inside a plugin (the module cannot call `fetch` in a test plugin; the net layer was run against example.org by hand); proxy support.
### Phase 7 WP9: Android (2026-11-03, branch `p7-android`)
- Built: first-party plugins bundled from `plugins/*` by `packaging/android/build.sh` (`xt plugin pack` per package, so template-fields joins with no code change), `Plugins.java` store (bundled + installed, SHA-256 record `install.toml`, on/off in SharedPreferences, `grant: []` always), `Toml.java` manifest reader, loader routes `/shell-config.json` (descriptors added) and `/plugins/<sha256>` (digest re-checked), install from file through the system picker, the Plugins page on Android (`SuiteHost.plugins`, `page.js`, one new string `plg.page.restart`, 4 languages, uz `reviewed:agent-sonnet:2026-11-03`). No network on Android: no INTERNET permission, no grants. Doc: `docs/phase7/android.md`.
- **Bug found and fixed:** the Docs phone layout never draws the ribbon, whose per-frame hook registers the plugin commands, so in the app `plugin.*` commands were "unknown". `docs-office::plugin_web::sync` now calls `docs_plugins::frame` (our layer, no upstream patch).
- Emulator (HRM_Pixel_8): uz-translit toCyrillic/toLatin on `phone-ux.docx` (saved and pulled), install from file, disable, M1 M2 M3 M4 M6 M7 M9 M11 and no-network all pass; tapfix in the app: before loses the picture, after keeps it. Evidence `docs/phase7/evidence/android/`, SHA lines in `docs/phase7/evidence/SHA256SUMS`.
- APK +110,943 B (53,368,768 -> 53,479,711). Shell budget: +15 B (about 65,467 of 65,536, headroom about 69 B); plugins 13,009 of 16,384 B.
- Not done / unverified: M5 M8 M10 (need the WP10 `evil` plugin), M12; the palette or a toolbar button for the plugin commands (no slot in the manifest; the run used `wordcraft:run`); a real phone; manifest validation on Android is a subset of `plugin-api` (documented shortcut); a change of the enabled set needs an app restart.

### Phase 7 WP10: malicious-plugin suite (2026-11-03, branch `p7-evil`)
- Table: `docs/phase7/malicious.md` (M1-M12 and R1 by desktop native, docs.exe, web shell, docserver wrapper, Android pending WP9; each cell names an existing test or says `gap`). Gaps: M8 and M10 in a browser, redirect handling in `host.js`, the page-side rows inside the wrapper, M1/M2/M6/M7/M9 in `docs.exe`.
- Found and fixed: `next.reads` was uncapped natively (`docs-plugins/src/exec.rs` `Run::read` now refuses the 17th blob of a round; test `more_than_16_reads_in_a_round_are_refused_before_any_is_done`). M3 aligned: a fetch outside the manifest's `network` list aborts the invocation natively and on the web (`host.js` `checkAnswer`); PLAN 5.4 and `docs/plugins.md` updated.
- CI: the `plugins` job runs plugin-api/plugin-host, docs-plugins, the docserver `plugins` test, the two plugin Playwright specs (when dists exist) and `scripts/plugins/evil-desktop.py` (when a release `docs.exe` exists).

### Phase 7 WP11: `docs/plugins.md` final, command list, SDK template (2026-11-03, branch `p7-sdkdoc`)
- `docs/plugins.md` rewritten as one guide (concepts, manifest, ABI, actions, text map, widgets, events, permissions, install/admin per platform, command catalog, SDK quick start, versioning, security model; the web bridge is Appendix A). Generated `docs/plugins-commands.md` (306 allowed commands with params) by `cargo xt plugin commands`; `--check` runs in `ci.sh gen-check` (test `the_generated_command_list_is_current` in docs-office).
- `plugins/template` (one command, one panel, `strings.tsv`, `build.rs`); `cargo xt plugin build|pack <folder>` also takes a folder outside the workspace; native test `docs-translit/tests/template.rs` loads the real module in wasmtime (`ci.sh plugins`).
- Fixed a hole found while checking the doc: `next.reads` (and a manifest `reads`) did not need the document permission, so a `document = "none"` plugin could read the text. `Run::read` now refuses without `read`/`write` (`docs-plugins` test `next_reads_need_the_document_permission`).
- Doc test: a fresh haiku agent wrote a "hello" plugin from the doc alone; it built, packed and loaded in the native harness after the doc got the missing `build.rs` target gate, a minimal `Cargo.toml`/`lib.rs` and the notes on `Call`/`Reply` in tests.
- Open: PLAN R4 said plugin commands are not recordable; the engine records them like any mutating command (doc says not to rely on replay). No security contact address exists yet.

### Phase 7 hardening A: memory, stack, time (2026-11-03, branch `p7-harden-a`)
- F1: a module-defined second memory bypassed the cap on the web (V8 has multi-memory). `worker.js` now walks the section headers before compile and refuses a memory section and wasm GC / typed-reference types; native `Host::new` turns multi-memory off. Tests: M2b in `host.rs` and `plugins-host.spec.ts`.
- F4: wasm stack 256 KiB, and each native round runs on its own 1 MiB thread (a 256 KiB limit alone still aborted a 400 KiB thread in a debug build). Test: `deep_recursion_traps_cleanly_on_a_small_stack` (100 KiB caller).
- F3: 10 s per-invocation deadline, 8 fetches, 100 ms event round, off after 3 timeouts (constants in `plugin-api/src/wire.rs`, repeated in `host.js`, new string `plg.disabled`). Native auto-disable is counted in the sync path only; the web bridge (`bridge.rs`) is not counted, `host.js` does its own.
### Phase 7 gate review fixes, part B (2026-11-03, branch `p7-harden-b`)
- F2: `insert.field` and `fields.setCode` excepted; `link`, `ins`, `del`, `fmtChange`, `numChange` are denied param keys at any depth (also inside `styles.*`, `format.set`). F7: `review.addToDictionary`, `review.accept`, `review.reject`, `review.restrict` excepted; the bridge refuses to start on a document with its own protection (`plg.protected`). F5: a panel-click round is refused while another plugin is pending (`plg.busy`) and ended string-call ids (64) are answered with an error. F6: Android `install()` disables the plugin when `manifest_sha256` changed. Tests and table rows: `docs/phase7/malicious.md` F2 to F6. Not run: Android emulator, Java has no unit tests (compiled only).

### Phase 7 columns: Android and docserver wrapper (2026-11-03, branch `p7-columns`)
- `docs/phase7/malicious.md`: the Android and wrapper columns are filled. Android (emulator `HRM_Pixel_8`, hardened tree): M1, M2, M2b (own memory, GC type), M3 as an abort, M4, M5, M6, M7, M9, M11, R1 and F6 blocked; F5 blocked with a low UX finding (the refusal says "protected (read only)", not "another plugin is still running"); M8, M10, F2, F7, S2 gaps; M12 n/a. Evidence in `docs/phase7/evidence/android/`, recordings in `E:/office-target/p7-evidence/evidence/p7-columns/` (SHA256SUMS).
- Wrapper: `ci/web-smoke/docserver.spec.ts` "plugins in the wrapper" runs M1-M7, M9, M11, R1 in the docserver wrapper with the real editor and five fixture plugins from a provider entry; the saved text equals the opened one and the server audit holds exactly the three `plugin.network` lines. Four consecutive passes after a retry for commands refused while the editor still holds the previous invocation's lock.
- Runner: `android-plugins.mjs` `cols` (M3 abort, M5, R1, F5, F6) and `rows` extended (M2b); the picker list is scrolled when a package is not on screen. No product code changed, no new finding that needed a fix.

### Phase 7 gaps: F5 wording, M8/M10/M4 in a browser (2026-11-03, branch `p7-gaps`)
- F5 (Finding 3) fixed in our layer: `slot_enabled` in `docs-plugins/src/lib.rs` answers `bridge::BUSY` while an invocation is pending (the engine runs `enabled` before its protection gate, so no upstream patch). Test `web_bridge::a_palette_command_of_another_plugin_while_one_is_pending_says_busy`; docs-plugins 40 pass, clippy `-D warnings` clean. Not checked in a browser or on the device: that needs a rebuilt docs wasm, which was not built in this run.
- Fixture `ci/plugins/webhost-test` (`gen.mjs`, `plugin.wat`, `plugin.wasm` regenerated): command `bad` answers with a header of invalid UTF-8; an event is answered with a `text.insert`. Cases: `plugins-host.spec.ts` M8, M10 (16/16 pass); `bridge-plugins.spec.ts` `bad` added to test 9 and "M4 redirect" (10/10 pass). The redirect case uses Playwright routes for the https origins (A answers 302 to B, B is not granted): `host.js` already refuses it (`redirect: 'manual'`, `{error: "a redirect was refused"}`), B is never requested, the audit holds the first hop only. No product code in `host.js` changed, no new finding.
- Run against a suite-serve built from this tree and the shell dist of p7-savelock with `web/plugins/*` laid over it (the p1 binary and the old dist predate the Worker CSP and failed 13 of 16, unrelated to this change). `malicious.md` M4, M8, M10, F5 cells and Findings updated; wrapper and Android cells for M8/M10 say "shared code, same fixture; not re-run".

### Phase 7 upstream locks (2026-11-03, branch `p7-locks`)
- The four upstream `Cargo.lock` files gain suite-core's Phase 7 dependencies (plugin-api, zip, typed-path, toml 0.8, serde_spanned, nix) as carried patches wordcraft/0901, gridcraft/0900, deckcraft/0900, pdfcraft/0901 (ci-fix, additions only); `cargo metadata --locked` now passes in all four upstream workspaces.

### Phase 7 full CI (2026-11-03..05, phase-7 @ 3e33e99)
- `ci.sh full` ran twice; E: filled up (0 B) during the second run (target dir 191 GB, p1 caches 491 GB), so lint and the early jobs of that run were lost. After clearing the caches (owner OK for p1, MSI kept) the lost and failed jobs were rerun: every job is ok.
- Not covered: brand-scan-bin (no release binaries in this target), the docserver Docker rehearsal (not requested).
- Fixed on the way, all stale tests rather than product bugs (made on phase-4 or phase-6, merged forward):
  - the npm smoke and node test still expected the 1995 Latin default (owner decision 2026-10-29, d0a4be5); 1995 is now asked for explicitly where checked;
  - convert-api `tests/e2e.rs` found the release CLIs only under `E:/office-target/p1`, now `$OFFICE_TARGET`;
  - wordcraft/0146 test picture: clippy `precedence` (parenthesised).
- Lesson: `npm ci` through a junctioned `node_modules` replaces the junction and empties its target (phase-5's was emptied and reinstalled). A full CI needs about 190 GB free on E:.

### Phase 7 owner decisions (2026-11-06)
- The owner accepted every recommendation of PLAN 13.4 (O2-O8) and of the gate report; O2 is Docs + Sheets + Slides (WP13, PDF later). `[shell]` budget 65,536 -> 81,920 B brotli (measured about 65,467 B). No proxy for plugin fetches in v1. Plugins stay refused on protected documents. The first-party signing key is the owner's, kept offline.

### Phase 7 re-check (2026-11-06, branch `p7-recheck` from phase-7 f2bbaee)
- Goal: close the "not run / gap" cells of the Phase 7 evidence, no new features. The docs web dist was rebuilt from phase-7 code (`scripts/build.sh docs web`, dev profile, this worktree), the services (hoshiya.exe, hoshiya-serve, fake-hrm, fake_diff) and the Android APK from the same tree. Machine busy with two other agents: no timing numbers taken (the S2 round sizes move with the load, see below).
- **F5 `plg.busy` fix in a browser and on the device**: Chromium with the real editor (`bridge-plugins.spec.ts` "F5 ...": "another plugin is still running", not "read only"), the docserver wrapper and the emulator (`F5-busy.json`). Closed.
- **Docserver wrapper** (`docserver.spec.ts` "plugins in the wrapper (re-check)", with the fake HRM, the real editor in the wrapper frame): M2b, M8, M10, S1, S2 (deadline and fetch cap), F2, F5, F7a, F7b blocked; F6 n/a (Android install path). The first wrapper test (M1-M7, M9, M11, R1) still passes. The History panel test passes with `fake_diff` built; "History panel takes the host app look ..." and the WP9 bookmark cases need the Sheets/Slides/PDF dists and were not run (Docs-only dist root; the first of them failed on the missing dists, which also broke the next test's server restart).
- Regression with the new fixture wasm: `plugins-host.spec.ts` 16 of 16 (suite-serve needed more than its 15 s start timeout on this busy PC; run with a longer timeout, spec unchanged), `plugins-page.spec.ts` 7 of 7, `bridge-plugins.spec.ts` 17 of 17, `docs-plugins` 40 native tests.
- **Browser, real editor** (`bridge-plugins.spec.ts`, 17 of 17): new S1, S2 deadline, S2 fetch cap, M10, F2/F7a (7 commands), F7b (comments: the engine's gate; tracked changes: the bridge), F5.
- **Android** (emulator `HRM_Pixel_8`, 540x1200 rig, `android-plugins.mjs recheck`): M5, M8, M10, S1, S2 (deadline, fetch cap), F2, F5, F7a, F7b all pass; evidence `docs/phase7/evidence/android/*.json` and `recheck-*.png`, recording `E:/office-target/p7-recheck/evidence/p7-recheck/recheck.mp4` (SHA-256 in `docs/phase7/evidence/SHA256SUMS`). An emulator, not a phone.
- **Web translit selection and caret scopes** (`plugins-translit.spec.ts` "selection and caret scopes ..."): 9 Word fixtures x tracked off/on x {selection, caret} to Cyrillic plus the selection back to Latin = 45 of 45 identical to the built-in command; the selection is narrower than the document in 4 of 9 fixtures. `wordcraft:run` carries no parameter, so the web "caret" case is a collapsed caret under scope `auto` (whole document); scope=selection with a collapsed caret (a no-op) stays native only (`plugin_matrix.rs`).
- **Desktop** (`scripts/plugins/desktop-install-check.mjs`, `docs/phase7/evidence/desktop/desktop-install.json`): the real hoshiya.exe (debug build), its WebView2 window found through UI Automation (class `Window`, title Hoshiya, 1116x799), the Plugins page opened through the WebView2 debugging port, "Install from file" clicked: the native Open dialog (rfd, Russian "Открытие") appears and is driven by Win32 messages (`scripts/plugins/file-dialog.ps1`: WM_CLOSE for Cancel, WM_SETTEXT into the file name box and BM_CLICK on Open). Cancel: one `cancelled` reply, no message, nothing installed. Open with the uz-translit package: `installed`, the page lists it, `install.toml` written. UI Automation alone could not do it (the file name box is a legacy ComboBoxEx32 without a value pattern; Invoke on the Open button accepted the first list item instead of the typed path). No screenshots: the interactive desktop was on the lock screen during the runs (a capture shows the lock screen), so the proof is UI Automation and the IPC replies, not pixels.
- **Bugs found and fixed**
  - `web/plugins/page.js` `send`: a 20 s "did not respond" timer also ran for `plugins_install`, whose reply waits for the user in the file dialog (found with the real dialog: cancelling after 20 s left the error on the page). No timer for `plugins_install`. Test `plugins-page.spec.ts` "a dialog that stays open for more than 20 s ..." (fails on the old page.js, passes now; 7 of 7). Plugins budget: +60 B.
  - Not a bug: `selectionChanged` on an idle page. It looked as if the debounce needs another frame (nothing requests one); the old and the new dist both fire it within seconds (the caret blink draws frames), so no code change was made (an `events_pending` helper tried and reverted).
- Fixtures (all under `ci/plugins/webhost-test`, `scripts/mobile/fixtures`): `gen.mjs` has the new commands (`link chr incl tracked vocab accept wall overflow ping pong pang fetches`) and the `.wasm` was rebuilt with the `wat` crate; a manifest holds at most 16 commands, so the browser specs use a second plugin and Android the package `test.recheck`. `make-protected.py` makes `protected.docx` (comments) and `protected-tracked.docx` (trackedChanges). `android-plugins.mjs`: the CDP calls time out after 30 s (a paused page used to hang the run), the picker helper checks that the Downloads folder opened and scrolls to the top first.
- Not run / still open: `docs.exe` column cells (M1, M2, M6, M7, M9: not asked); a real phone; the S2 deadline numbers depend on the machine (round sizes `ping` 6e8 and `pong` 3e9 loop turns; the tests take the first that fits); the wrapper tests that need Sheets, Slides and PDF dists.

### Phase 7 signing and uninstall (2026-11-06, branch `p7-sign`)
- Built to decision P7-O4 (pin + optional signature). `SIGNATURE` = `ed25519 <key hex> <sig hex>` over `plugin-package-signature-v1` + one `<name> <sha256>` line for `plugin.toml`, `plugin.wasm` and (when present) `strings.tsv` (`docs/plugins.md` 9.1). Checked on every read in `suite-core::plugins` with ring 0.17 (already in the root, docs, pdf graphs), so the desktop shell, native Docs, `<product>-serve` and the docserver share one function; Android has its own `Signatures.java` (RFC 8032, BigInteger; `java.security` Ed25519 needs API 33, minSdk is 26). A bad signature refuses the package everywhere; verified = signed by a trusted key; unsigned runs locally with a Plugins page warning, is refused in the user folder under `policy.toml user_unsigned = false`, and always on the docserver (`[docserver.plugins] trusted_keys`). Trusted keys: `brand.toml [plugins] publisher_keys` (empty: the project key is the owner's, offline), `policy.toml`, `settings.toml`, the two server configs. Dev tool `cargo xt plugin keygen|pubkey|sign`; published TEST keys only in `ci/plugins/signing`.
- Uninstall: `Store::uninstall` + IPC `plugins_uninstall` + a Plugins page button (user folder only; IT's machine packages are refused), Android `Plugins.uninstall` (installed only, not bundled), servers by configuration (documented in `docs/plugins.md` 9.4, `docs/phase5/deploy.md`). Folder, install record, `enabled` and grants go; the user's plugin settings stay; a loaded plugin keeps running from memory and is never served again.
- Five new strings (`plg.page.verified`, `.unsigned`, `.uninstall`, `.uninstalled`, `.signedOnly`), uz `reviewed:agent-opus:2026-11-06`.
- Locks: ring/untrusted/getrandom 0.2/wasi 0.11 added (additions only) to the sheets and slides domain locks and, as carried patches, wordcraft/0902, gridcraft/0901, deckcraft/0901, pdfcraft/0902 (`patches/REGISTER.md`).
- Measured: three independent Ed25519 implementations agree: the xtask/ring signature of `ci/plugins/signing/pkg` is byte-identical to Node's (OpenSSL) for the same seed and message, and ring accepts the RFC 8032 vectors 1-3 that the Java verifier also accepts. Java verify on this PC's JVM: median 7.9-8.5 ms (20 runs; cached per package in the app); no phone measured.
- Tests: suite-core `plugins` 11 (4 new: RFC 8032 vectors and the message format; good, tampered wasm, tampered `strings.tsv` on disk, forged key, 9 malformed forms, CRLF, untrusted key, user key; unsigned local, `user_unsigned = false`, IT folder; uninstall while loaded, IT package, path id); xtask sign/keygen 1; office-brand key check 1; suite-desktop IPC 1 (extended: uninstall, `verified`, `removable`, policy); suite-serve 3 bin + 3 + 4 + 7 integration; docserver `plugins` 10 (2 new: signed-only, removed package); `sigcheck.sh` 44/44 (Java vs the RFC vectors and the Rust-signed fixture); Playwright `plugins-page.spec.ts` 8/8 (2 new) against a hoshiya-serve built from this tree.
- CI (`OFFICE_TARGET=E:/office-target/p7-sign`): `queue` ok (153 patches, 153 rows), `gen-check` ok (lockdiff x4 green), `deny` ok (root + 4 domains), `i18n` ok, `plugins` ok (matrix 140 identical, fields, evil host/executor/docserver, Playwright plugins-host 16/16, bridge-plugins 10/10 on the phase-7 docs-web dist, template; evil-desktop skipped: no release docs.exe), lint subset ok (root fmt + clippy `-D warnings`, services clippy for suite-desktop, suite-serve, docserver), `test-ours`: root 331 passed, sheets 56, slides 47, pdf 74 passed; docs 213 of 214, the failure `docs-office layer_tests::a_busy_history_store_does_not_hold_up_the_save` (a 2 s timing bound, history code untouched, PC shared with other builds) passed 3 of 3 reruns.
- Not run: the APK on the emulator (compiled with `javac --release 11` and dexed with `d8 --min-api 26`, sigcheck on a desktop JVM only), `docs.exe` with a signed or removed plugin, the docserver wrapper Playwright spec with the newly signed fixtures, the upstream clippy ratchet (upstream code unchanged, only lock files). The Windows file dialog / WebView2 window of the Plugins page is still not run on a real desktop.
- Owner decisions: (1) the server refuses every unsigned package, so until the project key exists an admin must sign the first-party plugins with an own key (the tool) to use them on a server; alternative: count the admin's SHA-256 pin as enough on the server. (2) The project key itself (make it offline, put the public half in `brand.toml`), with the Phase 10 signing identities.

### Phase 7 WP13: Sheets and Slides plugin adapters (2026-11-06, branch `p7-wp13`)
- Owner decision O2 = Docs + Sheets + Slides (2026-11-06, "fix everything"); PDF stays later.
- Built:
  - `crates/plugin-adapter` (new, root workspace): the app-neutral half of the two adapters, modelled on `docs-plugins` (which is
    unchanged): the loaded plugins and 64 slots (`register_command` has no remove, so a registry id keeps its slot for the
    process; the slot looks its plugin command up by id on every run), the executor over an `Editor` trait (one undo step,
    restore on any error, stale check, protected-document refusal, 16 reads / 16 rounds / 8 fetches / 10 s, three timeouts switch
    the plugin off), the web bridge (pending invocation, input lock flag, timeout, panel clicks, busy, ended ids) and its JS end
    (`<prefix>:plugin*`), and `native` (wasmtime guest + suite-core store loader, filtered by app).
  - `domains/sheets/crates/sheets-plugins` and `domains/slides/crates/slides-plugins` (domain extensions, `office-ext`): the
    engine sides (text maps through the built-ins' own walkers, the undo step, the restore, the slot tables, the web input lock as
    the engine's command hook, the toolbar menu).
  - `plugin_api::allow::{SHEETS, SLIDES, list, check_run_in}` and `action::check_actions_in` (Docs keeps `check_run` /
    `check_actions`, unchanged). Sheets refuses any params string it would read as a formula (external references, WEBSERVICE,
    HYPERLINK, DDE); Slides refuses `hover` and `action` on top of `link`.
  - Carried patches gridcraft/0157 (the translit cell walk and write as functions, no behaviour change), gridcraft/0158 (toolbar
    Plugins menu, desktop loader, web forwarding), deckcraft/0157 (walk, pieces and write as functions, no behaviour change),
    deckcraft/0158 (same seams). Domains: `[extensions]` in `domains/{sheets,slides}/domain.toml`, regenerated.
  - `plugins/uz-translit`: `apps = ["docs", "sheets", "slides"]`; "N cells converted" in Sheets (the reviewed gridcraft row, copied
    with its status); `auto` in Slides detects on what the built-in detects on. template-fields stays Docs only.
  - Strings: "Plugins" in `i18n/gridcraft.tsv` and `i18n/deckcraft.tsv` (the wordcraft row's wording),
    `reviewed:agent-opus:2026-11-06`.
  - Docs: `docs/plugins.md` section 6a and the rest, `docs/phase7/malicious.md` "Sheets and Slides" table, `scripts/ci.sh plugins`
    runs the two adapters' tests with the real module.
- Safety (each with a test; table: `docs/phase7/malicious.md` "Sheets and Slides"): allowlist checked before the first action,
  one undo step per invocation (natively Sheets folds its own snapshot, Slides uses the slot's `Session::execute`; on the web both
  fold), restore of document, selection and undo/redo on any error (Sheets also rebuilds the calculation chain), stale check (Slides:
  a content revision that ignores selection changes), protected workbook refused for writing plugins, limits (16 reads, 16 rounds,
  8 fetches, 10 s, three timeouts switch off), web input lock (every changing command, `file.*`, `edit.*`, `plugin.*`, `office.*`
  refused while a call waits; selection moves allowed), panel click busy/ended/protected refusals.
- Tests: `plugin-adapter` 25 (fake editor under the Sheets allowlist: M3, M4, M5, M5a, M8, R1, S2, rounds, timeouts, protected,
  panels, and 10 web-bridge cases); `sheets-plugins` 9 + equivalence; `slides-plugins` 8 + equivalence; `plugin-api` +1
  (`per_app_allowlists_and_the_formula_rule`, 35 in `api.rs`); `uz-translit-plugin` +1 (4). The allowlists against the real
  registries: Sheets 75 of 344 commands, Slides 87 of 324. Built-in tests unchanged and green after the walker refactors:
  gridcraft-engine `tests/translit.rs` 13 passed, deckcraft-engine `tests/translit.rs` 11 passed.
- **Output (PLAN 6.3) equals the built-in:** `sheets-plugins/tests/equiv.rs`: 2 xlsx fixtures + a generated workbook (2,000 x 5 and a
  second sheet) x {to Cyrillic, back to Latin, auto} x {auto, selection, workbook} x {the plugin's code natively, its wasm module in
  wasmtime} = **54 of 54 identical** (workbook in memory, saved xlsx bytes, selection, cells written; undo walks back to the opened
  workbook). `slides-plugins/tests/equiv.rs`: 2 pptx fixtures + a generated 40-slide deck (fields, a break, a link, ru/de/uz tags,
  notes, a digraph across runs) x 3 commands x {auto, slide, deck, selected shapes, a text range} x 2 guests = **90 of 90
  identical** (presentation, selection, paragraphs; undo restores). Module: `uz-translit-1.0.0.zip` 253,170 B (was 252,303),
  SHA-256 f5bcb498...c1c1.
- **Web size (stop condition hit, budgets NOT raised):** `scripts/build.sh <d> web --release`, `brotli -q 11 --lgwin=24`, against the
  phase-7 CI dist (`E:/office-target/phase-7/dist`, built at 3e33e99 = this branch's base code):
  - Sheets: 4,953,717 -> **5,044,570 B (+90,853)**; budget 4,966,771 (base_br 4,737,395 + delta_br_max 229,376): base headroom
    13,054 B, **over by 77,799 B**. Raw 19,758,560 -> 20,294,933 (+536,373). JS glue 20,055 -> 20,146 B (max 24,576, ok).
  - Slides (with the craft fonts, as the budget row says): 9,510,848 -> **9,596,663 B (+85,815)**; budget 9,524,217 (9,327,609 +
    196,608): base headroom 13,369 B, **over by 72,446 B**. Raw 36,104,738 -> 36,513,882 (+409,144). JS glue 19,987 B (ok).
  - For scale: the same plugin system cost Docs +83,019 B brotli at WP4 (200,313 -> 283,332), where the budget was re-baselined by
    the budgets.toml rule. Owner decision needed (below); `ci.sh sizes` would fail on this branch.
- Not done / unverified: Playwright with the real page host inside a Sheets or Slides frame (the editor end of the bridge is
  tested natively; the page host is unchanged), the docserver wrapper and the Android app with these editors, a desktop run of
  sheets.exe / slides.exe with an installed plugin (the loader is the Docs one, tested through `wasm_guest`; no exe was started),
  native panels and events (not in v1), a generated command list like `docs/plugins-commands.md` for Sheets and Slides (the
  allowlist tests print the counts), N1/N2 speed numbers for Sheets and Slides.
- Owner decisions: (1) the Sheets and Slides web budgets: raise `delta_br_max` by the budgets.toml rule (measured delta + 10 %,
  16 KiB steps), or keep the plugin bridge out of the web builds (plugins on the desktop only for these two editors), or ask for a
  slimmer web path first; (2) the formula refusal in Sheets (a plugin cannot write `=...`): keep for v1 (recommended) or allow
  formulas without external references later.
- CI on this branch (`scripts/ci.sh`, OFFICE_TARGET=E:/office-target/p7-wp13): `queue` ok (verify-queue all five trees, 153 rows),
  `gen-check` ok (gen-domain, plugin-commands, lockdiff docs/sheets/slides/pdf: 0 foreign additions), `deny` ok (root, docs,
  sheets, slides, pdf), `i18n` ok (lint, roundtrip, check-stale), `lint`: fmt root and upstream ok, clippy root ok, upstream
  ratchet wordcraft/deckcraft/pdfcraft ok, gridcraft had 1 new warning (a needless borrow in 0158), fixed in the next commit and
  re-checked with clippy on the touched crates. `plugins`: every cargo step ok (pack x3, plugin tests, Docs plugin-matrix and fields with the changed module, evil-host with plugin-adapter, plugins-sheets 54/54, plugins-slides 90/90, evil-executor, evil-docserver), then infra exit 2 at the Playwright specs (no shell or docs-web dist in this target), so the template step after them did not run; evil-desktop skipped (no release docs.exe). `sizes`: not run as a job (it needs all four dists);
  the two editors were measured with the job's own commands (above). Not run: `test gridcraft`/`test deckcraft` whole suites (run instead: both translit test files, gridcraft-ui-egui `tests/office_toolbar.rs` 5 passed, deckcraft-ui-egui `office_toolbar` 7 of 8: `the_pill_does_not_cover_a_tab_at_1024` fails the same way with the base `office_toolbar.rs`, "pill starts at 889 but the tabs end at 922", with the craft fonts in the tree, so not from this WP); `web-smoke`, `shell`, `docserver`, Android.
- Budgets after WP13 (merged 2026-11-06): `[sheets] delta_br_max` 229,376 -> 344,064 and `[slides]` 196,608 -> 311,296, by the
  measured + 10 % rule (as Docs at WP4), because O2 now includes Sheets and Slides. Measured on the p7-wp13 dist before the p7-sign
  merge; `ci.sh sizes` in the next full CI is the check.

### Phase 6 WP5 done-check and WP13 checklist (2026-11-11, branch `p6-check`)
- Full write-up and the table: `docs/phase6/checklist.md`; evidence `docs/phase6/evidence/checklist/` (13 MB: 12 short mp4, 31 png). AVD `HRM_Pixel_8` (412 dp = density 210, 360 dp = density 240, both at `wm size 540x1200`),
  Chrome 133 and the WP8 app (rebuilt over the `p6-tapfix` dist, 53,208,707 B). Docs only. The host was saturated (Chrome ran at about 12 Hz), so timing-dependent gestures are weaker evidence than on a phone.
- **Done:** 40 checklist rows: PASS 20 (6 are older WP6/WP7/WP8 evidence), FAIL 11, INCONCLUSIVE 1 (autocorrect), BLOCKED 4, not run 4. PASS: long press selects the word and opens the bar (412/360, Chrome and app),
  Gboard glide, Cyrillic typed over a selection, momentum scroll, rotation keeps the document, the contextual bottom toolbar.
- **WP5 is NOT done: selection handles and the action bar cannot be used by a finger on the emulator** (Chrome and app, 4 surfaces, DPR 1.0 and 1.31/1.5, two injectors): a press on a knob or a bar button clears the selection and does nothing
  (Cut left the text in place). The 5 kittest tests of wordcraft/0139 pass (re-run below), so the headless harness does not see it. Not root-caused; suspects and file:line in checklist.md F1 (the bar's plain `egui::Button::clicked()` is the one WP6 already found broken on the web).
- Other findings: a pinch drag-selects and raises the keyboard (F2); a table wider than the phone is clipped at 360 and 412 dp (F3); the first long press after opening often does nothing (F4); landscape with the keyboard up shows no document (F5).
- **Measured:** `ui-egui --lib` 734 passed, 8 failed, 2 ignored (`E:/office-target/p6-check/wc-test`, 47 min under load): the 7 known i18n failures (cs, hu, hr, pl, tr, vi, `untranslated_interface_text`) plus `backstage::...recovery_restore_uses_save_changes_and_resets_autosave_permission`, which passes alone (load flake). Mouse regression: the 0139 tests
  `long_press_opens_the_bar_not_the_menu_and_the_mouse_is_unchanged` (right-click menu, a mouse draws no handles) pass and no other ui-egui mouse test changed result; no browser mouse run today.
- **Sizes, 0139 alone** (two `docs web --release` builds of `4362fb03^` and `4362fb03`, `brotli -q 11 --lgwin=24`): Docs wasm 9,862,495 -> 9,850,956 B (**-11,539 B**, raw 45,162,887 -> 45,027,097); JS 20,773 B and html 868 -> 869 B unchanged.
  A patch that adds a 444-line module cannot shrink the wasm: this is codegen/`wasm-opt` variance, so the true delta is below the noise (about 10 KB); no budget change needed. Current tapfix build: 9,875,600 B of 9,904,416.
- **Not checked:** a real phone; Sheets, Slides, PDF; page-break handle drag (Web Layout has no pages; the drag fails first); autocorrect after a handle move; Cyrillic over a handle-made selection; the bar on C412, C360, A360; a desktop-browser mouse run; WP16.

### Phase 7 leftovers: Sheets/Slides in the real frames, N1/N2, desktop malicious column (2026-11-11, branch `p7-left`, not merged)
- **Done.**
  - `ci/web-smoke/bridge-plugins-apps.spec.ts` (+ config): the real page host, a Worker, the fixture plugin and the real uz-translit module inside the **Sheets and
    Slides frames** (built sheets-web, slides-web of this tree). The editors have no `run` message, so commands start through the toolbar's Plugins menu (clicks
    on the canvas). **12 of 12** (6 per app): registration, `hello`, M5/R1/M8, M6/M7/S1/M9 (and the lock released afterwards), M4 redirect, real translit through the
    text map with one-step undo. `ci.sh plugins` runs it when the two dists exist.
  - `docserver.spec.ts` "plugins in the wrapper (Sheets and Slides)": a docserver of this tree, a signed fixture plugin per app, files/2 (xlsx) and files/6 (pptx):
    `hello`, M5, R1, M7, M6 (timers kept), M9, saved text equals the opened one. 1 of 1 (both apps). The other docserver specs were not run.
  - Generated command lists `docs/plugins-commands-sheets.md` (75 of 344) and `docs/plugins-commands-slides.md` (87 of 324): a test in each `*-plugins` crate writes and checks
    them, `cargo xt plugin commands [--check]` now runs the three tests (Docs, Sheets, Slides).
  - Desktop malicious column on a **release `docs.exe`** built from this tree (`scripts/plugins/evil-desktop.py`, extended; PASS in 19 s): M5 `deny`, R1 `many`, M6 `spin`
    (stopped after 6.3 s), M7 `grow` (capped, "hog capped"), M9 `xss` (panel refused: "widget text too long ..."), M1/M2/M2b (four refused packages: no command registered,
    each named on stderr), S1 `overflow` (trap, the process lives, `hello` works after); the document text unchanged after each. `docs/phase7/malicious.md` filled; the CI
    step `evil-desktop` is the same script and no longer skips with this exe.
- **Tests.** `sheets-plugins` 10 lib + equivalence **54 of 54** (native + wasm); `slides-plugins` 9 lib + equivalence **90 of 90**; the two generated lists current.
- **Measured (release, PC shared with other builds in the first runs, quiet in the final N1; median of 5, alternating, uz-translit wasm in wasmtime vs the built-in command).**
  - N1 native, headless (`sheets-plugins/tests/perf.rs`, `slides-plugins/tests/perf.rs`, `--ignored`; the command only: these editors have no separate relayout step):
    Sheets 2000 x 5 (8,000 text cells): to Cyrillic 38 -> 62 ms (+24), to Latin 45 -> 59 (+14) = pass (<= +50 ms); **20,000 x 5 (80,000 text cells): 405 -> 675 ms (+270, +67 %) and
    334 -> 568 (+234, +70 %) = FAIL**. Slides 40 slides: 6 -> 31 ms (+26) and 2 -> 7 (+4) = pass; 400 slides x 8 paragraphs (3,200): 31 -> 72 (+41) and 24 -> 65 (+41) = pass.
    The first (loaded) run: Sheets 10k cells +110/+117 ms, 100k +694/+2055 ms; Slides 400 +62/+54 ms. Reading: a steady cost of about 3 us per text cell (wasm convert
    1.6-1.8x native, text map, the apply's check per cell), so the +50 ms line is crossed at about 15,000 text cells.
  - N2 desktop, release `sheets.exe` / `slides.exe`, control channel (`scripts/plugins/perf-desktop-apps.py`, fresh process per mode, document built through the channel, 4 timed
    rounds after a warm-up): **Sheets 20,000 x 5: command 385 -> 717 ms (+333), next frame +85 %, painted (`ui.screenshot`) 946 -> 1,266 ms (+34 %) = FAIL** (same cause as N1).
    **Slides 400 slides: command 31 -> 68 ms (+37), next frame 52 -> 90 ms, painted 592 -> 634 ms (+42 ms, +7.1 %) = pass.** Raw: `E:/office-target/p7-left/n2-*.json` (not committed).
  - `ci.sh sizes` on the dists of this tree (docs, sheets, slides built here, pdf copied from the phase-7 dist, which this branch does not change): **green** (exit 0), measured brotli against budget: docs 9,960,384 / 10,002,720 (headroom 42,336), sheets 5,054,684 / 5,081,459 (26,775), slides **with the craft fonts** 9,599,928 / 9,638,905 (38,977), pdf 9,570,303 / 9,599,462 (29,159, old dist), shell 65,468 / 81,920, plugins host 15,038 / 16,384; JS glue under 24,576 for all four. Slides was first built without `upstream/craft-fonts` (not in the worktree; 7,112,228 B, not comparable) and rebuilt with the fonts copied from the main checkout.
- **Findings.**
  1. Sheets plugin speed misses the PLAN 6.3 line on big sheets (above). No defect found in the adapter loop (`sheets-plugins/src/lib.rs` `read_map`, `apply_map`): the cost is linear; a decision
     for the owner is whether v1 accepts +50 ms to +0.3 s on 100k-cell sheets, or a bulk text-map path is built. Not changed.
  2. The wrapper's Sheets menu layout depends on the frame width (the overflow list changes at 1200 px), so the specs fix the window at 1100 px and the menu positions are pixels from screenshots.
     The editors still have no `run` message (Docs has `wordcraft:run`), which is why the commands are started by clicks; a `gridcraft:run`/`deckcraft:run` message would make these specs robust (a carried patch; not made here).
  3. The Slides control channel and the Sheets one list no plugin commands in `engine.commands` (they live in a separate registry); the N2 script probes with `engine.execute` and an invalid scope instead.
- **Not checked.** Android with Sheets/Slides; M1, M2, M2b, M3, M10-M12, S2, F5, F7b and a formula/`link` answer (M5a, M5b) in a browser frame (fixture has no such answers; native tests only);
  `sheets.exe`/`slides.exe` against the evil fixtures; the other docserver specs; panels on the desktop (not in v1); signing-key items; the Sheets formula decision (owner).
### Phase 6 WP5 fix: handles and the action bar by finger (2026-11-12, branch `p6-handles`, wordcraft/0186)
- **Root cause of checklist F1 (also F6, half of F2):** Chromium fires `pointerdown` before `touchstart`; eframe 0.36 presses the primary pointer for both and runs a
  frame at once from `pointerdown` with no `Touch` event, so the page took every finger press first as a mouse press (handles off, caret placed under the finger) and
  then counted the `touchstart` press as a second click. Isolated on the old wasm by injecting only a capture-phase `pointerdown` stop: the handle drag and Cut worked.
  The other half of F2: eframe presses the primary pointer again for a second finger's `touchstart`.
- **Fix (wordcraft/0186, 5 files +108/-3):** `web.rs` `finger_presses_once` (a finger's `pointerdown` on the canvas never reaches eframe); `canvas::mouse` stands aside
  during a multi-touch gesture; `touch_select` takes no handle press from a pinch; `TouchScroll` counts a pinch as a scroll (no keyboard from its drag). Two ui-egui tests.
- **Emulator re-run** (`docs/phase6/checklist.md`, section "Re-run 2026-11-12"): handle drag, Cut / Copy / Paste / Select All, pinch without selection or keyboard,
  first long press after load and 6/6 taps placing a caret: PASS on C412, C360, A412, A360 (rows as listed there). Still open: rows 4b/6b (Gboard after a handle move),
  F3 (wide table), F4 on a real phone, F5; the bar can cover the end knob of a multi-line selection when it sits below (low, new).
- **Measured:** Docs wasm 9,872,684 B brotli of 9,904,416 (headroom 31,732; `ci.sh sizes` ok, no budget change); APK 53,188,227 B. ui-egui touch tests 12 passed;
  `a_pinch_neither_selects_nor_takes_the_keyboard` fails without the canvas/touch_scroll part. `ui-egui --lib` 737 passed, 7 failed (the 7 known i18n failures), 2 ignored.

### Phase 5: real install on the HRM test server (2026-11-08, owner OK)
- Image `hoshiya-docserver:p5-53da47d5` (services of phase-5 53da47d5, dists of the p5-ro build), tarball 117 MB, SHA-256 `4a6a00a3…`.
  `install.sh --profile profiles/hrm.env` as root: the old ONLYOFFICE 9.3.1 container is kept as `docs-server-replaced`
  (stopped); ours runs as `docs-server` on `backend_net` with both names, shared :80, memory 4096 MB, healthy. Through Traefik
  `doc-editor`'s `api.js` answers 200 (ours); `/ds/files/` 404 from outside; a stranger on the network gets 403. The backend calls no
  ONLYOFFICE service endpoint (healthcheck, CommandService, ConvertService: none in its code), only the callback and the saved file.
- First attempt failed and showed two bugs, both fixed and rehearsed (54 of 54): (1) a backend container name with an underscore
  was refused by the config (`NameSet::is_name`, a5cc8a49 rewritten as 24173606); (2) the rollback then left the old container on
  no network, so the editor was down about 15 minutes until it was rejoined by hand (same IP, both aliases, healthcheck true from
  the backend, 200 through Traefik): Docker 29 prints the empty IPAMConfig that compose leaves as "invalid IP", which the state kept
  as a fixed address (53da47d5). The rehearsal now mirrors both (underscore backend, compose-like rejoin); the old install.sh fails
  3 of its rollback checks.
- Not done: a real HRM open/edit/save by a person (docx, xlsx, pptx, pdf, view-only), Android and iOS app. The backend's compose
  project still defines `docs-server`: a `docker compose up` there brings the old one back (install.sh prints the override; we do
  not edit HRM files). Rollback: `sudo ./install.sh rollback --etc /opt/docserver` in `~/hoshiya-install`.

### Phase 5: host-mode engines (2026-11-09, branch `p5-hengines`; details in docs/phase5/hengines.md)
- Done: Slides follows the browser language without `?lang=` (deckcraft 0171); English left on the host-mode screens translated (deckcraft 0172,
  gridcraft 0171 with 13 new `reviewed:agent-sonnet:2026-11-09` rows); autosave to the host 3 s after the last edit, 30 s ceiling, one copy per
  changed state (deckcraft 0170, gridcraft 0170, pdfcraft 0130); Sheets and Slides read the toolbar mode through suite-core on the web
  (`?toolbar=full` gives upstream's ribbon; gridcraft 0172/0173, deckcraft 0174/0175; PDF has no full mode); the Slides Font box shows the theme
  font when nothing is selected (deckcraft 0173, 0176).
- Measured (dists built from the merged branch): bridge-sheets 54 passed, bridge-slides 37 passed, bridge-pdf 40 passed. Sizes (brotli, headroom to budget): sheets 4,958,840 (7,931 B), slides 9,522,246 (34,739 B; delta +194,637 with the fonts embedded),
  pdf 9,572,670 (26,792 B). No budget changed.
- Not done: Sheets Watch Window and the Slides Backstage were not walked for English; Docs unchanged (p5-hdocs).

### Phase 5: Times New Roman as the default for HRM's blank files (2026-11-10; branch `p5-font`, from phase-5 a54fcd10)
- Owner request: the default font is always Times New Roman. HRM creates blank files server-side (python-docx `Document()`, openpyxl
  `Workbook()`, python-pptx `Presentation()`), which opened in Cambria / Calibri. New documents made by the editors were already Times
  New Roman (wordcraft 0135, gridcraft 0151, deckcraft 0152); the gap was a file that is empty when opened.
- Done: **wordcraft/gridcraft/deckcraft 0185** (one patch each, engine only, no UI strings, no host-bridge file touched). Rule: content
  at open means no change; an empty file gets Times New Roman in memory only, so the session is not dirty, no autosave is sent, and the
  file changes when the user edits and saves. Docs: no text, table or picture: docDefaults run font, body theme font and a Normal that
  has its own font (`io::default_font_when_empty`, wordcraft). Sheets: no cells, charts, pictures or shapes in any sheet: the Normal
  style font and the theme body font (`StyleTable::set_default_font_name`). Slides: a PPTX whose slides hold only empty placeholders
  and empty notes: heading and body Latin font of every master's theme. Sizes are not touched: a python-docx blank is 11 pt (Normal,
  not 14 pt), openpyxl 11 pt, python-pptx's template text sizes; no engine setting pairs font and size, so 12 pt/14 pt stays a
  separate owner decision. Headings in a python-docx blank keep their own theme heading font (Calibri), as the rule is body text.
- Measured: engine unit tests (synthetic bytes, an empty file changes, a file with text does not): wordcraft
  `an_empty_document_opens_in_times_new_roman_without_being_dirty`, gridcraft `an_empty_workbook_opens_in_times_new_roman_and_one_with_content_keeps_its_font`,
  deckcraft `an_empty_deck_opens_in_times_new_roman_and_one_with_text_keeps_its_font`: 1/1 each. bridge-docs 14 passed (new case 13: a
  python-docx-style blank, Cambria theme font, opens with the Font box on Times New Roman, no dirty message, no autosave, then dirty when
  typed in; screenshot `docs/phase5/evidence/docs-default-font-times-new-roman.png`), bridge-sheets 54 passed, bridge-slides see the
  summary below. Sizes (brotli, headroom): docs 9,853,908 (17,740 B), sheets 4,958,424 (8,347 B), slides 9,512,363 (44,622 B); no budget
  changed. `verify-queue.sh all` and `cargo xt register check` (171 rows) green; 0950 of wordcraft and deckcraft regenerated.
- Rendering: Times New Roman is not bundled. Docs web draws it with the bundled Source Serif 4 (serif, not metric-compatible, as the
  tables entry above); Docs desktop uses the system Times New Roman, else Liberation Serif or Tinos. Sheets web and Slides web have no
  serif face embedded, so the name falls back to the bundled UI sans there (a visible gap; embedding a metric-compatible serif such as Tinos is a size-budget decision, not measured); Sheets desktop and Slides desktop use the system font.
- Not checked: a real python-docx/openpyxl/python-pptx file (the tools are not installed here; fixtures are synthetic and hand-built);
  desktop MSI; a Sheets or Slides screenshot (the unit tests cover the model, not the web render); an empty `.docx` that has a table
  or a picture is treated as content by design.

### Phase 5: a serif face for Times New Roman in Sheets web and Slides web (2026-11-10; branch `p5-serif`, from phase-5 b130b25e)
- Owner request: Times New Roman is the default font everywhere. Sheets web and Slides web embedded no serif, so the name drew in the
  bundled UI sans (see the p5-font entry above). Docs web already draws it with Source Serif 4; desktop builds use system fonts and are not changed.
- Chosen: **Tinos Regular** (Steve Matteson; metric twin of Times New Roman; v1.340, google/fonts `5bb28733cec313b63fbd415b574b5ae35c8aa8b8`
  `ofl/tinos/Tinos-Regular.ttf`, SHA-256 `60a0e8ef0c04dd5dd69ffe91025fa2ae5836cbd35600a82ba031977557e2cb61`), SIL OFL-1.1 (the current Tinos
  is OFL, not Apache-2.0; `deny.toml` already allows OFL-1.1; text `assets/fonts/fallback/OFL-Tinos.txt`, NOTICE entry via `scripts/gen-legal.sh`). Metric-compatible, so lines
  break as in Word. Regular only (the engines fake bold/italic); subset with the existing `scripts/subset-fallback-font.sh` (Latin, Latin-1, Ext-A/B,
  U+02BB/U+02BC, Cyrillic block, punctuation; hinting dropped): 74,024 B raw, 37,251 B brotli. The check covers oʻ gʻ and ҳ қ ғ ў.
- Done: **gridcraft/0186** (the third overlay face `office-fallback-serif` leads the grid's serif family behind an installed serif; "Times",
  "Times New Roman", "Cambria", "*serif*" already map to that family) and **deckcraft/0186** (face registered in the slide font database; `substitutes`
  for Times New Roman, Times, Liberation Serif and Cambria name Tinos; egui interface stacks never use it). Brand: `brand.toml` `fonts.fallback_serif`; `FONTS` carries
  the third file only for sheets and slides (docs and pdf build scripts name every later entry mono). Unit tests: gridcraft `times_new_roman_cells_resolve_to_the_bundled_serif`,
  deckcraft `times_new_roman_slides_get_the_bundled_serif`, office-brand `only_sheets_and_slides_carry_the_serif_face`.
- Measured (release web builds, brotli -q 11 --lgwin=24): sheets 4,958,424 -> 4,995,050 (+36,626); slides 9,512,363 -> 9,552,378 (+40,015). **Sheets budget raised**: `delta_br_max`
  229,376 -> 294,912 (delta now +257,655; ceil(1.10 x delta / 16 KiB) x 16 KiB); slides stays 229,376 and fits with **4,607 B headroom** (the next slides change needs a raise). bridge-sheets 55 passed (new case 15),
  bridge-slides 38 passed (new case 15). Screenshots: `docs/phase5/evidence/sheets-serif-names.png`, `slides-serif-names.png` (Times New Roman, Times, Liberation Serif,
  Cambria in serif; Arial in sans for contrast; fixtures `ci/web-smoke/fixtures/sheets-serif-names.xlsx`, `slides-serif-names.pptx`).
- Not checked: desktop builds (unchanged by design: an installed serif still leads); the PDF export of Sheets (it keeps its own default face); Docs; Georgia, Garamond and the like
  still use the sans in Slides and the generic serif role in Sheets, which is now Tinos; Liberation Serif/Tinos bold and italic are synthesised, not drawn from real faces.

### Merge forward: phase-5 5b752c69 into phase-6 (2026-11-12; p5-save, p5-hdocs, p5-hengines, p5-author, p5-font, p5-serif, p5-flush, p5-idle)
- Merge 898193ec (+ 78b4cbb0 REGISTER). Conflicts: wordcraft-web host.rs (phase-5's `unsaved_bytes`, used by `__hostFinalCopy`, now calls
  phase-6's `office_overlay::autosave_bytes` of 0146); docserver editor.js (the Docs palette bar stays, History is phase-5's floating
  icon); shell.js (download path keeps the phone toast and adds the `saved ok:true` ack; the Android `hostSaved()` also acks, ok true or
  false, so the R6 bridges clear or keep their mark there too); REGISTER. No patch number collided. wordcraft 0160, 0161, 0164
  regenerated over the phase-6 queue (context), wordcraft 0950 regenerated with rustfmt.
- Measured: `verify-queue.sh all` OK (wordcraft 47, gridcraft 72, deckcraft 47, pdfcraft 22); register 188 rows OK; `ci.sh queue lint i18n`
  ok (upstream clippy: only baselined warnings); services fmt, clippy -D warnings, `cargo test` docserver/fake-hrm/suite-serve 113 passed;
  Playwright bridge-docs 15, a11y-palette 4, shell 21 passed (7 skipped). Sizes: Docs wasm 9,866,631 B brotli (headroom 37,785).
  **Shell 65,707 B brotli, 171 B over 65,536**: br_max raised to 81,920 by the rule (phase-7's value).
- Not checked on phase-6: Sheets/Slides/PDF web builds, their bridge specs and docserver.spec (run on phase-7, which carries all of this);
  the Android `hostSaved` ack on a device or emulator.
- Follow-up bfe1aba1 (found by docserver.spec on phase-7): the History icon floated over Docs' title row, which the palette bar moves
  down (colour spread 25 under it); it now sits in the Docs palette bar after Commands (bar padding 4 px); spread 0 in the rerun.

### Merge forward: phase-6 (with phase-5 5b752c69) and p7-left into phase-7 (2026-11-12)
- Merges c633cd8f (phase-6), a7d906f6 (p7-left), fde6e097, 12425773, 51180060 (phase-6 follow-ups). Patches: phase-5's suite-core web-sys
  lock patches renumbered after phase-7's own lock patches (wordcraft 0901 -> 0903, gridcraft 0901 -> 0902, deckcraft 0901 -> 0902, pdfcraft
  0901 -> 0903) and regenerated; gridcraft 0170, deckcraft 0171 and 0177 regenerated over the plugin patches; the four 0950 rustfmt patches
  regenerated. docserver: the session answer carries `lang` and `plugins`; config has `ui_lang`, `close_idle_minutes` and the plugin tables.
- **Found by the merge and fixed (522055c1):** phase-5 makes suite-core a wasm dependency of the web editors (toolbar mode), phase-7 gave
  suite-core `ring` (signatures): docs/sheets/slides web did not build (ring's C code needs clang for wasm32). ring and the plugin store
  (`suite_core::plugins`, its only user) are now native only; suite-core 62 tests pass, wasm32 check clean.
- **Found, not fixed (owner decision):** the docserver wrapper opens Sheets and Slides with the full ribbon (p5-hengines), and their Plugins
  menu exists only in the simple toolbar (gridcraft/deckcraft 0158). A user reaches the plugins there only after switching to the simple
  toolbar. The wrapper's Sheets/Slides plugin test now saves the simple-toolbar choice first. Options: a Plugins entry in the full ribbon,
  or the simple toolbar in the wrapper when the provider has plugins.
- Measured: `verify-queue.sh all` OK (wordcraft 52, gridcraft 76, deckcraft 51, pdfcraft 24); register 203 rows OK; `ci.sh queue lint i18n`
  ok (only baselined upstream clippy warnings); docserver fmt, clippy, cargo test ok; Playwright bridge-docs 15, bridge-sheets 55,
  bridge-slides 38, bridge-pdf 40 passed; docserver.spec 37 passed + 4 that failed under load (History close poll 5 s x3, xlsx HRM Back's
  first key) and passed on rerun, 1 skipped, the Sheets/Slides wrapper plugin test passed; `ci.sh plugins` ok except bridge-plugins-apps,
  whose Sheets menu positions moved (the simple toolbar now ends with the Full ribbon switch): remeasured, 12 of 12 passed.
- Sizes (`ci.sh sizes` exit 0): Docs 9,949,350 (headroom 53,370), Sheets 5,095,308 (delta +357,913, over 344,064: `delta_br_max` raised to
  409,600 by the rule; headroom 51,687), Slides 7,156,490 (built without craft fonts, fonts=unset), PDF 9,573,286 (headroom 26,176), shell
  65,944 of 81,920, plugins 15,038 of 16,384.
- Not checked: the docker rehearsal; evil-desktop with a fresh release docs.exe (the job used the one in the target); a real device for the
  mobile patches merged here.

### Docs: Ctrl+S on a read-only document (2026-11-12; branch `p5-rosave`, HRM test finding)
- **Bug.** Docs opened read only by the host (`readOnly: true`), Ctrl+S: the status bar read `{name} yuklab olinmadi: the document is open read only` (uz-Latn): the "Couldn't download" wrapper (wrong
  verb, nothing was downloaded) around an English reason from the bridge's download hook (`apps/wordcraft-web/src/web/host.rs`, not in any catalog).
- **Fix (wordcraft/0165).** `WordApp::run` (`crates/ui-egui/src/lib.rs`, the web save/export branch) asks the host `blocked("document.readOnly")` first; the bridge answers from its read-only flag. The
  status bar and the error are now the catalog row `This document is open read-only.` (uz-Latn `Hujjat faqat oʻqish uchun ochilgan.`, uz-Cyrl, ru; `reviewed:agent-sonnet:2026-11-11`; xtask `OWN_R6` keeps the row
  extracted). Nothing is downloaded or posted to the parent. Sheets, Slides: already a translated read-only notice (gridcraft/0160, deckcraft/0160), unchanged. PDF: `Couldn't save {name}: Not allowed`, both
  parts translated and the verb is right, unchanged (the reason is generic, shared with the withheld-download case).
- Tests: ui-egui `a_save_of_a_read_only_document_says_so`; `bridge-docs.spec.ts` case 15 (read-only open, Ctrl+S: no `wordcraft:save`, no `wordcraft:failed`, no download; the notice itself is drawn on the canvas, so
  the Playwright case cannot read its text).

### Sheets and Slides: Plugins tab in the full ribbon (2026-11-12; branch `p7-ribbon`)
- Closes the owner-decision finding of the phase-7 merge (the docserver wrapper opens Sheets and Slides with the full ribbon, p5-hengines, and
  their Plugins menu existed only in the simple toolbar). Carried patches `gridcraft/0187` and `deckcraft/0187` (REGISTER rows; the 0950
  patches unchanged, the new lines are rustfmt-clean): a "Plugins" tab after the built-in tabs of the full ribbon, shown exactly while
  `office_overlay::plugin_menu()` is non-empty (the condition that shows the simple toolbar's menu). Sheets: one ribbon button per plugin with
  its commands in a list; Slides: one Plugins button with the same per-plugin submenus as the simple toolbar. The command loop (run, status
  toast or alert) is shared with the simple toolbar's code, not copied. The tab label reuses the catalog row `Plugins` (no new strings).
- The wrapper test (`docserver.spec.ts`, "plugins in the wrapper (Sheets and Slides)") no longer forces `suite.toolbar=simple`: it clicks the
  Plugins tab, the plugin and the command in the full ribbon the wrapper asks for (positions measured on screenshots at the 1100 px window).
