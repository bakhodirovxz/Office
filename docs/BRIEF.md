# Office Suite on the Craft engines: project brief and phased plan

> Owner-provided brief, received 2026-10-15. Stored verbatim as the source of truth for scope,
> constraints and exit gates. Agent working rules are summarised in `/CLAUDE.md`.

> **For the Claude Code agent working in this repository.** Read this whole file before you
> start. Work through the phases **in order**. Each phase ends with an **exit gate**. When you
> reach a gate, stop, report to the owner, and wait for an explicit "go" before starting the
> next phase. Never skip a gate and never merge two phases into one.

---

## 1. Mission

Build an open-source office suite (documents, spreadsheets, presentations and PDF) that:

- runs on **desktop** (Windows, macOS, Linux), in the **web browser** and on **mobile**
  (Android, iOS), with editing on every platform, not only viewing;
- has a **simple, easy interface** for non-technical office staff;
- has a **plugin system**;
- has a **document server** that can replace ONLYOFFICE Docs inside our HRM system
  (embedded editor, open by URL, save by callback);
- includes our own tools: a **file converter** and **Uzbek Latin ⇄ Cyrillic transliteration**.

Build it on the ArtCraft "Craft" apps. Reuse their engines and do not rewrite them.

## 2. Sources

All sources are at github.com/storytold. They are pure Rust under MIT OR Apache-2.0, and all
of them are 2–3 weeks old at the time of writing (2026-10-15).

| Repo | Role in our suite |
|---|---|
| `wordcraft` | Docs: Word-style editor (.docx; .doc import; odt/rtf/html/md/tex/txt; PDF export). Alpha, v0.7.0 |
| `gridcraft` | Sheets: Excel-style spreadsheet (.xlsx; .xls/.xlsb/.ods import) |
| `deckcraft` | Slides: PowerPoint-style presentations (.pptx; PDF/PNG export). Alpha |
| `pdfcraft` | PDF: reader/editor (combine, split, extract, edit, text) |
| `craft-libs` | Shared crates (`craft-ui` for egui, `craft-pdf`, and others) |
| `craft-fonts` | Fonts for the Craft apps |

Ignore the other Craft apps (photocraft, filmcraft, cadcraft, etc.) unless the owner asks.

Owner-provided material:

- `corpus/`: real HRM documents (orders, templates, tables), **anonymised**. Never commit
  personal data, and never copy corpus contents into issues, logs or commit messages.
- `vendor-notes/merge_lines.py`: the post-processor for the PDF→DOCX pipeline (Phase 3).
- `vendor-notes/uz-translit-onlyoffice/`: our existing ONLYOFFICE transliteration plugin.
  It is the seed for the rules in Phase 2.

## 3. What is already known

These facts were measured on 2026-10-15. Re-verify each one; do not take them on trust.

- **Architecture.** wordcraft is ~240k lines of Rust in layered crates:
  `geom → doc / fonts / proof → layout / docx / formats → render / pdf → engine
  (433 commands, each with an id) → mcp → ui-egui`. The README calls the UI layer "swappable".
  Every app exposes a command catalog, a CLI and an MCP server.
- **Web build.** eframe/egui draws everything on one `<canvas>` (WebGPU, with WebGL2 as
  fallback). The wordcraft-web v0.7.0 wasm is **37.9 MB raw / 14.3 MB gzip**. HOSTING.md says
  13 / 5 MB, which is outdated. The web build opens files from the user's disk, and Save is a
  browser download. There is no open-by-URL, no save callback and no auth.
- **Mobile.** The authors say phone and tablet apps are out of scope. Under Playwright mobile
  emulation (390×844, DPR 3, touch):
  - the UI rendered at the wrong scale, with only the top-left of the ribbon visible;
  - swipe scrolling did nothing;
  - typed text was inserted.

  At a desktop-sized viewport it renders correctly.
- **Self-reported gaps** (wordcraft `docs/gaps.md`):
  - DOCX has never been tested on a real-world corpus;
  - charts, SmartArt and OLE objects are dropped silently;
  - pagination differs from Word;
  - there is no co-authoring and no add-in support;
  - there are open crash and hang reports.
- **Upstream velocity.** Dozens of commits a day, mostly AI-agent driven. A deep fork will
  drift fast.
