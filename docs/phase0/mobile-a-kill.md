# Spike 3, steps A1-A3: approach A kill test (egui `TextEdit` on native Android, Gboard)

Evidence only. Question: can Gboard-committed Cyrillic, Uzbek Latin, an autocorrected word and a glide-typed word reach an egui
`TextEdit` in a native Android eframe 0.36.2 app (winit 0.30.13, android-activity 0.6.1) on the emulator, (1) with stock
eframe/winit, (2) with the GameActivity text-state shim from plan 3A.3?
Everything below comes from runs on the emulator. Files: `docs/phase0/mobile-a/` (3.7 MB: screenshots, two short recordings,
per-test probe logs in `logs/`). Code: `spikes/mobile-a/probe/` (nothing under `upstream/` touched, nothing committed).

## Verdict (short)
- **Stock eframe/winit: FAIL.** Only (a) the keyboard show/hide works. For b-h nothing reaches the `TextEdit`: zero `Text` and zero `Ime` events
  in every run. The text goes to GameActivity's `TextInputState` (GameTextInput) and winit drops it. winit delivers at most `Key` release
  events (`Backspace`, hardware ASCII `H E L L O`), which egui ignores.
- **Stock + a ~170-line app-side shim (no winit / android-activity patch): PASS for a-h** in both runs (glow, and wgpu on the GL backend), with
  Gboard EN, RU and UZ layouts: Latin, `Салом дунё`, `oʻgʻil`, autocorrect `teh ` to `The `, glide typing, backspace, Enter.
- So approach A is **not killed on keyboard grounds**. Cost and risks are listed at the end. Two non-keyboard problems showed up that
  matter for A: wgpu's default (Vulkan) backend fails to start on the emulator, and the Activity hangs on destroy (ANR).
- Compared with the web path (A0 + our patches, `mobile-a0.md`, `wordcraft-patches.md`): the web path passes the same tests with a smaller
  change set (one ~37-line wordcraft patch for DeleteSurrounding), because eframe's web text agent already emits Commit/DeleteSurrounding.
  The native path needs the shim, which emits the **same egui events** (`Ime::Commit`, `Ime::DeleteSurrounding`, `Key::Enter`), so
  wordcraft patch 3 serves both.

## Environment and tool versions
| Item | Value |
|---|---|
| Host | Windows 10, 11.8 GB RAM, Git Bash. Other agents were building, so every build waited for no `rustc.exe` and at least 3 GB free |
| Rust | rustc 1.97.1, cargo 1.97.1, target `x86_64-linux-android` (`rustup target add`), host `stable-x86_64-pc-windows-gnu` |
| cargo-ndk | **4.1.2** (`cargo install cargo-ndk --locked --root E:/office-tools/cargo`, 2 m 24 s) |
| NDK / SDK | NDK 27.1.12297006, compileSdk/targetSdk 36, minSdk 26, build via `cargo ndk -t x86_64 -P 26 ... build --release -j 4`, `CARGO_TARGET_DIR=E:/office-target/mobile-a` |
| Crates | eframe 0.36.2 (`default-features=false`, `android-game-activity`, `wgpu`, `glow`, `default_fonts`), egui 0.36.2, egui-winit 0.36.2, egui-wgpu 0.36.2, egui_glow 0.36.2, winit 0.30.13, wgpu 30.0.1, glow 0.17.0, android-activity 0.6.1. The probe `Cargo.lock` was seeded from `upstream/wordcraft/Cargo.lock` (copy; the original is untouched), so versions are the wordcraft ones |
| Gradle | 9.3.1 wrapper, AGP 9.1.0, `androidx.games:games-activity:4.4.0`, JDK 17.0.19 Temurin from `~/.gradle/jdks`. **Not** the Android Studio JBR: Gradle's TLS failed with `PKIX path building failed` (corporate TLS interception), and the fix is `-Djavax.net.ssl.trustStoreType=Windows-ROOT`, which needs the `jdk.crypto.mscapi` module. The JBR 21 has none, Temurin 17 has it. Set in `GRADLE_OPTS` and `android/gradle.properties` |
| Scaffold licence | `spikes/mobile-a/probe/android/` is adapted from `rust-mobile/rust-android-examples/agdk-eframe` (build.gradle, wrapper, settings, MainActivity structure). **The repository has no LICENSE file** (GitHub API: `license: null`; README and the example README have no licence text). Test scaffold only, never ship it; licence of the borrowed lines is UNVERIFIED |
| Emulator | AVD HRM_Pixel_8, Android 16 (API 36) x86_64 Play image, `-no-snapshot-save -gpu swiftshader_indirect`, 1080x2400 @ 420 dpi, 4 GB. Gboard as in A0 (15.1; I did not re-query the exact build), layouts English (US), Русский, O'zbek (lotin), glide typing and auto-correction on (checked: Glide typing settings screen) |
| Leftover state from earlier spikes | `/data/local/tmp` empty (no `chrome-command-line`), `wm size`/`density` default, no adb reverse/forward rules. `show_ime_with_hard_keyboard=1` was already set and is required for any soft keyboard here; left as is. Chrome's `enable-command-line-on-non-rooted-devices` flag may still be "Enabled" in Chrome itself (chrome:// URLs cannot be opened by intent; I did not drive the Chrome UI); it is inert without the command-line file. The Expo "Uzgidro HRM" app was not touched |

