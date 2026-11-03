"""Phase 7 WP10: two hostile test commands in the real desktop Docs app (docs.exe), over the control channel
(the setup is that of scripts/translit/perf-desktop-plugin.py).

    python -I scripts/plugins/evil-desktop.py <docs.exe> [--port 7992]

The fixture is ci/plugins/webhost-test/plugin.wasm (not a product plugin). `deny` answers with a `run file.saveAs {path}` action (not on
the allowlist, PLAN 5.2); `many` answers with 17 text map reads in one `next` (the cap is 16). Each command must fail and leave the
document text as it was; a control command (`hello`, a panel and a status line) must still work, so the plugin did load.
Exit code 0 only if all of that holds.
"""
import hashlib, json, os, re, shutil, socket, subprocess, sys, tempfile, time

exe = sys.argv[1]
port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7992
wasm = os.path.abspath("ci/plugins/webhost-test/plugin.wasm")
docx = os.path.abspath("domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx")
product = re.search(r'(?m)^name = "([^"]+)"', open("brand/brand.toml", encoding="utf-8").read().split("[product]")[1]).group(1)
MANIFEST = """id = "evil-test"
version = "1.0.0"
api = 1
apps = ["docs"]
publisher = "Test"
name.en = "Evil test"

[permissions]
document = "write"
memory_mb = 2

[[command]]
id = "hello"
label.en = "Hello"
reads = ["none"]

[[command]]
id = "deny"
label.en = "Deny"
reads = ["none"]

[[command]]
id = "many"
label.en = "Many"
reads = ["none"]

[[panel]]
id = "p"
title.en = "Panel"
"""

appdata = tempfile.mkdtemp(prefix="evil-plugin-")
settings = os.path.join(appdata, product)
pkg = os.path.join(settings, "plugins", "evil-test", "1.0.0")
os.makedirs(pkg)
open(os.path.join(pkg, "plugin.toml"), "w", encoding="utf-8", newline="\n").write(MANIFEST)
shutil.copy(wasm, os.path.join(pkg, "plugin.wasm"))
sha = lambda n: hashlib.sha256(open(os.path.join(pkg, n), "rb").read()).hexdigest()
open(os.path.join(pkg, "install.toml"), "w").write(f'sha256 = "{sha("plugin.wasm")}"\nmanifest_sha256 = "{sha("plugin.toml")}"\n')
open(os.path.join(settings, "settings.toml"), "w").write('[plugins]\nenabled = ["evil-test"]\n')
env = dict(os.environ, APPDATA=appdata)
keyfile = os.path.join(appdata, "WordCraft", f"control-key.local-{port}")
proc = subprocess.Popen([exe, docx, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ok = True
try:
    key = None
    for _ in range(240):
        if os.path.exists(keyfile) and open(keyfile).read().strip():
            key = open(keyfile).read().strip()
            break
        time.sleep(0.5)
    s = socket.create_connection(("127.0.0.1", port), timeout=120)
    f = s.makefile("rw", encoding="utf-8", newline="\n")
    n = [0]

    def call(method, params=None):
        n[0] += 1
        req = {"id": n[0], "method": method, "params": params or {}}
        if key:
            req["key"] = key
        f.write(json.dumps(req) + "\n")
        f.flush()
        return json.loads(f.readline())

    for _ in range(120):
        cs = call("engine.commands").get("result", [])
        ids = [c.get("id") if isinstance(c, dict) else c for c in (cs.get("commands", cs) if isinstance(cs, dict) else cs)]
        if "plugin.evil-test.deny" in ids:
            break
        time.sleep(0.5)
    else:
        raise SystemExit("plugin.evil-test.deny never registered: the plugin did not load")
    time.sleep(2)
    text = lambda: json.dumps(call("engine.execute", {"command": "document.text", "params": {}}).get("result"), sort_keys=True)
    before = text()
    for cmd in ("deny", "many"):
        r = call("engine.execute", {"command": f"plugin.evil-test.{cmd}", "params": {}})
        same = text() == before
        print(json.dumps({"command": cmd, "ok": r.get("ok"), "error": str(r.get("error") or r.get("result"))[:160], "document_unchanged": same}), flush=True)
        ok &= r.get("ok") is False and same
    r = call("engine.execute", {"command": "plugin.evil-test.hello", "params": {}})
    print(json.dumps({"command": "hello", "ok": r.get("ok")}), flush=True)
    ok &= r.get("ok") is True
finally:
    proc.kill()
    time.sleep(1)
    shutil.rmtree(appdata, ignore_errors=True)
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
