"""tpl_table.py: medians of the formula-template spike's final runs (labels BUILD_fREP_OP_FILE in
E:/office-target/gridcraft-spike/results.jsonl) as a Markdown table: peak working set (MB) and
wall time (s) per build, with MB per MB of file on disk."""
import json
import os
import statistics
from collections import defaultdict

runs = defaultdict(list)
for line in open("E:/office-target/gridcraft-spike/results.jsonl", encoding="utf-8"):
    r = json.loads(line)
    parts = r["label"].split("_")
    if len(parts) < 4 or not parts[1].startswith("f") or r["rc"] != 0 or r["killed_low_ram"]:
        continue
    build, op, file = parts[0], parts[2], "_".join(parts[3:])
    runs[(op, file, build)].append(r)

SIZES = {"w10": "w10.xlsx", "w10s": "var/w10_shared.xlsx", "c10": "c10.xlsx", "w40": "w40.xlsx", "w80": "w80.xlsx", "w120": "w120.xlsx"}
mb = {k: os.path.getsize("E:/office-target/xlsx-bench/" + v) / 1e6 for k, v in SIZES.items()}

print("| Op | File (MB on disk) | Build | n | Peak MB (median) | MB per file-MB | Wall s (median, min) |")
print("|---|---|---|---|---|---|---|")
for op in ["info", "conv"]:
    for file in ["w10", "w10s", "c10", "w40", "w80", "w120"]:
        for build in ["base", "tpl1", "tpl2"]:
            rs = runs.get((op, file, build))
            if not rs:
                continue
            peak = statistics.median(r["peak_mb"] for r in rs)
            wall = statistics.median(r["wall_s"] for r in rs)
            print(f"| {op} | {file} ({mb[file]:.1f}) | {build} | {len(rs)} | {peak:.0f} | {peak / mb[file]:.1f} | {wall:.1f}, {min(r['wall_s'] for r in rs):.1f} |")
