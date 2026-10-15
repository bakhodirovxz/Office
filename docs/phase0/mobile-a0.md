# Spike 3, step A0: mobile-web baseline (wordcraft-web v0.7.0 in Chrome on an Android emulator)

Evidence only, zero build. Every claim below comes from a run on the emulator; files are in `docs/phase0/mobile/`.
Served by the Spike 4 docserver (`/wc`, stock v0.7.0 dist) plus `fake_hrm`. The HRM page `http://localhost:8790/` loaded the editor
through the `editor.html` shim, exactly as in `docserver-results.md`. Nothing under `upstream/` was touched.

## Environment
| Item | Value |
|---|---|
| AVD | HRM_Pixel_8, Android 16 (API 36) x86_64 Play image, 1080x2400 @ 420 dpi (412x915 dp), 4 GB RAM (`MemTotal` 4,013,812 kB) |
| GPU `-gpu host` | Emulator accepted the Intel UHD 630 (Vulkan found), but **Chrome rendered blank pages**: GPU process failed with `Failed to initialize Skia for SharedContextState` / `GrContext creation failed` (seen in `adb logcat`, log not kept). Unusable |
| GPU `-gpu swiftshader_indirect` | Chrome renders. WebGL2 `MAX_TEXTURE_SIZE` = **4096** |
| GPU `-gpu angle_indirect` | Rejected by this emulator ("not valid, switching to auto"), fell back to swiftshader |
| Chrome | 133.0.6943.137 |
| Gboard | 15.1.08.726012951-preload-x86_64, selected with `ime set`. `show_ime_with_hard_keyboard=1` made the soft keyboard show (VERIFIED). Languages added through Gboard settings: English (US) multilingual with O'zbek (Latin), Russian (ЙЦУКЕН) (`03-gboard-languages.png`). Glide typing and auto-correction left at defaults (auto-correction ON, `03-gboard-text-correction-settings.png`) |
| Chrome override needed | `chrome://flags` `enable-command-line-on-non-rooted-devices` = Enabled, plus `/data/local/tmp/chrome-command-line` = `_ --use-angle=swiftshader --enable-unsafe-swiftshader --disable-gpu-driver-bug-workarounds --ignore-gpu-blocklist`. Then WebGL2 `MAX_TEXTURE_SIZE` = 16384 (Chrome's own software GL, renderer string "ANGLE (Mesa, Vulkan ... llvmpipe)"). See Blocker 1 |
| Test harness | `adb input tap` on Gboard keys (coordinates from screenshots), `adb input motionevent`, Chrome DevTools socket (`adb forward localabstract:chrome_devtools_remote`) for JS state, tab control and `Input.synthesizePinchGesture`. Gboard languages and Chrome first-run state reset on every emulator restart (restarted twice) |

## Blocker 1 (found first): stock Chrome on this emulator cannot start wordcraft
`10-wc-fail-swiftshader-4096.png`: the editor shows `WordCraft failed to start: JsValue("Limit 'max_texture_dimension_2d' value 8192 is better than allowed 4096")`
and "A browser with WebGPU or WebGL2 is required". wordcraft's wgpu (WebGL2 backend) asks for 8192; the emulator's GL translator
reports 4096 (read through DevTools: `MAX_TEXTURE_SIZE` 4096, renderer "Android Emulator OpenGL ES Translator (Google SwiftShader)").
Reproduced 3 times. The override above is a Chrome command-line change, so **every wordcraft result below is for Chrome using its own
software GL, not the emulator GPU**. Real phones with a 4096 maximum texture size would hit the same hard failure; whether such phones
exist in the target fleet is **UNVERIFIED**. This is a wordcraft requirement, not a keyboard issue.
An "unsupported feature flag" infobar (from the command-line flag) shows in some screenshots and costs a strip of viewport.

## Results
Control = the same action in a plain `<textarea>` page in the same Chrome and Gboard (so keyboard-setup problems are not blamed on wordcraft).

| # | Test | Control (textarea) | wordcraft | Evidence | Notes |
|---|---|---|---|---|---|
| 1a | Load: scale and fit | n/a | **PASS scale, FAIL fit** | `11-load-editor-hrm-portrait.png`, `11-load-contact-sheet.png`, `11-load-screenrecord.mp4` | DPR 2.625, CSS viewport 412 px, so scale is correct. The UI is the desktop ribbon in a 412 px viewport: ribbon clipped after "References", status bar text overlaps, page clipped on the right (no touch path found to scroll horizontally). The brief's "wrong scale" is the desktop layout, not DPR |
| 1b | Load time | n/a | numbers below | contact sheet, HRM `readyMs` | |
| 1c | Renderer memory | n/a | numbers below | `12-meminfo-by-process.txt` | |
| 2 | Open a document (HRM flow) | n/a | **PASS** | `11-load-editor-hrm-portrait.png` | Fixture paragraph rendered, caret visible, 3 words |
| 3a | Tap in text, soft keyboard appears | PASS | **PASS** | `20-tap-text-keyboard.png`, `01-control-urlbar-kbd.png` | `mInputShown=true`. Keyboard shows autofill chips (key, card, pin) because the text agent is a generic input |
| 3b | Back hides the keyboard | PASS | **PASS** | `20-back-hides-keyboard.png` | Caveat: Back with the keyboard already hidden left Chrome (to another app on this emulator) and the editor was dead on return, see Lifecycle |
| 4b | Latin `hello world` | PASS ("Hello world", autocap) | **PASS** | `02-control-textarea-hello.png`, `21-wc-b-latin-line.png`, `22-saved-docx-text.txt` | In the saved docx |
| 4c | Cyrillic `Салом дунё` (ё via long-press popup) | PASS | **PASS** | `02-control-textarea-cyrillic.png`, `21-wc-c-cyrillic.png` | Composition works through the web text agent. Saved docx has `салом дунё` (U+0451 correct) |
| 4d | Uzbek `o‘g‘il` and the apostrophe variants | PASS | **PASS** | `02-control-textarea-uzbek.png`, `21-wc-d-uzbek-line.png`, `22-saved-docx-text.txt` | Gboard's O'zbek keys emit **U+02BB** (ʻ) for o‘/g‘ and **U+02BC** (ʼ) for the apostrophe key, never U+2018 or ASCII `'`. Both landed exactly. ASCII `'` through the IME was not tested; a hardware KEYCODE_APOSTROPHE came out as U+2019 in the document (smart quotes, which layer is UNVERIFIED) |
| 4e | Autocorrect `teh ` to `the ` | **Not reproducible**: Gboard did not autocorrect `teh`/`adn` on this emulator (gave `Teg`, `Adn`) | **FAIL, bug observed** | `21-wc-e-autocorrect.png`, `22-saved-docx-text.txt` | `teh ` and `wolrd ` became `tehthe` and `wolrdworld`: Gboard replaced the word (delete-surrounding plus commit) and wordcraft applied only the commit. Docx text: `tehthe wolrdwor`. This is the predicted unhandled `ImeEvent::DeleteSurrounding` (`ui-egui/src/keys.rs`). Gboard did try to correct inside wordcraft although it did not in the textarea |
| 4f | Glide / swipe typing | inconclusive | **UNVERIFIED** | `02-control-textarea-glide-trail.png` | `adb shell input motionevent` strokes are slow (about 100 ms per event). Gboard drew the glide trail but committed no word even in the control. Not attributable to wordcraft. Needs a manual recorded glide or a real device |
| 4g | Backspace into a committed word | PASS-ish (control deleted the whole word after one tap, Gboard composing behaviour, not a clean control) | **PASS** | `21-wc-g-backspace.png` | Three Backspace taps removed ` `, `d`, `l` one char at a time |
| 4h | Enter | the Gboard Enter tap inserted no newline in the control (anomaly, `02-control-textarea-enter.png`) | **PASS** | `21-wc-c-cyrillic.png`, `21-wc-h-enter-after.png` | New paragraph created, text after it landed, `ok` paragraph in the docx |
| 5a | One-finger swipe scroll | n/a | **FAIL** | `23-touch-1-swipe-up.png` | The swipe selects text (and raises the keyboard and the floating toolbar). No scroll, so no momentum to test. Matches the brief and `canvas.rs:1496-1529` |
| 5b | Pinch zoom | n/a | **PASS** | `23-touch-2b-pinch-143pct.png` | Synthesized pinch 1.6x: status bar 100% to 143%, browser visual-viewport scale stayed 1. Pinched back out too |
| 5c | Long-press selection | n/a | **PARTIAL** | `23-touch-3-longpress.png` | Selects the word and opens the desktop context menu (spelling, Cut/Copy/Paste, Font...). No native selection handles |
| 5d | Rotation | n/a | **PASS state, poor ergonomics** | `24-rotate-landscape.png`, `24-rotate-back-portrait.png` | Document, caret and word selection survive both rotations. Landscape shows the full ribbon but the page area is only about 60 dp tall, so editing is not practical |
| 6 | 360 dp (`wm size 1080x2280`, `wm density 480`) | n/a | **FAIL (overflow)** | `25-small-screen-360dp.png` | Ribbon cut at "Ref...", search box truncated, page clipped. The density change **reloaded the page** (unsaved edits lost). Reset afterwards (verified 1080x2400, 420) |
| 7 | HRM embed flow on mobile | n/a | **PASS** | `26-hrm-flow-typed-marker.png`, `26-hrm-flow-saved.png`, `26-verify-docx.txt`, `22-save-touch-file-save.png` | Typed `mobilea` and Cyrillic `салом` (the typed line also contains `xfkjc`, a mis-selected layout in my first attempt), saved by **touch via File > Save** (Backstage works), `received.jsonl` got a `status:6` line, `verify_docx.py` printed `OK ['mobilea', 'салом']`, HRM banner `saved v1`. Save to callback visible: about 1.1 s including adb tap latency. Hardware Ctrl+S also works when a keyboard is attached (callbacks 8 to 9); a phone user needs File > Save |

## Numbers
- **Load** (HRM page navigation to `onDocumentReady`, includes the shim's 500 ms settle): first runs 3857, 3961, 2892 ms; warm runs 2306, 2326, 1991 ms; after clearing Chrome's cache through DevTools 2287, 2432 ms. Loopback over `adb reverse`, so network cost is a lower bound; the wasm is 39,785,016 B uncompressed.
- Screenshot polling (`11-load-contact-sheet.png`, about 1 s sampling, counted from the `am start` call, which itself takes 1.8 s `TotalTime`): "Loading WordCraft..." at +5.9 s, editor chrome at +7.5 s, document text at +8.9 s.
- **Memory** (`dumpsys meminfo` per process, document open): renderer (`sandboxed_process0`) **PSS 284 MB / RSS 482 MB** right after load, **PSS 326 MB / RSS 529 MB** after typing and two saves. Main Chrome process PSS 103-116 MB, privileged process PSS 56-65 MB. First reading was taken before the per-process file was overwritten: `12-meminfo-chrome-after-load.txt` has only the main process. The emulator has 4 GB and ran near full (`MemFree` about 190 MB).

## Lifecycle problems seen (matters for "HRM-embedded on mobile")
- Leaving Chrome (opened Gboard settings, or Back with the keyboard hidden) and returning left the editor iframe as a broken-frame icon with a stale "ready" banner; edits were lost (`30-resume-after-app-switch-*.png`). `logcat` shows `GPU process has crashed` at those times (`30-logcat-renderer-gpu-events.txt`).
- A GPU-process crash on its own (`GPU process exited unexpectedly`, seen once while pressing Back to hide the keyboard) turned the editor grey and it never recovered (`21-wc-uz-ru-keyboard-hidden.png`). That happened with 7 editor tabs open on a 4 GB emulator; after closing the other tabs there was no repeat in about 25 minutes. Cause (memory pressure, SwiftShader in the GPU process, or missing context-restore handling) is **UNVERIFIED**; a real device must confirm.
- The page asks "Leave site? Changes you made may not be saved" on navigation, so wordcraft tracks dirty state itself (the shim cannot see it).
- Side effect to note: one Back press with the keyboard hidden left Chrome and landed in another app already on the emulator (an Expo Go HRM login screen); two stray taps and a typed `il` went into it before I noticed. Nothing was submitted, nothing signed in.

## Verdict
egui's web path gives **usable text entry on Android Chrome with Gboard**: Latin, Cyrillic (with the ё popup), both Uzbek apostrophe keys, Enter, Backspace, soft-keyboard show/hide, File > Save by touch and the full HRM round trip all pass and the saved docx matches. It is **not yet a usable mobile editor**.

Failures that look like **fixable glue or small upstream patches**: (1) autocorrect appends instead of replacing, because `ImeEvent::DeleteSurrounding` is unhandled (about 10 lines in `keys.rs`); (2) one-finger drag selects instead of scrolling (touch-to-wheel adapter, about 100 lines); (3) the caret is not scrolled into view above the keyboard (new lines land under it); (4) a desktop ribbon in a 360-412 dp viewport (needs a compact ribbon or mobile toolbar, which is real UI work); (5) wgpu asks for an 8192 texture limit instead of what the adapter offers; (6) lifecycle: keep or restore the document across app switches and recover from WebGL context loss.

Failures that look like **egui-core or platform limits**: no native selection handles or magnifier, a generic hidden input as the IME target (autofill chips, limited surrounding-text context for Gboard), software-GL cost (renderer 284-326 MB), and wgpu's behaviour on Android GL context loss.

Unverified: glide typing, ASCII `'` through the IME, real-GPU behaviour (`-gpu host` is unusable with Chrome here), whether the crashes are emulator-only, real-phone memory and speed.
