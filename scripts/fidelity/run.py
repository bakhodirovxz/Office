#!/usr/bin/env python
"""Fidelity harness for conversion DIRECTIONS (PLAN phase 3, section 7 / WP9).

  python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity
  python -I scripts/fidelity/run.py --directions docs,pdf.pdf2docx --out ... [--adapter raw|convert] [--no-office] [--no-soffice]

For every direction in fid/directions.py and every synthetic fixture:
  craft    : the candidate, through `convert --json` (fid/adapter.py; falls back to the raw domain CLIs if no convert binary)
  office   : MS Office 16 COM doing the same conversion (reference, fid/refs.py + office_ref.ps1)
  soffice  : LibreOffice sidecar (POST /v1/convert, OOXML only)         -> "unavailable: <reason>" when not reachable
  x2t      : ONLYOFFICE document server /converter (X2T_URL)            -> "unavailable: <reason>" when not configured
Metrics per output: page count, word-level text similarity, per-page pixel diff vs the Office PDF of the source (pdf-cli
render, 96 dpi), paragraph ratio, structural counts kept/lost, Cyrillic letters kept, csv cell equality, PDF-op page
hashes, and for pdf -> docx the PLAN 3.4 metrics. Finally the "loss without warning" check: every measured structural,
text, page or Cyrillic loss of the craft output must be matched by a warning in the convert report.
Output: <out>/directions/<id>.json, <out>/summary.json (+ work files under <out>/work). Synthetic fixtures only.
Test tooling: needs pillow + numpy; never builds anything. Kills only the Office PID it started.
"""
import argparse, hashlib, json, os, re, subprocess, sys, time, urllib.request, csv, html
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))           # python -I drops the script dir from sys.path
from fid import structure as S                                   # noqa: E402
from fid.adapter import Adapter                                  # noqa: E402
from fid.directions import select                                # noqa: E402
from fid.p2d import docx_info, p2d_metrics                       # noqa: E402
from fid.refs import Soffice, X2T, app_of, office_batch          # noqa: E402
from fid.util import CLI, rasterise, pdf_pages, pdf_text, text_metrics, pix_metrics, visual_metrics, run  # noqa: E402

SRC_FIXTURE_DIR = None


def key(p):
    return str(p).replace("\\", "/")


def have(p):
    p = Path(p)
    return p.exists() and (p.is_dir() and any(p.iterdir()) or p.is_file() and p.stat().st_size > 0)


# ---------------------------------------------------------------- Office job queue (two rounds: sources/refs, then outputs -> pdf)
class OQ:
    def __init__(self, work, enabled, force):
        self.work, self.enabled, self.force = work, enabled, force
        self.jobs, self.res = {}, {}
        self.reason = None if enabled else "disabled (--no-office)"

    def need(self, src, fmt, out):
        out = key(out)
        Path(out).parent.mkdir(parents=True, exist_ok=True)   # Office will not create folders
        if not self.enabled or not app_of(src):
            return out
        if have(out) and not self.force:
            self.res[out] = {"ok": True, "error": None, "cached": True}
        else:
            self.jobs[out] = {"app": app_of(src), "src": key(src), "out": out, "fmt": fmt}
        return out

    def flush(self, tag):
        by = {}
        for j in self.jobs.values():
            by.setdefault(j["app"] + ("_pdf" if j["src"].lower().endswith(".pdf") else ""), []).append({k: v for k, v in j.items() if k != "app"})
        for app, jobs in by.items():
            print("[office:%s] %s: %d conversions" % (tag, app, len(jobs)), flush=True)
            pdf = app.endswith("_pdf")   # PDF reflow batch is separate: it is the one that hangs
            res = office_batch(app.split("_")[0], jobs, self.work, "%s_%s" % (tag, app), per_item=20 if pdf else 60)
            for v in res.values():
                if pdf and str(v.get("error", "")).endswith("timeout"):
                    v["error"] = "Word 16 PDF reflow via COM hangs headless (modal dialog, even with DisableConvertPdfWarning=1); killed after the batch timeout"
            self.res.update(res)
        self.jobs = {}

    def ok(self, out):
        r = self.res.get(key(out))
        return bool(r and r.get("ok") and have(out))

    def error(self, out):
        r = self.res.get(key(out)) or {}
        return r.get("error") or ("no result" if self.enabled else self.reason)


