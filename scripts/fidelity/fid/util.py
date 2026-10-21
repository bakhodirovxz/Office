"""Shared helpers: process runner, rasteriser (pdf-cli, 96 dpi like Phase 0), text and pixel metrics."""
import collections, difflib, re, subprocess, time, unicodedata
from pathlib import Path

TOOLS = Path("E:/office-target/p1")
CLI = {"docs": TOOLS / "docs/release/docs-cli.exe", "sheets": TOOLS / "sheets/release/sheets-cli.exe",
       "slides": TOOLS / "slides/release/slides-cli.exe", "pdf": TOOLS / "pdf/release/pdf-cli.exe"}
DPI = 96
MAX_PAGES = 30          # pixel diffs look at the first pages only (as Phase 0)
CYR = re.compile("[\u0400-\u04FF]")


def run(cmd, timeout=180, cwd=None, env=None):
    """-> (rc, stdout, stderr, seconds). rc 124 = timeout. Never raises."""
    t0 = time.time()
    try:
        p = subprocess.run([str(c) for c in cmd], capture_output=True, timeout=timeout, cwd=cwd, env=env)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace"), time.time() - t0
    except subprocess.TimeoutExpired:
        return 124, "", "timeout", time.time() - t0
    except OSError as e:
        return 127, "", str(e), time.time() - t0


def pdf_pages(pdf):
    rc, o, _, _ = run([CLI["pdf"], "info", pdf], 60)
    try:
        return int(__import__("json").loads(o)["pages"])
    except Exception:
        return 0


def pdf_text(pdf):
    rc, o, _, _ = run([CLI["pdf"], "text", pdf], 120)
    return o if rc == 0 else ""


def rasterise(pdf, png_dir, tag):
    """-> (pages, [png or None] for the first MAX_PAGES)."""
    png_dir.mkdir(parents=True, exist_ok=True)
    n = pdf_pages(pdf)
    out = []
    for p in range(1, min(n, MAX_PAGES) + 1):
        f = png_dir / ("%s-p%02d.png" % (tag, p))
        if not f.exists():
            run([CLI["pdf"], "render", pdf, "--page", p, "--dpi", DPI, "--out", f], 60)
        out.append(f if f.exists() else None)
    return n, out


def norm_text(t):
    t = unicodedata.normalize("NFKC", t).replace("\x0c", " ").casefold()
    for ch in "\u02bb\u02bc\u2019\u2018\u02b9\u0027":
        t = t.replace(ch, "'")
    return re.sub(r"\s+", " ", t).strip()


def text_metrics(ref_txt, cand_txt):
    """Word-level similarity (difflib ratio), counts and the first missing/extra words; cyrillic letters kept."""
    a, b = norm_text(ref_txt), norm_text(cand_txt)
    wa, wb = re.findall(r"\w+", a), re.findall(r"\w+", b)
    cr, cc = len(CYR.findall(ref_txt)), len(CYR.findall(cand_txt))
    if not wa and not wb:
        return {"word_sim": 1.0, "ref_words": 0, "cand_words": 0, "missing_count": 0, "missing_words": [],
                "cyr_ref": cr, "cyr_cand": cc, "cyr_kept": 1.0 if cr == 0 else None}
    sim = difflib.SequenceMatcher(None, wa, wb, autojunk=False).ratio()
    ca, cb = collections.Counter(wa), collections.Counter(wb)
    miss = list((ca - cb).elements())
    # the letters and digits of the reference that the candidate still has (order and spaces ignored): a PDF text extractor glues words
    # that are separated by a tab, and reads tables in another order, but it never makes text up
    ka, kb = collections.Counter("".join(wa)), collections.Counter("".join(wb))
    kept = sum((ka & kb).values()) / sum(ka.values()) if ka else 1.0
    return {"word_sim": round(sim, 4), "ref_words": len(wa), "cand_words": len(wb), "missing_count": len(miss), "chars_kept": round(kept, 4),
            "missing_words": miss[:10], "cyr_ref": cr, "cyr_cand": cc,
            "cyr_kept": round(min(cc, cr) / cr, 4) if cr else None}


def page_diff(ref_png, cand_png):
    """Percent of pixels whose blurred grey value differs by > 48 (Phase 0 method); cand is resized to ref."""
    import numpy as np
    from PIL import Image, ImageFilter
    A = Image.open(ref_png).convert("L"); B = Image.open(cand_png).convert("L")
    same = A.size == B.size
    if not same:
        B = B.resize(A.size)
    a = np.asarray(A.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    b = np.asarray(B.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    return round(float((np.abs(a - b) > 48).mean() * 100), 3), same


def pix_metrics(ref_pngs, cand_pngs):
    pix = []
    for r, c in zip(ref_pngs, cand_pngs):
        if r and c:
            pix.append(page_diff(r, c)[0])
    return {"pix_pages": pix, "pix_mean": round(sum(pix) / len(pix), 3) if pix else None, "pix_max": max(pix) if pix else None}


def visual_metrics(ref_pdf, cand_pdf, work, tag):
    """Page count, text similarity and per-page pixel diff of two PDFs (same rasteriser for both)."""
    rn, rp = rasterise(ref_pdf, work, "ref-" + Path(ref_pdf).stem)
    cn, cp = rasterise(cand_pdf, work, tag)
    m = {"pages_ref": rn, "pages_cand": cn, "page_match": rn == cn}
    m.update(text_metrics(pdf_text(ref_pdf), pdf_text(cand_pdf)))
    m.update(pix_metrics(rp, cp))
    return m
