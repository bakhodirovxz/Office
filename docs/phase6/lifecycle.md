# Phase 6 WP12: lifecycle on Android (2026-11-03, branch `p6-lifecycle`)

Docs, on the emulator `HRM_Pixel_8` (API 36, Chrome 133.0.6943.137, Android System WebView 133.0.6943.137, Gboard 15.1), 412 dp. Same document and edit in
every case (`scripts/mobile/fixtures/phone-ux.docx`, synthetic; edit mode, caret at the end of the first heading, `lifecycle тест` typed with Gboard keys).
**Build used:** `E:/office-target/p6-mobileux/dist` (Docs is the read-first build; the pencil is tapped first) and the WP8 APK (`E:/office-target/p6-android/office.apk`,
built from the same dists). The rig drives the page over raw CDP (`scripts/mobile/lifecycle.mjs`): a recorder keeps every window message and visibility change, so the
timeline below survives a dead page. The display is `wm size 540x1200` / `wm density 210` (still 412 dp; native size made System UI stop responding, as in WP7), `RIG_SCALE=0.5`.
Case commands: Home = `am start -a MAIN -c HOME` (no `input keyevent`), back = `am start -n <activity>`, rotation = `settings put system user_rotation`,
kill = Home, 8 s, `am kill <pkg>`, relaunch; crash = CDP `Page.crash`; WebGL = CDP `WEBGL_lose_context.loseContext()` on the editor canvas.
No file was Saved in any run, so "lost" means lost since the document was opened (about 40 s before the event).

## Result: case x surface
| Case | Chrome (tab) | WP8 app (WebView) | Autosave (editor to shell) | Evidence (`E:/office-target/p6/evidence/<run>/`, SHA-256 in `evidence/SHA256SUMS`) |
|---|---|---|---|---|
| App switch (Home 8 s, back) | **kept** (typed text on screen, same document) | **kept** | `visibilitychange` hidden, `wordcraft:autosave` 0.4 s (Chrome) / 0.9 s (app) later | `22-01-31/chrome-home`, `21-47-10/app-home` |
| Rotation (landscape 8 s, portrait) | **kept** | **kept** (`configChanges`, no reload) | periodic one at 30 s after the first edit (29.9 s Chrome, 30.1 s app) | `22-02-37/chrome-rotate`, `21-48-21/app-rotate` |
| Background kill (`am kill`) | **lost**: all Chrome processes gone, the tab came back reloaded on the shell home (frames 0) | **lost**: process gone, the relaunched app shows the shell home, no editor | sent 0.3 s (app) / 0.5 s (Chrome) after hidden, then the process died | `22-03-54/chrome-kill`, `21-50-28/app-kill` |
| Renderer crash in the foreground (`Page.crash`) | **lost**: "Aw, Snap!"; Reload loads the shell home | **the whole app dies** (back on the launcher, no process): `MainActivity` has no `onRenderProcessGone` | none (a crash gives no hide event; the first periodic one is due at 30 s, the crash came at 10 s) | `22-09-05/chrome-crash`, `21-51-46/app-crash` |
| WebGL context loss | **lost in the UI**: blank canvas, shell shows "The editor could not start: webglcontextlost" | same | `wordcraft:failed webglcontextlost` and `wordcraft:autosave` in the same instant (0.0 to 0.1 s) | `22-23-20/chrome-glloss` (+ `-autosave.docx`), `21-54-03/app-glloss` |

Loss window as shipped (every "lost" row): **everything since the last Save**, unbounded: the edit survived in the editor's memory only. The test edit was 10 to 30 s old.

## Does the 0005/0127 autosave deliver the bytes to the APK host? No.
- Editor side works as designed (`patches/wordcraft/0005`, `0127`): `wordcraft:autosave {name, bytes}` 0.3 to 0.9 s after the page is hidden, at the same instant as a WebGL
  loss, and every 30 s (`AUTOSAVE_MS`) while dirty. The timelines (`<run>/<case>.json`) show it in all six runs that had a hide, a loss or 30 s of editing.
- **Shell side does not use it:** the message switch in `web/shell/shell.js:142-160` has no `autosave` case (`grep autosave web/shell/shell.js` finds nothing): the bytes reach the window and are dropped. Nothing reaches `SuiteHost` either (`MainActivity.java` has `pick`, `save` and no autosave method). So with the
  hooks that exist today the hand-off is half wired: a hidden tab that Chrome or Android then kills loses everything since the last Save.
