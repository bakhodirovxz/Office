"""Structural counts straight from the file bytes (zip + regex, no Office), the same keys for every format, so a
source and a converted output can be compared. Also the "loss without warning" check."""
import re, zipfile
from pathlib import Path

FEATURES = ["tables", "images", "comments", "revisions", "footnotes", "headers_footers", "charts", "cond_formats",
            "formulas", "sheets", "slides", "notes", "merged"]
# a feature loss counts as "warned" when some warning (code + text) matches its pattern
COVER = {
    "tables": r"table", "images": r"image|picture|graphic|media|shape", "comments": r"comment", "revisions": r"track|revision|change",
    "footnotes": r"footnote|endnote", "headers_footers": r"header|footer", "charts": r"chart", "cond_formats": r"conditional|format",
    "formulas": r"formula|values", "sheets": r"sheet|worksheet", "slides": r"slide", "notes": r"note", "merged": r"merge|format",
    "text": r"text|font|glyph|ocr|scan|encoding", "pagination": r"page|layout|paginat", "cyrillic": r"cyril|font|glyph|encoding",
}


def _n(pat, s, flags=0):
    return len(re.findall(pat, s, flags))


def _zip_text(z, pat):
    return "".join(z.read(n).decode("utf-8", "replace") for n in z.namelist() if re.fullmatch(pat, n))


def _blank():
    d = {k: 0 for k in FEATURES}
    d["paragraphs"] = 0
    return d


