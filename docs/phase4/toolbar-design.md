# Phase 4 WP1: the simplified toolbar (Docs, Sheets, Slides, PDF)

| | |
|---|---|
| **Status** | Design only, written 2026-10-22 on branch `phase-4`. No product code, no builds. Waiting for the owner's review (PLAN Q5). |
| **Implements** | PLAN §2.1–§2.4 (the design WP2 and WP3 build), WP1. The labels feed WP17; the settings key feeds WP5; the keyboard and accessibility rules feed WP18. |
| **Upstream BASEs read** | wordcraft `eae2421`, gridcraft `f1894c6`, deckcraft `1dc6972`, pdfcraft `924c4da` (the trees under `upstream/` are the **queue applied**). Every `file:line` below is relative to `upstream/` unless it says `domains/`. **WP0 moves every BASE: WP0b re-checks every id and line in §9 and Appendix A.** |
| **Mock-ups** | ASCII below, and the same four layouts drawn in [`toolbar-design.html`](toolbar-design.html) (a static page, no external resources; open it in any browser). |
| **Owner time** | 1–2 hours: the Uzbek summary, the four mock-ups, and the three questions in §11. |

---

## Egasi uchun qisqacha (oʻzbekcha)

**Bu nima.** Toʻrtta dastur uchun soddalashtirilgan asboblar paneli loyihasi (WP1). Kod yozilmadi: bu faqat hujjat va
maketlar. Maketlarni `docs/phase4/toolbar-design.html` faylini brauzerda ochib koʻring.

**Umumiy qoida.**
- Har bir dastur sukut boʻyicha **sodda panel** bilan ochiladi. Unda faqat har kuni kerak boʻladigan buyruqlar bor: xat,
  buyruq, tabel, xodimlar roʻyxati va oddiy taqdimot uchun.
- Tugmalardagi yozuvlar tekshirilgan oʻzbekcha katalogdan olinadi: "Qalin", "Chop etish", "Matnni oʻrash",
  "Koʻrsatish/yashirish ¶". Hali katalogda yoʻq 7 ta yozuv (masalan, "Toʻliq lenta") 8-boʻlimda alohida belgilangan.
- **Toʻliq lentaga oʻtish faqat kompyuter versiyasida.**
  - Hujjatlar: chap yuqoridagi tezkor panelda "Toʻliq lenta" tugmasi.
  - Jadvallar va Taqdimotlar: sodda panelning oʻng chetida "Toʻliq lenta" tugmasi.
  - Orqaga qaytish: oʻsha joyda "Sodda lenta" tugmasi paydo boʻladi.
  - Tanlov umumiy sozlamalar faylida (`settings.toml`, `toolbar`) saqlanadi. Boshqa dasturlar uni keyingi ochilishda
    oladi. Bosh ekranning "Sozlamalar" sahifasida ham shu tanlov bor.
- Veb-versiyada faqat sodda panel boʻladi, almashtirish tugmasi yoʻq.

**Hujjatlar (Word oʻrniga).** Uchta yorliq qoladi:
- **Asosiy**: Worddagi "Главная" bilan bir xil (shrift, xatboshi, uslublar, topish va almashtirish);
- **Kiritish**: jadval, rasm, kolontitullar, sahifa raqami;
- bizning **Hujjat** yorligʻimiz: Chop etish, PDF, Yoʻnalish, Sahifa parametrlari, Imlo, Transliteratsiya (lotin ↔ kirill)
  va Versiyalar tarixi.

Qolgan 10 ta yorliq (Dizayn, Maket, Havolalar, Pochta, Taqriz, Koʻrinish va boshqalar) toʻliq lentada turadi.
Jadval yoki rasm tanlanganda ularning maxsus yorliqlari avvalgidek chiqadi.

**¶ belgisi (Koʻrsatish/yashirish ¶) ikki joyda turadi.**
- Asosiy yorligʻida, yoqilgan yoki oʻchirilganligi koʻrinib turadi.
- Tezkor panelda: qaysi yorliq ochiq boʻlishidan qatʼi nazar, doim koʻrinadi.
- Klaviatura: Ctrl+Shift+8, yoki Alt, keyin 4.

**Jadvallar (Excel oʻrniga).** Bitta qator:
- shrift va oʻlcham, Qalin / Kursiv / Tagi chizilgan, shrift rangi, toʻldirish rangi, chegaralar;
- tekislash, matnni oʻrash, birlashtirish va markazlash;
- son formati (Umumiy, Son, Qisqa sana, Foiz, Matn);
- qator yoki ustun kiritish va oʻchirish;
- Avtoyigʻindi, saralash, filtr;
- sohalarni qotirish, formulalarni koʻrsatish;
- chop etish, transliteratsiya, versiyalar tarixi.

Saqlash, Qaytarish va Qayta bajarish avvalgidek sarlavha qatorida turadi. Excelning Alt ketma-ketliklari ishlayveradi
(masalan, Alt, H, 1 — qalin).

**Taqdimotlar (PowerPoint oʻrniga).** Bitta qator:
- yangi slayd, maket;
- shrift, oʻlcham, Q / K / T, rang;
- markerlar, raqamlash, tekislash;
- matn qutisi, rasm, shakllar, jadval;
- transliteratsiya, versiyalar tarixi.

Saqlash, Qaytarish va "Namoyish" tugmasi sarlavha qatorida turadi.

**PDF.** Oʻzgarishsiz qoladi.
- Yuqorida rejimlar: Barcha vositalar, Oʻqish, Tahrirlash, Aylantirish, Elektron imzo.
- Yonida qidiruv, Chop etish va Saqlash.
- PDFda lenta yoʻq, shuning uchun almashtirish ham yoʻq.

**Sizdan kerakli qarorlar (Q5).** Javobingiz 11-boʻlimga yoziladi.
1. Hujjatlar uchun qaysi biri: 3 yorliqli sodda lenta (tavsiya), yoki Jadvallardagi kabi bitta qator (+2 kun)?
2. Panel tanlovi barcha dasturlar uchun bitta boʻlsinmi (tavsiya), yoki har biriga alohida?
3. Taqdimotlarning sodda qatorida "Chop etish" yoʻq (Ctrl+P va Fayl menyusida bor). Shunga rozimisiz?

---

## 1. What this design decides

| # | Decision | Why |
|---|---|---|
| 1 | **Docs keeps a ribbon, cut to three tabs**: the built-in Home and Insert tabs plus our tab "Hujjat". It is applied as a **preset** through upstream's Customize Ribbon (`tools.customizeRibbon`, `wordcraft/crates/engine/src/cmd/tools.rs:377`). | PLAN F1/§2.2: no hot-file patch. Word users already know Home and Insert, including where ¶ is. Risk R3 ("still a ribbon") is answered by owner question 1 and by WP23. |
| 2 | **Sheets and Slides get one row of our own** (`office_toolbar.rs` + `office_toolbar.toml`), drawn instead of `ribbon::show` (gridcraft `ui-egui/src/lib.rs:1149`, deckcraft `ui-egui/src/lib.rs:1836`). | PLAN §2.3. Their ribbons have no customization hook (F3), so a preset is not possible. |
| 3 | **PDF is unchanged.** | PLAN F5/§2.4: there is no ribbon. The mode bar is already task-shaped (`pdfcraft/crates/ui-egui/src/chrome.rs:496-536`). |
| 4 | **What the title bar already shows is not repeated.** Sheets' title bar has Save/Undo/Redo (`gridcraft/crates/ui-egui/src/ribbon.rs:71-97`). Slides' QAT has Save/Undo/Redo/Start show (`deckcraft/crates/ui-egui/src/qat.rs:59-64`). | The row stays short enough for a 1366 px laptop screen (§4.4). |
| 5 | **One shared setting** `toolbar = "simple" \| "full"` in `settings.toml` (WP5) drives Docs, Sheets and Slides. Default `simple`. | One choice for the user to learn; owner question 2. |
| 6 | **Labels come from the reviewed catalogs** wherever a row exists. Every new string is listed in §8 for WP17. | CLAUDE.md: Uzbek strings are never machine-trusted; the review is delegated to the agent (`reviewed:agent-*`). |
| 7 | **Every id is checked against the registry at the current BASE** (§9), and the WP2/WP3 test `office_toolbar_ids_exist` keeps checking at every sync. | PLAN §2.1(2): ids are tested, not merged. |

## 2. Shared rules (all apps)

