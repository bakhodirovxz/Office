"""Docs toolbar evidence (Phase 4 WP2): launch the debug docs.exe with its control port, an empty profile and an empty settings
folder, switch to uz-Latn and screenshot the simple ribbon (Asosiy, Hujjat) and the full ribbon through the app's own control channel.

    python -I scripts/docs-toolbar-evidence.py <docs.exe> <outdir> [--port N] [--lang uz-latn]
"""
import json, os, socket, subprocess, sys, tempfile, time

exe, outdir = sys.argv[1], os.path.abspath(sys.argv[2])
port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7991
lang = sys.argv[sys.argv.index("--lang") + 1] if "--lang" in sys.argv else "uz-latn"
os.makedirs(outdir, exist_ok=True)
prof = tempfile.mkdtemp(prefix="docs-evidence-")
env = dict(os.environ, WORDCRAFT_PROFILE_DIR=prof, SUITE_SETTINGS_DIR=prof)
proc = subprocess.Popen([exe, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    keyfile = os.path.join(prof, f"control-key.local-{port}")
    key = None
    for _ in range(240):
        if os.path.exists(keyfile):
            key = open(keyfile).read().strip()
            break
        time.sleep(0.5)
    time.sleep(6)
    f = socket.create_connection(("127.0.0.1", port), timeout=60).makefile("rw", encoding="utf-8", newline="\n")
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

    ex = lambda cmd, p=None: call("engine.execute", {"command": cmd, "params": p or {}})
    shot = lambda name: print(name, call("ui.screenshot", {"path": os.path.join(outdir, name)}).get("ok"))
    print("language", ex("ui.language", {"value": lang}).get("result", {}).get("effective"))
    time.sleep(2)
    shot(f"docs-simple-asosiy-{lang}.png")
    # A no-op query (params make it skip the dialog): the layout as the app holds it.
    layout = ex("tools.customizeRibbon", {"qatRemove": "no.such.command"}).get("result", {})
    print("tabs", json.dumps([t["name"] for t in layout.get("tabs", []) if not t.get("hidden")], ensure_ascii=False), "qat", layout.get("qat"))
    tab = next((t["name"] for t in layout.get("tabs", []) if t.get("custom")), "Hujjat")
    ex("ui.tab", {"tab": tab})
    time.sleep(1.5)
    shot(f"docs-simple-hujjat-{lang}.png")
    print("toggle", ex("office.ribbonFull"))
    time.sleep(2)
    shot(f"docs-full-{lang}.png")
    print("toggle back", ex("office.ribbonSimple"))
    time.sleep(2)
    ex("ui.tab", {"tab": "Home"})
    shot(f"docs-simple-again-{lang}.png")
    print("settings.toml:", open(os.path.join(prof, "settings.toml")).read() if os.path.exists(os.path.join(prof, "settings.toml")) else "(none)")
finally:
    proc.kill()
