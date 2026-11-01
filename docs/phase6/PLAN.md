# Phase 6 (Mobile): execution plan

> **Owner decisions and requirements in force**
> - **Mobile is A-web for v1, conditionally** (ADR B-3, `docs/ARCHITECTURE-DECISION.md:30`, `:787`; `CLAUDE.md:24`). The final
>   B-3 decision was moved to the **start of Phase 6** (`docs/phase4/mobile-evidence.md:147-153`, `docs/phase4/PLAN.md:39`);
>   the **handle design** is written with it (owner, 2026-10-24, `docs/phase4/PLAN.md:39`). Both are in this plan (§1, §4).
> - **Android was deferred** until "the converter and the UI translations go first" (owner, 2026-10-20, `CLAUDE.md:24`).
>   Both are done (Phase 2 / Phase 4 gates). Priority stays **fidelity → mobile → co-editing**.
> - **Accounts are optional** (owner, 2026-10-21): the Android app works in local mode with no login. Mobile sign-in
>   (SIZING 6.8) waits for SSO, which moved after the Phase 5 pilot gate (`docs/phase5/PLAN.md:749`).
> - **Phase discipline:** Phase 4's gate is open (WP23), Phase 5 runs in parallel (`CLAUDE.md:3-6`). Branch `phase-6`
>   was cut from `phase-5` at `b5ff50c`. Fixes flow **phase-4 → phase-5 → phase-6** by merge-forward, never back.
>   **Implementation starts only on the owner's go for Phase 6.** "Starts now" in §12 means: needs no further owner input
>   or phone once that go is given.
> - **Standing rules:** product name only from `brand/brand.toml` (also for the APK's label, icon and id); MIT OR
>   Apache-2.0; permissive deps only (Gradle/AndroidX deps checked like `cargo-deny`); every upstream change a carried
>   patch with a `patches/REGISTER.md` row; never hand-edit `upstream/`; new UI strings through the four catalogs, Uzbek rows
>   `reviewed:agent-<model>:<date>`; HRM test environment only; never read or copy `corpus/`.

| | |
|---|---|
| **Status** | Plan only, written 2026-11-01 on branch `phase-6` (from `phase-5` `b5ff50c`). No product code, no builds, no spikes run for it. |
| **Inputs** | `docs/BRIEF.md` Phase 6 (:417-443); `docs/phase0/mobile-a0.md`, `mobile-a-kill.md`, `mobile-b.md`; ADR §2.2.1 (:190-262); `docs/phase1/SIZING.md` §2.5 (:283-315); `docs/phase4/PLAN.md` §9 (:467-491), R12 (:990); `docs/phase4/mobile-evidence.md`; `docs/phase4/gate-review.md:464`, §10 item 5; `docs/phase4/a11y-baseline.md` A1/A2/A10 (:36-45); `docs/phase5/PLAN.md` (template, H11 :486); `docs/phase0/hrm-current-state.md` §4.4 (:302-319); `patches/REGISTER.md`; code under `upstream/` (see note). |
| **Exit gate (brief)** | (1) The touch checklist passes on Android, **with recordings** (device **or emulator**, `BRIEF.md:424-430`). (2) iOS passes the same checklist **or is explicitly deferred by the owner**. |
| **Estimate** | **Docs-scope gate path 33.5 stream-days likely (22.5–53.5)**, plus 9 (5.5–15) beside it; Sheets/Slides/PDF phone work another 22 (15–37) if the owner puts them in Phase 6 (§13.1). |

**Citation note.** `upstream/<repo>` in this repo is the **pristine snapshot with our queue applied** (the Q commit,
`docs/UPSTREAM.md:10-11`), so `upstream/...:N` lines below are the code as built today, carried patches included.
eframe/egui are cited from the cargo registry copy `eframe-0.36.2` / `egui-0.36.2` (the versions in the lock).

How to use this plan. §0 holds the facts found while planning. §1 is the B-3 pack for the owner. §2–§8 are the designs
(handles §4, phone layout §5, keyboard §6, M3 §7, accessibility §8). §9 is the patch and size budget. §10 says what the
emulator can and cannot prove. §11 holds the work packages; §12 the order; §13 estimate, risks, gate map, owner decisions.

---

## 0. New facts found while planning

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **A long press fires twice.** Patch 0004's touch handling turns it into a word select (`clicks = 2`), and egui flags the same press `LONG_TOUCHED`, which `secondary_clicked()` reports, so the **desktop** context menu opens too. | `upstream/wordcraft/crates/ui-egui/src/canvas.rs:2443`; `touch_scroll.rs:106`; `egui-0.36.2/src/response.rs:211-213`; menu `canvas.rs:1669-1738`; seen on the emulator (`mobile-a0.md:48`) | The long-press menu is a routing change on an existing path (§4.5), not new input plumbing. |
| F2 | **Handles need no engine change.** `select.range {anchor, focus}` exists; screen→page is `page_at`; page→position is `layout.hit`; position→caret box is `caret_on`. | `upstream/wordcraft/crates/engine/src/cmd/caret.rs:54`, `:305-310`; `canvas.rs:2228-2233`; `crates/layout/src/hit.rs:269`, `:440` | The handle code is UI only: one new file in `ui-egui` plus call lines (§4). |
| F3 | **Word's mini toolbar already has the placement rules an action bar needs** (above the selection, clamped, flips below, hidden over dialogs/menus/cells). | `upstream/wordcraft/crates/ui-egui/src/mini_toolbar.rs:50`, `:73`, `:106-141`, `:166-200` | The long-press action bar reuses `visible_anchor` / `anchor_rect` / the `visible` rules (§4.5). |
| F4 | **The web text agent is a 1 px `<input type=text autocapitalize=off>` parked at the caret.** It knows only text typed since its last clear. The canvas feeds it the caret through `IMEOutput`; `ime_reset` → `should_interrupt_composition` → the agent is cleared. | `eframe-0.36.2/src/web/text_agent.rs:25-47`, `:228-277`, `:284-342`; `canvas.rs:1488`, `:1556-1566`; `eframe-0.36.2/src/web/app_runner.rs:415-427`; `keys.rs:596-606` | Every selection change made by a handle must set `ime_reset`, or the next Gboard correction (`DeleteSurrounding`) counts from stale text and erases the wrong characters (§6). |
| F5 | **eframe re-focuses the text agent on every `touchend` while it has focus** (an iOS keyboard workaround). On Android this re-raises a soft keyboard the user hid. | `eframe-0.36.2/src/web/events.rs:797-804` | Most likely root of "the soft keyboard reappears after a swipe" (`STATUS.md:309`), which is **eframe, not wordcraft**. Not provable here without a run; WP7 tests it and the fix candidates (§6). |
| F6 | **eframe's web runner drops AccessKit output** (`accesskit_update: _, // not currently implemented`), while wordcraft builds a full AccessKit document tree every frame a reader asks. | `eframe-0.36.2/src/web/app_runner.rs:394`; `upstream/wordcraft/crates/ui-egui/src/a11y.rs:1-17`, call `canvas.rs:1649` | A1 is an **eframe-web gap**; the tree exists. Options in §8. |
| F7 | **Web Layout reflows text to the window width**, and **Focus mode** already keeps only the title bar and the page. | `upstream/wordcraft/crates/engine/src/cmd/view.rs:26`; `canvas.rs:515-521`; `lib.rs:2255-2279` | The phone layout is a Focus-like branch plus Web Layout by default: mostly our code behind one call-site branch (§5). Text is readable at 360 dp without zooming a whole A4 page. |
| F8 | **Upstream handles `DeleteSurrounding` itself since v0.9.0**; our 0003 was dropped at the 2026-10-22 sync. | `keys.rs:366`, `:620`; `patches/REGISTER.md:146` | Autocorrect on Gboard must be re-checked on the current build (REGISTER asks for it) — WP7. |
| F9 | **The other three engines differ on touch:** Sheets pans with one finger, PDF has pinch, Slides only reads touch pressure. | `upstream/gridcraft/crates/ui-egui/src/grid.rs:647-659`; `upstream/pdfcraft/crates/ui-egui/src/motion.rs:157-166`; `upstream/deckcraft/crates/ui-egui/src/canvas.rs:1001-1010` | Per-app phone work is not a copy of Docs'; Sheets/Slides/PDF are separate WPs with their own scope decision (O4). |
| F10 | **The HRM mobile app embeds the editor as inline HTML in a WebView with `baseUrl` = the document server**, and only clients that send `editor=auto` get our editor. HRB forces mobile to view only for ONLYOFFICE. | `docs/phase0/hrm-current-state.md:315-319`; `docs/phase0/hr-backend-prompt.md:493-495`, `:509`; unsigned `type:"mobile"` ignored by our docserver (`docs/phase5/hrb-handoff.md:102`) | Task 6 needs a **change in the HRM mobile app** (MOB), not only ours. The inline-HTML origin vs our `frame-ancestors` is untested (WP4). |
| F11 | **Bundle headroom is under 10 KB for Docs and Slides.** | Docs 10,005 B, Sheets 14,246 B, Slides 9,377 B, PDF 30,230 B, shell 9,901 B (`STATUS.md:1535-1536`) | Phone code will exceed it. Re-baseline per the `web/budgets.toml` rule (measured + 10 %, STATUS line) in each WP; no 360 dp code in the wasm without a measured delta (§9). |
| F12 | **`canvas.rs` is a hot file:** wordcraft 0004 conflicted in the Phase 4 sync dry run (565 upstream commits in 2 days). | `docs/phase4/gate-review.md:422`, `:430-436` | New mobile code goes into new files; hot files get call lines only (§9). |
| F13 | **The Android toolchain works on this PC**, and a 40-line WebView APK already served a wasm dist from its assets. The scaffold's Gradle wrapper came from a repo with **no licence**. | `mobile-a-kill.md:30-31`; `mobile-b.md:103-108`, `:143` | The app (WP8) is written from scratch with a freshly generated wrapper; nothing from `spikes/mobile-*/android*` ships. |
| F14 | **The gate allows the emulator.** "each item verified on a device **or emulator** with a screen recording". Only task 4 (performance on a mid-range device) names a device. | `docs/BRIEF.md:424-430`, `:432-433` | The touch checklist can pass here (§10). The phone (B-10) is needed for task 4 and for closing B-3 unconditionally. |
| F15 | **The emulator rig exists and is current:** AVDs `HRM_Dev_33`, `HRM_Pixel_8` (API 36, 4 GB, 412×915 dp), emulator 37.1.11, release dists in `E:/office-target/p1/dist`. Chrome on it renders only with `-gpu swiftshader_indirect`; stock Chrome runs our editors since patch 0002. | checked 2026-11-01 (`~/.android/avd`, `emulator -version`); `mobile-a0.md:10-16`; `STATUS.md:294-300` | WP0/WP1 can start at once. `-gpu host` is unusable for Chrome on this PC (`mobile-a0.md:11`). |

---

## 1. The B-3 re-decision pack (completed)

### 1.1 Options and evidence

| | **A-web, mobile browser / PWA** | **A-web in a thin Android WebView app** (and the HRM app's WebView) | **B hybrid** (bind crate + own web UI) | **A-native** (eframe on Android) |
|---|---|---|---|---|
| What it is | The same four wasm editors and shell, served by `suite-serve`/docserver, opened in Chrome | The same dists, from APK assets (local mode) or from the docserver (HRM embed) | `domains/docs/crates/docs-bind` (281 lines) + a phone UI written from scratch | egui on `GameActivity`, own APK |
| Keyboard (Latin, Cyrillic, `oʻgʻil`, Backspace, Enter) | PASS (`mobile-a0.md:39-45`) | PASS in WebView for B's page (`mobile-b.md:106`); our editors in WebView **not run** | PASS (`mobile-b.md:67-74`) | PASS only with a 170-line shim (`mobile-a-kill.md:10-15`) |
| Autocorrect | PASS with 0003, now upstream (F8); **re-check** | same | PASS, no patch | PASS (shim) |
| One-finger scroll + momentum | PASS with 0004 (`STATUS.md:302`) | same | PASS, native | not tested |
| Handles + long-press menu | PARTIAL: word selected, desktop menu (F1) | same | PARTIAL: no handles (canvas, no DOM text) (`mobile-b.md:76`) | not tested |
| 360 dp | **FAIL**: ribbon clips and overlaps in all four editors (`mobile-evidence.md:51-68`) | same | PASS (4-button page) | not tested |
| App switch / context loss | PASS short absences with 0005 (ADR :209) | WebView: not run for our editors | PASS (Canvas2D) | Activity-destroy ANR twice (`mobile-a-kill.md:121-123`) |
| HRM embed + save | PASS in Chrome vs fake HRM (`mobile-a0.md:51`) | **not run** in Android WebView or the Expo app (ADR :210) | not built | none (no WebView embed) |
| Large document | M3: 108-page docx never opened on the emulator in 240 s; on desktop one synchronous layout task, 51 s at 20× (`mobile-evidence.md:70-136`) | same | not measured | not measured |
| Renderer memory | 284–326 MB PSS (flags on, `mobile-a0.md:56`); wasm memory 114–176 MB (`mobile-evidence.md:28`, `:82`) | ~same plus app | 148–226 MB PSS (`mobile-b.md:97`) | 54–58 MB, **no document** |
| Wasm (brotli) | Docs 9.86 MB (`STATUS.md:1470`) | same; APK ≈ dists deflated (unmeasured) | 5.8 MB (`mobile-b.md:89`) | `.so` 10.3 MB |
| UI reuse | all 629 commands, every dialog | same | 4 buttons; whole phone UI to build | same as A-web |
| Remaining cost | phone layout, handles, M3, keyboard: **sized here** at ~25 stream-days for Docs (WP2, WP5, WP6, WP7) | + the app shell (WP8, 4 d) | **7–11 engineer-months, Docs only**; Sheets blocked (no egui-free grid renderer) (ADR :218) | A-web's work + ANR + wgpu Vulkan-limit failure (`mobile-a-kill.md:47`) |

### 1.2 What the emulator can and cannot prove

| Can prove here (x86_64 AVD, SwiftShader, Gboard 15.1) | Cannot prove here |
|---|---|
| Every **functional** touch item: handles, long-press menu, Gboard Latin/Cyrillic/Uzbek, autocorrect, glide (by console injection, `mobile-a-kill.md:74-76`), pinch, momentum, rotation, 360 dp, overflow, Save/HRM round trip, app-switch and context-loss recovery, WebView embedding, **with `adb shell screenrecord` recordings** | **Performance** on an ARM mid-range SoC (start-up, memory pressure, scroll FPS, thermal): the AVD uses this PC's CPU and software GL, so CPU numbers flatter a phone and frame times understate a real GPU |
| The M3 open on stock Chrome (the Phase 4 optional re-run, `mobile-evidence.md:141-145`) | Real GPU/WebGL drivers (Mali/Adreno limits, context loss frequency); `-gpu host` is unusable for Chrome here |
| The HRM-app embed **shape** (inline HTML + `baseUrl` in an Android WebView, F10) | The real Expo build of the HRM app on a phone, and OEM keyboards (Samsung, SwiftKey) |
| Memory **ratios** between options | Whether a 3–4 GB phone kills a ~300 MB renderer in the background |

### 1.3 Recommendation (for the owner, two lines)

**Confirm A-web for v1:** the same wasm editors in the HRM app's WebView and in our own thin Android WebView app (local mode,
no login); B's bind crate stays compiling in CI as the fallback; A-native stays rejected. **The condition left** is a real-phone
run (performance + M3), which closes B-3 unconditionally when a phone arrives; until then the gate runs on the emulator, which the brief allows.

Why, in one trade-off: the ADR's condition was that A-web's phone layout and handles must not become a large carried patch
(ADR :234-240). This plan shows both fit in **new files plus about 5 call lines in two hot files** (§4, §5, §9), because the
engine already has `select.range`, Web Layout, Focus mode and the mini-toolbar geometry (F2, F3, F7). B costs 7–11
engineer-months for Docs alone and blocks Sheets; nothing measured so far is a defect only B fixes (handles are missing in B too).

### 1.4 What a real phone would change
- **Closes B-3** (removes "conditionally") and answers brief task 4 (startup, memory, FPS).
- Confirms or refutes M3 at phone speed after WP2's fix, and whether the renderer survives in the background (A0's GPU
  crashes may be emulator-only, `mobile-a0.md:58-60`).
