#!/usr/bin/env python3
"""Makes the starter documents in web/shell/templates/ and writes web/shell/templates.json (PLAN phase 4, WP5).

Clean-room originals: every text below was written for this project. No corpus file, no real person, organisation, number or
address. Every blank the user fills in is a [bracket]. The packages are hand-written OOXML, so there is no library to trust and
no library version in the output; the zips are deterministic (fixed time stamps, fixed order), so a rerun changes nothing.

    python scripts/gen-templates.py          # rewrite web/shell/templates/* and web/shell/templates.json
    python scripts/gen-templates.py --check  # fail if the committed files differ from what this script makes
"""
import io
import json
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "web" / "shell" / "templates"
CATALOG = ROOT / "web" / "shell" / "templates.json"
NS_W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NS_PR = "http://schemas.openxmlformats.org/package/2006/relationships"
NS_CT = "http://schemas.openxmlformats.org/package/2006/content-types"
NS_S = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
NS_A = "http://schemas.openxmlformats.org/drawingml/2006/main"
NS_P = "http://schemas.openxmlformats.org/presentationml/2006/main"
HDR = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
T = "ʼ"  # the Uzbek Latin apostrophe: ta'til


def zip_bytes(parts):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for name, text in parts:  # the order given: [Content_Types].xml first
            zi = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.create_system = 0
            zi.external_attr = 0o600 << 16
            z.writestr(zi, text.encode("utf-8"))
    return buf.getvalue()


def rels(items):
    body = "".join(f'<Relationship Id="{i}" Type="{REL}/{t}" Target="{tg}"/>' for i, t, tg in items)
    return f'{HDR}<Relationships xmlns="{NS_PR}">{body}</Relationships>'


def root_rels(main):
    core_rel = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"
    return rels([("rId1", "officeDocument", main)]).replace("</Relationships>", f'<Relationship Id="rId2" Type="{core_rel}" Target="docProps/core.xml"/></Relationships>')


def content_types(defaults, overrides):
    d = "".join(f'<Default Extension="{e}" ContentType="{c}"/>' for e, c in defaults)
    o = "".join(f'<Override PartName="{p}" ContentType="{c}"/>' for p, c in overrides)
    return f'{HDR}<Types xmlns="{NS_CT}">{d}{o}</Types>'


CT_XML = ("xml", "application/xml")
CT_RELS = ("rels", "application/vnd.openxmlformats-package.relationships+xml")
CT_CORE = ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml")


def core(title):
    return (
        f'{HDR}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
        'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
        f"<dc:title>{escape(title)}</dc:title><dc:creator></dc:creator>"
        '<dcterms:created xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:created></cp:coreProperties>'
    )


# ---------------------------------------------------------------- Word

def run(text, bold=False, size=None, italic=False, tab_before=False):
    pr = ("<w:b/>" if bold else "") + ("<w:i/>" if italic else "") + (f'<w:sz w:val="{size * 2}"/>' if size else "")
    pr = f"<w:rPr>{pr}</w:rPr>" if pr else ""
    tab = f"<w:r>{pr}<w:tab/></w:r>" if tab_before else ""
    return f'{tab}<w:r>{pr}<w:t xml:space="preserve">{escape(text)}</w:t></w:r>'


def para(runs, align=None, after=120, before=0, tabs=None, ind_left=None, first=None, keep=False):
    ppr = ""
    if keep:
        ppr += "<w:keepNext/>"
    if tabs:
        ppr += "<w:tabs>" + "".join(f'<w:tab w:val="{v}" w:pos="{p}"/>' for v, p in tabs) + "</w:tabs>"
    ppr += f'<w:spacing w:before="{before}" w:after="{after}"/>'
    if ind_left is not None or first is not None:
        ppr += "<w:ind" + (f' w:left="{ind_left}"' if ind_left is not None else "") + (f' w:firstLine="{first}"' if first is not None else "") + "/>"
    if align:
        ppr += f'<w:jc w:val="{align}"/>'
    return f"<w:p><w:pPr>{ppr}</w:pPr>{runs}</w:p>"


