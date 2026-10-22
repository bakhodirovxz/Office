# Phase 4 (Unified shell, web + desktop): execution plan

> **Owner decisions and requirements in force (gathered 2026-10-15 … 2026-10-22)**
> - **Phase 3 gate passed 2026-10-22** ("go"). Carry-forward: `docs/phase3/gate-review.md` §10.6.
> - **Owner answers to §9 questions, 2026-10-22:**
>   - **Q10, MSVC: yes.** Install VS Build Tools and the Windows SDK under the **Visual Studio Community** licence
>     terms for open-source development. Put it on E: where possible.
>   - **Q4, signing: internal now, public later.** The pilot uses the organisation's internal (enterprise CA)
>     code-signing certificate, trusted through GPO. A public OV certificate on a hardware token is for the Phase 10
>     release; procurement starts now. Both certificates are owner/IT items.
>   - **Q2, default handlers: none for now.** The MSI only registers our apps under "Open with" (§7.2) and changes
>     no defaults. The GPO default-association XML is still produced and documented as an optional artefact, but it is
>     not deployed.
>   - **Q11, offline converter: yes.** The installer ships the four domain CLIs (about +224 MB), so the converter
>     works without a server.
>   - **Q7 (O8):** not answered, so the default stays the native PDF → DOCX engine, with the D5 fallback.
> - **Replace MS Office on Windows 10/11 office PCs, Office 2010+ files** (STATUS "Owner acceptance criteria"). Phase 4
>   delivers the installable suite: one installer, our own ProgIDs, the **"Open with"** registration, a GPO
>   default-association XML, and a **signed MSVC** build. The certificate is an owner item.
> - **Show/Hide ¶ must be easy to find** in the simplified toolbar (STATUS:714-719).
> - **Excel workload:** heavy formulas, many sheets, external links, files of 80+ MB that keep growing (STATUS:721-730).
>   Phase 4 must not regress; the engine work is Phase 8 / track E (§6).
> - **Accounts optional (O6, revised 2026-10-21):** local mode with no login is the default. A server connection is
>   optional. The HRM is one configured provider and is never hard-coded. SSO is Phase 5; Phase 4 only leaves the seam.
> - **Version history independent of the HRM (owner, 2026-10-21; STATUS:7-30):** Phase 4 = the History panel and local
>   snapshots. Phase 5 = the server store, the diffs and the HRM backend.
> - **XDFU is opt-in and OFF by default everywhere** (owner, 2026-10-21; phase3 PLAN §10). Phase 4 adds, in the editors:
>   the "Insert XDFU grif" command, the print/export/copy restriction, and a watermark on print and export.
> - **Android deferred** (owner, 2026-10-20). The brief's mobile evidence task (SIZING 4.9) and the **B-3 re-decision at
>   this gate** still stand. Phase 4 does the minimum that keeps that re-decision honest, with no Android build (§9).
> - **Uzbek UI strings:** the review is delegated to the agent (`reviewed:agent-<model>:<date>`, never an owner review).
> - **Product name deferred:** every name comes from `brand/brand.toml`. Licence MIT OR Apache-2.0. No upstream PRs:
>   every upstream change is a carried patch with a `patches/REGISTER.md` row. Never hand-edit `upstream/`.

| | |
|---|---|
| **Status** | Plan only, written 2026-10-22 on branch `phase-4` (from `phase-3`). No product code, no builds. **WP0 (the upstream sync with the 13-conflict hand-merge) is in progress in another agent's hands while this is written** (§11 WP0). |
| **Inputs** | `docs/BRIEF.md` Phase 4 (:366-386); `docs/phase1/SIZING.md` §2.3 (:168-209), §2.4, §2.5, §2.10; `docs/ARCHITECTURE-DECISION.md` §2.2, §2.2.1, §2.3.1, §2.5, App. B/C; `docs/STATUS.md`; `docs/phase3/gate-review.md` §10; `docs/phase3/PLAN.md` §5.5, §10; `docs/phase2/gate-review.md` M3; `docs/phase0/gridcraft-memory-r2.md`. |
| **Exit gate (brief)** | (1) On web and desktop, a new user can create, edit, save and reopen a document, a spreadsheet and a presentation, and convert a PDF, **without instructions**. (2) The bundle budgets are met. |
| **Estimate** | **53 stream-days likely (range 38–76), plus 2 for the optional Linux packages.** That is above SIZING's 44.5: the history, XDFU, TLS, sync and Excel-guard work was added since SIZING (+15), and the toolbar design below saves about 5.5 (§12.1). With four streams the calendar is about **18–22 working days**, plus owner latency (§11, §12). |

How to use this plan. §0 holds the facts found while planning. §1–§10 are the designs. §11 holds the work packages;
each can be handed to one sub-agent as written. Every `file:line` was re-read for this plan on 2026-10-22 at the
current BASEs: wordcraft `eae2421`, gridcraft `f1894c6`, deckcraft `1dc6972`, pdfcraft `924c4da`. Paths are under
`upstream/<repo>/` unless stated, and those trees are the **queue applied**, not pristine (`docs/UPSTREAM.md:3-14`).
**WP0 moves every BASE**, so every upstream reference must be re-checked after the sync lands.

---

