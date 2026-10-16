# gridcraft: memory round 2 (Phase 0) — measured against Excel

Date: 2026-10-16. Base: round 1 (`spike/formula-templates`, `ac87f55`, patches 0001–0008, see
`gridcraft-templates-spike.md`). Round 2: branch `spike/memory-r2` in `E:/office-work/gridcraft-spike`,
11 commits, patches `spikes/gridcraft-patches/0009…0019`. Source paths are relative to the gridcraft repo
unless they start with `spikes/`.

**Evidence types:** MEASURED (a run on this PC), CODE (read in the source), UNVERIFIED (estimate).
"MB" is MiB (2^20 bytes), as psutil reports it.

## 0. Answer in short

The owner's complaint was "1.5 GB for an 80 MB file; Office uses 300 MB". Measured properly, the same
80 MB workbook takes **742 MB in Excel 16** (what Task Manager shows), not 300 MB. "300 MB" must be a
different (smaller or simpler) file.

**Main table** (MEASURED, medians of 3; the desktop app `gridcraft.exe` vs Excel 16 32-bit via COM).
"Steady" is the private working set (Task Manager's "Memory" column) 10 s after the open finished.
"Peak" is the peak working set. "After F9" is the private working set 10 s after a full recalculation.

| File | | Excel 16 | gridcraft round 1 | **gridcraft round 2** | round 2 / Excel |
|---|---|---|---|---|---|
| w40 (40 MB) | steady | 389 | 904 | **566** | 1.46 |
| | peak | 479 | 985 | **646** | 1.35 |
| | after F9 | 389 | 1,218 | **566** | 1.46 |
| w80 (79 MB) | steady | 742 | 1,401 | **903** | 1.22 |
| | peak | 851 | 1,499 (2,409 with F9) | **984** (1,071 with F9) | 1.16 |
| | after F9 | 742 | 2,026 | **905** | 1.22 |
| w120 (119 MB) | steady | 1,090 | 1,897 | **1,240** | 1.14 |
| | peak | 1,222 | 2,425 (3,540 with F9) | **1,321** (1,476 with F9) | 1.08 |
| | after F9 | 1,090 | 2,836 | **1,241** | 1.14 |

The w-files are the openpyxl bench files (no cached values, `fullCalcOnLoad`). The same workbooks saved by Excel
(x-files, §2) give the same picture: x80 steady 724 (Excel) / 1,389 (round 1) / **928** (round 2).

**What closes and what doesn't:**
- **The document itself now costs what it costs in Excel.** Above each app's empty-window baseline
  (gridcraft 226 MB, Excel 33 MB) w80 takes 677 MB in gridcraft and 709 MB in Excel (0.95). w120 takes
  1,014 vs 1,057 (0.96), x80 702 vs 691 (1.02).
- **The engine alone beats Excel's peak.** `gridcraft-cli info`, without a window, peaks at 817 MB on
  w80 against Excel's 851, and at 731 against 797 on x80.
- **The remaining gap is the desktop app's fixed cost:** 226 MB with an empty workbook, against
  Excel's 33. Of that:
  - 57 MB is font files read whole at start-up, 46 MB of them CJK fallback faces;
  - about 90 MB is the DirectX 12 renderer and driver (the OpenGL backend: 134 MB in all);
  - the rest is egui, winit and the program itself.
  Round 2 already took this base down from 406 MB (§4, commit 8).
- **Round 2 versus round 1, w80:** steady −36% (1,401 → 903), peak −34%, after F9 −55%
  (2,026 → 905). F9 no longer duplicates the workbook.

**Correctness (MEASURED):** every value is bit-identical to the unpatched build on w10, c10 and w40.
- Every row value of w80 and w120 matches the Python expectations.
- The 35-step edit script gives the same result.
- Excel opens the saved files and finds identical formulas and recalculated values.
- The fuzz outcomes are the same as round 1's (2 pre-existing upstream failures). Two seeds that
  exceeded the 2.5 GB cap in both earlier builds now finish.
- Tests:
  - formula / model / calc / xlsx / engine all pass (+12 new tests);
  - ui-egui and the app: 698 tests in 49 suites pass.

**Verdict.** Excel's level is reached for the workbook data and for the engine's peak. The desktop app
is still +14% (w120) to +46% (w40) above Excel in Task Manager, because of its fixed 226 MB.

Closing that needs:
- UI work: CJK fonts loaded on demand, and the renderer's footprint;
- optionally, the allocator and two more cell-layout changes.

Together these are estimated (UNVERIFIED) at 8–15 engineer-days (§6–7). Remaining production work for
round 2 itself: 5–8 days.

## 1. How it was measured (Step 0)

**Counters.** All three apps are read with one sampler (`spikes/xlsx-bench/memsample.py`, psutil 7.2.2,
`memory_full_info`), so Excel and gridcraft are compared on the same counters:

| Counter | Windows name | psutil | Notes |
|---|---|---|---|
| peak WS | PeakWorkingSetSize | `peak_wset` | lifetime peak kept by the OS, so no polling gaps |
| WS | WorkingSetSize | `wset` | includes shared pages (DLLs). Excel's is about 75 MB above its private WS |
| **private WS** | Working Set − Private | `uss` | **what Task Manager's "Memory" column shows** |
| private bytes | PagefileUsage (commit) | `private` | committed private memory, resident or not |

**Procedure, the same for both apps:**
1. Open the file and detect the end of loading.
2. Sample (the peak of the open).
3. Wait 10 s and sample (steady).
4. Full recalculation, then sample.
5. Wait 10 s and sample (steady after recalc).
6. Close.

| | Excel (`spikes/xlsx-bench/excel_mem.ps1`) | gridcraft GUI (`gui_mem.py`) | gridcraft engine only (`cli_mem.py`) | CLI peak (`measure_tpl.py`) |
|---|---|---|---|---|
| Start | COM `Excel.Application`, `Visible`/`DisplayAlerts`/`AskToUpdateLinks`/`ScreenUpdating` off (as `excel_ref.ps1`), read-only open | `gridcraft.exe --control PORT FILE`, `GRIDCRAFT_NO_PREFS=1` | `gridcraft-cli mcp --in FILE` (the same `file.open`) | `gridcraft-cli info FILE` |
| Loaded when | `Workbooks.Open` returns | first answer on the control port (the app opens the file before it creates its window and control server) | first MCP answer | process exit |
| Full recalc | `Application.CalculateFull` | `formulas.calculateNow` (F9: graph rebuild + full calculation) via `engine.execute` | the same via `execute_command` | — |
| Close | `Close`, `Quit`; only the EXCEL PID this script started (found by diffing the process list) is ever stopped | `app.quit` | stdin closed | — |

**Machine discipline:**
- Every run and every build waited for no `rustc.exe` and enough available RAM (`quiet.sh`, ≥ 3–3.5 GB).
- Excel runs also waited until no WINWORD/EXCEL/POWERPNT process existed.
- A RAM guard stops a gridcraft run when the system has under 1 GB available.
- Runs were interleaved (Excel, GUI, CLI per file, then the next file), 3 repetitions, medians.
- `others` in the raw data records heavy processes present: none were recorded during the measured
  runs.
- Raw data:
  - `E:/office-target/gridcraft-spike/r2/mem_results.jsonl` (Excel, GUI, engine);
  - `E:/office-target/gridcraft-spike/results.jsonl` (CLI, labels `r2m_*`, `r2a_*`, `r2s_*`).
- Tables: `spikes/xlsx-bench/mem_table.py`.

**Files.**
- `w40/w80/w120`: the openpyxl bench files (`xlsx-bench.md`). Inline strings, no cached values,
  `fullCalcOnLoad`, so both apps calculate everything on open.
- `x40/x80/x120`: the same workbooks saved by Excel (`excel/wNN_excel.xlsx`, from `excel_ref.ps1`).
  They have shared strings, Excel's shared formulas and cached values, and no `fullCalcOnLoad`, which
  is what the owner's real files look like.

**Builds** (`E:/office-target/gridcraft-spike/bin/`):
- `r1`: round 1's tip, rebuilt with the GUI;
- `c2…c7`: CLI per round-2 commit;
- `c8`: commit 8 (GUI + CLI);
- `c11`: the round-2 tip (GUI, CLI, fuzz);
- `c8seg`: c8 with the segment heap (§4.3);
- `probe_r1`/`probe_r4`: the counting-allocator probe at round 1 / round 2
  (`spikes/xlsx-bench/mem-probe-r2`).

**An incident, and how it was handled.** At 15:47 the round-1 GUI, measuring w80, **saved w80.xlsx
over itself**.
- Something had clicked the GUI's AutoSave toggle in its title bar: the measured window is a real,
  visible window. F9 had made the workbook dirty (`calc_now` always replaces the workbook, even when
  no value changes), and AutoSave saves a dirty `.xlsx` within 2 s.
- Every w80 run after that point was marked invalid in the raw data (label suffix `_inv80`).
- w80 was regenerated with `gen.py` (seed 42). It was proved identical: c8's conversion of it equals
  c8's conversion of the original, made at 13:31, in all 10,368,467 cells and 3,456,120 formula
  values.
- All w80 runs were then done again.
- All bench inputs are now read-only. The other files were never touched (their timestamps are from
  Oct 15).
- Upstream issues worth filing:
  - F9 on an unchanged workbook marks it dirty;
  - with AutoSave on, it then rewrites the file.

## 2. Results (MEASURED)

### 2.1 Desktop app vs Excel (medians of 3; MB)

| File | App | open s | peak WS (open) | **private WS steady** | private bytes steady | WS steady | F9 s | peak WS after F9 | **private WS after F9** |
|---|---|---|---|---|---|---|---|---|---|
| w40 | Excel | 29 | 479 | **389** | 412 | 462 | 0.3 | 479 | **389** |
| | round 1 | 36 | 985 | **904** | 930 | 939 | 8 | 1,426 | **1,218** |
| | round 2 | 32 | 646 | **566** | 588 | 602 | 5 | 667 | **566** |
| w80 | Excel | 58 | 851 | **742** | 775 | 815 | 1.3 | 851 | **742** |
| | round 1 | 71 | 1,499 | **1,401** | 1,439 | 1,436 | 17 | 2,409 | **2,026** |
| | round 2 | 63 | 984 | **903** | 932 | 938 | 10 | 1,071 | **905** |
| w120 | Excel | 88 | 1,222 | **1,090** | 1,134 | 1,163 | 1 | 1,222 | **1,090** |
| | round 1 (2 runs) | 105 | 2,425 | **1,897** | 1,947 | 1,932 | 26 | 3,540 | **2,836** |
| | round 2 | 96 | 1,321 | **1,240** | 1,274 | 1,275 | 15 | 1,476 | **1,241** |
| x40 | Excel | 23 | 455 | **381** | 404 | 454 | 0 | 455 | **381** |
| | round 1 | 28 | 980 | **900** | 925 | 935 | 8 | 1,421 | **1,212** |
| | round 2 | 20 | 659 | **578** | 599 | 613 | 5 | 680 | **579** |
| x80 | Excel | 44 | 797 | **724** | 755 | 797 | 0 | 798 | **724** |
| | round 1 | 56 | 1,514 | **1,389** | 1,425 | 1,424 | 16 | 2,398 | **2,015** |
| | round 2 | 40 | 1,009 | **928** | 956 | 963 | 10 | 1,098 | **929** |
| x120 | Excel | 65 | 1,139 | **1,064** | 1,106 | 1,136 | 1 | 1,139 | **1,064** |
| | round 1 | 82 | 2,444 | **1,876** | 1,923 | 1,911 | 25 | 3,520 | **2,814** |
| | round 2 | 59 | 1,355 | **1,274** | 1,309 | 1,309 | 15 | 1,511 | **1,276** |

- The third round-1 w120 run was stopped by the RAM guard during F9, at 3,353 MB private and still
  growing. One round-1 w80 run's after-F9 sample (2,278) includes the AutoSave of §1; the median is
  not affected.
- **Excel's own UI costs nothing extra.** Excel *visible* (one run): w80 748 MB, x80 730 MB, against
  742/724 invisible. So comparing with the invisible Excel is fair for the owner's Task Manager view.
- **Empty workbook (the fixed cost):** Excel 33 MB (37 visible). gridcraft round 1 406 MB, round 2
  226 MB; on the OpenGL backend 134 MB.

### 2.2 Engine without a window, and the CLI peak (MB)

| File | Excel peak WS | CLI `info` peak, round 1 → **round 2** | engine steady (`mcp`), round 1 → **round 2** | engine after F9, round 1 → **round 2** |
|---|---|---|---|---|
| w40 | 479 | 751 → **413** | | |
| w80 | 851 | 1,495 → **817** | 998 → **680** | 1,621 → **679** |
| w120 | 1,222 | 2,421 → **1,223** | | |
| x40 | 455 | 760 → **369** | | |
| x80 | 797 | 1,510 → **731** | 986 → **704** | 1,609 → **704** |
| x120 | 1,139 | 2,440 → **1,092** | | |

CLI open times are the same as the GUI's within a few seconds (w80 69 → 66 s, x80 54 → 39 s).

### 2.3 Speed

- **Opening:** round 2 opens the Excel-saved files faster than Excel (x80: 40 s vs 44 s; x120: 59 vs 65),
  because a calculated file's values are kept (§4, commit 7). It is about 10% slower than Excel on the
  openpyxl files, which both apps must calculate (w80: 63 vs 58 s).
- **Recalculation:** F9 is 10 s on w80 against Excel's 1.3 s. Excel calculates on several threads;
  gridcraft has none (`docs/architecture.md:62`). This is a speed gap, not a memory one.

## 3. What the steady state consists of (w80, round 2, 903 MB private WS)

| Part | MB | How known |
|---|---|---|
| Desktop app with an empty workbook | 226 | MEASURED |
| — font files read whole at start-up | 57 | MEASURED: file sizes of the faces `theme.rs` loads on this PC. CJK fallbacks: `msyh.ttc` 19.7, `YuGothR.ttc` 13.7, `malgun.ttf` 13.5 |
| — DirectX 12 renderer and driver (above OpenGL) | ~92 | MEASURED: 226 vs 134 on `WGPU_BACKEND=gl` |
| — egui, winit, wgpu, program image, the rest | ~77 | difference |
| Workbook model (requested heap, probe) | 605 | MEASURED (`probe_r4`, w80) |
| — cell store (bands, cell slots) | 446 | 45.1 B/cell for 10.4M cells |
| — formulas (`Arc<Formula>` per formula cell; trees shared per run) | 106 | 32 B per formula |
| — text values (`Arc<str>`) | 53 | 1.15M distinct strings for 2.3M text cells |
| — dependency graph | 0.2 | runs of shared formulas, round 1 |
| Heap overhead (Windows heap headers and rounding, ~12%) | ~72 | 903 − 226 − 605 |

**Not kept after the open** (CODE, confirmed by the probe and the engine-only measurement):
- **Parsed XML.** Rows are pull-parsed and dropped, and each sheet's decompressed part is dropped after
  the sheet. From commit 10 on, the file itself is read part by part.
- **File bytes.** Freed before anything is built.
- **Shared-string table.** Dropped after reading (its strings live on in the cells).
- **Recalculation state.** The pass's slots are freed at its end. The graph is 0.2 MiB.
- **Undo snapshots.** The history is empty after the open. `DocState.saved` is an `Arc` sharing every
  band, so it costs nothing until an edit.
- **Render caches.** Galleys are laid out per frame for the visible cells only (scout of
  `crates/ui-egui`, `grid.rs:1191-1300`). AutoComplete scans on demand.

The engine-only steady state (680 MB) is the model plus about 75 MB of heap overhead and the CLI
itself.

**Round 1's steady state (w80, 1,401 MB) for comparison:**
- the app at 406 MB;
- model 920 MiB requested (cell store 603, formulas 237, text 80, graph 0.2);
- heap overhead.

After F9, round 1 added 625 MB: every band holding a formula was copied (§4, commit 1).

## 4. The changes and what each saved

### 4.1 The series

Every commit passes `cargo check --workspace --all-targets` with 0 warnings on its own (checked
commit by commit). `git am` of 0009–0019 onto round 1 gives the branch's exact tree (`533cba33…`).
In all: 30 files, +1,368 −244, of which production about +933 −404 and tests +638 −43.

| # | Patch | Commit | Change | Files | prod +/− | tests + |
|---|---|---|---|---|---|---|
| 1 | 0009 | `d5d4c69` calc: write back only the values a recalculation changed | compare before `get_mut`, which copied the band (`Arc::make_mut`) first | 2 | +9 −6 | 29 |
| 2 | 0010 | `38c6021` engine: free a file's bytes before the first recalculation | `drop(bytes)` before `DocState::new` | 1 | +7 −3 | — |
| 3 | 0011 | `4e569a8` calc: hold a recalculation pass in per-column slots, not hash maps | `PassSlots`: per column a dense `Vec<Slot>` (map fallback) instead of `exprs`/`results`/`pending` maps; trees taken from the cells | 1 | +158 −51 | 11 |
| 4 | 0012 | `95cb269` model: keep shifted copies of formulas to 16 bytes | `enum Repr { Own(Box<Own>), Shifted{Arc<Expr>,i32,i32} }`; `array()`/`set_array()`/`with_array()` | 21 | +85 −43 | 48 |
| 5 | 0013 | `1d317a4` xlsx: share one string among inline-string cells with the same short text | per-sheet `TextPool` (≤ 64 B, ≤ 65,536 texts) | 2 | +34 −4 | 24 |
| 6 | 0014 | `e078719` model: keep a row's cells in a vector sized to them | `Row(Vec<(u32, Cell)>)` instead of `BTreeMap<u32, Cell>`; whole-row shifts; `shrink_to_fit` | 4 | +236 −48 | 321 |
| 7 | 0015 | `eb5489d` calc, xlsx, engine: keep a calculated file's values on open, as Excel does | `ReadReport::calculated`, `io::open_bytes_calculated`, `DocState::open`, `Calc::open` | 10 | +70 −16 | 147 |
| 8 | 0016 | `4ff8599` gridcraft: ask wgpu for small GPU memory blocks | `MemoryHints::MemoryUsage` in the eframe device descriptor | 1 | +11 −1 | — |
| 9 | 0017 | `fc34ad4` model: keep a band's cells in one vector | per band: row starts, columns and cells in three vectors; take/merge shifts; a band appended after another is sized like it | 2 | +257 −221 | 23 |
| 10 | 0018 | `5dd5c18` xlsx, engine: read an XLSX file from the file, not from its bytes | `Package` over any `Read + Seek`; `read_xlsx_reader`; `io::open_xlsx_file` | 6 | +58 −10 | 18 |
| 11 | 0019 | `3884ffa` calc: on open, calculate the formulas the saving program couldn't | cached `#NAME?` formulas and their dependents are calculated | 2 | +8 −1 | 17 |

Commit 9 replaces commit 6's row vectors. Both are kept so the saving of each step is measured; for
upstream they would be squashed.

### 4.2 What each saved (MEASURED, MB)

CLI peak WS (`info`) and engine steady private WS (`mcp`), one run each per build. Both are
deterministic to within a few MB, except the single-run noise noted below.

| After commit | CLI peak w80 | + F9 peak w80 | CLI peak x80 | engine steady w80 | engine steady x80 | Saved by this commit |
|---|---|---|---|---|---|---|
| round 1 | 1,495 | 1,974 | 1,510 | 998 (after F9: 1,621) | 986 (1,609) | |
| 1–2 | 1,419 | 1,421 | 1,408 | 998 (999) | 986 (987) | F9 no longer copies the bands: −625 after F9. Bytes: −76/−102 peak |
| 3 | 1,134 | 1,135 | 1,123 | 996 | 984 | pass state: −285 peak (it is transient, so steady is unchanged) |
| 4 | 1,029 | 1,030 | 1,019 | 891 | 881 | 16-byte formula: −105 peak, −105 steady |
| 5 | 995 | 994 | 1,019 | 856 | 881 | text pool: −34/−35 on inline-string files; 0 on x80 (shared strings) |
| 6 | 924 | 924 | 948 | 786 | 810 | row vectors: −71/−70 |
| 7 | 925 | 924 | 941 | 786 | 810 | kept values: x80 opens in 40 s, not 51 (the w-files must be calculated) |
| 9–11 | **818** | **818** | **731** | **680** | **704** | bands −106 steady. Peak −107 on w80; −210 on x80, where the peak is at read time and the file's 103 MB are no longer held (the split between the two commits was not measured) |

The desktop-only commit 8 took the empty app from 406 to 226 MB. This saving is in every GUI number.

Smaller notes:
- **Commit 3's after-F9 engine samples on w80 are noisy** (996 → 1,051 → back down in later commits)
  because they are single runs. x80 shows no such noise.
- **Commit 2's −76 MB on w80** is the file's bytes leaving the recalculation peak. After commit 10 the
  peak of a file that must be calculated is the recalculation (model + 156 MiB of pass state), so
  commit 10 lowers only the peaks of calculated files (x80: −107 MB).
- **Probe, requested heap, w80, round 1 → round 2:**
  - model 920 → 605 MiB (−34%);
  - open peak 1,361 → 761 MiB (−44%);
  - recalculation transient 441 → 156 MiB;
  - read time 59.9 → 61.3 s.

### 4.3 Measured and not adopted

- **The Windows segment heap** (manifest `heapType SegmentHeap`). It was A/B-tested on c8 by patching
  the binary's embedded manifest at the same length (`spikes/xlsx-bench/segheap_patch.py`;
  `heaptype.py` confirmed the process heap type).
  - Memory: GUI w80 1,011 → 928 MB (−8%), x80 1,035 → 950, CLI w80 925 → 843.
  - Speed: **opening 16–33% slower** (w80 68 → 79 s, x80 40 → 53 s).
  - It is also a toolchain risk: MinGW and MSVC each embed a default manifest.
  - Not adopted. A faster allocator (mimalloc) is the alternative worth measuring (§6).
