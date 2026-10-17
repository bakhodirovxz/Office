"""Desktop brand evidence (WP7): launch a release exe with its control port, print the window title (Get-Process
MainWindowTitle), screenshot the window and the About dialog through the app's own control channel.

    python -I scripts/desktop-brand-evidence.py <docs|sheets|slides> <exe> <outdir> [--port N]

The window capture with the title bar (brand-<app>-desktop.png) is made by scripts/window-shot.ps1.
wordcraft, gridcraft and deckcraft share one protocol: `--control PORT`, a key file `<settings>/control-key.local-<port>`,
JSON lines over loopback. (pdfcraft uses a control file instead: see the pdf section of the evidence README.)
"""
import json, os, socket, subprocess, sys, time

app, exe, outdir = sys.argv[1], sys.argv[2], sys.argv[3]
port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7981 + ["docs", "sheets", "slides"].index(app)
settings = {"docs": "WordCraft", "sheets": "GridCraft", "slides": "DeckCraft"}[app]
about = {
    "docs": ("engine.execute", {"command": "ui.dialog", "params": {"name": "about"}}),
    "sheets": ("ui.dialog", {"name": "about"}),
    "slides": ("engine.execute", {"command": "app.dialog", "params": {"id": "about"}}),
}[app]
os.makedirs(outdir, exist_ok=True)
# wordcraft keeps a per-instance key file in its settings folder; gridcraft and deckcraft have no key.
keyfile = os.path.join(os.environ["APPDATA"], settings, f"control-key.local-{port}") if app == "docs" else None
if keyfile and os.path.exists(keyfile):
    os.remove(keyfile)
proc = subprocess.Popen([exe, "--control", str(port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    key = None
    for _ in range(240):
        if keyfile and os.path.exists(keyfile):
            key = open(keyfile).read().strip()
            break
        if not keyfile:
            try:
                socket.create_connection(("127.0.0.1", port), timeout=1).close()
                break
            except OSError:
                pass
        time.sleep(0.5)
    time.sleep(8)  # first frames
    title = subprocess.run(["powershell", "-NoProfile", "-Command", f"[Console]::OutputEncoding = [Text.Encoding]::UTF8; (Get-Process -Id {proc.pid}).MainWindowTitle"], capture_output=True, text=True, encoding="utf-8").stdout.strip()
    s = socket.create_connection(("127.0.0.1", port), timeout=60)
    f = s.makefile("rw", encoding="utf-8", newline="\n")
    n = 0

    def call(method, params=None):
        global n
        n += 1
        req = {"id": n, "method": method, "params": params or {}}
        if key:
            req["key"] = key
        f.write(json.dumps(req) + "\n")
        f.flush()
        return json.loads(f.readline())

    shot = lambda name: call("ui.screenshot", {"path": os.path.abspath(os.path.join(outdir, name))})
    method, params = about
    r2 = call(method, params)
    time.sleep(2)
    r3 = shot(f"brand-{app}-desktop-about.png")
    state = call("ui.inspect")
    print(json.dumps({"app": app, "exe": exe, "window_title": title, "about_cmd": r2.get("ok", r2.get("error")), "about": r3.get("ok"), "dialog": json.dumps(state.get("result", {}).get("dialog"))[:120]}))
finally:
    proc.kill()
