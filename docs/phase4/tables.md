# Docs: tables made in Word (owner finding, usability test 2026-10-29)

Owner finding: "tables made in Word lose their layout when opened in Hoshiya Docs". No file was given.
Branch `p4-tables` (from `phase-4` 74a2c1f). Patches wordcraft/0130 and wordcraft/0131.

## 1. Method, and what could not be used

- **`corpus/` does not exist** on this PC or in any worktree (O9 is still open, STATUS). So nothing here is ranked by
  "how many corpus documents": the ranking is by measured damage on Word-made files, and the counts are synthetic.
- **Truth = Word 16 itself.** `scripts/fidelity/gen_word_tables.ps1` makes Word build 29 table documents through COM (one per
  table feature class: grids, AutoFit modes, merges, styles, borders, shading, row heights, alignment, nested, header repeat,
  floating, cell margins and spacing, percentages, irregular rows, a timesheet, a form card, compatibility modes 11/14/15, tables
  wider than the page), save them as .docx and export Word's own PDF. Those files stay on E: (Word output is never committed).
- **Measure:** `scripts/fidelity/tables.py`. Ours = `docs-cli convert x.docx x.pdf`; both PDFs rasterised by `pdf-cli` at 96 dpi.
  Besides the page count and pixel diff it finds the table **rules** (long dark runs) on each page: for every vertical (column)
  and horizontal (row) rule of Word's page, the distance to our nearest rule; farther than 3 pt = "missed". Round trip:
  `docs-cli convert x.docx out.docx`, the table markup (`tblGrid`/`gridCol`, `tcW`, `tblW`, borders, spans, merges, heights,
  cell margins and spacing, ...) of source and saved file is counted (`fid/tablexml.py`), and Word re-exports the saved file to
  PDF (compared with Word's PDF of the source).
- **Not measured:** `.doc` and `.rtf` tables. Word's `SaveAs2` to those formats hung a hidden Word on this PC every time
  (read-only or not, new or opened documents); Docker (LibreOffice) was not running. Old binary `.doc` files are common in HRM
  work, so this is the largest unknown (section 5).

## 2. Failure classes, ranked (before the fix)

Rules = table rule positions against Word (missed of all, mean distance). 29 Word-made files: **35 of 237 column rules and 18 of
257 row rules missed before, 2 and 9 after** (the 2 column "rules" are a letter on t07 and the cell-spacing table t16; the 9 row
rules are all t16). Mean pixel diff over the 29 first pages 1.10 % before, 0.73 % after. Per file:
`docs/phase4/evidence/tables/word-made-before-after.json`.

| # | Class | Markup (as Word writes it) | Files | Before | After |
|---|---|---|---|---|---|
| 1 | **Tables in compatibility mode** (Word 2010 and older, every .doc opened and saved as .docx in Compatibility Mode) with AutoFit to contents / window or percentage widths | `compatibilityMode` 14 or lower; `w:tblW` auto or `pct`, `w:tcW` auto or `pct` | t20, t25, t27 | 8 of 14 column rules missed, the table 10.8 pt narrower than Word's and every column shifted (mean 5.3 pt, max 10.9) | 0 of 14 (mean 0.27 pt) |
| 2 | **Cells spanning a narrow grid column**: merged headers over narrow columns, rows with their own cell boundaries (forms, personnel cards, timesheets) | `w:gridSpan` over a grid where rows have different boundaries | t18, t22, t23 | 21 of 64 column rules missed (mean up to 6.8 pt), 3 row rules (rows re-wrapped) | 0 of 64 (mean 0.17-0.22 pt) |
| 3 | **Columns sized wider than the text column** (a landscape table on a portrait page): squeezed into the column, text re-wrapped, rows taller | `w:tblW` in dxa past 1.5 times the column | t28 | 4 of 5 column and 4 of 5 row rules missed (mean 18.9 / 10.7 pt), pixel diff 3.3 % | 0 of 5 and 0 of 5 |
| 4 | **Cell spacing** ("Allow spacing between cells", tables pasted from web pages) | `w:tblCellSpacing` in `tblPr` and every `trPr` | t16 | not laid out (9 of 15 row rules missed, mean 5.3 pt) **and lost on save** (Word re-render of our saved file 1.99 % pixels off) | kept on save (Word re-render 0.00 %); layout still missing |
| 5 | **Row exception cell margins with top and bottom only** (tables without a style, older documents, borderless signature blocks): read as left = right = 0, so a save turned the cell margins off | `w:tblPrEx/w:tblCellMar` with `w:top`, `w:bottom` only | t09 | our render right, but Word's re-render of our saved file 0.28 % off (every cell's text against its cell edge, 5.4 pt) | Word's re-render 0.00 % |
| 6 | Automatic table width with cell widths past the column | `w:tblW` auto, `w:tcW` dxa summing to 1.2-1.6 times the column (not what Word writes itself) | fixtures `wide_auto120/160` | Word fits the table into the column with its own split | unchanged (known gap) |
| 7 | **Fonts in the web build** (not table code): Times New Roman has no metric-compatible face in the web editor (no system fonts, `CRAFT_FONTS_DIR` unset for docs), so Source Serif 4 is used | any | t01, t05, t14, t22, t23 rendered with no system fonts | columns stay right; every row rule moves (mean 5.6-15.8 pt), the 60-row table goes 4 to 5 pages | unchanged; owner decision (section 4) |

