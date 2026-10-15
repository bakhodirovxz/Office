"""expect_rows.py OUT.xlsx --rows N [--sheets 30]

Replays gen.py's seed-42 sequence and checks every formula cell of sheets S01..Snn in OUT.xlsx (a
file saved by gridcraft, so values are cached) against the value the formula must give, compared
exactly (==) as IEEE doubles:
  G = IF(C>500, C*D, 0)            -> amt*qty if amt > 500 else 0
  H = VLOOKUP(F, Lookup, 2, FALSE) -> round(1 + k*0.05, 2) for category Kk
  I = C+D (S01) / Sk!C+D           -> amt+qty  (the formula reads its own sheet: gen.py's known
                                       "Link" bug, so this is what the formula computes, not what
                                       gen.py's expectations assumed)
Prints a JSON summary; exit code 1 on any mismatch.
"""
import argparse
import json
import random
import re
import sys
import zipfile

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("--rows", type=int, required=True)
ap.add_argument("--sheets", type=int, default=30)
a = ap.parse_args()

rnd = random.Random(42)
WORDS = ["Приход", "Расход", "Оплата", "Склад", "Товар", "Order", "Invoice", "Клиент", "Supplier", "Возврат", "Delivery", "Акт"]
factor = {f"K{i:02d}": round(1 + i * 0.05, 2) for i in range(20)}
z = zipfile.ZipFile(a.out)
wb = z.read("xl/workbook.xml").decode("utf-8")
names = re.findall(r'<sheet [^>]*name="([^"]*)"', wb)
CELL = re.compile(r'<c r="([GHI])(\d+)"[^>]*>(?:<f[^>]*/>|<f[^>]*>[^<]*</f>)<v>([^<]*)</v></c>')
checked = 0
bad = []
for s in range(a.sheets):
    rows = []
    for r in range(a.rows):
        amt = round(rnd.uniform(1, 1000), 2)
        qty = rnd.randint(1, 50)
        cat = f"K{rnd.randint(0, 19):02d}"
        rnd.randint(0, 2000)
        f"{rnd.choice(WORDS)} {rnd.choice(WORDS)} №{rnd.randint(1000, 99999)}"
        rows.append((amt * qty if amt > 500 else 0, factor[cat], amt + qty))
    name = f"S{s + 1:02d}"
    xml = z.read(f"xl/worksheets/sheet{names.index(name) + 1}.xml").decode("utf-8")
    seen = 0
    for col, row, v in CELL.findall(xml):
        r = int(row) - 2
        if not 0 <= r < a.rows:
            continue
        want = rows[r]["GHI".index(col)]
        seen += 1
        if float(v) != float(want):
            bad.append((name, f"{col}{row}", v, want))
    checked += seen
    if seen != 3 * a.rows:
        bad.append((name, "formula cells found", seen, 3 * a.rows))
    del xml
print(json.dumps({"file": a.out, "rows": a.rows, "formula_values_checked": checked, "mismatches": bad[:10], "ok": not bad}))
sys.exit(1 if bad else 0)
