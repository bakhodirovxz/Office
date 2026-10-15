# gridcraft: shared formula templates spike (Phase 0)

Date: 2026-10-15/16. Upstream: `upstream/gridcraft` at e6b6b34 (0.7.0 + one CI commit). Spike
branch `spike/formula-templates` in the worktree `E:/office-work/gridcraft-spike`; patches in
`spikes/gridcraft-patches/`. Source paths are relative to the gridcraft repo unless they start with
`spikes/`. Line numbers are at the spike's HEAD.

Question: how much RAM does a shared formula template representation actually save, and can it be
done without breaking calc correctness? This spike answers with measurements. It implements step 1
(templates) of `gridcraft-scaling-and-links.md` §2 and the template-based variant of step 2 (B2,
"run-based graph").

**Evidence types:** MEASURED (a run on this PC), CODE (read in the source), UNVERIFIED (estimate).

## 0. Answer in short

**Peak RAM at open: down 69–70%.** The patched build needs **19–20 MB per file-MB** at open, where
the unpatched build needs **64** (MEASURED).

| File | Before | After |
|---|---|---|
| w10 | 635 MB | 194 MB |
| w40 | 2,525 MB | 751 MB |
| w80 | aborted above 4 GB | **1,496 MB**, opens in 80 s |
| w120 | aborted above 4 GB | **2,421 MB** |

On the 80 MB and 120 MB files, the 0.7.0 build was aborted by the RAM guard on this machine.

- **Step 1 alone (templates):** 635 → 352 MB on w10 (−45%).
- **Step 2 (one graph entry per run):** 352 → 194 MB.

**Calc correctness is preserved, bit for bit, on everything compared:**
- every cached value of every cell: w10 (432,120 formulas), c10 (270,120, cached-value file) and
  w40 (1,728,120);
- every formula text after the unpatched build re-saves our output;
- every row formula of w80 and w120 against values recomputed in Python (3.46M and 5.18M);
- the Summary sheets;
- a 35-step script of edits on w10 (value edits feeding runs, a Lookup edit feeding 144,000
  VLOOKUPs, insert/delete rows and columns, delete cells, sort, fill, cut/paste, rename, undo/redo);
- Excel itself: the same formula and recalculated value in every cell.

**Tests:**
- All existing tests of formula, model, calc, xlsx and engine pass, plus 25 new ones.
- The new ones include a differential test that runs every structural edit on a template workbook
  and on its per-formula twin.
- ui-egui (touched only mechanically) passes 675 tests in 48 suites.

**Bugs found on the way:**
- **One of mine, caught by the tests and fixed.** Shared-formula groups also need the same origin,
  not only the same tree. Without that, a delete-cells shift silently changed formulas on save.
- **Excel compatibility.** Excel refuses to open a file whose shared formulas contain
  sheet-qualified references. The writer now leaves those per-cell.
  - The colleague's `w10_shared.xlsx` variant has exactly this shape, so it is not a file Excel
    would write or open.

**Open time** improves only modestly: 11.3 → 8.7 s on w10 and 42.7 → 36.6 s on w40 (minimum
times; the machine was shared, see §5). The saving is memory, not speed.

**Verdict.** The design is correct within the tested scope and much better than projected. The
earlier projection was −50% for templates (A) plus −25% for a compact graph (B1) → about 2.6 GiB
at w120; measured is 2.4 GB at w120 with A + B2. The remaining work to production is about
**5–9 engineer-days**, against the plan's 14–27 for steps 1–2 (§8).

## 1. Design

### 1.1 Representation: a formula is its own tree, or a shifted copy of a shared one (CODE)

`crates/model/src/cell.rs:25-38`:

```rust
pub struct Formula { repr: Repr, pub array: Option<RangeRef> }
enum Repr {
    Own { text: String, expr: Option<Arc<Expr>> },     // as before
    Shifted { base: Arc<Expr>, dr: i32, dc: i32 },      // base moved by (dr, dc), as copy/fill moves it
}
```

