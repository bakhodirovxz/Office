#!/usr/bin/env python3
"""A one-paragraph document with its own protection (settings.xml documentProtection, enforced), the F7b fixtures of docs/phase7/malicious.md:
a plugin must not be started on it. Usage: make-protected.py <out.docx> [comments|trackedChanges]   (our code, MIT OR Apache-2.0)"""
import sys, zipfile
MODE = sys.argv[2] if len(sys.argv) > 2 else "comments"
CT = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>'
RELS = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
DRELS = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>'
W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
DOC = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body><w:p><w:r><w:t>Protected fixture text</w:t></w:r></w:p></w:body></w:document>'
SET = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings {W}><w:documentProtection w:edit="{MODE}" w:enforcement="1"/></w:settings>'
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    for n, b in (("[Content_Types].xml", CT), ("_rels/.rels", RELS), ("word/_rels/document.xml.rels", DRELS), ("word/document.xml", DOC), ("word/settings.xml", SET)):
        z.writestr(zipfile.ZipInfo(n, (1980, 1, 1, 0, 0, 0)), b)
