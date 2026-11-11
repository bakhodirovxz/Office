# Phase 6 touch checklist, Docs (WP13) and the WP5 done-check

Run 2026-11-11 on branch `p6-check` (from `phase-6` d1fe9c5b). Docs only (O4 not decided: Sheets, Slides, PDF only have the WP10a survey,
`other-editors.md`). BRIEF `docs/BRIEF.md:424-433`.

## Rig
- AVD `HRM_Pixel_8` (API 36, Chrome 133.0.6943.137, Android System WebView 133, Gboard 15.1) at `wm size 540x1200`, the same 360/412 dp as the
  rig's other runs (a quarter of the pixels; keyboard.md explains why): **412 dp = density 210 (DPR 1.3125), 360 dp = density 240 (DPR 1.5)**.
  Changing the density restarts the activity: the page is reopened each time.
- Page: `dist-final` of `p6-tapfix` (Docs wasm 9,875,600 B brotli: includes wordcraft/0139, 0140, 0143, 0149), served by `hoshiya-serve`,
  opened through `scripts/mobile/session.mjs`. App: `packaging/android/build.sh` over the same dist, 53,208,707 B, `app.hoshiya.suite`, opened by a VIEW intent
  with a write grant. Fixture `scripts/mobile/fixtures/wp13.docx` (synthetic, 3 pages, a wide 5-column table, a hard page break; `make-wp13.py`).
- Input: `adb shell input swipe` and the emulator console (`scripts/mobile/touch.mjs`: drag, press, two-finger pinch; Gboard glide and key taps as in WP7).
  Recordings and screenshots: `docs/phase6/evidence/checklist/` (small ones) and `E:/office-target/p6-check/rec/` (originals).
