"""Usage: python -I verify_docx.py <file.docx> <marker> [<marker> ...]
Exit 0 if every marker is present in the text of word/document.xml."""
import re
import sys
import zipfile

path, markers = sys.argv[1], sys.argv[2:]
with zipfile.ZipFile(path) as z:
    xml = z.read("word/document.xml").decode("utf-8")
text = re.sub(r"<[^>]+>", "", xml)
missing = [m for m in markers if m not in text]
if missing:
    print("MISSING:", missing)
    print("TEXT:", text[:400])
    sys.exit(1)
print("OK", markers)