### 2.1 The simple/full toggle and where the choice lives
- **Source of truth:** `%APPDATA%\<brand data root>\settings.toml`, key `toolbar = "simple" | "full"` (PLAN §5.1, schema in
  `crates/suite-core`, WP5). It is missing or unreadable → `simple`. One value for Docs, Sheets and Slides; PDF ignores it.
- **In an app:** the toggle switches at once and writes the key through `suite-core` (an atomic write, as for the other
  shared values). Apps that are already running keep their mode until their next start. There is no live sync in Phase 4.
- **In the shell:** Settings › "Asboblar paneli" offers two choices: "Sodda lenta" and "Toʻliq lenta" (desktop only).
- **Web:** no toggle and no key. The simple toolbar is always shown (`cfg(target_arch = "wasm32")`, BRIEF §4.3).
- **The toggle's labels describe the action:**
  - in simple mode, the button reads **"Toʻliq lenta"** (Full ribbon);
  - in full mode, it reads **"Sodda lenta"** (Simple ribbon).
  - Both strings are new (§8).
- **Docs mechanics** (in `docs-office`, WP2):
  - Two commands, `office.ribbonFull` and `office.ribbonSimple`. Each swaps itself for the other on the QAT
    (`qatRemove`/`qatAdd`), so the QAT tooltip names what the button does.
  - Full: `tools.customizeRibbon {"reset": true}` (`ribbon.rs:380`, test `ribbon.rs:466`).
  - Simple: the preset, as one batch of `hide` / `newTab` / `newGroup` / `add` operations (the format of the test at
    `ribbon.rs:447-453`).
  - At start-up:
    - `toolbar = simple` and a default layout (`ribbon_is_default`, `ribbon.rs:183`) → apply the preset;
    - `toolbar = full` and a layout equal to the preset → reset;
    - **any other layout is the user's own and is left alone.**
- **Sheets and Slides mechanics** (WP3):
  - `office_overlay::toolbar_simple()` reads a process-wide flag. It is loaded from `settings.toml` at start and flipped by
    the toggle.
  - The call site branches on it. **In full mode** our file also draws a small "Sodda lenta" pill at the right end of the
    tab row. It is an `egui::Area` placed from the top of the ribbon panel, so `ribbon.rs` itself is untouched.
  - WP3's snapshot test checks at 1024 px that the pill does not cover a contextual tab. Fallback: one hunk in the title
    bar.
- **Said plainly:**
  - In Docs, toggling to full and back **discards a user's own ribbon customization**, because `reset_ribbon` clears it.
    The QAT survives (`ribbon.rs:379-382`).
  - Stashing the user's layout is possible (one more key in `settings.toml`), but it is not proposed for Phase 4.

### 2.2 Keyboard
- **Docs:**
  - Word keytips are upstream's (`wordcraft/crates/ui-egui/src/keytips.rs`). Alt shows F (File), H (Home) and N (Insert)
    (`:70-82`), and numbers on the QAT (`qat_label`, `:594-601`): Save 1, Undo 2, Redo 3, **¶ 4**, toggle 5. The Home
    and Insert controls keep Word's sequences (`:150-210`).
  - **Our "Hujjat" tab has no Alt letter**: `TAB_LETTERS` lists built-in tabs only. Its commands are reached by their
    own shortcuts (Ctrl+P, F7), or by the command search: Alt+Q, `tools.search`, `cmd/tools.rs:383-384`. Each command's
    label is registered, so search finds it by its Uzbek name.
- **Sheets:**
  - Excel's sequences stay valid in simple mode: the state machine does not need the ribbon to be drawn
    (`gridcraft/crates/ui-egui/src/keytips.rs`; the table is `keytips/keytips.json`). Examples:
    - Alt,H,1 / 2 / 3: bold / italic / underline;
    - Alt,H,A,L / C / R: align;
    - Alt,H,M,C: merge;
    - Alt,H,N: number format;
    - Alt,H,I,R / Alt,H,I,C: insert rows / columns;
    - Alt,H,U,…: AutoSum;
    - Alt,A,T: filter;
    - Alt,W,F,F: freeze panes;
    - Alt,M,H: show formulas;
    - Alt,F,P: print.
  - Our row paints each badge with the full sequence. That reuses `keytip`/`control_keytip`
    (`gridcraft/crates/ui-egui/src/ribbon.rs:324-340`): a one-word `pub(crate)` hunk each.
  - **It must also consume `keytip_invoke`** (`invoke`, `:356-363`) for the five `control`-kind ids: `home.fontName`,
    `home.fontSize`, `home.fillColor`, `home.fontColor` and `home.numberFormat`. Otherwise Alt,H,F,F and the other four
    do nothing in simple mode. A kittest covers it.
- **Slides:** deckcraft has **no keytip system** (no keytip code in `deckcraft/crates/ui-egui/src`).
  - What exists: Alt+F (File), the shortcuts on each command (Ctrl+B/I/U, Ctrl+L/E/R, F5) and the palette
    (Ctrl+Shift+P, `deckcraft/crates/ui-egui/src/lib.rs:425`).
  - We add no keytips. This is a gap to record in WP18.
- **PDF:** no keytips. Ctrl+K (palette), Ctrl+P, Ctrl+S and Ctrl+D (`pdfcraft/crates/engine/src/commands.rs:186-198`).
- **Tooltips** show `<label> (<shortcut>)`, built from the registry shortcut, as the apps already do.

### 2.3 Accessibility (requirements for WP2/WP3; WP18 measures them)
- **Tab order = drawing order**, left to right, group by group, with the toggle last. No custom focus order.
- **Accessible name** = the translated label, never the icon name. Our buttons call `widget_info` the way upstream's do
  (`wordcraft/crates/ui-egui/src/ribbon.rs:296`, `gridcraft/crates/ui-egui/src/ribbon.rs:155`). Toggles report
  their state (`WidgetInfo::selected`): B/I/U, alignment, wrap, merge, filter, show formulas, bullets and ¶.
- **Targets:** at least 24×24 logical px on desktop. Compact mode (§7): at least 40×40.
- **Colours:** upstream's theme tokens only. Our toolbar adds no colour of its own.
- **Unverified, said plainly:** whether Tab moves focus from the canvas into the ribbon or into our row. In gridcraft
  the grid keeps keyboard focus by design (`ribbon.rs:323`). WP18 measures it with Narrator and keyboard only. The
  keyboard path that is known to work is keytips plus shortcuts plus command search (§2.2).

---

## 3. Docs (wordcraft): a three-tab preset

### 3.1 Mock-up (simple mode, Asosiy tab in front)
```
┌ Title bar ─────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ [Saqlash][Qaytarish▾][Qayta bajarish][¶][⇱ Toʻliq lenta][⋯]       Buyruq 12.docx — Suite Hujjatlar     [–][□][×]│
├ Tab strip ─────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Fayl  Asosiy  Kiritish  Hujjat                                                    [Izohlar] [✎ Tahrirlash ▾]  │
├ Asosiy (upstream, unchanged) ──────────────────────────────────────────────────────────────────────────────────┤
│ Qoʻyish▾ Kesib olish │ [Times New Roman▾][14▾] A+ A- Aa ⌫ │ •▾ 1.▾ ≣▾ ⇤ ⇥ ⇅ [¶] │ Uslublar   │ Topish▾      │ 🎙 │ ✓  │
│          Nusxalash   │ Q K T▾ S x₂ x² ▢ ⓐ 🖍▾ A▾          │ ⯇ ≡ ⯈ ☰ ⇄ ⇆ ↕▾ ▦▾ ▭▾ │ (galereya) │ Almashtirish │grey│    │
│          Formatni n. │                                     │                     │            │ Tanlash▾     │    │    │
│ Almashinuv buferi    │ Shrift                              │ Xatboshi            │ Uslublar   │ Tahrirlash   │Ovoz│Muh.│
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```
The same window with **Hujjat** in front (our tab, the preset):
```
├ Hujjat (ours) ─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ [🖶 Chop etish] [PDF sifatida  │ [Yoʻnalish] [Sahifa      │ [Imlo va     │ [Lotinchadan] [Kirillchadan] [Avtomatik] │ [Versiyalar] │
│                  eksport qilish]│             parametrlari]│  grammatika] │ [kirillchaga] [lotinchaga]   [aniqlash]  │ [tarixi]     │
│ Fayl                            │ Sahifa                   │ Imlo         │ Transliteratsiya                         │ Tarix        │
```
The same window in full mode: all 13 tabs, as upstream. The QAT button now reads "Sodda lenta".