### Probe app
- `spikes/mobile-a/probe`: `Cargo.toml`, `src/lib.rs` (354 lines), `android/` Gradle project (a `MainActivity extends GameActivity`, plus a
  `ControlActivity` with a stock multiline `EditText` used as the control).
- UI: checkbox "shim", one multiline `TextEdit`, label `text = ...`, and a log panel. Log lines: `winit> ...` = events in `RawInput.events` as winit/egui-winit delivered them, `GTI ...` =
  GameActivity text state (read in both modes), `shim> ...` = events the shim injected. The same lines go to logcat (tag `probe`).
- Mode and renderer are files in the app's data dir (`shim` on/off, `renderer` wgpu/glow, `backend` = `WGPU_BACKEND` value), so one APK serves every run.
- Harness (throwaway): `spikes/mobile-a/probe/scripts/` (key coordinates, a glide injector, `matrix.sh` that runs a-h and stores logs and screenshots).

## Which renderer renders (A2 done-check)
| Renderer | Result | Evidence |
|---|---|---|
| glow (GLES via EGL) | **renders**, UI visible | `p-00-start-glow.jpg` |
| wgpu default (Vulkan, SwiftShader device) | **fails at start**: `Limit 'max_uniform_buffer_binding_size' value 65536 is better than allowed 16384` (`RequestDeviceError`), app exits to the launcher. Same class of failure as the A0 texture-limit one: egui-wgpu requests default limits, not the adapter's | `logs/wgpu-vulkan-default-fail.txt` |
| wgpu with `WGPU_BACKEND=gl` (emulator GLES translator, SwiftShader) | **renders**; `Software rasterizer detected` warning | `p-00-start-wgpu-gl.jpg`, full a-h run `shim-wgpu+gl-*` |

## Result matrix
Key: PASS / FAIL for what reaches the thing under test. Control = a stock multiline `EditText` (`inputType` 0x2c001: text, multiline,
autocorrect, cap sentences), same keyboard, same taps. Stock = shim off. Shim = shim on (adds the same editor info as the control).
"Landed" = text visible in the egui `TextEdit` (`text = "..."` line). Taps are `adb shell input tap` on Gboard keys.

