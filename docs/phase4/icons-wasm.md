# Docs icons: the SVG loader does not fit the wasm budget (WP2 step 0)

Question (toolbar-design B.3 point 2): can the Docs custom-group and QAT buttons draw Fluent SVGs through
`craft_ui::icons::SvgIconSet` and `egui_extras[svg]` within `web/budgets.toml`?

**Method.** Measured the loader's own cost in isolation, so that other agents' builds and the Docs wasm link time did not
decide it. A `cdylib` for `wasm32-unknown-unknown` with `egui 0.36` and `egui_extras 0.36` that installs the image loaders and
loads one embedded SVG into a texture (`install_image_loaders`, `include_bytes`, `try_load_texture`), built twice with the Docs
domain profile (opt-level 3, thin LTO, codegen-units 16): `svg` feature off and on. Brotli `-q 11 --lgwin=24`, as `ci.sh sizes`.

| Build | wasm raw | wasm brotli |
|---|---|---|
| without `egui_extras/svg` | 517,212 B | 70,887 B |
| with `egui_extras/svg` (resvg, usvg, tiny-skia, ...) | 1,767,290 B | 380,067 B |
| **delta** | **+1,250,078 B** | **+309,180 B** |

**Result.** +309 KB brotli against a Docs headroom of 19,271 B at the Phase 3 close (`docs/phase3/gate-review.md`; the whole
`delta_br_max` is 196,608 B and the catalogs use most of it). It does not fit, by a factor of about 16. The licences are fine
(resvg and usvg are `Apache-2.0 OR MIT` in the lock); size is the only reason.

**Decision: the build-time bitmap fallback (B.3 point 2).**
- `scripts/gen-docs-icon-bitmaps.sh` rasterises the Fluent icons our buttons use (10 distinct, regular 20 px, MIT) at 20, 28 and 40 px
  into 8-bit alpha masks: `domains/docs/crates/docs-office/assets/icons.bin`, 28,000 B raw, **3,055 B brotli**. resvg is a dev
  tool of `tools/icon-bitmaps` (its own workspace); nothing SVG-related is linked into any app.
- At draw time `docs_office::paint_icon` picks the smallest mask that covers the on-screen pixels, uploads it once as a white texture
  with that alpha, and tints it with the theme's icon colour (`Painter::image`). This also makes `fill="currentColor"` unnecessary.
- `craft_ui::icons::SvgIconSet` is therefore not used in Docs. Sheets and Slides (WP3) rows can still use it if the same
  measurement passes for them (Sheets base 4.7 MB, headroom unknown; measure before choosing).
- Desktop and web share this one path, so what the owner sees on the desktop is what the web build draws.
