#!/usr/bin/env python
"""WP10 record: the 52 `convert formats --json` rows that fid/directions.py did not list were run with this wrapper.

They are now part of the table (scripts/fidelity/fid/directions.py, WP10 follow-up), the per-file Office hang isolation that this
wrapper emulated lives in fid/refs.py, and csv_recheck.py is folded into run.py's csv_compare. Run everything with

  python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity

This file only forwards to it (so the old command line still works).
"""
import runpy, sys
from pathlib import Path

H = Path(__file__).resolve().parents[3] / "scripts" / "fidelity"
sys.argv = [str(H / "run.py")] + [a for a in sys.argv[1:]]
runpy.run_path(str(H / "run.py"), run_name="__main__")
