# Accessibility minimum for Docs: the DOM command palette and the keyboard exit (Phase 6 WP11)

Decision: PLAN section 8, option (b), O9 (A1 = DOM command palette now). Baseline findings A1 and A2: `docs/phase4/a11y-baseline.md`.

## What a screen-reader or keyboard user gets

| Need | How |
|---|---|
| Find and run any editor command (Bold, Insert table, Save, ...) | A command palette in the page that hosts the editor: a modal `<dialog>` with a combobox (the filter) and a listbox (the commands), ARIA 1.2 pattern. Each option reads "<translated label> <shortcut>"; a disabled command is `aria-disabled`. |
| Open the palette | Tab to the **Commands** button (it is visually hidden until it has focus, it comes right after "Home" in the shell and first in the docserver wrapper's bar), or press **F1** (in the document or on the page). |
| Run, close | Up/Down move the active option (`aria-activedescendant`), Enter runs it, Esc closes; the focus goes back to the document either way. Typing filters on the label and the shortcut. |
| Leave the document by keyboard | **F6** in the document (the canvas swallows Tab): focus moves to the page (the shell's "Home" button, the wrapper's Commands button). This fixes A2 (WCAG 2.1.2). |

F1 and F6 were chosen because the editor binds neither (checked in `crates/engine/src/cmd` shortcuts and `crates/ui-egui/src/keys.rs`;
Ctrl+Shift+P is "Font Size" in Docs, so the Slides-style palette key was not free). The keys are caught by a capture-phase `keydown`
listener in the editor page (wordcraft/0141), because a key pressed inside the iframe never reaches the parent page.

## The bridge shape (wordcraft/0141, `apps/wordcraft-web/src/web/host.rs`)

| Message | Direction | Fields |
|---|---|---|
| `<prefix>:commands` | page to editor | none: asks for the list |
| `<prefix>:commands` | editor to page | `commands: [{ id, label, enabled, shortcut }]`: commands that take no parameters; `label` in the editor's language (`i18n::t`); `enabled` from `widgets::enabled`; `shortcut` as the platform writes it |
| `<prefix>:run` | page to editor | `id`: runs it through `WordApp::run`, the path a ribbon button takes (dialogs open, blocked commands are refused) |
| `<prefix>:ran` | editor to page | `id, ok, error` |
| `<prefix>:focus` | page to editor | focuses the canvas |
| `<prefix>:focusOut` | editor to page | `palette`: false = F6 (leave), true = F1 (open the palette) |

Same origin and parent-only rules as the other bridge messages (`event.source` is the parent, `event.origin` is the parent origin).
The page treats every field as text (`palette.js` stringifies them), so a hostile editor page cannot inject markup.

## Sheets, Slides and PDF later

The page side is ready: `web/shell/palette.js` is app-neutral (`Palette.make({ t, post, back, say })`); the shell lists the apps that
answer in `CMD_APPS` (`shell.js`), the docserver wrapper passes `app.id === 'docs'` to `addBar`. An app joins by carrying the same six
messages in its bridge (`gridcraft:`, `deckcraft:`, `pdfcraft:` prefixes) and one line in each of those two places. Their engines have
the same shape (a registry of id, label, shortcut, enabled; `run`), so the Rust side is about 50 lines per app. Slides already has a
command search (Ctrl+Shift+P, `a11y-baseline.md` A7) whose list the bridge would reuse.

## Options (a) and (c)

- **(c), stated plainly:** the palette gives keyboard and screen-reader access to the **commands**, not to the **document text**. On the
  web the canvas is still invisible to a screen reader (no caret, no paragraph text). A screen-reader user who needs to read or write the
  body of a document should use the **desktop apps** for now: they expose a UI Automation tree (`a11y-baseline.md` section 1) and
  WebView2 aside, the desktop editors are native egui.
- **(a), future upstream issue:** an AccessKit web adapter. wordcraft already builds the AccessKit tree
  (`crates/ui-egui/src/a11y.rs`, call `canvas.rs:1649`), but eframe's web runner discards it (`eframe-0.36.2/src/web/app_runner.rs:394`,
  `accesskit_update: _`). The right home is an egui/eframe upstream change (a hidden DOM mirror driven by the tree). To be filed when
  upstream PRs are allowed (owner decision B-6); until then it stays a recorded gap (A1).

## Evidence

Playwright, `ci/web-smoke/a11y-palette.spec.ts` (shell) and the docserver spec case "command palette" (wrapper): Tab reaches the
button, the palette opens, `ariaSnapshot` lists named options, Bold run on a selection makes the saved docx bold, F6 leaves the
editor. TalkBack on a phone or emulator was **not** run (the emulator is busy with WP8): the DOM uses standard roles
(dialog, combobox, listbox, option) that TalkBack and NVDA read, but speech output is unverified.
