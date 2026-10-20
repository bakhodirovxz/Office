# Uzbek UI terminology: sources, method and the step-2 work split (step 1, 2026-10-21)

| | |
|---|---|
| **Goal** | Translate the remaining P2 UI strings (uz-Latn and uz-Cyrl) from authoritative sources. Step 1 (this file): find the sources, settle their licences, and extend the glossary. Step 2: parallel translation agents, one catalog file each. |
| **Mandate** | CLAUDE.md gate decisions: the Uzbek review is delegated to the agent. Status `reviewed:agent-opus:2026-10-21`, never an owner review. Default alphabet: the 1995 Latin alphabet. |
| **Result** | `i18n/glossary.tsv`: 156 → **783 terms**. 627 terms added, all with priority `P2`. A new `source` column holds the citations. The 156 earlier terms are unchanged except for the new column. |
| **Not done here** | No P2 catalog row is translated; that is step 2. `i18n/glossary-use.tsv` is not refreshed (see §5.3). |

## 1. Sources and licence verdicts

"Consult" means we read a source to choose one term at a time. "Bundle" means we ship or commit the source's text. Words and short terms are
not protected on their own, but a collection is. So the citations in the `source` column name an id, and we never commit a source's term list.

| Id | Source | URL and revision | Licence (quoted) | Verdict |
|---|---|---|---|---|
| **CR** | Chromium UI translations, Uzbek (`uz`, Latin): `ui_strings_uz.xtb` (242 strings), `components_strings_uz.xtb` (4,856), `generated_resources_uz.xtb` (10,785), `chromium_strings_uz.xtb` (805) | https://github.com/chromium/chromium/tree/main/ui/strings/translations , `components/strings/`, `chrome/app/resources/`. Main branch, commit `e9d982edeed4194dbab6520afe28a3bc81ba171a` | BSD-3-Clause, in the repository `LICENSE`: "Redistribution and use in source and binary forms, with or without modification, are permitted provided that … Redistributions of source code must retain the above copyright notice …" | **Usable.** We may consult and quote it, and we could bundle individual strings with the BSD notice. Google localises it, and people in Uzbekistan see it every day (Chrome, Android WebView). We use it per term only. |
| **AOSP** | Android Open Source Project, Uzbek (`values-uz`, Latin): `frameworks/base/core/res`, `packages/PrintSpooler`, `packages/apps/DocumentsUI`. 2,544 aligned en/uz pairs. | https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/res/res/values-uz/strings.xml (and `PrintSpooler/res/values-uz`). Commit `1cdfff555f4a21f71ccc978290e2e212e2f8b168`; https://android.googlesource.com/platform/packages/apps/DocumentsUI/+/refs/heads/main/res/values-uz/strings.xml , commit `6b61c8e5a7bcbaaacf4afb8b695c24ca56b369e2` | Apache-2.0, in every file header: "Licensed under the Apache License, Version 2.0 …" | **Usable.** Same as CR. It is the reference for the print-dialog terms (copies, paper size, two-sided, destination) and the file-manager terms (rename, sort direction). |
| **CLDR** | Unicode CLDR, `common/main/uz.xml` and `uz_Cyrl.xml`: date fields, relative days, weeks and months, language and territory names | https://github.com/unicode-org/cldr/tree/main/common/main , commit `a70bfe93a2c74473baaedf01eea43b027e782979` | Unicode License V3, in the repository `LICENSE`: "All other content in this repo is released under UNICODE LICENSE V3". The UTS #35 *specification text* is under separate, restrictive terms; the data files are not. | **Usable.** We may consult and bundle it with the Unicode notice. It was already used in Phase 1 for date abbreviations. |
| **LEX** | lex.uz, official legal texts in both scripts (Latin `/docs/-<id>`, Cyrillic `/docs/<id>`): Law ЎРҚ-793 *Elektron raqamli imzo toʻgʻrisida* (12.10.2022) `-6234904`; Law 611-II *Elektron hujjat aylanishi toʻgʻrisida* (29.04.2004) `-165079` | https://lex.uz/docs/-6234904 , https://lex.uz/docs/6234904 , https://lex.uz/docs/-165079 , https://lex.uz/docs/165079 | Not protected. Copyright law ЎРҚ-42 (https://lex.uz/docs/-1022944), article 8: "Quyidagilar mualliflik huquqi obyektlari boʻlmaydi: rasmiy hujjatlar (qonunlar, qarorlar, toʻxtamlar va shu kabilar), shuningdek ularning rasmiy tarjimalari" | **Usable without restriction.** It is the authority for signature and document-law terms (elektron raqamli imzo, yopiq/ochiq kalit, amal qilish muddati, imzo qoʻyuvchi, asl nusxa, qabul qilib oluvchi). It is the "government" voice the owner asked for. |
| **MS** | Microsoft Terminology Collection, `UZBEK (LATIN).tbx` (15,999 entries) and `UZBEK (CYRILLIC).tbx` (910 entries); `RUSSIAN.tbx` was used for sense checks | https://learn.microsoft.com/en-us/globalization/reference/microsoft-terminology → https://download.microsoft.com/download/b/2/d/b2db7a7c-8d33-47f3-b2c1-ee5e6445cf45/MicrosoftTermCollection.zip (TBX files dated 2024-11-07). The zip contains no licence file. | Microsoft Learn Globalization License, https://learn.microsoft.com/en-us/globalization/license-agreement (dated 2023-06-27). It covers "documentation, terminology and other product information (collectively the 'Globalization Materials')". It grants "a non-exclusive, nontransferable, royalty-free, license to use, copy, and distribute the Globalization Materials". For any materials you distribute, you must "require distributors and external end users to agree to terms that protect it at least as much as this agreement", "indemnify, defend, and hold harmless Microsoft", and include the notice "Entry from Microsoft Learn's Globalization Essentials. © 2023 [or applicable year] Microsoft Corporation. All rights reserved." §2: "You may not (i) publish the Globalization Materials for others to copy; or (ii) sell, rent, lease or lend the Globalization Materials in whole." | **Consult per term: yes. Bundle: no.** Those conditions conflict with our MIT OR Apache-2.0 layer: protective terms passed down, an indemnity, and no publishing for others to copy. So the TBX files, and any extract or term list taken from them, are never committed, bundled or published. The `source` column cites an MS term id (`MS:469081`) as evidence of agreement or of a rejected alternative, never as copied content. The files stay in the session scratchpad. MS Uzbek terminology often departs from current Uzbek usage (Cell = *uya*, Worksheet = *bloknot*, Bold = *quyuq*, Options = *Opsiyalar*), so it is a cross-check and not the default. Clean-room note: this is a published terminology list, not Microsoft Office code or its UI string catalog, and no whole UI string was taken from it. |
| **UNDP** | *Axborot-kommunikatsiya texnologiyalari izohli lugʻati*, UNDP Digital Development Initiative and the Uzbek Communications and Information Agency, Tashkent 2004. Includes an En-Uz-Ru appendix. | https://www.undp.org/sites/g/files/zskgke326/files/migration/uz/un_uzb_Glossary_of_ICT_terms_in_Uzbek.pdf | "Copyright @ 2004 UNDP Digital Development Initiative Programme". The PDF gives no reuse licence. | **Consult per term only.** It is a government co-publication, so it carries authority, but it is from 2004 and pre-dates today's office UI vocabulary. The text extraction of the appendix is misaligned, so it was used only for hand-checked terms (1 citation: *giperhavola*, p. 372). |

