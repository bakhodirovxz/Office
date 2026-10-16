"""cli_mem.py LABEL EXE FILE [--settle S]

Steady-state memory of the engine alone, with no window: `gridcraft-cli mcp --in FILE` opens FILE
(file.open, as the app does) and then serves MCP (JSON-RPC lines on stdin). Measured like
gui_mem.py: the first answer (to `initialize`, then `inspect_workbook`) means loaded; sample; wait S
seconds; sample; `formulas.calculateNow` through `execute_command`; sample; wait; sample. RAM guard
as in gui_mem.py. Appends to E:/office-target/gridcraft-spike/r2/mem_results.jsonl (app "cli-mcp").
"""
import json
import os
import queue
import subprocess
import sys
import threading
import time

import psutil

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gui_mem import OUT, LowRam, free_mb, others  # noqa: E402
from memsample import sample  # noqa: E402


def main():
    label, exe, path = sys.argv[1:4]
    settle = float(sys.argv[sys.argv.index("--settle") + 1]) if "--settle" in sys.argv else 10.0
    rec = dict(label=label, app="cli-mcp", exe=exe, file=os.path.basename(path), free_before_mb=round(free_mb()))
    t0 = time.time()
    p = subprocess.Popen([exe, "mcp", "--in", path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=dict(os.environ, GRIDCRAFT_NO_PREFS="1"))
    rec["others"] = others(p.pid)
    lines = queue.Queue()
    threading.Thread(target=lambda: [lines.put(l) for l in p.stdout], daemon=True).start()
    minfree = free_mb()
    ids = iter(range(1, 1 << 30))

    def guard():
        nonlocal minfree
        fr = free_mb()
        minfree = min(minfree, fr)
        if fr < 1024:
            raise LowRam()
        if p.poll() is not None:
            raise RuntimeError(f"exited with {p.returncode}")

    def call(method, params, limit=1800):
        i = next(ids)
        p.stdin.write((json.dumps({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n").encode())
        p.stdin.flush()
        end = time.time() + limit
        while time.time() < end:
            guard()
            try:
                msg = json.loads(lines.get(timeout=0.1))
            except queue.Empty:
                continue
            if msg.get("id") == i:
                return msg
        raise RuntimeError(f"{method}: no reply")

    def wait(sec):
        end = time.time() + sec
        while time.time() < end:
            guard()
            time.sleep(0.1)

    try:
        call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "cli_mem", "version": "1"}})
        p.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        p.stdin.flush()
        r = call("tools/call", {"name": "inspect_workbook", "arguments": {}})
        rec["open_s"] = round(time.time() - t0, 2)
        rec["inspect_ok"] = "error" not in r
        rec["open_peak"] = sample(p.pid, "open")
        wait(settle)
        rec["steady"] = sample(p.pid, "steady")
        t1 = time.time()
        r = call("tools/call", {"name": "execute_command", "arguments": {"command": "formulas.calculateNow", "params": {}}})
        rec["recalc_s"] = round(time.time() - t1, 2)
        rec["recalc_peak"] = sample(p.pid, "recalc")
        wait(settle)
        rec["steady_recalc"] = sample(p.pid, "steady_recalc")
        rec["ok"] = True
    except LowRam:
        rec["ok"] = False
        rec["killed_low_ram"] = True
    except Exception as e:  # noqa: BLE001
        rec["ok"] = False
        rec["error"] = str(e)
    finally:
        try:
            p.stdin.close()
            p.wait(30)
        except Exception:  # noqa: BLE001
            p.kill()
            p.wait()
    rec["min_free_mb"] = round(minfree)
    with open(OUT, "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec))


if __name__ == "__main__":
    main()