### 3.2 What is on it
| Area | Content | Count |
|---|---|---|
| QAT (title bar, every tab) | Save, Undo, Redo, **Show/Hide ¶**, the simple/full toggle | 5 buttons |
| Tab strip (upstream, right side) | Comments; Editing / Reviewing (the track-changes switch, `wordcraft/crates/ui-egui/src/ribbon.rs:253-269`) | 2 |
| **Asosiy** (built-in Home, unchanged) | Clipboard, Font, Paragraph (with ¶ showing its state), Styles, Editing, Voice, Editor | 38 keytip controls |
| **Kiritish** (built-in Insert, unchanged) | Pages, Tables, Illustrations, Links, Comments, Header & Footer, Text, Symbols | 24 keytip controls |
| **Hujjat** (ours) | Fayl: Print, Export PDF · Sahifa: Orientation, Page Setup · Imlo: Spelling & Grammar · Transliteratsiya: three directions · Tarix: Version History | 9 buttons |
| Contextual tabs (upstream) | Table Design/Layout, Picture/Shape Format, Equation, Chart/SmartArt Design: still appear on selection (`ribbon.rs:206-212`); hiding tabs does not affect them | — |

Every id, with params, labels and its registration line, is in **Appendix A.1**.

The preset's tab and group names are literal text (`ribbon.rs:288-289`). `docs-office` sets them from these reviewed
rows and re-applies the preset when the interface language changes:

| Name in the preset | en (catalog source) | uz-Latn | uz-Cyrl | ru | Status |
|---|---|---|---|---|---|
| tab | Document | Hujjat | Ҳужжат | Документ | reviewed (`wordcraft.tsv`) |
| group | File | Fayl | Файл | Файл | reviewed |
| group | Page | Sahifa | Саҳифа | Страница | reviewed |
| group | Spelling | Imlo | Имло | Орфография | reviewed |
| group | Transliterate | Transliteratsiya | Транслитерация | Транслитерация | reviewed |
| group | History | Tarix | Тарих | **missing** | glossary row only (`glossary.tsv`); **new app row needed (WP17)** |

### 3.3 What moves behind the full ribbon
These 10 built-in tabs are hidden: Draw, Design, Layout, References, Mailings, Review, View, Developer, Zotero and Help
(the tab list is in `wordcraft/crates/engine/src/ribbon.rs:18-19`). Notable commands that only the full ribbon shows:
- margins presets, columns, breaks and hyphenation (Layout; Page Setup covers margins and paper);
- table of contents and footnotes (References);
- mail merge (Mailings);
- the Review tab's comment navigation and accept/reject (track changes itself stays on the tab strip);
- views and zoom (View; zoom also stays on the status bar);
- watermark and page colour (Design).

Everything also stays reachable through Alt+Q command search.

### 3.4 Findings that shaped the Docs tab
1. **Custom groups run every command with `{}`** (`wordcraft/crates/ui-egui/src/ribbon.rs:424-444`). They also never
   show a toggle state (`:431`, `false`). Consequences:
   - `file.exportPdf` requires `path` (`cmd/file.rs:82-90`), so it **cannot** sit in a custom group.
     `layout.margins {}` is a no-op (`cmd/page.rs:460-472`), and `layout.size {}` fails (`:515-523`). The Sahifa
     group therefore uses `layout.orientation` (toggles, `:18-26`) and `layout.pageSetup` (opens the dialog,
     `:42-48`).
   - `ui.dialog` targets (Word Count, Replace) are UI commands, not registry ids. `RibbonLayout::add` refuses them
     (`ribbon.rs:275-278`).
2. **Export PDF needs no upstream hunk.**
   - `file.print` opens File › Print (`cmd/file.rs:106-111`, handled at `ui-egui/src/lib.rs:1249-1252`). That page draws
     the "save a copy" formats, **PDF first**, below the print settings (`ui-egui/src/print_page.rs:220`, `:312-327`).
   - So `office.exportPdf` (ours, label "Export PDF", a reviewed row) pushes the same `{"open":"print"}`.
   - If WP23 shows staff look for a direct PDF dialog, the fix is one match arm in `handle_request`
     (`ui-egui/src/lib.rs:1240-1259`): `"exportPdf" => self.export_dialog("pdf")` (the function is at
     `file_dialogs.rs:282`). That is a carried line in a hot file, so it is not the default.
3. **¶ is on the QAT as well as on Home.**
   - Home shows its on/off state (`ribbon.rs:944-945`).
   - The QAT is visible on every tab, so ¶ never hides behind a tab switch. The QAT has no on/off state (`chrome.rs:42-62`).
   - Ctrl+Shift+8 works everywhere (`cmd/view.rs:23`).
4. **Icons.** Custom-group and QAT buttons take their icon from the id's last segment (`ui-egui/src/widgets.rs:367-392`).
   - `print`, `exportPdf`, `translitCyrl`/`Latn`/`Auto`, `history`, `ribbonFull` and `ribbonSimple` have no icon there,
     so they draw upstream's fallback: a tile with the first letter (`icons.rs:1437-1442`).
   - Accept that for now. If the WP2 screenshots look wrong, add rows to the `ICONS` table in the wordcraft overlay
     patch (data in a hot file).
5. **Home has a dead button.** "Dictate" (`ribbon.rs:1012`, keytip `keytips.rs:181`) runs `tools.dictate`, which **is not
   registered at this BASE**. `widgets::enabled` (`widgets.rs:92-105`) therefore greys it out. It is upstream's, and
   we cannot hide one built-in group. Recorded; it is not fixed here.
6. **Two "Version History" entries could confuse staff.**
   - Upstream's `file.versions` (`cmd/tools.rs:397`) keeps in-memory versions under File › Info.
   - Ours (`office.history`, WP10) opens the shell's on-disk History panel.
   - WP10 should re-register `file.versions` from `docs-office` to point at ours (F2 allows a re-registration), so
     there is one feature with one name.

---

## 4. Sheets (gridcraft): one row of our own

### 4.1 Mock-up (simple mode, wide window ≥ 1500 px)
```
┌ Title bar (upstream) ──────────────────────────────────────────────────────────────────────────────────────────┐
│ Avtosaqlash (○) [⌂][Saqlash][Qaytarish▾][Qayta bajarish][⋯]   Tabel_oktyabr.xlsx •   [🔍][Ulashish][💬][◐] │
├ Sodda panel (ours) ────────────────────────────────────────────────────────────────────────────────────────────┤
│ [Calibri ▾][11▾] Q K T A▾ ▒▾ ▦▾ │ ⯇ ≡ ⯈ ↵ ⊞ │ [Umumiy ▾] │ ⊕ Kiritish▾ ⊖ Oʻchirish▾ │ Σ▾ ⇅ Saralash▾ ⏷ Filtr │ │
│ ❄ Qotirish▾ ƒx │ 🖶 Chop etish  ⇄ Transliteratsiya▾  🕓 Versiyalar tarixi                           [Toʻliq lenta] │
├ Formula bar, grid, sheet tabs (upstream) ──────────────────────────────────────────────────────────────────────┤
```
(The row is one line on screen; it is wrapped here only to fit the page.)

**Narrow window (< 1500 px):**
- labels drop from the menu buttons, leaving icons with tooltips and accessible names;
- below about 1100 px the items from the right overflow into a "…" menu, in reverse order of the list below;
- the toggle never overflows.

WP3 measures both thresholds.

**Full mode:** upstream's ribbon, unchanged, plus a "Sodda lenta" pill at the right end of the tab row.

### 4.2 What is on it (25 controls, 35 ids: 32 upstream and 3 ours)
| Group | Controls | Command ids |
|---|---|---|
| Font | font, size, B, I, U, font colour, fill colour, borders | `home.fontName`, `home.fontSize`, `home.bold`, `home.italic`, `home.underline`, `home.fontColor`, `home.fillColor`, `home.borders` |
| Alignment | left, centre, right, wrap, merge & centre | `home.alignLeft`, `home.alignCenter`, `home.alignRight`, `home.wrapText`, `home.mergeCenter` |
| Number | one picker: General, Number, Short Date, Percentage, **Text** (staff numbers and passport series keep their leading zeros), More… | `home.numberFormat` `{"format": …}` (names from `format_code_for`, `ribbon.rs:1117-1143`) |
| Cells | Insert ▾ (rows, columns, sheet), Delete ▾ (rows, columns) | `home.insertRows`, `home.insertColumns`, `home.insertSheet`, `home.deleteRows`, `home.deleteColumns` |
| Data | AutoSum, Sort ▾ (A→Z, Z→A), Filter | `formulas.autoSum`, `data.sortAscending`, `data.sortDescending`, `data.filter` |
| View | Freeze ▾ (panes, top row, first column, unfreeze), Show formulas | `view.freezePanes`, `view.freezeTopRow`, `view.freezeFirstColumn`, `view.unfreezePanes`, `formulas.showFormulas` |
| Output | Print, Transliterate ▾ (3), Version history | `file.print`, `tools.translitCyrl`, `tools.translitLatn`, `tools.translitAuto`, `office.history` (ours) |
| Toggle | Full ribbon / Simple ribbon | `office.ribbonFull`, `office.ribbonSimple` (ours) |

