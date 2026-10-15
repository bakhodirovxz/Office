# gridcraft: memory scaling and external workbook links (Phase 0 feasibility)

Date: 2026-10-15. Upstream: `upstream/gridcraft` at e6b6b34. Source paths are relative to that
repo unless they start with `spikes/`. Nothing in the gridcraft workspace was compiled.

**Evidence types used below:**
- **MEASURED**: a run on this PC.
- **CODE**: read in the source; the `file:line` is given.
- **PROJECTION**: arithmetic from measured per-unit costs.
- **UNVERIFIED**: a design estimate or a guess.

## 0. How this was measured

**Release CLI.** `gridcraft-cli 0.7.0 info` was run under a RAM guard
(`spikes/xlsx-bench/measure_var.py`, a copy of `measure.py` writing to
`E:/office-target/xlsx-bench/var/results.jsonl`). It measured w10 and three variants:
- `v10_num`: columns A–D only, numbers.
- `v10_val`: A–F, numbers + text, no formulas.
- `w10_shared`: w10 with G/H/I rewritten as Excel-style `t="shared"` runs.

The variants were built with `spikes/xlsx-bench/genvar.py` and `mkshared.py` (same seed as `gen.py`).

**Memory probe.** The probe is `spikes/xlsx-bench/mem-probe/`.
- It is a standalone crate that depends on gridcraft's `core`, `formula`, `model`, `calc` and
  `xlsx` crates by path, and on nothing else of the workspace.
- It was built with `-j 2`, `--offline` and `CARGO_TARGET_DIR=E:/office-target/probe`, in the dev
  profile at opt-level 1. The build took 1 min 53 s.
- A counting global allocator records requested heap bytes and live allocations per phase. It
  calls the same functions the engine calls on open: `read_xlsx` → `Calc::recalc_all`
  (`crates/engine/src/cmd/file.rs:59,79`, `crates/engine/src/lib.rs:120-123`), then
  `write_xlsx`.
- Logs: `E:/office-target/xlsx-bench/var/logs/probe_w10*.out`.

The data-structure sizes below do not depend on the optimisation level. Run time in the probe
does (it is slower than release).

**Mapping requested heap to the Windows peak working set.** The probe's peak requested heap for
the open path is 546.9 MiB on w10. The release CLI's peak working set is 635 MiB. The factor is
**1.16** (MEASURED on w10 only; heap headers and fragmentation).

## 1. Where the memory goes

### 1.1 Headline (MEASURED, w10: 1,296,467 cells, of which 432,120 are formulas)

| Release CLI `info` | Peak working set | Open time |
|---|---|---|
| `v10_num`: 576k numbers, no text, no formulas | **88 MiB** | 1.7 s |
| `v10_val`: + 288k text cells, no formulas | **103 MiB** | 1.8 s |
| `w10`: + 432k formulas | **635 MiB** | 10.7 s |
| `w10_shared`: the same formulas as Excel shared-formula runs | **619 MiB** | 8.0 s |

**Formulas cause about 84% of the peak and most of the open time.** XML parsing, numbers and
text are cheap.

Probe phases on w10 (requested heap, MiB):

| Phase | MiB | Share of the 546.9 peak | Source |
|---|---|---|---|
| Formulas in the model: `Arc<Formula>` + text + `Arc<Expr>` + AST | 279.2 (432,120 × ~678 B) | **51%** | §1.3 |
| Calc dependency graph (live after `rebuild`) | 134.2 (326 B/formula) | **25%** | §1.4 |
| Non-formula cells + row/band containers + text | 85.2 (~99 B per value cell) | 16% | §1.2 |
| Recalc pass state (transient in `recalc_all`) | 48.3 (~117 B/formula) | 9% | §1.4 |
| XML transient during `read_xlsx` (peak 376.0 minus live 373.9) | 2.1 | **<0.5%** | §1.5 |
| Save (`write_xlsx`): transient on top of the live model | +109.0 | (save path) | §1.5 |

**Scaled to the owner's sizes (PROJECTION).** w120 has 12× the rows of w10: 5,185,440 formulas
and about 10.4M value cells.

| Item | w120 MiB |
|---|---|
| Formulas | 3,350 |
| Graph | 1,610 |
| Value cells | 1,022 |
| Recalc transient | 580 |
| File bytes | 113 |
| **Total requested heap** | **6,675** |
| **× 1.16 = working set** | **≈ 7.6 GiB** |

