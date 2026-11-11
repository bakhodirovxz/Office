# Host-mode engines: language, strings, autosave window, toolbar mode, Slides font box (2026-11-09, branch `p5-hengines`)

Measured on the test server on 2026-11-09 (Playwright, 1366x768, browser locale uz-UZ, the three web editors in an iframe of
the document server): Slides was English, Sheets showed an English "AutoSave", unsaved edits could be lost for up to 30 s, the
Slides font box was empty. This file records what was changed and how it was checked. The patches are in `patches/<repo>/`
(registered in `patches/REGISTER.md`).

## 1. Language (`?lang=`)

| app | how the language is chosen | change |
|---|---|---|
| Sheets | `index.html` answers `navigator.language` with `?lang=`; `web.rs` takes `Language::system()` (no saved choice exists on the web) | none needed: uz-Latn, uz-Cyrl and ru checked |
| PDF | same `index.html` trick; the `auto` preference follows the system language; the saved preference (eframe storage) wins | none needed: uz-Latn, uz-Cyrl and ru checked |
| Slides | `?lang=` (deckcraft 0147), else English (`auto` meant English) | **deckcraft/0171**: without `?lang=` follow `navigator.language`; a regional tag (`ru-RU`) falls back to its language. Web preferences are not persisted in Slides, so there is no saved user choice to protect |

## 2. English strings left in host mode (uz-Latn, ru)

Found by walking the default screens (main view, status bar, notes and comments panes, Present, menus, context menus) in the
before-dists and reading the screenshots. The main screens of all three were already translated once `?lang=` is passed.

Slides (deckcraft/0172, no new catalog rows: every string already had one): pane titles (Comments, Format Shape, Animation
Pane, Selection, ...), the Quick Access Toolbar customize list, the shape context menu (Cut, Copy, Paste, Bullets, Numbering,
Bring to Front, Send to Back, Group, Layout, Grid and Guides, Lock/Unlock, guides).

Sheets (gridcraft/0171, 13 new rows in `i18n/gridcraft.tsv`, the shared rows Print..., Resolve, Reopen and Selection now
also serve gridcraft; `OWN_GRID_HE` in `xtask/src/i18n/extract.rs`): the AutoSave toggle and tooltip, the title bar's More menu,
the Share menu, the pane titles, the Comments pane (empty text, Resolved, Delete, Reopen/Resolve, Send, Reply), the status bar's
Sum, Count and Average, the view-mode tooltips.

PDF: no English found on the screens walked (menu, tools, context menu, inspector).

Not done (rare screens, not walked): the Sheets Watch Window pane, the Slides Backstage pages, dialogs beyond the context menus.
The Latin Uzbek rows use U+02BB for o and g (the repo catalogs' convention, not U+2018) and U+02BC for the tutuq.
Status of the new Uzbek and Russian rows: `reviewed:agent-sonnet:2026-11-09`.

## 3. Autosave to the host

Same rule in all three bridges (deckcraft/0170, gridcraft/0170, pdfcraft/0130): the recovery copy goes 3 s after the last edit
while the document is unsaved, at the latest 30 s after the first edit that was not yet handed over (continuous editing),
never twice within 3 s, none for a document that has not changed since the last copy, none when clean, read only (view only) or
with a save unanswered. The edit signature is the presentation's or workbook's `Arc` pointer (Sheets plus its revision) and the
PDF engine's `edit_generation()`. A small `AutosaveClock` state machine is repeated in each bridge (the three upstream repos
share no crate).

Spec: `ci/web-smoke/bridge-{sheets,slides,pdf}.spec.ts`, new case `8b` in each: a copy within 12 s of one edit, no second copy
for an unchanged document, none while edits keep coming for 20 s, exactly one more by the 30 s ceiling, one more after the
edits stop, then quiet. The older 30 s cases (clean, read only, save unanswered) stay.

## 4. Toolbar mode on the web

`crates/suite-core` (branch `p5-hdocs`, commit cherry-picked unchanged): on wasm `toolbar_mode()` = the saved choice in
localStorage, then `?toolbar=full|simple`, then simple. Sheets (gridcraft/0172) and Slides (deckcraft/0174) now read their
mode through it on the web too, offer the Full ribbon / Simple ribbon switch on the web, and save the choice through
`set_toolbar_mode`. Full mode is upstream's ribbon in both. PDF has no simple/full mode (the setting ignores it).

## 5. Slides font box

deckcraft/0173: the Home Font box shows the deck's body (theme minor) font when nothing is selected or the text names no font.
With text selected the engine's `format.state` already resolved to the theme font.

## 6. Builds, specs, sizes (dists built after merging phase-5 9eaca703)

Dists: `E:/office-target/p5-hengines/dist/{sheets,slides,pdf}-web`. Slides needs `scripts/fetch-fonts.sh` (craft-fonts) first, else the
wasm is 4.7 MB smaller and not comparable to the budget. Brotli wasm and headroom to `web/budgets.toml`: sheets 4,958,840 (7,931 B),
slides 9,522,246 (34,739 B), pdf 9,572,670 (26,792 B); no budget raised. Screenshots (before from the phase-5 dists, after from these):
`E:/office-target/p5-hengines/shots/{before,after}-*.png`. The 8b autosave case allows a copy wherever two edits ended more than 3 s apart,
because a loaded machine stalls an edit (the palette edit in the PDF spec takes 1.6 to 3.5 s).