| Test | Control (EditText) | Stock eframe/winit | Stock + shim |
|---|---|---|---|
| (a) keyboard shows on tap, hides on Back | PASS. Shows; Back hides it (`mInputShown=false`) | PASS. Shows; Back hides it (the IME consumes it; winit logs only `Key BrowserBack up`); re-tap re-shows | PASS. Same; re-tap re-shows |
| (b) Latin `hello world` | PASS (`Hello world`, autocap) | **FAIL**. 0 `Text`/`Ime` events, TextEdit empty. GTI holds `hello world` | **PASS**. 11 `Ime::Commit`, landed |
| (c) `Салом дунё` (ё by long-press popup) | PASS (`салом дунё`) | **FAIL**. 0 events, TextEdit empty. GTI holds the text | **PASS**. `Commit` per letter, `Commit("ё")`, landed (`Салом дунё`) |
| (d) `oʻgʻil` (Gboard UZ layout) | PASS, `Oʻgʻil` with U+02BB (checked by code point) | **FAIL**. 0 events. GTI holds `oʻgʻil` | **PASS** for delivery: `Commit("oʻ")`, `Commit("gʻ")`, ... landed. egui's default font has **no glyph for U+02BB**: tofu boxes (rendering, not input; wordcraft's own fonts not checked, UNVERIFIED) |
| (e) autocorrect `teh ` to `the ` | PASS: `Teh` then, on the space, `removed=3 added=4` gives `The ` | **FAIL**. Gboard does autocorrect inside GameTextInput (`GTI text="the "`), egui gets nothing | **PASS**. `DeleteSurrounding{before_chars:2}` then `Commit("he ")`, landed `The `. Caveat below |
| (f) glide typing (`world`) | PASS (one change, `World`, 5 chars), with the injection described below | **FAIL**. GTI `... world`, egui gets nothing | **PASS**. `Commit(" world")` (one event), landed |
| (g) backspace into a committed word | PASS: one character per tap (after a glide, one tap removed the whole 5-char word, Gboard behaviour) | **FAIL**. winit gives only `Key Backspace` with `pressed:false` (egui ignores); the TextEdit is empty, so nothing to delete; GTI shrinks | **PASS**. `DeleteSurrounding{1}` per tap; winit's extra `Backspace up` is ignored, so no double delete |
| (h) Enter | PASS: a `\n` is inserted (multiline) | **FAIL**. Default editor info is single-line with a Done action (key shows a check mark, `inputType=0x1 imeOptions=0x2000006`). No key event, no text change at all | **PASS**. With multiline + no-enter-action the IME puts `\n` in GTI, the shim turns it into `Key::Enter`, newline landed. egui's `TextEdit` ignores `Commit("\n")` on purpose, so a raw commit would not work (`widgets/text_edit/builder.rs:1306-1316`) |

Other stock observation: hardware-style ASCII keys (`adb shell input text hello`) also do not type. winit delivered `Key H up`, `Key E up`, ... (releases
only) and no `Text`; GTI received the letters (`stock-glow-hwkeys-ascii.jpg`). The plan's prediction "at most ASCII key events arrive" was too optimistic.

Same shim run on **wgpu (GL backend)**: a-h identical (`logs/shim-wgpu+gl-*.txt`, `shim-wgpu+gl-*.jpg`). The stock matrix was run on glow only; the
layers involved (GameTextInput, winit input iterator) do not touch the renderer.

### How the control and tests were made reliable (so the numbers can be trusted)
- Control logs every `TextWatcher` change (`ControlActivity`, logcat tag `control`, also on screen: `c-02` to `c-09`). Control results were run before any probe test.
- **Glide**: `adb shell input motionevent` is too slow (A0 saw a trail but no word). Touches injected through the emulator console
  (`event send` on `virtio_input_multi_touch_1`, 14 ms per step, about 18 px spacing, dwell on the last key) are accepted: the trail is drawn and
  Gboard commits the word. A sparse path (`hello`, double letter, 31 points) was drawn but not committed, so the glide result needs a decent path. Glide of non-Latin words was not tried.
- **Do not use `adb shell input keyevent`** (also Back): it flips Gboard into its physical-keyboard compact toolbar (mic, backspace, enter, emoji, language
  label only) until `am force-stop com.google.android.inputmethod.latin` + `ime disable/enable/set` (or a reboot). This happens in the **control EditText too** (`c-09-reshow-after-back.jpg`),
  so it is an emulator `hw.keyboard=yes` / Gboard behaviour, not eframe. Back was therefore sent as an edge swipe gesture.
- Language switch: the globe key cycles in a recency-based order, so the harness detects the space-bar label by image comparison. The long-press language picker dialog takes window focus
  from the app, which makes egui drop focus and hide the IME (a lesson for any app: IME-side dialogs unfocus the egui `TextEdit`).
- Autocorrect depends on the language context of the field: after Uzbek or Russian text in the same field Gboard did not correct `teh` (seen in an early
  shim run). So (e) was run first, in an empty field with the EN layout, for control, stock and shim alike. The mixed-context case was not run on the control.
- Gboard never reported a composing region through GameTextInput in any run (`comp=Some` occurs 0 times in all logs), including during autocorrect. The `Preedit` branch of the shim therefore has **not** been exercised on a device.

## Shim design (A3)
Everything is in `spikes/mobile-a/probe/src/lib.rs`; no winit or android-activity patch.
- **Hook**: `eframe::App::raw_input_hook` (`eframe-0.36.2/src/epi.rs:279`) runs each frame before egui's input pass. It reads
  `AndroidApp::text_input_state()` (non-consuming, `android-activity-0.6.1/src/lib.rs:1088`) and appends events to `RawInput.events`.
