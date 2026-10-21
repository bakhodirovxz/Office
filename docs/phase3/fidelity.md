# Phase 3 WP10: conversion fidelity report

> **SYNTHETIC set only.** Every number below comes from files that Microsoft Office 16 and our generators wrote. The
> owner's anonymised `corpus/` (B-14, owner item O9) has not been delivered, so **no claim here is about real HRM
> documents**. See section 5.

Plan: PLAN section 7 (report spec), section 3.4 (PDF -> DOCX metrics), WP10, gate items 9.3 and O8 (9.4).
Run on 2026-10-21, branch `phase-3` at `db46604`, Windows 10 dev PC.

## 0. Headline

- **All 80 rows of `convert formats --json` were run and measured** (117 cases: 65 in the CI harness set, 52 added by
  WP10). Every row has numbers; where one column cannot be measured the table says why. 0 unexpected failures: the 3
  failures are the intended ones (`.ppt` exit 3 with the "re-save as .pptx" message, a 0-byte PDF exit 9).
- **Silent losses.** The CI set (`scripts/fidelity/run.py --directions all`) has **1**, D1 (txt -> docx page count),
  the same as `ci/fidelity/silent-baseline.json`: **0 new**. **The 52 rows the CI set does not cover have 22 more**
  (12 directions). Two more silent losses are outside what `run.py` checks at all: native PDF -> DOCX dropped a table
  column, and CSV/TSV input in the uz/ru format has its dates swapped. All are listed in `defects.md` (D2-D6). None
  is fixed here (WP10 does not touch `services/`).
- **Where we stand.** pptx -> pdf (mean pixel diff 0.02-0.61 %) and docx -> pdf (0.41-1.50 %) are closer to Office
  than LibreOffice 7.4 and ONLYOFFICE x2t 8.2 are. odt and rtf output and legacy (.doc, .xls) import are clearly worse
  than Office and x2t (8-12 % pixel diff, page counts off by one, headers, footnotes and comments dropped with
  warnings). Sheets output keeps every letter, but numbers and dates are en-US (warned).
- **O8 recommendation: pdfcraft native as the default PDF -> DOCX engine, after D5 is guarded; the pdf2docx sidecar
  as an optional engine.** Reasons and caveats in section 2.4.

## 1. Method

**Command.** `python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity` (the WP9
harness, unchanged), then `python -I docs/phase3/evidence/fidelity_extra.py --out E:/office-target/p3/fidelity/extra
--fixtures E:/office-target/p3/fidelity/fixtures-extra` for the 52 `formats` rows that `fid/directions.py` does not
list. The second script is the same harness with another direction table (one fixture per source format). It
also gives each HTML source its own Word instance, because Word hung on one SaveAs. The table below is rendered from
the JSON by `docs/phase3/evidence/fidelity_table.py`; CSV cells were re-measured by
`docs/phase3/evidence/csv_recheck.py` (section 3.2).

**Candidate.** Everything goes through `convert --json` (`E:/office-target/p1/services/release/convert.exe`). It was
rebuilt under the build lock at HEAD because it was older than `4c42b44`. The domain CLIs are at
`E:/office-target/p1/<domain>/release`. The harness sets `WORDCRAFT_REGION=en-US` and does not set `GRIDCRAFT_NUM_LOCALE`.

**References.**

| Engine | Version | Role | Notes |
|---|---|---|---|
| MS Office 16 COM | Word 16.0.19127.20860 | primary: same conversion, and Office PDF of every source = the truth | Kills only the PIDs the harness started. Word PDF reflow (PDF -> DOCX) **hangs headless** (modal dialog even with `DisableConvertPdfWarning=1`), so it was killed after the batch timeout on all 5 PDF fixtures. Word also hung twice on `order.html` SaveAs2 RTF (killed, row marked). Word has no Markdown reader or writer. |
| LibreOffice | 7.4.7.2 (our `office/soffice:dev` sidecar) | second reference | The sidecar takes OOXML in only and writes pdf/png or the same OOXML kind. It refuses legacy input (`415 legacy_format`, O2). So it gives numbers only for docx/xlsx/pptx -> pdf/png/same format. |
| ONLYOFFICE x2t | documentserver 8.2.3-6, JWT off, local container, never shipped | third reference | It needs `-e ALLOW_PRIVATE_IP_ADDRESS=true`: without it every job fails `-4` (the server refuses to download from `host.docker.internal`, a private IP). A first run had that problem and was discarded. It cannot write md (`-7`) or read tsv. |
| pdf2docx sidecar | pdf2docx 0.5.13, PyMuPDF 1.26.7, python-docx 1.2.0, merge_lines `9563dd3a` | PDF -> DOCX candidate | `sidecars/pdf2docx/run.sh`, internal network |

**Metrics** (as WP9, `scripts/fidelity/fid/util.py`): all PDFs are rasterised by `pdf-cli render` at 96 dpi. Pixel diff
is grey with a 1 px blur, and |a-b| > 48 counts as different. Text similarity is the word-level difflib ratio of
`pdf-cli text` (it is order-sensitive). "Letters kept" is the share of the reference's letters and digits still
present in any order. Cyrillic kept is a letter count. Structural counts come from the OOXML/ODF/RTF/HTML parts. For
an editable output, Office opens our file ("reopen") and exports it to PDF. That PDF is compared with the Office PDF of
the source. **Noise floor:** Office's own conversion reopened in Word is not 0 either (docx -> odt 4.8-5.4 %, doc -> odt
3.0 %), so a few percent of pixel diff on ODF and RTF partly comes from Word's importers.

**Runs.** The main set ran twice: once fresh with `--force` (x2t blocked, discarded for x2t only), then again with
x2t fixed and the Office cache kept. Every non-x2t metric was **identical** in both runs (diffed). The extra set ran
once fresh, then once more for the Office jobs that the HTML hang had taken down.

| JSON (all under `E:/office-target/p3/fidelity/`) | What |
|---|---|
| `summary.json`, `directions/<id>.json` | CI direction set, 38 ids, 65 cases, generated 2026-10-21 20:42:57 |
| `extra/summary.json`, `extra/directions/<id>.json` | the 52 extra `formats` rows, generated 21:12:44 |
| `csv-recheck.json` | CSV cell equality with each file's own delimiter (section 3.2) |
| `csv-locale-probe.json` | CSV date and number parsing by `GRIDCRAFT_NUM_LOCALE` (D6) |
| `p2d-widths-table.json` | the table cells of `p2d_widths` for source, native and sidecar (D5) |
| `summary.wp10-run1-x2t-blocked.json`, `directions_wp10-run1-x2t-blocked/` | the first main run (x2t `-4`) |
| `run-wp10.log`, `run-wp10b.log`, `run-wp10-extra*.log` | console logs |
| repo: `docs/phase3/evidence/fidelity-summary.json` (103 KB) | per case: status, warnings, craft metrics, reference metrics, silent losses |

**Fixtures** (`E:/office-target/p3/fidelity/fixtures`, from `scripts/fidelity/gen_fixtures.py`): the Phase 0
Office-16 set (`hrm_order.docx/.doc`, `hrm_letter`, `hrm_staff.xlsx/.xls/.xlsb/.ods`, `hrm_timesheet.*`,
`hrm_deck.pptx/.ppt`, `01-shapes-notes`, `02-languages`, `02-table-rich`), the `order.*` family written by Word
(odt, rtf, dot, html + `order_files/`, txt in cp1251, md), the Phase 2 translit fixtures, the section 3.4
`p2d_*.docx` written by Word and exported by Word to PDF (with `truth.json`), and the edge PDFs. `fixtures-extra` adds
`staff.csv` (Excel 16's own CSV of `hrm_staff.xlsx`, from the main run's Office reference, so `;`, decimal comma and
`dd.mm.yyyy`) and `staff.tsv` (the same cells, tab-separated).

## 2. PDF -> DOCX: the candidates (input to O8)