- A shifted copy is defined as `shift_relative(base, dr, dc)`. Its text and tree are built on demand:
  `text()` returns a `Cow` (`cell.rs:116`), `expr()`/`expr_arc()` build the tree (`cell.rs:131-147`).
- **The offset is from the shared tree, not from the cell.** A formula therefore means the same
  wherever its cell is stored. Moving cells (sort, insert/delete, cut, undo) needs no change, and any
  edit or rewrite that builds a new formula detaches that one cell: copy-on-write comes for free.
- `Cell` is unchanged (`Option<Arc<Formula>>`, 40 B). Each formula cell keeps a small
  `Arc<Formula>` of 56 B plus a 16-B header (CODE, computed). The shared tree is one per run.
- `PartialEq` compares by meaning, so a shifted copy equals the same formula held on its own
  (`cell.rs:59-72`). Serde keeps the `{text, array}` form (`cell.rs:24, 41-57`).
- New API:
  - `Formula::shifted` (`cell.rs:92`);
  - `Formula::copied(dr, dc)`, which is exactly `expr().map(|e| from_expr(shift_relative(e, dr, dc)))`
    but shares the tree; copies of copies compose only when the first move kept every reference on
    the sheet (`cell.rs:103-114`);
  - `Formula::template()`, which returns the shared tree and offset (`cell.rs:149`).
- `FormulaRuns` (`cell.rs:170-221`) finds runs while cells are visited in row-major order. It
  compares each formula with the last template of its column and of its row, using
  `is_shifted_copy`.

**Alternative rejected:** a tree in relative (R1C1) form plus the cell's position, with every cell
of a run pointing to one `Arc<Formula>`. That saves the remaining ~72 B per cell. But it needs the
position at every place that reads a formula's text or tree: about 110 call sites in 6 crates,
including `Cell::input_text()`, which has no position. The offset-from-tree form keeps all of
those unchanged and is the narrowest seam.

### 1.2 Formula primitives (CODE)

`crates/formula/src/adjust.rs` adds three functions next to `shift_relative`. Each is defined to
agree with it exactly.

- `shifted(e, dr, dc)` (`:78`) builds the moved tree in one pass. The left spine of operator chains
  is walked iteratively, as the printer does.
- `is_shifted_copy(base, dr, dc, other)` (`:114`) answers "is `other` == `shift_relative(base)`?".
  It walks both trees with an explicit stack and allocates nothing. Two cells hold the same formula
  in R1C1 terms exactly when it returns true; a reference moved off the sheet counts as `#REF!`.
- `shift_stays_on_sheet` (`:169`).

### 1.3 Where runs come from (CODE)

| Source | Where | How |
|---|---|---|
| Excel `<f t="shared">` | `xlsx/src/sheet_read.rs:464-490` | Followers become `Formula::shifted(master tree, dr, dc)`. Before, each follower got its own cloned, shifted and re-printed tree (`sheet_read.rs:462-480` at e6b6b34). |
| Ordinary formulas identical in R1C1 terms | `sheet_read.rs:523-531` → `FormulaRuns` | The cell's text is parsed once. If the tree `is_shifted_copy` of the column's or row's last template, the cell shares that tree and nothing is printed. Runs are never assumed from text. |
| Fill, Range Fill, sort, paste | `engine/src/fill.rs:99`, `cmd/edit.rs:349, 1006`, `cmd/data.rs:307` | `Formula::copied`. Cut-and-paste on the same sheet keeps the formula as it is (`cmd/edit.rs:999`). |
| Structural rewrites: insert/delete rows, columns and cells; cut; rename/delete sheet; rename table | `cmd/edit.rs:1196`, `cmd/sheet.rs:732, 785`, `cmd/insert.rs:498` | Rewritten formulas go through `FormulaRuns`, unchanged ones are `keep`-ed. A run whose references all moved the same way keeps one tree. Delete Sheet and Rename Table no longer rewrite formulas they leave unchanged. |

