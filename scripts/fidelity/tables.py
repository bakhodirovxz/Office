#!/usr/bin/env python
"""Table fidelity: our Docs rendering of Word-made tables against Word's own PDF (docs/phase4/tables.md).

  python -I scripts/fidelity/tables.py --in E:/office-target/p4-tables/fid/word --out E:/office-target/p4-tables/fid/run-before
         [--cli E:/office-target/p4-tables/docs/release/docs-cli.exe] [--office-reopen]

For every <name>.docx with a <name>.pdf beside it (Word's export, the truth; scripts/fidelity/gen_word_tables.ps1 or
hand-written fixtures exported by Word):
  render   : docs-cli convert x.docx x.pdf, both PDFs rasterised by pdf-cli at 96 dpi
  pages    : page counts
  pix      : per page pixel diff (fid/util.page_diff)
  rules    : table rules found in the raster (long dark runs). For every vertical (column) and horizontal (row) rule of
             Word's page, the distance to our nearest rule of the same direction, in pt; "missed" = farther than 3 pt
  roundtrip: docs-cli convert x.docx x.rt.docx; the table markup of the source and of the saved file is counted
             (fid/tablexml.py); a count that drops is a loss. --office-reopen: Word exports the saved file to PDF and it is
             compared with Word's PDF of the source (pixels and rules)
Output: <out>/tables.json (+ work files). Synthetic files only; prints a one-line-per-file table.

Golden check (no Word needed; the ci.sh fidelity-directions job):
  python -I scripts/fidelity/tables.py --golden ci/fidelity/tables-golden.json --out <dir> [--cli ...]
  writes the hand-written fixtures of fid/table_fixtures.py, renders them and fails (exit 1) when a rule of Word's page 1 has no
  rule of ours within the golden tolerance, or when saving loses table markup. A fixture marked "known_gap" is reported, not failed.
  --record <json>: Word renders the same fixtures and their rule positions become the golden file (needs Word on this PC).
"""
import argparse, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from fid.util import CLI, rasterise, page_diff, run   # noqa: E402
from fid import tablexml, table_fixtures                # noqa: E402

PT = 72 / 96          # pt per pixel at 96 dpi
TOL = 3.0             # pt: a rule farther than this from every rule of ours is "missed"


def rules(png, min_run=20):
    """-> (xs, ys): positions in px of vertical and horizontal dark runs at least `min_run` px long (table rules)."""
    import numpy as np
    from PIL import Image
    a = np.asarray(Image.open(png).convert("L")) < 200

    def runs(m):   # rows of m: positions (row index) having a run >= min_run
        out = []
        for i, row in enumerate(m):
            if row.sum() < min_run:
                continue
            d = np.diff(np.concatenate(([0], row.astype(np.int8), [0])))
            st, en = np.where(d == 1)[0], np.where(d == -1)[0]
            if ((en - st) >= min_run).any():
                out.append(i)
        return cluster(out)
    return runs(a.T), runs(a)


def cluster(pos):
    """Adjacent pixel lines (a thick or anti-aliased rule) -> their centre."""
    out, grp = [], []
    for p in pos:
        if grp and p - grp[-1] > 1:
            out.append(sum(grp) / len(grp)); grp = []
        grp.append(p)
    if grp:
        out.append(sum(grp) / len(grp))
    return out


def rule_err(ref, cand):
    """mean / max distance in pt from each reference rule to the nearest candidate rule, and how many are missed."""
    if not ref:
        return {"n_ref": 0, "n_cand": len(cand), "mean_pt": 0.0, "max_pt": 0.0, "missed": 0}
    d = [min((abs(r - c) for c in cand), default=999) * PT for r in ref]
    d = [min(x, 50.0) for x in d]
    return {"n_ref": len(ref), "n_cand": len(cand), "mean_pt": round(sum(d) / len(d), 2), "max_pt": round(max(d), 2),
            "missed": sum(1 for x in d if x > TOL)}


