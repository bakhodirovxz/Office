"""gui_mem.py LABEL EXE FILE [--settle S] [--no-recalc]

Steady-state memory of the gridcraft desktop app (gridcraft.exe), measured the way the Excel
reference is (excel_mem.ps1):
  1. start `EXE --control PORT FILE` (GRIDCRAFT_NO_PREFS=1: no prefs, no log file);
  2. the app opens the file before it creates its window and starts the control server, so the
     first answer to `document.inspect` on the control port means: loaded, recalculated, and the UI
     thread is running frames. `open_s` is the time to that answer;
  3. sample right away (peak of the open), wait S seconds, sample again (steady state);
  4. run `formulas.calculateNow` (F9: graph rebuild + full recalc) through `engine.execute`, wait
     for the UI thread to answer again, sample, wait S seconds, sample (steady state after recalc);
  5. `app.quit`, kill after 30 s.
A RAM guard kills the app when the system has less than 1 GiB available. Appends one JSON line to
E:/office-target/gridcraft-spike/r2/mem_results.jsonl.
"""
import json
import os
import socket
import subprocess
import sys
import time

import psutil

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from memsample import sample  # noqa: E402

OUT = "E:/office-target/gridcraft-spike/r2/mem_results.jsonl"
HEAVY = ("rustc.exe", "cargo.exe", "wasm-opt.exe", "qemu-system-x86_64.exe", "excel.exe", "winword.exe", "powerpnt.exe", "gridcraft.exe", "gridcraft-cli.exe")


class LowRam(Exception):
    pass


def others(skip):
    return sorted({(q.info["name"] or "") for q in psutil.process_iter(["name", "pid"]) if (q.info["name"] or "").lower() in HEAVY and q.info["pid"] != skip})


def free_mb():
    return psutil.virtual_memory().available / 2**20


def main():
    label, exe, path = sys.argv[1:4]
    settle = float(sys.argv[sys.argv.index("--settle") + 1]) if "--settle" in sys.argv else 10.0
    recalc = "--no-recalc" not in sys.argv
    port = 7900 + os.getpid() % 90
    env = dict(os.environ, GRIDCRAFT_NO_PREFS="1")
    rec = dict(label=label, app="gridcraft-gui", exe=exe, file=os.path.basename(path) if path != "-" else "(empty)", free_before_mb=round(free_mb()))
    args = [exe, "--control", str(port)] + ([path] if path != "-" else [])
    t0 = time.time()
    p = subprocess.Popen(args, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rec["others"] = others(p.pid)
    minfree = free_mb()

    def guard():
        nonlocal minfree
        fr = free_mb()
        minfree = min(minfree, fr)
        if fr < 1024:
            raise LowRam()
        if p.poll() is not None:
            raise RuntimeError(f"app exited with {p.returncode}")

    def wait(sec):
        end = time.time() + sec
        while time.time() < end:
            guard()
            time.sleep(0.1)

    sock = None
    buf = b""

    def request(method, params=None, limit=900):
        """Sends one request and waits (guarding RAM) for its reply line; the control server gives
        up after 60 s with an error reply, so a long command is followed by `ui.inspect` until the
        UI thread answers."""
        nonlocal sock, buf
        while sock is None:
            guard()
            try:
                sock = socket.create_connection(("127.0.0.1", port), timeout=0.5)
            except OSError:
                time.sleep(0.1)
        sock.settimeout(0.2)
        sock.sendall((json.dumps({"id": 1, "method": method, "params": params or {}}) + "\n").encode())
        end = time.time() + limit
        while time.time() < end:
            guard()
            if b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                return json.loads(line)
            try:
                chunk = sock.recv(1 << 20)
                if not chunk:
                    raise RuntimeError("control connection closed")
                buf += chunk
            except socket.timeout:
                pass
        raise RuntimeError(f"{method}: no reply in {limit} s")

    def settled(method, params=None):
        r = request(method, params)
        while not r.get("ok") and "timed out" in str(r.get("error", "")):
            r = request("ui.inspect")
        return r

    try:
        r = settled("document.inspect")
        rec["open_s"] = round(time.time() - t0, 2)
        res = r.get("result") or {}
        rec["sheets"] = len(res.get("sheets", [])) if isinstance(res.get("sheets"), list) else res.get("sheets")
        rec["open_peak"] = sample(p.pid, "open")
        wait(settle)
        rec["steady"] = sample(p.pid, "steady")
        if recalc:
            t1 = time.time()
            r = settled("engine.execute", {"command": "formulas.calculateNow", "params": {}})
            if not r.get("ok"):
                rec["recalc_error"] = r.get("error")
            else:
                settled("ui.inspect")
            rec["recalc_s"] = round(time.time() - t1, 2)
            rec["recalc_cells"] = (r.get("result") or {}).get("cells")
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
            if p.poll() is None:
                rec.setdefault("final", sample(p.pid, "final"))
        except Exception:  # noqa: BLE001
            pass
        if p.poll() is None:
            try:
                if sock is not None:
                    sock.settimeout(2)
                    sock.sendall(b'{"id":9,"method":"app.quit","params":{}}\n')
            except OSError:
                pass
            try:
                p.wait(30)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()
    rec["min_free_mb"] = round(minfree)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec))


if __name__ == "__main__":
    main()
