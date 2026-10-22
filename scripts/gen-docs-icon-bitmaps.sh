#!/usr/bin/env bash
# Regenerates domains/docs/crates/docs-office/assets/icons.bin: alpha masks (20, 28 and 40 px) of the Fluent icons the Docs
# ribbon draws (assets/icons/fluent, MIT). Dev tool only: the SVG loader does not fit the Docs wasm budget (docs/phase4/icons-wasm.md).
# Keep the list in step with ICONS in domains/docs/crates/docs-office/src/icons.rs (a test checks the names).
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="$OFFICE_TARGET/icon-bitmaps"
cargo run --release --manifest-path domains/docs/crates/docs-office/tools/icon-bitmaps/Cargo.toml -- \
  assets/icons/fluent/svg domains/docs/crates/docs-office/assets/icons.bin 20,28,40 \
  print document_pdf orientation document_landscape text_grammar_checkmark translate translate_auto history \
  window_header_horizontal window_header_horizontal_off
