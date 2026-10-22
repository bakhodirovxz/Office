"""The candidate adapter: ONE entry point per job, two back ends.

  convert : `convert <in> --to <fmt> --out-dir D --json` (PLAN 2.2); the report (status, code, warnings, steps) is the
            product's own, so the harness measures the product, not the raw CLIs.
  raw     : the domain CLIs with the argv of scripts/fidelity/census.ps1. Only used when no `convert` binary exists;
            it fakes a report (upstream stderr `Warning:` lines are the only warnings it can know about).

Never resolves `convert` through PATH: C:\\Windows\\System32\\convert.exe is the FAT -> NTFS disk tool.
"""
import json, os, re, shutil, time
from pathlib import Path
from .util import CLI, TOOLS, pdf_pages, run

CONVERT_CANDIDATES = [TOOLS / "services/release/convert.exe"]
WARN_RE = re.compile(r"warn|ignored|not imported|lost|unsupported|skipp", re.I)
DOC_EXT = {"docx", "doc", "dot", "dotx", "docm", "odt", "rtf", "html", "htm", "md", "txt"}
SHEET_EXT = {"xlsx", "xls", "xlsb", "ods", "xlsm", "csv", "tsv"}
SLIDE_EXT = {"pptx", "ppt", "ppsx"}


def find_convert(explicit=None):
    cands = [Path(explicit)] if explicit else []
    if os.environ.get("CONVERT_BIN"):
        cands.append(Path(os.environ["CONVERT_BIN"]))
    return next((c for c in cands + CONVERT_CANDIDATES if c.exists()), None)


