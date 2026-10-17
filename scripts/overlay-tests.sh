#!/usr/bin/env bash
# Run the brand/font overlay tests of one app in its upstream workspace with the overlay ON (WP7/WP8 done-check).
#   scripts/overlay-tests.sh <docs|sheets|slides|pdf> [test filter ...]   (default filter: office_overlay; WP9: office_overlay i18n)
# One build at a time: waits for the build lock, no other rustc.exe and 3 GB of free RAM.
set -uo pipefail
. "$(dirname "$0")/env.sh"
. "$OFFICE_ROOT/scripts/lib/lock.sh"
cd "$OFFICE_ROOT"
d=${1:?docs|sheets|slides|pdf}; shift; filters=${*:-office_overlay}
case "$d" in docs) app=wordcraft;; sheets) app=gridcraft;; slides) app=deckcraft;; pdf) app=pdfcraft;; *) echo "unknown domain $d" >&2; exit 2;; esac
(unset CARGO_TARGET_DIR RUSTC_WRAPPER; cargo xt overlay "$d" && cargo xt i18n export "$app" --mode "${OFFICE_I18N_MODE:-review}" --out "$OFFICE_OVERLAY_ROOT/_i18n") || exit 2
for f in "$OFFICE_OVERLAY_ROOT/_i18n/$app/"*; do cmp -s "$f" "$OFFICE_OVERLAY_ROOT/$d/$(basename "$f")" || cp "$f" "$OFFICE_OVERLAY_ROOT/$d/"; done
wait_no_rustc() { while tasklist //FI "IMAGENAME eq rustc.exe" 2>/dev/null | grep -qi rustc.exe; do echo "another rustc.exe is running; waiting"; sleep 20; done; }
run() {
  wait_no_rustc; ram_gate 3
  export OFFICE_OVERLAY_DIR="$OFFICE_OVERLAY_ROOT/$d" OFFICE_PRODUCT_NAME=$(sed -n 's/^PRODUCT_NAME=//p' "$OFFICE_OVERLAY_ROOT/$d/overlay.env" | tr -d '\r')
  export OFFICE_PRODUCT_VERSION=$(sed -n 's/^VERSION=//p' "$OFFICE_OVERLAY_ROOT/$d/overlay.env" | tr -d '\r')
  export CARGO_TARGET_DIR="$OFFICE_TARGET/upstream-$app" CARGO_BUILD_JOBS=4
  [ "$d" = slides ] && export CRAFT_FONTS_DIR="$OFFICE_ROOT/upstream/craft-fonts"
  (cd "upstream/$app" && cargo test --locked -j 4 --lib -p "$app-ui-egui" -- $filters)
}
with_build_lock run