OFFICE_WRITES = {"word": {"pdf", "docx", "odt", "rtf", "html", "txt"}, "excel": {"pdf", "xlsx", "csv"}, "ppt": {"pdf", "pptx", "png"}}


def office_plan(d, fx):
    """-> (fmt or None, reason). What the Office reference does for this direction."""
    ext = Path(fx).suffix.lower().lstrip("."); app = app_of(fx)
    if d["kind"] in ("translit", "pdfop", "pdftext", "pdf2docx", "edge"):
        return None, "no Office equivalent for this direction kind"
    if ext == "md":
        return None, "Word has no Markdown reader"
    if ext == d["to"]:
        return None, "same format"
    if d["to"] not in OFFICE_WRITES.get(app, set()):
        return None, "%s cannot write %s" % (app, d["to"]) if d["to"] != "png" else "no per-page png export (reference = Office PDF pages)"
    return d["to"], None


def soffice_plan(d, fx):
    ext = Path(fx).suffix.lower().lstrip("."); to = d["to"]
    if d["kind"] in ("translit", "pdfop", "pdftext", "pdf2docx", "edge"):
        return None, "no LibreOffice equivalent for this direction kind"
    if ext not in ("docx", "xlsx", "pptx", "doc", "xls", "ppt"):
        return None, "sidecar takes OOXML (docx xlsx pptx) only"
    same = {"doc": "docx", "docx": "docx", "xls": "xlsx", "xlsx": "xlsx", "ppt": "pptx", "pptx": "pptx"}[ext]
    if to in ("pdf", "png") or to == same:
        return to, None
    return None, "sidecar converts to pdf/png or same-kind OOXML only (%s -> %s)" % (ext, to)


# ---------------------------------------------------------------- helpers for judging
def plain_text(path):
    p = Path(path); ext = p.suffix.lower()
    try:
        if ext in (".html", ".htm"):
            return html.unescape(re.sub(r"<(script|style).*?</\1>|<[^>]+>", " ", p.read_text("utf-8", "replace"), flags=re.S))
        return p.read_text("utf-8", "replace")
    except OSError:
        return ""


def csv_compare(ref, cand):
    def load(p):
        with open(p, encoding="utf-8-sig", newline="") as f:
            return list(csv.reader(f))
    a, b = load(ref), load(cand)
    cells = sum(len(r) for r in a); eq = 0; diffs = []
    for i, r in enumerate(a):
        for j, v in enumerate(r):
            w = b[i][j] if i < len(b) and j < len(b[i]) else None
            if v == w: eq += 1
            elif len(diffs) < 3: diffs.append({"cell": [i + 1, j + 1], "office": v[:30], "craft": (w or "")[:30]})
    return {"rows_ref": len(a), "rows_cand": len(b), "cells_ref": cells, "cells_equal": eq,
            "cell_equality": round(eq / cells, 4) if cells else 1.0, "first_diffs": diffs}


def page_hashes(pdf, work, tag):
    n, pngs = rasterise(Path(pdf), work, tag)
    return n, [hashlib.sha256(p.read_bytes()).hexdigest()[:12] if p else None for p in pngs]


