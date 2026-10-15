# Layer map: pdfcraft, craft-libs, craft-fonts

Static read; nothing was built. Paths are relative to `upstream/`.
HEADs: pdfcraft afa8e1843cfed613f3d403119f6c2649bce824c9, craft-libs 8f5a2831614e632f91ad367f7af2a6e5b61b9529,
craft-fonts 3d9ab3a132ed0eff457e424226a435a901da30d3.

## A. pdfcraft — UI replaceability 4/5

### Crates
- **UI-bound (3 crates):** `crates/ui-egui` (about 58.7k lines), `apps/pdfcraft` and `apps/pdfcraft-web`.
- **UI-agnostic:** the other 33 members, including:
  - foundation: geom, filters, crypt, cos, content, model, fonts;
  - features: annot, forms, xfa, edit, redact, audit, sign, optimize, a11y, preflight, create,
    export, compare, measure, organize, xfdf, ocr, and js (a boa interpreter);
  - render, engine, platform, automation, and `apps/pdfcraft-cli`.
- The only egui hits outside `ui-egui` are in comments.

### Command layer
pdfcraft is built differently from the other three apps.
- `COMMANDS` (`pdfcraft/crates/engine/src/commands.rs:163`) holds about 200 entries, but they are
  static data: id, label, menu, shortcut and gating. The frontend implements what each id does.
- The real editing API is typed: `Session::apply(DocId, Edit)` (`crates/engine/src/lib.rs:3594`).
  `Edit` is a Rust enum (`:1232`) and is **not serialisable**.
- The serialisable surface is `Automation::call(name, &Value)`
  (`crates/automation/src/lib.rs:185`): about 229 tools with JSON-Schema inputs.
- Undo lives in the engine (`lib.rs:3786, 3801`). Its labels are structured `Label` values (`:654`),
  so they can be localised.

### Rendering
- The `render` crate wraps a vendored `hayro` (CPU) and produces premultiplied RGBA8
  (`crates/render/src/raster.rs:196-210`). It has no egui dependency.
- It includes tile offsets, a watchdog and a pixel cap.
- `Automation::render_pixels` is at `crates/automation/src/lib.rs:1833`.

### CLI (`apps/pdfcraft-cli/src/main.rs:95-113`)
- Subcommands: info, render, text, edit, combine (merge), extract, split, check, check-one, audit,
  tools, run, batch, ui, and mcp (behind a feature flag).
- `edit` can rotate, delete, move or insert blank pages, and set the title and author.

### Web app — the most integration-ready of the four
- **Open by URL:** `?file=<url>` opens a document through `fetch`. The URL must be same-origin or
  CORS-enabled (`apps/pdfcraft-web/src/main.rs:30-37, 110-113`).
- **Host bridge:** a postMessage bridge for embedding, with Nextcloud as the stated use
  (`main.rs:126-215`).
  - Incoming: `pdfcraft:open`.
  - Outgoing: `pdfcraft:save` and `pdfcraft:dirty`.
- **Unsaved work:** a `beforeunload` guard (`main.rs:90`).
- Build: `trunk build --release`.

**This is the reference pattern for the document-server spike.** gridcraft also has a `?host=parent`
postMessage bridge (`gridcraft/apps/gridcraft-web/src/host.rs`). wordcraft and deckcraft have
neither.

### vendor/ and deny.toml
- **Vendored crates**, patched copies wired in through `[patch.crates-io]`: hayro\*, vello_common,
  lopdf 0.45, winit 0.30.13 and egui-wgpu 0.36.2.
  - All are MIT and/or Apache-2.0.
  - `vendor/README.md:4` forbids vendoring copyleft code.
- **Foxit `.pfb` fonts** (about 0.25 MB): PDFium BSD-style licence, attribution required.
- **ICC profile** (CGATS): CC0.
- **`deny.toml` allow-list:** MIT, MIT-0, Apache-2.0 (+LLVM exception), BSD-2/3, ISC, Zlib,
  Unicode-3.0, BSL-1.0, CC0-1.0.
  - Exceptions: epaint_default_fonts (OFL-1.1 and the Ubuntu font licence) and webpki-root-certs
    (CDLA-Permissive-2.0).
  - Only the `storytold/craft-libs` and `storytold/winprint.rs` git sources are allowed.
  - **This is a good template for our own `deny.toml`.**