- **Wake-up**: winit drops `TextEvent` (below), so nothing wakes the loop; the app calls `ctx.request_repaint_after(33 ms)` and polls. Latency at most one 33 ms tick plus a frame.
- **Index units**: GameTextInput's `selection` and `compose_region` are **Java UTF-16 code-unit offsets**, but android-activity passes them through as if they were Rust offsets. The shim converts to `char`
  indices (and back for `set_text_input_state`). The test strings are all BMP, so this was not exercised with surrogate pairs.
- **Translation** (`translate(old, new)`): split each state into committed text, cursor and preedit. If the tail after the cursor is unchanged: clear egui's inline preedit
  (`Preedit("")`), `DeleteSurrounding{before_chars: n}` for the removed suffix of the "before" text, `Commit(inserted)` (`"\n"` becomes `Key::Enter` down and up), then `Preedit(new preedit)`.
  Only the preedit changed: one `Preedit`. Cursor moved only: set egui's `TextEditState` cursor. Anything else (edit after the cursor): resync the IME from egui (logged, never seen).
- **egui to IME**: on focus gain, and whenever egui's text or cursor differs from what the IME was last told (taps, hardware keys), `set_text_input_state` pushes egui's state. This keeps Gboard's
  surrounding text right for autocorrect. egui inserts preedit text into the buffer itself, so egui's text and GTI's text stay equal.
- **Editor info**: `set_ime_editor_info(TEXT | MULTI_LINE | AUTO_CORRECT | CAP_SENTENCES, Unspecified, NO_FULLSCREEN | NO_ENTER_ACTION)` before `run_native`; Gboard logged `inputType=2c001 imeOptions=42000000`.
  Without it (stock default) Enter is a Done action and there is no autocap.
- **Size**: translator `lines 24-136` = 97 non-comment lines, glue (`egui_cursor`, `set_egui_cursor`, `shim_step`) 59, editor info 12, hook 3: about **170 lines** in one file.
- **Upstreamability**: the shim itself is app code. The proper fixes are small, in the registry sources:
  - `winit-0.30.13/src/platform_impl/android/mod.rs:476-478`: the catch-all `_ => warn!("Unknown android_activity input event ...")` swallows `InputEvent::TextEvent` and `TextAction`. A fix maps `TextEvent` to
    `WindowEvent::Ime(Preedit/Commit)` (compose region to `Preedit`, delta to `Commit`). **But `winit::event::Ime` (`event.rs:774-803`) has no `DeleteSurrounding`**, so autocorrect of already-committed text cannot be expressed
    without a new variant or a backspace emulation; that is the part a winit-only patch cannot solve.
  - `mod.rs:916` (`set_ime_cursor_area`) and `:926` (`set_ime_purpose`) are no-ops; `:918-924` `set_ime_allowed` always calls `show_soft_input(true)` (implicit). Purpose could map to `set_ime_editor_info`.
  - `android-activity-0.6.1/src/game_activity/mod.rs:723-736` (read) and `:343-349` (write): selection/compose offsets are UTF-16 units from Java but handled as byte offsets; should convert.
  - egui-winit 0.36.2 `lib.rs` `on_ime` (about line 699-760) turns winit `Ime::Preedit/Commit` into egui events and has no source for `DeleteSurrounding` on native.
  - The shim is a workaround for the missing winit path; `eframe::App::raw_input_hook` is the supported extension point, so it can ship in our shell crate with no fork.

