"""The direction table (PLAN section 1). One entry = one row of the fidelity report. `cases` are fixture file names.
kind: how the output is judged -> pdf | png | edit (docx odt rtf xlsx pptx) | text (html md txt) | csv | pdfop | pdftext | pdf2docx | translit | edge"""

def D(id, group, cases, to, kind=None, **kw):
    kind = kind or {"pdf": "pdf", "png": "png", "html": "text", "md": "text", "txt": "text", "csv": "csv"}.get(to, "edit")
    return dict(id=id, group=group, cases=cases, to=to, kind=kind, **kw)

ORDER = ["hrm_order.docx", "hrm_letter.docx"]
DIRECTIONS = [
    # ---- docs
    *[D("docs.docx2" + e, "docs", ORDER, e) for e in ("odt", "rtf", "html", "md", "txt", "pdf", "png")],
    *[D("docs.%s2docx" % e, "docs", ["order." + e], "docx") for e in ("odt", "rtf", "html", "md", "txt")],
    D("docs.doc2docx", "docs", ["hrm_order.doc", "hrm_letter.doc"], "docx"),
    D("docs.dot2docx", "docs", ["order.dot"], "docx"),
    *[D("docs.%s2pdf" % e, "docs", ["order." + e], "pdf") for e in ("odt", "rtf", "md")],
    D("docs.doc2pdf", "docs", ["hrm_order.doc"], "pdf"),
    D("docs.translit.docx", "docs", ["uz108-latn.docx", "p2d_latn.docx"], "docx", "translit", translit="cyrl"),
    # ---- sheets
    D("sheets.xls2xlsx", "sheets", ["hrm_staff.xls", "hrm_timesheet.xls"], "xlsx"),
    D("sheets.xlsb2xlsx", "sheets", ["hrm_staff.xlsb", "hrm_timesheet.xlsb"], "xlsx"),
    D("sheets.ods2xlsx", "sheets", ["hrm_staff.ods", "hrm_timesheet.ods"], "xlsx"),
    D("sheets.xlsx2pdf", "sheets", ["hrm_staff.xlsx", "hrm_timesheet.xlsx", "02-table-rich.xlsx"], "pdf"),
    D("sheets.xls2pdf", "sheets", ["hrm_staff.xls"], "pdf"),
    D("sheets.xlsx2csv", "sheets", ["hrm_staff.xlsx", "hrm_timesheet.xlsx"], "csv"),
    D("sheets.translit.xlsx", "sheets", ["hrm_staff.xlsx", "02-table-rich.xlsx"], "xlsx", "translit", translit="cyrl"),
    # ---- slides
    D("slides.pptx2pdf", "slides", ["hrm_deck.pptx", "01-shapes-notes.pptx", "02-languages.pptx"], "pdf"),
    D("slides.pptx2png", "slides", ["hrm_deck.pptx"], "png"),
    D("slides.ppt2pptx", "slides", ["hrm_deck.ppt"], "pptx"),
    D("slides.ppt2pdf", "slides", ["hrm_deck.ppt"], "pdf"),
    D("slides.translit.pptx", "slides", ["hrm_deck.pptx", "02-languages.pptx"], "pptx", "translit", translit="cyrl"),
    # ---- pdf
    D("pdf.combine", "pdf", ["p2d_latn.pdf+p2d_cyrl.pdf+order.pdf"], "pdf", "pdfop", op="combine"),
    D("pdf.split", "pdf", ["order.pdf"], "pdf", "pdfop", op="split", arg=["--every", "2"]),
    D("pdf.extract", "pdf", ["order.pdf"], "pdf", "pdfop", op="extract", arg="1,3-4"),
    D("pdf.pdf2png", "pdf", ["order.pdf", "p2d_cyrl.pdf"], "png", "pdftext", op="png"),
    D("pdf.pdf2txt", "pdf", ["p2d_latn.pdf", "p2d_cyrl.pdf"], "txt", "pdftext", op="txt"),
    D("pdf.pdf2docx", "pdf", ["p2d_latn.pdf", "p2d_cyrl.pdf", "p2d_ru.pdf", "p2d_overimage.pdf", "p2d_widths.pdf"], "docx", "pdf2docx"),
    D("pdf.edge", "pdf", ["empty.pdf", "truncated.pdf", "scan.pdf"], "docx", "edge"),
]

# ---- the rows of `convert formats --json` that the table above does not list (WP10 found D2-D6 in them): same format and the other
# source formats, one fixture per source. staff.csv / staff.tsv are Excel's own CSV of hrm_staff.xlsx (gen_fixtures.py).
_SRC = {"doc": "hrm_order.doc", "odt": "order.odt", "rtf": "order.rtf", "html": "order.html", "md": "order.md", "txt": "order.txt"}
_MISSING = {"doc": "odt rtf html md txt png", "odt": "odt rtf html md txt png", "rtf": "odt rtf html md txt png",
            "html": "odt rtf html md txt pdf png", "md": "odt rtf html md txt png", "txt": "odt rtf html md txt pdf png"}
DIRECTIONS += [D("docs.docx2docx", "docs", ["hrm_order.docx"], "docx")]
DIRECTIONS += [D("docs.%s2%s" % (s, t), "docs", [_SRC[s]], t) for s, ts in _MISSING.items() for t in ts.split()]
DIRECTIONS += [D("sheets.xlsx2xlsx", "sheets", ["hrm_staff.xlsx"], "xlsx"), D("sheets.xls2csv", "sheets", ["hrm_staff.xls"], "csv")]
DIRECTIONS += [D("sheets.%s2%s" % (s, t), "sheets", ["hrm_staff." + s], t) for s in ("xlsb", "ods") for t in ("pdf", "csv")]
DIRECTIONS += [D("sheets.%s2%s" % (s, t), "sheets", ["staff." + s], t) for s in ("csv", "tsv") for t in ("pdf", "xlsx", "csv")]
DIRECTIONS += [D("slides.pptx2pptx", "slides", ["hrm_deck.pptx"], "pptx")]
assert len({d["id"] for d in DIRECTIONS}) == len(DIRECTIONS), "duplicate direction ids"


def select(spec):
    if spec in (None, "all"):
        return DIRECTIONS
    want = spec.split(",")
    return [d for d in DIRECTIONS if d["id"] in want or d["group"] in want or any(d["id"].startswith(w + ".") for w in want)]
