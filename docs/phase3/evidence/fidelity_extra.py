#!/usr/bin/env python
"""WP10: run scripts/fidelity/run.py on the `convert formats --json` rows that fid/directions.py does not list.

  python -I docs/phase3/evidence/fidelity_extra.py --out E:/office-target/p3/fidelity/extra --fixtures E:/office-target/p3/fidelity/fixtures-extra

Same harness, same metrics, same Office/LibreOffice/x2t references; only the direction table is swapped. One fixture per
source format. The fixtures dir must hold the main fixtures plus staff.csv / staff.tsv (Excel's CSV of hrm_staff.xlsx and
its tab-separated copy). shortcut: kept out of fid/directions.py so the CI run time stays as it is; merge it there if CI
should cover all 80 rows.
"""
import runpy, sys
from pathlib import Path

H = Path(__file__).resolve().parents[3] / "scripts" / "fidelity"
sys.path.insert(0, str(H))
import fid.directions as FD  # noqa: E402

D = FD.D
SRC = {"doc": "hrm_order.doc", "odt": "order.odt", "rtf": "order.rtf", "html": "order.html", "md": "order.md", "txt": "order.txt"}
MISSING = {"doc": "odt rtf html md txt png", "odt": "odt rtf html md txt png", "rtf": "odt rtf html md txt png",
           "html": "odt rtf html md txt pdf png", "md": "odt rtf html md txt png", "txt": "odt rtf html md txt pdf png"}
EXTRA = [D("docs.docx2docx", "docs", ["hrm_order.docx"], "docx")]
EXTRA += [D("docs.%s2%s" % (s, t), "docs", [SRC[s]], t) for s, ts in MISSING.items() for t in ts.split()]
EXTRA += [D("sheets.xlsx2xlsx", "sheets", ["hrm_staff.xlsx"], "xlsx"), D("sheets.xls2csv", "sheets", ["hrm_staff.xls"], "csv")]
EXTRA += [D("sheets.%s2%s" % (s, t), "sheets", ["hrm_staff." + s], t) for s in ("xlsb", "ods") for t in ("pdf", "csv")]
EXTRA += [D("sheets.%s2%s" % (s, t), "sheets", ["staff." + s], t) for s in ("csv", "tsv") for t in ("pdf", "xlsx", "csv")]
EXTRA += [D("slides.pptx2pptx", "slides", ["hrm_deck.pptx"], "pptx")]
assert len(EXTRA) == 52, len(EXTRA)

import fid.refs as FR  # noqa: E402
_batch = FR.office_batch


def office_batch(app, jobs, work, tag, per_item=90):
    """Word hung headless on one html -> rtf SaveAs2 (2026-10-21) and the batch timeout then loses every job of the batch.
    shortcut: html sources get a Word instance each, so a hang costs only that job; drop this when office_ref.ps1 saves per item."""
    solo = [j for j in jobs if str(j["src"]).lower().endswith((".html", ".htm"))]
    res = _batch(app, [j for j in jobs if j not in solo], work, tag, per_item)
    for i, j in enumerate(solo):
        res.update(_batch(app, [j], work, "%s_solo%d" % (tag, i), per_item))
    return res


FR.office_batch = office_batch
FD.DIRECTIONS[:] = EXTRA
sys.argv = [str(H / "run.py")] + sys.argv[1:]
runpy.run_path(str(H / "run.py"), run_name="__main__")