- **Trademark.** The ArtCraft name and logos are NOT open source
  (`docs/brand/LICENSE-brand.txt`). Our product needs its own name and branding. Keep every
  upstream LICENSE, NOTICE and ATTRIBUTION file.
- **PDF→DOCX.** None of the Craft apps converts PDF to editable DOCX. Our tested pipeline:
  - pdf2docx with `parse_stream_table=False` (the default turns justified paragraphs into
    fake tables);
  - then `merge_lines.py` (pdf2docx leaves Cyrillic body text as one paragraph per line).
  - Known weak spots: text drawn over images is lost (signature/QR, stamp), and table column
    widths come out wrong.
  - ONLYOFFICE x2t produces a faithful visual copy, but puts every line in an absolutely
    positioned text box, so it is poor for editing.
- **ONLYOFFICE translit plugin lessons.** On a 108-page document the plugin needs 62 s. That
  time is spent on ONLYOFFICE plugin-API limits (`ReplaceTextSmart`, hex-encoded payloads,
  batching to keep the tab responsive), not on the transliteration algorithm. With direct
  access to the document model this should take well under a second.

## 4. Target for v1.0

These items set the direction. They are not the scope of any single phase.

1. **One product.** One install and one URL. A home screen with Docs / Sheets / Slides / PDF /
   Converter, recent files and templates.
2. **Editing everywhere:** web, desktop, Android and iOS.
3. **Easy UI.**
   - A simplified toolbar by default; the full ribbon is optional, on desktop only.
   - A mobile-first touch UI: selection handles, long-press menu, native keyboard/IME
     (autocorrect, swipe typing, Cyrillic), pinch zoom, momentum scroll and rotation.
   - Accessible.
4. **Document server and integration API**, so our HRM can embed the editor the way it embeds
   ONLYOFFICE today:
   - open by URL and save via callback;
   - a JWT-signed config;
   - permissions (view / comment / review / edit) and a read-only mode;
   - version history.

   Mirror the ONLYOFFICE DocsAPI config shape where reasonable, so switching the HRM is cheap.
5. **Real-time co-editing.**
6. **Plugin system:** sandboxed (WASM), built on the command catalogs, running on every
   platform.
7. **Converter** for all supported formats, plus PDF→editable DOCX, plus transliteration.
8. **Languages.** UI in uz-Latn, uz-Cyrl, ru and en; Uzbek proofing later.
9. **Fidelity.** Real-world docx/xlsx/pptx open and save without silent data loss. When
   something cannot be kept, the user is warned.

## 5. Hard constraints

- **Do not rewrite the engines.**
  - Keep our work as thin layers (new crates and adapters) plus minimal upstream patches,
    ideally ones we could send upstream as PRs.
  - Track upstream as pinned git subtrees or submodules, with a documented weekly sync
    procedure.
- **One Cargo workspace (monorepo).**
  - Deduplicate shared crates (fonts, craft-libs).
  - Resolve dependency version conflicts between the four apps explicitly.
- **Web bundle.** Lazy-load each editor; never ship one combined bundle. Set size budgets from
  measurements and enforce them in CI.
- **Evidence over assertion.** Every claim about performance, fidelity or mobile UX must be
  backed by a measurement or a real-device/emulator test, not by reading code.
- **Clean-room.** Do not copy code from Microsoft Office, ONLYOFFICE (AGPL) or LibreOffice
  (MPL) into this repo. Reading public specifications is fine.
- **Licensing.**
  - Our code and linked dependencies use permissive licenses only; enforce this with
    `cargo-deny`.
  - GPL/AGPL tools (pdf2docx, PyMuPDF) may run only as separate sidecar processes or
    containers called over HTTP or a CLI. Never link or vendor them.
- **HRM safety.** Integration work happens against the HRM **test** environment only. Never
  touch production systems or data.

## 6. Working rules

- **Language.** Talk to the owner in Uzbek. Code, comments, commit messages and docs are in
  English. User-facing UI strings go through the i18n catalogs (uz-Latn, uz-Cyrl, ru, en).
- **First task:** create `CLAUDE.md` in this repo recording these working rules and the phase
  you are in.
- **Git.**
  - Never add `Co-Authored-By` trailers to commits.
  - Do not push, create remote repositories, publish releases or deploy anything without the
    owner's explicit OK.
  - Work on a branch per phase.