def structure(path):
    """-> dict with FEATURES + paragraphs (+ 'kind'), or None if the format is not understood."""
    p = Path(path)
    ext = p.suffix.lower().lstrip(".")
    d = _blank(); d["kind"] = ext
    try:
        if ext in ("docx", "docm", "dotx", "dotm"):
            with zipfile.ZipFile(p) as z:
                body = z.read("word/document.xml").decode("utf-8", "replace")
                names = z.namelist()
                d["paragraphs"] = _n(r"<w:p[ >]", body); d["tables"] = _n(r"<w:tbl>", body)
                d["images"] = len([n for n in names if n.startswith("word/media/")])
                d["comments"] = _n(r"<w:comment ", _zip_text(z, r"word/comments\.xml"))
                d["revisions"] = _n(r"<w:(?:ins|del) ", body)
                d["footnotes"] = _n(r"<w:footnote (?![^>]*w:type=)", _zip_text(z, r"word/footnotes\.xml"))
                d["headers_footers"] = len([n for n in names if re.fullmatch(r"word/(header|footer)\d*\.xml", n)
                                            and "<w:t" in z.read(n).decode("utf-8", "replace")])
                d["charts"] = len([n for n in names if re.fullmatch(r"word/charts/chart\d*\.xml", n)])
                d["merged"] = _n(r"<w:(?:gridSpan|vMerge)", body)
        elif ext in ("xlsx", "xlsm", "xltx"):
            with zipfile.ZipFile(p) as z:
                names = z.namelist(); sh = _zip_text(z, r"xl/worksheets/sheet\d+\.xml")
                d["sheets"] = _n(r"<sheet ", z.read("xl/workbook.xml").decode("utf-8", "replace"))
                d["formulas"] = _n(r"<f[ >]", sh); d["cond_formats"] = _n(r"<conditionalFormatting", sh)
                d["merged"] = _n(r"<mergeCell ", sh)
                d["charts"] = len([n for n in names if re.fullmatch(r"xl/charts/chart\d*\.xml", n)])
                d["images"] = len([n for n in names if n.startswith("xl/media/")])
                d["comments"] = _n(r"<comment ", _zip_text(z, r"xl/comments\d*\.xml"))
                d["paragraphs"] = _n(r"<row ", sh)
        elif ext in ("pptx", "ppsx", "potx"):
            with zipfile.ZipFile(p) as z:
                names = z.namelist(); sl = _zip_text(z, r"ppt/slides/slide\d+\.xml")
                d["slides"] = len([n for n in names if re.fullmatch(r"ppt/slides/slide\d+\.xml", n)])
                d["notes"] = len([n for n in names if re.fullmatch(r"ppt/notesSlides/notesSlide\d+\.xml", n)])
                d["tables"] = _n(r"<a:tbl>", sl); d["paragraphs"] = _n(r"<a:p>", sl)
                d["images"] = len([n for n in names if n.startswith("ppt/media/")])
                d["charts"] = len([n for n in names if re.fullmatch(r"ppt/charts/chart\d*\.xml", n)])
                d["comments"] = len([n for n in names if re.fullmatch(r"ppt/comments/.*\.xml", n)])
        elif ext in ("odt", "ods"):
            with zipfile.ZipFile(p) as z:
                c = z.read("content.xml").decode("utf-8", "replace"); st = z.read("styles.xml").decode("utf-8", "replace") if "styles.xml" in z.namelist() else ""
                d["paragraphs"] = _n(r"<text:p[ >]", c); d["tables"] = _n(r"<table:table ", c) if ext == "odt" else 0
                d["sheets"] = _n(r"<table:table ", c) if ext == "ods" else 0
                d["images"] = _n(r"<draw:image", c); d["comments"] = _n(r"<office:annotation[ >]", c)
                d["revisions"] = _n(r"<text:changed-region", c); d["footnotes"] = _n(r"<text:note ", c)
                d["headers_footers"] = _n(r"<style:(?:header|footer)>", st); d["charts"] = _n(r"<chart:chart", c)
                d["formulas"] = _n(r"table:formula=", c); d["merged"] = _n(r"number-(?:columns|rows)-spanned", c)
        elif ext == "rtf":
            t = p.read_bytes().decode("latin-1")
            d["paragraphs"] = _n(r"\\par[ \\\r\n]", t); d["tables"] = _n(r"(?:\\trowd.*?\\row\s*)+", t, re.S)
            d["images"] = _n(r"\\pict", t); d["comments"] = _n(r"\\atrfstart|\{\\\*\\annotation", t)
            d["revisions"] = _n(r"\\revised|\\deleted", t); d["footnotes"] = _n(r"\\footnote", t)
            d["headers_footers"] = _n(r"\{\\(?:header|footer)", t)
        elif ext in ("html", "htm"):
            t = p.read_text("utf-8", "replace")
            d["paragraphs"] = _n(r"<p[ >]", t); d["tables"] = _n(r"<table[ >]", t); d["images"] = _n(r"<img[ >]", t)
            d["comments"] = 0
        elif ext == "md":
            t = p.read_text("utf-8", "replace")
            d["paragraphs"] = len([b for b in re.split(r"\n\s*\n", t) if b.strip()])
            d["tables"] = _n(r"(?m)^\|[ -:|]+\|\s*$", t); d["images"] = _n(r"!\[", t)
        elif ext in ("txt", "csv", "tsv"):
            t = p.read_text("utf-8", "replace")
            d["paragraphs"] = len([l for l in t.splitlines() if l.strip()])
            if ext != "txt":
                d["sheets"] = 1
        elif ext == "pdf":
            return d
        else:
            return None
    except (OSError, KeyError, zipfile.BadZipFile) as e:
        d["error"] = str(e)[:100]
    return d


RELEVANT = {"csv": ["sheets", "formulas"]}   # a value-only target can only lose these; charts etc. are inherent


def structural_losses(src, out, target=None):
    """Features present in the source and fewer in the output -> [{feature, before, after}]."""
    if not src or not out:
        return []
    return [{"feature": k, "before": src[k], "after": out.get(k, 0)} for k in RELEVANT.get(target, FEATURES) if src.get(k, 0) > out.get(k, 0)]


def silent_losses(losses, warnings):
    """Losses with no matching warning. warnings: [{code,text}]. This is the 'loss without warning is a bug' list."""
    blob = " ".join("%s %s" % (w.get("code", ""), w.get("text", "")) for w in warnings).lower()
    return [l for l in losses if not re.search(COVER.get(l["feature"], l["feature"]), blob)]