Labels, params and registration lines: **Appendix A.2**.

The data table is `domains/sheets/office_toolbar.toml`: one entry per control, holding id, params, label key, icon
and group. It is drawn with upstream's `pub` widgets (`gridcraft/crates/ui-egui/src/widgets.rs:18-109`). The font,
size and number pickers reuse `font_combo`/`size_combo`/`number_combo` (`ribbon.rs:1064-1143`), each a one-word
`pub(crate)` hunk. Paste stays on Ctrl+V and the context menu, as Excel users expect. It is left off the row on purpose.

### 4.3 What moves behind the full ribbon
- Clipboard, cell styles, conditional formatting, Format as Table and Find & Select (Home);
- PivotTable, charts, pictures, shapes and links (Insert);
- the whole of Draw, Page Layout, Formulas (except AutoSum and Show Formulas), Data (except sort and filter), Review,
  View (except freeze) and Automate.

Page setup is also reachable from Print. Charts are reachable through command search (Ctrl+Shift+Space).

### 4.4 Sheets specifics
- **Never covers the Excel workload signals** (PLAN §6): the row adds no work per frame beyond drawing about 25 widgets.
  WP12's memory and FPS re-run includes the simple toolbar.
- **`file.print`** is routed by the app to the system print path (`gridcraft/crates/ui-egui/src/lib.rs:520`, `:572-583`).
  Our button uses `app.run_or_alert("file.print", {})` exactly as Ctrl+P does (`keymap.rs:109`).

---

## 5. Slides (deckcraft): one row of our own

### 5.1 Mock-up (simple mode)
```
┌ Title bar (upstream) ──────────────────────────────────────────────────────────────────────────────────────────┐
│ [⌂][Saqlash][Eksport][Qaytarish][Qayta bajarish][▶ Boshidan]   Hisobot 2026.pptx    [🔍][ Namoyish qilish ][💬][◐] │
├ Sodda panel (ours) ────────────────────────────────────────────────────────────────────────────────────────────┤
│ [+ Yangi slayd ▾][Maket ▾] │ [Calibri ▾][24▾] Q K T A▾ │ • 1. ⯇ ≡ ⯈ │ [T] Matn qutisi  🖼 Rasm  ◇ Shakllar▾  ▦ Jadval▾ │ │
│ ⇄ Transliteratsiya▾  🕓 Versiyalar tarixi                                                         [Toʻliq lenta] │
├ Slide thumbnails | slide | notes (upstream) ───────────────────────────────────────────────────────────────────┤
```
(One line on screen; wrapped here to fit the page.)

### 5.2 What is on it (20 controls, 23 ids: 20 upstream and 3 ours)
| Group | Controls | Command ids |
|---|---|---|
| Slides | New slide ▾ (layout gallery), Layout ▾ | `slide.new`, `slide.layout` |
| Font | font, size, B, I, U, colour | `format.font`, `format.size`, `format.bold`, `format.italic`, `format.underline`, `format.color` |
| Paragraph | bullets, numbering, left, centre, right | `format.bullets`, `format.numbering`, `format.alignLeft`, `format.alignCenter`, `format.alignRight` |
| Insert | text box, picture, shapes ▾, table ▾ | `insert.textBox`, `app.insertPictureDialog` (UI command), `shape.insert`, `insert.table` |
| Output | Transliterate ▾ (3), Version history | `tools.translitCyrl`, `tools.translitLatn`, `tools.translitAuto`, `office.history` (ours) |
| Toggle | Full ribbon / Simple ribbon | `office.ribbonFull`, `office.ribbonSimple` (ours) |

Labels and lines: **Appendix A.3**.

Upstream helpers are reused as they are. These are already `pub`:
- `fmt_state`, `font_families`, `shapes_gallery`, `table_grid_picker` and `translit_menu`
  (`deckcraft/crates/ui-egui/src/ribbon.rs:331`, `:472`, `:819`, `:1308`, `:1160`);
- the widgets (`widgets.rs:30-223`).

`layout_menu` (`ribbon.rs:381`) needs a one-word `pub(crate)` hunk.

### 5.3 What moves behind the full ribbon
- Clipboard, sections, the paragraph details, the drawing gallery, Find/Replace and Designer (Home);
- the whole of Insert's media, links, header/footer and symbols;
- Draw, Design, Transitions, Animations, Slide Show (beyond starting it), Record, Review and View.

**Print is not on the row** (owner question 3). It stays on Ctrl+P, on File › Print, and in the QAT's Customize menu
(`qat.rs:42-55`).

---

## 6. PDF (pdfcraft): unchanged

### 6.1 Mock-up (what is there today)
```
┌ Tab strip (upstream) ──────────────────────────────────────────────────────────────────────────────────────────┐
│ [Buyruq_12.pdf ×] [Hisobot.pdf ×]  +                                                              [–][□][×]  │
├ Mode bar ──────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ [☰ Menyu] │ Barcha vositalar  Oʻqish  Tahrirlash  Aylantirish  Elektron imzo   [🔍 Vositalar va buyruqlarni topish] ⓘ 💾 🖶 │
├ Home (no file open) / document + right rail ───────────────────────────────────────────────────────────────────┤
```
Sources:
- the mode tabs: `pdfcraft/crates/ui-egui/src/chrome.rs:508-514`;
- the right-hand icons: `:516-531`;
- the menu: `:541-624`;
- the Home view's recommended tools (`organize, comment, form, edit, protect`): `home.rs:10`.

### 6.2 Decision
- No patch and no toggle: there is no ribbon (PLAN F5).
- **What WP23 checks:** that staff find **PDF → Word**, which is the Convert mode and the `export.docx` tool
  (`pdfcraft/crates/engine/src/commands.rs:324`, catalog `catalog.rs:86`).
- If they do not, the cheapest fix is a carried one-line change to `RECOMMENDED` (`home.rs:10`) that puts `export`
  first. Ids and labels: **Appendix A.4**.

---

## 7. The compact (360 dp) variant (input to WP16; nothing is built for phones here)
| App | Compact row (left to right; the rest goes to "…") | Note |
|---|---|---|
| Sheets | Q · K · toʻldirish ▾ · tekislash ▾ (L/C/R/wrap/merge) · son formati ▾ · Σ · saralash/filtr ▾ · … | 8 targets of 40 dp; the font pickers go to "…" |
| Slides | yangi slayd · Q · K · rang ▾ · markerlar · tekislash ▾ · rasm · … | 8 targets |
| Docs | **No compact row.** The ribbon falls back to upstream's own overflow: tabs go to a » menu (`ribbon.rs:144`, `:272-335`), and groups scroll with edge chevrons (`:395-418`) | If B-3 picks A-web, Docs needs the one-row design (owner question 1's alternative). WP16 measures the 360 dp fit and says so. |
| PDF | the upstream mode bar | WP16 measures whether 5 mode tabs and the 260 px search box fit; they likely do not |

The compact rows are a different `[compact]` list in the same `office_toolbar.toml`, so WP16 can measure them with no
second implementation.

## 8. Labels: status and what WP17 must add
Source: `i18n/<app>.tsv` and `i18n/shared.tsv` (rows whose `apps` include the app), matched on `source_en`, read
2026-10-22. "Reviewed" means both `uz_latn_status` and `uz_cyrl_status` are `reviewed:agent-*`. **No label on the four
toolbars has an unreviewed Uzbek row.** The gaps are new strings, rows that exist in one app's catalog but not
another's, and missing Russian.

**New rows (no catalog row in that app).** The drafts are the agent's. WP17 runs the uz-Cyrl transliteration and the
opus review, and marks the result `reviewed:agent-opus:<date>`.

