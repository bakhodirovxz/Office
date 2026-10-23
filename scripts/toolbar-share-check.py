"""Shared toolbar setting across two apps (Phase 4 integration): launch an editor exe with its control port, screenshot it, optionally click
points (x y pairs), screenshot again, quit. Used twice in a row on two apps to show that the toolbar mode written by one is read by the other.

    python scripts/toolbar-share-check.py <exe> <port> <out.png> [x y]...
It uses the real %APPDATA% settings.toml of the product, so back that file up first and delete it after.
"""
import json, os, socket, subprocess, sys, time

exe, port, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
clicks = [(float(sys.argv[i]), float(sys.argv[i + 1])) for i in range(4, len(sys.argv) - 1, 2)]
env = dict(os.environ, GRIDCRAFT_NO_PREFS="1", DECKCRAFT_NO_PREFS="1")
p = subprocess.Popen([exe, "--control", str(port)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    f = None
    for _ in range(120):
        try:
            f = socket.create_connection(("127.0.0.1", port), timeout=30).makefile("rw", encoding="utf-8", newline="\n")
            break
        except OSError:
            time.sleep(1)
    n = 0

    def call(m, params=None):
        global n
        n += 1
        f.write(json.dumps({"id": n, "method": m, "params": params or {}}) + "\n")
        f.flush()
        return json.loads(f.readline())

    time.sleep(5)
    print(call("ui.screenshot", {"path": out})["ok"])
    for x, y in clicks:
        call("ui.click", {"x": x, "y": y})
        time.sleep(2)
    if clicks:
        print(call("ui.screenshot", {"path": out.replace(".png", "-after.png")})["ok"])
    try:
        call("app.quit")
    except Exception:
        pass
finally:
    time.sleep(1)
    p.kill()
