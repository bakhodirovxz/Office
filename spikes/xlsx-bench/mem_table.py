"""mem_table.py: medians of the round-2 memory runs.

Reads E:/office-target/gridcraft-spike/r2/mem_results.jsonl (excel_mem.ps1, gui_mem.py) and
E:/office-target/gridcraft-spike/results.jsonl (CLI runs, labels r2m_* / r2a_* / r2s_*), and prints
markdown tables: per app/build and file, the median over repetitions of
  peak WS (lifetime peak at the end of the open, and after the recalc),
  private WS and private bytes 10 s after open, and 10 s after the full recalc,
  open and recalc seconds.
"""
import json
import re
import statistics
from collections import defaultdict

R2 = "E:/office-target/gridcraft-spike/r2/mem_results.jsonl"
CLI = "E:/office-target/gridcraft-spike/results.jsonl"


def load(path):
    out = []
    for line in open(path, encoding="utf-8-sig"):
        line = line.strip().lstrip("\ufeff")
        if line:
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return out


def med(xs):
    xs = [x for x in xs if x is not None]
    return round(statistics.median(xs)) if xs else None


def fkey(f):
    m = re.search(r"w(\d+)(_excel)?\.xlsx", f)
    return ("x" if m.group(2) else "w") + m.group(1) if m else f


groups = defaultdict(list)
for r in load(R2):
    if not r.get("ok") or "label" not in r:
        continue
    lab = r["label"]
    if lab.startswith(("smoke", "seg_", "base_", "excelvis", "mcp_")):
        app = lab.rsplit("_r", 1)[0] if re.search(r"_r\d$", lab) else lab
        groups[(app, "-")].append(r)
        continue
    m = re.match(r"(excel|r1gui|r2gui|r3gui|r4gui)_(\w+?)_r\d$", lab)
    if m:
        groups[(m.group(1), m.group(2))].append(r)

g = lambda r, k, f: (r.get(k) or {}).get(f)
cols = ["runs", "open s", "peak WS open", "priv WS steady", "priv bytes steady", "WS steady", "recalc s", "peak WS after recalc", "priv WS after recalc", "priv bytes after recalc"]
print("| app | file | " + " | ".join(cols) + " |")
print("|" + "---|" * (len(cols) + 2))
order = {"excel": 0, "r1gui": 1, "r2gui": 2, "r3gui": 3, "r4gui": 4}
for (app, f), rs in sorted(groups.items(), key=lambda kv: (order.get(kv[0][0], 9), kv[0][0], kv[0][1])):
    row = [len(rs), med([r.get("open_s") for r in rs]), med([g(r, "open_peak", "peak_ws") for r in rs]),
           med([g(r, "steady", "priv_ws") for r in rs]), med([g(r, "steady", "commit") for r in rs]), med([g(r, "steady", "ws") for r in rs]),
           med([r.get("recalc_s") for r in rs]), med([g(r, "steady_recalc", "peak_ws") for r in rs]),
           med([g(r, "steady_recalc", "priv_ws") for r in rs]), med([g(r, "steady_recalc", "commit") for r in rs])]
    print(f"| {app} | {f} | " + " | ".join("" if v is None else str(v) for v in row) + " |")

print()
cli = defaultdict(list)
for r in load(CLI):
    lab = r.get("label", "")
    m = re.match(r"(r2[mas])_(\w+?)_(info|f9)_(\w+?)(?:_r\d)?$", lab)
    if m and r.get("rc") == 0 and not r.get("killed_low_ram"):
        cli[(m.group(1), m.group(2), m.group(3), m.group(4))].append(r)
print("| set | build | cmd | file | runs | peak WS MB | wall s (min) |")
print("|---|---|---|---|---|---|---|")
for k, rs in sorted(cli.items()):
    print(f"| {k[0]} | {k[1]} | {k[2]} | {k[3]} | {len(rs)} | {med([r['peak_mb'] for r in rs])} | {min(r['wall_s'] for r in rs)} |")
