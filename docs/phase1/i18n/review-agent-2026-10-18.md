# Agent review of the Uzbek UI strings (2026-10-18)

| | |
|---|---|
| **Mandate** | Owner decision 2026-10-18 (CLAUDE.md, gate decisions): the review of the uz-Latn and uz-Cyrl UI strings is delegated to the agent; "check them yourself; I will report problems found in use". Status `reviewed:agent-opus:2026-10-18`, never an owner review. |
| **Scope** | The glossary (batch 0: 150 terms, plus 6 terms added for the open questions) and every P1 row of `i18n/{shared,wordcraft,gridcraft,pdfcraft,deckcraft}.tsv`: 1,722 rows with a draft (the 2 `no-uz:` ArtCraft-sentence rows have nothing to translate). |
| **Reviewer** | opus agent. Native-quality standard Uzbek as used in government and business software; official Latin orthography (U+02BB in oʻ gʻ, U+02BC for the tutuq belgisi); Uzbek Cyrillic with ў қ ғ ҳ and ъ. Clean room: no proprietary localisation was consulted; general Uzbek vocabulary, upstream's own ru catalog (for context) and CLDR data (date abbreviations) were. |
| **Trust** | `i18n/config.toml` `[review] trust_agent_review = true`: agent-reviewed rows are trusted (release exports, no draft marker once every P1 row of an app is trusted). Lint, coverage and export still count owner and agent reviews apart. |

## 1. Method

1. **Glossary first.** Every term decided (section 2). The use index was refreshed (`cargo xt i18n glossary-scan`), the glossary changed, and the changes re-applied with `cargo xt i18n glossary-apply --write --date 2026-10-18` (151 string values re-applied, 0 to repair by hand). glossary-apply replaces a term mechanically; it cut "eksport qilish" to the noun in 7 strings with an object and gave `парольини` for a suffixed form. These were repaired by hand in the row review.
2. **Every P1 row read** in its UI context (the `where` file: ribbon, menu, backstage, dialog, pane, message) with the English and upstream's Russian: grammar and case suffixes, word order, imperative vs noun (a toggle such as Underline takes the adjective, a command takes the verb, a field hint takes the polite imperative "-ing"), glossary consistency, placeholders, `\n`, `…`, accelerators.
3. **Cyrillic re-derived** for every row from the final Latin by the official letter mapping (`i18n/tools/p1_drafts.py`) plus word rules, then every difference read by hand:
   Russian loans keep the soft sign when the word stands alone (Пароль, Фильтр, Модель, Консоль, Альбом, Компьютер, the month names январь … декабрь); a suffixed form drops it (паролини, моделингиздан), as in common use;
   Latin tokens with an Uzbek suffix stay Latin (PDFни, Wordга, GitHubдаги);
   key names stay Latin (Ctrl+O, ⌘U).
4. **Length.** Labels flagged by lint or by the clipping evidence (`docs/phase1/evidence/i18n/README.md`) were shortened where a shorter **standard** form exists. No label was truncated.
5. **Rows left as draft** where no established Uzbek term exists (section 4). Their status stays `draft:*` with a `review-agent-2026-10-18: left as draft:` note.

Every changed row carries a `review-agent-2026-10-18: <reason>` note in the catalog.

## 2. Glossary decisions

### 2.1 The open questions

| Question | Decision | Why |
|---|---|---|
| Options / Settings / Preferences | **Options = Parametrlar**; **Settings = Sozlamalar**; **Preferences = Sozlamalar** | "Sozlamalar" is the word every Uzbek OS and app uses for settings. "Afzalliklar" (the Preferences draft) means *advantages* and would mislead. Options keeps "Parametrlar" (a dialog of choices, ru Параметры, familiar to staff), so an app that shows both Options and Settings still has two words. |
| Insert: Kiritish or Qoʻshish | **Kiritish** | Three verbs stay apart: Insert = Kiritish, Add = Qoʻshish, Paste = Qoʻyish. "Kiritish" is the usual verb for putting content into a document ("formula kiritish", "maʼlumot kiritish"). |
| Home: Bosh sahifa or Asosiy | **Asosiy** | The ribbon tab is the main tab, not a web home page; "Asosiy" is shorter (6 against 11 characters). The backstage "Home" uses the same word. The one place where Home really is a start page ("Pin folder to Home…", pdfcraft) says "bosh sahifa". |
| footnote / endnote | **Sahifa osti izohi** / **Yakuniy izoh** | The established descriptive terms. "Quyi izoh" (draft) is not established, and "Izoh" alone is Comment. |
| redact | **Oʻchirib yopish** | Redaction deletes the content for good and covers it. "Maxfiylashtirish" (draft) reads as *classify / encrypt*; Hide (Yashirish) is reversible and must stay distinct. |
| flatten | **Sahifaga singdirish** | Comments and form fields become part of the page. "Yassilash" (draft) is a literal calque nobody would understand. |
| timestamp | **Vaqt tamgʻasi** | The term of Uzbek e-signature services. Draft kept. |
| month and weekday names | **lower case** (yanvar, dushanba), Cyrillic with the soft sign (январь, сентябрь) | Uzbek orthography writes them in lower case; a capital appears only at the start of a sentence. Short forms follow CLDR uz: yan fev mar apr may iyun iyul avg sen okt noy dek, yak dush sesh chor pay jum shan (Cyrillic янв фев мар апр май июн июл авг сен окт ноя дек, як душ сеш чор пай жум шан). The draft weekday abbreviations were mixed (yak, du, sha). |

