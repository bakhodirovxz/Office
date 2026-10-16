#!/usr/bin/env python
"""Render summary.json (synthetic runs only) into docs/phase0/fidelity-baseline.md and copy worst images.

  python -I spikes/fidelity/report.py --summary E:/office-target/fidelity/summary.json \
      --md docs/phase0/fidelity-baseline.md --images docs/phase0/fidelity --notes spikes/fidelity/notes.md
"""
import argparse, collections, json, shutil
from pathlib import Path


def f(x, d=3):
    return "-" if x is None else ("%.*f" % (d, x) if isinstance(x, float) else str(x))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--summary", required=True); ap.add_argument("--md", required=True)
    ap.add_argument("--images", required=True); ap.add_argument("--notes")
    a = ap.parse_args()
    S = json.loads(Path(a.summary).read_text(encoding="utf-8"))
    if not S["synthetic"]:
        raise SystemExit("refusing: corpus-derived summaries are never rendered into the repo")
    base = Path(a.summary).parent
    img_dir = Path(a.images); img_dir.mkdir(parents=True, exist_ok=True)
    L = []
    w = L.append
    w("# Fidelity baseline (Phase 0, task 5 prep)\n")
    w("> **SYNTHETIC set, produced by Microsoft Office 16.** The owner's anonymised `corpus/` run is **pending**.")
    w("> All numbers below come from `spikes/fidelity/run.py` runs on %s; none are about real HRM documents.\n" % S["generated"])
    w("Corpus run command (artefacts go under `E:/` only; names are replaced by ids):\n")
    w("```\npython -I spikes/fidelity/run.py --input corpus/ --out E:/office-target/fidelity-corpus\n```\n")
    w("## Method\n")
    w("- Test set: `spikes/fidelity/gen_office.ps1` drives Word/Excel/PowerPoint through COM and saves each document as "
      "modern (`docx/xlsx/pptx`) and legacy (`doc/xls/ppt`) files (10 files, synthetic text incl. Uzbek Cyrillic/Latin with U+02BB/U+02BC, Russian).")
    w("- Reference (\"truth\"): the same Office 16 opens the file read-only and exports PDF (`ExportAsFixedFormat` / `SaveAs 32`).")
    w("- Candidate, per file: Craft CLI `direct` = open -> PDF; `roundtrip` = open -> save native format -> reopen -> PDF. "
      "Excel PDFs use `file.exportPdf` with `sheets:all`; deckcraft saves with `run --save`.")
    w("- All PDFs (reference and candidates) are rasterised by the same `pdfcraft-cli render --dpi %d`; pixel diff = "
      "grayscale, Gaussian blur r=1, |a-b|>48 counted as different; pages compared by index for pages present in both "
      "(a page-count mismatch therefore inflates later pages; read it together with the pagination loss).\n" % S["dpi"])
    w("- Text similarity: `pdfcraft-cli text` on both PDFs, NFKC + casefold + whitespace-normalised, U+02BB/U+02BC/U+2019 folded to `'`; "
      "ratio = difflib SequenceMatcher (character level). Missing words = multiset difference of word tokens.")
    w("- Office reopen: the Craft-saved file is opened read-only in Office via COM with alerts suppressed; failure to open = not ok. "
      "A silent auto-repair by Excel is not detectable this way (limitation).")
    w("- Structural counts (before = original in Office, after = Craft-saved file in Office): Word paragraphs/tables/images/bookmarks/fields/"
      "header+footer fields/comments/revisions/footnotes/sections; Excel sheets/formulas/charts/conditional formats/names; PowerPoint slides/shapes/tables/pictures/charts/notes.")
    w("- Hooks for LibreOffice and ONLYOFFICE x2t references exist (`libreoffice_reference`, `x2t_reference` in run.py) but were not run (Docker down).\n")
    w("## Versions\n")
    w("| Component | Version |\n|---|---|")
    for k, v in S["versions"].items():
        w("| %s | %s |" % (k, v))
    w("")
    for b in S["builds"]:
        w("## Per-file results (build: %s)\n" % b)
        w("| File | Direct open->PDF | Save native | Office reopen | Pages ref / direct / rt | Text sim direct / rt | Pixel diff % mean / max (rt) | Pixel diff % mean / max (direct) | Structural deltas (rt) |")
        w("|---|---|---|---|---|---|---|---|---|")
        for r in S["results"]:
            d, t = r["builds"][b]["direct"], r["builds"][b]["roundtrip"]
            st = "; ".join("%s %s->%s" % (k, v["before"], v["after"]) for k, v in (t.get("struct") or {}).items()) or "none"
            w("| %s | %s | %s | %s | %s / %s / %s | %s / %s | %s / %s | %s / %s | %s |" % (
                r["file"], "ok" if d["convert_ok"] else "FAIL", "ok" if t.get("save_ok") else "FAIL",
                "ok" if t.get("office_reopen") else ("FAIL" if t.get("save_ok") else "-"),
                r["pages_ref"], f(d.get("pages_cand")), f(t.get("pages_cand")),
                f((d.get("text") or {}).get("char_sim")), f((t.get("text") or {}).get("char_sim")),
                f(t.get("pix_mean"), 2), f(t.get("pix_max"), 2), f(d.get("pix_mean"), 2), f(d.get("pix_max"), 2), st))
        w("")
        w("### Loss categories (build: %s)\n" % b)
        cat = collections.defaultdict(list)
        for r in S["results"]:
            for kind in ("direct", "roundtrip"):
                for x in r["builds"][b][kind]["losses"]:
                    cat[x["category"]].append((r["file"], kind, x["detail"]))
        w("| Category | Observations (file x variant) | Files | Examples |\n|---|---|---|---|")
        for c, items in sorted(cat.items(), key=lambda kv: -len(kv[1])):
            files = sorted({i[0] for i in items})
            ex = "; ".join("%s/%s: %s" % (i[0], i[1], i[2][:90]) for i in items[:3])
            w("| %s | %d | %d | %s |" % (c, len(items), len(files), ex.replace("|", "/")))
        w("")
        w("### Per-app headline (build: %s)\n" % b)
        w("| App | files | direct open ok | save ok | Office reopen ok | mean text sim (rt) | mean pixel diff % (rt) | page-count match (rt) |\n|---|---|---|---|---|---|---|---|")
        for app in ("word", "excel", "ppt"):
            rs = [r for r in S["results"] if r["app"] == app]
            if not rs:
                continue
            n = len(rs)
            do = sum(r["builds"][b]["direct"]["convert_ok"] for r in rs)
            so = sum(bool(r["builds"][b]["roundtrip"].get("save_ok")) for r in rs)
            ro = sum(bool(r["builds"][b]["roundtrip"].get("office_reopen")) for r in rs)
            sims = [r["builds"][b]["roundtrip"]["text"]["char_sim"] for r in rs if r["builds"][b]["roundtrip"].get("text")]
            pix = [r["builds"][b]["roundtrip"]["pix_mean"] for r in rs if r["builds"][b]["roundtrip"].get("pix_mean") is not None]
            pm = sum(1 for r in rs if r["builds"][b]["roundtrip"].get("pages_cand") == r["pages_ref"])
            w("| %s | %d | %d/%d | %d/%d | %d/%d | %s | %s | %d/%d |" % (app, n, do, n, so, n, ro, n,
              f(sum(sims) / len(sims)) if sims else "-", f(sum(pix) / len(pix), 2) if pix else "-", pm, n))
        w("")
    w("## Worst pages (side by side: Office reference | Craft | red = differing pixels)\n")
    for x in S["worst"]:
        src = base / "diffs" / x["file"]
        if src.exists():
            shutil.copy(src, img_dir / x["file"])
            w("- `%s` (%s, %s, page %d, %.1f%% pixels differ)\n\n  ![%s](fidelity/%s)\n" % (x["id"], x["build"], x["kind"], x["page"], x["pct"], x["file"], x["file"]))
    if a.notes and Path(a.notes).exists():
        w(Path(a.notes).read_text(encoding="utf-8"))
    Path(a.md).write_text("\n".join(L) + "\n", encoding="utf-8")
    print("wrote", a.md, sum(p.stat().st_size for p in img_dir.glob("*")) // 1024, "KB of images")


if __name__ == "__main__":
    main()