### i18n
- The `tl!` catalog inside `ui-egui` covers 23 languages with roughly 2,000–2,600 entries each,
  including CLDR plural forms (`docs/localization.md`).
- Locale detection uses `sys-locale`.

### Obstacles
1. About 58.7k lines of UI behaviour live in `ui-egui`.
2. The editing API (`Edit`) is not serialisable. A non-Rust UI must use the automation tools, which
   are a different surface from the egui shell's.
3. `COMMANDS` only names actions; their behaviour is implemented in the frontend.
4. Some logic is duplicated in `ui-egui`, for example `write_atomically` (`ui-egui/src/editing.rs:986`).

## B. craft-libs

| Crate | Role | egui-bound |
|---|---|---|
| atomic | crash-safe file writes | no |
| cms | ICC v2/v4 colour management | no |
| pdf | hayro raster + optional krilla writer | no |
| print | CUPS/Windows printer discovery and jobs | no |
| quit | macOS-correct quit/app menu | **yes** |
| settings | settings store, platform paths, TOML | no |
| ui | reusable egui controls (buttons, docking, icons, tabs, theme, printing) | **yes** |

### Consumers
Every consumer uses craft-libs as a git dependency pinned by `rev`. **The revs differ, which matters
for a one-workspace build:**

| App | Uses | Rev |
|---|---|---|
| wordcraft | craft-ui, craft-print | `d27646a` |
| gridcraft | craft-ui, craft-print | `d27646a` |
| deckcraft | craft-pdf (+write), krilla | `c348732` |
| pdfcraft | craft-ui `5c874d5`, craft-quit `f58bc1a`, craft-print `666c358` | three different revs |

### Unused crates and duplicated code
- `craft-settings`, `craft-atomic` and `craft-cms` are **not used by any of the four apps**.
- The apps re-implement atomic writes locally instead of using `craft-atomic`. There are at least
  5 copies:
  - `deckcraft/crates/engine/src/recovery.rs:82`
  - `deckcraft/crates/ui-egui/src/settings.rs:42`
  - `pdfcraft/crates/platform/src/staging.rs:73`
  - `pdfcraft/crates/automation/src/lib.rs:2556`
  - `pdfcraft/crates/ui-egui/src/editing.rs:986`
- wordcraft and gridcraft each have their own PDF writer crate. deckcraft wraps `craft-pdf`, and
  pdfcraft uses its own hayro and lopdf.

### Licences
- No GPL, AGPL, LGPL or MPL code.
- `NOTICE` flags one non-OSI test file, `crates/cms/tests/data/FOGRA39.txt`. It may be redistributed
  only unmodified and with credit, and it is test data only.

## C. craft-fonts
- **Contents:** about 40 MB of OFL-1.1 fonts:
  - BIZ UDPGothic and BIZ UDMincho;
  - Shippori Mincho;
  - Noto Sans CJK SC (the bulk of the size);
  - Noto Sans Arabic and Noto Sans Thai.
- **No Cyrillic-focused families.** Those come from each app's bundled fonts or from system fonts;
  the Cyrillic coverage of those fonts is still to be verified.
- **How apps use it:** no app has it as a Cargo dependency. Apps read it optionally through
  `CRAFT_FONTS_DIR` in a fonts-crate `build.rs` (e.g. `pdfcraft/crates/fonts/build.rs:11-14`).
  Release CI sets `CRAFT_FONTS_REQUIRED=1`.
- **Fonts bundled in the apps:**
  - wordcraft: Inter, JetBrainsMono, SourceSans3 and SourceSerif4, about 4.3 MB.
  - pdfcraft: Inter, JetBrainsMono and DancingScript.
- **Byte-identical duplicates** between wordcraft and pdfcraft: Inter ×3 and JetBrainsMono-Regular,
  1,518,908 bytes in total.
- **Foxit fonts** are duplicated in `pdfcraft/vendor` and `craft-libs/vendor`.
