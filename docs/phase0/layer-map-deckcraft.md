# Layer map: deckcraft

Static read of `upstream/deckcraft` (nothing built). Paths are relative to the deckcraft repo root.
Upstream HEAD: 9859da39255c0262cd592bb57101d7f2fa961f72.

**UI replaceability: 4/5.** The engine and everything below it are UI-agnostic. egui is confined to
`ui-egui` and the two shell apps, and the layering is enforced by `xtask/src/layers.rs:35-62`.

## 1. Crates

All crates are v0.7.0 (`Cargo.toml:1-35`). Only `ui-egui`, `apps/deckcraft` and
`apps/deckcraft-web` depend on egui, eframe, winit, wgpu, glow, web-sys or wasm-bindgen.

| Crate | Role | Internal deps | UI-bound? |
|---|---|---|---|
| geom | units, transforms, presets, hit testing | none | agnostic |
| color | RGB/HSL, theme colours | none | agnostic |
| metafile | WMF/EMF reader | none | agnostic |
| model | document model | geom, color | agnostic |
| fonts | font database, shaping | none | agnostic (leak: `epaint_default_fonts`) |
| text | text layout, hit testing, caret | model, geom, color, fonts | agnostic |
| anim | animation and transition timing | model, geom | agnostic |
| format | native `.deckcraft` zip format | model | agnostic |
| render | renders a slide to pixels (vello_cpu) | geom, color, metafile, model, anim, fonts, text | agnostic |
| pptx | PPTX import and export | model, geom, color | agnostic |
| pdf | PDF export through craft-pdf and krilla | model, fonts, text, render | agnostic |
| engine | session, undo, selection, 320 commands, pointer tools | geom, color, model, fonts, text, render, pdf, format, pptx, media (`crates/engine/Cargo.toml:12-21`) | agnostic |
| media | probe, decode, mixer, clock | all 8 codec/container crates (`crates/media/Cargo.toml:12-19`) | agnostic |
| bitstream, matroska, ogg, opus, isobmff, h264, hevc, vp9, av1 | codecs and containers | `bitstream` or none | agnostic |
| mcp | MCP server over stdio | engine, render | agnostic |
| ui-egui | the PowerPoint-style egui frontend | geom, color, model, fonts, text, render, anim, engine, media | **UI-bound** |
| apps/deckcraft | desktop app | engine, ui-egui, media | **UI-bound** (eframe, winit, cpal) |
| apps/deckcraft-web | wasm app | engine, ui-egui | **UI-bound** (`apps/deckcraft-web/Cargo.toml:24-30`) |
| apps/deckcraft-cli | headless CLI | engine, mcp | agnostic |

**Media codecs are always compiled in, including in the wasm build.**
- `engine` depends on `media` unconditionally (`crates/engine/Cargo.toml:21`).
- `media` depends on all codecs and on symphonia unconditionally (`crates/media/Cargo.toml:12-19,27`).
- The only codec feature is `threads` (`crates/av1/Cargo.toml:12-19`).
- Trimming wasm size would need a new feature gate on engine and media, which is an upstream patch.

## 2. Command layer

- **Catalog.** `command_specs()` (`crates/engine/src/cmd/mod.rs:161`) collects the commands from 28
  submodules. A grep finds 320 `cmd!` entries.
- **Spec.** `CommandSpec` (`mod.rs:44`) holds an id, label, menu, shortcut, a params schema string,
  `enabled`/`run` function pointers, and `journal`/`undoable` flags.
- **Invocation.** `Session::execute(&mut self, id: &str, params: &Value) -> Result<Value>`
  (`crates/engine/src/lib.rs:579`). It is JSON in and JSON out.
- **Listing.** `Session::commands()` returns serializable `CommandInfo` (`mod.rs:60`).
- **Undo is in the engine.**
  - `History` and `HistoryEntry` keep `Arc<Presentation>` snapshots (`lib.rs:162-175`).
  - A command that fails is rolled back.
  - A pointer drag is grouped into one undo step (`crates/engine/src/tools.rs:482-500`).
  - IME composition is one undo step.
  - A panic inside a command is caught and returned as `EngineError::Internal` (`lib.rs:576-580`).

## 3. Rendering

- `render_slide(pres, index, &RenderOpts) -> Image` (`crates/render/src/lib.rs:388`) uses vello_cpu
  and does not depend on egui.
- `Image::to_png`, `to_jpeg` and `to_straight` return the result as PNG, JPEG or straight RGBA
  (`render/src/lib.rs:75-126`).
- PDF export goes through `deckcraft_pdf::export` (`crates/pdf/src/lib.rs:130`).
- egui only uploads the resulting bitmap as a texture (`crates/ui-egui/src/canvas.rs:205`).
- Rendering is therefore CPU and bitmap-based, not vector or GPU.

## 4. Text input

The engine owns text input:
- caret and hit testing: `crates/text/src/lib.rs:2332-2355`;
- keys: `Session::key` (`crates/engine/src/keys.rs:51`);
- IME: `compose_text` and `commit_text` (`keys.rs:231,243`);
- pointer: `Session::pointer` (`tools.rs:433`).

`ui-egui` only forwards events to these.

## 5. Leaks

- `fonts` depends on `epaint_default_fonts` (`crates/fonts/Cargo.toml:18`). It is data only.
- `engine` uses `js-sys` on wasm32 for the comment clock (`crates/engine/Cargo.toml:30-31`).
- About 30.9k lines of UI logic exist only in `ui-egui`: dialogs, ribbon, backstage, sorter,
  selection pane, animation timeline and slide show.
- Audio output (`cpal`) is in the desktop app.

## 6. CLI and MCP

`apps/deckcraft-cli/src/main.rs` provides these subcommands:
- `info`, `bench`;
- `render` (PNG per slide);
- `render-transition`;
- `convert` (pptx, deckcraft, png, jpeg, gif, pdf);
- `run` (`--cmd` repeatable, plus `--save` and `--export`);
- `commands`, `describe`;
- `app` (the control channel);
- `mcp`;
- `links`, `version`.

MCP has two backends:
- `Headless`, an in-process session (`crates/mcp/src/lib.rs:4-12`);
- `Remote`, which uses the JSON-lines control channel (`docs/control-protocol.md`).

## 7. Web app

- **Build.** trunk (`Trunk.toml`, `index.html:8`, with `data-wasm-opt="z"`).
- **Rendering.** An eframe canvas using WebGPU, falling back to WebGL2. `?webgl` forces WebGL2.
- **Open.** `rfd` file dialog or drag-and-drop.
- **Save.** A Blob download (`apps/deckcraft-web/src/web.rs:113-173`).
- **No open by URL or fetch.**

## 8. i18n

There is none: the UI is English only (`docs/gaps.md:24`). Strings are hardcoded in `ui-egui` and in
`CommandSpec.label`.

## 9. Obstacles to a new UI

1. About 30.9k lines of UI logic in `ui-egui` would have to be rebuilt.
2. There are no FFI or JS bindings over `Session`. The only remote access is the JSON-lines control
   protocol.
3. Rendering is CPU bitmap only. Overlays such as handles, the caret and squiggles are drawn by the
   UI and would have to be redrawn.
4. Binary size: all codecs, symphonia, krilla and the fonts are always compiled in.
5. There is no localization mechanism.

## craft-libs and craft-fonts

- `craft-pdf` is a git dependency pinned to rev `c3487320…` (`Cargo.toml:60`).
- `krilla` is pinned to the same rev through `[patch.crates-io]` (`Cargo.toml:97-98`).
- `craft-fonts` is not a dependency.
- Nothing is vendored. Building needs network access or a local git mirror.
