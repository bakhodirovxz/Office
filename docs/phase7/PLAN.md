# Phase 7 (Plugin system): execution plan

> **Owner decisions and requirements in force**
> - **Phase 7 starts early, in parallel** (owner, 2026-11-03: "keep going; when Phase 6 is done, move to Phase 7"). Branch
>   `phase-7` was cut from `phase-6` at `b89c6de`. Fixes flow **phase-4 → phase-5 → phase-6 → phase-7** by merge-forward, never
>   back. Gates stay in order 4 → 5 → 6 → 7. **Implementation starts only on the owner's go for Phase 7**; "starts now" in §12
>   means: needs no further owner input once that go is given.
> - **Licensing:** our layer MIT OR Apache-2.0; linked deps permissive only, checked by `cargo-deny` (`deny.toml`, `ci/deny/*.toml`).
>   The only exception is MPL-2.0 for symphonia (`CLAUDE.md` gate decisions). Plugins we ship follow the same rule.
> - **Thin layers:** new code in our crates; upstream changes only as carried patches with a `patches/REGISTER.md` row; never
>   hand-edit `upstream/`. Product name only from `brand/brand.toml` (also in plugin ids, package names and UI text).
> - **Accounts are optional** (owner, 2026-10-21): plugins work in local mode with no server and no login.
> - **No HRM names in our code** (`services/convert-api/tests/no_hrm_names.rs:39`): HRM field names reach plugin #2 only as
>   operator configuration.
> - **Web:** each editor lazy-loaded, size budgets in `web/budgets.toml` enforced by `ci.sh sizes`; raising a budget above
>   measured + 10 % needs a STATUS line (`web/budgets.toml:19`).
> - **Evidence over assertion:** every speed and size claim below cites a measurement (the spike `spikes/p7-runtime`, or an
>   earlier STATUS/perf line) or is marked as a projection.

| | |
|---|---|
| **Status** | Plan, written 2026-11-03 on branch `phase-7` (from `phase-6` `b89c6de`). One throwaway spike run for it (`spikes/p7-runtime`, §1). No product code. |
| **Inputs** | `docs/BRIEF.md` Phase 7 (:445-474), ONLYOFFICE lessons (:89-92, :328), App. B (:575); `docs/phase1/SIZING.md` §2.6 (:317-337); `docs/ARCHITECTURE-DECISION.md:541`; `docs/STATUS.md` Phase 2 (:965-1011), Phase 6 (:1663-1686); `docs/phase2/perf.md`; `docs/phase4/hooks-docs.md`, `desktop-host.md`; `docs/phase5/PLAN.md` §1, §5; `docs/phase6/PLAN.md`, `a11y-palette.md`, STATUS WP8; `patches/REGISTER.md`; code cited below. |
| **Exit gate (brief)** | (1) **Both plugins run on web, desktop and mobile.** (2) **A malicious-plugin test** (network or filesystem without permission) **is blocked** (`BRIEF.md:470-474`). |
| **Estimate** | **Docs scope: 38 stream-days likely (27–57)**, critical path about 22.5. Sheets + Slides adapters another 8 (6–12) if the owner puts them in v1 (§13.1, O2). |

**Citation note.** `upstream/<repo>` is the pristine snapshot with our queue applied (`docs/UPSTREAM.md:10-11`), so
`upstream/...:N` is the code as built today, carried patches included.

How to use this plan. §0 facts found while planning. §1 the spike and the runtime decision. §2 shape. §3 decisions.
§4 the API v1. §5 sandbox and the malicious-plugin test. §6 plugin #1 (uz-translit). §7 plugin #2. §8 install and admin control.
§9 patches and size budgets. §10 what this PC can and cannot prove. §11 work packages. §12 order. §13 estimate, risks, gate map,
owner decisions.

---

