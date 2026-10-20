# deckcraft P2 translation notes (2026-10-21)

| | |
|---|---|
| **File** | `i18n/p2/deckcraft.csv`, in the review-import column format (`key … glossary terms`), BOM, CRLF |
| **Rows** | 997 = every P2 row of `i18n/deckcraft.tsv` whose status was `todo` (the 5 obsolete rows and the one P2 row already reviewed in Phase 1, "DeckCraft", are not included) |
| **Status** | `reviewed:agent-opus:2026-10-21` on every row, in both scripts. Agent review under the owner's delegation, not an owner review |
| **Method** | Glossary (`i18n/glossary.tsv`) first, then the forms that the reviewed P1 rows of deckcraft, shared, wordcraft and gridcraft use. I read upstream `crates/ui-egui` / `crates/engine` at each `where` site when the English was ambiguous, because there is no ru column. No web source and no Microsoft Terminology entry was consulted for this file. uz-Cyrl comes from `i18n/tools/p1_drafts.py` (`latn_to_cyrl`) plus the soft-sign loans (альбом, компьютер, панель, пиксель), and I then read every pair by hand |
| **Self-check** | Run with Python on the CSV (scratch script, not committed). It checks coverage (exactly the 997 ids), status, `en_hash` (same method as `review.rs`; tested on batch-01..03, with 0 mismatches over 1,723 rows), placeholders `{…}` / `%s` / `&`, `\n` and `\t` counts, leading and trailing spaces, trailing `…` and `:`, the apostrophe rules of `lint.rs` (U+02BB only after o/g, U+02BC after a letter, no ASCII or curly apostrophe and no backtick next to a letter in either script), no Cyrillic in Latin, no Latin word in the Cyrillic text unless the English has it, and the letter-mapping skeleton. Result: **0 errors**. The 48 warnings are expected: ц in -tsiya loans (анимация, табуляция, позиция), the obyekt/объект stem pair, and correct е after a vowel (градиент, проектор) |

## 1. Kept verbatim in both scripts (71 rows)

These rows are not UI words, or they must not change:
- Language autonyms in `squiggles.rs` (23 rows).
- Photo-album theme names Nocturne, Meadow, Slate, Harbor, Coral Reef and Ember. The model stores them as theme ids (`model/src/theme.rs`, `templates.rs`).
- Key names Escape, End, PageUp, PageDown, F2, F5 and F7. `lib.rs` maps egui keys to session key names with them.
- Icon glyphs B, A, S, Ω, Aa, Abc and A-.
- Font-matching ids in `theme.rs`: Inter, SemiBold, Regular, BIZ UDPGothic, Noto Sans Arabic and `fallback {} {} {}`.
- The command-parameter schemas in `lib.rs` (`{on?: bool}`, `{pane: format|…}`, …).
- The numbering samples (I. II. III., a) b) c) …).
- `{path}: {e}`, `{label} ({})`, ` — {n}`, ` ({sha}, {date})`, `• {d}`, `URL…` and `RRGGBB`.

Eight rows have a Latin form equal to the English, with a Cyrillic form of their own: Radial, Material, Video, Audio and Media,
plus `{r} pt`, `{w} pt` and `{:.0} pt{side}` (Cyrillic пт).

## 2. Rows that need a code change, not a better translation

The template splices English grammar into the string. The placeholders are kept, because lint and the runtime catalog reader drop a row whose placeholders differ. On screen, these rows still show an English fragment.

| id | English | uz-Latn | Problem |
|---|---|---|---|
| 2198b175545c | `{n} Column{}` | `{n} ta ustun{}` | `{}` is the English plural "s" (ribbon.rs:772) |
| 101d8cc1dee7 | `{n} Slide{}` | `{n} ta slayd{}` | same (ribbon.rs:3327) |
| 8d97086a708a | `{names} {it} installed … uses {them} …` | `{names} {it} bu kompyuterda oʻrnatilmagan. {them} ishlatilgan matn …` | `{it}` = isn't/aren't, `{them}` = it/them (toast.rs:28) |
| cc386cbf509d | `Align text to the {l} of the cell` | `Matnni katakda tekislash: {l}` | `{l}` = Top/Center/Bottom, and the whole string is lower-cased. It reads correctly only if `{l}` is translated too |

Suggestion for the patch queue: give these sites count-neutral templates (`Columns: {n}`), or run `{l}` through `t()` first.

## 3. Unsure rows (agent judgement; owner feedback welcome)