### 1.4 Evaluation seam (CODE)

- **The evaluator is unchanged.** It still receives an ordinary `&Expr`.
- The only calc change needed for step 1 is in the recalculation pass. The pass held
  `HashMap<Key, Arc<Expr>>` of every formula it evaluates (`calc/src/recalc.rs:951-962` at
  e6b6b34). For shifted copies that would have rebuilt every tree at once and given the saving back
  at the peak.
- It now holds `PassExpr::{Tree, Shifted(Arc<Formula>)}` (`recalc.rs:665-690`) and builds a shifted
  copy's tree only while that formula is evaluated. The tree is dropped right after.

### 1.5 Dependency graph: one entry per run (step 3 stretch, CODE)

`crates/calc/src/recalc.rs:189-642`:

- **A `Run`** is a set of consecutive formulas down one column that are shifted copies of one tree
  at the same offset, i.e. the same origin = cell − offset. It stores precedent areas as `RunArea`s:
  each end is a constant row or the formula's row plus a constant, and the columns are fixed.
- **Indexing.** Run areas go into interval indexes by the rows the whole run reads. `dependents` and
  `dependents_of_range` turn a hit into the exact rows of the run that read the cell
  (`RunArea::readers`, `run_readers` `:487`). Dirty marking and recalc therefore see exactly the
  formula keys they saw before.
- **Run creation.** `Calc::build` (`:1110`) forms runs on every full rebuild (open, structural edit,
  F9). `run_areas` (`:607`) accepts a run only when `precedents()` of its first, last and middle
  formulas give exactly the interpolated areas. It rejects runs with dynamic references
  (INDIRECT/OFFSET/names), an area that turns over inside the run, or a reference off the sheet.
  Rejected runs are stored as per-formula nodes, as before.
- **Edits.** Editing a formula of a run splits the run (`split_out` `:371`). New formulas from edits
  are nodes until the next full rebuild.

### 1.6 Save (CODE)

- `xlsx/src/sheet_write.rs:77-166` writes as one Excel shared formula each set of consecutive cells
  down a column with the same tree and origin, where each cell's move stays on the sheet. The first
  cell carries the text and `ref`, the others only `si` (`:786-791`).
- Array formulas (legacy, spilling or needing array evaluation), table headers and cell pictures are
  written on their own.
- So are formulas Excel cannot read as shared (`excel_can_share`, `:158`): any sheet-qualified
  reference, name or structured reference (§4).
- Everything else is written per cell as before.
- The writer no longer prints a formula per cell for shared runs; members are written as
  `<f t="shared" si="n"/>`.

## 2. Diffstat

Seven code commits on e6b6b34, plus one docs commit (`docs/decisions.md` entry).

- Patches: `spikes/gridcraft-patches/0001…0008-*.patch` (`git format-patch`, upstream-style
  messages, no Co-Authored-By).
- Verified: `git am` onto e6b6b34 in a throwaway clone gives the branch's exact tree
  (98e2f2b2…).
- Every commit passes `cargo check --workspace --all-targets` with 0 warnings.
- Commit 6 (everything but the run graph) also passes the full test suites of
  formula/model/calc/xlsx/engine (621 tests).

| # | Commit | Files | +/− |
|---|---|---|---|
| 1 | formula: build and recognise shifted copies without rebuilding the tree | 2 | +173 −5 |
| 2 | model: formulas can share one tree (shifted copies) (incl. the mechanical `text` → `text()` sweep: calc, engine, ui-egui, xlsx, tests) | 27 | +380 −59 |
| 3 | xlsx: read shared formulas and filled-down runs as shared trees | 3 | +89 −7 |
| 4 | calc: keep shifted copies unbuilt during a recalculation pass | 1 | +37 −8 |
| 5 | engine: keep shared trees through fill, sort, paste and structural edits | 6 | +324 −27 |
| 6 | xlsx: write runs of shared trees as shared formulas | 2 | +174 −18 |
| 7 | calc: one dependency-graph entry per run of shared formulas | 1 | +465 −11 |
| 8 | docs: record the shared formula trees decision | 1 | +29 −1 |
| | **Total** | **39** | **+1,667 −132** |

