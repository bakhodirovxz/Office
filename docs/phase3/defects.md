# Phase 3 defects the converter reports or accepts (WP9 fixes, 2026-10-21)

PLAN section 7: every loss without a warning is either fixed in the loss table (`services/convert/data/losses.toml`) or listed here with its
evidence. The evidence is the fidelity run of 2026-10-21 (`python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity`):
`directions/<id>.json`, case = fixture name, plus `summary.json` (`loss_without_warning`). The run before the fixes had 73 silent losses; the run
after them has the entries of section 1 and nothing else (`ci/fidelity/silent-baseline.json` holds exactly those).

## 1. Silent, accepted for now (no warning; the layout differs, the content does not)

### D1. `.txt` to `.docx`: the page count differs from Word's (2 pages in Word, 5 here)

- **Evidence**: `docs.txt2docx.json`, case `order.txt` (109 lines of Cyrillic and Latin text, 2.6 KB): the reference Word makes 2 pages, `docs-cli`
  makes 5 (`pagination 2 -> 5`). The text is the same (Cyrillic letters 1260 in both, `chars_kept` 1.0); the paragraph count is the same (110 against
  109 lines). The cause is the default paragraph style of the docx `docs-cli` writes: `w:docDefaults` has Aptos 12 pt, 8 pt of space after each
  paragraph and 1.15 line spacing, while Word opens a text file with tight single spacing.
- **Why it is not warned**: nothing is lost. A warning on every `.txt` to `.docx` would be noise (PLAN 2.6: a plain input stays quiet).
- **Fix**: a wordcraft default for text imports (a patch in `patches/`, registered in `patches/REGISTER.md`), or a planner step that sets the
  styles after the conversion. Owner decision; not done in WP9.

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
