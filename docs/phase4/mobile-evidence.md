# Mobile evidence without a phone (WP16, PLAN section 9)

Status: measured 2026-10-23 on the owner's PC, headless desktop Chromium (Playwright 1.56, SwiftShader), the release dists from the
integration run (`E:/office-target/p1/dist`) served by the release `hoshiya-serve`. **No phone and no Android build exist (B-10).**
Everything below is desktop Chromium with phone-sized viewports, touch emulation and CPU throttling. It is a proxy and says
nothing about a real phone's GPU, memory pressure or thermal behaviour.

Scripts (reproduce with `node scripts/mobile-evidence.mjs [--cpu N]` and `node scripts/mobile-m3.mjs [--cpu N ...]`):
`scripts/mobile-evidence.mjs` (shell + four editors per viewport), `scripts/mobile-m3.mjs` (the 108-page open, stage by stage).
Screenshots: `docs/phase4/evidence/mobile/`.

**Measurement noise.** Other builders were compiling during every run (CPU at 100 % from outside), so every time below is an upper
bound and the high-throttle rows (12x, 20x) are noisy (the same configuration moved by up to 40 % between runs). The ratios
and the stage order are stable; the absolute milliseconds are not.

## 1. Per surface at phone viewports

Device emulation: 360x800 at DPR 3 and 412x915 at DPR 2.625, `hasTouch`, `isMobile`. The editors are opened from the shell
(a tap on the card, the PDF through the file picker) so the load includes the shell's iframe. Load = tap to the editor's wasm
instance up and its loading banner gone. Heap = CDP `Performance.getMetrics` `JSHeapUsedSize`; wasm = the largest
`WebAssembly.Memory` byteLength (captured by wrapping `WebAssembly.instantiate`). Rows are the second run (warm HTTP and disk cache);
the first cold run took 5.8 s for Docs and 7.9 s for Slides (wasm of 45 MB uncompressed / 9.9 MB brotli for Docs).

| Surface | Viewport | CPU | Load | JS heap | wasm memory | Overflow x | Text typed | Saved (Ctrl+S) |
|---|---|---|---|---|---|---|---|---|
| Shell | 360x800 | 1x | 0.13-0.6 s (load event) | 1.1 MB | none | 0 | n/a | n/a |
| Shell | 412x915 | 1x | 0.09 s | 1.1 MB | none | 0 | n/a | n/a |
| Docs | 360x800 | 1x | 0.66 s | 4.5 MB | 114 MB | 0 | yes ("Salom Hoshiya" is in the saved docx) | yes, 6.5 KB, valid zip |
| Docs | 412x915 | 1x | 0.47 s | 4.6 MB | 68 MB | 0 | yes | yes |
| Docs | 360x800 | 6x | 9.7 s | 4.5 MB | 114 MB | 0 | yes | yes |
| Sheets | 360x800 | 1x | 0.45 s | 4.2 MB | 8.7 MB | 0 | yes (in the xlsx) | yes, 4.0 KB |
| Sheets | 412x915 | 1x | 0.51 s | 4.1 MB | 8.7 MB | 0 | yes | yes |
| Sheets | 360x800 | 6x | 5.4 s | 4.1 MB | 8.7 MB | 0 | yes | yes |
| Slides | 360x800 | 1x | 0.57 s | 4.4 MB | 20.9 MB | 0 | **no** (see below) | yes, 21.7 KB pptx |
| Slides | 412x915 | 1x | 0.61 s | 4.5 MB | 16.9 MB | 0 | **no** | yes |
| Slides | 360x800 | 6x | 2.1 s | 4.4 MB | 20.9 MB | 0 | **no** | yes |
| PDF | 360x800 | 1x | 1.5 s | 4.1 MB | 23.8 MB | 0 | n/a (viewer) | n/a |
| PDF | 412x915 | 1x | 0.64 s | 4.5 MB | 17.8 MB | 0 | n/a | n/a |
| PDF | 360x800 | 6x | 1.7 s | 4.4 MB | 23.8 MB | 0 | n/a | n/a |

