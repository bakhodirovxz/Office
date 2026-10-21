#!/usr/bin/env python
"""Build the synthetic fixture set for the direction harness (nothing here reads corpus/; binaries are never committed).

  python -I scripts/fidelity/gen_fixtures.py --out E:/office-target/p3/fidelity/fixtures

1. base set: copied from the Phase 0 synthetic dir (gen_office.ps1 builds it with Office 16 if it is missing)
2. gen_word_excel.ps1: odt rtf html txt md dot pdf (Word), xlsb ods and staff.csv (Excel; staff.tsv is made from it here)
3. gen_pdf2docx.ps1: the PLAN 3.4 PDF -> DOCX fixtures (Word-made PDFs + truth files)
4. the Phase 2 translit fixtures (copied from domains/*/tests), plus PDF edge cases (scan, 0 bytes, truncated)
5. MANIFEST.json (name -> size, sha256)
"""
import argparse, hashlib, json, os, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
SYN = Path("E:/office-target/fidelity/synthetic")
BASE = ["hrm_order.docx", "hrm_order.doc", "hrm_letter.docx", "hrm_letter.doc", "hrm_staff.xlsx", "hrm_staff.xls",
        "hrm_timesheet.xlsx", "hrm_timesheet.xls", "hrm_deck.pptx", "hrm_deck.ppt", "sample.png"]
TRANSLIT = ["domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx",
            "domains/docs/crates/docs-translit/tests/fixtures/uz108-cyrl.docx",
            "domains/sheets/tests/translit-fixtures/xlsx/02-table-rich.xlsx",
            "domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx",
            "domains/slides/tests/translit-fixtures/pptx/02-languages.pptx"]


def ps(script, *args, timeout=900):
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / script), *map(str, args)]
    p = subprocess.run(cmd, capture_output=True, timeout=timeout)
    sys.stdout.write(p.stdout.decode("utf-8", "replace"))
    if p.returncode:
        raise SystemExit("%s failed rc=%s\n%s" % (script, p.returncode, p.stderr.decode("utf-8", "replace")[-1500:]))


def edge_pdfs(out):
    from PIL import Image
    Image.open(out / "sample.png").convert("RGB").resize((1240, 1754)).save(out / "scan.pdf")   # image-only: no text layer
    (out / "empty.pdf").write_bytes(b"")
    data = (out / "order.pdf").read_bytes()
    (out / "truncated.pdf").write_bytes(data[: len(data) // 2])


def tsv_from_csv(src, dst):
    """The tab-separated copy of Excel's CSV (Excel COM writes UTF-16 for tabs; the converter's input is UTF-8)."""
    import csv
    with open(src, encoding="utf-8-sig", newline="") as f:
        head = f.readline()
        f.seek(0)
        rows = list(csv.reader(f, delimiter=max(";,", key=head.count)))
    with open(dst, "w", encoding="utf-8", newline="") as f:
        csv.writer(f, delimiter="\t", lineterminator="\r\n").writerows(rows)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()
    out = Path(a.out)
    if not str(out).upper().startswith("E:"):
        sys.exit("--out must be on E:")
    if a.force and out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True, exist_ok=True)
    if not all((SYN / f).exists() for f in BASE):
        ps("gen_office.ps1", "-Out", SYN)
    for f in BASE:
        shutil.copy2(SYN / f, out / f)
    if not (out / "order.odt").exists() or not (out / "hrm_staff.ods").exists() or not (out / "staff.csv").exists():
        ps("gen_word_excel.ps1", out)
    if not (out / "staff.tsv").exists():
        tsv_from_csv(out / "staff.csv", out / "staff.tsv")
    if not (out / "p2d_widths.pdf").exists():
        ps("gen_pdf2docx.ps1", out, "-Png", out / "sample.png")
    for t in TRANSLIT:
        shutil.copy2(REPO / t, out / Path(t).name)
    edge_pdfs(out)
    man = {p.name: {"bytes": p.stat().st_size, "sha256": hashlib.sha256(p.read_bytes()).hexdigest()[:16]}
           for p in sorted(out.iterdir()) if p.is_file() and p.name != "MANIFEST.json"}
    (out / "MANIFEST.json").write_text(json.dumps(man, indent=1), encoding="utf-8")
    print("fixtures: %d files in %s" % (len(man), out))


if __name__ == "__main__":
    main()
