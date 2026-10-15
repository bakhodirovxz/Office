"""expect_summary.py OUT.xlsx --rows N [--sheets 30]

Recomputes, with gen.py's seed-42 sequence, the Summary values gen.py's workbook should show
(per sheet: SUM of Big, SUMIFS of Amount for K05, XLOOKUP of the first K05 id, COUNTIF of K05),
reads the cached Summary values from OUT.xlsx (a file saved by gridcraft, so values are cached),
and compares them (relative tolerance 1e-9 for sums, exact for counts and ids). Prints a JSON
summary; exit code 1 on a mismatch. Only the random sequence of gen.py is replayed, nothing is
written.
"""
import argparse
import datetime
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
base = datetime.date(2020, 1, 1)
exp = []
for s in range(a.sheets):
    sumG = 0.0
    sumC = 0.0
    cnt = 0
    first = None
    for r in range(a.rows):
        amt = round(rnd.uniform(1, 1000), 2)
        qty = rnd.randint(1, 50)
        cat = f"K{rnd.randint(0, 19):02d}"
        rnd.randint(0, 2000)  # date
        f"{rnd.choice(WORDS)} {rnd.choice(WORDS)} №{rnd.randint(1000, 99999)}"
        sumG += amt * qty if amt > 500 else 0
        if cat == "K05":
            sumC += amt
            cnt += 1
            first = first or (r + 1)
    exp.append((sumG, sumC, first, cnt))

z = zipfile.ZipFile(a.out)
wb = z.read("xl/workbook.xml").decode("utf-8")
names = re.findall(r'<sheet [^>]*name="([^"]*)"', wb)
idx = names.index("Summary") + 1
xml = z.read(f"xl/worksheets/sheet{idx}.xml").decode("utf-8")
vals = {m.group(1): float(m.group(2)) for m in re.finditer(r'<c r="([B-E][0-9]+)"[^>]*>.*?<v>([^<]*)</v>', xml)}
bad = []
for s, (g, c, first, cnt) in enumerate(exp):
    row = s + 2
    got = [vals.get(f"{col}{row}") for col in "BCDE"]
    want = [g, c, first, cnt]
    for col, x, y in zip("BCDE", got, want):
        if x is None or y is None:
            ok = x == y
        elif col in "BC":
            ok = abs(x - y) <= 1e-9 * max(1.0, abs(y))
        else:
            ok = x == y
        if not ok:
            bad.append((f"{col}{row}", x, y))
print(json.dumps({"file": a.out, "rows": a.rows, "sheets": a.sheets, "checked": 4 * len(exp), "mismatches": bad[:10], "ok": not bad}))
sys.exit(1 if bad else 0)