- **Sub-agents.**
  - Run independent work in parallel (e.g. one sub-agent per upstream repo).
  - Choose the cheapest model that can do the task well: haiku for search and inventory,
    sonnet for single-module engineering, opus for architecture, hard debugging and the final
    adversarial review.
  - Sub-agent reports are conclusions with `file:line` references, not raw dumps.
- **Measure, don't assume.** If something blocks you (no macOS for iOS, no GPU, no emulator),
  say so plainly instead of guessing.
- **Progress log.** Keep `docs/STATUS.md` up to date: current phase, what is done, what is
  measured, open risks and next step.
- **Report at every exit gate**, in Uzbek. Keep it short, using this structure:
  1. What was done.
  2. What was measured (numbers).
  3. What failed or is still unknown.
  4. Decisions the owner must make.
  5. A proposal for the next phase.

---

## 7. Roadmap: phases in order

| # | Phase | Main result | Depends on |
|---|---|---|---|
| 0 | Evidence and architecture decision | `docs/ARCHITECTURE-DECISION.md` | none |
| 1 | Foundation | Monorepo, upstream sync, rebrand, CI, i18n | 0 |
| 2 | Transliteration (`uz-translit`) | Crate + Docs command + WASM package | 1 |
| 3 | Converter | CLI → HTTP API → UI, PDF→DOCX sidecar | 1, 2 |
| 4 | Unified shell (web + desktop) | Home screen, 4 lazy-loaded editors, simple toolbar | 1 |
| 5 | Document server + HRM integration | Embedded editor in HRM test env | 4 |
| 6 | Mobile | Android, then iOS, with editing | 0 decision, 4 |
| 7 | Plugin system | WASM plugin API; translit ported as first plugin | 4 |
| 8 | Fidelity hardening | Corpus regression harness, no silent loss | 1 (runs continuously from 3) |
| 9 | Real-time co-editing | Spike, then implementation | 5 |
| 10 | Release and operations | Packages, containers, docs, security review | all |

Phase order rationale:

- **Phases 2 and 3 come first** because they deliver value to the owner quickly and do not
  depend on solving mobile.
- **The document server (5) comes before mobile (6)** because HRM integration is the
  business driver.
- **Plugins (7) come after the shell (4)** so that the API is designed against a stable host.

---

### Phase 0: Evidence and architecture decision

**Goal:** get the facts needed to choose the architecture. Write no product code beyond
throwaway spikes, and put those in `spikes/`.

**Tasks**

1. **Inventory.**
   - Clone the six repos into `upstream/` and build each app for desktop and web.
   - Record build time, wasm size (raw / gzip / brotli), test counts and test pass rate.
   - Try putting all of them in one workspace and list the dependency conflicts.
2. **Layer map.** For each app, list:
   - which crates are UI-agnostic (model, file I/O, layout, render, engine/commands);
   - which crates are bound to egui.

   Say how cleanly the UI could be replaced in each app, with `file:line` evidence.
3. **Mobile spike.** Compare two approaches on an Android emulator. Use an iOS simulator too
   if macOS is available; if not, say so.
   - **A. egui everywhere.** Build wordcraft for Android (eframe Android target via xbuild,
     cargo-apk or cargo-mobile2). Test:
     - Gboard input, including Cyrillic, autocorrect and swipe typing;
     - text selection, scrolling, zoom and rotation;
     - memory use and startup time.
   - **B. Hybrid.**
     - The Rust engine renders pages to a canvas or texture: WASM on web, a native library
       via FFI on desktop and mobile.
     - Toolbars, menus and dialogs live in a cross-platform UI layer. Evaluate at most two
       options, for example Tauri v2 with a web UI vs Flutter.
     - Text input goes through a native or hidden-input bridge.

   For each approach report what works, what breaks, an effort estimate and the risks. Save
   screenshots and recordings in `docs/phase0/`.
4. **Document server spike.** Build the smallest possible headless service (Rust, axum) on the
   wordcraft engine. The flow:
   1. GET a signed config;
   2. the web editor loads the document from a URL;
   3. the user edits;
   4. the server POSTs the saved .docx to a callback URL, with the JWT verified.

   Prove the whole flow end to end with the web build.
5. **Fidelity baseline.**
   - Run open → save → reopen → render-to-PDF on `corpus/` with the wordcraft, gridcraft and
     deckcraft CLIs.
   - Compare against LibreOffice and ONLYOFFICE x2t renderings.
   - Classify the losses: text, formatting, tables, images, headers/footers, fields,
     pagination.
