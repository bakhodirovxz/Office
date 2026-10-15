# Layer map: wordcraft

This is a static read of `upstream/wordcraft`; nothing was built for it. Paths are relative to the
wordcraft repo root. Upstream HEAD: fd0bb2fb1ec081e3eddc4d01917637fef0cafdfe.

**UI replaceability: 4/5.** The engine and the renderer are clean. Input handling (pointer→selection
and IME) is the main piece that lives only in the UI.

## Corrections to the brief (verified 2026-10-15)
- **Command count is 629, not 433.** That is the number of distinct `CommandSpec::new("…")` ids under
  `crates/engine/src`, counted with grep. The README (`README.md:98`) and `docs/architecture.md`
  say 422, so they are out of date. The count will be cross-checked with `wordcraft-cli commands
  --json` once the build is done.
- **`docs/HOSTING.md` does not exist.** It is not in any of the six repos and not in wordcraft's git
  history (`git log --diff-filter=D`). The "13 / 5 MB" figure the brief quotes from it cannot be
  traced. The real wasm size will be measured in `inventory.md`.

## 1. Crates

`cargo xtask layers` enforces that nothing below L6 depends on egui, eframe, winit or rfd
(`docs/architecture.md:12-14`).

| Crate | Role | Internal deps | GUI deps | Class |
|---|---|---|---|---|
| geom | units, rects | — | no | agnostic |
| doc | document model, edit primitives | geom | no | agnostic |
| fonts | font DB, shaping | — | no | agnostic |
| metafile | WMF/EMF | — | no | agnostic |
| proof | spelling, grammar, hyphenation | — | no | agnostic |
| layout | pagination, display list, hit-testing | geom, doc, fonts, proof | no | agnostic |
| docx | OOXML read/write | doc, geom | no | agnostic |
| docbin | `.doc` reader | doc, geom | no | agnostic |
| formats | TXT/MD/HTML/RTF/ODT/TeX | doc, geom | no | agnostic |
| render | CPU raster of pages (vello_cpu) | geom, doc, fonts, layout, metafile | no | agnostic |
| pdf | PDF export via krilla | doc, geom, fonts, layout, metafile | no | agnostic |
| engine | Session, commands, I/O dispatch | all of the above | `js-sys` on wasm only (`crates/engine/Cargo.toml:31`) | agnostic |
| zotero | Zotero protocol | doc, engine, formats | no | agnostic |
| control-key | control-port key files | — | no | agnostic |
| mcp | MCP server, headless or bridged | doc, engine, render, control-key | no | agnostic |
| ui-egui | ribbon, canvas, dialogs, i18n | most of the above + `craft-ui` | egui, craft-ui | **UI-bound** |
| apps/wordcraft | desktop app | … + ui-egui | eframe+wgpu, winit, rfd, arboard, `craft-print` | **UI-bound** |
| apps/wordcraft-web | browser app | engine, ui-egui, doc, fonts | eframe wgpu, wasm-bindgen, web-sys | **UI-bound** |
| apps/wordcraft-cli | headless CLI | doc, engine, render, mcp, zotero | no | agnostic |

## 2. Command layer
- **Registry.** `registry()` is built in `crates/engine/src/cmd/mod.rs:57-100` from about 43 modules.
- **Spec fields** (`crates/engine/src/lib.rs:61-74`): id, label, location, shortcut, params (a
  free-text hint, not a schema), mutates, enabled, run.
- **Invocation.** `Session::run(&mut self, id, &Value) -> Result<Value, CmdError>`
  (`crates/engine/src/session.rs:1167`). It is JSON in and JSON out, and `Registry::describe()`
  emits the full list as JSON.
- **Undo** is in the engine, as whole-document `Arc<Undo>` snapshots (`session.rs:379-382, 845-935`).
  A failing or panicking command is rolled back (`session.rs:1167-1260`).

## 3. Rendering
- **Display list.** `layout` produces a backend-neutral `Draw` display list
  (`crates/layout/src/display.rs:25`).
