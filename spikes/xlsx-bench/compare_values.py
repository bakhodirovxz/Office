"""compare_values.py A.xlsx B.xlsx [--formulas]

Compares the cached cell values (<v> text and t= type) of every worksheet in two xlsx files
written by gridcraft. gridcraft writes numbers with Rust's shortest round-trip formatting, so equal
text means bit-identical f64 values. With --formulas, also compares the formula text of cells
whose <f> carries text in both files (shared-formula members carry none and are skipped; use a
file re-saved by the unpatched build to compare every formula). Shared-string cells are compared
by their text, not their index.

Prints one JSON line per sheet and a summary line; exit code 1 on any difference.
"""
import json
import re
import sys
import zipfile

CELL = re.compile(r'<c r="([A-Z]+[0-9]+)"([^>]*?)(?:/>|>(.*?)</c>)', re.S)
TYPE = re.compile(r'\bt="([^"]*)"')
VAL = re.compile(r"<v>(.*?)</v>", re.S)
FORM = re.compile(r"<f(?:\s[^>]*)?>(.*?)</f>", re.S)
SHEET = re.compile(r'<sheet [^>]*name="([^"]*)"[^>]*r:id="([^"]*)"')
REL = re.compile(r'<Relationship [^>]*Id="([^"]*)"[^>]*Target="([^"]*)"')
REL2 = re.compile(r'<Relationship [^>]*Target="([^"]*)"[^>]*Id="([^"]*)"')


SI = re.compile(r"<si>(.*?)</si>", re.S)
T = re.compile(r"<t(?:\s[^>]*)?>(.*?)</t>", re.S)


def strings(z):
    try:
        x = z.read("xl/sharedStrings.xml").decode("utf-8")
    except KeyError:
        return []
    return ["".join(T.findall(si)) for si in SI.findall(x)]


def sheets(path, sst=None):
    z = zipfile.ZipFile(path)
    if sst is not None:
        sst.extend(strings(z))
    wb = z.read("xl/workbook.xml").decode("utf-8")
    rels = z.read("xl/_rels/workbook.xml.rels").decode("utf-8")
    target = {i: t for i, t in REL.findall(rels)}
    target.update({i: t for t, i in REL2.findall(rels)})
    out = {}
    for name, rid in SHEET.findall(wb):
        t = target[rid].lstrip("/")
        part = t if t.startswith("xl/") else "xl/" + t
        out[name] = z.read(part).decode("utf-8")
    return out


def cells(xml, sst=None):
    vals, forms = {}, {}
    for ref, attrs, body in CELL.findall(xml):
        body = body or ""
        t = TYPE.search(attrs)
        v = VAL.search(body)
        f = FORM.search(body)
        kind, val = (t.group(1) if t else "n"), (v.group(1) if v else None)
        if kind == "s" and sst is not None and val is not None and val.isdigit() and int(val) < len(sst):
            val = sst[int(val)]
        vals[ref] = (kind, val, f is not None or "<f " in body)
        if f and f.group(1):
            forms[ref] = f.group(1)
    return vals, forms


def main():
    a, b = sys.argv[1], sys.argv[2]
    with_formulas = "--formulas" in sys.argv
    ta, tb = [], []
    sa, sb = sheets(a, ta), sheets(b, tb)
    bad = 0
    total = {"cells": 0, "formula_cells": 0, "value_diffs": 0, "formula_diffs": 0, "formulas_compared": 0}
    if list(sa) != list(sb):
        print(json.dumps({"error": "sheet lists differ", "a": list(sa), "b": list(sb)}))
        sys.exit(1)
    for name in sa:
        va, fa = cells(sa[name], ta)
        vb, fb = cells(sb[name], tb)
        keys = set(va) | set(vb)
        diffs = []
        nform = 0
        for k in keys:
            x, y = va.get(k), vb.get(k)
            if x and x[2]:
                nform += 1
            if (x and (x[0], x[1])) != (y and (y[0], y[1])):
                diffs.append((k, x and x[:2], y and y[:2]))
        fdiffs = []
        ncmp = 0
        if with_formulas:
            for k in set(fa) & set(fb):
                ncmp += 1
                if fa[k] != fb[k]:
                    fdiffs.append((k, fa[k], fb[k]))
        total["cells"] += len(keys)
        total["formula_cells"] += nform
        total["value_diffs"] += len(diffs)
        total["formula_diffs"] += len(fdiffs)
        total["formulas_compared"] += ncmp
        rec = {"sheet": name, "cells": len(keys), "formula_cells": nform, "value_diffs": len(diffs)}
        if with_formulas:
            rec.update(formulas_compared=ncmp, formula_diffs=len(fdiffs))
        if diffs:
            rec["first_value_diffs"] = sorted(diffs)[:5]
        if fdiffs:
            rec["first_formula_diffs"] = sorted(fdiffs)[:5]
        bad += len(diffs) + len(fdiffs)
        print(json.dumps(rec, ensure_ascii=False))
    print(json.dumps({"summary": total, "a": a, "b": b, "identical": bad == 0}))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
