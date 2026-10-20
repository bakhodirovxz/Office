#!/usr/bin/env python3
"""Synthetic ~108-page Uzbek .docx for the transliteration performance work (Phase 2 WP10a).

Raw OOXML, no dependencies. Text is seeded from the golden list vocabulary (Latin or Cyrillic column), so the file holds
no personal data. Content: title, TOC field, headings, numbered lists, tables, header and footer with PAGE fields,
footnotes, hyperlinks, mid-word bold/italic run splits, ru/en language-tagged runs and a text box.

  python scripts/translit/make-108.py --script latn|cyrl --chapters 12 --paras 22 --out file.docx
"""
import argparse
import csv
import random
import re
import sys
import zipfile
from xml.sax.saxutils import escape

NS = (
    'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
    'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
    'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" '
    'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
    'xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"'
)
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PROLOG = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
WORD_RE = re.compile(r"[^\W\d_][^\W\d_]*[^\W\d_]", re.U)


def vocab(script):
    col = "latin" if script == "latn" else "cyrillic"
    words = []
    with open("crates/uz-translit/data/golden.tsv", encoding="utf-8") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            if not row["status"].startswith("reviewed") or row["dir"] != "both":
                continue
            w = row[col]
            if WORD_RE.fullmatch(w) and 3 <= len(w) <= 14:
                words.append(w)
    return sorted(set(words))


def fld(instr, result):
    return (
        '<w:r><w:fldChar w:fldCharType="begin"/></w:r>'
        f'<w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r>'
        '<w:r><w:fldChar w:fldCharType="separate"/></w:r>'
        f"<w:r><w:t>{result}</w:t></w:r>"
        '<w:r><w:fldChar w:fldCharType="end"/></w:r>'
    )


