#!/usr/bin/env bash
# Re-create assets/fonts/fallback/*.ttf from the pinned Noto sources (WP8, docs/phase1/designs/font.md).
#   scripts/subset-fallback-font.sh [--check]    --check only verifies the committed subsets (cmap coverage)
# Sources are fetched with curl --ssl-no-revoke into a scratch dir, verified by SHA-256, subset with fontTools,
# then the cmap check asserts that every required code point is present.
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
OUT=assets/fonts/fallback
PIN=5c3913063bb189df4edc94b766584dfb4c04a187
BASE_URL="https://raw.githubusercontent.com/notofonts/notofonts.github.io/$PIN/fonts"
SANS_SHA=f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8
MONO_SHA=87f8ce0522a6c99b743ee5fc75b4073cfdd575639119672828b7b9944b65b4f4
RANGES='U+0020-007E,U+00A0-024F,U+02B9-02BC,U+02C6-02C7,U+02D8-02DD,U+0300-0304,U+0306-0308,U+030C,U+0400-04FF,U+2010-2027,U+2030-203A,U+20AC,U+20B8,U+20BD,U+2116,U+2122,U+2212,U+FFFD'
PY="${PYTHON:-python}"

check() {
  "$PY" -I - "$OUT/NotoSans-Regular-uz.ttf" "$OUT/NotoSansMono-Regular-uz.ttf" <<'PYEOF'
import sys
from fontTools.ttLib import TTFont
TEXT = "O\u02bbzbekiston g\u02bboya ma\u02bcno \u040e\u0437\u0431\u0435\u043a\u0438\u0441\u0442\u043e\u043d \u049b\u045e\u0493\u0438\u0440\u0447\u043e\u049b \u0493\u043e\u044f \u04b3\u0430 \u20bd \u2116"
REQ = {"U+02BB": [0x2BB], "U+02BC": [0x2BC], "Russian": range(0x410, 0x450), "Yo": [0x401, 0x451],
       "Uzbek Cyrillic": [0x40E, 0x45E, 0x49A, 0x49B, 0x492, 0x493, 0x4B2, 0x4B3],
       "punct": [0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x201E, 0x2026, 0x2022, 0xAB, 0xBB],
       "symbols": [0x20BD, 0x2116, 0x20AC, 0x2122], "ASCII": range(0x20, 0x7F),
       "Latin-1": [c for c in range(0xA0, 0x100) if c != 0xAD], "TEXT": [ord(c) for c in TEXT if c != " "]}
bad = False
for p in sys.argv[1:]:
    cmap = set((TTFont(p, lazy=True).getBestCmap() or {}).keys())
    miss = {k: [hex(c) for c in v if c not in cmap] for k, v in REQ.items()}
    miss = {k: v for k, v in miss.items() if v}
    print(p, "OK" if not miss else f"MISSING {miss}")
    bad |= bool(miss)
sys.exit(1 if bad else 0)
PYEOF
}

if [ "${1:-}" = "--check" ]; then check; exit $?; fi

tmp=$(mktemp -d)
curl -fsS --ssl-no-revoke -o "$tmp/sans.ttf" "$BASE_URL/NotoSans/unhinted/ttf/NotoSans-Regular.ttf"
curl -fsS --ssl-no-revoke -o "$tmp/mono.ttf" "$BASE_URL/NotoSansMono/unhinted/ttf/NotoSansMono-Regular.ttf"
curl -fsS --ssl-no-revoke -o "$OUT/OFL.txt" "https://raw.githubusercontent.com/notofonts/latin-greek-cyrillic/main/OFL.txt"
[ "$(sha256sum "$tmp/sans.ttf" | cut -d' ' -f1)" = "$SANS_SHA" ] || { echo "NotoSans-Regular.ttf SHA-256 mismatch" >&2; exit 1; }
[ "$(sha256sum "$tmp/mono.ttf" | cut -d' ' -f1)" = "$MONO_SHA" ] || { echo "NotoSansMono-Regular.ttf SHA-256 mismatch" >&2; exit 1; }
for pair in "sans:NotoSans-Regular-uz.ttf" "mono:NotoSansMono-Regular-uz.ttf"; do
  "$PY" -I -m fontTools.subset "$tmp/${pair%%:*}.ttf" --unicodes="$RANGES" --layout-features='kern,liga,calt,ccmp,locl,mark,mkmk' \
    --name-IDs='*' --notdef-outline --output-file="$OUT/${pair#*:}"
done
rm -rf "$tmp"
check
