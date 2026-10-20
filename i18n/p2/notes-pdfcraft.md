# pdfcraft P2: translation notes (agent, 2026-10-21)

| | |
|---|---|
| **Output** | `i18n/p2/pdfcraft.csv`: 1,978 rows (every P2 row of `i18n/pdfcraft.tsv`), review-import columns (`key … glossary terms`), scope `pdfcraft`, pass `both`, `en_hash` computed from the unescaped English as `review.rs` does. uz-Latn and uz-Cyrl filled in every row; status `reviewed:agent-opus:2026-10-21`. |
| **Not touched** | `i18n/*.tsv`, `i18n/glossary.tsv`. Nothing built, nothing committed. |
| **Sources** | Glossary (P1 and P2 rows), the reviewed P1 pdfcraft rows, upstream ru for context, the upstream `where` code for fragments. CLDR `uz.xml` / `uz_Cyrl.xml` at commit `a70bfe93` for months, weekdays and date patterns. LEX terms through the glossary. No MS term was used. LibreOffice, ONLYOFFICE, Mozilla, KDE and GNOME were not consulted. |
| **Cyrillic** | Derived from the Latin by the `i18n/tools/p1_drafts.py` letter mapping plus the hand exceptions (soft sign on standalone Russian loans: Пароль, Панель, Модель, months; компьютер, мультимедиа, фильтр, -кция, Субъект). Latin tokens keep their script with a joined suffix (PDFни, PdfCraftга, IDлар, Acrobatнинг). Every value was then checked by the script below. |

## Import caveat (blocking)

`cargo xt i18n review-import` (`xtask/src/i18n/review.rs:357`) accepts only `reviewed`, `ok` or `approved` in the status column, and turns them into
`reviewed:owner:<date>`. The requested value `reviewed:agent-opus:2026-10-21` is not recognised. A changed cell with an unrecognised verdict is
stored as `draft:owner-fix:<date>`. So this CSV needs an importer that writes the agent status, or a direct apply step. Do not run the stock
`review-import` on it as it stands. Running it with `status=reviewed` would mislabel the rows as owner-reviewed.

## Self-check (scratch script, mirrors `xtask/src/i18n/lint.rs`)

- 0 errors: placeholder sets, `&` accelerators, `\n` and `\t` counts, trailing `…`, `:` and whitespace, three ASCII dots, product tokens, plural form count (2), Cyrillic letters in the Latin text, foreign Latin words in the Cyrillic text, and apostrophes. Latin uses U+02BB only after o/g (1,000 uses) and U+02BC only after a letter (54). The Cyrillic text has no apostrophes.
- The only backticks are the 4 in `` `{command}` {when} ``, copied from the English.
- 9 pair warnings, all intended (same kind as P1): sentabr/сентябрь and oktabr/октябрь (×2 each), Subyekt/Субъект, Yakuniy subyekt/Якуний субъект, and the CLDR short weekdays Ch/Чо, Sh/Ша, Ya/Як.

## Conventions used

- **Plurals:** the 13 `@plural` rows carry `one|other` with the same text twice (`{n} ta fayl|{n} ta fayl`). The noun stays singular after a number throughout.
- **Page numbers:** ordinal form `{n}-sahifa`, `{p}-sahifadagi`. "X of Y" becomes `X/Y` (`{p}/{n}-sahifa`, `fayl {d}/{t}`, `Rasm {i}/{n}{page}`).
- **Dates (CLDR uz):** standalone months are capitalised (`Yanvar`, `{month}, {y}`). The `calendar date` rows use the lower-case format forms (`yanvar`), with `{day}-{month}, {y}` (yMMMMd `d-MMMM, y`). The Cyrillic follows P1 (январь with ь). Calendar weekdays use the CLDR short forms: Latin Ya Du Se Ch Pa Ju Sh; Cyrillic Як Ду Се Чо Па Жу Ша.
- **Signatures:** Signer = *Imzo qoʻyuvchi*, Issuer / Issued by = *Sertifikat beruvchi*, Subject = *Subyekt*, Digital ID = *Raqamli ID*, Self-signed = *Oʻzi imzolagan*, validity = *Amal qilish muddati (boshlanishi / tugashi)*, Public key = *Ochiq kalit*, Revision = *Tahrir*.
- **Redaction:** *Oʻchirib yopish*, as in the glossary and P1. The task text mentions *maxfiylashtirish*, but the glossary and the P1 decision (review-agent-2026-10-18 §2.1) reject it, so it was not used.
- **Accessibility checker:** Tagged = *teglangan*, untagged = *teglanmagan*, alternate text = *muqobil matn*, artifact = *artefakt*, assistive technology = *yordamchi texnologiyalar*, screen reader = *ekrandan oʻqish vositasi*, output intent = *chiqish maqsadi*.
  - Structure tag names (Table, TR, TH, TD, THead, TBody, TFoot, L, LI, Lbl, LBody, Form, H1, Summary, MarkInfo) stay in Latin in both scripts.
  - Annotation = *izoh* (glossary).
  - Content = *tarkib* in the checker; *mazmun* in editing commands, as in P1 (Add content = Mazmun qoʻshish).