6. **Write `docs/ARCHITECTURE-DECISION.md`** with:
   - the recommended architecture, and the rejected options with reasons;
   - an updated phase plan with estimates;
   - the upstream sync strategy;
   - a rename/branding checklist and a licensing checklist;
   - the top 10 risks.

**Exit gate.** The ADR exists, and every number in it comes from a measurement made in this
phase. The owner decides:

- the product name;
- the license of our own layer;
- the mobile approach (A, B or other);
- the priority order of co-editing vs fidelity vs mobile.

---

### Phase 1: Foundation

**Goal:** a monorepo that builds, tests and syncs with upstream, under our own brand.

**Tasks**

1. **Monorepo.** Bring the upstream apps in as pinned subtrees or submodules, according to the
   ADR. Use one Cargo workspace with deduplicated shared crates.
2. **Upstream sync.**
   - Write `scripts/sync-upstream` and `docs/UPSTREAM.md` (how to sync, how to carry and
     rebase patches).
   - Keep a `patches/` register: every upstream modification, why it exists, and its upstream
     PR status.
3. **Rebrand.**
   - Introduce the new product name, icons and colours.
   - Remove ArtCraft trademarks from our builds.
   - Keep the attribution files.
4. **CI.**
   - fmt, clippy and tests for all crates;
   - `cargo-deny` (licenses, advisories, duplicate crates);
   - web bundle size budget checks;
   - desktop and web builds.
5. **i18n.** Add uz-Latn, uz-Cyrl, ru and en catalogs, hooked into the existing i18n
   mechanism of each app. Uzbek strings are reviewed by the owner and are not
   machine-trusted.

**Exit gate.**

- A clean checkout builds desktop and web for all four apps.
- CI is green.
- A dry run of the upstream sync succeeds.
- The UI shows the new brand and switches to uz-Latn / uz-Cyrl.

---

### Phase 2: Transliteration (`uz-translit`)

**Goal:** fast, correct Latin ⇄ Cyrillic conversion that keeps formatting, usable everywhere.

**Tasks**

1. **Crate `uz-translit`.** Pure Rust with no dependencies, built for native and WASM. Seed
   the rules from `vendor-notes/uz-translit-onlyoffice/`, then cover everything in
   Appendix A.
2. **Editor command in Docs.**
   - Transliterate the selection or the whole document, in either direction.
   - Work on the document model at run level, so formatting is preserved.
   - Make the whole operation one undo step, optionally recorded as tracked changes.
   - Convert digraphs split across run boundaries (bold "S" + plain "h") correctly.
   - Skip field codes and hyperlink targets.
3. **The same command in Sheets** (cell text, not formulas) **and Slides** (text frames,
   notes).
4. **WASM/npm package** so the HRM web app can import the same logic.
5. **Tests.**
   - A golden list of ≥500 word pairs in both directions. **The owner must approve this list;
     do not treat a self-generated list as truth.**
   - A property test for the Latin → Cyrillic → Latin round-trip on the dictionary.
   - Formatting-preservation tests on .docx fixtures.

**Exit gate.**

- Golden tests pass.
- A 108-page .docx converts in under 2 s in the editor. Report the measured number; the
  ONLYOFFICE plugin needs 62 s.
- Formatting is identical before and after, apart from the text itself.

---

### Phase 3: Converter ("Konvertor")

**Goal:** one place to convert files between formats, in bulk, including PDF→editable DOCX
and transliteration.

**Tasks**

1. **CLI.** Build a `convert` command over the upstream engines:
   - docx ↔ odt/rtf/html/md/txt; .doc import; → pdf/png;
   - xlsx; .xls/.xlsb/.ods import; → pdf/csv;
   - pptx → pdf/png;
   - PDF combine / split / extract pages / text.
2. **PDF→DOCX sidecar.** A separate container running pdf2docx (`parse_stream_table=False`) +
   `merge_lines.py`, called over HTTP. It is GPL/AGPL, so it stays isolated and is never
   linked into our code.
3. **Transliteration as a conversion option**, e.g. Latin .docx → Cyrillic .docx with
   formatting kept.
4. **HTTP API.**
   - Batch upload, a job queue with progress, and downloads as zip.
   - Limits on file size and pages, timeouts, and cleanup of temporary files.
5. **UI page** in the shell: drag-and-drop, choose the target format and options, see
   progress, download. If Phase 4 is not ready yet, build a minimal standalone web page.