class Gen:
    def __init__(self, script, seed):
        self.rng = random.Random(seed)
        self.words = vocab(script)
        self.footnotes = []
        self.links = 0
        self.boxes = 0

    def word(self):
        return self.rng.choice(self.words)

    def sentence(self, n=None):
        n = n or self.rng.randint(6, 16)
        ws = [self.word() for _ in range(n)]
        ws[0] = ws[0][0].upper() + ws[0][1:]
        return " ".join(ws) + "."

    def t(self, text, rpr=""):
        pr = f"<w:rPr>{rpr}</w:rPr>" if rpr else ""
        return f'<w:r>{pr}<w:t xml:space="preserve">{escape(text)}</w:t></w:r>'

    def para_runs(self):
        """Runs of one body paragraph: plain text with mid-word splits, tagged-language sentences and rare objects."""
        r, out = self.rng, []
        for _ in range(r.randint(4, 7)):
            roll = r.random()
            s = self.sentence()
            if roll < 0.18:  # mid-word bold / italic split: the digraph-stress case
                i = r.randint(2, max(2, len(s) // 2))
                fmt = r.choice(["<w:b/>", "<w:i/>", "<w:b/><w:i/>"])
                out += [self.t(s[:i]), self.t(s[i : i + 5], fmt), self.t(s[i + 5 :] + " ")]
            elif roll < 0.24:
                out.append(self.t(s + " ", f'<w:lang w:val="{r.choice(["ru-RU", "en-US"])}"/>'))
            elif roll < 0.28:
                h = len(s) // 2
                out += [self.t(s[:h], '<w:rFonts w:ascii="Arial" w:hAnsi="Arial"/>'), self.t(s[h:] + " ")]
            else:
                out.append(self.t(s + " "))
        if r.random() < 0.10:
            self.links += 1
            out.append(f'<w:hyperlink r:id="rIdL1"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>{escape(self.word())}</w:t></w:r></w:hyperlink>')
            out.append(self.t(" "))
        if r.random() < 0.12:
            self.footnotes.append(self.sentence())
            out.append(f'<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{len(self.footnotes)}"/></w:r>')
        return "".join(out)

    def p(self, runs, ppr=""):
        pr = f"<w:pPr>{ppr}</w:pPr>" if ppr else ""
        return f"<w:p>{pr}{runs}</w:p>"

    def heading(self, lvl, text, bookmark=None):
        bs = f'<w:bookmarkStart w:id="{bookmark}" w:name="_Toc{bookmark}"/>' if bookmark else ""
        be = f'<w:bookmarkEnd w:id="{bookmark}"/>' if bookmark else ""
        return self.p(bs + self.t(text) + be, f'<w:pStyle w:val="Heading{lvl}"/>')

    def table(self, rows=5, cols=4):
        grid = "".join('<w:gridCol w:w="2200"/>' for _ in range(cols))
        trs = ""
        for ri in range(rows):
            tcs = ""
            for _ in range(cols):
                txt = " ".join(self.word() for _ in range(self.rng.randint(1, 3)))
                tcs += f'<w:tc><w:tcPr><w:tcW w:w="2200" w:type="dxa"/></w:tcPr>{self.p(self.t(txt, "<w:b/>" if ri == 0 else ""))}</w:tc>'
            trs += f"<w:tr>{tcs}</w:tr>"
        return f'<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid>{grid}</w:tblGrid>{trs}</w:tbl>' + self.p("")

    def textbox(self):
        self.boxes += 1
        inner = self.p(self.t(self.sentence(8)))
        return (
            '<w:p><w:r><w:drawing><wp:inline><wp:extent cx="3200400" cy="685800"/>'
            f'<wp:docPr id="{100 + self.boxes}" name="Box{self.boxes}"/><a:graphic>'
            '<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">'
            '<wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:prstGeom prst="rect"/></wps:spPr>'
            f"<wps:txbx><w:txbxContent>{inner}</w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp>"
            "</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
        )

    def body(self, chapters, paras):
        out = [self.p(self.t(self.sentence(5)), '<w:pStyle w:val="Title"/>')]
        titles = [self.sentence(4).rstrip(".") for _ in range(chapters)]
        for i, ti in enumerate(titles):  # one TOC field spanning the entries
            first = (
                '<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \\o "1-2" \\h \\z \\u </w:instrText></w:r>'
                '<w:r><w:fldChar w:fldCharType="separate"/></w:r>'
                if i == 0
                else ""
            )
            last = '<w:r><w:fldChar w:fldCharType="end"/></w:r>' if i == chapters - 1 else ""
            out.append(self.p(first + self.t(f"{ti}\t{i + 2}") + last, '<w:pStyle w:val="TOC1"/>'))
        for c, ti in enumerate(titles):
            out.append(self.heading(1, ti, c + 1))
            for k in range(paras):
                if k % 7 == 3:
                    out.append(self.heading(2, self.sentence(3).rstrip(".")))
                if k % 9 == 5:
                    for _ in range(4):
                        out.append(self.p(self.t(self.sentence(8)), '<w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>'))
                if k % 11 == 7:
                    out.append(self.table())
                if k == 4:
                    out.append(self.textbox())
                out.append(self.p(self.para_runs(), '<w:jc w:val="both"/>' if k % 3 == 0 else ""))
        sect = (
            '<w:sectPr><w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF1"/>'
            '<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1134" w:bottom="1417" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr>'
        )
        return f'{PROLOG}<w:document {NS}><w:body>{"".join(out)}{sect}</w:body></w:document>'


def pstyle(i, name, ppr="", rpr="", nxt=None):
    n = f'<w:next w:val="{nxt}"/>' if nxt else ""
    return f'<w:style w:type="paragraph" w:styleId="{i}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/>{n}<w:qFormat/><w:pPr>{ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>'


def styles(lang):
    borders = "".join(f'<w:{s} w:val="single" w:sz="4" w:space="0" w:color="auto"/>' for s in ["top", "left", "bottom", "right", "insideH", "insideV"])
    return (
        f"{PROLOG}<w:styles {NS}><w:docDefaults><w:rPrDefault><w:rPr>"
        '<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:cs="Times New Roman"/>'
        f'<w:sz w:val="24"/><w:lang w:val="{lang}"/></w:rPr></w:rPrDefault>'
        '<w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>'
        '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>'
        + pstyle("Title", "Title", '<w:jc w:val="center"/><w:spacing w:after="240"/>', "<w:b/><w:sz w:val=\"40\"/>", "Normal")
        + pstyle("Heading1", "heading 1", '<w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/>', '<w:b/><w:sz w:val="32"/>', "Normal")
        + pstyle("Heading2", "heading 2", '<w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="1"/>', '<w:b/><w:i/><w:sz w:val="28"/>', "Normal")
        + pstyle("TOC1", "toc 1", '<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9630"/></w:tabs><w:spacing w:after="60"/>')
        + pstyle("ListParagraph", "List Paragraph", '<w:ind w:left="720"/>')
        + pstyle("Header", "header", '<w:jc w:val="right"/>', '<w:i/><w:sz w:val="20"/>')
        + pstyle("Footer", "footer", '<w:jc w:val="center"/>', '<w:sz w:val="20"/>')
        + pstyle("FootnoteText", "footnote text", '<w:spacing w:after="0"/>', '<w:sz w:val="20"/>')
        + '<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>'
        + '<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>'
        + f'<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblBorders>{borders}</w:tblBorders></w:tblPr></w:style></w:styles>'
    )


def build(script, chapters, paras, seed, out):
    g = Gen(script, seed)
    doc = g.body(chapters, paras)
    lang = "uz-Latn-UZ" if script == "latn" else "uz-Cyrl-UZ"
    head = f"{PROLOG}<w:hdr {NS}>" + g.p(g.t(g.sentence(4)) + g.t(" - ") + '<w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>', '<w:pStyle w:val="Header"/>') + "</w:hdr>"
    foot = f"{PROLOG}<w:ftr {NS}>" + g.p(g.t(g.word().capitalize() + " ") + fld("PAGE", "1") + g.t(" / ") + fld("NUMPAGES", "108"), '<w:pStyle w:val="Footer"/>') + "</w:ftr>"
    fn = (
        f"{PROLOG}<w:footnotes {NS}>"
        '<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>'
        '<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>'
    )
    for i, txt in enumerate(g.footnotes, 1):
        fn += (
            f'<w:footnote w:id="{i}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr>'
            f'<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r>{g.t(" " + txt)}</w:p></w:footnote>'
        )
    fn += "</w:footnotes>"
    num = (
        f'{PROLOG}<w:numbering {NS}><w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>'
        '<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/>'
        '<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>'
    )
    wml = "application/vnd.openxmlformats-officedocument.wordprocessingml"
    ct = (
        f'{PROLOG}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'
        + "".join(f'<Override PartName="/word/{p}.xml" ContentType="{wml}.{t}+xml"/>' for p, t in [("document", "document.main"), ("styles", "styles"), ("numbering", "numbering"), ("footnotes", "footnotes"), ("header1", "header"), ("footer1", "footer")])
        + "</Types>"
    )
    rels = (
        f'{PROLOG}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        + "".join(f'<Relationship Id="{i}" Type="{REL}/{t}" Target="{tg}"/>' for i, t, tg in [("rIdS", "styles", "styles.xml"), ("rIdN", "numbering", "numbering.xml"), ("rIdFn", "footnotes", "footnotes.xml"), ("rIdH1", "header", "header1.xml"), ("rIdF1", "footer", "footer1.xml")])
        + f'<Relationship Id="rIdL1" Type="{REL}/hyperlink" Target="https://example.org/hujjat/1" TargetMode="External"/></Relationships>'
    )
    root = f'{PROLOG}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>'
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in [
            ("[Content_Types].xml", ct),
            ("_rels/.rels", root),
            ("word/document.xml", doc),
            ("word/_rels/document.xml.rels", rels),
            ("word/styles.xml", styles(lang)),
            ("word/numbering.xml", num),
            ("word/footnotes.xml", fn),
            ("word/header1.xml", head),
            ("word/footer1.xml", foot),
        ]:
            z.writestr(name, data)
    print(f"{out}: {len(doc) // 1024} KiB document.xml, {len(g.footnotes)} footnotes, {g.links} links, {g.boxes} boxes, {len(g.words)} vocab words", file=sys.stderr)


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--script", choices=["latn", "cyrl"], required=True)
    a.add_argument("--chapters", type=int, default=12)
    a.add_argument("--paras", type=int, default=22)
    a.add_argument("--seed", type=int, default=108)
    a.add_argument("--out", required=True)
    x = a.parse_args()
    build(x.script, x.chapters, x.paras, x.seed, x.out)