This matches the linear 64 MB/MB projection in `xlsx-bench.md`. w80 (×8) projects to **≈ 5.0 GiB**.

### 1.2 Cell representation (CODE + MEASURED `size_of`)

- **`Value`** (`crates/core/src/value.rs:146-154`): `Empty | Number(f64) | Text(Arc<str>) | Bool |
  Error | Array(Arc<Array>)`. `size_of::<Value>()` = **24 B**, because `Arc<str>` is a 16-byte fat
  pointer.
- **`Cell`** (`crates/model/src/cell.rs:55-61`): `{ value: Value, formula: Option<Arc<Formula>>,
  style: StyleId(u32) }`. **40 B**. The style is a 4-byte id into an interned `StyleTable`
  (`crates/model/src/style.rs:364-400`), so styles are not a per-cell cost.
- **`CellStore`** (`crates/model/src/store.rs:14-26`):
  - It is `BTreeMap<band, Arc<Band>>`.
  - A band is 64 rows: `BTreeMap<row, BTreeMap<col, Cell>>`.
  - Each row with cells allocates at least one B-tree leaf with 11 `Cell` slots, about 496 B.
  - Probe: the first 4 numeric cells of a row cost **137 B/cell** (≈548 B/row). Two more text
    cells add only 24.5 B/cell, because they fill free leaf slots, plus about 49 B for one
    `Arc<str>` (MEASURED, synthetic 19,200-row sheet).
- **Text.** Each text cell holds an `Arc<str>`.
  - Shared-string (`t="s"`) cells clone the `Arc` from the SST, so repeated strings are shared
    (`crates/xlsx/src/sheet_read.rs:396-398`; SST built at `crates/xlsx/src/read.rs:143-159`).
  - Inline strings (as in the bench files, `t="inlineStr"`) allocate one `Arc<str>` per cell
    (`sheet_read.rs:407-409`).

### 1.3 Formula storage: one full AST per cell, shared formulas expanded (CODE + MEASURED)

- **`Formula`** (`crates/model/src/cell.rs:12-20`) is `{ text: String, expr: Option<Arc<Expr>>,
  array: Option<RangeRef> }` = 56 B. A cell holds it behind another `Arc`.
- **`Formula::new`** parses the text and then *prints it again* to store canonical text
  (`cell.rs:25-31`).
- **The reader formats each formula twice.** It first runs `from_file`, which also parses and
  prints (`crates/xlsx/src/fmla.rs:109-118`, called at `sheet_read.rs:459`). It then calls
  `Formula::new` (`sheet_read.rs:467, 487, 505`). So each formula cell is parsed twice and
  printed twice on load.
- **`Expr`** (`crates/formula/src/ast.rs:186-205`) is **96 B**, because the largest variant,
  `Ref(Reference)`, is 72 B:
  - `Reference` = `SheetSel` (48 B; `Named(String)`/`Span(String,String)`, `ast.rs:34-40`) +
    `RefKind` (24 B).
  - Every child is a `Box<Expr>` (96 B even for the number `500`).
  - Function names are an owned `String` per call (`Expr::Call(String, Vec<Expr>)`).
  - Sheet names are an owned `String` per reference.
- **Per-formula heap** (MEASURED, probe):

  | Formula | Heap | Allocations |
  |---|---|---|
  | `IF(C2>500,C2*D2,0)` | 986 B | 9 |
  | `VLOOKUP(F2,Lookup!$A$2:$B$21,2,FALSE)` | 645 B | 6 |
  | `S02!C2+D2` | 395 B | 6 |
  | Synthetic sheet average | 677.6 B/formula | 7 |

- **Shared formulas are expanded.** For each follower cell of a `t="shared"` run, the reader
  clones the master AST, shifts it, prints new text and allocates a new `Arc<Formula>` and
  `Arc<Expr>` (`sheet_read.rs:462-480`).
  - MEASURED on `w10_shared`: still **432,120 distinct `Arc<Formula>` and 432,120 distinct
    `Arc<Expr>`**, and model live is 351 vs 364 MiB.
  - Excel-saved files, which use shared runs for filled-down columns, therefore get no memory
    benefit. They only open faster: 8.0 vs 10.7 s, because one parse per run is saved.

### 1.4 Calc dependency graph and recalc state (CODE + MEASURED)

