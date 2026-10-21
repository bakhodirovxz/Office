#!/usr/bin/env python
"""WP10: the per-direction table of docs/phase3/fidelity.md and the small evidence summary, from the run JSON.

  python -I docs/phase3/evidence/fidelity_table.py E:/office-target/p3/fidelity docs/phase3/evidence/formats.json \
      --md E:/office-target/p3/fidelity/table.md --summary docs/phase3/evidence/fidelity-summary.json

Reads <root>/directions/*.json and <root>/extra/directions/*.json (fidelity_extra.py). One row per `convert formats --json`
row, then the rows that are not in it (translit, pdf operations, edge cases, .ppt). Numbers are copied, never recomputed.
"""
import argparse, json, re
from pathlib import Path


def f(x, d=3):
    return "-" if x is None else ("%.*f" % (d, x) if isinstance(x, float) else str(x))


def load(root):
    out = {}
    for sub in ("directions", "extra/directions"):
        for p in sorted((root / sub).glob("*.json")):
            d = json.loads(p.read_text("utf-8")); d["file"] = "%s/%s" % (sub, p.name); out[d["id"]] = d
    return out


def ref_cell(c, e):
    v = (c.get("engines") or {}).get(e) or {}
    if v.get("status") != "ok":
        r = (v.get("reason") or "n/a")
        if "not applicable" in r or "equivalent" in r: return "n/a"
        for k, t in (("same format", "same format"), ("Markdown", "no md reader"), ("cannot write", "cannot write it"), ("hangs", "Word PDF reflow hangs"),
                     ("OOXML", "no such route"), ("same-kind", "no such route"), ("legacy_format", "refuses legacy"), ("per-page png", "no png export")):
            if k in r: return t
        return "unavail.: " + r[:40]
    m = v.get("metrics") or {}
    if "cell_equality" in m:
        return "csv"
    s = []
    if m.get("pages_cand") is not None and m.get("pages_ref") is not None: s.append("%s/%s pp" % (m["pages_ref"], m["pages_cand"]))
    if m.get("pix_mean") is not None: s.append("pix %s" % f(m["pix_mean"], 2))
    if m.get("word_sim") is not None: s.append("sim %s" % f(m["word_sim"], 2))
    return ", ".join(s) or "ok"


CSV = {}   # csv-recheck.json (csv_recheck.py): the cell equality with each file's own delimiter


