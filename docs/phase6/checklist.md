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

## Re-run 2026-11-12 after the F1/F2/F6 fix (wordcraft/0186, branch `p6-handles`)

Rig as above (`HRM_Pixel_8`, Chrome 133, `wm size 540x1200`, density 210 = 412 dp and 240 = 360 dp, `touch.mjs` console touch, `RIG_SCALE=0.5`). Page: `p6-handles`
(phase-6 `f7743895` + wordcraft/0186) Docs built by `scripts/build.sh docs web --release` (wasm 9,872,684 B brotli of 9,904,416, `ci.sh sizes` ok), the other
editors copied from `p6-tapfix/dist-final`, served by `hoshiya-serve` through `session.mjs`. App: `packaging/android/build.sh` over that dist (53,188,227 B),
VIEW intent with a write grant. Same fixture. Screenshots `docs/phase6/evidence/checklist/fix-*.png`; recordings `E:/office-target/p6h/rec/` (SHA-256 in `evidence/SHA256SUMS`).

**Cause isolated before the rebuild** (`fix-old-wasm-cause-6up.png`, the checklist's old wasm): the first long press after load did nothing (F4), the second selected the
word, a handle drag cleared the selection (F1 as before). Then, with no wasm change, a capture-phase `pointerdown` listener that stops a finger's event on the canvas was
injected into the editor frame (`cdp-eval.mjs`; it logged the order `pointerdown:touch touchstart: pointerup:touch touchend:`): the same handle drag extended the selection
and Cut on the bar cut the text. A pinch still drag-selected and raised the keyboard with only that listener: the second finger's press needs the canvas fix too.

| # | Item | Surface | Result | Evidence | Date |
|---|---|---|---|---|---|
| 1 | Long press selects the word, handles and the bar show (first long press after load) | C412, C360, A412, A360 | PASS on the first try on all four (F4 not seen) | `fix-c412-handles-4up.png` (1st), `fix-c360-4up.png` (1st), `fix-a412-5up.png` (1st), `fix-a360-3up.png` (1st) | 2026-11-12 |
| 2 | Drag a handle across a line | C412 | PASS: end handle down two lines, start handle up into the heading (across a paragraph) | `fix-c412-handles-4up.png` (2nd, 3rd), `rec/c412-after.mp4` | 2026-11-12 |
| 2 | same | C360 | PASS (end handle from the heading into the next paragraph) | `fix-c360-4up.png` (2nd) | 2026-11-12 |
| 2 | same | A412 | PASS (end handle two lines down) | `fix-a412-5up.png` (2nd), `rec/a412-after.mp4` | 2026-11-12 |
| 2 | same | A360 | PASS | `fix-a360-3up.png` (2nd) | 2026-11-12 |
| 2b | ... across a page break | all | BLOCKED as before (Web Layout has no pages in phone mode); covered by the ui-egui test `dragging_a_handle_across_a_page_break` only | | |
| 3 | Cut / Copy / Paste / Select All from the bar | C412 | PASS: Cut removed the selection (Android clipboard chip shows it), Copy then Paste over another long-pressed word replaced it with the copied word, Select All selected the document | `fix-c412-handles-4up.png` (4th), `fix-c412-bar-4up.png` | 2026-11-12 |
| 3 | Cut | C360 | PASS | `fix-c360-4up.png` (3rd) | 2026-11-12 |
| 3 | Cut | A412 | PASS | `fix-a412-5up.png` (3rd) | 2026-11-12 |
| 3 | Select All | A360 | PASS | `fix-a360-3up.png` (3rd) | 2026-11-12 |
| 4b | Autocorrect right after a handle move (keyboard up, long press a word, drag a handle across words or a line, type `teh `) | C412 | PASS: the selection (several words, 1-2 lines) is replaced and Gboard turns `teh ` into `the ` (`wolrd ` was not corrected in the same flow, see below) | `hh-c412-4b-2-handle-moved.png`, `hh-c412-4b-3-teh-corrected.png` | 2026-11-21 |
| 4b | Autocorrect right after a handle move (keyboard up, long press a word, drag a handle across words or a line, type `teh `) | C360 | PASS: the selection (several words, 1-2 lines) is replaced and Gboard turns `teh ` into `the ` | `hh-c360-4b-1-handle-selected.png`, `hh-c360-4b-2-teh-corrected.png` | 2026-11-21 |
| 4b | Autocorrect right after a handle move (keyboard up, long press a word, drag a handle across words or a line, type `teh `) | A412 | PASS: the selection (several words, 1-2 lines) is replaced and Gboard turns `teh ` into `the ` | `hh-a412-4b-1-handle-selected.png`, `hh-a412-4b-2-teh-corrected.png` | 2026-11-21 |
| 4b | Autocorrect right after a handle move (keyboard up, long press a word, drag a handle across words or a line, type `teh `) | A360 | PASS: the selection (several words, 1-2 lines) is replaced and Gboard turns `teh ` into `the ` | `hh-a360-4b-1-handle-selected.png`, `hh-a360-4b-2-teh-corrected.png` | 2026-11-21 |
| 6b | Cyrillic over a selection made with a **handle** (RU layout, long press a word, drag a handle, type `привет`) | C412 | PASS: the whole handle selection is replaced by `привет`, text around it intact | `hh-c412-6b-1-handle-selected.png`, `hh-c412-6b-2-privet.png` | 2026-11-21 |
| 6b | Cyrillic over a selection made with a **handle** (RU layout, long press a word, drag a handle, type `привет`) | C360 | PASS: the whole handle selection is replaced by `привет`, text around it intact | `hh-c360-6b-1-handle-selected.png`, `hh-c360-6b-2-privet.png` | 2026-11-21 |
| 6b | Cyrillic over a selection made with a **handle** (RU layout, long press a word, drag a handle, type `привет`) | A412 | PASS: the whole handle selection is replaced by `привет`, text around it intact | `hh-a412-6b-1-handle-selected.png`, `hh-a412-6b-2-privet.png` | 2026-11-21 |
| 6b | Cyrillic over a selection made with a **handle** (RU layout, long press a word, drag a handle, type `привет`) | A360 | PASS: the whole handle selection is replaced by `привет`, text around it intact | `hh-a360-6b-1-handle-selected.png`, `hh-a360-6b-2-privet.png` | 2026-11-21 |

Rows 4b and 6b, 2026-11-21 (branch `p6-land`, run on the page and app of the 2026-11-12 re-run: `p6h` dist with wordcraft/0186; rig as above, `RIG_SCALE=0.5`, Gboard EN and RU
layouts through `lib.mjs` key maps (`typ`: one tap per key, 450 ms apart), the console `touch.mjs press 1200` then `drag` for the handle; the keyboard was raised by a tap in the
text before the long press and stayed up). The page was the 3-page `wp13.docx` (Chrome) and `phone-ux.docx` (the app; no wp13 media-store row on this AVD, same kind of text).
Text read from screenshots; nothing was saved. Screenshots `docs/phase6/evidence/checklist/hh-*.png` (22; no recordings).
- **Gboard autocorrect cannot be proved with `wolrd `** here: in the first C412 run `wolrd ` typed over a handle selection stayed `wolrd` (`hh-c412-4b-wolrd-not-corrected.png`), but
  `teh ` right after it, over a plain caret and over a handle selection, was corrected on C412 and on the other three surfaces. So the handle path delivers Gboard's
  `DeleteSurrounding` + commit as at a plain caret; whether Gboard corrects a given misspelling depends on its own dictionary state (same as row 4's earlier INCONCLUSIVE),
  not on the page. The WP7 `wolrd` PASS (clean document) stands.
- Gboard capitalises a sentence start: on A412 the result is `The inov matni.`, not `the` (Gboard, not the editor).
- Not run: a handle drag across a page break (2b, no pages in phone mode), a real phone, Gboard glide after a handle move.
- Side note, not a finding: `adb shell input keyevent 3` (Home) was sent once at the end to leave the app; the rig's own rule is never to send keyevents while Gboard is in use (it did not matter, the run was over).
| 7 | Pinch zoom: no selection, no keyboard | C412 | PASS: out about 3x and back, horizontal and vertical, fingers landing together and 80 ms apart; no selection, keyboard stays down | `fix-c412-pinch-5up.png`, `rec/c412-pinch.mp4` | 2026-11-12 |
| 7 | same | C360, A412 | PASS (out and back) | `fix-c360-4up.png` (4th), `fix-a412-5up.png` (4th, 5th) | 2026-11-12 |
| F6 | A tap places a caret | C412 | PASS: 6 of 6 taps on text gave a caret, none selected a word (the stray text in the shots is from my first taps landing on the raised keyboard and its clipboard chip, not from the app) | `fix-c412-taps-3up.png` | 2026-11-12 |

New observation (low): when the bar has no room above the selection it is placed below and can cover the end knob (`fix-c360-4up.png` 2nd, `fix-c412-handles-4up.png` 3rd).
`touch_select::show` puts it at `anchor.max.y + DROP + R + GAP`, i.e. below the knob of the *first* line; with a multi-line selection the end knob is lower and can sit under it.

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
**Fixed 2026-11-12 (wordcraft/0186).** Root cause: neither suspect. Chromium fires `pointerdown` before `touchstart`; eframe 0.36 turns both into a primary press, and
its `pointerdown` handler runs a frame at once (`runner.logic()`) holding only `PointerButton { pressed }`, with no `Touch` event. In that frame `touch_select::pointer`
saw a mouse press (`touch_select.rs:110-111` set `tsel.touch = false`: no handles, the bar not drawn) and `touch_scroll::hold_press` (no `Touch Start`) let it through, so
`canvas::mouse` placed the caret under the finger; the `touchstart` frame then found a caret and nothing to grab. The bar's `Button::clicked` failed only because the bar
was gone in that frame. Fix: wordcraft-web stops a finger's `pointerdown` on the canvas in the capture phase (`web.rs` `finger_presses_once`), so a finger's press arrives once,
with its touch, as the kittest harness already fed it. Emulator re-run above: PASS on C412, C360, A412, A360.

**F2 (medium): a pinch also drag-selects text and raises the soft keyboard** (C412, A360; staged two-finger landing, 80 ms apart, did not avoid it on this slow host).
After `TouchScroll` leaves `Waiting` on a second finger (`touch_scroll.rs:96-99`, `phase = Idle`) the first finger is still the primary pointer and `canvas::mouse` takes it
as a drag-select (`canvas.rs:2343-2360`). A real phone may land the fingers differently; recheck on a device. Candidate fix: stay out of `mouse()` until the primary pointer lifts after a pinch.
**Fixed 2026-11-12 (wordcraft/0186).** Two presses caused it: the first finger's `pointerdown` frame (above, a mouse drag-select that the first finger's moves then
grew) and, independently, eframe's `touchstart` for the second finger, which presses the primary pointer again at the first finger (`multi_touch()` is already on in that
frame, so `hold_press` declines and `mouse()` took it as a click and drag; the drag's `drag_started` then focused the page: keyboard). Fix: `canvas::mouse` returns
during a multi-touch gesture, `touch_select` takes no handle press from a pinch, and `TouchScroll` counts a pinch as a scroll so the focus rule (`canvas.rs` `focus_click`)
ignores its drag. Test `touch_select::tests::a_pinch_neither_selects_nor_takes_the_keyboard` (fails without the change). Emulator: PASS on C412 (also staged 80 ms), C360, A412.

**F3 (medium): content wider than the phone is clipped, not reflowed or scrollable.** Web Layout reflows text but not a fixed-width table (`layout/src/lib.rs:2296`
branch, not traced further); 1D and 1E of a 6.25 in table are unreachable at 360 and 412 dp, and a horizontal swipe selects cells. Needs a decision: shrink to the web width
or allow a horizontal pan (SIZING 6.x, O4-style scope).
**Fixed 2026-11-24 (wordcraft/0188, branch `p6-widepan`; owner chose "horizontal pan").** Root cause: the Web Layout sheet was only as wide as the window, so a fixed-width table
(`tblW` set, as in the 6.25 in fixture) was clipped by the page raster and the canvas had nothing to scroll to; `TouchScroll` already turns horizontal travel into scroll. Fix: in
`ViewMode::Web` the layout widens each page to its table cells' right edge (+8 pt); text still wraps at the window width, so reflowing documents keep a sheet exactly as wide as before.
Test `touch_select::tests::a_horizontal_swipe_pans_a_wide_table_and_selects_nothing` (fails without the layout part). Emulator (`HRM_Pixel_8`, Chrome 133, 540x1200, density 210 = 412 dp
and 240 = 360 dp, synthetic `scripts/mobile/fixtures/widepan.docx`, swipes by `adb shell input swipe`): before (`widepan/before-c412-0.png`, `before-c412-1-swipe-left.png`) 1D is cut and
1E unreachable, the swipe moves nothing; after, C412 (`after-c412-1-swipe-left.png`) and C360 (`after-c360-1-swipe-left.png`) the swipe pans to 1C-1D-1E, stops at the table's right edge,
and selects no cells. The "before" page is the `p6-handles` dist (phase-6 without 0187), not a rebuild of phase-6 HEAD. Not run: the Android app (A412/A360; same wasm), a real phone,
caret scrolled into view by typing or arrow keys into an off-screen column (not tested: it goes through the canvas's existing scroll-to-caret, which scrolls both axes), tap and long press
on the panned table (unchanged code, ui-egui touch tests pass). Side effect: the page panned right shows blank sheet beside the text; desktop Web Layout also gains the scroll for a wide table.

**F4 (low): the first long press after a document opens often does nothing** (it only focuses; seen on the first try in the app twice and in Chrome twice, the second try works).
Also a long press on a word that already holds the caret just kept the caret (once). Timing under the slow host may be the cause; recheck on a device.
**2026-11-12:** reproduced once on the old wasm (first long press after load ignored), not seen after wordcraft/0186 in 4 of 4 first long presses (C412, C360, A412,
A360). Likely the same `pointerdown` frame (a mouse press before the first `touchstart` turned the phone's touch handling on), but not proven: keep it open for a real phone.

**F5 (low): landscape on a phone** shows the desktop ribbon: with Chrome's own bar about one line of document; in the app with the keyboard up none (`a360-landscape.png`).
Known (WP8 saw landscape); a landscape phone layout was not planned.
**Fixed 2026-11-21 (wordcraft/0187, branch `p6-land`).** The phone layout test was `innerWidth <= 600 && finger` (`docs_office::phone::decide`), so 915 CSS px wide in landscape
was the desktop. New rule: `finger && (innerWidth <= 600 || min(screen.width, screen.height) <= 600)`. The short side of the *screen* (reported by the web host through
`phone::set_screen_short_side`, 4 lines in `web.rs`) is the same in both orientations and does not shrink when the soft keyboard opens (the window height does, so a
window-height test would have flipped a tablet to the phone layout while typing). Unchanged: tablets with a short side of 768 and up (landscape and portrait, keyboard up or
not), desktop and touch laptops (short side 768 and up), a mouse as last pointer, a screen that cannot say (native, tests: width test alone), `?phone=on|off`. Changed
on purpose: a landscape phone, and any touch device whose screen is 600 CSS px or less on its short side (a 7 inch tablet in landscape; in portrait it was the phone layout already).
Tests: `docs-office` `the_mode_switch_needs_a_narrow_window_and_a_finger` (old table, now with the short side unknown) and
`a_phone_in_landscape_keeps_the_phone_layout_a_tablet_or_laptop_does_not` (915x412 phone = same as 412x915, 1280x800 and 800x800 tablets, 1366x768 laptop, mouse, unknown short side);
`cargo test -p docs-office phone` 19 passed. Emulator (HRM_Pixel_8, Chrome 133, density 210, rotation 90, wasm built with the change, wasm 9,877,537 B brotli of the 9,904,416
budget, `ci.sh sizes` ok): `f5-c412-landscape-phone-layout.png` shows the top row, the document (about 12 lines) and the bottom toolbar. **Still true:** with the soft keyboard up
in landscape (`f5-c412-landscape-keyboard-up.png`) Gboard fills the lower 340 of 540 px and with Chrome's own bar only the top row and a sliver of page remain, the bottom toolbar is
under the keyboard (Android's landscape keyboard, not the layout; the app has no Chrome bar and was not re-run, its APK was not rebuilt). A landscape-specific keyboard
arrangement (hide the top row while typing) was not attempted.

**F6 (info): a tap on text selected a word with handles** twice (C412 after a rest, A360) where a caret was expected. Not investigated.
**Fixed 2026-11-12 (wordcraft/0186).** Cause: one tap made two presses (`pointerdown` frame, then `touchstart`), so `canvas::press_count` saw the second as a
double-click within `max_double_click_delay` and selected the word (the tapfix run's "second tap selected a word" too). With one press per finger, 6 of 6 taps placed a caret.

## Tooling added
`scripts/mobile/touch.mjs` (drag, press, pinch through the console), `cdp-eval.mjs` (one expression in the emulator's tab), `fixtures/make-wp13.py` + `wp13.docx`,
`RIG_SVC` in `lib.mjs` (the `hoshiya-serve` to use: `p1/services` is gone).

## Real phone
BLOCKED on a phone, would differ: frame timing (long press, two fingers landing in one frame, momentum feel), the real Gboard swipe model, multi-touch hardware,
physical rotation sensor, thermal and memory (WP14).

## WP16 needs
a real-phone check of F4 (rows 4b and 6b passed on the emulator 2026-11-21); F3 decision; the same checklist for Sheets, Slides, PDF if O4 puts them in scope;
a real-phone pass of the timing rows; WP14 numbers.
