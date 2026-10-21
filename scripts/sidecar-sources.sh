#!/usr/bin/env bash
# Fetch the exact source tarballs of the AGPL components of the pdf2docx sidecar image (AGPL source offer).
# Usage: scripts/sidecar-sources.sh [outdir]   (default: E:/office-sidecar-sources/pdf2docx/<pymupdf version>)
# Reads the pins from sidecars/pdf2docx/requirements.lock, downloads each sdist from PyPI, verifies the sha256 that
# PyPI publishes, and writes MANIFEST.txt (name, version, file, sha256, URL). Keep the directory with the image tag.
set -euo pipefail
cd "$(dirname "$0")/.."
LOCK=sidecars/pdf2docx/requirements.lock
PY=${PYTHON:-python}
AGPL=(PyMuPDF)   # add a name here if another AGPL part appears.
ver() { sed -n "s/^$1==\([^ ]*\) .*/\1/Ip" "$LOCK"; }
out=${1:-E:/office-sidecar-sources/pdf2docx/pymupdf-$(ver PyMuPDF)}
mkdir -p "$out"
: > "$out/MANIFEST.txt"
for name in "${AGPL[@]}"; do
  v=$(ver "$name"); [ -n "$v" ] || { echo "no pin for $name in $LOCK" >&2; exit 1; }
  json=$(curl -fsS --ssl-no-revoke "https://pypi.org/pypi/$name/$v/json")
  read -r url file sha < <(printf '%s' "$json" | "$PY" -c '
import json, sys
u = [x for x in json.load(sys.stdin)["urls"] if x["packagetype"] == "sdist"][0]
print(u["url"], u["filename"], u["digests"]["sha256"])' | tr -d '\015')
  curl -fsS --ssl-no-revoke -o "$out/$file" "$url"
  got=$(sha256sum "$out/$file" | cut -d" " -f1)
  [ "$got" = "$sha" ] || { echo "sha256 mismatch for $file: $got != $sha" >&2; exit 1; }
  echo "$name $v $file sha256:$sha $url" >> "$out/MANIFEST.txt"
  tar -xzOf "$out/$file" --wildcards '*/COPYING' > "$out/$name-COPYING.txt" 2>/dev/null || true
done
# MuPDF is not inside the PyMuPDF sdist (PyMuPDF's build downloads it). The version must equal the one in the image:
# docker run --rm <image> python -c 'import fitz; print(fitz.__doc__)'  prints "... MuPDF x.y.z" (WP5 recorded 1.26.12).
MUPDF=${MUPDF_VERSION:-1.26.12}
mf=mupdf-$MUPDF-source.tar.gz
curl -fsSL --ssl-no-revoke -o "$out/$mf" "https://mupdf.com/downloads/archive/$mf"
echo "mupdf $MUPDF $mf sha256:$(sha256sum "$out/$mf" | cut -d" " -f1) https://mupdf.com/downloads/archive/$mf (no published checksum; recorded here)" >> "$out/MANIFEST.txt"
cat "$out/MANIFEST.txt"
echo "sources in $out"