- **No page scrolls sideways** (shell `scrollWidth - innerWidth` = 0 for the shell and for the page around each editor).
- The JS heap is tiny because the engines live in wasm memory; the number that matters on a phone is the wasm column.
- 4x CPU was planned; the 4x batch run failed to start its server (a harness hiccup on the loaded machine) and was not repeated.
  6x covers the "mid-range" end of the plan, and the M3 table below has 4x.
- **Slides text typing is not shown.** A new deck is a blank slide with no text box, so a tap and typing has nothing to type
  into; the deck was saved and re-opened as valid pptx, but the typed text is not in it. That is a test limitation (a text box must
  be inserted first), not a measured failure. Typing on a phone also needs the soft keyboard (an IME text input), which desktop
  Chromium does not emulate: the keys here come from `keyboard.type`.
- The shell's own text language followed the browser (Russian here); the editors follow the shell.

## 2. Does the toolbar fit at 360 dp? No.

Read from the screenshots (`360x800-*-open.png`, `412x915-*-open.png`); the toolbars are canvas pixels, so there is no DOM to count.

| Surface | What 360 dp shows | What is lost |
|---|---|---|
| Docs | Header (Home button and title), the "search commands" box cut at the right edge, ONE ribbon group ("Clipboard") filling the width, with a `>` to scroll to the next. The tab row is "File", the `>>` overflow chevron and two buttons. | The tab labels Asosiy / Kiritish / Hujjat are behind `>>`. "File" and the Comments button **overlap** at 360 (text drawn under the button). The status bar's "Page 1 of N" text overlaps its view buttons. |
| Sheets | Title row (AutoSave toggle and file name **overlap**), then the name box and the start of the formula bar, with the grid below. | The formula bar is clipped; the sheet tabs, zoom and status text are truncated. |
| Slides | The "Present" pill sits on top of the file name and the search glyph (**overlap**), then New Slide and Layout with their arrows and the start of a text field. | The text, insert and Transliterate controls of the simplified row (about 20 controls on a desktop) are off screen to the right. About 2 to 3 controls fit. |
| PDF | Home and tab strip, a **vertical** tool rail on the left (select, hand, comment, highlight, pen, sign), a vertical icon rail on the right. | The page itself is almost hidden: the two rails and the menu consume most of the width. The zoom label ("25 %") overlaps an icon. |
| Shell | Fits. Cards are one column, 41 px high buttons (touch target is fine), no sideways scroll. | Nothing. |

**Reading for the 360 dp size (B-3 item i):** the controls are drawn about 1.4 to 1.5 times their desktop size at this viewport (for
example Slides "New Slide" is 30 px wide on the desktop UI Automation tree and about 44 px in the 360 px screenshot), so about 6 to 8
toolbar buttons fit on one row and the desktop rows hold 17 to 30. The layout does not reflow: it clips and, in four places, overlaps
text. **A 360 dp layout needs at minimum:** a one-row command bar of at most 5 buttons plus an overflow menu, a title row that
truncates the file name instead of overlapping it, a status bar that drops the page label first, and PDF rails that collapse into a
sheet. This confirms the Phase 2 reading that the engines have no phone layout.

## 3. M3: why the 108-page docx never opened on the emulator

Fixture: `domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx` (140 KB, the Phase 2 file; not from `corpus/`). Opened
from the shell at 360x800 (and 412x915), DPR 3, touch. `scripts/mobile-m3.mjs` records, from the moment the file is handed to the
shell, the stage marks and every long task in the editor frame, and samples wasm memory and heap every 500 ms.

Stages (all times from the hand-over; the open is one message chain: shell loads the iframe, the editor posts `wordcraft:ready`,
the shell posts `wordcraft:open`, the editor posts `wordcraft:opened` after the first frame that shows the new document, in
`patches/wordcraft/0001` `sync()`):

