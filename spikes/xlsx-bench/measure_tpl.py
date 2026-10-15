"""measure_tpl.py LABEL -- cmd args...   Same as measure.py (psutil peak working set polled every 50 ms,
kills the run if system available RAM drops under 1 GB), for the formula-template spike: appends a JSON
line to E:/office-target/gridcraft-spike/results.jsonl; stdout/stderr go to
E:/office-target/gridcraft-spike/runlogs/LABEL.{out,err}."""
import json
import os
import subprocess
import sys
import time

import psutil

ROOT = "E:/office-target/gridcraft-spike"
label = sys.argv[1]
cmd = sys.argv[sys.argv.index("--") + 1:]
os.makedirs(f"{ROOT}/runlogs", exist_ok=True)
fo = open(f"{ROOT}/runlogs/{label}.out", "wb")
fe = open(f"{ROOT}/runlogs/{label}.err", "wb")
free0 = psutil.virtual_memory().available / 2**20
# Other heavy processes running when the measurement starts (they share the CPU).
others = sorted({q.info["name"] for q in psutil.process_iter(["name"]) if (q.info["name"] or "").lower() in ("rustc.exe", "wasm-opt.exe", "qemu-system-x86_64.exe", "cargo.exe")})
t0 = time.time()
p = subprocess.Popen(cmd, stdout=fo, stderr=fe)
pp = psutil.Process(p.pid)
peak = 0
minfree = free0
killed = False
while p.poll() is None:
    try:
        m = pp.memory_info()
        peak = max(peak, getattr(m, "peak_wset", m.rss))
    except Exception:
        pass
    fr = psutil.virtual_memory().available / 2**20
    minfree = min(minfree, fr)
    if fr < 1024:
        p.kill()
        killed = True
        break
    time.sleep(0.05)
p.wait()
dt = time.time() - t0
r = dict(label=label, cmd=" ".join(cmd), rc=p.returncode, wall_s=round(dt, 2), peak_mb=round(peak / 2**20),
         free_before_mb=round(free0), min_free_mb=round(minfree), killed_low_ram=killed, others=others)
open(f"{ROOT}/results.jsonl", "a").write(json.dumps(r) + "\n")
print(json.dumps(r))
