#!/usr/bin/env bash
# Clean checkout -> tools present, E: dirs created, craft-fonts fetched, sccache started.
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
rc=0
mkdir -p /e/office-tools/bin /e/office-target/p1/logs /e/office-cache /e/office-upstream

# Tool versions must match scripts/tools.lock
while read -r name ver _; do
  case "$name" in \#*|"") continue ;; esac
  case "$name" in
    cargo-deny)    got=$(cargo deny --version 2>&1 | awk '{print $2}') ;;
    cargo-nextest) got=$(cargo nextest --version 2>&1 | awk 'NR==1{print $2}') ;;
    sccache)       got=$(sccache --version 2>&1 | awk '{print $2}') ;;
    actionlint)    got=$(actionlint --version 2>&1 | head -1) ;;
    trunk)         got=$(trunk --version 2>&1 | awk '{print $2}') ;;
    wasm-bindgen-cli) got=$(wasm-bindgen --version 2>&1 | awk '{print $2}') ;;   # must equal the wasm-bindgen pin in Cargo.lock
    binaryen)      got=$(wasm-opt --version 2>&1 | awk '{print $3}') ;;
    *) continue ;;
  esac
  if [ "$got" = "$ver" ]; then echo "tool ok: $name $ver"; else echo "TOOL MISMATCH: $name want $ver got '$got'"; rc=1; fi
done < scripts/tools.lock

for c in rustfmt clippy; do rustup component list --installed | grep -q "^$c" || { echo "missing rustup component: $c"; rc=1; }; done
rustup target list --installed | grep -q '^wasm32-unknown-unknown$' || { echo "missing target wasm32-unknown-unknown"; rc=1; }

# Upstream cache clones must exist on E: (scripts/sync-upstream.sh clones them if absent)
for r in wordcraft gridcraft deckcraft pdfcraft craft-libs craft-fonts; do
  [ -d "$UPSTREAM_CACHE/$r/.git" ] || echo "note: $UPSTREAM_CACHE/$r missing (clone it from patches/$r/BASE url)"
done

[ -x scripts/fetch-fonts.sh ] && scripts/fetch-fonts.sh || true
sccache --start-server >/dev/null 2>&1 || echo "note: sccache server did not start; unset RUSTC_WRAPPER and record it"
exit $rc
