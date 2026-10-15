# wordcraft-web patches: host bridge and mobile fixes (spike, S8-S9 plus mobile-a0 fixes)

Throwaway spike. Worktree `E:/office-work/wordcraft-spike`, branch `spike/bridge-mobile`, from upstream HEAD `fd0bb2f`.
Nothing under `upstream/wordcraft` was touched. Patches (`git format-patch`, English messages, no Co-Authored-By):
`spikes/wordcraft-patches/0001..0005-*.patch`. Total: 9 files, +651/-18 lines (about 200 of them are unit tests).
Build: `trunk build --release` to `E:/office-target/spike4/wc-bridge`, shared cargo target `E:/office-target/wordcraft`.

## Summary

| # | Patch | Diffstat | Upstreamable | Result |
|---|---|---|---|---|
| 1 | Host bridge (S8) | 4 files, +213/-8 | yes (third copy of the gridcraft/pdfcraft pattern) | bridge adapter green 3x on desktop; works on Android Chrome |
| 2 | Texture limit | 1 file, +13/-4 | yes | stock Chrome on the emulator: start failure before, starts after |
| 3 | IME DeleteSurrounding | 1 file, +37 | yes | `teh ` becomes `the ` (was `tehthe`) on Gboard |
| 4 | One-finger touch scroll | 4 files, +328/-7 (about 190 non-test) | yes, with the caveats below | swipe scrolls with momentum, tap places the caret, long press selects the word |
| 5 | Lifecycle autosave | 1 file, +65/-4 | yes (opt-in, host mode only) | HOME app switch: host holds the unsaved bytes about 4 s later; GL context loss recovered |

Wasm size, release build (raw / gzip -9 / brotli -q 11):

| | raw | gzip | brotli |
|---|---|---|---|
| Unpatched HEAD (given) | 42,597,532 | 15,849,354 | 9,441,597 |
| Patched (all five) | 42,618,253 | 15,874,939 | 9,450,860 |
| Delta | +20,721 (+0.05%) | +25,585 (+0.16%) | +9,263 (+0.10%) |

(Intermediate builds with the same patches measured +5.8 kB to +10.7 kB raw; wasm-opt output varies by a few kB between runs.)

## Patch 1: host bridge (`0001`)

