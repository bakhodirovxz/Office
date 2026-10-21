"""PDF -> DOCX sidecar HTTP layer (stdlib only). API: docs/phase3/PLAN.md section 3.3."""
import hashlib, json, os, shutil, subprocess, sys, tempfile, threading
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
DOCX = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
MERGE_SHA = "9563dd3ae3ff705d571bb49772e43c5f4feb2515b0f178bda62b7ceac666e8a1"


def default_config():
    e = os.environ.get
    return {
        "host": e("HOST", "0.0.0.0"), "port": int(e("PORT", "8080")),
        "slots": int(e("SLOTS", "1")),
        "max_bytes": int(e("MAX_BYTES", str(50 * 1024 * 1024))),
        "max_pages": int(e("MAX_PAGES", "200")),
        # shortcut: one flat timeout instead of 60 s + 3 s/page; refine when WP9 measures real jobs.
        "timeout_s": float(e("JOB_TIMEOUT_S", "300")),
        "mem_bytes": int(e("WORKER_MEM_BYTES", str(1536 * 1024 * 1024))),
        "worker": e("WORKER", os.path.join(HERE, "worker.py")),
    }


def versions():
    import importlib.metadata as md
    h = hashlib.sha256(Path(HERE, "merge_lines.py").read_bytes()).hexdigest()
    return {"pdf2docx": md.version("pdf2docx"), "pymupdf": md.version("PyMuPDF"),
            "python-docx": md.version("python-docx"),
            "merge_lines": h[:8] + ("" if h == MERGE_SHA else "-MODIFIED")}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "pdf2docx-sidecar"

    def log_message(self, fmt, *a):
        sys.stderr.write("%s\n" % (fmt % a))

    def send(self, status, body, ctype="application/json", headers=()):
        if isinstance(body, dict):
            body = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in headers:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def err(self, status, code, message, headers=(), close=False):
        if close:
            self.close_connection = True  # unread request body: do not reuse the socket
        self.send(*self.errv(status, code, message, headers))

    @staticmethod
    def errv(status, code, message, headers=()):
        return status, {"code": code, "message": message}, "application/json", headers

    def do_GET(self):
        if urlparse(self.path).path != "/healthz":
            return self.err(404, "not_found", "no such route")
        s = self.server
        self.send(200, {"ok": True, "versions": s.versions, "busy": s.cfg["slots"] - s.sem._value,
                        "slots": s.cfg["slots"]})

    def do_POST(self):
        u = urlparse(self.path)
        if u.path != "/v1/pdf2docx":
            return self.err(404, "not_found", "no such route", close=True)
        cfg = self.server.cfg
        try:
            n = int(self.headers.get("Content-Length", ""))
        except ValueError:
            return self.err(411, "length_required", "Content-Length is required", close=True)
        if n > cfg["max_bytes"]:
            return self.err(413, "too_large", "body over %d bytes" % cfg["max_bytes"], close=True)
        if not self.server.sem.acquire(blocking=False):
            return self.err(503, "busy", "all slots are busy", [("Retry-After", "5")], close=True)
        try:  # release the slot before replying, so a client that retries at once never sees a stale 503
            out = self.convert(self.rfile.read(n), parse_qs(u.query))
        finally:
            self.server.sem.release()
        self.send(*out)

    def convert(self, data, q):
        cfg = self.server.cfg
        if not data.startswith(b"%PDF-"):
            return self.errv(400, "bad_pdf", "not a PDF (missing %PDF- header)")
        opts = {"merge": q.get("merge", ["1"])[0] != "0", "pages": q.get("pages", [""])[0],
                "max_pages": cfg["max_pages"], "mem_bytes": cfg["mem_bytes"]}
        tmp = tempfile.mkdtemp(prefix="job-")
        try:
            src, dst, op = (os.path.join(tmp, x) for x in ("in.pdf", "out.docx", "opts.json"))
            Path(src).write_bytes(data)
            Path(op).write_text(json.dumps(opts))
            try:
                r = subprocess.run([sys.executable, "-I", cfg["worker"], src, dst, op], capture_output=True,
                                   text=True, timeout=cfg["timeout_s"], cwd=tmp)
            except subprocess.TimeoutExpired:  # subprocess.run kills the child before raising
                return self.errv(504, "timeout", "conversion exceeded %g s" % cfg["timeout_s"])
            try:
                res = json.loads(r.stdout.strip().splitlines()[-1])
            except (IndexError, ValueError):
                res = None
            if r.returncode != 0 or res is None:
                sys.stderr.write("worker crashed rc=%s: %s\n" % (r.returncode, r.stderr[-500:]))
                return self.errv(500, "worker_crashed", "the conversion worker died (rc=%s)" % r.returncode)
            if not res["ok"]:
                return self.errv(res["status"], res["code"], res["message"])
            return 200, Path(dst).read_bytes(), DOCX, [("X-Convert-Report", json.dumps(res["report"]))]
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


def make_server(cfg=None, vers=None):
    cfg = cfg or default_config()
    s = ThreadingHTTPServer((cfg["host"], cfg["port"]), Handler)
    s.daemon_threads = True
    s.cfg, s.sem = cfg, threading.BoundedSemaphore(cfg["slots"])
    s.versions = vers if vers is not None else versions()
    return s


if __name__ == "__main__":
    make_server().serve_forever()