- **Switch to B** (ADR :251-257) only if, on the phone: A-web fails a checklist item B passes; or the renderer is killed or
  unusable on memory/context loss; or WP5+WP6 needed more than ~60 lines in hot files (tripwire measured in §9).

---

## 2. Shape

```
                 one build of the four web editors + shell (scripts/build.sh <d> web --release)
                     │                         │                                 │
   (a) Chrome / PWA  │   (b) our Android app   │                (c) HRM mobile app (Expo, MOB)
   suite-serve or    │   packaging/android:    │                inline HTML in react-native-webview,
   docserver URL     │   WebView + asset loader│                baseUrl = docserver ─▶ /ds/api.js ─▶ /ds/editor
                     │   serves dists locally, │                ─▶ /docs/?host=parent (Phase 5 bridge, callbacks)
                     │   local mode, no login; │
                     │   VIEW/SEND intents ─▶  │
                     │   shell bridge open/save│
```

- **One editor code path for all three.** Phone behaviour switches on **viewport and pointer** inside the wasm (§5.1), not on
  the container, so (a), (b) and (c) need no separate builds.
- (b) is the brief's "Android app" (task 1: open from the device, share sheet; save). (c) is task 6. (a) is free and is the
  test path for the emulator rig.
- **No PWA work is planned** (service worker, manifest): (b) covers offline and file open. A PWA can be added later if the
  owner wants Chrome-installable editors.

