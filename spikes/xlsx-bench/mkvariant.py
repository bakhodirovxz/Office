"""mkvariant.py SRC DST COLS LAST [SHEETS]: turn column COLS (e.g. "G" or "GHI") of sheets (default all S
sheets) of a gridcraft-written xlsx into shared formulas from row 2 to LAST (master keeps its text and
value, followers keep their value)."""
import re
import sys
import zipfile

src, dst, cols, last = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
only = sys.argv[5].split(",") if len(sys.argv) > 5 else None
zin = zipfile.ZipFile(src)
zout = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
wb = zin.read("xl/workbook.xml").decode()
names = re.findall(r'<sheet [^>]*name="([^"]*)"', wb)
pat = re.compile(r'<c r="([A-Z]+)(\d+)"([^>]*)><f>([^<]*)</f>')
for it in zin.infolist():
    data = zin.read(it.filename)
    m = re.match(r"xl/worksheets/sheet(\d+)\.xml", it.filename)
    if m:
        name = names[int(m.group(1)) - 1]
        if name.startswith("S") and (only is None or name in only):
            s = data.decode()

            def rep(x):
                col, r, attrs, f = x.group(1), int(x.group(2)), x.group(3), x.group(4)
                if col not in cols or r < 2 or r > last:
                    return x.group(0)
                si = cols.index(col)
                if r == 2:
                    return f'<c r="{col}{r}"{attrs}><f t="shared" ref="{col}2:{col}{last}" si="{si}">{f}</f>'
                return f'<c r="{col}{r}"{attrs}><f t="shared" si="{si}"/>'

            data = pat.sub(rep, s).encode()
    zout.writestr(it, data)
zout.close()
