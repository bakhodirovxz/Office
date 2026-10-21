"""PDF -> DOCX metrics (PLAN 3.4). Truth comes from the Word-made source docx (same stem as the PDF) and <stem>.truth.json."""
import json, re, zipfile
from pathlib import Path
from .util import norm_text, text_metrics

TXBX = re.compile(r"<w:txbxContent>.*?</w:txbxContent>|<v:textbox.*?</v:textbox>", re.S)


def docx_info(path):
    """paragraphs, tables, text, first-table column widths (pt), share of paragraphs inside text boxes."""
    with zipfile.ZipFile(path) as z:
        body = z.read("word/document.xml").decode("utf-8", "replace")
    boxes = TXBX.findall(body)
    in_box = sum(len(re.findall(r"<w:p[ >]", b)) for b in boxes)
    total = len(re.findall(r"<w:p[ >]", body))
    text = "\n".join("".join(re.findall(r"<w:t(?: [^>]*)?>([^<]*)</w:t>", p)) for p in re.findall(r"<w:p[ >].*?</w:p>", body, re.S))
    text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", '"').replace("&apos;", "'")
    grids = [[int(w) / 20 for w in re.findall(r'<w:gridCol w:w="(\d+)"', g)] for g in re.findall(r"<w:tblGrid>.*?</w:tblGrid>", body, re.S)]
    return {"paragraphs": total, "tables": len(re.findall(r"<w:tbl>", body)), "text": text, "grids": grids,
            "textbox_paragraphs": in_box, "textbox_share": round(in_box / total, 3) if total else 0.0}


def width_error(src_grids, out_grids):
    """Mean relative error of the column width fractions of the first source table, vs the best output table with the same column count."""
    if not src_grids:
        return None
    s = src_grids[0]; ss = sum(s) or 1
    best = None
    for g in out_grids:
        if len(g) == len(s) and sum(g):
            e = sum(abs(a / sum(g) - b / ss) / (b / ss) for a, b in zip(g, s)) / len(s)
            best = e if best is None else min(best, e)
    return None if best is None else round(best, 3)


def p2d_metrics(src_docx, out_docx, truth_json=None):
    src, out = docx_info(src_docx), docx_info(out_docx)
    truth = json.loads(Path(truth_json).read_text(encoding="utf-8-sig")) if truth_json and Path(truth_json).exists() else {}
    t = text_metrics(src["text"], out["text"])
    known = {w: (w.casefold() in out["text"].casefold()) for w in (truth.get("known_words") or [])}
    return {
        "paragraphs_src": src["paragraphs"], "paragraphs_out": out["paragraphs"],
        "paragraph_ratio": round(out["paragraphs"] / src["paragraphs"], 3) if src["paragraphs"] else None,
        "tables_src": src["tables"], "tables_out": out["tables"], "tables_kept": min(out["tables"], src["tables"]),
        "text_sim": t["word_sim"], "cyr_kept": t["cyr_kept"], "missing_words": t["missing_words"],
        "known_words": known, "col_width_err": width_error(src["grids"], out["grids"]), "col_count_out": [len(g) for g in out["grids"]][:5],
        "textbox_share": out["textbox_share"],
    }