class Adapter:
    def __init__(self, mode="auto", convert_bin=None, locale=None):
        self.bin = find_convert(convert_bin)
        self.locale = locale
        self.mode = "convert" if (mode in ("auto", "convert") and self.bin) else "raw"
        if mode == "convert" and not self.bin:
            raise SystemExit("--adapter convert: no convert binary (set CONVERT_BIN or build services/convert)")
        self.env = dict(os.environ, WORDCRAFT_REGION="en-US")
        for k, v in (("CONVERT_DOCS_CLI", "docs"), ("CONVERT_SHEETS_CLI", "sheets"), ("CONVERT_SLIDES_CLI", "slides"), ("CONVERT_PDF_CLI", "pdf")):
            self.env.setdefault(k, str(CLI[v]))

    def describe(self):
        return {"mode": self.mode, "convert": str(self.bin) if self.bin else None, "locale": self.locale}

    # ---- public
    def convert(self, src, to, out_dir, translit=None, sheet=None, pdf_engine=None, dpi=None, timeout=600, locale=None):
        out_dir = Path(out_dir); shutil.rmtree(out_dir, ignore_errors=True); out_dir.mkdir(parents=True)
        if self.mode == "convert":
            argv = [self.bin, src, "--to", to, "--out-dir", out_dir, "--json", "--timeout", "300"]   # a loaded box trips the 10 s floor
            if translit: argv += ["--translit", translit]
            if sheet: argv += ["--sheet", sheet]
            if pdf_engine: argv += ["--pdf-engine", pdf_engine]
            if locale or self.locale: argv += ["--locale", locale or self.locale]
            if dpi: argv += ["--dpi", dpi]
            return self._run_convert(argv, timeout)
        return self._raw(Path(src), to, out_dir, translit, sheet, dpi, timeout)

    def pdf_op(self, op, srcs, out_dir, arg=None, timeout=300):
        """op: combine | split-every N | split-before LIST | extract PAGES."""
        out_dir = Path(out_dir); shutil.rmtree(out_dir, ignore_errors=True); out_dir.mkdir(parents=True)
        out = out_dir / ("%s.pdf" % op)
        if self.mode == "convert":
            if op == "combine": argv = [self.bin, "pdf", "combine", *srcs, "-o", out]
            elif op == "extract": argv = [self.bin, "pdf", "extract", srcs[0], "--pages", arg, "-o", out]
            else: argv = [self.bin, "pdf", "split", srcs[0], *arg, "--out-dir", out_dir]
            argv.append("--json")
            return self._run_convert(argv, timeout)
        if op == "combine": argv = [CLI["pdf"], "combine", *srcs, "--out", out]
        elif op == "extract": argv = [CLI["pdf"], "extract", srcs[0], "--pages", arg, "--out", out]
        else: argv = [CLI["pdf"], "split", srcs[0], *arg, "--out-dir", out_dir]
        return self._single(argv, out_dir, timeout)

    # ---- convert back end
    def _run_convert(self, argv, timeout):
        rc, o, e, sec = run(argv, timeout, env=self.env)
        res = {"adapter": "convert", "argv": [str(a) for a in argv], "exit": rc, "sec": round(sec, 3), "stderr_tail": e[-400:]}
        try:
            rep = json.loads(o)[0]
        except Exception:
            rep = None
        if rep is None:
            res.update(status="failed", code=rc, message=(e.strip().splitlines() or ["no JSON report"])[-1][:200], warnings=[], outputs=[], steps=[])
            return res
        res.update(status=rep["status"], code=rep["code"], message=rep["message"], warnings=rep["warnings"],
                   outputs=[str(Path(p)) for p in rep["outputs"]], steps=rep["steps"])
        res["peak_mb"] = max([s.get("peak_mb", 0) or 0 for s in rep["steps"]] or [0])
        return res

    # ---- raw back end (census.ps1 argv)
    def _single(self, argv, out_dir, timeout, outputs=None):
        rc, o, e, sec = run(argv, timeout, env=self.env)
        outs = outputs if outputs is not None else sorted(str(p) for p in Path(out_dir).iterdir())
        outs = [p for p in outs if Path(p).exists() and (Path(p).is_dir() or Path(p).stat().st_size > 0)]
        warns = [{"code": "upstream", "count": 1, "text": l.strip()} for l in (e + "\n" + o).splitlines() if WARN_RE.search(l) and not l.startswith("{")]
        ok = rc == 0 and bool(outs)
        return {"adapter": "raw", "argv": [str(a) for a in argv], "exit": rc, "sec": round(sec, 3), "status": ("warn" if warns else "ok") if ok else "failed",
                "code": 0 if ok else (rc or 8), "message": "ok" if ok else (e.strip().splitlines() or ["no output"])[-1][:200],
                "warnings": warns, "outputs": outs, "steps": [{"tool": Path(str(argv[0])).stem, "exit": rc, "ms": int(sec * 1000)}], "stderr_tail": e[-400:]}

    def _raw(self, src, to, out_dir, translit, sheet, dpi, timeout):
        ext = src.suffix.lower().lstrip(".")
        out = out_dir / (src.stem + "." + to)
        J = lambda scope: '{"scope":"%s"}' % scope
        if ext == "pdf" and to == "docx":      # native route (F3)
            job = out_dir / "_job"; (job / "in").mkdir(parents=True); (job / "out").mkdir()
            shutil.copy2(src, job / "in/a.pdf")
            steps = [{"tool": "doc_open", "args": {"path": "in/a.pdf"}}, {"tool": "doc_export_office", "args": {"doc": 1, "path": "out/a.docx"}}]
            (job / "s.json").write_text(json.dumps(steps), encoding="utf-8")
            r = self._single([CLI["pdf"], "run", "--script", job / "s.json", "--root", job], out_dir, timeout, [str(job / "out/a.docx")])
            if r["outputs"]:
                shutil.move(job / "out/a.docx", out); r["outputs"] = [str(out)]
            return r
        if ext == "pdf" and to == "png":
            n = pdf_pages(src); outs = []
            for p in range(1, n + 1):
                f = out_dir / ("%s-p%02d.png" % (src.stem, p))
                run([CLI["pdf"], "render", src, "--page", p, "--dpi", dpi or 150, "--out", f], 60); outs.append(str(f))
            return self._single([CLI["pdf"], "info", src], out_dir, 60, outs)
        if ext == "pdf" and to == "txt":
            rc, o, e, sec = run([CLI["pdf"], "text", src], timeout, env=self.env); out.write_text(o, encoding="utf-8")
            r = self._single([CLI["pdf"], "info", src], out_dir, 60, [str(out)] if o.strip() else [])
            r["message"] = "ok" if o.strip() else "no text layer"; return r
        if ext in DOC_EXT:
            if to == "png":                    # F4: the pdf route
                mid = out_dir / (src.stem + ".pdf"); r = self._single([CLI["docs"], "convert", src, mid], out_dir, timeout, [str(mid)])
                if not r["outputs"]: return r
                n = pdf_pages(mid); outs = []
                for p in range(1, n + 1):
                    f = out_dir / ("%s-p%02d.png" % (src.stem, p)); run([CLI["pdf"], "render", mid, "--page", p, "--dpi", dpi or 150, "--out", f], 60); outs.append(str(f))
                mid.unlink(); r["outputs"] = outs; return r
            if translit:
                return self._single([CLI["docs"], "run", "--file", src, "--cmd", "tools.translitCyrl=" + J("document"), "--save", out], out_dir, timeout, [str(out)])
            return self._single([CLI["docs"], "convert", src, out], out_dir, timeout, [str(out)])
        if ext in SHEET_EXT:
            if translit:
                return self._single([CLI["sheets"], "run", "--in", src, "--cmd", "tools.translitCyrl=" + J("workbook"), "--out", out], out_dir, timeout, [str(out)])
            if to == "pdf":
                cmd = 'file.exportPdf={"path":"%s","sheets":"all"}' % str(out).replace("\\", "/")
                return self._single([CLI["sheets"], "run", "--in", src, "--cmd", cmd], out_dir, timeout, [str(out)])
            argv = [CLI["sheets"], "convert", src, out] + (["--sheet", sheet] if sheet and sheet not in ("active", "all") else [])
            return self._single(argv, out_dir, timeout, [str(out)])
        if ext in SLIDE_EXT:
            if to == "png":
                d = out_dir / "png"; d.mkdir()
                r = self._single([CLI["slides"], "render", src, "--all", d], out_dir, timeout, [])
                r["outputs"] = sorted(str(p) for p in d.glob("*.png")); r["status"] = "ok" if r["outputs"] and r["exit"] == 0 else "failed"; return r
            if translit:
                return self._single([CLI["slides"], "run", "--in", src, "--cmd", "tools.translitCyrl=" + J("deck"), "--save", out], out_dir, timeout, [str(out)])
            return self._single([CLI["slides"], "convert", src, out], out_dir, timeout, [str(out)])
        return {"adapter": "raw", "argv": [], "exit": 3, "sec": 0, "status": "failed", "code": 3, "message": "unsupported in raw adapter: %s -> %s" % (ext, to),
                "warnings": [], "outputs": [], "steps": [], "stderr_tail": ""}
