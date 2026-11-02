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