By kind:
- production code: +865 −106;
- tests: +773 −25 (test files and inline `#[cfg(test)]` modules);
- docs: +29.

Net production lines by file:

| File | Net production lines | Content |
|---|---|---|
| `calc/recalc.rs` | +337 (1,116 → 1,453) | graph runs and the pass |
| `model/cell.rs` | +180 (96 → 276) | |
| `formula/adjust.rs` | +108 | |
| `xlsx/sheet_write.rs` | +100 | |
| `xlsx/sheet_read.rs` | +20 | |
| engine commands | +14 (+52 −38) | |

## 3. Tests

| Crate / suite | Before (e6b6b34) | After | New tests |
|---|---|---|---|
| formula (unit + `spanish_input`) | 23 + 5 | 25 + 5 pass | `shifted_matches_shift_relative`, `shifted_copies_are_r1c1_equal` (every pair of 9 formula kinds × 5 cells × 9 offsets, incl. off-sheet) |
| model | 14 | 22 pass | 8 in `cell.rs`: copy == plain copy (6 formulas × 7 offsets), composition and sticky `#REF!`, tree sharing, unparsed, array range, serde, `FormulaRuns` runs/breaks/off-sheet, `keep` |
| calc | 59 + 1 ignored | 61 + 1 ignored pass | `runs_in_the_graph_answer_like_one_node_per_formula` (every cell's dependents, 600 random ranges, precedents, keys, before and after 5 edits that split runs, then values vs full recalc); `edits_reach_the_formulas_of_runs` |
| xlsx | 172 | 175 pass | `tests/shared_formulas.rs`: Excel shared groups share the master's tree; filled-down runs share, different formulas never do; writer groups (a moved copy with another origin is not grouped; off-sheet copy, legacy array and sheet-qualified formula are written per cell), round trip |
| engine lib | 286 | 286 pass | — |
| engine integration | 39 (13 files) | 49 pass | `tests/formula_templates.rs`: 10 twin tests (template workbook vs every-formula-own copy, compared cell by cell after every command and after save/reopen): read, edit/detach, insert/delete rows+cols (+undo/redo), insert/delete cells (4 directions), sort (2), fill down/right/up + Range Fill, copy/cut/paste (same sheet, other sheet, onto a run), rename/delete sheet, copy off the sheet |
| engine `command_sequence_fuzz` (release, per seed) | — | seeds 0–199 × 40 steps and 0–99 × 100 steps: every outcome identical to base. 296 pass; seed 138/40 fails an undo invariant ("other sheet properties differ") and seed 79/100 a round-trip invariant (`#SPILL!` cached as `#VALUE!`) **in base too**; seeds 33/100 and 51/100 exceed the 2.5 GB cap in both builds | upstream's fuzzer: undo/redo, XLSX round trip and full-recalc invariants |
| ui-egui | — | 675 pass (48 suites) | — (mechanical `text()` changes only) |
| `cargo check --workspace --all-targets` | — | clean, 0 warnings | — |
| rustfmt / clippy (`cargo xtask ci`) | — | **not run**: components not installed | — |

## 4. Correctness comparison (MEASURED)

All comparisons are between the unpatched build (`base`, built from a pristine worktree at
e6b6b34) and the patched builds, on the same machine. They use `spikes/xlsx-bench/compare_values.py`,
which compares the cached `<v>` of every cell. gridcraft writes numbers with Rust's shortest
round-trip format, so equal text means bit-identical doubles. Shared strings are compared by text.

| Check | Cells compared | Result |
|---|---|---|
| w10 `convert`: every cell's cached value, base vs patched | 1,296,467 cells, 432,120 formulas | identical |
| c10 (xlsxwriter, cached values, `fullCalcOnLoad`) | 810,467 / 270,120 | identical |
| w10_shared (Excel-style shared runs) vs base w10 | 1,296,467 / 432,120 | identical |
| w40 `convert` | 5,184,467 / 1,728,120 | identical |
| Formula text: base build re-saves the patched output, compared with base's own save | 432,120 formulas | identical; the re-saved file has the same size in bytes as base's own save |
| Summary vs Python expectations (`expect_summary.py`, seed-42 replay): SUM, SUMIFS, XLOOKUP, COUNTIF | 120 per file: w10, c10, w40, w80, w120 | all match (rel. 1e-9 for sums; equal otherwise) |
| Every row formula vs Python (`expect_rows.py`; the "Link" column checked as the formula reads, `amt+qty`, because of the known generator bug) | w10 432,000 · w40 1,728,000 · w80 3,456,000 · w120 5,184,000 | all exactly equal |
| Edit script on w10 (35 commands, `spikes/xlsx-bench/tpl_edits.jsonl`, changes 135,728 cells), base vs patched | 1,296,737 / 432,400 values; 432,400 formulas after base re-save | identical |
| **Excel 16.0** (COM, `excel_compare.ps1`): our file vs base's file, `Range.Formula` and `Value2` after `CalculateFull` | w10 1,296,467 cells / 432,120 formulas; edit-script result 1,321,372 / 432,400 | identical, no open errors |

Excel compatibility (MEASURED, bisected with `mkvariant.py`):
- Excel 16.0 refuses to open a workbook (`Workbooks.Open` fails) when a shared formula holds a
  sheet-qualified reference. Failing examples: `VLOOKUP(F2,Lookup!$A$2:$B$21,…)`, and `S03!C2+D2`
  even on S03 itself.
- Shared `IF(C2>500,C2*D2,0)` over 4,800 rows and `C2+D2` open fine.
- The writer therefore shares only formulas without sheet-qualified references, names or
  structured references (`excel_can_share`, `sheet_write.rs:158`).
- **The colleague's `var/w10_shared.xlsx` (built by `mkshared.py`) also fails to open in Excel.**
  Its memory result still shows how gridcraft reads shared groups, but Excel never writes formulas
  with sheet references as shared.

## 5. RAM and time, before → after (MEASURED)

`spikes/xlsx-bench/measure_tpl.py`:
- It is the same method as `measure.py`: peak working set from psutil, with a kill when the
  system has under 1 GB free.
- Results are in `E:/office-target/gridcraft-spike/results.jsonl`. `tpl_table.py` computes the
  medians.

Builds, all release, from the same tree:
- `base`: e6b6b34;
- `tpl1`: templates (commits 1–5, with an early writer);
- `tpl2`: plus the run graph;
- `tpl3`: the final series (writer with the origin and Excel fixes).

The writer does not run on open, and tpl2 and tpl3 differ only in `sheet_write.rs`, so their open
numbers are the same.

**Peak working set is deterministic** (identical to the MB across 3 repeats). **Wall time is not.**
Another agent's cargo builds and an Android emulator shared the machine; the `others` field of
each result records it.
- I give the median and minimum of interleaved repeats (base, tpl1, tpl2 in turn).
- Base w40 was also measured once on a quiet machine: 42.7 s (`base_info_w40`), consistent with
  0.7.0's 44.9 s.

**Open (`gridcraft-cli info`: read + full recalc):**

| File (MB on disk) | Formulas | base peak MB | tpl1 (templates) | tpl2 (+ run graph) | base → tpl2 | base s (min) | tpl2 s (min) |
|---|---|---|---|---|---|---|---|
| w10 (9.9) | 432,120 | 635 | 352 | **194** | −69% | 11.3 | 8.7 |
| w10_shared (7.4) | 432,120 | 619 | 350 | **191** | −69% | 7.4 | 4.8 |
| c10 (7.2), cached values | 270,120 | 431 | 256 | **140** | −68% | 6.5 | 5.4 |
| w40 (39.6) | 1,728,120 | 2,525 | 1,390 | **751** | −70% | 42.7 (quiet) / 73.8 | 36.6 |
| w80 (79.2) | 3,456,120 | aborted at 3,993 after 62 s (0.7.0, `xlsx-bench.md`) | 2,771 | **1,496** | opens | — | 80.4 |
| w120 (118.8) | 5,184,120 | aborted at 4,011 after 69 s (0.7.0) | — | **2,421** | opens | — | 182.4 (rustc running) |

**MB of peak RAM per MB of file:**

| File | base | tpl1 | tpl2 |
|---|---|---|---|
| w10 | 64.1 | 35.5 | 19.6 |
| w40 | 63.8 | 35.1 | 19.0 |
| w80 | — | 35.0 | 18.9 |
| w120 | — | — | 20.4 |

Per formula: (base − tpl2) / formulas = 1,070–1,076 B less working set (w10, w40); templates alone save 687 B, the run graph another 383 B (w10). "MB" in these tables is MiB (2^20 bytes), as `measure.py` reports.

**Save (`convert` xlsx → xlsx: read + recalc + write):**

| File | base peak / s | tpl2 peak / s | tpl3 peak / s |
|---|---|---|---|
| w10 | 677 MB / 17.3 | 224 MB / 14.4 | 247 MB / 12.4 |
| w40 | 2,681 MB / 61.4 | 868 MB / 46.8 | 922 MB / 54.2 (quiet) |
| w80 | aborted (0.7.0) | 1,726 MB / 131.3 | 1,833 MB / 102.5 (quiet) |
| w120 | aborted (0.7.0) | 2,585 MB / 198.8 | 2,635 MB / 157.2 (quiet) |

- Output size for w10: base 11.4 MB; tpl3 10.2 MB (only column G and S01's I are shareable for
  Excel). tpl2 wrote 8.8 MB, but that is not Excel-valid.
- Edit script on w10 (open + 35 commands + save): base 1,361 MB / 82.7 s → tpl3 871 MB / 44.7 s.
  The undo history (up to 100 snapshots) and the copy-on-write band copies of the edits dominate
  that peak.

**Where the remaining memory is** (tpl2, w10, 194 MB working set; CODE + arithmetic from the
colleague's probe, UNVERIFIED as a breakdown):
- non-formula cells and containers: ≈85 MiB;
- recalc pass state: ≈48 MiB (transient at open);
- `Arc<Formula>` per formula cell: ≈432k × 72 B ≈ 30 MiB;
- XML and file buffers: ≈10 MiB.

The graph and the trees are now negligible. By CODE reasoning, not counted: 30 sheets × 3 formula columns give 90 runs on w10, and the 120 Summary formulas stay nodes.

## 6. What remains for production quality

Ordered by value for the owner's workload. "d" = engineer-days for someone new to the code.

| # | Item | Why | Effort |
|---|---|---|---|
| 1 | **Run `cargo xtask ci`**: rustfmt, clippy `-D warnings`, `xtask layers`, `xtask wasm` | Not possible here: the `rustfmt` and `clippy` components are not installed and I did not change the shared toolchain. The code was kept to the house style by hand (150 columns, no `unwrap`/`expect`/`panic` outside tests, saturating arithmetic). `cargo check --workspace --all-targets` is clean with 0 warnings. | 0.5 |
| 2 | **Excel round trip of the shared-formula output** at scale | Done for w10 and for the edit-script result (§4, Excel 16.0 via COM): no open errors, identical formulas and values. Not done: the larger files, an interactive check for a repair prompt (COM opens with alerts off), LibreOffice. The sheet-qualified-reference rule was found by bisection; other rules Excel may apply to shared formulas (e.g. to names) are untested, which is why names are kept out | 0.5 |
| 3 | **Merge new formulas into runs** in the graph between full rebuilds | Fill-down or paste of N formulas adds N per-formula graph nodes (326 B each) until the next structural edit or F9. Extend the run above or below in `Graph::insert`. The model side already shares trees. | 1–1.5 |
| 4 | **Faster open: parse once** | Open time improves only 20–30% while memory drops 3×. With `GRIDCRAFT_PROFILE=1` on w10, evaluating all 432,120 formulas takes 1.82 s at baseline and 1.60 s patched (MEASURED). Building each shifted copy's tree during evaluation therefore costs nothing measurable, and the evaluator needs no offset support. Most of the remaining ~7 s of open is reading: XML, then `fmla::from_file` parses **and prints** every formula, then the reader parses the printed text again. Parsing once needs proof that `parse(print(t)) == t` for every tree (e.g. number literals printed with 15 significant digits); it was kept as is for exact parity. | 1–2 |
| 5 | (Not needed now) evaluator offsets | Evaluating a shared tree with an offset in `reference_areas` (`calc/src/eval.rs:152`) instead of building the copy. Measured above as no gain. It would also need care so that names, INDIRECT and name-defined LAMBDAs see offset 0. | — |
| 6 | **Recalc pass state** (`exprs`/`results`/`pending` hash maps, ~117 B per formula, transient at open) | Now the largest per-formula cost at the open peak. Option G (dense vectors) or F (trust cached values) from `gridcraft-scaling-and-links.md` §2. | 2–3 |
| 7 | Shrink the per-cell `Arc<Formula>` (72 B → ~40 B by boxing the `Own` variant and moving `array` into it) | ~14 MB on w10, ~170 MB on w120 (CODE arithmetic). `array` is a public field read in ~30 places. | 1 |
| 8 | Other readers (`xls`, `xlsb`, `ods`) and horizontal runs in the writer | Use `FormulaRuns` in the other readers. Write fill-right runs as shared formulas too (only vertical runs are written as shared now). | 0.5 |
| 9 | Hardening | `Graph` run ids are `u32` and wrap after 4·10⁹ runs (unreachable in practice, but not proven); `PartialEq` between an own formula and a shifted copy prints both; `Formula::text()` returning `Cow` is an API change for downstream users of the model crate. | 0.5 |
| 10 | **Soak** | The fuzzer was run per seed: 200 seeds × 40 steps and 100 × 100 (§3); every outcome is the same as the unpatched build's. Some seeds exceed 2.5 GB in **both** builds at 100–150 steps, so upstream's suggested 200×200 soak does not fit an 11.8 GB machine shared with other agents. Run it on a bigger machine. The two pre-existing fuzz failures (seeds 138/40 and 79/100) are upstream bugs worth reporting. | 0.5 |


## 7. Upstreamability

**Fit with upstream's architecture: good.**
- Layering is respected (`cargo xtask layers` rules, CODE):
  - the new primitives are in `formula` (L1);
  - the representation is in `model` (L2);
  - reader, writer and graph are in `xlsx`/`calc` (L2);
  - engine commands only call `Formula::copied` and `FormulaRuns`.
- No new dependency, no `unsafe`, nothing wasm-specific.
- It builds on upstream's own copy-on-write design. `docs/architecture.md` "Edits change the model
  through copy-on-write bands" now holds for formula trees too.
- No conflict with `docs/decisions.md`. The spike adds a decision entry.

**Reviewability: reasonable.** The series is seven commits, each about one concern (commit
messages in `spikes/gridcraft-patches/`):
1. `formula:` primitives with tests.
2. `model:` representation, plus a mechanical `text` → `text()` sweep of 24 files (+380/−59).
3. `xlsx:` reader.
4. `calc:` pass.
5. `engine:` copies and rewrites, with the differential test.
6. `xlsx:` writer.
7. `calc:` run graph (+465/−11, the largest and riskiest).

Commits 1–6 can land without 7.

**Upstream's own plan.** Upstream frames large files as #175, a "streaming XLSX reader"
(`docs/architecture.md:64`, `docs/file-format-parity.md:77`). This spike shows that the formula
representation and the graph are where the memory goes, and that fixing them is a contained
change.
- The PR should lead with the measurements (§5) and the correctness method (§4), and offer
  commit 7 separately.
- Upstream works agent-first with one task id per commit (`CLAUDE.md`). The messages would need
  their milestone ids added.

**Risks a reviewer will raise:**
- (a) Every consumer of `Formula` now pays a print to read a shifted copy's text. That happens in
  UI display, the formula bar, FORMULATEXT, save, and structural rewrites; it is cheap per cell but
  is not free.
- (b) The run graph's correctness depends on `run_areas` accepting only linear runs. It is checked
  at three formulas and by the differential tests, but it is the part to soak longest.
- (c) `copied` composes moves only when the first move kept every reference on the sheet. This
  mirrors `shift_relative`'s sticky `#REF!` and is tested.


## 8. Updated estimate for steps 1–2

The plan's steps were: step 1 (templates, "A") 10–15 d, then step 2 (compact graph: B1 4–6 d,
or the run-based B2 at +8–12 d after A). The spike covers A and a working B2 for vertical runs. It
is measured and tested, but not production-hardened.

| Step | Plan estimate | Spike status | Remaining to production (this spike's §6) |
|---|---|---|---|
| 1. Templates (A) | 10–15 d | Done: all sources of runs, structural edits, save, tests, measured | 3–5 d: items 1, 2, 7, 8, 9 of §6, review |
| 2. Graph | B1 4–6 d / B2 +8–12 d | B2 done for vertical runs | 2–4 d: items 3 and 10, longer soak, review |
| Optional: faster open | — | — | 1–2 d: item 4 (parse once) |
| **Total steps 1–2** | **14–27 d** | | **5–9 d** (+1–2 d for a faster open), plus upstream review latency |

**The revised memory forecast is now MEASURED, not projected:**
- w10: 194 MB peak at open, 19.6 MB per file-MB;
- w40: 751 MB, 19.0 MB per file-MB;
- w80: 1,496 MB, 18.9 MB per file-MB;
- w120: 2,421 MB, 20.4 MB per file-MB.

That is already under the "1.5–2 GB" target for 80 MB files without steps G, F or C. It replaces
the earlier projection of ≈2.6 GiB for w120 after A + B1.


## 9. Reproducing

**Spike scripts in `spikes/xlsx-bench/`:**

| Script | What it does |
|---|---|
| `measure_tpl.py` | Measurement wrapper (as `measure.py`; records concurrent heavy processes) |
| `tpl_table.py` | Medians table from the results |
| `compare_values.py` | Every cached value, optionally every formula text |
| `expect_summary.py` | Summary checked against the seed-42 replay of `gen.py` |
| `expect_rows.py` | Every G/H/I value checked against the replay |
| `tpl_edits.jsonl` | The 35-command edit script (`gridcraft-cli run --in w10.xlsx --script … --out …`) |
| `excel_compare.ps1` | Excel COM: formulas and values of two files |
| `mkvariant.py` | Bisect variants for Excel |
| `fuzz_seeds.py` | Upstream's fuzzer per seed, with a memory cap |

**Builds and data:**
- Wrapper scripts are in `E:/office-target/gridcraft-spike/`:
  - `build.sh` waits for no other rustc and enough free RAM, then runs cargo with `-j 4`;
  - `measure.sh`;
  - `final_runs.sh`.
- Binaries are in `E:/office-target/gridcraft-spike/bin/{base,tpl1,tpl2,tpl3}/`. The baseline was
  built from a pristine detached worktree of e6b6b34 (removed afterwards; recreate it with `git worktree add --detach <dir> e6b6b34`) in a separate target directory
  (`E:/office-target/gridcraft-spike/base`).
  - **Why separate:** cargo hashes path crates relative to the workspace root, so two worktrees of
    one repo share artifact slots in one target directory.
  - A first baseline build was discarded for that reason: it had reused crates compiled from the
    spike worktree.
- Outputs are in `E:/office-target/gridcraft-spike/{out,scen,excel}/`.
