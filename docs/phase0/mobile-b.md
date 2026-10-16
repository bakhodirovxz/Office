# Spike 3, approach B (hybrid): engine renders pages, a web page owns the UI (steps B1-B5)

Evidence only. Every number below comes from a run on this machine or on the `HRM_Pixel_8` emulator (Android 16, x86_64, 1080x2400 at 420 dpi, DPR 2.625, `-gpu swiftshader_indirect`, stock Chrome 133, Gboard 15.1, `-no-snapshot-save`).
Files are in `docs/phase0/mobile-b/` (4.3 MB). Code is in `spikes/mobile-b/` (nothing committed, nothing under `upstream/` touched; see "Worktree").
**Emulator numbers are indicative only.** The emulator runs on the host CPU with software GL, so CPU-bound figures (render ms) are probably better than a mid-range phone and frame times are probably worse.

## Result in one paragraph
A thin binding crate over the unpatched wordcraft engine (about 280 lines of code, 170 of them API) plus a 400-line web page passes the whole A0 test matrix on a real Gboard in Chrome, except where marked UNVERIFIED or PARTIAL. It needed **zero upstream patches**: the three wordcraft patches that A-web needed (DeleteSurrounding, touch scroll, lifecycle) and the texture-limit patch are not needed, because the page uses a Canvas2D tile cache in a native scroller and a hidden textarea. Renderer memory is about half of A-web, and the wasm is 40 percent smaller. The cost is that **the whole UI has to be rebuilt** (this page has four buttons; ui-egui is about 51k lines), and native selection handles are not available (the canvas has no text).

