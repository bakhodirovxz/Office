# WP14: gridcraft NumLocale, design note

Status: built 2026-10-21 (section 13; decisions in section 12). Design written the same day for PLAN §8 WP14 (owner decision O7). A builder implements it as
carried patches in `patches/gridcraft/` with REGISTER rows. Inputs: `docs/phase1/locale-check.md` (matrix §2, code
map §3, recommendation §5), `spikes/locale/results_cldr.txt`, `spikes/locale/results_excel_ru.tsv`, and Windows NLS
values printed by `spikes/locale/nls_values.ps1` (PowerShell 5.1, .NET Framework, so Windows NLS data).

All `file:line` references are under `upstream/gridcraft/crates/` in the **patched** tree at `27deba0` (BASE
f1894c6 plus queue 0001-0122), which is the tree the new patches apply to. Some locale-check §3 numbers came from the
older 0.1.0 tree. They are re-checked here, and the new numbers win.

## 0. Decisions at a glance

| # | Decision | Reason |
|---|---|---|
| D1 | The **UI language** decides the number locale. The OS locale only seeds the UI language at first run, as it does today. There is no document setting. The env var `GRIDCRAFT_NUM_LOCALE` overrides it for tests. | It already exists and the user can see and change it. It also works on the web, which has no OS locale. A file looks the same on every machine with the same language. |
| D2 | One `Copy` enum, `gridcraft_core::NumLocale {EnUs, Ru, UzLatn, UzCyrl}`, is passed **explicitly**. There is no global and no thread-local. It is stored once, in `Session.prefs`, never in `Workbook`. | Parallel tests in one process need different locales. PDF and CSV code may run off the UI thread. Putting it in `Workbook` behind its `Arc` would force copy-on-write, undo-snapshot and dirty-flag traps. |
| D3 | **Text a person sees follows the locale. Anything parsed, calculated or stored stays en-US.** | This matches Excel's `.Text` against `.Value`/`.Formula`, and upstream's own rule (`ui-egui/src/formula_locale.rs:1`, "Engine commands, control/API callers and XLSX keep English"). |
| D4 | Typed input is localized **at the UI boundary**. The UI turns locale text into en-US input text, and `cell.set` stays en-US. | Engine commands, the journal, macro replay, data validation and MCP agents stay invariant. This is the same boundary upstream already uses for Spanish (`formula_locale.rs:5-47`). |
| D5 | uz and ru formula entry uses `;` between arguments and `,` as the decimal, with English function names. This is a new `FormulaLocale::DecimalComma`. | It matches Excel in those locales (measured: `=SUM(1;2)` and `=1,5+1` through `.FormulaLocal`). The file still stores `SUM(1,2)`. |
| D6 | The locale data is hard-coded: about 80 lines in `core/src/locale.rs`, with Windows NLS names. No new crate. | Excel shows NLS names (measured), while ICU4X and chrono give CLDR or glibc forms. The sheets wasm budget is tight (patch 0112 note). |
| D7 | The group separator is **U+00A0** (NBSP). The input also accepts U+0020 and U+202F. | CLDR and Windows NLS both give U+00A0 for ru, uz-Latn and uz-Cyrl. U+202F is CLDR's choice for fr and others, not for these locales. |
| D8 | en-US is **byte-identical** to today by construction. Every existing function stays, as a wrapper that calls the new `_in` function with `NumLocale::EnUs`. | The existing test suites become the en-US golden test with no edits. |

## 1. Where the locale comes from

- Today `Language::system()` (`i18n.rs:134`) seeds the UI language from the OS: `sys_locale::get_locale()` (Windows
  `GetUserDefaultLocaleName`) at `ui-egui/src/i18n.rs:138`, `navigator.language` on the web at `:142`. The language
  is then saved in `ui.json` and changed with `app.language.set` (`ui-egui/src/lib.rs:650-661`).
- New: `Language::num_locale()` (next to `formula_locale()` at `i18n.rs:146-147`) maps `Ru` to `NumLocale::Ru`,
  `UzLatn` to `UzLatn`, `UzCyrl` to `UzCyrl`, and **every other language to `EnUs`**. That includes `Es`, whose
  existing formula-entry behaviour is unchanged.
- One source of truth: once per frame, where `i18n::set_current` runs (`ui-egui/src/lib.rs:1128`), the UI writes
  `self.session.prefs.num_locale = env override, else self.ui.language.num_locale()`. Engine commands (PDF, CSV,
  `sheet.read` formatted) read `s.prefs.num_locale`, and the grid and the editor read the same value.
