#!/usr/bin/env bash
# scripts/build.sh <domain|all|services> <desktop|web|cli|all> [--release] [--msvc]   (PLAN WP3, WP13)
# --msvc: target x86_64-pc-windows-msvc into $OFFICE_TARGET/msvc/<d> (desktop and cli only; `services` builds convert, convert-api, hoshiya (suite-desktop) and hoshiya-serve;
#   `all --msvc` also builds services). Needs VS Build Tools (docs/phase4/msvc.md).
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
release=""; msvc=""; for a in "$@"; do [ "$a" = "--release" ] && release=1; [ "$a" = "--msvc" ] && msvc=1; done

if [ "$d" = all ]; then
  rc=0; for x in slides pdf docs sheets; do "$0" $x "$what" "$@" || rc=$?; done
  [ -n "$msvc" ] && { "$0" services cli "$@" || rc=$?; }
  exit $rc
fi
if [ "$what" = all ]; then
  ws="desktop cli web"; [ -n "$msvc" ] && ws="desktop cli"
  [ "$d" = services ] && ws=cli
  rc=0; for w in $ws; do "$0" "$d" "$w" "$@" || rc=$?; done; exit $rc
fi

mkdir -p "$OFFICE_TARGET/logs" "$OFFICE_TARGET/dist"
log="$OFFICE_TARGET/logs/build-$d-$what${release:+-release}${msvc:+-msvc}.log"
export CARGO_TARGET_DIR="$OFFICE_TARGET/${msvc:+msvc/}$d"
mt=""; [ -n "$msvc" ] && mt="--target x86_64-pc-windows-msvc"
[ "$what" = web ] && [ -n "$msvc" ] && { echo "--msvc: no web target"; exit 2; }
export CARGO_BUILD_JOBS=4
[ "${OFFICE_FONTS:-parity}" = all ] || { [ "$d" = slides ] || unset CRAFT_FONTS_DIR; }
mf="domains/$d/Cargo.toml"


# Brand overlay (WP7): render brand/brand.toml into $OFFICE_OVERLAY_ROOT/<d>/ and point the builds at it. The web page is
# rendered from index.html.in because build.rs cannot write index.html. OFFICE_NO_OVERLAY=1 builds pristine-like.
overlay_prepare() {
  [ "$d" = services ] && return 0
  [ "${OFFICE_NO_OVERLAY:-0}" = 1 ] && { unset OFFICE_OVERLAY_DIR OFFICE_PRODUCT_VERSION OFFICE_PRODUCT_NAME; return 0; }
  (unset CARGO_TARGET_DIR RUSTC_WRAPPER; cargo xt overlay "$d") || return 1
  export OFFICE_OVERLAY_DIR="$OFFICE_OVERLAY_ROOT/$d"
  # WP9c/d: the Uzbek catalogs (review mode = drafts included, labelled in the language picker; OFFICE_I18N_MODE=release for reviewed rows only)
  # are exported next to overlay.env. The language hooks in the apps read catalog-uz-*.tsv, catalog-meta.env from there.
  local up; case "$d" in docs) up=wordcraft;; sheets) up=gridcraft;; slides) up=deckcraft;; pdf) up=pdfcraft;; esac
  if [ "${OFFICE_NO_I18N:-0}" != 1 ]; then
    (unset CARGO_TARGET_DIR RUSTC_WRAPPER; cargo xt i18n export "$up" --mode "${OFFICE_I18N_MODE:-review}" --out "$OFFICE_OVERLAY_ROOT/_i18n") || return 1
    for f in "$OFFICE_OVERLAY_ROOT/_i18n/$up/"*; do cmp -s "$f" "$OFFICE_OVERLAY_DIR/$(basename "$f")" || cp "$f" "$OFFICE_OVERLAY_DIR/" || return 1; done   # unchanged files keep their mtime (no rebuild)
  fi
  local web="domains/$d/apps/$d-web" name
  export OFFICE_PRODUCT_NAME=$(sed -n 's/^PRODUCT_NAME=//p' "$OFFICE_OVERLAY_DIR/overlay.env" | tr -d '\r')   # written into the files we produce (O3)
  export OFFICE_PRODUCT_VERSION=$(sed -n 's/^VERSION=//p' "$OFFICE_OVERLAY_DIR/overlay.env" | tr -d '\r')   # one product version for every app
  name=$(sed -n 's/^APP_NAME=//p' "$OFFICE_OVERLAY_DIR/overlay.env" | tr -d '\r')
  # trunk 0.21 mangles an absolute "E:/..." href when the repo is on the same drive ("E:office-target\..."): use a relative one there
  local fav="$OFFICE_OVERLAY_DIR/favicon.ico"
  [ "$(cygpath -m "$PWD/$web" | cut -c1-2)" = "$(cygpath -m "$fav" | cut -c1-2)" ] && fav=$(realpath --relative-to="$(cygpath -u "$PWD/$web")" "$(cygpath -u "$fav")")
  sed -e "s|{{APP_NAME}}|$name|g" -e "s|{{FAVICON}}|$fav|g" "$web/index.html.in" > "$web/index.html"
}

run() {
  ram_gate 6   # two builds may overlap (2-slot lock): free RAM must cover this one plus the running one
  overlay_prepare || return 1
  # msvc host toolchain (after the overlay step, which uses the default gnu one): build scripts such as winresource pick rc.exe only when the host is msvc
  [ -n "$msvc" ] && export RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc
  local t0=$SECONDS; echo "== $d $what ${release:+release} start $(date -Is) fonts=${CRAFT_FONTS_DIR:-unset} wrapper=${RUSTC_WRAPPER:-none} overlay=${OFFICE_OVERLAY_DIR:-none}"
  case "$what" in
    desktop) cargo build --locked -j 4 $mt ${release:+--release} --manifest-path "$mf" -p "$d-desktop" ;;
    cli)     if [ "$d" = services ]; then cargo build --locked -j 4 $mt ${release:+--release} --manifest-path services/Cargo.toml -p convert -p convert-api -p suite-desktop -p suite-serve
             else dp=""; case "$d" in docs|sheets) dp="-p $d-diff";; esac   # WP12 compare tools, shipped next to the apps
                  cargo build --locked -j 4 $mt ${release:+--release} --manifest-path "$mf" -p "$d-cli" $dp; fi ;;
    web)     # sccache cannot spawn rustc for web-sys (command line too long on Windows), so web builds run without it
             (unset RUSTC_WRAPPER; cd "domains/$d/apps/$d-web" && TRUNK_BUILD_DIST="$OFFICE_TARGET/dist/$d-web" trunk build ${release:+--release} --locked) ;;
    *) echo "unknown target $what"; return 2 ;;
  esac
  local rc=$?; local secs=$((SECONDS - t0))
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -Is)" "$d" "$what${release:+-release}${msvc:+-msvc}" "$rc" "$secs" "wrapper=$([ "$what" = web ] && echo none || echo "${RUSTC_WRAPPER:-none}")" >> "$OFFICE_TARGET/logs/times.tsv"
  echo "== $d $what done rc=$rc in ${secs}s"; return $rc
}

with_build_lock run 2>&1 | tee "$log"
exit "${PIPESTATUS[0]}"
