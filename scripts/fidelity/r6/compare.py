"""R6 round-trip comparison (docs/phase5/r6.md): what an open -> one edit -> save -> reopen kept, read with independent libraries
(openpyxl, python-pptx, pypdf) and the package's part list. Usage: python -I compare.py <xlsx|pptx|pdf> <original> <saved> [skip]
`skip` names what the edit itself changed (xlsx: Sheet!A12; pptx: the slide number the edit added one shape to). Prints JSON
{"losses": [...], "notes": [...], "counts": {...}}; a loss is anything of the original that differs or is gone."""
import sys, json, zipfile, re, hashlib
from collections import Counter

kind, orig, saved = sys.argv[1:4]
skip = sys.argv[4] if len(sys.argv) > 4 else ""
losses, notes, counts = [], [], {}


def parts(path):
    """Package parts by kind (charts, media, drawings, ...): a writer that drops a feature loses its parts."""
    c = Counter()
    with zipfile.ZipFile(path) as z:
        for n in z.namelist():
            if n.endswith("/") or n.endswith(".rels") or n == "[Content_Types].xml":
                continue
            m = re.match(r"(?:xl|ppt|word)/([A-Za-z]+)", n)
            c[m.group(1) if m else n.split("/")[0]] += 1
    return c


def cmp_parts():
    a, b = parts(orig), parts(saved)
    counts["parts_original"], counts["parts_saved"] = dict(a), dict(b)
    for k, n in a.items():
        if b.get(k, 0) < n:
            # calcChain is Excel's recalculation order cache: Excel rebuilds it when it is missing (ECMA-376 Part 1, 18.6)
            (notes if k == "calcChain" else losses).append(f"package parts '{k}': {n} -> {b.get(k, 0)}")


def norm(v):
    if isinstance(v, float) and v.is_integer():
        return int(v)
    if isinstance(v, str) and v.startswith("="):
        return v.replace(" ", "").replace("$", "").upper()
    return v


def color(c):
    if c is None:
        return None
    if c.type == "rgb":
        return ("rgb", str(c.rgb)[-6:], 0)  # the alpha byte of an ARGB colour is ignored by Excel
    return (c.type, c.theme if c.type == "theme" else c.indexed, round(c.tint or 0, 3))


def font_color(c):
    # no colour is "automatic", which Excel draws as the theme's dark 1 (text 1); the writer names it
    return color(c) or ("theme", 1, 0)