6. **Fidelity report.** One table per conversion direction: what is kept and what is lost,
   measured on `corpus/` against ONLYOFFICE x2t and LibreOffice.

**Exit gate.**

- Every supported direction works from CLI, API and UI.
- The fidelity table exists.
- Failures are reported to the user and never fail silently.

---

### Phase 4: Unified shell (web + desktop)

**Goal:** one app with one home screen that opens Docs, Sheets, Slides, PDF and Converter.

**Tasks**

1. **Home screen:** new document per type, templates, recent files, search.
2. **Lazy loading on web.** The shell is small, and each editor loads on first use.
   Size budgets are enforced in CI.
3. **Simplified toolbar by default**, with a "full ribbon" toggle on desktop. Plain Uzbek
   labels.
4. **Desktop packaging:** a single installer for the suite on Windows, macOS and Linux.
5. **Shared services:** settings, recent files, fonts, spell-check and the i18n catalogs are
   shared across the editors.

**Exit gate.**

- On web and desktop, a new user can create, edit, save and reopen a document, a spreadsheet
  and a presentation, and convert a PDF, without instructions.
- Bundle budgets are met.

---

### Phase 5: Document server + HRM integration

**Goal:** the HRM embeds our editor instead of ONLYOFFICE, on the HRM **test** environment.

**Tasks**

1. **Document server (Rust, axum).**
   - Session management and a JWT-signed editor config.
   - Open by URL, save via callback, autosave and a forced save.
   - Permissions (view / comment / review / edit), read-only mode, version history.
2. **JS embed API** shaped like ONLYOFFICE DocsAPI where reasonable (`new DocEditor(id,
   config)`, `document.url`, `editorConfig.callbackUrl`, `token`, `permissions`), so the
   HRM-side change is small.
3. **Compatibility notes:** which ONLYOFFICE config fields are supported, partially supported
   or not supported.
4. **HRM pilot:** one HRM document flow switched to our editor on the test environment, behind
   a feature flag.
5. **Security.** JWT verification on every call. SSRF protection for document URLs (allowlist,
   no internal addresses unless configured). Size limits. Audit log.

**Exit gate.**

- One real HRM document flow works end to end on the test environment: open → edit → save →
  reopen → download.
- The security checklist passes.

---

### Phase 6: Mobile

**Goal:** editing on Android, then iOS, using the approach chosen in Phase 0.

**Tasks**

1. **Android app:** open from the device, share sheet and HRM links; edit; save.
2. **Touch UX checklist**, each item verified on a device or emulator with a screen
   recording:
   - selection handles and long-press menu;
   - native keyboard with autocorrect, swipe typing and Cyrillic;
   - pinch zoom, momentum scroll and rotation;
   - working on small screens (360 px wide);
   - no layout overflow.
3. **Mobile toolbar:** a compact, contextual bottom toolbar, not the desktop ribbon.
4. **Performance:** startup time, memory and scroll FPS measured on a mid-range Android
   device.
5. **iOS app**, with the same checklist.
6. **Embedding in our HRM mobile app** (React Native / Expo): open a document from the HRM app
   in the editor, and save back through the document server.

**Exit gate.**

- The touch checklist passes on Android, with recordings.
- iOS passes the same checklist, or is explicitly deferred by the owner.

---

### Phase 7: Plugin system

**Goal:** third parties (and we) can extend the suite safely on every platform.

**Tasks**

1. **Plugin model.**
   - WASM plugins (e.g. wasmtime on native, the browser runtime on web).
   - A manifest with name, version, permissions and the UI contributions (toolbar button,
     menu item, panel).
2. **API built on the command catalogs:**
   - read and modify the document through commands;
   - selection and events;
   - UI contributions.

   Version the API and document it in `docs/plugins.md`.
3. **Sandbox and permissions.** No network or filesystem access unless the manifest declares
   it and the user or admin grants it.
4. **Port `uz-translit` as the first plugin.** It must behave identically to the built-in
   command, including speed.
5. **A second sample plugin** that exercises a different API surface (e.g. insert an HRM
   template field).
6. **Admin control:** install, enable and disable plugins per organisation (document server
   setting).

**Exit gate.**

- Both plugins run on web, desktop and mobile.
- A malicious-plugin test (one that tries the network or filesystem without permission) is
  blocked.

---

