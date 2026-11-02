# Phase 6 WP7: keyboard, IME and viewport (2026-11-02, branch `p6-kbd`)

Plan section 6, items 1-3 (item 4 is a regression row). Docs on the emulator `HRM_360` (360 dp), Chrome 133, Gboard 15.1.08, dist built from `phase-6`
HEAD (137b877, wordcraft 0139 handles included). Synthetic text only (`scripts/mobile/fixtures/blank.docx` is a generated empty document).
Recordings are in `docs/phase6/evidence/kbd/` (SHA-256 in `docs/phase6/evidence/SHA256SUMS`), the scripts in `scripts/mobile/`.

## Rig change that every number below depends on
At the native 1080x2400 @ 480 dpi the emulator's software GL kept all 12 host cores busy while the editor was open (qemu 800-1400 % CPU, 63 % with
`about:blank`); the guest then showed "System UI / Chrome isn't responding" and lost taps. The runs therefore use `wm size 540x1200` + `wm density 240`
(still **360 dp**, device pixel ratio 1.5 instead of 3; set before the page loads, the density change restarts Chrome). The rig keeps all coordinates in
1080x2400 panel space and scales them with `RIG_SCALE=0.5` (`lib.mjs`). Not tested: DPR 3 at 360 dp (the keyboard logic does not depend on it).
`hw.cpu.ncore` of `HRM_360` was raised from 4 to 8 in the AVD's `config.ini` (not changed in `create360`).

## 1. The A0 keyboard matrix on HEAD
`kbd-matrix.mjs`: rows typed on Gboard keys (maps in `lib.mjs`: EN, RU, UZ, read off `rig.mjs shot`; EN is identical at 360 and 412 dp), one recording per
row, then File > Save and the docx read from the blob the shell hands to the download (`matrix/matrix-docs-360.txt`, the docx next to it).
Gboard capitalises the first letter of a paragraph (it saw an empty field), so `Hello`, `Ok`, `Салом` are capitalised; that is Gboard, not the editor.

| Row | Typed | Saved docx paragraph | Result | Evidence |
|---|---|---|---|---|
| Uzbek, U+02BB / U+02BC | `oʻ` key, `gʻ` key, `i`, `l`, apostrophe key | `Oʻgʻilʼ` = U+02BB U+02BB U+02BC (code points checked) | PASS | `kbd-uz-apostrophes-360.mp4`, `matrix/uz-apostrophes.png` |
| Latin | `hello world` | `Hello world` | PASS | `kbd-latin-360.mp4` |
| Autocorrect | `wolrd` + space | `world ` (replaced, not `wolrdworld` as in A0 4e) | PASS (F8: upstream `DeleteSurrounding` works with Gboard) | `kbd-autocorrect-360.mp4` |
| Glide | glide `hello` (console touch injection) | `Hello` | PASS (no stray letters at 360 dp) | `kbd-glide-360.mp4` |
| Backspace | `xyz`, Backspace | `Xy` | PASS | `kbd-backspace-360.mp4` |
| Cyrillic with ё | `салом дун`, hold `е`, slide to `ё` | `Салом дунё` (U+0451) | PASS | `kbd-cyrillic-yo-360.mp4`, `matrix/cyrillic-yo-popup.png` (popup: е́ 5 ё ъ) |
| Enter | `ok`, Enter, `no` (and Enter between every row) | separate paragraphs `Ok` / `no`; each row is its own paragraph | PASS (item 4 regression row: the `<input>` agent still makes paragraphs) | `kbd-enter-360.mp4` |

Run twice with the same text: on the unpatched dist (`matrix/matrix-docs-360-head-baseline.*`; its recordings were replaced) and on the dist with the page fix
of section 2 (`matrix/matrix-docs-360.*`; the recordings kept). Both give the same text.
Layout switching: Gboard orders its languages by recent use, so the rig fingerprints the key area (`gboard-fp.json`) and presses the globe until the wanted
layout shows. Russian and Uzbek (lotin) were added through Gboard's own Settings > Languages UI; the Android `enabled_input_methods` hashes of WP1 are overwritten by Gboard.

## 2. "Keyboard reappears after a swipe" (F5): cause and fix
Baseline (`swipe/baseline*.png`, `kbd-swipe-baseline-360.mp4`): tap in text (keyboard up), hide it with Gboard's chevron (`mInputShown=false`), swipe-scroll:
**keyboard shown again** after the first swipe. Reproduced in every run (4).