---

## 3. Design decisions (options, recommendation, deciding trade-off)

| # | Question | Options | Recommendation | Why |
|---|---|---|---|---|
| D1 | Mobile approach (B-3) | §1.1 | **A-web** in (a)/(b)/(c) | §1.3 |
| D2 | Where the handle code lives | in `canvas.rs` / a new `ui-egui` file with call lines / in `docs-office` via a hook | **New file `ui-egui/src/touch_select.rs`** (patch wordcraft/0139) + ~4 call lines in `canvas.rs` | Needs crate-private canvas state (`app.canvas.touch`, rects, layout) that a hook would have to export; a new file never conflicts (F12), as `touch_scroll.rs` shows. |
| D3 | Where the phone chrome lives | patch `chrome.rs`/`ribbon.rs` / our `docs-office` behind one hook | **`domains/docs/crates/docs-office`** (our crate) behind one branch in `lib.rs:2274` through `office_overlay` (patch wordcraft/0140) | The 0122 ribbon hook pattern already carries clean (`gate-review.md:434`). Upstream's title bar overlaps at 360 dp (`mobile-evidence.md:57-59`), so phone mode draws its own top row instead of patching it. |
| D4 | Default view on a phone | Print Layout at fit width / Web Layout / Read Mode | **Web Layout** on entering phone mode; Print Layout one tap away | A4 at 360 dp shows 12 pt text at about 7 px; Web Layout reflows (F7). Read Mode is view-only. Saved files are unaffected (view setting). |
| D5 | Phone-mode trigger | user agent / container flag / viewport + pointer | **CSS width ≤ 600 px and the last pointer was a touch** (egui `Event::Touch`, as 0004 already detects) | Same build everywhere (§2); a desktop window made narrow keeps the desktop UI unless it is touched. |
| D6 | Large-document open (M3) | §7 options | **Measure first (WP2 step 1)**, then the cheapest fix that meets the target | The split between shaping and pagination is unknown; the fix differs by an order of magnitude in size. |
| D7 | Android app shell | Tauri / plain WebView / Capacitor-style framework | **Plain Android WebView + `androidx.webkit.WebViewAssetLoader`**, written from scratch | Measured in B5 (`mobile-b.md:103-108`); Apache-2.0 deps only; no Rust in the APK; Tauri never tried here. |
| D8 | Accessibility minimum (A1) | §8 | **DOM command palette + keyboard exit (A2)**, Docs first | §8 |

---

## 4. Selection handles and the long-press menu (the missing design, `mobile-evidence.md:157`)

### 4.1 Where the input lives today (web)
1. A finger produces, in eframe's web runner, `PointerButton` + `PointerMoved` + `Event::Touch` (start/move/end)
   (`eframe-0.36.2/src/web/events.rs:697-806`). The canvas has `touch-action: none`
   (`upstream/wordcraft/apps/wordcraft-web/index.html:11`), so Chrome starts no pan of its own.
2. Patch 0004's `TouchScroll` holds every finger press until it is a tap, a scroll (> 8 pt, `touch_scroll.rs:19`) or a
   long press (`max_click_duration`, egui default 0.8 s) (`touch_scroll.rs:50-112`). `mouse()` takes the held press
   (`canvas.rs:2324-2333`) and runs the normal mouse path; a long press counts as a double-click (word) (`canvas.rs:2443`).
