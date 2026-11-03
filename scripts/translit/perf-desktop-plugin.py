"""Phase 7 N2 (PLAN section 6.3): the transliteration command on the 108-page document in the release desktop app (docs.exe), built-in
against the uz-translit plugin, alternating, over the control channel (as perf-desktop.py).

    python -I scripts/translit/perf-desktop-plugin.py <docs.exe> <uz-translit-1.0.0.zip> [--rounds 6] [--out result.json]

A temporary %APPDATA% holds a user store with the packed plugin installed and enabled (the install record is the digests of the two
files, as `Store::install_zip` writes it), so the real loader runs: verify, compile, register. Every round starts a fresh process
per mode (built-in, then plugin), waits until the document is laid out and idle and (plugin) until the command is registered, then
times from the client side: cmd (`engine.execute`), inspect (`ui.inspect` straight after: the next frame), shot (`ui.screenshot`
straight after: a frame with the new layout painted and read back; an upper bound). The first round is a warm-up. The summary is the
median of the others per mode, and the plugin's cost over the built-in end to end as a percentage of `shot` and of `inspect`.
"""
import hashlib, json, os, re, shutil, socket, statistics, subprocess, sys, tempfile, time, zipfile

exe, zpath = sys.argv[1], sys.argv[2]
rounds = int(sys.argv[sys.argv.index("--rounds") + 1]) if "--rounds" in sys.argv else 6
out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else None
fx = os.path.abspath("domains/docs/crates/docs-translit/tests/fixtures")
cases = [("uz108-latn.docx", "tools.translitCyrl", "plugin.uz-translit.toCyrillic"),
         ("uz108-cyrl.docx", "tools.translitLatn", "plugin.uz-translit.toLatin")]
port = 7991
product = re.search(r'(?m)^name = "([^"]+)"', open("brand/brand.toml", encoding="utf-8").read().split("[product]")[1]).group(1)

appdata = tempfile.mkdtemp(prefix="n2-plugin-")
settings = os.path.join(appdata, product)
pkg = os.path.join(settings, "plugins", "uz-translit", "1.0.0")
os.makedirs(pkg)
with zipfile.ZipFile(zpath) as z:
    z.extractall(pkg)
sha = lambda n: hashlib.sha256(open(os.path.join(pkg, n), "rb").read()).hexdigest()
open(os.path.join(pkg, "install.toml"), "w").write(f'sha256 = "{sha("plugin.wasm")}"\nmanifest_sha256 = "{sha("plugin.toml")}"\n')
env_plugin = dict(os.environ, APPDATA=appdata)
# the built-in runs get an empty settings folder: no plugin enabled
empty = tempfile.mkdtemp(prefix="n2-none-")
env_builtin = dict(os.environ, APPDATA=empty)
open(os.path.join(settings, "settings.toml"), "w").write('[plugins]\nenabled = ["uz-translit"]\n')


def session(path, env):
    keyfile = os.path.join(env["APPDATA"], "WordCraft", f"control-key.local-{port}")
    if os.path.exists(keyfile):
        os.remove(keyfile)
    proc = subprocess.Popen([exe, path, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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

    return proc, call


def one(name, mode, cmd, env, rnd):
    proc, call = session(os.path.join(fx, name), env)
    try:
        last = None
        for _ in range(240):
            st = call("ui.inspect").get("result", {})
            pages = len(st.get("pages", []))
            lay = st.get("perf", {}).get("layoutMs")
            if pages and lay and last == (pages, lay):
                break
            last = (pages, lay)
            time.sleep(0.5)
        if mode == "plugin":  # the loader runs on a thread: wait until the command is registered
            for _ in range(60):
                cs = call("engine.commands").get("result", [])
                if any((c.get("id") if isinstance(c, dict) else c) == cmd for c in (cs.get("commands", cs) if isinstance(cs, dict) else cs)):
                    break
                time.sleep(0.5)
            else:
                raise SystemExit(f"{cmd} never registered: plugin not loaded")
        time.sleep(2)
        before = call("ui.inspect")["result"]
        t0 = time.perf_counter()
        r = call("engine.execute", {"command": cmd, "params": {}})
        t1 = time.perf_counter()
        call("ui.inspect")
        t2 = time.perf_counter()
        call("ui.screenshot", {"path": os.path.join(tempfile.gettempdir(), "translit-n2p.png")})
        t3 = time.perf_counter()
        after = call("ui.inspect")["result"]
        row = {"file": name, "mode": mode, "round": rnd, "ok": r.get("ok"), "cmd_ms": (t1 - t0) * 1e3, "inspect_ms": (t2 - t0) * 1e3, "shot_ms": (t3 - t0) * 1e3,
               "layoutMs_after": after["perf"]["layoutMs"], "pages_before": len(before["pages"]), "pages_after": len(after["pages"])}
        print(json.dumps(row), flush=True)
        return row
    finally:
        proc.kill()
        time.sleep(2)


results = []
try:
    for name, builtin, plugin in cases:
        rows = {"builtin": [], "plugin": []}
        for rnd in range(rounds):
            rows["builtin"].append(one(name, "builtin", builtin, env_builtin, rnd))
            rows["plugin"].append(one(name, "plugin", plugin, env_plugin, rnd))
        med = lambda m, k: statistics.median(r[k] for r in rows[m][1:])
        for k in ("cmd_ms", "inspect_ms", "shot_ms"):
            pass
        summary = {"file": name, "rounds_timed": rounds - 1}
        for m in rows:
            for k in ("cmd_ms", "inspect_ms", "shot_ms"):
                summary[f"{m}_{k}"] = med(m, k)
        for k in ("inspect_ms", "shot_ms"):
            summary[f"plugin_over_builtin_{k}_pct"] = (summary[f"plugin_{k}"] / summary[f"builtin_{k}"] - 1) * 100
        print(json.dumps(summary), flush=True)
        results.append({"summary": summary, "rows": rows})
finally:
    shutil.rmtree(appdata, ignore_errors=True)
    shutil.rmtree(empty, ignore_errors=True)
if out:
    json.dump(results, open(out, "w"), indent=1)