## Numbers (probe, emulator, indicative only)
| Metric | Value |
|---|---|
| Startup, `am force-stop` then `am start -W`, cold, 5 runs, `TotalTime` | glow: 1587, 1650, 1672, 1770, 1897 ms, **median 1672** (an earlier set: 1560, 1804, 1357, 1385, 1892, median 1560). wgpu on GL: 1604, 1749, 1577, 1644, 1650 ms, **median 1644**. First start after boot: 4359 ms (not counted). This is the activity's first draw, not egui's first frame. CPU load from other agents was present |
| `dumpsys meminfo` TOTAL PSS, idle, probe | glow **54.2 MB** (RSS 178 MB); wgpu on GL **56.5 MB** (RSS 180 MB). After the typing runs: 54.6 / 58.1 MB. Java heap 10.7 MB, native heap 17.5-18.0 MB. "Graphics" reads 0 (the emulator's GL lives in the host process), so a phone will differ. Files `m-meminfo-*.txt`, `logs/*-meminfo.txt` |
| APK | **13.5 MB** debug-signed, x86_64 only (dex 6.8 MB, `.so` 10.3 MB stored uncompressed). AGP's incremental packaging had left a 19.2 MB file with dead bytes; deleting the output and repackaging gives 13.5 MB |
| `libmain.so` | stripped (`llvm-strip --strip-unneeded`) **10.27 MB** (4.37 MB deflated); as built by cargo (release, `opt-level=s`, thin LTO, `strip=false`) 15.99 MB; AGP could not strip it itself ("Unable to strip libmain.so") |
| Build times | cargo-ndk release, cold: 228 s for the dependency tree plus 62 s for the probe crate and link; later incremental rebuilds 1-2.5 min (some of it waiting on the cargo package-cache lock); first Gradle build 2 m 25 s, then about 30 s |
| Shim cost | not measured separately; the app polls at 30 Hz while running (also in stock mode, to log GTI) |

## What else I saw (matters for A)
1. **Activity destroy hangs.** Two times (an Activity relaunch, `handleRelaunchActivity` in the trace, before I widened `configChanges`, cause not identified; and, most likely, a Back swipe sent while the keyboard was already hidden, which finishes the activity) the main thread
   sat in `GameActivity.onDestroy` -> native `onDestroy` -> `pthread_cond_wait`, while `android_main` sat in `ALooper_pollOnce` inside winit's `pump_events` (`p-anr-relaunch-trace.txt`, trimmed ANR dump, main and `android_main` threads). Result: `ANR ... FocusEvent(hasFocus=false)` and
   `Activity destroy timeout`. It happened in stock mode; I did not isolate whether it is eframe, winit or android-activity. Any Activity recreation (rotation without `configChanges`, locale change, the system killing a task) can hit it. Needs its own investigation before A is credible.
2. The probe never confirmed a `Pointer`/touch-scroll story, selection handles or a ribbon layout; this spike is keyboard only.
3. Showing the IME again after a Back hide worked by tapping the field (egui toggles `set_ime_allowed` on focus change).

## Comparison with the web path (approach A0 plus our patches)
| | Web (A0 + patches 2, 3) | Native A, stock | Native A + shim |
|---|---|---|---|
| Latin, Cyrillic, Uzbek | PASS (A0) | FAIL | PASS |
| Autocorrect | PASS after wordcraft patch 3 (about 37 lines, `teh ` to `the `) | FAIL | PASS (shim; same egui event as the web path) |
| Glide typing | UNVERIFIED (A0 could not inject one; the injection method found here could be reused) | FAIL | PASS |
| Backspace, Enter | PASS | FAIL | PASS |
| Keyboard show/hide | PASS | PASS | PASS |
| Extra code | none for the keyboard (web text agent is eframe's) | n/a | about 170 lines in our shell crate, no upstream fork |
| Other costs seen | WebGL texture limit (patch 2), Chrome GPU/lifecycle crashes, 284-326 MB renderer PSS, desktop ribbon | wgpu Vulkan limit failure on emulator, Activity destroy hang | same |

**Recommendation for the keyboard question:** A is viable for text entry, provided we keep the shim (or an upstream equivalent) and fix the destroy hang. The web path is still ahead on effort for the keyboard
(nothing to write) and on fidelity evidence (HRM flow passed), while A has a 54-58 MB PSS probe and no browser, but it inherits the lifecycle problem and all the unsolved mobile-UI work from the plan (ribbon, touch scroll, selection handles).
This test does not decide A against B or the web path; it removes "native egui cannot take Gboard input" as a reason to reject A.

## Not done / unverified
- Preedit (inline composition) path of the shim: Gboard never exposed a composing region here.
- Surrogate-pair (emoji) offsets; mixed-language autocorrect; glide for Cyrillic/Uzbek words; selection replace by IME; long-press/handles.
- Real device: everything is emulator + SwiftShader, Gboard with a virtual hardware keyboard present.
- Stock matrix on wgpu(GL); wordcraft's own `TextEdit`-less canvas input (`keys.rs:313-330`) was not run, only an egui `TextEdit`.
- Clean-up done: emulator shut down (`adb emu kill`, no emulator process left), no adb reverse/forward rules existed, nothing pushed to `/data/local/tmp`, `E:/office-target/PAUSE` untouched.
- Build outputs not meant for git: `android/app/src/main/jniLibs/`, `android/app/build/` (see `probe/.gitignore`).