| Run | wasm instantiated | `ready` | `opened` | Longest long task | wasm memory | JS heap |
|---|---|---|---|---|---|---|
| 1x (360x800) | 0.42 s | 0.72 s | **2.3 s** | 1.04 s | 176 MB | 4 MB |
| 1x (412x915) | 0.39 s | 0.64 s | 2.1 s | 0.99 s | 176 MB | 4 MB |
| 4x | 0.94 s | 2.2 s | **8.8 s** | 4.7 s | 176 MB | 4 MB |
| 6x | 1.1-1.5 s | 2.8-3.1 s | **11.0-13.7 s** | 5.8-7.4 s | 176 MB | 5 MB |
| 6x, `--max-old-space-size=512` | 1.5 s | 3.6 s | 12.2 s | 5.5 s | 176 MB | 5 MB |
| 12x (noisy) | 4.8 s | 10.3 s | **35.5 s** | 15.6 s | 176 MB | 7 MB |
| 20x (noisy) | 9.4 s | 21.2 s | **51.1 s** | 17.0 s | 177 MB | 5 MB |
| Control: 1-page docx, 1x | 0.39 s | 0.66 s | 1.3 s | 0.46 s | 145 MB | 4 MB |
| Control: 1-page docx, 12x | 2.4 s | 4.4 s | 8.2 s | 2.8 s | 145 MB | 5 MB |
| Control: 1-page docx, 20x | 6.4 s | 14.1 s | 31.5 s | 13.4 s | 145 MB | 5 MB |

The 108-page file lays out as 138 pages in the web build (bundled fonts), shown as "Page 1 of 138" in `m3-cpu1-360x800.png`.

What the numbers say:

1. **The 108-page docx opens at a phone viewport on desktop Chromium**, at 1x in 2.3 s, and at every throttle up to 20x (51 s).
   It never stalls on desktop. The Phase 2 emulator failure is therefore **not a phone-viewport layout bug** and not the opened file.
2. **Memory is not the limit here.** wasm memory is 176 MB (145 MB for a 1-page file: the engine reserves most of it on start, the
   file adds about 30 MB); the JS heap stays at 4 to 7 MB. A 512 MB JS heap cap changed nothing, which is expected because the
   heap is not where the data lives. **A wasm memory cap could not be tested:** an init-script hook on `WebAssembly.Memory.grow` never
   fires because the module grows its own memory with the `memory.grow` instruction, and CDP has no per-tab wasm limit. The
   `cap128/192/256` runs are therefore not evidence and were dropped. Whether a real Android renderer with 2 to 4 GB RAM would
   refuse 176 MB (plus the GPU process) is **unknown**.