| en | Apps | Draft uz-Latn | Draft uz-Cyrl | Draft ru | Note |
|---|---|---|---|---|---|
| Full ribbon | docs, sheets, slides | Toʻliq lenta | Тўлиқ лента | Полная лента | "Lenta" is the reviewed term (`glossary.tsv` Ribbon) |
| Simple ribbon | docs, sheets, slides | Sodda lenta | Содда лента | Простая лента | same |
| Version History | sheets, slides | Versiyalar tarixi | Версиялар тарихи | Журнал версий | reviewed in `wordcraft.tsv`; copy the row |
| Font Size | sheets | Shrift oʻlchami | Шрифт ўлчами | Размер шрифта | reviewed in `deckcraft.tsv`; copy the row |
| Number Format | sheets | Son formati | Сон формати | Числовой формат | new |
| History (Docs group name) | docs | Tarix | Тарих | История | `glossary.tsv` has uz only |
| Simple / full ribbon (shell Settings) | shell | Asboblar paneli | Асбоблар панели | Панель инструментов | `glossary.tsv` Toolbar; goes to `i18n/shell.tsv` |

**Rows with reviewed Uzbek but no Russian** (`ru_status = todo`). ru users see English for these:

| App | en |
|---|---|
| gridcraft | Save (⌘S), Undo (⌘Z), Redo (⌘Y) (title bar tooltips), Insert Sheet |
| deckcraft | Redo, Slide Show, New Slide, Underline |

**Wording proposal (agent, not blocking):** the reviewed "Koʻrsatish/yashirish ¶" is accurate. PLAN §2.1 suggests the
plainer "Belgilarni koʻrsatish ¶" ("show marks"). If WP23 shows staff miss the ¶ button, WP17 changes the row; that
is one catalog edit, and every app picks it up.

**Where our labels live.** New command labels are registry labels, and the apps translate them through their own
embedded catalogs (`tl!(s.label)`, `wordcraft/crates/ui-egui/src/ribbon.rs:427`). So the new rows go into the
**app** catalogs as well as `i18n/shell.tsv`, or the buttons show English.

## 9. Command-id verification at the current BASE
**Method.**
- A script indexed every registered id in the four trees:
  - wordcraft `CommandSpec::new("…"`: 629 ids;
  - gridcraft `cmd!(… "…"`: 344 ids;
  - deckcraft `cmd!(… "…"`: 324 ids, plus the 51 `UI_COMMANDS` (`deckcraft/crates/ui-egui/src/lib.rs:388-445`);
  - pdfcraft `c("…"`/`ct("…"`: 192 ids;
  - wordcraft `ui.*` arms in `WordApp::ui_command` (`wordcraft/crates/ui-egui/src/lib.rs:1299-1560`).
- Every id in Appendix A was looked up there. The first registration line is given.
- The translit ids in Docs are ours (`domains/docs/crates/docs-translit/src/lib.rs:24-26`). In Sheets and Slides they
  come from our carried patch 0120 (`cmd/translit.rs`).

**Result.**
- Every upstream id used by our own rows (the Docs QAT and Hujjat tab, Sheets, Slides, PDF) **exists**.
- **One id on the unchanged Docs Home tab does not exist: `tools.dictate`** (§3.4 item 5).
- Six ids are new and ours: `office.ribbonFull`, `office.ribbonSimple`, `office.exportPdf`, and `office.history`
  (×3 apps).

**What keeps it true.**
- WP2/WP3's `office_toolbar_ids_exist` tests assert each id in `office_toolbar.toml` and in the Docs preset against the
  live registry.
- **WP0b re-runs this check after the WP0a sync lands**: every line in Appendix A, plus F4's call sites
  (`gridcraft/crates/ui-egui/src/lib.rs:1149`, `deckcraft/crates/ui-egui/src/lib.rs:1836`). It records "moved" or
  "confirmed" in `docs/phase4/wp0-notes.md`.

## 10. Counts (simple vs full)
| App | Simple: controls on screen | Simple: command ids | Full ribbon: distinct ids reachable | Registry |
|---|---|---|---|---|
| Docs | Home in front: 38 + QAT 5 + strip 2 = 45; across the 3 tabs: 78 | 76 (61 upstream on Home/Insert/strip, 10 upstream on QAT/Hujjat, 5 ours) | ≈357 (every id named in `ribbon.rs`, `equation_tab.rs` and `chrome.rs` that is registered) | 629 + 31 UI |
| Sheets | 25 (+ 5 on the title bar) | 35 (32 upstream + 3 ours) | ≈204 (171 ids + 33 `dialog:` targets in `ribbon.rs`/`paper_view.rs`) | 344 |
| Slides | 20 (+ 6 on the QAT, 4 right) | 23 (20 upstream + 3 ours) | ≈195 (in `ribbon.rs`/`qat.rs`) | 324 + 51 UI |
| PDF | unchanged (mode bar: 5 modes + 4 actions + menu) | 9 listed | n/a (no ribbon) | 192 |

"≈" because the ribbon files also hold ids in menus and galleries. The counts are distinct quoted ids that resolve
in the registry, outside `#[cfg(test)]`.

## 11. Owner review (Q5)
| # | Question | Recommendation | Owner's answer |
|---|---|---|---|
| 1 | Docs: three-tab preset (Asosiy, Kiritish, Hujjat), or a one-row toolbar like Sheets (+2 d, R3)? | Preset now; WP23 decides whether to switch | _pending_ |
| 2 | One toolbar setting for all apps, or one per app? | One setting | _pending_ |
| 3 | Slides: Print not on the simplified row (Ctrl+P, File › Print, QAT)? | Yes | _pending_ |

Label wording stays delegated to the agent (`reviewed:agent-*`). The owner only reports problems found in use.

---

## Appendix A. Every command id, with labels and its registration line

Columns:
- *uz-Latn / en / ru*: the label the control shows, or its tooltip and accessible name for icon-only controls.
- *Label status*: as in §8.
- *Registered at BASE*: the first registration, relative to `upstream/` (or `domains/` where it says so).

### A.1 Docs

#### Docs: Quick Access Toolbar (always visible, both modes)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | QAT | `file.save` | — | Saqlash | Save | Сохранить | reviewed | `wordcraft/crates/engine/src/cmd/file.rs:72` |
| 2 | QAT | `edit.undo` | — | Qaytarish | Undo | Отменить | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:36` |
| 3 | QAT | `edit.redo` | — | Qayta bajarish | Redo | Вернуть | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:53` |
| 4 | QAT | `view.marks` | toggle (Ctrl+Shift+8) | Koʻrsatish/yashirish ¶ | Show/Hide ¶ | Показать/скрыть ¶ | reviewed | `wordcraft/crates/engine/src/cmd/view.rs:18` |
| 5 | QAT | `office.ribbonFull` | shown in simple mode; desktop only | Toʻliq lenta *(draft)* | Full ribbon | Полная лента *(draft)* | **NEW** (WP17) | NEW (ours) |
| 6 | QAT | `office.ribbonSimple` | shown in full mode; desktop only | Sodda lenta *(draft)* | Simple ribbon | Простая лента *(draft)* | **NEW** (WP17) | NEW (ours) |

#### Docs: the "Hujjat" custom tab (ours, the preset)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Fayl | `file.print` | opens File › Print (`{"open":"print"}`) | Chop etish | Print | Печать | reviewed | `wordcraft/crates/engine/src/cmd/file.rs:106` |
| 2 | Fayl | `office.exportPdf` | ours: `{"open":"print"}`; that page's first export button is PDF (§3.4) | PDF sifatida eksport qilish | Export PDF | Экспорт в PDF | reviewed | NEW (ours) |
| 3 | Sahifa | `layout.orientation` | `{}` toggles portrait/landscape | Yoʻnalish | Orientation | Ориентация | reviewed | `wordcraft/crates/engine/src/cmd/page.rs:18` |
| 4 | Sahifa | `layout.pageSetup` | `{}` opens Page Setup (margins, paper) | Sahifa parametrlari | Page Setup | Параметры страницы | reviewed | `wordcraft/crates/engine/src/cmd/page.rs:42` |
| 5 | Imlo | `review.spelling` | `{}` next issue | Imlo va grammatika | Spelling & Grammar | Орфография и грамматика | reviewed | `wordcraft/crates/engine/src/cmd/review.rs:57` |
| 6 | Transliteratsiya | `tools.translitCyrl` | `{}` = scope auto | Lotinchadan kirillchaga | Latin to Cyrillic | С латиницы на кириллицу | reviewed | ours: `domains/docs/crates/docs-translit/src/lib.rs:24` |
| 7 | Transliteratsiya | `tools.translitLatn` | `{}` = scope auto | Kirillchadan lotinchaga | Cyrillic to Latin | С кириллицы на латиницу | reviewed | ours: `domains/docs/crates/docs-translit/src/lib.rs:25` |
| 8 | Transliteratsiya | `tools.translitAuto` | `{}` = scope auto | Avtomatik aniqlash | Auto-detect | Автоопределение | reviewed | ours: `domains/docs/crates/docs-translit/src/lib.rs:26` |
| 9 | Tarix | `office.history` | ours (WP10): opens the shell History panel | Versiyalar tarixi | Version History | Журнал версий | reviewed | NEW (ours) |

