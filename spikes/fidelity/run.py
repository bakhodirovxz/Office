#!/usr/bin/env python
"""Fidelity harness: Craft CLIs vs a reference renderer, page-image + text + structure metrics.

  python -I spikes/fidelity/run.py --input E:/office-target/fidelity/synthetic --out E:/office-target/fidelity
  python -I spikes/fidelity/run.py --input corpus/ --out E:/office-target/fidelity-corpus     (owner's corpus)

Per file and per build (release / head):
  reference : Microsoft Office 16 (COM) -> PDF                      [providers: office; libreoffice/x2t = hooks]
  direct    : <craft>-cli open -> PDF
  roundtrip : <craft>-cli open -> save native (docx/xlsx/pptx) -> reopen -> PDF
  all PDFs are rasterised with the SAME rasteriser (pdfcraft-cli render, 96 dpi)
  metrics   : page count, text similarity (+ missing words), per-page pixel diff,
              Office reopen of the Craft-saved file + structural counts before/after.
Corpus safety: when --input is not the synthetic set, names are replaced by ids (f001...), nothing
corpus-derived is printed beyond ids/numbers, and every artefact is written under --out only.
Test tooling only (needs pillow + numpy). Never builds anything.
"""
import argparse, collections, ctypes, difflib, hashlib, json, re, shutil, subprocess, sys, time, unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
REL = Path("E:/office-tools/releases/win")
EXE = {
    "release": {
        "word": REL / "wordcraft-0.7.0-windows-x64-portable/wordcraft-0.7.0-windows-x64-portable/wordcraft-cli.exe",
        "excel": REL / "gridcraft-0.7.0-windows-x64-portable/gridcraft-0.7.0-windows-x64-portable/gridcraft-cli.exe",
        "ppt": REL / "deckcraft-0.7.0-windows-x64-portable/deckcraft-0.7.0-windows-x64-portable/deckcraft-cli.exe",
    },
    "head": {
        "word": Path("E:/office-target/wordcraft/release/wordcraft-cli.exe"),
        "excel": Path("E:/office-target/gridcraft/release/gridcraft-cli.exe"),
        "ppt": Path("E:/office-target/deckcraft/release/deckcraft-cli.exe"),
    },
}
PDFCRAFT = REL / "pdfcraft-0.8.0-windows-x64-portable/pdfcraft-0.8.0-windows-x64-portable/pdfcraft-cli.exe"
APP_OF = {".doc": "word", ".docx": "word", ".xls": "excel", ".xlsx": "excel", ".ppt": "ppt", ".pptx": "ppt"}
NATIVE = {"word": ".docx", "excel": ".xlsx", "ppt": ".pptx"}
LEGACY = {".doc", ".xls", ".ppt"}
DPI = 96
MAX_PAGES = 30
SYNTH_MARK = "synthetic"


# ---------------------------------------------------------------- helpers
def free_ram_gb():
    class MS(ctypes.Structure):
        _fields_ = [("l", ctypes.c_ulong), ("load", ctypes.c_ulong), ("tp", ctypes.c_ulonglong),
                    ("ap", ctypes.c_ulonglong), ("tpf", ctypes.c_ulonglong), ("apf", ctypes.c_ulonglong),
                    ("tv", ctypes.c_ulonglong), ("av", ctypes.c_ulonglong), ("ae", ctypes.c_ulonglong)]
    m = MS(); m.l = ctypes.sizeof(MS)
    ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(m))
    return m.ap / 2**30


def wait_ram(min_gb=2.0, max_wait=3 * 3600):
    t0, last = time.time(), 0
    while free_ram_gb() < min_gb:
        if time.time() - t0 > max_wait:
            raise RuntimeError("free RAM stayed below %.1f GB" % min_gb)
        if time.time() - last > 300:
            print("  waiting for free RAM >= %.1f GB (now %.2f)" % (min_gb, free_ram_gb()), flush=True)
            last = time.time()
        time.sleep(10)