def docx(title, body):
    # A4, left margin 3 cm, the others 1.5 to 2 cm: the usual office layout
    sect = (
        '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
        '<w:pgMar w:top="1134" w:right="851" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr>'
    )
    document = f'{HDR}<w:document xmlns:w="{NS_W}" xmlns:r="{NS_R}"><w:body>{body}{sect}</w:body></w:document>'
    styles = (
        f'{HDR}<w:styles xmlns:w="{NS_W}"><w:docDefaults><w:rPrDefault><w:rPr>'
        '<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:cs="Times New Roman" w:eastAsia="Times New Roman"/>'
        '<w:sz w:val="28"/><w:szCs w:val="28"/><w:lang w:val="uz-Latn-UZ"/></w:rPr></w:rPrDefault>'
        '<w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>'
        '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style></w:styles>'
    )
    return zip_bytes(
        [
            (
                "[Content_Types].xml",
                content_types(
                    [CT_RELS, CT_XML],
                    [
                        ("/word/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
                        ("/word/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
                        CT_CORE,
                    ],
                ),
            ),
            ("_rels/.rels", root_rels("word/document.xml")),
            ("word/document.xml", document),
            ("word/_rels/document.xml.rels", rels([("rId1", "styles", "styles.xml")])),
            ("word/styles.xml", styles),
            ("docProps/core.xml", core(title)),
        ]
    )


def buyruq():
    right = [("right", 9354)]
    b = [
        para(run("[Tashkilot nomi]", bold=True), align="center", after=60),
        para(run("BUYRUQ", bold=True, size=16), align="center", after=240, before=120),
        para(run("[Sana]") + run("№ [raqam]", tab_before=True), tabs=right, after=60),
        para(run("[Shahar yoki aholi punkti]"), align="center", after=240),
        para(run("[Buyruqning mavzusi] toʻgʻrisida", bold=True), after=240),
        para(run("[Buyruq chiqarishning sababi yoki maqsadi: qisqacha] hamda [asos boʻlgan hujjatning nomi, sanasi va raqami] asosida"), first=567, after=120),
        para(run("BUYURAMAN:", bold=True), after=120, keep=True),
        para(run("1. [Birinchi topshiriq: kim, nima qiladi, qaysi muddatgacha]."), first=567),
        para(run("2. [Ikkinchi topshiriq]."), first=567),
        para(run("3. Mazkur buyruq ijrosini nazorat qilish [lavozimi, F.I.Sh.] zimmasiga yuklansin."), first=567, after=240),
        para(run("Asos: [ariza, bildirishnoma yoki boshqa hujjat, uning sanasi va raqami]."), after=480),
        para(run("[Rahbarning lavozimi]") + run("____________  [I.Sh. Familiya]", tab_before=True), tabs=right, after=480),
        para(run("Buyruq bilan tanishib chiqdim:", italic=True), after=120, keep=True),
        para(run("[Lavozimi]  ____________  [F.I.Sh.]  [sana]"), after=60),
        para(run("[Lavozimi]  ____________  [F.I.Sh.]  [sana]"), after=60),
    ]
    return docx("Buyruq", "".join(b))


def xizmat_xati():
    right = [("right", 9354)]
    b = [
        para(run("[Tashkilot nomi], [boʻlim nomi]", bold=True), after=0),
        para(run("Chiqish raqami: [raqam]") + run("Sana: [sana]", tab_before=True), tabs=right, after=360),
        para(run("[Qabul qiluvchi tashkilot nomi]"), align="right", after=0, ind_left=4536),
        para(run("[Qabul qiluvchining lavozimi]"), align="right", after=0, ind_left=4536),
        para(run("[I.Sh. Familiya]ga", bold=True), align="right", after=360, ind_left=4536),
        para(run("XIZMAT XATI", bold=True, size=16), align="center", after=240),
        para(run("[Xatning qisqacha mazmuni] toʻgʻrisida", bold=True), after=240),
        para(run("Hurmatli [ismi, otasining ismi]!"), align="center", after=240),
        para(run("[Birinchi xatboshi: xatning maqsadi, nima uchun murojaat qilinayotgani.]"), first=567, after=120),
        para(run("[Ikkinchi xatboshi: zarur maʼlumotlar (sana, miqdor, asos).]"), first=567, after=120),
        para(run("[Oxirgi xatboshi: soʻrov yoki taklif va javob muddati.]"), first=567, after=360),
        para(run("Ilova: [ilova nomi], [varaqlar soni] varaqda."), after=360),
        para(run("Hurmat bilan,"), after=240, keep=True),
        para(run("[Rahbarning lavozimi]") + run("____________  [I.Sh. Familiya]", tab_before=True), tabs=right, after=480),
        para(run("Ijrochi: [F.I.Sh.]", italic=True, size=11), after=0),
        para(run("Tel.: [telefon yoki ichki raqam]", italic=True, size=11), after=0),
    ]
    return docx("Xizmat xati", "".join(b))


# ---------------------------------------------------------------- Excel

def col(n):  # 1 -> A
    s = ""
    while n:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s


class Sheet:
    def __init__(self):
        self.cells = {}  # (row, col) -> xml
        self.merges = []
        self.shared = []

    def s(self, r, c, text, style=0):
        if text not in self.shared:
            self.shared.append(text)
        self.cells[(r, c)] = f'<c r="{col(c)}{r}" s="{style}" t="s"><v>{self.shared.index(text)}</v></c>'

    def n(self, r, c, num, style=0):
        self.cells[(r, c)] = f'<c r="{col(c)}{r}" s="{style}"><v>{num}</v></c>'

    def f(self, r, c, formula, style=0):
        self.cells[(r, c)] = f'<c r="{col(c)}{r}" s="{style}"><f>{escape(formula)}</f></c>'

    def blank(self, r, c, style):
        self.cells[(r, c)] = f'<c r="{col(c)}{r}" s="{style}"/>'

    def rows_xml(self, heights):
        out = []
        for r in sorted({k[0] for k in self.cells}):
            cs = "".join(self.cells[k] for k in sorted(k for k in self.cells if k[0] == r))
            ht = f' ht="{heights[r]}" customHeight="1"' if r in heights else ""
            out.append(f'<row r="{r}"{ht}>{cs}</row>')
        return "".join(out)


STYLES = (
    f'{HDR}<styleSheet xmlns="{NS_S}">'
    '<numFmts count="1"><numFmt numFmtId="164" formatCode="dd\\.mm\\.yyyy"/></numFmts>'
    '<fonts count="4"><font><sz val="11"/><name val="Arial"/></font><font><b/><sz val="11"/><name val="Arial"/></font>'
    '<font><b/><sz val="14"/><name val="Arial"/></font><font><i/><sz val="10"/><name val="Arial"/></font></fonts>'
    '<fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>'
    '<fill><patternFill patternType="solid"><fgColor rgb="FFE7E6E6"/><bgColor indexed="64"/></patternFill></fill></fills>'
    '<borders count="2"><border><left/><right/><top/><bottom/><diagonal/></border>'
    '<border><left style="thin"><color auto="1"/></left><right style="thin"><color auto="1"/></right>'
    '<top style="thin"><color auto="1"/></top><bottom style="thin"><color auto="1"/></bottom><diagonal/></border></borders>'
    '<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>'
    '<cellXfs count="9">'
    '<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>'  # 0 plain
    '<xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>'  # 1 header
    '<xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>'  # 2 body
    '<xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>'  # 3 body centred
    '<xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1" applyAlignment="1"><alignment horizontal="center"/></xf>'  # 4 title
    '<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>'  # 5 bold
    '<xf numFmtId="164" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>'  # 6 date
    '<xf numFmtId="0" fontId="3" fillId="0" borderId="0" xfId="0" applyFont="1"/>'  # 7 note
    '<xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>'  # 8 total
    '</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>'
)


def xlsx(title, sheet_name, sh, widths, heights, pane=None, autofilter=None, print_landscape=False):
    cols = "".join(f'<col min="{a}" max="{b}" width="{w}" customWidth="1"/>' for a, b, w in widths)
    view = '<sheetView workbookViewId="0">'
    if pane:
        view += f'<pane xSplit="{pane[0]}" ySplit="{pane[1]}" topLeftCell="{col(pane[0] + 1)}{pane[1] + 1}" activePane="bottomRight" state="frozen"/>'
    view += "</sheetView>"
    merges = f'<mergeCells count="{len(sh.merges)}">' + "".join(f'<mergeCell ref="{m}"/>' for m in sh.merges) + "</mergeCells>" if sh.merges else ""
    af = f'<autoFilter ref="{autofilter}"/>' if autofilter else ""
    page = f'<pageSetup paperSize="9" orientation="{"landscape" if print_landscape else "portrait"}" fitToWidth="1" fitToHeight="0"/>'
    sheet = (
        f'{HDR}<worksheet xmlns="{NS_S}" xmlns:r="{NS_R}"><sheetPr><pageSetUpPr fitToPage="1"/></sheetPr>'
        f"<sheetViews>{view}</sheetViews><sheetFormatPr defaultRowHeight=\"15\"/><cols>{cols}</cols>"
        f"<sheetData>{sh.rows_xml(heights)}</sheetData>{af}{merges}"
        '<pageMargins left="0.5" right="0.5" top="0.6" bottom="0.6" header="0.3" footer="0.3"/>'
        f"{page}</worksheet>"
    )
    sst = (
        f'{HDR}<sst xmlns="{NS_S}" count="{len(sh.shared)}" uniqueCount="{len(sh.shared)}">'
        + "".join(f'<si><t xml:space="preserve">{escape(t)}</t></si>' for t in sh.shared)
        + "</sst>"
    )
    wb = (
        f'{HDR}<workbook xmlns="{NS_S}" xmlns:r="{NS_R}"><bookViews><workbookView/></bookViews>'
        f'<sheets><sheet name="{escape(sheet_name)}" sheetId="1" r:id="rId1"/></sheets>'
        + (f'<definedNames><definedName name="_xlnm._FilterDatabase" localSheetId="0" hidden="1">\'{escape(sheet_name)}\'!{autofilter}</definedName></definedNames>' if autofilter else "")
        + '<calcPr calcId="191029" fullCalcOnLoad="1"/></workbook>'
    )
    return zip_bytes(
        [
            (
                "[Content_Types].xml",
                content_types(
                    [CT_RELS, CT_XML],
                    [
                        ("/xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"),
                        ("/xl/worksheets/sheet1.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"),
                        ("/xl/styles.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"),
                        ("/xl/sharedStrings.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"),
                        CT_CORE,
                    ],
                ),
            ),
            ("_rels/.rels", root_rels("xl/workbook.xml")),
            ("xl/workbook.xml", wb),
            ("xl/_rels/workbook.xml.rels", rels([("rId1", "worksheet", "worksheets/sheet1.xml"), ("rId2", "styles", "styles.xml"), ("rId3", "sharedStrings", "sharedStrings.xml")])),
            ("xl/worksheets/sheet1.xml", sheet),
            ("xl/styles.xml", STYLES),
            ("xl/sharedStrings.xml", sst),
            ("docProps/core.xml", core(title)),
        ]
    )


def tabel():
    sh = Sheet()
    last = 3 + 31 + 5  # A..C, 31 day columns, 5 totals -> AM
    sh.s(1, 1, "ISH VAQTINI HISOBGA OLISH TABELI", 4)
    sh.merges.append(f"A1:{col(last)}1")
    sh.s(2, 1, "Tashkilot: [Tashkilot nomi]", 5)
    sh.s(2, 10, "Hisobot oyi: [oy, yil]", 5)
    sh.s(2, 27, "Kunlik ish soati:", 5)  # from AA, so the label has room before the number in AG
    sh.n(2, 33, 8, 3)
    heads = ["№", "F.I.Sh.", "Lavozimi"] + [str(d) for d in range(1, 32)] + ["Ishlangan kunlar", "Ishlangan soatlar", "Kasallik (kun)", f"Ta{T}til (kun)", "Sababsiz kelmagan (kun)"]
    for i, h in enumerate(heads, 1):
        if 4 <= i <= 34:
            sh.n(4, i, i - 3, 1)
        else:
            sh.s(4, i, h, 1)
    first, lastrow = 5, 24
    for r in range(first, lastrow + 1):
        sh.n(r, 1, r - first + 1, 3)
        sh.s(r, 2, f"[F.I.Sh. {r - first + 1}]", 2) if r - first < 3 else sh.blank(r, 2, 2)
        sh.s(r, 3, "[Lavozim]", 2) if r - first < 3 else sh.blank(r, 3, 2)
        for d in range(4, 35):
            sh.blank(r, d, 3)
        rng = f"D{r}:AH{r}"
        sh.f(r, 35, f'COUNTIF({rng},"I")', 3)
        sh.f(r, 36, f"AI{r}*$AG$2", 3)
        sh.f(r, 37, f'COUNTIF({rng},"K")', 3)
        sh.f(r, 38, f'COUNTIF({rng},"T")', 3)
        sh.f(r, 39, f'COUNTIF({rng},"S")', 3)
    tr = lastrow + 1
    sh.s(tr, 2, "Jami", 8)
    for c in (35, 36, 37, 38, 39):
        sh.f(tr, c, f"SUM({col(c)}{first}:{col(c)}{lastrow})", 8)
    sh.s(tr + 2, 1, f"Shartli belgilar: I – ish kuni, K – kasallik, T – ta{T}til, S – sababsiz kelmaslik, D – dam olish kuni. Har bir kunga belgini kiriting, jami ustunlar avtomatik hisoblanadi.", 7)
    sh.s(tr + 4, 2, "Tabelni yuritgan: [lavozimi, F.I.Sh.]  ____________", 5)
    sh.s(tr + 5, 2, "Rahbar: [lavozimi, F.I.Sh.]  ____________", 5)
    widths = [(1, 1, 5), (2, 2, 28), (3, 3, 20), (4, 34, 3.6), (35, 39, 12)]
    return xlsx("Ish vaqti tabeli", "Tabel", sh, widths, {4: 45}, pane=(3, 4), print_landscape=True)


def xodimlar():
    sh = Sheet()
    sh.s(1, 1, "XODIMLAR ROʻYXATI", 4)
    sh.merges.append("A1:G1")
    sh.s(2, 1, "Tashkilot: [Tashkilot nomi]", 5)
    sh.s(2, 5, "Sana: [sana]", 5)
    heads = ["№", "F.I.Sh.", "Lavozimi", "Boʻlimi", "Ishga qabul qilingan sana", "Telefon raqami", "Izoh"]
    for i, h in enumerate(heads, 1):
        sh.s(4, i, h, 1)
    first, lastrow = 5, 34
    for r in range(first, lastrow + 1):
        sh.n(r, 1, r - first + 1, 3)
        if r - first < 3:
            sh.s(r, 2, f"[F.I.Sh. {r - first + 1}]", 2)
            sh.s(r, 3, "[Lavozim]", 2)
            sh.s(r, 4, "[Boʻlimi]", 2)
        else:
            for c in (2, 3, 4):
                sh.blank(r, c, 2)
        sh.blank(r, 5, 6)
        sh.blank(r, 6, 2)
        sh.blank(r, 7, 2)
    sh.s(lastrow + 2, 2, "Jami xodimlar soni:", 5)
    sh.f(lastrow + 2, 3, f"COUNTA(B{first}:B{lastrow})", 5)
    widths = [(1, 1, 5), (2, 2, 30), (3, 3, 24), (4, 4, 20), (5, 5, 16), (6, 6, 16), (7, 7, 24)]
    return xlsx("Xodimlar roʻyxati", "Xodimlar", sh, widths, {4: 32}, pane=(0, 4), autofilter=f"A4:G{lastrow}")


# ---------------------------------------------------------------- PowerPoint

def sp_ph(idn, name, ph, text_paras, off=None, ext=None):
    xfrm = f'<p:spPr><a:xfrm><a:off x="{off[0]}" y="{off[1]}"/><a:ext cx="{ext[0]}" cy="{ext[1]}"/></a:xfrm></p:spPr>' if off else "<p:spPr/>"
    return (
        f'<p:sp><p:nvSpPr><p:cNvPr id="{idn}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr>{ph}</p:nvPr></p:nvSpPr>'
        f"{xfrm}<p:txBody><a:bodyPr/><a:lstStyle/>{text_paras}</p:txBody></p:sp>"
    )


def pp(text, lvl=0):
    ppr = f'<a:pPr lvl="{lvl}"/>' if lvl else ""
    return f'<a:p>{ppr}<a:r><a:rPr lang="uz-Latn-UZ" dirty="0"/><a:t>{escape(text)}</a:t></a:r></a:p>'


def slide_xml(shapes):
    return (
        f'{HDR}<p:sld xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:spTree>'
        '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
        '<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>'
        f"{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    )


THEME = (
    f'{HDR}<a:theme xmlns:a="{NS_A}" name="Oddiy"><a:themeElements>'
    '<a:clrScheme name="Oddiy"><a:dk1><a:srgbClr val="1F2937"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>'
    '<a:dk2><a:srgbClr val="0F766E"/></a:dk2><a:lt2><a:srgbClr val="F3F4F6"/></a:lt2>'
    '<a:accent1><a:srgbClr val="0F766E"/></a:accent1><a:accent2><a:srgbClr val="2563EB"/></a:accent2>'
    '<a:accent3><a:srgbClr val="D97706"/></a:accent3><a:accent4><a:srgbClr val="7C3AED"/></a:accent4>'
    '<a:accent5><a:srgbClr val="DC2626"/></a:accent5><a:accent6><a:srgbClr val="4B5563"/></a:accent6>'
    '<a:hlink><a:srgbClr val="2563EB"/></a:hlink><a:folHlink><a:srgbClr val="7C3AED"/></a:folHlink></a:clrScheme>'
    '<a:fontScheme name="Oddiy"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>'
    '<a:minorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>'
    '<a:fmtScheme name="Oddiy"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
    '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>'
    '<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
    '<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
    '<a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>'
    '<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>'
    '<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>'
    '<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
    '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>'
)


def lvl_style(tag, size, bold, bullet):
    b = ' b="1"' if bold else ""
    bu = '<a:buFont typeface="Arial"/><a:buChar char="&#8226;"/>' if bullet else "<a:buNone/>"
    return f'<a:{tag} marL="{342900 if bullet else 0}" indent="{-342900 if bullet else 0}" algn="l"><a:buClr><a:schemeClr val="accent1"/></a:buClr>{bu}<a:defRPr sz="{size}"{b}><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:{tag}>'


def pptx():
    W, H = 12192000, 6858000
    master = (
        f'{HDR}<p:sldMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:bg><p:bgPr><a:solidFill><a:schemeClr val="bg1"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>'
        '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
        '<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>'
        + sp_ph(2, "Title", '<p:ph type="title"/>', pp("Sarlavha"), (762000, 457200), (10668000, 1143000))
        + sp_ph(3, "Body", '<p:ph type="body" idx="1"/>', pp("Matn"), (762000, 1828800), (10668000, 4343400))
        + '</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
        '<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/><p:sldLayoutId id="2147483650" r:id="rId2"/></p:sldLayoutIdLst>'
        f'<p:txStyles><p:titleStyle>{lvl_style("lvl1pPr", 4000, True, False)}</p:titleStyle>'
        f'<p:bodyStyle>{lvl_style("lvl1pPr", 2800, False, True)}{lvl_style("lvl2pPr", 2400, False, True)}</p:bodyStyle>'
        f'<p:otherStyle>{lvl_style("lvl1pPr", 1800, False, False)}</p:otherStyle></p:txStyles></p:sldMaster>'
    )

    def layout(name, typ, shapes):
        return (
            f'{HDR}<p:sldLayout xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" type="{typ}" preserve="1"><p:cSld name="{name}"><p:spTree>'
            '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
            '<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>'
            f"{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
        )

    lay_title = layout(
        "Sarlavha slaydi",
        "title",
        sp_ph(2, "Title", '<p:ph type="ctrTitle"/>', pp("Sarlavha"), (1219200, 2057400), (9753600, 1371600))
        + sp_ph(3, "Subtitle", '<p:ph type="subTitle" idx="1"/>', pp("Kichik sarlavha"), (1219200, 3657600), (9753600, 1143000)),
    )
    lay_body = layout(
        "Sarlavha va matn",
        "obj",
        sp_ph(2, "Title", '<p:ph type="title"/>', pp("Sarlavha"))
        + sp_ph(3, "Content", '<p:ph idx="1"/>', pp("Matn")),
    )
    slides = [
        (1, slide_xml(sp_ph(2, "Title", '<p:ph type="ctrTitle"/>', pp("[Taqdimot sarlavhasi]")) + sp_ph(3, "Subtitle", '<p:ph type="subTitle" idx="1"/>', pp("[Maʼruzachi: F.I.Sh., lavozimi]") + pp("[Sana]")))),
        (2, slide_xml(sp_ph(2, "Title", '<p:ph type="title"/>', pp("Reja")) + sp_ph(3, "Content", '<p:ph idx="1"/>', pp("[Birinchi masala]") + pp("[Ikkinchi masala]") + pp("[Uchinchi masala]") + pp("Xulosa va savollar")))),
        (2, slide_xml(sp_ph(2, "Title", '<p:ph type="title"/>', pp("[Birinchi masala]")) + sp_ph(3, "Content", '<p:ph idx="1"/>', pp("[Asosiy fikr]") + pp("[Dalil yoki raqam]", 1) + pp("[Dalil yoki raqam]", 1) + pp("[Keyingi qadam]")))),
        (1, slide_xml(sp_ph(2, "Title", '<p:ph type="ctrTitle"/>', pp("Eʼtiboringiz uchun rahmat!")) + sp_ph(3, "Subtitle", '<p:ph type="subTitle" idx="1"/>', pp("Savollaringiz bormi?") + pp("[Aloqa: telefon yoki elektron pochta]")))),
    ]
    pres = (
        f'{HDR}<p:presentation xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" saveSubsetFonts="1">'
        '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>'
        "<p:sldIdLst>" + "".join(f'<p:sldId id="{256 + i}" r:id="rId{2 + i}"/>' for i in range(len(slides))) + "</p:sldIdLst>"
        f'<p:sldSz cx="{W}" cy="{H}"/><p:notesSz cx="6858000" cy="9144000"/>'
        '<p:defaultTextStyle><a:defPPr><a:defRPr lang="uz-Latn-UZ"/></a:defPPr></p:defaultTextStyle></p:presentation>'
    )
    parts = [
        (
            "[Content_Types].xml",
            content_types(
                [CT_RELS, CT_XML],
                [
                    ("/ppt/presentation.xml", "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"),
                    ("/ppt/slideMasters/slideMaster1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"),
                    ("/ppt/slideLayouts/slideLayout1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"),
                    ("/ppt/slideLayouts/slideLayout2.xml", "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"),
                    ("/ppt/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml"),
                ]
                + [(f"/ppt/slides/slide{i + 1}.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml") for i in range(len(slides))]
                + [CT_CORE],
            ),
        ),
        ("_rels/.rels", root_rels("ppt/presentation.xml")),
        ("ppt/presentation.xml", pres),
        ("ppt/_rels/presentation.xml.rels", rels([("rId1", "slideMaster", "slideMasters/slideMaster1.xml")] + [(f"rId{2 + i}", "slide", f"slides/slide{i + 1}.xml") for i in range(len(slides))] + [(f"rId{2 + len(slides)}", "theme", "theme/theme1.xml")])),
        ("ppt/slideMasters/slideMaster1.xml", master),
        ("ppt/slideMasters/_rels/slideMaster1.xml.rels", rels([("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"), ("rId2", "slideLayout", "../slideLayouts/slideLayout2.xml"), ("rId3", "theme", "../theme/theme1.xml")])),
        ("ppt/slideLayouts/slideLayout1.xml", lay_title),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels", rels([("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")])),
        ("ppt/slideLayouts/slideLayout2.xml", lay_body),
        ("ppt/slideLayouts/_rels/slideLayout2.xml.rels", rels([("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")])),
        ("ppt/theme/theme1.xml", THEME),
    ]
    for i, (lay, xml) in enumerate(slides, 1):
        parts.append((f"ppt/slides/slide{i}.xml", xml))
        parts.append((f"ppt/slides/_rels/slide{i}.xml.rels", rels([("rId1", "slideLayout", f"../slideLayouts/slideLayout{lay}.xml")])))
    parts.append(("docProps/core.xml", core("Oddiy taqdimot")))
    return zip_bytes(parts)


FILES = {
    "buyruq.docx": buyruq,
    "xizmat-xati.docx": xizmat_xati,
    "tabel.xlsx": tabel,
    "xodimlar-royxati.xlsx": xodimlar,
    "taqdimot.pptx": pptx,
}

# the names and the template text are uz-Latn / uz-Cyrl UI strings; their review is delegated to the agent (CLAUDE.md)
STATUS = "reviewed:agent-opus:2026-10-23"

CATALOG_ITEMS = [
    ("docs", "buyruq.docx", {"en": "Order (buyruq)", "ru": "Приказ (buyruq)", "uz-Latn": "Buyruq", "uz-Cyrl": "Буйруқ"}),
    ("docs", "xizmat-xati.docx", {"en": "Official letter (xizmat xati)", "ru": "Служебное письмо (xizmat xati)", "uz-Latn": "Xizmat xati", "uz-Cyrl": "Хизмат хати"}),
    ("sheets", "tabel.xlsx", {"en": "Timesheet (tabel)", "ru": "Табель учёта рабочего времени", "uz-Latn": "Ish vaqti tabeli", "uz-Cyrl": "Иш вақти табели"}),
    ("sheets", "xodimlar-royxati.xlsx", {"en": "Staff list", "ru": "Список сотрудников", "uz-Latn": "Xodimlar roʻyxati", "uz-Cyrl": "Ходимлар рўйхати"}),
    ("slides", "taqdimot.pptx", {"en": "Simple presentation", "ru": "Простая презентация", "uz-Latn": "Oddiy taqdimot", "uz-Cyrl": "Оддий тақдимот"}),
]


def main():
    check = "--check" in sys.argv
    made = {f"templates/{n}": fn() for n, fn in FILES.items()}
    catalog = json.dumps([{"app": a, "file": f"templates/{f}", "status": STATUS, "name": n} for a, f, n in CATALOG_ITEMS], ensure_ascii=False, indent=2) + "\n"
    bad = 0
    for rel, data in made.items():
        p = ROOT / "web" / "shell" / rel
        if check:
            if not p.is_file() or p.read_bytes() != data:
                print("differs:", rel)
                bad = 1
        else:
            OUT.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
    if check:
        if not CATALOG.is_file() or CATALOG.read_text(encoding="utf-8") != catalog:
            print("differs: templates.json")
            bad = 1
        sys.exit(bad)
    CATALOG.write_text(catalog, encoding="utf-8", newline="\n")
    for rel, data in made.items():
        print(f"{rel}: {len(data)} bytes")


if __name__ == "__main__":
    main()
