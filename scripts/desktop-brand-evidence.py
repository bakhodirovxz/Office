"""Desktop brand evidence (WP7): launch a release exe with its control port, print the window title (Get-Process
MainWindowTitle), screenshot the window and the About dialog through the app's own control channel.

    python -I scripts/desktop-brand-evidence.py <docs|sheets|slides> <exe> <outdir> [--port N]

wordcraft, gridcraft and deckcraft share one protocol: `--control PORT`, a key file `<settings>/control-key.local-<port>`,
JSON lines over loopback. (pdfcraft uses a control file instead: see the pdf section of the evidence README.)
"""
import json, os, socket, subprocess, sys, time

app, exe, outdir = sys.argv[1], sys.argv[2], sys.argv[3]
port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7981 + ["docs", "sheets", "slides"].index(app)
settings = {"docs": "WordCraft", "sheets": "GridCraft", "slides": "DeckCraft"}[app]
about = {
    "docs": ("engine.execute", {"command": "ui.dialog", "params": {"name": "about"}}),
    "sheets": ("engine.execute", {"command": "ui.dialog", "params": {"name": "about"}}),
    "slides": ("engine.execute", {"command": "app.about", "params": {}}),
}[app]
os.makedirs(outdir, exist_ok=True)
keyfile = os.path.join(os.environ["APPDATA"], settings, f"control-key.local-{port}")
if os.path.exists(keyfile):
    os.remove(keyfile)
proc = subprocess.Popen([exe, "--control", str(port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(120):
        if os.path.exists(keyfile):
            break
        time.sleep(0.5)
    key = open(keyfile).read().strip()
    time.sleep(8)  # first frames
    title = subprocess.run(["powershell", "-NoProfile", "-Command", f"(Get-Process -Id {proc.pid}).MainWindowTitle"], capture_output=True, text=True).stdout.strip()
    s = socket.create_connection(("127.0.0.1", port), timeout=60)
    f = s.makefile("rw", encoding="utf-8", newline="\n")
    n = 0

    def call(method, params=None):
        global n
        n += 1
        f.write(json.dumps({"id": n, "key": key, "method": method, "params": params or {}}) + "\n")
        f.flush()
        return json.loads(f.readline())

    shot = lambda name: call("ui.screenshot", {"path": os.path.abspath(os.path.join(outdir, name))})
    r1 = shot(f"brand-{app}-desktop.png")
    method, params = about
    r2 = call(method, params)
    time.sleep(2)
    r3 = shot(f"brand-{app}-desktop-about.png")
    state = call("ui.inspect")
    print(json.dumps({"app": app, "exe": exe, "window_title": title, "main": r1.get("ok"), "about_cmd": r2.get("ok", r2.get("error")), "about": r3.get("ok"), "dialog": json.dumps(state.get("result", {}).get("dialog"))[:120]}))
finally:
    proc.kill()