### 1.1 Sources looked for and not usable

| Source | Finding | Verdict |
|---|---|---|
| Ministry of Digital Technologies and the Uzbek Language Development Fund, *Ахборот технологиялари атамаларининг изоҳли луғати* (about 10,000 terms, prepared with UNICON.UZ, presented 27 Jan 2023) | Only news of the presentation is online: https://gov.uz/uz/digital/news/view/856 , https://uza.uz/ar/posts/axborot-texnologiyalari-atamalarining-izohli-lugati-yaratildi_449267 . No public file was found. | **Not used: not available.** It would be the strongest government source. **Owner action:** obtain a copy (the ministry or the fund). The glossary can then be re-checked against it. |
| Oʻz DSt national standards for ICT terms | Not on lex.uz. Uzstandard sells national standards; no free text was found. | Not used. |
| Cabinet terminology commission (Vazirlar Mahkamasi huzuridagi Atamalar komissiyasi) | Its mandate is described (yuz.uz, kun.uz 2024-04-20), but no approved ICT term list is published. | Not used. |
| gov.uz digital-transformation glossary | https://gov.uz/oz/ima/pages/atamalar : rating and e-government terms, not UI terms | Not needed. |
| Google Workspace / Android Uzbek glossary | Google publishes none. AOSP and Chromium are the open part of Google's Uzbek localisation. | Use AOSP and CR. |
| LibreOffice (MPL-2.0) and ONLYOFFICE (AGPL) Uzbek translations | Excluded by the clean-room rule and the licence policy | **Not consulted.** |
| Mozilla / Pontoon (MPL-2.0), KDE and GNOME (GPL), translatewiki.net | Not permissive (CLAUDE.md: only symphonia may be MPL) | **Not consulted.** |
| Microsoft Office's own Uzbek language-interface strings | Proprietary UI catalog; clean room | **Not consulted.** Only the published terminology collection was used, under its licence. |