- **Graph structure** (`crates/calc/src/recalc.rs:190-202`):
  - `nodes: HashMap<Key, Node{areas: Vec<Area>, dynamic}>`
  - `cell_deps: HashMap<Key, HashSet<Key>>`: one heap-allocated `HashSet` per precedent cell.
  - `col_deps`: interval index per (sheet, column).
  - `wide_deps`
- **Sizes:**
  - `Key = (usize, CellRef)` = 16 B (`recalc.rs:49`).
  - `Area` = 24 B (`crates/calc/src/eval.rs:23-26`).
- **Built for every formula on open:**
  - `Calc::rebuild` walks every formula (`recalc.rs:797-810`).
  - On each one it calls `Formula::expr()`, which *clones* the whole AST, though only
    temporarily (`cell.rs:36-38`).
  - It also calls `precedents()` (`eval.rs:1736-1753`).
- **MEASURED:** **134.2 MiB live = 326 B per formula**, built in 0.9 s.
- **Recalc pass state.** `recalc_all` (`recalc.rs:813-827`) runs every formula through `run`
  (`recalc.rs:951-975`). That builds `exprs: HashMap<Key, Arc<Expr>>`, `results: HashMap<Key,
  Value>` and `pending: HashSet<Key>`, each sized to all formulas (`recalc.rs:382-420`).
  MEASURED: +48.3 MiB transient, about 117 B/formula.
- **Threading:** none (`docs/architecture.md:62`, `docs/hardware-parity.md:10`).

### 1.5 XML, zip and save buffering (CODE + MEASURED)

**Reading:**
- The whole xlsx file is read into one `Vec<u8>` (`crates/engine/src/io.rs:404-409`) and parsed
  from memory (`crates/xlsx/src/read.rs:69`, `package.rs:41-53`).
- Each sheet part is decompressed whole (`read_to_end`, `crates/xlsx/src/package.rs:65-87`;
  `sheet_read.rs:43`).
- The sheet is then parsed with a **pull parser** (quick-xml `Reader`). It hands each `<row>` to
  a callback and drops it, so no full DOM of the sheet is kept (`crates/xlsx/src/xml.rs:1-2,
  155-247`; `sheet_read.rs:47-50`). The SST is streamed item by item (`read.rs:143-151`).
- **Peak duplicates during load are small:**
  - the compressed file (9.5 MiB on w10; 113 MiB on w120);
  - one decompressed sheet (2.2 MB on w10; 27.4 MB on w120, measured with
    `spikes/xlsx-bench/zipsizes.py`);
  - one row's `El` tree.
- MEASURED: the read peak exceeds the live model by only 2.1 MiB.

**Upstream's #175 framing does not match the profile.** Upstream describes #175 as "no
streaming" (`docs/architecture.md:64`, `docs/file-format-parity.md:77`). The probe shows the
reader already streams rows; the memory is in the model and the graph. (The text of issue #175
itself was not read: `gh` is not installed. **UNVERIFIED**.)

**Hard size cap.** `MAX_TOTAL = 1 GiB` of decompressed parts per package
(`crates/xlsx/src/package.rs:13`, enforced at `:74-85`).
- Uncompressed totals (MEASURED, decompression ratio ≈6.9): w10 66 MB, w40 270 MB, w80 546 MB,
  **w120 822 MB**.
- A file with these contents fails with "package expands to more than 1 GB" at about **150 MB
  on disk**, however much RAM the PC has (PROJECTION).
- The owner's files "keep growing", so this is a wall independent of memory.

**Saving.** The writer keeps every uncompressed part in `Out.parts`
(`crates/xlsx/src/write.rs:107-112, 132-137`) and zips them only at the end (`write.rs:453-469`).
The shared-string table stores each unique string twice: `list: Vec<String>` + `map:
HashMap<String,u32>` (`write.rs:86-104`).
- MEASURED: +109 MiB on w10.
- PROJECTION: **≈1.3 GiB extra at save on w120**.
- The writer never emits shared formulas (no `shared` in `crates/xlsx/src/sheet_write.rs`), so
  saved files are larger than Excel's.

### 1.6 Undo snapshots: copy-on-write works, except for wide recalc (CODE + MEASURED)

- **Mechanism:**
  - The workbook is `Arc<Workbook>` with `Vec<Arc<Sheet>>` (`crates/model/src/workbook.rs:89-90`).
  - Cells live in `Arc<Band>`s of 64 rows (`store.rs:1-4, 14`).
  - Before each command, `execute` keeps the old `Arc` as the undo entry
    (`crates/engine/src/lib.rs:323, 348-359`), up to `UNDO_LIMIT = 100` (`lib.rs:118`).