| id | English | uz-Latn | Why unsure |
|---|---|---|---|
| 1e8c747cf2b5 | With (AutoCorrect: Replace … With …) | Nimaga | No natural one-word label. *Bilan* and *Oʻrniga* read worse next to *Almashtirish* |
| 74a064e5a0e3 | Justify low | Kenglik boʻyicha (kam) | Kashida "low" justification. No Uzbek term exists |
| 6f29a49c2574, 189a4761b73c | Stealth / Stealth end (arrowhead) | Oʻtkir / Oʻtkir uch | Describes the shape. No established term |
| 6b60972ce694 | Warp (text effect) | Egish | Alternative: *Qiyshaytirish* |
| bbdfdcced51f | Cameo (live camera feature) | Kameo | Kept as a feature name. Alternative: *Kamera* |
| f78f6448ffd2, 6ec1b3742f90, 7a67a8bafbca | Subtle / Exciting / Dynamic Content (transition groups) | Sokin / Taʼsirli / Dinamik tarkib | No source for the group names |
| 8673cc01977c, d9d68da3ef0f, f2851cbb1bf8 | Summary / Section / Slide Zoom | Xulosa / Boʻlim / Slayd masshtabi | Follows the glossary (Zoom = Masshtab). But this is a navigation-link object, not a zoom level |
| f2edbb2e3e78 | Baseline (font dialog: None/Super/Sub) | Tayanch chiziq | Alternative: *Indeks* |
| f861eaea6158, 6f6a85f3f4f9 | By / At (paragraph dialog values) | Kattaligi / Qiymati | Labels next to a number field |
| 13630f0167b5, b4aed37c1024 | In / Out (effect direction) | Ichkariga / Tashqariga | Alternative: *Yaqinlashish* / *Uzoqlashish* |
| 2b8bfb1624dc | Seek (media command) | Pozitsiyaga oʻtish | |
| b5ab3d39248e | Poster Frame | Muqova kadri | |
| 1456aa7d28f0 | Commit Composition (IME) | Kiritilayotgan matnni tasdiqlash | |
| 6d78a3b471a3 | End trim (s): | Oxiridan qirqish (s): | Read as "seconds trimmed from the end" |
| 6d73dfc4eb60 | Foreground (pattern fill colour) | Oldingi plan | |
| 26182d798c68 | Compound type (line) | Qoʻshma chiziq turi | |
| 1055edfcd0ca | Type and organise the diagram's text (SmartArt) | Grafika matnini kiritish va tartibga solish | Diagramma = Chart, so the SmartArt "diagram" is *grafika* |
| 633a87f557b7 | `  Present  ` (accent button) | `  Namoyish qilish  ` | Glossary Present. Longer than the English; check the width |
| 1fdde1c6c5a9 | Photo\nAlbum | Foto-\nalbom | The `\n` had to stay, so the one word is hyphenated across the two lines |

## 4. Choices applied throughout

- **Code tokens in backticks** (control channel and command errors: ``missing `command` ``) are written «command». lint.rs rejects a backtick next to a letter in both scripts. The token itself stays Latin in Cyrillic.
- **Units:** pt = Cyrillic пт; in = *dyuym* (glossary Inch); px = *piksel*; s = Cyrillic с; ppi, KB and MB stay Latin.
- **No suffix after a placeholder.** The text is rephrased instead: "Oʻzgarishlar saqlansinmi: {what}?", "Saqlandi: {path}", "{MIN}–{MAX} piksel". Numeric ordinals take the hyphen: `{}-slayd oʻchirildi`.
- **Show toolbar "X on" states** use *yoniq*, as the shared P1 row "yoniq qolishi uchun" does: "Qora yoniq", "✏ yoniq".
- **Strings that never reach `t()`** (the control channel in control.rs, the temp file name `{safe} (print).pdf`, the fallback name "dropped file") are translated anyway. This is harmless, because the i18n module never routes them.

## 5. Glossary suggestions (glossary.tsv not edited)

New terms used consistently in this file (candidate P2 glossary rows):

| English | uz-Latn | uz-Cyrl |
|---|---|---|
| Guide (singular) | Yoʻnaltiruvchi chiziq | Йўналтирувчи чизиқ |
| Handout Master | Tarqatmalar namunasi | Тарқатмалар намунаси |
| Notes Master | Qaydlar namunasi | Қайдлар намунаси |
| Master View | Namuna rejimi | Намуна режими |
| Action Button / Action Settings | Amal tugmasi / Amal sozlamalari | Амал тугмаси / Амал созламалари |
| Insertion point | Kiritish nuqtasi | Киритиш нуқтаси |
| Gradient stop | Gradiyent nuqtasi | Градиент нуқтаси |
| Tab stop kinds Left/Center/Right/Decimal | Chap / Markaziy / Oʻng / Oʻnli tabulyatsiya | Чап / Марказий / Ўнг / Ўнли табуляция |
| Hanging (indent) | Osma | Осма (follows wordcraft P1 "Osma chekinish") |
| Caps | Bosh harflar | Бош ҳарфлар (follows wordcraft P1 All Caps / Small Caps) |
| Loose / Very loose | Siyrak / Juda siyrak | Сийрак / Жуда сийрак (pairs with Tight = Zich) |
| Wavy | Toʻlqinsimon | Тўлқинсимон |
| Depth / Lighting / Material | Chuqurlik / Yoritish / Material | Чуқурлик / Ёритиш / Материал |
| Kiosk | Kiosk | Киоск |
| Thread (comment) | Muhokama | Муҳокама |

Conflicts the owner or the step-2 shared agent should settle:
1. **AutoCorrect.** The glossary says *Avtotuzatish*, but the reviewed shared P1 "AutoCorrect Options" says *Avtomatik tuzatish parametrlari*. For the two deckcraft rows that pair with it (4f27570eece4, be8624a3e75a), I followed the P1 row. AutoRecover keeps the glossary *Avtotiklash*. Suggestion: change the glossary AutoCorrect to *Avtomatik tuzatish*, or fix the P1 row.
2. **Eraser.** I used the glossary form *oʻchirgʻich* ("Jadval oʻchirgʻichi", 571f8b879d99). The shared P1 row still has the slip *Oʻchirgich* (sources.md §4).
3. **Format Shape** (pane) and the **Shape Format** tab (shared P1) are both *Shakl formati*. This is the same pattern as the glossary's Format Background = *Fon formati*, and it is acceptable. Distinguishing them would need *Shaklni formatlash* for the pane.
