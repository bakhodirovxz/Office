"""Reference engines: MS Office 16 (COM, primary), LibreOffice (our soffice sidecar over HTTP), ONLYOFFICE x2t (document
server /converter). Every engine answers the same question: "convert this source to this format" -> (ok, reason).
Unavailable engines never raise; they return a reason that ends up in the JSON ("unavailable: ...")."""
import json, os, subprocess, threading, time, urllib.error, urllib.parse, urllib.request, uuid
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
APP_OF = {}
for _a, _e in (("word", "doc docx dot dotx odt rtf html htm txt md pdf"), ("excel", "xls xlsx xlsb ods csv"), ("ppt", "ppt pptx")):
    for _x in _e.split():
        APP_OF[_x] = _a


def app_of(path):
    return APP_OF.get(Path(path).suffix.lower().lstrip("."))


# ---------------------------------------------------------------- Office COM
def office_batch(app, jobs, work, tag, per_item=90):
    """jobs: [{src, out, fmt}] for ONE app. -> {out: {ok, error, ms}}. One Office instance; on a timeout only its PID is killed."""
    if not jobs:
        return {}
    work.mkdir(parents=True, exist_ok=True)
    lst, res, pidf = work / ("_%s_list.json" % tag), work / ("_%s_result.json" % tag), work / ("_%s.pid" % tag)
    lst.write_text(json.dumps([{k: str(v).replace("/", "\\") for k, v in j.items()} for j in jobs]), encoding="utf-8")
    for f in (res, pidf):
        if f.exists():
            f.unlink()
    env = dict(os.environ, FID_PIDFILE=str(pidf))
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "office_ref.ps1"), "-App", app, "-List", str(lst), "-Result", str(res)]
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=120 + per_item * len(jobs), env=env)
        err = p.stderr.decode("utf-8", "replace")[-300:]
    except subprocess.TimeoutExpired:
        err = "timeout"
        if pidf.exists():   # kill exactly the Office we started
            subprocess.run(["taskkill", "/PID", pidf.read_text().strip(), "/F"], capture_output=True)
    if not res.exists():
        return {str(j["out"]).replace("\\", "/"): {"ok": False, "error": "office batch failed: " + err} for j in jobs}
    return {d["out"].replace("\\", "/"): d for d in json.loads(res.read_text(encoding="utf-8-sig"))}


# ---------------------------------------------------------------- LibreOffice sidecar
class Soffice:
    """POST /v1/convert?to=pdf|png|docx|xlsx|pptx (OOXML in), GET /healthz. 503 = busy -> retried."""
    def __init__(self, url=None, disabled=False):
        self.url = (url or os.environ.get("SOFFICE_URL") or "http://127.0.0.1:18081").rstrip("/")
        self.reason = None
        if disabled:
            self.reason = "disabled (--no-soffice)"
            return
        try:
            with urllib.request.urlopen(self.url + "/healthz", timeout=5) as r:
                self.versions = json.loads(r.read().decode("utf-8", "replace"))
        except Exception as e:
            self.reason = "sidecar not reachable at %s (%s); build office/soffice:dev and run sidecars/soffice/run.sh" % (self.url, type(e).__name__)

    def convert(self, src, to, out):
        if self.reason:
            return False, self.reason
        for attempt in range(4):
            req = urllib.request.Request("%s/v1/convert?to=%s" % (self.url, to), data=Path(src).read_bytes(), method="POST",
                                         headers={"Content-Type": "application/octet-stream"})
            try:
                with urllib.request.urlopen(req, timeout=200) as r:
                    Path(out).write_bytes(r.read())
                return True, "ok"
            except urllib.error.HTTPError as e:
                body = e.read().decode("utf-8", "replace")[:200]
                if e.code == 503 and attempt < 3:
                    time.sleep(3); continue
                try:
                    body = json.loads(body).get("code", body)
                except Exception:
                    pass
                return False, "soffice HTTP %s %s" % (e.code, body)
            except Exception as e:
                return False, "soffice request failed: %s" % type(e).__name__
        return False, "soffice busy"


# ---------------------------------------------------------------- ONLYOFFICE x2t (document server)
class _Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


class X2T:
    """POST {url}/converter {async:false,filetype,key,outputtype,title,url} (JWT off). The document server fetches the input
    from a throw-away HTTP server on this host (host.docker.internal)."""
    def __init__(self, url=None, disabled=False):
        self.url = (url or os.environ.get("X2T_URL") or "").rstrip("/")
        self.host = os.environ.get("X2T_HOST", "host.docker.internal")
        self.reason = None
        if disabled:
            self.reason = "disabled (--no-x2t)"
        elif not self.url:
            self.reason = "no document server configured (set X2T_URL; image onlyoffice/documentserver, JWT off)"
        else:
            try:
                with urllib.request.urlopen(self.url + "/healthcheck", timeout=5) as r:
                    if b"true" not in r.read():
                        self.reason = "document server at %s is not healthy" % self.url
            except Exception as e:
                self.reason = "document server not reachable at %s (%s)" % (self.url, type(e).__name__)

    def convert(self, src, to, out):
        if self.reason:
            return False, self.reason
        src = Path(src)
        srv = ThreadingHTTPServer(("0.0.0.0", 0), partial(_Quiet, directory=str(src.parent)))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            body = {"async": False, "filetype": src.suffix.lstrip(".").lower(), "key": uuid.uuid4().hex, "outputtype": to,
                    "title": src.name, "url": "http://%s:%d/%s" % (self.host, srv.server_address[1], urllib.request.quote(src.name))}
            req = urllib.request.Request(self.url + "/converter", data=json.dumps(body).encode(), method="POST",
                                         headers={"Content-Type": "application/json", "Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=240) as r:
                j = json.loads(r.read().decode("utf-8", "replace"))
            if j.get("error") or not j.get("fileUrl"):
                return False, "x2t error %s" % j.get("error", "no fileUrl")
            u = urllib.parse.urlparse(j["fileUrl"]); b = urllib.parse.urlparse(self.url)
            with urllib.request.urlopen(u._replace(netloc=b.netloc).geturl(), timeout=120) as r:
                Path(out).write_bytes(r.read())
            return True, "ok"
        except Exception as e:
            return False, "x2t request failed: %s" % type(e).__name__
        finally:
            srv.shutdown()
