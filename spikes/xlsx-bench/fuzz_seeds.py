"""fuzz_seeds.py BUILD FIRST LAST [STEPS] [CAP_MB]

Runs gridcraft's command-sequence fuzzer (release test binary
E:/office-target/gridcraft-spike/bin/BUILD/fuzz.exe) one seed at a time (GRIDCRAFT_FUZZ_SEED), so
each seed's peak working set and time are measured and a runaway seed is stopped (process over
CAP_MB, default 2500, or system available RAM under 1 GB) instead of paging the machine. Appends one
JSON line per seed to E:/office-target/gridcraft-spike/fuzz_seeds.jsonl.
"""
import json
import os
import subprocess
import sys
import time

import psutil

build, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
steps = sys.argv[4] if len(sys.argv) > 4 else "150"
cap = int(sys.argv[5]) if len(sys.argv) > 5 else 2500
exe = f"E:/office-target/gridcraft-spike/bin/{build}/fuzz.exe"
out = open("E:/office-target/gridcraft-spike/fuzz_seeds.jsonl", "a")
for seed in range(first, last + 1):
    env = dict(os.environ, GRIDCRAFT_FUZZ_SEED=str(seed), GRIDCRAFT_FUZZ_STEPS=steps)
    t0 = time.time()
    p = subprocess.Popen([exe, "command_sequences_keep_the_workbook_consistent", "--exact", "--nocapture"],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env)
    pp = psutil.Process(p.pid)
    peak = 0
    killed = None
    while p.poll() is None:
        try:
            m = pp.memory_info()
            peak = max(peak, getattr(m, "peak_wset", m.rss))
        except Exception:
            pass
        if peak > cap * 2**20:
            killed = "cap"
        elif psutil.virtual_memory().available < 2**30:
            killed = "low_ram"
        if killed:
            p.kill()
            break
        time.sleep(0.05)
    text = p.communicate()[0].decode("utf-8", "replace")
    fail = [l for l in text.splitlines() if "fuzz failure" in l or "panicked" in l or "invariant" in l][:3]
    r = dict(build=build, seed=seed, steps=int(steps), rc=p.returncode, wall_s=round(time.time() - t0, 2),
             peak_mb=round(peak / 2**20), killed=killed, fail=fail)
    out.write(json.dumps(r, ensure_ascii=False) + "\n")
    out.flush()
    print(json.dumps(r, ensure_ascii=False), flush=True)