#### Docs: tab-strip buttons (upstream, always visible)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Tab strip (right) | `view.commentsPane` | upstream, always visible | Izohlar | Comments | Комментарии | reviewed | `wordcraft/crates/engine/src/cmd/view.rs:74` |
| 2 | Tab strip (right) | `review.trackChanges` | Editing / Reviewing menu `{"value": bool}` | Tahrirlash | Editing | Редактирование | reviewed | `wordcraft/crates/engine/src/cmd/review.rs:27` |

#### Docs: Home tab (built-in, unchanged; the controls that carry a keytip, in drawing order)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Clipboard | `edit.paste` | split: Keep Text Only, Merge, Paste Special | Qoʻyish | Paste | Вставить | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:64` |
| 2 | Clipboard | `edit.cut` | — | Kesib olish | Cut | Вырезать | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:62` |
| 3 | Clipboard | `edit.copy` | — | Nusxalash | Copy | Копировать | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:63` |
| 4 | Clipboard | `edit.formatPainter` | toggle | Formatni nusxalash | Format Painter | Формат по образцу | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:146` |
| 5 | Font | `format.font` | combo `{"name"}` | Shrift | Font | Шрифт | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:98` |
| 6 | Font | `format.size` | combo `{"size"}` | Shrift oʻlchami | Font Size | Размер шрифта | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:99` |
| 7 | Font | `format.growFont` | — | Shrift oʻlchamini oshirish | Increase Font Size | Увеличить размер шрифта | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:100` |
| 8 | Font | `format.shrinkFont` | — | Shrift oʻlchamini kamaytirish | Decrease Font Size | Уменьшить размер шрифта | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:101` |
| 9 | Font | `format.changeCase` | menu `{"mode"}` | Harf registri | Change Case | Регистр | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:116` |
| 10 | Font | `format.clear` | — | Barcha formatlashni tozalash | Clear All Formatting | Очистить всё форматирование | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:119` |
| 11 | Font | `format.bold` | toggle | Qalin | Bold | Полужирный | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:17` |
| 12 | Font | `format.italic` | toggle | Kursiv | Italic | Курсив | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:31` |
| 13 | Font | `format.underline` | split `{"style"}` | Tagi chizilgan | Underline | Подчёркнутый | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:44` |
| 14 | Font | `format.strikethrough` | toggle | Ustidan chizilgan | Strikethrough | Зачёркнутый | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:55` |
| 15 | Font | `format.subscript` | toggle | Quyi indeks | Subscript | Подстрочный | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:66` |
| 16 | Font | `format.superscript` | toggle | Yuqori indeks | Superscript | Надстрочный | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:75` |
| 17 | Font | `format.highlight` | split `{"color"}` | Matnni ajratish rangi | Text Highlight Color | Цвет выделения текста | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:105` |
| 18 | Font | `format.color` | split `{"color"}` | Shrift rangi | Font Color | Цвет шрифта | reviewed | `wordcraft/crates/engine/src/cmd/format.rs:104` |
| 19 | Paragraph | `para.bullets` | split `{"kind"}` | Markerlar | Bullets | Маркеры | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:137` |
| 20 | Paragraph | `para.numbering` | split `{"kind"}` | Raqamlash | Numbering | Нумерация | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:138` |
| 21 | Paragraph | `para.multilevel` | split `{"kind"}` | Koʻp darajali roʻyxat | Multilevel List | Многоуровневый список | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:140` |
| 22 | Paragraph | `para.outdent` | — | Chekinishni kamaytirish | Decrease Indent | Уменьшить отступ | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:41` |
| 23 | Paragraph | `para.indent` | — | Chekinishni oshirish | Increase Indent | Увеличить отступ | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:40` |
| 24 | Paragraph | `para.sort` | — | Saralash | Sort | Сортировка | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:167` |
| 25 | Paragraph | `view.marks` | toggle, **shows its on/off state** | Koʻrsatish/yashirish ¶ | Show/Hide ¶ | Показать/скрыть ¶ | reviewed | `wordcraft/crates/engine/src/cmd/view.rs:18` |
| 26 | Paragraph | `para.alignLeft` | toggle | Chapga tekislash | Align Left | По левому краю | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:15` |
| 27 | Paragraph | `para.alignCenter` | toggle | Markaz | Center | По центру | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:16` |
| 28 | Paragraph | `para.alignRight` | toggle | Oʻngga tekislash | Align Right | По правому краю | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:17` |
| 29 | Paragraph | `para.justify` | toggle | Kenglik boʻyicha | Justify | По ширине | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:18` |
| 30 | Paragraph | `para.lineSpacing` | menu `{"value"}` | Satr va xatboshi oraligʻi | Line and Paragraph Spacing | Интервал между строками и абзацами | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:75` |
| 31 | Paragraph | `para.shading` | split `{"color"}` | Soyalash | Shading | Заливка | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:159` |
| 32 | Paragraph | `para.borders` | split `{"kind"}` | Chegaralar | Borders | Границы | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:164` |
| 33 | Styles | `styles.pane` | gallery + pane | Uslublar | Styles | Стили | reviewed | `wordcraft/crates/engine/src/cmd/para.rs:221` |
| 34 | Editing | `edit.find` | menu: Find, Advanced Find, Go To | Topish | Find | Найти | reviewed | `wordcraft/crates/engine/src/cmd/edit.rs:95` |
| 35 | Editing | `ui.dialog` | `{"name":"replace"}` | Almashtirish | Replace | Заменить | reviewed | `wordcraft/crates/ui-egui/src/lib.rs:1322` (UI command) |
| 36 | Editing | `select.all` | menu: all, paragraph, sentence | Tanlash | Select | Выделить | reviewed | `wordcraft/crates/engine/src/cmd/caret.rs:53` |
| 37 | Voice | `tools.dictate` | upstream button, greyed (§3.4 item 5) | Ovoz bilan yozish | Dictate | Диктовка | reviewed | **NOT FOUND** |
| 38 | Editor | `review.spelling` | — | Muharrir | Editor | Редактор | reviewed | `wordcraft/crates/engine/src/cmd/review.rs:57` |

#### Docs: Insert tab (built-in, unchanged; the controls that carry a keytip)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Pages | `insert.coverPage` | menu | Titul sahifasi | Cover Page | Титульная страница | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:25` |
| 2 | Pages | `insert.blankPage` | — | Boʻsh sahifa | Blank Page | Пустая страница | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:17` |
| 3 | Pages | `insert.pageBreak` | — | Sahifa uzilishi | Page Break | Разрыв страницы | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:13` |
| 4 | Tables | `insert.table` | grid picker `{"rows","cols"}` | Jadval | Table | Таблица | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:26` |
| 5 | Illustrations | `insert.picture` | — | Rasmlar | Pictures | Рисунки | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:27` |
| 6 | Illustrations | `insert.shape` | menu `{"kind"}` | Shakllar | Shapes | Фигуры | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:30` |
| 7 | Illustrations | `insert.icon` | — | Piktogrammalar | Icons | Значки | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:28` |
| 8 | Illustrations | `insert.smartArt` | — | SmartArt | SmartArt | SmartArt | reviewed | `wordcraft/crates/engine/src/cmd/smart_art.rs:31` |
| 9 | Illustrations | `insert.chart` | — | Diagramma | Chart | Диаграмма | reviewed | `wordcraft/crates/engine/src/cmd/chart.rs:30` |
| 10 | Links | `ui.dialog` | `{"name":"link"}` | Havola | Link | Ссылка | reviewed | `wordcraft/crates/ui-egui/src/lib.rs:1322` (UI command) |
| 11 | Links | `insert.crossReference` | — | Oʻzaro havola | Cross-reference | Перекрёстная ссылка | reviewed | `wordcraft/crates/engine/src/cmd/citations.rs:344` |
| 12 | Comments | `review.newComment` | — | Izoh | Comment | Комментарий | reviewed | `wordcraft/crates/engine/src/cmd/review.rs:14` |
| 13 | Header & Footer | `insert.header` | menu `{"preset"}` | Yuqori kolontitul | Header | Верхний колонтитул | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:51` |
| 14 | Header & Footer | `insert.footer` | menu `{"preset"}` | Quyi kolontitul | Footer | Нижний колонтитул | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:52` |
| 15 | Header & Footer | `insert.pageNumber` | menu `{"position","align"}` | Sahifa raqami | Page Number | Номер страницы | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:53` |
| 16 | Text | `insert.textBox` | — | Matn qutisi | Text Box | Текстовое поле | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:32` |
| 17 | Text | `insert.quickParts` | menu | Tayyor bloklar | Quick Parts | Готовые блоки | reviewed | `wordcraft/crates/engine/src/cmd/tools.rs:458` |
| 18 | Text | `insert.wordArt` | gallery | WordArt | WordArt | WordArt | reviewed | `wordcraft/crates/engine/src/cmd/wordart.rs:22` |
| 19 | Text | `insert.dropCap` | — | Birinchi katta harf | Drop Cap | Буквица | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:109` |
| 20 | Text | `insert.signatureLine` | — | Imzo qatori | Signature Line | Строка подписи | reviewed | `wordcraft/crates/engine/src/cmd/tools.rs:487` |
| 21 | Text | `insert.dateTime` | — | Sana va vaqt | Date & Time | Дата и время | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:99` |
| 22 | Text | `insert.object` | — | Obyekt | Object | Объект | reviewed | `wordcraft/crates/engine/src/cmd/tools.rs:507` |
| 23 | Symbols | `insert.equation` | — | Tenglama | Equation | Уравнение | reviewed | `wordcraft/crates/engine/src/cmd/equation.rs:25` |
| 24 | Symbols | `insert.symbol` | grid `{"char"}` | Simvol | Symbol | Символ | reviewed | `wordcraft/crates/engine/src/cmd/insert.rs:100` |

