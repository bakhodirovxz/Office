"""Phase 2 WP10 N2: the transliteration command on the 108-page document in the release desktop app (docs.exe),
driven through the app's control channel (`--control PORT`, JSON lines over loopback; same protocol as desktop-brand-evidence.py).

    python -I scripts/translit/perf-desktop.py <docs.exe> [--rounds 6] [--out result.json]

Every round starts a fresh process on the document and waits until it is laid out and idle, then times (wall clock, client side):
  cmd      the `engine.execute` round trip (the app runs the command inside a frame and answers)
  inspect  `ui.inspect` straight after: answered by the next frame, so it ends after that frame's layout work
  shot     `ui.screenshot` straight after: answered after a frame with the NEW layout was painted and read back (upper bound)
and reads the app's own timers from `ui.inspect` (perf.frameMs, perf.layoutMs, the page count). The first round is a warm-up.
"""
import json, os, socket, statistics, subprocess, sys, tempfile, time

exe = sys.argv[1]
rounds = int(sys.argv[sys.argv.index("--rounds") + 1]) if "--rounds" in sys.argv else 6
out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else None
fx = os.path.abspath("domains/docs/crates/docs-translit/tests/fixtures")
cases = [("uz108-latn.docx", "tools.translitCyrl"), ("uz108-cyrl.docx", "tools.translitLatn")]
port = 7990


def session(path):
    keyfile = os.path.join(os.environ["APPDATA"], "WordCraft", f"control-key.local-{port}")
    if os.path.exists(keyfile):
        os.remove(keyfile)
    proc = subprocess.Popen([exe, path, "--control", str(port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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


results = []
for name, cmd in cases:
    path = os.path.join(fx, name)
    rows = []
    for rnd in range(rounds):
        proc, call = session(path)
        try:
            # wait until the document is shown and idle (pages known, layout time stable)
            last = None
            for _ in range(240):
                st = call("ui.inspect").get("result", {})
                pages = len(st.get("pages", []))
                lay = st.get("perf", {}).get("layoutMs")
                if pages and lay and last == (pages, lay):
                    break
                last = (pages, lay)
                time.sleep(0.5)
            time.sleep(2)
            before = call("ui.inspect")["result"]
            t0 = time.perf_counter()
            r = call("engine.execute", {"command": cmd, "params": {}})
            t1 = time.perf_counter()
            st = call("ui.inspect")
            t2 = time.perf_counter()
            shot = call("ui.screenshot", {"path": os.path.join(tempfile.gettempdir(), "translit-n2.png")})
            t3 = time.perf_counter()
            after = call("ui.inspect")["result"]
            res = r.get("result", r)
            row = {
                "round": rnd, "ok": r.get("ok"), "cmd_ms": (t1 - t0) * 1e3, "inspect_ms": (t2 - t0) * 1e3, "shot_ms": (t3 - t0) * 1e3,
                "app_convert_ms": res.get("ms", {}).get("convert"), "app_apply_ms": res.get("ms", {}).get("apply"),
                "paragraphs": res.get("paragraphs"), "layoutMs_before": before["perf"]["layoutMs"], "layoutMs_after": after["perf"]["layoutMs"],
                "frameMs_after": after["perf"]["frameMs"], "pages_before": len(before["pages"]), "pages_after": len(after["pages"]),
            }
            rows.append(row)
            print(name, json.dumps(row), flush=True)
        finally:
            proc.kill()
            time.sleep(2)
    timed = rows[1:]
    med = lambda k: statistics.median(r[k] for r in timed)
    summary = {"file": name, "command": cmd, "rounds_timed": len(timed), "median_cmd_ms": med("cmd_ms"), "median_inspect_ms": med("inspect_ms"), "median_shot_ms": med("shot_ms"),
               "median_layoutMs_after": med("layoutMs_after"), "max_shot_ms": max(r["shot_ms"] for r in timed)}
    print(json.dumps(summary), flush=True)
    results.append({"summary": summary, "rows": rows})
if out:
    json.dump(results, open(out, "w"), indent=1)
