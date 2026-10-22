#!/usr/bin/env bash
# scripts/gen-slides-icon-bitmaps.sh [out.bin]   (Phase 4 WP3)
# Rasterises the Fluent icons the Slides toolbar draws (assets/icons/fluent, MIT) into 8-bit alpha masks at 20, 28 and 40 px:
# the file upstream/deckcraft/crates/ui-egui/assets/office_icons.bin of patch deckcraft/0140. Same dev tool as the Docs icons
# (resvg runs here only); the SVG loader does not fit the Slides wasm budget (docs/phase4/icons-slides.md).
# Keep the list in step with ICONS in office_toolbar.rs (a test checks the names).
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="$OFFICE_TARGET/icon-bitmaps"
out=${1:-upstream/deckcraft/crates/ui-egui/assets/office_icons.bin}
mkdir -p "$(dirname "$out")"
cargo run --release --manifest-path domains/docs/crates/docs-office/tools/icon-bitmaps/Cargo.toml -- \
  assets/icons/fluent/svg "$out" 20,28,40 \
  slide_add slide_layout text_bold text_italic text_underline text_color text_bullet_list text_number_list_ltr \
  text_align_left text_align_center text_align_right textbox image shapes table translate history \
  window_header_horizontal window_header_horizontal_off
