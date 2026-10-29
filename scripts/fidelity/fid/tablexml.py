"""Counts of the table markup in a docx body (ECMA-376 Part 1, 17.4), for the round-trip check of scripts/fidelity/tables.py."""
import collections, zipfile

W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
# element local name -> its attributes that are part of the key, so a changed width counts as a loss
VALUED = {"gridCol": ("w",), "tcW": ("w", "type"), "tblW": ("w", "type"), "tblInd": ("w", "type"), "trHeight": ("val", "hRule"),
          "gridSpan": ("val",), "vMerge": ("val",), "tblLayout": ("type",), "jc": ("val",), "textDirection": ("val",),
          "tblCellSpacing": ("w", "type")}
COUNTED = ("tbl", "tr", "tc", "tblGrid", "tblBorders", "tcBorders", "insideH", "insideV", "shd", "tblStyle", "tblLook", "tblpPr",
           "tblCellMar", "tcMar", "tblHeader", "cantSplit", "vAlign", "noWrap", "hideMark", "tblOverlap")
# Only the table-level copy counts: Word repeats these in every w:trPr (or w:tblPrEx) with the same meaning.
TABLE_LEVEL = {"jc", "tblCellSpacing"}
SIDES = ("top", "left", "bottom", "right", "start", "end", "insideH", "insideV")


def counts(path):
    import xml.etree.ElementTree as ET
    with zipfile.ZipFile(path) as z:
        root = ET.fromstring(z.read("word/document.xml"))
    c = collections.Counter()
    for tbl in root.iter(W + "tbl"):
        for parent in tbl.iter():
            for el in parent:
                name = el.tag.replace(W, "")
                if name in TABLE_LEVEL and parent.tag != W + "tblPr":
                    continue
                if name in COUNTED:
                    c[name] += 1
                if name in VALUED:
                    c[name + ":" + "/".join(str(el.get(W + k)) for k in VALUED[name])] += 1
                if name in SIDES and el.get(W + "val") is not None:
                    c["border:" + el.get(W + "val") + "/" + str(el.get(W + "sz"))] += 1
    return dict(sorted(c.items()))


# Spellings with one meaning (ECMA-376 Part 1): ST_Border "none" and "nil" are both no border; a w:trHeight without w:hRule is
# "atLeast".
SAME = (("border:none/", "border:nil/"), ("/None", "/atLeast"))


def losses(before, after):
    """keys whose count dropped: {key: [before, after]}, after the spellings in SAME are matched up. Paragraph-level elements are
    not counted, so a pure re-ordering is no loss."""
    lost = {}
    for k, v in before.items():
        got = after.get(k, 0)
        for a, b in SAME:
            if a in k:
                got += after.get(k.replace(a, b), 0)
        if got < v:
            lost[k] = [v, after.get(k, 0)]
    return lost
