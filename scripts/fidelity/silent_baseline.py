#!/usr/bin/env python
"""Ratchet for the "loss without warning" list of run.py (PLAN phase 3, WP9/WP12).

  python -I scripts/fidelity/silent_baseline.py check <summary.json> [baseline]   exit 1 on any NEW silent loss
  python -I scripts/fidelity/silent_baseline.py seed  <summary.json> [baseline]   rewrite the baseline from the summary

A loss is keyed "direction|fixture|feature". Fixing a loss only removes its key: shrink the baseline with `seed`.
"""
import json, sys
from pathlib import Path

DEFAULT = Path(__file__).resolve().parents[2] / "ci" / "fidelity" / "silent-baseline.json"


def keys(summary):
    return {"%s|%s|%s" % (x["direction"], x["fixture"], x["feature"]) for x in summary["loss_without_warning"]}


def main(argv):
    if len(argv) < 3 or argv[1] not in ("check", "seed"):
        print(__doc__); return 2
    try:
        cur = keys(json.loads(Path(argv[2]).read_text("utf-8")))
    except (OSError, KeyError, ValueError) as e:
        print("cannot read %s: %s" % (argv[2], e)); return 2
    base_p = Path(argv[3]) if len(argv) > 3 else DEFAULT
    if argv[1] == "seed":
        base_p.write_text(json.dumps({"comment": "silent losses accepted for now; see scripts/fidelity/silent_baseline.py", "losses": sorted(cur)}, indent=1) + "\n", "utf-8")
        print("seeded %s: %d losses" % (base_p, len(cur))); return 0
    base = set(json.loads(base_p.read_text("utf-8"))["losses"])
    new, gone = sorted(cur - base), sorted(base - cur)
    for k in new: print("NEW silent loss: " + k)
    print("silent losses: %d now, %d in the baseline, %d new, %d gone%s" % (len(cur), len(base), len(new), len(gone), " (shrink the baseline with `seed`)" if gone and not new else ""))
    return 1 if new else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