- **Raster.** `render` turns it into premultiplied RGBA8 with vello_cpu
  (`crates/render/src/lib.rs:38-43`). The entry points are `render_page`, `render_region`,
  `render_area` and `render_items` (`lib.rs:489, 497, 1102, 1112`), and there is PNG and JPEG
  output (`lib.rs:59, 73`).
- **PDF.** `crates/pdf` consumes the same display list.
- **egui** only uploads the page pixmaps as textures and paints the overlays.

## 4. Text input, selection, hit-testing

In the engine or layout:
- hit-testing, caret placement, selection rectangles and cell lookup (`crates/layout/src/hit.rs:269`
  and nearby);
- caret and selection commands (`crates/engine/src/cmd/caret.rs`), with typing done by `text.insert`;
- table mouse operations (`cmd/table_mouse.rs`, `table_resize.rs`, `table_draw.rs`).

Only in `ui-egui`:
- **IME preedit.** Handled at `crates/ui-egui/src/canvas.rs:86, 1387-1408`. The engine has no
  composition API (deckcraft's engine does have one).
- **Pointer→selection.** Click, drag and double/triple-click write `session.sel` directly
  (`canvas.rs:1496-1529, 2175-2233`). There is no engine-level pointer command.
- Key mapping (`keys.rs`), caret animation, autoscroll, rulers and clipboard.

## 5. Leaks
- No non-UI crate imports egui. The only grep hits are a comment and a doc comment.
- The `ui.*` automation methods (pointer, render, screenshot) are served only by the egui app
  (`crates/ui-egui/src/control.rs`). MCP's remote mode needs the running app.

## 6. CLI (`apps/wordcraft-cli/src/main.rs:15-54, 306-479`)

| Subcommand | What it does |
|---|---|
| `convert <in> <out>` | docx, doc (read only), pdf, odt, rtf, html, md, tex, txt, json, png, xml; `--markup` |
| `info`, `text`, `inspect` | print document info, text, or structure |
| `render <file> <out.png> [--page N] [--scale S]` | render one page to PNG |
| `run [--file\|--template] --cmd 'id={json}' … --save OUT` | run commands headless and save |
| `commands [--json]`, `parity` | list commands, report parity |
| `mcp [--connect HOST:PORT]` | MCP server; `--automation-read-root` / `--automation-write-root` sandbox it |
| `zotero` | Zotero bridge |

## 7. Web app
- **Build.** Trunk (`apps/wordcraft-web/index.html:9`, `Trunk.toml:1-10`) with
  `trunk build --release`, output to `dist/web`. `xtask wasm` only runs `cargo check`.
- **Open.** Drag-and-drop or a file picker. Fonts come from the Local Font Access API
  (`web.rs:229-256`).
- **Save.** `showSaveFilePicker`, falling back to an anchor download (`web.rs:347-366, 473-490`).
- **No open-by-URL and no fetch.** The only query parameters are `?webgl` and `?sample`.

## 8. i18n
- A custom TSV catalog in `crates/ui-egui/src/i18n/` (`mod.rs`, `catalog.rs`, `audit.rs`,
  `pseudo.rs`, one `.tsv` per language).
- `LANGUAGES` has 23 entries (`mod.rs:55`).
- The English source string is the lookup key: `tl!`, `t`, `tr`, `tc`, `fmt`.
- **This is where uz-Latn, uz-Cyrl and ru plug in for Phase 1.** Command labels in the engine are
  English only.

## 9. craft-libs / craft-fonts
- `craft-ui` (`crates/ui-egui/Cargo.toml:12`) and `craft-print` (`apps/wordcraft/Cargo.toml:71`) are
  git deps on craft-libs at rev `d27646a`. `craft-ui` is egui-based.
- craft-fonts is not referenced in any Cargo.toml.

## Obstacles to a new UI
1. Pointer→selection, IME preedit and key mapping are in `ui-egui`. They need an engine-level input
   API, which would be an upstream patch.
2. There is no in-process FFI or wasm API for non-Rust shells. Only stdio MCP and the TCP control
   channel exist.
3. About 51k lines of UI (ribbon, dialogs, panes, Backstage, rulers) would have to be rebuilt.
   Command params have no machine schema.
4. The i18n loader lives inside `ui-egui`. The TSV data itself is portable.