- **Embedded** (files, fonts, thumbnails, print settings) = *joylashtirilgan*. P1 used *ichki* only for the embedded timestamp and index.
- **Scrolling** = *aylantirish*, following the reviewed P1 pdfcraft rows (Continuous scrolling = Uzluksiz aylantirish).
- **Undo-history labels** (`lib.rs:1770–1896`) are verbal nouns (*Sahifani oʻchirish*), because they are shown after "Qaytarish:" / "Qayta bajarish:".
- **Keys:** Space = *Probel*. Home, End, Esc, Delete, Ctrl and Shift stay as they are.

## Rows I am unsure about

| id | context | English | uz-Latn | Why |
|---|---|---|---|---|
| `fb8fd0c81ca2` | @id | file.word_count | Soʻzlar soni | The display text is "Word count…" (ru «Количество слов…»). The ellipsis was dropped because lint compares against the id `file.word_count` and would fail with "trailing ellipsis differs". Restore `Soʻzlar soni…` once lint exempts `@id` rows. |
| `d034f54dc0b6` | | of | / | There is no `where` and no caller in the upstream code; "X of Y" was assumed. |
| `af86edc8617f`, `474b75010f91`, `600540bcdcfa`, `9de2a61a3e1e` | | Pages from / From / to / to {n} | Sahifalar oraligʻi / Oraliq / – / – {n} | Label fragments around number fields. Uzbek *…dan …gacha* cannot be split across widgets. |
| `22abd97e91d8`, `5ede26ebdd3d` | | of {pages} / of {name} | (jami {pages}) / (jami {name}) | As above. |
| `812d0144d795`, `85f4e948a28c`, `c70d52274924` | | Limit of / characters / Comb of | Chegara / ta belgi / Katakcha | Fragments around a number field: "Chegara [10] ta belgi". |
| `8866feea99d8`, `c71e05afad5f` | | Value is the / of the following fields: | Qiymat — / quyidagi maydonlar boʻyicha: | Fragments around the sum/product/average drop-down. |
| `4128397204d3` | | Part of | Quyidagi loyiha tarkibida | Followed by the ArtCraft logo. That sentence may be dropped by rebrand. |
| `12c84bbd4115` (+ Watermark, Background) | | {verb} Header and Footer | Kolontitullar: {verb} | `{verb}` is Add/Update (verbal nouns). The word order was changed. |
| `fb1b03dc83fa` | | {verb} as "{name}" | {verb}: "{name}" sifatida | Same reason. |
| `270adfa13b1f` | | E-Sign | Elektron imzo | A tab name. *E-imzo* is shorter, but it collides with the state E-IMZO service name. |
| `603f2798d0ed` | | Convert | Aylantirish | Glossary form. Users may expect *Konvertatsiya* for a tab name. |
| `254cecf54552`, `48a9f0da341b` | action wizard / – | Start | Boshlash | ru «Начало» for the second row. Its use was not found. |
| `7e2171259b08` | | Save (M4) | Saqlash (M4) | Meaning of "M4" unclear (milestone?). Kept verbatim. |
| `3e19ded6700d` | | Special | Tayyor niqoblar | Format category (SSN, zip, phone). *Maxsus* is already Custom. |
| `24928761352e` | | Identity | Shaxsiy maʼlumotlar | A Preferences section (name on comments). |
| `cf7d7d197f76`, `2571046eeb42` | | Creator / Producer | Yaratuvchi dastur / Ishlab chiqaruvchi dastur | PDF metadata fields. |
| `0605ff70b407`, `27ec123af91f`, `efd47ba37413` | | Beveled / Inset / Butt | Qiya kesikli / Botiq / Toʻmtoq | Border styles and a line ending. No established term. |
| `322915ee483a`, `8e8250f37286` | | Shadings / Document Overhead | Soyalashlar / Hujjatning xizmat maʼlumotlari | Space-audit categories. |
| `278993e62524`, `9af614be8f25` | | A clean-room… / An open-source PDF workbench… | «Toza xona» usulida… / …vositalari toʻplami… | Marketing wording. |
| `effea86bf527`, `534f67da542e`, `bca282026300`, `4878b3a754da` | | Distinguished name / Certificate authority / End entity / Issued to | Farqlovchi nom / Sertifikat beruvchi markaz / Yakuniy subyekt / Sertifikat egasi | Certificate-viewer terms. These were not found in the glossary or in LEX through the glossary. Law ЎРҚ-793 may say *roʻyxatga olish markazi* for a CA. A check there is advised. |
| `98804642f3c4`, `bb3667d7b811`, `f0c0add9657f` | | Minimize / Restore / Maximize | Yigʻish / Qayta tiklash / Yoyish | Window controls. *Yigʻish* / *Yoyish* are also Collapse / Expand. |
| `8c0604ac0ec2`, `3f02d96f8cce` | animation effects | On / Off | Yoqilgan / Oʻchiq | Asymmetric pair. *Yoqiq* is colloquial. |
| `d40c89a31653`, `d7181d07bb3f`, `b6273d95f08d`, `5c9b7adf3e3f`, `74339c0fef49` | | …tab order (lower-case "tab") | tabulyatsiya tartibi | P1 says *Tab tartibi* with "Tab" kept in Latin. Lint allows a Latin word in the Cyrillic text only if the English has it, and these English strings have lower-case "tab". So *Tabulyatsiya tartibi* / *Табуляция тартиби* was used. |
| `871ae4c6717a` etc. | calendar weekday | Mo … Su | Du … Ya | The CLDR short Cyrillic forms differ from the mapped Latin (pair warnings). |

