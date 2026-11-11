"""Phase 7 WP10: hostile test plugins in the real desktop Docs app (docs.exe), over the control channel
(the setup is that of scripts/translit/perf-desktop-plugin.py).

    python -I scripts/plugins/evil-desktop.py <docs.exe> [--port 7992]

The fixture is ci/plugins/webhost-test/plugin.wasm (not a product plugin). `deny` answers with a `run file.saveAs {path}` action (not on
the allowlist, PLAN 5.2); `many` answers with 17 text map reads in one `next` (the cap is 16); `spin` loops forever (M6, the time
budget); `grow` grows its memory until refused (M7, `memory_mb = 2`) and then says so; `xss` asks for a panel with markup and a
10,000-character text (M9). Each failing command must fail and leave the document text as it was; a control command (`hello`, a panel
and a status line) must still work, so the plugin did load. Four more packages hold the modules the host must refuse to load (M1
WASI import, M2 exported memory, M2b an own unexported memory, M2b a wasm GC type): none of their commands may register, the app
must keep running, and its stderr must name each of them. Last, `overflow` (S1, unbounded recursion) must end the invocation, not the process.
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

[[command]]
id = "spin"
label.en = "Spin"
reads = ["none"]

[[command]]
id = "grow"
label.en = "Grow"
reads = ["none"]

[[command]]
id = "xss"
label.en = "Xss"
reads = ["none"]

[[command]]
id = "overflow"
label.en = "Overflow"
reads = ["none"]

[[panel]]
id = "p"
title.en = "Panel"
"""

appdata = tempfile.mkdtemp(prefix="evil-plugin-")
settings = os.path.join(appdata, product)
fixtures = os.path.dirname(wasm)
# plugin id -> module file; the refused modules get a one-command manifest
REFUSED = {"evil-m1": "bad-import.wasm", "evil-m2": "bad-export.wasm", "evil-m2b-mem": "own-memory.wasm", "evil-m2b-gc": "gc-type.wasm"}
SMALL = MANIFEST.split("[[command]]")[0] + '[[command]]\nid = "hello"\nlabel.en = "Hello"\nreads = ["none"]\n'


def install(pid, module, manifest):
    pkg = os.path.join(settings, "plugins", pid, "1.0.0")
    os.makedirs(pkg)
    open(os.path.join(pkg, "plugin.toml"), "w", encoding="utf-8", newline="\n").write(manifest)
    shutil.copy(module, os.path.join(pkg, "plugin.wasm"))
    sha = lambda n: hashlib.sha256(open(os.path.join(pkg, n), "rb").read()).hexdigest()
    open(os.path.join(pkg, "install.toml"), "w").write(f'sha256 = "{sha("plugin.wasm")}"\nmanifest_sha256 = "{sha("plugin.toml")}"\n')


install("evil-test", wasm, MANIFEST)
for pid, mod in REFUSED.items():
    install(pid, os.path.join(fixtures, mod), SMALL.replace("evil-test", pid))
open(os.path.join(settings, "settings.toml"), "w").write("[plugins]\nenabled = [%s]\n" % ", ".join('"%s"' % p for p in ["evil-test", *REFUSED]))
errfile = os.path.join(appdata, "stderr.txt")
env = dict(os.environ, APPDATA=appdata)
keyfile = os.path.join(appdata, "WordCraft", f"control-key.local-{port}")
proc = subprocess.Popen([exe, docx, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=open(errfile, "w"))
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
    def ids():
        cs = call("engine.commands").get("result", [])
        return [c.get("id") if isinstance(c, dict) else c for c in (cs.get("commands", cs) if isinstance(cs, dict) else cs)]

    for cmd, row in (("deny", "M5"), ("many", "R1"), ("spin", "M6"), ("xss", "M9")):
        t0 = time.perf_counter()
        r = call("engine.execute", {"command": f"plugin.evil-test.{cmd}", "params": {}})
        secs = time.perf_counter() - t0
        same = text() == before
        print(json.dumps({"row": row, "command": cmd, "ok": r.get("ok"), "error": str(r.get("error") or r.get("result"))[:160], "document_unchanged": same, "seconds": round(secs, 1)}), flush=True)
        ok &= r.get("ok") is False and same
    # M7: memory growth stops at memory_mb = 2 (the fixture says so with a status), the document is untouched
    r = call("engine.execute", {"command": "plugin.evil-test.grow", "params": {}})
    same = text() == before
    print(json.dumps({"row": "M7", "command": "grow", "ok": r.get("ok"), "result": str(r.get("result") or r.get("error"))[:160], "document_unchanged": same}), flush=True)
    ok &= r.get("ok") is True and same and "hog capped" in str(r.get("result"))
    r = call("engine.execute", {"command": "plugin.evil-test.hello", "params": {}})
    print(json.dumps({"command": "hello", "ok": r.get("ok")}), flush=True)
    ok &= r.get("ok") is True
    # M1, M2, M2b: the refused modules registered nothing, and the app said why on stderr
    have = ids()
    err = open(errfile, encoding="utf-8", errors="replace").read()
    for pid in REFUSED:
        loaded = any(str(i).startswith(f"plugin.{pid}.") for i in have)
        said = any(line.startswith(f"plugins: {pid}:") for line in err.splitlines())
        print(json.dumps({"row": {"evil-m1": "M1", "evil-m2": "M2"}.get(pid, "M2b"), "plugin": pid, "command_registered": loaded, "stderr_names_it": said}), flush=True)
        ok &= not loaded and said
    # S1: unbounded recursion ends the invocation, not the process (last: if the process died, the check after it fails)
    r = call("engine.execute", {"command": "plugin.evil-test.overflow", "params": {}})
    r2 = call("engine.execute", {"command": "plugin.evil-test.hello", "params": {}})
    print(json.dumps({"row": "S1", "command": "overflow", "ok": r.get("ok"), "error": str(r.get("error") or r.get("result"))[:160], "document_unchanged": text() == before, "alive_after": r2.get("ok")}), flush=True)
    ok &= r.get("ok") is False and text() == before and r2.get("ok") is True
finally:
    proc.kill()
    time.sleep(1)
    shutil.rmtree(appdata, ignore_errors=True)
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
