# Phase 3 WP1: direction census (2026-10-21, branch phase-3)

Every PLAN section 1 direction run once with the raw domain CLIs (`E:/office-target/p1/{docs,sheets,slides,pdf}/release/*-cli.exe`,
built at HEAD by WP0) by `scripts/fidelity/census.ps1` (`-Phase prep` builds the inputs, `-Phase run` measures). Inputs are
synthetic only: Office 16 COM for `.dot .rtf .odt .xlsb .ods` and the Word-made PDFs, the Phase 0 synthetic set
(`.doc .xls .ppt` and the docx/xlsx/pptx), the Phase 2 translit fixtures, generated 5k/50k/300k-row workbooks and an
image-only PDF. Everything is under `E:/office-target/p3/census/` (not in git); raw numbers are in its `results.json`.
One run per direction on a busy 12-core box (other agents were building and using Office), so times carry noise of tens
of percent. Peak RAM is the working set polled every 20 ms (`n/s` = the process ended before the first sample, under
about 30 ms). `WORDCRAFT_REGION=en-US` was set.

**Result: 77 runs, 74 PASS, 3 FAIL.** The 3 FAIL are `.ppt -> pptx/pdf/png` ("not a presentation DeckCraft can read",
rc 1), as expected (sidecar, WP6/O2). (A first run also failed `--sheet` because the test used a wrong sheet name; with
the right name it passes.) Slowest: pdf render of 108 pages in one script (12.6-16.3 s, 0.12-0.15 s/page, PNG
encoding), then sheets with 300k rows / 17 MB (8.9 s xlsx->xlsx, 6.3 s ->csv). Highest peak RAM: sheets 300k rows
315 MB (18.6x the file), slides 96 MB, docs 75 MB (110 pages), pdf 26 MB. Nothing came near the section 2.5 memory caps.

## Section 1 rows, status

| Section 1 row | Status | Notes |
|---|---|---|
| 1.1 docx -> odt/rtf/html/md/txt | PASS (5/5) | 0.07-0.63 s, <= 7 MB |
| 1.1 odt/rtf/html/md/txt -> docx | PASS (5/5) | Word-made odt/rtf; html/md/txt from the previous row. 0.07-0.78 s. The html round trip of the 4-page order gives 3 pages (html has no pagination) |
| 1.1 .doc/.dot -> docx | PASS, **no warning printed** | rc 0, empty stderr. The source comment (1) is gone after import (`info.comments` 0) with no message: the converter must add the warning text itself (as the plan says) |
| 1.1 docx/doc/odt/rtf/md -> pdf | PASS (5/5 + 110-page doc) | page counts 4/4/4/4 for docx/doc/odt/rtf; md 2; the 110-page Latin Uzbek doc 1.3 s, 75 MB |
| 1.1 docx -> png | PASS, see F4 | `convert` writes page 1 only |
| 1.1 docx (+translit) -> docx/odt/rtf | PASS (4/4) | 110-page doc: 0.4-1.1 s, <= 26 MB |
| 1.2 xls/xlsb/ods -> xlsx | PASS (4/4) | each prints a `Warning:` line on **stderr** (xls: fonts, fills, borders, charts, pictures, comments not imported; xlsb/ods: data only, formulas become values). F8 confirmed: capture stderr |
| 1.2 xlsx (any input) -> pdf | PASS (4/4) | xlsx 4 pages, timesheet 1, **xls 3 pages (chart lost)**, table-rich 1 |
| 1.2 xlsx -> csv (+tsv, json, html) | PASS (4/4) and `--sheet Tabel` PASS | `--sheet` with a wrong name: rc 1, "no sheet named `X`" (generic exit 1) |
| 1.2 xlsx -> xlsx / pdf (+translit) | PASS (3/3) | 0.03-0.1 s |
| 1.2 large workbooks | PASS | see scaling below |
| 1.3 pptx -> pdf | PASS (3/3) | 5 / 1 / 2 pages, 0.2-0.4 s, <= 96 MB |
| 1.3 pptx -> png (all) | PASS | `slide-01.png` .. `slide-05.png`, 1920x1080 |
| 1.3 pptx (+translit) -> pptx/pdf | PASS (2/2) | |
| 1.3 .ppt -> pptx/pdf/png | **FAIL (3/3), expected** | rc 1, stderr "not a presentation DeckCraft can read" |
| 1.4 combine / split / extract | PASS | 8 pp combined; 110 pp split every 10 in 1.0 s; extract `1,3,5-8` gives 6 pp |
| 1.4 text | PASS, but **scan = rc 0 with empty stdout** | an image-only PDF gives nothing and no warning: the converter must detect it (section 1.4) |
| 1.4 pdf -> png | PASS | one page 0.13 s (144 dpi); 108 pages in one `run --script` 12.6-16.3 s |
| 1.4 pdf -> docx (native, F3) | PASS (rc 0, 0.08 s, 12 MB) | quality: see F3. The pptx, html and rtf targets also PASS |
| 1.4 pdf -> docx (+translit) | PASS | chain via `docs-cli run` on the native docx, 0.10 s |