## Worktree and layout
- Worktree `E:/office-work/wordcraft-b`, branch `spike/mobile-b`, from `fd0bb2f`, **no edits** (`git status` clean). Creating it added only worktree metadata to `upstream/wordcraft/.git`. Upstream files are unmodified; the checked-out `upstream/wordcraft` also shows a clean `git status`.
- `spikes/mobile-b/bind` (own Cargo workspace, path deps into the worktree, `Cargo.lock` seeded from wordcraft's, so wasm-bindgen is the 0.2.129 that Trunk had cached).
- `spikes/mobile-b/web` (Trunk page: `index.html`, `app.js`, `Trunk.toml`, dev server `serve.py`, desktop check `desktop-check.js`).
- `spikes/mobile-b/android-webview` (B5), `spikes/mobile-b/scripts` (emulator harness adapted from `spikes/mobile-a/probe/scripts`, CDP helpers, `bench.js`, `typeburst.js`), `spikes/mobile-b/fixtures` (HRM `sample.docx`; `long.docx`, 4 pages Latin / Cyrillic / Uzbek, made by `bind/examples/gen_fixture.rs`).
- Build output: `CARGO_TARGET_DIR=E:/office-target/mobile-b`, trunk dist `E:/office-target/mobile-b/dist`. All builds waited for no other `rustc.exe` and at least 3 GB free; `-j 4`.

## B1: the bind API (`spikes/mobile-b/bind/src/lib.rs`)

Lines of code: `lib.rs` is 397 lines. Excluding comments and blanks: 281 lines before the test module, of which the wasm-bindgen export layer is about 111 and the shell-neutral `Editor` about 170. The test module is 57 lines. The page is `app.js` 400 + `index.html` 40.
The plan said "about 300 lines": confirmed.

| `Editor` method | What it does | Engine call it sits on (file:line, same in the worktree and `upstream/wordcraft` at `fd0bb2f`) |
|---|---|---|
| `open(name, bytes)` | parse any supported format by extension | `crates/engine/src/io.rs:151` `open_bytes_with`, then `Session::new` `session.rs:590` |
| `blank()` | empty document on default paper | `session.rs:1350` `blank_document` |
| `run(id, json)` | any of the 629 commands (undo, bold, ...) | `session.rs:1167` `Session::run`; ids e.g. `edit.undo` `cmd/edit.rs:36`, `format.bold` `cmd/format.rs:17` |
| `page_count`, `page_size_pt`, `layout_ms` | page list | `session.rs:766` `Session::layout` -> `DocLayout.pages` (`layout/src/lib.rs:453`) |
| `render_tile(page, scale, x, y, w, h)` | premultiplied RGBA8 window of a page | `crates/render/src/lib.rs:497` `render_region` (`render_page` is `:489`) |
| `tap(page, x_pt, y_pt, extend)` | hit test, sets caret or extends | command `caret.set` `cmd/caret.rs:50`, handler `caret.rs:269`, which calls `layout.hit` `layout/src/hit.rs:269` |
| `select_word()` | long-press word selection | `select.word` `cmd/caret.rs:55` |
| `caret()` | `{page,x,top,height,width}` in points | `layout/src/hit.rs:440` `caret_on` |
| `selection_rects()` | highlight rectangles per page | `layout/src/hit.rs:613` `selection_rects` |
| `insert(s)` | type text (`\n` becomes a paragraph) | `text.insert` `cmd/text.rs:13`, handler `:91` |
| `delete_before(n)`, `delete_after(n)` | IME `deleteSurroundingText` | `text.backspace` `cmd/text.rs:20`/`:659`, `text.delete` `:21` |
| `text_context(before, after)` | the paragraph around the caret in UTF-16 units, with a leading `\n` sentinel for "start of paragraph" | reads `Session.doc.para_at` (`doc/src/lib.rs:809`) and `Session.sel`; **no engine command needed** |
| `replace_utf16(from, to, text)` | replaces a range of the mirror: typing, autocorrect, delete, Enter, deleting the sentinel joins paragraphs | sets `Session.sel` directly (it is `pub`, `session.rs:335`), then `text.insert` or `text.backspace` |
| `select_utf16(a, b)` | IME cursor moves | sets `Session.sel` |
| `can_undo`, `dirty`, `bold_at_caret`, `word_count`, `plain_text` | toolbar state | `session.rs:897`, `:343`, `:1314`, `:1333`, `doc.plain_text` |
| `save(name)` | docx (or any extension) bytes | `session.rs:883` `stamp_save`, `io.rs:328` `save_bytes_with` |

Differences from the plan's 3B.2 sketch: `render_tile` takes `&mut self` (layout is lazy), `text_context` returns a sentinel-prefixed paragraph and `replace_utf16` is the single IME primitive. Both `replace_utf16` and `select_utf16` write `Session.sel` instead of going through a command (the egui UI does the same); it works because the field is `pub`.
The wasm layer is `#[wasm_bindgen] WcEditor`; `render_tile` returns `Clamped<Vec<u8>>` (a `Uint8ClampedArray`, ready for `ImageData`). Trunk built the lib crate and the exports are reachable from JS through `window.wasmBindings` (the plan marked this UNVERIFIED: it works, no fallback to the wasm-bindgen CLI was needed).

**Native test** (`cargo test --release`, log `mobile-b/b1-cargo-test.log`): 2 passed.
1. `open_insert_save_contains_x`: opens the HRM fixture `sample.docx` (does not already contain X), `caret.docEnd`, `insert("X")`, `save`, the saved `word/document.xml` contains X, and re-opening the saved bytes gives text with X.
2. `tap_render_caret_and_ime_edits`: renders a page and a 20x20 tile with the exact byte counts, ink pixels exist, `tap` gives a caret, `replace_utf16` turns `teh ` into `the `, Enter adds a paragraph, `replace_utf16(0,1,"")` through the sentinel joins it again.

## B2: the page (`spikes/mobile-b/web`)
- `<canvas>` tiles (1024x1024 device pixels, created lazily, evicted beyond 900 CSS px) inside `.page` divs inside a native `overflow:auto` scroller. Native scroll, momentum and pinch apply. After a pinch settles (300 ms) the tiles are re-rendered at `DPR x pinch` scale (checked: text stays sharp at 2x).
- Viewport meta `interactive-widget=resizes-content`, so the layout viewport shrinks with Gboard and the fixed toolbar sits above the keyboard (verified, `b3-02`).
- Hidden-textarea IME bridge: the textarea holds `text_context()` of the current paragraph with a leading `\n`. Every `input` event is turned into the minimal edit by a prefix/suffix diff (anchored at the new caret) and applied with `replace_utf16`. During a composition the preedit is drawn as an overlay and the commit happens on `compositionend`. `beforeinput` is only logged (`__perf.inputTypes`). `selectionchange` on the textarea maps to `select_utf16`. Arrow Up/Down and Ctrl+Z/Y/B/S go to commands.
- Caret overlay, selection rectangles, 4-button bottom toolbar (Bold -> `format.bold`, Undo -> `edit.undo`, Save -> `save` + download + POST to the dev server, Zoom cycles 100/150/200/75 percent), long press (480 ms) selects the word.
- Build: `trunk build --release` with `data-wasm-opt="z"`, as for wordcraft-web.
- **Desktop done-check** (headless Chromium, 412x915 at DPR 2.625, `desktop-check.js`): tap puts the caret and focuses the textarea, typing `Hello Салом oʻgʻil`, Backspace, Enter, `Second`, then Save: the posted docx has paragraphs `HRM fixture paragraphHello Салом oʻgʻi` and `Second` (U+02BB intact). Evidence `d1-loaded.png`, `d2-typed.png`, `d-saved-desktop.docx`.

## B3: the A0 test matrix, side by side

Key: PASS, FAIL, PARTIAL, UNVERIFIED, n/t = not tested in that spike. A-web = A0 plus wordcraft patches 2-5 (`mobile-a0.md`, `wordcraft-patches.md`). A-native = stock eframe/winit plus the 170-line shim, keyboard-only probe (`mobile-a-kill.md`).

| # | Test | A-web (A0, then after patches) | A-native (shim probe) | **B (this spike)** | B evidence |
|---|---|---|---|---|---|
| 1a | Load and scale | PASS scale, FAIL fit (desktop ribbon in 412 px) | n/t | **PASS**: page fits width, DPR 2.625 correct, no ribbon (toolbar of 4 buttons) | `b3-01-load.jpg` |
| 1b | Load time | warm 2.0-2.4 s, first 2.9-3.9 s; about 90 s first start after `pm clear` | cold `am start -W` median 1.67 s (probe, no document) | first start after `pm clear` 22.3 s to ready (cold wasm compile and cold font caches; logcat shows systemui and Gboard ANRs in that window); process-cold 2.6-3.4 s; warm 1.1-2.1 s (below) | `b4-load-*.jsonl` |
| 2 | Open a document | PASS | n/t | **PASS** (4-page fixture and HRM fixture) | `b3-01` |
| 3a | Tap in text, keyboard appears | PASS | PASS | **PASS**, toolbar stays above the keyboard, caret scrolled into view above it | `b3-02`, `b3-07` |
| 3b | Back hides keyboard | PASS | PASS | **PASS** (`mInputShown=false`) | `b3-08` |
| 4b | Latin | PASS | PASS | **PASS** | `b3-03`, `b3-04` |
| 4c | Cyrillic `Салом дунё` (ё popup) | PASS | PASS | **PASS**, one `insertText` per letter, `ё` too | `b3-05` |
| 4d | Uzbek `oʻgʻil` | PASS (U+02BB, U+02BC) | PASS for delivery; tofu for U+02BB in egui's default font | **PASS**, `oʻ` and `gʻ` arrive as single `insertText` with U+02BB; saved docx has U+02BB; wordcraft's bundled fonts draw it (no tofu) | `b3-04`, `b3-saved-from-emulator.docx` |
| 4e | Autocorrect | FAIL (`tehthe`), PASS after patch 3 (37 lines) | PASS via shim | **PASS with no patch**: `wolrd ` gives `deleteContentBackward` (a 5-char range) then `insertText "world "`, the diff turns it into one replace. Note: wordcraft's own AutoCorrect already turns `teh ` into `The ` on commit, so `wolrd` is the real Gboard test | `b3-03b` |
| 4f | Glide typing | UNVERIFIED | PASS (Latin) | **PASS (Latin `world`)**, injected through the emulator console as in A. Arrives as one `insertText "world"`; the next Backspace removes the whole word. Cyrillic and Uzbek glide not tried | `b3-06` |
| 4g | Backspace into a committed word | PASS | PASS | **PASS** (`deleteContentBackward` x3 removed `world`, space, `ё`) | `b3-06` |
| 4h | Enter | PASS | PASS | **PASS** (`insertLineBreak` becomes a paragraph, next word autocapitalised thanks to the sentinel) | `b3-07` |
| 5a | One-finger swipe scroll, momentum | FAIL (selects text); PASS after patch 4 (190 lines) | n/t | **PASS, native**: a 1100 px swipe over 120 ms (419 CSS px) coasted to 1366 CSS px in 2.9 s; no text selected | `b3-swipe1.json`, `b4-scroll.mp4` |
| 5b | Pinch zoom | PASS | n/t | **PASS, native**, visual viewport 1 to 2 and back; tiles re-rendered at scale 3.4 (sharp). Weakness: the fixed toolbar leaves the screen while zoomed | `b3-10-pinch-2x.jpg` |
| 5c | Long-press selection, native handles | PARTIAL: word selected, desktop context menu, no handles | n/t | **PARTIAL**: word selected and drawn by the overlay, **no native handles or magnifier or floating toolbar** (canvas has no text for Chrome to select). Also, the keyboard did not open after the long press (focus is moved from a timer, outside the gesture). Native handles would need a DOM text proxy; not attempted | `b3-11-longpress.jpg` |
| 5d | Rotation | PASS state, poor ergonomics (60 dp page area) | n/t | **PASS**: page re-fits width (landscape 864 CSS px wide), selection survives both ways; browser chrome leaves about 304 dp of height | `b3-12`, `b3-13` |
| 6 | 360 dp (`wm size 1080x2280`, `density 480`) | FAIL (ribbon cut, page clipped), density change reloads the page | n/t | **PASS layout**: page fit 360 px, toolbar fits (the Zoom label wraps), keyboard opens, caret placed. The density change reloaded the page as in A0 (state lost, a Chrome behaviour) | `b3-14*.jpg` |
| 7 | App switch and return | FAIL (broken frame icon, edits lost); PASS for short absences after patch 5 (autosave and context-loss recovery) | Activity-destroy ANR seen twice | **PASS** with no code: 25 s in Settings and 55 s on the launcher, return: same page, text intact, 12 canvases, tap and keyboard work. No GPU-process crash in logcat (Canvas2D, no WebGL). Caveat: DevTools was attached, which keeps `visibilityState` at `visible`, so Chrome may throttle differently without it; Chrome killing the renderer in the background was not provoked | `b3-15*`, `b3-16*` |
| 8 | Save and HRM flow | PASS (File > Save, callback) | n/t | **PASS** Save button: docx of 7,156 bytes posted to the dev server, text correct. No HRM bridge was built (not asked) | `b3-saved-from-emulator.docx` |
| 9 | Bold, Undo buttons | n/a | n/a | **PASS** on device (Undo removed text with keyboard staying up; Bold toggled the selected word) | `b3-17-bold-toggled.jpg` |

Not exercised in B: the preedit overlay. Gboard on this emulator sent **no `composition*` events** into the textarea at all (plain `insertText` per key, `deleteContentBackward` + `insertText` for corrections), so the `compositionstart`/`end` path was never hit on a device. It is unit-equivalent to the commit path (same diff) but UNVERIFIED on a keyboard that composes (other Gboard settings, Samsung, SwiftKey, CJK).

## B4: numbers

| Metric | Value | Compare |
|---|---|---|
| wasm raw / gzip -9 / brotli -q 11 | **22,668,040 / 9,007,491 / 5,816,742 B** (`b4-wasm-size.json`) | wordcraft-web patched: 42,618,253 / 15,874,939 / 9,450,860 (`wordcraft-patches.md`). B is 53 / 57 / 62 percent of it. The remainder is the engine, bundled fonts (4.2 MB of TTF in `assets/fonts`), dictionaries. A split of the 22.7 MB was not measured. JS glue 19 kB + `app.js` 21 kB |
| `render_tile` full page, 1040x1346 px (page fit, scale 1.70), median of 5 | 41-46 ms (pages 0, 1, 3) | on the emulator |
| `render_tile` full page at **DPR 2.625, CSS 100 percent (scale 3.5), 2142x2772 px = 5.9 MP**, median of 5 | **90-118 ms** (first call 92-184 ms) | `b4-render-tile-emulator.json` |
| `render_tile` 1024x1024 tile | 22-39 ms median (up to 87 ms) | what the page uses |
| Very first tile after a cold start | 2451 ms (cold font and shaping caches); next tiles 17-90 ms, 342 ms for the first of page 1 | one-off; included in the 22 s first-run load |
| Keystroke to repaint, CDP key events, mid-document, n=43 | engine p50 **0.5** / p95 4.9 ms; first tile re-rendered p50 24.9 / p95 58.1 ms; handler to first paint p50 **31** / p95 71 ms; **event to next rAF p50 41 / p95 87 ms** (max 228) | `b4-typing-latency-cdp-keys.json`. The rest of the visible tiles are queued after the first, so a long edit that reflows pages repaints off-caret tiles later (not measured as a total) |
| Same, with real Gboard taps (48 edits) | event to rAF p50 45.8 / p95 135 ms | `b4-edits-typing.json`; includes emulator tap-injection noise |
| `text_context` / `caret` calls | 0.1 ms / 0.02 ms | |
| Chrome renderer PSS / RSS, document open, after load and scrolling | **148 MB / 339 MB** (main Chrome process 109 MB, privileged 46 MB) | A-web: 284 MB / 482 MB at load, 326 / 529 after typing. Sandboxed renderer only |
| Same renderer at the end of the session (pinch 2x, zooms, typing, long press, several reloads in this Chrome) | 226 MB / 422 MB | `b4-meminfo-end-of-session.txt`; growth is real, tile canvases at high zoom plus wasm heap, not isolated |
| Scroll smoothness | rAF frames during scroll windows (6 swipes up and down): p50 16.7 ms, p95 83 ms, max 183 ms; 10 of 55 frames over 50 ms. First swipe: 28 frames, p95 83 ms. Tile renders run on the main thread, 20-140 ms each, one per task | `b4-scroll-summary.json`, **recording `b4-scroll.mp4` (540x1200, 14 s)**. I did not analyse the video frame by frame; the frame figures are from an in-page rAF sampler. Native scroll itself runs on the compositor, so blank (white) tiles for a moment after a fast fling is the visible cost, not a janky scroll |
| Load, warm (page reload) | navigation to editor ready 1.1-2.1 s (wasm fetch 0.6-1.0 s over `adb reverse`, open+layout 73-90 ms, first tiles 275-800 ms) | A-web warm 2.0-2.4 s |
| Save | 26 ms (desktop), 50 ms on the emulator, 7 kB docx | |

### B5: plain Android WebView APK (Tauri not attempted)
`spikes/mobile-b/android-webview`: a 40-line `Activity` with a `WebView` and `androidx.webkit` `WebViewAssetLoader`, the dist copied into `assets/`. Gradle 9.3.1 / AGP 9.1.0 with the Temurin 17 from the A spike; `assembleDebug` 1 min 10 s. APK **11.2 MB** (debug-signed, x86_64-independent, wasm deflated).
- `am start -W` `TotalTime` after `force-stop`: 1818, 2094, 1931 ms, then 1740, 1688, 1808 ms in a second set (`b5-am-start-W.txt` has a first set that started at 3.3-3.5 s while the emulator was still settling after the install). That is to the activity's first draw. Editor ready is 2.96-3.25 s after navigation start (`b5-webview-ready.txt`), so roughly 4.7-5 s from the launcher tap on this emulator.
- Typing works in the WebView exactly as in Chrome (`wolrd hello` gives `World hello`, `b5-02`). Memory: renderer **148 MB PSS**, app process 106 MB PSS (`b5-meminfo.txt`).
- Not done: Save has no bridge (the download link does nothing in a bare WebView; a `DownloadListener` or a JS interface is needed), edge-to-edge insets hide the toolbar behind the navigation bar at targetSdk 36 (visible in `b5-01`).
- **Tauri v2: not attempted** (time and RAM; the plain WebView already shows the page is WebView-clean). The plan's note that Tauri's Windows prerequisites ask for the MSVC toolchain, and that this host is windows-gnu, is **UNVERIFIED** here: I neither confirmed nor refuted it.

## Effort to a real mobile editor on approach B (estimates, not measurements)
What the spike proves is the **runtime**: rendering, hit testing, selection, IME, scroll, zoom and save. What is left is the product.
- Shell hardening: damage tracking (nothing tells which pages a command changed; the spike re-renders the caret tile first and the others after), tile cache tuned for memory, sub-tile slivers (1040 px pages leave a 16 px column), page-break anchoring on zoom, bridge for save/open in the host (HRM), scroll position restore, accessibility text layer: **1.5-2.5 engineer-months**.
- Engine patches for correctness: composition API (`text.compose`/`commitComposition`, plan estimate about 150 lines, UNVERIFIED), selection handles through a DOM text proxy, header/footer, table and object pointer behaviours that live only in `ui-egui` (`canvas.rs:1496-1529, 2175-2233`): **1.5-2.5 engineer-months**.
- The UI. ui-egui is about 51k lines (ribbon, Backstage, dialogs, panes, rulers; layer map "Obstacles" 3), command params have no schema, and 629 commands exist. A purpose-built phone UI (formatting bar, paragraph/list/style sheets, table and image basics, find, comments, open/save, 4 languages through the TSV i18n catalogs that are portable) for the 60-80 commands users need: **4-6 engineer-months**. Each further dialog costs about 1-3 days after the parameter list is read from the engine source. Parity with desktop is **multiple engineer-years** and is not a goal worth stating.
- Total for a credible phone editor: about **7-11 engineer-months** (UNVERIFIED, from the module list above, not from a size model). A, by contrast, keeps the egui UI and needs a compact layout and the already-built patches (about 3-3.5 days of spike plus real UI work per plan 3A), but keeps desktop and mobile on one UI codebase.

## Risks
1. **UI rebuild** (above): the dominant cost and the dominant risk. Two UIs forever (egui for desktop, web/mobile for phones) unless desktop moves to the same web UI.
2. **No native selection handles** or magnifier on canvas pages. Users will notice. A DOM text proxy (invisible `contenteditable` mirror) can give them back but doubles layout work and was not tried.
3. **No composition API** in the engine; preedit is an overlay that hides the text underneath it and does not reflow the line. Needs the upstream patch for correct inline composition. The overlay path was not exercised by Gboard here.
4. **Single-threaded wasm rendering**: tiles are 20-140 ms on the main thread in the emulator, a phone is probably slower; large documents (hundreds of pages, big images) untested. Wasm threads need cross-origin isolation (SharedArrayBuffer), not tried.
5. **Memory**: renderer PSS 148 MB after load and 226 MB after a session; large documents unmeasured. Tile canvases are 4 bytes per pixel.
6. **Cold start**: 22 s first run after a Chrome data clear on this emulator (22.7 MB wasm, cold compile, ANRs in other apps); warm 1.1-2.1 s. A real phone with a slower CPU: UNVERIFIED. Streaming compile and the brotli-compressed 5.8 MB transfer help on a network.
7. **Gboard variants**: the diff approach is keyboard-agnostic, but only Gboard 15.1 was tried; no composition events were seen; no Samsung/SwiftKey/CJK.
8. **Fixed toolbar leaves the screen under pinch zoom**; visual-viewport handling is basic.
9. Browser-specific: Chrome 133 only. Safari/iOS WebKit (no macOS here, so untestable), `interactive-widget` is Chrome-only, iOS keyboard behaviour for hidden textareas differs.
10. Upstream churn: `Session.sel` and `layout()` are used directly (pub fields); the engine has no stable API. The bind crate has to track upstream, though it touches few symbols (table above).
11. Tauri/Windows host (UNVERIFIED, not attempted).

## Licences of the toolkits used or proposed
| Piece | Licence | Source of the statement |
|---|---|---|
| Trunk 0.21.14 | MIT/Apache-2.0 | its `Cargo.toml` in the cargo registry (VERIFIED) |
| wasm-bindgen 0.2.129, js-sys | MIT OR Apache-2.0 | `Cargo.toml` in the registry (VERIFIED) |
| wasm-opt (Binaryen, `version_123` cached by Trunk) | Apache-2.0 | upstream knowledge, no LICENSE file in the cache (UNVERIFIED) |
| androidx.webkit 1.12.1 | Apache-2.0 | POM in the Gradle cache (VERIFIED) |
| Gradle, Android Gradle Plugin | Apache-2.0 | UNVERIFIED here |
| Android System WebView / Chrome | BSD-style plus bundled third-party licences | not checked |
| Tauri v2 (proposed, not used) | MIT OR Apache-2.0 | plan 3B.3; UNVERIFIED |
| Flutter / flutter_rust_bridge (proposed, not used) | BSD-3-Clause / MIT | plan 3B.3; UNVERIFIED |
| wordcraft engine crates and `vello_cpu` | MIT OR Apache-2.0 (workspace); vello_cpu dual licensed | `Cargo.toml` of wordcraft (VERIFIED for wordcraft), not re-checked for the 400-crate tree. Run `cargo-deny` before Phase 1 |
| Gboard, Chrome on the emulator | proprietary, test environment only | |
All are permissive. The only borrowed scaffold is the Gradle wrapper and `gradle.properties` copied from `spikes/mobile-a/probe/android` (itself adapted from `rust-android-examples`, whose licence is UNVERIFIED, see `mobile-a-kill.md`); `app/build.gradle`, the manifest and the activity here are written from scratch. Test scaffold only, never ship it.

## What was not done or not proven
- Gboard composition events (see above); glide in Cyrillic and Uzbek; autocorrect in a mixed-language field.
- Native selection handles (not possible on a canvas; DOM proxy not built).
- Real device, real GPU, real memory pressure. `-gpu host` was not tried (A0: unusable with Chrome). A renderer kill in the background was not provoked.
- Large documents, tables, images, headers in tiles (the fixtures are text only; the engine renders them, the page does not care).
- Tauri; HRM host bridge in the WebView; Save in the WebView.
- Frame-by-frame analysis of the scroll recording.
- Clean-up: emulator killed (`adb emu kill`), `adb kill-server`, dev server stopped, adb forwards and reverses removed with the server, `E:/office-target/PAUSE` untouched. The Expo "Uzgidro HRM" app was not touched. Chrome on the AVD keeps four tabs of the spike page and the `settings put global hide_error_dialogs 1` / `show_ime_with_hard_keyboard 1` flags (same as earlier spikes).
- Machine: the first `cargo test` waited about 10 minutes at the RAM gate and compiled in 12 m 51 s in all; wasm trunk build about 7 minutes including `wasm-opt`.

## Evidence files (`docs/phase0/mobile-b/`)
`b1-cargo-test.log`; `d1-loaded.png`, `d2-typed.png`, `d-saved-desktop.docx`, `d-console.txt` (desktop); `b3-*.jpg` (emulator screenshots, per test row), `b3-saved-from-emulator.docx`, `b3-swipe1.json`; `b4-wasm-size.json`, `b4-render-tile-emulator.json`, `b4-typing-latency-cdp-keys.json`, `b4-edits-typing.json`, `b4-load-*.jsonl`, `b4-meminfo-*.txt`, `b4-scroll-summary.json`, `b4-scroll.mp4`; `b5-*`.
