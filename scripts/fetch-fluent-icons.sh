#!/usr/bin/env bash
# Re-creates assets/icons/fluent/ from the pinned @fluentui/svg-icons release and manifest.txt.
# Usage: scripts/fetch-fluent-icons.sh   (needs curl, tar, sha256sum; writes only under assets/icons/fluent/)
set -euo pipefail
VER=1.1.344
SHA256=6fe61f489a6d0d065ec2182794fdadf0e15db7d3388997c5ba8039dcfaaa35e8
LIC_SHA256=69bc45dc42b9acb96a69823adbc6ae538374e3c0bde169b855b32c48eaaef52f
URL="https://registry.npmjs.org/@fluentui/svg-icons/-/svg-icons-$VER.tgz"
LIC_URL="https://raw.githubusercontent.com/microsoft/fluentui-system-icons/$VER/LICENSE"
cd "$(dirname "$0")/.."
OUT=assets/icons/fluent
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
curl --ssl-no-revoke -fsSL "$URL" -o "$tmp/pkg.tgz"
echo "$SHA256  $tmp/pkg.tgz" | sha256sum -c - >/dev/null || { echo "archive sha256 mismatch" >&2; exit 1; }
curl --ssl-no-revoke -fsSL "$LIC_URL" -o "$tmp/LICENSE"
echo "$LIC_SHA256  $tmp/LICENSE" | sha256sum -c - >/dev/null || { echo "LICENSE sha256 mismatch" >&2; exit 1; }
grep -q '^MIT License' "$tmp/LICENSE" || { echo "LICENSE is not MIT" >&2; exit 1; }
mkdir "$tmp/x" && tar xzf "$tmp/pkg.tgz" -C "$tmp/x"
grep -q '"license": "MIT"' "$tmp/x/package/package.json" || { echo "package is not MIT" >&2; exit 1; }
rm -rf "$OUT/svg" && mkdir -p "$OUT/svg"
cp "$tmp/LICENSE" "$OUT/LICENSE"
for name in $(grep -v '^#' "$OUT/manifest.txt" | awk 'NF{print $2}' | sort -u); do
  for size in 20 24; do
    f="${name}_${size}_regular.svg"
    if [ ! -f "$tmp/x/package/icons/$f" ]; then # a few icons exist at 20 px only; 20 px is the one we embed
      [ "$size" = 24 ] && { echo "note: no 24 px for $name" >&2; continue; }
      echo "missing icon $f" >&2; exit 1
    fi
    cp "$tmp/x/package/icons/$f" "$OUT/svg/$f"
  done
done
echo "$(ls "$OUT/svg" | wc -l) files, $(cat "$OUT"/svg/* | wc -c) bytes in $OUT/svg"