## 2. Citation codes in the `source` column

The `source` column is the optional 15th column of the catalog format (`xtask/src/i18n/catalog.rs`, `SOURCE_COLUMN`). The reader accepts
14 or 15 columns. The writer emits the 15th column only for a file in which some row has a source, so the five app catalogs keep 14 columns.
Several citations are separated by `; `.

| Code | Meaning | How to check it |
|---|---|---|
| `CR:g#<id>`, `CR:c#<id>`, `CR:u#<id>` | Chromium message id in `generated_resources_uz.xtb`, `components_strings_uz.xtb` or `ui_strings_uz.xtb` | grit id = the first 8 bytes of MD5(English text) as a signed 64-bit number, with the top bit cleared; find `<translation id="<id>">` |
| `AOSP:fw/<name>`, `AOSP:ps/<name>`, `AOSP:du/<name>` | `<string name>` in frameworks/base core, PrintSpooler or DocumentsUI, `values-uz` | look up the name in `values/` (en) and `values-uz/` |
| `CLDR:uz/fields/<field>[/relative<n>]` | CLDR `uz.xml` `<field type>` display name or relative form | |
| `LEX:<doc id>` | lex.uz document (Latin `-<id>`, Cyrillic `<id>`) in whose text the term occurs | |
| `UNDP:p<page>` | UNDP 2004 dictionary page | |
| `MS:<term id>`, `MSC:<term id>` | Microsoft Terminology Collection uz-Latn / uz-Cyrl term id that **agrees** (same word; MS lists verbs in the *-moq* infinitive, which counts as agreement) | TBX `<term id>`; not redistributed |
| `MS-ru:<term id>` | MS Russian term id, used only to confirm the sense | |
| `CR≠`, `AOSP≠`, `MS≠` | Consulted and **rejected**; the reason is in the note | |
| `P1:<scope>/<row id>` | A P1 row reviewed on 2026-10-18 that already uses the term. The term follows it for consistency. | `i18n/<scope>.tsv` |
| `GL`, `GL:<term>` | Composed from existing glossary terms and the standard vocabulary; no external source found | |
| `agent-review-2026-10-18 (…)` | The 156 batch-0 terms: decided in Phase 1, see `docs/phase1/i18n/review-agent-2026-10-18.md` | |

