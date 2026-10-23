# Accessibility baseline (WP18, PLAN 4.10)

Measured 2026-10-23. What was run, with what, and what could not be run:

| Check | Tool | Result |
|---|---|---|
| Keyboard walk, shell and converter (web) | `scripts/a11y-web.mjs` (Playwright, real Tab key presses, light and dark scheme) | Run. Evidence: `docs/phase4/evidence/a11y/web-a11y.json` |
| Contrast of the shell and converter tokens | same script, WCAG 2.x relative luminance on the CSS tokens | Run, every text pair listed in section 3 |
| Screen reader names, desktop apps | `scripts/uia-dump.ps1` (Windows UI Automation, `System.Windows.Automation`, the same tree Narrator reads) on the four release apps | Run. Dumps: `docs/phase4/evidence/a11y/uia-{docs,sheets,slides,pdf}-before.txt` |
| Web editor accessibility tree | Playwright `ariaSnapshot` and a DOM query of the editor iframe | Run |
| Keyboard-only walk of the desktop editors with real key presses | `scripts/uia-dump.ps1 -Tabs N` (SendKeys) | **Not run.** The Windows session was locked (the lock screen was the foreground window; the first Tab walk landed on the lock screen's widgets). The script is ready for an unlocked desktop. |
| Narrator itself (desktop apps, web canvas in Edge) | Narrator | **Not run** (needs the interactive desktop; same reason). UI Automation is Narrator's source, so the names below are what it would read, but speech output is unverified. |

## 1. Pass / fail matrix

| Surface | Tab order | Focus visible | Every control reachable | Names | Escape | Contrast |
|---|---|---|---|---|---|---|
| Shell (web, 13 stops) | pass (DOM order: search, language, 4 cards, Open file, 5 templates, converter link) | pass, 3 px outline on all 13 | pass | pass after fix (repeated "New" / "Use" now carry the app or template name) | no dialogs; Escape does nothing, nothing to close | pass (section 3) |
| Converter page (web) | pass; 3 stops until a file is chosen (language, connection summary, the file chooser), the option controls appear after (10 stops: all with a 3 px ring and a label) | pass | pass | pass (every field has a label or `aria-describedby`) | n/a | pass |
| Web editors in the shell (iframe) | **fail**: Tab goes into the iframe and never comes out (30 presses, Shift+Tab and Escape included) | not testable (canvas) | **fail** (nothing inside is a DOM control) | **fail**: the iframe exposes one unnamed `textbox` (the IME input) and a canvas; zero `role` or `aria-label` nodes | Escape does not leave the editor | n/a |
| Docs desktop (UIA) | tree order = drawing order (ribbon left to right, then the document) | not run (needs key input) | 110 nodes | mixed: 57 named buttons; 44 unnamed interactive nodes | not run | see section 3 |
| Sheets desktop (UIA) | same | not run | 64 nodes | 28 named buttons; 31 unnamed interactive | not run | |
| Slides desktop (UIA) | same | not run | 62 nodes | 28 named buttons; 19 unnamed (18 after the fix) | not run | |
| PDF desktop (UIA) | same | not run | 67 nodes | 54 named buttons; 4 unnamed (2 `Custom`, 1 `Edit`, 1 more `Custom` panel) | not run | |

The desktop apps do expose a real tree: window, buttons with the translated label (for example Docs "Qalin", "Kursiv"; Sheets
"Bold", "Fill Color", "Fill Color ▾"), toggles, a `Document` node (Docs) and `Text`. Disabled controls report `IsEnabled=False`
(Slides Bold/Italic with nothing selected).

## 2. Findings

Severity: High = blocks a screen-reader or keyboard-only user from a task; Medium = a task is harder; Low = polish.

| # | Issue | Severity | Status | Phase |
|---|---|---|---|---|
| A1 | **The web editors are invisible to a screen reader.** The editor iframe's DOM is a `canvas` and one hidden `input` (`ariaSnapshot` = `- textbox`, 0 nodes with `role` or `aria-label`). AccessKit is compiled in (F17) but produces no DOM on the web. Everything the desktop UIA tree shows is missing on web, and so in the shell and in the WebView2 desktop host, which loads the same web editors. | High | **Deferred.** Needs a web accessibility layer (an AccessKit web adapter or a hidden DOM mirror of the command palette and toolbar). Not a cheap fix. | Decision at the Phase 6 start with B-3; at least a keyboard-accessible command palette first |
| A2 | **Keyboard trap in a web editor.** Once focus is in the iframe, Tab (30 presses), Shift+Tab and Escape never return to the shell, so "Home" is unreachable by keyboard (the browser's F6 pane switch is the only way out). WCAG 2.1.2. | High | **Deferred.** Cheap in principle, but needs a patch in the editor side of the host bridge (a `focus-out` message on a chosen key, handled by the shell), which touches the shared bridge patches (wordcraft 0001, deckcraft 0002, gridcraft and pdfcraft bridges) that the WP10/WP11 builder is changing now. | Phase 5 (with the bridge changes) |
| A3 | **Shell: repeated button text.** Three "New" buttons, five "Use" buttons and "Open PDF" had no accessible name beyond the visible word, so a button list read "New, New, New". | Medium | **Fixed.** `aria-label` = "<visible text>: <app or template name>" (the visible text stays first, WCAG 2.5.3). `web/shell/shell.js`; asserted in `ci/web-smoke/shell.spec.ts`. | done |
| A4 | **Shell: focus lost on view change.** Opening an editor hid the focused card, so focus dropped to the page body; "Back" dropped it too. | Medium | **Fixed.** Opening focuses "Back" (one Tab from the editor); "Back" returns focus to the card of the app the user left. Asserted in `shell.spec.ts`. | done |
| A5 | **Shell: input and select borders were 1.4 to 1.6 to 1 against the surface** (WCAG 1.4.11 asks 3 to 1 for a control's boundary). The converter page already used the muted token (7.0 to 1). | Low | **Fixed** (`web/shell/shell.css`: border uses `--muted`, 5.4 to 1 light, 7.1 to 1 dark). | done |
| A6 | **Slides: the split-button arrows (New Slide, Font Color) had no accessible name** (UIA `Custom`, empty name, 14x28 px). | Medium | **Fixed.** `patches/deckcraft/0140`: the arrow is named "<label> ▾", the same form Sheets already uses (UIA shows "Font Color ▾"). A kittest asserts both names (8 toolbar tests pass). The released `slides.exe` was **not rebuilt**, so the UIA dump after the fix is not taken; see the build note below. | done |
| A7 | **Slides has no keytips.** deckcraft has no keytip system, so there is no Alt-key path through the toolbar (Docs and Sheets have one). Slides' keyboard routes are its shortcuts and command search (Ctrl+Shift+P). | Medium | **Deferred (recorded).** An upstream-sized feature; ours would be a new module in the 0140 toolbar. | Phase 6 or later, with the owner's priority |
| A8 | **Unnamed upstream-drawn controls remain in the desktop apps:** Docs 44, Sheets 31, Slides 18 (after A6), PDF 4 interactive nodes (`Custom`, `Edit`, `Slider`) with empty names. By position and size they are the Docs ribbon's split-arrows (11x22), the quick-access toolbar icons (26x24) and the AutoSave toggle in the title bar, the status-bar zoom and view buttons (18x18, 26x20), the Sheets formula-bar and sheet-tab buttons, the Slides slide thumbnails and notes box. They are upstream's widgets in hot files (`ribbon.rs`, `app.rs`, status bars). | Medium | **Deferred.** One small patch per app naming the QAT, status-bar and arrow widgets is possible (the Docs list alone is about 40 controls); not done in this pass to keep the shared patch queue quiet. | Phase 5 |
| A9 | **Shortcut hints use the Mac glyphs on Windows:** the accessible names read "Search commands (⇧⌘P)" (Slides) and "Qoʻyish (⌘V)" (Docs Paste). A Windows user is told "Command V". | Low | **Deferred.** An upstream string; the fix is the platform's modifier name in the shortcut formatter. | Phase 5 |
| A10 | **Control sizes below 24 px high on the desktop:** Docs ribbon buttons are 24x22, Sheets formula bar buttons 22x22, status bar icons 18x18 (WCAG 2.5.8 minimum is 24x24, with spacing exceptions). The phone layout needs 40. | Low | **Deferred.** Upstream metrics. | With the mobile layout (Phase 6) |
| A11 | **The status "Saved" text and errors** in the shell are `role="status"` / `role="alert"` (checked in the markup); the editors' own status line is canvas text and not announced. | Low | Recorded. | with A1 |
| A12 | **Desktop keyboard walk and Narrator speech were not run** (locked session). | gap | **Open.** `scripts/uia-dump.ps1 -Tabs 40` is ready; run it on an unlocked desktop and add Narrator in Edge for A1. | owner or next session |

Build note (A6): `scripts/build.sh slides desktop --release` currently fails at HEAD with `cannot update the lock file
domains/slides/Cargo.lock because --locked was passed`: the lock lacks seven packages (`sha2`, `digest`, `block-buffer`,
`cpufeatures`, `crypto-common`, `generic-array`, `typenum`), which is the in-progress `xdfu-core` work of the other builder. Not
touched here (the lock was restored after my check). The A6 change was compiled (`cargo check -p deckcraft-ui-egui`) and tested
(`cargo test -p deckcraft-ui-egui --lib office_toolbar`, 8 passed) in the upstream workspace.

## 3. Contrast of the Hoshiya palette (WCAG 2.x, text needs 4.5 to 1, UI parts and large text 3 to 1)

Computed from the tokens in `web/shell/shell.css` and `services/convert-api/ui/app.css` (all pairs the pages use; the full list prints
from `node scripts/a11y-web.mjs`, raw values in `web-a11y.json`).

| Pair | Light | Dark |
|---|---|---|
| Shell text on page | 16.6 to 1 (`#1b1f23` on `#fff`) | 14.9 |
| Shell muted text on page / card | 5.9 / 5.4 | 7.1 / 6.3 |
| Button text on accent (`#fff` on `#1E4FA3`; `#101828` on `#6C9BF0`) | 7.8 | 6.4 |
| Accent as text or focus ring on page / card | 7.8 / 7.1 | 6.4 / 5.7 |
| Shell error text on page | 6.5 (`#b3261e`) | 10.5 |
| Converter text on page / card | 14.4 / 15.7 | 15.3 / 13.6 |
| Converter muted on page / card | 6.5 / 7.0 | 9.1 / 8.1 |
| Converter focus ring on card / page | 6.7 / 6.2 | 7.7 / 8.6 |
| Converter warning, error, ok text on their backgrounds | 8.5 / 9.3 / 9.1 | 9.8 / 10.8 / 10.2 |
| Decorative dividers and card borders (`--line`) | 1.5 | 1.6 to 1.7 |

Every text pair passes AA, most pass AAA. The only low pair is the `--line` colour, which is used for card edges and list dividers
(decoration, not the only cue: cards also differ by background), so it is left as designed. The brand accent in the desktop apps'
theme (the egui Hoshiya palette) is the same `#1E4FA3` and `#6C9BF0` pair, so the same ratios hold there; the egui apps' grey text tokens were
not measured separately.

## 4. How to reproduce

- Web: `node scripts/a11y-web.mjs --json out.json` (release dists and `hoshiya-serve` from `E:/office-target/p1`).
- Desktop UIA: `powershell -File scripts/uia-dump.ps1 -Exe E:\office-target\p1\slides\release\slides.exe -File <pptx> -Out slides.txt`
  (Docs needs `-Invoke "Continue without restoring"` when stale recovery drafts exist; Sheets needs `-Wait 15`).
- The shell assertions are in `ci/web-smoke/shell.spec.ts` (`scripts/ci.sh shell`).

## 5. Checks run for this change

`ci.sh shell convert-ui i18n` could not complete as one command: both build slots were held by the other builders for over an hour
(its job step waited on `E:/office-target/p1`). The parts that need no cargo build ran directly: `shell-dist.sh` plus the shell
Playwright spec (19 passed, including the new aria-label and focus assertions), the converter Playwright spec (12 passed, 1 skipped screenshot test),
`cargo xt i18n lint` (0 errors). `verify-queue all` (82 patches, all trees OK) and `cargo xt register check` (82 rows) passed after the
deckcraft 0140 change.