## F4: docx -> png, pdf route vs `docs-cli render`

4-page Word-made file (`hrm_order.docx`, page 4 landscape). `docs-cli render --page N` (default scale 2 = 1191x1684)
against `docs-cli convert -> pdf` followed by `pdf-cli run --script` with one `page_render` per page at **144 dpi** (same
pixel size). Metric as in `spikes/fidelity/run.py` (grey, blur 1, |diff| > 48): page 1 **0.000 %**, page 2 **0.000 %**,
page 3 **0.121 %**, page 4 (landscape) **0.000 %**. The PNG bytes differ (encoder) but the pictures are practically the
same, and `docs-cli convert x.docx x.png` equals `render --page 1` (0.000 %). **Finding: the pdf route is equivalent to
wordcraft's own renderer; use it for "all pages"** (one pdf-cli process for N pages, and the pdf comes for free). Time for
these 4 pages: 4 reopens with `render` about 0.9 s in total, pdf route 1.76 s (docx->pdf plus the script), so the route
pays off only with many pages.

## F3: pdfcraft native pdf -> docx on 2 Word-made PDFs

Word-made (`ExportAsFixedFormat`) 2-page PDFs with the same layout (title, 5x3 paragraphs, 4x3 table), Latin and Cyrillic
Uzbek, exported with `doc_open` + `doc_export_office` (0.08 s, 12 MB), compared with the source PDF:

| | Latin | Cyrillic |
|---|---|---|
| word-level text similarity (docx text vs source pdf text) | 1.000 | 1.000 |
| Cyrillic letters kept | n/a | 1860 / 1860 |
| paragraphs in the docx (Word original: 31) | 17 | **6** |
| tables kept (original 1) | 1 | **0** (table text flattened into paragraphs) |
| pages when the docx is re-rendered (source 2) | 2 | 2 |
| page 1 pixel diff, source pdf vs docx->pdf | 20.7 % | 17.9 % |

All text survives, with the Uzbek apostrophes and the Cyrillic intact. Lines are merged into real paragraphs (5-10
non-blank lines in the docx against 37-42 in the pdf text), so the "one paragraph per line" defect of pdf2docx on Cyrillic
is not present. Weak spots: the Cyrillic table is lost, and 18-21 % of the pixels differ. This is one tiny synthetic
sample; the owner's choice (O8) needs the larger WP9 set.

## Large workbooks (`sheets-cli convert`)

| file | MB | xlsx->xlsx: s / peak MB | ->csv: s / peak MB |
|---|---|---|---|
| big5k (5k rows x 8) | 0.27 | 0.17-0.23 / 11 | 0.12-0.14 / 9 |
| big50k | 2.7 | 1.5-1.6 / 54 | 1.0-1.1 / 45 |
| big300k | 16.2 | 8.8-8.9 / 315 | 6.1-6.3 / 244 |

Linear: about **0.55 s/MB and 19 MB of RAM per MB** for plain cells (the older formula-heavy w80 file: 10x). xlsx->pdf
of big5k (117 pages) takes 0.6 s.