def xlsx():
    import openpyxl
    cmp_parts()
    A, B = openpyxl.load_workbook(orig), openpyxl.load_workbook(saved)
    if A.sheetnames != B.sheetnames:
        losses.append(f"sheet names {A.sheetnames} -> {B.sheetnames}")
    sk_sheet, sk_cell = (skip.split("!") + [""])[:2] if skip else ("", "")
    cells = diff = 0
    for name in A.sheetnames:
        if name not in B.sheetnames:
            continue
        a, b = A[name], B[name]
        if a.sheet_state != b.sheet_state:
            losses.append(f"{name}: state {a.sheet_state} -> {b.sheet_state}")
        for row in a.iter_rows():
            for ca in row:
                if name == sk_sheet and ca.coordinate == sk_cell:
                    continue
                cb = b[ca.coordinate]
                cells += 1
                for what, x, y in [
                    ("value", norm(ca.value), norm(cb.value)),
                    ("number format", ca.number_format, cb.number_format),
                    ("font", (ca.font.b, ca.font.i, ca.font.u, ca.font.sz, ca.font.name, font_color(ca.font.color)),
                             (cb.font.b, cb.font.i, cb.font.u, cb.font.sz, cb.font.name, color(cb.font.color))),
                    ("fill", (ca.fill.fill_type, color(ca.fill.fgColor)) if ca.fill.fill_type else None,
                             (cb.fill.fill_type, color(cb.fill.fgColor)) if cb.fill.fill_type else None),
                    ("border", tuple(getattr(ca.border, s).style for s in ("left", "right", "top", "bottom")),
                               tuple(getattr(cb.border, s).style for s in ("left", "right", "top", "bottom"))),
                    ("alignment", (ca.alignment.horizontal, ca.alignment.vertical, bool(ca.alignment.wrap_text)),
                                  (cb.alignment.horizontal, cb.alignment.vertical, bool(cb.alignment.wrap_text))),
                    ("hyperlink", ca.hyperlink.target if ca.hyperlink else None, cb.hyperlink.target if cb.hyperlink else None),
                    ("comment", ca.comment.text.strip() if ca.comment else None, cb.comment.text.strip() if cb.comment else None),
                ]:
                    if x != y:
                        diff += 1
                        if diff <= 40:
                            losses.append(f"{name}!{ca.coordinate} {what}: {x!r} -> {y!r}")
        if sorted(map(str, a.merged_cells.ranges)) != sorted(map(str, b.merged_cells.ranges)):
            losses.append(f"{name}: merged {sorted(map(str, a.merged_cells.ranges))} -> {sorted(map(str, b.merged_cells.ranges))}")
        # a <col> element may span several columns (min..max): compare per column
        cols = lambda ws: {i: (d.width, bool(d.hidden)) for d in ws.column_dimensions.values() for i in range(d.min or 0, (d.max or 0) + 1)}
        ca_, cb_ = cols(a), cols(b)
        for i, (w, h) in ca_.items():
            w2, h2 = cb_.get(i, (None, False))
            if w and abs(w - (w2 or 0)) > 0.6:
                losses.append(f"{name}: column {i} width {w} -> {w2}")
            if h != h2:
                losses.append(f"{name}: column {i} hidden {h} -> {h2}")
        for k, d in a.row_dimensions.items():
            e = b.row_dimensions[k]
            if d.height and abs((d.height or 0) - (e.height or 0)) > 0.6:
                losses.append(f"{name}: row {k} height {d.height} -> {e.height}")
            if bool(d.hidden) != bool(e.hidden):
                losses.append(f"{name}: row {k} hidden {d.hidden} -> {e.hidden}")
        for what, x, y in [
            ("freeze panes", a.freeze_panes, b.freeze_panes),
            ("data validations", sorted((str(v.sqref), v.type, v.formula1) for v in a.data_validations.dataValidation),
                                 sorted((str(v.sqref), v.type, v.formula1) for v in b.data_validations.dataValidation)),
            ("conditional formats", sorted((str(r.sqref), tuple(x.type for x in r.rules)) for r in a.conditional_formatting),
                                    sorted((str(r.sqref), tuple(x.type for x in r.rules)) for r in b.conditional_formatting)),
            ("print area", a.print_area, b.print_area),
            ("orientation", a.page_setup.orientation or "portrait", b.page_setup.orientation or "portrait"),
            ("paper", a.page_setup.paperSize, b.page_setup.paperSize),
            ("header", a.oddHeader.center.text, b.oddHeader.center.text),
            ("footer", a.oddFooter.right.text, b.oddFooter.right.text),
            ("protection", a.protection.sheet, b.protection.sheet),
            ("tab colour", color(a.sheet_properties.tabColor), color(b.sheet_properties.tabColor)),
            ("images", len(a._images), len(b._images)),
            ("tables", sorted(a.tables.keys()), sorted(b.tables.keys())),
        ]:
            if x != y:
                losses.append(f"{name}: {what} {x!r} -> {y!r}")
    na, nb = sorted(A.defined_names.keys()), sorted(B.defined_names.keys())
    if not set(na) <= set(nb):
        losses.append(f"defined names {na} -> {nb}")
    counts.update(cells=cells, cell_property_differences=diff)
    if diff > 40:
        losses.append(f"... {diff - 40} more cell differences")