Five Word-made PDFs (PLAN 3.4): `p2d_latn`, `p2d_cyrl`, `p2d_ru` (an order: title, justified body, a 4x3 table),
`p2d_overimage` (words drawn over a stamp and a signature picture) and `p2d_widths` (a 5-column table with known
`gridCol` widths 24/70/150/110/60 pt and merged cells, Cyrillic names). The truth is the source docx. Every candidate's
docx was opened in Word and exported to PDF, then compared with the source PDF. Source: `directions/pdf.pdf2docx.json`.

### 2.1 Per fixture

"Pages" is pages of the source PDF / pages after Word re-renders the docx. "Paras" is paragraphs out / in the source
docx. "Width err" is the mean relative error of the column width fractions of the first table (lower is better).
"Box" is the share of paragraphs inside absolutely positioned text boxes (lower is more editable).

| Fixture | Engine | Pages | Text sim (docx) | Cyrillic kept | Paras | Tables out/src | Width err | Box | Pixel % mean / max |
|---|---|---|---|---|---|---|---|---|---|
| p2d_latn | native | 2/**3** | 0.988 | 1.00 | 51/37 | 1/1 | **0.108** | 0 | 12.95 / 20.33 |
| | sidecar (pdf2docx + merge_lines) | 2/2 | 0.988 | 1.00 | 35/37 | 1/1 | 0.866 | 0 | 7.00 / 12.64 |
| | pdf2docx raw | 2/2 | 0.988 | 1.00 | 35/37 | 1/1 | 0.866 | 0 | **5.09** / 9.31 |
| | x2t | 2/2 | 0.839 | 1.00 | 66/37 | **0/1** | - | 0.29 | 15.18 / 26.16 |
| p2d_cyrl | native | 2/**3** | 0.981 | 1.00 | 47/38 | 1/1 | **0.147** | 0 | 12.28 / 19.69 |
| | sidecar | 2/2 | 0.981 | 1.00 | 34/38 | 2/1 (split) | 0.866 | 0 | 6.11 / 11.79 |
| | pdf2docx raw | 2/2 | 0.981 | 1.00 | 34/38 | 2/1 (split) | 0.866 | 0 | **5.53** / 10.62 |
| | x2t | 2/2 | 0.838 | 1.00 | 63/38 | 0/1 | - | 0.32 | 13.26 / 25.52 |
| p2d_ru | native | 2/**3** | 0.985 | 1.00 | 52/37 | 1/1 | **0.108** | 0 | 11.77 / 17.97 |
| | sidecar | 2/2 | 0.985 | 1.00 | 36/37 | 1/1 | 0.866 | 0 | 10.27 / 13.50 |
| | pdf2docx raw | 2/2 | 0.985 | 1.00 | 37/37 | 1/1 | 0.866 | 0 | **4.37** / 6.79 |
| | x2t | 2/2 | 0.834 | 1.00 | 71/37 | 0/1 | - | 0.27 | 14.63 / 22.87 |
| p2d_overimage | native | 1/1 | 0.979 | - | 21/14 | 0/0 | - | **0.95** | **6.20** / 6.20 |
| | sidecar | 1/1 | 0.979 | - | 10/14 | 0/0 | - | 0 | 8.66 / 8.66 |
| | pdf2docx raw | 1/**2** | 0.979 | - | 11/14 | 0/0 | - | 0 | 7.77 / 7.77 |
| | x2t | 1/1 | 0.590 | - | 28/14 | 0/0 | - | 0.43 | 11.38 / 11.38 |
| p2d_widths | native | 2/2 | **0.761** | **0.963** | 32/35 | 1/1, **4 of 5 columns** | - | 0 | 3.96 / 5.95 |
| | sidecar | 2/2 | 0.980 | 1.00 | 45/35 | 2/1 (split) | 0.742 | 0 | **0.68** / 0.73 |
| | pdf2docx raw | 2/2 | 0.980 | 1.00 | 45/35 | 2/1 (split) | 0.742 | 0 | **0.68** / 0.73 |
| | x2t | 2/2 | 0.401 | 1.00 | 107/35 | 0/1 | - | 0.90 | 2.66 / 2.88 |
| all | Word 16 reflow (Office) | **not measurable: Word hangs headless on PDF open via COM** (killed after the batch timeout, all 5) | | | | | | | |
| all | LibreOffice | **not measured**: our soffice sidecar takes OOXML only. LibreOffice opens PDF in Draw (positioned frames); its Writer import is not in the image | | | | | | | |

The known words drawn over pictures (`STAMPWORD`, `SIGNWORD`) are in the docx text for **every** engine (`known_words`
true). The brief's "text over images is lost" did not reproduce on a Word-made PDF.

### 2.2 Totals over the five fixtures

| | native | sidecar | pdf2docx raw | x2t |
|---|---|---|---|---|
| page count kept after Word re-render | **2 of 5** (the 3 orders grow 2 -> 3) | **5 of 5** | 4 of 5 | 5 of 5 |
| mean pixel diff % (mean of the 5 means) | 9.43 | 6.54 | **4.69** | 11.42 |
| letters kept (worst fixture) | **0.963 (a column lost, no warning: D5)** | 1.00 | 1.00 | 1.00 |
| tables kept as tables (4 source tables) | 4 (one with a column missing) | 4 (2 of them split in two) | 4 (2 split) | **0** |
| column width error, 3 orders | **0.11-0.15** | 0.87 | 0.87 | no table |
| text in positioned text boxes | 0, except 95 % on the page with art | 0 | 0 | 27-90 % |
| one paragraph per line (brief section 3) | no (ratio 0.91-1.50, lines merged) | no (0.71-1.29) | no (0.79-1.29) | yes-ish (1.66-3.06) |
| time per job | 0.13-0.78 s (4.9 s once, cold) | not recorded by the harness | not recorded | not recorded |
| licence, deployment | permissive, built into `pdf-cli`, no container | AGPL container (PyMuPDF), Docker, image 867 MB | same | AGPL, reference only, never shipped |

### 2.3 What the numbers say

- **The pdf2docx weak spot the brief names (Cyrillic one paragraph per line) did not appear** on these Word-made PDFs:
  the raw pdf2docx paragraph ratio is 0.79-1.29, and `merge_lines.py` merged 0 or 1 paragraph per file (sidecar
  `X-Convert-Report`). On these fixtures merge_lines made the picture **worse** on 4 of 5 files: mean pixel diff 4.37 % raw
  against 10.27 % merged on `p2d_ru`, 5.09 against 7.00 on `p2d_latn`. It fixed the page count of `p2d_overimage`
  (2 -> 1). That brief claim may be about PDFs from other
  producers (1C, browsers, scanners with OCR). Only the corpus can show it.
- **Native is the best editable structure:** real tables with close column widths (error 0.11-0.15 against 0.87 for
  pdf2docx, which gives near-equal columns). But it adds paragraph spacing, so Word re-flows the 2-page orders to 3 pages
  (pixel diff about 12 %). On the art page it falls back to positioned text boxes (95 %). On `p2d_widths` it **dropped
  the first column (24 pt wide) and two cell texts with no warning** (`p2d-widths-table.json`: rows of 3-4 cells
  against 4-5; 18 of 480 Cyrillic letters missing). That is D5.
- **pdf2docx is the most faithful picture** (5/5 page counts, the lowest pixel diffs) and kept every letter. But it
  splits tables, and its column widths are wrong.
- **x2t** gives the "faithful copy built from text boxes" the brief describes: no tables at all, 27-90 % of the text in
  positioned boxes, and the worst pixel diff. It is the worst to edit. It is a reference only.

### 2.4 Recommendation for O8

**Default: pdfcraft native, once D5 is guarded. The pdf2docx sidecar stays as an optional engine (`--pdf-engine
sidecar`, a choice in the UI) where the operator runs it.**

Reasons:
1. Licence and deployment. Native is permissive and built in. It needs no Docker and no AGPL container with its
   source-offer obligations (PLAN 3.1), so local mode (O1/O6: the user's own PC, no server) gets PDF -> DOCX at all.
   With the sidecar as the default, a plain local install would have no default engine.
2. The editable result is better where it matters for HRM orders: the tables are real tables with the right column
   proportions.
3. The fidelity gap is moderate on this set (mean pixel diff 9.4 % against 6.5 %). Most of it is extra paragraph
   spacing (one page more), not lost content.

Caveats (the owner should weigh these):
- **D5 must be closed first.** A table column lost with no warning is exactly what the gate forbids. A cheap guard
  covers it whatever the engine: compare the letter count of `pdf-cli text` with the letter count of the docx. If letters
  are missing, fall back to the sidecar when it is configured, otherwise warn. The harness also has to loss-check
  pdf2docx outputs (today it does not; section 4).
- **If visual fidelity is to weigh more than editability** (owner priority: fidelity first), the numbers favour the
  sidecar: 5/5 page counts and lower pixel diffs. Then the default would be "sidecar when reachable, native
  otherwise". That is the hybrid this data supports.
- **A per-script hybrid (native by default, the sidecar for Cyrillic) is not supported by the numbers:** Cyrillic did
  no worse than Latin with either engine (`p2d_cyrl`/`p2d_ru` against `p2d_latn`).
- Five synthetic Word-made PDFs, one producer. Real HRM PDFs (other producers, scans, stamps) can change this. Re-run
  on the corpus (O9) before the choice is final.
- `merge_lines.py` is unmodified (owner code, sha256 checked by the sidecar). It did not help here: the picture got worse on 4 of 5
  files. Leave it on for now (it targets other producers), but measure it on the corpus.

## 3. Per direction (every row of `convert formats --json`)

Columns follow PLAN section 7. "Pages" is the Office PDF of the source / ours, opened in Office for editable targets.
"Text sim / letters kept" is the word-order similarity against the Office PDF (or Office's own text export for
html/md/txt) and the share of letters kept. Pixel diff is against the Office PDF of the source. "Structure lost" is
counts in the source -> ours. The reference columns give the same metrics for that engine's own conversion of the
source (reopened in Word for editable targets). "Silent" is the loss-without-warning check of `run.py` for that case.
Several fixtures in one cell are separated by `·`. Peak MB is what `convert` reports per step (the tool process, sampled).

### 3.1 Notes that the table cannot carry

- **Not measurable, and why:**
  - **md:** Word has no Markdown reader, so md sources have no Office conversion. Their "reference PDF" is Word showing
    the raw Markdown as plain text, so pixel and page numbers there show only that the text survived.
  - **Same-format rows** (docx -> docx, odt -> odt, xlsx -> xlsx, ...): Office has nothing to do. Our output is still
    compared with the Office PDF of the source.
  - **tsv:** Office COM cannot open it, so there is no Office PDF of the source. Only structure and status are
    measured, plus the CSV re-measure of tsv -> csv against the source.
  - **png targets:** Office has no per-page PNG export, so the reference is the pages of the Office PDF.
  - **PowerPoint's png export (5/0):** a harness artefact (a folder is not counted).
- **pdf -> docx** is measured in section 2. Its row shows the native default.
- **Text similarity of html/md/txt targets** is word-order similarity against Word's own text export, and is about 0.57-0.63
  for **every** engine, Office's own html included. Tables are read in another order. Letters kept (0.91-1.00) is the
  number to read.
- **x2t PDF text:** its PDFs give lower text similarity through `pdf-cli text` (0.76 on docx -> pdf, letters kept 0.94)
  at a pixel diff of 0.59 %. That is the text layer of its PDFs, not missing text on the page.
- **Translit rows:** the text is supposed to change, so only pagination is judged (Phase 2 owns the text checks).
  `uz108-latn` grows 108 -> 113 pages (+4.6 %, under the 10 % allowance).

### 3.2 CSV: the harness misread Excel's CSV

`run.py`'s `csv_compare` reads Excel's file with `,`. But Excel 16 wrote `;`, decimal comma and `dd.mm.yyyy`, even
though `office_ref.ps1` passes `Local:=False` with format 62 (xlCSVUTF8). So the table's harness number was 0 equal
cells. `csv_recheck.py` reads each file with its own delimiter and drops Excel's trailing empty padding.
**xlsx/xls -> csv: every cell is either equal or differs only by locale** (staff 55 + 24 / 79, timesheet 44 + 5 / 49:
`3500000,0` against `3500000.0`, `01.03.2019` against `01/03/2019`). That is E10, warned by `csv_one_sheet_en_us`.
xlsb and ods -> csv differ in 24 cells that are not locale only: dates as `2019-03-01 00:00:00.000`, and
`3500000` without its format. That is E9 (formats lost on import), warned by `upstream`.
**csv/tsv -> csv: 74 of 79 cells equal. The 5 that differ are dates moved from `dd.mm.yyyy` to `d/m/yyyy`**, and the
day and month are swapped in the stored value. That is D6.

### 3.3 The table

#### In `convert formats --json` (80 rows)

| Direction | Fixture(s) | Status, exit | Time s / peak MB | Pages ref/ours | Text sim / letters kept | Cyrillic kept | Pixel diff % mean / max | Structure lost (counts) | Warnings | Office | LibreOffice | x2t | Silent | JSON |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| docx -> docx | hrm_order.docx | ok 0 | 0.13 / 1 | 4/4 | 1.00 / 1.00 | 1.00 | 0.23 / 0.90 | none | - | same format | 4/4 pp, pix 0.03, sim 1.00 | 4/4 pp, pix 0.00, sim 1.00 | 0 | `extra/directions/docs.docx2docx.json` |
| docx -> odt | hrm_order.docx · hrm_letter.docx | warn 0 · warn 0 | 0.45 / 1 · 0.10 / 3 | 4/5 · 1/1 | 0.76 / 0.90 · 0.83 / 0.98 | 0.91 · 1.00 | 10.17 / 14.96 · 6.44 / 6.44 | comments 1>0, footnotes 1>0, headers_footers 2>0 · headers_footers 2>0 | comments_dropped,footnotes_dropped,headers_footers_dropped · headers_footers_dropped | 4/4 pp, pix 5.38, sim 1.00 · 1/1 pp, pix 4.76, sim 1.00 | no such route · no such route | 4/4 pp, pix 7.62, sim 1.00 · 1/1 pp, pix 9.96, sim 1.00 | 0 · 0 | `directions/docs.docx2odt.json` |
| docx -> rtf | hrm_order.docx · hrm_letter.docx | warn 0 · warn 0 | 0.11 / 3 · 0.17 / 1 | 4/3 · 1/1 | 0.88 / 0.90 · 0.98 / 0.98 | 0.91 · 1.00 | 11.71 / 17.73 · 11.35 / 11.35 | comments 1>0, footnotes 1>0, headers_footers 2>0 · headers_footers 2>0 | comments_dropped,footnotes_dropped,headers_footers_dropped · headers_footers_dropped | 4/4 pp, pix 0.70, sim 1.00 · 1/1 pp, pix 1.29, sim 1.00 | no such route · no such route | 4/4 pp, pix 0.74, sim 1.00 · 1/1 pp, pix 3.23, sim 1.00 | 0 · 0 | `directions/docs.docx2rtf.json` |
| docx -> html | hrm_order.docx · hrm_letter.docx | warn 0 · warn 0 | 0.12 / 2 · 0.20 / 1 | - | 0.57 / 0.95 · 0.63 / 0.96 | - | - | comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0 · headers_footers 2>0 | comments_dropped,footnotes_dropped,headers_footers_dropped,revisions_dropped · headers_footers_dropped | sim 0.58 · sim 0.63 | no such route · no such route | sim 0.57 · sim 0.63 | 0 · 0 | `directions/docs.docx2html.json` |
| docx -> md | hrm_order.docx · hrm_letter.docx | warn 0 · warn 0 | 0.13 / 3 · 0.11 / 2 | - | - | - | - | comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0, merged 4>0 · headers_footers 2>0, merged 2>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout,revisions_dropped · merged_cells_flattened,plain_text_layout | cannot write it · cannot write it | no such route · no such route | unavail.: x2t error -7 · unavail.: x2t error -7 | 0 · 0 | `directions/docs.docx2md.json` |
| docx -> txt | hrm_order.docx · hrm_letter.docx | warn 0 · warn 0 | 0.22 / 3 · 0.12 / 3 | - | 0.58 / 0.96 · 0.63 / 0.96 | - | - | tables 2>0, images 1>0, comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0, merged 4>0 · tables 1>0, headers_footers 2>0, merged 2>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout,revisions_dropped,tables_to_text · merged_cells_flattened,plain_text_layout,tables_to_text | sim 1.00 · sim 1.00 | no such route · no such route | sim 0.59 · sim 0.63 | 0 · 0 | `directions/docs.docx2txt.json` |
| docx -> pdf | hrm_order.docx · hrm_letter.docx | ok 0 · ok 0 | 1.66 / 28 · 0.61 / 27 | 4/4 · 1/1 | 0.95 / 1.00 · 0.95 / 1.00 | 1.00 · 1.00 | 0.41 / 1.02 · 1.50 / 1.50 | none · none | - · - | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | 4/4 pp, pix 0.52, sim 0.95 · 1/1 pp, pix 3.51, sim 0.95 | 4/4 pp, pix 0.59, sim 0.76 · 1/1 pp, pix 3.67, sim 0.87 | 0 · 0 | `directions/docs.docx2pdf.json` |
| docx -> png | hrm_order.docx · hrm_letter.docx | ok 0 · ok 0 | 0.62 / 29 · 0.44 / 21 | 4/4 · 1/1 | - | - | 0.04 / 0.07 · 1.05 / 1.05 | none · none | - · - | no png export · no png export | 4/1 pp, pix 1.36 · 1/1 pp, pix 4.11 | n/a · n/a | 0 · 0 | `directions/docs.docx2png.json` |
| doc -> docx | hrm_order.doc · hrm_letter.doc | warn 0 · warn 0 | 0.11 / 2 · 0.12 / 3 | 4/4 · 1/1 | 0.89 / 1.00 · 0.92 / 1.00 | 1.00 · 1.00 | 7.58 / 12.19 · 10.67 / 10.67 | none · none | doc_import_loss · doc_import_loss | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | refuses legacy · refuses legacy | 4/4 pp, pix 0.56, sim 1.00 · 1/1 pp, pix 1.10, sim 1.00 | 0 · 0 | `directions/docs.doc2docx.json` |
| doc -> odt | hrm_order.doc | warn 0 | 0.11 / 3 | 4/5 | 0.76 / 0.90 | 0.91 | 9.85 / 14.43 | none | doc_import_loss | 4/4 pp, pix 3.00, sim 1.00 | no such route | 4/4 pp, pix 7.84, sim 0.95 | 0 | `extra/directions/docs.doc2odt.json` |
| doc -> rtf | hrm_order.doc | warn 0 | 0.11 / 3 | 4/3 | 0.84 / 0.90 | 0.91 | 10.86 / 16.49 | none | doc_import_loss | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 4.49, sim 0.99 | 0 | `extra/directions/docs.doc2rtf.json` |
| doc -> html | hrm_order.doc | warn 0 | 0.12 / 3 | - | 0.57 / 0.95 | - | - | none | doc_import_loss | sim 0.57 | no such route | sim 0.56 | 0 | `extra/directions/docs.doc2html.json` |
| doc -> md | hrm_order.doc | warn 0 | 0.10 / 3 | - | - | - | - | none | doc_import_loss,plain_text_layout | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/docs.doc2md.json` |
| doc -> txt | hrm_order.doc | warn 0 | 0.09 / 3 | - | 0.58 / 0.96 | - | - | none | doc_import_loss,plain_text_layout | sim 1.00 | no such route | sim 0.59 | 0 | `extra/directions/docs.doc2txt.json` |
| doc -> pdf | hrm_order.doc | warn 0 | 0.16 / 3 | 4/4 | 0.93 / 1.00 | 1.00 | 8.00 / 12.94 | none | doc_import_loss | 4/4 pp, pix 0.00, sim 1.00 | refuses legacy | 4/4 pp, pix 1.25, sim 0.76 | 0 | `directions/docs.doc2pdf.json` |
| doc -> png | hrm_order.doc | warn 0 | 0.45 / 29 | 4/4 | - | - | 7.85 / 12.54 | none | doc_import_loss | no png export | refuses legacy | n/a | 0 | `extra/directions/docs.doc2png.json` |
| odt -> docx | order.odt | warn 0 | 0.11 / 2 | 4/4 | 0.90 / 0.90 | 0.91 | 8.26 / 14.92 | comments 1>0, footnotes 1>0, headers_footers 4>0 | comments_dropped,footnotes_dropped,headers_footers_dropped | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 1.63, sim 1.00 | 0 | `directions/docs.odt2docx.json` |
| odt -> odt | order.odt | ok 0 | 0.11 / 3 | 4/5 | 0.76 / 0.90 | 0.91 | 9.81 / 14.48 | comments 1>0, footnotes 1>0, headers_footers 4>0 | - | same format | no such route | 4/4 pp, pix 8.95, sim 0.97 | comments 1>0, footnotes 1>0, headers_footers 4>0, pagination 4>5, text 1.0>0.7612, cyrillic 1311>1198 | `extra/directions/docs.odt2odt.json` |
| odt -> rtf | order.odt | warn 0 | 0.17 / 3 | 4/3 | 0.88 / 0.90 | 0.91 | 10.92 / 16.81 | comments 1>0, footnotes 1>0, headers_footers 4>0 | comments_dropped,footnotes_dropped,headers_footers_dropped | 4/4 pp, pix 0.89, sim 1.00 | no such route | 4/4 pp, pix 2.51, sim 0.99 | 0 | `extra/directions/docs.odt2rtf.json` |
| odt -> html | order.odt | warn 0 | 0.12 / 3 | - | 0.56 / 0.91 | - | - | comments 1>0, footnotes 1>0, headers_footers 4>0 | comments_dropped,footnotes_dropped,headers_footers_dropped | sim 0.58 | no such route | sim 0.56 | 0 | `extra/directions/docs.odt2html.json` |
| odt -> md | order.odt | warn 0 | 0.16 / 3 | - | - | - | - | comments 1>0, footnotes 1>0, headers_footers 4>0, merged 4>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/docs.odt2md.json` |
| odt -> txt | order.odt | warn 0 | 0.16 / 3 | - | 0.57 / 0.91 | - | - | tables 2>0, images 1>0, comments 1>0, footnotes 1>0, headers_footers 4>0, merged 4>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout,tables_to_text | sim 1.00 | no such route | sim 0.57 | 0 | `extra/directions/docs.odt2txt.json` |
| odt -> pdf | order.odt | warn 0 | 1.65 / 33 | 4/4 | 0.88 / 0.90 | 0.91 | 8.67 / 15.86 | none | comments_dropped,footnotes_dropped,headers_footers_dropped | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 2.58, sim 0.76 | 0 | `directions/docs.odt2pdf.json` |
| odt -> png | order.odt | warn 0 | 0.59 / 33 | 4/4 | - | - | 8.42 / 15.09 | none | comments_dropped,footnotes_dropped,headers_footers_dropped | no png export | no such route | n/a | 0 | `extra/directions/docs.odt2png.json` |
| rtf -> docx | order.rtf | warn 0 | 0.65 / 4 | 4/4 | 0.95 / 0.94 | 0.91 | 0.94 / 1.72 | comments 1>0, footnotes 1>0 | comments_dropped,footnotes_dropped | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 0.06, sim 1.00 | 0 | `directions/docs.rtf2docx.json` |
| rtf -> odt | order.rtf | warn 0 | 0.18 / 3 | 4/5 | 0.76 / 0.90 | 0.91 | 10.22 / 15.09 | comments 1>0, footnotes 1>0, headers_footers 2>0 | comments_dropped,footnotes_dropped | 4/4 pp, pix 5.75, sim 1.00 | no such route | 4/4 pp, pix 7.87, sim 1.00 | headers_footers 2>0 | `extra/directions/docs.rtf2odt.json` |
| rtf -> rtf | order.rtf | ok 0 | 0.18 / 3 | 4/3 | 0.88 / 0.90 | 0.91 | 11.62 / 17.56 | comments 1>0, footnotes 1>0, headers_footers 2>0 | - | same format | no such route | 4/4 pp, pix 0.08, sim 1.00 | comments 1>0, footnotes 1>0, headers_footers 2>0, pagination 4>3, text 1.0>0.882, cyrillic 1311>1198 | `extra/directions/docs.rtf2rtf.json` |
| rtf -> html | order.rtf | warn 0 | 0.18 / 3 | - | 0.57 / 0.95 | - | - | comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0 | comments_dropped,footnotes_dropped | sim 0.57 | no such route | sim 0.57 | revisions 1>0, headers_footers 2>0 | `extra/directions/docs.rtf2html.json` |
| rtf -> md | order.rtf | warn 0 | 0.18 / 3 | - | - | - | - | comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0, merged 4>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout | cannot write it | no such route | unavail.: x2t error -7 | revisions 1>0 | `extra/directions/docs.rtf2md.json` |
| rtf -> txt | order.rtf | warn 0 | 0.17 / 3 | - | 0.58 / 0.96 | - | - | tables 2>0, images 1>0, comments 1>0, revisions 1>0, footnotes 1>0, headers_footers 2>0, merged 4>0 | comments_dropped,footnotes_dropped,merged_cells_flattened,plain_text_layout,tables_to_text | sim 1.00 | no such route | sim 0.59 | revisions 1>0 | `extra/directions/docs.rtf2txt.json` |
| rtf -> pdf | order.rtf | warn 0 | 0.22 / 9 | 4/4 | 0.89 / 0.94 | 0.91 | 1.39 / 2.41 | none | comments_dropped,footnotes_dropped | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 0.71, sim 0.76 | 0 | `directions/docs.rtf2pdf.json` |
| rtf -> png | order.rtf | warn 0 | 0.62 / 29 | 4/4 | - | - | 1.00 / 1.83 | none | comments_dropped,footnotes_dropped | no png export | no such route | n/a | 0 | `extra/directions/docs.rtf2png.json` |
| html -> docx | order.html | warn 0 | 1.04 / 4 | 5/2 | 0.89 / 1.00 | 1.00 | 11.34 / 12.22 | images 2>0 | html_images_not_embedded | 5/5 pp, pix 0.02, sim 1.00 | no such route | 5/3 pp, pix 9.54, sim 0.83 | 0 | `directions/docs.html2docx.json` |
| html -> odt | order.html | warn 0 | 0.15 / 3 | 5/3 | 0.80 / 1.00 | 1.00 | 9.70 / 11.71 | images 2>0 | html_images_not_embedded | 5/5 pp, pix 1.07, sim 0.96 | no such route | 5/3 pp, pix 10.60, sim 0.85 | 0 | `extra/directions/docs.html2odt.json` |
| html -> rtf | order.html | warn 0 | 0.13 / 3 | 5/2 | 0.78 / 1.00 | 1.00 | 12.43 / 15.19 | images 2>0 | html_images_not_embedded | unavail.: office batch failed: timeout | no such route | 5/3 pp, pix 9.61, sim 0.87 | 0 | `extra/directions/docs.html2rtf.json` |
| html -> html | order.html | ok 0 | 0.14 / 3 | - | - | - | - | images 2>0 | - | same format | no such route | ok | images 2>0 | `extra/directions/docs.html2html.json` |
| html -> md | order.html | warn 0 | 0.20 / 3 | - | - | - | - | images 2>0, merged 4>0 | merged_cells_flattened,plain_text_layout | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/docs.html2md.json` |
| html -> txt | order.html | warn 0 | 0.14 / 3 | - | 0.58 / 1.00 | - | - | tables 2>0, images 2>0, merged 4>0 | merged_cells_flattened,plain_text_layout,tables_to_text | sim 1.00 | no such route | sim 0.58 | 0 | `extra/directions/docs.html2txt.json` |
| html -> pdf | order.html | warn 0 | 0.20 / 3 | 5/2 | 0.89 / 1.00 | 1.00 | 13.38 / 14.01 | none | html_images_not_embedded | 5/5 pp, pix 0.00, sim 1.00 | no such route | 5/3 pp, pix 12.95, sim 0.58 | 0 | `extra/directions/docs.html2pdf.json` |
| html -> png | order.html | warn 0 | 3.56 / 20 | 5/2 | - | - | 12.26 / 12.39 | none | html_images_not_embedded | no png export | no such route | n/a | 0 | `extra/directions/docs.html2png.json` |
| md -> docx | order.md | ok 0 | 0.16 / 2 | 2/2 | 0.84 / 1.00 | 1.00 | 8.72 / 10.87 | none | - | no md reader | no such route | 2/2 pp, pix 6.40, sim 1.00 | 0 | `directions/docs.md2docx.json` |
| md -> odt | order.md | ok 0 | 0.10 / 1 | 2/2 | 0.98 / 1.00 | 1.00 | 9.13 / 10.75 | none | - | no md reader | no such route | 2/2 pp, pix 5.74, sim 1.00 | 0 | `extra/directions/docs.md2odt.json` |
| md -> rtf | order.md | ok 0 | 0.12 / 5 | 2/2 | 0.78 / 1.00 | 1.00 | 8.26 / 12.76 | none | - | no md reader | no such route | 2/1 pp, pix 4.30, sim 0.00 | 0 | `extra/directions/docs.md2rtf.json` |
| md -> html | order.md | ok 0 | 0.11 / 5 | - | - | - | - | none | - | no md reader | no such route | ok | 0 | `extra/directions/docs.md2html.json` |
| md -> md | order.md | ok 0 | 0.09 / 5 | - | - | - | - | none | - | no md reader | no such route | unavail.: x2t error -7 | 0 | `extra/directions/docs.md2md.json` |
| md -> txt | order.md | ok 0 | 0.10 / 5 | - | - | - | - | tables 2>0 | - | no md reader | no such route | ok | tables 2>0 | `extra/directions/docs.md2txt.json` |
| md -> pdf | order.md | ok 0 | 0.13 / 6 | 2/2 | 0.79 / 1.00 | 1.00 | 11.19 / 13.69 | none | - | no md reader | no such route | 2/2 pp, pix 4.78, sim 0.59 | 0 | `directions/docs.md2pdf.json` |
| md -> png | order.md | ok 0 | 0.40 / 18 | 2/2 | - | - | 10.69 / 13.23 | none | - | no md reader | no such route | n/a | 0 | `extra/directions/docs.md2png.json` |
| txt -> docx | order.txt | warn 0 | 0.41 / 3 | 2/5 | 0.88 / 1.00 | 1.00 | 4.64 / 8.06 | none | text_transcoded | 2/2 pp, pix 0.67, sim 0.88 | no such route | 2/3 pp, pix 1.92, sim 0.48 | pagination 2>5 | `directions/docs.txt2docx.json` |
| txt -> odt | order.txt | warn 0 | 0.12 / 5 | 2/4 | 0.88 / 1.00 | 1.00 | 6.80 / 11.01 | none | text_transcoded | 2/2 pp, pix 3.00, sim 0.88 | no such route | 2/3 pp, pix 3.55, sim 0.48 | pagination 2>4 | `extra/directions/docs.txt2odt.json` |
| txt -> rtf | order.txt | warn 0 | 0.09 / 5 | 2/3 | 0.88 / 1.00 | 1.00 | 8.06 / 11.72 | none | text_transcoded | 2/2 pp, pix 0.00, sim 1.00 | no such route | 2/1 pp, pix 6.83, sim 0.00 | pagination 2>3 | `extra/directions/docs.txt2rtf.json` |
| txt -> html | order.txt | warn 0 | 0.10 / 4 | - | 0.59 / 1.00 | - | - | none | text_transcoded | sim 0.59 | no such route | sim 1.00 | 0 | `extra/directions/docs.txt2html.json` |
| txt -> md | order.txt | warn 0 | 0.12 / 5 | - | - | - | - | none | text_transcoded | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/docs.txt2md.json` |
| txt -> txt | order.txt | warn 0 | 0.11 / 4 | - | - | - | - | none | text_transcoded | same format | no such route | ok | 0 | `extra/directions/docs.txt2txt.json` |
| txt -> pdf | order.txt | warn 0 | 0.20 / 5 | 2/5 | 0.88 / 1.00 | 1.00 | 6.81 / 11.94 | none | text_transcoded | 2/2 pp, pix 0.00, sim 1.00 | no such route | 2/2 pp, pix 4.49, sim 0.48 | pagination 2>5 | `extra/directions/docs.txt2pdf.json` |
| txt -> png | order.txt | warn 0 | 0.58 / 29 | 2/5 | - | - | 6.87 / 12.05 | none | text_transcoded | no png export | no such route | n/a | 0 | `extra/directions/docs.txt2png.json` |
| xlsx -> pdf | hrm_staff.xlsx · hrm_timesheet.xlsx · 02-table-rich.xlsx | warn 0 · warn 0 · warn 0 | 0.15 / 3 · 0.15 / 3 · 0.25 / 3 | 4/4 · 1/1 · 1/1 | 0.85 / 1.00 · 0.56 / 1.00 · 0.71 / 1.00 | 1.00 · 1.00 | 4.07 / 5.85 · 3.61 / 3.61 · 1.20 / 1.20 | none · none · none | pdf_en_us · pdf_en_us · pdf_en_us | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | 4/4 pp, pix 3.36, sim 0.67 · 1/1 pp, pix 2.73, sim 1.00 · 1/1 pp, pix 0.96, sim 0.84 | 4/1 pp, pix 4.20, sim 0.61 · 1/1 pp, pix 2.86, sim 0.51 · 1/1 pp, pix 0.46, sim 0.71 | 0 · 0 · 0 | `directions/sheets.xlsx2pdf.json` |
| xlsx -> xlsx | hrm_staff.xlsx | ok 0 | 0.83 / 6 | 4/4 | 0.97 / 1.00 | 1.00 | 2.72 / 4.93 | none | - | same format | 4/4 pp, pix 1.24, sim 0.93 | 4/4 pp, pix 0.00, sim 1.00 | 0 | `extra/directions/sheets.xlsx2xlsx.json` |
| xlsx -> csv | hrm_staff.xlsx · hrm_timesheet.xlsx | warn 0 · warn 0 | 0.18 / 2 · 0.27 / 1 | - | cells 55 equal + 24 locale-only / 79 (vs office) · cells 44 equal + 5 locale-only / 49 (vs office) | - | - | sheets 3>1, formulas 75>0 · formulas 10>0 | csv_one_sheet_en_us · csv_one_sheet_en_us | csv · csv | no such route · no such route | csv · csv | 0 · 0 | `directions/sheets.xlsx2csv.json` |
| xls -> pdf | hrm_staff.xls | warn 0 | 0.53 / 5 | 4/3 | 0.86 / 0.93 | 1.00 | 4.92 / 6.42 | none | pdf_en_us,upstream,xls_pdf_chart_page | 4/4 pp, pix 0.00, sim 1.00 | refuses legacy | 4/1 pp, pix 5.62, sim 0.61 | 0 | `directions/sheets.xls2pdf.json` |
| xls -> xlsx | hrm_staff.xls · hrm_timesheet.xls | warn 0 · warn 0 | 5.29 / 5 · 0.09 / 3 | 4/3 · 1/1 | 0.93 / 0.93 · 0.90 / 1.00 | 1.00 · 1.00 | 4.61 / 5.37 · 3.17 / 3.17 | none · none | upstream · upstream | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | refuses legacy · refuses legacy | 4/4 pp, pix 2.22, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | 0 · 0 | `directions/sheets.xls2xlsx.json` |
| xls -> csv | hrm_staff.xls | warn 0 | 0.13 / 3 | - | cells 55 equal + 24 locale-only / 79 (vs office) | - | - | none | csv_one_sheet_en_us,upstream | csv | no such route | csv | 0 | `extra/directions/sheets.xls2csv.json` |
| xlsb -> pdf | hrm_staff.xlsb | warn 0 | 0.19 / 3 | 4/3 | 0.47 / 0.84 | 1.00 | 4.67 / 5.81 | none | pdf_en_us,upstream | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/1 pp, pix 4.24, sim 0.49 | 0 | `extra/directions/sheets.xlsb2pdf.json` |
| xlsb -> xlsx | hrm_staff.xlsb · hrm_timesheet.xlsb | warn 0 · warn 0 | 0.08 / 3 · 0.08 / 3 | 4/3 · 1/1 | 0.56 / 0.84 · 0.46 / 0.95 | 1.00 · 1.00 | 4.56 / 5.43 · 3.23 / 3.23 | none · none | upstream · upstream | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | no such route · no such route | 4/4 pp, pix 3.59, sim 0.65 · 1/1 pp, pix 0.69, sim 0.89 | 0 · 0 | `directions/sheets.xlsb2xlsx.json` |
| xlsb -> csv | hrm_staff.xlsb | warn 0 | 0.10 / 3 | - | cells 55 equal + 0 locale-only / 79 (vs office) | - | - | none | csv_one_sheet_en_us,upstream | csv | no such route | csv | 0 | `extra/directions/sheets.xlsb2csv.json` |
| ods -> pdf | hrm_staff.ods | warn 0 | 0.18 / 3 | 4/3 | 0.48 / 0.82 | 1.00 | 4.75 / 6.05 | none | pdf_en_us,upstream | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/1 pp, pix 5.29, sim 0.43 | 0 | `extra/directions/sheets.ods2pdf.json` |
| ods -> xlsx | hrm_staff.ods · hrm_timesheet.ods | warn 0 · warn 0 | 0.10 / 3 · 0.08 / 3 | 4/3 · 1/1 | 0.57 / 0.82 · 0.29 / 0.90 | 1.00 · 1.00 | 4.64 / 5.68 · 3.36 / 3.36 | formulas 75>0, merged 4>2 · formulas 10>0, merged 2>1 | upstream · upstream | 4/4 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 | no such route · no such route | 4/4 pp, pix 4.04, sim 0.74 · 1/1 pp, pix 3.99, sim 0.24 | 0 · 0 | `directions/sheets.ods2xlsx.json` |
| ods -> csv | hrm_staff.ods | warn 0 | 0.15 / 3 | - | cells 55 equal + 0 locale-only / 79 (vs office) | - | - | sheets 3>1, formulas 75>0 | csv_one_sheet_en_us,upstream | csv | no such route | csv | 0 | `extra/directions/sheets.ods2csv.json` |
| csv -> pdf | staff.csv | ok 0 | 0.20 / 3 | 1/1 | 0.53 / 1.00 | 1.00 | 5.17 / 5.17 | none | - | 1/1 pp, pix 0.00, sim 1.00 | no such route | 1/1 pp, pix 4.89, sim 0.50 | 0 | `extra/directions/sheets.csv2pdf.json` |
| csv -> xlsx | staff.csv | ok 0 | 0.11 / 1 | 1/1 | 0.83 / 1.00 | 1.00 | 4.29 / 4.29 | none | - | 1/1 pp, pix 0.00, sim 1.00 | no such route | 1/1 pp, pix 4.45, sim 0.50 | 0 | `extra/directions/sheets.csv2xlsx.json` |
| csv -> csv | staff.csv | ok 0 | 0.12 / 0 | - | cells 74 equal + 0 locale-only / 79 (vs source fixture) | - | - | none | - | same format | no such route | ok | 0 | `extra/directions/sheets.csv2csv.json` |
| tsv -> pdf | staff.tsv | ok 0 | 0.19 / 3 | -/1 | - | - | - | none | - | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/sheets.tsv2pdf.json` |
| tsv -> xlsx | staff.tsv | ok 0 | 0.10 / 4 | -/1 | - | - | - | none | - | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/sheets.tsv2xlsx.json` |
| tsv -> csv | staff.tsv | ok 0 | 0.11 / 3 | - | cells 74 equal + 0 locale-only / 79 (vs source fixture) | - | - | none | - | cannot write it | no such route | unavail.: x2t error -7 | 0 | `extra/directions/sheets.tsv2csv.json` |
| pptx -> pdf | hrm_deck.pptx · 01-shapes-notes.pptx · 02-languages.pptx | ok 0 · ok 0 · ok 0 | 12.03 / 74 · 1.40 / 18 · 0.23 / 73 | 5/5 · 1/1 · 2/2 | 1.00 / 1.00 · 1.00 / 1.00 · 1.00 / 1.00 | 1.00 · 1.00 | 0.61 / 1.67 · 0.02 / 0.02 · 0.37 / 0.75 | none · none · none | - · - · - | 5/5 pp, pix 0.00, sim 1.00 · 1/1 pp, pix 0.00, sim 1.00 · 2/2 pp, pix 0.00, sim 1.00 | 5/5 pp, pix 1.96, sim 1.00 · 1/1 pp, pix 0.45, sim 1.00 · 2/2 pp, pix 1.10, sim 1.00 | 5/5 pp, pix 1.13, sim 0.47 · 1/1 pp, pix 0.39, sim 0.90 · 2/2 pp, pix 0.68, sim 1.00 | 0 · 0 · 0 | `directions/slides.pptx2pdf.json` |
| pptx -> png | hrm_deck.pptx | ok 0 | 4.50 / 33 | 5/5 | - | - | 0.57 / 1.55 | none | - | 5/0 pp | 5/1 pp, pix 3.82 | n/a | 0 | `directions/slides.pptx2png.json` |
| pptx -> pptx | hrm_deck.pptx | ok 0 | 0.43 / 8 | 5/5 | 1.00 / 1.00 | 1.00 | 0.00 / 0.00 | none | - | same format | 5/5 pp, pix 1.31, sim 1.00 | 5/5 pp, pix 0.00, sim 1.00 | 0 | `extra/directions/slides.pptx2pptx.json` |
| pdf -> docx | p2d_latn.pdf · p2d_cyrl.pdf · p2d_ru.pdf · p2d_overimage.pdf · p2d_widths.pdf | ok 0 · ok 0 · ok 0 · ok 0 · ok 0 | 4.90 / 11 · 0.39 / 2 · 0.24 / 7 · 0.40 / 11 · 0.14 / 6 | 2/3 · 2/3 · 2/3 · 1/1 · 2/2 | 0.99 / 1.00 · 0.98 / 1.00 · 0.98 / 1.00 · 0.98 / 1.00 · 0.76 / 0.97 | 1.00 · 1.00 · 1.00 · 0.96 | 12.95 / 20.33 · 12.28 / 19.69 · 11.77 / 17.97 · 6.20 / 6.20 · 3.96 / 5.95 | none · none · none · none · none | - · - · - · - · - | Word PDF reflow hangs · Word PDF reflow hangs · Word PDF reflow hangs · Word PDF reflow hangs · Word PDF reflow hangs | no such route · no such route · no such route · no such route · no such route | ok · ok · ok · ok · ok | not checked by run.py · not checked by run.py · not checked by run.py · not checked by run.py · not checked by run.py | `directions/pdf.pdf2docx.json` |
| pdf -> txt | p2d_latn.pdf · p2d_cyrl.pdf | ok 0 · ok 0 | 0.13 / 5 · 0.12 / 5 | - | 0.94 / 1.00 · 0.92 / 1.00 | 1.00 · 1.00 | - | none · none | - · - | n/a · n/a | n/a · n/a | n/a · n/a | 0 · 0 | `directions/pdf.pdf2txt.json` |
| pdf -> png | order.pdf · p2d_cyrl.pdf | ok 0 · ok 0 | 0.42 / 30 · 0.32 / 29 | 4/4 · 2/2 | - | - | 0.00 / 0.00 · 0.00 / 0.00 | none · none | - · - | n/a · n/a | n/a · n/a | n/a · n/a | 0 · 0 | `directions/pdf.pdf2png.json` |

#### Not rows of `formats` (options, PDF operations, failure cases, retired inputs)

| Direction | Fixture(s) | Status, exit | Time s / peak MB | Pages ref/ours | Text sim / letters kept | Cyrillic kept | Pixel diff % mean / max | Structure lost (counts) | Warnings | Office | LibreOffice | x2t | Silent | JSON |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| docs.dot2docx | order.dot | warn 0 | 0.10 / 3 | 4/4 | 0.89 / 1.00 | 1.00 | 7.58 / 12.19 | none | doc_import_loss | 4/4 pp, pix 0.00, sim 1.00 | no such route | 4/4 pp, pix 0.56, sim 1.00 | 0 | `directions/docs.dot2docx.json` |
| docs.translit.docx | uz108-latn.docx · p2d_latn.docx | ok 0 · ok 0 | 6.27 / 19 · 0.10 / 3 | 108/113 · 2/2 | 0.00 / 0.01 · 0.11 / 0.05 | 1.00 | 21.04 / 24.53 · 9.78 / 13.67 | none · none | - · - | n/a · n/a | n/a · n/a | n/a · n/a | 0 · 0 | `directions/docs.translit.docx.json` |
| pdf.combine | p2d_latn.pdf+p2d_cyrl.pdf+order.pdf | ok 0 | 1.69 / 10 | 8/8 | - | - | hash-equal 8/8 | - | - | n/a | n/a | n/a | 0 | `directions/pdf.combine.json` |
| pdf.edge | empty.pdf · truncated.pdf · scan.pdf | failed 9 · warn 0 · warn 0 | 0.05 / 0 · 0.52 / 8 · 0.15 / 6 | - | - | - | - | docx: failed 9, text no; txt: failed 9, text no · docx: warn 0, text yes; txt: warn 0, text yes · docx: warn 0, text no; txt: warn 0, text no | the file is empty (0 bytes), so there is nothing to convert · pdf_damaged,upstream · empty_text | n/a · n/a · n/a | n/a · n/a · n/a | n/a · n/a · n/a | 0 · 0 · 0 | `directions/pdf.edge.json` |
| pdf.extract | order.pdf | ok 0 | 0.29 / 9 | 3/3 | - | - | hash-equal 3/3 | - | - | n/a | n/a | n/a | 0 | `directions/pdf.extract.json` |
| pdf.split | order.pdf | ok 0 | 0.25 / 10 | 4/4 | - | - | hash-equal 4/4 | - | - | n/a | n/a | n/a | 0 | `directions/pdf.split.json` |
| sheets.translit.xlsx | hrm_staff.xlsx · 02-table-rich.xlsx | warn 0 · warn 0 | 2.82 / 6 · 0.69 / 6 | 4/4 · 1/1 | 0.88 / 0.91 · 0.43 / 0.31 | 1.00 | 2.84 / 4.95 · 1.88 / 1.88 | none · none | translit_formula_text · translit_formula_text | n/a · n/a | n/a · n/a | n/a · n/a | 0 · 0 | `directions/sheets.translit.xlsx.json` |
| slides.ppt2pdf | hrm_deck.ppt | failed 3 | 0.07 / 0 | - | - | - | - | none | .ppt files are not supported: re-save the file as .pptx in PowerPoint  | 5/5 pp, pix 0.00, sim 1.00 | refuses legacy | 5/5 pp, pix 1.13, sim 0.47 | 0 | `directions/slides.ppt2pdf.json` |
| slides.ppt2pptx | hrm_deck.ppt | failed 3 | 0.06 / 0 | - | - | - | - | none | .ppt files are not supported: re-save the file as .pptx in PowerPoint  | 5/5 pp, pix 0.00, sim 1.00 | refuses legacy | 5/5 pp, pix 0.80, sim 1.00 | 0 | `directions/slides.ppt2pptx.json` |
| slides.translit.pptx | hrm_deck.pptx · 02-languages.pptx | ok 0 · ok 0 | 4.62 / 9 · 0.61 / 7 | 5/5 · 2/2 | 0.82 / 0.82 · 0.05 / 0.05 | 1.00 · 1.00 | 0.76 / 2.87 · 1.37 / 2.52 | none · none | - · - | n/a · n/a | n/a · n/a | n/a · n/a | 0 · 0 | `directions/slides.translit.pptx.json` |

## 4. Warnings against losses

**CI set (`summary.json`): 1 silent loss**, `docs.txt2docx|order.txt|pagination` (2 -> 5 pages), which is D1, deferred
to Phase 8. `python -I scripts/fidelity/silent_baseline.py check E:/office-target/p3/fidelity/summary.json`:
`silent losses: 1 now, 1 in the baseline, 0 new, 0 gone`.

**Not in the CI set: 22 silent losses in 12 directions** (`extra/summary.json`, `loss_without_warning`), plus 2 that
`run.py` does not check. Each is now in `defects.md`:

| Defect | Directions (fixture) | What is lost, with no warning |
|---|---|---|
| D2 | odt -> odt (order.odt), rtf -> rtf (order.rtf) | **no warning at all**: comments 1 -> 0, footnotes 1 -> 0, headers/footers 4 -> 0 (odt) and 2 -> 0 (rtf). The same losses are warned for odt -> docx, rtf -> docx and the others. Cause: the reader-loss rows are `cross = true`, and those are skipped when the target is the input's format (`services/convert/src/losses.rs:48`) |
| D3 | rtf -> odt; rtf -> html, md, txt | rtf -> odt: headers/footers 2 -> 0. rtf -> html/md/txt: tracked changes 1 -> 0 (E3 lists docx only). rtf -> html: headers/footers 2 -> 0 |
| D4 | html -> html; md -> txt | html -> html: pictures 2 -> 0 (`html_images_not_embedded` missing). md -> txt: tables 2 -> 0 (`tables_to_text` missing) |
| D1 (wider) | txt -> odt, rtf, pdf, png | pages 2 -> 4, 3, 5 and 5. Same cause as D1 (the docx defaults of `docs-cli` for text), same decision |
| D5 | pdf -> docx, native (p2d_widths.pdf) | a table column and two cell texts (18 of 480 Cyrillic letters). `run.py` does not loss-check pdf2docx outputs |
| D6 | csv, tsv -> xlsx, pdf, csv (staff.csv/.tsv) | dates `01.03.2019` read as 3 January, `11.09.2019` as 9 November. `3500000,0` stored as text. No warning (csv input has one sheet, so `csv_one_sheet_en_us` is not raised). Cause measured: `convert` does not set `GRIDCRAFT_NUM_LOCALE`. With `ru` or `uz-latn` the same file gives 1 March and the number 3500000 (`csv-locale-probe.json`) |

Every other measured loss in the 117 cases is matched by a warning: E1-E12 of `defects.md` section 2, plus
`text_transcoded` (cp1251 text in), `doc_import_loss`, `pdf_en_us` and `csv_one_sheet_en_us`. The edge cases behave
(`pdf.edge`): the 0-byte file is refused with exit 9, the truncated file gives text plus `pdf_damaged`, and the scan
gives `empty_text`. None is an empty success.

**Harness gaps found** (all closed on 2026-10-21 in `scripts/fidelity`: pdf2docx outputs are loss-checked, `csv_compare` reads each file with its own
delimiter and the source fixture is the csv to csv reference, DERIVED names the loss kind, an Office hang costs one file, the 52 rows are in
`fid/directions.py`, and x2t is documented with `ALLOW_PRIVATE_IP_ADDRESS`; the list is kept as found):
1. pdf -> docx outputs are not loss-checked (this is how D5 got past it).
2. `csv_compare` assumes `,` (section 3.2).
3. csv -> csv and tsv rows have no reference.
4. A text, page or Cyrillic difference counts as explained by *any* loss warning on the same case (`DERIVED` in
   `fid/structure.py`). That is lenient: a second, unwarned text loss on the same file would pass.
5. x2t needs `ALLOW_PRIVATE_IP_ADDRESS=true`.
6. One Word hang takes down every job of its batch (`office_ref.ps1` writes the result only at the end).
7. 52 of the 80 `formats` rows are not in `fid/directions.py`, so CI does not see D2-D4. Merging the
   `fidelity_extra.py` table into `fid/directions.py` and re-seeding the baseline would put them under the ratchet.

## 5. The corpus run: not delivered

The owner's anonymised `corpus/` (B-14, O9) has not arrived, so no fidelity number in this report is about a real HRM
document. What that means:
- All inputs come from **one producer** (Office 16 on this PC, plus our generators). Real HRM files come from many
  versions of Word and Excel, LibreOffice, 1C, browsers and scanners. Import defects of other producers (the pdf2docx
  "one paragraph per line" claim is one) cannot show up here.
- The sets are small: 1-5 fixtures per direction. One fixture can hide or exaggerate a defect (D5 was found by one
  fixture).
- The O8 choice (section 2.4) and the "no silent loss" claim hold for this set only.
- To re-run when it arrives (artefacts stay on E:, names become ids): the same two commands with
  `--fixtures <anonymised copy>`. The direction tables pick fixtures by name, so a corpus run needs a fixture
  mapping. That is a small WP for the harness owner.

## 6. Conclusions

**Better than the references (on this set):**
- pptx -> pdf and pptx -> png are within 0.02-0.61 % of PowerPoint, better than LibreOffice (0.45-1.96 %) and x2t
  (0.39-1.13 %).
- docx -> pdf: mean pixel diff 0.41 % (`hrm_order`) and 1.50 % (`hrm_letter`), against LibreOffice 0.52 and 3.51 %
  and x2t 0.59 and 3.67 %. Phase 0 measured 6.2 % mean,
  so the font patches paid off.
- md in and out works (Office cannot do it; x2t fails md -> rtf with no text and loses md tables).
- cp1251 text input is transcoded and warned. x2t, given the same cp1251 `order.txt`, loses every Cyrillic letter
  (txt -> docx, odt, rtf, pdf: Cyrillic kept 0.00).
- PDF operations are lossless (combine, split, extract: every page's render hash equals the source page's).
- PDF -> png gives 0.00 % against the source pages.
- PDF -> DOCX works headless. Word 16 cannot do it headless at all.
- Every failure case gives a message or a warning, never an empty success.

**Worse than the references:**
- **odt and rtf output** (from docx, doc, odt, rtf): 6.4-11.7 % pixel diff and often a page more (odt) or less (rtf).
  Office's own conversions reopened give 0.7-5.8 %. x2t's odt is as far off (7.6-10.0 %), but its rtf is not
  (0.7-4.5 %). Headers, footers, footnotes and comments are dropped. That is warned except where D2/D3 apply.
- **Legacy import:** doc -> docx 7.6-10.7 % (x2t 0.6-1.1 %), with comments and tracked changes lost (warned).
  xls -> xlsx loses the chart page (4 -> 3; x2t keeps 4).
- **xlsb and ods import** turn formulas into values and lose number formats (letters kept 0.82-0.84; warned).
- **Sheets PDF/CSV** show en-US numbers and dates (warned; NumLocale is built but `convert` does not use it yet, see D6).
- **html with linked pictures** loses them (5 -> 2 pages, warned; x2t too).
- **txt -> docx/odt/rtf/pdf** run 1-3 pages longer than Word (D1).

**What the owner should know:**
1. The CI ratchet shows 1 silent loss, but **the full `formats` matrix has 6 open defects (D2-D6)**. D6 changes data:
   a ru/uz CSV opened by the converter gets wrong dates, and nothing warns. D5 drops a table column. These two matter
   most for HRM documents. Their fix is in `services/` (D6: set `GRIDCRAFT_NUM_LOCALE` per job, as the region is set for
   paper size; D5: a letter-count guard). It is not part of WP10.
2. O8: native by default once D5 is guarded, the sidecar optional. Or, if the picture should win over editability,
   the sidecar first. Decide again on the corpus.
3. The corpus (O9) is still the largest gap in every claim here.

**For Phase 8 (fidelity):** D1 (text-import defaults, a wordcraft patch), odt/rtf writers (headers, footnotes,
comments, the page drift), the `.doc` importer (comments, tracked changes, the paragraph inflation), the `.xls` chart
and conditional formats, xlsb/ods number formats, native PDF -> DOCX paragraph spacing and narrow columns, the harness
gaps of section 4, and the corpus run.
