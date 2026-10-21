# Phase 3 defects the converter reports or accepts (WP9 fixes, 2026-10-21)

PLAN section 7: every loss without a warning is either fixed in the loss table (`services/convert/data/losses.toml`) or listed here with its
evidence. The evidence is the fidelity run of 2026-10-21 (`python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity`):
`directions/<id>.json`, case = fixture name, plus `summary.json` (`loss_without_warning`). The run before the fixes had 73 silent losses; the run
after them has the entries of section 1 and nothing else (`ci/fidelity/silent-baseline.json` holds exactly those). WP10 ran the same harness
over every `convert formats --json` row and found D2-D6 (section 1b) in rows the CI direction set does not cover.

## 1. Silent, accepted for now (no warning; the layout differs, the content does not)

### D1. `.txt` to `.docx`: the page count differs from Word's (2 pages in Word, 5 here)

- **Evidence**: `docs.txt2docx.json`, case `order.txt` (109 lines of Cyrillic and Latin text, 2.6 KB): the reference Word makes 2 pages, `docs-cli`
  makes 5 (`pagination 2 -> 5`). The text is the same (Cyrillic letters 1260 in both, `chars_kept` 1.0); the paragraph count is the same (110 against
  109 lines). The cause is the default paragraph style of the docx `docs-cli` writes: `w:docDefaults` has Aptos 12 pt, 8 pt of space after each
  paragraph and 1.15 line spacing, while Word opens a text file with tight single spacing.
- **Why it is not warned**: nothing is lost. A warning on every `.txt` to `.docx` would be noise (PLAN 2.6: a plain input stays quiet).
- **Fix**: a wordcraft default for text imports (a patch in `patches/`, registered in `patches/REGISTER.md`), or a planner step that sets the
  styles after the conversion. Owner decision; not done in WP9.
- **WP10 (2026-10-21): the same cause on the other text targets.** `extra/directions/docs.txt2odt.json`, `docs.txt2rtf.json`,
  `docs.txt2pdf.json`, `docs.txt2png.json`, all with case `order.txt`: 2 pages in Word, 4, 3, 5 and 5 here. Same decision as D1 (Phase 8).

## 1b. Silent, found by WP10, open (not accepted)

Found by the WP10 runs over all 80 rows of `convert formats --json` (`docs/phase3/fidelity.md` section 4). The evidence is under
`E:/office-target/p3/fidelity/extra/` (the 52 rows that `scripts/fidelity/fid/directions.py` does not list, run by
`docs/phase3/evidence/fidelity_extra.py`) unless another path is given. These rows are not in the CI direction set, so
`ci/fidelity/silent-baseline.json` does not see them. **None is fixed yet. The fixes are in `services/convert`** (loss rows,
planner, environment), not in WP10.

### D2. Same-format conversions carry no warnings at all

- **Evidence**: `docs.odt2odt.json` (case `order.odt`): comments 1 to 0, footnotes 1 to 0, headers and footers 4 to 0, pages 4 to 5,
  status `ok` with **no warning**. `docs.rtf2rtf.json` (`order.rtf`): comments 1 to 0, footnotes 1 to 0, headers and footers 2 to 0, pages
  4 to 3, no warning. The same losses are warned on odt to docx and rtf to docx (E5).
- **Cause** (read in the code): in `services/convert/data/losses.toml`, the odt/rtf rows of `comments_dropped` and `footnotes_dropped` are
  `cross = true`, and `services/convert/src/losses.rs:48` skips a cross row when the target is the input's own format. The idea that the
  same format keeps them is wrong: the reader drops them, so the writer never sees them. The odt row of `headers_footers_dropped` lists
  `to = [docx, rtf, html, pdf, png]`, without odt. The rtf header loss comes from the rtf writer, and its row exists for docx sources only (D3).
- **Fix**: drop `cross` from those two rows (a reader loss applies to every target) and add odt to the odt header row's targets. Change
  the unit test that expects them to stay quiet for the same format. Add rtf to rtf to the D3 header rows.

