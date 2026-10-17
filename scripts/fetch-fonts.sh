#!/usr/bin/env bash
# Fetch craft-fonts at the pinned SHA into upstream/craft-fonts (ignored by git) and verify it.
#   scripts/fetch-fonts.sh            fetch if missing, then verify
#   scripts/fetch-fonts.sh --verify   verify only
#   scripts/fetch-fonts.sh --gen      (re)generate patches/craft-fonts/SHA256SUMS from the extracted tree
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
. <(grep -E '^(sha|url)=' patches/craft-fonts/BASE)
C="$UPSTREAM_CACHE/craft-fonts"; DEST=upstream/craft-fonts; SUMS=patches/craft-fonts/SHA256SUMS
mode="${1:-fetch}"

verify() {
  [ -d "$DEST" ] || { echo "craft-fonts: $DEST missing"; return 1; }
  local n; n=$(wc -l < "$SUMS")
  ( cd "$DEST" && sha256sum --quiet -c "$OFFICE_ROOT/$SUMS" ) || { echo "craft-fonts: hash mismatch"; return 1; }
  [ "$(find "$DEST" -type f | wc -l)" = "$n" ] || { echo "craft-fonts: unexpected extra or missing files"; return 1; }
  echo "craft-fonts ${sha:0:7}: $n files OK"
}

if [ "$mode" = "--verify" ]; then verify; exit; fi

if [ ! -d "$DEST" ]; then
  [ -d "$C/.git" ] || { mkdir -p "$UPSTREAM_CACHE"; git clone -q "$url" "$C"; git -C "$C" config core.autocrlf false; }
  git -C "$C" cat-file -e "$sha^{commit}" 2>/dev/null || git -C "$C" fetch -q --tags origin
  mkdir -p "$DEST"
  git -c core.autocrlf=false -C "$C" archive "$sha" | tar -x -C "$DEST"
  mkdir -p "$OFFICE_CACHE"; [ -e "$OFFICE_CACHE/craft-fonts-${sha:0:7}.tar" ] || git -c core.autocrlf=false -C "$C" archive -o "$OFFICE_CACHE/craft-fonts-${sha:0:7}.tar" "$sha"
fi
if [ "$mode" = "--gen" ] || [ ! -s "$SUMS" ]; then
  ( cd "$DEST" && find . -type f | LC_ALL=C sort | sed 's|^\./||' | xargs sha256sum ) > "$SUMS"
  echo "wrote $SUMS"
fi
verify
