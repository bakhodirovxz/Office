# P2 translation notes: `shared` and `gridcraft` (agent-opus, 2026-10-21)

| | |
|---|---|
| **Files** | `i18n/p2/shared.csv` (250 rows: 249 P2 and the P1 Eraser fix), `i18n/p2/gridcraft.csv` (452 rows: all P2) |
| **Status** | Every row `reviewed:agent-opus:2026-10-21`, uz-Latn and uz-Cyrl both filled, `pass` = `both`, `en_hash` = `review-export`'s hash of the current English |
| **Method** | Glossary first, then the reviewed P1 rows of the same app. Context was read at each `where` site in `upstream/*/crates/ui-egui`. Cyrillic was derived with `i18n/tools/p1_drafts.py` plus the §3.4 loan exceptions in `docs/phase3-i18n/sources.md`. Four rows have hand-written Cyrillic: AaBbDdEe, UPPERCASE, tOGGLE cASE and the AQSH abbreviation. |
| **Self-check** | A script mirrors `xtask/src/i18n/lint.rs`: placeholder parity, `\n` and `\t`, a trailing `…` or `:`, edge whitespace, apostrophe code points in both scripts, Latin words in the Cyrillic text, and the Latin/Cyrillic skeleton pair. **0 problems** on 702 rows. Not run: `cargo xt i18n lint`, because nothing was built. |

## Before importing

`review-import` (`xtask/src/i18n/review.rs:399`) accepts only `reviewed`, `ok`, `approved` or `rejected` in the `status` column, and it
records them as `reviewed:owner:<date>`. With `reviewed:agent-opus:2026-10-21` in that column, the import reads no verdict. Every cell
changes, so every row becomes `draft:owner-fix`. These files therefore need an agent-import path, or the orchestrator applies them
directly. Do not import them with the stock command. The `owner comment` column is empty on purpose, because the import would write
any text there as `owner: …`.

## Rows I was unsure about (7)

| Scope / id | English | uz-Latn chosen | Why unsure |
|---|---|---|---|
| shared `51b41f5a4e1d` | Stacked | Yigʻma | One row, two senses. In gridcraft it is a chart type (`panes.rs:183`), where the glossary gives *Yigʻma*. In deckcraft it is a text direction next to "Rotate all text 90°" (`ribbon.rs:792`), where *Ustun shaklida* would fit better. A context split would fix it. |
| shared `e81790f8017e` | To: | Gacha: | pdfcraft's From/To range fits *Gacha:*. The gridcraft Solver line "To: ○ Max ○ Min ○ Value Of" reads awkwardly with it. |
| shared `cf9639c75ff2` | Decimal | Oʻnli son | The glossary term fits gridcraft validation (Whole number / Decimal). The deckcraft tab-stop kind (Left/Center/Right/Decimal) would read better as *Oʻnli ajratgich boʻyicha*. |
| shared `80b7e4fc31e8` | Small caps | Kichik bosh harflar | There is no established Uzbek term. It pairs with All caps = *Katta harflar*. |
| gridcraft `964fa0ccabca` | Series{n} | Qator{n} | Default chart-series name (like ru Ряд1). *Qator* is also Row; the alternative is *Seriya{n}*. |
| gridcraft `01d7e6e3abc4` | applies to | qoʻllanish sohasi | A fragment glued by code: `"{}. {kind}  applies to {}"` (`dialogs.rs:2233`). Uzbek word order cannot follow it well, so a whole-string template would be better. |
| gridcraft `022a7e1dd7d8` | Weekday | Ish kuni | Fill Series date unit, meaning workdays. I departed from the glossary *Hafta kuni* (CLDR day-of-week), which would mislead here. |

Also off-glossary, on purpose: gridcraft `12efea8132f1` / `299f7665001a` Series / Series… = *Progressiya* (the Fill Series dialog, ru
Прогрессия). The glossary *Maʼlumotlar qatori* is the chart sense.

## Glossary changes I suggest (glossary.tsv not edited)

Rule 1 of sources.md §3 ("a term that the reviewed P1 rows already use stays as it is") is broken by these P2 glossary rows. For in-app
consistency, I followed the reviewed P1 gridcraft rows:

| Term | Glossary now | Reviewed P1 form (used here) | P1 row |
|---|---|---|---|
| Name Manager | Nomlar boshqaruvchisi | **Nomlarni boshqarish** | gridcraft ribbon "Name\nManager"; P1 review §5 #18 |
| Lookup & Reference | Qidiruv va havolalar | **Qidirish va havolalar** | gridcraft "Lookup &\nReference" |
| Remove Duplicates | Takrorlarni olib tashlash | **Takrorlarni oʻchirish** | gridcraft "Remove\nDuplicates" |
| Print Titles | Chop etish sarlavhalari | **Sarlavhalarni chop etish** | gridcraft "Print\nTitles" |
| What-If Analysis | "Agar shunday boʻlsa" tahlili | **«Agar… boʻlsa» tahlili** | gridcraft "What-If\nAnalysis" (guillemets, not ASCII quotes) |
| Command Palette | Buyruqlar palitrasi | Buyruqlar paneli | gridcraft "Command\nPalette" (not in my rows; for the other agents) |

Terms to add:
- **Space** (key and delimiter) = *Probel* / *Пробел*.
- **Weekday** [fill unit] = *Ish kuni*.
- **Series** [fill] = *Progressiya*.
- **Banded Rows / Banded Columns** = *Navbatlashuvchi qatorlar / ustunlar*.
- **Sample** (statistics, as in t-test) = *Tanlanma*.
- **Legend** = *Shartli belgilar* (already the P1 form).

The Cyrillic soft-sign practice is uneven in the repo. Фильтр keeps ь before a suffix (Фильтрни, фильтрлаш: glossary and P1). Пароль drops
it (паролини: P1). I followed both precedents: Пароль standalone, Паролни suffixed, Фильтр… always with ь. The Phase 2 transliteration
golden list should settle this once for all loans.