## Glossary suggestions (not applied)

1. **Signer conflict:**
   - The glossary says *Imzo qoʻyuvchi* (LEX), and P2 uses it.
   - 14 reviewed P1 pdfcraft rows say *Imzolovchi* (signature-verification messages).
   - Align the P1 rows to the glossary, or record *Imzolovchi* as the accepted alternative.
2. **Callout conflict:**
   - The glossary says *Chiqma izoh*, and P2 uses it.
   - P1 `dd1fa99dc6e0` "Add a callout" says *Koʻrsatkichli izoh qoʻshish*.
3. **Scroll:**
   - The glossary says *Varaqlash*.
   - The reviewed P1 pdfcraft rows say *aylantirish*, and P2 follows P1.
   - Pick one. *Aylantirish* is also Convert in the glossary, so *varaqlash* may be the better long-term choice.
4. **Add these terms:**
   - Tab order = *Tab tartibi* (alt *Tabulyatsiya tartibi*, for lint when the English has lower-case "tab")
   - Embedded = *Joylashtirilgan*
   - Opacity = *Noshaffoflik*
   - Public key = *Ochiq kalit*; Private key = *Yopiq kalit* (LEX)
   - Serial number = *Seriya raqami*
   - Key usage = *Kalitning qoʻllanilishi*
   - Issued to = *Sertifikat egasi*
   - Certificate authority = *Sertifikat beruvchi markaz* (verify against LEX)
   - Milestone = *Bosqich*
   - Appearance stream = *Koʻrinish oqimi*
   - Render(ing) = *Chizish*
   - Digest (hash value) = *Xesh qiymati*
   - Minimize / Maximize / Restore (window) = *Yigʻish / Yoyish / Qayta tiklash*
   - Space (key) = *Probel*
5. **Small P1 slips found while reading:**
   - "PDF ni avtomatik teglash", "PDF ni himoyalash" and "JavaScript dan foydalanish" should be joined: *PDFni*, *JavaScriptdan*.
   - "Find untagged content" = *Tegsiz tarkibni topish*; the glossary says Untagged = *Teglanmagan*.
6. **Lint:** exempt `@id` rows from the ending-parity check, because the source is a command id and not the displayed text.