Two lint `script-mismatch` warnings remain on purpose: official Latin "sentabr", "oktabr" against Cyrillic "сентябрь", "октябрь" (the Latin rules drop the я and ь), and two on the CLDR short forms "yan"/"янв", "noy"/"ноя".

### 2.2 All terms

20 terms changed, 130 kept, 6 added. "Rejected alternatives" lists the previous draft (for a changed term) and the alternatives that were offered.

| Term | uz-Latn | uz-Cyrl | Rejected alternatives | Decision |
|---|---|---|---|---|
| About | Dastur haqida | Дастур ҳақида | Haqida | kept |
| Accessibility | Maxsus imkoniyatlar | Махсус имкониятлар | Foydalanish imkoniyati | kept: the term Uzbek OS localisations use |
| Account | Hisob | Ҳисоб | Akkaunt | kept: used by Uzbek localisations of web accounts |
| Add | Qoʻshish | Қўшиш | – | kept |
| Align | Tekislash | Текислаш | – | kept |
| All | Barchasi | Барчаси | Hammasi | kept |
| Annotation | Izoh | Изоҳ | Annotatsiya, Izohlash | **changed**: the PDF app presents annotations as comments (Comments panel, Flatten comments); 'annotatsiya' means a summary in Uzbek |
| Apply | Qoʻllash | Қўллаш | Tatbiq etish | kept |
| Author | Muallif | Муаллиф | – | kept |
| Auto | Avto | Авто | Avtomatik | kept |
| Back | Orqaga | Орқага | – | kept |
| Bold | Qalin | Қалин | Yarim qalin | kept |
| Border | Chegara | Чегара | Ramka | kept |
| Bottom | Quyi | Қуйи | Pastki | kept |
| Browse | Tanlash | Танлаш | Koʻrib chiqish | **changed**: file-chooser button; 'Koʻrib chiqish' means to examine/review |
| Cancel | Bekor qilish | Бекор қилиш | – | kept |
| Cell | Katak | Катак | Yacheyka | kept: native term of Uzbek spreadsheet textbooks; 'yacheyka' is the spoken Russian loan |
| Center | Markaz | Марказ | Markazga tekislash | kept: noun in alignment lists (Chap / Markaz / Oʻng); commands may say 'Markazga tekislash' |
| Certificate | Sertifikat | Сертификат | Guvohnoma | kept |
| Change | Oʻzgartirish | Ўзгартириш | – | kept |
| Character | Belgi | Белги | Harf | kept |
| Chart | Diagramma | Диаграмма | Grafik | kept |
| Clear | Tozalash | Тозалаш | – | kept |
| Close | Yopish | Ёпиш | – | kept |
| Color | Rang | Ранг | – | kept |
| Column | Ustun | Устун | – | kept |
| Comment | Izoh | Изоҳ | Sharh | kept: established ('izoh qoldirish'); footnote is now 'sahifa osti izohi' |
| Copy | Nusxalash | Нусхалаш | Nusxa olish | kept |
| Crop | Qirqish | Қирқиш | Kesish | kept |
| Custom | Maxsus | Махсус | – | kept |
| Cut | Kesib olish | Кесиб олиш | Qirqib olish | kept: established clipboard term; 'Kesish' alone also means cut a shape/crop |
| Dark | Qorongʻi | Қоронғи | Tungi | kept: standard pair with Yorugʻ |
| Date | Sana | Сана | – | kept |
| Default | Standart | Стандарт | Odatiy | kept: 'standart holatda' is used where the English says 'by default' |
| Delete | Oʻchirish | Ўчириш | Yoʻq qilish | kept |
| Design | Dizayn | Дизайн | Loyihalash | kept |
| Document | Hujjat | Ҳужжат | – | kept |
| Draw | Chizish | Чизиш | Chizma | kept |
| Edit | Tahrirlash | Таҳрирлаш | Tahrir | kept: the verbal noun matches the other menu names (Kiritish) and the command form |
| Equation | Tenglama | Тенглама | Formula | kept |
| Error | Xato | Хато | – | kept |
| Export | Eksport | Экспорт | Eksport qilish | **changed**: standalone label as a noun (ru Экспорт); 'eksport qilish' stays where there is an object |
| Field | Maydon | Майдон | – | kept |
| File | Fayl | Файл | – | kept |
| Fill | Toʻldirish | Тўлдириш | Boʻyash | kept |
| Find | Topish | Топиш | Qidirish | kept: Search takes Qidirish, so Find and Search stay apart |
| Folder | Jild | Жилд | Papka | kept: used by Uzbek OS and app localisations; 'papka' is the spoken loan |
| Font | Shrift | Шрифт | – | kept |
| Footer | Quyi kolontitul | Қуйи колонтитул | Pastki kolontitul | kept: established, pairs with Header |
| Form | Forma | Форма | Anketa | kept |
| Format | Format | Формат | Formatlash | kept |
| Formula | Formula | Формула | – | kept |
| Function | Funksiya | Функция | – | kept |
| Header | Yuqori kolontitul | Юқори колонтитул | – | kept: established (ru колонтитул) |
| Heading | Sarlavha | Сарлавҳа | Boʻlim sarlavhasi | kept |
| Height | Balandligi | Баландлиги | Boʻyi | kept |
| Help | Yordam | Ёрдам | Qoʻllanma | kept |
| Hide | Yashirish | Яшириш | – | kept |
| Home | Asosiy | Асосий | Bosh sahifa | **changed**: ribbon tab and start page: 'Bosh sahifa' is a web home page; Asosiy is shorter and is what office staff call the main tab |
| Import | Import | Импорт | Import qilish | **changed**: standalone label as a noun (ru Импорт); 'import qilish' stays where there is an object |
| Indent | Chekinish | Чекиниш | – | kept |
| Insert | Kiritish | Киритиш | Qoʻshish | kept: keeps Insert (Kiritish), Add (Qoʻshish) and Paste (Qoʻyish) apart; 'kiritish' is the usual verb for putting content into a document (formula, data) |
| Italic | Kursiv | Курсив | Qiya | kept |
| Justify | Kenglik boʻyicha | Кенглик бўйича | Kenglik boʻyicha tekislash, Ikki tomonga tekislash | **changed**: alignment option (ru По ширине); 'tekislash' is implied in the Align group |
| Landscape | Albom | Альбом | Albom yoʻnalishi, Yotiq | **changed**: short form (ru Альбомная); Cyrillic keeps the soft sign |
| Language | Til | Тил | – | kept |
| Layout | Maket | Макет | Sahifa tuzilishi | kept: ribbon tab and slide layout (ru Макет) |
| Left | Chap | Чап | – | kept |
| Light | Yorugʻ | Ёруғ | Kunduzgi | kept |
| Line (shape) | Chiziq | Чизиқ | – | kept: text line = Satr; drawn line = Chiziq; row = Qator |
| Line (text) | Satr | Сатр | Qator | kept: text line = Satr; drawn line = Chiziq; row = Qator |
| Link | Havola | Ҳавола | – | kept |
| Location | Joylashuv | Жойлашув | Manzil | kept |
| Margins | Hoshiyalar | Ҳошиялар | Sahifa chetlari | **changed**: hoshiya is the term of the Uzbek document-format rules (chap hoshiya 30 mm); shorter than 'Sahifa chetlari' |
| Merge | Birlashtirish | Бирлаштириш | – | kept |
| More | Yana | Яна | Koʻproq | kept |
| Move | Koʻchirish | Кўчириш | Surish | kept |
| Name | Nom | Ном | – | kept |
| New | Yangi | Янги | – | kept |
| Next | Keyingi | Кейинги | – | kept |
| No | Yoʻq | Йўқ | – | kept |
| None | Yoʻq | Йўқ | Hech biri | kept |
| Number | Son | Сон | Raqam | kept: a quantity or numeric format; an identifier (page number) takes 'raqam' |
| OK | OK | OK | – | kept |
| Open | Ochish | Очиш | – | kept |
| Options | Parametrlar | Параметрлар | Sozlamalar | kept: dialog of choices (ru Параметры, familiar to staff); Settings and Preferences take Sozlamalar, so a menu that has both stays distinct |
| Orientation | Yoʻnalish | Йўналиш | – | kept |
| Page | Sahifa | Саҳифа | Bet | kept |
| Paragraph | Xatboshi | Хатбоши | Abzats | kept: standard Uzbek term (ru Абзац) |
| Password | Parol | Пароль | – | **changed**: Cyrillic keeps the soft sign of the Russian loan (1956 rules): Пароль |
| Paste | Qoʻyish | Қўйиш | Joylashtirish | kept |
| Picture | Rasm | Расм | Surat | kept |
| Portrait | Kitob | Китоб | Kitob yoʻnalishi, Tik | **changed**: short form in orientation lists (ru Книжная); 'Kitob yoʻnalishi' where the English says Orientation |
| Preferences | Sozlamalar | Созламалар | Afzalliklar | **changed**: 'Afzalliklar' means advantages; Sozlamalar is the word every Uzbek OS and app uses |
| Presentation | Taqdimot | Тақдимот | Prezentatsiya | kept |
| Previous | Oldingi | Олдинги | – | kept |
| Print | Chop etish | Чоп этиш | Bosib chiqarish | kept |
| Print Preview | Oldindan koʻrish | Олдиндан кўриш | Chop etishdan oldin koʻrish | **changed**: standard short term for preview |
| Properties | Xususiyatlar | Хусусиятлар | Xossalar | **changed**: the common word for properties; 'xossa' is a technical/maths word |
| Range | Diapazon | Диапазон | Oraliq | kept |
| Recent | Soʻnggi | Сўнгги | Yaqinda ishlatilgan | kept |
| Redo | Qayta bajarish | Қайта бажариш | Takrorlash | kept: 'Takrorlash' (shorter) is taken by Repeat, which wordcraft also shows |
| Remove | Olib tashlash | Олиб ташлаш | Oʻchirish | kept |
| Replace | Almashtirish | Алмаштириш | Oʻrniga qoʻyish | kept |
| Review | Tekshirish | Текшириш | Retsenziya | kept: ribbon tab; 'retsenziya' is not understood by most staff |
| Right | Oʻng | Ўнг | – | kept |
| Rotate | Burish | Буриш | Aylantirish | kept: shorter than Aylantirish, which is Convert |
| Row | Qator | Қатор | Satr | kept |
| Save | Saqlash | Сақлаш | – | kept |
| Save As | Boshqa nomda saqlash | Бошқа номда сақлаш | Boshqa nom bilan saqlash, Nomlab saqlash | **changed**: same meaning, 4 characters shorter (clipped in the backstage side menu) |
| Search | Qidirish | Қидириш | Izlash | kept |
| Section | Boʻlim | Бўлим | – | kept |
| Security | Xavfsizlik | Хавфсизлик | – | kept |
| Select | Tanlash | Танлаш | Belgilash | kept: general choose/select; 'belgilash' is mark |
| Select All | Hammasini tanlash | Ҳаммасини танлаш | Barchasini tanlash | kept |
| Shape | Shakl | Шакл | Figura | kept |
| Share | Ulashish | Улашиш | Baham koʻrish | kept |
| Sheet | Varaq | Варақ | List | kept: official term; 'list' is the spoken Russian loan |
| Show | Koʻrsatish | Кўрсатиш | – | kept |
| Sign | Imzolash | Имзолаш | – | kept |
| Signature | Imzo | Имзо | – | kept |
| Size | Oʻlcham | Ўлчам | – | kept |
| Slide | Slayd | Слайд | Taqdimot varagʻi | kept |
| Slide Show | Namoyish | Намойиш | Slaydlar namoyishi | **changed**: ribbon tab; the slides are implied in a presentation app |
| Sort | Saralash | Саралаш | Tartiblash | kept |
| Spacing | Oraliq | Оралиқ | Interval | kept |
| Spelling | Imlo | Имло | – | kept |
| Strikethrough | Ustidan chizilgan | Устидан чизилган | Ustidan chizish, Ustiga chizilgan | **changed**: adjective form like Underline; commands keep 'ustidan chizish' |
| Style | Uslub | Услуб | Stil | kept |
| Subscript | Quyi indeks | Қуйи индекс | Pastki indeks | kept |
| Superscript | Yuqori indeks | Юқори индекс | – | kept |
| Tab (ribbon) | Yorliq | Ёрлиқ | Tab | kept |
| Table | Jadval | Жадвал | – | kept |
| Text | Matn | Матн | – | kept |
| Theme | Mavzu | Мавзу | Tema | **changed**: native term used by Uzbek phone and app localisations; ru also uses one word (Тема) for theme and subject |
| Title | Sarlavha | Сарлавҳа | Nomi | **changed**: Title is mostly a heading (chart, slide, axis title, title bar); 'Nomi' is the possessive of Name and collided with it |
| Tools | Vositalar | Воситалар | Asboblar | kept |
| Top | Yuqori | Юқори | – | kept |
| Type | Tur | Тур | – | kept |
| Underline | Tagi chizilgan | Таги чизилган | Tagiga chizish, Tagchiziq | **changed**: format toggle next to Qalin/Kursiv takes the adjective form; commands keep the verb 'tagiga chizish' |
| Undo | Qaytarish | Қайтариш | Bekor qilish | kept: Cancel already takes 'Bekor qilish' |
| Value | Qiymat | Қиймат | – | kept |
| Version | Versiya | Версия | – | kept |
| View | Koʻrinish | Кўриниш | Koʻrish | kept |
| Width | Kengligi | Кенглиги | Eni | **changed**: pairs with Height = Balandligi; 'Eni' is colloquial |
| Window | Oyna | Ойна | – | kept |
| Word | Soʻz | Сўз | – | kept |
| Workbook | Ish kitobi | Иш китоби | Kitob | kept |
| Yes | Ha | Ҳа | – | kept |
| Zoom | Masshtab | Масштаб | Kattalashtirish | kept |
| Endnote | Yakuniy izoh | Якуний изоҳ | – | added: note at the end of the document or section |
| Flatten | Sahifaga singdirish | Саҳифага сингдириш | – | added: comments or form fields become part of the page; 'yassilash' is a literal calque |
| Footnote | Sahifa osti izohi | Саҳифа ости изоҳи | – | added: established descriptive term; 'Izoh' alone is Comment |
| Redact | Oʻchirib yopish | Ўчириб ёпиш | – | added: redaction removes the content for good and covers it; Hide (Yashirish) is reversible; 'maxfiylashtirish' reads as classify/encrypt |
| Settings | Sozlamalar | Созламалар | – | added: Options = Parametrlar (dialog of choices, ru Параметры); Settings and Preferences = Sozlamalar |
| Timestamp | Vaqt tamgʻasi | Вақт тамғаси | – | added: term of Uzbek e-signature services |