- Payload check (`chrome-glloss-autosave.docx`, 5,752 B): it holds the typed text (`Lifecycle тест...`) but **no picture part** (the source file has `word/media/image1.png`, 49 kB, and the
  `<w:drawing>` has no blip). 10 of 11 autosaves in these runs were 5,752 B, one (Chrome rotation, the periodic one) 55,664 B. Cause not isolated (the picture was missing on screen after the edit in
  several "after" shots too, but my taps also hit the picture in at least one run, so that observation is confounded). Needs a check that an explicit Save after an edit keeps the picture; if only
  autosave drops it, a recovery from autosave would silently lose images.

## Proposal (not done: larger than a few lines, touches Java, an i18n string and a recovery UX)
1. **Shell (`web/shell/shell.js`)**: keep the latest `autosave` per frame (`f.auto = {name, bytes}`); on `failed` with `webglcontextlost`, reload that iframe and re-post `f.auto` (the editor already
   accepts `:open`), and flag the document as recovered and unsaved (one new i18n string in the four catalogs).
2. **Android (`MainActivity.java`)**: `SuiteHost.autosave(name, base64)` writes `files/autosave/<doc>.docx` (app-private, one file per open document, deleted on a successful save); on a cold start with
   such a file the shell offers "Recover unsaved work". That covers `am kill`, a crashed renderer and Android process death in the app; ~40 lines of Java, ~15 of JS.
3. **App: `WebViewClient.onRenderProcessGone`** returning true (reload the page) so a renderer crash does not kill the host; with 2, an autosaved document survives it. About 5 lines.
4. Chrome (web) has no host to hand the bytes to except IndexedDB (a shell-side store with the same recovery prompt); the 30 s period then bounds the loss window at 30 s, 0 s for app switch.
Window after the proposal: app switch and glloss about 0 s, hard kill or crash at most 30 s (`AUTOSAVE_MS`), plus the picture question above.

## Not verified
- Process death through the system's own memory pressure (the rig uses `am kill`); whether Android hands `onCreate` a saved-state bundle after it (`web.restoreState(b)`, `MainActivity.java:96-98`): the relaunched app showed the shell home, no document.
- A real low-memory Chrome tab discard, a real phone, other Chrome / WebView versions, `-gpu host`.
- Typing was Gboard (the Latin word came out as `Lifecycle`/`Ecycle` in some runs: Gboard drops a key now and then on the software-rendered emulator); the text, not the exact word, is the evidence.
- Chrome's tab count grew during the runs (one new tab per run); no effect seen.
- Recordings per case exist; the first `app home` run (a "System UI isn't responding" dialog at native size) was discarded.

# WP12 recovery (2026-11-03, branch `p6-recover`)

## The picture-less autosave: root cause
**Not a serialisation bug; the picture had been removed from the document by taps of the rig before the first key.** Evidence:
- The payload is faithful. `wordcraft:autosave` and File > Save serialise the same `session.doc` with the same writer
  (`wordcraft_engine::io::save_bytes_with`, same password and export options); the docx writer writes only the media a drawing references
  (`crates/docx/src/write/mod.rs:433-444`). In the 5,752 B payload the picture's paragraph is there but empty (`<w:p></w:p>`, the 4th empty
  paragraph after the table): the inline picture itself was gone from the model, and with it the media part.
- Reproduced on the emulator step by step (same dist, display, taps and Gboard keys as WP12; screenshot after every tap,
  `E:/office-target/p6/evidence/2026-11-02T23-10-picture-rootcause/`): **(1)** the tap on the round Edit button (`input tap` at the button, read-first build)
  leaves the picture selected (handles drawn, the bottom toolbar switches to the picture set; `02-pencil.png`): the tap reaches the page as well;
  **(2)** the next tap, on the first heading, *moves* the selected picture to the start of the document, and the document is already marked unsaved
  before any key (`03-caret.png`: picture above the heading, blue dirty dot, picture toolbar); **(3)** the first typed key replaces the selected
  picture (typing over a selected object replaces it, as in Word), so the text lands at the start of the heading (` Lifecycle тестPhone reading test`, the
  WP12 payloads exactly). The glloss payload of that run: 5,752 B, no media.
- In desktop Chromium (touch emulation, DPR 1) step (1) reproduces (after the Edit tap a typed `QQ` lands right after the picture, the caret is
  beside it, so `objects::selected` reports it), step (2) does not: the second tap places the caret and the picture stays.
- The one 55,664 B payload (Chrome rotation) is not explained: its typed screenshot shows the text where the picture was, yet the payload had a media
  part; the rig kept only the last payload of a run, so it cannot be checked.
