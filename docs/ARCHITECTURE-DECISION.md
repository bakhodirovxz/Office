# Architecture decision: office suite on the Craft engines

| | |
|---|---|
| **Status** | Proposed. For the owner's decision at the Phase 0 exit gate. |
| **Date** | 2026-10-16 |
| **Phase / branch** | Phase 0, `phase-0` |
| **Inputs** | [BRIEF], [STATUS] and every report in `docs/phase0/` (listed under "Sources" at the end) |

**How to read the numbers**

- A number is **MEASURED** in Phase 0 unless it is marked otherwise. Its source report is linked next to it.
- **PROJECTION** means arithmetic on measured unit costs.
- **ESTIMATE** means an engineering judgement written down in a Phase 0 report. It is not a measurement.
- **UNVERIFIED** means not tested.
- **UNSIZED** means Phase 0 gives no evidence to estimate the item. The phase that owns it sizes it first.
- **Test bench.** One PC: Windows 10 Pro 19045, 12 cores, 11.8 GB RAM, Intel UHD 630, the `windows-gnu` toolchain. Plus
  one Android emulator: Pixel 8 AVD, x86_64, API 36, software GL.
- **Not available:** macOS, Windows 11, a physical phone, the owner's `corpus/` ([STATUS]).

---

## 0. Decisions in one page

| # | Area | Recommendation | Owner decision needed? |
|---|---|---|---|
| 1 | Repository | **One monorepo, but not one merged Cargo workspace.** Each editor gets its own workspace (a "build domain"), and the services get one more. Each domain path-depends on an unmodified upstream app and reuses its lockfile, `[patch]` set and profiles. Upstream is vendored as pinned pristine snapshots with an explicit patch queue. | **Yes.** This deviates from BRIEF §5 "One Cargo workspace" (§2.1, B-5). |
| 2 | Desktop | Use the upstream egui apps as they are, rebranded, with a simplified toolbar by default. Ship four editor processes plus a home launcher in **one suite MSI**, with an optional default-association policy. | Default-handler policy (B-11) |
| 3 | Web | egui wasm, **one module per editor, lazy-loaded**. Open and save go through the host bridge. Size budgets are a ratchet on *our* delta. | No |
| 4 | Mobile | **v1 = A-web.** That is the same egui wasm running in the HRM WebView, in a thin WebView wrapper or as a PWA, plus our upstreamable patches and a compact touch toolbar. **Approach B's binding layer stays compiling as the fallback**, to be re-decided at the Phase 6 gate on real-device data. A-native is rejected for v1. | **Yes** (B-3) |
| 5 | Document server / HRM | Rust/axum, DocsAPI-shaped, grown from the spike. The target is `hr-backend`. **The HRM security fixes land before any Craft connection.** The pilot is Docs-only, behind a flag, and ONLYOFFICE stays as the instant fallback. | Security actions and the open questions (B-9) |
| 6 | Excel | Replacing Excel for the owner's workload needs **31.5–51 engineer-days of gridcraft engine work** (PROJECTION from spike estimates; §2.3), and that work must be accepted upstream. Cut over per file: Excel stays for the heaviest and linked workbooks until it lands. | **Yes** (B-7) |
| 7 | Upstream | Send a PR first, carry the patch only if the PR is rejected, never rewrite. Sync weekly. Keep a register of every patch. | **Yes.** Permission to send PRs (B-6) |
| 8 | Priorities | **Fidelity (including the Excel engine) → mobile → co-editing.** | **Yes** (B-4) |

---

## 1. Context and decision drivers

### 1.1 Hard criteria

An option that fails any of these is rejected.

| ID | Criterion | Why it is hard | Evidence |
|---|---|---|---|
| D1 | **Editing on desktop, web and mobile**, not only viewing. This covers the editors, the HRM embed (web and the Expo mobile app), the converter and transliteration. | Owner requirement, re-stated 2026-10-15: "an architecture that passes on desktop and web but fails the Android touch and keyboard tests is rejected" | [STATUS] |
| D2 | **Replace MS Office on Windows 10/11 office PCs** | Requires: Office 2010+ files; a Windows installer; heavy Excel (formulas, many sheets, links to other workbooks, files over 80 MB that keep growing); Show/Hide ¶ for Word users | [STATUS] |
| D3 | **HRM embedding** | Open by URL, save by callback, JWT-signed config, permissions. Also HRM sign-in and sync for the standalone apps. The target is `hr-backend` (owner decision 2026-10-16). | [BRIEF] §4.4, [STATUS], [hrm-current-state] |
| D4 | **Upstream velocity** | Commits in the last 7 days: wordcraft 701, gridcraft 993, deckcraft 859, pdfcraft 823 (about 100–140 a day each). Total commits: 727, 1015, 884, 1165. So 96%, 98%, 97% and 71% of each repo's history is from the last week (PROJECTION: ratio of the two counts). | [STATUS] |
| D5 | **No rewrite** | Our work must be thin layers plus minimal, upstreamable patches | [BRIEF] §5 |
| D6 | **Permissive licensing** | Our code and linked dependencies are permissive and enforced by `cargo-deny`. GPL/AGPL tools run only as sidecars. | [BRIEF] §5 |

Derived criteria:
- **Evidence over assertion.** A claim without a measurement is marked UNVERIFIED.
- **Thoroughness over speed.** This is the owner's stated preference.

### 1.2 Requirement trace: where Phase 0 leaves each owner requirement

| Requirement | State today (upstream + our spikes) | Gap | Source |
|---|---|---|---|
| Open/save .docx .docm .dotx .dotm | Supported | Fidelity on real files is unmeasured | [STATUS] (`wordcraft/crates/engine/src/io.rs:11-15,183`) |
| .doc import | Import only, saves as .docx. The round trip **loses comments and tracked changes** (1→0 each) | Warn the user | [STATUS], [fidelity-baseline] |
| .xlsx family open/save; .xls/.xlsb/.ods | Open and save the xlsx family. The others are import only; writing them is refused. `.xls` import **drops charts and conditional formats** | Warn the user | [STATUS], [fidelity-baseline] |
| .pptx | 5/5 slides, shapes, table, picture and notes survive; pixel diff 0.61% | Chart survival not measured | [fidelity-baseline] |
| **.ppt (97–2003)** | **Does not open** | New work (§3) | [fidelity-baseline], [STATUS] |
| Show/Hide ¶ | The `view.marks` command exists | Must be visible in the simplified toolbar | [STATUS] (`wordcraft/crates/engine/src/cmd/view.rs:18`) |
| Excel formulas | 505 of 523 functions present, 496 working (94.8%) | GETPIVOTDATA, IMAGE, RTD and some others missing | [STATUS] |
| **Excel external links** | Kept on save, **never refreshed**. Typing a link gives `#REF!` | 17–25 engineer-days (ESTIMATE) | [xlsx-bench], [gridcraft-scaling-and-links] |
| **Excel files over 80 MB** | 0.7.0: aborted above 4 GB. With the spike patches, the 79.2 MB file opens at a **1,496 MB** peak | 14.5–26 engineer-days to production (§2.3) | [xlsx-bench], [gridcraft-templates-spike] |
| Macros | Never executed; the VBA project is kept in .xlsm | Owner must say whether macros are in use | [STATUS] |
| Windows 10 | All four apps build, run and pass the CLI smoke tests on this PC | — | [inventory] |
| **Windows 11** | **UNVERIFIED** (no machine) | Owner to provide a PC or VM | [STATUS] |
| Default file handler, one installer | The upstream MSIs register only `OpenWithProgids`; there is one MSI per app | New work (§2.5) | [STATUS] |
| Android editing | A-web passes the keyboard, scroll, lifecycle and HRM-save tests after 5 patches, **for Docs only** | Mobile UI, handles; Sheets/Slides/PDF untested | [wordcraft-patches], [mobile-b] |
| iOS | **UNVERIFIED** (no macOS) | — | [STATUS] |
| HRM embed | Spike: JWT config → fetch → edit → callback, 3/3 green on desktop and green on Android Chrome | Pilot on hr-backend | [docserver-results], [wordcraft-patches] |
| HRM SSO and sync | Designed; the hr-backend agent prompt is written | Nothing implemented | [hrm-integration-design], [hr-backend-prompt] |

---

## 2. Recommended architecture

### 2.1 Repository and upstream strategy

#### 2.1.1 Evidence