What changes: new `apps/wordcraft-web/src/web/host.rs` (note: `web.rs` is a module, so the file lives under `web/`, not `src/`; the plan's path was wrong),
`web.rs` (`let bridge` at `web.rs:50`, `bridge.sync` at `web.rs:155`, `query_param`), `main.rs` doc line, `Cargo.toml` web-sys features `MessageEvent`, `UrlSearchParams`.
Activated by `?host=parent` (optional `&parent_origin=`). Messages (all prefixed `wordcraft:`):
`ready {version}` (to host), `open {name, bytes}` (from host), `opened {name}` (to host, added: a real completion signal, the shim had to guess with 500 ms),
`requestSave` (from host), `save {name, bytes}` (to host, transferred ArrayBuffer, for Save, Save As and exports),
`dirty {dirty}` (to host), `failed {error}`.
Origin check: a message is heard only if `event.origin` equals the configured parent origin (default: the page's own) and `event.source` is the parent window; everything is posted with that targetOrigin.
In host mode the page starts with a blank document (no File > Home) and does not install its own `beforeunload` guard (the parent guards, and the guard turned every recovery reload into a "Leave site?" prompt on Android: found on the emulator).
Found while testing and fixed in the patch: `requestSave` must request a repaint, otherwise the clean `dirty:false` is not reported until the next input.
Known timing behaviour: a requestSave is handled in `logic`, which runs before the frame's input pass, so keys typed in the same frame are not in the saved bytes (tests wait 800 ms after typing).
Upstreamable: yes. No new dependencies, no effect without `?host=parent` or without a parent.
Remaining limits: a failed host upload still leaves the document marked saved (async-save tickets, plan section 4.4 optional, not done); `opened` carries the name of the last `open` even after File > New.

Evidence (desktop Chromium 1.56.1, headless, SwiftShader, patched dist, `spikes/docserver/e2e`, 3 full runs of the spec per adapter after the last build):

| Adapter | Result | HRM page to ready | save to callback | notes |
|---|---|---|---|---|
| shim, patched dist (regression check) | 6 passed, 2 skipped (bridge-only) | 1350-1560 ms | 120-490 ms | S6 x3 and S7 (i)(ii)(iii) green |
| bridge, run 1 / 2 / 3 | 8 passed each | 1470-1780 ms | 200-290 ms (S6), requestSave to callback 330-360 ms | S6 x3, S7 x3, S9, lifecycle |

S9 (bridge): `dirty` sequence seen at the HRM `[false, true, false, true, false]`; callbacks received `[6, 6, 2]`: requestSave gives status 6, `/close` (docserver, new route) saves first if dirty and sends status 2 (status 4 if nothing was ever saved); close took 110-1040 ms.
Spike harness changes: `static/editor.html` (adapter=bridge), `static/api.js` (`requestSave()`, `close()`, `adapter` config key), `hrm/index.html`, `src/main.rs` (`/close`, `/autosave`), `e2e/spike4.spec.ts` (`ADAPTER=shim|bridge`, S9 and lifecycle tests).

## Patch 2: texture limit (`0002`)

Origin of 8192: `egui-wgpu-0.36.2/src/setup.rs:272` (`max_texture_dimension_2d: 8192` in the default `device_descriptor`, a registry crate, not wordcraft).
Fix: `apps/wordcraft-web/src/web.rs:39-46` wraps that descriptor and lowers the requested size to `adapter.limits().max_texture_dimension_2d`; all other limits stay egui-wgpu's. Also restructured the existing `?webgl` branch into the same `if let`.
Upstreamable: yes (could equally go into egui-wgpu; the wordcraft-side wrapper is 10 lines).
Evidence: before (unpatched HEAD dist, stock Chrome 133, emulator `-gpu swiftshader_indirect`, no flags): `WordCraft failed to start: JsValue("Limit 'max_texture_dimension_2d' value 8192 is better than allowed 4096")`, `docs/phase0/mobile-fixes/10-before-texture-limit-unpatched.png`.
After (patched dist, same Chrome, no flag override): editor ready, HRM `readyMs` 6-13 s warm, about 90 s the first time after `pm clear` (cold shader and wasm compile on a software GL; the same slowness also triggered "System UI isn't responding" ANR dialogs, hidden with `settings put global hide_error_dialogs 1`).
mobile-a0 left no flag file on the device (`/data/local/tmp/chrome-command-line` absent) but may have left the `chrome://flags` entry; `pm clear com.android.chrome` was run, so Chrome was stock.
Remaining limits: 4096 caps the canvas at 4096 px per side (a 5K window on such a GPU would clip); real phones unverified.

## Patch 3: IME DeleteSurrounding (`0003`)

Code path: eframe 0.36.2 `web/text_agent.rs:314` turns Gboard's delete-then-insert into `ImeEvent::DeleteSurrounding` followed by `Text`/`Preedit`; wordcraft's `crates/ui-egui/src/keys.rs` (`canvas_events`) ignored it, so the correction was appended.
Fix: `keys.rs:342` runs `text.backspace` `before_chars` times and `text.delete` `after_chars` times; `keys.rs:304` does the same with `equation.backspace/delete` while an equation is edited.
Test: `keys::tests::ime_delete_surrounding_replaces_the_word` (typed `teh `, delete 4, `the ` gives `the`; delete 1 before and 1 after works).
Upstreamable: yes.
Evidence: native test passes. Emulator, Gboard 15.1 (English, auto-correction on, tapped keys): `teh ` gave `HRM fixture paragraph the ` and `wolrd ` gave `... the world` (`31-autocorrect-teh.png`, `31-autocorrect-wolrd.png`); before (mobile-a0, 4e): `tehthe`, `wolrdworld`. Saved docx text through the bridge: `HRM fixture paragraph the world` (`32-saved-docx-text.txt`).
Remaining limits: each deleted character is one undo step; a selection makes the first backspace delete the selection; glide typing still unverified.

## Patch 4: one-finger touch scroll (`0004`)

Where: new `crates/ui-egui/src/touch_scroll.rs` (state machine plus 4 unit tests, reuses `scroll::Momentum`), `canvas.rs` (`touch_delta` at 1011 added to the scroll delta at 1017, edge handling at 1039, held press in `mouse()` at 2116-2122, long press as double-click at 2231, no focus request from a touch drag at 1465), `lib.rs` (module), `index.html` (`touch-action: none` on the canvas).
Behaviour: a finger's press is held back; moving past 8 points scrolls (content follows the finger, coasts after a flick); lifting replays the press at the landing point (tap places the caret); resting 0.8 s replays it as a double-click (long press selects the word, context menu opens, the drag after it extends by words); a second finger leaves pinch zoom alone. Mouse path untouched.
Things found on the emulator that shaped the patch (none visible in unit tests):
1. egui reports a touch press to the widget one frame late (`contains_pointer` false in the press frame), so the press is read from the input events.
2. `Event::Touch` can arrive a pass after the press, and a slow frame can hold press, moves and lift together; handled.
3. Sign: `clamped_scroll` subtracts the delta, so the finger delta is `now - before`.
4. Without `touch-action: none` Chrome may start a native pan and cancel the touch.
Upstreamable: yes, but about 190 non-test lines in a hot file; upstream may prefer it behind a setting. Does not need an engine change.
Evidence (Android emulator, stock Chrome 133, patched dist): fast flick (800 px in 120 ms) scrolls from page 1 to page 5 and coasts further than the finger travelled (`21-swipe-0/1/2*.png`), no text selected; before (mobile-a0, 5a): the swipe selected text and nothing scrolled. Tap places the caret (a0: tap selected a word); long press selects `fixture` and opens the menu (`40-longpress-1-after.png`). Unit tests: swipe with momentum, batched gesture in one frame, tap vs long press, mouse not held back.
Remaining limits:
- The soft keyboard still appears after a swipe when the canvas already holds the text focus: eframe's `touchend` re-focuses the text agent inside the gesture (eframe `events.rs`, "Fix virtual keyboard IOS"). Not changed.
- Touch-dragging a picture, a table handle or a ruler marker no longer moves it (the press is not delivered to those handlers); needs a hit-test hook. Read Mode and dialogs keep the old behaviour.
- Pinch zoom after a one-finger scroll start is not exercised; momentum numbers are from a 120 ms flick only.
- No native selection handles (egui core).

## Patch 5: lifecycle autosave (`0005`)

Where: `web/host.rs:133` `listen_lifecycle`, autosave in `sync` (`host.rs:106`). `visibilitychange` (hidden) or `pagehide` sets a flag; eframe keeps running `logic` in a hidden tab on a timer (eframe `events.rs:115-135`, `web_runner.rs:243-277`), and the next frame builds the docx bytes directly (not through `file.save`: the document stays dirty, nothing is downloaded) and posts `wordcraft:autosave {name, bytes}` if the document is dirty. `webglcontextlost` on the canvas posts `failed {error: "webglcontextlost"}` and an autosave.
Host side (spike `editor.html`): stores the bytes (`POST /api/session/{sid}/autosave`, kept as `autosave.docx`, no HRM callback) and, on context loss, reloads the editor frame and reopens the autosaved copy (`onRecovered`).
Upstreamable: yes (opt-in, host mode only).
Evidence:
- Desktop (synthetic hidden state, `document.hidden` overridden, `WEBGL_lose_context.loseContext()`): autosave stored in 75-280 ms, 6.5 kB docx containing the typed marker; after the context loss the wrapper reloaded and reopened the copy in 1.1-2.8 s and a requestSave then delivered the marker to the HRM. Green in every full run.
- Android emulator, real app switch (HOME key, Chrome kept alive): docx with the unsaved text reached the docserver about 4 s after the key press; coming back, the editor was intact (no broken frame icon, edits kept). Real `loseContext()` on the device: `webglcontextlost` reported, editor reloaded with the autosaved text in about 2.7 s, no "Leave site?" prompt after the guard change in patch 1 (`34-after-context-loss.png`).
What is not feasible or not proven:
- If the OS or Chrome kills the renderer without a prior hidden event, or the page is frozen before the timer tick (the autosave needs one timer tick after hiding, about 1 s while foreground timers run; Chrome throttles hidden timers), the host has nothing newer than the last autosave. A periodic autosave while dirty would close that gap (not done).
- `pagehide` autosave is best effort: the page may be gone before the next frame.
- Keys typed within about one frame before hiding are not in the bytes (a hidden tab runs no input pass).
- After recovery the reopened document is shown as clean although it differs from the HRM's version; the host must remember it is dirty.
- The mobile-a0 failures after leaving Chrome for a long time (renderer crash, 4 GB emulator under memory pressure) were not reproduced in these short absences; the crash cause is still unverified. During this spike a foreign 4 GB fuzz test on the same machine starved the emulator and caused ANR dialogs, unrelated to wordcraft.
- Real GPU and real phone behaviour untested.

## Test summary

| Check | Result |
|---|---|
| Native unit tests (`cargo test --release -p wordcraft-ui-egui --lib`, target `E:/office-target/wordcraft`) | 554 passed, 1 failed (`keys::tests::alt_numpad_code_types_its_character_without_keytips`: expects `é`, got `й`, it depends on this machine's Russian OEM code page, in alt-code code the patches do not touch; not re-run on the unpatched tree), 4 new touch tests and the new IME test pass |
| Desktop Chromium spike4 spec, adapter=shim | 6 passed, 2 skipped |
| Desktop Chromium spike4 spec, adapter=bridge, 3 runs | 8/8, 8/8, 8/8 |
| Android emulator (HRM_Pixel_8, API 36, stock Chrome 133, no flags) | start, autocorrect, swipe plus momentum, long press, HOME lifecycle, GL context loss, HRM save: all pass as described above |

Not re-tested from mobile-a0 (no patch touches them): ribbon overflow at 360-412 dp, caret not scrolled above the keyboard, glide typing, rotation, 360 dp.
Evidence files: `docs/phase0/mobile-fixes/` (3.8 MB). Servers, emulator, adb forwards stopped at the end; the `chrome://flags` entry and flag file are gone (Chrome data cleared); `E:/office-target/PAUSE` was not touched. Rule slip to note: three builds were started while an inventory `gridcraft-cli` rustc was running, and a foreign fuzz test used up to 4.5 GB during emulator runs.
