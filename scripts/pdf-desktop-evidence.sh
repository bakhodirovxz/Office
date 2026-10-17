#!/usr/bin/env bash
# Desktop brand evidence for the pdf app (WP7): window title, window capture, About dialog and the accessibility-tree check.
#   scripts/pdf-desktop-evidence.sh [outdir]
# pdfcraft is driven with --control <file> and `pdf-cli ui` (the other apps: scripts/desktop-brand-evidence.py).
set -euo pipefail
. "$(dirname "$0")/env.sh"
OUT="${1:-$OFFICE_ROOT/docs/phase1/evidence/brand}"; mkdir -p "$OUT"
EXE="$OFFICE_TARGET/pdf/release/pdf.exe"; CLI="$OFFICE_TARGET/pdf/release/pdf-cli.exe"
CTL="$OFFICE_TARGET/pdf-evidence-control.json"; rm -f "$CTL"
powershell -NoProfile -File "$OFFICE_ROOT/scripts/window-shot.ps1" -Exe "$EXE" -Out "$OUT/brand-pdf-desktop.png" -Wait 12
"$EXE" --control "$CTL" >/dev/null 2>&1 &
PID=$!
trap 'taskkill //F //IM pdf.exe >/dev/null 2>&1 || true' EXIT
for _ in $(seq 1 60); do [ -s "$CTL" ] && break; sleep 1; done; sleep 8
echo "window title: $(powershell -NoProfile -Command "(Get-Process pdf | Select-Object -First 1).MainWindowTitle" | tr -d '\r')"
"$CLI" ui --control "$CTL" ui.command id=help.about >/dev/null; sleep 2
"$CLI" ui --control "$CTL" ui.screenshot > "$OUT/.pdf-about.json"
python -I - "$OUT" <<'PY'
import base64, json, sys
out = sys.argv[1]
d = json.load(open(out + "/.pdf-about.json"))
open(out + "/brand-pdf-desktop-about.png", "wb").write(base64.b64decode(d["png_base64"]))
PY
rm -f "$OUT/.pdf-about.json"
"$CLI" ui --control "$CTL" ui.inspect limit=5000 > "$OUT/.pdf-inspect.json"
python -I - "$OUT" <<'PY'
import json, re, sys
d = json.load(open(sys.argv[1] + "/.pdf-inspect.json", encoding="utf-8"))
allowed = re.compile(r"based on|основано|asosida|асосида", re.I)  # the attribution line (exact text from brand.toml)
bad = [w for w in d["widgets"] for t in (w.get("label"), w.get("value")) if t and re.search(r"craft|discord|storytold", t, re.I) and not allowed.search(t)]
print(f"accessibility tree: {len(d['widgets'])} widgets, {len(bad)} with an upstream brand token outside the attribution line")
sys.exit(1 if bad else 0)
PY
rm -f "$OUT/.pdf-inspect.json"
