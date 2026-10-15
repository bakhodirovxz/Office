# Phase 0: gridcraft v0.7.0 on the owner's Excel workload (xlsx-bench)

All numbers below come from real runs on this machine (2026-10-15). Nothing is extrapolated unless marked so.

## Environment
- Windows 10 Pro, 11.8 GB RAM. The other agent's cargo builds were paused for the final runs (4.6-5.0 GB available at the start of each run).
- Binary: official `gridcraft-0.7.0-windows-x64-portable` (`gridcraft-cli.exe`, `gridcraft.exe`), SHA256-verified by the caller. No upstream code modified.
- Measuring: `spikes/xlsx-bench/measure.py` (psutil, polls peak working set every 50 ms, kills the run if system available RAM drops under 1 GB). Raw results: `E:/office-target/xlsx-bench/results.jsonl`, logs in `.../logs/`.
- Test data generator: Python 3.10, openpyxl 3.1.5 (write_only) for uncached files, xlsxwriter 3.2.0 for the cached file. Venv `E:/office-tools/venv-bench`.
- Other scripts in `spikes/xlsx-bench/`: `gen_all.sh`, `run_sizes.sh`, `mkappend.py`, `gui.ps1`, `ext.py`.

## Generated data (`spikes/xlsx-bench/gen.py`, seed 42)
30 sheets `S01..S30` plus `Lookup` (20 rows) and `Summary`. Each S sheet has ID, date, amount, qty, Cyrillic+Latin text, category, and 3 formula columns per row: `IF`, `VLOOKUP` into Lookup, and a sheet-qualified add. Summary has per sheet `SUM`, `SUMIFS`, `XLOOKUP`, `COUNTIF` over that sheet's full range.

| File | Rows/sheet | Total rows | Size on disk | Cached values |
|---|---|---|---|---|
| w10.xlsx | 4,800 | 144k | 9.9 MB | no |
| c10.xlsx | 3,000 | 90k | 7.2 MB | yes (270,120 formulas) |
| w40.xlsx | 19,200 | 576k | 39.6 MB | no |
| w80.xlsx | 38,400 | 1.15M | 79.2 MB | no |
| w120.xlsx | 57,600 | 1.73M | 118.8 MB | no |

Known generator flaw: the "Link" formula column in sheets after S01 refers to its own sheet (`=S02!C2+D2` on S02), not the previous sheet. The Python expected values for that column are therefore wrong and were not used. All checks use the Summary sheet. No file with real cross-sheet row links was tested.

## Results (gridcraft-cli)

| Size | open (`info`) s | save (`convert` xlsx to xlsx) s | peak RAM MB (open / convert) | output size | reopen ok | formula check ok |
|---|---|---|---|---|---|---|
| 10 MB | 11.0 | 16.5 | 635 / 676 | 11.36 MB | yes (10.0 s, 632 MB) | yes (90/90 Summary values match) |
| 40 MB | 44.9 | 57.3 | 2525 / 2682 | 45.76 MB | yes (40.1 s, 2524 MB) | not run (no expected file) |
| 80 MB | ABORTED at 3.99 GB peak after 62 s | ABORTED at 3.95 GB peak after 60 s | n/a | none | n/a | n/a |
| 120 MB | ABORTED at 4.01 GB peak after 69 s | ABORTED at 3.92 GB peak after 65 s | n/a | none | n/a | n/a |

Notes:
- 80/120 MB aborts: my harness killed the process when system available RAM fell below 1 GB (it started with about 5 GB available, so the process had used about 4 GB). It was still in the read phase, since `convert` never wrote an output. Gridcraft's own result at these sizes is unknown. Peak RAM is about 64 MB per MB of file on disk at both 10 MB and 40 MB (linear), which projects roughly 5 GB for 80 MB and 7.6 GB for 120 MB (projection, not measured). On an 11.8 GB machine with other software open, 80 MB is borderline and 120 MB very likely fails. This matches upstream's "100 MB files fail (#175)".
- `info` alone costs nearly the full load: the file is parsed completely into memory (docs/architecture.md:64: no streaming).
- Output files are 15-16% larger than the input (the input has no cached values, the output does).
- Formula check: `gridcraft-cli check` reported all 270,120 formulas of c10 as "no oracle" because xlsxwriter sets fullCalcOnLoad, so gridcraft ignores the cached values. Instead I compared gridcraft's computed Summary values (SUM, SUMIFS, COUNTIF) for all 30 sheets against Python expectations: w10, c10 and the w10 convert output all match (90/90 each, tolerance 0.01). So gridcraft computes the formulas on load without cached values. `check` also reports that the round trip loses the `core-properties` and `sheetMetadata` relationships.

## Append test (40 MB file, the largest that opened)
- `range.setValues` with `range: "S01!A19202"` and 10,000 rows x 9 columns (3 formulas per row) in one command, plus three `cell.set` formulas on Summary (`=SUM(S01!G2:G29201)`, `=COUNT(S01!A2:A29201)`, `=S01!G29201`). Run: `run --in w40.xlsx --script append.jsonl --out w40_app.xlsx`.
- Wall 70.2 s total (includes the roughly 45 s load and the save), peak 2716 MB, output 46.5 MB.
- Reopen: 40.0 s, 2549 MB. Verified: S01 used range A1:I29201; row 29201 holds the appended text and its formulas; Summary H1 = 280,609,624.7 vs Python expected 183,676,608.70 + 96,933,015.93 = 280,609,624.63 (OK); H2 = 29,200 (OK); H3 = 0 (row 29201 amount 396.2 is under the 500 threshold, OK).
- My first attempt omitted the sheet prefix and wrote the rows to the active sheet (Summary); that attempt is discarded. Without a sheet prefix `range.setValues` targets the active sheet; sheet-qualified ranges work.
- The 80 MB file could not be loaded here, so the append test on it was not possible.

