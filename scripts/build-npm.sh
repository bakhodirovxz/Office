#!/usr/bin/env bash
# Build the uz-translit npm package (PLAN phase 2 section 7): wasm32 build -> wasm-bindgen --target web -> wasm-opt -Oz
# -> package.json from the template -> `npm pack`. Output: $OFFICE_TARGET/dist/npm/{uz-translit/,uz-translit-<ver>.tgz}.
# Nothing is published. Needs wasm-bindgen 0.2.129 and wasm-opt (scripts/tools.lock).
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
unset RUSTC_WRAPPER   # sccache cannot spawn rustc for wasm32 on Windows (as in the bind job)
OUT="$OFFICE_TARGET/dist/npm"; PKG="$OUT/uz-translit"; TGT="$OFFICE_TARGET/npm-wasm"
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/uz-translit-wasm/Cargo.toml | head -1)
want=$(awk '$1=="wasm-bindgen-cli"{print $2}' scripts/tools.lock)
[ "$(wasm-bindgen --version | awk '{print $2}')" = "$want" ] || { echo "wasm-bindgen CLI must be $want"; exit 2; }
rm -rf "$OUT"; mkdir -p "$PKG"
CARGO_TARGET_DIR="$TGT" cargo build --locked --profile npm -p uz-translit-wasm --target wasm32-unknown-unknown
wasm-bindgen --target web --out-name uz_translit --out-dir "$PKG" "$TGT/wasm32-unknown-unknown/npm/uz_translit_wasm.wasm"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  "$PKG/uz_translit_bg.wasm" -o "$PKG/uz_translit_bg.wasm.opt" && mv "$PKG/uz_translit_bg.wasm.opt" "$PKG/uz_translit_bg.wasm"
sed "s/@VERSION@/$VER/" ci/npm/package.template.json >"$PKG/package.json"
cp ci/npm/README.md LICENSE-MIT LICENSE-APACHE "$PKG/"
(cd "$OUT" && npm pack "./uz-translit" --silent)
ls -l "$OUT"/*.tgz
