# Phase 6 WP6: Docs phone layout and the contextual bottom toolbar (2026-11-02, branch `p6-phone`)

Plan section 5.1 (trigger and layout), D3-D5, patch **wordcraft/0140** plus `domains/docs/crates/docs-office/src/phone.rs`. Docs only; Sheets, Slides and PDF
are untouched (plan 5.2, WP10). Evidence is in `docs/phase6/evidence/phone-layout/` (Chromium shots at 360 and 412 dp, emulator shots and JSON under `rig/` (HRM_Pixel_8) and
`rig360/` (HRM_360; its shots show a "System UI isn't responding" dialog because two emulators ran next to a build: the layout under it is intact, the earlier
run's files are the same checklist, a rerun on a quiet machine is open); recordings are under `E:/office-target/p6/evidence/<run id>/` with their SHA-256 in `docs/phase6/evidence/SHA256SUMS` (the rig now writes each run to its own
subfolder, `scripts/mobile/rig.mjs`, so another run cannot overwrite them). Synthetic documents only (`scripts/mobile/fixtures/blank.docx`).

## 1. Trigger and layout (three lines)
- **Trigger (D5):** the window is at most 600 CSS px wide and the last pointer was a finger. `docs_office::phone::decide` (`phone.rs:51`) and `active` (`phone.rs:76`): a touch
  sets it, a mouse button clears it after a one-second grace (the web runner also reports a finger as pointer events in frames of their own); it starts from
  `(pointer: coarse)`; `?phone=on` or `?phone=off` forces it. The web app is the only caller of `set_phone` (`web.rs:89`), so the desktop apps never see it.
- **Hook:** one `else if office_overlay::phone_chrome(self, ui)` branch in `WordApp::ui` (`lib.rs:2274`, 5 lines) before the title bar and ribbon; `phone_chrome`
  (`office_overlay.rs:177`) draws nothing and returns false unless the extension says phone. In phone mode it shows the document in Web Layout (D4; leaving restores the
  view), skips the title bar, ribbon, status bar and panes, and runs the commands the chrome returns with `WordApp::run` (dialogs, file picker and paste behave as from the ribbon).
- **Chrome:** `phone::chrome` (`phone.rs:469`) draws a 48 dp **top row** (file name cut with an ellipsis, a dirty dot, Undo, Redo, More), a contextual 48 dp **bottom
  toolbar** (at most five slots) and **sheets** (at most 60 % of the window, scrolling inside) above it. Back/Home is the shell's own bar above the frame (`web/shell`),
  not repeated. Every size is dp times `phone::dp` (egui points per CSS px, from `window.innerWidth`), every colour comes from the app's own theme tokens, every icon from the app's icon set.

## 2. Toolbar sets (`slots`, `phone.rs:168`; `set_of`, `phone.rs:116`; precedence picture, table, selection, caret)
| Selection | Slots |
|---|---|
| caret in text | Format (sheet), List (sheet), Insert (sheet), Keyboard (show or hide the soft keyboard), More (menu) |
| text selected | Bold, Italic, Underline (pressed from `format.state`), Highlight (yellow), More (the Format sheet) |
| caret in a table | Row (insert below), Column (insert right), Delete (sheet: rows, columns, table), Format, Table (sheet: insert above/below/left/right, merge, split) |
| picture selected | Wrap Text (sheet: six wraps), Size (`picture.size`, its dialog), Delete, More (change, reset) |

Sheets: Format (bold, italic, underline, strikethrough; size minus, size, plus; four font colours; three highlights and none; Normal, Heading 1, 2; four alignments),
List (bullets, numbering, outdent, indent), Insert (tables 2x2, 3x3, 4x4; Picture; Page Break), the top row's More menu (Save, Save As, Find, Print Layout or Web Layout, Desktop layout).
Each control is a command id upstream already has; nothing new in the engine. Deviations from plan 5.1: no Print and no Language in the menu (section 4), Keyboard on the caret
set only, the selection set's Highlight applies yellow (the full palette is in the Format sheet).

## 3. Keyboard: `interactive-widget=resizes-content` is shipped, for Docs only
With the phone layout the default (`resizes-visual`) is wrong: the keyboard covers the bottom toolbar (`rig/` first run: the Format tap landed on Gboard). With
`resizes-content` the layout viewport shrinks, the toolbar sits on the keyboard and the page keeps a usable height (emulator HRM_Pixel_8, Chrome 133, Gboard 15.1: `rig/*-bold-on.png`).
The WP7 finding (editor 120 px tall, zoom 22 %) was with the desktop ribbon; with the phone layout the editor keeps the top row, the page and the bar. It is applied from
the shell (`web/shell/shell.js`, `viewport()`), only while Docs is shown, so Sheets, Slides and PDF keep Chrome's default. The window shrinking also scrolls the caret into view
(PLAN 5.3, `office_overlay.rs`, `scroll_to_caret` when the height drops). A sheet is at most 60 % of the shrunken window, so with the keyboard up it is about 220 dp and scrolls.
Not tested: Android WebView (`adjustResize` is WP8's question) and the 5.3 caret scroll in a long document (only the page-bottom case of WP7 was seen).

## 4. Dialogs reachable from the phone chrome (done-check: fit 360 dp or full screen)
Measured in desktop Chromium at 360x780 and 412x915 (`phone-dialogs.mjs`; `*-dlg-*.png`).
| Reached by | Dialog | 360 dp | 412 dp |
|---|---|---|---|
| More > Find | Find and Replace | fits (about 340 dp) | fits |
| Table > Split Cells | Split Cells | fits | fits |
| Size (picture) | Size | fits | fits |
| Insert > Picture, Change Picture | the browser's file chooser | not egui | not egui |
| More > Save, Save As | none on the web (the shell downloads the file) | n/a | n/a |
| More > Print | opens the Backstage Print page: 200 px rail, 160 px of page, clipped | **not offered** | **not offered** |
Dialogs: patch 0140 makes every egui dialog window at most the screen wide, scrolling inside (`dialogs.rs`); the three above needed nothing more. Not reachable and not
addressed: the Backstage (File), Options, Font, Paragraph and the other desktop dialogs (no phone control opens them). A dialog opened from the keyboard of a desktop-class
tablet is out of scope. "Language" in the menu was dropped: `ui.language` only reports the language; the interface language is the shell's.

## 5. Done-check
| Item | Result |
|---|---|
| zero overlaps at 360 and 412 | Chromium screenshots at both sizes (`360-*.png`, `412-*.png`) and the emulator at 412 (and 360, `rig360/`): the name is cut with an ellipsis, no label overlaps a button; the unit test `the_chrome_draws_at_360_and_412_without_overflow` bounds the used area. Screenshot check by eye, as in `docs/phase4/mobile-evidence.md` section 2 |
| every bottom-toolbar button 48 pt or more | height 48 dp (`phone::ROW`); width window / 5 = 72 dp at 360, test `the_toolbar_has_at_most_five_slots_of_48_dp_at_320` asserts at least 48 dp at 320 |
| every reachable dialog fits or is full-screen | section 4 |
| docs-office tests: mode switch, contextual sets | `phone/tests.rs`, 12 tests: width, finger and force; the pointer-after-touch grace; the set per selection; slots; every control a registered command with valid parameters; every label in the catalog (uz-Latn, uz-Cyrl, ru); a tap on a slot and on a sheet row (split, batched, doubled events); the layout at 360 and 412 |
| recordings via the rig | `phone-docs` checklist, HRM_Pixel_8 (412 dp, DPR 2.625) and HRM_360 (360 dp, 540x1200 at 240 dpi, DPR 1.5) |
| `sizes` delta | section 7: **over budget by 3,705 B** |
| desktop layout unchanged | the hook is unset in the desktop apps, `phone_chrome` returns false; ui-egui tests 733 passed, the same 7 i18n audit failures as before the change (translit strings, unrelated); docs-office 34 tests pass |

## 6. Findings worth keeping
1. **egui's hit test did not report the bar's widgets as under a finger** on the web (`contains_pointer` false with the pointer inside the rect; found with a log build), because
   the page widget (`wordcraft_canvas`) is hit-tested on the same pointer; and a press on the bar also reached the page, which put the caret at the end of the document (a caret
   in a table cell left the table when Row was tapped). Fix: the chrome reads taps from the pointer events itself (`phone.rs`, `target`), and `office_overlay::chrome_down`
   makes `canvas::mouse` stand aside while a finger that went down on a bar is down (0140, 4 lines in `canvas.rs`).
2. A quick tap puts press and release in one frame and the pointer is already gone at the frame's end: events, not the pointer state, are read.
3. Headless desktop Chromium at a device pixel ratio above 1 maps touch positions at the wrong scale for the whole app (the old ribbon too); desktop-Chromium evidence therefore
   uses DPR 1, the emulator (DPR 1.5 and 2.625) is right. Cause not found; not caused by 0140. Playwright's `touchscreen.tap` (touch down and up in one message) is also missed by the page; the scripts use CDP touches with a 60 ms hold.
4. The shell logs `Refused to apply inline style` (CSP) in the test dist: seen in every run, not from this work.

## 7. Size (Docs wasm, brotli -q 11 --lgwin=24, `dist/docs-web`)
9,875,353 B against 9,855,771 B at phase-6 HEAD (`docs/phase6/m3.md`): **+19,582 B**; the headroom was 15,877 B, so the budget (base_br + delta_br_max = 9,871,648 B) is exceeded by
**3,705 B** and `ci.sh sizes` will fail. JS glue 20,803 B (was 20,773 B, limit 24,576). The plan estimate for phone chrome was 8-15 KB. No budget was edited: either the owner raises
`delta_br_max` for Docs (measured delta plus 10 % rounded to 16 KiB would be 212,992 B, a STATUS line is needed) or the phone code is slimmed (the sheet rows are `const` data, the icons are upstream's).

## 8. Not verified, needs the owner
- A real phone (touch feel, 48 dp reachability with one thumb, battery); everything here is emulator or desktop Chromium.
- Android WebView; Gboard in Russian and Uzbek with the phone layout (only English typing was driven).
- The owner's review of the Uzbek labels of three new rows (`Keyboard`, `More`, `Desktop layout`, `reviewed:agent-sonnet:2026-11-02`).
- Picture and text selection flows beyond the screenshots: wrap sheet, delete, Page Break and the List sheet were rendered, the commands are the ribbon's and are unit-tested only as registered ids.
