"""One conversion job in a child process: python -I worker.py in.pdf out.docx opts.json

stdout: one JSON line. {"ok":true,"report":{...}} or {"ok":false,"status":N,"code":"...","message":"..."}.
Anything else (a crash, a signal) is reported by server.py as 500 worker_crashed.
"""
import json, os, re, resource, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))


def fail(status, code, message):
    print(json.dumps({"ok": False, "status": status, "code": code, "message": message}))
    sys.exit(0)


def parse_pages(spec, n):
    """'1-5,7' (1-based) -> sorted 0-based list; empty means all."""
    if not spec:
        return list(range(n))
    out = set()
    for part in spec.split(","):
        m = re.fullmatch(r"(\d+)(?:-(\d+))?", part.strip())
        if not m:
            fail(400, "bad_request", "pages must look like 1-5,7")
        a = int(m[1])
        b = int(m[2] or a)
        if a < 1 or b < a or b > n:
            fail(400, "bad_request", "pages out of range (the PDF has %d pages)" % n)
        out.update(range(a - 1, b))
    return sorted(out)


def main():
    src, dst, optpath = sys.argv[1:4]
    opts = json.load(open(optpath))
    limit = int(opts.get("mem_bytes", 0))
    if limit:
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
    import fitz  # PyMuPDF (AGPL): used only inside this isolated container
    try:
        doc = fitz.open(src)
    except Exception as e:
        fail(400, "bad_pdf", "cannot open the PDF: %s" % e)
    if doc.needs_pass or doc.is_encrypted:
        fail(422, "encrypted", "the PDF is encrypted; remove the protection first")
    n = doc.page_count
    if n == 0:
        fail(400, "bad_pdf", "the PDF has no pages")
    if n > opts["max_pages"]:
        fail(422, "too_many_pages", "%d pages; the limit is %d" % (n, opts["max_pages"]))
    pages = parse_pages(opts.get("pages"), n)
    empty = [i + 1 for i in pages if not doc[i].get_text().strip()]
    if len(empty) == len(pages):
        fail(422, "no_text_layer", "the PDF has no text layer (scanned?); OCR is not supported")
    warnings = []
    if empty:
        warnings.append("pages without a text layer: %s" % empty)
    doc.close()

    from pdf2docx import Converter
    mid = dst + ".mid.docx" if opts.get("merge") else dst
    try:
        cv = Converter(src)
        try:
            cv.convert(mid, pages=pages, parse_stream_table=False)
        finally:
            cv.close()
    except Exception as e:
        fail(400, "bad_pdf", "pdf2docx failed: %s" % e)

    merged = 0
    if opts.get("merge"):
        r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "merge_lines.py"), mid, dst],
                           capture_output=True, text=True)
        m = re.match(r"merged (\d+)", r.stdout)
        if r.returncode != 0 or not m:
            fail(500, "merge_failed", "merge_lines.py failed: %s" % r.stderr[-300:])
        merged = int(m[1])

    import docx
    paras = sum(1 for p in docx.Document(dst).paragraphs if p.text.strip())
    print(json.dumps({"ok": True, "report": {"pages": len(pages), "merged": merged,
                                             "paragraphs": paras, "warnings": warnings}}))


main()
