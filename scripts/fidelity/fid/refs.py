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
def _okey(j):
    return str(j["out"]).replace("\\", "/")


def _office_once(app, jobs, work, tag, per_item):
    """One PowerShell + one Office instance over `jobs`. office_ref.ps1 appends one JSON line per finished item to <tag>_progress.jsonl, so
    what a hang cannot take away is kept. A watchdog kills our Office PID (and only that) when no item finishes within `per_item` seconds
    (+60 s for the first one: Office starting up). -> ({out: result}, hung job or None)."""
    work.mkdir(parents=True, exist_ok=True)
    lst, res, pidf, prog = work / ("_%s_list.json" % tag), work / ("_%s_result.json" % tag), work / ("_%s.pid" % tag), work / ("_%s_progress.jsonl" % tag)
    lst.write_text(json.dumps([{k: str(v).replace("/", "\\") for k, v in j.items()} for j in jobs]), encoding="utf-8")
    for f in (res, pidf, prog):
        if f.exists():
            f.unlink()
    env = dict(os.environ, FID_PIDFILE=str(pidf))
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "office_ref.ps1"), "-App", app, "-List", str(lst), "-Result", str(res),
           "-Progress", str(prog)]
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, env=env)
    done, last, t0, hung = 0, time.time(), time.time(), False
    while p.poll() is None:
        time.sleep(1)
        n = len(prog.read_text(encoding="utf-8-sig").splitlines()) if prog.exists() else 0
        if n != done:
            done, last = n, time.time()
        if time.time() - last > per_item + (60 if done == 0 else 0):
            hung = True
            if pidf.exists():   # kill exactly the Office we started, then the script that waits for it
                subprocess.run(["taskkill", "/PID", pidf.read_text().strip(), "/F"], capture_output=True)
            p.kill()
            break
    err = p.stderr.read().decode("utf-8", "replace")[-300:] if p.stderr else ""
    p.wait()
    if p.stderr:
        p.stderr.close()
    if res.exists():
        got = {d["out"].replace("\\", "/"): d for d in json.loads(res.read_text(encoding="utf-8-sig"))}
    else:   # killed or crashed: the items that finished are in the progress file
        got = {}
        for line in (prog.read_text(encoding="utf-8-sig").splitlines() if prog.exists() else []):
            try:
                d = json.loads(line)
                got[d["out"].replace("\\", "/")] = d
            except ValueError:
                pass
    pending = [j for j in jobs if _okey(j) not in got]
    if pending and hung:   # the items run in order: the first without an answer is the one Office hung on
        got[_okey(pending[0])] = {"ok": False, "error": "Office hung on this file (no answer in %d s; our Office PID was killed): timeout" % per_item}
        return got, pending[0]
    if pending:   # the script failed without a hang: nothing more can be learned from a retry of the same jobs
        for j in pending:
            got[_okey(j)] = {"ok": False, "error": "office batch failed: " + err}
    return got, None


def office_batch(app, jobs, work, tag, per_item=90):
    """jobs: [{src, out, fmt}] for ONE app. -> {out: {ok, error, ms}}. One Office instance at a time; a hang on one file costs that file only:
    it is marked as a timeout, our Office PID is killed, and the rest of the batch runs in a fresh instance."""
    out, todo, i = {}, list(jobs), 0
    while todo:
        got, hung = _office_once(app, todo, work, "%s_%d" % (tag, i), per_item)
        out.update(got)
        todo = [j for j in todo if _okey(j) not in out]
        i += 1
        if not hung and todo:   # defensive: no progress and no hang
            for j in todo:
                out[_okey(j)] = {"ok": False, "error": "office batch made no progress"}
            break
    return out


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
    from a throw-away HTTP server on this host (host.docker.internal).

    Start the reference container with BOTH settings, or every job fails with error -4 (the server refuses to download from a private IP):
      docker run -d --name fid-x2t -p 18082:80 -e JWT_ENABLED=false -e ALLOW_PRIVATE_IP_ADDRESS=true onlyoffice/documentserver:8.2.3
      X2T_URL=http://127.0.0.1:18082   (Git Bash: MSYS_NO_PATHCONV=1 in front of docker). Remove the container afterwards."""
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
                hint = " (start the container with -e ALLOW_PRIVATE_IP_ADDRESS=true)" if j.get("error") == -4 else ""
                return False, "x2t error %s%s" % (j.get("error", "no fileUrl"), hint)
            u = urllib.parse.urlparse(j["fileUrl"]); b = urllib.parse.urlparse(self.url)
            with urllib.request.urlopen(u._replace(netloc=b.netloc).geturl(), timeout=120) as r:
                Path(out).write_bytes(r.read())
            return True, "ok"
        except Exception as e:
            return False, "x2t request failed: %s" % type(e).__name__
        finally:
            srv.shutdown()