def office_running():
    out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout.upper()
    return [n for n in ("WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE") if n in out]


def run(cmd, timeout=180):
    t0 = time.time()
    try:
        p = subprocess.run([str(c) for c in cmd], capture_output=True, timeout=timeout)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace"), time.time() - t0
    except subprocess.TimeoutExpired:
        return 124, "", "timeout", time.time() - t0


def version_of(exe):
    if not Path(exe).exists():
        return None
    for flag in ("--version", "version"):
        rc, o, e, _ = run([exe, flag], 20)
        txt = (o or e).strip().splitlines()
        if rc == 0 and txt:
            return txt[0]
    return "unknown"


def fwd(p):
    return str(p).replace("\\", "/")


# ---------------------------------------------------------------- reference providers (hooks)
def office_batch(app, mode, items, out_dir):
    """items: list of (src, out). Returns {src: status dict}. One Office instance per call."""
    items = [(fwd(a), fwd(b)) for a, b in items]
    if not items:
        return {}
    busy = office_running()
    t0 = time.time()
    while busy:
        if time.time() - t0 > 1800:
            raise RuntimeError("Office process present for 30 min: %s" % busy)
        print("  waiting: Office process running %s" % busy, flush=True)
        time.sleep(30)
        busy = office_running()
    wait_ram(2.0)
    lst = out_dir / ("_com_%s_%s_list.json" % (app, mode)); res = out_dir / ("_com_%s_%s_result.json" % (app, mode))
    lst.write_text(json.dumps([{"src": a.replace("/", "\\"), "out": b.replace("/", "\\")} for a, b in items]), encoding="utf-8")
    if res.exists():
        res.unlink()
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", HERE / "office_com.ps1",
           "-Mode", mode, "-App", app, "-List", lst, "-Result", res]
    rc, o, e, dt = run(cmd, 1500)
    if not res.exists():
        raise RuntimeError("office_com failed rc=%s %s" % (rc, (e or o)[:500]))
    data = json.loads(res.read_text(encoding="utf-8-sig"))
    return {fwd(d["src"]): d for d in data}


def libreoffice_reference(src, out_pdf):
    """HOOK (not run: Docker was down). Intended: docker run --rm -v <dir>:/w <image> soffice --headless
    --convert-to pdf --outdir /w/out /w/<file>. Returns True on success."""
    raise NotImplementedError("LibreOffice reference not wired yet")


def x2t_reference(src, out_pdf):
    """HOOK (not run). Intended: ONLYOFFICE x2t container, XML job file -> PDF."""
    raise NotImplementedError("x2t reference not wired yet")


# ---------------------------------------------------------------- candidate (Craft CLIs)
def craft_convert(app, build, src, dst):
    """open src with the Craft CLI, write dst (format by extension). Returns (ok, note)."""
    exe = EXE[build][app]
    dst = Path(dst)
    if dst.exists():
        dst.unlink()
    wait_ram(1.0)
    if app == "excel" and dst.suffix.lower() == ".pdf":
        cmd = [exe, "run", "--in", src, "--cmd", 'file.exportPdf={"path":"%s","sheets":"all"}' % fwd(dst)]
    elif app == "ppt" and dst.suffix.lower() == ".pptx":
        cmd = [exe, "run", "--in", src, "--save", dst]
        # deckcraft convert only handles what it can read; run --save re-serialises natively
        rc, o, e, dt = run(cmd, 240)
        ok = rc == 0 and dst.exists()
        return ok, ((e or o).strip()[:300] if not ok else (e.strip()[:300]))
    else:
        cmd = [exe, "convert", src, dst]
    rc, o, e, dt = run(cmd, 240)
    ok = rc == 0 and dst.exists() and dst.stat().st_size > 0
    note = (e or "").strip()
    if not ok:
        note = (e or o).strip()
    return ok, note[:400]


# ---------------------------------------------------------------- rasterise + compare
def pdf_pages(pdf):
    rc, o, e, _ = run([PDFCRAFT, "info", pdf], 60)
    try:
        return json.loads(o)["pages"]
    except Exception:
        return 0