### Phase 8: Fidelity hardening

This phase runs continuously from Phase 3 onward.

**Goal:** real-world files open and save without silent loss.

**Tasks**

1. **Regression harness.**
   - Every file in `corpus/` and in public sample sets goes through open → save → reopen →
     render.
   - Page images are diffed against reference renderings.
   - The harness runs in CI nightly.
2. **No silent loss.** When content cannot be represented (charts, SmartArt, OLE), preserve
   it as an opaque part on save if possible, and always show the user a warning.
3. **Fix the top loss categories** from the Phase 0 baseline, in order of impact on the HRM
   corpus.
4. **Upstream fixes** go upstream as PRs where possible.

**Exit gate:** this phase has no single end point. A loss-rate trend is reported at every
other phase gate.

---

### Phase 9: Real-time co-editing

**Goal:** several users edit the same document at the same time.

**Tasks**

1. **Spike.** Compare a CRDT (e.g. yrs/Yjs or automerge) with OT on top of the command
   stream. Measure:
   - conflict behaviour on concurrent typing, formatting and table edits;
   - bandwidth;
   - latency.
2. **Implementation** in the document server: presence, cursors, conflict-free merging, and
   offline edits merged on reconnect.
3. **Comments and review** working under co-editing.

**Exit gate.** Three simultaneous users on web and mobile edit one document for 10 minutes
with scripted random edits, and every replica converges to the same final state.

---

### Phase 10: Release and operations

**Goal:** the owner can deploy and maintain the suite.

**Tasks**

1. **Packages:**
   - signed desktop installers;
   - Android APK/AAB and an iOS build;
   - Docker images for the document server, the converter and the PDF sidecar;
   - static web assets with the correct headers (wasm MIME type, compression, caching).
2. **Ops documentation:** install, upgrade, backup and monitoring.
3. **User guide in Uzbek**, with screenshots.
4. **Security review** of the whole system. Fix the findings.
5. **Release notes and versioning policy.**

**Exit gate:** a fresh install from the docs works on a clean server, and the security review
has no open high-severity findings.

---

## Appendix A: Uzbek transliteration rules checklist

- **Apostrophes.** Accept o‘ / g‘ written with any apostrophe variant: ʻ U+02BB, ʼ U+02BC,
  ‘ U+2018, ’ U+2019, ' U+0027 and the backtick. Normalise the output to the official forms.
- **Tutuq belgisi.** ʼ ↔ ъ (e.g. ma'no ↔ маъно, san'at ↔ санъат).
- **Digraphs.** sh ↔ ш, ch ↔ ч, ng (as a letter combination) ↔ нг.
  - Separated letters must not merge: s'h → сҳ (e.g. Is'hoq → Исҳоқ).
  - Keep apostrophe-separated cases like mas'ul ↔ масъул.
- **Ye / е / э.**
  - Word-initial е ↔ ye (ер ↔ yer).
  - After a vowel, е ↔ ye.
  - Otherwise е ↔ e.
  - Word-initial э ↔ e.
  - Treat Latin e at word start as э unless an exception applies.
- **ё, ю, я ↔ yo, yu, ya**, context-aware: word start, after vowels, after ъ.
- **ц ↔ s / ts**, context-dependent and mostly in loanwords; use the exceptions dictionary.
- **Letter case.** Sh / SH / sh ↔ Ш / ш, and all-caps words (SHAHAR ↔ ШАҲАР). Mixed case is
  preserved as far as possible.
- **Exceptions dictionary.** Russian loanwords such as sentabr ↔ сентябрь, oktabr ↔ октябрь,
  aktyor ↔ актёр, and similar.
- **Leave untouched:** URLs, e-mail addresses, codes such as "PF-1234", numbers, field codes,
  hyperlink targets, and text tagged with a non-Uzbek language.
- **Run boundaries.** A digraph split across two formatting runs is still one letter.

## Appendix B: Decisions only the owner makes

| When | Decision |
|---|---|
| Phase 0 gate | Product name; license of our layer; mobile approach; priority order (co-editing / fidelity / mobile) |
| Phase 1 | Brand assets (logo, colours); whether to send patches upstream |
| Phase 2 | Approval of the golden transliteration word list and the exceptions dictionary |
| Phase 5 | Which HRM document flow is the pilot |
| Phase 7 | Who may install plugins (admin only vs users) |
| Phase 10 | Release channels and signing identities |
