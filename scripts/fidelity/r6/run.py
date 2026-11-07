"""R6 round trips (docs/phase5/r6.md): open -> one edit -> save -> reopen with the engines' own CLIs (the same engine code and
writer the web editors run), then compare.py. Usage:
  python -I run.py <out_dir> <sheets-cli> <slides-cli> <pdf-cli> <file>...   (kind by extension)
Prints one line per file and writes <out_dir>/r6-roundtrip.json."""
import json, os, subprocess, sys

out, sheets, slides, pdfcli, *files = sys.argv[1:]
here = os.path.dirname(os.path.abspath(__file__))
os.makedirs(out, exist_ok=True)
rows = []


def run(cmd):
    p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode:
        raise RuntimeError((p.stderr or p.stdout).strip()[-400:])
    return p.stdout


for f in files:
    stem, ext = os.path.splitext(os.path.basename(f))
    ext = ext[1:].lower()
    saved = os.path.join(out, f"{stem}.rt.{ext}")
    row = {"file": os.path.basename(f), "kind": ext}
    try:
        if ext == "xlsx":
            import openpyxl
            ws = openpyxl.load_workbook(f).worksheets[0]
            cell = f"A{ws.max_row + 2}"
            skip = f"{ws.title}!{cell}"
            row["edit"] = f"cell.set {skip} = 'R6 edit'"
            run([sheets, "run", "--in", f, "--cmd", 'sheet.activate={"sheet":0}',
                 "--cmd", json.dumps({"cell": cell, "input": "R6 edit"}).join(["cell.set=", ""]), "--out", saved, "--quiet"])
        elif ext == "pptx":
            from pptx import Presentation
            n = len(Presentation(f).slides)
            skip = str(n)
            row["edit"] = f"text box added on slide {n}"
            run([slides, "run", "--in", f, "--cmd", f'slide.go={{"index":{n - 1}}}',
                 "--cmd", 'insert.textBox={"rect":[20,20,200,40],"text":"R6 edit"}', "--save", saved])
        elif ext == "pdf":
            skip = ""
            row["edit"] = "sticky note added on page 1"
            script = os.path.join(out, f"{stem}.steps.json")
            json.dump([{"tool": "doc_open", "args": {"path": os.path.abspath(f)}},
                       {"tool": "comment_add", "args": {"doc": 1, "page": 1, "type": "note", "at": [40, 40], "contents": "R6 edit"}},
                       {"tool": "doc_save", "args": {"doc": 1, "path": os.path.abspath(saved), "full": False}}], open(script, "w"))
            run([pdfcli, "run", "--script", script])
        else:
            continue
        r = json.loads(run([sys.executable, "-I", os.path.join(here, "compare.py"), ext, f, saved, skip]))
        row.update(r)
    except Exception as e:  # a failed step is a result too
        row["error"] = str(e)
    rows.append(row)
    print(row["file"], "ERROR " + row["error"] if "error" in row else f"losses {len(row['losses'])}", row.get("losses", [])[:5])
json.dump(rows, open(os.path.join(out, "r6-roundtrip.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