- Rejected: following the **OS locale** (Excel's model). Our dev PC has an English Excel UI with ru-RU separators.
  Following the OS would add a second setting to explain and to test, and the web has no OS locale. This is owner
  question Q2. Also rejected: a **document setting**. OOXML has no workbook-level locale, only per-format `[$-419]`
  tags (handled in §3, P0025).
- Tests: `GRIDCRAFT_NUM_LOCALE=ru|uz-latn|uz-cyrl|en` is read by `sheets-cli`'s session constructors
  (`apps/gridcraft-cli/src/main.rs:166-175` and the other `Session::new()` sites) and by the desktop and web app at
  start. Engine unit tests set `prefs.num_locale` directly.

## 2. The type and its data (`core/src/locale.rs`, new)

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumLocale { #[default] EnUs, Ru, UzLatn, UzCyrl }
// from_tag(&str), from_lcid(u32) (low 16 bits: 0x409, 0x419, 0x443, 0x843), decimal(), group(), list_sep(),
// months(), months_abbr(), weekdays(), weekdays_abbr(), short_date() -> Option<&'static str>
```

| | EnUs (today) | Ru | UzLatn | UzCyrl |
|---|---|---|---|---|
| decimal / group / list | `.` `,` `,` | `,` U+00A0 `;` | `,` U+00A0 `;` | `,` U+00A0 `;` |
| built-in id 14 `m/d/yyyy` shown as | unchanged | `dd.mm.yyyy` | `dd.mm.yyyy` (Q1) | `dd.mm.yyyy` (Q1) |
| built-in id 22 `m/d/yyyy h:mm` shown as | unchanged | `dd.mm.yyyy h:mm` | same | same |
| numeric date input order | M/D/Y | D.M.Y | D.M.Y | D.M.Y |
| months (`mmmm`) | January… | Январь Февраль Март Апрель Май Июнь Июль Август Сентябрь Октябрь Ноябрь Декабрь | Yanvar Fevral Mart Aprel May Iyun Iyul Avgust Sentabr Oktabr Noyabr Dekabr | январ феврал март апрел май июн июл август сентябр октябр ноябр декабр |
| `mmm` | first 3 letters (today) | янв фев мар апр май июн июл авг сен окт ноя дек | Yan Fev Mar Apr May Iyn Iyl Avg Sen Okt Noy Dek | янв фев мар апр май июн июл авг сен окт ноя дек |
| `dddd` (Sun first) | Sunday… | воскресенье понедельник вторник среда четверг пятница суббота | yakshanba dushanba seshanba chorshanba payshanba juma shanba | якшанба душанба сешанба чоршанба пайшанба жума шанба |
| `ddd` | first 3 letters (today) | Вс Пн Вт Ср Чт Пт Сб | Yak Dush Sesh Chor Pay Jum Shan | якш душ сеш чор пай жум шан |

- The names are Windows NLS nominative forms, because Excel shows those. Measured on ru-RU: `d mmmm yyyy` gives
  `12 Октябрь 2026`, `dddd, d mmm yyyy` gives `понедельник, 12 окт 2026`, and `[$-443]` gives `12 Oktabr 2026`.
  This **replaces** the CLDR-genitive "Expected ru" cell (`12 октября`) in locale-check §2, because the fidelity
  gate compares against Excel.
- `AM/PM` stays as the code's text, as Excel ru showed (`3:04:05 PM`). `TRUE`/`FALSE`, `#N/A` and the other errors
  stay English. The Russian words belong with the function-name translation in Phase 8.
- The Uzbek rows are Uzbek user-facing data. The table comment carries `reviewed:agent-opus-5.5:2026-10-21`
  (delegated review, CLAUDE.md). They are format data, not catalog strings, because they must equal Excel's bytes.

## 3. Threading path, site by site

| Site | Today | Change | Locale from |
|---|---|---|---|
| Decimal point | `numfmt/src/render.rs:337` (`"."`) | `loc.decimal()` | `render_number(sec, v, loc)` |
| Group separator | `render.rs:159` (`fill_int`), `:198` (`with_commas`) | `loc.group()`, passed as a `char` param | same |
| Sub-seconds `ss.0` | `render.rs:119-126` (`.`) | `loc.decimal()` (COM row C7 confirms) | `render_date(sec, v, sys, loc)` |
| General | `numfmt/src/decimal.rs:138` (`format_general_fit`, `.` at `:161,:184`) | new `format_general_fit_in(n, w, loc)`: the same output with `.` changed to `,` byte by byte (same width) | caller |
| Entry point | `numfmt/src/lib.rs:308` `format_value(v, fmt, sys)` | new `format_value_in(.., loc)`; the old function calls it with `EnUs`. `text_function` (`:398`, the TEXT worksheet function) stays en-US (D3) | caller |
| Month and day names | `core/src/date.rs:191-193`, used at `render.rs:83-84` | names from `sec.names.filter(|_| loc != EnUs).unwrap_or(loc)` | `Section.names` comes from the LCID |
| `[$-419]`, `[$-443]`, `[$-843]` | dropped at `numfmt/src/parse.rs:413-433` | keep `from_lcid(hex & 0xFFFF)` in a new `Section.names: Option<NumLocale>` (`parse.rs:110`). Digits are not affected (Excel measured). In en-US the tag stays ignored (D8, Q3) | parse |
| Built-in ids 14 and 22 | the code text `m/d/yyyy` is written as id 14 (`xlsx/src/styles.rs:567`, `tables.rs:51`), and Excel shows id 14 in the reader's short date | `display::format_in` swaps `m/d/yyyy` and `m/d/yyyy h:mm` for `loc.short_date()` **before** `parsed()`. The parse cache stays keyed by code. Stored codes never change | `display` |
| Engine display | `engine/src/display.rs:32-34` `format`, `:84` `cell_text`, `:96` `value_text`, `:112` `range_text` | add `format_in`, `cell_text_in`, `value_text_in`, `range_text_in`; the old functions call them with `EnUs`, so their about 30 callers (filters, pivot, what-if, agents) stay en-US | caller |
| Editor start text | `display.rs:41-63` `edit_text`; reads back with en `parse_input` at `:49` | `edit_text_in(.., loc)` formats with `format_in` and reads back with `parse_input_in`, so a ru date opens as `12.10.2026` | UI `lib.rs:909` |
| Screen | `ui-egui/src/grid.rs:520-551` `display_text` (General fit `:546`, `display::format` `:550`), called at `:1162` | `_in` with `loc` | `app.session.prefs.num_locale` (the `p.ctx()` language is already used at `:1164`) |
| PDF and print | `engine/src/cmd/print.rs:1070` `display::format`, `:1078,:1086` `cell_text`, `:1084` `format_general_fit`; `&D`/`&T` at `:393-397` | add `loc` to `Block` (`:807`), `make_block` (`:1433`) and `render_pdf` (`:1469`, `hf_date_time` call at `:1474`); `&T` uses `h:mm` when `loc != EnUs` (NLS ru has no AM/PM) | `build_pdf` callers `:1537-1539,:1566` from `s.prefs` |
| CSV save | `engine/src/io.rs:241-254` `csv()`: General gives `number.to_string()` (`:247`), other formats `value_text` (`:251`); delimiter `wb.csv_delimiter.unwrap_or(b',')` at `:217` | delimiter `wb.csv_delimiter.unwrap_or(loc.list_sep())`. Then `let loc = if delim == loc.decimal() { EnUs } else { loc }` (a comma-delimited file is always en-US). General: `to_string()` with `.` changed to `,`; other cells: `value_text_in` | `save_bytes_as_in(.., loc)` from `cmd/file.rs:135-183` |
| CSV open | `io.rs:162-176`; fields parsed by `parse_input` at `xlsx/src/csv.rs:164` | `CsvOptions.locale` (`xlsx/src/lib.rs:153-166`, default `EnUs`), with the same comma rule after sniffing: a `,` delimiter means en-US, `;` or tab means the session locale. So our uz CSV opens back to the same numbers, and an en CSV still opens correctly in uz | file-open command from `s.prefs` |
| `sheet.read` `formatted` | `engine/src/cmd/inspect.rs:107` `cell_text` | `cell_text_in(s.prefs.num_locale)`; this is what `sheets-cli cat` prints, so the CLI can measure the matrix | `s.prefs` |
| Copy to system clipboard | `engine/src/cmd/edit.rs:662` `range_text` | `range_text_in(s.prefs.num_locale)` (Excel copies the displayed text) | `s.prefs` |
| Typed input | `ui-egui/src/lib.rs:933` calls `formula_locale::canonical`; its constant branch `formula_locale.rs:41-47` only swaps the decimal | for `num != EnUs`, constants go through `core::parse::to_invariant(text, sys, num)` (§4); formulas go through `FormulaLocale::DecimalComma` | `EditState.num_locale` (next to `locale`, `ui-egui/src/editor.rs:23`) |

**Not changed in WP14**, which stays en-US and is listed for the gate report: chart axis and data labels
(`chart/src/axis.rs:139`, `resolve.rs:150`), AutoFilter lists, pivot and slicer labels, HTML export, Format Cells
samples, status-bar aggregates, row autofit (`cmd/mod.rs:492`), clipboard paste parsing (`edit.rs:803`, HTML paste
inference `clip_html.rs:266`), `[$-F800]` system long date, and wordcraft DATE fields (locale-check P4). Each is a
one-line `_in` switch once its caller can reach a `NumLocale`. None of them affects stored data.

## 4. Typed input and formula entry

**`core::parse::parse_input_in(s, sys, loc)`** (new; `parse_input` at `core/src/parse.rs:19` calls it with `EnUs`)
applies only these rules when `loc != EnUs`:
- **Plain number.** `,` is the decimal. U+0020, U+00A0 and U+202F are group separators, with 3-digit groups (the
  check at `:99-112`). A number containing `.` is not a plain number: it goes on to the date rules. Then the
  existing `parse_plain` runs on the normalized text, so `%`, `$`, `E`, `(…)` and the auto formats are unchanged.
- **Numeric date** (`.`, `/` or `-`). A 4-digit first field means Y-M-D, as today. Otherwise the order is **D.M.Y**
  (instead of `:225-228`), with the same 2-digit-year rule and the same auto format `m/d/yyyy`, which is stored as
  id 14. `d.m` means the current year with `d-mmm`, as Excel ru does (`3.5` gives 3 May, measured).
- **Month names.** `month_from_name` (`:164`) also matches the locale's full or abbreviated name, case-insensitive
  with `to_lowercase` (today's `to_ascii_lowercase` does not fold Cyrillic) and an optional trailing `.`. English
  names stay accepted. Exact full or abbreviated match only, not a prefix: `Iyun` and `Iyul` share `Iyu`.
- Times, `TRUE`/`FALSE` and errors are unchanged.

**`to_invariant(s, sys, loc) -> String`**:
- For `EnUs` it returns `s` unchanged.
- Otherwise it rewrites the parsed pieces as en-US input text: `1 234,5` becomes `1,234.5`, `25,5%` becomes
  `25.5%`, `12.10.2026` becomes `10/12/2026`, `12.10.2026 15:00` becomes `10/12/2026 15:00`, and `3 окт 2026`
  becomes `3-Oct-2026`.
- It then **checks** that `parse_input(out, sys)` gives exactly the same `(value, format)`.
- Locale text that the en-US parser would read as something else (`1.234` in ru) is sent as `'1.234`, so it stays
  text.
- If the check fails, the result is forced text, never a different number. A property test (§6) shows that this
  never happens for the supported shapes.
- Because the engine only ever sees en-US text, validation (`check_validation` before `cell.set`, `lib.rs:945`),
  `range.fill`, the journal and macro replay need no change.

**Formula entry (D5).** `FormulaLocale::DecimalComma` is added in `formula/src/input.rs:8-12`:
- `list_separator()` gives `;`, `decimal_separator()` gives `,`, and `array_column_separator()` gives `\` (as `Es`
  does; COM row C9 confirms or changes it).
- It has no function-name table, so `function_name` and `canonical_name` return the input. `parser.rs:38` keeps
  `input` (alias normalization) for `Es` only.
- The lexer already takes every separator from these methods (`lexer.rs:209,240,260,284,330`).
- `Language::formula_locale()` (`i18n.rs:146-147`) maps `Ru`, `UzLatn` and `UzCyrl` to it. Today `Ru` maps to `En`.

Round-trip and test risk of D5:
- Storage stays `SUM(A1,A2)` and `1.5`. The formula bar shows the stored formula through `formula_locale::display`
  (`:50`), as `Es` already does.
- On commit, a formula typed in uz or ru is re-printed by `formula::print`, so its spelling is normalized (case,
  spacing). `Es` users get the same today, and en-US keeps the "return the text unchanged" fast path at
  `formula_locale.rs:20`.
- No en-US test changes. New tests cover string literals containing `;` and `,`, union `(A1;B1)`, sheet names with
  `;`, and arrays.

## 5. Stored values stay invariant: where this is enforced

1. `Workbook` gets no new field, so the xlsx writer and the JSON serde format are untouched.
2. Format codes are never rewritten. The id 14/22 swap and the `[$-xxx]` names exist only at display time.
3. Typed constants and formulas reach the engine only as en-US text (§4).
4. The calc engine is not changed. `TEXT()`, `&""`, `VALUE("3,5")` and `DATEVALUE("12.10.2026")` keep en-US
   semantics. **This differs from Excel ru on purpose** (locale-check note A): the cached values written to the
   file stay locale-free. It belongs with Phase 8 function-name translation (risk R5).
5. CSV is a display format by definition (Microsoft: "as displayed"), so it may differ by locale. It is never the
   source of truth.

## 6. Tests

| Level | Test | Proves |
|---|---|---|
| core | `locale.rs`: tables against the NLS values in §2, `from_tag`, `from_lcid` (incl. `0x10419` giving Ru) | data |
| core | `parse.rs` per locale: `3,5`; `1 234,5` (3 group chars); `-1 234,50`; `(1 234)`; `25,5%`; `1,5E3`; `12.10.2026` (46307); `12/10/2026`; `12.10.26`; `12.10`; `3.5` (ru: 3 May); `1.234` (text); `1,234` (1.234); `2026-10-12`; `12.10.2026 15:00`; `12 окт 2026`; `12 Oktabr 2026`; `10:30`; `Iyu` (text) | typed input |
| core | property: every serial 1..=73050 × 3 locales, shown as `dd.mm.yyyy`, goes through `to_invariant` and en-US `parse_input` back to the same serial and `m/d/yyyy`; 10^4 seeded random numbers shown with `#,##0.00` and General give equal f64 values | D4 safety |
| numfmt | per locale, the locale-check §2 display rows (corrected names), including `0.0` of 3500000 giving `3500000,0`, `#,##0.00` giving `3 500 000,50` (U+00A0), `0.00E+00`, `[$-419]` names under uz, General width unchanged | display |
| numfmt | **en-US golden**: every built-in id 0..=49 × 25 values (negatives, 0, 1e-5, 1e15, dates, times) gives `format_value_in(EnUs) == format_value`, plus the unchanged existing `numfmt/src/tests.rs`, `core` and `engine` suites | D8 |
| formula | `DecimalComma`: `SUM(1;2)` gives `SUM(1,2)`, `1,5+1` gives `1.5+1`, strings, union, arrays, round trip through `print_input` | D5 |
| engine | `tests/numlocale.rs`: CSV save per locale (`;`, `,`, U+00A0), the comma-delimiter rule, CSV save-open round trip, `sheet.read` formatted | CSV |
| engine | PDF per locale: `contains(&pdf, "(3500000,5) Tj")` in the print.rs test style (`print.rs:1909-1924`); a grouped value; `&D` giving `dd.mm.yyyy` | PDF text |
| engine | **xlsx invariance**: one input script, typed through `to_invariant` under each locale and directly under en-US, then saved. Every zip part except `docProps/*` must be byte-equal | stored values |
| ui-egui | `canonical` with `num_locale` (extends `ui-egui/tests/formula_locale.rs`); `display_text` per locale | UI wiring |
| matrix | re-run locale-check §2 with `GRIDCRAFT_NUM_LOCALE=ru|uz-latn|uz-cyrl sheets-cli cat/convert/run file.exportPdf`, PDF text through the spike's pdfplumber step; uz and ru rows must be correct, and the en row must equal `spikes/locale/results_gridcraft.csv` | done-check |

## 7. Excel COM cross-check (MS Office 16, own Excel PID only, dev PC user locale ru-RU)

| # | Check | How | Settles |
|---|---|---|---|
| C1 | display | `spikes/locale/excel_text.ps1` `.Text` against `sheets-cli cat` / CSV / PDF under ru. Treat U+0020 and U+00A0 as the same: Excel's `.Text` returned U+0020 (locale-check caveat) | ru display |
| C2 | invariance | open the uz-saved and ru-saved xlsx in Excel and compare `.Value2`, `.Formula` and `.NumberFormat` of every cell with the en-US-saved file: all equal | O7 core |
| C3 | typed input | `.FormulaLocal` for each §6 core input, then compare `.Value2` and `.NumberFormat` with our `to_invariant` + `cell.set` result | D4 |
| C4 | formulas | `.FormulaLocal = "=SUM(1;2)"` / `"=1,5+1"` against our canonical `.Formula` | D5 |
| C5 | CSV | Excel `SaveAs` xlCSV and xlCSVUTF8: delimiter, decimal, and the group byte (0xA0 or 0x20) | CSV group char |
| C6 | ids 15-17 | `d-mmm-yy` / `d-mmm` / `mmm-yy` `.Text` in ru (`typed 3.5` showed `ДД.МММ`) | extend the id swap if Excel localizes them |
| C7 | `ss.0` | `.Text` of `mm:ss.0` | sub-second decimal |
| C8 | `1.234`, `ddd` | `.FormulaLocal "1.234"`; `.Text` of `ddd` | text or number; `Пн` |
| C9 | arrays | `.Formula = "={1,2;3,4}"`, then read `.FormulaLocal` | array column separator |

uz rows can be checked against Excel only through `[$-443]`/`[$-843]` tags on the ru-RU account. A real
uz-Latn-UZ culture needs a culture switch or a second Windows user (Q4). Until then, uz separators rest on NLS and
CLDR, which agree.

## 8. Patch split (functional 0023-0030 before the overlay; overlay 0130 after 0122; category `i18n`)

| Patch | Subject | Files | Touches / depends on | Est. |
|---|---|---|---|---|
| 0023 | core: `NumLocale` and its tables | `core/src/locale.rs` (new), `core/src/lib.rs` | no carried patch | 0.5 d |
| 0024 | numfmt: decimal and group separators by locale | `numfmt/src/lib.rs`, `render.rs`, `decimal.rs` | 0023; no carried patch touches numfmt | 1 d |
| 0025 | numfmt: month and day names, `[$-xxx]` name tags, ids 14/22 short date | `render.rs:83-84`, `parse.rs:110,413-433`, `lib.rs` | 0023, 0024 | 1 d |
| 0026 | core: typed input under a locale, `to_invariant` | `core/src/parse.rs` | 0023 | 1.5 d |
| 0027 | formula: `FormulaLocale::DecimalComma` | `formula/src/input.rs`, `formula/tests/` (new file) | no carried patch | 0.5 d |
| 0028 | engine: `Prefs.num_locale`, `display::*_in`, CSV save/open, `sheet.read`, copy | `engine/src/lib.rs:198-227`, `display.rs`, `io.rs`, `cmd/file.rs`, `cmd/inspect.rs`, `cmd/edit.rs:662`, `xlsx/src/lib.rs:153-166`, `xlsx/src/csv.rs:164` | after 0002 (`display.rs:86`), 0015 (`io.rs:157-177`, same context as the CSV-open hunk), 0018 (`xlsx/src/lib.rs:86`); keep the hunks narrow | 1.25 d |
| 0029 | engine: PDF and print text by locale | `engine/src/cmd/print.rs` | **after 0021** (its hunks at `print.rs:1079-1107` sit in the same lines as ours) | 0.5 d |
| 0030 | cli: `GRIDCRAFT_NUM_LOCALE` for sheets-cli sessions | `apps/gridcraft-cli/src/main.rs` | no carried patch touches it | 0.25 d |
| 0130 | office overlay: UI language to NumLocale and FormulaLocale, frame sync, grid, editor, canonical | `ui-egui/src/i18n.rs`, `lib.rs`, `grid.rs`, `editor.rs`, `formula_locale.rs`, `tests/formula_locale.rs` | **needs 0110** (cfg `office_uz` variants); i18n.rs contexts of 0100/0112; grid.rs of 0002 (`:529`) and 0121/0122 (far hunks) | 1 d |
| - | REGISTER rows, regenerate the trailing 0111 rustfmt patch (`scripts/sync-upstream.sh --fmt`), `verify-queue.sh`, matrix re-measure, COM C1-C9, perf §9 | | | 1 d |

**Total: about 8.5 d**, against the plan's 6-8 d. The gap is the CSV-open rule, `to_invariant` with its property
test, and the COM rows.
- Order of value if time runs short: 0023 → 0026 + 0130's canonical part (fixes the data-integrity bug,
  `12.10.2026` read as 10 December) → 0024 + 0028 + 0029 (display, CSV and PDF) → 0025 (names) → 0027 (`;` entry).
- Why the ui-egui wiring, including `Ru`, sits in overlay 0130 and not in a functional patch: it must apply after
  0100, 0110 and 0112 have changed `i18n.rs`.
- Split for upstream later: 0023-0030 are upstream-shaped and need no overlay. The `Ru` arm of 0130 can move into
  them.

## 9. Performance and memory (no measurable regression allowed)

- **Expected cost.**
  - en-US: one enum compare per formatted number and no new allocation.
  - uz/ru: one byte pass over a short string for General, and a `char` param in place of a literal.
  - Memory: no per-cell or per-workbook state, one byte in `Session.prefs`, and `Workbook` unchanged.
  - Size: about 2 KB of tables in the wasm.
- **How to measure.** Release build of sheets-cli at the current queue against current queue plus 0023-0030.
  - Time: `sheets-cli convert w80.xlsx w80.csv`, which formats every cell, and `file.exportPdf` of a fixed w80
    range. 5 runs each, median and spread.
  - Memory: peak RSS through `spikes/xlsx-bench/measure_tpl.py` / `memsample.py`, on the same w80 bench file as the
    memory rounds (`docs/phase0/gridcraft-memory-r2.md` §1).
  - Pass if the en-US median is within the before run's spread (and at most +2 %), and peak RSS is within 5 MB.
    Report uz/ru against en-US: under +5 % is expected. Wasm size: `ci.sh sizes`.
- **Memory patches.**
  - 0001-0019 touch calc, model, xlsx read/write and `io.rs` open. WP14 changes none of their data structures:
    calc and the model are not changed. The only shared files are `display.rs` (0002) and `io.rs` (0015, 0018),
    and the hunks there are separate.
  - 0015 (keep cached values on open) is exactly why formula *results* must stay locale-free (§5.4). A locale-
    dependent TEXT() would make the cached values depend on who saved the file.

## 10. Risks

- **R1, ambiguous input.**
  - In ru and uz, `1.234` stays text, and `1,234` is 1.234 (decimal). `3.5` becomes 3 May (a date, as in Excel).
  - The effect is visible, never silent: the text is left-aligned, and the date shows as `03.май`-style text.
  - A user who types en-US habits gets Excel-ru behaviour. Covered by C3/C8 and the core tests.
- **R2, CSV.**
  - The default delimiter becomes `;` for uz and ru (Excel parity). An HRM importer that expects `,` would break.
  - The comma rule (§3) keeps en CSVs readable both ways. This is Q5.
- **R3, NBSP.**
  - Excel's `.Text` gives U+0020, and fidelity comparers (WP9/WP10) must treat the two as the same.
  - PDF subsets must include U+00A0. Fallback 0102 covers it, but check it in the PDF test with an embedded font.
- **R4, formula re-print.** uz and ru formula text is normalized on commit (the `Es` precedent).
- **R5, calc stays en-US.** ru-authored files whose `TEXT()` pictures are written in ru syntax (`"# ##0,00"`) show
  differently from Excel ru, and they do so in en-US Excel as well. Count these in the WP9 corpus by count only,
  never copying content. Phase 8.
- **R6, upstream sync.** The numfmt render signatures gain a param. The old public functions remain, so upstream
  callers keep compiling. Expect a manual rebase if upstream edits `render.rs`.
- **R7, gaps (§3 list).** A uz session still shows en-US text in charts, filter lists, pivots and the status bar.
  This is a consistency gap, not a data risk. The gate report names it.

## 11. Owner questions

- **Q1.** Should the uz short date (id 14) show as `dd.mm.yyyy` (proposed: your typing habit, and what Excel on a
  ru-RU Windows shows) or as `dd/mm/yyyy` (Windows uz-UZ NLS, what Excel on a uz-Latn-UZ Windows shows)?
- **Q2.** Should an English UI on a ru or uz Windows keep en-US numbers (proposed, minimal) or follow the OS like
  Excel?
- **Q3.** Should `[$-419]`/`[$-443]` tags also show local names in the en-US UI (what Excel does)? That would change
  en-US output for such files only. Proposed: not in WP14.
- **Q4.** May uz be measured in Excel with a temporary uz-Latn-UZ user culture, or a second Windows user? Without
  it, the uz rows rest on NLS and CLDR plus `[$-443]` tags.
- **Q5.** Should a new CSV use `;` in uz and ru (proposed, Excel parity), or must it stay `,` for HRM imports?

## 12. Decisions

Owner answers of 2026-10-21, taken before WP14 was built:

- **Q1.** The uz short date (built-in ids 14 and 22) shows as `dd.mm.yyyy`, as in ru.
- **Q2.** The locale follows the **UI language**. An English UI keeps en-US numbers, also on a ru or uz Windows.
- **Q3.** `[$-419]`/`[$-443]`/`[$-843]` tags do **not** change the en-US UI output in WP14. They name months and days only
  under a non-English interface.
- **Q4.** (switching the Windows culture to measure uz in Excel) **Yes (owner, 2026-10-22).**
  - A temporary uz-Latn-UZ user culture may be used for the Excel COM uz rows.
  - It runs after the Phase 3 gate's `ci.sh full`, so the culture switch cannot disturb the locale-dependent tests.
  - The original culture is restored afterwards.
- **Q5.** (`;` or `,` as the CSV default for uz and ru) **Decided by the agent (owner delegated it, 2026-10-22):
  `;` stays.**
  - Excel in these locales writes `;`, so a double-clicked CSV opens in columns.
  - `,` remains one choice away: `convert --locale en-US`, or the constant `LOCALE_CSV_LIST_SEPARATOR = false` in
    `engine/src/io.rs` (patch 0028).
  - A file opened from CSV keeps its own separator either way.

## 13. Built (2026-10-21)

Nine carried patches in `patches/gridcraft/`, REGISTER rows for each (category `i18n`, 72 rows before, 81 after).

| Patch | What it does | Files / lines |
|---|---|---|
| 0023 | `core::NumLocale` and its tables. `builtin_date` maps the built-in date codes of ids 14-17 and 22 to the locale forms | 2 / +193 |
| 0024 | numfmt: decimal, group, sub-second point and General follow the locale (`format_value_in`, `format_general_fit_in`). En-US golden (1500 rows recorded before the change) | 7 / +1703 (1500 are the golden) |
| 0025 | numfmt: `mmm`/`mmmm`/`mmmmm`/`ddd`/`dddd` names and `[$-419]`/`[$-443]`/`[$-843]` name tags (non-English UI only, Q3) | 3 / +81 |
| 0026 | core: `parse_input_in`, `to_invariant`, property tests (all dates 1900-2100, 10,000 numbers) | 1 / +222 |
| 0027 | formula: `FormulaLocale::DecimalComma` | 2 / +79 |
| 0028 | engine: `Prefs.num_locale`, `display::*_in`, `edit_text_in`, CSV save and open, `sheet.read`, copy and paste, `CsvOptions.locale`, `LOCALE_CSV_LIST_SEPARATOR` (Q5), `env_num_locale` | 9 / +350 |
| 0029 | engine: PDF text, `&D`/`&T` | 1 / +80 |
| 0030 | sheets-cli: `GRIDCRAFT_NUM_LOCALE` for `cat`, `convert`, `run`, `eval` | 1 / +15 |
| 0130 | ui-egui: `Language::num_locale`, frame sync, grid, editor start text, `canonical`, Ctrl+; and Ctrl+', formula entry mapping | 9 / +224 |

Differences from the design, all small:
- **Paste.** Text pasted from the system clipboard is typed in the session locale (`to_invariant` in `paste_text`). The design listed paste parsing as not changed, but copy now produces `12.10.2026`, and pasting it back as en-US would store 10 December. The internal clipboard path is unchanged.
- **Built-in ids 15-17.** Excel ru shows them as `12.окт.26`, `12.окт`, `окт.26` (COM row C6), so `NumLocale::builtin_date` swaps them like ids 14 and 22. uz follows ru (Q1); the uz forms are not measured in Excel (Q4).
- **`edit_text_in` numbers.** A constant number opens in the editor with the locale decimal (`3500000,5`). Without it, F2 then Enter would turn a number into text under ru (the typed-input rule reads `3500000.5` as a date or text).
- **One sync point.** `SheetApp::num_locale()` (language, or `GRIDCRAFT_NUM_LOCALE`) is used by `begin_edit` and by the frame sync, so headless UI tests do not depend on a frame having run. `Prefs.num_locale` is `serde(skip)`.
- **Month names in typed text.** English names are still accepted in ru and uz (design). Excel ru does not accept them (`12 Oct 2026` stays text there), and Excel ru reads `$5,5` as text where we read a dollar amount. Both are deliberate leniency, not fidelity.
- **Custom `m/d/yyyy`.** Excel ru shows a *custom* `m/d/yyyy` month first (`10.12.2026`, the `/` becomes the date separator) and only the built-in id 14 as `12.10.2026`. Our model reads both as the same code, so both show `12.10.2026`. The `/` and `-` of date codes stay literal in all other codes too.

### Tests added

| Level | Where | Count |
|---|---|---|
| core | `locale.rs` (3), `parse.rs` (+6: en-US identity, per locale numbers, dates, the integrity case, every date 1900-2100 x 3 locales, 10,000 numbers x 3) | 9 |
| numfmt | `tests/en_us_golden.rs` (1500 rows), `tests/locale_separators.rs` (3), `tests/locale_names.rs` (2) | 6 |
| formula | `tests/decimal_comma_input.rs` | 6 |
| engine | `tests/numlocale.rs` (8: CSV per locale, comma rule, CSV round trip, `sheet.read`/copy, editor round trip, paste, built-in date ids, xlsx parts byte-equal across en/ru/uz-Latn/uz-Cyrl), `print.rs` (2: PDF text, header date and time) | 10 |
| ui-egui | `tests/num_locale.rs` (7: typing in ru and en, F2-Enter keeps the value for 6 kinds of cell, editor text, `@` cells, ru formulas) | 7 |

The existing suites run unedited except one line in `numfmt/src/tests.rs` (a private method gained the locale argument).

### Matrix re-measured (locale-check section 2)

`sheets-cli cat/convert/run` with `GRIDCRAFT_NUM_LOCALE`, PDF text through pdfplumber, Excel 16 on the ru-RU account. Files: `spikes/locale/gen_locale_xlsx.py` (24 cases) and `gen_wp14_xlsx.py` (20 cases), driver `spikes/locale/excel_wp14.ps1`.

- **Before** (en-US everywhere): 5 of 24 rows equal Excel ru.
- **After:** 21 of 24 rows equal Excel ru, and 19 of 20 of the new rows. The 3 differences are `TEXT(3500000.5,"#,##0.00")`, `TEXT(DATE(..),"dd mmmm yyyy")` and `3500000.5&""`: calc stays en-US on purpose (D3, R5). The 1 of 20 is the custom `m/d/yyyy` above.
- Screen, CSV and PDF text agree in every row for ru, uz-Latn and uz-Cyrl (a row's screen text is found in the CSV and in the PDF text layer).
- The en-US column equals `spikes/locale/results_gridcraft.csv`, row by row (only the padding of the accounting row differs: `cat` trims).

| Case (3500000.5 unless stated) | Excel ru-RU | ours ru | ours uz-Latn | ours uz-Cyrl | ours en |
|---|---|---|---|---|---|
| General | `3500000,5` | `3500000,5` | same | same | `3500000.5` |
| `#,##0.00` | `3 500 000,50` | `3 500 000,50` (U+00A0) | same | same | `3,500,000.50` |
| `0.00%` of 0.255 | `25,50%` | `25,50%` | same | same | `25.50%` |
| `d mmmm yyyy` | `12 Октябрь 2026` | `12 Октябрь 2026` | `12 Oktabr 2026` | `12 октябр 2026` | `12 October 2026` |
| `dddd, d mmm yyyy` | `понедельник, 12 окт 2026` | same | `dushanba, 12 Okt 2026` | `душанба, 12 окт 2026` | `Monday, 12 Oct 2026` |
| `[$-443]d mmmm yyyy` | `12 Oktabr 2026` | same | same | same | `12 October 2026` (Q3) |
| built-in id 14 | `12.10.2026` | `12.10.2026` | same | same | `10/12/2026` |
| built-in id 15 / 16 / 17 | `12.окт.26` / `12.окт` / `окт.26` | same | `12.Okt.26` / `12.Okt` / `Okt.26` | Cyrillic as ru | `12-Oct-26` / `12-Oct` / `Oct-26` |
| `mm:ss.0` | `00:00,9` | same | same | same | `00:00.9` |

Typed input (Excel `.FormulaLocal` against our UI commit path, 27 inputs, ru and en): every ru row agrees with Excel ru except the two leniencies above (`12 Oct 2026`, `$5,5`). Results: `3,5` is 3.5; `3.5` is 3 May (46145, `d-mmm`); `1 234,5` is 1234.5 with `# ##0,00`; `1.234` text; `1,234` is 1.234; `12.10.2026`, `12/10/2026`, `12.10.26`, `12.10`, `2026-10-12` are 46307; `10.13.2026` and `31.02.2026` text; `25,5%`, `(1 234)`, `1,5E3`, `10:30`, `TRUE`, `12 окт 2026`, `окт 2026`, `10.2026` agree; `1 23 456` and `1,234,567` text. In the en UI the old behaviour is unchanged (`12.10.2026` is still 46366).

### Excel COM rows (MS Office 16, own PID, Windows culture ru-RU)

| # | Result |
|---|---|
| C1 display | see the matrix; U+0020 and U+00A0 compared as equal |
| C2 invariance | xlsx saved by `sheets-cli convert` under en, ru, uz-Latn and uz-Cyrl: 72 cells, `.Value2`, `.Formula` and `.NumberFormat` equal in all four |
| C3 typed input | as above |
| C4 formulas | `=SUM(1;2)` stores `SUM(1,2)` = 3; `=1,5+1` stores `1.5+1` = 2.5; `SUM(1,2)` typed in ru is `SUM(1.2)` = 1.2; `=ROUND(2,675;2)` = 2.68; `;` inside a string stays. All equal our `DecimalComma` results |
| C5 CSV | `xlCSV` and `xlCSVUTF8`: delimiter `;`, decimal `,`, date as displayed, field with `;` quoted (UTF-8 adds the BOM). The group byte is **0x20**, not U+00A0: this PC's Windows thousands separator is U+0020 (`International(4)`), so this does not settle the default; we keep U+00A0 (NLS and CLDR) |
| C6 ids 15-17 | Excel localizes them; implemented |
| C7 `ss.0` | decimal comma (`00,9`), as implemented |
| C8 `1.234`, `ddd` | `1.234` text; `ddd` is `Пн`; both as implemented |
| C9 arrays | `.Formula = "=SUM({1,2;3,4})"` reads back `.FormulaLocal` `=SUM({1\2;3\4})`, so `\` is right |
| uz rows | **not run**: they need a Windows culture change (Q4). The uz-Latn and uz-Cyrl outputs rest on NLS, CLDR and the `[$-443]`/`[$-843]` tags measured on the ru account |

Windows notes: COM's `.NumberFormat` takes the culture's own separators and letters (`'#,##0.00'` and `'m/d/yyyy'` are refused on ru-RU), so the format cases go in through xlsx files with en-US codes.

### Performance (w80, `E:/office-target/xlsx-bench/w80.xlsx`, 79 MB, release `sheets-cli`, 5 runs interleaved)

| Workload | before | after en-US | after ru |
|---|---|---|---|
| `convert w80.xlsx w80.csv`: median s (min-max) | 71.3 (66.0-96.5) | 65.9 (62.9-71.5) | 66.4 (65.2-71.3) |
| `file.exportPdf` range A1:L200: median s | 65.1 (64.1-68.6) | 65.9 (63.8-69.7) | 68.3 (63.4-69.0) |
| peak working set, MB | 818 | 817 | 818 |

en-US after is inside the spread of before in both workloads (PDF +1.3 %, CSV -7.6 % with a 96 s outlier in before), and the peak working set is 1 MB lower. ru is +0.7 % (CSV) and +4.9 % (PDF) of the after-en median, inside the spread of before. The workload is dominated by open and calculation (about 65 s), so the formatting cost is below the noise of this measurement.

### Still en-US

Chart axes and data labels, AutoFilter lists, pivot and slicer labels, HTML export, Format Cells samples, status-bar aggregates, row autofit widths, `[$-F800]`, wordcraft DATE fields, `TEXT()` pictures, `VALUE`/`DATEVALUE` and the text of `&` concatenation, function names, `TRUE`/`FALSE`/error names, and the `/` and `-` of date codes (Excel ru shows them as `.`).
