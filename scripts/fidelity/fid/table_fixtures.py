"""Synthetic table fixtures (hand-written WordprocessingML, ECMA-376 Part 1 section 17.4), shaped like the markup Word writes for
each table class of docs/phase4/tables.md. No Word output is stored: Word only rendered them once to record the golden rule
positions (ci/fidelity/tables-golden.json, `tables.py --record`). Synthetic text only."""
import zipfile
from pathlib import Path

NS = ('xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
      'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"')
NAMES = ["Karimov Aziz Botirovich", "Абдуллаева Нилуфар Шавкатовна", "Rahimova Dilnoza", "Тошпулатов Шерзод Акмалович",
         "Yusupov Jamshid", "Норматова Гулнора", "Xolmatov Bekzod Erkinovich", "Сафарова Мадина Рустамовна"]
BORDERS = "".join('<w:%s w:val="single" w:sz="4" w:space="0" w:color="auto"/>' % s
                  for s in ("top", "left", "bottom", "right", "insideH", "insideV"))


def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def p(text=""):
    return '<w:p><w:r><w:t xml:space="preserve">%s</w:t></w:r></w:p>' % esc(text) if text else "<w:p/>"


def tc(text, w=None, typ="dxa", span=1, vmerge=None):
    pr = '<w:tcW w:w="%s" w:type="%s"/>' % (w if w is not None else 0, typ if w is not None else "auto")
    if span > 1:
        pr += '<w:gridSpan w:val="%d"/>' % span
    if vmerge:
        pr += '<w:vMerge w:val="restart"/>' if vmerge == "restart" else "<w:vMerge/>"
    return "<w:tc><w:tcPr>%s</w:tcPr>%s</w:tc>" % (pr, p(text))


def tbl(grid, rows, tblw='<w:tblW w:w="0" w:type="auto"/>', extra="", row_pr=""):
    g = "".join('<w:gridCol w:w="%d"/>' % x for x in grid)
    body = "".join("<w:tr>%s%s</w:tr>" % ("<w:trPr>%s</w:trPr>" % row_pr if row_pr else "", "".join(r)) for r in rows)
    return ('<w:tbl><w:tblPr>%s%s<w:tblBorders>%s</w:tblBorders><w:tblLook w:val="04A0"/></w:tblPr><w:tblGrid>%s</w:tblGrid>%s</w:tbl>'
            % (tblw, extra, BORDERS, g, body))


def spans_narrow():
    """A narrow grid column (283 tw) under cells spanning it: rows with their own cell boundaries (Word: Split Cells, cell widths)."""
    g = [1134, 1701, 283, 1985, 3402]
    rows = [[tc("1.", 1134), tc(NAMES[0], 3969, span=3), tc("bosh mutaxassis", 3402)],
            [tc("2.", 1134), tc(NAMES[1], 3969, span=3), tc("ведущий инженер", 3402)],
            [tc("3.", 1134), tc("a", 1984, span=2), tc("b", 1985), tc("bo'lim boshlig'i", 3402)],
            [tc("4.", 2835, span=2), tc(NAMES[3], 2268, span=2), tc("бухгалтер", 3402)],
            [tc("5. " + NAMES[4] + " operator", 8505, span=5)]]
    return tbl(g, rows, '<w:tblW w:w="8505" w:type="dxa"/>')


def form_card():
    """A form whose every row has its own boundaries: 17 grid columns, every cell spans several (personnel-card style)."""
    g = [1617, 329, 424, 235, 133, 512, 950, 586, 473, 280, 963, 170, 510, 2172, 41, 200, 301]
    spec = [[3, 13], [6, 4, 3, 3], [2, 7, 3, 4], [9, 7], [4, 4, 4, 4], [1, 10, 6], [5, 2, 8], [14]]
    labels = ["Tabel raqami", "Familiyasi", "Ismi", "Otasining ismi", "Tug'ilgan sanasi", "Фуқаролиги", "Маълумоти", "Mutaxassisligi"]
    rows = []
    for r, spans in enumerate(spec, 1):
        cells, at = [], 0
        for c, s in enumerate(spans, 1):
            w = sum(g[at:at + s]); at += s
            cells.append(tc(labels[(r + c) % 8] if c % 2 else NAMES[(r + c) % 8], w, span=s))
        rows.append(cells)
    return tbl(g, rows, '<w:tblW w:w="9896" w:type="dxa"/>')


def staff_rows(n, cols, widths=None, typ="dxa"):
    posts = ["bosh mutaxassis", "ведущий инженер", "bo'lim boshlig'i", "бухгалтер", "operator"]
    rows = []
    for i in range(n):
        vals = ["%d." % (i + 1), NAMES[i % 8], posts[i % 5], "%d,00" % (1200 + 37 * i)][:cols]
        rows.append([tc(v, widths[j] if widths else None, typ) for j, v in enumerate(vals)])
    return rows