def pptx():
    from pptx import Presentation
    cmp_parts()
    A, B = Presentation(orig), Presentation(saved)
    if (A.slide_width, A.slide_height) != (B.slide_width, B.slide_height):
        losses.append(f"slide size {(A.slide_width, A.slide_height)} -> {(B.slide_width, B.slide_height)}")
    if len(A.slides) != len(B.slides):
        losses.append(f"slides {len(A.slides)} -> {len(B.slides)}")
    shapes = 0

    def desc(sh):
        d = {"type": str(sh.shape_type), "box": tuple(round((v or 0) / 12700) for v in (sh.left, sh.top, sh.width, sh.height))}
        if sh.has_text_frame:
            d["text"] = [[(r.text, r.font.bold, r.font.size) for r in p.runs] for p in sh.text_frame.paragraphs]
        if getattr(sh, "has_table", False):
            d["table"] = [[c.text for c in r.cells] for r in sh.table.rows]
        if getattr(sh, "has_chart", False):
            d["chart"] = (str(sh.chart.chart_type), [tuple(s.values) for s in sh.chart.plots[0].series])
        if sh.shape_type == 13:
            d["image"] = hashlib.sha256(sh.image.blob).hexdigest()[:12]
        return d

    for i, (sa, sb) in enumerate(zip(A.slides, B.slides), 1):
        da, db = [desc(s) for s in sa.shapes], [desc(s) for s in sb.shapes]
        shapes += len(da)
        if str(i) == skip:
            db = db[: len(da)]  # the edit appended one shape here
        if len(da) != len(db):
            losses.append(f"slide {i}: shapes {len(da)} -> {len(db)}")
        for j, (x, y) in enumerate(zip(da, db)):
            for k in x:
                if x[k] != y.get(k):
                    losses.append(f"slide {i} shape {j + 1} {k}: {x[k]!r} -> {y.get(k)!r}")
        na = sa.notes_slide.notes_text_frame.text.strip() if sa.has_notes_slide else ""
        nb = sb.notes_slide.notes_text_frame.text.strip() if sb.has_notes_slide else ""
        if na != nb:
            losses.append(f"slide {i}: notes {na!r} -> {nb!r}")
        if sa.slide_layout.name != sb.slide_layout.name:
            notes.append(f"slide {i}: layout name {sa.slide_layout.name!r} -> {sb.slide_layout.name!r}")
    counts.update(slides=len(A.slides), shapes=shapes)


def pdf():
    import pypdf
    a, b = open(orig, "rb").read(), open(saved, "rb").read()
    counts.update(bytes_original=len(a), bytes_saved=len(b), original_is_prefix=b.startswith(a))
    if not b.startswith(a):
        notes.append("not an incremental update: the original bytes are not a prefix of the saved file")
    A, B = pypdf.PdfReader(orig), pypdf.PdfReader(saved)
    if len(A.pages) != len(B.pages):
        losses.append(f"pages {len(A.pages)} -> {len(B.pages)}")
    for i, (pa, pb) in enumerate(zip(A.pages, B.pages), 1):
        if [round(float(v)) for v in pa.mediabox] != [round(float(v)) for v in pb.mediabox] or pa.rotation != pb.rotation:
            losses.append(f"page {i}: box/rotation changed")
        if (pa.extract_text() or "") != (pb.extract_text() or ""):
            losses.append(f"page {i}: text differs")
        ka, kb = set(pa.keys()) - {"/Annots"}, set(pb.keys()) - {"/Annots"}
        if not ka <= kb:
            losses.append(f"page {i}: page keys lost {sorted(ka - kb)}")
        aa = [str(x.get_object().get("/Subtype")) for x in pa.get("/Annots", []) or []]
        ab = [str(x.get_object().get("/Subtype")) for x in pb.get("/Annots", []) or []]
        if Counter(aa) - Counter(ab):
            losses.append(f"page {i}: annotations lost {dict(Counter(aa) - Counter(ab))}")
        counts.setdefault("annotations_added", 0)
        counts["annotations_added"] += len(ab) - len(aa)
    fa, fb = A.get_fields() or {}, B.get_fields() or {}
    for k, v in fa.items():
        if k not in fb or fb[k].get("/V") != v.get("/V"):
            losses.append(f"form field {k!r}: {v.get('/V')!r} -> {fb.get(k, {}).get('/V')!r}")

    def sig(f):
        return [(k, list(v["/V"].get_object().get("/ByteRange", []))) for k, v in f.items() if v.get("/FT") == "/Sig" and v.get("/V")]

    if sig(fa) != sig(fb):
        losses.append(f"signatures {sig(fa)} -> {sig(fb)}")
    ka, kb = set(A.trailer["/Root"].keys()), set(B.trailer["/Root"].keys())
    if not ka <= kb:
        losses.append(f"catalog keys lost {sorted(ka - kb)}")
    counts.update(pages=len(A.pages), fields=len(fa), signatures=len(sig(fa)))


{"xlsx": xlsx, "pptx": pptx, "pdf": pdf}[kind]()
print(json.dumps({"losses": losses, "notes": notes, "counts": counts}, ensure_ascii=False))
