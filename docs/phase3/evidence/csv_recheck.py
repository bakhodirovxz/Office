"""Re-measure csv cell equality with each file's own delimiter (the harness reads Excel's ';' CSV with ',').
usage: csv_recheck.py <out json> <fixtures dir> <work dir>...
When Office made no CSV (csv -> csv is "same format"; Office COM cannot open .tsv) the source fixture is the reference."""
import csv, json, sys
from pathlib import Path


def load(p):
    raw = Path(p).read_text("utf-8-sig")
    first = raw.splitlines()[0] if raw else ""
    delim = max(";,\t", key=first.count) if first else ","
    rows = list(csv.reader(raw.splitlines(), delimiter=delim))
    return delim, rows


def norm(rows):
    # Excel pads every row to the used width; drop trailing empty cells so padding is not counted as content
    out = []
    for r in rows:
        while r and r[-1] == "":
            r = r[:-1]
        out.append(r)
    return out


res = {}
FX = Path(sys.argv[2])
for w in sys.argv[3:]:
    for case in sorted(Path(w).glob("sheets.*2csv/*")):
        o = list((case / "office").glob("*.csv")); c = list((case / "craft").glob("*.csv")); ref = "office"
        if not o:
            o = [FX / ("%s.%s" % tuple(case.name.rsplit("_", 1)))]; ref = "source fixture"
            o = [x for x in o if x.exists()]
        if not o or not c:
            res["%s/%s" % (case.parent.name, case.name)] = {"note": "office or craft csv missing"}; continue
        od, a = load(o[0]); cd, b = load(c[0]); a, b = norm(a), norm(b)
        cells = sum(len(r) for r in a); eq = 0; diffs = []; loc = 0
        lf = lambda s: s.replace(chr(0xa0), "").replace(" ", "").replace(",", ".").replace("/", ".")   # locale-neutral form
        for i, r in enumerate(a):
            for j, v in enumerate(r):
                x = b[i][j] if i < len(b) and j < len(b[i]) else None
                if v == x: eq += 1
                elif x is not None and lf(v) == lf(x): loc += 1
                elif len(diffs) < 6: diffs.append({"cell": [i + 1, j + 1], "office": v[:40], "craft": (x or "")[:40]})
        res["%s/%s" % (case.parent.name, case.name)] = {"reference": ref, "office_delimiter": od, "craft_delimiter": cd, "rows_office": len(a), "rows_craft": len(b),
                                                      "cells_office": cells, "cells_equal": eq, "cells_differ_only_by_locale": loc, "cells_other_diff": cells - eq - loc, "cell_equality": round(eq / cells, 4) if cells else None,
                                                      "first_diffs": diffs}
Path(sys.argv[1]).write_text(json.dumps(res, indent=1, ensure_ascii=False), "utf-8")
for k, v in res.items():
    print(k, v.get("office_delimiter"), v.get("craft_delimiter"), v.get("cells_equal"), "/", v.get("cells_office"), v.get("cell_equality"), "locale-only", v.get("cells_differ_only_by_locale"), "other", v.get("cells_other_diff"))