| Finding | Number | Source |
|---|---|---|
| **"Member of the wrong workspace."** An upstream crate cannot join our root workspace while its app's root `Cargo.toml` declares `[workspace]`. The merged spike only resolved after **deleting the four app root manifests and lockfiles** in a copy. | 4 root manifests to delete and carry on every sync | [workspace-conflicts] §1, §6 |
| **Two incompatible hayro forks.** pdfcraft `vendor/` and craft-libs `vendor/` differ in hayro (2 files), hayro-syntax (15) and hayro-interpret (10). A merged `[patch]` lets only one win, and wordcraft and deckcraft would then silently use it. | 2 forks, 27 differing files | [workspace-conflicts] §3.4 |
| pdfcraft's `[patch]` (winit, egui-wgpu, vello_common, lopdf) would apply to **all four apps** in a merged workspace | 4 patched crates | [workspace-conflicts] §3.4 |
| **craft-libs is pinned at 5 revisions:** `d27646a` (wordcraft, gridcraft), `c348732` (deckcraft), and `5c874d5` / `666c358` / `f58bc1a` (pdfcraft) | 5 revs | [layer-map-pdfcraft-craftlibs] |
| **Duplicate crate names.** Merging removes almost none: 32 in the merged lock against 33 for the union of the four apps (wordcraft 15, gridcraft 7, deckcraft 17, pdfcraft 27). | 32 vs 33 | [workspace-conflicts] §1, §5 |
| A forced single `version`: pdfcraft is 0.8.0, the others 0.7.0, and pdfcraft's path deps require `^0.8.0`. One lint set and one profile set (codegen-units 1 vs 16). | — | [workspace-conflicts] §3 |
| The merged workspace **was never compiled**, so API breakage after unifying versions is unknown | 97 members, 923 lock entries | [workspace-conflicts] §1 |
| **A thin workspace of ours that path-depends on an unmodified upstream app builds and runs.** The B binding crate (own workspace, path deps into wordcraft, `Cargo.lock` seeded from wordcraft's) built to wasm and passed its tests. The memory probe (path deps into 5 gridcraft crates) built in 1 min 53 s. | 2 working examples | [mobile-b] B1, [gridcraft-scaling-and-links] §0 |
| Each upstream root uses `members = ["crates/*", "apps/*", "xtask"]`. Only deckcraft (1 entry) and pdfcraft (8 entries) carry a `[patch.crates-io]`. | Read for this ADR from `upstream/*/Cargo.toml` | — |
| Upstream app glue is not thin: the desktop `apps/<app>/src` is 2,886 / 1,768 / 1,622 / 3,138 lines and the web `apps/<app>-web/src` is 719 / 603 / 222 / 251 lines (wordcraft / gridcraft / deckcraft / pdfcraft) | Counted for this ADR with `wc -l` | — |

#### 2.1.2 Options

| Option | What it is | For | Against | Verdict |
|---|---|---|---|---|
| **W1. One merged workspace** (the brief's wording) | Delete the 4 app roots. One root `Cargo.toml` with merged `[workspace.dependencies]`, one `[patch]` set, one profile set, one lockfile. | One lock, one `cargo-deny`, one `cargo update`. Third-party versions are already nearly identical (egui, wgpu, winit, serde, zip, image at the same locked versions everywhere, [workspace-conflicts]). | We **carry the deletion of 4 root manifests** and re-derive one root from four roots that change about 100–140 times a day each. We **pick one hayro fork for all apps** and push pdfcraft's patched winit and egui-wgpu into wordcraft, gridcraft and deckcraft. We ship a dependency graph **no upstream builds or tests**. Upstream's `cargo xtask ci` no longer runs as written. The dedupe gain is 32 vs 33 names. Still uncompiled. | **Rejected for now.** Revisit when upstream converges (one craft-libs rev, one hayro fork). |
| **W2. Build domains (recommended)** | Upstream trees stay pristine apart from the patch queue. They keep their own root, lock, `[patch]` and profiles, so upstream CI runs unchanged. **Our crates live in five small workspaces:** `domains/{docs,sheets,slides,pdf}` path-depend on one upstream app each, and `services/` holds the server-side crates. Each domain seeds its `Cargo.lock` from the upstream lock and copies the upstream `[patch]` and `[profile]` blocks (Cargo applies those only from the workspace being built, so they must be copied; this is documented Cargo behaviour, not exercised for pdfcraft). | Each editor builds exactly as its upstream builds and tests it. **The hayro and craft-libs conflicts never meet**, because the domains are separate graphs. A sync touches only the patch queue and a lock refresh. It matches the product shape anyway: one wasm per editor on web (lazy load) and one process per editor on desktop. Proven pattern (bind crate, memory probe). | Five lockfiles, so security updates and `cargo-deny` run ×5 (scriptable). Shared third-party crates are compiled once per domain (CI cache cost). **Not literally "one workspace".** No single binary can link all four engines; this is not needed, see the consequences below. | **Recommended** |
| W3. Build domains, but our crates inside upstream trees | Put our crates under `upstream/<app>/apps/` so the `apps/*` glob makes them members | No lock or `[patch]` copying | Our code inside upstream trees blurs "pristine plus queue" and depends on upstream keeping the glob. Upstream `xtask layers` may reject unknown crates (UNVERIFIED). | Rejected |

**Consequences of W2**
- **Desktop.** Four editor processes, the same model as WINWORD/EXCEL/POWERPNT, plus a small home launcher, in one MSI.
  Shared services (settings, recent files, fonts, spell-check, i18n) are files plus a tiny shared crate, not one
  process.
- **Converter.** Runs the upstream CLIs as **subprocesses**, one job per process. They exist and pass smoke tests
  ([inventory]). This also contains engine crashes and gridcraft's multi-GB peaks ([xlsx-bench]).
- **Document server.** Serves the per-editor dists and links no engine. Phase 9 co-editing may later link an engine
  into a server. That would again be one domain per editor.
- **Brand and app glue.** The upstream app glue is 1.6k–3.1k lines per desktop app (counted above), so **we do not copy
  it.** Branding goes in through small carried patches or upstream product-config hooks. One already exists:
  `show_discord: bool`, read for this ADR at `wordcraft/crates/ui-egui/src/lib.rs:259`.
- **Separate target directories per domain.** Two worktrees of one repo shared artefact slots in one target directory
  and corrupted a baseline build ([gridcraft-templates-spike] §9).

**Shared-crate dedupe** (BRIEF §5):
- **craft-fonts.** Already consumed once through `CRAFT_FONTS_DIR` ([layer-map-pdfcraft-craftlibs]). One copy serves all
  domains.
- **craft-libs.** Pointing all domains at one rev is an **untested combination**, because nothing in the workspace spike
  compiled. It needs a Phase 1 compile-and-test experiment. Until that passes, each domain keeps upstream's pinned
  rev, fetched from one local mirror.
- **Byte-identical fonts.** Inter ×3 and JetBrainsMono, 1,518,908 B, are duplicated between wordcraft and pdfcraft
  ([layer-map-pdfcraft-craftlibs]). They stay duplicated: removing them needs upstream `include_bytes!` edits for
  little gain.

**If the owner insists on one workspace:** W1 is feasible at resolve level ([workspace-conflicts] §1). It first needs
a Phase 1 compile and a full test of the merged graph. Release test builds took up to 2.2 h per app on this PC
([inventory]). It also needs explicit owner sign-off on using pdfcraft's hayro fork, winit and egui-wgpu in every app.

#### 2.1.3 How upstream is tracked

| Option | For | Against | Verdict |
|---|---|---|---|
| git submodule (pinned) | Small repo; exact SHA | Our patches need a hosted fork, a new remote, which needs the owner's OK ([BRIEF] §6). A clean checkout needs `submodule init` and network access. | Rejected |
| `git subtree` merge | Code in our tree, so builds work offline | At 700–1000 commits a week, our patches dissolve into merge commits. "What is our delta?" then needs archaeology. | Rejected as the primary mechanism |
| **Vendored pristine snapshot + patch queue (recommended)** | `upstream/<repo>/` = upstream at `patches/<repo>/BASE` plus `patches/<repo>/NNNN-*.patch` (`git format-patch`). The queue is the source of truth, and each patch is a ready PR. The spikes already work this way: `git am` of the gridcraft series onto `e6b6b34` reproduced the spike tree exactly ([gridcraft-templates-spike] §2). | Needs a sync script and a CI check that the tree equals BASE plus the queue | **Recommended.** It is a squashed subtree in spirit: pinned, in-tree, offline. |

Size: the five code repos without `.git` or `target` are 83 MB ([spikes/workspace/README.md][ws-readme]), plus about
40 MB of fonts in craft-fonts ([layer-map-pdfcraft-craftlibs]).

**Layout (Phase 1)**
```
upstream/{wordcraft,gridcraft,deckcraft,pdfcraft,craft-libs,craft-fonts}/   BASE + queue, never hand-edited
patches/<repo>/{BASE,NNNN-*.patch}   patches/REGISTER.md
crates/      our engine-free crates: uz-translit, brand, i18n data, host-protocol, fallback fonts
domains/{docs,sheets,slides,pdf}/    one Cargo workspace each; path deps into upstream/<app>/crates/*
services/    Cargo workspace: docserver, converter API, auth client
web/         home shell (static), per-editor dist assembly, size budgets
packaging/   suite MSI, Android wrapper     scripts/sync-upstream     deny.toml
```

### 2.2 UI per platform

| Platform | v1 UI | Engine binding | Evidence | Still open |
|---|---|---|---|---|
| **Windows 10/11 desktop** | Upstream egui/eframe apps, rebranded. Simplified toolbar by default, full ribbon optional. Graphics fall back from DX12 to OpenGL to WARP. | Native | All four build. GUI exe sizes (release zips): 102.4 / 32.6 / 86.5 / 92.8 MB. The exes are MSVC-built and import only system DLLs. ([STATUS], [inventory]) | Windows 11, WARP and RDP performance, the uz/ru Alt+numpad code page (a wordcraft test gives "й" for "é" on this Cyrillic-locale PC, [inventory]) |
| macOS / Linux desktop | Same | Native | Not built here | UNVERIFIED |
| **Web** | egui wasm, one module per editor, lazy-loaded from a small home shell. `?host=parent` bridge to the document server. | wasm | HEAD wasm brotli: **9.4 / 4.7 / 9.3 / 9.4 MB** ([inventory]); release v0.7.0/0.8.0 brotli 8.5 / 4.5 / 5.7 / 8.8 MiB ([STATUS]) | deckcraft compiles 8 codecs into the wasm with no feature gate ([layer-map-deckcraft]) |
| **Android** | **A-web**: the same wasm in the HRM Expo WebView (the embed path), a thin WebView wrapper or a PWA. Wordcraft patches 0001–0005 plus a compact touch toolbar. | wasm | §2.2.1 | Mobile toolbar, handles, real devices, Sheets/Slides/PDF |
| iOS | A-web in WKWebView | wasm | **None** (no macOS) | UNVERIFIED end to end |

#### 2.2.1 Mobile decision matrix

All runs are on the emulator with real Gboard 15.1 ([mobile-a0], [wordcraft-patches], [mobile-a-kill], [mobile-b]).
**Docs (wordcraft) only.**

| Criterion | **A-web** (egui wasm in Chrome/WebView + 5 patches) | A-native (eframe on Android + 170-line shim) | **B** (thin bind crate + web UI) |
|---|---|---|---|
| Keyboard: Latin, Cyrillic `Салом дунё`, Uzbek `oʻgʻil`, Backspace, Enter | PASS | PASS | PASS |
| Autocorrect | PASS with patch 0003 (+37 lines) | PASS (shim) | PASS, no patch |
| Glide typing | UNVERIFIED | PASS (Latin) | PASS (Latin) |
| One-finger scroll with momentum | PASS with patch 0004 (about 190 non-test lines) | not tested | PASS, native |
| Pinch zoom; rotation | PASS; PASS (but a 60 dp page area in landscape) | not tested | PASS; PASS |
| Long-press selection, native handles | PARTIAL: word selected, no handles | not tested | PARTIAL: word selected, no handles |
| 360 dp width | **FAIL**: the desktop ribbon overflows | not tested | PASS |
| App switch / GPU context loss | PASS for short absences with patch 0005 (host holds the unsaved docx about 4 s after hiding; reload in about 2.7 s) | **Activity-destroy ANR seen twice** | PASS, no code |
| **HRM embed + save on mobile** | **PASS** (status-6 callback, docx verified) | n/a (separate app, no WebView embed) | Not built (no bridge) |
| wasm (brotli) / APK | 9,450,860 B / no APK measured | `.so` 10.27 MB stripped; APK 13.5 MB (probe) | 5,816,742 B; WebView APK 11.2 MB |
| Renderer memory (PSS) | 284 MB after load, 326 MB after edits (Chrome on its own software GL, flags on) | 54–58 MB (probe, **no document**; "Graphics" reads 0 on the emulator) | 148 MB after load; 226 MB at the end of a session (stock Chrome) |
| Ready time, stock Chrome on emulator GL | 6–13 s warm; about 90 s first start after `pm clear` | `am start -W` median 1.64–1.67 s (probe, no document) | 1.1–2.1 s warm; 2.6–3.4 s process-cold; 22.3 s first start after `pm clear` |
| Keystroke to repaint | not measured | not measured | p50 41–46 ms, p95 87–135 ms |
| New code in the spike | 5 patches, +651/−18 (about 200 lines are tests) | about 170-line shim | 281-line bind crate + 440-line page |
| Upstream patches needed | 5 wordcraft patches (all upstreamable) | 0 for the keyboard (shim); destroy-ANR cause not isolated | 0 for the runtime; a composition API later (about 150 lines, UNVERIFIED) |
| UI reuse | **The whole desktop editor**: all 629 commands and the existing dialogs | Same as A-web | 4 buttons today. **The whole phone UI must be built.** |
| Remaining cost | Compact toolbar / mobile layout (**UNSIZED**). Keyboard reappears after a swipe. Finger-drag of pictures and table handles. Caret above the keyboard (not re-tested). | All of A-web's UI work, plus the ANR, the wgpu Vulkan-limit failure and Gradle packaging | **7–11 engineer-months for Docs alone** (ESTIMATE). **Sheets is blocked**: gridcraft has no egui-free grid renderer ([layer-map-gridcraft]). |
| One codebase across desktop, web and mobile | Yes | Yes (but a separate APK) | No: two UIs forever unless desktop moves too |

**Recommendation, in phases**
1. **v1 (Phase 6): A-web.** It is the only option that:
   - (a) keeps the complete editors on every platform;
   - (b) already passes the real HRM embed path on mobile, which is a WebView inside the Expo app
     ([hrm-current-state] §4.4);
   - (c) needs only small, upstreamable patches for what was measured.

   The compact toolbar is needed on the desktop anyway ([BRIEF] §4.3 simplified toolbar), so one effort serves both
   (reasoning).
2. **Keep B's binding layer alive** (281 lines, zero upstream patches): compile it and run its 2 tests in CI, so the
   option stays cheap. B wins on memory (148 vs 284–326 MB), wasm size (5.8 vs 9.45 MB brotli), load time on stock
   Chrome, 360 dp fit and lifecycle robustness.
3. **Re-decide at the Phase 6 gate on a real mid-range phone.** If A-web fails a touch-checklist item that B passes, or
   if it is unusable on memory or GPU-context loss, fund B **for Docs only** as the phone UI, at 7–11 engineer-months
   (ESTIMATE).
4. **A-native is rejected for v1.** It has A-web's UI gaps plus the Activity-destroy ANR, it fails on the default
   Vulkan backend (`max_uniform_buffer_binding_size`), and it has no WebView embed path. Its only edge (54–58 MB PSS)
   was measured **without a document** ([mobile-a-kill]).

**Caveats**
- All mobile evidence is emulator plus software GL.
- The tests covered **wordcraft only**. The same matrix must be repeated for Sheets, Slides and PDF before they are
  committed.
- The egui-wgpu 8192 texture request (patch 0002) probably affects the other three web apps too: they use the same
  egui-wgpu 0.36.2 (UNVERIFIED per app).

### 2.3 Excel engine (the MS Office replacement hinge)

**Round 1 numbers: final** ([xlsx-bench], [gridcraft-templates-spike])

| File (on disk) | gridcraft 0.7.0 peak | Patched peak (spike 0001–0007) | Excel 16 32-bit peak | Patched / Excel (PROJECTION: ratio) |
|---|---|---|---|---|
| w10 (9.9 MB) | 635 MB | 194 MB | 220 MB | 0.9× |
| w40 (39.6 MB) | 2,525 MB | 751 MB | 515 MB | 1.5× |
| w80 (79.2 MB) | aborted above 4 GB | **1,496 MB** (re-check 1,505 MB, 73.7 s, idle machine) | 916 MB | 1.6× |
| w120 (118.8 MB) | aborted above 4 GB | 2,421 MB | 1,298 MB | 1.9× |

- **RAM per file-MB:** 64 before, 19–20 after; Excel about 11 ([STATUS]).
- **Save peaks** (tpl3): w80 1,833 MB / 102.5 s; w120 2,635 MB / 157.2 s ([gridcraft-templates-spike] §5). Excel saves
  w80 in 16.5 s after a 57.8 s open ([xlsx-bench]).
- **Open time** improves only modestly: w10 11.3 → 8.7 s, w40 42.7 → 36.6 s ([gridcraft-templates-spike]).
- **Correctness:**
  - bit-identical values on w10, c10 and w40;
  - 5.18M row values on w120 match Python;
  - a 35-step edit script gives the same results;
  - Excel 16 reads the same formulas and values in every cell;
  - 25 new tests;
  - 300 fuzz seeds give the same outcomes as base ([gridcraft-templates-spike] §3–4).
- **Size wall.** `MAX_TOTAL` = 1 GiB of decompressed parts blocks files above about **150 MB on disk** whatever the RAM
  (PROJECTION, [gridcraft-scaling-and-links] §1.5).

**Work to production** (engineer-days; ESTIMATES from the spike reports)

| Step | Change | Days | Source |
|---|---|---|---|
| 0 | Census of the owner's real workbooks; raise `MAX_TOTAL` | 1–1.5 | [STATUS] fix plan |
| 1–2 | Shared formula templates + run-based graph (spike done; hardening, Excel checks at scale, fmt/clippy, soak) | **5–9** (+1–2 optional "parse once" for faster open) | [gridcraft-templates-spike] §8 |
| 3 | Streaming writer (W) + bounded undo (U1) | 3.5–6.5 | [STATUS] fix plan |
| 4 | Compact recalc state (G), or trust cached values (F) | 2–4 | [STATUS] fix plan |
| 5 | Streaming zip (C), removes the 1 GiB wall | 3–5 | [STATUS] fix plan |
| | **Memory subtotal** | **14.5–26** | sum |
| L | External links: syntax, model, I/O, calc, commands, consent/security, verification | **17–25** | [gridcraft-scaling-and-links] §3.2 |
| | **Total** | **31.5–51 (≈6.3–10.2 engineer-weeks)** | sum; plus upstream review latency (unknown) |

- **Links have to be an upstream engine change.** The lexer, parser, AST, evaluator, graph and externalLink I/O have no
  extension points. Only the `LinkResolver`, the consent prompts, the trusted locations and the Edit Links dialog can
  be ours ([gridcraft-scaling-and-links] §3.2).
- **Cutover rule.** Keep Excel per file for (a) linked workbooks until L lands, and (b) files whose census RAM forecast
  exceeds the target PC's budget. The fleet RAM is unknown (B-12). gridcraft 0.7.0 already handles about 40 MB without
  links ([gridcraft-scaling-and-links] §4).

#### 2.3.1 gridcraft memory, round 2 (pending)

<!-- R2: fill from docs/phase0/gridcraft-memory-r2.md -->

> **Placeholder.** Round 2 is running in another agent, and its report will be `docs/phase0/gridcraft-memory-r2.md`.
> The round 1 numbers above are final. Round 2 may refine steps 3–5, the breakdown of the remaining memory, and the
> GUI and save peaks. Until it lands, the totals in this section stand as written.

### 2.4 Document server and HRM integration

**Shape.** It is built from the spike ([docserver-results], [wordcraft-patches], [hrm-integration-design]):

```
HRM SPA / HRM Expo WebView ──GET editor-config (HS256, DocsAPI shape)──▶ hr-backend
        │ loads {editorUrl}/web-apps/apps/api/documents/api.js  (our api.js alias)
        ▼
[redacted: HRM security detail]
        │ iframe: per-editor wasm dist, ?host=parent bridge (open / save / dirty / requestSave / autosave)
        ▼
save → new version on disk → POST callbackUrl {key,status 6|2|4,url,users,token} + Authorization: Bearer
                                   ▼
hr-backend: verify (per-provider secret) → download via provider allowlist → DocumentStore.commit (new version)
Standalone apps (desktop / mobile / web BFF) ──OAuth 2.1 + PKCE──▶ hr-backend /office/* (ETag / If-Match / lease)
```

**Measured in the spike**

| Run | Ready time | Save → callback | Source |
|---|---|---|---|
| Shim adapter, stock v0.7.0 | 4.9 / 1.9 / 2.2 s | 0.33 / 0.45 / 0.15 s | [docserver-results] |
| Bridge adapter, patched dist (3 runs, 8/8 tests each) | 1,470–1,780 ms | 200–290 ms; requestSave → callback 330–360 ms; `/close` 110–1040 ms | [wordcraft-patches] |
| Negative tests | Tampered token → 401; SSRF to 169.254.169.254 → 403; wrong callback secret → file not accepted, error surfaced | | [docserver-results] |
| Android Chrome, HRM flow | Typed text saved by touch through File › Save, docx verified | about 1.1 s including adb tap latency | [mobile-a0] |

**Integration target: `hr-backend` (HRB).**
- It already embeds ONLYOFFICE 9.3.1 for files, decrees, letters and reports ([hrm-current-state] §4).
- Our server is a drop-in behind a flag, and ONLYOFFICE stays the instant fallback. The order of HRM-side work is set
  in [hr-backend-prompt] §4:

| Order | HRM-side PR | Why first |
|---|---|---|
[redacted: HRM security detail]
| PR-3 | Craft provider: `editor=auto` opt-in, pilot users, `docx` only, per-provider secret, allowlist and internal URL | Without the per-provider map, a Craft save URL is rewritten to `onlyoffice-docs` and lost ([hr-backend-prompt] §5.2) |
| PR-4a/b, PR-5 | Versioned store (`document_versions`, conflict versions), editor-session registry, leases | Today saves overwrite in place, with no version and no ETag ([hrm-current-state] §4.3) |
| PR-6, PR-7, PR-8 | OAuth 2.1 + PKCE (desktop loopback, mobile app links, web BFF), a 15-minute `aud`-scoped token, the `/office/*` sync API, hardening | Needed for the standalone apps (D3) |

**Our side (Phase 5 backlog, from the spike and [hr-backend-prompt] §5)**
- api.js alias at `/web-apps/apps/api/documents/api.js`.
- Callback statuses 2/4 (spike) plus 1.
- Configs without `callbackUrl` (view-only).
- An `onError` shape of `{errorCode, errorDescription}`, plus `onAppReady` and `onReady`.
- Raise our callback timeout above the current 10 s.
- **Bookmark round trip of `_hrm_*`.** It survived 4/4 synthetic Word files ([fidelity-baseline]). It still needs a
  test on a real HRM-generated decree.
- Async-save tickets, so a failed host upload leaves the document dirty. Today it shows as saved
  ([wordcraft-patches]).
- A periodic autosave while dirty.
- A per-request session token instead of the UUID capability URL; session ids kept out of logs ([STATUS]).
- DNS/IP checks on SSRF.
- Bridges for Sheets and PDF: gridcraft and pdfcraft already have `?host=parent`; deckcraft has none
  ([layer-map-pdfcraft-craftlibs]).
- The desktop OAuth client: loopback listener, system browser, `keyring` for Windows Credential Manager
  ([hrm-integration-design] §1.5).

**Co-editing is not a regression risk for the pilot.** HRB issues a new random key on every open, so today two users
get separate ONLYOFFICE sessions and the later save wins ([hrm-current-state] §5.6; UNVERIFIED in practice). HRM mobile
is also forced to view-only today unless `ONLYOFFICE_MOBILE_EDIT` is set ([hrm-current-state] §4.2).

### 2.5 Windows desktop specifics (new work found)

| Item | Evidence | Plan |
|---|---|---|
| One suite installer, default associations | The upstream MSI registers only `OpenWithProgids` (`wordcraft/packaging/windows/wordcraft.wxs:79-98`, [STATUS]). The in-app "make default" opens Settings › Default apps, because Windows lets the user choose, not the app (read for this ADR: `wordcraft/apps/wordcraft/src/default_apps.rs:1-14`). | One suite MSI (WiX, as upstream) with our own ProgIDs and UpgradeCode. **Mass default = a GPO / DISM "default app associations" XML** for domain PCs. Manual PCs use Settings. UNSIZED. |
| Footprint | Release exes: GUI 314.3 MB, CLI 224.1 MB (PROJECTION: sum of the four of each) | Decide whether the CLIs ship (the converter needs them server-side) |
| **U+02BB `ʻ` (oʻ, gʻ)** | Gboard's Uzbek keys emit U+02BB ([mobile-a0]). egui's default UI font (Ubuntu Light) has no glyph for it, so it shows as tofu ([mobile-a-kill]). The gridcraft PDF default face also lacks it ([gridcraft-pdf-fonts]). wordcraft's bundled document fonts draw it ([mobile-b]). | Phase 1: bundle a permissively licensed UI and document fallback font covering U+02BB, U+02BC and Cyrillic, and register it in egui and in `pdf_fonts::register_font`. Calibri, Arial and Segoe cover it but cannot be bundled. Test on every platform. |
| Locale decimal separator | On this ru-locale Windows, Excel shows `3500000,0` and gridcraft `3500000.0` ([STATUS]) | Phase 1 investigation (numfmt/display; likely upstream). UNSIZED. |
| Old PCs, RDP | All apps fall back from DX12 to GL to WARP (code, [STATUS]) | Measure WARP and RDP in Phase 4. UNVERIFIED. |
| Toolchain | Releases are MSVC-built ([STATUS]); Phase 0 built with `windows-gnu` ([inventory]) | Build shipping artefacts on an MSVC runner |

### 2.6 Upstream patch strategy

**Policy**
- Every change to an upstream tree is a numbered patch in `patches/<repo>/`, with a register entry: why it exists,
  upstream PR link and status, and a "drop when" condition.
- **PR first. Carry only if the PR is rejected or slow. Never rewrite.**
- Sending a PR needs the owner's OK and a GitHub identity (B-6).

| Patch | Size | Measured effect | Plan | Carry risk if rejected |
|---|---|---|---|---|
| **wordcraft 0001** host bridge (`?host=parent`) | +213/−8 | Bridge spec 8/8 ×3; requestSave, dirty and `/close` work | **PR.** It is the third copy of the gridcraft/pdfcraft pattern, opt-in | Low (new file plus glue) |
| wordcraft 0002 texture limit | +13/−4 | Stock Chrome on 4096-limit GL: start failure → starts | **PR**, plus an issue or PR to egui-wgpu (root cause `egui-wgpu-0.36.2/src/setup.rs:272`) | Low |
| wordcraft 0003 IME DeleteSurrounding | +37 | Gboard `teh ` → `tehthe` before, `the ` after | **PR first, as the upstream-responsiveness probe** | Low |
| wordcraft 0004 one-finger touch scroll | +328/−7 (about 190 non-test) | Swipe scrolls with momentum; tap places the caret; long press selects | **PR**, offered behind a setting. Upstream calls phones out of scope ([BRIEF] §3). | **Medium**: hot file `canvas.rs` |
| wordcraft 0005 lifecycle autosave | +65/−4 | Unsaved docx reaches the host about 4 s after hiding; context-loss recovery about 2.7 s | **PR** with 0001 (opt-in, host mode) | Low |
| **gridcraft 0001–0008** shared formula templates + run graph | 39 files, +1,667/−132 (production +865/−106) | Peak RAM −69 to −70%; values bit-identical (§2.3) | **PR series.** Lead with the measurements; offer commit 7 (run graph, +465/−11) separately; add upstream milestone ids; first finish fmt/clippy and the Excel check at scale ([gridcraft-templates-spike] §6–7) | **High**: commit 2 touches 27 files; `recalc.rs` +337 |
| **gridcraft pdf-fonts 0001–0003** TrueType subset embedding | 15 files, +1,503/−18 | Excel PDF text similarity 0.415 → 0.754; `?` and `####` gone; wasm +65,891 B raw | **PR in two parts** (0001 pdf crate, 0002 engine wiring; 0003 docs). Add FlateDecode first, because PDFs are 3–10× larger. | Medium |
| winit / android-activity text input | Not written (shim instead) | Stock eframe on Android passes only keyboard show/hide (1 of 8 tests) | **File issues with the evidence**: winit 0.30.13 `android/mod.rs:476-478` drops `TextEvent`; `event.rs:774-803` has no `DeleteSurrounding`; android-activity UTF-16 offsets at `game_activity/mod.rs:723-736, 343-349` ([mobile-a-kill]). No patch while A-native is rejected. | n/a |
| egui-wgpu default limits (texture 8192, uniform buffer 65536) | Not written | Causes the A0 web start failure and the A-native Vulkan start failure | Issue/PR to egui | n/a |
| **Still to write** (identified, UNSIZED unless noted) | | | **gridcraft:** memory steps 0, 3–5 (§2.3), external links 17–25 d, locale decimal separator. **deckcraft:** codec feature gate for wasm; an i18n mechanism (none exists, [layer-map-deckcraft]); a host bridge. **All:** a product-config hook for brand and community chrome (Discord, Share, Zotero seen in the stock UI, [docserver-results]). **Report upstream:** fuzz seeds 138/40 and 79/100 fail in base ([gridcraft-templates-spike]); pdfcraft catalog and test bugs ([inventory]). | PR each |

---

## 3. Updated phase plan

### 3.1 Changes to the brief's order, each with its reason

1. **Start fidelity (Phase 8) in Phase 1, not Phase 3.**
   - Corpus fidelity is the largest unmeasured risk (§6, §7).
   - The harness already exists (`spikes/fidelity/run.py`) and runs with one command once `corpus/` arrives
     ([fidelity-baseline]).
2. **Add a parallel Excel engine track (E) from Phase 1.**
   - It is the longest pole: 31.5–51 engineer-days (§2.3).
   - It depends on upstream review latency, which is unknown.
   - It gates Excel cutover (D2).
3. **The hr-backend security PRs (PR-1, PR-2) start now.** They are HRM-side work, independent of our phases, and
   address an exposure that exists today (§6 risk 8).
4. **Keep 2 → 3 → 4 → 5.**
   - Phase 2 (translit) must precede the Phase 5 pilot: pilot users would otherwise lose the ONLYOFFICE `uz-translit`
     plugin that HRB mounts today ([hr-backend-prompt] §2.7, OPEN-Q1).
   - Phase 5's first pilot is Docs-only (`OFFICE_CRAFT_FILE_TYPES=docx`).
5. **Real-device A-web check early.** Move the cheapest Phase 6 evidence (A-web on one mid-range Android phone) into
   Phase 4 as an evidence task. That lets the Phase 6 decision on B be made before the toolbar work is sunk.
6. **Co-editing stays last** (Phase 9). HRM does not co-edit today (§2.4).

### 3.2 Phases

| Phase | Changes driven by Phase 0 evidence | Estimates (source) | Gate additions |
|---|---|---|---|
| **0 (closing)** | Gridcraft memory round 2 (in progress). Corpus run (blocked on `corpus/`). | Fill §2.3.1 when R2 lands | Owner decisions (Appendix B) |
| **1 Foundation** | (a) Monorepo per §2.1 (W2 + queue). The 16 spike patches go into `patches/` as the first register. (b) **Sync dry run = rebase the 16 patches from their 2026-10-15 bases (`fd0bb2f`, `e6b6b34`) to the current HEAD.** This is the first real measurement of weekly rebase cost. (c) craft-libs single-rev compile-and-test experiment. (d) CI per §4.4. (e) Rebrand per §5.1, including the U+02BB fallback font. (f) i18n hooked into 3 mechanisms (wordcraft TSV, gridcraft Rust tables, pdfcraft `tl!`); **deckcraft has none** ([layer-map-deckcraft]). (g) Diagnose the deckcraft test crash. (h) Fidelity harness on the corpus, plus LibreOffice and x2t references in Docker. (i) Excel census (step 0). | Census + cap 1–1.5 d ([STATUS]). Everything else UNSIZED: sized in the first week of the phase. | Rebase cost reported per patch. Slides i18n: a design plus an upstream PR, or the owner accepts an English-only Slides UI at this gate (B-13). |
| **E Excel engine** (parallel, new) | Steps 1–5, then L (§2.3), sent upstream as PR series | **31.5–51 d** (+1–2 d optional faster open) | w80 opens and saves under the census budget; Excel opens the saved files with no repair prompt; links update. Measured by `xlsx-bench`. |
| **2 Transliteration** | `vendor-notes/` is missing ([STATUS]). HRB's mounted ONLYOFFICE plugin (`deploy/onlyoffice-plugins/`, untracked, [hr-backend-prompt] §1) is a possible seed if the owner agrees. Sheets and Slides commands must handle U+02BB/U+02BC input as emitted by Gboard ([mobile-a0]). | UNSIZED | As in the brief (108 pages in under 2 s, against 62 s for ONLYOFFICE) |
| **3 Converter** | Upstream CLIs as subprocesses. gridcraft PDF needs `run --cmd file.exportPdf` ([inventory]). **.ppt via a LibreOffice headless sidecar** (MPL, separate process) if the owner keeps .ppt (B-8). Warn on .doc/.xls → OOXML saves. | UNSIZED | The fidelity table uses the LibreOffice/x2t references from Phase 1 |
| **4 Shell (web + desktop)** | Home launcher plus 4 editor processes; lazy-loaded wasm per editor. **The simplified toolbar is designed once for desktop and mobile**, with Show/Hide ¶ on it. Suite MSI, own ProgIDs, GPO association XML. WARP/RDP measurement. **Evidence task: A-web on a real Android phone.** | UNSIZED | Bundle ratchet met. Windows 11 run (needs hardware, B-10). |
| **5 Document server + HRM** | §2.4 our-side backlog. Pilot on HRB `files` docx behind PR-3's flag. Requires PR-1/PR-2 merged and secrets rotated (B-9). | UNSIZED (our side). HRM side: 9 PRs (PR-0–PR-8), UNSIZED ([hr-backend-prompt]) | `_hrm_*` bookmarks survive a real HRM decree. ONLYOFFICE flip-back works without a restart. |
| **6 Mobile** | A-web per §2.2: productionise patches 0001–0005, compact toolbar, caret above the keyboard, keyboard after a swipe, finger-drag of objects. Repeat the A0 matrix for Sheets, Slides and PDF. iOS only if a Mac is available. **Re-decide B at the gate.** | A-web UNSIZED. B, if chosen: 7–11 engineer-months for Docs (ESTIMATE, [mobile-b]) | Touch checklist on a real device (brief), including startup, memory and frame times |
| **7 Plugins** | No Phase 0 evidence | UNSIZED | — |
| **8 Fidelity** | Runs from Phase 1. Top known losses on the synthetic set: gridcraft PDF fonts (patch ready); Word layout offset with 5–11% pixel diff; .doc comments and revisions lost; .xls charts and conditional formats lost; .ppt ([fidelity-baseline]) | Continuous | Loss-rate trend at every gate |
| **9 Co-editing** | Unchanged | UNSIZED | — |
| **10 Release** | Signed MSI (MSVC build), Docker images for the docserver, converter and sidecars. The docserver's TLS stack on the target OS is UNVERIFIED ([spike-plans] §4.3). | UNSIZED | — |

**Honest scope note.** Phase 0 sized only the Excel engine track and approach B. Every other phase starts with a
sizing step whose output goes into that phase's gate report.

---

## 4. Upstream sync strategy

### 4.1 Weekly procedure (`scripts/sync-upstream`, per repo)

1. **Fetch** upstream into a cache clone outside the monorepo. Record the new SHA and the commits since `BASE`.
2. **Read** upstream's own change signals: `docs/decisions.md`, `docs/gaps.md`, `docs/roadmap.md`,
   `file-format-parity.md`, and `CLAUDE.md` rule changes. The upstream is agent-driven, and its docs drift (§6 risk 10).
3. **Rebase the queue.** Check out the new SHA on a scratch branch and run `git am -3 patches/<repo>/*.patch`. Record per
   patch: applied clean / fuzz / conflicted / minutes to fix. Regenerate with `git format-patch`.
4. **Refresh the domain locks.** Re-seed `domains/<app>/Cargo.lock` from the new upstream lock and re-copy
   `[patch]`/`[profile]`. Run `cargo-deny`.
5. **Gate on our regression suite** (one domain at a time, §4.4):
   - debug tests;
   - wasm sizes;
   - docserver Playwright e2e;
   - fidelity harness (corpus);
   - `xlsx-bench` w10/w40 peak RAM, which is **deterministic to the MB** ([gridcraft-templates-spike] §5) and so a
     good regression metric.
6. **Materialise or hold.** Copy the tree into `upstream/<repo>/`, bump `BASE`, update `REGISTER.md`, and commit
   `sync <repo> <sha> (+N patches)`. If step 5 fails, the pin stays and an issue is opened. Never ship an unvetted
   HEAD.

### 4.2 Cadence

- **Weekly**, because each repo moves 700–1000 commits a week (D4).
- An extra sync for a security advisory.
- A release tag replaces HEAD as the sync point whenever one is newer than the last sync. The v0.7.0 release was a day
  older than HEAD `fd0bb2f`, and HEAD's wasm was already 7% larger ([STATUS]).

### 4.3 Patch register (`patches/REGISTER.md`)

`id | repo | file(s) | lines | why | measured effect | upstream PR (url, state) | carried since | last rebase (clean/fuzz/conflict, minutes) | drop when`.
It starts with the 16 spike patches in §2.6.

**Rebase cost**
- **UNMEASURED.** This is the first number Phase 1 must produce (§3.2, Phase 1 item (b)).
- Hot spots are already visible: wordcraft `canvas.rs` (0004) and gridcraft `cell.rs`/`recalc.rs` (templates).
- A carried templates series at this velocity is the most expensive carry we could take on (PROJECTION from its
  27-file footprint). That is why it goes upstream first.

**When to send PRs** (after the owner's OK, B-6):
1. Phase 1, week 1: 0003 and 0002. They are small and measure upstream responsiveness.
2. 0001 + 0005.
3. The pdf-fonts series after FlateDecode.
4. The templates series after fmt/clippy and the Excel check at scale.
5. External links: open an issue with the design first ([gridcraft-scaling-and-links] §3.2).

### 4.4 CI design (from the build and test measurements)

| Fact | Number | Source |
|---|---|---|
| First `cargo test --workspace --release` | wordcraft 3784 s; gridcraft 7962 s (cargo "Finished" after 130 min); deckcraft 3213 s; pdfcraft 8810 s (upper bound) | [inventory] |
| Rerun with test binaries cached | wordcraft 205 s; pdfcraft 465 s | [inventory] |
| Cold release GUI / web builds | 323–612 s / 420–897 s (wordcraft, gridcraft, deckcraft; pdfcraft's 2636 / 1341 s are inflated upper bounds) | [inventory] |
| pdfcraft `debug = "line-tables-only"` | Cuts 500–600 MB of test PDBs | [workspace-conflicts] §3.3 |

Design:
- **Per push:** debug-profile tests (upstream `dev` is already opt-level 1, deps opt-level 2), with **sccache**, run per
  crate and filtered by path to the domains that changed. Debug test time is UNMEASURED; Phase 1 measures it.
- **Nightly:** full `--release` tests per app, the fidelity harness, `xlsx-bench` memory, and the mobile e2e on the
  emulator.
- **Quarantine list of environment-dependent failures**, each with an owner:
  - wordcraft: 2 emoji tests, Alt+numpad "é"/"й", 2 "Breaks" label tests;
  - deckcraft: the replace-fonts ribbon test;
  - pdfcraft: 14 failures ([inventory]).
- **The deckcraft `ui-egui` lib test binary runs with `--test-threads=1`** until diagnosed. It crashed in 1 of 8
  multi-threaded runs and in 0 of 4 single-threaded runs ([inventory]).
- **Upstream `cargo xtask ci`** (fmt, clippy, layers, wasm) runs per upstream tree on patched code. rustfmt and clippy
  were not installed in Phase 0 ([gridcraft-templates-spike]).
- **Bundle budget = ratchet on our delta.** Fail if an editor's brotli wasm exceeds the pinned-upstream size by more
  than our accepted delta. Spike deltas: wordcraft +9,263 B brotli ([wordcraft-patches]); gridcraft +8,487 B gzip
  ([gridcraft-pdf-fonts]). Report upstream growth separately as a trend.
- **`cargo-deny` per lockfile** (5 domains + services). Shipping artefacts are built on an MSVC Windows runner.
- **CI hardware.** The 11.8 GB Phase 0 PC was RAM-bound: builds ran at `-j 2/4` behind RAM gates, and fuzz seeds
  exceeded a 2.5 GB cap ([gridcraft-templates-spike]). Size the runners from the Phase 1 measurement.

---

## 5. Checklists

### 5.1 Rename and branding

The ArtCraft names and logos are not open source (`docs/brand/LICENSE-brand.txt` in each app; logo files
`artcraft-*.svg/png` in `docs/brand/`, listed for this ADR).

- [ ] Product name chosen by the owner (B-1), with uz-Latn, uz-Cyrl, ru and en forms.
- [ ] No ArtCraft, WordCraft, GridCraft, DeckCraft or PdfCraft in user-visible strings, window titles, About dialogs,
      installer UI, web `<title>`/favicon, error strings ("WordCraft failed to start", [mobile-a0]) or i18n catalogs.
      A CI grep over UI strings and assets. Crate names (`wordcraft-engine`) are code identifiers and stay.
- [ ] `docs/brand/*` assets and app icons are excluded from our builds; our own icons and colours are added.
- [ ] Community and other chrome removed: Discord button (`ui.show_discord`, `wordcraft/crates/ui-egui/src/lib.rs:259`),
      Share, Zotero tab ([docserver-results]). Check the other three apps for equivalents.
- [ ] Installer: our MSI ProductName, Manufacturer, **UpgradeCode**, ProgIDs, AppUserModelID; the
      `registeredAppMachine` name used by "make default" (`default_apps.rs`).
- [ ] Settings and config directory names, MCP server names, CLI binary names, user agent, update and telemetry or
      crash URLs (check for any; UNVERIFIED that none exist).
- [ ] **The HRM integration identifiers use "craft"**: `client_id` `craft-desktop`/`craft-mobile`/`craft-web`,
      `aud:"craft"`, `CRAFT_*` env vars, `editorProvider:"craft"`, the scheme `uz.uzgidro.craft`
      ([hr-backend-prompt] §3, PR-6). **Rename them to the product name before the hr-backend agent implements PR-3
      and PR-6**, or the owner explicitly accepts them as internal identifiers (B-1).
- [ ] An attribution line ("based on WordCraft … by storytold, MIT OR Apache-2.0") in About and NOTICE. The wording is
      reviewed by the owner, because it is nominative use of a trademark.
- [ ] Android package and app-link domain, and the iOS bundle id (Phase 6). Signing identities (Phase 10).

### 5.2 Licensing

- [ ] **The license of our layer** (B-2). Recommendation: **MIT OR Apache-2.0**, the same as upstream, so patches and
      crates move both ways without relicensing.
- [ ] **Keep every upstream `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md` (pdfcraft also
      `ATTRIBUTION.toml`) and `licenses/` directory** (present in all six repos and listed for this ADR).
      Generate one third-party notice bundle for the MSI, APK and web.
- [ ] **`deny.toml` modelled on pdfcraft's**:
  - allow MIT, MIT-0, Apache-2.0 (+LLVM exception), BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, BSL-1.0, CC0-1.0;
  - exceptions `epaint_default_fonts` (OFL-1.1 + Ubuntu Font Licence) and `webpki-root-certs` (CDLA-Permissive-2.0);
  - git sources only `storytold/craft-libs` and `storytold/winprint.rs`
    ([layer-map-pdfcraft-craftlibs]);
  - advisories on (the docserver hand-rolls HS256 to avoid `rsa` RUSTSEC-2023-0071, [docserver-results]);
  - bans `multiple-versions = "warn"` (32 duplicate names, [workspace-conflicts]);
  - run once per lockfile.
- [ ] **Foxit `.pfb` fonts** (about 0.25 MB; PDFium BSD-style; **attribution required**), duplicated in
      `pdfcraft/vendor` and `craft-libs/vendor` ([layer-map-pdfcraft-craftlibs]).
- [ ] **FOGRA39 test data** (`craft-libs/crates/cms/tests/data/FOGRA39.txt`): non-OSI, redistribute only unmodified and
      with credit, **test-only**. Keep it in source with its NOTICE and exclude it from every shipped artefact.
      The CGATS ICC profile is CC0.
- [ ] **OFL-1.1 fonts:**
  - craft-fonts, about 40 MB (BIZ UD, Shippori Mincho, Noto Sans CJK SC / Arabic / Thai);
  - deckcraft's Carlito, Caladea and Liberation;
  - the bundled UI fonts.

  Ship the OFL text with the fonts. Respect Reserved Font Names if a font is modified. Embedding subsets in documents
  is allowed (gridcraft PDF patch). The new U+02BB fallback font must be OFL or a similar permissive licence.
  Microsoft fonts are never bundled.
- [ ] **GPL/AGPL tools only as sidecars**, called over HTTP or CLI, never linked or vendored:
  - pdf2docx + PyMuPDF for PDF→DOCX (Phase 3);
  - LibreOffice headless (MPL-2.0) for .ppt, if kept;
  - ONLYOFFICE x2t only for reference renders in test, never shipped.
- [ ] **Test-only scaffolds, never shipped:**
  - **agdk-eframe**: `rust-android-examples` has **no LICENSE file**, and the probe's Gradle files are adapted from it
    ([mobile-a-kill]). The B WebView wrapper copied the wrapper and properties from the probe ([mobile-b]). Rewrite any
    Android scaffold from scratch before it ships.
  - **ADBKeyboard** (GPL-2.0): planned as an emulator-only test IME ([spike-plans] §3A.5). The spike reports record
    tap and console injection instead, so do not adopt it beyond test machines.
  - Gboard and Chrome are proprietary and used only in the test environment.
- [ ] Clean-room: ONLYOFFICE was read only as its public API documentation ([spike-plans] §4.2). The api.js path alias
      is interface compatibility, not copied code. Keep it that way.

---

## 6. Top 10 risks

L = likelihood, I = impact (H/M/L).

| # | Risk | L | I | Evidence | Mitigation |
|---|---|---|---|---|---|
| 1 | **Upstream drift** outpaces our patch queue | H | H | 701–993 commits a week per repo; 71–98% of each history is from the last week. wordcraft HEAD wasm is +7% over the release built a day earlier; deckcraft HEAD wasm is 35,120,783 B raw against 25,537,284 B in its v0.7.0 release ([STATUS], [inventory]). Rebase cost unmeasured. | W2 keeps the delta small. Weekly sync with our own regression gates. Measure rebase cost in Phase 1. Upstream patches early. Pin on failure. |
| 2 | **Upstream rejects or ignores our engine patches** | M (no PR sent yet) | H | Templates +1,667/−132 over 39 files; links 17–25 d of engine change; upstream's #175 plan differs (streaming saves about 2%, [gridcraft-scaling-and-links]); "phones out of scope" ([BRIEF] §3) | Small PRs first (0003, 0002) to learn their latency. Lead with measurements. Split series. If rejected: carry and budget the rebase, or keep Excel for heavy files (cutover rule). |
| 3 | **Excel external links** | H (needed) | H | Never refreshed; typed links give `#REF!`; mixed formulas freeze silently (code); not on upstream's roadmap ([xlsx-bench], [gridcraft-scaling-and-links]) | Track E. Issue plus design upstream first. Keep Excel for linked workbooks until it lands. |
| 4 | **Large-file memory** | M | H | Patched peaks 1.5–1.9× Excel at 40–120 MB; save peak w120 2,635 MB; 1 GiB wall at about 150 MB on disk (PROJECTION); files keep growing; fleet RAM unknown ([gridcraft-templates-spike], [gridcraft-scaling-and-links]) | Steps 0–5 (§2.3). Census-based per-file forecast. R2 results (§2.3.1). |
| 5 | **Mobile UI cost** | H | H | A-web: desktop ribbon fails at 360 dp, no handles, the compact toolbar is UNSIZED. B: 7–11 engineer-months for Docs alone (ESTIMATE). Sheets B is blocked by the egui-bound grid ([mobile-a0], [mobile-b], [layer-map-gridcraft]). | One simplified toolbar for desktop and mobile. Real-device evidence in Phase 4. Re-decide B at the Phase 6 gate. |
| 6 | **Single-threaded wasm and WebGL on real phones** | M | H | Emulator only. A-web renderer PSS 284–326 MB; a 4096 texture limit hit on emulator GL; GPU-process crashes seen; B tiles take 20–140 ms on the main thread ([mobile-a0], [mobile-b]) | Patch 0002. Lifecycle autosave (0005). Real mid-range device in Phase 4. B fallback. Wasm threads need cross-origin isolation (untried). |
| 7 | **Fidelity on the real corpus** | M–H | H | Only 10 synthetic Office-16 files. Word pixel diff 6.2% mean; .doc loses comments and revisions; .ppt fails. No LibreOffice/x2t references. Upstream says docx was never tested on a real corpus ([fidelity-baseline], [BRIEF] §3). | Corpus run in Phase 1; nightly harness; `_hrm_*` bookmark gate; user warnings (no silent loss). |
[redacted: HRM security detail]
| 9 | **Unverified target platforms: Windows 11 and iOS** | M | H (Win11 is a primary target) | No Windows 11 machine; no macOS ([STATUS]). B's `interactive-widget` is Chrome-only; hidden-textarea behaviour differs on iOS ([mobile-b]). | The owner provides a Win11 PC or VM (Phase 4 gate) and a Mac or cloud Mac, or explicitly defers iOS (B-10). |
| 10 | **AI-driven upstream quality** | H | M | Stale docs (629 vs 422 commands; "13/5 MB" vs a measured 42.6/15.8 MB wasm); 14 pdfcraft test failures including data bugs; base fuzz failures; an intermittent deckcraft crash; self-reported silent drops of charts, SmartArt and OLE ([layer-map-wordcraft], [inventory], [gridcraft-templates-spike], [BRIEF] §3) | Never take HEAD without our gates. Quarantine list. Report bugs upstream. Our own fidelity and regression suites are the arbiter. |

Other risks tracked but outside the top 10:
- legacy .ppt (B-8);
- U+02BB tofu;
- locale decimal separator;
- deckcraft has no i18n;
- the craft-libs single-rev compile;
- "craft" identifiers in HRM;
- the W2 deviation from the brief;
- CI time;
- docserver TLS on the target OS;
- the A-web soft keyboard reappearing after a swipe ([wordcraft-patches]).

---

## 7. What is still unknown or unmeasured

| Unknown | Why | What it blocks | How it gets measured |
|---|---|---|---|
| **The owner's corpus** | `corpus/` not provided ([STATUS]) | Real fidelity, the bookmark gate, Excel census, Office 2010–365 spread | `python -I spikes/fidelity/run.py --input corpus/ …` ([fidelity-baseline]) once delivered |
| **iOS** | No macOS | Phase 6 iOS | Mac or cloud Mac (B-10) |
| **Windows 11** | No machine | D2 acceptance | Win11 PC or VM |
| **Real Android devices** | Emulator plus software GL only | Every mobile performance and memory figure; GPU-context loss; texture limits | One mid-range phone (Phase 4 evidence task) |
| **LibreOffice / x2t references** | Docker was down during the fidelity run ([fidelity-baseline]) | Phase 3 fidelity table | Hooks exist in `run.py` |
| **Gridcraft memory round 2** | In progress | §2.3 steps 3–5 | §2.3.1 placeholder |
| Rebase cost per weekly sync | No sync done yet | The cost of carrying patches | Phase 1 dry run (16 patches) |
| Upstream PR acceptance and latency | No PR sent | Risk 2 | First small PRs |
| Merged or unified craft-libs compile | Nothing compiled in the workspace spike ([workspace-conflicts]) | Shared-crate dedupe | Phase 1 experiment |
| Debug-profile test time; CI runner size | Only release runs were measured | CI design | Phase 1 |
| Excel silent auto-repair of our saved files | COM opens with alerts off ([fidelity-baseline]) | Excel trust | Interactive Excel check at scale |
| Glide typing in Cyrillic/Uzbek; keyboards that compose (Samsung, SwiftKey, CJK) | Only Gboard; no composition events seen ([mobile-b], [mobile-a-kill]) | Mobile IME confidence | Real devices |
| Mobile behaviour of Sheets, Slides and PDF | Spikes were wordcraft-only | Phase 6 scope | Repeat the A0 matrix |
| Fleet facts: PC count, Windows versions, RAM, GPO/domain, macros, .ppt prevalence | Owner questions open ([STATUS]) | MSI policy, Excel cutover, .ppt work | Owner (B-12) |
| WARP and RDP performance | Not run | Old PCs | Phase 4 |
| The docserver's TLS stack on its deployment OS | Spike ran HTTP only ([spike-plans] §4.3) | Phase 5 deployment | Phase 5 |

---

## Appendix B. Decisions for the owner at the Phase 0 gate

| # | Decision | Options | Recommendation |
|---|---|---|---|
| **B-1** | **Product name** | Owner's choice | Choose it before the HRM agent implements PR-3/PR-6, so the "craft" identifiers can take the new name (§5.1) |
| **B-2** | **License of our layer** | (a) MIT OR Apache-2.0; (b) Apache-2.0 only (explicit patent grant); (c) MIT only. Copyleft (MPL/GPL/AGPL) is excluded by BRIEF §5 "permissive only". | **(a)**: identical to upstream, so patches and crates flow both ways and PRs need no relicensing |
| **B-3** | **Mobile approach** | A-web; A-native; B (7–11 engineer-months for Docs, ESTIMATE) | **A-web for v1**, with B's binding layer kept compiling and re-decided at the Phase 6 gate on real-device data (§2.2.1). Reject A-native for v1. |
| **B-4** | **Priority order** | Any order of co-editing / fidelity / mobile | **Fidelity (including the Excel engine) → mobile → co-editing.** Fidelity decides whether MS Office can be replaced (D2). Mobile editing is a hard requirement (D1). HRM does not co-edit today (§2.4). |
| B-5 | Repository layout | W1 one merged workspace (brief wording); **W2 build domains in one monorepo** | **W2** (§2.1). Revisit W1 when upstream converges on one craft-libs rev and one hayro fork. |
| B-6 | Upstream engagement | Send PRs (needs a GitHub identity and the owner's OK to push) / carry only | **Send PRs**, starting with 0003 and 0002 in Phase 1 week 1 (§4.3) |
| B-7 | Excel scope and cutover | Fund track E (31.5–51 d, PROJECTION) / keep Excel indefinitely for heavy and linked files | **Fund E.** Meanwhile keep Excel per file for linked or over-budget workbooks, gated by the census. |
| B-8 | Legacy .ppt | LibreOffice headless sidecar converter (MPL, separate process) / an upstream importer (large, UNSIZED) / no .ppt | Say how common .ppt is. If any are in use: **sidecar** in Phase 3. |
[redacted: HRM security detail]
| B-10 | Verification hardware | Win11 PC/VM; a mid-range Android phone; Mac or cloud Mac | Provide all three, or explicitly defer iOS |
| B-11 | Default-handler policy on office PCs | Opt-in per user (Settings) / GPO association XML on domain PCs / MSI property | GPO XML where PCs are domain-joined (§2.5) |
| B-12 | Fleet facts | — | PC count, Windows versions, RAM, domain/GPO, VBA macro use, .ppt prevalence ([STATUS]) |
| B-13 | Phase 1 gate scope for Slides i18n | deckcraft has no i18n mechanism ([layer-map-deckcraft]) | Accept "design + upstream PR" for Slides at the Phase 1 gate, rather than a full uz-Latn/uz-Cyrl switch |
| B-14 | Corpus | — | Deliver `corpus/` with files saved by Office 2010, 2013, 2016/2019 and 365, and real linked workbooks. Allow the read-only census script on the large workbooks. |

---

## Sources

[BRIEF]: BRIEF.md
[STATUS]: STATUS.md
[inventory]: phase0/inventory.md
[workspace-conflicts]: phase0/workspace-conflicts.md
[ws-readme]: ../spikes/workspace/README.md
[layer-map-wordcraft]: phase0/layer-map-wordcraft.md
[layer-map-gridcraft]: phase0/layer-map-gridcraft.md
[layer-map-deckcraft]: phase0/layer-map-deckcraft.md
[layer-map-pdfcraft-craftlibs]: phase0/layer-map-pdfcraft-craftlibs.md
[spike-plans]: phase0/spike-plans.md
[docserver-results]: phase0/docserver-results.md
[wordcraft-patches]: phase0/wordcraft-patches.md
[mobile-a0]: phase0/mobile-a0.md
[mobile-a-kill]: phase0/mobile-a-kill.md
[mobile-b]: phase0/mobile-b.md
[xlsx-bench]: phase0/xlsx-bench.md
[gridcraft-scaling-and-links]: phase0/gridcraft-scaling-and-links.md
[gridcraft-templates-spike]: phase0/gridcraft-templates-spike.md
[gridcraft-pdf-fonts]: phase0/gridcraft-pdf-fonts.md
[fidelity-baseline]: phase0/fidelity-baseline.md
[hrm-current-state]: phase0/hrm-current-state.md
[hrm-integration-design]: phase0/hrm-integration-design.md
[hr-backend-prompt]: phase0/hr-backend-prompt.md

| Label | File |
|---|---|
| BRIEF, STATUS | `docs/BRIEF.md`, `docs/STATUS.md` |
| inventory, workspace-conflicts | `docs/phase0/inventory.md`, `docs/phase0/workspace-conflicts.md` (+ `spikes/workspace/README.md`) |
| layer maps | `docs/phase0/layer-map-{wordcraft,gridcraft,deckcraft,pdfcraft-craftlibs}.md` |
| spike-plans, docserver-results, wordcraft-patches | `docs/phase0/…` |
| mobile-a0, mobile-a-kill, mobile-b | `docs/phase0/…` (evidence in `docs/phase0/mobile*/`) |
| xlsx-bench, gridcraft-scaling-and-links, gridcraft-templates-spike, gridcraft-pdf-fonts | `docs/phase0/…` |
| fidelity-baseline | `docs/phase0/fidelity-baseline.md` |
| hrm-current-state, hrm-integration-design, hr-backend-prompt | `docs/phase0/…` (`hrm-backend-prompt.md` is superseded) |

Facts marked "read for this ADR" or "counted for this ADR" were taken on 2026-10-16 from `upstream/` at the Phase 0 HEADs
listed in [STATUS].
