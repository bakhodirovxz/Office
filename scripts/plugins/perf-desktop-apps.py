"""Phase 7 WP13 N2 (PLAN section 6.3) for Sheets and Slides: the transliteration command in the release desktop app, built-in against the
uz-translit plugin, alternating, over the control channel (as scripts/translit/perf-desktop-plugin.py does for Docs).

    python -I scripts/plugins/perf-desktop-apps.py <sheets.exe|slides.exe> <uz-translit-1.0.0.zip> <sheets|slides> [--rounds 4] [--size N] [--port 7993] [--out result.json]

A temporary %APPDATA% holds a user store with the packed plugin installed and enabled (the install record is the digests of the two
files, as `Store::install_zip` writes it), so the real loader runs: verify, compile, register. The document is built through the
control channel in every fresh process (Sheets: `--size` rows x 5 columns of Latin text and one number column, default 20,000;
Slides: `--size` slides with a text box of 8 paragraphs, default 400). Then, per mode, times from the client side: cmd
(`engine.execute`), inspect (`ui.inspect` straight after: the next frame), shot (`ui.screenshot` straight after: a frame with the
result painted and read back; an upper bound). The first round is a warm-up. The summary is the median of the others per mode.
"""
import hashlib, json, os, re, shutil, socket, statistics, subprocess, sys, tempfile, time, zipfile

exe, zpath, app = sys.argv[1], sys.argv[2], sys.argv[3]
arg = lambda n, d: (type(d) if d is not None else str)(sys.argv[sys.argv.index(n) + 1]) if n in sys.argv else d
rounds, port, out = arg("--rounds", 4), arg("--port", 7993), arg("--out", None)
size = arg("--size", 20000 if app == "sheets" else 400)
cases = [("Cyrillic", "tools.translitCyrl", "plugin.uz-translit.toCyrillic")]
scope = {"sheets": ("workbook", "document"), "slides": ("deck", "document")}[app]
product = re.search(r'(?m)^name = "([^"]+)"', open("brand/brand.toml", encoding="utf-8").read().split("[product]")[1]).group(1)
WORDS = ["Toshkent shahri", "O'zbekiston Respublikasi", "yangi ish o'rni", "Xodimlar bo'limi", "mehnat shartnomasi", "ish haqi"]

appdata = tempfile.mkdtemp(prefix="n2-apps-plugin-")
settings = os.path.join(appdata, product)
pkg = os.path.join(settings, "plugins", "uz-translit", "1.0.0")
os.makedirs(pkg)
with zipfile.ZipFile(zpath) as z:
    z.extractall(pkg)
sha = lambda n: hashlib.sha256(open(os.path.join(pkg, n), "rb").read()).hexdigest()
open(os.path.join(pkg, "install.toml"), "w").write(f'sha256 = "{sha("plugin.wasm")}"\nmanifest_sha256 = "{sha("plugin.toml")}"\n')
env_plugin = dict(os.environ, APPDATA=appdata)
empty = tempfile.mkdtemp(prefix="n2-apps-none-")   # the built-in runs get an empty settings folder: no plugin enabled
env_builtin = dict(os.environ, APPDATA=empty)
open(os.path.join(settings, "settings.toml"), "w").write('[plugins]\nenabled = ["uz-translit"]\n')