def rasterise(pdf, png_dir, tag):
    n = pdf_pages(pdf)
    out = []
    for p in range(1, min(n, MAX_PAGES) + 1):
        f = png_dir / ("%s-p%02d.png" % (tag, p))
        if not f.exists():
            run([PDFCRAFT, "render", pdf, "--page", p, "--dpi", DPI, "--out", f], 60)
        out.append(f if f.exists() else None)
    return n, out


def pdf_text(pdf):
    rc, o, e, _ = run([PDFCRAFT, "text", pdf], 60)
    return o if rc == 0 else ""


def norm_text(t):
    t = unicodedata.normalize("NFKC", t).replace("\x0c", " ").casefold()
    t = t.replace("\u02bb", "'").replace("\u02bc", "'").replace("\u2019", "'").replace("\u2018", "'")
    return re.sub(r"\s+", " ", t).strip()


def text_metrics(ref_txt, cand_txt):
    a, b = norm_text(ref_txt), norm_text(cand_txt)
    wa, wb = re.findall(r"\w+", a), re.findall(r"\w+", b)
    if not a and not b:
        return {"char_sim": 1.0, "word_sim": 1.0, "ref_words": 0, "cand_words": 0, "missing_words": [], "extra_words": []}
    if max(len(a), len(b)) <= 20000:
        cs = difflib.SequenceMatcher(None, a, b, autojunk=False).ratio()
    else:
        cs = difflib.SequenceMatcher(None, wa, wb, autojunk=False).ratio()
    ws = difflib.SequenceMatcher(None, wa, wb, autojunk=False).ratio()
    ca, cb = collections.Counter(wa), collections.Counter(wb)
    miss = list((ca - cb).elements()); extra = list((cb - ca).elements())
    return {"char_sim": round(cs, 4), "word_sim": round(ws, 4), "ref_words": len(wa), "cand_words": len(wb),
            "missing_words": miss[:40], "missing_count": len(miss), "extra_words": extra[:40], "extra_count": len(extra)}