- **The host was saturated** (a release wasm build and a CI run: Chrome's `requestAnimationFrame` ran at about 12 Hz idle). Long presses had to be held 2.5 s;
  a real phone at 60 Hz will behave differently for timing-dependent gestures (long press, two fingers landing in one frame). No real phone exists.

Surfaces: **C412 / C360** Chrome at 412 / 360 dp; **A412 / A360** the Android app. `ref` = evidence from an earlier WP on the same code path, not re-run today.

## Table

| # | Item | Surface | Result | Evidence | Date |
|---|---|---|---|---|---|
| 1 | Long press selects the word, handles and the action bar (Cut, Copy, Paste, Select All, ...) show | C412 | PASS | `c412-01-longpress.png`, `chrome-412-handles.mp4` | 2026-11-11 |
| 1 | same | C360 | PASS (empty area: bar with Cut greyed, no word select, PLAN 4.4) | `c360-longpress.png`, `chrome-360-handles.mp4` | 2026-11-11 |
| 1 | same | A412 | PASS | `a412-longpress.png`, `app-412-handles.mp4` | 2026-11-11 |
| 1 | same | A360 | PASS | `a360-longpress.png`, `app-360-session.mp4` | 2026-11-11 |
| 2 | Drag a handle across a line / a page break | C412 | **FAIL**: a press on a handle clears the selection, nothing is dragged | `c412-handle-press-sequence-6up.png`, `c412-handle-press-midhold-4up.png`, `c412-a-2-after-drag.png` | 2026-11-11 |
| 2 | same | C360 | **FAIL** (same) | `c360-handle-drag.png`, `chrome-360-handles.mp4` | 2026-11-11 |
| 2 | same | A412 | **FAIL** (same) | `a412-handle-drag.png`, `app-412-handles.mp4` | 2026-11-11 |
| 2 | same | A360 | **FAIL** (same) | `a360-handle-drag.png` | 2026-11-11 |
| 2b | ... across a page break | all | BLOCKED: Web Layout has no pages (phone mode); a page break needs "Desktop layout" and the handle drag fails first | | |
| 3 | Cut / Copy / Paste from the bar | A412 | **FAIL**: Cut on a selected word: the text stays, the selection is gone | `a412-bar-0.png`, `a412-bar-1-after-cut.png`, `app-412-bar.mp4` | 2026-11-11 |
| 3 | same (Select All tapped) | C540 DPR 1 (diagnostic, DPR 1.0 to rule out the scale) | **FAIL** (same) | `c540-dpr1-longpress.png`, `c540-dpr1-bar-selectall-tap.png` | 2026-11-11 |
| 3 | same | C412, C360, A360 | BLOCKED: not run; the bar and the handles fail the same way | | |
| 4 | Gboard autocorrect | C412 | INCONCLUSIVE: `wolrd ` was not corrected in this run (a document that had just taken `world`); WP7 row passes in a clean document | `c412-autocorrect.png`, `chrome-412-autocorrect.mp4`; ref `evidence/kbd/kbd-autocorrect-360.mp4` (2026-11-02) | 2026-11-11 |
| 4 | same | C360 | PASS (ref, WP7 on HEAD of 2026-11-02) | `evidence/kbd/kbd-autocorrect-360.mp4` | 2026-11-02 |
| 4 | same | A412, A360 | not run | | |
| 4b | Autocorrect right after a handle move | all | BLOCKED by row 2 | | |
| 5 | Swipe typing (glide) | C412 | PASS (`world`, suggestions strip shows) | `c412-glide.png`, `chrome-412-glide.mp4` | 2026-11-11 |
| 5 | same | A360 | PASS | `a360-glide.png`, `app-360-session.mp4` | 2026-11-11 |
| 5 | same | C360 | PASS (ref, WP7) | `evidence/kbd/kbd-glide-360.mp4` | 2026-11-02 |
| 5 | same | A412 | not run | | |
| 6 | Cyrillic typing, also over a selection | C412 | PASS (a tapped word `dunyo` replaced by ` salom` + `привет`) | `c412-typed-over-selection.png`, `chrome-412-type-over-selection.mp4` | 2026-11-11 |
| 6 | same | A360 | PASS (same) | `a360-typed-over-selection.png`, `app-360-session.mp4` | 2026-11-11 |
| 6 | same | C360 | PASS (ref, WP7: Cyrillic with `ё`, Uzbek U+02BB/U+02BC) | `evidence/kbd/kbd-cyrillic-yo-360.mp4`, `kbd-uz-apostrophes-360.mp4` | 2026-11-02 |
| 6 | same | A412 | PASS (ref, WP8: EN + RU, saved and reopened) | `evidence/android/picker-typed.png` | 2026-11-03 |
| 6b | Cyrillic over a selection made with a **handle** | all | BLOCKED by row 2 (a tapped or long-pressed selection was used) | | |
| 7 | Pinch zoom | C412 | **FAIL (partly)**: the zoom works (about 3x out, back in), but the same gesture drag-selects a large text range and raised the keyboard | `chrome-412-pinch.mp4`, `c412-p1.png`, `c412-p2.png`, `c412-q1.png` | 2026-11-11 |
| 7 | same | A360 | **FAIL (partly)** (same; a selection and the keyboard) | `a360-pinch-out.png` | 2026-11-11 |
| 7 | same | C360, A412 | not run | | |
| 8 | Momentum scroll | C412 | PASS: a 600 px flick moved the page about 820 px in 0.4 s and on by about 210 px more | `chrome-412-momentum.mp4`, `c412-m1.png`, `c412-m2.png` | 2026-11-11 |
| 8 | same | A360 | PASS (flick scrolls on) | `a360-momentum.png` | 2026-11-11 |
| 8 | same | C360, A412 | not run | | |
| 9 | Rotation keeps the document | C412 | PASS: landscape shows the desktop ribbon (document area about one line), portrait again is the same state (zoom, selection) | `chrome-412-rotation.mp4`, `c412-rot-landscape.png` | 2026-11-11 |
| 9 | same | A360 | PASS (state kept) with a **finding**: landscape with the keyboard up shows no document at all | `a360-landscape.png` | 2026-11-11 |
| 9 | same | A412 | PASS (ref, WP8) | `evidence/android/view-landscape.png` | 2026-11-03 |
| 10 | 360 dp width, no layout overflow | C360 | **FAIL**: no page overflow (`scrollWidth` 360 = `innerWidth`), top row and bottom bar fit, but a table wider than the page (the fixture's 5 columns, 6.25 in) is clipped (1D, 1E unreachable) and a horizontal swipe selects cells instead of scrolling | `c360-table.png`, `c360-table-hswipe.png`, `chrome-360-overflow.mp4` | 2026-11-11 |
| 10 | same | A360 | **FAIL** (same clipping, `a360-pinch-out.png` / app recording) | `app-360-session.mp4` | 2026-11-11 |
| 10 | same | C412 / A412 | **FAIL** (the same table clips 1D/1E at 412 dp) | `c412-m1.png` | 2026-11-11 |
| 10 | Phone layout without overlaps at 360 / 412 | both | PASS (ref, WP6 screenshot check) | `evidence/phone-layout/`, `phone-layout.md` | 2026-11-02 |
| 11 | Contextual bottom toolbar (caret / text selected / table) | C360 | PASS: Format, List, Insert, Keyboard, More; Bold, Italic, Underline, Highlight, More; Row, Column, Delete, Format, Table | `c360-table-hswipe.png` | 2026-11-11 |
| 11 | same | C412, A412, A360 | PASS (the same bars in the other screenshots) | `c412-typed-over-selection.png`, `a412-longpress.png`, `a360-longpress.png` | 2026-11-11 |

Counts (40 table rows, one per item x surface or group): **PASS 20** (14 of them from today's runs, 6 `ref` rows from WP6/WP7/WP8), **FAIL 11**, INCONCLUSIVE 1, BLOCKED 4, not run 4.

## Findings

**F1 (high): selection handles and the action bar cannot be used by a finger in Chrome or in the app.** Reproduced on all four surfaces and on a DPR 1.0 run,
with two injectors (`input swipe`, console touch), press-and-drag at 250 ms and 800 ms holds: a press on a knob (or on a bar button) clears the selection and starts
nothing; with a hold of 0.8 s or more it is the old long-press-then-drag by words (that path works). A mid-hold screenshot already shows the selection gone
(`c412-handle-press-midhold-4up.png`), so it is the press, not the release. The kittest tests of wordcraft/0139 pass (the headless harness feeds the events in one shape).
Not root-caused. Suspects, in the order I would test them:
(a) the bar's buttons are plain `egui::Button::clicked()` (`touch_select.rs:210`); WP6 found that on the web egui does not hit-test a widget under a finger that
had no hover the frame before (`office_overlay.rs:230-235`, STATUS WP6: "egui did not hit-test the bar's widgets on the web"); the phone chrome fixed it by reading
the pointer events itself. The bar needs the same;
(b) `touch_select::pointer` (`touch_select.rs:98-125`, called at `canvas.rs:2343`) takes a press only when a `Touch Start` and `primary_pressed()` are in one frame and
`layer_id_at(at)` is the background: on the web the press may be taken by the held-press path first (`touch_scroll.rs` `hold_press`) or `at` may differ from the painted knob;
add a log of `start`, `grabbed` and the knob rectangle in a debug build and replay `touch.mjs drag`.
Fix is not small (needs a wasm rebuild, about 40 min on this host, and a new hand-rolled hit path) and was not attempted. WP5 stays open on this.

**F2 (medium): a pinch also drag-selects text and raises the soft keyboard** (C412, A360; staged two-finger landing, 80 ms apart, did not avoid it on this slow host).
After `TouchScroll` leaves `Waiting` on a second finger (`touch_scroll.rs:96-99`, `phase = Idle`) the first finger is still the primary pointer and `canvas::mouse` takes it
as a drag-select (`canvas.rs:2343-2360`). A real phone may land the fingers differently; recheck on a device. Candidate fix: stay out of `mouse()` until the primary pointer lifts after a pinch.

**F3 (medium): content wider than the phone is clipped, not reflowed or scrollable.** Web Layout reflows text but not a fixed-width table (`layout/src/lib.rs:2296`
branch, not traced further); 1D and 1E of a 6.25 in table are unreachable at 360 and 412 dp, and a horizontal swipe selects cells. Needs a decision: shrink to the web width
or allow a horizontal pan (SIZING 6.x, O4-style scope).

**F4 (low): the first long press after a document opens often does nothing** (it only focuses; seen on the first try in the app twice and in Chrome twice, the second try works).
Also a long press on a word that already holds the caret just kept the caret (once). Timing under the slow host may be the cause; recheck on a device.

**F5 (low): landscape on a phone** shows the desktop ribbon: with Chrome's own bar about one line of document; in the app with the keyboard up none (`a360-landscape.png`).
Known (WP8 saw landscape); a landscape phone layout is not planned.

**F6 (info): a tap on text selected a word with handles** twice (C412 after a rest, A360) where a caret was expected. Not investigated.

## Tooling added
`scripts/mobile/touch.mjs` (drag, press, pinch through the console), `cdp-eval.mjs` (one expression in the emulator's tab), `fixtures/make-wp13.py` + `wp13.docx`,
`RIG_SVC` in `lib.mjs` (the `hoshiya-serve` to use: `p1/services` is gone).

## Real phone
BLOCKED on a phone, would differ: frame timing (long press, two fingers landing in one frame, momentum feel), the real Gboard swipe model, multi-touch hardware,
physical rotation sensor, thermal and memory (WP14).

## WP16 needs
The F1 fix and a re-run of rows 2, 3, 4b, 6b on four surfaces; F2 decision; F3 decision; the same checklist for Sheets, Slides, PDF if O4 puts them in scope;
a real-phone pass of the timing rows; WP14 numbers.
