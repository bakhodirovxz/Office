## Observations (manual, from the side-by-side images and the JSON)

Classification of what was actually observed (release builds, synthetic set):

- **text / fonts (gridcraft)**: Cyrillic in Excel sheets is rendered as `?` in Craft PDFs (inferred: the substitute font used for the sheet grid has no Cyrillic glyphs; the `?` is in the PDF text layer and in the raster); `hrm_timesheet` title and all names show `??????`. Word and PowerPoint
  Cyrillic render correctly. This is the single most serious finding for the HRM use case.
- **formatting (gridcraft)**: date and percent cells show `####` / `###` (column too narrow after font substitution);
  conditional-format and chart are dropped for `.xls` input (documented importer warning: fonts, fills, borders, charts,
  pictures, conditional formats are not imported from `.xls`).
- **formatting / layout (wordcraft)**: with identical page counts, 5-11% of pixels differ per page; the diff images show a
  small constant vertical offset of the body (roughly 10 px by eye; likely header/line-height metrics) with Times New Roman substituted; tables,
  merged cells, numbering, footnote, header/footer PAGE/NUMPAGES fields, image, text box and section/landscape page all survive.
- **comments / track changes (legacy import)**: `.doc` -> docx round trip loses the comment and the tracked change
  (1 -> 0 each); the `.docx` path keeps both. Paragraph count after `.doc` import is inflated (99 -> 113, 46 -> 53).
- **bookmarks**: `_hrm_desc_` survives in all four Word files (docx and doc, direct round trip), verified by reopening in Office.
- **legacy-import (deckcraft)**: `.ppt` is not readable at all ("not a presentation DeckCraft can read").
- **pagination**: page counts match in every file where both sides produced a PDF (Word 4/4 and 1/1, Excel 4/4 and 1/1 with
  `sheets:all`; `.xls` 4 vs 3 pages because the chart is lost), PowerPoint 5/5.
- **PowerPoint**: 5/5 slides, shapes, table, picture, notes survive; pixel diff is below 2% (slides 1-2 differ most; the generated chart slide is not counted as a chart by the Office inspector, so chart survival is NOT measured here).
- **Office reopen**: every Craft-saved docx/xlsx/pptx opened in Office without error (Excel silent auto-repair would not be seen).

Limitations: only 10 synthetic files; fonts on the Craft side are bundled substitutes, so some pixel diff is font metrics, not
layout bugs; Excel/PDF pagination is compared by page index; LibreOffice and x2t references not run; HEAD builds not run.

Harness artefacts to ignore: `footnotes None->0` and `sections 0->1` on hrm_letter.docx are Office-inspector quirks on the original file, not Craft losses. Text similarity for Excel (0.34-0.49) is dominated by the `?` Cyrillic and `####` cells, plus differing extraction order.