- The current build (phase-6 HEAD, documents open in edit mode, no Edit tap) does not show it: in every re-run below the kept copy has the picture.

**Explicit Save cannot drop a picture the document holds**: test `office_overlay_tests::autosave_and_save_keep_the_picture_of_an_opened_document`
(wordcraft/0146, `crates/ui-egui/src/office_overlay_tests.rs`): a docx with a ~50 kB picture, opened through the inbox as `wordcraft:open` does,
edited at the start of the heading: the autosave payload (`office_overlay::autosave_bytes`, now the only code that builds it, `host.rs` uses it)
and an explicit `file.save` both contain the same `word/media/` part, byte for byte, and the edit; and once a typed key replaces the selected picture,
both lose it alike. Result: pass (1 test, 0.04 s). Desktop Chromium also: the docx saved after each recovery below holds `word/media/image1.png`
(49,270 B) and the typed text.

**Fixed afterwards** (branch `p6-tapfix`, section "Tap-through fix" below).

## What was built
- **Shell** (`web/shell/shell.js`, +27/-6 lines): `case 'autosave'` keeps the latest payload per editor frame (`f.auto`) and outside the page: the
  Android host's file (`SuiteHost.autosave`) or, in the plain web host, the shell's IndexedDB (`blob` store, key `rec:<app>`). Names mode (connected to a
  server, or the desktop host's browser fallback) keeps no copy outside the tab, as for Recent. After `failed: webglcontextlost` the next autosave
  (sent with the loss) reloads the frame and reopens that copy (`revive`). On every start the shell **reopens a kept copy by itself, without a
  question** (see "Why no Restore/Discard question" below). A reopened copy is posted with `dirty: true` and the editor keeps it unsaved
  (wordcraft/0146), so its leave-page guard and its autosave go on; the status line (and the toast on a phone) says `rec.done`. The copy is deleted
  only after a Save in its own format (the download path, or `hostSaved` ok) or when the user discards the changes: the existing "leave unsaved
  changes?" confirm when another document or New replaces it (while a copy is still on its way in, `f.rec`, that confirm is asked too).
- **Android host** (`MainActivity.java`, +70/-6): `SuiteHost.autosave/discard`, `GET /__recover/<app>`, write to a temp file + `fsync` + rename,
  `onRenderProcessGone` (returns true, new WebView, shell reloads and reopens the copy). `docs/phase6/android-app.md` "Recovery of unsaved work".
- **Editor** (wordcraft/0146, `host.rs` +13/-5): `wordcraft:open { dirty: true }`; the autosave bytes from `office_overlay::autosave_bytes`.
- **Strings:** one, `rec.done` "Recovered unsaved changes: {name}", in `i18n/shell.tsv` and `web/shell/i18n.json` (en, ru, uz-Latn, uz-Cyrl; Uzbek
  `reviewed:agent-opus:2026-11-03`).
- **Shell size:** 65,274 B brotli of 65,536 (headroom 262 B; +665 B on phase-6). No budget change.

### Why no Restore/Discard question
The first version asked with `confirm()` (OK reopens, Cancel deletes). On the emulator, in Chrome relaunched after `am kill`, **`confirm()` returned false without
being shown** (no dialog on the screen or in the recording; probably because the page loaded in a tab that was not in front, not isolated), so the
shell deleted the kept copy (two runs, `2026-11-03T01-13-12` and
`01-18-27`; the copy was in IndexedDB just before the kill, `55495` B, and gone after; strict IndexedDB durability did not change it). A browser may do
that for background tabs or after "prevent this page from creating more dialogs", so a Cancel cannot mean "delete". Reopening without a question has
no such path: the worst case is the same copy reopened again on the next start. With the change and default durability the copy came back (`01-23-52`).

## Re-run of the lost cases (emulator `HRM_Pixel_8`, same AVD, display, fixture and Gboard keys as above)
Build: phase-6 HEAD + this branch, `E:/office-target/p6-recover/dist` (Docs built here; Sheets, Slides, PDF copied from `p6-mobileux/dist`) and the APK
built from it (`E:/office-target/p6-recover/apk/office.apk`, 53,208,707 B). HEAD opens documents for editing, so the rig runs `--edit-first`
(no Edit tap; one tap at the end of the first heading, then the keys). "Kept copy" is what the next start gets (`<case>-recovery.docx`: Chrome's
IndexedDB, the app's `files/recover/docs`), checked for the typed text and `word/media/image1.png` (49,270 B).

| Case | Surface | Result | Kept copy (all: the typed text and `image1.png`, 49,270 B) | Evidence (`E:/office-target/p6/evidence/<run>/`) |
|---|---|---|---|---|
| WebGL context loss | Chrome | **recovered in place**: the frame reloads with the copy, unsaved (blue dot), "Recovered unsaved changes" | 55,495 B; autosave 0.1 s after the loss, page back 2.4 s later | `2026-11-03T00-03-35/chrome-glloss` |
| WebGL context loss | app | **recovered in place** (same) | 55,495 B; autosave with the loss, page back 2.5 s later (1.8 s in `00-22-31`) | `2026-11-03T01-45-13/app-glloss` |
| Background kill (`am kill`, 8 s after Home) | Chrome | **recovered**: Chrome relaunched, the page opened again, the shell reopens the document unsaved | 55,495 B; written 0.6 s after hide | `2026-11-03T01-23-52/chrome-kill` |
| Background kill (`am kill`, 8 s after Home) | app | **recovered**: the relaunched app reopens it unsaved | 55,495 B; written 0.6 s after hide (1.0 s in `00-44-53`) | `2026-11-03T01-32-55/app-kill` |
| Renderer crash right after typing (about 10 s after the first key) | Chrome | **lost**: no autosave yet (the first one is due 30 s after the first edit); "Aw, Snap!", the reloaded page has nothing to reopen | none | `2026-11-03T00-16-36/chrome-crash` |
| Renderer crash right after typing | app | **app survives** (`onRenderProcessGone`: new WebView, shell home; WP12: the whole app died); the edit is **lost** as in Chrome | none | `2026-11-03T00-28-48/app-crash` |
| Renderer crash 30 s after typing (`--before 30`) | Chrome | **recovered**: "Aw, Snap!", page opened again, document reopened unsaved | 55,495 B (periodic autosave at 29.6 s) | `2026-11-03T01-39-51/chrome-crash` |
| Renderer crash 30 s after typing (`--before 30`) | app | **recovered**: app survives, the new WebView reopens it unsaved | 55,495 B (periodic autosave at 30.2 s) | `2026-11-03T01-38-06/app-crash` |

App switch and rotation were kept before and were not re-run. Runs with the first (confirm) version, kept for the record: `00-34-11/app-crash`,
`00-44-53/app-kill`, `00-57-11/chrome-crash` (recovered after OK; in two of them OK was tapped by hand, the rig's `tapText` scaled display
coordinates by `RIG_SCALE` a second time, fixed in `lib.mjs`), `01-07-39`, `01-13-12`, `01-18-27` (Chrome kill: copy deleted by the unseen Cancel).
Desktop Chromium (412x860, touch, final shell): WebGL loss, reload after the 30 s autosave and renderer crash after it each reopen the edit unsaved;
the docx saved afterwards has the text and `image1.png`, and the Save deletes the kept copy.

## Loss window now (Docs; Chrome with the plain web host, and the app)
| Event | Before (WP12) | Now |
|---|---|---|
| App switch, rotation | none | none |
| WebGL context loss | everything since the last Save | none: the autosave goes with the loss, the page is back in about 2.5 s |
| Kill of a hidden page (Chrome, app) | everything since the last Save | none if the kill comes later than about 1 s after hide (copy written 0.6-1.0 s after hide); inside that second, up to 30 s |
| Renderer crash in the foreground | everything (the app died too) | **up to 30 s** (`AUTOSAVE_MS`): edits since the last periodic autosave; the first comes 30 s after the first edit |
| Server-connected shell (names mode), desktop host | everything since the last Save | reload or kill: unchanged (no copy outside the tab, by design, as for Recent); WebGL loss: none (memory) |

## Not verified (recovery)
- A real phone; Android's own low-memory kill and Chrome's tab discard (the rig uses `am kill`); other Chrome / WebView versions; power loss during the
  app's temp + fsync + rename.
- The picture move by a tap (step (2) of the root cause): seen only on the emulator, code path not isolated, not fixed.
- Two shell tabs of the plain web host share one copy per editor (`rec:docs`): the last autosave wins, and a second tab started while the first one
  edits reopens the first one's copy.
- A private window or blocked storage: IndexedDB fails, the shell keeps the copy in memory only (WebGL loss still recovers; reload and kill do not).
- The desktop host (WebView2) path: memory only, not run. Sheets, Slides and PDF send no autosave (none recovers; the shell code is app-neutral).
- The app's recovered document has no write-back URI after process death (Save goes through Save as); after a renderer crash it keeps it.
- Shell budget: 262 B left; the next shell change will need a budget decision.

## Tap-through fix (p6-tapfix, 2026-11-03)
The three steps above were one bug in two places, found with a debug build that logged every press at `canvas::mouse`, `phone_chrome` and
`objects::pointer` (read over CDP `Runtime.consoleAPICalled`, temporary, not in the tree):
- **Root cause (docs-office `phone::active`).** `active` clears "touched" when a pointer press arrives with no `Event::Touch` in its frame and the last
  touch is over a second old (`TOUCH_GRACE`); with no touch ever seen the "last touch" is minus infinity. Android Chrome delivers `pointerdown` in a frame of its
  own before `touchstart`, so a tap after a quiet second turned the phone layout **off for the press frame**: `phone_chrome` returned false (the desktop
  ribbon layout was drawn for that frame, `PHONE_READING` reset, `CHROME_DOWN` not set), and `canvas::mouse` saw the press with `chrome_down=false
  reading=false`. The press of a tap on the Edit button, which floats over the page, went to `objects::pointer` ("object press" in the log), selected the picture
  (the page scrolled to it) and began an object drag. The `wordcraft:phone` messages of the run show it: 9 toggles in one 6-step run, 1 after the fix.
- **Why the next tap moved it.** The release of that finger met the (now closed) gate in `canvas::mouse` and returned early, so the drag stayed in
  `canvas.obj_drag`. The next press (a heading) was taken as that drag's pointer (`primary_down`, far from where the drag began): the picture was dropped there
  (`arrange.bounds`), the document went dirty before any key, and the first typed key replaced the still selected picture. The log shows `obj_drag=true` at the
  heading's press. Not a threshold problem: a real drag needs `Drag::update` to cross it, and a stale drag was never reset.
- **Fix 1 (ours, `domains/docs/crates/docs-office/src/phone.rs`, `touched_after`).** On a device whose primary pointer is a finger (`(pointer: coarse)`, known
  at start) a pointer press never clears "touched"; a device with a mouse still switches after the grace. Test `a_coarse_pointer_device_keeps_the_phone_layout_on_a_pointer_press_without_a_touch`
  (the existing `a_pointer_event_after_a_touch_keeps_the_phone_layout_until_the_grace_is_over` still passes).
- **Fix 2 (wordcraft/0149, `canvas.rs` `mouse()`, 2 lines).** The gate that swallows a finger for the chrome (`chrome_down`, `phone_reading`) also drops
  `canvas.obj_drag`, so no path can leave a drag behind for the next tap. Test `objects::tests::a_touch_tap_on_text_does_not_move_the_selected_picture`
  (a finger's tap on text with a picture selected, press and release in one frame and in two: nothing moves, the caret is placed and the picture deselected; it passes
  without the patch as well, because the desktop-test harness never closes the gate, so it pins step 3 of the wish list, not the drag drop).
- **Emulator, before and after** (`HRM_Pixel_8`, Chrome 133, 540x1200 at density 210, `scripts/mobile/tapfix.mjs`: open `phone-ux.docx`, More, Done editing,
  tap the Edit button, tap the heading, type `tapfix` and `тест` with Gboard, Home; the editor's autosave payload is read in the shell). Screenshots
  `docs/phase6/evidence/tapfix/` (SHA-256 in `evidence/SHA256SUMS`):

  | | before (phase-6 `5925291`, `E:/office-target/p6-recover/dist`) | after (`E:/office-target/p6-tapfix/dist-final`) |
  |---|---|---|
  | 1. tap on the Edit button | picture selected, handles, page scrolled to it, picture toolbar (`before-04`) | editing starts, caret at the heading, no picture selected (`after-04`) |
  | 2. tap on the heading area | picture moved to the start of the document, unsaved dot (`before-05`) | a word is selected by the 0139 touch select, nothing moved (`after-05`) |
  | 3. typed text | ` Tapfix тестPhone reading test`, picture gone (`before-06`) | the selected word replaced by the typed text, picture still in the document (`after-06`) |
  | autosave payload on Home | 5,750 B, no `word/media/image1.png` | 55,490 B, has `word/media/image1.png` |
  | `wordcraft:phone` messages in the run | 9 | 1 |

  In the "after" run the second tap selected a word (a double-tap-like read of two taps a few seconds apart by 0139's long-press path, not investigated), and the typed
  text replaced that word as any selection is replaced: the picture is untouched, which is what was asked.
- **Not verified:** a real phone and a physical finger (the rig's `input tap` is a 1 ms tap; a long finger press may order its events differently), WebView in the
  Android app (same page code, not re-run), a phone with a mouse attached (it keeps the phone layout; the `?phone=off` setting still turns it off), the debug logs are
  not in the tree. Size: see STATUS.