Cause, proven in three steps (`kbd-probe.mjs` logs focus, blur and touch events inside the Docs frame):
1. F5 as written is real but is not the root. On a swipe the log shows `pointerup`, `canvas.focus()` (eframe `events.rs:603`), the agent blurs, the frame calls `agent.focus()`,
   then `touchend` does `blur()` + `focus()` (`events.rs:797-804`). With all of those focus calls turned into no-ops the keyboard **still** came back.
2. Blocking touch events from eframe (a capture listener with `stopImmediatePropagation`) kept the keyboard hidden; blocking pointer events did not.
3. **Control without egui:** a plain page with a focused `<input>` and non-passive `preventDefault()` on `touchstart`, `touchmove` and `touchend` re-raises the keyboard
   after a swipe in the same Chrome; with `preventDefault()` on only one of the three, or none, it does not. eframe prevents default on all three touch events
   (`events.rs` touchstart/touchmove/touchend handlers; `should_prevent_default` is true for touches) and keeps its text agent focused after the user hid the keyboard
   (Chrome does not blur an input when the keyboard is dismissed). So the root is Chrome's reaction to a fully consumed touch sequence over a focused text input;
   the eframe refocus calls are a second path to the same result.

Plan candidates: (a) and (b) were not needed in their stated form; (c) (an eframe fork) was **not** needed. Chosen, page code only
(`domains/*/apps/*-web/index.html.in`, one inline script, identical in the four editors; no upstream change, **no patch number used**):
- when the visual viewport grows (keyboard hidden; read from `top.visualViewport`, the shell, same origin; the frame's own viewport as a fallback) the text agent is blurred.
  eframe then sees no focus (`has_focus()` is false) and does not refocus it, so a swipe over an unfocused page raises nothing;
- while a touch has moved 10 px, `focus()`/`blur()` of the canvas and the agent are no-ops until just after its `touchend` (a scroll or a handle drag, WP5, cannot refocus it);
- a tap behaves as before: `canvas.focus()` on pointer-up, then the frame's `agent.focus()` with the tap's activation raises the keyboard again.

After the fix (`swipe/fixed*.png`, `kbd-swipe-fixed-360.mp4`): tap = shown, chevron = hidden, **two swipes = still hidden**, tap in text = shown. Typing after the tap works
(the second matrix run was on this build). Unverified: Android **WebView** (the APK, WP8, and the HRM app) was not run, Chrome only; the inline script needs a new CSP
hash, which `hoshiya-serve` derives from the dist's `index.html` at start (checked: the page was blocked until the server was restarted, then ran).

## 3. Caret above the keyboard
`kbd-caret.mjs`: 45 times (letter, Enter) in a blank document with the keyboard up, so the caret walks past the first screen onto page 2
(`caret/fixed-end.png`, `kbd-caret-fixed-360.mp4`): the caret stays in the visible part of the canvas, above the status bar and the keyboard (y 530 of 680 px in the 540x1200 screenshot).
Chrome's default (`resizes-visual`) shrinks the visual viewport and scrolls the shell so the canvas bottom sits on the keyboard, and wordcraft scrolls the caret into view. **PASS as is.**

Departure from the plan text: `interactive-widget=resizes-content` (shell `index.html`, `editor.html`) was tried and is **not shipped**. With the desktop layout still on the phone
(ribbon about 540 px tall at 360 dp) the editor region shrinks to about 120 px while the keyboard is up and Docs drops to 22 % zoom (screenshots of that experiment not kept).
Re-test it together with the phone layout (WP2). The APK's `adjustResize` (WP8) is the same question for a WebView.

## Files and size
`scripts/mobile/`: `kbd-matrix.mjs`, `kbd-swipe.mjs`, `kbd-caret.mjs`, `kbd-probe.mjs` (diagnostic), `session.mjs` (a session with fixed commands only: open, blob, reload),
`gboard-fp.json`, `fixtures/blank.docx`; `lib.mjs` gained `RIG_SCALE`, `KEY`, `RU_LAYOUT`, `UZ_LAYOUT`, `kbdFingerprint`, `hold` (touch-and-hold with a popup pick).
Size: wasm and JS unchanged (the budgets count only those); `index.html` of each editor grows by 718 B brotli (875 to 1593 B, measured on Docs).