def autofit_long():
    """AutoFit to contents (tblW and every tcW auto) with a cell too long for one line: the table takes the whole room."""
    rows = staff_rows(5, 4)
    rows[2][1] = tc("Juda uzun matn bir katakda, u qatorni bir necha marta o'raydi: " + "so'z " * 20)
    return tbl([396, 6602, 1577, 996], rows)


def pct(table_pct, col_pct):
    """Preferred widths in percent (Word's AutoFit to Window writes 5000 = 100 %, columns 1250 each)."""
    w = '<w:tblW w:w="%d" w:type="pct"/>' % (table_pct * 50)
    total = 9571 * table_pct // 100
    return tbl([total * c // 100 for c in col_pct], staff_rows(5, len(col_pct), [c * 50 for c in col_pct], "pct"), w)


def grid_dxa():
    """Insert Table defaults: fixed dxa cell widths equal to the grid (the control case: it matched before)."""
    g = [2392, 2393, 2393, 2393]
    return tbl(g, staff_rows(5, 4, g))


def wide(factor, tblw):
    """Columns `factor` times as wide as the 9355 tw text column (a landscape table pasted on a portrait page). With tblw Word's
    own markup once the columns are sized by hand (w:tblW in dxa); without it w:tblW is auto."""
    w = round(9355 * factor / 6)
    g = [w] * 6
    tw = '<w:tblW w:w="%d" w:type="dxa"/>' % sum(g) if tblw else '<w:tblW w:w="0" w:type="auto"/>'
    return tbl(g, [r + [tc("", w), tc("", w)] for r in staff_rows(4, 4, g[:4])], tw)


def cell_spacing():
    """Allow spacing between cells (tblCellSpacing in the table and in every row, as Word writes it)."""
    g = [3118, 3118, 3119]
    sp = '<w:tblCellSpacing w:w="28" w:type="dxa"/>'
    return tbl(g, staff_rows(4, 3, g), extra=sp, row_pr=sp)


# name -> (compatibility mode, title, body builder)
FIXTURES = {
    "spans_narrow": (15, "Spanning cells over a narrow grid column", spans_narrow),
    "form_card": (15, "Form card: every row has its own cell boundaries", form_card),
    "compat14_autofit_long": (14, "Word 2010 mode, AutoFit to contents with a long cell", autofit_long),
    "compat14_pct100": (14, "Word 2010 mode, 100 % wide table", lambda: pct(100, [25, 25, 25, 25])),
    "compat14_pct80": (14, "Word 2010 mode, 80 % wide table, columns 10/50/40 %", lambda: pct(80, [10, 50, 40])),
    "compat15_autofit_long": (15, "Word 2013 mode, AutoFit to contents with a long cell", autofit_long),
    "compat14_grid_dxa": (14, "Word 2010 mode, fixed widths (control)", grid_dxa),
    "wide_dxa": (15, "Columns sized by hand to 1.6 times the text width", lambda: wide(1.6, True)),
    "wide_auto160": (15, "Columns 1.6 times the text width, automatic table width", lambda: wide(1.6, False)),
    "wide_auto120": (15, "Columns 1.2 times the text width, automatic table width", lambda: wide(1.2, False)),
    "cell_spacing": (15, "Spacing between cells", cell_spacing),
}

STYLES = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles %s><w:docDefaults><w:rPrDefault><w:rPr>'
          '<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:cs="Times New Roman" w:eastAsia="Times New Roman"/>'
          '<w:sz w:val="24"/><w:szCs w:val="24"/><w:lang w:val="ru-RU"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>'
          '<w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>'
          '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
          '<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/>'
          '<w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/>'
          '<w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style></w:styles>') % NS
CT = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
      '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'
      '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
      '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>'
      '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>')
RELS = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
DOC_RELS = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>')


def write(out):
    """Writes every fixture as <out>/<name>.docx; returns the paths."""
    out = Path(out); out.mkdir(parents=True, exist_ok=True)
    paths = []
    for name, (compat, title, body) in FIXTURES.items():
        doc = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document %s><w:body>%s%s%s'
               '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" '
               'w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>') % (NS, p(title), body(), p())
        settings = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings %s><w:compat><w:compatSetting '
                    'w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="%d"/></w:compat></w:settings>') % (NS, compat)
        f = out / (name + ".docx")
        with zipfile.ZipFile(f, "w", zipfile.ZIP_DEFLATED) as z:
            for part, data in (("[Content_Types].xml", CT), ("_rels/.rels", RELS), ("word/_rels/document.xml.rels", DOC_RELS),
                               ("word/document.xml", doc), ("word/styles.xml", STYLES), ("word/settings.xml", settings)):
                z.writestr(part, data)
        paths.append(f)
    return paths
