# Layer map: gridcraft

This is a static read of `upstream/gridcraft` (nothing was built). Paths are relative to the
gridcraft repo root. Upstream HEAD: e6b6b349708ab4978cb3f061f6bd6ecbc25667cb.

**UI replaceability: 3/5.** The engine is clean and fully command-driven. The live grid renderer,
screen hit-testing and the in-cell editor exist only in `ui-egui`.

## 1. Crates

The layer numbers (L0–L7) come from `xtask/src/layers.rs:30-60`. The repo's rule is that nothing
below L6 touches egui.

| Crate | Role | Internal deps | Class |
|---|---|---|---|
| core (L0) | cell refs, values, errors | — | agnostic |
| numfmt (L0) | number formats | core | agnostic |
| pdf (L0) | self-contained PDF 1.7 writer (`crates/pdf/src/lib.rs:1`) | — | agnostic |
| formula (L1) | lexer, parser, printer | core | agnostic |
| functions (L1) | worksheet functions | core | agnostic |
| model (L2) | workbook, sheet and style model | core, formula | agnostic |
| calc (L2) | dependency graph and recalculation | core, formula, functions, numfmt, model | agnostic |
| xlsx (L2) | XLSX/CSV I/O; XLS, XLSB, ODS import | core, formula, model | agnostic |
| chart (L3) | chart layout into primitives, plus a rasteriser | core, model, calc, numfmt | agnostic |
| engine (L5) | session, commands, file I/O | everything above | agnostic |
| mcp (L6) | MCP JSON-RPC server | engine | agnostic |
| ui-egui (L6) | egui frontend | engine, chart, craft-ui | **UI-bound** |
| apps/gridcraft | desktop app | engine, ui-egui, craft-print | **UI-bound** |
| apps/gridcraft-web | browser app | engine, ui-egui | **UI-bound** |
| apps/gridcraft-cli | headless CLI | engine, mcp | agnostic |

## 2. Command layer
- **Registry:** `command_specs()` in `crates/engine/src/cmd/mod.rs:123`, built from 20 modules.
  `crates/engine/src/catalog.rs:6` holds a parity list of ribbon ids.
- **Count:** grep finds 340 `cmd!(` invocations; `docs/architecture.md:24` says 313. Both numbers
  are provisional until `gridcraft-cli commands` confirms the count.
- **Invocation:** `Session::execute(&mut self, id, Value) -> Result<Json>` (`crates/engine/src/lib.rs:318`).
  `execute_headless` (`lib.rs:374`) turns UI dialog requests into errors.
- **Journal:** the session records `(id, params)` for each command, with passwords stripped
  (`lib.rs:238`).
- **Undo:** handled in the engine. It keeps up to 100 copy-on-write workbook snapshots
  (`lib.rs:101-102, 348-359`). Each command is one undo step; there is no transaction that spans
  several commands.

## 3. Rendering
- **Charts:** backend-neutral. The chart crate builds a `Prim` list and turns it into RGBA with
  `rasterize` (`crates/chart/src/raster.rs:198`).
- **PDF:** `file.exportPdf` (`crates/engine/src/cmd/print.rs:1531`) writes PDF from the engine. Its
  text uses only the standard-14 fonts (Helvetica and Times metrics).
- **Grid:** egui-bound. `crates/ui-egui/src/grid.rs` is 3630 lines. There is no display list, so the
  live grid cannot be rendered to RGBA without egui.
- **Neutral helpers:** `crates/engine/src/display.rs` (cell display text) and
  `crates/engine/src/paper.rs` (page-layout geometry and hit-testing).

## 4. Editing and input

**In the engine:**
- the `Selection` model (`crates/engine/src/selection.rs`);
- `Session.mode` (Ready, Enter, Edit or Point);
- `Sheet::row_at` and `Sheet::col_at` (`crates/model/src/sheet.rs:294-297`).

