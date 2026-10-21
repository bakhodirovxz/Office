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


def _rtf_tables(t):
    """Tables of an RTF. A table starts at the first paragraph marked `\\intbl` (or the first `\\trowd`) and ends at the first
    ordinary paragraph (one with no `\\intbl`) after it: Word writes a row definition before some rows and not before others."""
    n, in_table, para_intbl = 0, False, False
    for m in re.finditer(r"\\pard(?![a-z])|\\intbl(?![a-z])|\\trowd(?![a-z])|\\par(?![a-z])", t):
        w = m.group(0)
        if w == "\\pard":
            para_intbl = False
        elif w == "\\par":
            if not para_intbl:
                in_table = False
        else:
            para_intbl = True
            if not in_table:
                n, in_table = n + 1, True
    return n


def _rtf_header_groups_with_text(t):
    """Number of {\\header*} / {\\footer*} groups that hold visible characters (Word writes empty first-page variants)."""
    n = 0
    for m in re.finditer(r"\{\\(?:header|footer)", t):
        depth, i, skip, text = 0, m.start(), None, False
        while i < len(t) and not text:
            c = t[i]
            if c == "{":
                depth += 1
                if skip is None and t.startswith("{\\*", i): skip = depth - 1
            elif c == "}":
                depth -= 1
                if skip == depth: skip = None
                if depth == 0: break
            elif c == "\\":
                w = re.match(r"\\([a-zA-Z]+)(-?\d+)? ?", t[i:])
                if w:
                    text = skip is None and w.group(1) == "u" and w.group(2) is not None
                    i += w.end(); continue
                text = skip is None and t.startswith("\\'", i)
                i += 1
            elif skip is None and c.isalnum():
                text = True
            i += 1
        n += text
    return n


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
                d["merged"] = _n(r"<w:gridSpan", body) + _n(r'<w:vMerge w:val="restart"', body)
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
            d["paragraphs"] = _n(r"\\par[ \\\r\n]", t)
            d["tables"] = _rtf_tables(t)
            # Word writes every picture twice (shppict + nonshppict); one comment is an annotation group plus its atrfstart mark
            d["images"] = _n(r"\\shppict", t) or _n(r"\\pict", t); d["comments"] = _n(r"\{\\\*\\annotation", t)
            d["revisions"] = _n(r"\\revised|\\deleted", t); d["footnotes"] = _n(r"\\footnote", t)
            d["headers_footers"] = _rtf_header_groups_with_text(t)
            d["merged"] = _n(r"\\clmgf|\\clvmgf", t)
        elif ext in ("html", "htm"):
            t = p.read_text("utf-8", "replace")
            d["paragraphs"] = _n(r"<p[ >]", t); d["tables"] = _n(r"<table[ >]", t); d["images"] = _n(r"<img[ >]", t)
            d["merged"] = len([v for v in re.findall(r"(?:colspan|rowspan)=[\"']?(\d+)", t, re.I) if int(v) > 1])
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


# RTF has no merge marker for a horizontal merge (the writer widens one cell), so merged cells cannot be counted in an RTF output
UNCOUNTABLE = {"rtf": ("merged",)}
RELEVANT = {"csv": ["sheets", "formulas"]}   # a value-only target can only lose these; charts etc. are inherent


def structural_losses(src, out, target=None):
    """Features present in the source and fewer in the output -> [{feature, before, after}]."""
    if not src or not out:
        return []
    skip = UNCOUNTABLE.get(out.get("kind"), ())
    return [{"feature": k, "before": src[k], "after": out.get(k, 0)} for k in RELEVANT.get(target, FEATURES) if k not in skip and src.get(k, 0) > out.get(k, 0)]


# Text, pagination and Cyrillic differences are consequences of something else being lost or reformatted (a header, a chart, a number
# format, a picture), not features of their own. A warning explains one only when it NAMES that kind of loss: the codes below
# (data/losses.toml and the job codes of services/convert), or an engine message (`upstream`) that says content was dropped. Any other
# warning on the same case explains nothing, so a second, unwarned text loss on a file that already carries a warning stays silent.
_TEXT = {"headers_footers_dropped", "footnotes_dropped", "comments_dropped", "revisions_dropped", "plain_text_layout", "tables_to_text",
         "merged_cells_flattened", "doc_import_loss", "xls_pdf_chart_page", "pdf_en_us", "csv_one_sheet_en_us", "pdf_text_missing",
         "empty_text", "pdf_damaged"}
NAMES = {
    "text": _TEXT,
    "pagination": {"headers_footers_dropped", "footnotes_dropped", "comments_dropped", "html_images_not_embedded", "doc_import_loss",
                   "xls_pdf_chart_page", "plain_text_layout", "tables_to_text", "pdf_text_missing", "empty_text", "pdf_damaged"},
    "cyrillic": _TEXT - {"pdf_en_us", "csv_one_sheet_en_us"},
}
DERIVED = set(NAMES)
UPSTREAM_DROP = re.compile(r"not imported|not supported|unsupported|ignored|dropped|lost|skipp|not kept|not read|cannot|could not", re.I)
NOT_A_LOSS = {"kind_mismatch", "hidden_sheet_exported", "very_hidden_sheet_skipped", "xdfu_copy", "xdfu_refused", "xdfu_no_watermark",
              "xdfu_cannot_check", "translit_nothing", "translit_formula_text", "text_transcoded"}


def names_loss(w, feature):
    """Does warning `w` name a loss of this derived kind (text, pagination, Cyrillic)?"""
    return w.get("code") in NAMES[feature] or (w.get("code") == "upstream" and bool(UPSTREAM_DROP.search(w.get("text", ""))))


def silent_losses(losses, warnings):
    """Losses with no matching warning. warnings: [{code,text}]. This is the 'loss without warning is a bug' list."""
    warnings = [w for w in warnings if w.get("code") not in NOT_A_LOSS]
    blob = " ".join("%s %s" % (w.get("code", ""), w.get("text", "")) for w in warnings).lower()
    return [l for l in losses if not (any(names_loss(w, l["feature"]) for w in warnings) if l["feature"] in DERIVED
                                      else re.search(COVER.get(l["feature"], l["feature"]), blob))]