### A.2 Sheets

#### Sheets: title bar (upstream, always visible; not repeated in the toolbar)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Title bar (upstream) | `file.save` | always visible (`ribbon.rs:71-76`) | Saqlash (⌘S) | Save (⌘S) | — | reviewed; **ru missing** | `gridcraft/crates/engine/src/cmd/file.rs:12` |
| 2 | Title bar (upstream) | `edit.undo` | always visible, with the undo list | Qaytarish (⌘Z) | Undo (⌘Z) | — | reviewed; **ru missing** | `gridcraft/crates/engine/src/cmd/edit.rs:41` |
| 3 | Title bar (upstream) | `edit.redo` | always visible | Qayta bajarish (⌘Y) | Redo (⌘Y) | — | reviewed; **ru missing** | `gridcraft/crates/engine/src/cmd/edit.rs:42` |

#### Sheets: simplified toolbar (ours)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Font | `home.fontName` | combo `{"name"}` | Shrift | Font | Шрифт | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:55` |
| 2 | Font | `home.fontSize` | combo `{"size"}` | Shrift oʻlchami *(draft)* | Font Size | Размер шрифта *(draft)* | **NEW** (WP17) | `gridcraft/crates/engine/src/cmd/format.rs:56` |
| 3 | Font | `home.bold` | toggle | Qalin | Bold | Полужирный | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:12` |
| 4 | Font | `home.italic` | toggle | Kursiv | Italic | Курсив | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:18` |
| 5 | Font | `home.underline` | toggle | Tagi chizilgan | Underline | Подчёркивание | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:25` |
| 6 | Font | `home.fontColor` | split `{"color"}` | Shrift rangi | Font Color | Цвет шрифта | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:62` |
| 7 | Font | `home.fillColor` | split `{"color"}` | Toʻldirish rangi | Fill Color | Цвет заливки | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:70` |
| 8 | Font | `home.borders` | split, upstream border menu | Chegaralar | Borders | Границы | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:72` |
| 9 | Alignment | `home.alignLeft` | toggle | Chapga tekislash | Align Left | По левому краю | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:90` |
| 10 | Alignment | `home.alignCenter` | toggle | Markaz | Center | По центру | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:91` |
| 11 | Alignment | `home.alignRight` | toggle | Oʻngga tekislash | Align Right | По правому краю | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:92` |
| 12 | Alignment | `home.wrapText` | toggle | Matnni oʻrash | Wrap Text | Переносить текст | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:97` |
| 13 | Alignment | `home.mergeCenter` | toggle | Birlashtirish va markazlash | Merge & Center | Объединить и поместить в центре | reviewed | `gridcraft/crates/engine/src/cmd/format.rs:122` |
| 14 | Number | `home.numberFormat` | picker `{"format"}`: General, Number, Short Date, Percentage, Text; "More…" opens Format Cells | Son formati *(draft)* | Number Format | Числовой формат *(draft)* | **NEW** (WP17) | `gridcraft/crates/engine/src/cmd/format.rs:128` |
| 15 | Cells | `home.insertRows` | menu "Insert ▾" | Varaq qatorlarini kiritish | Insert Sheet Rows | Вставить строки листа | reviewed | `gridcraft/crates/engine/src/cmd/sheet.rs:15` |
| 16 | Cells | `home.insertColumns` | menu "Insert ▾" | Varaq ustunlarini kiritish | Insert Sheet Columns | Вставить столбцы листа | reviewed | `gridcraft/crates/engine/src/cmd/sheet.rs:23` |
| 17 | Cells | `home.insertSheet` | menu "Insert ▾" | Varaq kiritish | Insert Sheet | — | reviewed; **ru missing** | `gridcraft/crates/engine/src/cmd/sheet.rs:50` |
| 18 | Cells | `home.deleteRows` | menu "Delete ▾" | Varaq qatorlarini oʻchirish | Delete Sheet Rows | Удалить строки листа | reviewed | `gridcraft/crates/engine/src/cmd/sheet.rs:28` |
| 19 | Cells | `home.deleteColumns` | menu "Delete ▾" | Varaq ustunlarini oʻchirish | Delete Sheet Columns | Удалить столбцы листа | reviewed | `gridcraft/crates/engine/src/cmd/sheet.rs:33` |
| 20 | Data | `formulas.autoSum` | `{}` = SUM | Avtoyigʻindi | AutoSum | Автосумма | reviewed | `gridcraft/crates/engine/src/cmd/formulas.rs:12` |
| 21 | Data | `data.sortAscending` | menu "Sort ▾" | A dan Z gacha saralash | Sort A to Z | Сортировка от А до Я | reviewed | `gridcraft/crates/engine/src/cmd/data.rs:16` |
| 22 | Data | `data.sortDescending` | menu "Sort ▾" | Z dan A gacha saralash | Sort Z to A | Сортировка от Я до А | reviewed | `gridcraft/crates/engine/src/cmd/data.rs:19` |
| 23 | Data | `data.filter` | toggle | Filtr | Filter | Фильтр | reviewed | `gridcraft/crates/engine/src/cmd/data.rs:30` |
| 24 | View | `view.freezePanes` | menu "Freeze ▾" | Sohalarni qotirish | Freeze Panes | Закрепить области | reviewed | `gridcraft/crates/engine/src/cmd/view.rs:12` |
| 25 | View | `view.freezeTopRow` | menu "Freeze ▾" | Yuqori qatorni qotirish | Freeze Top Row | Закрепить верхнюю строку | reviewed | `gridcraft/crates/engine/src/cmd/view.rs:20` |
| 26 | View | `view.freezeFirstColumn` | menu "Freeze ▾" | Birinchi ustunni qotirish | Freeze First Column | Закрепить первый столбец | reviewed | `gridcraft/crates/engine/src/cmd/view.rs:21` |
| 27 | View | `view.unfreezePanes` | menu "Freeze ▾" | Qotirishni bekor qilish | Unfreeze Panes | Снять закрепление областей | reviewed | `gridcraft/crates/engine/src/cmd/view.rs:25` |
| 28 | View | `formulas.showFormulas` | toggle | Formulalarni koʻrsatish | Show Formulas | Показать формулы | reviewed | `gridcraft/crates/engine/src/cmd/formulas.rs:53` |
| 29 | Output | `file.print` | the app routes it to `print_now` (`lib.rs:520`) | Chop etish | Print | Печать | reviewed | `gridcraft/crates/engine/src/cmd/print.rs:31` |
| 30 | Output | `tools.translitCyrl` | menu "Transliterate ▾" | Lotinchadan kirillchaga | Latin to Cyrillic | С латиницы на кириллицу | reviewed | `gridcraft/crates/engine/src/cmd/translit.rs:21` (patch 0120) |
| 31 | Output | `tools.translitLatn` | menu "Transliterate ▾" | Kirillchadan lotinchaga | Cyrillic to Latin | С кириллицы на латиницу | reviewed | `gridcraft/crates/engine/src/cmd/translit.rs:30` (patch 0120) |
| 32 | Output | `tools.translitAuto` | menu "Transliterate ▾" | Avtomatik aniqlash | Auto-detect | Автоопределение | reviewed | `gridcraft/crates/engine/src/cmd/translit.rs:39` (patch 0120) |
| 33 | Output | `office.history` | ours (WP10) | Versiyalar tarixi *(draft)* | Version History | Журнал версий *(draft)* | **NEW** (WP17) | NEW (ours) |
| 34 | Toggle | `office.ribbonFull` | right end of the row; desktop only | Toʻliq lenta *(draft)* | Full ribbon | Полная лента *(draft)* | **NEW** (WP17) | NEW (ours) |
| 35 | Toggle | `office.ribbonSimple` | full mode: a pill at the tab row end | Sodda lenta *(draft)* | Simple ribbon | Простая лента *(draft)* | **NEW** (WP17) | NEW (ours) |

