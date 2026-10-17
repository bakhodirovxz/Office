# WP10: locale decimal-separator check (numbers, dates, typed input)

Measured 2026-10-17 on the dev PC (Windows 10, user locale ru-RU, Excel 16 via COM with an English UI, own PID only,
no culture switching). gridcraft: `E:/office-target/p1/sheets/release/sheets-cli.exe` (0.1.0 domain build).
Generator and raw outputs: `spikes/locale/`. The Windows regional group separator here is U+00A0.

## 1. Conclusions
1. **gridcraft ignores every locale.** Number formatting, date names, CSV, PDF and typed-input parsing are all fixed
   en-US. The OS locale is read in exactly one place, `sys_locale::get_locale()` in `ui-egui/src/i18n.rs:103`, and
   only to choose the UI language for labels; it never reaches the formatter.
2. **The workbook's locale tags are discarded.** `[$-419]` and `[$-443]` parse to nothing
   (`numfmt/src/parse.rs:433`, "locale tags... ignored"); only the currency symbol of `[$₽-419]` is kept (`parse.rs:421-427`).
3. **Screen, PDF and CSV are one code path** (`engine/src/display.rs` -> `numfmt`; PDF `engine/src/cmd/print.rs:1067,1084`;
   grid `ui-egui/src/grid.rs:538-551`; CSV `engine/src/io.rs:242`). All print identical text (the PDF text layer equals the
   CSV, verified). I did not drive the egui window; it calls the same two functions.
4. **The spike's `3500000,0` vs `3500000.0` is the whole class of problem.** Excel formats with the OS locale at display
   time, gridcraft hard-codes `.` and `,`. Files are not wrong (they store en-US format codes); the rendering is.
5. **Typed input is a data-integrity risk, not only cosmetics.** Typing `12.10.2026` (12 October, the ru and uz habit)
   stores serial 46366 = **10 December** 2026 (Excel ru: 46307 = 12 October). `3,5` stays text. `1 234,5` stays text.
   `=SUM(1;2)` is a syntax error.

## 2. Measured matrix

`_` in the expected columns is U+00A0. Excel ru-RU is the ground-truth `.Text`. gridcraft screen, PDF and CSV are
identical in every row, so they share one column. "Expected" = what the locale's users should see (Excel behaviour
plus CLDR, section 4).

