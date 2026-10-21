# Unsure P2 translation rows that were not changed (2026-10-21, agent)

The "unsure" rows of `notes-*.md` (about 50) were reviewed. Decided and kept (fine as they are): Table of Contents (`Mundarija\nkiritish`), Scripts, Through, Log, Craft (probe string, not UI), Warp, Cameo, Baseline, By/At, In/Out (effect), Seek, Poster Frame, Commit Composition, End trim, Foreground, Compound type, SmartArt text, Photo Album, Small caps, Convert, Start, Save (M4), Identity, Creator/Producer, Shadings/Document Overhead, marketing wording, E-Sign, the tab-order wording and the CLDR weekday abbreviations.

Changed on review:
- gridcraft `964fa0ccabca` `Series{n}`: now `Maʼlumotlar qatori {n}` (not *Qator*, which is Row; same as wordcraft `Series {n}`).
- AutoCorrect, Command Palette, Content control (decided 2026-10-21, agent, owner-delegated): the reviewed P1 catalog form wins, so *Avtomatik tuzatish*, *Buyruqlar paneli*, *Boshqaruv elementi*; the glossary keeps the old form as `alt-*` and the P2 rows that used it were changed.
- pdfcraft `8c0604ac0ec2` / `3f02d96f8cce` `On` / `Off` (animation effects): `Yoniq` / `Oʻchiq` (a symmetric pair; matches the deckcraft *yoniq*).

## Still open (decision or an outside check needed)

| Scope / id | English | Current uz-Latn | Reason |
|---|---|---|---|
| shared `51b41f5a4e1d` | Stacked | Yigʻma | One row, two senses: chart type (gridcraft) and text direction (deckcraft *Ustun shaklida*). Needs a `context` split in the code |
| shared `e81790f8017e` | To: | Gacha: | Reads badly in the gridcraft Solver line. Needs a context split |
| shared `cf9639c75ff2` | Decimal | Oʻnli son | The deckcraft tab-stop sense wants *Oʻnli ajratgich boʻyicha* (now the glossary has `Decimal Tab`). Needs a context split |
| gridcraft `01d7e6e3abc4` | applies to | qoʻllanish sohasi | Fragment glued by code (`dialogs.rs:2233`); a whole-string template is needed |
| deckcraft `1e8c747cf2b5` | With (AutoCorrect) | Nimaga | No natural one-word label; *Bilan* / *Oʻrniga* read worse |
| deckcraft `74a064e5a0e3` | Justify low | Kenglik boʻyicha (kam) | No Uzbek term for Kashida low justification |
| deckcraft `6f29a49c2574`, `189a4761b73c` | Stealth / Stealth end | Oʻtkir / Oʻtkir uch | Describes the shape; no established term |
| deckcraft `f78f6448ffd2`, `6ec1b3742f90`, `7a67a8bafbca` | Subtle / Exciting / Dynamic Content | Sokin / Taʼsirli / Dinamik tarkib | Transition group names; no source |
| deckcraft `8673cc01977c`, `d9d68da3ef0f`, `f2851cbb1bf8` | Summary / Section / Slide Zoom | Xulosa / Boʻlim / Slayd masshtabi | A navigation-link object, not a zoom level |
| deckcraft `633a87f557b7` | `  Present  ` | `  Namoyish qilish  ` | Longer than the English; check the button width on screen |
| wordcraft `311bd57b4ddf` | Strong | Yaqqol | Bold character style; no better word found (*Kuchli* = Intense, *Qalin* = Bold) |
| wordcraft `2297175b36af` + 5 | Inverse Sine ... Cosecant Function | Teskari sinus funksiyasi ... | Uzbek textbooks say *arksinus*; the 8 hyperbolic ones have no such name, so *Teskari* was kept for all |
| pdfcraft `fb8fd0c81ca2` | `file.word_count` (@id) | Soʻzlar soni | Ellipsis dropped because lint compares an id row; restore `…` once lint exempts `@id` rows |
| pdfcraft `d034f54dc0b6` | of | / | No caller found in upstream |
| pdfcraft `af86edc8617f`, `474b75010f91`, `600540bcdcfa`, `9de2a61a3e1e`, `22abd97e91d8`, `5ede26ebdd3d`, `812d0144d795`, `85f4e948a28c`, `c70d52274924`, `8866feea99d8`, `c71e05afad5f` | label fragments around number fields and drop-downs | various | Uzbek *...dan ...gacha* cannot be split across widgets; needs UI templates |
| pdfcraft `4128397204d3` | Part of | Quyidagi loyiha tarkibida | Followed by the logo; may be dropped by the rebrand |
| pdfcraft `12c84bbd4115` (+2), `fb1b03dc83fa` | `{verb} Header and Footer`, `{verb} as "{name}"` | word order changed | `{verb}` is a spliced English word in code |
| pdfcraft `3e19ded6700d` | Special | Tayyor niqoblar | Format category (SSN, zip, phone); *Maxsus* is Custom |
| pdfcraft `0605ff70b407`, `27ec123af91f`, `efd47ba37413` | Beveled / Inset / Butt | Qiya kesikli / Botiq / Toʻmtoq | No established term |
| pdfcraft `effea86bf527`, `534f67da542e`, `bca282026300`, `4878b3a754da` | Distinguished name / Certificate authority / End entity / Issued to | see glossary | Verify against LEX (law ЎРҚ-793 may say *roʻyxatga olish markazi*) |
| pdfcraft `98804642f3c4`, `bb3667d7b811`, `f0c0add9657f` | Minimize / Restore / Maximize | Yigʻish / Qayta tiklash / Yoyish | Window controls; the words also mean Collapse / Expand |
| deckcraft `13630f0167b5`, `b4aed37c1024` | In / Out (effect direction) | Ichkariga / Tashqariga | Alternative *Yaqinlashish* / *Uzoqlashish* |
| pdfcraft tab-order rows (5) | tab order (lower-case) | Tabulyatsiya tartibi | Lint workaround: Latin "Tab" is allowed in Cyrillic only if the English has it. Decide whether lint should allow the kept token |
