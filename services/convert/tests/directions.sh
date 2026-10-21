#!/usr/bin/env bash
# One run of every PLAN section 1 direction that needs no sidecar, through `convert`, on synthetic files (never corpus/).
# Usage: directions.sh <convert.exe> <input-dir> <out-dir>   (input-dir = the WP1 census inputs, E:/office-target/p3/census/in)
C=$1; IN=$2; OUT=$3; rm -rf "$OUT"; mkdir -p "$OUT"; fail=0
export CONVERT_TOOLS_DIR=${CONVERT_TOOLS_DIR:-}
run() { # label, expected rc, args...
  local label=$1 want=$2; shift 2
  "$C" "$@" --out-dir "$OUT/$label" --json >"$OUT/$label.json" 2>"$OUT/$label.err"; local rc=$?
  local n; n=$(ls "$OUT/$label" 2>/dev/null | wc -l)
  if [ "$rc" = "$want" ]; then echo "PASS $label rc=$rc files=$n"; else echo "FAIL $label rc=$rc (want $want): $(head -c 200 "$OUT/$label.err")"; fail=1; fi
}
for t in odt rtf html md txt pdf png; do run docx2$t 0 "$IN/hrm_order.docx" --to $t; done
for k in odt rtf; do run $k-docx 0 "$IN/order.$k" --to docx; done
run doc2docx 0 "$IN/hrm_order.doc" --to docx
run doc2pdf 0 "$IN/hrm_order.doc" --to pdf
run xls2xlsx 0 "$IN/hrm_staff.xls" --to xlsx
run xlsb2xlsx 0 "$IN/hrm_staff.xlsb" --to xlsx
run ods2xlsx 0 "$IN/hrm_staff.ods" --to xlsx
run xlsx2pdf 0 "$IN/hrm_staff.xlsx" --to pdf
run xls2pdf 0 "$IN/hrm_staff.xls" --to pdf
run xlsx2csv 0 "$IN/hrm_staff.xlsx" --to csv
run xlsx2csv-all 0 "$IN/hrm_staff.xlsx" --to csv --sheet all
run xlsx2csv-badsheet 2 "$IN/hrm_staff.xlsx" --to csv --sheet NoSuchSheet
run pptx2pdf 0 "$IN/hrm_deck.pptx" --to pdf
run pptx2png 0 "$IN/hrm_deck.pptx" --to png
run ppt2pdf 3 "$IN/hrm_deck.ppt" --to pdf
run pdf2png 0 "$IN/uz_latn.pdf" --to png --pages 1-2
run pdf2txt 0 "$IN/uz_latn.pdf" --to txt
run pdf2txt-scan 0 "$IN/scan.pdf" --to txt
run pdf2docx-native 0 "$IN/uz_latn.pdf" --to docx
run translit-docx 0 "$IN/uz_latn.docx" --to docx --translit cyrl
run translit-xlsx 0 "$IN/hrm_staff.xlsx" --to xlsx --translit cyrl
run translit-pptx 0 "$IN/02-languages.pptx" --to pptx --translit cyrl
cp "$IN/hrm_order.docx" "$OUT/mislabel.xlsx"; run mislabel 0 "$OUT/mislabel.xlsx" --to pdf
run batch-j2 0 "$IN/hrm_order.docx" "$IN/hrm_letter.docx" --to pdf -j 2
"$C" pdf combine "$IN/uz_latn.pdf" "$IN/uz_cyrl.pdf" -o "$OUT/combined.pdf" && echo "PASS pdf-combine" || { echo FAIL pdf-combine; fail=1; }
"$C" pdf split "$IN/uz_latn.pdf" --every 1 --out-dir "$OUT/split" >/dev/null && echo "PASS pdf-split $(ls "$OUT/split" | wc -l)" || { echo FAIL pdf-split; fail=1; }
"$C" pdf extract "$IN/uz_latn.pdf" --pages 1 -o "$OUT/extract.pdf" && echo "PASS pdf-extract" || { echo FAIL pdf-extract; fail=1; }
exit $fail