- **A lazy dependency graph** (built on the first edit). The graph is 0.2 MiB after round 1's runs, so
  there is nothing to save.

### 4.4 Found on the way

- **Commit 9 first doubled the read time** (an 80 MB workbook: 84 s instead of 46). The cause: band
  vectors grown cell by cell are reallocated at every doubling. Those are 23–40 KB blocks outside
  Windows' low-fragmentation heap; they fragment it and leave committed but untouched pages (private
  bytes 235 MB above the private WS).
  - Fixed in the same commit: a band started right after another is sized like it.
  - Read time back to 45.6 s, and private bytes ≈ private WS again.
  - Found because the first round-2 measurement batch showed it; that batch was discarded.
- **Commit 7, as first written, showed Excel's `#NAME?` for XLOOKUP.** The 32-bit Excel 16 on this
  PC has no XLOOKUP, so the Excel-saved files carry `#NAME?` in the Summary.
  - Comparing the kept values with gridcraft's own calculation of x10 found exactly those 30 cells,
    and no other difference among 432,120 formulas.
  - Commit 11 calculates cached `#NAME?` formulas (and their dependents) on open. With it the
    comparison is identical.

## 5. Tests and correctness

### 5.1 Unit and integration tests (MEASURED, at the tip `3884ffa`)