Everything else matched Word before and after (0 rule misses, pixel diff 0.2-2.2 %, mostly glyph anti-aliasing): Insert Table
defaults, AutoFit to contents and window (mode 15), hand-set widths, two-level merged headers (`gridSpan` + `vMerge`), vertical
merges in the body, table styles with header row and banding, custom and missing borders, `tcBorders` over `tblBorders`, shading,
vertical alignment, vertical text, exact and at-least row heights, centred / right / indented tables, a nested table, a 60-row table
with a repeated header row over 4 pages, a floating table, cell margins, percentages in mode 15, a compatibility-mode-11 table with
an indent and a merge, Word's default Normal style. (One "missed" rule on t07 is a letter, not a rule: that style has no vertical
borders.)

**Round trip.** Before: table markup lost on save in 2 of 23 files measured that way: `w:tblCellSpacing` dropped (t16) and the
partial row-exception margins written back as 0 (t09). Our writer spells some things differently with the same meaning (`none`
for `nil` borders, `hRule="atLeast"` written out, one `jc` in `tblPr` instead of one per row). Word's re-render of our saved file
against Word's render of the source: 21 of 23 at 0.00 % before (t16 1.99 %, t09 0.28 %); **29 of 29 at 0.00 % after**.

## 3. What was changed

- **wordcraft/0130** (`crates/layout/src/table.rs`, tests in `crates/layout/src/tests.rs`):
  1. `column_content`: a cell spanning columns needs from each column its share of the cell **in proportion to that column's
     grid width** (`c = max(c, need * w / sum)`), so the columns grow only when together they are too narrow. Before, each spanned
     column got an even share of what was missing, which pushed a 14 pt column under a merged cell past its width and took the
     difference from its neighbours. Without a grid (nested measuring fallback) the even share stays.
  2. `layout_table`: in compatibility mode 14 and lower the room for percentage widths and AutoFit is the text column **plus the
     first cell's left and the table's right cell margin** (Word writes a 9571 tw grid for a 100 % table in a 9355 tw column in
     mode 14, and 9355 in mode 15). The table already hung out by the left margin in those modes; now it also reaches as far
     right as Word's.
  3. `layout_table`: the "wider than 1.5 times the column is squeezed into it" rule now applies only to tables without a
     preferred width of their own (`w:tblW` auto). Word writes `w:tblW` in dxa as soon as columns are sized by hand and then lets
     the table run past the margin and the page edge.
- **wordcraft/0131** (`crates/doc/src/props.rs`, `crates/docx/src/read/props.rs`, `crates/docx/src/read/story.rs`,
  `crates/docx/src/write/props.rs`, tests in `crates/docx/tests/row_property_exceptions.rs`):
  1. `TableProps::cell_spacing` reads `w:tblCellSpacing` and writes it back in its `CT_TblPr` place (after `jc`, before `tblInd`).
     Layout ignores it (as before).
  2. A `w:tblCellMar` in `w:tblPr` or `w:tblPrEx` changes only the sides it names: the others keep the table style's (else
     Word's 0 / 108 tw), and for a row exception the table's. Before, a left-out side was read as 0.

## 4. Tests and checks

- Golden check, no Office needed: `python -I scripts/fidelity/tables.py --golden ci/fidelity/tables-golden.json --out <dir>
  [--cli docs-cli]` writes 11 hand-written fixtures (`scripts/fidelity/fid/table_fixtures.py`, Word-like markup from the
  spec, synthetic text), renders them and compares the rule positions with Word's (recorded once with `--record`; numbers only),
  1.5 pt tolerance (2.0 for `form_card`, see the note in the JSON), plus the round trip. Before: 7 of 11 fail; after: 8 ok,
  3 known gaps (`cell_spacing` layout, `wide_auto120`, `wide_auto160`). It runs in the nightly `ci.sh fidelity-directions` job.
- Web size (`scripts/build.sh docs web --release`, measured as `ci.sh sizes` does): docs wasm brotli 9,852,341 B against the
  budget 9,871,648 B, **headroom 19,307 B**; +526 B against the docs dist of 2026-10-24 (`E:/office-target/p1/dist`, not the
  identical base commit). JS glue 20,760 B of 24,576.
- Upstream tests: wordcraft layout, docx and doc test suites 1015 passed, 0 failed; clippy `-D warnings` clean on the three
  crates. Added: `a_cell_spanning_a_narrow_column_keeps_the_grid`, `legacy_mode_percentage_tables_take_the_cell_margins_too`,
  `a_preferred_table_width_wider_than_the_column_stands` (layout; all three fail without 0130), `table_cell_spacing_round_trips`,
  `partial_exception_cell_margins_keep_the_other_sides` (docx).

## 5. Open, and decisions

- **Which file did the owner open, and where?** On the desktop app (MSI) the fixed classes 1-3 are the likely ones (old
  documents in Compatibility Mode, forms with merged cells, wide tables). In the **web** editor every Times New Roman table gets
  taller rows (class 7). Bundling metric-compatible faces (Liberation Serif / Tinos) does not fit the docs headroom (about
  16 KB brotli; the fonts are hundreds of KB); loading them on demand from the server is an owner decision.
- **One real file from the owner would settle it**, especially a **.doc** (not measured here) or a file where the table "falls
  apart" rather than shifting by a few points: none of the measured classes breaks a table apart on the desktop.
- Cell spacing layout (class 4) and Word's own split for `tblW` auto tables past the column (class 6) are not done.
