"""memsample.py PID [TAG] -> one JSON line with the process's memory counters, in MiB.

Shared by the gridcraft and Excel drivers (gui_mem.py, excel_mem.ps1) so both apps are read with
the same counters:
  ws        working set (WorkingSetSize)
  peak_ws   peak working set over the process lifetime (PeakWorkingSetSize)
  priv_ws   private working set (what Task Manager's "Memory" column shows; psutil `uss`,
            QueryWorkingSet pages not shared)
  commit    private bytes / commit charge (PagefileUsage)
  peak_commit
"""
import json
import sys

import psutil


def sample(pid, tag=""):
    p = psutil.Process(pid)
    m = p.memory_full_info()
    mb = lambda b: round(b / 2**20, 1)
    return dict(tag=tag, ws=mb(m.wset), peak_ws=mb(m.peak_wset), priv_ws=mb(m.uss), commit=mb(m.private), peak_commit=mb(m.peak_pagefile))


if __name__ == "__main__":
    print(json.dumps(sample(int(sys.argv[1]), sys.argv[2] if len(sys.argv) > 2 else "")))