| Crate / suite | Round 1 | Round 2 | New round-2 tests |
|---|---|---|---|
| formula (unit + `spanish_input`) | 25 + 5 | 25 + 5 | — |
| model | 22 | 26 | `shifted_copies_are_small_and_array_ranges_live_in_own_formulas`; `bands_behave_like_the_b_tree_store` (differential: 40 seeds × 400 random set/remove/blank-in-place/shift_rows/shift_cols/shift_cols_in_rows/shift_rows_in_cols/take_range/snapshot/shrink, compared after every step with the old B-tree store kept in the test: cells, count, used range, diffs against snapshots, range and row iteration, JSON); `bands_filled_in_order_are_sized_like_the_band_before`; `bands_give_back_spare_room` |
| calc | 61 + 1 ignored | 64 + 1 ignored | `recalculation_keeps_unchanged_sheets_shared_with_snapshots`, `open_keeps_calculated_values_unless_a_formula_needs_calculating`, `open_calculates_what_the_saving_program_could_not` |
| xlsx | 175 | 178 | `inline_strings_with_the_same_short_text_share_one_string`, `a_workbook_calculated_on_save_is_reported_as_calculated`, `reading_from_a_reader_gives_what_reading_the_bytes_gives` |
| engine lib | 286 | 286 | — |
| engine integration | 49 (13 files) | 51 (14 files) | `tests/open_calculated.rs`: a stale cached value is kept (and F9 recalculates it); `fullCalcOnLoad` is calculated on open |
| ui-egui + gridcraft app | 675 (48 suites, ui-egui) | **698 in 49 suites pass** | — (mechanical `array()` change in `formula_bar.rs`) |
| `cargo check --workspace --all-targets` | clean | **clean, 0 warnings, at every commit** | |
| rustfmt / clippy (`cargo xtask ci`) | not run | **not run**: components not installed | |