def row(d, label):
    cells = {k: [] for k in ("fx", "st", "tm", "pg", "ts", "cy", "px", "lost", "w", "off", "lo", "x2", "sil")}
    for c in d["cases"]:
        cr = c.get("craft") or {}; m = cr.get("metrics") or {}
        if d["kind"] == "pdf2docx":   # section 2 has the details; here the native default, re-rendered by Word
            m = dict(m.get("visual") or {}, cyr_kept=m.get("cyr_kept"), losses=[]); m["word_sim"] = (cr.get("metrics") or {}).get("text_sim")
        cells["fx"].append(c["fixture"])
        cells["tm"].append("%s / %s" % (f(cr.get("sec"), 2), f(cr.get("peak_mb"))) if cr else "-")
        cells["st"].append("%s %s" % (cr.get("status", "error"), cr.get("code", "")) if cr else "error")
        if d["kind"] == "pdfop":
            cells["pg"].append("%s/%s" % (m.get("pages_expected"), m.get("pages_got")))
            cells["px"].append("hash-equal %s/%s" % (m.get("pages_hash_equal"), m.get("pages_expected")))
        elif d["kind"] == "edge":
            cells["lost"].append("; ".join("%s: %s %s, text %s" % (k, v["status"], v["code"], "yes" if v["output_has_text"] else "no") for k, v in m.items()))
        else:
            if m.get("pages_cand") is not None: cells["pg"].append("%s/%s" % (f(m.get("pages_ref")), m["pages_cand"]))
            if m.get("word_sim") is not None: cells["ts"].append("%s / %s" % (f(m["word_sim"], 2), f(m.get("chars_kept"), 2)))
            if "cell_equality" in m or d["kind"] == "csv":
                k = CSV.get("%s/%s" % (d["id"], re.sub(r"\W+", "_", c["fixture"])))
                cells["ts"].append("cells %s equal + %s locale-only / %s (vs %s)" % (k["cells_equal"], k["cells_differ_only_by_locale"], k["cells_office"], k["reference"]) if k else "-")
            if m.get("cyr_kept") is not None: cells["cy"].append(f(m["cyr_kept"], 2))
            if m.get("pix_mean") is not None: cells["px"].append("%s / %s" % (f(m["pix_mean"], 2), f(m["pix_max"], 2)))
            L = [l for l in m.get("losses", []) if l["feature"] not in ("text", "pagination", "cyrillic")]
            cells["lost"].append(", ".join("%s %s>%s" % (l["feature"], l["before"], l["after"]) for l in L) or "none")
        if m.get("office_reopen") is False: cells["lost"].append("Office reopen FAILED")
        cells["w"].append(",".join(sorted({w["code"] for w in cr.get("warnings", [])})) or "-")
        if cr.get("status") == "failed": cells["w"][-1] = (cr.get("message") or "")[:70]
        for k, e in (("off", "office"), ("lo", "soffice"), ("x2", "x2t")): cells[k].append(ref_cell(c, e))
        sil = (c.get("loss_check") or {}).get("silent") or []
        cells["sil"].append("not checked by run.py" if d["kind"] == "pdf2docx" else ", ".join("%s %s>%s" % (s["feature"], s["before"], s["after"]) for s in sil) or "0")
    j = lambda k: " · ".join(x for x in cells[k] if x) or "-"
    return "| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | `%s` |" % (
        label, j("fx"), j("st"), j("tm"), j("pg"), j("ts"), j("cy"), j("px"), j("lost"), j("w"), j("off"), j("lo"), j("x2"), j("sil"), d["file"])


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("root"); ap.add_argument("formats"); ap.add_argument("--md", required=True); ap.add_argument("--summary")
    a = ap.parse_args()
    D = load(Path(a.root))
    if (Path(a.root) / "csv-recheck.json").exists(): CSV.update(json.loads((Path(a.root) / "csv-recheck.json").read_text("utf-8")))
    F = json.loads(Path(a.formats).read_text("utf-8"))
    group = {**{x: "docs" for x in "docx doc odt rtf html md txt".split()}, **{x: "sheets" for x in "xlsx xls xlsb ods csv tsv".split()}, "pptx": "slides", "pdf": "pdf"}
    head = ("| Direction | Fixture(s) | Status, exit | Time s / peak MB | Pages ref/ours | Text sim / letters kept | Cyrillic kept | Pixel diff % mean / max "
            "| Structure lost (counts) | Warnings | Office | LibreOffice | x2t | Silent | JSON |\n|" + "---|" * 15)
    L = ["### In `convert formats --json` (%d rows)\n" % len(F["rows"]), head]
    used = set()
    for r in F["rows"]:
        did = "%s.%s2%s" % (group[r["from"]], r["from"], r["to"])
        if did in D:
            L.append(row(D[did], "%s -> %s" % (r["from"], r["to"]))); used.add(did)
        else:
            L.append("| %s -> %s | | NOT RUN | | | | | | | | | | | | |" % (r["from"], r["to"]))
    L += ["", "### Not rows of `formats` (options, PDF operations, failure cases, retired inputs)\n", head]
    for did in sorted(set(D) - used):
        L.append(row(D[did], did))
    Path(a.md).write_text("\n".join(L) + "\n", "utf-8")
    print("rows:", len(F["rows"]), "measured:", len(used), "other:", len(set(D) - used), "->", a.md)
    if a.summary:
        root = Path(a.root)
        S = {"generated_from": [str(root / "summary.json"), str(root / "extra/summary.json")], "formats_rows": len(F["rows"]), "rows_measured": len(used),
             "runs": {k: {x: json.loads((root / p).read_text("utf-8"))[x] for x in ("generated", "adapter", "engines", "loss_without_warning")}
                      for k, p in (("main", "summary.json"), ("extra", "extra/summary.json"))},
             "directions": {}}
        for did, d in sorted(D.items()):
            S["directions"][did] = {"file": d["file"], "cases": [{"fixture": c["fixture"], "status": (c.get("craft") or {}).get("status"), "code": (c.get("craft") or {}).get("code"),
                                    "warnings": sorted({w["code"] for w in (c.get("craft") or {}).get("warnings", [])}),
                                    "metrics": {k: v for k, v in ((c.get("craft") or {}).get("metrics") or {}).items() if k not in ("structure", "outputs", "pix_pages", "missing_words", "first_diffs")},
                                    "silent": (c.get("loss_check") or {}).get("silent"),
                                    "refs": {e: {k: v for k, v in ((c.get("engines") or {}).get(e) or {}).get("metrics", {}).items()
                                                 if k in ("pages_ref", "pages_cand", "word_sim", "chars_kept", "cyr_kept", "pix_mean", "pix_max", "office_reopen",
                                                          "paragraph_ratio", "tables_out", "col_width_err", "textbox_share", "text_sim", "visual", "known_words")}
                                             for e in ("office", "soffice", "x2t", "craft_sidecar", "pdf2docx_raw", "pdf2docx_merged") if (c.get("engines") or {}).get(e, {}).get("status") == "ok"}}
                                   for c in d["cases"]]}
        Path(a.summary).write_text(json.dumps(S, indent=0, ensure_ascii=False, default=str), "utf-8")
        print("summary", Path(a.summary).stat().st_size // 1024, "KB")


if __name__ == "__main__":
    main()
