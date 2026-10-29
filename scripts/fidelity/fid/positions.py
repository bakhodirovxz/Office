"""Text position metrics for PDF -> DOCX (where the words land when the docx is printed again).

Words of the source PDF (truth) are matched, in order, to the words of a PDF printed from the converted docx (our Docs,
Word or LibreOffice). Each matched word gives a vertical displacement (its baseline, exact); the first word of each source
row gives the horizontal one (where the row starts). Inside a text-show group a word's x is placed by its share of the
characters, so x is only judged at row starts and in the overlap test. pypdf (BSD) reads the text: it is measurement
tooling only, imported here and nowhere in the product.

  python -I scripts/fidelity/fid/positions.py truth.pdf printed.pdf [--json out.json] [--pages]
"""
import bisect, difflib, json, re, statistics, sys, unicodedata

MOVED_PT = 3.0          # a word (or a row start) more than this far from its source position counts as moved
LINE_PT = 1.5           # words whose baselines differ by less than this are on one line


def _norm(w):
    w = unicodedata.normalize("NFKC", w).casefold()
    w = re.sub("[ʻʼ‘’“”\"'`«»]", "", w)
    return re.sub(r"[^\w]+", "", w)


def words(pdf):
    """-> [(page index, page height, [(token, x, baseline from the top, size, width)])], rotated text left out."""
    import pypdf
    from pypdf.generic import ContentStream
    from pypdf._text_extraction._layout_mode import _fixed_width_page as layout
    r = pypdf.PdfReader(pdf)
    out = []
    for i, pg in enumerate(r.pages):
        h = float(pg.mediabox.height)
        ws = []
        contents = pg.get("/Contents")
        if contents is not None:
            ops = iter(ContentStream(contents.get_object(), r, "bytes").operations)
            fonts, state, groups = pg._layout_mode_fonts(), layout.TextStateManager(), []
            # layout.text_show_operations without its last step, which shifts every x by the page's leftmost text
            for operands, op in ops:
                if op in (b"BT", b"q"):
                    groups += layout.recurse_to_target_op(ops, state, b"ET" if op == b"BT" else b"Q", fonts, True)[0]
                elif op == b"Tf":
                    state.set_font(layout.resolve_font(fonts, operands[0]), operands[1])
                else:
                    state.set_state_param(op, operands)
            for g in groups:
                t, x0, x1 = g["text"], float(g["tx"]), float(g["displaced_tx"])
                n = max(len(t), 1)
                for m in re.finditer(r"\S+", t):
                    tok = _norm(m.group())
                    if tok:  # interpolate inside the text-show group by character offset
                        a, b = x0 + (x1 - x0) * m.start() / n, x0 + (x1 - x0) * m.end() / n
                        ws.append((tok, a, h - float(g["ty"]), float(g["font_height"] or g["font_size"] or 10), b - a))
        ws.sort(key=lambda w: (round(w[2] / LINE_PT), w[1]))
        out.append((i, h, ws))
    return out


def _lines(ws):
    """Line id per word: consecutive words (in reading order) on one baseline."""
    ids, cur, last = [], -1, None
    for w in ws:
        if last is None or abs(w[2] - last) >= LINE_PT:
            cur += 1
        ids.append(cur)
        last = w[2]
    return ids


def _overlaps(ws):
    """How many words overlap another word's box on the same page (text drawn over text)."""
    boxes = sorted((w[1], w[1] + w[4], w[2] - w[3] * 0.7, w[2]) for w in ws if w[4] > 0)
    starts = [b[0] for b in boxes]
    hit = set()
    for i, (x0, x1, y0, y1) in enumerate(boxes):
        for j in range(i + 1, bisect.bisect_left(starts, x1 - 0.5, i + 1)):
            u0, u1, v0, v1 = boxes[j]
            if min(x1, u1) - max(x0, u0) > 0.5 and min(y1, v1) - max(y0, v0) > 0.5 * min(y1 - y0, v1 - v0):
                hit.update((i, j))
    return len(hit)


def _pct(v, q):
    if not v:
        return None
    v = sorted(v)
    return round(v[min(len(v) - 1, int(q * len(v)))], 2)


def compare(truth_pdf, cand_pdf):
    """Per page and in total: matched share; |dy| median and p90 and the share of words moved more than MOVED_PT up or down;
    the same for where each source row starts (row_dx_*, rows_shifted); source rows split over several printed rows and
    printed rows that merge several source rows; words drawn over another word in the printed docx; page counts."""
    T, C = words(truth_pdf), words(cand_pdf)
    tw = [(p, h, w, li) for p, h, ws in T for w, li in zip(ws, _lines(ws))]
    cw = [(p, h, w, li) for p, h, ws in C for w, li in zip(ws, _lines(ws))]
    sm = difflib.SequenceMatcher(None, [w[2][0] for w in tw], [w[2][0] for w in cw], autojunk=False)
    pairs = [(tw[i + k], cw[j + k]) for i, j, n in sm.get_matching_blocks() for k in range(n)]
    pages = {}
    for p, h, ws in T:
        pages[p] = {"words": len(ws), "dx": [], "dy": [], "tl": {}, "cl": {}}
    for (tp, th, t, tl), (cp, ch, c, cl) in pairs:
        r = pages[tp]
        r["dy"].append(abs((cp - tp) * th + (c[2] - t[2])))
        if tl not in r["tl"]:  # the source row's first matched word: where the row starts
            r["dx"].append(abs(c[1] - t[1]))
        r["tl"].setdefault(tl, set()).add((cp, cl))
        r["cl"].setdefault((cp, cl), set()).add(tl)
    over = {p: _overlaps(ws) for p, _, ws in C}

    def summary(rs, words_c, overlap):
        d = [x for r in rs for x in r["dy"]]
        dx = [x for r in rs for x in r["dx"]]
        n = sum(r["words"] for r in rs)
        tl = [v for r in rs for v in r["tl"].values()]
        cl = [v for r in rs for v in r["cl"].values()]
        return {
            "words": n, "matched": round(len(d) / n, 3) if n else None,
            "dy_med": _pct(d, 0.5), "dy_p90": _pct(d, 0.9), "moved": round(sum(x > MOVED_PT for x in d) / len(d), 3) if d else None,
            "row_dx_med": _pct(dx, 0.5), "row_dx_p90": _pct(dx, 0.9),
            "rows_shifted": round(sum(x > MOVED_PT for x in dx) / len(dx), 3) if dx else None,
            "lines_split": round(sum(len(v) > 1 for v in tl) / len(tl), 3) if tl else None,
            "lines_merged": round(sum(len(v) > 1 for v in cl) / len(cl), 3) if cl else None,
            "overlap_words": round(overlap / words_c, 3) if words_c else None,
        }

    per = []
    for p in sorted(pages):
        cws = len(C[p][2]) if p < len(C) else 0
        per.append(dict(page=p + 1, **summary([pages[p]], cws, over.get(p, 0))))
    total = summary(list(pages.values()), sum(len(ws) for _, _, ws in C), sum(over.values()))
    total.update(pages_truth=len(T), pages_out=len(C))
    return {"total": total, "pages": per}


if __name__ == "__main__":
    a = sys.argv[1:]
    res = compare(a[0], a[1])
    if "--json" in a:
        open(a[a.index("--json") + 1], "w", encoding="utf-8").write(json.dumps(res, indent=1))
    print(json.dumps(res["total"]))
    if "--pages" in a:
        for r in res["pages"]:
            print(json.dumps(r))