### D3. RTF sources: header and revision losses not covered

- **Evidence**: `docs.rtf2odt.json`: headers and footers 2 to 0, while only `comments_dropped` and `footnotes_dropped` are raised.
  `docs.rtf2html.json`, `docs.rtf2md.json`, `docs.rtf2txt.json`: tracked changes 1 to 0 with no `revisions_dropped` (E3 lists docx only;
  the rtf reader keeps revisions, as rtf to docx shows). `docs.rtf2html.json`: headers and footers 2 to 0 as well.
- **Fix**: extend the `headers_footers_dropped` (to odt, html) and `revisions_dropped` (to html, md, txt) rows to rtf sources.

### D4. html to html drops linked pictures; md to txt drops tables, both silently

- **Evidence**: `docs.html2html.json` (`order.html` with `order_files/`): images 2 to 0, no warning (html to docx, odt, rtf, pdf and png
  raise `html_images_not_embedded`). `docs.md2txt.json` (`order.md`): tables 2 to 0, no warning (docx, odt, rtf and html to txt raise
  `tables_to_text`).
- **Fix**: add html to html to the `html_images_not_embedded` row and md to the `tables_to_text` row.

### D5. Native PDF to DOCX drops a narrow table column

- **Evidence**: `E:/office-target/p3/fidelity/directions/pdf.pdf2docx.json`, case `p2d_widths.pdf` (a Word-made 5-column table, first
  column 24 pt, Cyrillic names), `convert --pdf-engine native`: status `ok`, no warning. The docx table has 3 or 4 cells per row against
  4 or 5 in the source: the whole first column (`1.1 Рахимов Ш.Т.` and so on) and the text of cells 2.5 and 5.5 are missing.
  Cyrillic letters 480 to 462, docx text similarity 0.761 (`E:/office-target/p3/fidelity/p2d-widths-table.json` lists the cells). The
  sidecar keeps every letter of the same file.
- **Why the harness did not flag it**: `run.py` does not loss-check `pdf2docx` outputs.
- **Fix**: an upstream (pdfcraft export) defect. Mitigate in `convert`: compare the letter count of the PDF text layer with the letter count
  of the docx. If letters are missing, use the sidecar when it is configured, otherwise raise a warning. This must be done before native
  is the default (O8).

### D6. CSV/TSV in the uz/ru format: dates swapped, numbers become text

- **Evidence**: `sheets.csv2xlsx.json`, `sheets.csv2pdf.json`, `sheets.csv2csv.json` and the three tsv rows (cases `staff.csv` and
  `staff.tsv`). The input is Excel 16's own CSV on this PC: `;`, `3500000,0`, `01.03.2019`. Status `ok`, no warning. In the xlsx,
  `01.03.2019` is stored as 2019-01-03 and `11.09.2019` as 2019-11-09, and `3500000,0` is a text cell. The csv output reads `1/3/2019`
  (`E:/office-target/p3/fidelity/csv-recheck.json`: 5 of 79 cells differ, all dates).
- **Cause, measured** (`E:/office-target/p3/fidelity/csv-locale-probe.json`): `sheets-cli convert staff.csv out.xlsx` gives 2019-01-03 and
  text without `GRIDCRAFT_NUM_LOCALE`. With `GRIDCRAFT_NUM_LOCALE=ru` or `uz-latn` it gives 2019-03-01, 2019-09-11 and the number 3500000
  (WP14 patch 0028: a `;` or tab CSV is read in the session locale). `convert` never sets that variable.
- **Fix**: `convert` sets `GRIDCRAFT_NUM_LOCALE` per job from the user's language (as it sets the region for paper size, PLAN F7). Until
  then, warn on every csv/tsv input that is not `,`-delimited. With the variable set, the `pdf_en_us` and `csv_one_sheet_en_us` warnings can
  be revisited too.