## 3. Statistics (P1 rows with a draft)

| Scope | Rows | Accepted unchanged | Fixed (Latin) | Shortened | Cyrillic only | Left as draft |
|---|---|---|---|---|---|---|
| shared | 335 | 294 | 20 | 18 | 3 | 0 |
| wordcraft | 491 | 413 | 50 | 10 | 2 | 16 |
| gridcraft | 285 | 239 | 26 | 12 | 6 | 2 |
| pdfcraft | 498 | 397 | 79 | 4 | 17 | 1 |
| deckcraft | 113 | 79 | 18 | 14 | 2 | 0 |
| **all** | **1,722** | **1,422** | **193** | **58** | **30** | **19** |

- "Fixed" counts a changed uz-Latn text (its Cyrillic was re-derived and read too); 6 of these only took a changed glossary term. "Shortened" is a fix whose main purpose was length. "Cyrillic only": the Latin was right, the Cyrillic was not.
- Statuses now: 1,703 rows `reviewed:agent-opus:2026-10-18` in both scripts, 19 rows draft. Glossary: 156 terms `reviewed:agent-opus:2026-10-18`. No owner reviews yet (`reviewed:owner` 0).
- Outside P1, one P2 row was marked reviewed: deckcraft `badcd1b657ae` "DeckCraft" (the upstream product token, unchanged), so deckcraft's review build has no draft on screen.