## Timeouts (section 2.5): proposal

Rule from the plan: 3x the measured p95, rounded. Measured p95 over all runs of a tool: docs 0.53 s, sheets 6.3 s (the
300k file), slides 0.40 s, pdf 16.3 s (the 108-page render script). A sub-second p95 would give a base timeout of 1-2 s
that a loaded host or a cold start would trip, so the proposed base has a **10 s floor**; the per-MB and per-page terms
are 3x the worst measured rate, rounded up. Defaults for the `convert` crate (the 15 min cap stays):

| tool | plan default (estimate) | raw 3x p95 base | **proposed default** | evidence for the slope |
|---|---|---|---|---|
| docs | 60 s + 2 s/MB | 2 s | **10 s + 25 s/MB** | worst 7.8 s/MB (110-page text doc, 0.13 MB, 1.04 s); the cost follows pages, not MB |
| sheets (convert) | 60 s + 3 s/MB | 19 s | **10 s + 3 s/MB** | 0.61 s/MB measured (x3 = 1.8); kept at 3 because the earlier realistic w40 opened at 0.9 s/MB |
| sheets (pdf export) | same | | **10 s + 7 s/MB** | big5k to 117 pages: 2.3 s/MB |
| slides | 60 s + 2 s/MB | 2 s | **10 s + 10 s/MB** | slope UNMEASURED: only 0.05 MB decks exist; revisit in WP9 with a large deck |
| pdf | 30 s + 1 s/MB | 49 s | **10 s + 3 s/MB, plus 0.5 s per rendered page** | 1 MB split/text 0.7-0.8 s; render 0.12-0.15 s/page |
| pdf native -> docx | n/a | | **10 s + 1 s/page** | 0.08 s for 2 pages; UNMEASURED beyond that |
| pdf2docx, soffice | 60 s + 3 s/page, 120 s | | **keep** | sidecars, not runnable here (WP5/WP6) |

The memory caps of section 2.5 (docs 3 GB, slides 3 GB, pdf 2 GB, sheets 6 GB) are 10-100x above every measured peak
(max 315 MB): keep them. At 19 MB per MB, 6 GB for sheets covers a file of roughly 300 MB.

## Surprises and caveats

- `.doc` import gives no warning at all while it drops the comment: the loss text is ours to add (WP3).
- The pdf route for docx -> png is pixel-equivalent, so F4 is closed in favour of the pdf route for multi-page output.
- Native pdf -> docx keeps all text but loses the table in the Cyrillic sample.
- Image-only PDF: `pdf-cli text` returns rc 0 and empty stdout; the "no text layer" warning must come from the converter.
- Every error is exit 1 (wrong sheet name, unreadable .ppt): the converter must classify by the stderr text.
- Not covered: password-protected inputs, pptx chart survival, a large pptx/docx/pdf for the per-MB slope.
- Peak sampling misses processes shorter than about 30 ms (shown `n/s`).

## Raw table (one row per run)

`sec` = wall seconds, `peak MB` = peak working set, `in MB` = input size; "N pp" = pages of the produced pdf.

