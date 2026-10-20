# wordcraft P2 translation notes (2026-10-21)

| | |
|---|---|
| **File** | `i18n/p2/wordcraft.csv`, in the review-export column format (13 columns, UTF-8 with BOM, CRLF). |
| **Rows** | 1,626 P2 rows of `i18n/wordcraft.tsv`. All of them have uz-Latn and uz-Cyrl text and the status `reviewed:agent-opus:2026-10-21`. |
| **Method** | The Latin was written per row, using the `where` site and the upstream ru column for context. The Cyrillic was derived with the `i18n/tools/p1_drafts.py` letter mapping, plus the hand exceptions listed below. |
| **Sources** | The glossary, the reviewed P1 wordcraft rows, and CLDR `uz` (language names and weekday abbreviations). No other source was consulted, and no Microsoft term was used. |

## Import caveat

`cargo xt i18n review-import` does not know this status value. It only accepts the verdicts `reviewed`, `ok` and `approved`, and it stamps them `reviewed:owner:<date>`.

As written, the importer would store every row as `draft:owner-fix:2026-10-21`. Both outcomes are wrong, because these rows were reviewed by the agent, not the owner. They need an agent-aware import step, or a direct write of the status.

## Self-check (a Python script in the session scratchpad, not committed)

The script reported 0 errors on these checks:
- placeholders and `%` specifiers
- `\n` and `\t` counts
- trailing `…` and `:`
- leading and trailing whitespace
- apostrophe code points (ʻ only after o/g, ʼ only after a letter, no ASCII ' or ’ next to a letter, none in Cyrillic)
- no Cyrillic letters in the Latin text, and no foreign Latin words in the Cyrillic text
- the same punctuation in both scripts
- the lint letter-mapping skeleton (0 mismatches)
- the WordCraft, ArtCraft and Discord tokens: each kept the same number of times, and never with a suffix joined to it

The suffix check matters because `office_overlay::rebrand` replaces only whole tokens. The text therefore says "WordCraft dasturida" and never "WordCraftda".

**Cyrillic hand exceptions:**
- дифференциал, коэффициент, конференция, компьютер, Дельта, Ателье, Модуль, Фильм, Фильтр-, Байес
- Пароль when it stands alone; паролни, паролда with a suffix (the P1 rule)
- АҚШ, plus the capital-letter Uzbek words of the watermarks (МАХФИЙ, ЗУДЛИК БИЛАН, ШОШИЛИНЧ …)
- the font sample AaBbOʻoʻGʻgʻ → АаБбЎўҒғ

Latin kept in the Cyrillic text:
- paper names (Legal, Letter, Executive, Statement, Tabloid)
- citation style names (Chicago, Harvard, Turabian)
- `a`/`an` in the grammar rules
- maths variables
- the URLs, file chooser names and locale codes
- the macOS labels

## Decisions worth knowing

- **Built-in style and table-style names** were translated, for example Subtle Emphasis *Nozik urgʻu*, Intense … *Kuchli …*, and List Table 3 – Orange *Roʻyxat jadvali 3 – toʻq sariq*. Upstream translates them for display only, through `i18n::builtin_style_name`, and its test confirms that the document keeps the English names.
- **Grammar messages about English** translate the explanation and keep the quoted English word: `Undosh tovush oldidan "a" ishlating`.
- **Two backtick pairs:** in row `options.rs:543`, the backticked commands are in “…” quotes in Uzbek. A closing backtick after a letter fails the apostrophe lint.
- **Paper dimensions** keep the source's decimal point (`8.50" × 14.00"`).
- **Weekday samples** in the table-style preview use the lower-case CLDR forms *dush / sesh / chor*, following the P1 decision.
- **macOS Finder steps** (`options.rs:698`) keep the English menu names (*File › Get Info*, *Open with*, *Change All*), because macOS has no Uzbek interface.

## Unsure rows (7)

| Id | English | uz-Latn | Why |
|---|---|---|---|
| d69eb99518da | Table of\nContents | Mundarija\nkiritish | Lint requires the same number of `\n` as the English. *Mundarija* is one word, so a verb was added. |
| 8565b2a55f2a | Scripts | Yozma harflar | A symbol set of script, fraktur and double-struck letters. Upstream's ru *Индексы* is wrong here, and the glossary's Script (math) = *Indeks* does not fit. |
| ee5ebbe88fa9 | Through | Ichidan oʻtuvchi | A wrap style. No established Uzbek term was found. |
| 311bd57b4ddf | Strong | Yaqqol | The bold character style. *Kuchli* is already taken by Intense, and *Qalin* is Bold. |
| 86184ea2779b | Log | logarifm | The second half of the split label "Limit and" + "Log". Lower case matches the full label *Limit va\nlogarifm*. |
| ec2573365600 | Craft | Craft | Not a UI string. It is the `s.contains("Craft")` probe of the rebrand hook, so it was left unchanged in both scripts. |
| 2297175b36af (+5 siblings) | Inverse Sine Function … | Teskari sinus funksiyasi … | Follows the glossary's Inverse = *Teskari*. Uzbek textbooks say *arksinus*, *arkkosinus*. |

## Glossary suggestions (glossary.tsv was not edited)

1. **Citation: conflict between P1 and the glossary.** The glossary sets Citation = *Manbaga havola*, and the P2 rows follow it. The reviewed P1 wordcraft rows still say *Iqtibos*: Insert Citation, Mark Citation, Citations & Bibliography, Move Past Citation, and Skip Citations and Bibliography. The References tab now mixes both terms. Re-align those P1 rows, or reverse the glossary.
2. **Content control: conflict between P1 and the glossary.** The glossary says *Tarkib boshqaruvi elementi*, and P2 rows `dialogs.rs:1111` and `dialogs.rs:1287` use it. The P1 rows say *Boshqaruv elementi: …*. Align them.
3. **Structure, in equations:** the P1 equation rows use *Struktura*, and P2 follows them (*Operator strukturalari*). The glossary has only Structure = *Tuzilma* (pdf). Add a `context` row: Structure (equation) = *Struktura*.
4. **New terms:**

   | English | uz-Latn |
   |---|---|
   | Wildcards | Shablon belgilari |
   | Leader (tab) | Toʻldiruvchi |
   | Banded rows / columns | Navbatlashuvchi qatorlar / ustunlar |
   | Elbow connector | Tirsakli ulagich |
   | Sparse matrix | Siyrak matritsa |
   | Hue / Lum / Sat | Tus / Yorqinlik / Toʻyinganlik |
   | Courtesy title | Murojaat |
   | Subtle / Intense (style names) | Nozik / Kuchli |
   | Series {n} (chart) | Maʼlumotlar qatori {n} |
   | Manual line break | Majburiy satr uzilishi |

5. **Language names:** use the CLDR `uz` forms ending in *-cha* (*Nemischa (Germaniya)*, *Xitoycha (soddalashgan)*), with an upper-case first letter in lists. Hebrew and Arabic as Unicode subsets are *Ivrit* and *Arab*.
