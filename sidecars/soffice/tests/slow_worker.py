"""Test double for worker.py: starts a grandchild and then hangs, to prove a timeout kills the whole group."""
import os, subprocess, time

p = subprocess.Popen(["sleep", "120"])
open(os.environ["SLOW_PID_FILE"], "w").write(str(p.pid))
time.sleep(120)
