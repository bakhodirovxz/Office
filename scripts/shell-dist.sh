#!/usr/bin/env bash
# Assemble what suite-serve serves (PLAN phase 4, 3.1): copy web/shell and the Noto Sans Uz subset to $OFFICE_TARGET/dist/shell, then
# write a .br next to every shell file and every file of the four trunk dists. Same settings as the size budgets
# (brotli -q 11 --lgwin=24). A .br that is newer than its source is kept, so a rerun after one rebuilt editor is cheap.
#   scripts/shell-dist.sh [--check]     --check only verifies that every .br exists and is not older than its source
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/env.sh"
cd "$OFFICE_ROOT"
DIST="${CI_DIST_ROOT:-$OFFICE_TARGET/dist}"
SHELL_OUT="$DIST/shell"
check=0; [ "${1:-}" = --check ] && check=1
command -v brotli >/dev/null 2>&1 || { echo "shell-dist: brotli CLI not found on PATH" >&2; exit 2; }

if [ "$check" = 0 ]; then
  rm -rf "$SHELL_OUT"; mkdir -p "$SHELL_OUT"
  cp -r web/shell/. "$SHELL_OUT/"
  cp assets/fonts/fallback/NotoSans-Regular-uz.ttf "$SHELL_OUT/"
  # the plugin page host (Phase 7 WP5): fetched by the shell and the docserver wrapper only when a plugin is enabled; suite-serve gives the .js files the Worker CSP
  mkdir -p "$SHELL_OUT/plugins"; cp web/plugins/host.js web/plugins/worker.js web/plugins/host.css web/plugins/i18n.json web/plugins/page.js "$SHELL_OUT/plugins/"
  # the logo pack is the only source of brand art (brand/logo): favicon, touch icon, PWA icons, header lockups
  L=brand/logo
  cp "$L/web/favicon.ico" "$L/web/favicon.svg" "$L/web/apple-touch-icon.png" "$L/web/icon-192.png" "$L/web/icon-512.png" "$L/web/icon-maskable-512.png" "$SHELL_OUT/"
  cp "$L/svg/hoshiya-lockup-light.svg" "$SHELL_OUT/logo-light.svg"; cp "$L/svg/hoshiya-lockup-dark.svg" "$SHELL_OUT/logo-dark.svg"
  sed 's#"/icon#"icon#' "$L/web/site.webmanifest" > "$SHELL_OUT/site.webmanifest"
fi

bad=0
while IFS= read -r f; do
  if [ "$check" = 1 ]; then
    { [ -e "$f.br" ] && [ ! "$f" -nt "$f.br" ]; } || { echo "stale or missing: $f.br"; bad=1; }
  elif [ ! -e "$f.br" ] || [ "$f" -nt "$f.br" ]; then
    brotli -q 11 --lgwin=24 -f -c "$f" > "$f.br"
  fi
done < <(find "$SHELL_OUT" "$DIST"/{docs,sheets,slides,pdf}-web -type f \( -name '*.js' -o -name '*.wasm' -o -name '*.html' -o -name '*.css' -o -name '*.json' -o -name '*.ttf' \) 2>/dev/null)
exit $bad