**Lint** (`cargo xt i18n lint`): 0 errors; warnings 367 = 363 length + 4 script-mismatch (section 2.1). Length warnings went from 386 to 363. The remaining ones are mostly one-word labels where Uzbek has no shorter standard form ("View" = Koʻrinish, "Edit" = Tahrirlash, "Header" = Yuqori kolontitul): the lint rule is a ratio to the English (1.4×), so a 4-letter English verb against a 9-letter Uzbek noun is always flagged. The clipping that was actually seen was addressed: "Save As" is 4 characters shorter (it still exceeds the backstage side menu, about 15 characters; the menu needs to be wider), "Developer" = Dasturchi (9 instead of 16; it was folding into the tab overflow), "Slide Show" = Namoyish, "Export" = Eksport. A lint rule that also weighs absolute width would give a more useful count; that is a suggestion, the rule was not changed.

## 4. Rows left as draft (19)

> **Note (gate review 2026-10-19).** Superseded later on 2026-10-18: the orchestrator resolved these 19 rows (row note "orchestrator 2026-10-18: resolved left-as-draft row",
> commit `99d1ba8`), so every P1 row is now `reviewed:agent-opus:2026-10-18` and `cargo xt i18n lint` reports draft 0. The "(qoralama)" consequence below no longer holds.