- MEASURED: one-cell edit = **14.1 KiB** per undo step. That is fine.
- **The problem is recalc write-back.**
  - Recalculated values are written into the cells through `CellStore::get_mut`
    (`recalc.rs:994`), which calls `Arc::make_mut` on the band (`store.rs:42-48`).
  - Every band that holds a formula whose value changed is therefore deep-copied (its row
    B-trees; the formulas themselves stay shared).
  - This happens whenever an older snapshot shares those bands. An older snapshot always exists:
    `DocState.saved` is a full `Arc` snapshot used only for the dirty flag
    (`lib.rs:127, 142-144`).
- MEASURED: changing `Lookup!B2` (it feeds all 144,000 VLOOKUPs) with a snapshot alive costs
  **+75.4 MiB**, which is 15% of the live workbook.
- PROJECTION: **≈0.9 GiB per such edit on w120**, kept for as long as `saved` or any of the 100
  undo entries refers to the old bands.

## 2. Options to get 80–120 MB under about 1.5–2 GB

Per-unit costs are from §1 (requested heap); working set = ×1.16. The w120 baseline is 6,675 MiB
(formulas 3,350, graph 1,610, values 1,022, recalc 580, file 113).

| # | Change | Saving on w120 (requested) | Crates / files | Effort (eng-days) | Regression risk |
|---|---|---|---|---|---|
| **A** | **Shared-formula templates**: one AST per run of formulas that are identical in R1C1 form; a cell keeps its existing `Option<Arc<Formula>>` pointer to the shared template | 5,185,440 × ~678 B → ~0: **−3,350 MiB (−50%)**. Also roughly halves open time (§1.1: formulas dominate load) | `formula` (relative anchors, or shift on demand; `adjust.rs:9` `shift_relative` and `printer.rs:161` `print_r1c1` exist); `model/cell.rs` (`text`/`expr()` become position-aware: 15 `.expr()` + 3 `expr_arc()` call sites, about 75 formula-text uses); `xlsx/sheet_read.rs:458-510` (keep runs, dedupe non-shared formulas by R1C1 key), `sheet_write.rs` (emit `t="shared"`); `calc/eval.rs` + `recalc.rs` (resolve relative refs against `at`); `engine` fill/copy/insert/delete/sort (split runs on edit) | **10–15** (stage A1, which materialises the absolute AST on access: 6–8) | **High**: structural edits, paste and sort on partial runs; out-of-grid shifts → `#REF!`; INDIRECT and names are unaffected |
| **B1** | **Compact per-formula graph**: `Key` packed into a u64; dense node ids; `One(id)\|Many(arena)` instead of a `HashSet` per precedent cell; areas in a flat arena | 326 → ~110 B/formula (**UNVERIFIED** design estimate, §1.4): **−1,068 MiB** | `calc/recalc.rs:49-253` (Graph, Intervals), `dirty_closure` `:917-949` | **4–6** | Medium: stale-value bugs; upstream's fuzzers and tests help (`fmla.rs:130` mentions a command-sequence fuzzer) |
| B2 | **Run-based graph** (needs A): one node per template run with relative precedents | Graph → ~10 MiB: **−1,600 MiB** | `calc/recalc.rs` (dirty closure over partial runs) | +8–12 after A | High |
| **G** | **Dense recalc pass state**: `Vec` by node id instead of three hash maps sized to all formulas | 117 → ~30 B/formula (**UNVERIFIED**): **−430 MiB** | `calc/recalc.rs:382-420, 951-1000` | **2–3** | Low–medium |
| F | **Trust cached values on open** (Excel's behaviour); skip `recalc_all` in `DocState::new` (`engine/lib.rs:123`) when every formula has a cached value and `fullCalcOnLoad` is unset | Recalc transient at load: **−580 MiB**. Saves much of the open time | `engine/lib.rs:120-140`, `xlsx/read.rs:178-192` (calcPr) | 3–4 | Medium: stale caches from non-Excel producers. openpyxl files still need a full calc |
| **C** | **Streaming read**: zip from the file instead of a `Vec`; decompress sheet parts as a stream; replace the 1 GiB `MAX_TOTAL` wall with a per-part ratio check | File bytes + one sheet buffer: **−140 MiB (−2%)**. Removes the **≈150 MB file-size wall** | `engine/io.rs:404-409`, `xlsx/package.rs:11-87`, `sheet_read.rs:43-50` | **3–5** (raising the cap alone: 0.5) | Low–medium (zip-bomb policy) |
| D | **Lazy sheet loading** | Little or nothing. The graph needs every formula; cross-sheet totals (`Summary`) touch every sheet; `recalc_all` on open reads everything | `xlsx/read.rs:202-242`, `engine` | 8–12 | High. **Not recommended** |
| H | **Compact value cells**: 16-B packed cells, exact per-row `Vec`, string pool | Values 1,022 → ~420 MiB (**UNVERIFIED**): **−600 MiB** | `model/store.rs`, `cell.rs`, `core/value.rs`; about 149 `.formula` and 35 `cells.set/get_mut` call sites | 10–15 | High (the `&Cell` API is everywhere) |
| **W** | **Streaming writer**: zip each part as it is produced; SST keyed by the cells' `Arc<str>`; write straight to the file | Save transient 1,308 → ~250 MiB: **−1.0 GiB at save** | `xlsx/write.rs:86-137, 453-469`, `engine/io.rs` save path | **2–4** | Low |
| **U1** | **Undo hygiene**: dirty flag by revision instead of the `saved` snapshot; memory-budgeted undo (drop the oldest entries past N MiB of copied bands) | Up to **−0.9 GiB per wide edit** (§1.6) | `engine/lib.rs:118, 127, 142-144, 348-359`; `cmd/edit.rs:405`, `cmd/file.rs:125`, `cmd/view.rs:165`, `cmd/extra.rs:679` | **1.5–2.5** | Low |

**Projected w120 working set after each cumulative step** (PROJECTION, ×1.16):

| Step | Requested MiB | Working set |
|---|---|---|
| Baseline | 6,675 | 7.6 GiB |
| + A | 3,326 | 3.8 GiB |
| + A + B1 | 2,258 | 2.6 GiB |
| + A + B1 + G | 1,828 | **2.07 GiB** |
| + A + B1 + F (instead of G at load) + C | 1,567 | **1.78 GiB** |
| + A + B2 + F + C | ≈1,030 | ≈1.17 GiB (Excel 32-bit: 1.30 GiB) |

w80 with A + B1 + G: 1 + 363 + 99 + 682 + 75 = 1,220 MiB → **≈1.38 GiB**.

Save adds about 0.25 GiB with W, and about 1.3 GiB without it. U1 keeps edits from adding
0.9 GiB steps.

**Upstream fit.**
- Large workbooks are upstream's own **beta gate**: "100 MB workbooks open; 1M rows usable;
  multi-threaded recalc", 25–40 agent-hours (`docs/roadmap.md:17, 48`; gap #2, `docs/gaps.md:28-33`).
- Upstream's plan names only a "streaming XLSX reader … part-size limits that degrade instead of
  failing", 10–15 h (`docs/file-format-parity.md:77`).
- Our measurement shows that plan alone saves about 2%. A, B1, G and W are what the gate actually
  needs. They fit the existing architecture:
  - the copy-on-write store (`model/src/lib.rs:1-3`);
  - existing R1C1 printing and relative shifting;
  - no conflicting entry in `docs/decisions.md`, which holds only 4 decisions on unrelated topics.
- Upstream estimates in agent-hours. Ours are in engineer-days for someone new to the code.

**Recommended sequence:**
0. **Census** (0.5–1 d, before committing): a read-only script over the owner's real workbooks.
   It counts cells, formulas, shared runs, unique strings, external links and the uncompressed
   size. The per-unit costs above turn it straight into a RAM forecast per file. Raise
   `MAX_TOTAL` at the same time (0.5 d).
1. **A** (templates): the biggest win, and it speeds up opening.
2. **B1** (compact graph).
3. **W + U1**: so that save and editing stay inside the budget the load reaches.
4. **G**, or **F** if the census shows Excel-saved files with cached values.
5. **C** (streaming zip). This is upstream's literal #175 ask, and it removes the 1 GiB wall.
6. Stretch, only if the census demands it: B2 or H.

Core total (0 + A + B1 + W + U1 + G + C): **≈23–36 engineer-days**.

## 3. External workbook links

### 3.1 Current behaviour, and why (CODE + probe)

- **On load,** `externalLink` parts, their rels and `<externalReferences>` are kept byte for byte
  and never parsed (`crates/xlsx/src/passthrough.rs:152, 412-433`; warning at `:431`). External
  link paths are rebased on Save As (`passthrough.rs:1173`; `engine/io.rs:217-229`).
- **Stored `[1]Sheet1!A1` fails to parse.**
  - The lexer reads `[1]` as a structured-reference body (`crates/formula/src/lexer.rs:173-196`).
    The parser then sees `Sheet("Sheet1")` and rejects it (`parser.rs:277-298`).
  - MEASURED with the probe `--parse`: `unexpected Sheet("Sheet1", None)`.
  - `Formula::new` therefore keeps the text with `expr: None` (`cell.rs:29`). The formula never
    enters the graph (`recalc.rs:801-802`), so its cached `<v>` (`sheet_read.rs:394-455`) stays
    frozen. That is why the cached values "survive".
  - **Hazard (CODE, not run):** a formula that mixes an external and a local reference never
    recalculates when the local cell changes. A formula that is re-entered gets `#NAME?`
    (`recalc.rs:850-855`).
- **Typed `'[source.xlsx]Data'!B2`** parses as a local sheet named `[source.xlsx]Data`. No such
  sheet exists, so it gives `#REF!` (`eval.rs:144`).
- **Typed `'E:\dir\[source.xlsx]Sheet1'!A1`** is split at the drive-letter colon into a 3-D span
  `E` … `\dir\[source.xlsx]Sheet1` (`lexer.rs:144`), which gives `#REF!` (`eval.rs:146`). Both are
  MEASURED with the probe `--parse`.
- **Unquoted `[source.xlsx]Data!B2`** gives a parse error, the same as the stored form.

### 3.2 Design: where each piece plugs in

| Piece | What | Plug-in point | Effort (eng-days) |
|---|---|---|---|
| Syntax | New `Tok::ExtSheet{book, sheet, last}` for `[n]Sheet!` and `[file.xlsx]Sheet!`; inside quoted names, detect `…[file]Sheet` before the `:` split; external names `[1]!Name`; AST `SheetSel::External{book: ExtBook, sheet, last}` with `ExtBook::Index(u32) \| Path(String)`; printer: the file form `[n]` and the display form `'C:\…\[f.xlsx]S'!A1` | `lexer.rs:121-150, 144, 173-196`; `parser.rs:277-287, 355-360`; `ast.rs:34-40`; `printer.rs`; `xlsx/fmla.rs:7-13, 109-118` | 3–4 |
| Model | `Workbook.external_books: Vec<ExternalBook{target, sheet_names, defined_names, cache: per sheet sparse values, status: Cached\|Updated\|Missing\|Denied, last_update}>`; resolve `Path` → index when a formula is entered (like `apply_name_call_case`, `read.rs:344`) | `model/workbook.rs:89-122`; `model/preserved.rs:15-35` (stop treating links as opaque parts) | 1–2 |
| File I/O | Read `externalLink` parts (ECMA-376 Part 1 §18.14: `externalBook`, `sheetNames`, `sheetDataSet`, `definedNames`) into the model; write them back from the model (and the `externalLinkPath` rels); keep output byte-identical when nothing changed | Replace the passthrough at `passthrough.rs:412-433` (and drop `xl/externallinks/` from `OWN_DIRS`, `:152`); new `xlsx/src/external_link.rs`; keep the rebase at `:1173` | 3–4 |
| Calc | Map external sheets to **virtual sheet indices** above the real ones (`Area{sheet: usize}` stays unchanged, `eval.rs:23-26`); `resolve_sheet` (`eval.rs:140-150`) and `precedents` (`eval.rs:1736-1753`) map external refs; `PassHost::cell_value`/`range_values`/`used_range` read from `external_books[i].cache` (`recalc.rs:382+`; `Host` trait `eval.rs:53-82`); INDIRECT into a closed book → `#REF!` (Excel behaviour, **UNVERIFIED**) | `calc/eval.rs`, `calc/recalc.rs`; audit every `wb.sheet(area.sheet)` | 3–5 |
| Source loading | `gridcraft_xlsx::read_values(bytes, wanted: &[(sheet, ranges)])`: values only, from the cached `<v>`, streamed with the existing `parse_streaming`; skips styles, formulas, drawings and pivots; SST read on demand. This matches how Excel reads a closed source (**UNVERIFIED**). For `.xls`/`.xlsb` sources, fall back to the full readers (`xls/mod.rs:131`, `xlsb.rs`). | New fn in `xlsx/src/read.rs` next to `read_xlsx` (`:69`) | 2–3 |
| Recalc | After an update: diff the old and new cache → seed `cells_changed` with the dependents of the changed virtual cells (`recalc.rs:830-915`; `Graph::dependents_of_range` `:321`) | `calc/recalc.rs` | (in Calc) |
| Commands | `data.editLinks` (query: list of links with source, status, sheets), `data.links.updateValues {link?}`, `data.links.changeSource {link, path}`, `data.links.breakLink {link}` (formulas → values, undoable), `data.links.openSource`, `data.links.startupPrompt {mode}`; ribbon dialog through `UiRequest::Dialog("editLinks")` | Register in `engine/src/cmd/mod.rs:123-147` with `cmd!` (`:110-121`); a new `cmd/links.rs`; catalog row `("data.editLinks","Data","Queries & Connections","Edit Links")` beside `catalog.rs:198`; `UiRequest` `engine/lib.rs:222-229` | 2–3 |
| Security | No auto-update on open (status `Cached`, message-bar prompt). A `LinkResolver` trait supplied by the host app does all file access; the engine never touches the filesystem for links. **Consent needed** for UNC `\\server\…`, `file://host/…`, `http(s)` (never fetched, in line with `docs/decisions.md:36`), paths outside the workbook's folder or trusted locations, and device paths `\\?\`, `\\.\` (always refused). Existing package limits apply (`package.rs:11-15`). Encrypted sources ask for a password. CLI/MCP need an explicit `allowExternal` flag. The web build only allows a file picker. | Engine: trait on `Session` (`engine/lib.rs:231+`); app layer: prompt UI and trusted-locations prefs | 2–3 |
| Missing source | Keep the cached values, status `Missing`, list it in Edit Links; never turn values into errors unless the user runs Update and a sheet is gone (`#REF!`, Excel behaviour, **UNVERIFIED**) | Commands + Calc | (in the rows above) |
| Verification | The owner's Excel opens our saved files without a repair prompt; corpus of real linked workbooks | — | 2–3 |

**Total: ≈17–25 engineer-days.**

**Thin layer or upstream?** It must be an **upstream engine change**:
- The lexer, parser, AST, evaluator sheet resolution, graph precedents and the externalLink
  reader/writer have no extension points (citations above).
- `Host` is implemented privately by `PassHost` (`recalc.rs:382`).

Only these can live in our layer:
- the `LinkResolver` implementation (desktop filesystem or document-server storage);
- the consent prompt;
- trusted locations;
- the Edit Links dialog UI.

Upstream does not have link resolution on its roadmap. Gap #3 only keeps links
(`docs/gaps.md:36-41`), and gap #12 is Power Query (`docs/gaps.md:104-106`). We would propose it
as a new issue plus a PR series.

## 4. Strategy verdict

- **Is replacing Excel for this workload realistic in the v1.0 timeframe?** **Conditionally.** It
  requires about 40–60 engineer-days of upstream engine work:
  - memory core: 23–36 d;
  - external links: 17–25 d;
  - review and upstream latency on top.

  On the projections, the 80 MB files then fit in about 1.4 GiB and the 120 MB files in about
  1.8–2.1 GiB. It is **not** reachable as a thin layer. That conflicts with the BRIEF's "minimal
  upstream patches" constraint (`docs/BRIEF.md` §5), so it only works if upstream accepts the
  changes. Large files are already their beta gate.
- **What we would upstream:**
  - A (formula templates), B1, G/F, W, U1, C, and the `MAX_TOTAL` fix;
  - the external-link syntax, model, I/O, calc and commands;
  - our measurements, to redirect #175 from "streaming" to formula and graph representation.
- **Fallback:**
  - Keep Excel for the heaviest and linked workbooks during migration. gridcraft 0.7.0 already
    handles up to about 40 MB with no links.
  - Gate the cutover per file on the census: size, formula count and links.
  - Splitting workbooks (for example, archiving closed periods into separate files) cuts RAM, but
    it makes the workbooks depend *more* on links. It is only useful after links land, or when
    the archive is plain data that is imported instead of linked.
- **Single biggest risk:** the formula-template change (A). It is half of the memory saving and
  the precondition for every target. It touches the formula representation used by the
  evaluator, structural edits and the writer, which carries a high regression risk. Its timing
  depends on upstream accepting an engine refactor that differs from their own #175 plan.