## 0. New facts found while planning (they change the design)

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **wordcraft already has Customize Ribbon and a Quick Access Toolbar.** The user can hide any built-in tab, add custom tabs, and fill custom groups with **any command id**. The layout is saved with the preferences and survives unknown ids. The ADR's "no extension point" holds for gridcraft and deckcraft, **not for Docs**. | `crates/engine/src/ribbon.rs:1-7`, `:24` (`DEFAULT_QAT`), `:117`, `:225`, `:236`, `:257`, `:275`; command `tools.customizeRibbon` `crates/engine/src/cmd/tools.rs:377`; custom groups drawn with each command's own icon and translated label `crates/ui-egui/src/ribbon.rs:422-444` | The Docs simplified toolbar can be a **preset layout** applied through a command. That needs no hot-file patch (§2.2). The SIZING item 4.4 for wordcraft (3/4/6 d) drops to about 1.5 d. |
| F2 | **The wordcraft registry can be extended, and overridden, from our domain crate.** `install()` rebuilds the registry from `all()` plus our specs. `Registry::new` maps an id to its **last** spec, so a re-registered id replaces upstream's handler. `CommandSpec` carries `enabled: fn(&Session) -> Option<&str>` and `run`. | `domains/docs/crates/docs-translit/src/lib.rs:55-62`; `crates/engine/src/lib.rs:61-74` (`CommandSpec`), `:104-116` (`Registry::new`); dispatch `crates/engine/src/session.rs:1167` | For Docs, the XDFU restriction, the snapshot-on-save hook and Phase 5's permission gating can wrap `file.print`, `file.exportPdf`, `file.save`, `edit.copy` … **without a new upstream patch**: they reuse the existing `office-ext` hook (patch `wordcraft/0120`). |
| F3 | **gridcraft and deckcraft have no such hook.** gridcraft's registry is a closed `OnceLock` (patch `gridcraft/0120` header). Dispatch is `Session::execute`. | gridcraft `crates/engine/src/lib.rs:339`; deckcraft `crates/engine/src/lib.rs:579`; `patches/gridcraft/0120-gridcraft-translit-commands.patch:5-7` | One small carried **command-hook** patch per engine (a before/after callback on `execute`) serves XDFU, snapshots and Phase 5 permissions (§4). |
| F4 | **The ribbon call sites are single lines.** wordcraft `lib.rs:2028`, `:2034`; gridcraft `lib.rs:1149`; deckcraft `lib.rs:1836`. `ribbon_collapsed` has moved since the ADR read it: wordcraft `lib.rs:250`, gridcraft `:74`, deckcraft `:268` (the ADR has `:236`, `:55`, `:242`). | read 2026-10-22 | The Sheets/Slides toolbar is a **new file of ours plus a one-line branch at the call site** (§2.3). The bulk of the carry lands in files upstream never touches. |
| F5 | **pdfcraft has no ribbon.** Its chrome is a tab strip, a mode bar and a right rail, with a Home view (recommended tools, open card, recent files). | `crates/ui-egui/src/lib.rs:3123-3126`, `:3144`; `crates/ui-egui/src/home.rs:1-2`, `:10` | PDF needs **no** simplified-toolbar patch, only a check in the usability test. SIZING 4.4 for pdfcraft (2/3/4 d) goes to about 0.5 d. |
| F6 | **Show/Hide ¶ is already on the Docs Home tab** (Paragraph group) and has an icon mapping. | engine `crates/engine/src/cmd/view.rs:18`; ribbon `crates/ui-egui/src/ribbon.rs:944-945`; icon `crates/ui-egui/src/widgets.rs:386` | The Docs preset keeps the Home tab, so ¶ stays visible with its toggle state. The usability check confirms that staff find it. |
| F7 | **Three of the four web apps have a `?host=parent` postMessage bridge; deckcraft has none.** wordcraft's bridge is our carried patch 0001 (open, requestSave, save, dirty); gridcraft's is upstream; pdfcraft's is upstream and also takes `?file=<url>`. | `patches/wordcraft/0001-…:9-22`; gridcraft `apps/gridcraft-web/src/web.rs:30`, `apps/gridcraft-web/src/host.rs:1-13`; pdfcraft `apps/pdfcraft-web/src/main.rs:30-31`, `:59` | The web shell hosts editors in iframes in host mode. **The deckcraft bridge (SIZING 5.5, Phase 5) moves into Phase 4**, so that Slides gets templates and recent files on the web. |
| F8 | **The editors' web pages bootstrap with an inline `<script type="module">`** written by trunk, and instantiate wasm. The converter's CSP is `script-src 'self'`, with no `'wasm-unsafe-eval'`. | `E:/office-target/p1/dist/docs-web/index.html` (inline module script); `services/convert-api/src/routes.rs:96` | Serving the editors from our server needs a **per-route CSP**: `'wasm-unsafe-eval'` plus the inline script's sha256 hash, for editor routes only. The shell and converter routes keep the strict CSP (§3.3). |
| F9 | **Each app keeps its own recent-files list, in its own settings folder.** | wordcraft `crates/ui-egui/src/lib.rs:251`, `:347`, file `apps/wordcraft/src/main.rs:139-141` (`ui.json`); gridcraft `apps/gridcraft/src/main.rs:76-134` (`prefs.json`); deckcraft `crates/ui-egui/src/settings.rs:1`, `:15`, `:104`; pdfcraft `crates/ui-egui/src/lib.rs:479`, `:1364-1366`, `:2225` | The home screen **reads** the four lists (read-only, tolerant parse) instead of patching four save paths (§5.2). |
| F10 | **Spell-check reads system Hunspell dictionaries; none are bundled.** wordcraft reads them from `WORDCRAFT_DICTIONARIES` or a settings directory; deckcraft from `DECKCRAFT_DICTIONARIES`. The deckcraft test crash (STATUS_ACCESS_VIOLATION) is narrowed to the two `squiggles_*` tests with process-wide `OnceLock<Mutex<…>>` spell state, and is not root-caused. | wordcraft `crates/proof/src/hunspell.rs:1-10`, `:20-27` (`set_dictionary_dir`); deckcraft `crates/engine/src/spell.rs:1-11`, `:556-612`; `docs/phase1/tests.md:107-127` | Shared spell-check = **one dictionaries folder** that the suite installs and points both apps at (§5.4). The crash must be reproduced **in the GUI with dictionaries installed** before we turn spell-check on by default (WP6). |
| F11 | **This PC has no MSVC toolchain, no Windows SDK (no `signtool`), no WiX and no .NET SDK.** C: has 16.4 GB free. | `vswhere.exe` absent; `Get-Command link,cl,signtool,wix` empty; `rustup target list --installed` = `wasm32-unknown-unknown`, `x86_64-linux-android`, `x86_64-pc-windows-gnu`; STATUS:552 | **The signed MSVC build is blocked until VS 2022 Build Tools and the Windows SDK are installed.** That is about 7–8 GB, mostly on C:, and needs the owner's OK on licence and space (Q10). Until then everything is built and tested on windows-gnu, and the MSI can be built unsigned (§7.4). |
| F12 | **Upstream packages with WiX v5** (`dotnet tool install -g wix`, extensions 5.0.2), and ships a `sign.ps1`. WiX **v6 and later carry the Open Source Maintenance Fee**, which is required of organisations over $10k annual revenue ([OSMF](https://docs.firegiant.com/wix/osmf/), [what's new](https://docs.firegiant.com/wix/whatsnew/)). | `packaging/windows/wordcraft.wxs:3-8`; `packaging/windows/sign.ps1` | We **pin WiX 5.0.2** (MS-RL, a build tool that is never shipped). That avoids the OSMF question. |
| F13 | **Upstream's MSI registers Capabilities, `OpenWithProgids` and `Applications\<exe>\SupportedTypes`, and never takes over a default.** "Make default" opens Settings › Default apps. | `packaging/windows/wordcraft.wxs:78-98`; `apps/wordcraft/src/default_apps.rs:1-14`, `:87` | The "Open with" design (§7.2) follows the same model under our ProgIDs. Defaults come from the user ("Always") or from GPO, **never** from writing `UserChoice`. |
| F14 | **Single instance:** pdfcraft hands a second launch's files to the running instance as tabs. wordcraft, gridcraft and deckcraft open **a new process and window per launch**. Their control ports are bound only on request, so a second launch does not fail. | pdfcraft `apps/pdfcraft/src/main.rs:14`, `:234-237`, `apps/pdfcraft/src/single_instance.rs:2`; wordcraft `apps/wordcraft/src/control_server.rs:40`; gridcraft `apps/gridcraft/src/control_server.rs:15`; deckcraft `apps/deckcraft/src/main.rs:142-174` | The minimum is to **keep upstream's behaviour** (Word also opens a window per document) and **test** it (§7.3). A second Sheets process re-pays the fixed 226 MB start-up cost; that is noted for track E. |
| F15 | **Argument parsing on non-Unicode paths:** wordcraft is careful (`args_os`, `main.rs:286-295`). gridcraft uses `std::env::args()` (`main.rs:253`), which panics on a file name that is not valid Unicode. Cyrillic and spaces are valid Unicode, so they are fine. | read | The "Open with" test covers Cyrillic and spaces. A non-Unicode name is a known, rare gridcraft crash; it is recorded for Phase 8 rather than patched. |
| F16 | **WebView2 Runtime 155.0.4283.62 is present on this Windows 10 22H2 PC, and this PC is domain-joined.** | registry `EdgeUpdate\Clients\{F3017226-…}` `pv`; `Win32_ComputerSystem.PartOfDomain = True` | A WebView2 desktop shell is viable on this fleet type (§3.4). **This PC is the owner's work PC, so it is not used for GPO tests** (Q1). |
| F17 | **AccessKit is enabled in every app, desktop and web, and wordcraft exposes the document to screen readers** (#488). | e.g. `apps/wordcraft/Cargo.toml:26`, `apps/wordcraft-web/Cargo.toml:31`, `crates/ui-egui/src/a11y.rs:1-6`; the same feature in the other three apps' `Cargo.toml` | The accessibility baseline measures what exists instead of building anything (WP18). Web canvas support is UNVERIFIED. |
| F18 | **gridcraft caps a workbook at 1 GiB uncompressed** (about 150 MB on disk, ADR §2.3), and upstream's own table says "100 MB files fail (#175)". External links are passed through, never followed or refreshed. | gridcraft `crates/xlsx/src/package.rs:13`; `docs/file-format-parity.md:25`; STATUS:730 | The owner's files "keep growing" toward that wall. Phase 4 makes it **never silent** and gives an "open in Excel" escape hatch; the fix is track E5 (§6). |
| F19 | **The size budgets have no absolute or shell budget yet**; budgets.toml defers it to Phase 4. The headroom at the Phase 3 close was docs 19,271 B, sheets 21,869, slides 26,935 and pdf 22,771 brotli. | `web/budgets.toml:20`; `docs/phase3/gate-review.md:501` (the sizes row) | WP7 adds a `[shell]` budget and a lazy-load check. Adding a third Uzbek catalog (uz-Latn-2026) may not fit the current deltas (§8.2). |
| F20 | **The app catalogs do not derive uz-Latn-2026**; only the converter catalog does. | STATUS:1056-1057; the four `patches/*/0110-*` add exactly two languages | The cheap path is §8.2: the same export derivation plus a third language row in each 0110 patch. |

---

## 1. Product shape

```
Suite (one install, one URL)
├── Home  (HTML shell, the same code on web and desktop)
│   ├── New: Document · Spreadsheet · Presentation · PDF (open)
│   ├── Templates (ours, uz-Latn / uz-Cyrl / ru / en)
│   ├── Recent (desktop: the four apps' own lists merged; web: browser storage)
│   ├── Search (recent + known folders on desktop; recent on web)
│   ├── Converter  (the Phase 3 page, moved to /convert/)
│   ├── History    (local snapshots of a document, §5.5)
│   └── Settings   (language, alphabet, toolbar mode, spell-check, server connection, XDFU when the owner turns it on)
├── Docs / Sheets / Slides / PDF editors
│   ├── desktop: the four native exes (unchanged process model; ProgIDs point straight at them)
│   └── web: four separate wasm bundles, each loaded only when first opened (iframe, ?host=parent)
└── Converter tools: the four CLIs + convert (local mode, loopback, in-process in the desktop host)
```

**Why the four desktop exes stay separate.** The W2 build domains are four Cargo workspaces (`domains/{docs,sheets,slides,pdf}`).
They cannot link into one binary without undoing ADR B-5. Separate processes also isolate a gridcraft multi-GB peak from
the other editors. "One product" therefore means one installer, one home screen, one settings file and one brand,
not one binary.

---

## 2. Simplified toolbar (the second-largest carry; designed to minimise sync cost)

### 2.1 Principles
1. **Data, not code, in hot files.** A toolbar is a list of command ids with parameters, rendered by a file of ours.
   Upstream files gain only a call-site branch (one or two lines each), which is the only part that can conflict at a sync.
2. **Ids are tested, not merged.** A test per app (`office_toolbar_ids_exist`) asserts that every id in our toolbar
   table exists in that app's command registry. A sync that renames a command fails CI with a precise message. It
   never becomes a merge conflict or a dead button.
3. **One design for desktop and mobile** (SIZING 4.3, ADR §2.2.1 step 3). The simplified toolbar has a compact mode
   (one row, icons plus short labels) that is the candidate 360 dp toolbar. It is measured in §9, never built for mobile here.
4. **Simplified is the default; the full ribbon is a toggle on desktop only** (BRIEF §4.3). The toggle is hidden on
   web (`cfg(target_arch = "wasm32")`).
5. **Plain Uzbek labels** come from our catalog rows (`i18n/shell.tsv`, §8.1). They are short verbs ("Qalin",
   "Belgilarni koʻrsatish ¶", "Jadval qoʻshish"), not Office jargon.

### 2.2 Docs: a ribbon preset, no hot-file patch (F1, F2)
- A new domain crate, `domains/docs/crates/docs-office` (installed by the existing `office-ext` hook next to
  `docs-translit`), registers `office.ribbonSimple {"on": bool}`. The command runs `tools.customizeRibbon` operations:
  - hide every built-in tab except Home and Insert;
  - add a custom tab with our plain-labelled groups (Fayl: Save, Print, Export PDF; Tekshirish: spelling, Show/Hide ¶;
    Transliteratsiya; Tarix: Version history);
  - set the QAT to Save, Undo, Redo and the **simplified/full toggle**.
  `{"on": false}` resets to the default ribbon (`reset_ribbon`, `ribbon.rs:380`).
- **First run:** if the saved layout is the default one (`ribbon_is_default`, `ribbon.rs:183`), the preset is applied
  once. WP2 step 1 checks the order of "session created → hook → preferences loaded". If preferences overwrite the
  preset, the fallback is one call in the overlay's startup, in our own file `office_overlay.rs`.
- **Known limits, said plainly:** a custom tab's name is literal text, not translated (`ribbon.rs:288-289`), so the
  preset is re-applied when the interface language changes. Buttons in custom groups show no toggle state
  (`ribbon.rs:431`, `false`); that is why ¶ stays on the built-in Home tab, where it does show state.
- If the owner's design review or the usability check (WP23) finds a ribbon with fewer tabs is "not simple enough",
  the fallback is the Sheets/Slides design for Docs too (+2 d, R3).

### 2.3 Sheets and Slides: a carried toolbar in our own file
- **New file per app**, `crates/ui-egui/src/office_toolbar.rs` (ours, MIT OR Apache-2.0), plus a data table
  `domains/<d>/office_toolbar.toml` embedded by the existing `build_office.rs`. The file draws one row: icon plus
  label, with groups separated by spacing, an overflow "…" menu, and the toggle at the right end. It runs commands
  through the app's own dispatcher, and reuses the ribbon's button helpers where they are `pub(crate)`. Making a helper
  `pub(crate)` is a one-word hunk.
- **Call site:** `if office_overlay::toolbar_simple() { office_toolbar::show(self, ui) } else { ribbon::show(self, ui) }`
  at gridcraft `lib.rs:1149` and deckcraft `lib.rs:1836`. The mode lives in our settings (§5.1), not in upstream's
  `UiState`, so no upstream struct changes.
- **Content (draft for WP1, the owner reviews it):**
  - Sheets: Save · Undo/Redo · font, size, B/I/U · fill · borders · number format (General, Number, Percent, Date) ·
    merge · wrap · sort · filter · AutoSum · insert row/column · freeze · Show formulas · Print · Transliterate · History.
  - Slides: Save · Undo/Redo · New slide · Layout · font, size, B/I/U · alignment · bullets · picture · shape · table ·
    Start slideshow · Print · Transliterate · History.
- **Patch shape:** `gridcraft/0140-office-toolbar.patch` and `deckcraft/0140-office-toolbar.patch`, each about
  400–700 lines **in new files**, plus about 5–15 lines in `lib.rs` and `ribbon.rs`. REGISTER rows: "drop when
  upstream offers a simplified mode".

### 2.4 PDF (F5)
No patch. The Home view, mode bar and right rail are already task-oriented. WP23 checks it. If staff fail there, the
cheapest fix is a carried change to `RECOMMENDED` (`home.rs:10`), not a toolbar.

### 2.5 Carry cost, measured (ADR §2.2.1 (ii), replaced per SIZING 4.9)
The new patches (0140 ×2, the command hooks of §4, the bridge of §3.2) are carried through **two syncs**: WP0's
landing sync and a `sync-upstream.sh --dry-run --survey` at the end of the phase (WP24). Per patch we record
`clean / 3way / conflict` and the minutes spent. That number replaces "upstream's answer to a layout-mode hook PR",
which B-6 rules out.

---

## 3. Web shell and lazy loading

### 3.1 Serving: `suite-serve`
- A new bin in the services workspace, `services/suite-serve`. It **mounts the existing `convert_api` router**
  (`services/convert-api/src/routes.rs:311-352`) under the same origin and adds static routes:
  - `/` → `web/shell/` (the home page);
  - `/convert/` → the converter page, moved from `/`. The Playwright spec is updated, and the old `/` keeps working in
    `convert-api` alone;
  - `/docs/`, `/sheets/`, `/slides/`, `/pdf/` → the four trunk dists.
- Static files come from `tower-http` `ServeDir` with `precompressed_br()` (tower-http is already a dependency of
  convert-api). The build writes `.br` next to each file at `brotli -q 11 --lgwin=24`, the same settings as the
  budgets (SIZING 4.2). `application/wasm` MIME type. `Cache-Control: immutable` on hashed file names; `no-store`
  stays on the API.
- Local mode stays as in Phase 3: `auth = none` only on loopback. Server mode keeps TLS, JWT and the headers of
  phase3 PLAN §5.5.

### 3.2 Lazy loading
- **The shell is plain HTML, JS and CSS** (no framework, no wasm), like the converter page. It loads its catalog JSON
  and the Noto Sans Uz subset, and nothing else.
- Opening an editor creates an `<iframe src="/docs/?host=parent">` the first time. Only then is that editor's wasm
  fetched. An editor already loaded is reused (one iframe per editor kind, hidden when not in front).
- Open, new and template: the shell posts `{type: "<app>:open", name, bytes}` after `<app>:ready` (F7). Save:
  `<app>:save` bytes arrive in the shell, which (a) offers the download, as the editor alone does today, and (b) keeps
  a copy in browser storage for Recent (IndexedDB, per origin, capped at 200 MB, with an LRU). `<app>:dirty` drives a
  "leave page?" guard.
- **deckcraft bridge (moved from Phase 5, F7):** `deckcraft/0001`-style patch adding `apps/deckcraft-web/src/host.rs`
  with the same five messages as wordcraft 0001. It is a new file plus about 10 lines in `web.rs`.

### 3.3 CSP per route (F8)
- `/` and `/convert/`: unchanged `script-src 'self'` (phase3 PLAN §5.5).
- `/docs/` and the other editor routes: `script-src 'self' 'wasm-unsafe-eval' 'sha256-<inline loader>'`. The hash is
  computed at build time from each dist's `index.html` and written next to the dist. `frame-ancestors 'self'`, so the
  shell may frame its own editors and nobody else may.
- A test reads every route's headers (the Phase 3 header test, extended).

### 3.4 Desktop: the same shell in a WebView2 host
**Options.**
- (a) An egui launcher: native, no WebView dependency, but a second copy of the home UI and of its i18n, and the
  converter page needs a browser anyway.
- (b) **A WebView2 host (`wry`, MIT/Apache) showing the same HTML shell**: one UI, the converter embedded, and HTML
  accessibility for free.
- (c) The default browser on a loopback URL: no install-time dependency, but it is not an application window.

**Recommendation: (b), with (c) as the automatic fallback** when WebView2 is missing. The deciding trade-off: one
home screen codebase and one i18n path for web and desktop, against a runtime dependency that is present on
Windows 11 and on this Windows 10 22H2 PC (F16). Fleet coverage is a B-12 question.
- `suite.exe` (our crate, `apps/suite-desktop` in the services workspace, so it links `convert_api` but no engine)
  starts the `suite-serve` router **in-process** on `127.0.0.1:<random port>` and opens one WebView2 window on it.
- **Desktop-only actions never go over HTTP.** "Open in editor", "pick a file", "new from template", "list recent",
  "search folders" and "history" use `wry`'s IPC (`window.ipc.postMessage`) only. The loopback port therefore exposes
  only what local-mode convert-api exposes today. A web page in a browser on the same PC cannot launch processes
  through it (R15).
- "Open in editor" spawns `suite-docs.exe "<path>"` and the other exes. The binary names come from `brand.toml`
  `bin` (`brand/brand.toml:23` and similar). There is one process per document, as today (F14).
- Linux: `wry` needs webkit2gtk (LGPL), which would be a **linked non-permissive dependency**. On Linux the host
  uses fallback (c) unless the owner decides otherwise (R16, Q12).

---

## 4. Command hooks (one seam for XDFU, snapshots and Phase 5 permissions)

| App | Mechanism | New upstream patch |
|---|---|---|
| Docs | `docs-office` re-registers the guarded ids with wrappers (F2): `enabled` returns a reason when a policy refuses, and `run` calls the original, then the after-hook. The original `fn` pointers are kept in a `OnceLock` table. | none |
| Sheets | `gridcraft/0141-office-command-hook.patch`: `Session::execute` (`lib.rs:339`) calls `office_hook::before(id, &params)?` and `after(id, &result)`. A no-op unless installed. | ~25 lines in `lib.rs` + a new file |
| Slides | `deckcraft/0141-office-command-hook.patch`, the same shape at `lib.rs:579` | the same |
| PDF | none in Phase 4. PDF gets no XDFU editor features and no snapshots in Phase 4 (said plainly). | — |

**Guarded ids** (checked against each registry by a test): `file.print*`, `file.export*`, `file.saveAs`, `edit.copy`,
`edit.cut`, and `file.save` (after-hook: the snapshot). **WP4 step 1 verifies that every UI path for these actions goes
through the dispatcher.** Paths that may not: wordcraft's background saves (`crates/ui-egui/src/async_saves.rs`), the
system clipboard (`system_clipboard.rs`), and keyboard shortcuts handled in the UI. Any bypass found becomes a one-line
carried call or a stated limit (R11).

---

## 5. Shared services

### 5.1 Settings
- One file, `%APPDATA%\<brand data root>\settings.toml` (the root comes from `brand.toml`). It is read by the shell
  host and by each app's overlay at start-up. `crates/suite-core` (ours) holds the schema:
  `lang`, `latin_alphabet`, `toolbar = "simple" | "full"`, `spell_dictionaries`, `history.*`, `xdfu = "off"` (the
  default), and `[server]` (§5.6).
- Apps keep their own `ui.json`/`prefs.json`. The overlay applies **only** the shared values at start-up: the language
  when the app's own setting is "auto", the toolbar mode, and the dictionaries folder. Those changes stay in
  `office_overlay.rs`, a file of ours, so no new hot hunks.

### 5.2 Recent files (F9)
`suite-core::recent` reads the four apps' own lists (tolerant: an unreadable list is skipped and logged, never fatal),
merges them by last-opened time, and drops files that no longer exist. A contract test runs against each app's
current JSON shape, so a sync that changes a shape fails in CI. **Zero upstream carry.** Web: the IndexedDB copies of §3.2.

### 5.3 Fonts
Already shared: the Noto Sans fallback (patches `*/0102`), covering U+02BB, U+02BC and Cyrillic (STATUS WP7a). The MSI
installs the font set once under the install folder, and `CRAFT_FONTS_DIR` is set for the CLIs as in Phase 3. The
shell uses the same Noto Sans Uz subset as the converter page (`services/convert-api/src/routes.rs:355-360`).

### 5.4 Spell-check (F10)
- The suite ships `dictionaries/` (en_US and ru_RU first) and points both readers at it: wordcraft through
  `set_dictionary_dir` (`proof/src/hunspell.rs:20-27`) from the overlay, deckcraft through `DECKCRAFT_DICTIONARIES`
  set by the overlay before first use. gridcraft and pdfcraft have no spell-check UI; nothing to share there.
- **Licences first:** the dictionary licences are checked in WP6. Dictionaries are data, but they ship in our
  installer, so the permissive-only rule is applied. An LGPL or MPL dictionary needs an owner decision.
- **Uzbek proofing is "later"** (BRIEF §4.8). No Uzbek dictionary is promised in Phase 4.
- **The deckcraft access-violation item:** WP6 reproduces it in the GUI with dictionaries present (typing in two
  text boxes while squiggles settle). Two outcomes:
  - a GUI crash: root-cause it (architect) and carry the fix;
  - test-harness-only: record it and keep the quarantine.

  **Spell-check is enabled by default in Slides only if the GUI run is clean.**

### 5.5 Version history, Phase 4 part (STATUS:7-30)
- **Store (desktop, local mode):** `%LOCALAPPDATA%\<brand>\history\`.
  - `objects/<sha256>`: content-addressed, written atomically.
  - `docs/<id>.jsonl`: one line per version `{v, parent, sha256, size, time, author (OS user), message?, name?, path}`,
    **hash-chained** with the same chaining as `services/convert/src/audit.rs`, unkeyed (local).
  - The document id is a hash of the canonical path plus the volume file id, so a rename keeps the chain where
    Windows allows it.
- **When a version is made:** the after-hook of `file.save` (§4). Docs through `docs-office`; Sheets and Slides through
  the 0141 hooks.
- **Retention**, with defaults the owner can change: the last 50 versions or 90 days, whichever keeps more; at most
  2 GB in total, oldest first. **Files over 50 MB keep the last 5 versions**, because an 80 MB workbook saved 20 times
  would be 1.6 GB (R10). Pruning never touches a named version.
- **History panel (in the shell, our HTML):** list versions; open one **read-only** (a temp copy opened in the
  editor); save a copy as; restore (the current file is snapshotted first, then the old version is written as a **new**
  version, never a rewrite); name a version. Each editor gets a "Version history" entry (Docs: a command in the
  preset tab; Sheets and Slides: a toolbar button) that opens the shell on that document.
- **Not in Phase 4:** diffs and blame (Phase 5), the server store and the HRM backend (Phase 5), web history (Phase 5;
  the web shell keeps only the last copy, §3.2), PDF.
- **Size of the Phase 4 part:** 3.5 stream-days (WP10). This is the Phase 4 share of STATUS's "+8–12 over SIZING 5.3".

### 5.6 Accounts: local by default, a provider seam for later
- `settings.toml` `[server] url = ""`. Empty means local mode, which is the default, and nothing ever asks for a login.
- "Connect to server" in Settings **navigates** to the server's own page, exactly as the converter page does (phase3
  PLAN §5.5, review-wp7 M8). It never fetches across origins.
- `[[providers]]` entries (`name`, `issuer`, `kind = "jwt" | "oidc"`) are **configuration**. Phase 4 ships none
  enabled. The HRM appears only as an example in docs (`docs/phase4/accounts.md`), never in code. `kind = "oidc"` is
  rejected with "available in a later version" until Phase 5.
- A grep test fails if the HRM's host names or its product name appear under `services/`, `crates/`, `domains/` or
  `web/` (the list comes from `docs/phase0/hrm-current-state.md`).

---

## 6. The Excel workload in Phase 4 (and what waits for Phase 8 / track E)

**Measured today** (ADR §2.3.1, `docs/phase0/gridcraft-memory-r2.md:26-29`): on w80 (79 MB), the GUI's steady private
working set is **903 MB against Excel 16's 742 MB**. Peak is 984 against 851. On w120 (119 MB) it is 1,240 against
1,090. The document itself costs the same as in Excel (677 vs 709 MB). The gap is the **fixed 226 MB start-up cost**:
57 MB of fonts (46 MB CJK) and about 92 MB for the DX12 renderer (`gridcraft-memory-r2.md:38-44`).

**Phase 4 does:**
1. **No regression, measured.** WP12 re-runs the round-2 protocol (`E:/office-target/xlsx-bench/ext/` scripts;
   w40/w80/w120) on the Phase 4 gridcraft GUI (toolbar, hook, overlay). Steady and peak private WS must be within +5 %
   of 566 / 903 / 1,240 MB. Open time must not be longer than round 2's. A breach blocks the gate.
2. **The shell adds nothing to an editor process.** The home screen is a separate process. Its own WS is measured and
   reported, and the shell can be closed while an editor runs.
3. **Never silent at the wall (F18).** Before the shell opens a workbook, it reads the zip central directory with the
   Phase 3 scanner code (`services/convert/src/scan.rs`, the zip-bomb checks).
   - Over 1 GiB uncompressed: "This workbook is larger than Sheets can open yet. Open it in Excel."
   - `xl/externalLinks/` present: "Links to other workbooks are kept but not updated."
   - Each offers **Open in Excel** when Excel is installed (B-7's "keep Excel per file").
   - WP12 also checks what the Sheets GUI itself shows when a file over the wall is opened by double-click. If it is
     unclear or silent, a carried one-message fix follows.
4. **Default-handler advice:** Sheets is registered for "Open with" but **not proposed as the .xlsx default** until
   track E (Q2).

**Waits for Phase 8 / track E** (SIZING §2.10): round 1 and 2 to production (E1, E2); the fixed start-up gap, with
fonts on demand and a lighter renderer (E3); the streaming writer and bounded undo (E4); the streaming zip that removes
the 1 GiB wall (E5); **external links followed and refreshed (E6, 13.5 d likely)**; a GUI editing session at w80
(E7); and F9 recalculation at 10 s vs Excel's 1.3 s.

---

## 7. Windows installation (requirement 1, and the "Open with" requirement)

### 7.1 One suite MSI
- WiX **5.0.2 pinned** (F12). `packaging/windows/suite.wxs` is **generated** by `cargo xt brand msi` from
  `brand/brand.toml`: names, `aumid`, `bin`, `UpgradeCode` (a new `[product] upgrade_code`). No name is hard-coded.
- Contents:
  - `suite.exe` (the home screen);
  - the four GUI exes and the four CLIs. The CLIs are needed by the local converter. That costs about 224 MB by the
    release-size projection (ADR §2.5); Q11;
  - fonts, `dictionaries/`, templates, LICENSE/NOTICE/ATTRIBUTION (every upstream file kept);
  - the WebView2 check (§3.4).
- **Per-machine (HKLM, `ALLUSERS=1`)** for office PCs and GPO software deployment. A per-user variant
  (`ALLUSERS=2 MSIINSTALLPERUSER=1`, HKCU) is built only if WiX 5 gives it from the same source with no second
  manifest. Otherwise it is dropped and the plan says so.
- Start menu: one "Suite" entry (the home screen) plus the four editors, each with its AUMID.
- Upgrade: major upgrade on version; uninstall removes everything it added (§7.3 check).

### 7.2 "Open with" registration (owner requirement, 2026-10-22)
Following upstream's model (F13), under **our** ProgIDs:

| App (exe from `brand.toml`) | Extensions registered | Not registered (said plainly) |
|---|---|---|
| Docs | docx, docm, dotx, dotm, doc, dot, odt, rtf, txt, md, markdown, html, htm (`wordcraft/crates/engine/src/io.rs:9-29`) | — |
| Sheets | xlsx, xlsm, xltx, xltm, xls, xlt, xlsb, ods, csv, tsv (`gridcraft/crates/engine/src/io.rs:12-39`) | — |
| Slides | pptx, potx, ppsx (`deckcraft/crates/engine/src/cmd/file.rs:263`, `:340`) | **odp** (deckcraft does not open it) and **.ppt** (O2) |
| PDF | pdf | — |

For each extension the installer writes:
- `HKLM\Software\Classes\<ext>\OpenWithProgids\<ProgID>`;
- the ProgID `<aumid>.Document` (e.g. `Suite.Docs.Document`; the prefix comes from `brand.toml`), with a friendly
  type name (plain text from the `brand.toml` names, in the install language), an icon, and
  `shell\open\command = "<exe>" "%1"`;
- `Applications\<exe>\SupportedTypes\<ext>`, plus `FriendlyAppName`;
- `RegisteredApplications\<Name>` → `Software\<Vendor>\<App>\Capabilities`, with
  `ApplicationName`/`Description`/`Icon` and `FileAssociations\<ext> = <ProgID>`, so the app appears in Settings ›
  Default apps.

**Defaults.** Windows 10/11 protect the per-user `UserChoice` with a hash, and an app cannot make itself the default.
We never write `UserChoice`. Defaults are set three ways:
1. The user picks the app in "Open with" with "Always use this app" once.
2. **Domain PCs:** the GPO "Set a default associations configuration file" with the XML we ship
   (`packaging/windows/default-associations.xml`, generated from `brand.toml`; B-11 decides which types are in it).
3. "Set as default" in our app or the home screen opens Settings › Default apps (`ms-settings:defaultapps`), as
   upstream does (`default_apps.rs:87`). It never does more.

### 7.3 Tests (WP14 done-check; clean Windows 10 user)
- Install in **Windows Sandbox**, a clean Windows 10 image every run (needs the optional feature and admin, Q1).
  Fallback: a Hyper-V VM on E:.
  - Right-click a .docx, .xlsx, .pptx and .pdf → **Open with** lists our app for each. This is checked by script:
    `assoc`, `HKCR\<ext>\OpenWithProgids`, and the Shell `IAssocHandler` enumeration
    (`SHAssocEnumHandlers` through PowerShell). A screenshot of each menu goes to `docs/phase4/evidence/openwith/`.
  - Opening through it loads the file, for paths with spaces and Cyrillic: `C:\Users\…\Рабочий стол\Буйруқ 1.docx`,
    `…\Hisobot 2026.xlsx`. The window title shows the name; the control channel's document info matches.
  - After choosing "Always", a double-click opens the file in our app.
  - **Single instance (F14):** with Docs running, open a second .docx → a second window, both documents intact. The
    same for Sheets and Slides. For PDF, the second file opens as a tab in the running instance.
  - Uninstall → a registry diff against the pre-install snapshot is empty for every key we wrote (ProgIDs,
    OpenWithProgids values, Applications, RegisteredApplications, Capabilities, App Paths, shortcuts).
- **Windows 11:** the same script and screenshots when owner hardware exists (Q1). Windows 11's "Open with" UI differs;
  the registry and handler-enumeration checks are the same.
- GPO: on a domain test VM, the shipped XML via `Dism /Online /Import-DefaultAppAssociations` (local equivalent), then
  the real GPO in a test OU when the owner provides one.

### 7.4 MSVC build and signing (F11)
- **Toolchain:** VS 2022 Build Tools (C++ workload, Windows 11 SDK) with the install path on E: where the installer
  allows it. Then `rustup target add x86_64-pc-windows-msvc` and a `CARGO_TARGET_DIR=E:/office-target/msvc/<d>`.
  **Blocked until the owner confirms the licence and C: space (Q10).** If it stays blocked, the gate is reported on the
  windows-gnu build, and "signed MSVC" moves to Phase 10, said plainly.
- **Signing:** `signtool` from the SDK, or `osslsigncode` (a GPL build tool, never shipped) if the SDK is not
  installed. Every exe, then the MSI. RFC 3161 timestamp. WP13 proves the pipeline with a **self-made test
  certificate**; the real certificate is the owner's (Q4). Since 2023, publicly trusted code-signing keys must live on
  hardware (a token or cloud HSM). Lead time matters.
- **Recommended interim:** for a pilot on domain PCs, an **internal enterprise-CA** code-signing certificate, trusted
  by GPO, plus GPO software deployment. An MSI deployed by GPO carries no mark-of-the-web, so SmartScreen does not
  prompt. A public OV certificate is for Phase 10 releases.

### 7.5 Windows verification (SIZING 4.8)
- A GUI smoke test ×4 on the MSVC build: start, open a sample, type, save, reopen, via the control channels.
- **WARP:** force the software adapter, measure start time and w40 scroll FPS.
- **RDP:** a session into a second machine (the Win11 VM). Loopback RDP to this PC is not assumed to work.
- Windows 11 and a domain PC: owner hardware (Q1).

---

## 8. i18n

### 8.1 New catalog rows
`i18n/shell.tsv`, same columns as the others (`i18n/shared.tsv:1`): the home screen, settings, history, the toolbar
labels of §2.3, the Docs preset's tab and group names, the "Open with" friendly names, and the Excel warnings of §6.
uz-Cyrl drafts via `uz-translit`, then the opus review → `reviewed:agent-opus:<date>`.
`cargo xt i18n export shell --format json` reuses the converter's export target. The fallback chains are
`i18n/config.toml:7-10`.

### 8.2 uz-Latn-2026 in the app catalogs (cheap path, F20)
- The export already derives uz-Latn-2026 from trusted uz-Cyrl rows for the converter (STATUS:1052-1055). Extend it to
  write `catalog-uz-latn-2026.tsv` for each app. Each `*/0110` patch then gains a **third, file-gated** language
  entry: one row in wordcraft and pdfcraft `LANGUAGES`, one cfg'd variant in gridcraft `Language`, one registry
  entry in deckcraft's own `i18n.rs`.
- **Cost:** about 1 d, plus a **size check**. Each extra embedded catalog adds an estimated 60–90 KB brotli, against
  19–27 KB of headroom (F19). If the measured delta breaks a budget, use runtime derivation: transliterate the uz-Cyrl
  catalog at language switch with `uz-translit`. That costs about 30 KB wasm where uz-translit is not linked yet
  (pdfcraft). Decide on the numbers; raising `delta_br_max` needs the STATUS line the budget rules require.
- The "Oʻzbekcha (1995)" / "Oʻzbekcha (2026)" picker labels match the converter page.

---

## 9. Mobile evidence, the minimum without an Android build (SIZING 4.9, ADR §2.2.1 step 3, Phase 2 M3)

**What the B-3 re-decision needs** (ADR §2.2.1, step 3): (i) a size for the 360 dp layout and a handle design; (ii) the
carry cost of a layout hook; (iii) A-web in the Expo WebView on a real phone. Android is deferred and no phone exists
(B-10), so (iii) **cannot be produced**, and the gate report says so.

**What Phase 4 does instead (WP16, 2.5 d):**
1. **Phone-sized web runs** in desktop Chromium: 360×800 and 390×844, DPR 3, touch, 4× CPU throttling (a mid-range
   proxy, stated as such). The shell and the four editors with the simplified toolbar in compact mode. Screenshots,
   plus an overflow measurement (how many toolbar items fit, and what overflows) → the **360 dp layout size** (i).
2. **M3 diagnosis** (the 108-page docx never reported `wordcraft:opened` on the emulator in 240 s;
   `docs/phase2/gate-review.md:32`, `docs/phase2/perf.md:109-111`):
   - instrument the open path with `performance.mark` in our bridge: fetch, `wordcraft:open` received, parse done,
     layout done, first paint;
   - run it with SwiftShader software GL and with WebGL, at 4× and 6× throttling, with a 512 MB heap cap
     (`--js-flags=--max-old-space-size=512`);
   - record which stage stalls, and the wasm memory high-water mark (`WebAssembly.Memory.buffer.byteLength`).
   - The outcome is one of: a cause found and a fix proposed; or the stage that stalls is named, and an emulator Chrome
     re-run (no Android build) stays optional for the owner.
3. **Handle design:** a written design for egui-drawn selection handles (geometry, hit targets, the long-press menu),
   no code.
4. **Hook carry cost (ii):** §2.5.

**Recommendation for the gate:** keep A-web conditional and move the final B-3 decision to the start of Phase 6, when
a phone exists. That says plainly that (iii) is missing.

---

## 10. XDFU in the editors (opt-in, off by default; phase3 PLAN §10 mapping)

- **Off means absent.** With `xdfu = "off"` (the default) no command is registered, no menu or toolbar entry exists,
  no check runs, and no audit file is created. A test asserts this per app.
- **Shared code:** `crates/xdfu-core` (ours) takes from `services/convert/src/xdfu.rs`:
  - `Mode` (`:53`), `find_grif` and its phrase table (`:102-117`), and the stamp line format;
  - the audit chain from `audit.rs` (HMAC under an operator key, `:71-121`).

  `services/convert` then depends on it. A pure move: the services tests must stay green with no change in output.
- **"Insert XDFU grif"** (cl. 32: upper-right of page 1, with the copy number under it):
  - Docs: `office.xdfu.insertGrif` in `docs-office`, built from engine commands: a right-aligned paragraph at the
    start of the body, with the grif text and "Nusxa №" from a copy counter;
  - Sheets: the right section of the first sheet's header, through the engine's page-setup command;
  - Slides: a text box at the top right of slide 1.

  The grif text is chosen in the command's dialog: XDFU / ХДФУ / ДСП, in the document's language.
- **Restriction** (cl. 11, 38) when the document carries a grif (detected on open and after insert, with `find_grif`
  over the top of page 1):
  - `refuse` mode: print, export, Save As to another format, copy and cut are disabled through the §4 hooks
    (`enabled` → a reason shown as a tooltip).
  - `watermark` mode: print and export carry the cl. 12 line (copy number, the computer's network address, the OS
    user, date and time):
    - Docs applies `design.watermark` (`crates/engine/src/cmd/design.rs:70`) for the operation and undoes it;
    - Sheets and Slides add a temporary header line or text box the same way.

    Copy and cut stay blocked in both modes. Every allowed print or export writes an audit line.
- **Plain save of the working file is allowed** (otherwise the document could not be edited). The local-mode warning
  of phase3 PLAN §10 is shown once per session.
- **Desktop only in Phase 4.** On the web, XDFU arrives with the docserver (Phase 5), where the server stamps.
- **Limits, said plainly (R11):** screenshots, OS-level printing of a screenshot, and copying from a PDF the user
  exported in `watermark` mode are outside the editor's control. Local mode on an uncertified PC cannot be compliant
  (phase3 PLAN §10). Switching XDFU on anywhere still needs O10 and the m13 remainder.

---

## 11. Work packages

Tiers: **scout** = haiku, **builder** = sonnet, **architect** = opus. Two cargo build slots (`scripts/lib/lock.sh:4`).
A Windows Sandbox or VM run counts as one slot; Playwright does not count.

### WP0. Upstream sync (in progress) and kick-off
- **Goal:** land the M2 sync (13 conflicts, 6 3-way, including gridcraft `0028`/`0029` against upstream
  `value_text_as`). Then re-verify this plan.
- **Steps:**
  - **WP0a (in progress, another agent):** the hand-merge, then the NumLocale tests, the WP14 matrix,
    `ci.sh convert-e2e`, the m10 locale probe, and D7 (the pdfcraft ru "Move down" one-liner) if upstream has not
    fixed it.
  - **WP0b (this phase's agent, after WP0a lands):**
    - re-grep every `file:line` in §0–§10 at the new BASEs;
    - re-measure `base_br` (`ci.sh sizes --measure-base`);
    - record each new BASE;
    - list the plan references that moved.
  - **Until WP0a lands, only streams B and C (no upstream patches) start.**
- **Files:** `patches/**`, `docs/phase1/sync-reports/<date>.md` (WP0a); `docs/phase4/wp0-notes.md` (WP0b).
- **Done-check:**
  - `scripts/verify-queue.sh all` exits 0;
  - `cargo xt register check` prints OK;
  - `scripts/ci.sh quick` exits 0;
  - `grep -c "moved\|confirmed" docs/phase4/wp0-notes.md` equals the number of references in §0;
  - `scripts/ci.sh convert-e2e` exits 0 with `directions.sh` "43 PASS / 0 FAIL" or more.
- **Estimate:** WP0a 1.5 d remaining (high 3); WP0b 0.5 d. **Tier:** architect (WP0a, in progress); scout (WP0b greps).

### WP1. Toolbar design (once for desktop and mobile) and owner review
- **Goal:** §2.1–§2.3 content frozen, with mockups.
- **Steps:**
  - the command lists of §2.3 checked against each registry, with the parameters for pickers;
  - the compact (360 dp) variant;
  - mockups as screenshots (the HTML shell's design tokens and the egui toolbars as image mockups);
  - label wording in 4 languages (drafts);
  - the owner reviews the layout (Q5).
- **Files:** `docs/phase4/toolbar.md`, `docs/phase4/evidence/toolbar/`.
- **Done-check:** `test -s docs/phase4/toolbar.md && grep -c "^| " docs/phase4/toolbar.md` ≥ 40 (rows of command
  ids); the owner's answer is recorded in the file.
- **Estimate:** 2 d (1.5–3). **Tier:** architect; owner 1–2 h.

### WP2. Docs: the ribbon preset (no hot-file patch)
- **Goal:** §2.2.
- **Steps:**
  1. Read the start-up order (hook → preferences). Pick install-time or overlay start-up for the first-run preset.
  2. `domains/docs/crates/docs-office`: `office.ribbonSimple`, the QAT toggle (desktop only), and re-applying the
     preset on a language change.
  3. Tests:
     - after first run the layout equals the preset;
     - the toggle round-trips;
     - an unknown id is dropped, not fatal;
     - `office_toolbar_ids_exist`.
  4. Screenshots in 5 languages.
- **Files:** `domains/docs/crates/docs-office/**`, `domains/docs/Cargo.toml`, `i18n/shell.tsv`.
- **Done-check:**
  - `cargo nextest run --manifest-path domains/docs/Cargo.toml -p docs-office` reports all passed;
  - `git diff --stat <BASE> -- upstream/wordcraft` shows **no new upstream file** from this WP;
  - the screenshot of the default start shows the preset tab and ¶ on Home.
- **Estimate:** 1.5 d (1–3). **Tier:** builder.

### WP3. Sheets and Slides: the carried simplified toolbar; the PDF check
- **Goal:** §2.3, §2.4.
- **Steps:**
  - `office_toolbar.rs` + `office_toolbar.toml` per app, the call-site branch, `pub(crate)` on the helpers needed;
  - the full-ribbon toggle (desktop only);
  - `office_toolbar_ids_exist` per app;
  - patches `gridcraft/0140`, `deckcraft/0140` with REGISTER rows;
  - kittest snapshot tests (simple and full);
  - the PDF Home walk-through.
- **Files:** `patches/{gridcraft,deckcraft}/0140-*`, `domains/{sheets,slides}/office_toolbar.toml`,
  `patches/REGISTER.md`.
- **Done-check:**
  - `scripts/verify-queue.sh gridcraft deckcraft` exits 0;
  - `scripts/ci.sh test gridcraft` and `test deckcraft` exit 0 with `office_toolbar_ids_exist` passed;
  - `git diff --stat` of each 0140 shows ≤ 20 changed lines in pre-existing upstream files (the rest in new files).
- **Estimate:** 4.5 d (3–7). **Tier:** builder; architect review of the patch shape (0.25 d).

### WP4. Command hooks (Sheets, Slides) and the Docs wrappers
- **Goal:** §4.
- **Steps:**
  1. Verify the bypass paths per app (async saves, system clipboard, keys). Write them down.
  2. `gridcraft/0141`, `deckcraft/0141` (a no-op unless installed).
  3. The `docs-office` wrappers.
  4. Tests: a refusing hook disables print; the after-hook sees `file.save`; with no hook, behaviour is byte-identical
     (an existing test suite passes unchanged).
- **Files:** `patches/{gridcraft,deckcraft}/0141-*`, `domains/docs/crates/docs-office/src/hooks.rs`,
  `docs/phase4/hooks.md`.
- **Done-check:** `scripts/ci.sh test-ours` exits 0, including `hook_refuses_print`, `hook_sees_save` for docs,
  sheets and slides; `docs/phase4/hooks.md` lists every bypass with its handling.
- **Estimate:** 1 d (0.75–2). **Tier:** builder; architect reads the bypass list.

### WP5. Shared settings, recent files, fonts and language
- **Goal:** §5.1–§5.3.
- **Steps:**
  - `crates/suite-core` (settings schema, recent merge with the four contract tests);
  - the overlay start-up reading of the shared values in the four `office_overlay.rs`;
  - fonts installed once.
- **Files:** `crates/suite-core/**`, `patches/*/0100-*` (our overlay file only), `Cargo.toml`.
- **Done-check:**
  - `cargo nextest run -p suite-core` all passed, including `recent_reads_{wordcraft,gridcraft,deckcraft,pdfcraft}`;
  - changing `lang = "uz-cyrl"` in `settings.toml` and starting each GUI shows uz-Cyrl (four screenshots).
- **Estimate:** 2 d (1.5–3). **Tier:** builder.

### WP6. Spell-check service and the deckcraft crash
- **Goal:** §5.4.
- **Steps:**
  - choose and licence-check the en_US and ru_RU dictionaries (`docs/phase4/licences.md`);
  - ship `dictionaries/`;
  - point both apps at it;
  - GUI reproduction of the squiggles crash: 20 runs, typing in two text boxes, control channel;
  - root cause and a carried fix if it reproduces.
- **Files:** `assets/dictionaries/**`, `docs/phase4/licences.md`, `docs/phase4/spell-crash.md`, maybe
  `patches/deckcraft/0142-*`.
- **Done-check:**
  - a misspelt Russian word shows a squiggle in Docs and Slides (screenshots);
  - `spell-crash.md` states "0 of 20 GUI runs crashed" or names the root cause with the fix's test;
  - `cargo deny` is unaffected (data only).
- **Estimate:** 1.5 d (1–3). **Tier:** builder; architect if the crash reproduces.

### WP7. Web shell and `suite-serve`, lazy loading and budgets
- **Goal:** §3.1–§3.3 and the web half of §1.
- **Steps:**
  - `web/shell/` (home, new, templates, recent in IndexedDB, search, settings, history placeholder for the web);
  - `services/suite-serve`;
  - the converter moved to `/convert/`;
  - per-route CSP with build-time inline hashes;
  - `.br` precompression;
  - `[shell]` budget (proposal: HTML + JS + CSS + i18n JSON + font ≤ 160 KiB brotli, measured + 10 % rounded to
    16 KiB) and **absolute per-editor budgets** (`base_br + delta_br_max`, stated);
  - Playwright specs:
    - `shell.spec.ts`: no `.wasm` request before an editor is opened; opening Docs fetches only `docs-web*.wasm`;
    - the gate flow on the web: new → type → save (download plus the shell copy) → reopen from Recent, for document,
      spreadsheet and presentation; PDF → DOCX in `/convert/`.
- **Files:** `web/shell/**`, `services/suite-serve/**`, `web/budgets.toml`, `ci/web-smoke/shell.spec.ts`,
  `scripts/ci.sh`.
- **Done-check:**
  - `npx playwright test -c ci/web-smoke/playwright.shell.config.ts` reports all passed, with the flows above;
  - `scripts/ci.sh sizes` exits 0 with a `shell` row;
  - the header test passes for `/`, `/convert/` and `/docs/`.
- **Estimate:** 4 d (3–6). **Tier:** builder; architect reviews the CSP and framing (0.25 d).

### WP8. deckcraft web host bridge
- **Goal:** §3.2, F7.
- **Steps:**
  - `apps/deckcraft-web/src/host.rs` with the five messages;
  - patch `deckcraft/0002-*` (or the next free number) with a REGISTER row;
  - a bridge spec mirroring wordcraft's 8 cases.
- **Files:** `patches/deckcraft/*`, `ci/web-smoke/bridge-slides.spec.ts`.
- **Done-check:** the bridge spec passes 8/8 three times in a row; `scripts/ci.sh check slides` exits 0 (native and wasm32).
- **Estimate:** 1.5 d (1–2.5). **Tier:** builder.

### WP9. Desktop host (`suite.exe`, WebView2)
- **Goal:** §3.4.
- **Steps:**
  - `wry` window over the in-process router;
  - the IPC actions (open in editor, pick, new from template, recent, search Documents/Desktop with a cap, history);
  - the browser fallback when WebView2 is missing;
  - `cargo deny` for the new crates;
  - a test that a cross-origin page cannot trigger an IPC action (IPC only from the WebView's own origin).
- **Files:** `services/suite-desktop/**`, `ci/deny/services.toml`.
- **Done-check:**
  - `cargo nextest run --manifest-path services/Cargo.toml -p suite-desktop` all passed;
  - a scripted desktop run (the control channels of the 3 editors plus the host's test IPC) creates, edits, saves and
    reopens one document, spreadsheet and presentation, and converts a PDF. The log ends `GATE-FLOW desktop: 4/4 ok`;
  - `cargo deny … services` reports "licenses ok".
- **Estimate:** 2.5 d (2–4). **Tier:** builder; architect security review of the IPC (0.25 d).

### WP10. History: local snapshots and the History panel
- **Goal:** §5.5.
- **Steps:**
  - `suite-core::history` (store, hash chain, retention, restore-as-new-version);
  - the after-save hooks (WP4);
  - the History panel in the shell;
  - "Version history" entries in the editors;
  - tests: restore never destroys; pruning keeps named versions; a tampered index line is detected; a 2 GB cap is
    honoured with synthetic 80 MB files.
- **Files:** `crates/suite-core/src/history.rs`, `web/shell/history.*`, `domains/docs/crates/docs-office/src/history.rs`.
- **Done-check:**
  - `cargo nextest run -p suite-core history` all passed;
  - a manual flow script saves a docx three times, restores v1, and lists 4 versions with v1's hash equal to the
    restored file's: `scripts/history-check.ps1` prints `OK 4 versions, restore = v1`.
- **Estimate:** 3.5 d (2.5–5). **Tier:** builder.

### WP11. XDFU in the editors (opt-in, off by default)
- **Goal:** §10.
- **Steps:**
  - extract `crates/xdfu-core` (a pure move; services tests unchanged);
  - the three grif commands;
  - grif detection on open;
  - refuse and watermark modes through the WP4 hooks;
  - the audit lines;
  - the "off means absent" test per app;
  - i18n rows.
- **Files:** `crates/xdfu-core/**`, `services/convert/src/xdfu.rs`, `domains/docs/crates/docs-office/src/xdfu.rs`,
  the gridcraft/deckcraft command modules (inside the existing 0120-style command patches), `i18n/shell.tsv`.
- **Done-check:**
  - `cargo nextest run --manifest-path services/Cargo.toml` passes at least **206 of 206** with no output change
    (the move);
  - per app:
    - `xdfu_off_registers_nothing` passes;
    - with `watermark`, an exported PDF's text contains the copy number and the OS user (`pdf-cli text`);
    - with `refuse`, `file.print` returns the reason;
    - the audit chain verifies.
- **Estimate:** 4 d (3–6). **Tier:** builder; architect review with WP9's (0.5 d).

### WP12. Excel workload guard
- **Goal:** §6.
- **Steps:**
  - re-run the round-2 GUI memory protocol on w40/w80/w120 with the Phase 4 gridcraft;
  - the shell's pre-open scan (wall, external links) with "Open in Excel";
  - check the GUI's own message at the wall.
- **Files:** `docs/phase4/excel-guard.md`, `web/shell/` (the warning), `crates/suite-core/src/scan.rs` (a reuse of
  `services/convert` scan code).
- **Done-check:** `excel-guard.md` has the table "steady / peak / open s" for w40, w80, w120, each within +5 % of
  566/903/1,240 MB; a synthetic workbook with `xl/externalLinks/externalLink1.xml` shows the warning (screenshot); a
  synthetic over-the-wall file shows the wall message and never a blank window.
- **Estimate:** 1.5 d (1–2.5). **Tier:** builder.

### WP13. MSVC toolchain and the signing pipeline
- **Goal:** §7.4. **Blocked on Q10** for the toolchain; the signing pipeline proof is not.
- **Steps:**
  - after the OK: install Build Tools and the SDK (E: where possible);
  - `x86_64-pc-windows-msvc` builds of the 4 GUIs, 4 CLIs, `convert`, `suite.exe`;
  - compare with the gnu builds (sizes, a GUI smoke);
  - `scripts/sign.sh` with a test certificate and a timestamp.
- **Files:** `scripts/build.sh` (a `--msvc` flag), `scripts/sign.sh`, `docs/phase4/msvc.md`.
- **Done-check:**
  - `scripts/build.sh all desktop --release --msvc` exits 0;
  - `signtool verify /pa /v <exe>`, or `osslsigncode verify`, prints the test certificate's subject for every exe
    and the MSI;
  - if blocked: `msvc.md` states "blocked: <reason>", and the gate uses windows-gnu.
- **Estimate:** 2 d (1–3), with the owner OK. **Tier:** builder.

### WP14. Suite MSI, "Open with", GPO XML
- **Goal:** §7.1–§7.3.
- **Steps:**
  - `cargo xt brand msi` (wxs and default-associations XML from `brand.toml`);
  - WiX 5.0.2 pinned in `scripts/tools.lock`;
  - the per-machine MSI (and per-user if cheap);
  - `scripts/msi-test.ps1` run inside Windows Sandbox: install, the "Open with" checks, the spaces/Cyrillic paths, the
    single-instance checks, "Always", uninstall, the registry diff.
- **Files:** `xtask/src/brand.rs`, `packaging/windows/**` (generated + templates), `scripts/msi-test.ps1`,
  `docs/phase4/evidence/openwith/`.
- **Done-check:** `scripts/msi-test.ps1 -Sandbox` prints:
  - `OPENWITH docx ok`, `xlsx ok`, `pptx ok`, `pdf ok`;
  - `OPEN spaces+cyrillic 4/4 ok`;
  - `SINGLE docs/sheets/slides new-window ok, pdf tab ok`;
  - `ALWAYS docx ok`;
  - `UNINSTALL registry diff 0`.

  It exits 0.
- **Estimate:** 4 d (3–6). **Tier:** builder; architect reviews the registry design (0.25 d).

### WP15. Windows verification (MSVC, WARP, RDP, Windows 11, domain)
- **Goal:** §7.5.
- **Steps:** the GUI smoke ×4 on MSVC; WARP; RDP; the WP14 script on Windows 11; the GPO XML on a domain VM.
- **Files:** `docs/phase4/windows.md`.
- **Done-check:** `windows.md` has one row per check with a number or "not run: <owner item>"; the Win11 run prints the
  same `msi-test.ps1` lines as WP14.
- **Estimate:** 3 d (2–4). **Tier:** builder. **Needs Q1.**

### WP16. Mobile evidence minimum and the M3 diagnosis
- **Goal:** §9.
- **Files:** `docs/phase4/mobile.md`, `ci/web-smoke/phone.spec.ts`, `docs/phase4/evidence/phone/`.
- **Done-check:**
  - `npx playwright test ci/web-smoke/phone.spec.ts` produces 10 screenshots (5 surfaces × 2 viewports);
  - `mobile.md` has the 360 dp fit table, the M3 stage-timing table (which stage stalls, with ms and memory), the
    handle design, and the carry table of §2.5.
- **Estimate:** 2.5 d (2–4). **Tier:** builder; architect for the M3 diagnosis and the B-3 pack.

### WP17. i18n: shell catalog, plain labels, uz-Latn-2026 in the apps
- **Goal:** §8.
- **Steps:**
  - `i18n/shell.tsv` drafts, uz-Cyrl via translit, opus review;
  - export JSON;
  - the uz-Latn-2026 derivation for the four apps and the 0110 third entry;
  - size measurement; decide embedded vs runtime.
- **Files:** `i18n/shell.tsv`, `xtask/src/i18n/export.rs`, `patches/*/0110-*`, `web/budgets.toml` (only with a
  STATUS line).
- **Done-check:**
  - `scripts/ci.sh i18n` exits 0 (lint 0 errors);
  - screenshots of the shell and the 4 apps in uz-Latn-2026 with no tofu;
  - `scripts/ci.sh sizes` exits 0.
- **Estimate:** 2 d (1.5–3). **Tier:** builder; architect (opus) for the Uzbek review.

### WP18. Accessibility baseline
- **Goal:** a measured baseline (SIZING 4.10).
- **Steps:**
  - keyboard-only: the gate flow on the shell and the desktop editors;
  - Narrator on the shell and the desktop editors (AccessKit, F17);
  - the web canvas with Narrator in Edge;
  - the shell's contrast tokens;
  - a Playwright aria snapshot of the shell.
- **Files:** `docs/phase4/a11y.md`, `ci/web-smoke/shell-a11y.spec.ts`.
- **Done-check:** `a11y.md` has a pass/fail matrix (surface × check) with evidence links; the aria spec passes.
- **Estimate:** 1.5 d (1–2). **Tier:** builder.

### WP19. Server mode over TLS in a real browser; the accounts seam
- **Goal:** carry-forward §10.6 item 6; §5.6.
- **Steps:**
  - a test CA and a localhost certificate (generated in the test);
  - `suite-serve` in server mode (JWT, TLS);
  - Chromium launched with `--ignore-certificate-errors-spki-list=<pin>`, so the TLS handshake is real and pinned;
  - the checks: shell and converter load over https, a token arrives via the fragment, a conversion completes, HSTS
    and CSP are present, plain HTTP redirects;
  - Settings "Connect to server" (navigate only);
  - the providers config;
  - the HRM-name grep test.
- **Files:** `ci/web-smoke/tls.spec.ts`, `services/suite-serve/tests/tls.rs`, `docs/phase4/accounts.md`.
- **Done-check:** `npx playwright test ci/web-smoke/tls.spec.ts` all passed; `scripts/ci.sh test-ours` passes
  `no_hrm_names_in_code`.
- **Estimate:** 1.5 d (1–2). **Tier:** builder; architect security read.

### WP20. Carry-forward minors
- **Goal:** gate-review §10.6 items 2 and 4.
- **Steps:**
  - n1 (the api.md line); n2 (the combine-mode status column + a Playwright assertion); n3 (`reg.exe` by full path or
    `GetUserDefaultLocaleName`); n4 (an en-US csv ratchet case);
  - m6 (`crates/uz-translit/src/engine.rs:309-316`, before O13 flips the default);
  - m10 (the PDF locale probe, after WP0);
  - D7 if WP0a did not carry it.
  - m5 rest → Phase 5; m13 rest → before XDFU is switched on; m14 → Phase 10 (unchanged).
- **Done-check:** `scripts/ci.sh convert convert-e2e` exits 0; `silent_baseline.py check` prints "0 new";
  `cargo nextest run -p uz-translit` passes a new `auto_to_latin_follows_default` test.
- **Estimate:** 1 d (0.5–1.5). **Tier:** builder.

### WP21. Linux packages (optional; Q12)
- **Goal:** SIZING 4.11, only if the owner wants it.
- **Steps:** the four apps built in `rust:1.97-bookworm` (pinned digest); upstream's `nfpm.yaml` as the model (renamed
  from `brand.toml`); the shell via the browser fallback (§3.4); an Xvfb start smoke test.
- **Done-check:** `docker run … dpkg -i suite-*.deb && suite-docs --version` prints the brand version for 4 apps.
- **Estimate:** 2 d (1.5–4). **Tier:** builder. **Not in the 53-day total.** macOS: **no Mac here, UNSIZED**, said
  plainly.

### WP22. CI
- **Goal:** everything above runs in `ci.sh`.
- **Steps:**
  - jobs: `shell` (suite-serve tests, shell/phone/tls/a11y specs, sizes), `desktop-host`, `msi` (nightly; Sandbox
    when available), `excel-guard` (nightly);
  - the `quick` triggers on `web/shell/**`, `services/suite-*/**`, `crates/suite-core/**`;
  - mirror the workflow; actionlint.
- **Done-check:** `scripts/ci.sh quick` and `scripts/ci.sh full` exit 0 with the new jobs listed in the summary.
- **Estimate:** 1 d. **Tier:** builder.

### WP23. "New user, no instructions" check (owner item: staff)
- **Goal:** gate item 1 with people (SIZING 4.12).
- **Steps:**
  - a protocol: 5 tasks (a letter, a table with a sum, a 3-slide deck, PDF → Word, find Show/Hide ¶), no help given,
    think-aloud, time and success per task;
  - 2–3 staff on a test PC (desktop) and one task set on the web;
  - findings → fixes within the phase if they are small.
  - No personal data is recorded: only task ids, times and outcomes.
- **Files:** `docs/phase4/usability.md`.
- **Done-check:** `usability.md` has a table of participants (P1–P3) × tasks with success/time, and every failure has a
  fix commit or a gate note.
- **Estimate:** 1.5 d (1–2). **Tier:** builder (protocol, notes); architect (findings). **Needs Q6.**

### WP24. Gate: adversarial review and report
- **Goal:** prove §12.3; the B-3 pack; report to the owner in Uzbek.
- **Steps:**
  - an architect review covering security (CSP, IPC, TLS, XDFU off), licences (wry, dictionaries, WiX), carry
    (the second sync dry run, §2.5), and the gate flows on web and desktop;
  - `ci.sh full`;
  - STATUS.
- **Files:** `docs/phase4/gate-review.md`, `docs/STATUS.md`.
- **Done-check:** `scripts/ci.sh full` exits 0; every row of §12.3 has an evidence link; the B-3 pack is in
  `gate-review.md`.
- **Estimate:** 1.5 d. **Tier:** architect.

### Parallel streams and the critical path
```
day:        1    3    5    7    9   11   13   15   17   19   21
A patches   WP0a ███ WP0b▌WP1 ████ WP2 ██▌WP3 ███████▌WP4 █▌WP6 ██▌WP17 ███▌
B web/desk  WP7 ███████▌WP8 ██▌WP9 ████▌WP10 ██████▌WP19 ██▌
C windows   WP13 ███(Q10)▌WP14 ███████▌WP12 ██▌     WP15 █████(Q1)▌
D services  WP5 ███▌WP20 ▌WP16 ████▌WP11 ███████▌WP18 ██▌WP22 █▌
gate                                                   WP23 ██▌WP24 ██▌
```
- **Critical path (two near-equal chains):**
  - patches: WP0a → WP0b → WP1 → WP3 → WP17 → WP23 → WP24 = 1.5 + 0.5 + 2 + 4.5 + 2 + 1.5 + 1.5 = **13.5 stream-days**;
  - install: WP7 → WP9 → (host into WP14) → WP15 → WP23 → WP24 = 4 + 2.5 + 0.5 + 3 + 1.5 + 1.5 = **13 stream-days**.
- With the per-stream loads (A ≈ 15, B ≈ 13, C ≈ 10.5, D ≈ 12.5), integration slack and owner latency (Q1, Q5, Q6,
  Q10), the calendar is **about 18–22 working days likely, about 28 high**.
- **Owner latency sits on the critical path three times:** the toolbar review (WP1), the hardware and toolchain OK
  (WP13, WP15), and staff for WP23. Asking for all three on day 1 is the main schedule lever.
- Build slots: stream A's queue rebuilds plus one of B/D's cargo builds. Windows Sandbox runs (C) take one slot while
  they run.

---

## 12. Risks, estimate, gate mapping, owner questions

### 12.1 Estimate (stream-days)
| WP | Low | Likely | High | SIZING item |
|---|---|---|---|---|
| WP0 (a remaining + b) | 1 | 2 | 3.5 | carry-forward (new) |
| WP1 | 1.5 | 2 | 3 | 4.3 |
| WP2 | 1 | 1.5 | 3 | 4.4 (Docs, was 3/4/6) |
| WP3 | 3 | 4.5 | 7 | 4.4 (Sheets, Slides, PDF; was 6/9/12) |
| WP4 | 0.75 | 1 | 2 | 4.5 |
| WP5 | 1.5 | 2 | 3 | 4.5 |
| WP6 | 1 | 1.5 | 3 | 4.5 + the crash item |
| WP7 | 3 | 4 | 6 | 4.1 + 4.2 |
| WP8 | 1 | 1.5 | 2.5 | moved from 5.5 |
| WP9 | 2 | 2.5 | 4 | 4.1 |
| WP10 | 2.5 | 3.5 | 5 | new (history, STATUS) |
| WP11 | 3 | 4 | 6 | new (XDFU, phase3 §10) |
| WP12 | 1 | 1.5 | 2.5 | new (Excel guard) |
| WP13 | 1 | 2 | 3 | 4.7 |
| WP14 | 3 | 4 | 6 | 4.6 |
| WP15 | 2 | 3 | 4 | 4.8 |
| WP16 | 2 | 2.5 | 4 | 4.9 (reduced: no phone) |
| WP17 | 1.5 | 2 | 3 | new (i18n) |
| WP18 | 1 | 1.5 | 2 | 4.10 |
| WP19 | 1 | 1.5 | 2 | carry-forward 6 |
| WP20 | 0.5 | 1 | 1.5 | carry-forward 2, 4 |
| WP22 | 0.75 | 1 | 1.5 | (in 4.2) |
| WP23 | 1 | 1.5 | 2 | 4.12 |
| WP24 | 1 | 1.5 | 2 | 4.13 |
| **Total** | **38** | **53** | **~76** | SIZING 4.x: 30.5 / 44.5 / 63 |
| WP21 (optional) | 1.5 | 2 | 4 | 4.11 |

**Why it differs from SIZING:**
- Added since SIZING: history 3.5, XDFU 4, TLS 1.5, sync 2, Excel guard 1.5, i18n 2, minors 1, the deckcraft bridge
  1.5 (moved). That is about +17.
- Saved:
  - the toolbar 13 → 9 (WP1+WP2+WP3+WP4 hook share), because F1 and F5 remove the Docs and PDF hot-file patches;
  - the mobile task 4 → 2.5 (no phone, so no Expo run).
- The 12-day SIZING buffer stays the right size. The toolbar risk is lower, but the owner-hardware and MSVC
  uncertainty is new.

### 12.2 Risks
| # | Risk | Likelihood / impact | Mitigation | Proven by |
|---|---|---|---|---|
| R1 | The in-progress sync (13 conflicts) takes longer or breaks NumLocale | medium / blocks stream A | Streams B and C need no upstream change and start first; WP0a has its own gates | WP0 |
| R2 | The toolbar carry conflicts at every sync (hot `lib.rs`/`ribbon.rs`) | medium / weekly tax | New files plus a call-site line; id tests instead of merges; measured over two syncs | WP3, §2.5, WP24 |
| R3 | The Docs preset (still a ribbon) is "not simple enough" for staff | medium / UX | Owner review (WP1), usability (WP23); fallback: the Sheets-style toolbar for Docs, +2 d | WP1, WP23 |
| R4 | WebView2 missing or blocked on fleet PCs | low-medium / desktop home | Present on Win11 and on this Win10 22H2 (F16); automatic browser fallback; B-12 | WP9, Q3 |
| R5 | No MSVC here; licence and C: space (16.4 GB) | certain until Q10 / signed MSVC | Install on E: where possible; otherwise the gate on gnu and MSVC in Phase 10, said plainly | WP13 |
| R6 | Code-signing certificate lead time (HSM-held keys since 2023) | high / pilot rollout | Internal enterprise CA + GPO deployment for the pilot; start the public certificate now | WP13, Q4 |
| R7 | WiX OSMF (v6+) | low with the pin / cost | WiX 5.0.2 pinned, as upstream | WP14 |
| R8 | The deckcraft spell crash is a real app crash once dictionaries exist | medium / stability | GUI reproduction first; spell-check default-on in Slides only if clean | WP6 |
| R9 | Owner workbooks reach the 1 GiB wall; external links not refreshed | high / Excel users | Pre-open warnings, "Open in Excel", Sheets not the .xlsx default; track E5/E6 | WP12, Q2 |
| R10 | History fills the disk with 80+ MB workbooks | medium / disk | Retention: 5 versions over 50 MB, 2 GB cap, named versions kept | WP10 |
| R11 | XDFU restrictions bypassed (system clipboard, screenshots, unguarded UI paths) | medium / compliance | The bypass list in WP4; stated limits; off by default; O10 before switching on | WP4, WP11 |
| R12 | No phone: B-3 re-decided on weaker evidence | certain / mobile decision | Phone-viewport runs + M3 diagnosis; recommend deferring the final B-3 to the Phase 6 start | WP16 |
| R13 | Staff time for the usability check | medium / gate | Ask on day 1 (Q6); 45 minutes each | WP23 |
| R14 | Editor CSP needs `'wasm-unsafe-eval'` + inline hashes | certain / web security | Per-route CSP, editor routes only; frame-ancestors 'self' | WP7 |
| R15 | A loopback server lets other local pages trigger desktop actions | medium / security | Desktop actions only through WebView IPC, origin-checked; HTTP surface = Phase 3 local mode | WP9 |
| R16 | Linux desktop host would link webkit2gtk (LGPL) | certain if Linux / licence | Browser fallback on Linux; any linked LGPL needs an owner decision | WP21, Q12 |
| R17 | Windows 11 and domain hardware not provided | medium / gate evidence | Windows Sandbox for clean Win10; Win11 evaluation VM on E: as fallback (admin + Hyper-V) | WP15, Q1 |
| R18 | Upstream velocity (100–140 commits/day) moves every reference | high | WP0b re-grep; id tests; the sync gates | WP0, WP22 |
| R19 | The third uz catalog breaks the wasm budgets | medium / budgets | Measure; runtime derivation if over | WP17 |

### 12.3 Exit-gate items → proving WP
| Gate item | Evidence | WP |
|---|---|---|
| **Web:** a new user creates, edits, saves and reopens a document, a spreadsheet and a presentation | Playwright gate flow (`shell.spec.ts`), the deckcraft bridge spec, WP23 web task set | WP7, WP8, WP23 |
| **Desktop:** the same | the scripted desktop flow (`GATE-FLOW desktop: 4/4 ok`), the MSI install test, WP23 desktop sessions | WP9, WP14, WP23 |
| … **and converts a PDF** | PDF → DOCX through `/convert/` in the web flow and in the desktop host | WP7, WP9 |
| … **without instructions** | `docs/phase4/usability.md` (2–3 staff, no help, success per task) | WP23 |
| **Bundle budgets are met** | `ci.sh sizes` with the `[shell]` row and the absolute editor budgets; the lazy-load spec (no wasm before use; only the opened editor's wasm) | WP7, WP17, WP22 |
| Brief task 1: home screen (new, templates, recent, search) | shell specs; desktop IPC tests | WP7, WP9, WP5 |
| Brief task 3: simplified toolbar by default, full-ribbon toggle on desktop, plain Uzbek labels; Show/Hide ¶ visible | WP2/WP3 tests and screenshots (5 languages); WP23 "find ¶" task | WP1–WP3, WP17 |
| Brief task 4: single installer (Windows; macOS none, no Mac; Linux optional) | `msi-test.ps1` lines; `msvc.md`; `windows.md` | WP13–WP15, WP21 |
| Brief task 5: shared services | `suite-core` tests; the language from settings in four apps; the dictionaries; the fonts | WP5, WP6 |
| Owner: "Open with" for every type, defaults by user or GPO, single instance | WP14 done-check lines | WP14, WP15 |
| Owner: history (Phase 4 part), XDFU (off by default), accounts optional | WP10, WP11, WP19 tests | WP10, WP11, WP19 |
| B-3 re-decision pack | `mobile.md` + the carry table | WP16, WP24 |
| Carry-forward closed or re-homed | WP0, WP19, WP20; the list in `gate-review.md` | WP0, WP19, WP20 |

### 12.4 Owner questions (with recommendations)
| # | Question | Recommendation |
|---|---|---|
| **Q1 (B-10)** | Hardware: a **Windows 11** PC or VM; a **domain-joined test PC/VM** in a test OU for the GPO test; a mid-range **Android phone**; admin rights to enable Windows Sandbox/Hyper-V here | IT provides a Win11 VM and a domain test VM in a test OU in week 1. This PC is domain-joined but is the owner's work PC: we will not test GPO on it. The phone can wait while Android is deferred, but then the final B-3 decision moves to the Phase 6 start. |
| **Q2 (B-11)** | Default-handler policy on office PCs | GPO XML with **Docs** for docx/doc/rtf/odt, **Slides** for pptx, **PDF** for pdf. **Excel stays the .xlsx default** until track E (links, the 1 GiB wall); Sheets is in "Open with". Non-domain PCs: the user picks "Always". |
| **Q3 (B-12)** | Fleet facts: PC count, Win10/11 split, RAM, domain/GPO, WebView2 present, Office versions, VBA macro use, RDP/thin clients, GPU | Answer before WP14 freezes the MSI options. Without it we assume Win10 22H2+, ≥ 8 GB, domain-joined, WebView2 present. |
| **Q4** | Code-signing certificate | **Pilot:** an internal enterprise-CA code-signing certificate, trusted by GPO, with GPO deployment. **Public releases (Phase 10):** an OV certificate on a hardware token or cloud HSM. Start procurement now (weeks of lead time). |
| **Q5** | Toolbar design review | The owner reviews the WP1 layout (1–2 h); label wording stays delegated to the agent (`reviewed:agent-*`). |
| **Q6** | 2–3 office staff for the "no instructions" check | One Word-heavy, one Excel-heavy, one other; 45 min each, in about week 3; no personal data recorded. |
| **Q7 (O8)** | The default PDF → DOCX engine | Keep the code default **native** (WP10 Phase 3 numbers), with the D5 fallback. |
| **Q8 (O9)** | Deliver `corpus/` | Still the biggest gap. At least 20 files, including large and linked workbooks, saved by Office 2010–365. |
| **Q9 (O10)** | Will XDFU documents go through our tools at all? | Keep building it opt-in and off. Before anyone switches it on, the registry (devonxona) and security unit confirm it, and m13 is closed. |
| **Q10** | Install **VS 2022 Build Tools + Windows SDK** here (licence, about 7–8 GB, C: has 16.4 GB) | Yes, under the organisation's VS licence or Community's open-source terms (the owner confirms which), installed on E: where the installer allows. Otherwise "signed MSVC" moves to Phase 10. |
| **Q11** | Ship the four CLIs in the suite MSI (about +224 MB) so the converter works offline | Yes. Revisit the footprint in Phase 10. |
| **Q12** | Linux packages in Phase 4? (The desktop host would need webkit2gtk, LGPL, or the browser fallback) | Defer (WP21 optional). If wanted, use the browser fallback, so no LGPL linking. |
| **Q13** | Accept WebView2 as the desktop home screen's runtime | Yes, with the automatic browser fallback (F16). |

---

[ADR]: ../ARCHITECTURE-DECISION.md
[SIZING]: ../phase1/SIZING.md
[phase3-PLAN]: ../phase3/PLAN.md
[gate-review-3]: ../phase3/gate-review.md