Every citation was re-checked by a script against the downloaded file. A wrong id fails, and a claimed agreement whose text differs is listed
for review. All P1 claims resolve to a real reviewed row.

## 3. Method

1. **Term extraction.** All 5,303 P2 rows were tokenised. Words and bigrams occurring ≥ 3 times, plus the 1–3-word labels, gave about 720
   candidates. Terms that occur in no P2 row were dropped. 627 remain, in 16 categories: ui 97, slide 86, sheet 82, text 76, command 58,
   pdf 48, math 46, chart 23, line 20, sign 18, time 18, color 15, print 14, position 14, status 7, units 5. Senses that differ get a
   `context`: Script code/math, Key keyboard/crypto, Point unit/shape, Outline structure/shape, Accent math/color, Subject (plain)/certificate,
   Volume sound/book, Letter mail, Area measure.
2. **Lookup.** For each candidate, a script listed the uz-Latn and uz-Cyrl hits in MS, CR (by computing the grit id of the English text in
   four capitalisations), AOSP, the reviewed P1 rows, and upstream's own Russian for the same English.
3. **Decision rules**, in order:
   1. A term that the reviewed P1 rows already use stays as it is (consistency inside the app beats a better word). Exceptions are listed in §4.
   2. For legal and signature concepts, the lex.uz statute term wins.
   3. Otherwise, the form on which CR and AOSP agree, since it is what users in Uzbekistan see on their phones and browsers. Where they disagree, the form most common in Uzbek government and business usage wins, and the other goes to `alt-latn`.
   4. MS terminology is a cross-check. It is followed where it matches usage and rejected (`MS≠`) where it is a calque or a rare word (*tasma*, *uya*, *chertmoq*, *bloknot*, *nurlanish*, *tomizgʻich*, *Quyun*).
   5. Where no source has the term, it is composed from glossary terms (`GL`). Example: Bring to Front = *Eng oldinga chiqarish*, from the P1 Bring Forward.
   6. Date fields follow CLDR `uz`. When CLDR `uz_Cyrl` picks a different word (*бу ҳафта* against Latin *shu hafta*), the pair follows the Latin, so that Latin and Cyrillic agree under the letter mapping.
4. **Cyrillic.** Derived by the official letter mapping (`i18n/tools/p1_drafts.py`). Hand exceptions: the soft sign of Russian loans
   standing alone (Панель, Модель, Пиксель, Фильтр, Фотоальбом, Компьютер, Қўшимча модуль, процентиль), э after a vowel (коэффициент),
   ц in -ensial (дифференциал, экспоненциал), and Subyekt = Субъект. Every pair was then read by hand.
5. **Checks.** `cargo xt i18n check`: all 7,824 ids match. `cargo xt i18n lint`: 0 errors. One new `script-mismatch` warning is intended:
   *Subyekt* / *Субъект*, the same pattern as *obyekt* / *объект*. `cargo test -p xtask`: 58 passed, including the new
   `optional_source_column_round_trips`.

**Evidence strength of the 627 new terms:**
- 272 have at least one agreeing external source (CR, AOSP, CLDR, LEX, MS, UNDP).
- 106 rest on a reviewed P1 row only; some of those also cite a rejected alternative.
- 249 are composed (`GL`) and rest on the glossary and standard vocabulary, not on a web source. Mostly they are transparent compounds (*Toq sahifalar*, *Quyi chegara*) and maths names (*Sinus*, *Kotangens*).

The downloaded sources and scripts are in the session scratchpad. They are not committed, and the MS files must never be committed.

## 4. Findings for the owner

- **P1 spelling slip.** `shared` Eraser is *Oʻchirgich* / *Ўчиргич*. The correct form is *oʻchirgʻich* / *ўчирғич* (CR `c#1455413310270022028`,
  MS 2418959). The glossary has the correct form. Step 2 (shared) should fix that P1 row.