def compare(ref_pdf, cand_pdf, work, tag):
    rn, rp = rasterise(ref_pdf, work, "ref")
    cn, cp = rasterise(cand_pdf, work, tag)
    pages, vx, hy = [], {"n_ref": 0, "missed": 0, "sum": 0.0, "max": 0.0}, {"n_ref": 0, "missed": 0, "sum": 0.0, "max": 0.0}
    for i, (r, c) in enumerate(zip(rp, cp)):
        if not (r and c):
            continue
        pix = page_diff(r, c)[0]
        (rx, ry), (cx, cy) = rules(r), rules(c)
        ev, eh = rule_err(rx, cx), rule_err(ry, cy)
        pages.append({"page": i + 1, "pix": pix, "vertical": ev, "horizontal": eh})
        for acc, e in ((vx, ev), (hy, eh)):
            acc["n_ref"] += e["n_ref"]; acc["missed"] += e["missed"]; acc["sum"] += e["mean_pt"] * e["n_ref"]; acc["max"] = max(acc["max"], e["max_pt"])
    tot = lambda a: {"n_ref": a["n_ref"], "missed": a["missed"], "mean_pt": round(a["sum"] / a["n_ref"], 2) if a["n_ref"] else 0.0, "max_pt": round(a["max"], 2)}
    pix = [p["pix"] for p in pages]
    return {"pages_ref": rn, "pages_cand": cn, "pix_mean": round(sum(pix) / len(pix), 3) if pix else None,
            "vertical": tot(vx), "horizontal": tot(hy), "per_page": pages}


def office_export(pairs, work):
    lst, res = work / "_word_list.json", work / "_word_result.json"
    lst.write_text(json.dumps([{"src": str(s), "out": str(o)} for s, o in pairs]), encoding="utf-8")
    subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "office_com.ps1"), "-Mode", "export",
                    "-App", "word", "-List", str(lst), "-Result", str(res)], timeout=900)


def page1_rules_pt(pdf, work, tag):
    n, pngs = rasterise(pdf, work, tag)
    if not pngs or not pngs[0]:
        return n, [], []
    xs, ys = rules(pngs[0])
    return n, [round(x * PT, 1) for x in xs], [round(y * PT, 1) for y in ys]


def record(golden, out, cli):
    fx = table_fixtures.write(out / "fixtures")
    office_export([(f, f.with_suffix(".pdf")) for f in fx], out)
    old = json.loads(Path(golden).read_text(encoding="utf-8")) if Path(golden).exists() else {}
    g = {"_about": "Rule positions (pt from the page's top-left, page 1) of Word 16's PDF of each fixture of "
                   "scripts/fidelity/fid/table_fixtures.py, recorded by tables.py --record. Numbers only; no Word output is kept.",
         "tol_pt": old.get("tol_pt", 1.5)}
    for f in fx:
        n, xs, ys = page1_rules_pt(f.with_suffix(".pdf"), out / f.stem, "word")
        g[f.stem] = {"pages": n, "vertical_pt": xs, "horizontal_pt": ys}
        for keep in ("known_gap", "tol_pt", "note"):   # hand-kept fields survive a re-record
            if keep in old.get(f.stem, {}):
                g[f.stem][keep] = old[f.stem][keep]
    txt = re.sub(r"\[[^\[\]{}]*\]", lambda m: " ".join(m.group(0).split()), json.dumps(g, indent=1, ensure_ascii=False))
    Path(golden).write_text(txt + chr(10), encoding="utf-8")
    print("recorded", len(fx), "fixtures into", golden)