## 2. Engine limits that `convert` warns about (kept here for the record; they are not silent)

Each is a row of `data/losses.toml` (code in brackets) or an engine message passed through as `upstream`. They are real defects of the engines, and
the warning is the only mitigation until the engine changes.

| # | Direction | What is lost | Evidence | Warning |
|---|-----------|--------------|----------|---------|
| E1 | docx to odt, rtf, html, md, txt | comments (1 to 0), footnote text (1 to 0) | `docs.docx2odt/rtf/html/md/txt`, `hrm_order.docx` | `comments_dropped`, `footnotes_dropped` |
| E2 | docx to odt, rtf, html | header and footer text, page numbers (2 to 0) | `docs.docx2odt/rtf/html`, `hrm_order.docx`, `hrm_letter.docx` | `headers_footers_dropped` (md and txt: `plain_text_layout`) |
| E3 | docx to html, md, txt | tracked changes as changes (1 to 0; odt and rtf keep them) | `docs.docx2html/md/txt`, `hrm_order.docx` | `revisions_dropped` |
| E4 | docx, odt, rtf, html to md and txt | merged cells (md and txt cannot hold them) and, for txt, tables | `docs.docx2md/txt` | `merged_cells_flattened`, `tables_to_text` |
| E5 | odt and rtf to anything | the reader drops comments and footnotes (odt2docx, rtf2docx 1 to 0); odt also drops headers and footers (4 to 0) | `docs.odt2docx`, `docs.rtf2docx`, `docs.odt2pdf` (52 header words missing) | `comments_dropped`, `footnotes_dropped`, `headers_footers_dropped` |
| E6 | html to docx, odt, rtf, pdf, png | pictures that are separate files (a one-file job never sees them): images 2 to 0, pages 5 to 2 | `docs.html2docx`, `order.html` | `html_images_not_embedded` |
| E7 | doc to docx | comments, tracked changes, paragraph count 99 to 113 | `docs.doc2docx`, `hrm_order.doc` | `doc_import_loss` |
| E8 | xls to xlsx | charts, pictures, conditional formatting (pages 4 to 3) | `sheets.xls2xlsx`, `hrm_staff.xls` | the engine's own messages (`upstream`); `xls_pdf_chart_page` for pdf |
| E9 | xlsb and ods to xlsx | formulas become values; formatting, so dates, currency and percentages show as numbers (text 1.0 to 0.46 and 0.29) | `sheets.xlsb2xlsx`, `sheets.ods2xlsx`, `hrm_timesheet` | the engine's own messages (`upstream`) |
| E10 | xlsx, xls, xlsb, ods to pdf and csv | numbers and dates use en-US formats (the harness reads this as text 1.0 to 0.56 on `hrm_timesheet.xlsx`: "12.10" against "12/10") | `sheets.xlsx2pdf`, `hrm_timesheet.xlsx` | `pdf_en_us`, `csv_one_sheet_en_us` (go away with WP14 NumLocale) |
| E11 | pdf to txt, docx | a scan has no text layer | `pdf.edge`, `scan.pdf` | `empty_text` |
| E12 | pdf to txt, docx | a cut-off file: `failed parsing cross reference table` | `pdf.edge`, `truncated.pdf` | `pdf_damaged` plus the engine's message |

## 3. Harness findings that were not defects (fixed in `scripts/fidelity`)

For the record: these were counted as silent losses and were measurement errors, so the harness was changed, not the converter.
RTF counts (tables 14 for 2, pictures twice, comments twice, empty first-page headers); docx merged cells counted once per continued cell;
html and RTF merged cells not counted at all (RTF has no marker for a horizontal merge, so it is not compared); a low word-order similarity with
no missing letters (reading order of tables, words glued by a tab in the PDF text); transliteration changing the page count by under 10 %;
a Word reference that misread a cp1251 `.txt` (the reference is now given the same text as UTF-8 with a BOM); and text, page and Cyrillic
differences that are consequences of a loss the report already warns about.
