"""Rewrite columns G,H,I of every worksheet into Excel-style shared formulas (master at row 2, ref to last row)."""
import sys, re, zipfile
src, dst = sys.argv[1], sys.argv[2]
zin = zipfile.ZipFile(src); zout = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
pat = re.compile(r'<c r="([GHI])(\d+)"><f>([^<]*)</f><v ?/></c>')
for it in zin.infolist():
    data = zin.read(it.filename)
    if it.filename.startswith("xl/worksheets/sheet"):
        s = data.decode("utf-8")
        rows = [int(m.group(2)) for m in pat.finditer(s)]
        if rows:
            last = max(rows); n = [0]
            def rep(m):
                col, r, f = m.group(1), int(m.group(2)), m.group(3); si = "GHI".index(col)
                n[0] += 1
                if r == 2: return f'<c r="{col}{r}"><f t="shared" ref="{col}2:{col}{last}" si="{si}">{f}</f></c>'
                return f'<c r="{col}{r}"><f t="shared" si="{si}"/></c>'
            s = pat.sub(rep, s); print(it.filename, n[0], "formulas shared")
        data = s.encode("utf-8")
    zout.writestr(it, data)
zout.close()
