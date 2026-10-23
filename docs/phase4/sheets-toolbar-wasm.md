# Sheets toolbar: the SVG loader and the wasm budget (WP3 step 0)

Question (toolbar-design B.3 point 2): can the Sheets row draw Fluent SVGs through `craft_ui::icons::SvgIconSet` and `egui_extras[svg]`
within `web/budgets.toml`?

**Headroom before the patch** (`ci.sh sizes` on the synced queue, 2026-10-23): sheets 4,909,511 B brotli against a budget of 4,934,003 = **24,492 B**.

**Cost of the loader.** The Docs lane measured `egui_extras[svg]` in isolation on the same egui 0.36: **+309,180 B brotli**
(`docs/phase4/icons-wasm.md`). That is about 13 times the Sheets headroom, so the SVG path is out.

My own attempt to measure it on the real crate did **not** reproduce that number, and I do not trust it: `cargo build --release
--target wasm32-unknown-unknown -p gridcraft-web` (domain profile: opt 3, thin LTO, 16 units, no wasm-opt) of the SVG variant gave
28,505,425 B raw / 5,112,330 B brotli, and of the bitmap variant 28,560,055 / 5,113,559. The SVG build's wasm contains no resvg or usvg strings,
so I believe the loader was dead-stripped in my spike. The Docs measurement drives the decision; mine is recorded only so nobody cites it.

**Decision: the build-time bitmap fallback**, the same one Docs uses:
- `scripts/embed-toolbar-icons.sh` runs the Docs lane's `icon-bitmaps` tool (resvg is a dev tool in its own workspace) and writes
  `crates/ui-egui/src/office_icons.bin` (carried in patch 0140): 25 Fluent icons x 20, 28 and 40 px, 69,962 B raw, **5,173 B brotli**.
- `office_toolbar.rs` picks the smallest mask that covers the on-screen pixels, uploads it once as a white texture with that alpha and tints it
  with the theme's text colour. No SVG code is linked. There is no `fill="currentColor"` step.

**Result** (`scripts/build.sh sheets web --release`, `ci.sh sizes`): sheets wasm brotli **4,943,443 B**, **+33,932 B** over the 4,909,511 B
before the patch (icons 5 KB, the rest is the toolbar's code and the new catalog rows). That is 9,440 B over the old budget, so
`delta_br_max` went from 196,608 to **229,376** (measured delta 206,048 + 10 %, rounded up to 16 KiB, the budgets.toml rule). Headroom after: 23,328 B.

## Width thresholds (measured, `cargo test -p gridcraft-ui-egui --test office_toolbar widths -- --nocapture`)

The row is laid out from the real text widths: labels drop when the labelled row does not fit, then items from the right move into the "..." menu.
Window width in logical px at which each state starts, per language (step 20 px):

| language | labels shown from | every item in the row from |
|---|---|---|
| en | 1,640 | 1,320 |
| ru | 1,820 | 1,340 |
| uz-Latn | 1,740 | 1,320 |
| uz-Cyrl | 1,820 | 1,320 |

The design estimated 1,500 and 1,100. The measured values are higher, and they depend on the language (ru and uz-Cyrl labels are wider). Below the "all items"
width the Cells, Data, View and Output groups move into "..." from the right (at 1,000 px only the font, alignment and number groups remain, screenshot
`evidence/toolbar/sheets-simple-en-1000px.png`). The font, alignment and number groups never move (about 700 px); below that the row is clipped.