| Case (format, value 3500000.5 unless stated) | Excel ru-RU | gridcraft screen / PDF / CSV | Expected ru | Expected uz-Latn | Expected uz-Cyrl |
|---|---|---|---|---|---|
| General | `3500000,5` | `3500000.5` | `3500000,5` | `3500000,5` | `3500000,5` |
| General 0.1234567 | `0,1234567` | `0.1234567` | `0,1234567` | same | same |
| `0.0` of 3500000 (the spike) | `3500000,0` | `3500000.0` | `3500000,0` | same | same |
| `#,##0.0` | `3_500_000,5` | `3,500,000.5` | `3_500_000,5` | `3_500_000,5` | `3_500_000,5` |
| `#,##0.00` | `3_500_000,50` | `3,500,000.50` | `3_500_000,50` | same | same |
| `0.00%` of 0.255 | `25,50%` | `25.50%` | `25,50%` (CLDR pattern has an nbsp before `%`; Excel keeps the code's own text) | `25,50%` | `25,50%` |
| `0.00E+00` | `1,23E+04` | `1.23E+04` | `1,23E+04` | same | same |
| `yyyy-mm-dd` | `2026-10-12` | `2026-10-12` | same | same | same |
| `dd.mm.yyyy` | `12.10.2026` | `12.10.2026` | same | same | same |
| `d mmmm yyyy` | `12 Октябрь 2026` | `12 October 2026` | `12 октября 2026` (CLDR genitive; this Excel gave nominative) | `12 oktabr 2026` | `12 октябрь 2026` |
| `dddd, d mmm yyyy` | `понедельник, 12 окт 2026` | `Monday, 12 Oct 2026` | `понедельник, 12 окт. 2026` | `dushanba, 12 okt 2026` | `душанба, 12 окт 2026` |
| `h:mm:ss AM/PM` (15:04:05) | `3:04:05 PM` | `3:04:05 PM` | same (ru users normally write `H:mm:ss`) | same | same |
| `[$-419]#,##0.00` | `3_500_000,50` | `3,500,000.50` | `3_500_000,50` | the tag does not change digits in Excel | same |
| `[$-443]#,##0.00` | `3_500_000,50` (tag does not change digits) | `3,500,000.50` | `3_500_000,50` | `3_500_000,50` | `3_500_000,50` |
| `[$-419]d mmmm yyyy` | `12 Октябрь 2026` | `12 October 2026` | `12 Октябрь 2026` | n/a | n/a |
| `[$-443]d mmmm yyyy` | `12 Oktabr 2026` | `12 October 2026` | n/a | `12 Oktabr 2026` | Cyrillic needs `[$-843]` |
| `[$₽-419] #,##0.00` | `₽ 3_500_000,50` | `₽ 3,500,000.50` | same | same | same |
| `#,##0.00 "so'm"` | `3_500_000,50 so'm` | `3,500,000.50 so'm` | same | same | same |
| accounting `_-* #,##0.00_-;...` | `3_500_000,50` (padded) | `3,500,000.50` | same | same | same |
| `=TEXT(3500000.5,"#,##0.00")` | `#VALUE!` (Excel reads the picture in the ru locale) | `3,500,000.50` | see note A | | |
| `=TEXT(DATE(2026,10,12),"dd mmmm yyyy")` | `dd mmmm yyyy` (ru wants `дд мммм гггг`) | `12 October 2026` | note A | | |
| `=3500000.5&""` | `3500000,5` | `3500000.5` | `3500000,5` | same | same |
| `=DATE(2026,10,12)&""` | `46307` | `46307` | same | same | same |

Note A: in Excel, `TEXT` pictures and function names depend on the locale in the formula bar but are stored in the file in
en-US syntax. gridcraft always uses en-US pictures. That is right for round-trip; only a UI layer could translate them
(Phase 8 at the earliest).

Typed input (Excel through `.FormulaLocal`, which parses like typing; gridcraft through `cell.set`):

| Typed | Excel ru-RU | gridcraft | Expected ru / uz |
|---|---|---|---|
| `3,5` | number 3.5, shown `3,5` | **text** `3,5` | number 3.5 |
| `3.5` | **date** `03.май` (serial 46145) | number 3.5 | product decision (Excel ru makes a date) |
| `1 234,5` | number, format `# ##0,00` | **text** | number |
| `12.10.2026` | date 46307 (12 Oct) | date **46366 (10 Dec)** | 12 Oct |
| `2026-10-12` | date 46307 | date, shown `2026-10-12` | ok |
| `=1,5+1` | `2,5` | syntax error | `2,5` |
| `=SUM(1;2)` | `3` | **syntax error** (`unexpected Semicolon`) | `3` |
| `=SUM(1,2)` via `.Formula` (storage syntax) | `3` | `3` | file syntax stays comma |

Caveats: this Excel has English function names (`СУММ` gave `#NAME?`), so the ru function-name table was not measured.
Excel's `.Text` group separator came back as U+0020 although Windows sThousand is U+00A0; both render as a space.

## 3. Code locations (gridcraft paths are under `upstream/gridcraft/crates/`)
| Concern | Location |
|---|---|
| Decimal point `.` hard-coded in format render | `numfmt/src/render.rs:337` |
| Group separator `,` hard-coded | `numfmt/src/render.rs:159` (`fill_int`), `render.rs:198` (`with_commas`) |
| General format decimal `.` | `numfmt/src/decimal.rs:161,184` (`format_general_fit`) |
| Locale tags `[$-419]`, `[$-443]`, `[DBNum1]` dropped | `numfmt/src/parse.rs:433`; currency part kept `parse.rs:421-427` |
| Month and weekday names English-only | `core/src/date.rs:191-193`, used at `numfmt/src/render.rs:83-84` |
| Typed input is en-US only (`,` thousands, `.` decimal, M/D/Y) | `core/src/parse.rs:19-45,102-110,142,216`; module doc says "Locale: en-US" |
| Entry locale: only `Es` has `;` list and `,` decimal | `formula/src/input.rs:8-12,34-40`; `ui-egui/src/i18n.rs:111-113` (`Ru` maps to `En`); number entry `ui-egui/src/formula_locale.rs:42`; commit `ui-egui/src/lib.rs:909` |
| UI language from the OS (labels only); no `uz` tag | `ui-egui/src/i18n.rs:81-96,103` |
| Display entry points | `engine/src/display.rs:84-92`, `engine/src/cmd/print.rs:1067,1084` (PDF), `ui-egui/src/grid.rs:538-551` (screen), `engine/src/io.rs:242` (CSV), `engine/src/clip_html.rs:266` |
| CSV | `,` delimiter and displayed text (`xlsx/src/csv.rs:205`); Excel ru would write `;` |

wordcraft (`upstream/wordcraft/crates/`):
- `DATE`/`TIME` fields: `engine/src/cmd/insert.rs:791-860` (`format_date`, `format_date_parts`); English `MONTHS`/`DAYS` at
  `:805-806`; AM/PM literal; no language switch. Refresh on open: `engine/src/cmd/references.rs` (`refresh_date_fields`),
  test `engine/tests/date_fields.rs`.
- The clock is UTC: `engine/src/cmd/mod.rs:416-420`. A `DATE` field in Tashkent (UTC+5) shows yesterday between 00:00 and
  05:00 local (from code reading, not run).
- Table formulas: `engine/src/cmd/table_formula.rs:113-140` (`\#` picture, `.` decimal and `,` group hard-coded, as Word's en pictures).
- Content-control date picker `engine/src/cmd/controls.rs:711` reuses the same English formatter.

deckcraft: `crates/text/src/datetime.rs:80-130` follows the run's language for numeric dates (ru = `dd.mm.yyyy`) but
month/day-name formats are English-only and otherwise keep the saved text. The best of the three.

## 4. What the target locales expect (CLDR + Windows NLS; `spikes/locale/results_cldr.txt`, `nls_values.ps1`)
| | decimal | group | list sep | percent | long date | October (format) |
|---|---|---|---|---|---|---|
| ru | `,` | U+00A0 | `;` | `25,5_%` | `d MMMM y 'г.'` | октября |
| uz-Latn | `,` | U+00A0 | `;` | `25,5%` | `d-MMMM, y` | oktabr |
| uz-Cyrl | `,` | U+00A0 | `;` | `25,5%` | `d MMMM, y` | октябрь |
| en-US | `.` | `,` | `,` | `25.5%` | `MMMM d, y` | October |

Windows NLS agrees (decimal `,`, group U+00A0, list `;`, short date `dd/MM/yyyy` for uz, `dd.MM.yyyy` for ru). Numbers
behave the same for uz and ru; only names, short-date order and percent spacing differ.

## 5. Recommendation: a carried patch (proposed, not implemented)
Locale source: **the app/document language (uz-Latn, uz-Cyrl, ru, en), not the OS locale**, because the web build has no
OS locale and the product has a language selector. Add a `NumLocale { decimal, group, list_sep, month_names, day_names }`.

| Part | Change | Size |
|---|---|---|
| P1 separators (display) | thread `&NumLocale` into `numfmt::format_value` and `format_general_fit`, `render.rs:159,198,337`, `decimal.rs:161,184`; callers `display.rs`, `print.rs`, `grid.rs`, `clip_html.rs`, CSV | 2-3 d |
| P2 names (display) | month and weekday tables for ru, uz-Latn, uz-Cyrl in `core/src/date.rs`; honour `[$-419]`, `[$-443]`, `[$-843]` at `parse.rs:433` | 1-1.5 d |
| P3 input | `core/src/parse.rs` accepts locale decimal and group and D.M.Y order; `FormulaLocale` gains `Ru` and `Uz` (`;` list, `,` decimal, English function names); register `uz` in `i18n.rs:81` | 2.5-3.5 d |
| P4 wordcraft | locale names at `insert.rs:805`, local time zone at `mod.rs:416` | 1 d |
| Tests and golden re-baseline | numfmt, parse, CSV, PDF; keep the en-US goldens | 1.5 d |

Total about 8-10 engineer-days. **P1 alone is 2-3 d** (the cosmetic gap seen in the spike). P1+P2+P3 at 6-8 d is the useful
minimum. Option (b) "display-only" = P1+P2 and leaves the input bugs of section 1 item 5. Russian function-name
translation (hundreds of names) is extra and belongs to Phase 8. File semantics do not change (format codes and formulas
stay en-US on disk), so there is no round-trip risk.

## 6. Does it block v1 for the owner's users?
- **Phase 1 exit: no.** Nothing depends on it.
- **v1 for Uzbek and Russian users: it blocks release quality.** Ranked: (1) `12.10.2026` silently becomes 10 December and
  `3,5` becomes text, so wrong HRM data; (2) every number shows `3,500,000.50` instead of `3 500 000,50`, with English month
  names; (3) wordcraft `DATE` fields use UTC. Recommend scheduling P1+P2+P3+P4 as one carried patch early in Phase 2, so
  the fidelity phase re-baselines only once, not deferring to Phase 8.
- Not done: Windows culture switching for uz-Latn-UZ and en-US (Excel was measured on ru-RU only; uz and en expectations
  come from CLDR and NLS), egui window screenshots, the web build, and a run of wordcraft's DATE field.
