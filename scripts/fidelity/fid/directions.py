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


def select(spec):
    if spec in (None, "all"):
        return DIRECTIONS
    want = spec.split(",")
    return [d for d in DIRECTIONS if d["id"] in want or d["group"] in want or any(d["id"].startswith(w + ".") for w in want)]