**Only in `ui-egui`:**
- screen geometry and hit-testing: `Geo::cell_at`, freeze panes, outline bands and right-to-left
  layout (`grid.rs:191, 356-378`);
- the in-cell editor with autocomplete (`editor.rs`);
- formula-bar key handling and point mode (`formula_bar.rs:189-352`);
- the keymap.

**IME:** there is no IME-specific code anywhere. The app relies on egui text input.

## 5. Leaks
- **Dependencies:** none. No crate below `ui-egui` imports egui or another UI framework.
- **Logic that exists only in `ui-egui`:**
  - grid layout and hit-testing;
  - editor autocomplete;
  - the formula locale (`formula_locale.rs`);
  - dialogs (2741 lines);
  - the ribbon (2516 lines);
  - UI-only commands such as `app.language.set`, which the CLI cannot reach.

## 6. CLI (`apps/gridcraft-cli/src/main.rs:74-83`)

- **Subcommands:**
  - `info` (`:228`)
  - `convert` (`:269`), which writes only xlsx, csv, tsv, json or html
  - `eval` (`:286`)
  - `cat` (`:356`)
  - `run` (`:384`), which runs any engine command; PDF export is `run --cmd 'file.exportPdf=…'`
  - `commands` (`:435`)
  - `functions` (`:468`)
  - `mcp` (`:497`)
  - `send` (`:529`)
- **Formats** (`crates/engine/src/io.rs:13-32, 118-137, 204-208`):
  - read and write: XLSX family, CSV, TSV;
  - read only: `.xls`/`.xlt` (BIFF8), `.xlsb`, `.ods`;
  - export only: JSON, HTML;
  - writing xls, xlsb or ods is refused.
- **Encrypted workbooks:** supported (`crates/xlsx/src/crypto*`).
- **MCP:** about 28 tools. Its backend is either headless or a TCP client to a running app
  (`crates/mcp/src/backend.rs:4, 26`).

## 7. Web app
- **Build:** Trunk, `trunk build --release`, output to `dist/web`. eframe runs on the wgpu backend.
- **Open:** drag-and-drop or a file picker.
- **Save:** `showSaveFilePicker`, falling back to an anchor download (`web.rs:278-403`).
- **Print:** a PDF in a hidden iframe (`web.rs:219`).
- **Query parameters:** `?sample=`, `?host=parent` and `?author=` (`web.rs:30-63`).
  - `?host=parent` turns on a **postMessage bridge for embedding** (`apps/gridcraft-web/src/host.rs`),
    with Nextcloud as the example. This is directly relevant to the document-server design.
- **No open-by-URL and no fetch.**

## 8. i18n
- **Location:** `crates/ui-egui/src/i18n.rs` plus a file per language, 717 lines in all.
- **Languages:** 11 (En, Es, Zh, Ja, Ko, Ru, PtBr, De, Cs, Fr, Sr).
- **Mechanism:** Rust string tables, not the TSV format wordcraft uses. So the two apps have two
  different i18n mechanisms.

## 9. craft-libs / craft-fonts
- **craft-libs** is a git dependency at rev `d27646a5…` (`crates/ui-egui/Cargo.toml:12`,
  `apps/gridcraft/Cargo.toml:27`). It provides `craft-ui` and `craft-print`.
- **craft-fonts** is an optional build input through `CRAFT_FONTS_DIR` (`CLAUDE.md`,
  `.github/workflows/release.yml:27`).

## Obstacles to a new UI
1. There is no backend-neutral grid renderer. Cell painting, borders, conditional formatting and
   selection overlays are all egui painter code.
2. Screen geometry and hit-testing are egui-typed and live in `ui-egui`.
3. In-cell editing, autocomplete, point mode, keys and IME are all in `ui-egui`.
4. About 6k lines of dialogs and ribbon, plus the i18n tables, sit in `ui-egui`.
5. Font metrics: the engine's PDF and text-width code know only the standard-14 fonts. This is
   also a **fidelity risk for Cyrillic** in PDF export, still to be measured.