def losses_of(src_s, out_s, vis, tmet, translit=False):
    L = S.structural_losses(src_s, out_s)
    if vis and vis.get("page_match") is False:
        a, b = vis["pages_ref"], vis["pages_cand"]
        # transliteration changes the text length, so a few pages more or less is expected (up to 10%)
        if not (translit and a and abs(a - b) <= 0.1 * max(a, b)):
            L.append({"feature": "pagination", "before": a, "after": b})
    for m in (vis, tmet):
        # a low word-order similarity alone is a reading-order difference (tables, text boxes), not lost text: words must be missing
        if m and m.get("word_sim") is not None and m["word_sim"] < 0.9 and m.get("missing_count", 1) >= max(3, 0.02 * (m.get("ref_words") or 0)):
            L.append({"feature": "text", "before": 1.0, "after": m["word_sim"]}); break
    for m in (vis, tmet):
        if m and m.get("cyr_kept") is not None and m["cyr_kept"] < 0.99:
            L.append({"feature": "cyrillic", "before": m.get("cyr_ref"), "after": m.get("cyr_cand")}); break
    return L


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--directions", default="all", help="all | group (docs sheets slides pdf) | comma list of ids")
    ap.add_argument("--out", required=True)
    ap.add_argument("--fixtures", help="default <out>/fixtures (generated on first use)")
    ap.add_argument("--adapter", default="auto", choices=["auto", "convert", "raw"])
    ap.add_argument("--convert-bin")
    ap.add_argument("--no-office", action="store_true"); ap.add_argument("--no-soffice", action="store_true"); ap.add_argument("--no-x2t", action="store_true")
    ap.add_argument("--force", action="store_true", help="redo cached Office conversions")
    ap.add_argument("--list", action="store_true", help="print the directions and exit")
    a = ap.parse_args()
    dirs = select(a.directions)
    if a.list:
        for d in dirs: print(d["id"], d["kind"], d["cases"])
        return
    out = Path(a.out).resolve()
    if not str(out).upper().startswith("E:"):
        sys.exit("--out must be on E: (generated binaries never go in the repo)")
    fx = Path(a.fixtures) if a.fixtures else out / "fixtures"
    if not (fx / "MANIFEST.json").exists():
        print("[fixtures] generating into", fx, flush=True)
        r = subprocess.run([sys.executable, "-I", str(HERE / "gen_fixtures.py"), "--out", str(fx)])
        if r.returncode: sys.exit("fixture generation failed")
    (out / "directions").mkdir(parents=True, exist_ok=True)
    work = out / "work"; work.mkdir(exist_ok=True)

    ad = Adapter(a.adapter, a.convert_bin)
    so, x2 = Soffice(disabled=a.no_soffice), X2T(disabled=a.no_x2t)
    oq = OQ(work, not a.no_office, a.force)
    p2d_url = (os.environ.get("CONVERT_PDF2DOCX_URL") or "http://127.0.0.1:18080").rstrip("/")
    try:
        with urllib.request.urlopen(p2d_url + "/healthz", timeout=4) as r: p2d_up = True
    except Exception as e:
        p2d_up = False; p2d_reason = "pdf2docx sidecar not reachable at %s (%s); start sidecars/pdf2docx/run.sh" % (p2d_url, type(e).__name__)
    print("adapter:", ad.describe(), "| soffice:", so.reason or "up", "| x2t:", x2.reason or "up", "| pdf2docx:", "up" if p2d_up else p2d_reason, flush=True)

    # ---------- round 0: build the case list, run craft / soffice / x2t, queue Office jobs
    cases = []
    for d in dirs:
        for name in d["cases"]:
            srcs = [fx / n for n in name.split("+")]
            c = {"d": d, "name": name, "srcs": srcs, "src": srcs[0], "w": work / d["id"] / re.sub(r"\W+", "_", name), "engines": {}}
            if not all(s.exists() for s in srcs):
                c["error"] = "fixture missing: " + ", ".join(s.name for s in srcs if not s.exists()); cases.append(c); continue
            c["w"].mkdir(parents=True, exist_ok=True)
            cases.append(c)
    for c in cases:
        if c.get("error"): continue
        d, kind, w, src = c["d"], c["d"]["kind"], c["w"], c["src"]
        t0 = time.time()
        # --- craft
        if kind == "pdfop":
            c["craft"] = ad.pdf_op(d["op"], [str(s) for s in c["srcs"]], w / "craft", d.get("arg"))
        elif kind == "pdf2docx":
            c["craft"] = ad.convert(src, "docx", w / "craft", pdf_engine="native")
            c["craft_sidecar"] = ad.convert(src, "docx", w / "craft_sidecar", pdf_engine="sidecar")
        elif kind == "edge":
            c["craft"] = ad.convert(src, "docx", w / "craft")
            c["craft_txt"] = ad.convert(src, "txt", w / "craft_txt")
        else:
            c["craft"] = ad.convert(src, d["to"], w / "craft", translit=d.get("translit"))
        print("[craft] %-26s %-28s %-6s code=%s %.2fs" % (d["id"], c["name"], c["craft"]["status"], c["craft"]["code"], time.time() - t0), flush=True)
        # --- Office source PDF (the reference for every pixel/page/text metric) + Office's own conversion
        stem = src.stem
        if app_of(src) and kind in ("pdf", "png", "edit", "text", "csv", "translit"):
            c["ref_pdf"] = oq.need(src, "pdf", work / "_ref" / (src.name + ".pdf")) if src.suffix.lower() != ".pdf" else None
        elif src.suffix.lower() == ".pdf":
            c["ref_pdf"] = key(src)
        fmt, why = office_plan(d, src.name)
        c["engines"]["office"] = {"status": "pending" if fmt else "unavailable", "reason": why or oq.reason}
        if fmt:
            ofile = w / "office" / ((stem + "_png") if fmt == "png" else (stem + "." + fmt))
            c["engines"]["office"]["out"] = oq.need(src, fmt, ofile)
            if kind == "text" or d["to"] in ("html", "md", "txt"):
                c["ref_txt"] = oq.need(src, "txt", work / "_ref" / (src.name + ".ref.txt")) if app_of(src) == "word" else None
        if kind == "csv" and not fmt:
            pass
        # --- soffice
        f2, why2 = soffice_plan(d, src.name)
        if f2:
            (w / "soffice").mkdir(exist_ok=True); o = w / "soffice" / (stem + "." + f2)
            ok, msg = so.convert(src, f2, o)
            c["engines"]["soffice"] = {"status": "ok" if ok else "unavailable", "reason": None if ok else msg, "out": key(o) if ok else None}
        else:
            c["engines"]["soffice"] = {"status": "unavailable", "reason": why2}
        # --- x2t
        if kind in ("pdf", "edit", "text", "csv", "pdf2docx") and not (d["to"] == "png"):
            (w / "x2t").mkdir(exist_ok=True); o = w / "x2t" / (stem + "." + d["to"])
            ok, msg = x2.convert(src, d["to"], o)
            c["engines"]["x2t"] = {"status": "ok" if ok else "unavailable", "reason": None if ok else msg, "out": key(o) if ok else None}
        else:
            c["engines"]["x2t"] = {"status": "unavailable", "reason": x2.reason or "not applicable to this direction kind"}
        # --- pdf2docx candidates over HTTP (merge off / on) + Office PDF reflow
        if kind == "pdf2docx":
            for tag, q in (("pdf2docx_raw", "merge=0"), ("pdf2docx_merged", "merge=1")):
                e = {"status": "unavailable", "reason": None if p2d_up else p2d_reason}
                if p2d_up:
                    o = w / (tag + ".docx")
                    try:
                        req = urllib.request.Request("%s/v1/pdf2docx?%s" % (p2d_url, q), data=src.read_bytes(), method="POST", headers={"Content-Type": "application/pdf"})
                        with urllib.request.urlopen(req, timeout=300) as r:
                            o.write_bytes(r.read()); e = {"status": "ok", "out": key(o), "report": r.headers.get("X-Convert-Report")}
                    except Exception as ex:
                        e = {"status": "unavailable", "reason": "pdf2docx request failed: %s %s" % (type(ex).__name__, str(ex)[:120])}
                c["engines"][tag] = e
            c["engines"]["office"] = {"status": "pending", "reason": None, "out": oq.need(src, "docx", w / "office" / (stem + ".docx"))}
            c["engines"]["soffice"] = {"status": "unavailable", "reason": "sidecar takes OOXML only (no PDF import)"}
    oq.flush("r1")

    # ---------- round 1: which outputs are in; queue Y -> PDF for editable outputs
    def engine_outputs(c, name):
        if name == "craft":
            return [p for p in c["craft"]["outputs"] if Path(p).exists()]
        e = c["engines"].get(name) or {}
        if name == "office" and e.get("out"):
            return [e["out"]] if oq.ok(e["out"]) else []
        return [e["out"]] if e.get("out") and have(e["out"]) else []

    ypdf = {}
    for c in cases:
        if c.get("error"): continue
        d = c["d"]
        cand = ["craft", "office", "soffice", "x2t"] + (["pdf2docx_raw", "pdf2docx_merged"] if d["kind"] == "pdf2docx" else [])
        for eng in cand:
            for y in engine_outputs(c, eng):
                if Path(y).suffix.lower() in (".docx", ".odt", ".rtf", ".xlsx", ".pptx", ".html"):
                    ypdf[(id(c), eng, y)] = oq.need(y, "pdf", c["w"] / "ypdf" / ("%s-%s.pdf" % (eng, Path(y).stem)))
        if d["kind"] == "pdf2docx" and c["craft_sidecar"]["outputs"]:
            for y in c["craft_sidecar"]["outputs"]:
                ypdf[(id(c), "craft_sidecar", y)] = oq.need(y, "pdf", c["w"] / "ypdf" / ("craft_sidecar-%s.pdf" % Path(y).stem))
    oq.flush("r2")

    # ---------- judge
    results = {}
    for c in cases:
        d = c["d"]; rec = {"fixture": c["name"], "engines": {}}
        results.setdefault(d["id"], []).append(rec)
        if c.get("error"):
            rec["error"] = c["error"]; continue
        kind, w = d["kind"], c["w"]
        craft = c["craft"]
        rec["craft"] = {k: craft[k] for k in ("adapter", "argv", "status", "code", "message", "sec", "warnings", "outputs", "peak_mb") if k in craft}
        rec["craft"]["steps"] = craft.get("steps")
        ref_pdf = c.get("ref_pdf") if c.get("ref_pdf") and have(c["ref_pdf"]) and (not str(c["ref_pdf"]).startswith(str(work)) or oq.ok(c["ref_pdf"]) or c["src"].suffix == ".pdf") else None
        if kind in ("pdf", "png", "edit", "text", "csv", "translit") and not ref_pdf:
            rec["ref_note"] = "no Office PDF of the source: " + (oq.error(c["ref_pdf"]) if c.get("ref_pdf") else "Office cannot read this input")
        src_s = S.structure(c["src"])
        rec["source_structure"] = src_s

        def judge(eng, ys, warnings=None):
            """-> metrics of one engine's outputs for this case."""
            m = {"outputs": [Path(y).name for y in ys][:8]}
            if not ys:
                return m
            y = ys[0]; vis = tm = None
            if kind == "pdf":
                if ref_pdf: vis = visual_metrics(ref_pdf, y, w / "png", "%s-%s" % (eng, Path(y).stem)); m.update(vis)
                else: m["pages_cand"] = pdf_pages(y)
            elif kind == "png":
                if ref_pdf:
                    rn, rp = rasterise(Path(ref_pdf), w / "png", "ref-" + Path(ref_pdf).stem)
                    cp = [Path(p) for p in sorted(ys) if str(p).lower().endswith(".png")]
                    m.update({"pages_ref": rn, "pages_cand": len(cp), "page_match": rn == len(cp) if eng == "craft" or len(cp) > 1 else None}); m.update(pix_metrics(rp, cp))
                else: m["pages_cand"] = len(ys)
            elif kind in ("edit", "translit"):
                yp = ypdf.get((id(c), eng, y)) or ypdf.get((id(c), "craft_sidecar", y))
                m["office_reopen"] = oq.ok(yp) if yp else None
                if yp and not oq.ok(yp): m["reopen_error"] = oq.error(yp)[:120]
                ys_s = S.structure(y); m["structure"] = ys_s
                if src_s and ys_s and src_s.get("paragraphs"): m["paragraph_ratio"] = round(ys_s["paragraphs"] / src_s["paragraphs"], 3)
                if yp and oq.ok(yp):
                    if ref_pdf: vis = visual_metrics(ref_pdf, yp, w / "png", "%s-%s" % (eng, Path(y).stem)); m.update(vis)
                    else: m["pages_cand"] = pdf_pages(yp)
                    if kind == "translit":
                        t = pdf_text(yp); lat = len(re.findall("[A-Za-z]", t)); cy = len(re.findall("[\u0400-\u04FF]", t))
                        m["cyrillic_share_of_letters"] = round(cy / (cy + lat), 3) if cy + lat else None
                m["structural_losses"] = S.structural_losses(src_s, ys_s) if kind == "edit" else (
                    [l for l in S.structural_losses(src_s, ys_s)] if kind == "translit" else [])
            elif kind == "text":
                ref_txt = c.get("ref_txt") if c.get("ref_txt") and oq.ok(c["ref_txt"]) else None
                ys_s = S.structure(y); m["structure"] = ys_s
                if ref_txt: tm = text_metrics(Path(ref_txt).read_text("utf-8", "replace"), plain_text(y)); m.update(tm)
                else: m["ref_note"] = "no Office text export of this source"
                m["structural_losses"] = S.structural_losses(src_s, ys_s)
            elif kind == "csv":
                o = (c["engines"]["office"] or {}).get("out")
                if o and oq.ok(o): m.update(csv_compare(o, y))
                else: m["ref_note"] = "no Office CSV"
                m["structural_losses"] = S.structural_losses(src_s, S.structure(y), "csv")
            if kind == "translit" and vis:   # the text is SUPPOSED to change: only pagination counts (Phase 2 owns the text checks)
                vis = {k: vis[k] for k in ("page_match", "pages_ref", "pages_cand")}
            m["losses"] = (m.get("structural_losses") or []) + losses_of(None, None, vis, tm, translit=(kind == "translit"))
            return m

        if kind in ("pdf", "png", "edit", "text", "csv", "translit"):
            rec["craft"]["metrics"] = judge("craft", engine_outputs(c, "craft")) if craft["status"] != "failed" else {}
            for eng in ("office", "soffice", "x2t"):
                e = dict(c["engines"].get(eng) or {"status": "unavailable", "reason": "n/a"})
                ys = engine_outputs(c, eng)
                if e["status"] == "pending":
                    e["status"] = "ok" if ys else "unavailable"
                    if not ys: e["reason"] = oq.error(e["out"]) if e.get("out") else e.get("reason")
                if ys and e["status"] == "ok": e["metrics"] = judge(eng, ys)
                e.pop("out", None); rec["engines"][eng] = e
            # loss-without-warning (craft)
            if craft["status"] != "failed":
                L = rec["craft"]["metrics"].get("losses", [])
                sil = S.silent_losses(L, craft["warnings"])
                rec["loss_check"] = {"losses": L, "warnings": len(craft["warnings"]), "silent": sil, "ok": not sil}
        elif kind == "pdfop":
            ys = sorted(Path(p) for p in craft["outputs"] if str(p).lower().endswith(".pdf"))
            exp = []   # (source pdf, page) per expected output page
            for s in c["srcs"]:
                n = pdf_pages(s)
                pages = list(range(1, n + 1))
                if d["op"] == "extract":
                    pages = [p for part in d["arg"].split(",") for p in (range(int(part.split("-")[0]), int(part.split("-")[-1]) + 1))]
                exp += [(s, p) for p in pages]
            exp_h = []
            for s, p in exp:
                f = w / "png" / ("exp-%s-p%03d.png" % (s.stem, p))
                (w / "png").mkdir(exist_ok=True); run([CLI["pdf"], "render", s, "--page", p, "--dpi", 96, "--out", f], 60)
                exp_h.append(hashlib.sha256(f.read_bytes()).hexdigest()[:12] if f.exists() else None)
            got = []
            for i, y in enumerate(ys):
                n, hs = page_hashes(y, w / "png", "got%d" % i); got += hs
            m = {"outputs": [y.name for y in ys], "pages_expected": len(exp_h), "pages_got": len(got), "page_match": len(exp_h) == len(got),
                 "pages_hash_equal": sum(1 for x, y in zip(exp_h, got) if x and x == y)}
            m["lossless"] = m["page_match"] and m["pages_hash_equal"] == len(exp_h)
            rec["craft"]["metrics"] = m
            rec["engines"] = {e: {"status": "unavailable", "reason": "not applicable (lossless PDF operation; reference = per-page render hashes of the sources)"} for e in ("office", "soffice", "x2t")}
        elif kind == "pdftext":
            stem = c["src"].stem; sdocx = Path(str(c["src"])[:-4] + ".docx")
            if d["op"] == "png":
                cp = [Path(p) for p in sorted(craft["outputs"]) if str(p).lower().endswith(".png")]
                n = pdf_pages(c["src"]); rp = []
                for p in range(1, min(n, 30) + 1):
                    f = w / "png" / ("ref150-p%02d.png" % p); (w / "png").mkdir(exist_ok=True)
                    run([CLI["pdf"], "render", c["src"], "--page", p, "--dpi", 150, "--out", f], 60); rp.append(f if f.exists() else None)
                m = {"pages_ref": n, "pages_cand": len(cp), "page_match": n == len(cp)}; m.update(pix_metrics(rp, cp))
            else:
                txt = plain_text(craft["outputs"][0]) if craft["outputs"] else ""
                ref = docx_info(sdocx)["text"] if sdocx.exists() else ""
                m = text_metrics(ref, txt)
            rec["craft"]["metrics"] = m
            rec["engines"] = {e: {"status": "unavailable", "reason": "not applicable (reference = the source document)"} for e in ("office", "soffice", "x2t")}
        elif kind == "pdf2docx":
            sdocx = Path(str(c["src"])[:-4] + ".docx"); truth = Path(str(c["src"])[:-4] + ".truth.json")
            def p2(eng, y):
                m = p2d_metrics(sdocx, y, truth)
                yp = ypdf.get((id(c), eng, y))
                if yp and oq.ok(yp):
                    m["visual"] = visual_metrics(c["src"], yp, w / "png", "%s-%s" % (eng, Path(y).stem))
                else:
                    m["visual"] = {"note": "Office could not render the output: " + (oq.error(yp) if yp else "no job")[:100]}
                return m
            rec["craft"]["metrics"] = p2("craft", craft["outputs"][0]) if craft["outputs"] else {}
            cs = c["craft_sidecar"]
            rec["engines"]["craft_sidecar"] = {"status": "ok" if cs["outputs"] else "unavailable", "reason": None if cs["outputs"] else "convert --pdf-engine sidecar: %s (code %s)" % (cs["message"], cs["code"]),
                                                **({"metrics": p2("craft_sidecar", cs["outputs"][0])} if cs["outputs"] else {})}
            for eng in ("pdf2docx_raw", "pdf2docx_merged", "office", "soffice", "x2t"):
                e = dict(c["engines"].get(eng) or {"status": "unavailable", "reason": "n/a"})
                ys = engine_outputs(c, eng)
                if e["status"] == "pending":
                    e["status"] = "ok" if ys else "unavailable"
                    if not ys: e["reason"] = oq.error(e["out"])
                if ys and e["status"] == "ok": e["metrics"] = p2(eng, ys[0])
                e.pop("out", None); rec["engines"][eng] = e
        elif kind == "edge":
            res = {}
            for tag, r in (("docx", craft), ("txt", c["craft_txt"])):
                has_text = False
                if r["outputs"] and tag == "docx": has_text = bool(docx_info(r["outputs"][0])["text"].strip())
                if r["outputs"] and tag == "txt": has_text = bool(plain_text(r["outputs"][0]).strip())
                failed = r["status"] == "failed"
                expected = {"scan.pdf": "failed or warning (no text layer)", "truncated.pdf": "failed or warning (damaged file)"}.get(c["name"], "failed")
                good = failed or (c["name"] in ("scan.pdf", "truncated.pdf") and bool(r["warnings"]))
                res[tag] = {"status": r["status"], "code": r["code"], "message": r["message"], "warnings": [x["text"][:100] for x in r["warnings"]],
                            "output_has_text": has_text, "expected": expected, "behaves": good, "silent_empty_success": (not failed) and not has_text and not r["warnings"]}
            rec["craft"]["metrics"] = res
            rec["engines"] = {e: {"status": "unavailable", "reason": "not applicable"} for e in ("office", "soffice", "x2t")}

    # ---------- write
    summary = {"generated": time.strftime("%Y-%m-%d %H:%M:%S"), "adapter": ad.describe(), "fixtures": str(fx), "engines": {
        "office": oq.reason or "MS Office 16 COM", "soffice": so.reason or "up (%s)" % so.url, "x2t": x2.reason or "up (%s)" % x2.url,
        "pdf2docx_sidecar": "up (%s)" % p2d_url if p2d_up else p2d_reason}, "directions": {}, "loss_without_warning": []}
    for d in dirs:
        rows = results.get(d["id"], [])
        (out / "directions" / (d["id"] + ".json")).write_text(json.dumps({"id": d["id"], "group": d["group"], "to": d["to"], "kind": d["kind"], "cases": rows},
                                                                       indent=1, ensure_ascii=False, default=str), encoding="utf-8")
        st = [r["craft"]["status"] for r in rows if "craft" in r]
        summary["directions"][d["id"]] = {"cases": len(rows), "ok": st.count("ok"), "warn": st.count("warn"), "failed": st.count("failed")}
        for r in rows:
            for s in (r.get("loss_check") or {}).get("silent", []):
                summary["loss_without_warning"].append({"direction": d["id"], "fixture": r["fixture"], **s})
    (out / "summary.json").write_text(json.dumps(summary, indent=1, ensure_ascii=False), encoding="utf-8")
    print("directions: %d, cases: %d, silent losses: %d -> %s" % (len(dirs), len(cases), len(summary["loss_without_warning"]), out / "summary.json"))


if __name__ == "__main__":
    main()