| Scope | Id | English | Current uz-Latn | Why |
|---|---|---|---|---|
| wordcraft | ce04ccdf3180 | Content Control at Selection | Tanlangan joydagi kontent boshqaruvi | content control (Word's form-control concept): no established Uzbek term |
| wordcraft | aed4b0dd0593 | Remove Content Control | Kontent boshqaruvini olib tashlash | same |
| wordcraft | 47e21a06fa66 | Rich Text Content Control | Boy matn kontent boshqaruvi | same |
| wordcraft | a9bb94f721b9 | Plain Text Content Control | Oddiy matn kontent boshqaruvi | same |
| wordcraft | 9a840e241306 | Picture Content Control | Rasm kontent boshqaruvi | same |
| wordcraft | b5b5bc52aeb5 | Building Block Gallery Content Control | Qurilish bloklari galereyasi kontent boshqaruvi | same |
| wordcraft | 5cc741c783b5 | Check Box Content Control | Belgilash katagi kontent boshqaruvi | same |
| wordcraft | 1878e386bebf | Combo Box Content Control | Kombinatsiyalangan roʻyxat kontent boshqaruvi | same |
| wordcraft | 3a72deee8bc3 | Drop-Down List Content Control | Ochiladigan roʻyxat kontent boshqaruvi | same |
| wordcraft | b08a5cd9a892 | Date Picker Content Control | Sana tanlagich kontent boshqaruvi | same |
| wordcraft | 140deefcae4c | Repeating Section Content Control | Takrorlanuvchi boʻlim kontent boshqaruvi | same |
| wordcraft | 680bab71e1ac | Table of Authorities | Havolalar jadvali | US legal concept (list of cited cases and statutes); no Uzbek equivalent |
| wordcraft | be2641e5d4e4 | Insert Table of Authorities | Havolalar jadvalini kiritish | same |
| wordcraft | fc2b2f058467 | Immersive Reader | Immersiv oʻqish | product feature name; "immersiv" is not Uzbek |
| wordcraft | 291a629d7097 | Immersive | Immersiv | same |
| wordcraft | d0f95c3e15ce | Organizer | Tashkilotchi | the dialog copies styles between documents; "tashkilotchi" is a person (organiser) |
| gridcraft | 5e11878b5007 | Sparklines | Mini-diagrammalar (new descriptive draft) | no established term; "Sparklaynlar" was meaningless |
| gridcraft | cce28430c610 | Threaded Comments | Muloqotli izohlar (new draft) | no established term; the old draft "muzokarali" means *negotiated* |
| pdfcraft | dd1fa99dc6e0 | Add a callout | Koʻrsatkichli izoh qoʻshish (new draft) | callout (ru выноска): no established term |

Consequence: the "(qoralama)" marker stays in wordcraft, gridcraft and pdfcraft until these rows are decided (by the owner's feedback in use, or by an owner decision); deckcraft has no untrusted P1 row and drops the marker in both review and release exports. Three new drafts that are better than the old ones were written into the catalog with status `draft:agent-opus:2026-10-18`; the other 16 keep their text and status, with a note.

## 5. The 20 most significant corrections

| # | Where | English | Before | After | Why |
|---|---|---|---|---|---|
| 1 | shared, pdfcraft menus | Preferences… | Afzalliklar… | Sozlamalar… | "afzallik" means advantage |
| 2 | all apps, ribbon and backstage | Home | Bosh sahifa | Asosiy | main tab, not a web home page; shorter |
| 3 | gridcraft Data tab | Get Data (Text/CSV) | uz-Cyrl `(матн/CСВ)` | `(матн/CSV)` | the token was half transliterated (Latin C + Cyrillic СВ); lint could not see it |
| 4 | deckcraft backstage, gridcraft | Open … (Ctrl+O), Bold (⌘B), Italic (⌘I), Cut (⌘X) | uz-Cyrl `(Cтрл+О)`, `(⌘Б)`, `(⌘И)`, `(⌘Х)` | `(Ctrl+O)`, `(⌘B)`, `(⌘I)`, `(⌘X)` | key names must stay Latin; the shortcut letters were transliterated |
| 5 | pdfcraft | Separate PDFs | uz-Cyrl `Алоҳида ПДФлар` | `Алоҳида PDFлар` | PDF stays Latin in Cyrillic text |
| 6 | wordcraft paragraph | Keep with Next / Keep Lines Together | Keyingisi bilan birga saqlash / Qatorlarni birga saqlash | Keyingisidan ajratmaslik / Satrlarni ajratmaslik | "saqlash" reads as Save (ru не отрывать от следующего) |
| 7 | wordcraft charts | Area (chart type) | Hudud boʻyicha ("by region") | Sohali | it is a chart type (ru С областями) |
| 8 | pdfcraft redaction (10 rows) | Redact, Apply redactions… | Maxfiylashtirish… | Oʻchirib yopish… | "maxfiylashtirish" reads as classify/encrypt; redaction deletes and covers |
| 9 | pdfcraft | Flatten comments / form fields | … yassilash | … sahifaga singdirish | literal calque replaced by what happens |
| 10 | pdfcraft OCR (4 rows) | Recognize text… | Matnni aniqlash… | Matnni tanib olish… | OCR is "tanib olish"; "aniqlash" is detect/determine |
| 11 | pdfcraft signatures | The signer's certificate has been revoked | … chaqirib olingan | … bekor qilingan | Uzbek e-signature usage; "chaqirib olish" is a calque of ru отзыв |
| 12 | wordcraft References (5 rows) | Footnotes, Insert Footnote, Next Footnote | Quyi izohlar … | Sahifa osti izohlari … | established term (section 2.1) |
| 13 | shared, wordcraft, pdfcraft (16 rows) | Line Start, Line Spacing, Line Numbers, Select Line | Qator … | Satr … | glossary: a text line is "satr"; "qator" is a table row |
| 14 | wordcraft equations, fields | Move in Equation; Toggle Field Codes; Toggle Check Box | Tenglamada koʻchirish; … almashtirish | Tenglama ichida harakatlanish; … koʻrsatish/yashirish; … yoqish/oʻchirish | the cursor moves, the content does not; "almashtirish" is replace, not toggle |
| 15 | pdfcraft | Pin folder to Home… | Papkani bosh sahifaga mahkamlash… | Jildni bosh sahifaga qadash… | glossary Folder = Jild; "qadash" is the usual verb for pin |
| 16 | wordcraft Draw tab | Add Pen / Remove Pen | Qalam qoʻshish / Qalamni olib tashlash | Ruchka qoʻshish / Ruchkani olib tashlash | Pen = Ruchka; Qalam is Pencil, which is a different tool on the same tab |
| 17 | shared Home tab | Format Painter | Format namunasi ("format sample") | Formatni nusxalash | names what the tool does; same as Copy Formatting |
| 18 | gridcraft | Insert Paste / AutoSum / Name Manager | Siljitib kiritish / Avtojamlama / Nomlar dispetcheri | Siljitib qoʻyish / Avtoyigʻindi / Nomlarni boshqarish | it is a paste; a sum is "yigʻindi"; "dispetcher" is a Russian loan |
| 19 | pdfcraft, wordcraft, deckcraft (13 rows) | Export to Word…, Print to PDF, PdfCraft on GitHub | Word ga eksport qilish…, PDF ga chop etish, GitHub dagi PdfCraft | Wordga …, PDFga …, GitHubdagi … | orthography: a suffix is written joined to an abbreviation or name |
| 20 | uz-Cyrl, 22 rows | Password, Filter, Model, Console, computer, the months | Парол, Филтр, Модел, Консол, компютер, сентабр, октабр, январ | Пароль, Фильтр, Модель, Консоль, компьютер, сентябрь, октябрь, январь | Uzbek Cyrillic keeps the Russian spelling of these loans (1956 rules); the letter mapping had dropped ь and я |

Other recurring fixes: "Toq va juft sahifalar" (the English order, odd before even), "Sahifalar nusxasini yaratish" for Duplicate pages (not "repeat"), "Boshqa joyga nusxalash" for Copy to, placeholder hints in the polite imperative ("Sarlavha qoʻshing"), "nogironligi boʻlgan shaxslar" (the official term), "veb-sahifa" hyphenated throughout, and no suffix after a quoted placeholder ("“{}” soʻroviga mos …").

## 6. Tooling (`xtask/src/i18n/*`, `xtask/src/i18n.rs`, `i18n/config.toml`)

- `i18n/config.toml` `[review] trust_agent_review = true`, with the owner decision quoted. Missing or `false`: owner-only trust, as before.
- `catalog.rs`: `Policy` (parsed from config.toml), `reviewer()` (`Owner` for `reviewed:owner…`, `Agent` for `reviewed:agent-<model>…`; a malformed reviewer is neither), `is_trusted(status, &policy)`, `exportable(status, mode, &policy)`, `ReviewCounts` (owner / agent / draft / missing per locale).
- `export.rs`: the policy reaches every chain step. `Built` counts trusted and draft own rows, owner and agent reviews apart, and `p1_untrusted` (P1 rows of the app, `no-uz:` rows excepted, whose own value is missing or untrusted). The picker marker "(qoralama)" is written while any P1 row is untrusted or a draft is exported. `catalog-meta.env` gains `UZ_*_REVIEWED_OWNER`, `UZ_*_REVIEWED_AGENT` and `UZ_*_P1_UNTRUSTED` (the existing keys are unchanged); the export report prints the split per app. `roundtrip` now runs on a copy with all Uzbek removed (`without_uzbek`), so its "no Uzbek" checks no longer depend on what has been reviewed.
- `lint.rs`: `review_summary` (P1 rows per scope and script: owner-reviewed, agent-reviewed, draft, trusted) is printed by `lint` and `coverage` and written into the lint report.
- Tests (all in the i18n module): trust under both policies and malformed reviewers; config parsing; review counts; release export of agent rows only under the policy; the marker per mode (one P1 draft keeps it, all trusted removes it, an exported draft that is P1 only in another app keeps it in a review build); the meta keys and the export report; `without_uzbek`; the lint review summary. `cargo test -p xtask`: 55 passed.
- `glossary-apply` already treats every `reviewed:*` row as reviewed: a later glossary change flags agent-reviewed rows instead of editing them.

## 7. Checks

| Command | Result |
|---|---|
| `cargo xt i18n lint --report docs/phase1/i18n/lint-report.md` | 0 errors, 363 length + 4 script-mismatch warnings (386 length before) |
| `cargo xt i18n roundtrip` | OK (wordcraft 2,571, gridcraft 398, pdfcraft 2,316 ru entries equal upstream; uz-Latn empty and uz-Cyrl all from ru with no Uzbek) |
| `cargo xt i18n check` | 7,188 rows, all ids match |
| `cargo xt i18n export all --mode review|release` (scratch output) | runs; marker gone for deckcraft, kept for wordcraft (16 untrusted P1), gridcraft (2), pdfcraft (1) |
| `cargo test -p xtask` | 55 passed |

## 8. Not verified, risks

- The review is an agent's judgement, as delegated. Terms that may draw owner feedback: Asosiy (Home), Kiritish (Insert), Kitob / Albom (Portrait / Landscape), Oʻchirib yopish (Redact), Sahifaga singdirish (Flatten), Hoshiyalar (Margins).
- Uzbek Cyrillic practice on the soft sign in Russian loans varies (Пароль and Парол are both seen). This review follows the 1956 rules for a word without a suffix and drops ь before a suffix (паролини). The Phase 2 transliteration golden list, which still needs the owner's approval, should settle it.
- Not checked on screen after the change: the shortened labels were not re-screenshotted. The backstage side menu is still narrower than "Boshqa nomda saqlash".
- The glossary has 156 rows now (6 added). `glossary-batch0.csv/.xlsx` were not regenerated; `cargo xt i18n review-export --batch 0` produces the current batch if the owner wants to look.