- **Reset.** The P1 rows say *Tiklash*; Chromium and Android say *Asliga qaytarish*. P1 is kept, and the alternative is noted. Restore and Recover take *Qayta tiklash*.
- **Margins.** The Phase 1 *Hoshiyalar* (Uzbek document-format rules) is kept against Chromium's *Maydonlar*. *Maydon* is Field in the glossary.
- **Subject vs Theme.** Both are *Mavzu* in P1 (document property vs design theme), as in Russian (Тема). A certificate subject is *Subyekt*.
- **Most valuable missing source:** the 2023 ministry IT dictionary (§1.1).

## 5. Step 2: work split

### 5.1 Rows per file (P2, untranslated)

| File | P2 rows | English words | Upstream ru reference | With placeholders | With `\n` | Special rows |
|---|---:|---:|---:|---:|---:|---|
| `i18n/shared.tsv` | 249 | 408 | 239 | 4 | 5 | Used by 2–3 apps each (115 deckcraft+wordcraft, 34 pdfcraft+wordcraft, …) |
| `i18n/wordcraft.tsv` | 1,626 | 5,437 | 1,583 | 73 | 56 | 276 equation-gallery rows (`crates/engine/src/math_gallery.rs`); 131 rows without `where` |
| `i18n/gridcraft.tsv` | 452 | 1,258 | 0 | 12 | 7 | No Russian reference at all |
| `i18n/pdfcraft.tsv` | 1,978 | 9,038 | 1,733 | 341 | 1 | 13 `@plural` rows; 66 rows with a context (certificate 17, calendar date 12, calendar weekday 7, key 7); 107 strings > 80 characters |
| `i18n/deckcraft.tsv` | 997 | 2,971 | 0 | 111 | 57 | No Russian reference; 5 obsolete rows (skip) |
| **Total** | **5,302** | **19,112** | 3,555 | 541 | 126 | |

Suggested agents: one per file. pdfcraft has the most text: it is 37 % of the rows and 47 % of the words. If wall time matters, split it in two by `where`
(for example `sign_ui.rs`, `prepare.rs` and the accessibility checker vs the rest). wordcraft can be split at the equation gallery.

### 5.2 Notes per file

- **All files.**
  - Use the glossary term (`i18n/glossary.tsv`, P1 and P2 rows) and the forms the reviewed P1 rows of the same file use.
  - Keep placeholders (`{name}`, `{}`, `%s`), `\n`, `…`, key names (Ctrl+S, ⌘, ⇧F3), file extensions and product names unchanged.
  - Uzbek keeps the noun singular after a number (*5 ta fayl*).
  - Commands take the verbal noun (*Saqlash*). Toggles take the adjective (*Tagi chizilgan*). Hints take the polite imperative (*… bosing*).
  - Derive uz-Cyrl with `i18n/tools/p1_drafts.py` rules plus the §3.4 exceptions, then run `cargo xt i18n lint`.
- **shared.** Fix the P1 Eraser spelling (§4). The rows are mostly effects (Glow *Nur sochish*, Reflection *Aks*, Soft Edges *Yumshoq chetlar*), line and border styles (Solid *Yaxlit*, Dashed *Shtrixli*, Double *Qoʻsh*), change-case items (Sentence case *Gapdagidek*; UPPERCASE and lowercase show their own case) and chart types (Clustered *Guruhlangan*, Stacked *Yigʻma*).
- **wordcraft.**
  - **Equation gallery** (276 rows): use the math terms (Integral, *Ikki karrali integral*, *Birlik matritsa*, *Teskari giperbolik …*). Script (math) is *Indeks*, Accent (math) is *Diakritik belgi*, Limit is *Limit*.
  - **References** (`dialogs_refs.rs`): Citation is *Manbaga havola*, Quote is *Iqtibos*, Caption is *Tagyozuv*, Volume (book) is *Tom*.
  - **Review:** Revision is *Tahrir*, Balloons are *Izoh oynachalari*.
  - **Mail merge:** *Xatlarni birlashtirish*, Recipient is *Qabul qiluvchi*, Letter (mail) is *Xat*.
  - **Grammar-checker messages** about English (`Use "a" before a consonant sound`): translate the explanation and keep the quoted English words.
  - **Built-in style and table-style names** (Subtle Emphasis, List Table 3 – Orange): translate only if the string is display-only. Check the `where` site; a style id that is stored in the file must stay English.