def check(golden, out, cli):
    g = json.loads(Path(golden).read_text(encoding="utf-8"))
    bad = 0
    for f in table_fixtures.write(out / "fixtures"):
        exp = g.get(f.stem)
        if exp is None:
            print("FAIL %s: no golden entry (run --record)" % f.stem); bad += 1; continue
        tol = exp.get("tol_pt", g["tol_pt"])
        w = out / f.stem; w.mkdir(exist_ok=True)
        cand, rt = w / "craft.pdf", w / "roundtrip.docx"
        run([cli, "convert", f, cand], 300); run([cli, "convert", f, rt], 300)
        n, xs, ys = page1_rules_pt(cand, w, "craft") if cand.exists() else (0, [], [])
        miss = [("x", r) for r in exp["vertical_pt"] if min((abs(r - c) for c in xs), default=99) > tol]
        miss += [("y", r) for r in exp["horizontal_pt"] if min((abs(r - c) for c in ys), default=99) > tol]
        lost = tablexml.losses(tablexml.counts(f), tablexml.counts(rt)) if rt.exists() else {"save failed": [1, 0]}
        gap = exp.get("known_gap")
        state = "ok" if not miss and not lost and n == exp["pages"] else ("known gap" if gap and not lost else "FAIL")
        print("%-9s %-24s pages %s/%s, rules missed %d of %d, markup lost %s%s" % (state, f.stem, exp["pages"], n, len(miss),
              len(exp["vertical_pt"]) + len(exp["horizontal_pt"]), lost or "-", " (%s)" % gap if gap and state != "ok" else ""))
        if miss and state == "FAIL":
            print("          missed (axis, pt):", miss[:8])
        bad += state == "FAIL"
    return bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="inp")
    ap.add_argument("--out", required=True)
    ap.add_argument("--cli", default=str(CLI["docs"]))
    ap.add_argument("--office-reopen", action="store_true")
    ap.add_argument("--golden")
    ap.add_argument("--record")
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    if a.record:
        return record(a.record, out, a.cli)
    if a.golden:
        sys.exit(1 if check(a.golden, out, a.cli) else 0)
    rows = {}
    reopen = []
    for src in sorted(f for f in Path(a.inp).iterdir() if f.suffix.lower() in (".docx", ".doc", ".rtf")):
        ref = src.with_suffix(".pdf")
        if not ref.exists():
            continue
        w = out / src.stem; w.mkdir(exist_ok=True)
        cand, rt = w / "craft.pdf", w / "roundtrip.docx"
        r1 = run([a.cli, "convert", src, cand], 300)
        r2 = run([a.cli, "convert", src, rt], 300)
        row = {"convert_rc": r1[0], "save_rc": r2[0]}
        if cand.exists():
            row.update(compare(ref, cand, w, "craft"))
        if rt.exists() and src.suffix.lower() == ".docx":
            before, after = tablexml.counts(src), tablexml.counts(rt)
            row["markup"] = {"before": before, "after": after, "lost": tablexml.losses(before, after)}
            reopen.append((rt, w / "roundtrip-word.pdf"))
        rows[src.stem] = row
    if a.office_reopen and reopen:
        office_export(reopen, out)
        for rt, pdf in reopen:
            if pdf.exists():
                m = compare(Path(a.inp) / (rt.parent.name + ".pdf"), pdf, rt.parent, "reopen")
                rows[rt.parent.name]["reopen"] = {k: v for k, v in m.items() if k != "per_page"}
    (out / "tables.json").write_text(json.dumps(rows, indent=1, ensure_ascii=False), encoding="utf-8")
    print("%-28s %5s %6s %16s %16s %s" % ("file", "pages", "pix%", "vert miss/n mean", "horiz miss/n mean", "markup lost"))
    for k, r in rows.items():
        v, h = r.get("vertical", {}), r.get("horizontal", {})
        print("%-28s %2s/%-2s %6s %5s/%-3s %6s %5s/%-3s %6s %s" % (k, r.get("pages_ref"), r.get("pages_cand"), r.get("pix_mean"),
              v.get("missed"), v.get("n_ref"), v.get("mean_pt"), h.get("missed"), h.get("n_ref"), h.get("mean_pt"),
              ",".join(r.get("markup", {}).get("lost", {})) or "-"))


if __name__ == "__main__":
    main()
