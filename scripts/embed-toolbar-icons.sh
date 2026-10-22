#!/usr/bin/env bash
# scripts/embed-toolbar-icons.sh <out.bin> <fluent icon name>...   (Phase 4 WP3, Sheets and Slides)
# Rasterises the 20 px regular Fluent icons (assets/icons/fluent, MIT) at 20, 28 and 40 px into 8-bit alpha masks, the format the
# Docs layer already uses (docs/phase4/icons-wasm.md: the SVG loader costs about 309 KB brotli in the wasm, no app has that headroom).
# The patch carries the result as `crates/ui-egui/src/office_icons.bin`; the list of names must equal ICON_NAMES in office_toolbar.rs
# (a test checks it). resvg is a dev tool of domains/docs/crates/docs-office/tools/icon-bitmaps; nothing SVG-related is linked into an app.
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
out=$1; shift
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="$OFFICE_TARGET/icon-bitmaps"
cargo run --release -q --manifest-path domains/docs/crates/docs-office/tools/icon-bitmaps/Cargo.toml -- \
  assets/icons/fluent/svg "$out" 20,28,40 "$@"
