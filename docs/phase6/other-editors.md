# Phase 6 WP10a: A0 survey of Sheets, Slides and PDF at 412 dp (2026-11-03, branch `p6-lifecycle`)

Survey only, no fixes. Chrome 133.0.6943.137 on the emulator `HRM_Pixel_8` (API 36, 412 dp via `wm size 540x1200` / `wm density 210`, software GL), Gboard 15.1, dist
`E:/office-target/p6-mobileux/dist` (the shell and the editors as the WP8 APK ships them). Synthetic files: `scripts/mobile/fixtures/survey.xlsx|pptx|pdf` (copies of the
repo's translit/WP23 fixtures). Driver: `scripts/mobile/survey.mjs` (a session with fixed commands), `survey-pdf.mjs` (scroll, pinch, long-press, typing). Screenshots:
`docs/phase6/evidence/other-editors/`. Not recorded as video per row (the rig's `screenrecord` was not run for this survey; screenshots only).

## Headline
1. **Sheets and Slides do not start on this emulator.** Both show "Hoshiya Sheets failed to start: Limit 'max_texture_dimension_2d' value 8192 is better than allowed 4096"
   (`sheets-open-noshim.png`, `slides-open-noshim.png`; messages `gridcraft:failed`, `deckcraft:failed`). It is the failure Docs had before `patches/wordcraft/0002` (egui-wgpu asks for
   8192, the emulator's WebGL2 offers 4096, "and on some phones" per that patch). `gridcraft-web/src/web.rs` and `deckcraft-web/src/web.rs` have no such clamp. PDF starts
   (`pdfcraft-web` clamps the limit, `upstream/pdfcraft/apps/pdfcraft/src/main.rs:619`).
2. To see more, the harness lied to the page (a test shim, `survey.mjs`: WebGL2 `MAX_TEXTURE_SIZE` reported as 8192; **not** a product change). With it Slides starts but draws
   corrupted (`slides-open.png`: the desktop layout cut off on the right, two slide thumbnails at odd places, black patches after a tap, `slides-tap2.png`) and Sheets starts and draws
   only a black canvas with a faded format strip (`sheets-open.png`). Those pictures say nothing reliable about the editors, only that the shim is no substitute for the clamp.
   So **no Sheets or Slides row below could be run**.
3. PDF runs, with the desktop layout: Tools pane, page and Inspector pane side by side at 412 dp, page at 13 %.

## Matrix
PASS / FAIL / BLOCKED (cannot be run because the app does not start). Evidence is a screenshot unless said otherwise.

| Row | Sheets | Slides | PDF |
|---|---|---|---|
| Starts on the emulator's GL | **FAIL** (texture-limit error) | **FAIL** (same) | PASS |
| Usable at 412 dp as it opens | BLOCKED | BLOCKED (with the shim: desktop layout, ribbon cut off) | **FAIL**: three panes, page strip about 100 px wide at 13 % (`pdf-open.png`); the top bar is clipped (menu, Convert, Save, Sign run off the right edge, no overflow) |
| Usable after closing the side panes by hand | BLOCKED | BLOCKED | PASS with limits: page readable at 75 % (`pdf-panes-closed.png`), the right rail and the top bar still clipped |
| Open file from the shell | PASS (file delivered, then the start error) | PASS (same) | PASS (`pdfcraft:ready`, document shown) |
| One-finger scroll | BLOCKED | BLOCKED | **FAIL**: a one-finger drag selects text (the drag from the lower page area selected "Papka Ja" and showed Add comment / Highlight / Underline / Strikethrough, `pdf-scroll-after.png`) instead of scrolling; the hand tool exists but is not the default. The sample is one page, so no vertical scroll of a long file was tried |
| Pinch zoom (two-finger, emulator console injection) | BLOCKED | BLOCKED | PASS: 75 % to 236 % (`pdf-pinch-out.png`); the zoom buttons also work (13 % to 75 %, `pdf-zoomin.png`) |
| Select by long-press | BLOCKED | BLOCKED | PASS: the word is selected, menu Copy / Add a sticky note here / Comments panel (`pdf-select-hold.png`) |
| Soft keyboard and typing (A0 rows: Latin, Uzbek apostrophes, autocorrect, glide, Cyrillic ё, Backspace, Enter) | BLOCKED | BLOCKED | PARTLY: the only text field is the "Find tools and commands" palette; the keyboard comes up and keys reach it (`pdf-type-focus.png`), the palette list is clipped at the left edge. The seven Docs rows do not apply (a viewer/annotator has no body text to type into); not run |
| Save / hand the file to the host | BLOCKED | BLOCKED | PASS for the bridge: tapping Save posted `pdfcraft:save` with 31,263 B (the source size). Chrome then showed "1 download failed" (`pdf-save.png`), cause not isolated. `pdfcraft` has no `requestSave` command, so the shell cannot ask it to save (Slides has one: `upstream/deckcraft/apps/deckcraft-web/src/web/host.rs`; no `requestSave` found for Sheets either; the shell does not use it for any editor) |

## Update 2026-11-03: re-survey with the texture clamp (`gridcraft/0156`, `deckcraft/0156`, branch `p6-texclamp`)
The first pass above stands as the record of the failure. Both web apps now request the adapter's real `max_texture_dimension_2d` (`upstream/gridcraft/apps/gridcraft-web/src/web.rs`,
`upstream/deckcraft/apps/deckcraft-web/src/web.rs`; patches `patches/gridcraft/0156-*`, `patches/deckcraft/0156-*`, same change as `wordcraft/0002`). Rig for this pass: emulator
**`HRM_360`** (API 36, 360 dp via `wm size 540x1200` / `wm density 240`, software GL, Chrome from a fresh profile) because `HRM_Pixel_8` was already running for someone else and was not touched;
`RIG_SCALE=0.5`, **no shim**, dist `E:/office-target/p6-texclamp/dist`. Screenshots `docs/phase6/evidence/other-editors/*-clamp.png`.
- **Both start.** Sheets: `gridcraft:ready`, no `:failed`; Slides: `deckcraft:ready`, `:opened`. Desktop Chromium (1280x800, swiftshader): both start and draw (`sheets-desktop-clamp.png`, `slides-desktop-clamp.png`).
- **The earlier black / corrupted frames were the shim.** Without it Sheets draws the workbook correctly (`sheets-open-clamp.png`) and Slides draws the slide and thumbnail (`slides-open-clamp.png`).
- **Sheets at 360 dp:** the grid is readable, the formula bar and a ribbon row (font, size, B/I/U) show, the ribbon is cut off after one row, and the title bar overlaps (title, theme, comments, Share run into each other). One-finger swipe scrolls the grid (rows 1-26 to 85-110, then to 531-556 and column BY, `sheets-far-clamp.png`; text and grid stay sharp), a tap selects a cell (BY536, `sheets-tap-clamp.png`). Pinch, typing, long-press not run.
- **Slides at 360 dp:** desktop layout squeezed: thumbnail pane about 60 % of the width, the slide at 7 % zoom in a strip on the right, the notes line and the status bar overlap and are clipped, the top bar is cut off. 8 taps on the zoom `+` gave 125 % and the slide redrew correctly (`slides-zoom-clamp.png`). Not run: touch editing, typing, Present, save.
- **Large textures:** nothing broke at the sizes reached (a far-scrolled sheet; a slide at 125 %, about 1.6k px wide at DPR 1.5, under the 4096 limit). A picture or canvas wider than 4096 px on a 4096-limit GPU was **not** tried; wgpu would reject such a texture rather than the device being refused at start.

| Row | Sheets | Slides |
|---|---|---|
| Starts on the emulator's GL, no shim | PASS | PASS |
| Usable at 360 dp as it opens | **FAIL**: readable grid, but the ribbon is cut off after one row and the title bar overlaps | **FAIL**: the desktop layout; slide at 7 %, panes squeezed, bars clipped |
| One-finger scroll | PASS (swipe scrolls rows and columns) | not run (one slide) |
| Tap selection | PASS | not run |
| Zoom | not run (pinch) | PASS via the `+` button (7 % to 125 %) |
| Typing, long-press, save | not run | not run |

## Is each editor usable at 412 dp?
- **Sheets:** unknown, does not start on this rig's GL. Even with a shim it drew only black.
- **Slides:** unknown for the same reason; the shimmed picture suggests the desktop layout (a ribbon wider than the screen).
- **PDF:** not as it opens (panes, 13 % zoom, a clipped top bar); readable and pinch-zoomable once the panes are closed. It needs a phone layout, but it is the closest.

## Recommendation for O4 (as written before the clamp; see the update above)
Keep the Phase 6 gate on Docs only, and treat Sheets and Slides as blocked, not as "bad": the first step for both is a few lines that were already written once (the `wordcraft/0002` clamp,
about 10 lines in each `*-web/src/web.rs`, no layout work), after which the matrix can be run for real; do that as a small WP10a2 beside the gate, since the same startup failure is a
risk for any phone whose WebView reports a 4096 texture limit and is independent of the phone layouts. PDF is the one to prefer if the owner wants a second editor on phones: it runs
today, zoom and long-press selection already work by touch, and what it lacks is layout (panes closed by default on a phone, a collapsing top bar, one-finger drag as scroll with
selection on long-press, a save hook the shell can call), which is the WP10d scope. Sheets touch selection stays the expensive item (SIZING 6.4, 9 days) and should wait for the re-survey.

## Not verified
- Anything about Sheets and Slides behind the startup failure (layout, scroll, selection, typing rows, save). A real phone may report 8192 or more and start; not known.
- Whether the black / corrupted frames under the shim are the shim or the apps.
- One-finger scroll of a multi-page PDF, PDF with form fields, large PDFs; the browser download failure of the saved PDF; the WP8 APK for these editors (not run; Chrome only).
- Dark theme, 360 dp (the second AVD was not started).

## O4 after the clamp
The picture changed in one way: Sheets and Slides are no longer blocked, they start and draw on a 4096-texture GPU. They are not phone-ready: both come up in the desktop layout (Sheets cut-off ribbon and
overlapping title bar, Slides squeezed panes at 7 %), like PDF. Sheets already scrolls and selects by touch, which makes its remaining work layout plus the selection handles; Slides needs the layout first.
So the O4 advice holds with one correction: keep the Phase 6 gate on Docs, and rank the second editor by layout cost: PDF (panes and top bar), then Sheets (ribbon and title bar; touch scroll and tap work),
then Slides. The SIZING 6.4 estimate for Sheets touch selection should be re-checked against this survey before the owner decides.

Sizes of the new builds (`CI_DIST_ROOT=E:/office-target/p6-texclamp/dist bash scripts/ci.sh sizes`, brotli of the wasm): Sheets 4,954,751 B (budget 4,966,771, headroom 12,020), Slides 9,512,548 B (budget 9,524,217, headroom 11,669). Budgets unchanged.
