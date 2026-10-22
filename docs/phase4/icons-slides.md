# Slides toolbar icons: the SVG loader does not fit the wasm budget (WP3 step 0)

Question (toolbar-design B.3): can the Slides row draw the Fluent SVGs through `craft_ui::icons::SvgIconSet` and `egui_extras[svg]`
within `web/budgets.toml`? Same question as the Docs one (`docs/phase4/icons-wasm.md`); the answer is measured here for Slides.

**Slides wasm brotli against its budget (`ci.sh sizes` method: `brotli -q 11 --lgwin=24` of the `*_bg.wasm`).**

| | wasm brotli | |
|---|---|---|
| budget (`base_br` 9,327,609 + `delta_br_max` 163,840) | 9,491,449 B | `web/budgets.toml` `[slides]` |
| slides-web at the new BASE `fe510bd` with our queue **before the toolbar** (patches 0001, 0002 bridge, 0100-0124), release | **9,471,505 B** | measured 2026-10-23 |
| **headroom** | **19,944 B** | |
| cost of `egui_extras[svg]` (resvg, usvg, tiny-skia, ...), isolated build with the Docs profile | +309,180 B | `docs/phase4/icons-wasm.md` |

The loader needs about 15 times the headroom. It does not fit, and Slides links the same `egui 0.36` / `egui_extras 0.36`
code, so the isolated measurement carries over. **Decision: the build-time bitmap fallback**, as Docs.

- `scripts/gen-slides-icon-bitmaps.sh` rasterises the 19 Fluent icons the row draws (regular 20 px, MIT) at 20, 28 and 40 px into
  8-bit alpha masks: `upstream/deckcraft/crates/ui-egui/assets/office_icons.bin` (carried in patch `deckcraft/0140`), 53,155 B raw,
  **4,286 B brotli**. resvg is a dev tool of the Docs `icon-bitmaps` tool (its own workspace): nothing SVG-related is linked into any app.
- `office_toolbar::paint_icon` picks the smallest mask that covers the on-screen pixels, uploads it once as a white texture with that
  alpha, and tints it with the theme's text colour. No `fill="currentColor"` step is needed.
- `craft-ui` and `egui_extras` are **not** added to deckcraft: the lock gains only `suite-core`'s dependencies (`toml_edit`, `ron` and
  friends, desktop builds only).
- The wasm size with the whole toolbar and the bitmaps is in `docs/phase4/slides-wp3.md` (section 2).
