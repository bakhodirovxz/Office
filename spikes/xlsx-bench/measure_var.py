"""measure.py LABEL -- cmd args...   Runs cmd, polls peak working set (psutil), kills if system free RAM < 1 GB.
Appends a JSON line to E:/office-target/xlsx-bench/var/results.jsonl; stdout/stderr to E:/office-target/xlsx-bench/var/logs/LABEL.{out,err}"""
import sys, subprocess, time, json, psutil, os
label = sys.argv[1]; cmd = sys.argv[sys.argv.index("--")+1:]
os.makedirs("E:/office-target/xlsx-bench/var/logs", exist_ok=True)
fo = open(f"E:/office-target/xlsx-bench/var/logs/{label}.out","wb"); fe = open(f"E:/office-target/xlsx-bench/var/logs/{label}.err","wb")
free0 = psutil.virtual_memory().available/2**20
t0 = time.time(); p = subprocess.Popen(cmd, stdout=fo, stderr=fe); pp = psutil.Process(p.pid)
peak = 0; minfree = free0; killed = False
while p.poll() is None:
    try:
        m = pp.memory_info(); peak = max(peak, getattr(m,"peak_wset",m.rss))
    except Exception: pass
    fr = psutil.virtual_memory().available/2**20; minfree = min(minfree, fr)
    if fr < 1024: p.kill(); killed = True; break
    time.sleep(0.05)
p.wait(); dt = time.time()-t0
r = dict(label=label, cmd=" ".join(cmd[1:]), rc=p.returncode, wall_s=round(dt,2), peak_mb=round(peak/2**20), free_before_mb=round(free0), min_free_mb=round(minfree), killed_low_ram=killed)
open("E:/office-target/xlsx-bench/var/results.jsonl","a").write(json.dumps(r)+"\n"); print(json.dumps(r))
