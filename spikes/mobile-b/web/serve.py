"""Throwaway dev server for the spike: serves a dist dir, wasm with the right MIME type, and
POST /__save?name=X writes the body to <saves>/X (evidence that Save produced a real docx).
usage: python -I serve.py DIST_DIR SAVES_DIR [PORT]"""
import http.server, os, sys, urllib.parse

dist, saves = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
port = int(sys.argv[3]) if len(sys.argv) > 3 else 8780
os.makedirs(saves, exist_ok=True)

class H(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript", ".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document"}
    def __init__(self, *a, **k):
        super().__init__(*a, directory=dist, **k)
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
    def do_POST(self):
        u = urllib.parse.urlparse(self.path)
        if u.path != "/__save":
            self.send_error(404); return
        name = os.path.basename(urllib.parse.parse_qs(u.query).get("name", ["saved.docx"])[0])
        n = int(self.headers.get("Content-Length", 0))
        data = self.rfile.read(n)
        with open(os.path.join(saves, name), "wb") as f:
            f.write(data)
        self.send_response(204); self.end_headers()
        print("saved", name, len(data), flush=True)
    def log_message(self, *a): pass

http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