3. Focus: a click or a non-scroll drag focuses the canvas (`canvas.rs:1634`); a focused canvas emits `IMEOutput` at the
   caret (`canvas.rs:1556-1566`); eframe then focuses the text agent, which raises the soft keyboard (F4, F5).
4. The selection is painted as rectangles per page (`canvas.rs:1438-1466`); `mini_anchor` is computed from the visible ones
   (`canvas.rs:1470`).

### 4.2 Geometry
- **Two handles** for a text selection, **one caret handle** after a tap on text (Android behaviour; hidden after 4 s idle or
  on typing).
- Anchor points come from `layout.caret_on(&sel.anchor)` and `caret_on(&sel.focus)` (`hit.rs:440`), **not** from the
  rectangles, so RTL lines, table cells and page breaks put the handle at the real end. Screen position = page rect +
  `page_screen_scale` (as `canvas.rs:1529-1534`).
- Drawn: a stem from the line's bottom and a 22 pt teardrop knob (start handle points right-up, end handle left-up, caret
  handle centred), in the theme accent; painted after the selection, on an `Order::Middle` area clipped to the document
  viewport (the mini toolbar's layer, `mini_toolbar.rs:184`) so it sits above the page and below popups.
- **Hit target 48×48 pt** centred on the knob (Android 48 dp; WCAG 2.5.8; a11y A10 `a11y-baseline.md:45`). When two
  targets overlap (a short selection) the knob centre nearer the touch wins; a tie goes to the focus end.

### 4.3 Drag behaviour
- **Routing:** `touch_select::pointer(...)` runs at the top of `mouse()` **before** `TouchScroll::hold_press`
  (`canvas.rs:2324`). A press inside a handle target is consumed: no scroll, no word select, no focus change (the
  `focus_click` test at `canvas.rs:1634` gets one more `&& !touch_select::dragging()`).
- **Offset:** on press, keep `grab = knob_centre − finger`. Each frame, the hit point is `finger + grab − (0, line_height/2 +
  knob_radius)`, so the finger never covers the text it is choosing.
- **Mapping:** hit point → `page_at` (`canvas.rs:2228`) → `layout.hit(page, x, y, story)` (`hit.rs:269`) → run
  `select.range {anchor: <the fixed end>, focus: <hit>}` (`cmd/caret.rs:54`). The dragged end is always written as
  `focus`, so the caret box, `IMEOutput` and "scroll into view" follow the finger. Handles may cross (the selection flips).
- **Granularity:** characters in v1 (Android's word-snap when growing is a later refinement).
- **Edge auto-scroll:** reuse `edge_scroll` (`canvas.rs:2297-2311`) while the finger is within its margin of the viewport.
- **Caret handle:** drag runs `caret.set {pos}` (`cmd/caret.rs:50`).
- **On release:** set `app.canvas.ime_reset = true` (F4, §6). Show the action bar (§4.5).
- **Scope v1:** text selections only. No handles when an object is selected or whole cells are (`objects::selected`,
  `session.cell_range()`; the exclusions of `canvas.rs:1442` and `mini_toolbar.rs:131`). Table and picture handles keep their own hit
  areas; making them finger-sized is listed as a known gap (SIZING 6.1 "finger-drag of objects").
- **Magnifier:** not in v1 (not in the brief's checklist).
- **Mouse unchanged:** nothing is drawn or routed unless the last pointer was a touch (D5).

### 4.4 Long press
- Keep 0004's behaviour: long press on text selects the word and shows both handles; long press then drag (no lift) still
  extends by words through `unit_drag`, as with a mouse double-click drag.
- Long press on an empty area or a selected object: no word select; the action bar opens with Paste / Select all.

### 4.5 The long-press menu (action bar)
- **Routing (F1):** in `canvas.rs` the menu opens on `secondary_clicked` (`:1669`, `:1726-1738`). With a touch, a
  `resp.long_touched()` (egui `response.rs:219-221`) opens **our action bar instead** of the desktop popup; a right-click
  keeps the popup. One condition in the existing `open` expression.
- **Content:** Cut, Copy, Paste, Select all, ⋯ (overflow). ⋯ opens the existing `context_menu` body (`canvas.rs:3124`), so
  spelling suggestions, Synonyms and our Transliterate entries (patch 0121) stay reachable; on a phone that popup is shown with
  48 pt rows (egui `spacing.interact_size`) at the bar's position.
- **Placement:** `mini_toolbar::visible_anchor` + `anchor_rect` (`mini_toolbar.rs:50`, `:73`): above the selection, flips
  below, never outside the viewport. Hidden while a handle is dragged, re-shown on release (Android behaviour).
- **The formatting mini toolbar is hidden in phone mode** (its 24 pt controls fail touch size; the bottom toolbar carries
  formatting, §5).
- **Labels** go through the catalogs (`tl!`); Cut/Copy/Paste/Select all are upstream catalog keys already used by
  `context_menu` (`canvas.rs:3160-3162`), so only "⋯ More" may be new.
- **Clipboard on web:** Paste from a button goes through `system_clipboard::fill` (`ui-egui/src/system_clipboard.rs:24-30`)
  and, on Android Chrome, the async Clipboard API permission. **UNVERIFIED** on the emulator; a WP5 test item.

### 4.6 Coexistence with the keyboard, IME and Cyrillic
- Handles never take or drop focus, so the keyboard state is what it was before the drag (but see F5: eframe may re-raise it
  on `touchend`; WP7 owns that).
- `ime_reset` on release clears the text agent (`app_runner.rs:417-420`), so Gboard's next autocorrect or glide commit
  starts from the new caret. Without it, `DeleteSurrounding{before_chars:n}` (`text_agent.rs:304-309`) would erase n
  characters before the **new** caret.
- Typing over a selection made with handles is the normal `Text`/`Ime::Commit` path (`keys.rs`); Cyrillic and Uzbek
  (U+02BB/U+02BC, `mobile-a0.md:40-41`) need nothing special: hit testing is script-agnostic and the IME rect is the caret box.
- A composition in progress (`ime_preedit` non-empty) blocks handle drags until it commits (`canvas.rs:1488` already refuses
  `ime_reset` during a preedit).

### 4.7 Tests (kittest, synthetic touch events as in `touch_scroll.rs` tests)
Press on a handle does not scroll and does not move the caret; drag of the focus handle extends `sel.focus`; crossing flips;
release sets `ime_reset`; long touch opens the action bar and **not** the desktop popup; right-click still opens the popup;
mouse drag still selects with no handle drawn; no handle for `cell_range()` or a selected object.

---

## 5. Phone layout and the contextual bottom toolbar (Docs)

### 5.1 Trigger and layout
- Phone mode = D5. Entering it: Web Layout (D4), rulers and side panes hidden, status bar hidden. Leaving it restores the
  user's view. A setting can force it on or off.
- `lib.rs:2274-2279` gets one branch: `if office_overlay::phone(self, &ctx) { office_overlay::phone_chrome(self, ui) } else`
  the existing Focus/ribbon code. `phone_chrome` lives in `docs-office` (our crate, D3) and draws:
  - **Top row (48 pt):** Home/Back (the shell's), the file name **truncated** (never overlapping, `mobile-evidence.md:57-59`),
    Undo, Redo, a saved/dirty dot, ⋯ (File: Save, Save as, Print where allowed by the bridge flags of 0128, Find, View,
    Language, Keyboard).
  - **Bottom toolbar (48 pt, at most 5 slots + overflow):** contextual, chosen from the selection state that
    `format.state` and `session.cell_range()`/`objects::selected` already report (`mini_toolbar.rs:204`):
    - caret in text: Format (Aa sheet: bold, italic, underline, size, colour, highlight, styles), List, Insert (table, picture,
      page break), Keyboard show/hide, ⋯
    - text selected: Bold, Italic, Underline, Highlight, ⋯
    - in a table: Insert row, Insert column, Delete, Table ⋯
    - picture selected: Wrap, Size, Delete, ⋯
  - **Sheets** (bottom sheets ≤ 60 % of height) replace the ribbon's groups; each control runs a command id from the Phase 4
    preset (`domains/docs/crates/docs-office/src/preset.rs`), so no new engine command.
- **Above the keyboard:** the bottom toolbar sits at the bottom of the canvas, which shrinks with the keyboard only if the
  top-level page resizes (§6.3).
- **Dialogs:** every dialog reachable from the phone chrome is listed in WP6 and either fits 360 pt or is shown full-screen;
  unreachable desktop dialogs are out of scope.
- **Strings:** new labels in `i18n/` (en, ru, uz-Latn, uz-Cyrl), Uzbek `reviewed:agent-<model>:<date>`.
- **Icons:** bitmaps as in Phase 4 (`docs/phase4/icons-wasm.md`; the SVG loader costs +309 KB brotli).

### 5.2 The other editors
Sheets (finger pan exists, F9) needs a phone row for the formula bar and cell editing with the soft keyboard; PDF needs its two
rails collapsed into a sheet (`mobile-evidence.md:60`); Slides needs an insert-text-box path before typing is even testable
(`mobile-evidence.md:45`). Each is a WP10 sub-package behind owner decision O4.

---

## 6. Keyboard, IME and viewport

1. **Re-run the A0 keyboard matrix on HEAD** (Latin, Cyrillic with `ё`, `oʻgʻil` with U+02BB/U+02BC, autocorrect `wolrd `,
   glide, Backspace, Enter), because 0003 was replaced by upstream code (F8) and the matrix last ran on the Phase 0 tree.
2. **Keyboard reappears after a swipe (F5).** Candidates, cheapest first, chosen by a recorded run:
   (a) after a scroll gesture, if the keyboard was hidden before it (visual viewport height unchanged), drop egui focus on the
   canvas so eframe blurs the agent (`app_runner.rs:421-425`); a tap focuses again. (b) set
   `virtualkeyboardpolicy="manual"` on eframe's text agent from our page script and call
   `navigator.virtualKeyboard.show()` on a tap in text (Chrome's VirtualKeyboard API; **support in Android WebView
   UNVERIFIED**). (c) a carried `[patch.crates-io]` of eframe: last resort (a crates.io fork is not in our patch model).
3. **Caret above the keyboard** (not re-tested since Phase 0, `STATUS.md:313`): the top-level page must resize with the
   keyboard. Our pages get `interactive-widget=resizes-content` in the viewport meta (`web/shell/index.html:5`,
   `services/docserver/static/editor.html:5`; Chrome-only); our APK uses `windowSoftInputMode=adjustResize`; the HRM app's
   setting is MOB's (WP9 prompt). Then check that the canvas scrolls the caret into view on resize.
4. **Single-line agent:** the text agent is `type=text` (`text_agent.rs:29`), yet Enter made a paragraph in A0
   (`mobile-a0.md:45`). Keep as a regression item, not a change.

---

## 7. M3: the large-document open stall

**Known** (`mobile-evidence.md:70-136`): on desktop the 108-page docx (`domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx`,
not corpus) opens in one synchronous main-thread task: `Session::layout()` (`upstream/wordcraft/crates/engine/src/session.rs:812`)
→ `wordcraft_layout::layout` (`crates/layout/src/lib.rs:2124-2155`), which lays out the whole document and repeats passes for
page totals (`MAX_TOTAL_PASSES = 3`, `:678`) and per-page footnotes (`MAX_NOTE_PASSES = 3`, `:2120`). 1.0 s at 1×, 7.4 s at 6×,
17 s at 20×; the emulator never reached `wordcraft:opened` (posted after the first frame, `apps/wordcraft-web/src/web/host.rs:155-158`).
**Unknown:** the emulator's own numbers, and how the ~0.6 s of document cost splits into shaping (cached in `LayoutCache.paras`,
`lib.rs:527-535`) versus pagination versus repeated passes.

**Step 1, measure (emulator and desktop, no code in the product):**
- Add `--cdp` to `scripts/mobile-m3.mjs` and run it against stock Chrome on `HRM_Pixel_8` (`adb forward
  localabstract:chrome_devtools_remote`): `ready`, `opened`, longest task, `logcat` for a renderer kill; the 1-page control
  alongside (the decision table is `mobile-evidence.md:144-145`).
- A native bench (test code in `domains/docs`, using the engine like `docs-bind` does) on the same fixture: total layout ms,
  passes taken, and a second layout with a warm `LayoutCache` (= pagination-only cost). Also the same in **Web Layout** (D4).

**Step 2, fix (pick by step 1):**

| Option | Change | Size | When |
|---|---|---|---|
| (a) Never a grey page | The shell paints "Opening…" with the file name until `opened`; long-lived cache headers for the wasm (`mobile-evidence.md:137`) | our JS, no patch | always |
| (b) Phone opens in Web Layout | D4 at open time, if step 1 shows Web Layout is cheaper (no page-total passes, `lib.rs:2129-2130` limits note passes to Print) | none beyond §5 | if cheaper |
| (c) Warm the cache in slices | Shape paragraphs into `LayoutCache` in 8–10 ms slices over frames, then run the (now pagination-only) full layout | carried patch in `crates/layout` + a call in the web app; ~100–200 lines (ESTIMATE) | if shaping is ≥ 2/3 of the cost |
| (d) First screen first | `layout()` with a page budget returns pages 1–3 flagged partial, posts `opened`, continues in sliced frames; totals and note passes only at the end; "Page 1 of about N" | carried patch in `crates/layout` + `crates/engine`, larger; resumable pagination is unproven (`mobile-evidence.md:130-135`) | if pagination dominates |
| (e) Layout in a Web Worker | engine in a worker | large; needs a second wasm instance | rejected for Phase 6 |

**Done-check (targets):** desktop Chromium at 6× CPU: 108-page `opened` ≤ 1.5× the 1-page control's `opened`, and no main-thread
task > 200 ms in the 10 s after `opened` without input; on the emulator: the 108-page file shows page 1 and a tap places the
caret, recorded. Re-measure on a phone when one exists (WP14).

---

## 8. A1 accessibility decision (web editors invisible to screen readers)

| Option | What | Cost | Covers |
|---|---|---|---|
| (a) AccessKit web adapter | turn the AccessKit tree wordcraft already builds (F6) into a hidden DOM | eframe has none (`app_runner.rs:394`); no adapter found in the local registry; writing one is a generic, large piece, and upstream egui is the right home | the document text, caret, all widgets — full A1 |
| (b) **DOM command palette + keyboard exit** | the host page (shell, docserver wrapper, APK page) renders an accessible `<dialog>` combobox; the editor posts its command list (id, translated label, enabled, shortcut) on request and runs `<app>:run {id}`; a chosen key posts `<app>:focusOut` (fixes A2, `a11y-baseline.md:37`, which is still open) | a bridge patch per app (Docs first, wordcraft/0141) + our JS; ~3 d | every command by keyboard and by TalkBack/NVDA; **not** the document text |
| (c) Point screen-reader users to the desktop apps | document it | none | desktop only (UIA works there, `a11y-baseline.md:13-17`) |

**Recommendation: (b) now for Docs, (c) stated in the docs, (a) recorded as an upstream (egui) issue to file when upstream
PRs are allowed.** (b) is the minimum the Phase 4 review asked for ("at least a keyboard-accessible command palette first",
`a11y-baseline.md:36`) and it also gives TalkBack users on a phone a usable command list. Trade-off: the document body stays
unreadable to a screen reader on the web until (a).

---

## 9. Carried patches and bundle budget

| Patch (next free numbers) | Files | Hot-file lines (target) |
|---|---|---|
| wordcraft/0139 `touch-select` (§4) | new `crates/ui-egui/src/touch_select.rs`; `canvas.rs` (pointer call in `mouse()`, paint call after the selection, focus condition, long-touch routing); `lib.rs` `mod` line | ≤ 6 in `canvas.rs` |
| wordcraft/0140 `phone-chrome-hook` (§5) | `lib.rs:2274` branch; `office_overlay.rs` hook; code in `domains/docs/crates/docs-office` | ≤ 4 in `lib.rs` |
| wordcraft/0141 `bridge-commands-focusout` (§8) | `apps/wordcraft-web/src/web/host.rs` (patched file of 0001/0127/0128) | 0 hot (host.rs is ours) |
| wordcraft/0142 `layout-sliced-open` (§7 c or d, if chosen) | `crates/layout/src/lib.rs`, `crates/engine/src/session.rs`, web app | measured in WP2 |
| gridcraft/0155, deckcraft/0156, pdfcraft/0121 | phone rows per app (WP10b-d), only if O4 puts them in Phase 6 | per WP |

- Each patch: `patches/REGISTER.md` row, `scripts/verify-queue.sh <repo>`, `cargo xt register check`, and the sync rule
  (re-grep the call sites at each sync). 0004 and 0139 touch the same `mouse()` lines: rebase them together.
- **Tripwire for B-3 (§1.4):** if 0139 + 0140 need more than ~60 lines in `canvas.rs`/`lib.rs`, stop and report.
- **Size:** Docs has 10,005 B and Slides 9,377 B of headroom (F11). Expected Docs delta: handles 3–6 KB, phone chrome 8–15 KB,
  palette 2–4 KB (ESTIMATE, scaled from the five mobile patches' +9.3 KB brotli, `STATUS.md:305`). Each WP measures with
  `scripts/ci.sh sizes` and re-baselines `delta_br_max` by the `web/budgets.toml` rule (measured + 10 %, rounded to 16 KiB)
  with a STATUS line. No editor gets a budget raise without a measured delta.
- The Android app (WP8) is not a wasm and has no budget row; its APK size is recorded per build.

---

## 10. The emulator rig and its limits

- AVDs: `HRM_Pixel_8` (API 36, 412×915 dp, 4 GB) and a **new 360 dp AVD** (e.g. 1080×2400 at 480 dpi) so the 360 dp items do
  not need `wm density`, which reloads the page and loses edits (`mobile-a0.md:50`). `-gpu swiftshader_indirect
  -no-snapshot-save`; Gboard with English, Русский, Oʻzbek (lotin) (`mobile-a-kill.md:32`).
- Never `adb shell input keyevent` (it flips Gboard into its hardware-keyboard bar, `mobile-a-kill.md:77-79`); Back as an
  edge swipe. Glide by emulator-console `event send` (`mobile-a-kill.md:74-76`).
- Recordings: `adb shell screenrecord` (3-minute cap per file), one file per checklist item, named
  `<app>-<item>-<dp>.mp4`, kept under `docs/phase6/evidence/` when small, else on `E:/office-target/p6/evidence/` with a
  SHA-256 list in the repo.
- RAM: emulator plus a wasm build fit in 31.8 GB; builds still wait for a free build slot (`E:/office-target/<repo>`).
- **Stated plainly:** no physical Android phone (B-10); no macOS, so no iOS simulator and no Safari test; Intel UHD 630, no
  discrete GPU, and `-gpu host` gives blank Chrome pages (`mobile-a0.md:11`).

---

## 11. Work packages

Each WP: goal, done-check, estimate (stream-days low/likely/high), tier, and **availability**:
**[now]** = emulator-verifiable now, no owner input; **[phone]** = needs a phone; **[owner]** = needs an owner decision.

### WP0. Kick-off and merge-forward  [now]
- **Goal:** record the merge-forward rule (phase-4 → phase-5 → phase-6) in STATUS; merge `phase-5` if it moved; baseline
  `scripts/ci.sh quick` and `sizes` on `phase-6`; boot both AVDs and record Chrome and Android System WebView versions;
  send §1.3 and §13.4 to the owner (Uzbek).
- **Done-check:** STATUS lines with the baseline numbers and versions.
- **Estimate:** 0.5 (0.5/0.5/1). **Tier:** builder.

### WP1. Emulator rig and checklist runner  [now]
- **Goal:** `scripts/mobile/` (our code; the Phase 0 harnesses are reference only): AVD start/stop, the 360 dp AVD, Gboard
  setup check, CDP forward, `screenrecord` per item, glide injector, `mobile-m3.mjs --cdp`, a checklist runner that writes a
  JSON + recordings per item.
- **Done-check:** one run on `HRM_Pixel_8` of the shell + Docs (open, type, save) with a recording and the JSON.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP2. M3 large-document open  [now]
- **Goal:** §7 step 1 (emulator + native split), then the chosen fix (a always; b/c/d by the numbers).
- **Done-check:** §7 targets; STATUS table of before/after; patch row if (c)/(d).
- **Estimate:** 4 (2.5/4/8). **Tier:** builder for the measurement; **architect** for the layout patch design.

### WP3. B-3 decision to the owner  [owner]
- **Goal:** §1 with WP2's emulator numbers and WP4's WebView result added; the owner's answer recorded in ADR App. C.
- **Done-check:** owner answer in STATUS and `CLAUDE.md` gate list.
- **Estimate:** 0.5 (0.5/0.5/1). **Tier:** architect.

### WP4. WebView embed proof (HRM-app shape)  [now]
- **Goal:** a test-only APK under `ci/android-harness/` that does what MOB does (inline HTML with `baseUrl` = docserver,
  `new DocsAPI.DocEditor`, F10) against the local docserver + Phase 5 fake HRM; open → type (Gboard) → save → status-6 callback →
  close; check `frame-ancestors` with that origin; check WebGL in the nested iframes of Android System WebView.
- **Done-check:** recorded run; callback log; the saved docx contains the typed Latin and Cyrillic text.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP5. Docs selection handles and long-press action bar  [now]
- **Goal:** §4 as patch wordcraft/0139.
- **Done-check:** §4.7 kittests; emulator recordings: select by long press, drag both handles across a line and a page break,
  Cut/Copy/Paste from the bar, type Cyrillic over the selection, autocorrect right after a handle move; mouse regression
  (existing ui-egui tests green); `sizes` delta recorded.
- **Estimate:** 5 (3/5/8). **Tier:** builder; architect review of the routing changes.

### WP6. Docs phone layout and contextual bottom toolbar  [now; owner review non-blocking]
- **Goal:** §5.1 as patch wordcraft/0140 + `docs-office` code + catalog rows.
- **Done-check:** at 360 and 412 dp: zero overlaps (screenshot check as `mobile-evidence.md` §2), every bottom-toolbar
  button ≥ 48 pt, every reachable dialog fits or is full-screen (list in `docs/phase6/phone-layout.md`); docs-office tests for
  the mode switch and the contextual sets; recordings; `sizes` delta. Toolbar sheet screenshots go to the owner like Phase 4 Q5.
- **Estimate:** 8 (5/8/12). **Tier:** builder; architect review of the hook.

### WP7. Keyboard, IME and viewport  [now]
- **Goal:** §6 items 1–3.
- **Done-check:** the A0 keyboard matrix on HEAD, recorded, each row with its evidence; "keyboard after swipe" fixed or its
  cause proven; caret stays above the keyboard while typing at the page bottom (recorded).
- **Estimate:** 3 (2/3/5). **Tier:** builder; architect for the keyboard-policy choice.

### WP8. Android app (thin WebView)  [now with a provisional id; owner for the final id]
- **Goal:** `packaging/android/`, written from scratch: one Activity, WebView + `WebViewAssetLoader` serving the shell and the
  four dists from assets (local mode, no login, no network permission unless server mode is configured); intent filters for
  VIEW (docx, xlsx, pptx, pdf, doc, odt …) and SEND (share sheet) → the shell bridge `open`; save back to the opened `content://`
  URI, or "Save as" through `ACTION_CREATE_DOCUMENT`, via a JS interface; edge-to-edge insets; `adjustResize`; app name, icon
  and id generated from `brand/brand.toml` (new `[android]` table, O5); our own signing key (`docs/phase4/PLAN.md:23`);
  a fresh Gradle wrapper; a licence check of every Gradle dependency.
- **Done-check:** APK installs on both AVDs; open a docx from Files and from a share, edit, save back, reopen (recorded);
  APK size per build in STATUS; dependency licence list in `docs/phase6/android-licences.md`.
- **Estimate:** 4 (3/4/6). **Tier:** builder.

### WP9. HRM mobile app embed  [owner + HRM team]
- **Goal:** `docs/phase6/hrm-mobile-prompt.md` (like `hr-backend-prompt.md`): MOB sends `editor=auto`, points the editor URL
  at the docserver, keeps ONLYOFFICE as the fallback, sets `adjustResize`; HRB's mobile view rule stays as is for us
  (`hr-backend-prompt.md:509`) subject to H11 (O7). Run it on the emulator with a MOB dev build when the owner allows.
- **Done-check:** WP4 green; with the owner's OK, a recorded MOB run on the emulator against the HRM test environment.
- **Estimate:** 2 (1/2/3). **Tier:** builder.

### WP10. Sheets, Slides, PDF on phones
- **WP10a  [now]:** run the A0 matrix on each (WP1 rig), record what passes and fails. 2 (1.5/2/3). builder.
- **WP10b Sheets / 10c Slides / 10d PDF  [owner, O4]:** phone rows and touch selection per §5.2 (SIZING 6.4–6.6: 9, 7, 6
  likely). Patches gridcraft/0155, deckcraft/0156, pdfcraft/0121. builder; architect for Sheets touch selection.

### WP11. Accessibility minimum (A1 option b, A2)  [now]
- **Goal:** §8 (b) for Docs: command list and run messages (wordcraft/0141), the DOM palette in `web/shell` and the docserver
  wrapper, a focus-out key handled by the shell.
- **Done-check:** Playwright: Tab reaches the palette, runs Bold on a selection, leaves the editor by keyboard
  (`ariaSnapshot` shows named options); TalkBack on the emulator reads the palette (recording); `a11y-baseline.md` A1/A2 updated.
- **Estimate:** 3 (2/3/5). **Tier:** builder.

### WP12. Lifecycle on Android  [now]
- **Goal:** app switch, rotation, background renderer kill (`am kill`), WebGL context loss, in Chrome and in the WP8 app; the
  0005/0127 autosave path delivers unsaved bytes to the APK host.
- **Done-check:** recordings; no lost edits in any case, or the loss window measured and stated.
- **Estimate:** 1.5 (1/1.5/2.5). **Tier:** builder.

### WP13. Checklist recordings (gate evidence)  [now]
- **Goal:** every brief checklist item (`BRIEF.md:424-433`) for Docs (and the apps in O4's scope), at 412 and 360 dp, in Chrome
  and in the WP8 app, with recordings; plus the bottom toolbar (task 3).
- **Done-check:** `docs/phase6/checklist.md`: one row per item × surface, PASS/FAIL, recording link.
- **Estimate:** 1.5 (1/1.5/2). **Tier:** builder.

### WP14. Performance on a mid-range phone  [phone]
- **Goal:** brief task 4: cold/warm start to `ready` and to `opened`, renderer PSS (`dumpsys meminfo`), scroll FPS (in-page rAF
  sampler + `dumpsys gfxinfo`), the M3 file; the same scripts on the emulator first, labelled indicative.
- **Done-check:** a STATUS table, phone model and Android/WebView versions named.
- **Estimate:** 2 (1/2/4). **Tier:** builder.

### WP15. iOS  [owner, O3]
- **Goal:** nothing unless the owner provides a Mac (or a cloud Mac) instead of deferring. **0 d if deferred.**

### WP16. Gate: checklist review, adversarial review, report  [now, at the end]
- **Goal:** §13.3 rows with evidence; an opus adversarial review of 0139–0142 and the APK; the Uzbek gate report.
- **Estimate:** 1.5 (1/1.5/2). **Tier:** architect.

---

## 12. Order, parallel streams, and what starts now

**Start immediately on the owner's Phase 6 go, with no further owner input and no phone:** WP0, then in parallel WP1, WP2
(step 1 needs WP1's `--cdp` only), WP4, WP5, WP7, WP11; then WP6 (after WP5 lands, same files), WP8, WP10a, WP12, WP13.

| Stream | Sequence | Waits on |
|---|---|---|
| A, Docs touch UI | WP5 → WP6 | nothing (owner review of WP6 sheets is non-blocking) |
| B, runtime | WP1 → WP2 → WP7 → WP12 | nothing |
| C, Android and embed | WP4 → WP8 → WP9 | O5 (final id) for WP8 release; O7/O8 + HRM team for WP9 |
| D, accessibility | WP11 | nothing |
| E, other editors | WP10a → WP10b-d | O4 |
| Gate | WP13 → WP16; WP3 after WP2+WP4 | O1; WP14 needs the phone (B-10) |

- **Critical path (Docs scope):** WP0 → WP5 → WP6 → WP13 → WP16, about 16.5 stream-days, with WP2 and WP8 beside it.
- **Merge-forward:** WP5, WP6, WP11 change `patches/wordcraft`, which Phase 4 and 5 also change. Merge `phase-5` before each,
  run `verify-queue.sh wordcraft` after.

---

## 13. Estimate, risks, gate mapping, owner decisions

### 13.1 Estimate (stream-days: low / likely / high)

| Group | WPs | Low | Likely | High |
|---|---|---|---|---|
| Gate path, Docs scope | WP0-WP8, WP12, WP13, WP16 | 22.5 | 33.5 | 53.5 |
| Beside the gate | WP9, WP10a, WP11, WP14 | 5.5 | 9 | 15 |
| **Total, Docs scope** | | **28** | **42.5** | **68.5** |
| Sheets, Slides, PDF (O4) | WP10b-d | 15 | 22 | 37 |
| iOS (O3) | WP15 | 0 (deferred) | 0 | 10 (with a Mac, SIZING) |

Against SIZING §2.5 (33.5/51/82 for four apps, `SIZING.md:308`): the Docs scope is higher than SIZING's Docs items (6.1–6.3
+ 6.7 + 6.9-6.12 ≈ 27 likely) by M3 (WP2, not in SIZING), the rig (WP1), the WebView proof (WP4) and accessibility (WP11).
Mobile sign-in (SIZING 6.8, 2 d) is out: accounts optional and SSO moved after the Phase 5 pilot gate. Risk buffer: 20 d (SIZING).

### 13.2 Risks

| # | Risk | Effect | Mitigation |
|---|---|---|---|
| **R1** | **No phone (B-10):** emulator results flatter the CPU and hide GPU/memory behaviour | the gate passes on the emulator, then the pilot is slow or crashes | B-3 stays "confirmed pending phone"; WP14 the day a phone exists; tripwires §1.4 |
| **R2** | **M3 fix is bigger than expected** (resumable pagination unproven) | large documents stay unusable on phones | measure first (WP2); (a)+(b) as the floor; (d) only with a design review |
| **R3** | **eframe behaviour we cannot patch in our model** (F5 keyboard re-raise, F6 AccessKit) | keyboard pops up on scroll; no screen reader on web | WP7 candidates (a)/(b) from our side; (c) a crates.io patch only with an owner OK |
| R4 | Hot-file conflicts at the next upstream sync (`canvas.rs`, `lib.rs`) | rebase time | new files + call lines only (§9); tripwire 60 lines |
| R5 | Bundle budget (F11) | CI red on `sizes` | measured re-baseline per rule, STATUS line per WP |
| R6 | HRM app change not done (MOB) | task 6 not provable on the real app | WP4 proves the shape; WP9 prompt ready; gate does not require task 6 (§13.3) |
| R7 | Android WebView differs from Chrome (VirtualKeyboard API, WebGL in nested iframes, clipboard) | items pass in Chrome and fail in the app | WP13 runs every item in both |
| R8 | Merge-forward conflicts in `patches/wordcraft` with Phase 4/5 work | rework | merge before each wordcraft WP; `verify-queue.sh` |
| R9 | Recording volume in git | repo growth | small files in repo, large ones on E: with SHA-256 list (§10) |

### 13.3 Exit-gate items → proving WP

| Gate item / brief task | Proof | WP |
|---|---|---|
| **Touch checklist on Android, with recordings** — handles + long-press menu | §4.7 tests + recordings | WP5, WP13 |
| — native keyboard: autocorrect, swipe typing, Cyrillic | A0 matrix on HEAD, recorded (glide by console injection) | WP7, WP13 |
| — pinch zoom, momentum scroll, rotation | recordings in Chrome and the app | WP13, WP12 |
| — 360 px, no overflow | 360 dp AVD screenshots + recordings, overlap check | WP6, WP13 |
| **iOS passes or is deferred** | owner's deferral recorded (O3) | WP15 |
| Task 1 Android app: open from device, share sheet, edit, save ("HRM links": O6) | recorded APK run | WP8 |
| Task 3 compact contextual bottom toolbar | WP6 screenshots + recordings | WP6 |
| Task 4 performance on a mid-range phone | **phone only**; emulator numbers labelled indicative | WP14 |
| Task 5 iOS app | deferred (O3) | WP15 |
| Task 6 HRM mobile app embed | WP4 shape on the emulator; real MOB with the HRM team | WP4, WP9 |

### 13.4 Owner decisions (each with a recommendation)

| # | Decision | Options | Recommendation |
|---|---|---|---|
| **O1** | **B-3, the mobile approach** | A-web / B / A-native | **A-web confirmed for v1** (HRM WebView + our thin WebView app), B's bind crate kept compiling, A-native rejected; the "conditional" part closes on a phone run (§1.3, §1.4). |
| **O2** | **A mid-range Android phone (B-10)** | provide / keep emulator only | **Provide one** (≈4 GB RAM, Android 13+, current Chrome/WebView), for WP14 and to close O1; the gate itself can pass on the emulator (F14). |
| **O3** | **iOS** | provide a Mac or cloud Mac / defer | **Defer iOS explicitly.** No Mac here; Safari/WKWebView behaviour is untested and the brief allows deferral. |
| **O4** | **Which editors the Phase 6 gate covers** | Docs only / all four | **Docs for the gate**; Sheets/Slides/PDF get the matrix (WP10a) and their phone rows beside or after the gate (−22 d likely off the gate, SIZING scope cut `:498`). |
| **O5** | **Android application id and app-link domain** (permanent once published, like the MSI UpgradeCode) | e.g. a reverse-DNS id under a domain the owner controls | **Choose it before WP8's first release build**; put it in `brand/brand.toml` `[android]`. A provisional id is used until then and never leaves this PC. |
| **O6** | **What "HRM links" means for our app** (brief task 1) | the HRM app opens documents (embed) / our app opens HRM URLs | **The HRM app handles HRM links** through the embed (WP9); our app opens files from the device and the share sheet. |
| **O7** | **H11: mobile editing of HR documents** once ONLYOFFICE is replaced (today forced to view) (`docs/phase5/PLAN.md:486`) | edit / view only | **Edit for the pilot users only**, behind the same pilot list as the web; view for everyone else. |
| **O8** | Who makes the HRM mobile app change, and when | HRM team / a Claude session on MOB / later | **After the Phase 5 pilot gate**, with the WP9 prompt, the same way as HRB PR-0 … PR-3 (Phase 5 Q3). |
| **O9** | **A1 accessibility** | §8 (a)/(b)/(c) | **(b) now for Docs** (palette + keyboard exit), (c) in the user docs, (a) as an upstream egui issue later. |
| O10 | APK distribution for the pilot | sideload / MDM / Play Store ($25 once) | **Sideload or MDM to IT staff only**; no store listing in Phase 6. |