| id | status | rc | sec | peak MB | in MB | notes |
|---|---|---|---|---|---|---|
| docs.docx2odt | PASS | 0 | 0.085 | 7 | 0.036 |  |
| docs.docx2rtf | PASS | 0 | 0.072 | 7 | 0.036 |  |
| docs.docx2html | PASS | 0 | 0.075 | 6 | 0.036 |  |
| docs.docx2md | PASS | 0 | 0.079 | 6 | 0.036 |  |
| docs.docx2txt | PASS | 0 | 0.071 | 6 | 0.036 |  |
| docs.wodt2docx | PASS | 0 | 0.078 | 7 | 0.015 |  |
| docs.wrtf2docx | PASS | 0 | 0.071 | 7 | 0.23 |  |
| docs.html2docx | PASS | 0 | 0.073 | 6 | 0.018 |  |
| docs.md2docx | PASS | 0 | 0.065 | 6 | 0.004 |  |
| docs.txt2docx | PASS | 0 | 0.375 | 6 | 0.003 |  |
| docs.doc2docx | PASS | 0 | 0.354 | 7 | 0.046 |  |
| docs.dot2docx | PASS | 0 | 0.244 | 7 | 0.046 |  |
| docs.letter.doc2docx | PASS | 0 | 0.141 | 6 | 0.035 |  |
| docs.docx2pdf | PASS | 0 | 0.242 | 29 | 0.036 | 4 pp |
| docs.doc2pdf | PASS | 0 | 0.526 | 17 | 0.046 | 4 pp |
| docs.odt2pdf | PASS | 0 | 0.184 | 34 | 0.015 | 4 pp |
| docs.rtf2pdf | PASS | 0 | 0.193 | 28 | 0.23 | 4 pp |
| docs.md2pdf | PASS | 0 | 0.266 | 17 | 0.004 | 2 pp |
| docs.uz108latn2pdf | PASS | 0 | 1.036 | 75 | 0.133 | 110 pp |
| docs.docx2png.convert | PASS | 0 | 0.178 | 39 | 0.036 |  |
| docs.docx2png.render.p1 | PASS | 0 | 0.195 | 39 | 0.036 |  |
| docs.docx2png.render.p2 | PASS | 0 | 0.314 | 40 | 0.036 |  |
| docs.docx2png.render.p3 | PASS | 0 | 0.317 | 39 | 0.036 |  |
| docs.docx2png.render.p4 | PASS | 0 | 0.245 | 39 | 0.036 |  |
| docs.docx2png.pdfroute | PASS | 0 | 0.738 | 28 |  |  |
| docs.translit.uz108.docx | PASS | 0 | 0.25 | 20 | 0.133 |  |
| docs.translit.uz108.odt | PASS | 0 | 0.276 | 23 | 0.133 |  |
| docs.translit.uz108.rtf | PASS | 0 | 0.438 | 25 | 0.133 |  |
| docs.translit.uz_latn | PASS | 0 | 0.31 | 7 | 0.014 |  |
| sheets.xls2xlsx | PASS | 0 | 0.043 | 4 | 0.043 | warn: Warning: Imported from an Excel 97-2003 (.xls) workbook: values, formulas, number formats,... |
| sheets.xlsb2xlsx | PASS | 0 | 0.043 | n/s | 0.016 | warn: Warning: XLSB data-only import: formulas become their saved results and will not recalcula... |
| sheets.ods2xlsx | PASS | 0 | 0.031 | n/s | 0.009 | warn: Warning: ODS data-only import: formulas become their saved results and will not recalculat... |
| sheets.timesheet.xls2xlsx | PASS | 0 | 0.031 | n/s | 0.026 | warn: Warning: Imported from an Excel 97-2003 (.xls) workbook: values, formulas, number formats,... |
| sheets.xlsx2pdf | PASS | 0 | 0.114 | 6 | 0.017 | 4 pp |
| sheets.timesheet2pdf | PASS | 0 | 0.084 | 6 | 0.009 | 1 pp |
| sheets.xls2pdf | PASS | 0 | 0.086 | 6 | 0.043 | 3 pp; warn: Warning: Imported from an Excel 97-2003 (.xls) workbook: values, formulas, number formats,... |
| sheets.tablerich2pdf | PASS | 0 | 0.088 | 6 | 0.01 | 1 pp |
| sheets.xlsx2csv | PASS | 0 | 0.035 | n/s | 0.017 |  |
| sheets.xlsx2tsv | PASS | 0 | 0.033 | n/s | 0.017 |  |
| sheets.xlsx2json | PASS | 0 | 0.039 | n/s | 0.017 |  |
| sheets.xlsx2html | PASS | 0 | 0.034 | n/s | 0.017 |  |
| sheets.xlsx2csv.sheet | PASS | 0 | 0.033 | n/s | 0.009 |  |
| sheets.translit.xlsx | PASS | 0 | 0.037 | n/s | 0.017 | warn: {"command":"tools.translitCyrl","result":{"cells":18,"direction":"cyrl","formulasWithText"... |
| sheets.translit.tablerich | PASS | 0 | 0.034 | n/s | 0.01 | warn: {"command":"tools.translitCyrl","result":{"cells":8,"direction":"cyrl","formulasWithText":... |
| sheets.translit.pdf | PASS | 0 | 0.094 | 7 | 0.017 | 4 pp; warn: {"command":"tools.translitCyrl","result":{"cells":18,"direction":"cyrl","formulasWithText"... |
| sheets.big5k.xlsx2xlsx | PASS | 0 | 0.228 | 11 | 0.272 |  |
| sheets.big5k.xlsx2csv | PASS | 0 | 0.136 | 8 | 0.272 |  |
| sheets.big50k.xlsx2xlsx | PASS | 0 | 1.629 | 54 | 2.692 |  |
| sheets.big50k.xlsx2csv | PASS | 0 | 1.095 | 45 | 2.692 |  |
| sheets.big300k.xlsx2xlsx | PASS | 0 | 8.895 | 315 | 16.155 |  |
| sheets.big300k.xlsx2csv | PASS | 0 | 6.277 | 244 | 16.155 |  |
| sheets.big5k.xlsx2pdf | PASS | 0 | 0.617 | 18 | 0.272 | 117 pp |
| slides.pptx2pdf | PASS | 0 | 0.377 | 83 | 0.049 | 5 pp |
| slides.01-shapes-notes.2pdf | PASS | 0 | 0.402 | 54 | 0.039 | 1 pp |
| slides.02-languages.2pdf | PASS | 0 | 0.221 | 84 | 0.041 | 2 pp |
| slides.pptx2png | PASS | 0 | 0.2 | 40 | 0.049 |  |
| slides.translit.pptx | PASS | 0 | 0.134 | 10 | 0.049 |  |
| slides.translit.pdf | PASS | 0 | 0.246 | 81 | 0.041 | 2 pp |
| slides.ppt2pptx | FAIL | 1 | 0.028 | n/s | 0.316 | deckcraft-cli: E:\office-target\p3\census\in\hrm_deck.ppt: not a prese |
| slides.ppt2pdf | FAIL | 1 | 0.029 | n/s | 0.316 | deckcraft-cli: E:\office-target\p3\census\in\hrm_deck.ppt: not a prese |
| slides.ppt2png | FAIL | 1 | 0.026 | n/s | 0.316 | deckcraft-cli: E:\office-target\p3\census\in\hrm_deck.ppt: not a prese |
| pdf.combine | PASS | 0 | 0.107 | 10 |  | 8 pp |
| pdf.split.every | PASS | 0 | 0.782 | 14 | 0.969 |  |
| pdf.split.before | PASS | 0 | 0.544 | 10 | 0.135 |  |
| pdf.extract | PASS | 0 | 0.215 | 12 | 0.969 | 6 pp |
| pdf.text.latn | PASS | 0 | 0.049 | 10 | 0.087 |  |
| pdf.text.cyrl | PASS | 0 | 0.044 | 10 | 0.084 |  |
| pdf.text.uz108 | PASS | 0 | 0.709 | 14 | 0.969 |  |
| pdf.text.scan | PASS | 0 | 0.036 | 6 | 0.055 | stdout empty, rc 0 |
| pdf.render.p1 | PASS | 0 | 0.172 | 26 | 0.084 |  |
| pdf.render.script108 | PASS | 0 | 16.268 | 23 | 0.969 |  |
| pdf.native2docx.latn | PASS | 0 | 0.107 | 12 | 0.087 |  |
| pdf.native2docx.cyrl | PASS | 0 | 0.131 | 12 | 0.084 |  |
| pdf.native2docx.translit | PASS | 0 | 0.094 | 7 | 0.003 |  |
| pdf.native2pptx | PASS | 0 | 0.133 | 12 | 0.084 |  |
| pdf.native2html | PASS | 0 | 0.108 | 12 | 0.084 |  |
| pdf.native2rtf | PASS | 0 | 0.122 | 12 | 0.084 |  |
