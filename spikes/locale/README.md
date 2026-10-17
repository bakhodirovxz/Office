# WP10 locale spike
- `gen_locale_xlsx.py OUT.xlsx` builds the 24-case workbook (`python -I`, openpyxl).
- `excel_text.ps1 -In X.xlsx -Out Y.tsv` dumps Excel's `.Text` per cell plus typed-input probes (own Excel PID only).
- `nls_values.ps1` prints Windows NLS values for uz-Latn-UZ, uz-Cyrl-UZ, ru-RU, en-US.
- gridcraft: `sheets-cli cat X.xlsx --range B1:B24`, `sheets-cli convert X.xlsx X.csv`,
  `sheets-cli run --in X.xlsx --cmd 'file.exportPdf={"path":"X.pdf"}'`, then pdfplumber text.
- `results_*` are the captured outputs (Excel ru-RU, gridcraft CSV/PDF text, CLDR summary).
