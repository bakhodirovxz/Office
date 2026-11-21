# Phase 6 WP6: Docs phone layout and the contextual bottom toolbar (2026-11-02, branch `p6-phone`; revised in section 9: read first, one bar)

Plan section 5.1 (trigger and layout), D3-D5, patch **wordcraft/0140** plus `domains/docs/crates/docs-office/src/phone.rs`. Docs only; Sheets, Slides and PDF
are untouched (plan 5.2, WP10). Evidence is in `docs/phase6/evidence/phone-layout/` (Chromium shots at 360 and 412 dp, emulator shots and JSON under `rig/` (HRM_Pixel_8) and
`rig360/` (HRM_360; its shots show a "System UI isn't responding" dialog because two emulators ran next to a build: the layout under it is intact, the earlier
run's files are the same checklist, a rerun on a quiet machine is open); recordings are under `E:/office-target/p6/evidence/<run id>/` with their SHA-256 in `docs/phase6/evidence/SHA256SUMS` (the rig now writes each run to its own
subfolder, `scripts/mobile/rig.mjs`, so another run cannot overwrite them). Synthetic documents only (`scripts/mobile/fixtures/blank.docx`).

## 1. Trigger and layout (three lines)
- **Trigger (D5):** the window is at most 600 CSS px wide (or, since 2026-11-21, the device screen's short side is, so a landscape phone keeps it; wordcraft/0187) and the last pointer was a finger. `docs_office::phone::decide` (`phone.rs:51`) and `active` (`phone.rs:76`): a touch
  sets it, a mouse button clears it after a one-second grace (the web runner also reports a finger as pointer events in frames of their own); it starts from
  `(pointer: coarse)`; `?phone=on` or `?phone=off` forces it. The web app is the only caller of `set_phone` (`web.rs:89`), so the desktop apps never see it.
- **Hook:** one `else if office_overlay::phone_chrome(self, ui)` branch in `WordApp::ui` (`lib.rs:2274`, 5 lines) before the title bar and ribbon; `phone_chrome`
  (`office_overlay.rs:177`) draws nothing and returns false unless the extension says phone. In phone mode it shows the document in Web Layout (D4; leaving restores the
  view), skips the title bar, ribbon, status bar and panes, and runs the commands the chrome returns with `WordApp::run` (dialogs, file picker and paste behave as from the ribbon).
- **Chrome:** `phone::chrome` (`phone.rs:469`) draws a 48 dp **top row** (file name cut with an ellipsis, a dirty dot, Undo, Redo, More), a contextual 48 dp **bottom
  toolbar** (at most five slots) and **sheets** (at most 60 % of the window, scrolling inside) above it. Back/Home was the shell's own bar above the frame (`web/shell`; replaced by the editor's own Back arrow, section 9),
  not repeated then. Every size is dp times `phone::dp` (egui points per CSS px, from `window.innerWidth`), every colour comes from the app's own theme tokens, every icon from the app's icon set.

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

## 9. Read first and one bar (revision, 2026-11-02, branch `p6-mobileux`)
The owner reviewed the WP6 screenshots and asked for a phone layout "close to WPS Office or ONLYOFFICE mobile, or else very simple and easy to understand". Four
problems in `evidence/phone-layout/360-*.png`: (1) the file name twice, in the shell's header and in the editor's row, about 110 px of 640; (2) the Keyboard button drew a
microphone and the styles an "A with a check" (spell check); (3) a blank page was a short white strip on grey; (4) a document opened for editing at once. The target is the common pattern of
the two reference apps (public UX only, nothing copied from them): one bar, a document opens for reading, a round Edit button, then the editing toolbar. Evidence:
`docs/phase6/evidence/phone-ux/` (Chromium, DPR 1, `scripts/mobile/phone-ux.mjs`, fixture `scripts/mobile/fixtures/phone-ux.docx` made by `make-phone-ux.py`).

### 9.1 What the phone shows now
| State | Top row (the only bar) | Page | Bottom | Floating |
|---|---|---|---|---|
| reading (after Done editing) | Back arrow, file name (ellipsis, dirty dot), More | the whole screen below the row; no caret, no soft keyboard, a tap does nothing; a finger drag scrolls | none | round Edit (pencil) button, 56 dp, accent, 16 dp from the right and bottom edges |
| editing (default) | Back, name, Undo, Redo, More | as before, the caret goes where the finger taps | the contextual toolbar of section 2 (caret, text, table and picture sets unchanged) | none |
- **Back** is the shell's Home (`wordcraft:back`); in the standalone page (no `?host=parent`) there is no arrow. **More** in editing leads with **Done editing** (back to reading), then Save, Save As, Find,
  Desktop layout; the Print/Web Layout switch is gone (the phone is always Web Layout, the rest is "Desktop layout").
- **No pencil** when the session cannot edit: the host's read-only open (view sessions, `FAKE_HRM_MOBILE_EDIT` off, PLAN O7) sets Restrict Editing's read-only protection and `phone::can_edit` reads that. Such a
  document stays in reading however it is asked (unit test). Side effect: no text selection or copy in a view-only document on a phone.
- **Edit first (owner decision 2026-11-03, branch `p6-editfirst`):** every document that can be edited opens in editing, with the bottom toolbar. The state kept is the id of the document the user switched to reading (`phone.rs` `reading_doc_id`), so **Opening another document** (shell New or Open: `Session::set_document` changes `document_id`) starts in editing again. More > Done editing still gives reading, the pencil brings editing back. A view session still opens in reading with no pencil. A blank New document is now ready to type in (the open question of 9.4 is closed).
- **No keyboard on open:** the page's `want_focus` starts true, which would focus the canvas and send IME output (a soft keyboard on a device). Patch **wordcraft/0145** (`office_overlay.rs` `phone_chrome`, +4/-1) drops `want_focus` and the canvas focus when the document id changes, so the caret sits at the document start and the keyboard comes with the user's first tap.
- **Saved toast:** the "Saved: <name>" status line is in the header that phone mode hides, so on a phone the shell shows it as a toast (`#toast`, `role="status"` `aria-live="polite"`, centred 72 px above the bottom, i.e. above the 48 dp toolbar, 2 s then a 0.3 s fade) from the same `save` message and the same translated string (`ed.saved`); `shell.js` `toast`, `shell.css` `#toast`. Desktop keeps the header line only. An error still brings the header back.
- **Sheets** have a grab handle, a header (title left, close right, 48 dp target) and close on a tap outside (as before). **Format** has two tabs, "Text" (B I U S, size, four colours, highlights) and "Paragraph" (Normal, Heading 1 and 2 drawn in that
  style's size and weight, four alignments, bullets, numbering, indent minus and plus); its height is that of the taller tab, so it does not jump. Size and indent rows are icon only (their long
  translated names were cut with an ellipsis at 360 dp); a long bottom-toolbar label is set down to 9 dp before it is cut.
- **Icons**: the Keyboard is a drawn keyboard, Back an arrow, Edit a pencil, Done editing a check (`phone/glyph.rs`, strokes in the theme's text colour; the app's icon set has none of them, and a Fluent bitmap would
  cost bytes in the Docs wasm). Close is upstream's. The styles have no icon any more.
- **Paper**: the phone page's panel is filled with the paper colour (white, or the dark paper of View > Switch Modes), so a short document is not a strip on grey. The page's own shadow is still drawn at its bottom
  and right edge (visible on a short document at 412 dp).

### 9.2 Where it lives
| What | Where |
|---|---|
| reading and editing state, tabs, the Back and layout reports, sheets, Edit button | `domains/docs/crates/docs-office/src/phone.rs`: `set_shell`, `take_phone_change`, `take_back` (:110-124), `can_edit` (:146), `is_editing` (:150), `reading` (:159), `Tab` and `tab` (:174), `sheet_rows`, `format_rows`, `chrome` (:630) |
| glyphs | `phone/glyph.rs` |
| the page takes no tap and no keyboard while reading | **patch wordcraft/0143**: `office_overlay.rs` `phone_reading` (:169) and the reset in `phone_chrome` (:180: `want_focus` and `focused` off, focus surrendered); `canvas.rs` `mouse` gate (:2333) and one line after `focus_click` (:1640) |
| the Edit button counts as chrome (its press must not reach the page) | docs-office leaves its rectangle in the egui context (`docs-office-phone-extra`), `phone_chrome` reads it (`office_overlay.rs:231`) |
| paper colour | `lib.rs:2276`, the phone branch of `WordApp::ui` |
| `wordcraft:phone { on }`, `wordcraft:back` | `host.rs` `sync` (:165-170); `web.rs:94` `set_shell`; the shell: `web/shell/shell.js` `bars` (:27) and the cases `phone` and `back` (:146), `shell.css` `#editor.phone > .bar` |
- Patch 0143: 5 files, +45/-9; hot files `canvas.rs` +3/-2 and `lib.rs` +3/-1. 0140 is unchanged (the third function pointer of `set_phone` is in 0143). `0950-wordcraft-rustfmt.patch` was regenerated (7 files, +53/-38; it now also formats `web.rs` and `office_overlay.rs`).
- The shell hides its header only while the editor says phone mode (`.phone`); desktop and the other editors keep it. An error (`#error`) shows the header again, it is the only place the message is. The "Saved <name>" status line is in that header too: on a phone it is the toast above.
- Strings: one new catalog row, "Done editing" (`i18n/wordcraft.tsv`, `reviewed:agent-sonnet:2026-11-02`, in `OWN` of `xtask/src/i18n/extract.rs`); "Text", "Paragraph", "Close" and "Back" were in the catalog.

### 9.3 Measured
| Item | Result |
|---|---|
| docs-office tests | 39 pass (5 new: reading first and the pencil, no pencil when the session cannot edit, Back and the layout change reported once, the Format tabs, the sheet's close button; the old tap tests now run in editing) |
| ui-egui tests (`cargo test -p wordcraft-ui-egui --lib`, upstream workspace) | 733 passed, the same 7 i18n failures as before this change (the cs, hu, hr, pl, tr, vi catalog tests and `untranslated_interface_text`) |
| `verify-queue.sh wordcraft`, `cargo xt register check`, `cargo xt i18n check-stale` | all OK (34 patches; 136 rows; catalogs current) |
| Docs wasm brotli (`ci.sh sizes`, `-q 11 --lgwin=24`) | 9,878,844 B, budget 9,904,416 B, **headroom 25,572 B** (WP6: 9,875,353 B, so +3,491 B); JS 20,836 of 24,576 B |
| screenshots | `evidence/phone-ux/`: 360x640 and 412x915 at DPR 1, light and dark, uz-Latn and ru, each: reading, edit (caret set), format-text, format-para, insert, menu, table, picture, reading-scrolled; `desktop-*`: 1280x800 with a mouse (the desktop layout, unchanged) |
- **Edit first (2026-11-03):** docs-office 39 tests pass (the reading-first test became "opens for editing, Done editing reads, the pencil edits, a replacing document edits"; the view-session test is unchanged). Docs wasm brotli 9,879,738 B (+894 B), headroom 24,678 B; shell 61,589 of 65,536 B (+274 B), headroom 3,947 B. `scripts/mobile/phone-editfirst.mjs` (Chromium, 360x640, light, uz-Latn): just opened = the editing bar and bottom toolbar, the editor frame's focused element is the canvas and not the text-agent input (after the first tap on the page it is the input); More > Save shows "Saqlandi: <name>" in `#toast` (`role=status`, class `on`), empty and opacity 0 about 3.6 s later; on a 1280x800 window the same save message sets the header status line only, the toast stays empty. Shots `evidence/phone-ux/360-light-uz-Latn-editfirst-{opened,saved,reading}.png`. Shell Playwright spec: 20 passed, 7 skipped, 1 failed once (`Docs round trip`, the `#ed-cover` text was already hidden when asserted, a timing race on a loaded machine) and passed 3 of 3 on a rerun.

### 9.4 Not verified, and what the Chromium proxy cannot say
- A real phone or the emulator: touch feel, whether the pencil is reachable with one thumb, the soft keyboard coming up on the first tap in editing (Chromium has no soft keyboard; in reading no input rectangle is sent, so no keyboard is asked for), Android WebView. No emulator was started (the rule of this task).
- The software-GL frames of headless Chromium take 0.5 s and more, so a 60 ms tap on a word is sometimes read as a long press (word selected, handles). The script therefore taps beside the text. A device is not expected to do this.
- (Closed 2026-11-03: documents now open in editing, see 9.1.) The soft keyboard not appearing by itself on open is reasoned from the code (no focus, no IME output), not seen on a device.
- The page's shadow lines at the bottom and right edge of a short page stay.
- Picture shots: the scripted taps are position based and the page scrolls when a caret is placed low on the screen, so in most `*-picture.png` the tap landed on a table row (row selected, table toolbar) and not on the picture. The picture selection with its toolbar (Wrap Text, Size, Delete, More) is in `412-light-uz-Latn-picture.png` (from a run with the same script, only `--table-y` differed). Table, edit, sheets, menu and reading shots were checked in all 8 sets.
