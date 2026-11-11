"""Writes wp13.docx next to this file: the synthetic document of the WP13 touch checklist (docs/phase6/checklist.md).
Three pages of text with a hard page break after page 1 (handles across a page break) and a wide table; nothing from a real document.  python make-wp13.py"""
import struct
import zlib
import zipfile
from pathlib import Path

W, H = 240, 140


def png() -> bytes:
    rows = b''.join(b'\x00' + bytes(v for x in range(W) for v in (40 + x * 150 // W, 90 + y * 120 // H, 200 - x * 100 // W)) for y in range(H))
    def chunk(t, d):
        c = struct.pack('>I', len(d)) + t + d
        return c + struct.pack('>I', zlib.crc32(t + d) & 0xFFFFFFFF)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')


NS = ('xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
      'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
      'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"')


def p(text, style=None):
    st = f'<w:pPr><w:pStyle w:val="{style}"/></w:pPr>' if style else ''
    return f'<w:p>{st}<w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>'


LOREM = ('Salom dunyo. Bu hujjat telefon ekranida oʻqish va tahrirlash qulayligini tekshirish uchun yozilgan sinov matni. '
         'Документ для проверки телефонного макета: текст переносится по ширине экрана. '
         'The quick brown fox jumps over the lazy dog.')
cell = lambda t: f'<w:tc><w:tcPr><w:tcW w:w="1200" w:type="dxa"/></w:tcPr>{p(t)}</w:tc>'
table = '<w:tbl><w:tblPr><w:tblW w:w="3600" w:type="dxa"/><w:tblBorders>' + ''.join(
    f'<w:{e} w:val="single" w:sz="4" w:space="0" w:color="808080"/>' for e in ('top', 'left', 'bottom', 'right', 'insideH', 'insideV')
) + '</w:tblBorders></w:tblPr><w:tblGrid>' + '<w:gridCol w:w="1200"/>' * 3 + '</w:tblGrid>' + ''.join(
    '<w:tr><w:trPr><w:trHeight w:val="600" w:hRule="atLeast"/></w:trPr>' + ''.join(cell(f'{r}{c}') for c in 'ABC') + '</w:tr>' for r in '123') + '</w:tbl>'
cx, cy = 2160000, 1260000
picture = (f'<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="1" name="Picture 1"/>'
           '<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="p.png"/><pic:cNvPicPr/></pic:nvPicPr>'
           f'<pic:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>'
           '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>')
PB = '<w:p><w:r><w:br w:type="page"/></w:r></w:p>'
wide = lambda: '<w:tbl><w:tblPr><w:tblW w:w="9000" w:type="dxa"/><w:tblBorders>' + ''.join(f'<w:{e} w:val="single" w:sz="4" w:space="0" w:color="808080"/>' for e in ('top', 'left', 'bottom', 'right', 'insideH', 'insideV')) + '</w:tblBorders></w:tblPr><w:tblGrid>' + '<w:gridCol w:w="1800"/>' * 5 + '</w:tblGrid>' + ''.join('<w:tr>' + ''.join(f'<w:tc><w:tcPr><w:tcW w:w="1800" w:type="dxa"/></w:tcPr>{p(r + c)}</w:tc>' for c in 'ABCDE') + '</w:tr>' for r in '12') + '</w:tbl>'
body = ''.join([p('Touch checklist', 'Heading1')] + [p(LOREM) for _ in range(6)] + [wide(), p('')] + [p(LOREM) for _ in range(3)] + [PB, p('Second page', 'Heading2')] + [p(LOREM) for _ in range(10)] + [PB, p('Third page', 'Heading2')] + [p(LOREM) for _ in range(6)])
document = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>'
styles = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
          '<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Noto Sans" w:hAnsi="Noto Sans"/><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults>'
          '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
          '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style>'
          '<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="80"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>'
          '</w:styles>')
types = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
         '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>'
         '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
         '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>')
rels = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
doc_rels = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>')

out = Path(__file__).with_name('wp13.docx')
with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED) as z:
    for name, data in [('[Content_Types].xml', types), ('_rels/.rels', rels), ('word/document.xml', document), ('word/_rels/document.xml.rels', doc_rels), ('word/styles.xml', styles), ('word/media/image1.png', png())]:
        z.writestr(name, data)
print(out, out.stat().st_size)