def session(env):
    errf = os.path.join(tempfile.gettempdir(), "n2-apps-stderr.txt")  # the last process's stderr, for a crash
    proc = subprocess.Popen([exe, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=open(errf, "w"))
    n = [0]
    box = {}

    def connect():
        s = socket.create_connection(("127.0.0.1", port), timeout=120)
        box["f"] = s.makefile("rw", encoding="utf-8", newline="\n")

    def call(method, params=None):
        n[0] += 1
        box["f"].write(json.dumps({"id": n[0], "method": method, "params": params or {}}) + "\n")
        box["f"].flush()
        return json.loads(box["f"].readline())

    for _ in range(240):  # the port may still belong to the process of the round before: a connection that does not answer is dropped
        try:
            connect()
            call("ui.inspect")
            break
        except (OSError, ValueError):
            time.sleep(0.5)
    return proc, call


def build(call):
    ex = lambda c, p=None: call("engine.execute", {"command": c, "params": p or {}})
    if app == "sheets":
        values = [[WORDS[(i + c) % len(WORDS)] if c < 4 else float(i) for c in range(5)] for i in range(size)]
        r = ex("range.setValues", {"range": "A1", "values": values})
        assert r.get("ok"), r
        ex("selection.set", {"range": "A1"})
    else:
        assert ex("file.new").get("ok")
        for i in range(size):
            if i:
                assert ex("slide.new", {"layout": "blank"}).get("ok")
            r = ex("shape.insert", {"preset": "rect", "rect": [40.0, 40.0, 400.0, 120.0], "text": "\n".join(f"{WORDS[(i + k) % len(WORDS)]} {k}" for k in range(8))})
            assert r.get("ok"), r
        ex("slide.go", {"index": 0})


def one(label, mode, cmd, env, rnd):
    proc, call = session(env)
    try:
        time.sleep(3)
        build(call)
        if mode == "plugin":  # the loader runs on a thread, and these editors' `engine.commands` does not list plugin commands: probe with a scope no command accepts
            for _ in range(120):
                r = call("engine.execute", {"command": cmd, "params": {"scope": "zzz"}})
                if "unknown command" not in str(r.get("error", "")).lower() and "no such command" not in str(r.get("error", "")).lower():
                    break
                time.sleep(0.5)
            else:
                raise SystemExit(f"{cmd} never registered: plugin not loaded")
        for _ in range(4):
            call("ui.inspect")
            time.sleep(0.5)
        time.sleep(2)
        sc = scope[1] if mode == "plugin" else scope[0]
        t0 = time.perf_counter()
        r = call("engine.execute", {"command": cmd, "params": {"scope": sc}})
        t1 = time.perf_counter()
        call("ui.inspect")
        t2 = time.perf_counter()
        call("ui.screenshot", {"path": os.path.join(tempfile.gettempdir(), "n2-apps.png")})
        t3 = time.perf_counter()
        row = {"app": app, "size": size, "to": label, "mode": mode, "round": rnd, "ok": r.get("ok"), "written": (r.get("result") or {}).get("cells", (r.get("result") or {}).get("paragraphs")),
               "cmd_ms": (t1 - t0) * 1e3, "inspect_ms": (t2 - t0) * 1e3, "shot_ms": (t3 - t0) * 1e3}
        print(json.dumps(row), flush=True)
        assert r.get("ok"), r
        return row
    finally:
        proc.kill()
        time.sleep(2)


results = []
try:
    for label, builtin, plugin in cases:
        rows = {"builtin": [], "plugin": []}
        for rnd in range(rounds):
            rows["builtin"].append(one(label, "builtin", builtin, env_builtin, rnd))
            rows["plugin"].append(one(label, "plugin", plugin, env_plugin, rnd))
        med = lambda m, k: statistics.median(r[k] for r in rows[m][1:])
        summary = {"app": app, "size": size, "to": label, "rounds_timed": rounds - 1}
        for m in rows:
            for k in ("cmd_ms", "inspect_ms", "shot_ms"):
                summary[f"{m}_{k}"] = med(m, k)
        for k in ("cmd_ms", "inspect_ms", "shot_ms"):
            summary[f"plugin_minus_builtin_{k}"] = summary[f"plugin_{k}"] - summary[f"builtin_{k}"]
        for k in ("inspect_ms", "shot_ms"):
            summary[f"plugin_over_builtin_{k}_pct"] = (summary[f"plugin_{k}"] / summary[f"builtin_{k}"] - 1) * 100
        print(json.dumps(summary), flush=True)
        results.append({"summary": summary, "rows": rows})
finally:
    shutil.rmtree(appdata, ignore_errors=True)
    shutil.rmtree(empty, ignore_errors=True)
if out:
    json.dump(results, open(out, "w"), indent=1)