## 0. New facts found while planning

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **Every Docs command is a `CommandSpec` with `&'static str` ids and plain `fn` pointers** (no closures), and the registry is rebuilt by replacing `Session.registry` (an `Arc<Registry>`). | `upstream/wordcraft/crates/engine/src/lib.rs:61-74`; `domains/docs/crates/docs-translit/src/lib.rs:55-62` | Plugin commands cannot capture their plugin. They need fixed **slots** (`fn run<const N: usize>`), the pattern `docs-office` already uses for its guarded commands (`domains/docs/crates/docs-office/src/hooks.rs:55-77`), plus leaked id/label strings (bounded by the slot count). |
| F2 | **The registry already describes itself for agents**: `params` is a parameter description "for agents", `describe()` returns id, label, location, shortcut, params, mutates; `wordcraft-cli mcp` serves it as `engine.commands`. | `lib.rs:68-69`, `:124-131`; `upstream/wordcraft/crates/mcp/src/lib.rs:40-60` | The plugin command API is this catalog (629 ids, `STATUS.md:97`); `docs/plugins.md` links a generated list instead of retyping it. |
| F3 | **Read commands exist but are capped**: `document.inspect` (body only, first 2,000 blocks, 50 runs a paragraph), `document.text`, `document.paragraph`, `document.selection`. | `upstream/wordcraft/crates/engine/src/cmd/file.rs:152-166`, `:421-442` | Fine for small reads (plugin #2). Bulk text (plugin #1) needs its own read: the **text map** (§4.4). |
| F4 | **The session has what one-undo-step plugin calls need**: `join_next_undo`, `edit_snapshot`/`restore` (rollback), and a document revision `rev()`. Every `Session::run` applies `enabled`, Restrict Editing (`protect::check_before`) and the read-only open of 0127. | `upstream/wordcraft/crates/engine/src/session.rs:997`, `:1189`, `:1282`, `:760`, `:1326-1338` | A plugin's edits are one undo step, roll back on error, and a view-only session refuses them **by construction** (all actions go through `Session::run`). The web path checks `rev()` to refuse stale results. |
| F5 | **The built-in transliteration is a walker, not just the converter**: it classifies each byte (outside the selection, U+FFFC objects, tracked deletions, skipped languages, locked content controls, field results), calls `uz_translit::convert_runs` per paragraph, rebuilds `Paragraph.text` once keeping every `Run.props`, and has a tracked-changes path (2 revisions). | `domains/docs/crates/docs-translit/src/lib.rs:157-368` (`run`, `story_pass` from :250), `:372-440` (`classify`), `:476-534` (`apply`) | "Identical output" (brief :463) is achievable only if the plugin gets **the same segments** and the host applies the answer with **the same `apply`**. The walker moves into a shared module; only the converter moves into the plugin (§6). |
| F6 | **Built-in speed on the 108-page files**: command 47-67 ms (convert 41-59, apply 4-6), command → finished relayout **0.35-0.44 s** (N1); desktop next frame 0.40-0.42 s, painted **0.93-0.94 s** (N2); web palette → presented frame **1.26-1.27 s** (N3). ONLYOFFICE plugin: 62 s. | `docs/STATUS.md:971`, `:976`, `:1005`; `docs/phase2/perf.md:115`; `docs/BRIEF.md:89-92` | These are the numbers plugin #1 is held to (§6.3). The cost is layout, not the converter: the plugin must not add per-call or per-run overhead. |
| F7 | **The ONLYOFFICE 62 s is API shape**: `ReplaceTextSmart`, hex-encoded payloads, batching to keep the tab responsive. | `docs/BRIEF.md:89-92` | Our API has one bulk read, one call, one bulk apply = one undo step and one relayout (§6.2). |
| F8 | **The Docs simple ribbon is a Customize-Ribbon preset** with groups of command ids (Transliterate is one group), rewritten on a language change; custom buttons draw icons through an icon hook. | `domains/docs/crates/docs-office/src/preset.rs:1-32`, `:94`; `patches/REGISTER.md:83` (wordcraft/0122); `upstream/wordcraft/crates/ui-egui/src/office_overlay.rs:112-117`, `:252` | A plugin toolbar button is a registry command placed in a preset group: **no new ribbon patch**. Icons come from our existing icon set by name. |
| F9 | **The editor ↔ page bridge already runs commands for a page** (`wordcraft:commands`, `wordcraft:run {id}` → `WordApp::run`, `wordcraft:ran`), same-origin and parent-only. | `upstream/wordcraft/apps/wordcraft-web/src/web/host.rs:1-45`; `patches/REGISTER.md:28` (wordcraft/0141); `docs/phase6/a11y-palette.md` | The web plugin host lives in the **page** (shell, docserver wrapper, Android page), and the editor gains a few bridge messages next to 0141's (§2, §9). |
| F10 | **All three web containers are our pages**: the shell frames editors with `?host=parent` (`web/shell/shell.js:110`); the docserver wrapper `/ds/editor` frames `/docs/?host=parent` (`docs/phase5/PLAN.md:76-81`); the Android app serves the shell from APK assets (`STATUS.md:1680`). The desktop in-window editors are the same shell in WebView2 (`docs/phase4/desktop-host.md:3`). | as cited | One JS page host covers web, docserver, Android and the desktop in-window editors. Native editor windows need the native host. |
| F11 | **The docserver has no "organisation" object: it has providers** (`[[docserver.providers]]`: issuer, secret, fetch/callback allow lists, frame origins), and is configured by file ("configuration, never code"). | `services/docserver/src/config.rs:26-52`; `docs/phase5/PLAN.md:90-91` | "Per organisation" = per provider. Admin control in v1 = provider config entries (§8); an admin web UI is an owner decision (O7). |
| F12 | **Budgets now**: Docs wasm 9,879,738 B brotli, headroom **24,678 B**; shell 61,589 of 65,536 B, headroom **3,947 B** (after Phase 6 WP6 follow-up). | `docs/STATUS.md:1686`; `web/budgets.toml:34`, `:86-87` | No plugin host JS in the shell sources: a separate lazy chunk with its own budget row; the editor-side code must be measured (§9). |
| F13 | **The Android app has no INTERNET permission** (local mode only). | `packaging/android/AndroidManifest.xml:3` | A plugin network grant cannot work in the Android local app; the denial path is the only path there. |
| F14 | **Content controls and bookmarks are catalog commands with parameters**: `developer.plainText {title?, tag?}`, `developer.properties {placeholder?, lockDelete?, ...}`, `insert.bookmark {name}`, `insert.field {instr, result?}`. | `upstream/wordcraft/crates/engine/src/cmd/controls.rs:24-29`, `:58-59`; `cmd/insert.rs:49`, `:101` | Plugin #2 needs no new engine command: it drives existing ones through the API. |
| F15 | **Some catalog commands touch the filesystem by parameter** (`file.saveAs {path}`, `insert.textFromFile {path}`, `mailings.finish {path}` among them) or run macros (`tools.macros`). | `docs/phase4/hooks-docs.md` (bypass list, `mailings.finish {path}`); `session.rs:1341` (macro ids) | "Run any catalog command" would be a filesystem hole. The plugin API runs only an **allowlist** of command ids and refuses path/url parameters (§5.2). |
| F16 | `zip 3.0.0` is in the Docs graph with RUSTSEC-2026-0367 ignored because we never call `extract()`. | `ci/deny/docs.toml:50-51` | Plugin packages are read entry by entry in memory, never extracted to disk (the same rule). |

---

## 1. The spike and the runtime decision

`spikes/p7-runtime/README.md` has the code, the commands and the raw numbers. Summary (2026-11-03, this PC, release, gnu):
uz-translit (`crates/uz-translit`, unchanged) compiled to a 108,282 B core wasm module with no imports, called on a batch
of 2,577 paragraphs / 12,555 runs / 334,029 B (the size of the Phase 2 108-page files, `STATUS.md:971`).

| Path | Batch time | vs native | Output |
|---|---|---|---|
| native `convert_runs` (Phase 2 measured "convert 41-59 ms", `STATUS.md:976`) | 40.5-41.1 ms | 1.0x | reference |
| **wasmtime 50.0.1** (Cranelift), binary batch | **65.5-73.3 ms** | 1.6-1.8x | identical |
| wasmtime, JSON batch | 104.0-106.9 ms | 2.6x | identical |
| **wasmi 2.0.0** (interpreter), binary batch | **474.9-645.8 ms** | 12-16x | identical |
| **Chromium 141**, dedicated Worker round trip | **44.8-61.5 ms** | 1.1-1.5x | identical |

| Cost | wasmtime | wasmi | Browser |
|---|---|---|---|
| Exe size (stripped) | **+12.4 MB raw, +4.2 MB gzip, +2.9 MB xz** | +2.3 MB raw, +0.8 MB gzip | 0 (built in) |
| Start-up | Cranelift compile 150-185 ms once per process; instantiate 0.05-0.12 ms | translate 1.0-1.3 ms | compile 1.8-2.3 ms; Worker start 7-23 ms |
| Runaway stop | epoch deadline: stopped after 50-51 ms | fuel (not timed) | `Worker.terminate()`; page stayed responsive |
| Memory cap | `StoreLimits`: hog stopped at 64 MiB | (not tested) | **only with an imported memory**: exported memory reached **4.28 GB** in a Worker; imported with `maximum` 1024 pages stopped at 51 MB |
| `cargo-deny` (root `deny.toml`, unchanged) | bans, licenses, sources, advisories **ok** (104 crates; `Apache-2.0 WITH LLVM-exception` is on the list, `deny.toml:17`) | ok (21 crates) | n/a |

For scale: release `docs.exe` is 87,830,277 B and the MSI 211,734,528 B (`spikes/p7-runtime/README.md`).

### 1.1 Decision D1: runtime per platform

| Platform | Runtime | Why |
|---|---|---|
| Desktop native editor windows | **wasmtime** (pinned, `default-features = false`, features `cranelift`, `runtime`, `std`; no WASI, no component model) | wasmi fails "same speed": 12-16x native would add about 0.45-0.6 s to a command whose whole relayout is 0.35-0.44 s (F6). wasmtime adds 25-33 ms. The +12.4 MB is paid by the editors that host plugins: Docs only in v1 (O2), so about +3-4 MB in the compressed MSI (xz/gzip figures above; MSI compression not measured). Cranelift compile happens on a background thread at first use, never on the UI thread. No on-disk precompiled cache in v1 (`Module::deserialize` is `unsafe` on trusted bytes; the 150-185 ms does not justify it). |
| Web (shell, docserver wrapper, desktop in-window) | **the browser's WebAssembly in a dedicated Worker** per plugin | Same speed as native in V8 (1.1-1.5x); the Worker gives a kill switch (`terminate`) that the main thread cannot have, and its script runs under its own CSP. The hop costs under 1 ms for 0.4 MB (Worker time = main-thread time). |
| Android | **the web path inside the WebView** (the app's page is the shell) | The same code; no second native runtime. Not run on the emulator here (§10). |

wasmi stays the fallback if wasmtime ever fails the licence, MSVC or advisory check (it passed all three that were run). The
four-app case (O2 = all) puts wasmtime in four exes (+12.4 MB each raw); the fix then is one shared plugin-host DLL (WP2 note),
not a different runtime.

### 1.2 Decision D2: one plugin binary for all platforms

**A core wasm module for `wasm32-unknown-unknown` with our own small ABI** (§4.2), not the component model.
- Browsers run core modules only; a component needs a transpiler (jco) and JS glue per plugin, which the web budget and the
  "same binary everywhere" rule both reject. wasmi has no component model either (the fallback must stay possible).
- The API is coarse (one call per invocation round with a message, §4.3), so typed WIT interfaces buy little; a binary message
  format we own costs about 150 lines on each side.
- The spike shows the module needs **no imports** for real work; the ABI adds exactly one required import (the host-made memory,
  §5.1), because the browser memory cap only works that way.
- Revisit for API v2 if browsers ship the component model natively.

---

## 2. Shape

```
           plugin package (zip in memory): plugin.toml + plugin.wasm (+ strings)        installed by admin / user (§8)
                                     │
        ┌────────────────────────────┼─────────────────────────────────────────┐
  native editor window          web page host (one JS chunk, lazy)              Android app page
  (docs.exe, egui)              shell │ docserver wrapper │ desktop WebView2      (shell from APK assets)
        │                             │                                          = web page host
  crates/plugin-host (wasmtime)   web/plugins/host.js ── Worker per plugin (web/plugins/worker.js, CSP default-src 'none')
        │                             │  postMessage: wordcraft:plugin* (bridge, §4.6)
        └──────────── domains/docs/crates/docs-plugins ──────────────┘   (the same Rust in docs.exe and in the docs wasm)
                 slot commands in Session.registry · text map read/apply · action executor (allowlist, one undo step)
                 contributions → ribbon preset group, context menu, command palette · events (selection, open, save)
```

- **One Docs adapter for every platform.** `docs-plugins` owns the commands, the text map, the action executor and the
  contributions; only the transport differs: a direct wasmtime call natively, a bridge message to the page on the web.
- **The page host is shared by every web container** (F10) and by all four editors later (app-neutral like `palette.js`).
- **The plugin never runs code that can reach the outside**: natively it has no WASI and only our imports; on the web it runs in
  a Worker whose script is ours and served with `Content-Security-Policy: default-src 'none'`. Network, if granted, is done by
  the host on the plugin's behalf (§5).

---

## 3. Design decisions (options, recommendation, deciding trade-off)

| # | Question | Options | Recommendation | Why |
|---|---|---|---|---|
| D1 | Native runtime | wasmtime / wasmi / Wasmer | **wasmtime** | §1.1: speed (wasmi 12-16x), licence ok; Wasmer adds LLVM/singlepass choices and nothing we need. |
| D2 | Binary format | core module + own ABI / component model + WIT | **core module** | §1.2 |
| D3 | Call model | sync host imports (plugin calls `run` while running) / **effects as data** (plugin returns actions, host runs them, plugin resumed with results) | **Effects as data, in rounds** (§4.3) | Works unchanged in a Worker (no sync call back into the editor, which the web cannot do without `SharedArrayBuffer` and cross-origin isolation the docserver embed cannot have); every action passes one permission check point; no re-entrancy into `&mut Session`. Cost: a plugin that needs many dependent reads takes several rounds (capped at 16). |
| D4 | Bulk text API | catalog commands per paragraph / `document.inspect` / a **text map** (one binary read, one binary apply) | **Text map** (§4.4) | Per-call shapes are the ONLYOFFICE trap (F7); `inspect` is capped (F3); JSON for the batch costs +35-40 ms native (§1). |
| D5 | Web host location | inside the editor wasm (Rust spawns the Worker) / **in the page** (JS) | **In the page** | Keeps the Docs wasm growth to the bridge messages (24,678 B headroom, F12); one host for four editors; DOM panels are accessible (the canvas is not, `a11y-palette.md`). |
| D6 | Plugin UI | plugin HTML/JS in an iframe / **declarative** (buttons, menu items, a panel from a fixed widget list) | **Declarative** | No plugin script in our pages; one description renders in egui (desktop) and DOM (web, Android); text is always text (no markup injection). Rich custom UI is API v2 if needed. |
| D7 | Command ids and buttons | new ribbon patch / **registry slots + preset group** | **Slots + the preset** | F1, F8: no new hot-file patch for buttons. 64 slots per process (the slot count is the cap on plugin commands). |
| D8 | Events | editable from events / **read-only events** | **Events may update the plugin's panel or status only**; document edits come only from a user action (button, menu, panel click) | No silent background edits; an event storm cannot fill the undo stack. |
| D9 | uz-translit after the plugin ships | replace the built-in / **keep the built-in, plugin as the proof and as an option** | **Keep both** (O6) | The HRM pilot depends on the built-in (`docs/phase5/PLAN.md:62`); Sheets and Slides have only the built-in; the plugin must equal it, which is only testable while both exist. |
| D10 | Apps in v1 | Docs only / all four | **Docs for the gate**, API app-neutral (O2) | Both brief plugins are Docs-shaped; Sheets/Slides have closed `OnceLock` registries plus `register_command` (`gridcraft/crates/engine/src/cmd/mod.rs:187`, `deckcraft/.../cmd/mod.rs:210`) and need their own text map and bridge messages (+8 d). PDF has no parameterised command API (`pdfcraft/crates/ui-egui/src/commands.rs:103`). |

---

## 4. API v1

### 4.1 Package and manifest (`plugin.toml`)

A package is a zip read in memory (F16): `plugin.toml`, `plugin.wasm`, optional `strings.tsv` (en, ru, uz-Latn, uz-Cyrl),
optional `SIGNATURE` (O4). Limits: package 8 MiB, wasm 4 MiB, 64 entries.

```toml
id = "uz-translit"            # [a-z0-9-.]{3,64}; unique per install; the product name never appears here (brand rule)
version = "1.0.0"             # semver
api = 1                       # host API major this plugin was built for
apps = ["docs"]
events = ["selectionChanged"] # "documentOpened" | "documentSaved" | "selectionChanged"
publisher = "…"               # free text, shown in the install dialog
name.en = "Transliteration (plugin)"   # also name.ru, name."uz-Latn", name."uz-Cyrl"; missing ones fall back to en

[permissions]
document = "write"            # "none" | "read" | "write"
network = []                  # origins, e.g. ["https://api.example.org"]; granted by admin or user (§5.3)
memory_mb = 32                # ≤ host cap (default 64)

[[command]]
id = "toCyrillic"             # registry id becomes plugin.<plugin id>.<command id>
label.en = "Latin to Cyrillic"
reads = ["textmap"]           # gathered before the first call: "textmap" | "selection" | "none"
params = { scope = "auto" }   # passed in the call context; the same keys as the built-in where it mirrors one

[[toolbar]]                   # also [[menu]] (place = "tools" | "context.text") and [[panel]]
command = "toCyrillic"
icon = "translit"             # a name from our icon set (F8); unknown names show the label only
group.en = "Plugins"

[[panel]]
id = "fields"
title.en = "Template fields"

[settings]                    # defaults; admin config or the user's settings override (§8)
```

Validation (WP1) refuses: unknown keys, an `api` the host does not support, more than 16 commands / 8 toolbar items / 2 panels,
labels over 64 characters, a `memory_mb` over the cap, network entries that are not `https://host[:port]` origins.
WP1 additions after coding the parser (the crate is the reference; `docs/plugins.md` section 2 lists all of it): `events` and
other top-level keys must precede the first table header (as the example now shows; after a `[[panel]]` TOML makes them panel
keys, which the strict parser then refuses); ids, versions, apps, languages (`en` required) and icon names are checked; every
user-visible string refuses control and bidi/invisible characters (label spoofing in the install dialog); `network` origins
must be lowercase DNS names with at least two labels (no IP literal, no `localhost`, no userinfo/path); a command that reads
the document needs `document` of `read` or `write`; `params` and `[settings]` are size-capped (1 KiB, 4 KiB); a package may
hold only the four flat names of §4.1 (`check_package`).

### 4.2 Binary ABI

| Item | Rule |
|---|---|
| Imports | exactly `env.memory` (memory, required; the host creates it with `maximum = memory_mb`) and optionally `plugin_v1.log(ptr, len)` (debug text, rate-limited, never shown to users). **Anything else refuses the load.** The `plugin_v1` namespace is neutral on purpose (the product name lives only in `brand/brand.toml`, so a rename never breaks plugins) and is fixed forever like an opcode. |
| Exports | `api_version() -> i32` (= 1), `alloc(len: i32) -> i32`, `free(ptr: i32, len: i32)`, `call(ptr: i32, len: i32) -> i64` (the answer as `ptr << 32 | len`). No exported memory. |
| Message | `u32 header_len, header (UTF-8 JSON), u32 blob_count, per blob: u32 len, bytes`. JSON for small structured data; blobs for the text map. |
| Limits per round | input and output 32 MiB each, 16 rounds per invocation, time budget 5 s per round (admin may lower), memory `memory_mb`. |

### 4.3 Invocation rounds (effects as data)

```
host ─call {kind:"command", command, params, settings, lang, app, api:1, reads:{textmap: blob 0}}─▶ plugin
host ◀──────── {actions:[…], next?: {reads:[…]}} ─────────────────────────────────────────────── plugin
host runs the actions (one undo step for the whole invocation), performs the reads, and if `next` was set:
host ─call {kind:"resume", results:[…], reads:{…}}─▶ plugin … until no `next` or 16 rounds
```

Kinds: `command` (a user ran a plugin command), `ui` (a click in the plugin's panel, with the widget id), `event`
(`selectionChanged` debounced 200 ms, `documentOpened`, `documentSaved`; actions limited to `ui.*`, D8), `resume`.

Actions v1:

| Action | Needs | Effect |
|---|---|---|
| `run {id, params}` | `document = write` for mutating ids, `read` for pure ones | `Session::run` on an **allowlisted** catalog id (§5.2); result returned in the next round |
| `textmap.apply {blob}` | `write` | applies edits to the text map read in this invocation (§4.4) |
| `read {what: "textmap" \| "selection" \| "paragraph", …}` | `read` | result in the next round |
| `fetch {url, method, headers, body}` | a **granted** `network` origin | done by the host (§5.3); response in the next round |
| `ui.panel {panel, widgets}` | the panel is declared | replaces the panel's widgets (fixed list: heading, text, list with items, button, text field, select) |
| `ui.status {text}` | — | one status line (Docs status bar / shell toast on phones) |

All mutating actions of one invocation are one undo step (`join_next_undo`, F4); an error in any round restores the
`edit_snapshot` taken before the first action, so a failed plugin leaves the document as it was.

### 4.4 The text map (the bulk read and write)

Read (`reads = ["textmap"]` with `params.scope = auto | selection | document`, `fieldResults`): the host walks the stories the
built-in walks (body and parts, F5) and sends one blob: per paragraph a stable index and per **segment** (a piece of a run
that never crosses a run, an object or a keep boundary): `u8 flags` (editable; why not: outside-selection, object, locked
control, field result), `lang` tag index into a small table, `u32 len`, UTF-8 text. Tracked deletions are left out (the built-in
leaves them out of the converter input too, `docs-translit/src/lib.rs:312`).

Apply (`textmap.apply`): per paragraph index and segment index, `u8 changed` and new text, the same shape as the spike's output
(`spikes/p7-runtime/wire.rs`). The host checks paragraph count, segment count and that only editable segments changed, then
runs **the built-in's `apply`** (tracked-changes path included) and maps the selection. The read and the apply belong to one
invocation; on the web the editor refuses an apply whose `Session::rev()` moved since the read (F4) and re-runs the read once.

### 4.5 Contributions and how they appear

| Contribution | Desktop native | Web / Android |
|---|---|---|
| Toolbar button | button in a "Plugins" group of the simple ribbon preset (F8); full ribbon: the same group in a "Plugins" custom tab | same (it is the same egui UI in the wasm) |
| Menu item | `tools`: the Plugins group's menu; `context.text`: the text context menu (as the Transliterate submenu, `STATUS.md:990`) | same |
| Command palette | every plugin command is a registry command, so `wordcraft:commands` lists it (F9) | same |
| Panel | an egui side panel drawn from the widget list (one new hook call, §9) | a DOM panel beside the editor iframe (shell) or the wrapper; a bottom sheet on a phone |

### 4.6 Bridge messages (web; new carried patch next to 0141)

Final shapes (WP4 with WP5, 2026-11-03): the field list is `docs/plugins.md` section 9; code is `docs-plugins::bridge` (state, no JS) and
`docs-office::plugin_web` (the JS end), patch `wordcraft/0148` only forwards. The page (WP5) builds every call and keeps `ui.*` and `fetch`; the
editor runs the editor actions. Differences from the first draft of this table: `plugins` carries the manifests (the slots derive from them,
as natively); `pluginResults` answers every `pluginActions` (it replaces the draft's separate results message) and ends with `error` on failure;
`pluginActions` has `more`, `abort` and `plugin`; `pluginsSet` acknowledges registration.

| Message | Direction | Fields |
|---|---|---|
| `<prefix>:plugins` | page -> editor | `{commands, toolbar, menu, plugins: [{manifest, settings?, enabled?}], timeoutMs?}`: registers the slot commands and the preset group (whole list) |
| `<prefix>:pluginsSet` | editor -> page | `{ids, errors}` |
| `<prefix>:pluginCall` | editor -> page | `{call, plugin, command, params, reads: {textmap?, selection?}}`, the reads as transferred `ArrayBuffer`s |
| `<prefix>:pluginActions` | page -> editor | `{call, plugin, actions, blobs, reads, more, abort?}`; the editor runs the actions and answers `pluginResults` |
| `<prefix>:pluginResults` | editor -> page | `{call, results, reads, error?}` |
| `<prefix>:pluginEvent` | editor -> page | `{event}` only for events a loaded plugin subscribed to |

A plugin command on the web returns at once with `{"pending": call}`; the document is locked for input until the answer with `more: false`
or the wait (default 20 s between two messages, `timeoutMs`) runs out (then "Plugin stopped: ..." in the status line, nothing changed).

### 4.7 Versioning

- `api` is the major. The host lists the majors it runs (v1 hosts: `[1]`). Additions inside v1 (a new action, context field,
  widget or event) are announced by `api_minor` in every call; plugins ignore unknown fields. A change of meaning or a removal
  is `api = 2`; a host then runs 1 and 2 side by side for at least one release.
- `docs/plugins.md` (WP11) outline: 1 Concepts (package, manifest, rounds, permissions); 2 Manifest reference; 3 ABI and message
  format; 4 Actions; 5 Text map; 6 Contributions and widgets; 7 Events; 8 Permissions, limits and what is refused; 9 Installing
  (desktop, web, docserver, Android) and admin settings; 10 The command catalog (generated from `describe()`, with the plugin
  allowlist marked); 11 Rust SDK and the two sample plugins; 12 Versioning and compatibility; 13 Security model and reporting.

---

## 5. Sandbox and permissions

### 5.1 Enforcement per platform

| Layer | Desktop native (wasmtime) | Web and Android (Worker) |
|---|---|---|
| What the plugin can call | only the imports of §4.2; no WASI linked; a module importing anything else is refused at load | the same import check through `WebAssembly.Module.imports()` before instantiation |
| Network | no import exists; `fetch` actions pass the grant check, then the host's HTTP client | no import exists; the worker script has CSP `default-src 'none'` (spike: worker `fetch` → `TypeError`); `fetch` actions run in the page after the grant check, `credentials: "omit"` |
| Filesystem | no import exists; file-touching catalog commands are not on the allowlist (§5.2) | the same allowlist; the page has no file API for plugins |
| Runaway code | epoch deadline (spike: stopped at 50 ms with a 50 ms tick) | `Worker.terminate()` after the round's time budget (spike: page responsive, worker killed) |
| Memory | `StoreLimits` at `memory_mb` (spike: capped at 64 MiB) | host-made imported memory with `maximum` (spike: capped; exported memory reached 4.28 GB, so it is refused) |
| Bad output | message parser with length checks; text map apply validates counts and editable flags; any error → `restore` | same parser on the page side (JS) and the editor side (Rust) |
| Document | every action through `Session::run` (protection, read-only open, `enabled`, F4) | same, inside the editor wasm |

### 5.2 Command allowlist

Deny by default. Allowed prefixes (Docs registry counts in brackets, grep of `CommandSpec::new` in `crates/engine/src/cmd`):
`caret.` (16), `select.` (14), `text.` (22), `format.` (39), `font.` (4), `para.` (50), `styles.` (16), `list.` (3), `table.` (48),
`fields.` (8), `developer.` (16, content controls), `insert.` (44) and `review.` (35) **minus** named exceptions, and the reads
`document.text`, `document.paragraph`, `document.selection`, `document.inspect`. Refused: `file.*` (28: open, save, save as,
print, export), `edit.*` (31: the system clipboard and the user's undo stack), `tools.*` (11, macros among them), `mailings.*`
(`mailings.finish {path}` writes files), `insert.textFromFile`, `insert.picture`, `link.*`, `view.*`, `readAloud.*`, `ui.*`,
`office.*`, `plugin.*` (no plugin runs another), `review` accept/reject-all, and **any params object with a `path`, `dir`, `file`
or `url` key** (WP1: also `uri`, `href`, `src`, any depth, any case; a value that is an array of non-negative integers is a
node path and passes, so `document.paragraph {path: [3]}` works). The list is data in `plugin-api`, with a test that every
exception and read id exists in the registry and that no allowed spec's `params` string mentions a path or URL (WP1; the test
lives in `docs-office` because the data crate cannot link an engine). **WP1 result against the real registry (633 commands):
306 allowed.** The test found nine more exceptions the plan text missed, all taking a path, file or bytes: `font.add`,
`font.loadSystem`, `font.remove`, `insert.link` (a link target can be a file or script URL), `insert.object`,
`review.compare`, `review.combine`, `styles.organizer`, `styles.copyFrom`. Families with no allowed prefix stay refused
(`layout.`, `arrange.`, `hf.`, `design.`, `picture.`, `shape.`, `outline.`, `references.` and the rest); adding one is a
reviewed change to `plugin_api::allow::PREFIXES`.

### 5.3 Grants

- `document`: declared in the manifest, shown in the install dialog, no separate grant.
- `network`: per origin. **Server mode:** only the admin grants it, in the provider's plugin entry (§8). **Local desktop / web:**
  the user is asked once per origin with the plugin name and publisher ("Allow … to send data to api.example.org?"), the answer
  stored in `settings.toml` `[plugins.grants]`; a machine policy can forbid user grants (O3). **Android local app:** never (F13).
- A grant is checked on every `fetch` action, against the exact origin; redirects to another origin are refused.

### 5.4 The malicious-plugin test (exit gate item 2)

One test plugin `evil` (built in CI from `ci/plugins/evil`), each command one attack; each must be **blocked, logged, and the
document unchanged**, on **desktop native (cargo test with the real host), web shell, docserver wrapper and the Android app**
(Playwright for web; the emulator through CDP like Phase 6 WP13).

| # | Attack | Expected |
|---|---|---|
| M1 | module imports `wasi_snapshot_preview1.path_open`, `fd_write`, `sock_open` | load refused: "imports not allowed: …" |
| M2 | module exports its own memory instead of importing it | load refused |
| M3 | `fetch` to `https://example.org` with no `network` permission | refused, audit line, next round sees `{error: "network not permitted"}` |
| M4 | permission for origin A, `fetch` to origin B, and to A with a redirect to B | refused |
| M5 | `run file.saveAs {path: "C:/Users/Public/x.docx"}`, `run insert.textFromFile {path: …}`, `run tools.macros` | refused by the allowlist; no file written or read (the test checks the paths) |
| M6 | infinite loop in `call` | stopped at the time budget; editor responsive (web: a timer fires on schedule); "plugin stopped" |
| M7 | grow memory until refused | capped at `memory_mb`; no crash of the editor or the page |
| M8 | malformed answers: truncated message, header not JSON, 1 GB length field, text map with an extra segment, a changed non-editable segment, invalid UTF-8 | refused, `restore`, document equal to before (`doc == before` as in the docs-translit tests) |
| M9 | panel label `<img src=x onerror=…>` and a 10,000-character label | shown as text, cut at the limit (Playwright checks `innerHTML` has no element) |
| M10 | edit from a `selectionChanged` event | refused (D8) |
| M11 | (web) a test-only worker variant calls `fetch` directly | blocked by the worker CSP (proves the second layer) |
| M12 | (server) a plugin not enabled for the provider requested by a crafted wrapper message | the page host only loads the session's list; the docserver does not serve it (`/ds/plugins/<sha>` 404 for that session) |

Plus the sandbox review (WP12, architect): imports, the allowlist, the parsers, the grant path and the docserver routes.

---

## 6. Plugin #1: uz-translit as a plugin

### 6.1 What moves where
- `docs-translit` is split without changing behaviour: the walker (targets, `story_pass`, `classify`, `field_spans`, `apply`,
  `map_offset`) becomes `docs-plugins::textmap` (read = classify into segments; apply = the existing `apply`). The built-in commands
  call textmap read → `uz_translit` in process → textmap apply; their 26 tests and the preservation tests must stay green
  unchanged before anything else is built on it (WP3 step 1).
- The plugin (`plugins/uz-translit`, our crate, `wasm32-unknown-unknown`) holds only: params → `Options`/`LangPolicy`,
  `lang_converts` per segment, `detect` for `Auto`, `convert_runs` per paragraph, the summary counts. It is the **same
  `uz-translit` crate** the built-in links, so identical output follows from identical segments plus the same apply.
- Commands: `toCyrillic`, `toLatin`, `auto` with the built-in's params (`scope`, `skipLangs`, `fieldResults`, `latinAlphabet`,
  `apostrophe`, `foreignGuard`, `docs-translit/src/lib.rs:28`). The summary goes to `ui.status` with the built-in's text.

### 6.2 Where the time could go, and how the API avoids the 62 s trap

| Possible cost | ONLYOFFICE | Here |
|---|---|---|
| Calls across the boundary | one per replacement | **one** per invocation (the text map comes with the first call) |
| Encoding | hex strings | binary blob (0.1-0.8 ms to encode, spike); JSON would add 35-40 ms |
| Replacement | search-based `ReplaceTextSmart` per item, each a layout | one `apply` over the map, one undo step, **one relayout** (as the built-in) |
| Keeping the UI responsive | batching across frames | not needed: the plugin part is 45-73 ms (spike) |
| Untouched paragraphs | — | an unchanged paragraph is not written (`Borrowed` → `changed = 0`), so its layout cache stays (as `docs-translit/src/lib.rs:337-339`) |
| Extra copies | — | native: guest memory copy in and out (included in the 65-73 ms); web: editor → page → worker → page → editor, transferred buffers (Worker hop < 1 ms measured; the iframe hop not measured) |

### 6.3 The measurement (gate evidence for "including speed", brief :463)

- **Files:** the two synthetic 108-page files from `scripts/translit/make-108.py` (`STATUS.md:971`), both directions, scope
  document; plus the 9 Word-written fixtures for output.
- **Same build, same session, alternating** built-in and plugin, median of 5 each, release:
  N1 headless native (`perf_108` extended with a plugin case: command time, command → finished relayout);
  N2 desktop `docs.exe` over the control channel (command → next frame, → painted);
  N3 web, `scripts/translit/perf-web.mjs` extended (palette → presented frame) in the shell **and** in the docserver wrapper;
  N4 the Android app on the emulator (indicative only, §10).
- **Pass (speed):** plugin command part ≤ built-in + 50 ms native and ≤ built-in + 100 ms web; end to end (N1 relayout, N2
  painted, N3 presented) within **+10 %** of the built-in measured in the same run; all under the Phase 2 gate of 2 s.
  Projection from the spike: +25-33 ms native (wasmtime 65-73 vs 40 ms) and about +5-20 ms web, i.e. 2-8 % of the
  0.35-1.27 s totals. Not yet measured with the real text map.
- **Pass (output):** for every fixture × direction × {tracked off, on} × {document, selection, caret}: the saved docx of the
  plugin run is **byte-identical** to the built-in run (same session start, same save path); in-memory `doc` equal; undo restores
  `doc == before`. Run natively and in the web build (save through the bridge).

---

## 7. Plugin #2: template fields (a different API surface)

Brief: "insert an HRM template field" (:465). With the no-HRM-names rule the field list is **configuration**: the plugin's
`settings.fields = [{name, label, kind}]`, filled by the admin per provider (§8) or by the user in local mode; the HRM team gives
the operator the names (O8).

| Surface exercised | How |
|---|---|
| Panel (declarative UI) | a list of fields with a filter box and an "Insert" button per field |
| `ui` events and catalog commands with params | Insert = `run developer.plainText {title: label, tag: name}` then `run developer.properties {placeholder: label, lockDelete: true}` (F14); `kind = "bookmark"` uses `insert.bookmark {name}` around a placeholder |
| Selection and events | `selectionChanged` → `read selection` (`document.selection`) → the panel highlights the field the caret is in (event actions are `ui.*` only, D8) |
| Settings | defaults in the manifest, admin values per provider, user values in local mode |
| A second command | "Check fields": lists missing and unknown tags (`document.inspect` objects) in the panel |

Done-check: insert, highlight and check on desktop, web and Android; the saved docx carries `w:sdt` with `w:tag` = the name and
round-trips through open/save; no field name appears in our code (`no_hrm_names` scan includes `plugins/`).

---

## 8. Install, enable, disable, admin control

| Where | Installed from | Stored | Enabled and granted by |
|---|---|---|---|
| Desktop (native windows and the in-window shell) | the shell's **Plugins** page, "Install from file" (desktop IPC, path rules as `open`, `desktop-host.md:4`), or a machine folder filled by IT/MSI | per user `%APPDATA%\<product>\plugins\<id>\<version>\`, machine `%PROGRAMDATA%\<product>\plugins\` | `settings.toml` `[plugins] enabled`, `grants` (suite-core); machine policy can forbid user install and user grants (O3) |
| Web local (`suite-serve` alone) | the operator's plugins folder | `suite-serve` config `plugins_dir` | the same config |
| Docserver (per organisation = per provider, F11) | the admin puts packages in `[docserver.plugins] dir` | read at start; SHA-256 computed; packages that fail validation are logged and skipped | `[[docserver.providers]] plugins = [{ id, version, sha256, grant = [...], settings = {...} }]`; the session answer lists them; `/ds/plugins/<sha256>` serves only plugins of that session's provider; audit lines `plugin.loaded`, `plugin.refused`, `plugin.network` |
| Android app | first-party plugins bundled in the APK assets; "Install from file" through the picker (O3) | the app's private files | app preferences; no network grants (F13) |

- **The config token cannot add plugins.** ONLYOFFICE's `editorConfig.plugins` (`pluginsData` URLs) is ignored: code comes only
  from the admin's folder, never from a URL in a token (`docs/phase5/onlyoffice-compat.md` gets the row).
- **Disable** = remove from `enabled` (local) or from the provider list (server); running editors drop the plugin's commands
  and panel at the next document open (server) or immediately (local, via the settings watch).
- **Integrity:** the install record stores the SHA-256; a package that changed on disk is refused. Signatures: O4.
- **Local mode with no server** needs nothing else: the desktop and Android apps read their own folders; no login.

---

## 9. Carried patches and size budgets

| Change | Where | Patch | Size effect |
|---|---|---|---|
| Plugin bridge messages (§4.6) | `apps/wordcraft-web/src/web/host.rs` (our product layer file, not hot) | new wordcraft/01xx, about +120 lines like 0141 (+61) | Docs wasm: **measure** (WP4) |
| Panel hook (desktop) | one call at the side-panel site of `ui-egui/src/lib.rs` (hot) to an `office_overlay` hook, panel drawn in `docs-plugins` | new wordcraft/01xx, about 6 lines in `lib.rs` (tripwire: >20 lines in hot files → architect review) | none on web (desktop only) |
| `docs-plugins` and the text map | our crate; the walker moves out of `docs-translit` | none (install through the existing `office-ext` hooks, `apps/wordcraft/src/main.rs:431-434`) | web: the walker is already in the wasm; new: slot table, codec, executor, contributions — **measure** |
| wasmtime | `docs.exe` only (desktop) | none | +12.4 MB raw per exe (spike) |
| Page host | `web/plugins/host.js`, `worker.js` | none (ours) | new `[plugins]` row in `web/budgets.toml`, measured + 10 %; loaded only when a plugin is enabled (one loader line in `shell.js` and the wrapper, ≤ 200 B against 3,947 B headroom); lazy rule checked in `shell.spec.ts` |

Docs wasm: 24,678 B headroom (F12). The bridge part of 0141 was +60 lines; the plugin part is about twice that plus the codec
and executor. If WP4 measures over the headroom, re-baseline by the rule (measured + 10 %, STATUS line, `web/budgets.toml:19`),
as Phase 2 and Phase 6 did; no lazy second wasm (an egui app is one module).

---

## 10. What this PC can and cannot prove

| Can prove here | Cannot prove here |
|---|---|
| Desktop native (gnu and MSVC builds), web in Chromium (Playwright), WebView2 in-window editors, the docserver wrapper with the fake HRM, the Android app on `HRM_Pixel_8` / `HRM_360` with recordings | Speed on a real phone (B-10 still open); iOS (no Mac, Phase 6 O3) |
| Every row of §5.4 on all four containers | That no wasmtime/V8 sandbox escape exists (we test our boundary, not the runtimes') |
| N1-N3 against the built-in on the same build | N4 on a phone (emulator figures labelled indicative, as Phase 6) |

---

## 11. Work packages

Each WP: goal, done-check, estimate (stream-days low/likely/high), tier, availability: **[now]** = no owner input needed after
the Phase 7 go; **[owner]** = needs an owner decision.

### WP0. Kick-off and merge-forward  [now]
- **Goal:** record the merge-forward rule phase-6 → phase-7 in STATUS; merge `phase-6` if it moved; baseline `ci.sh quick` and
  `sizes`; copy the spike numbers (§1) into STATUS; send §13.4 to the owner (Uzbek).
- **Done-check:** STATUS lines with the baseline. **Estimate:** 0.5 (0.5/0.5/1). **Tier:** builder.

### WP1. `plugin-api` crate and `docs/plugins.md` draft  [now]
- **Goal:** `crates/plugin-api` (root workspace; `serde`, `serde_json`, `toml` only; native + wasm32): manifest types and
  validation (§4.1), the message envelope and the text map codec (§4.2, §4.4), actions, the permission model, the command
  allowlist as data (§5.2), API version rules (§4.7). `docs/plugins.md` sections 1-8 as a draft.
- **Done-check:** unit tests for every refusal in §4.1; codec round trip property test (random paragraphs/segments, arbitrary
  UTF-8); a fuzz-style test of the parser on truncated and oversized inputs (M8 rows); allowlist test against the Docs registry.
- **Estimate:** 3 (2/3/4). **Tier:** architect for the API review, builder for the code.

### WP2. Native host `crates/plugin-host` (wasmtime)  [now]
- **Goal:** load (import/export check, imported memory with the cap), background compile, epoch ticker and per-round deadline,
  `StoreLimits`, the round loop with the 16-round cap, error mapping; wasmtime pinned (`=` version, Wasmtime LTS line if the
  release policy offers one: check in this WP). gnu **and** MSVC builds; `cargo-deny` for the docs domain with the new graph.
- **Done-check:** M1, M2, M6, M7, M8 as unit tests with test modules; `ci.sh deny` green; exe size delta of `docs.exe` in STATUS.
- **Estimate:** 3.5 (2.5/3.5/5). **Tier:** builder; architect review of the limits. Note for O2 = all: a shared `plugin-host`
  DLL instead of four static copies (+0.5-1 d).

### WP3. Docs adapter `domains/docs/crates/docs-plugins`  [now]
- **Goal:** step 1 the text map refactor of `docs-translit` with **no behaviour change** (its 26 tests and the preservation
  tests unchanged and green); step 2 slot commands (64) and re-registration on load and language change; the action executor
  (allowlist, one undo step, `restore` on error, `rev` check); events; contributions into the ribbon preset (F8), the context menu
  and the palette; the desktop panel through the new hook (§9).
- **Done-check:** docs-translit tests and `preserve` tests identical before/after (log both); docs-plugins tests with a fake
  in-process guest (no wasm) for every action; preset test that plugin buttons appear and disappear; `verify-queue.sh wordcraft`.
- **Estimate:** 6 (4/6/9). **Tier:** builder; architect review of the refactor and the executor.
- **Done in `p7-native` (fetch):** `crates/plugin-host/src/net.rs`, table test `net::tests::address_table`; the "admin allowed private targets" exception is not built (always refused). Original TODO:
- **TODO from WP2 (fetch):** `plugin-host` does not perform `fetch` (it only runs the module). The adapter that executes actions owns it
  (the desktop one: WP3's executor, with the HTTP client the app already has). It must, before connecting: require plain `https`, resolve the
  host name, refuse loopback, private (10/8, 172.16/12, 192.168/16), link-local (169.254/16 incl. the cloud metadata address), CGNAT
  (100.64/10), unspecified, multicast, IPv6 loopback/ULA (fc00::/7)/link-local (fe80::/10) and IPv4-mapped forms of all of these,
  then connect to the **checked address** (not re-resolve), and re-check on every redirect hop (`docs/plugins.md` section 8).
  Unless the admin allowed private targets for the provider. Needs a unit test table of these addresses.

### WP4. Web editor side (bridge)  [now]
- **Goal:** §4.6 as patch wordcraft/01xx; pending invocation state with input lock and timeout; transferred buffers.
- **Done-check:** a `bridge-plugins.spec.ts` with a stub page host: register, call, actions, stale `rev`, timeout; Docs wasm
  brotli delta in STATUS (re-baseline by rule if needed).
- **Estimate:** 3 (2/3/5). **Tier:** builder.

### WP5. Web page host  [now]
- **Goal:** `web/plugins/host.js` and `worker.js` (CSP header from `suite-serve`, the docserver and the Android asset loader);
  lazy loading from the shell and the wrapper; one Worker per plugin; import check, imported memory, time budget and terminate;
  grant prompts and the DOM panel (accessible names, phone bottom sheet); strings in `i18n/shell.tsv` (four languages, Uzbek
  `reviewed:agent-<model>:<date>`); `[plugins]` budget row.
- **Done-check:** Playwright: lazy rule (no plugin → no `host.js` request), load, panel, M3, M6, M7, M9, M11 in the shell; `sizes`.
- **Estimate:** 4 (3/4/6). **Tier:** builder; architect review of the CSP and grant path.

### WP6. Plugin #1 and the Rust SDK  [now]
- **Goal:** `crates/plugin-sdk` (alloc/free/call glue, codec, typed actions; no proc macros), `plugins/uz-translit` (§6),
  `cargo xt plugin pack` (zip + SHA-256, deterministic), first-party plugins built in CI.
- **Done-check:** §6.3 output matrix native and web; N1, N2, N3 in STATUS against the built-in in the same run; pass rules of §6.3.
- **Estimate:** 3.5 (2.5/3.5/5). **Tier:** builder.

### WP7. Plugin #2 template fields  [now; O8 for the real HRM names]
- **Goal:** §7.
- **Done-check:** §7 done-check on desktop and web; settings from a provider entry in the docserver spec.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP8. Install, enable, admin control  [O3, O4, O7]
- **Goal:** §8: suite-core plugin store and settings; desktop IPC `plugins_list/install/enable/grant`; the shell Plugins page;
  `suite-serve` `/plugins/` route; docserver `[docserver.plugins]`, provider entries, session answer, `/ds/plugins/<sha256>`,
  audit lines, `onlyoffice-compat.md` row; integrity and (per O4) signature check; machine policy (per O3).
- **Done-check:** docserver tests (only the provider's plugins are listed and served, M12, a changed package refused); desktop
  host tests for the IPC path rules; Playwright on the Plugins page.
- **Estimate:** 4.5 (3/4.5/7). **Tier:** builder; architect review of the docserver routes.

### WP9. Android  [now]
- **Goal:** bundle first-party plugins in the APK assets; the asset loader serves `/plugins/*` and `worker.js` with the CSP
  header; install from file (per O3); emulator runs of both plugins and the §5.4 rows with recordings.
- **Done-check:** recordings and a JSON per item as Phase 6 WP13; APK size delta.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP10. Malicious-plugin suite (gate item 2)  [now, after WP2, WP5, WP8]
- **Goal:** `ci/plugins/evil` and the M1-M12 runs on desktop native, web shell, docserver wrapper, Android; a CI job
  `plugins` (quick when `crates/plugin-*`, `domains/docs/crates/docs-plugins`, `web/plugins` change).
- **Done-check:** a table of 12 rows × 4 containers, each blocked with its evidence (test name, log line, recording).
- **Estimate:** 2.5 (2/2.5/4). **Tier:** builder; architect writes the attack list review.

### WP11. `docs/plugins.md` final and SDK template  [now]
- **Goal:** §4.7 outline complete; a template plugin repo layout under `plugins/template`; the generated command list.
- **Done-check:** the template builds and loads; a fresh sub-agent writes a "hello" plugin from the doc alone (doc test).
- **Estimate:** 1.5 (1/1.5/2). **Tier:** builder (sonnet), checked by a haiku-tier fresh read.

### WP12. Gate: security review, full CI, report  [now, at the end]
- **Goal:** adversarial review (architect) of §5; `ci.sh full`; the Uzbek gate report.
- **Done-check:** review findings closed or listed; report sent. **Estimate:** 2 (1.5/2/3). **Tier:** architect.

### WP13. Sheets and Slides adapters  [O2 = all]
- **Goal:** slots through `register_command`, a cell/shape text map, bridge messages in `gridcraft:`/`deckcraft:`, both plugins
  where they apply (translit yes; template fields Docs only).
- **Estimate:** 8 (6/8/12). **Tier:** builder.

---

## 12. Order, parallel streams, and what starts now

**Start immediately on the owner's Phase 7 go:** WP0, then WP1; after WP1 in parallel WP2, WP3, WP5, WP8 (the parts O3/O4/O7
do not touch); then WP4, WP6, WP7, WP9, WP10, WP11, WP12.

| Stream | Sequence | Waits on |
|---|---|---|
| A, core | WP1 → WP2 | nothing |
| B, Docs | WP3 → WP4 | WP1 |
| C, web host | WP5 | WP1 (a stub editor until WP4) |
| D, distribution | WP8 | WP1; O3, O4, O7 for their parts |
| E, plugins | WP6 → WP7 | WP2 + WP3 (native), WP4 + WP5 (web) |
| F, mobile | WP9 | WP5, WP8 |
| Gate | WP10 → WP11 → WP12 | all of the above |

- **Critical path:** WP0 → WP1 → WP3 → WP4 → WP6 (web) → WP9 → WP10 → WP12, about **22.5 stream-days** likely.
- **Merge-forward:** WP3 and WP4 change `patches/wordcraft`, which Phases 4-6 also change; merge `phase-6` before each and run
  `verify-queue.sh wordcraft` after.

---

## 13. Estimate, risks, gate mapping, owner decisions

### 13.1 Estimate (stream-days: low / likely / high)

| Group | WPs | Low | Likely | High |
|---|---|---|---|---|
| Core and Docs | WP0-WP4 | 12 | 16 | 24 |
| Web host, plugins, distribution, Android | WP5-WP9 | 11.5 | 16 | 24 |
| Gate | WP10-WP12 | 4.5 | 6 | 9 |
| **Total, Docs scope** | | **27** | **38** | **57** |
| Sheets + Slides (O2) | WP13 | 6 | 8 | 12 |

Against SIZING §2.6 (24/35/51 for four apps, `SIZING.md:317-337`): this plan is Docs-only at 38 because it adds what SIZING did
not see: the text map refactor of the built-in (F5, inside WP3), the install and admin UI on three platforms (WP8 beyond 7.8),
the Android run (WP9) and a 12-row × 4-container malicious suite (WP10). SIZING's risk buffer of 10 d stands.

### 13.2 Risks

| # | Risk | Effect | Mitigation |
|---|---|---|---|
| **R1** | **A sandbox escape in wasmtime or V8** (JIT bugs happen) | a plugin runs with the user's rights | plugins only from admin/user-chosen sources (§8), no host functions besides memory and log, pinned runtime with nightly `cargo-deny advisories`; an out-of-process host (AppContainer) is the v2 hardening if the owner wants third-party plugins at scale |
| **R2** | **"Identical" breaks in the refactor** of `docs-translit` | the built-in changes behaviour | WP3 step 1 alone, existing tests and preservation tests unchanged; byte-compare the 108-page outputs before/after |
| R3 | Docs wasm over budget (24,678 B headroom) | `sizes` red | measure in WP4; re-baseline by rule with a STATUS line |
| R4 | Async web calls vs sync command semantics (macro recording, Repeat, `wordcraft:ran`) | odd results in the palette or macros | `pending` result; plugin commands not recordable in v1 (stated in `plugins.md`) |
| R5 | wasmtime churn (MSRV 1.97.0 = our 1.97.1 today; frequent majors) | upgrades forced by rustc or advisories | pin to the LTS line (48.x until about 2028; every 12th major is LTS for 24 months); the bump is one WP-sized job per year |
| R6 | Worker CSP and imported memory behave differently in WebView2 / Android WebView | a §5.4 row passes in Chrome only | WP9 and WP10 run every row in every container |
| R7 | Third-party plugin strings bypass our Uzbek review | poor Uzbek in third-party UI | stated in `plugins.md`; our two plugins use the catalogs and the agent review rule |
| R8 | Config-file-only admin control is not what the owner means by "admin control" | rework | O7 asked now |
| R9 | The allowlist misses a side-effect command (file, print, network) | a filesystem or data-leak hole | deny by default, the path-parameter test, the M5 row, the WP12 review |
| R10 | Desktop size: +12.4 MB raw per plugin-hosting exe | bigger MSI | Docs only in v1; shared DLL if four apps (WP2 note) |

### 13.3 Exit-gate items → proving WP

| Gate item / brief task | Proof | WP |
|---|---|---|
| **Both plugins run on web, desktop and mobile** | plugin #1 output matrix + plugin #2 done-check on desktop native, web shell, docserver wrapper, Android app (recordings) | WP6, WP7, WP9 |
| **Malicious plugin blocked** (network, filesystem) | §5.4 M1-M12 × 4 containers | WP10 |
| Task 1 plugin model (wasm, manifest, UI contributions) | WP1 tests, WP3/WP5 contribution tests | WP1-WP5 |
| Task 2 API on the command catalogs, versioned, `docs/plugins.md` | allowlisted `run`, text map, events; the doc | WP1, WP3, WP11 |
| Task 3 sandbox and permissions | §5.1 per platform; grants | WP2, WP5, WP8 |
| Task 4 uz-translit identical, including speed | §6.3 matrix and N1-N3 | WP6 |
| Task 5 second plugin, other surface | §7 | WP7 |
| Task 6 admin control per organisation | provider entries, M12, audit | WP8 |

### 13.4 Owner decisions (each with a recommendation)

| # | Decision | Options | Recommendation |
|---|---|---|---|
| **P7-O1** | **Start Phase 7 implementation now, in parallel** (gates stay 4 → 5 → 6 → 7) | go / wait for the Phase 6 gate | **Go** for WP0-WP6 and WP9-WP11: none needs a decision below; WP8 starts with the parts O3/O4/O7 do not change. |
| **P7-O2** | **Which apps get plugins in v1** | Docs only / Docs + Sheets + Slides / all four | **Docs only for the gate**; the API is app-neutral; Sheets and Slides after the gate (+8 d, WP13); PDF later (no parameterised commands). |
| **P7-O3** | **Who may install plugins** (BRIEF App. B) | (a) admin only everywhere; (b) admin on the server, the user in local mode unless a machine policy forbids it; (c) users everywhere | **(b).** Server: only the admin, per organisation (the config). Local desktop/Android: the user may install from a file (most users have no admin, accounts are optional), IT can switch user install and user network grants off with a machine policy. Our two plugins come preinstalled. |
| **P7-O4** | **Signing of plugin packages** | none / SHA-256 pin only / pin + publisher signature (Ed25519) required / signature optional | **Pin + optional signature in v1:** every install records the SHA-256 and refuses a changed package; packages signed with a trusted key show "verified"; unsigned ones install only in local mode with a warning, and never if the machine policy says so. Our first-party plugins are signed with a new project key the owner keeps offline, decided together with the Phase 10 signing identities. |
| **P7-O5** | **Network permission in v1** | implement grants (host-performed fetch, per origin) / always deny in v1 | **Implement** (the brief asks for "unless declared and granted"); neither of our plugins uses it; Android local stays deny (no INTERNET permission). Deny-only saves about 1.5 d. |
| **P7-O6** | **Built-in uz-translit after the plugin ships** | keep / replace with the plugin | **Keep** the built-in as the default; the plugin is installed but its buttons hidden unless enabled. Revisit after a release. |
| **P7-O7** | **Admin control surface** | config file per provider (v1) / an admin web page | **Config file in v1** (the docserver is configured by file today, F11); an admin page needs admin sign-in and is +3-5 d. |
| **P7-O8** | **Real field list for plugin #2** | HRM team gives the field names for the operator config / a synthetic list only | **Synthetic list for the gate**; the HRM names go into the pilot provider's config when the HRM team sends them (never into code). |
