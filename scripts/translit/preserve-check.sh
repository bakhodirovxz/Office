#!/usr/bin/env bash
# Phase 2 WP9: formatting preservation for the Excel-written .xlsx and PowerPoint-written .pptx fixtures (PLAN 8.2).
#   scripts/translit/preserve-check.sh          needs release sheets-cli / slides-cli ($OFFICE_TARGET/{sheets,slides}/release)
# B = open -> save, R = open -> command -> save. Round 1 converts to Cyrillic, round 2 takes that file and converts back.
# Prints IDENTICAL per file and direction, or the deltas; exit 1 if any differs.
set -uo pipefail
. "$(dirname "$0")/../env.sh"
T=$(mktemp -d); rc=0
check() { # <kind> <cli> <savearg> <file>
  local kind=$1 cli=$2 save=$3 f=$4 n; n=$(basename "$f")
  "$cli" run --in "$f" $save "$T/b1.$kind" >/dev/null 2>&1 || { echo "FAIL $n: B1"; rc=1; return; }
  "$cli" run --in "$f" --cmd 'tools.translitCyrl={}' $save "$T/r1.$kind" >/dev/null 2>&1 || { echo "FAIL $n: R1"; rc=1; return; }
  "$cli" run --in "$T/r1.$kind" $save "$T/b2.$kind" >/dev/null 2>&1 || { echo "FAIL $n: B2"; rc=1; return; }
  "$cli" run --in "$T/r1.$kind" --cmd 'tools.translitLatn={}' $save "$T/r2.$kind" >/dev/null 2>&1 || { echo "FAIL $n: R2"; rc=1; return; }
  for r in 1 2; do
    printf '%s %s: ' "$n" "$([ $r = 1 ] && echo 'to Cyrillic' || echo 'to Latin')"
    python -I "$(dirname "$0")/preserve.py" "$kind" "$T/b$r.$kind" "$T/r$r.$kind" || rc=1
  done
}
for f in domains/sheets/tests/translit-fixtures/xlsx/*.xlsx; do check xlsx "$OFFICE_TARGET/sheets/release/sheets-cli.exe" --out "$f"; done
for f in domains/slides/tests/translit-fixtures/pptx/*.pptx; do check pptx "$OFFICE_TARGET/slides/release/slides-cli.exe" --save "$f"; done
rm -rf "$T"; exit $rc