## GUI check (`gridcraft.exe`, 40 MB file)
- The window titled `w40.xlsx` appeared after 43.6 s with `Responding = True`; peak working set 2795 MB at that point. This is about the same as the CLI load time. The 80 MB file was not tried in the GUI because the CLI could not load it within the RAM guard.
- Screenshot: not possible. The session has no interactive desktop; `CopyFromScreen` failed with a Win32 error and no image was saved. The process was closed afterwards.

## External links (verbatim observations)
Files: `E:/office-target/xlsx-bench/ext/` (built by `ext.py`). `linked.xlsx` was assembled by hand with `zipfile` following ECMA-376 Part 1 18.14: `xl/externalLinks/externalLink1.xml` (same-folder target `source.xlsx`) and `externalLink2.xml` (absolute `file:///E:\...\source.xlsx`), each with `sheetNames` and a cached `sheetDataSet`, rels with `TargetMode="External"`, and `<externalReferences>` in workbook.xml. Cells use Excel's stored formulas (`[1]Sheet1!A1`, `[2]Data!B2*2`, `SUM([1]Sheet1!A1:A5)`).
- Open: `Warning: links to external workbooks were kept; they are not followed or refreshed`. Formulas and cached values display (10, 21, 150, 7 ...). `check`: 7/7 formulas matched, round trip equal.
- Source changed (A1 10 to 100, A2..A5 x10, Data!B2 7 to 70), linked.xlsx reopened: values unchanged (10, 21, 150, 7). New values are not picked up.
- Commands: searching `commands` for refresh/external/link finds no update-links command. `data.refreshAll` returned `{"refreshed":0,"errors":[]}` (it only refreshes PivotTables). `formulas.calculateNow` returned `{"cells":1}` and external values stayed cached. Overwriting B1 with 5 and recalculating changed dependent B3 (20 to 15), but the external-reference cells kept their cached values.
- Typing new external references with `cell.set` (`='[source.xlsx]Data'!B2` and `='E:\...\[source.xlsx]Sheet1'!A1`) gives `#REF!` (the second is also stored as an array formula). Links can only exist if they were already in the file.
- Save (`run --out`): `externalLink1.xml` and `externalLink2.xml` are byte-identical, rels are intact (relative and absolute `file:///` targets, TargetMode External), `<externalReferences>` entries and the original cell formulas with cached values survive. Gridcraft rewrites workbook.xml and the sheet XML and adds styles, theme, sharedStrings, metadata and docProps parts. Not checked in real Excel (repair prompt unknown).

## Verdict
gridcraft v0.7.0 does NOT handle this workload:
1. Size: it loads 10 MB (11 s) and 40 MB (45 s, 2.5 GB RAM). The 80 MB and 120 MB files were aborted at about 4 GB peak by the RAM guard, in the read phase. Memory scales at about 64 MB RAM per MB of xlsx, so 80+ MB files that keep growing are at or beyond what an 11.8 GB machine supports. Every operation, even `info`, pays the full load.
2. External links: existing links are preserved on save but never refreshed or created. The owner's formulas that pull from other workbooks would stay frozen at old cached values, and typing a new link gives `#REF!`.
3. At the sizes that loaded, correctness and editing worked: formulas compute correctly without cached values, and a 10,000-row append with dependent formulas was correct after save and reopen. Speed (about 45 s per open of 40 MB) is already poor.

Upstream changes needed: a streaming or lazy xlsx reader and a compact in-memory cell store (#175); resolution and refresh of external workbook references (map `[1]Sheet!A1` to the externalLink part, read the target file, update cached values, add an "update links" command); and parsing/creation of `'[file.xlsx]Sheet'!A1` references typed into cells.

## Reference: Microsoft Excel 16.0 (32-bit) on the same PC and files

Measured by the orchestrator with `spikes/xlsx-bench/excel_ref.ps1`. Excel was driven over COM,
invisible, and opened each file read-only. The script ran `Workbooks.Open` and `CalculateFull`, then
`SaveAs` to xlsx. Peak is the EXCEL process PeakWorkingSet. Raw data is in
`E:/office-target/xlsx-bench/excel_results.jsonl`.

| File | Excel open + full calc | Excel save | Excel peak RAM | gridcraft 0.7.0 open | gridcraft peak RAM |
|---|---|---|---|---|---|
| w10 (9.9 MB) | 7.7 s | 3.4 s | 220 MB | 11.0 s | 635 MB |
| w40 (39.6 MB) | 28.2 s | 9.9 s | 515 MB | 44.9 s | 2525 MB |
| w80 (79.2 MB) | 57.8 s | 16.5 s | 916 MB | aborted (>4 GB) | — |
| w120 (118.8 MB) | 86.5 s | 25.6 s | 1298 MB | aborted (>4 GB) | — |

- **Memory:** gridcraft needs about 5× Excel's peak RAM (2525 vs 515 MB on w40).
- **Speed:** gridcraft is about 1.4–1.6× slower to open.
- **Large files:** 32-bit Excel handles the 80 and 120 MB files in about 1.3 GB or less.
