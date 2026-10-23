# Excel workload guard (WP12)

PLAN section 6: no regression in the Sheets GUI's memory, and the wall and external links never silent.

## 1. Memory and open time (round-2 protocol, the Phase 4 gridcraft)

Binary: `scripts/build.sh sheets desktop --release` at the Phase 4 queue (0140 toolbar, 0141 hook, overlay on), 2026-10-23.
Protocol: `spikes/xlsx-bench/gui_mem.py` unchanged apart from the output file (`sheets.exe --control PORT FILE`, `GRIDCRAFT_NO_PREFS=1`, open,
sample, 10 s, `formulas.calculateNow`, sample, 10 s), the same files `E:/office-target/xlsx-bench/w40|w80|w120.xlsx` (never committed),
3 runs each, medians. Private working set ("steady") and peak working set in MB (MiB, as psutil reports it). Raw data:
`E:/office-target/p4/guard/mem_p4.jsonl`.

| file | steady (round 2) | steady (P4) | peak (round 2) | peak (P4) | open s (round 2) | open s (P4, best / median / worst) |
|---|---|---|---|---|---|---|
| w40 | 566 | **573.3** (+1.3 %) | 646 | 654.4 (+1.3 %) | 31.7-35.4 | 31.9 / 43.9 / 61.8 |
| w80 | 903 | **904.5** (+0.2 %) | 984 | 984.6 (+0.1 %) | 39.8-40.4 and 62.8-63.4 (two clusters) | 74.1 / 81.3 / 159.9; quiet re-run 64.0 / 64.6 / 65.9 |
| w120 | 1,240 | **1,241.4** (+0.1 %) | 1,321 | 1,321.9 (+0.1 %) | 95.7-96.2 | 93.5 / 155.6 / 158.3 |

- **Gate (within +5 % of 566 / 903 / 1,240): passed**, by a wide margin (largest +1.3 %, w40).
- **Open time is not a clean pass and is not proven worse either.** Other agents were building the whole time (`others` in the raw data:
  `rustc.exe`, `cargo.exe`, and a running WINWORD), which Phase 0 forbade during a measured run and which this machine cannot avoid today. The best run
  of w40 and w120 equals round 2 (31.9 vs 31.7 s, 93.5 vs 95.7 s); w80 sits on the slower of round 2's two clusters (64-66 vs 63 s) and not on the
  faster one (40 s). A re-run on an idle machine is needed before the gate review claims "open time not longer". The toolbar and the hook run after
  the open, and `Session::execute` takes its former path with no hook, so I expect no change, but that is an expectation, not a measurement.
- Excel's own reference (742 MB steady on w80) is unchanged from round 2 and was not re-measured.

## 2. The wall (F18)

**What the Sheets GUI itself shows** (release build, the open path is unchanged by the patches; `E:/office-target/p4/guard/probe.py`, synthetic workbooks
made by `make_wall.py`, never committed): `sheets.exe FILE` on a workbook over the wall gives an empty "Book1" window **with a message box**, not a blank
window and not silence (`docs/phase4/evidence/guard/gui-wall.png`):

- 3 x 400 MiB parts (1.2 GiB): "We can't open '...wall-3x400MiB.xlsx': the file is too large: package expands to more than 1 GB".
- one 600 MiB part: "... the file is too large: xl/worksheets/sheet1.xml expands to 629145767 bytes".

Both say the file is too large, but they are English only, say nothing about Excel, and the second prints bytes. **No carried message fix is made**:
with the shell guard below the user is told before the editor starts, and a launch that bypasses the host still gets the box above.

**The shell's pre-open guard** (new, in the desktop host):
- `services/convert/src/scan.rs` `directory()` (new, 19 lines) reads a zip's central directory without inflating anything.
- `services/suite-desktop/src/guard.rs`: Sheets `.xlsx` and `.xlsm` only. `Wall` when the parts add up to over 1 GiB (the scanner's `MAX_TOTAL_BYTES`) or one part
  is over 512 MiB (the editor's `MAX_PART`, `gridcraft/crates/xlsx/src/package.rs`). `Links` when `xl/externalLinks/` is present. Anything else (not a zip, damaged,
  bomb-like) is left to the editor's own refusal. One directory read, so even a 150 MB file costs milliseconds. The scan lives in the host, not in
  `crates/suite-core` as the plan's file list said: suite-core must stay free of zip code and `convert` is a services crate.
- `Ctx::open` (every route: the command line, the second launch over the pipe, the native dialog, the page's Recent) applies it before spawning
  the editor. `Wall`: the editor is not started, the file is listed in Recent, the reply is `{ok:false, guard:"wall", excel, path}`.
  `Links`: the editor starts, the reply carries `guard:"links"` and the warning.
- Native dialogs (`host.rs` `open_and_report`): the sentences of the plan ("This workbook is larger than Sheets can open yet. Open it in Excel." and
  "Links to other workbooks are kept but not updated."), and **"Open it in Excel?" Yes/No when Excel is installed** (`App Paths\excel.exe` read with
  `reg.exe`, 64 and 32 bit views; debug builds honour `SUITE_EXCEL`). The page shows the same sentences translated (`guard.*` in `web/shell/i18n.json`, status
  `draft:agent-sonnet:2026-10-23` for ru and uz) with an "Open in Excel" button (IPC `open_in_excel{path}`: only a path already in the host's history, only a Sheets file).
- Tests: `guard.rs` (the wall at 900 MiB, 1.2 GiB and one part over 512 MiB, links in any case, a real zip read from its directory), `lib.rs`
  `the_guard_stops_a_workbook_over_the_wall_and_warns_about_links` (a real zip with a 513 MiB part: the editor is not started and Recent lists it; a linked workbook
  opens with the warning; a plain one has no guard field; `open_in_excel` refuses unknown paths and non-workbooks): `cargo test -p suite-desktop`, 18 passed.
  Playwright `shell.spec.ts` "guard result": both screens in uz-Latn, and the Excel button sends `open_in_excel`.
- Screenshots: `docs/phase4/evidence/guard/shell-guard-wall-uz-latn.png`, `shell-guard-links-uz-latn.png`, `gui-wall.png`.

**Not verified:** the native Yes/No dialog and a real Excel launch (needs a person at the screen; the code path is `Command::new(excel).arg(path)`), and a file
launched through the single-instance pipe (the same `open_and_report`; `tests/host.rs` covers only a plain file there).