def page_diff(ref_png, cand_png, diff_png=None):
    import numpy as np
    from PIL import Image, ImageFilter
    A = Image.open(ref_png).convert("L"); B = Image.open(cand_png).convert("L")
    same = A.size == B.size
    if not same:
        B = B.resize(A.size)
    a = np.asarray(A.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    b = np.asarray(B.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    d = np.abs(a - b) > 48
    pct = float(d.mean() * 100)
    if diff_png is not None:
        W, H = A.size
        rgb = Image.merge("RGB", (A, A, A)).convert("RGB"); crgb = Image.merge("RGB", (B, B, B))
        ov = np.asarray(Image.blend(rgb, rgb, 0)).copy(); ov[d] = (255, 0, 0)
        canvas = Image.new("RGB", (W * 3 + 20, H), "white")
        canvas.paste(rgb, (0, 0)); canvas.paste(crgb, (W + 10, 0)); canvas.paste(Image.fromarray(ov), (2 * W + 20, 0))
        canvas.save(diff_png, optimize=True)
    return round(pct, 3), same


# ---------------------------------------------------------------- structure deltas + loss classification
STRUCT_KEYS = {
    "word": [("paragraphs", "text"), ("tables", "tables"), ("inline_images", "images"), ("floating_shapes", "images"),
             ("bookmarks", "bookmarks"), ("fields", "fields"), ("header_footer_fields", "headers/footers"),
             ("comments", "comments/track changes"), ("revisions", "comments/track changes"),
             ("footnotes", "footnotes"), ("sections", "formatting")],
    "excel": [("sheets", "text"), ("formulas", "fields"), ("charts", "charts"), ("cond_formats", "formatting"),
              ("names", "fields")],
    "ppt": [("slides", "text"), ("shapes", "formatting"), ("tables", "tables"), ("pictures", "images"),
            ("charts", "charts"), ("notes_slides", "notes")],
}


def struct_deltas(app, before, after):
    out = {}
    if not before or not after:
        return out
    for k, cat in STRUCT_KEYS[app]:
        if k in before and k in after and before[k] != after[k]:
            out[k] = {"before": before[k], "after": after[k], "category": cat}
    if app == "word":
        bn = set(before.get("bookmark_names") or []) if isinstance(before.get("bookmark_names"), list) else {before.get("bookmark_names")} - {None}
        an = set(after.get("bookmark_names") or []) if isinstance(after.get("bookmark_names"), list) else {after.get("bookmark_names")} - {None}
        lost = sorted(bn - an)
        if lost:
            out["bookmark_names_lost"] = {"before": sorted(bn), "after": sorted(an), "category": "bookmarks"}
        out["hrm_desc_survives"] = {"before": "_hrm_desc_" in bn, "after": "_hrm_desc_" in an, "category": "bookmarks"}
        if "_hrm_desc_" not in bn or "_hrm_desc_" in an:
            out.pop("hrm_desc_survives")
    return out


def classify(app, ext, v):
    """v: result dict of one variant. Returns list of {category, detail}."""
    L = []
    if not v.get("convert_ok"):
        L.append({"category": "legacy-import" if ext in LEGACY else "open-failure", "detail": "convert failed: " + v.get("note", "")[:120]})
        return L
    if v.get("pages_ref") is not None and v.get("pages_cand") is not None and v["pages_ref"] != v["pages_cand"]:
        L.append({"category": "pagination", "detail": "pages %s -> %s" % (v["pages_ref"], v["pages_cand"])})
    t = v.get("text") or {}
    if t and t.get("char_sim", 1) < 0.99:
        mw = t.get("missing_words", [])[:6]
        L.append({"category": "text", "detail": "char_sim %.3f, %d words missing %s" % (t["char_sim"], t.get("missing_count", 0), mw)})
    if v.get("pix_mean") is not None and v["pix_mean"] > 2.0:
        L.append({"category": "formatting", "detail": "mean pixel diff %.2f%% (max %.2f%%)" % (v["pix_mean"], v["pix_max"])})
    if v.get("office_reopen") is False:
        L.append({"category": "open-failure", "detail": "Office could not reopen Craft-saved file: " + str(v.get("reopen_error"))[:100]})
    for k, d in (v.get("struct") or {}).items():
        L.append({"category": d["category"], "detail": "%s %s -> %s" % (k, d["before"], d["after"])})
    if ext in LEGACY:
        for x in L:
            x["legacy_source"] = True
    return L


# ---------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--builds", default="release", help="comma list: release,head")
    ap.add_argument("--apps", default="word,excel,ppt")
    ap.add_argument("--reference", default="office", choices=["office"])
    ap.add_argument("--force", action="store_true", help="recompute everything")
    ap.add_argument("--worst", type=int, default=6)
    ap.add_argument("--excel-exe", help="use this gridcraft-cli.exe for the Excel apps in every selected build (A/B tests of a patched CLI)")
    ap.add_argument("--no-reopen", action="store_true", help="skip step 3 (Office COM reopen/inspect of Craft-saved files); reuse existing ref.* from --out")
    a = ap.parse_args()
    if a.excel_exe:
        for b in EXE:
            EXE[b]["excel"] = Path(a.excel_exe)

    inp, out = Path(a.input).resolve(), Path(a.out).resolve()
    if not str(out).upper().startswith("E:"):
        sys.exit("--out must be on E: (corpus-derived artefacts must never land in the repo)")
    out.mkdir(parents=True, exist_ok=True)
    synthetic = SYNTH_MARK in inp.name.lower()
    builds = a.builds.split(",")
    apps = a.apps.split(",")
    if a.force:
        for d in out.glob("case_*"):
            shutil.rmtree(d, ignore_errors=True)

    files = sorted(p for p in inp.iterdir() if p.suffix.lower() in APP_OF and not p.name.startswith("~$") and APP_OF[p.suffix.lower()] in apps)
    cases = []
    for i, p in enumerate(files, 1):
        cid = ("%s%s" % (p.stem, p.suffix.replace(".", "_"))) if synthetic else "f%03d" % i
        cdir = out / ("case_" + cid); (cdir / "png").mkdir(parents=True, exist_ok=True)
        cases.append({"id": cid, "src": p, "ext": p.suffix.lower(), "app": APP_OF[p.suffix.lower()], "dir": cdir})
    if not synthetic:
        (out / "_name_map.json").write_text(json.dumps({c["id"]: c["src"].name for c in cases}, ensure_ascii=False, indent=1), encoding="utf-8")
    print("cases: %d  synthetic=%s  builds=%s" % (len(cases), synthetic, builds), flush=True)

    versions = {"office": None, "pdfcraft-cli": version_of(PDFCRAFT)}
    for b in builds:
        for app_, exe in EXE[b].items():
            versions["%s/%s" % (b, exe.stem)] = version_of(exe)
            try:
                versions["%s/%s mtime" % (b, exe.stem)] = time.strftime("%Y-%m-%d %H:%M", time.localtime(exe.stat().st_mtime))
                versions["%s/%s sha256" % (b, exe.stem)] = hashlib.sha256(exe.read_bytes()).hexdigest()[:16]
            except OSError:
                pass
    rc, o, e, _ = run(["reg", "query", r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Office\ClickToRun\Configuration", "/v", "VersionToReport"], 20)
    versions["office"] = (o.strip().splitlines()[-1].split()[-1] if rc == 0 and o.strip() else "Office16 (version unknown)")

    # ---- 1. reference via Office
    for app in apps:
        cs = [c for c in cases if c["app"] == app]
        todo = [c for c in cs if not (c["dir"] / "ref.pdf").exists()]
        if todo:
            print("[ref] %s: exporting %d via Office COM" % (app, len(todo)), flush=True)
            st = office_batch(app, "export", [(c["src"], c["dir"] / "ref.pdf") for c in todo], out)
            for c in todo:
                s = st.get(fwd(c["src"]), {})
                c["ref_err"] = s.get("error")
        todo = [c for c in cs if not (c["dir"] / "ref.json").exists()]
        if todo:
            print("[ref] %s: inspecting %d originals" % (app, len(todo)), flush=True)
            office_batch(app, "inspect", [(c["src"], c["dir"] / "ref.json") for c in todo], out)

    # ---- 2. candidate conversions
    for c in cases:
        c["variants"] = {}
        for b in builds:
            v = {"build": b}
            d = c["dir"] / b; d.mkdir(exist_ok=True)
            nat = NATIVE[c["app"]]
            ok1, n1 = craft_convert(c["app"], b, c["src"], d / "direct.pdf")
            v["direct"] = {"convert_ok": ok1, "note": n1}
            ok2, n2 = craft_convert(c["app"], b, c["src"], d / ("rt" + nat))
            ok3, n3 = (craft_convert(c["app"], b, d / ("rt" + nat), d / "rt.pdf") if ok2 else (False, "no roundtrip file"))
            v["roundtrip"] = {"convert_ok": ok2 and ok3, "note": (n2 if not ok2 else n3), "save_ok": ok2, "save_note": n2,
                              "reopen_craft_ok": ok3}
            c["variants"][b] = v
            print("[cand] %s %s direct=%s save=%s reopen=%s" % (c["id"], b, ok1, ok2, ok3), flush=True)

    # ---- 3. Office reopen + structure of Craft-saved files
    for b in builds:
        for app in ([] if a.no_reopen else apps):
            cs = [c for c in cases if c["app"] == app and c["variants"][b]["roundtrip"]["save_ok"]]
            todo = [c for c in cs if True]
            if not todo:
                continue
            print("[reopen] %s %s: %d files" % (b, app, len(todo)), flush=True)
            st = office_batch(app, "inspect", [(c["dir"] / b / ("rt" + NATIVE[app]), c["dir"] / b / "rt.json") for c in todo], out)
            for c in todo:
                s = st.get(fwd(c["dir"] / b / ("rt" + NATIVE[app])), {})
                c["variants"][b]["roundtrip"]["office_reopen"] = bool(s.get("ok"))
                c["variants"][b]["roundtrip"]["reopen_error"] = s.get("error")

    # ---- 4. rasterise + metrics
    allpages = []
    results = []
    for c in cases:
        ref_pdf = c["dir"] / "ref.pdf"
        ref_json = json.loads((c["dir"] / "ref.json").read_text(encoding="utf-8-sig")) if (c["dir"] / "ref.json").exists() else None
        rec = {"id": c["id"], "app": c["app"], "ext": c["ext"], "ref_ok": ref_pdf.exists(), "ref_struct": ref_json, "builds": {}}
        if synthetic:
            rec["file"] = c["src"].name
        png = c["dir"] / "png"
        ref_n, ref_pngs = (rasterise(ref_pdf, png, "ref") if ref_pdf.exists() else (0, []))
        rec["pages_ref"] = ref_n
        ref_txt = pdf_text(ref_pdf) if ref_pdf.exists() else ""
        for b in builds:
            bv = {}
            for kind in ("direct", "roundtrip"):
                v = dict(c["variants"][b][kind])
                pdf = c["dir"] / b / ("direct.pdf" if kind == "direct" else "rt.pdf")
                v["pages_ref"] = ref_n
                if v["convert_ok"] and ref_pdf.exists() and pdf.exists():
                    n, cp = rasterise(pdf, png, "%s-%s" % (b, kind))
                    v["pages_cand"] = n
                    v["text"] = text_metrics(ref_txt, pdf_text(pdf))
                    pix = []
                    for i in range(min(len(ref_pngs), len(cp))):
                        if ref_pngs[i] and cp[i]:
                            pct, same = page_diff(ref_pngs[i], cp[i])
                            pix.append(pct)
                            allpages.append((pct, c, b, kind, i + 1, ref_pngs[i], cp[i]))
                    v["pix_pages"] = pix
                    v["pix_mean"] = round(sum(pix) / len(pix), 3) if pix else None
                    v["pix_max"] = max(pix) if pix else None
                if kind == "roundtrip":
                    rj = c["dir"] / b / "rt.json"
                    after = json.loads(rj.read_text(encoding="utf-8-sig")) if rj.exists() else None
                    v["struct_after"] = after
                    v["struct"] = struct_deltas(c["app"], ref_json, after)
                v["losses"] = classify(c["app"], c["ext"], v)
                bv[kind] = v
            rec["builds"][b] = bv
        results.append(rec)
        print("[metrics] %s done" % c["id"], flush=True)

    # ---- 5. worst page diffs (side-by-side) -> out/diffs
    ddir = out / "diffs"; ddir.mkdir(exist_ok=True)
    allpages.sort(key=lambda x: -x[0])
    seen, worst = set(), []
    for pct, c, b, kind, pg, rp, cp in allpages:
        key = (c["id"], b)
        if key in seen:
            continue
        seen.add(key)
        f = ddir / ("%s_%s_%s_p%02d_%0.1fpct.png" % (c["id"], b, kind, pg, pct))
        page_diff(rp, cp, f)
        worst.append({"file": f.name, "id": c["id"], "build": b, "kind": kind, "page": pg, "pct": pct})
        if len(worst) >= a.worst:
            break

    summary = {"generated": time.strftime("%Y-%m-%d %H:%M:%S"), "synthetic": synthetic, "dpi": DPI, "versions": versions,
               "builds": builds, "results": results, "worst": worst, "reference": "Microsoft Office 16 PDF export"}
    (out / "summary.json").write_text(json.dumps(summary, ensure_ascii=not synthetic and True, indent=1, default=str), encoding="utf-8")
    print("summary:", out / "summary.json")


if __name__ == "__main__":
    main()