### 5.2 Round-1 correctness suite, rerun on the round-2 tip (MEASURED; `E:/office-target/gridcraft-spike/r2/correct.sh`)

| Check | Result |
|---|---|
| w10 `convert`, every cached value vs the unpatched build | 1,296,467 cells / 432,120 formulas: identical |
| c10 (xlsxwriter, `fullCalcOnLoad`) | 810,467 / 270,120: identical |
| w40 | 5,184,467 / 1,728,120: identical |
| Formula text: the unpatched build re-saves our w10, compared with its own save | 432,120 formulas: identical |
| w80 rows vs Python (`expect_rows.py`) + Summary (`expect_summary.py`) | 3,456,000 values + 120: all match (calculated on the restored w80, §1) |
| w120 rows + Summary | 5,184,000 values + 120: all match |
| 35-step edit script on w10, vs the unpatched build | 1,296,737 values identical; 432,400 formula texts identical after a base re-save |
| Excel 16 (COM, `excel_compare.ps1`): our files vs the unpatched build's, `Formula` and `Value2` after `CalculateFull` | w10 1,296,467 cells / 432,120 formulas and edit-script result 1,321,372 / 432,400: identical, no open errors |
| Kept values (commit 7 + 11) vs gridcraft's own calculation of the Excel-saved x10 | 1,296,467 / 432,120: identical. Without commit 11: 30 differences (§4.4) |
| Fuzzer (`fuzz_seeds.py`), seeds 0–199 × 40 and 0–99 × 100, release | identical to round 1 and the unpatched build. 138/40 (undo invariant) and 79/100 (`chart.set` round trip) fail in every build (upstream bugs). **33/100 and 51/100, which exceeded the 2.5 GB cap in both earlier builds, now pass** (785 and 2,077 MB) |