- **gridcraft.**
  - There is no Russian reference, so read the `where` site.
  - **Analysis ToolPak** (`cmd/analysis.rs`): the statistics terms (*Regressiya*, *Kovariatsiya*, *Qoldiqlar*, *Tavsifiy statistika*, *Sirpanuvchi oʻrtacha*, *Eksponensial tekislash*). Test names such as "t-Test: Two-Sample Assuming Equal Variances" keep *t-test*.
  - **Function categories** (Financial, Logical, Lookup & Reference …): use the glossary.
  - **Pivot pane:** *Yigʻma jadval*, *Hisoblanadigan maydon*, *Oraliq jami* / *Umumiy jami*.
  - **Validation operators:** *Oraligʻida*, *Teng*, *Katta*, *Kichik*.
  - The gridcraft "Solver" is *Yechim izlash* (P1).
- **pdfcraft.**
  - **@plural rows:** write `one|other` with the same text twice; lint requires exactly 2 forms.
  - **PDF/UA checker messages:** keep the structure tag names (Table, TR, THead, TBody, TFoot, Figure, L, LI) and PDF terms (PDF/A, XFA, OCR) as they are. Tag is *Teg*, Alternate text is *Muqobil matn*, Artifact is *Artefakt*, Reading order is *Oʻqish tartibi*.
  - **Signatures** (`sign_ui.rs`): use the LEX terms (*Raqamli imzo* / *elektron raqamli imzo* in legal text, *Imzo qoʻyuvchi*, *Sertifikat beruvchi*, *Amal qilish muddati*, *yopiq kalit*). Digital ID is *Raqamli ID*, Self-signed is *Oʻzi imzolagan*.
  - **Redaction:** *Oʻchirib yopish*.
  - **Calendar context rows:** follow CLDR; the month and weekday decisions are in the Phase 1 review §2.1.
  - **Print** (`print_ui.rs`): the AOSP print terms.
- **deckcraft.**
  - There is no Russian reference.
  - **Language autonyms** in `squiggles.rs` (Ελληνικά, Svenska, Português (Brasil)): keep them as written; they are names of languages in their own language.
  - **Slide show and animation:** Entrance *Kirish*, Exit *Chiqish*, Emphasis *Urgʻu*, Motion Paths *Harakat yoʻllari*, On Click *Bosilganda*, With/After Previous *Oldingisi bilan* / *Oldingisidan keyin*.
  - **Views:** Slide Master *Slaydlar namunasi*, Slide Sorter *Slaydlarni saralash*, Presenter View *Maʼruzachi rejimi*.
  - **Shapes:** Edit Points, Corner/Smooth Point (*Burchak nuqtasi* / *Silliq nuqta*), Merge Shapes.
  - **Theme colours** "Accent 1…6": *Urgʻu rangi 1…6*.
  - **Scheme and template names** (Nocturne, Meadow, Harbor, Slate): decide per row whether they are names (keep) or descriptions (translate).

### 5.3 After step 2

1. Run `cargo xt i18n glossary-scan`. It refreshes `i18n/glossary-use.tsv`, which does not know the 627 new terms yet.
2. Run `cargo xt i18n lint`, `cargo xt i18n check` and `cargo test -p xtask`.
3. Record the new statuses in `docs/STATUS.md`.
