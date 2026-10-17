#!/usr/bin/env bash
# Generates the root LICENSE-MIT and NOTICE of our layer (WP7g, docs/phase1/designs/brand.md section 8).
#   scripts/gen-legal.sh           write LICENSE-MIT and NOTICE
#   scripts/gen-legal.sh --check   fail when the committed files differ from the generated ones
# The product name and the attribution come from brand/brand.toml, never from this script. LICENSE-APACHE is the
# unmodified Apache-2.0 text and is not generated. The upstream NOTICE files are appended verbatim (Apache-2.0
# section 4(d)); their copyright lines are repeated in the "Upstream copyright notices" section (MIT and Apache-2.0
# section 4(c) require them to be kept with copies).
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"

get() { # <section> <key>
  awk -v sec="[$1]" -v key="$2" '$0==sec{f=1;next} /^\[/{f=0} f && $1==key {sub(/^[^=]*=[ \t]*/,""); sub(/[ \t]*#.*$/,""); gsub(/"/,""); print; exit}' brand/brand.toml
}
NAME=$(get product name); VENDOR=$(get product vendor); ATTR=$(get attribution en)
[ -n "$NAME" ] && [ -n "$VENDOR" ] && [ -n "$ATTR" ] || { echo "brand/brand.toml: product name, vendor or attribution missing" >&2; exit 2; }

out=$(mktemp -d)
{
  echo "MIT License"
  echo
  echo "Copyright (c) 2026 $VENDOR"
  echo
  sed -n '5,$p' upstream/pdfcraft/LICENSE-MIT
} > "$out/LICENSE-MIT"

{
  cat <<EOT
$NAME
Copyright (c) 2026 $VENDOR.
Licensed under the MIT License or the Apache License, Version 2.0, at your option
(LICENSE-MIT, LICENSE-APACHE). This applies to the files of our layer: crates/, xtask/, domains/,
scripts/, brand/, i18n/ and the patch files in patches/.

$ATTR

The products are built from unmodified vendored snapshots of those applications (upstream/<repo>/,
pinned in patches/<repo>/BASE) plus a queue of patches (patches/<repo>/NNNN-*.patch, listed with
the reason for each in patches/REGISTER.md). Every changed upstream file is therefore recorded
as a patch (Apache-2.0 section 4(b)); the upstream LICENSE, NOTICE and ATTRIBUTION files stay in the
snapshots untouched.

The ArtCraft name, wordmark and logos are trademarks of the ArtCraft Team. They are not open source,
are not used for this product, and are not covered by the licence above. The builds contain none of
the ArtCraft logo files (checked by \`cargo xt brand-grep\` and \`cargo xt brand-scan-bin\`).

Third-party material added by this layer

- Noto Sans and Noto Sans Mono, subset (assets/fonts/fallback/NotoSans-Regular-uz.ttf and
  NotoSansMono-Regular-uz.ttf; the faces office-fallback-sans and office-fallback-mono): Copyright 2022
  The Noto Project Authors (https://github.com/notofonts/latin-greek-cyrillic), SIL Open Font License 1.1,
  no Reserved Font Name. The files are modified: they are subsets made by scripts/subset-fallback-font.sh.
  Provenance and checksums: assets/fonts/fallback/SOURCE.md. The licence text follows.

$(sed 's/^/  /' assets/fonts/fallback/OFL.txt)

Third-party material carried by the vendored applications (full lists in each upstream NOTICE below)

- Foxit standard-14 substitute fonts (FoxitSans, FoxitFixed, FoxitDingbats and their styles, in
  vendor/hayro-interpret/assets, linked through hayro): Original code copyright 2014 Foxit Software Inc.
  (http://www.foxitsoftware.com); Copyright 2014 PDFium Authors. BSD-3-Clause. The licence text follows
  (upstream/pdfcraft/vendor/hayro-interpret/assets/LICENSE_FOXIT).

$(sed 's/^/  /' upstream/pdfcraft/vendor/hayro-interpret/assets/LICENSE_FOXIT)

Code dependencies under the Mozilla Public License 2.0

The slide application (audio decoding) links the following 16 crates of the Symphonia family, version 0.5.5,
under the Mozilla Public License 2.0. They are used unmodified. Their source code is available at
https://github.com/pdeljanov/Symphonia and from crates.io. The licence text is at
https://www.mozilla.org/MPL/2.0/. No other dependency is under a copyleft licence.

$(awk '/^name = "symphonia/ {gsub(/"/, "", $3); printf "  - %s\n", $3}' domains/slides/Cargo.lock)

Upstream copyright notices (from the upstream LICENSE-MIT files)

$(for r in wordcraft gridcraft deckcraft pdfcraft craft-libs; do printf '  %s: ' "$r"; grep -m1 '^Copyright' "upstream/$r/LICENSE-MIT"; done)

Upstream NOTICE files (verbatim)

The following are the NOTICE files of the vendored upstream projects, reproduced without change.
EOT
  for r in wordcraft gridcraft deckcraft pdfcraft craft-libs; do
    echo
    echo "----- upstream/$r/NOTICE -----"
    cat "upstream/$r/NOTICE"
  done
} > "$out/NOTICE"

if [ "${1:-}" = "--check" ]; then
  rc=0
  for f in LICENSE-MIT NOTICE; do
    cmp -s "$out/$f" "$f" || { echo "$f is out of date: run scripts/gen-legal.sh" >&2; rc=1; }
  done
  [ -f LICENSE-APACHE ] || { echo "LICENSE-APACHE missing" >&2; rc=1; }
  rm -rf "$out"; exit $rc
fi
cp "$out/LICENSE-MIT" LICENSE-MIT; cp "$out/NOTICE" NOTICE
[ -f LICENSE-APACHE ] || cp upstream/pdfcraft/LICENSE-APACHE LICENSE-APACHE
rm -rf "$out"
echo "wrote LICENSE-MIT, NOTICE ($(wc -l < NOTICE) lines)"