### 5.3 Semantics that changed (with tests)

- **Commit 7/11: values kept on open.** A workbook whose `calcPr` has no `fullCalcOnLoad` (Excel's and
  gridcraft's own saves) keeps its cached values on open. They are calculated anyway when:
  - a formula has no value;
  - the graph has a dynamic formula (volatile functions, INDIRECT/OFFSET, names, structured
    references, unknown functions; `eval.rs:1766-1772`);
  - spilled values were left out on read;
  - a value is `#NAME?` (in that case only those formulas and their dependents are calculated).
  F9 still calculates everything. Files with `fullCalcOnLoad` (openpyxl, xlsxwriter) and other formats
  are calculated as before.
  Known differences from Excel:
  - the circular-reference indicator only appears after the first calculation;
  - a file without `fullCalcOnLoad` saved by an older calc engine is trusted, where Excel would
    recalculate by `calcId`.
- **Commit 4: `Formula::array`** is now `array()`, `set_array()` and `with_array()`: an API change
  for downstream crates. A shifted copy given an array range becomes a formula of its own (same
  meaning, tested).

## 6. Gap analysis against Excel, and what it would take to close it

At w80, round 2 is 903 MB against Excel's 742 (+161). By part:

| Part | gridcraft | Excel | Gap | What would close it | Saving (w80) | Effort | Risk |
|---|---|---|---|---|---|---|---|
| Empty app | 226 | 33 | +193 | (a) load CJK/Thai/Hebrew fallback faces only when text needs them (UI language or workbook text) | −46 MB (MEASURED file sizes) | 1–1.5 d | low |
| | | | | (b) the DX12 footprint above OpenGL (~92 MB): find what wgpu/driver keep (swap-chain buffers, staging, pipeline caches); OpenGL as default where it is stable (upstream chose DX12 for driver-crash reasons, `graphics.rs`) | up to −92 (UNVERIFIED) | 1–2 d to investigate | medium |
| Document | 677 | 709 | −32 | already at Excel's level | | | |
| — heap overhead (~72) | | | | (c) a faster allocator: the segment heap gives −8% but opens 16–33% slower (§4.3); mimalloc is untested | −50…80 (UNVERIFIED) | 1 d to measure | low–medium (dependency, C code) |
| — cell slot (40 B, `Value` 24 B) | | | | (d) a 16-byte `Value` needs a thin text pointer: not possible in safe Rust without double allocation, and the workspace forbids `unsafe`. Or text as ids into a string pool (lifetime/API change in `core`) | −80 (8 B × 10.4M cells) | 5–8 d | high |
| — `Arc<Formula>` per formula cell (32 B + header) | | | | (e) hold a shifted copy inline in the cell (`Cell::formula` changes type; about 150 call sites) | −110 (UNVERIFIED) | 4–6 d | medium–high |
| Peak during F9 | +166 above steady | 0 | | pass state 156 MiB (24 B/formula + the dirty list); Excel's is invisible here | | | |
| F9 time | 10 s | 1.3 s | ×8 | multi-threaded recalculation (upstream's beta gate) | speed | upstream | high |

**Projection (UNVERIFIED):**
- (a) + (c) + (e) would put the desktop app at about 670–720 MB on w80, i.e. Excel's 742 or below.
- (a) alone takes it to about 857 (1.15× Excel).
- The engine (CLI) is already at or below Excel's peak.

**Not done this round, though planned:**
- **Plan step 3:** the streaming writer (W, saving: +1.0 GiB at save on w120 per
  `gridcraft-scaling-and-links.md`) and bounded undo (U1).
  - Neither affects the open/steady/recalc metrics measured here.
  - Commit 1 already removes the largest snapshot cost: F9 copying every band while `saved` shares
    them.
  - Edits with undo history still copy the bands they touch.
- **Parse each run's formula once.** Not attempted: speed only, and it needs the `parse(print(t)) == t`
  proof round 1 described.

## 7. Updated effort estimate

Round 1 estimated 5–9 days to production for its steps 1–2. Round 2 adds the following:

| Item | Why | Effort |
|---|---|---|
| Squash 0014 into 0017; benchmark edits on wide sheets (a band edit is O(band); up to 1M cells for 64 full rows) and Insert/Delete on large sheets | the new store's worst cases are not benchmarked | 1.5–2 |
| Review the kept-values policy (commit 7/11): `calcId`, volatile list, a UI cue, the circular indicator; a corpus of real Excel files from several versions | the only changed semantics | 1–2 |
| Streaming read for the other readers (xlsb, ods) | commit 10 covers XLSX | 0.5 |
| wgpu `MemoryUsage` on discrete GPUs and other drivers (frame time) | measured on one Intel UHD 630 only | 0.5 |
| `cargo xtask ci` (rustfmt, clippy `-D warnings`, layers, wasm) | not available on this PC | 0.5 |
| Upstream review latency, messages with task ids (`CLAUDE.md` of upstream) | | — |
| **Round 2 to production** | | **5–8 d** (incl. 1 d of tests on real owner files) |
| Closing the GUI gap: (a) fonts 1–1.5 d, (b) renderer 1–2 d, (c) allocator 1 d | §6 | 3–4.5 d |
| Optional: (e) inline shifted formulas 4–6 d; (d) 16-byte `Value` 5–8 d | §6 | 4–14 d |
| Plan step 3: streaming writer 2–4 d + bounded undo 1.5–2.5 d | save and edit memory, not measured here | 3.5–6.5 d |

**Memory core total from here:** round 1's 5–9 d + round 2's 5–8 d + the GUI gap's 3–4.5 d ≈
**13–21.5 engineer-days** to reach Excel's level in Task Manager for these files (UNVERIFIED for the
GUI items), plus 3.5–6.5 d for save/undo.

## 8. Risks and things not verified

- **Measured on one PC, one GPU and the synthetic bench.** Real owner files (more styles, fewer
  formulas, merged cells, wide rows) will shift the parts. The per-unit costs in §3 make a census
  (round-1 plan step 0) straightforward.
- **The band store's worst cases are untested:**
  - wide rows: each edit moves the rest of the band;
  - random edits after load: an appended band reserves its neighbour's size.
- **The segment-heap A/B was done on c8, not the tip.** Its relative result should carry over.
- **The desktop app was measured with a visible window on a shared desktop.** That window is how the
  AutoSave incident happened (§1). All inputs are now read-only.

## 9. Reproducing

All scripts are in `spikes/xlsx-bench/`; the batches are in `E:/office-target/gridcraft-spike/r2/`.

| Script | What it does |
|---|---|
| `memsample.py` | One sample of a process's counters (shared by all drivers) |
| `excel_mem.ps1` | Excel via COM: open / steady / CalculateFull / steady. `-Visible` for a visible run. Stops only its own PID |
| `gui_mem.py` | gridcraft.exe via its control port, the same phases |
| `cli_mem.py` | the engine without a window (`gridcraft-cli mcp --in`), the same phases |
| `measure_tpl.py` | CLI peak (round 1's harness) |
| `mem_table.py` | the tables of this document |
| `heaptype.py`, `segheap_patch.py` | which heap a process runs on; a copy of a binary with the segment heap |
| `mem-probe-r2/` | counting-allocator probe: live heap of the opened model, by component |
| `r2/measure_all.sh`, `master*.sh`, `attribution.sh`, `percommit_r2.sh`, `correct.sh`, `build_r1.sh` | builds and measurement batches. Every step waits for a quiet machine |

**Data:**
- `E:/office-target/gridcraft-spike/r2/mem_results.jsonl` and `../results.jsonl`;
- probe logs `r2/probe_*.txt`;
- outputs `r2/out/`;
- the overwritten w80 is kept as `E:/office-target/xlsx-bench/var/w80_gridcraft_autosaved.xlsx`.
