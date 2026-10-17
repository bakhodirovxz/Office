#!/usr/bin/env bash
# scripts/build.sh <domain|all> <desktop|web|cli|all> [--release]   (PLAN WP3)
# One build at a time (build lock), RAM gate, log and wall time. Output on E: only.
#   logs:  $OFFICE_TARGET/logs/build-<d>-<what>.log     times: $OFFICE_TARGET/logs/times.tsv
#   cargo target dir per domain: $OFFICE_TARGET/<d>;  web dist: $OFFICE_TARGET/dist/<d>-web (domains/<d>/apps/<d>-web/Trunk.toml)
# CRAFT_FONTS_DIR: the plan's env.sh exports it for every domain. Phase 0 had it only for deckcraft (its .cargo/config.toml),
# so for like-for-like size comparison it is kept only for `slides` unless OFFICE_FONTS=all (see docs/phase1/domain-build.md).
set -uo pipefail
. "$(dirname "$0")/env.sh"
. "$OFFICE_ROOT/scripts/lib/lock.sh"
cd "$OFFICE_ROOT"
d=${1:?domain|all}; what=${2:?desktop|web|cli|all}; shift 2
release=""; for a in "$@"; do [ "$a" = "--release" ] && release=1; done

if [ "$d" = all ]; then
  rc=0; for x in slides pdf docs sheets; do "$0" $x "$what" "$@" || rc=$?; done; exit $rc
fi
if [ "$what" = all ]; then
  rc=0; for w in desktop cli web; do "$0" "$d" "$w" "$@" || rc=$?; done; exit $rc
fi

mkdir -p "$OFFICE_TARGET/logs" "$OFFICE_TARGET/dist"
log="$OFFICE_TARGET/logs/build-$d-$what${release:+-release}.log"
export CARGO_TARGET_DIR="$OFFICE_TARGET/$d"
export CARGO_BUILD_JOBS=4
[ "${OFFICE_FONTS:-parity}" = all ] || { [ "$d" = slides ] || unset CRAFT_FONTS_DIR; }
mf="domains/$d/Cargo.toml"

wait_no_rustc() {
  while tasklist //FI "IMAGENAME eq rustc.exe" 2>/dev/null | grep -qi rustc.exe; do echo "another rustc.exe is running; waiting"; sleep 20; done
}

run() {
  wait_no_rustc; ram_gate 3
  local t0=$SECONDS; echo "== $d $what ${release:+release} start $(date -Is) fonts=${CRAFT_FONTS_DIR:-unset} wrapper=${RUSTC_WRAPPER:-none}"
  case "$what" in
    desktop) cargo build --locked -j 4 ${release:+--release} --manifest-path "$mf" -p "$d-desktop" ;;
    cli)     cargo build --locked -j 4 ${release:+--release} --manifest-path "$mf" -p "$d-cli" ;;
    web)     # sccache cannot spawn rustc for web-sys (command line too long on Windows), so web builds run without it
             (unset RUSTC_WRAPPER; cd "domains/$d/apps/$d-web" && trunk build ${release:+--release} --locked) ;;
    *) echo "unknown target $what"; return 2 ;;
  esac
  local rc=$?; local secs=$((SECONDS - t0))
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -Is)" "$d" "$what${release:+-release}" "$rc" "$secs" "wrapper=$([ "$what" = web ] && echo none || echo "${RUSTC_WRAPPER:-none}")" >> "$OFFICE_TARGET/logs/times.tsv"
  echo "== $d $what done rc=$rc in ${secs}s"; return $rc
}

with_build_lock run 2>&1 | tee "$log"
exit "${PIPESTATUS[0]}"