### A.3 Slides

#### Slides: title bar (upstream, always visible; not repeated in the toolbar)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Title bar QAT (upstream) | `file.save` | default QAT (`qat.rs:58-64`) | Saqlash | Save | Сохранить | reviewed | `deckcraft/crates/engine/src/cmd/file.rs:32` |
| 2 | Title bar QAT (upstream) | `edit.undo` | default QAT | Qaytarish | Undo | Отменить | reviewed | `deckcraft/crates/engine/src/cmd/edit.rs:15` |
| 3 | Title bar QAT (upstream) | `edit.redo` | default QAT | Qayta bajarish | Redo | — | reviewed; **ru missing** | `deckcraft/crates/engine/src/cmd/edit.rs:16` |
| 4 | Title bar (upstream) | `show.start` | the "Present" button; QAT "Start from Beginning" | Namoyish | Slide Show | — | reviewed; **ru missing** | `deckcraft/crates/ui-egui/src/lib.rs:411` (UI command) |

#### Slides: simplified toolbar (ours)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Slides | `slide.new` | split: layout gallery `{"layout"}` | Yangi slayd | New Slide | — | reviewed; **ru missing** | `deckcraft/crates/engine/src/cmd/slide.rs:15` |
| 2 | Slides | `slide.layout` | gallery `{"layout"}` | Maket | Layout | Макет | reviewed | `deckcraft/crates/engine/src/cmd/slide.rs:33` |
| 3 | Font | `format.font` | combo `{"family"}` | Shrift | Font | Шрифт | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:40` |
| 4 | Font | `format.size` | combo `{"size"}` | Shrift oʻlchami | Font Size | Размер шрифта | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:41` |
| 5 | Font | `format.bold` | toggle | Qalin | Bold | Полужирный | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:18` |
| 6 | Font | `format.italic` | toggle | Kursiv | Italic | Курсив | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:19` |
| 7 | Font | `format.underline` | toggle | Tagi chizilgan | Underline | — | reviewed; **ru missing** | `deckcraft/crates/engine/src/cmd/format.rs:21` |
| 8 | Font | `format.color` | split `{"color"}` | Shrift rangi | Font Color | Цвет шрифта | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:44` |
| 9 | Paragraph | `format.bullets` | toggle | Markerlar | Bullets | Маркеры | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:135` |
| 10 | Paragraph | `format.numbering` | toggle | Raqamlash | Numbering | Нумерация | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:144` |
| 11 | Paragraph | `format.alignLeft` | toggle | Chapga tekislash | Align Left | По левому краю | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:130` |
| 12 | Paragraph | `format.alignCenter` | toggle | Markaz | Center | По центру | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:131` |
| 13 | Paragraph | `format.alignRight` | toggle | Oʻngga tekislash | Align Right | По правому краю | reviewed | `deckcraft/crates/engine/src/cmd/format.rs:132` |
| 14 | Insert | `insert.textBox` | — | Matn qutisi | Text Box | Текстовое поле | reviewed | `deckcraft/crates/engine/src/cmd/insert.rs:36` |
| 15 | Insert | `app.insertPictureDialog` | file picker | Rasm | Picture | Рисунок | reviewed | `deckcraft/crates/ui-egui/src/lib.rs:429` (UI command) |
| 16 | Insert | `shape.insert` | gallery (`shapes_gallery`) | Shakllar | Shapes | Фигуры | reviewed | `deckcraft/crates/engine/src/cmd/insert.rs:17` |
| 17 | Insert | `insert.table` | grid `{"rows","cols"}` (`table_grid_picker`) | Jadval | Table | Таблица | reviewed | `deckcraft/crates/engine/src/cmd/insert.rs:64` |
| 18 | Output | `tools.translitCyrl` | menu "Transliterate ▾" (`translit_menu`) | Lotinchadan kirillchaga | Latin to Cyrillic | С латиницы на кириллицу | reviewed | `deckcraft/crates/engine/src/cmd/translit.rs:19` (patch 0120) |
| 19 | Output | `tools.translitLatn` | menu "Transliterate ▾" | Kirillchadan lotinchaga | Cyrillic to Latin | С кириллицы на латиницу | reviewed | `deckcraft/crates/engine/src/cmd/translit.rs:28` (patch 0120) |
| 20 | Output | `tools.translitAuto` | menu "Transliterate ▾" | Avtomatik aniqlash | Auto-detect | Автоопределение | reviewed | `deckcraft/crates/engine/src/cmd/translit.rs:37` (patch 0120) |
| 21 | Output | `office.history` | ours (WP10) | Versiyalar tarixi *(draft)* | Version History | Журнал версий *(draft)* | **NEW** (WP17) | NEW (ours) |
| 22 | Toggle | `office.ribbonFull` | right end of the row; desktop only | Toʻliq lenta *(draft)* | Full ribbon | Полная лента *(draft)* | **NEW** (WP17) | NEW (ours) |
| 23 | Toggle | `office.ribbonSimple` | full mode: a pill at the tab row end | Sodda lenta *(draft)* | Simple ribbon | Простая лента *(draft)* | **NEW** (WP17) | NEW (ours) |

### A.4 PDF

#### PDF: mode bar, menu and Home (upstream, unchanged)

| # | Group | Command id | Params / picker | uz-Latn | en | ru | Label status | Registered at BASE |
|---|---|---|---|---|---|---|---|---|
| 1 | Mode bar | `view.palette` | search box | Vositalar va buyruqlarni topish | Find tools and commands | Найти инструменты и команды | reviewed | `pdfcraft/crates/engine/src/commands.rs:198` |
| 2 | Mode bar | `print.dialog` | icon | Chop etish ({key}) | Print ({key}) | Печать ({key}) | reviewed | `pdfcraft/crates/engine/src/commands.rs:191` |
| 3 | Mode bar | `file.save` | icon | Saqlash ({key}) | Save ({key}) | Сохранить ({key}) | reviewed | `pdfcraft/crates/engine/src/commands.rs:186` |
| 4 | Mode bar | `file.properties` | icon | Hujjat xususiyatlari ({key}) | Document properties ({key}) | Свойства документа ({key}) | reviewed | `pdfcraft/crates/engine/src/commands.rs:192` |
| 5 | Menu | `file.open` | Menu › File | Ochish… | Open… | Открыть… | reviewed | `pdfcraft/crates/engine/src/commands.rs:164` |
| 6 | Menu | `edit.undo` | Menu › Edit | Qaytarish | Undo | Отменить | reviewed | `pdfcraft/crates/engine/src/commands.rs:194` |
| 7 | Menu | `edit.redo` | Menu › Edit | Qayta bajarish | Redo | Повторить | reviewed | `pdfcraft/crates/engine/src/commands.rs:195` |
| 8 | Convert | `export.docx` | Convert mode / Export tool | Wordga eksport qilish… | Export to Word… | Экспортировать в Word… | reviewed | `pdfcraft/crates/engine/src/commands.rs:324` |
| 9 | Home | `page.combine` | Home and Menu › File | Fayllarni birlashtirish… | Combine files… | Объединить файлы… | reviewed | `pdfcraft/crates/engine/src/commands.rs:185` |