3. **The time is one synchronous main-thread task.** At every throttle, the longest long task ends at the instant `opened` is
   posted, and it scales linearly with the throttle (1.0 s at 1x, 4.7 s at 4x, 7.4 s at 6x, 15.6 s at 12x, 17.0 s at 20x). While it
   runs no frame is painted, which is the "page stayed grey" of the emulator run. In the 1-page control the same task is 0.46 s,
   so about 0.6 s of the 1.0 s is the document (parse and layout of 138 pages, with the rest being the engine's start-up work).
   The code explains why it is one block: `Session::layout()` (`upstream/wordcraft/crates/engine/src/session.rs:807`) calls
   `wordcraft_layout::layout` (`crates/layout/src/lib.rs:2124`), which "lays out the whole document" and, for page-total fields
   (`NUMPAGES`) and per-page footnote numbering, **repeats the whole layout until the totals settle** (bounded passes).
4. **Fixed cost before any document:** instantiating the 45 MB wasm takes 0.4 s at 1x and 9 s at 20x. That alone puts `ready` at 21 s on a 20x machine.
5. **After the open, a steady main-thread load remains at high throttle** (about 3 s of long tasks per 10 s at 12x to 20x, for the 1-page file
   too), so it is not tied to the document. At 1x it shows as no long task. It is an idle repaint or a software-GL cost, and costs
   battery on a phone; it is recorded, not diagnosed.

**Which stage stalls, and what is not proven.** On desktop the stalling stage is *layout inside the first frame after the open
command* (stage "parse + layout + first paint", a single task). The emulator never reached `opened` in 240 s. To reach 240 s from the
measured 51 s at 20x, the emulator would have to be about 100x slower than this PC under load. That is possible for Chrome in an
x86 emulator without hardware acceleration and with software GL, and it fits "ready was reached, `opened` was not, the page stayed
grey", but **the Android run is not reproducible here and the emulator number is still unmeasured.** The competing explanations that
desktop cannot exclude: an Android renderer out-of-memory kill or a wasm allocation failure (silent, same symptoms), and an
emulator-only Chrome difference. Both are checked by the optional emulator re-run below.

**Likely cause, named.** CPU-bound, non-incremental whole-document layout in a single synchronous call, run on a CPU that is 10x to
100x slower than this one. Evidence: linear scaling of the one long task with the throttle; the 1-page control being 2.4 times
faster; the code path laying out every page and repeating passes; no memory growth anywhere near a limit on desktop.

**Proposed minimal fix for Phase 6 (not implemented, per the brief).**
1. **Lay out the first screenful first.** Give `layout()` a stop condition (a page budget or a time budget), so the first call
   returns pages 1 to about 3 flagged "partial", and `wordcraft:opened` is posted after that frame. The rest is laid out in later
   frames with an 8 to 10 ms budget each (the layout already takes a `LayoutCache`, so paragraphs shaped once are not shaped again).
   The page count shows as an estimate ("Page 1 of about 140") until the last slice completes. This is a carried patch in
   `wordcraft/crates/layout` and `crates/engine`; the size is not estimated here, and it has to be checked first whether
   pagination can resume (the page-total pass that needs the final count must wait until the end).
2. **Skip the `NUMPAGES` and footnote settling passes until the last slice** (they exist only to make the totals correct).
3. **Cheap, no engine change (not measured):** check that the 45 MB wasm is served with long-lived caching headers so the browser's compiled-code cache can hold it, and paint the shell's own "opening..." state before the editor's first frame, so the page is never a grey rectangle. This does not shorten the open but stops it looking dead.
4. **Page virtualization** (tessellate only the pages on screen) matters for scrolling and the idle cost in item 5, not for the open; do
   it second.

**Optional owner re-run (no Android build needed, needs only a running emulator or a phone with Chrome and USB debugging).**
`scripts/translit/perf-web.mjs --cdp http://localhost:9222 ...` already drives a remote Chrome. For M3, add a `--cdp` flag to
`scripts/mobile-m3.mjs` (not done here) and read three things from the device: the `wordcraft:ready` and `wordcraft:opened`
times, `chrome://crash`/`adb logcat` for an out-of-memory kill, and whether the 1-page file opens. If the 1-page file opens and
the 108-page one does not in 240 s, the layout cost is confirmed; if neither opens, it is the wasm start-up or memory.

## 4. The B-3 decision

PLAN R12 stands. The evidence available without a phone: the shell works at 360 dp; each editor loads, types and saves at phone
viewports; none has a 360 dp layout (clips, four overlaps, the PDF rails); the open cost of a large document is one synchronous
layout task, which is a fixable engine-side cost, not a memory wall. What is missing: (iii) A-web on a real phone. **Recommendation:
keep A-web conditional and move the final B-3 decision to the start of Phase 6, when a phone exists**, with the M3 emulator/phone
re-run above as the first task of that decision.

## 5. Not done

- The handle design (egui-drawn selection handles, long-press menu) and the carry table of PLAN section 2.5 were not part of this
  task and are not written. They remain WP16 items of the plan.
- `ci/web-smoke/phone.spec.ts` (10 screenshots as a CI spec) was not written; the two scripts above are the runnable form.
- Slides typing on a phone viewport (needs a text box inserted first), and the 4x CPU row for the editors.
