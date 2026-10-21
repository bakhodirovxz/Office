"""LibreOffice reference-renderer HTTP layer (stdlib only). Test-only sidecar for the WP9 fidelity harness.
POST /v1/convert?to=pdf|png|docx|xlsx|pptx  (body: a .docx, .xlsx or .pptx)   GET /healthz
"""
import io, json, os, shutil, signal, subprocess, sys, tempfile, threading, zipfile
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = {"pdf": "application/pdf", "png": "image/png",
        "docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx": "application/vnd.openxmlformats-officedocument.presentationml.presentation"}
KINDS = {"word/": "docx", "xl/": "xlsx", "ppt/": "pptx"}


def default_config():
    e = os.environ.get
    return {
        "host": e("HOST", "0.0.0.0"), "port": int(e("PORT", "8080")),
        "slots": int(e("SLOTS", "2")),
        "max_bytes": int(e("MAX_BYTES", str(50 * 1024 * 1024))),
        "timeout_s": float(e("JOB_TIMEOUT_S", "120")),
        "worker": e("WORKER", os.path.join(HERE, "worker.py")),
    }


def versions():
    def run(*a):
        return subprocess.run(a, capture_output=True, text=True).stdout.strip()
    v = {"libreoffice": run("soffice", "--version")}
    for p in ("libreoffice-writer", "libreoffice-calc", "libreoffice-impress", "fonts-crosextra-carlito",
              "fonts-crosextra-caladea", "fonts-liberation2", "fonts-noto-core"):
        v[p] = run("dpkg-query", "-W", "-f", "${Version}", p)
    return v


def sniff(data):
    """-> (kind, None) or (None, (status, code, message)). Only OOXML zips; legacy binary .doc/.xls/.ppt are refused."""
    if data.startswith(b"\xd0\xcf\x11\xe0"):
        return None, (415, "legacy_format", "legacy binary Office files (.doc/.xls/.ppt) are not accepted (owner O2)")
    try:
        names = zipfile.ZipFile(io.BytesIO(data)).namelist()
    except zipfile.BadZipFile:
        return None, (400, "bad_input", "not an OOXML (zip) file")
    for prefix, kind in KINDS.items():
        if any(n.startswith(prefix) for n in names):
            return kind, None
    return None, (400, "bad_input", "zip without word/, xl/ or ppt/ parts")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "soffice-sidecar"

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
        if u.path != "/v1/convert":
            return self.err(404, "not_found", "no such route", close=True)
        cfg = self.server.cfg
        to = parse_qs(u.query).get("to", [""])[0]
        if to not in MIME:
            return self.err(400, "bad_request", "to must be one of: " + ", ".join(MIME), close=True)
        try:
            n = int(self.headers.get("Content-Length", ""))
        except ValueError:
            return self.err(411, "length_required", "Content-Length is required", close=True)
        if n > cfg["max_bytes"]:
            return self.err(413, "too_large", "body over %d bytes" % cfg["max_bytes"], close=True)
        if not self.server.sem.acquire(blocking=False):
            return self.err(503, "busy", "all slots are busy", [("Retry-After", "5")], close=True)
        try:
            out = self.convert(self.rfile.read(n), to)
        finally:
            self.server.sem.release()
        self.send(*out)

    def convert(self, data, to):
        cfg = self.server.cfg
        kind, bad = sniff(data)
        if bad:
            return self.errv(*bad)
        if to in ("docx", "xlsx", "pptx") and to != kind:
            return self.errv(422, "unsupported_conversion", "%s -> %s is not possible" % (kind, to))
        tmp = tempfile.mkdtemp(prefix="job-")
        try:
            src, outdir, op = os.path.join(tmp, "in." + kind), os.path.join(tmp, "out"), os.path.join(tmp, "opts.json")
            os.mkdir(outdir)
            Path(src).write_bytes(data)
            Path(op).write_text(json.dumps({"to": to}))
            p = subprocess.Popen([sys.executable, "-I", cfg["worker"], src, outdir, op], stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True, cwd=tmp, start_new_session=True)
            try:
                stdout, stderr = p.communicate(timeout=cfg["timeout_s"])
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)  # soffice.bin is a grandchild: kill the whole group
                p.communicate()
                return self.errv(504, "timeout", "conversion exceeded %g s" % cfg["timeout_s"])
            finally:
                try:
                    os.killpg(p.pid, signal.SIGKILL)  # stragglers
                except ProcessLookupError:
                    pass
            try:
                res = json.loads(stdout.strip().splitlines()[-1])
            except (IndexError, ValueError):
                res = None
            if p.returncode != 0 or res is None:
                sys.stderr.write("worker crashed rc=%s: %s\n" % (p.returncode, stderr[-500:]))
                return self.errv(500, "worker_crashed", "the conversion worker died (rc=%s)" % p.returncode)
            if not res["ok"]:
                return self.errv(res["status"], res["code"], res["message"])
            return 200, Path(res["file"]).read_bytes(), MIME[to], []
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
