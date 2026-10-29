# One design language for the shell and the four apps (design and plan)

| | |
|---|---|
| **Status** | Design only, written 2026-10-29 on branch `p4-design` (from `phase-4` 74a2c1f). No product code. Waits on the owner's answers in §9. |
| **Trigger** | Owner feedback from the WP23 usability test (2026-10-29): "all apps must use ONE design language; there must be a global theme". He saw a black backstage on Docs' Recover page and a blue one on its Print page, plain grey default buttons next to styled ones, and a confusing Print page (two "all pages" dropdowns, a long list of "save a copy" buttons, settings in a loose column). He said the other apps have the same problems. |
| **Upstream trees read** | `upstream/{wordcraft,gridcraft,deckcraft,pdfcraft}` as checked out on `phase-4` (queue applied). A `file:line` is relative to `upstream/<repo>/crates/ui-egui/src/` unless it names another path. |
| **Before-screenshots** | `E:/office-target/design-audit/*.png` (41 files, outside the repo, synthetic documents only). Two small composites are committed: [`design/audit-editors.png`](design/audit-editors.png) and [`design/audit-print-recover.png`](design/audit-print-recover.png). |

---

## Egasi uchun qisqacha (oʻzbekcha)

**Nima topildi.**
- Toʻrtta dastur va bosh ekranda **beshta har xil asosiy rang** bor:
  - Hujjatlar: binafsharang-koʻk `#3B5BDB`.
  - Jadvallar: Excel yashili `#107C41`.
  - Taqdimotlar: toʻq sariq `#D95A1A`.
  - PDF: koʻk `#1B63E0`.
  - Bosh ekran: brend koʻki `#1E4FA3`.
- Hujjatlardagi qora "Qayta tiklash" sahifasi xato emas, upstream shunday yozgan (`backstage.rs:45`, `:153`). Chap panel esa qorongʻi rejimda ham koʻk boʻlib qoladi.
- "Chop etish" sahifasidagi ikkinchi "barcha sahifalar" roʻyxati aslida toq/juft sahifa filtri, lekin uning sarlavhasi yoʻq. Uzun format tugmalari roʻyxati paydo boʻlishining sababi shu: Hujjatlar "Chop etish" va "Eksport" sahifalari uchun bitta funksiyani chaqiradi (`backstage.rs:167`).
- Har bir dasturda 100–190 ta oddiy egui tugmasi bor. Ular dasturning oʻz tugmalari yonida turadi. Ikki dasturda bunday tugma ramkali, ikkitasida ramkasiz koʻrinadi.

**Taklif.**
- Bitta mavzu boʻladi: kichik rang va oʻlcham toʻplami. Asosiy rang `brand/brand.toml`dan olinadi.
- Uni bitta umumiy crate (`crates/office-theme`) yasaydi. Har bir dasturga juda kichik patch kerak (60–90 qator): "oʻz ranglaring oʻrniga mavzu ranglarini ol".
- Shunda barcha oddiy tugmalar bir xil koʻrinishga oʻtadi.
- Chap panel (backstage) hamma dasturda bir xil, neytral rangda boʻladi. Tanlangan bandda brend rangidagi chiziq turadi.
- "Chop etish" sahifasi toʻrtta dasturda bir xil tartibda boʻladi (§7):
  - tepada katta "Chop etish" tugmasi va nusxalar soni;
  - keyin printer, sahifalar, yoʻnalish, qogʻoz, masshtab;
  - oʻngda oldindan koʻrish.
- "Boshqa formatda saqlash" tugmalari "Chop etish" sahifasidan "Eksport" sahifasiga koʻchadi.
- Web hajmi har bir dastur uchun taxminan 1–3 KB (brotli) oshadi. Bu hali oʻlchanmagan, bajarish bosqichida oʻlchanadi.

**Sizdan kerak boʻlgan qarorlar (§9, 7 ta savol).**
1. Sukut boʻyicha rejim: yorugʻ, qorongʻi yoki "tizim boʻyicha"? Tavsiya: tizim boʻyicha.
2. Barcha dasturlar uchun bitta brend koʻki (`#1E4FA3`), yoki har bir dasturga oʻz rangi? Tavsiya: bitta rang.
3. Chap panel: neytral (tavsiya) yoki rangli?
4. Shrift oʻlchamlarini hozir tenglashtiramizmi? Tavsiya: keyinroq.
5. Ikki rangli piktogrammalar: oʻz rangida qoladimi yoki brend rangini oladimi?
6. Qogʻozning sukut boʻyicha oʻlchami A4 boʻlsinmi? Tavsiya: ha.
7. PDF dasturidagi yumaloq ("pill") tugmalar oddiy tugmaga aylansinmi? Tavsiya: ha.

---

## 1. Audit: what the screenshots show

**How the screenshots were taken.**
- Tool: Playwright (MCP browser, 1280×800, locale `uz-Latn-UZ`) against `hoshiya-serve --dist E:/office-target/p1/dist` (release build of 2026-10-24).
- Dark mode: emulated `prefers-color-scheme: dark` for Docs, Sheets and the shell, and each app's own toggle for Slides and PDF.
- Documents: only synthetic ones.
  - Typed text ("Sinov hujjati…").
  - A generated one-page PDF ("Sinov PDF – dizayn auditi").
  - Upstream's own "Northern Lights" sample deck.

**What could not be captured.**
- **Sheets backstage on the web.**
  - The web build is always in simple-toolbar mode (`office_toolbar.rs:191`), so there is no File tab.
  - The Home button opens a "New" dialog instead.
  - Its look is taken from code (§2.2), not from a screenshot.
- **Desktop exes (`E:/office-target/p1/msi/stage`).**
  - I tried them. Screen capture of the wgpu window (PrintWindow, CopyFromScreen) returned stale or occluded frames.
  - I stopped rather than drive the owner's live desktop.
  - The egui code is the same on desktop and web, so the web pixels stand for the desktop.

### 1.1 Inconsistencies, ranked (most visible first)

| # | Finding | Evidence | Where in code |
|---|---|---|---|
| 1 | **Five different accent colours.** Docs `#3B5BDB` (indigo), Sheets `#107C41` (green), Slides `#D95A1A` (orange), PDF `#1B63E0` (blue), shell/brand `#1E4FA3`. The accent tints selected tabs, primary buttons, links, the selection, the backstage and the Present button. | `audit-editors.png`; `docs-editor-light.png`, `sheets-editor-light.png`, `slides-start-light.png`, `pdf-start-light.png`, `shell-home-light.png` | wordcraft `theme.rs:8`; gridcraft `theme.rs:56-95`; deckcraft `theme.rs:43-101`; pdfcraft `theme.rs:71-122`; `web/shell/shell.css:3` |
| 2 | **Docs backstage changes colour by page and ignores dark mode.** The nav rail is `APP_COLOR` blue on every page and turns **black** on Recover; the content pane is black on Recover too, with white text forced. In dark mode the rail stays bright blue. | `docs-backstage-recover-light.png`, `docs-backstage-print-light.png`, `docs-backstage-print-dark.png` | wordcraft `backstage.rs:45` (rail), `:153` (pane), `:176` (`override_text_color = WHITE`) |
| 3 | **Every app's backstage looks different.** Docs: 200 px solid accent rail, white text. Sheets: solid green rail in light mode and `accent_soft` in dark mode (code only). Slides: neutral `#F5F5F5` rail with an orange selected item. PDF: no backstage at all; it uses a menu plus dialogs. | `docs-backstage-*`, `slides-backstage-*`, `pdf-menu-light.png` | wordcraft `backstage.rs:41-93`; gridcraft `backstage.rs:157-158`; deckcraft `backstage.rs:246-248`, `:325-396`; pdfcraft `home.rs:21-200` |
| 4 | **Docs Print page is confusing.** Problem (a): Print and Export share one function (`export_page`), so the Print page ends with the 11-button "Or save a copy in another format" list. Problem (b): the second "all pages" dropdown is the odd/even filter `print_parity` ("All pages in the range"), which has no label. Problem (c): "Print" and "Save these pages as PDF…" are two equal outlined buttons with no primary. Problem (d): it is one loose 380 px column. Problem (e): three strings are untranslated ("Auto orientation", "Scale to paper size", "No Scaling"). | `docs-backstage-print-light.png` | wordcraft `backstage.rs:167`, `:498-519`; `print_page.rs:78-79` (RANGES, PARITIES), `:212`, `:237-239`, `:335` (`export_formats`) |
| 5 | **Print differs in every app.** The controls, the order and the primary action all differ. Docs: system dialog plus a long column. Sheets: printer block (craft_ui), a square Print tile, scope and fit only; range, orientation and scale live in Page Setup. Slides: square Print tile, no printer, no copies; the paper defaults to **Letter**. PDF: a modal dialog with grouped sections, a pill primary button and a preview. It is the best of the four, but it also defaults to **US Letter**. | `audit-print-recover.png`; `slides-backstage-print-light.png`, `pdf-dialog-print-light.png` | gridcraft `backstage.rs:691-781`; deckcraft `backstage.rs:1031-1137`, `:1097`; pdfcraft `print_ui.rs:797-1236` |
| 6 | **Default egui buttons sit next to styled ones, and look different per app.** Plain `ui.button(` counts are about 128 in Docs, 99 in Sheets, 187 in Slides and 94 in PDF, plus many `egui::Button::new`. In Docs and Sheets the inactive egui widget has a grey outline (`input_border`). In Slides and PDF it has no stroke, so plain buttons look like loose text. Examples: Docs Save As "Tanlash…", Info "Parol bilan shifrlash…" and the Recover buttons; the Sheets New dialog; the Slides "Recover Unsaved Presentations". | `docs-backstage-saveas-light.png`, `docs-backstage-info-light.png`, `sheets-dialog-new-light.png` | wordcraft `theme.rs:740-773`; gridcraft `theme.rs:709-720`; deckcraft `theme.rs:115-157`; pdfcraft `theme.rs:431-470` |
| 7 | **Three button systems per app.** Each app has: (1) custom-painted ribbon/toolbar buttons; (2) its own "primary" helper (Docs `widgets::primary_button`, PDF `widgets::pill_button` with radius 14, Slides an accent `egui::Button` with min size 84×26, Sheets a `square_button` tile); (3) plain egui buttons. So a primary button looks different in every app. | `docs-dialog-font-light.png` (accent OK), `slides-dialog-preferences-light.png` (orange "Done"), `pdf-dialog-print-light.png` (pill) | wordcraft `widgets.rs:1190`; pdfcraft `widgets.rs:132`; deckcraft `backstage.rs:951`; gridcraft `backstage.rs:521` |
| 8 | **Dark mode is chosen four ways.** Docs and Sheets follow the OS by default. Slides has its own `Brightness` toggle, default Light, and does not follow the OS. PDF defaults to Light, with a System option on a title-bar button. The shell follows `prefers-color-scheme`. Nothing is shared between the apps. | `*-dark.png` | wordcraft `theme.rs:14-80`; gridcraft `lib.rs:137-145`, `:1106-1122`; deckcraft `theme.rs:6-10`, `lib.rs:323`; pdfcraft `theme.rs:9-39`, `lib.rs:2057-2079` |
| 9 | **Dialog chrome differs.** Docs, Sheets and Slides dialogs have a grey title bar with a centred title and an × button. PDF dialogs have a left-aligned semibold title and no bar. Window radius is 8 in Docs and 10 elsewhere. | `docs-dialog-font-light.png`, `sheets-dialog-new-light.png`, `pdf-dialog-print-light.png` | wordcraft `theme.rs:740-773`; pdfcraft `widgets.rs:16-62` |
| 10 | **Rounding and spacing differ.** Widget radius: Docs 4, Sheets 5, Slides 5, PDF 6 (4 in dialogs), pills 14. `button_padding`: 8×3, 6×3, 6×3 and 10×5. Body text: Docs 12.5, the others 13. Heading: 17, 18, 18 and 17. | code | wordcraft `theme.rs:711-722`; gridcraft `theme.rs:728-737`; deckcraft `theme.rs:145-155`; pdfcraft `theme.rs:471-484` |
| 11 | **Contrast failures today.** Slides links (`#D95A1A` on white, 3.87:1, needs 4.5) and the field borders in Slides and PDF (1.45:1 and 1.39:1, need 3:1 for a field boundary, WCAG 1.4.11). | §5.3 table | deckcraft `theme.rs:43-101`; pdfcraft `theme.rs:71-122` |
| 12 | **Icon styles differ.** Docs: vector painter on a 20 px grid, 1.25 stroke, two colours (line plus accent). Sheets and Slides simple toolbars: Fluent alpha masks, tinted (our patches). Sheets and Slides full ribbon: painter icons with fixed multi-colour constants. PDF: Lucide SVGs, tinted. | screenshots | wordcraft `icons.rs:75-79`; gridcraft `icons.rs:188-193`; deckcraft `icons.rs:16-28`; pdfcraft `icon_data.rs`, `icons.rs:28-37` |

Found during the audit but not about the theme. Each is listed so it gets its own fix and does not hide inside the theme work:

- **Upstream names leak.**
  - PDF home says "PdfCraftga xush kelibsiz" and "PdfCraftda ochgan fayllaringiz" (`pdf-start-light.png`). `rebrand()` evidently misses an agglutinated form.
  - The default user name in Docs Options is "WordCraft User" (`docs-backstage-options-light.png`).
- **Untranslated UI on the web.**
  - Slides on the web is entirely in English with the browser locale set to `uz-Latn-UZ`.
  - In Sheets: "AutoSave", "Book1", "Sheet1", the "…" menu and the New dialog.
  - In Docs: the Recover page and the three Print strings above.
- **The shell's language is not passed to the web editors.** With the shell set to uz-Latn and the browser locale ru, Docs opened in Russian.
- **Truncated Uzbek labels.** The Docs nav rail is 200 px and cuts off "Boshqa nomda saqlash".
- **Odd text layout.** The Docs Info page shows a justified paragraph with stretched word gaps.

---

## 2. Where each app defines its look

Each app already has **one theme module with a `Tokens` struct**. Custom widgets read it every frame through `Tokens::get(ctx)`, which picks light or dark from `visuals.dark_mode`. One `apply`/`visuals` function then builds egui `Visuals` and `Style` from those tokens. That shared shape is what makes small hooks possible.

### 2.1 Docs (wordcraft)

**Tokens.**
- `Tokens` struct: `theme.rs:233-298`.
- `light()` at `:299` and `dark()` at `:362`, with about 60 colours, including author, leader and ruler colours.
- `Tokens::get` at `:428`.

**Theme entry.** `theme::apply` (`:708`):
- installs both palettes with `set_visuals_of`;
- sets `all_styles_mut` (`:711-722`): item_spacing 6×4, button_padding 8×3, interact 24×22, Body/Button 12.5, Small 10.5, Heading semibold 17.

**Visuals.** `visuals(&Tokens)` (`:740-773`):
- radius: window 8, menu 6, widget 4;
- inactive widget fill = `input`;
- window shadow (0,8) blur 28.

**Accent.** `APP_COLOR #3B5BDB` (`:8`), used directly by the backstage rail (`backstage.rs:45`).

**Fonts.** Inter plus CJK fallback (`:435-584`). Type rungs are Chrome 12.5, Control 11.5 and Caption 11 (`:625-670`).

**Existing guard.** The test `interface_colours_come_from_the_theme` (`theme.rs:919`) bans a few literals outside `theme.rs`. We extend this guard (see §6).

### 2.2 Sheets (gridcraft)

**Tokens.**
- About 38 fields (`theme.rs:12-53`).
- `light()` at `:56`, `dark()` at `:98`, `get` at `:140`, read in 48 places.

**Theme entry.** `theme::apply(ctx, dark)` (`:703-738`):
- called once from `lib.rs:367-370`;
- radius: window 10, menu 8;
- spacing 6×4 and 6×3, interact 22;
- text sizes: Body 13, Small 11, Heading 18 bold, Mono 12.

**Grid.** The grid stays white in dark mode on purpose (`grid_bg` and gridline are the same in both palettes).

**Backstage rail.** `backstage.rs:157-158`:
- light: `t.accent` with white text;
- dark: `t.accent_soft`.

**Icons.**
- Our simple toolbar (`office_toolbar.rs`, patch 0140) already reads `Tokens` everywhere, and its Fluent masks are tinted.
- The full ribbon's painter icons have fixed colours (`icons.rs:188-193`).

### 2.3 Slides (deckcraft)

**Tokens.**
- 23 fields (`theme.rs:14-40`), with `light()` and `dark()` at `:48` and `:76`.
- The accent is orange `#D95A1A`, with `#F26B1D` in dark mode.
- `get` re-derives the palette from `dark_mode` (`:110`).

**Theme entry.** `theme::apply(ctx, &Tokens)` (`:115-157`):
- `override_text_color = text` (`:128`);
- radius 10, 8 and 5;
- spacing 6×4 and 6×3, interact 22;
- text: Body 13, Heading 18.

**Re-styling.** `apply` runs only when `restyle` is set (`lib.rs:1617-1619`).

**Backstage page panel.** `backstage.rs:389-396` adds its own widget override: 1 px border and accent hover.

**Icons.** Painter icons with fixed accent constants (`icons.rs:16-28`).

**Fonts.** Inter plus BIZ UDPGothic (`theme.rs:160-230`).

**Caret.** Black in both modes, so it is invisible on a dark text field. This is a bug.

### 2.4 PDF (pdfcraft)

**Tokens.**
- About 20 fields (`theme.rs:42-68`) via `Tokens::for_kind` (`:71-122`).
- `apply` stores them in `ctx.data` (`:430`), and `get` is at `:124`.

**Theme entry.** `theme::apply(ctx, kind)` (`:424-488`):
- radius: window 10, menu 8, widget 6;
- spacing 8×6 and 10×5;
- text: Body 13, Small 11, Heading 17;
- the same style is copied to both themes (`:485-487`).

**Local dialog override.** `widgets::begin_dialog` (`:48-62`): radius 4 and a 30 px field height.

**Pills.** `pill_button` radius 14 (`widgets.rs:132`).

**Icons.** Lucide SVGs, tinted by the caller.

### 2.5 Our layer

**Office overlay.**
- `office_overlay.rs` is our file, carried in each app's `0100-*-office-overlay-hook.patch`.
- It `include!`s `$OUT_DIR/office_overlay_gen.rs`, rendered from `overlay.env`, which `cargo xt overlay` writes (`crates/office-brand/src/lib.rs:391-445`).
- `overlay.env` **already carries `ACCENT_LIGHT`, `ACCENT_DARK` and `ACCENT_MARGIN`**, but no app reads them.
- None of the four overlays has a colour, Style or Visuals hook today.

**Brand colours.** `crates/office-brand` holds them (`brand.toml [colors]`) and provides `contrast()` (`lib.rs:104-108`). `validate()` already checks three pairs (`:353-369`).

**No colours elsewhere.**
- `domains/*` (docs-office and the app crates) contain no `Color32`, `Visuals` or `style_mut`.
- In our patches, colour code appears only in the toolbar patches (gridcraft/deckcraft `0140`), and those read `Tokens`.

**Shell.**
- `web/shell/shell.css:2-8` has its own CSS variables: accent `#1E4FA3` / `#6C9BF0`, which match `brand.toml`.
- The desktop host (`services/suite-desktop`) is WebView2 showing the same page.

**Shared settings.** `crates/suite-core/src/settings.rs` holds `lang`, `toolbar`, etc. (`:95-104`), with no `theme` key. Patch 0145/0126 (shared language at start) is the pattern to copy.

---

## 3. Options for the mechanism

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Shared crate `crates/office-theme`, plus one tiny hook per app** (recommended) | A new crate in our layer. It turns `brand.toml [colors]` into a `Palette` (light and dark), a `Metrics` set (radii, spacing) and a `style(&mut egui::Style, dark)` helper. Each app's `office_overlay.rs` maps `Palette` onto that app's own `Tokens` struct. `theme.rs` gets one call at the end of `Tokens::light()`/`dark()` and one at the end of `apply`. | One source of truth. Hooks of about 10 lines in upstream files; the mapping lives in our own `office_overlay.rs`. With the overlay off, the apps keep the upstream look, so the pristine size baselines stay valid. The contrast checks run as a unit test of one crate. | Adds a path dependency to each `ui-egui` crate. This is already done for `suite-core` (deckcraft 0140, 0143). |
| B. Compile-time constants through `overlay.env` | Add `THEME_*` keys to `overlay.env`, generated into each app's `office_overlay_gen.rs`. | No new crate dependency. | The token mapping is written four times, inside upstream `build.rs` patches. Contrast logic is duplicated. Neither the web shell CSS nor tests can share it. |
| C. Replace each app's `theme.rs` | A carried patch rewrites the whole theme module. | Full control. | Breaks the rule "do not rewrite the upstream engines": 300–1000-line patches that conflict on every sync. Rejected. |

**Why A wins.**
- The four apps already have the same shape: a `Tokens` struct read every frame, and one function that builds Visuals from it.
- So a single injection point per app re-skins nearly everything, including the hundreds of plain `ui.button` calls, because egui draws them from `Visuals`.

**The crate can depend on egui.**
- All four upstream lockfiles are on **egui 0.36.2**, so `office-theme` can depend on `egui = "0.36"` without a second copy.
- It needs only `egui`'s types (`Color32`, `Style`, `Visuals`).
- `office-brand` is used as a **build-dependency** only, to read and validate `brand.toml`. So `toml_edit` never reaches a wasm.

---

## 4. The mechanism (option A) in detail

```
brand/brand.toml [colors] ──(build.rs, office-brand as build-dep)──▶ crates/office-theme
                                                                       ├─ Palette::LIGHT / ::DARK   (const, generated)
                                                                       ├─ Metrics                   (const)
                                                                       ├─ fn style(&mut egui::Style, dark: bool)
                                                                       └─ tests: contrast table (§5.3), gen-check of theme.css
cargo xt theme css ─▶ web/shell/theme.css (CSS variables, generated, checked by `ci.sh gen-check`)

suite-core settings.toml:  theme = "system" | "light" | "dark"   (new key, default "system")

app (each of the four):
  office_overlay.rs   fn tokens_light(t: &mut Tokens) / tokens_dark(..)   ← maps Palette onto the app's Tokens (our file)
                      fn style(s: &mut egui::Style, dark)                 ← office_theme::style, only when ACTIVE
                      fn theme_pref() -> Option<Pref>                     ← suite-core `theme`, read at start
  theme.rs            Tokens::light(): `let mut t = …; office_overlay::tokens_light(&mut t); t`   (≈3 lines)
                      apply(): after the upstream style code, `office_overlay::style(s, dark)`        (≈3 lines)
  backstage.rs        literal colours → tokens (Docs: APP_COLOR, BLACK; Sheets: rail line)          (≈5–20 lines)
```

**When the hooks act.** Only when `office_overlay::ACTIVE` is true; otherwise they do nothing. Builds with `OFFICE_NO_OVERLAY=1`, used for the pristine size baselines, are unchanged.

**What `style()` sets** (§5.2):
- `visuals.widgets.{inactive,hovered,active,open}`: `weak_bg_fill`, `bg_fill`, `bg_stroke`, `corner_radius`.
- `selection`, `window_*`, `menu_corner_radius`, `popup_shadow`.
- `spacing.button_padding`, `spacing.interact_size.y`.

It deliberately does **not** set text sizes in step 1 (see §9 Q4).

**The global theme preference.**
- Every app's existing preference (Docs `Appearance`, Sheets `theme_preference`, Slides `Brightness`, PDF `ThemePreference`) is seeded at start from `settings.toml theme`.
- A change in an app's own Options is written back. This is the 0145 "shared language" pattern, and Docs/Sheets/Slides already do it for `toolbar`.
- On the web, apps follow `prefers-color-scheme` unless the shell passes `?theme=`. The bridge change is listed in §8, step 2.
- The default becomes **System** in all four apps. Today Slides and PDF default to Light (see §9 Q1).

---

## 5. The token set

### 5.1 Colours

The accent comes from `brand.toml` (`accent_light`, `accent_dark`). `ink` is the brand's text colour. The neutrals are fixed in `office-theme`, cool grey to sit with the brand blue.

| Token | Light | Dark | Used for |
|---|---|---|---|
| `accent` | `#1E4FA3` (brand `accent_light`) | `#6C9BF0` (brand `accent_dark`) | Primary button fill, selected tab underline, links, focus ring, selection stroke |
| `on_accent` | `#FFFFFF` (brand `icon_fg`) | `#101828` (brand `ink`) | Text and icons on `accent` |
| `accent_soft` | `#E3EBF8` | `#22375E` | Selected item fill (nav rail, list, toggled tool), text selection wash |
| `accent_hover` / `accent_pressed` | `#1A4590` / `#163B7C` | `#83ABF3` / `#9ABAF5` | Primary button states |
| `chrome` | `#F3F4F6` | `#1B1D22` | Title bar, tab strip, status bar, toolbar row background |
| `surface` | `#FFFFFF` | `#24272D` | Ribbon/toolbar card, backstage content, dialogs, menus, cards |
| `rail` | `#EDEFF3` | `#16181C` | Backstage nav rail (one colour on every page) |
| `desk` | `#E4E7EC` | `#121418` | Behind pages/slides/sheets (canvas, pasteboard) |
| `field` | `#FFFFFF` | `#1B1D22` | Text fields, combo boxes, spinners |
| `field_border` | `#8A94A6` | `#6B7380` | Field and secondary-button outline (≥3:1) |
| `divider` | `#E4E7EC` | `#343840` | Separators, group dividers, card outline (decorative) |
| `hover` / `pressed` | `#E9ECF1` / `#DDE2EA` | `#30343C` / `#3A3F48` | Frameless tool, menu item and nav item states |
| `text` | `#101828` (brand `ink`) | `#E8EAEE` | Body text |
| `text_muted` | `#4B5563` | `#A9B1BD` | Hints, group labels, status text |
| `text_disabled` | `#98A2B3` | `#6B7380` | Disabled controls (exempt from contrast) |
| `icon` | `#344054` | `#D0D5DD` | Monochrome icon tint |
| `danger` / `warning` / `success` | `#B42318` / `#B54708` / `#067647` | `#F97066` / `#FDB022` / `#47CD89` | Error text, warnings, OK states |
| `brand_mark` | `#E9A23B` (brand `accent_margin`) | same | Only the logo bar; never UI chrome |

These stay app-owned and are **not** themed: document content, the paper itself (always white), gridlines and cell colours, colour-picker swatches, author/markup colours, slide thumbnails, and preview mock-ups.

### 5.2 Shape, spacing, type, icons

| Token | Value | Today (Docs / Sheets / Slides / PDF) |
|---|---|---|
| `radius.control` | 4 | 4 / 5 / 5 / 6 |
| `radius.menu` | 6 | 6 / 8 / 8 / 8 |
| `radius.window` (dialogs, cards) | 8 | 8 / 10 / 10 / 10 |
| `radius.pill` | 4. PDF's 14-radius pills become ordinary buttons. | – / – / – / 14 |
| `space.item` | 6×4 | 6×4 / 6×4 / 6×4 / 8×6 |
| `space.button_padding` | 8×4 | 8×3 / 6×3 / 6×3 / 10×5 |
| `size.control_h` | 24 (toolbar), 28 (dialog and backstage fields) | 22 / 22 / 22 / 30 in dialogs |
| `focus` | 2 px `accent` stroke | – |
| `shadow.popup` / `shadow.window` | (0,4) blur 16 / (0,8) blur 24 | similar, all different |
| Type scale | 11 caption · 13 body · 15 subtitle · 20 page title · 26 backstage title. **Phase 1 keeps each app's sizes** (§9 Q4). | 12.5 / 13 / 13 / 13 body |
| UI font | Inter where the app embeds it (Docs, Slides, PDF), else the app's UI family. Noto Sans Uz fallback (`brand.toml [fonts]`), already appended by every overlay. | |
| Icon style | Monochrome line icons tinted `icon` (disabled: `text_disabled`). One accent spot colour allowed (`accent`) for "active" glyphs. Fixed multi-colour painter icons are left alone for now (§9 Q5). | |

**Buttons.** There are three kinds, and `style()` makes egui's default the "secondary" one, so all plain `ui.button` calls in all four apps become the same visible button:

| Kind | Fill | Border | Text | Where |
|---|---|---|---|---|
| **Primary** | `accent` → `accent_hover` → `accent_pressed` | none | `on_accent` | The one main action per page or dialog: Print, Save, OK, Export, Present. The app helpers (`primary_button`, `pill_button(primary)`, Slides accent Button, Sheets `square_button`) map here through tokens. |
| **Secondary** (egui default) | `surface`; hover `hover`; pressed `pressed` | 1 px `field_border` | `text` | Every other button, including Cancel, Browse… and Properties… |
| **Quiet** | transparent; hover `hover` | none | `text` | Toolbar and ribbon tools, menu items, nav rail items (already custom-painted, so they only need tokens) |

**egui caveat.** egui has no separate keyboard-focus visuals: a focused widget is drawn with the `hovered` style.
- So the 2 px focus ring applies only where we can draw it: `selection.stroke` for text fields, and `hovered.bg_stroke` for framed widgets.
- A true distinct focus ring for every widget needs per-widget painting. That is out of scope here and is listed as a risk.

### 5.3 Contrast check (WCAG, `office_brand::contrast`)

**How it was checked.** By a throwaway binary linked to `crates/office-brand` (`E:/office-target/design-audit/contrast`). In the implementation, this table becomes the unit test of `office-theme`.

**Result.** All proposed pairs pass. Three pairs in today's apps fail.

| Mode | Foreground | Background | Ratio | Need |
|---|---|---|---|---|
| L | text `#101828` | surface `#FFFFFF` / chrome `#F3F4F6` / rail `#EDEFF3` | 17.75 / 16.13 / 15.42 | 4.5 |
| L | text_muted `#4B5563` | surface / chrome / rail | 7.56 / 6.87 / 6.56 | 4.5 |
| L | accent `#1E4FA3` (link) | surface / chrome / accent_soft `#E3EBF8` | 7.77 / 7.06 / 6.48 | 4.5 |
| L | on_accent `#FFFFFF` | accent | 7.77 | 4.5 |
| L | text | accent_soft / hover `#E9ECF1` | 14.79 / 14.99 | 4.5 |
| L | field_border `#8A94A6` | surface | 3.06 | 3 |
| L | focus (accent) | surface / chrome | 7.77 / 7.06 | 3 |
| L | danger `#B42318` | surface | 6.57 | 4.5 |
| L | icon `#344054` | chrome | 9.51 | 3 |
| D | text `#E8EAEE` | surface `#24272D` / chrome `#1B1D22` / rail `#16181C` | 12.43 / 14.00 / 14.76 | 4.5 |
| D | text_muted `#A9B1BD` | surface / chrome | 6.92 / 7.80 | 4.5 |
| D | accent `#6C9BF0` (link) | surface / chrome | 5.41 / 6.09 | 4.5 |
| D | on_accent `#101828` | accent | 6.41 | 4.5 |
| D | text | accent_soft `#22375E` / hover `#30343C` | 9.82 / 10.36 | 4.5 |
| D | field_border `#6B7380` | surface | 3.13 | 3 |
| D | focus (accent) | surface | 5.41 | 3 |
| D | danger `#F97066` | surface | 5.37 | 4.5 |
| D | icon `#D0D5DD` | chrome | 11.44 | 3 |
| today | Slides accent link `#D95A1A` | white | **3.87 (fail)** | 4.5 |
| today | Slides field border `#D6D6D6` | white | **1.45 (fail)** | 3 |
| today | PDF field border `#DADADE` | white | **1.39 (fail)** | 3 |

**Margin for other brand colours.** `field_border` light (3.06) has the least margin. If the owner changes the brand neutrals, this pair is the one to watch. The test fails CI below 3.0.

---

## 6. Per-app patch plan

**Numbering.** New carried patches follow each app's queue numbers. The `0150` series is "theme". The sizes are estimated diff lines (+/−) in upstream files. The mapping code in `office_overlay.rs` is our own file, but it is still carried inside the patch, so it is counted separately.

| App | Patch | Upstream files touched | Upstream lines | `office_overlay.rs` lines | Risk |
|---|---|---|---|---|---|
| all | `crates/office-theme` (new, ours) | – | – | ~250 + tests | Low |
| all | `suite-core` `theme` key | – | – | ~40 + tests | Low |
| Docs | `wordcraft/0150-theme-tokens-hook` | `theme.rs` (2 tails + 1 `style` call), `backstage.rs:45`, `:153`, `:176`, `:63-93`, `:139-141` (rail colours: white-alpha → `t.text`/`accent_soft`, no BLACK), `theme.rs:919` guard adds `Color32::BLACK` fills in backstage | ~35 | ~45 | Low. Recover loses its black look; the guard test is updated. |
| Docs | `wordcraft/0151-print-export-split` | `backstage.rs:157`, `:167`, `:498-519` (Print and Export are separate pages); `print_page.rs` (layout §7: header row, sections, parity labelled, "Save as PDF" moves into the printer list, `export_formats` leaves Print) | ~150–220 | – | Medium. `print_page.rs` is upstream-active, so expect rebase conflicts on sync. The rail widens to 220 px for Uzbek labels. |
| Sheets | `gridcraft/0150-theme-tokens-hook` | `theme.rs:56`, `:98` tails, `:728` style call; `backstage.rs:157-158` rail | ~15 | ~45 | Low. The grid stays white in dark mode (upstream choice, kept). |
| Sheets | `gridcraft/0151-print-layout` | `backstage.rs:691-781`: same sections; adds Pages (active sheets/workbook/selection), Orientation, Paper and Scaling rows that write the page-setup state the Page Setup dialog already edits | ~120–160 | – | Medium. The page-setup fields must round-trip to XLSX `pageSetup`; covered by the existing print tests plus one new test. |
| Slides | `deckcraft/0150-theme-tokens-hook` | `theme.rs:48`, `:76` tails, `:144-155` style call; `backstage.rs:389-396` (drop the local widget override, now global); `dialogs.rs:2686-2687` (orange literals); caret colour from `text` | ~25 | ~45 | Low. `Brightness` gains a System value (seeded from suite-core). |
| Slides | `deckcraft/0151-print-layout` | `backstage.rs:1031-1137`: header row (Print primary), sections, A4 default for uz/ru (`:1097`) | ~80–120 | – | Low to medium. |
| PDF | `pdfcraft/0150-theme-tokens-hook` | `theme.rs:71` tail, `:441-444` and `:471-484` literals → `style()`; `widgets.rs:132` pill radius → token; `widgets.rs:48-62` dialog override uses tokens | ~25 | ~45 | Low. The pills become 4-radius buttons, which changes the look most. |
| PDF | `pdfcraft/0151-print-align` | `print_ui.rs`: section order and names as §7; A4 default for uz/ru | ~30–60 | – | Low. It stays a dialog (PDF has no backstage). |
| Shell | `web/shell/theme.css` (generated) + `shell.css` uses the variables | – | – | ~30 | Low |

**Totals.**
- About 100 upstream lines for the four theme hooks, plus about 180 lines of mapping in our overlay files.
- About 400–550 upstream lines for the three Print redesigns plus the PDF alignment.
- The theme hooks alone fix findings 1, 2, 3 (colour part), 6, 7 (colour part), 8, 9 (radius), 10 (radius and spacing) and 11.

**Web size impact (estimate, to be measured).**
- `office-theme` is two const palettes (about 25 colours each), about 10 metrics and one `style()` function of about 60 lines. The mapping functions add a similar amount.
- Estimate: **+1 to +3 KB brotli per editor wasm**. There is no new dependency, because egui is already linked.
- The Print and Export split is mostly moved code. Docs loses one copy of the format list, so it is about ±1 KB.
- Current headroom is 19–27 KB per editor (STATUS, Phase 4 WP7), so this fits. The rule still applies: measure with `scripts/ci.sh sizes` after each patch, and only raise a `delta_br_max` with a STATUS line.

**Risks.**
1. **Sync conflicts.** `theme.rs`, `backstage.rs` and `print_page.rs` are actively developed upstream. The hooks are tail calls, so conflicts stay small. The Print redesigns are not small. Keep them as separate patches so a sync can drop or redo them alone.
2. **Hard-coded colours outside the tokens.**
   - Docs has 14 white/black-alpha uses in `backstage.rs`.
   - Slides has `from_rgb` literals in ribbon, panes and dialogs.
   - PDF has about 15 per file in canvas, sign and forms, most of them annotation colours.
   - Some chrome will still look off after step 1. The after-screenshots (§8 step 5) find them, and the extended guard test stops new ones in the files we touch.
3. **egui focus visuals** (§5.2): there is no distinct focus ring everywhere.
4. **Ribbon layout.** Changing `button_padding` or `interact_size` can move Docs' ribbon collapse thresholds (`ribbon_fit.rs`). Step 1 changes only colours and radii in the ribbon. Spacing changes apply to egui defaults only, and need a ribbon-fit check at 1280 and 1024 px.
5. **Shared preference races.** Two apps writing `theme` at once. suite-core's settings writer already handles this for `toolbar` and `lang`.
6. **Desktop not verified by screenshot here.** The web pixels stand for the desktop. The after-check must include the MSI build on a real Windows desktop.

---

## 7. Print page, one layout for all four apps

**Rules.**
- One primary action at the top.
- Settings in titled groups, always in the same order.
- One control per question, each with a label.
- Preview on the right.
- Nothing that is not printing.

"Save a copy in another format" moves to **Export** (and to **Save As** for the native formats). "Save as PDF" stays reachable as a printer choice, as PDF already does it (`print_ui.rs:841`).

```
┌─ rail (220) ─┐┌─ Chop etish ─────────────────────────────────────────────────────────────────────────┐
│ ←            ││                                                                                      │
│ Asosiy       ││  ┌────────────────┐  Nusxalar [ 1 ▴▾]                ┌──────────────────────────────┐ │
│ Yangi        ││  │ 🖶 Chop etish  │  (primary, accent)                │                              │ │
│ Ochish       ││  └────────────────┘                                   │                              │ │
│ Maʼlumot     ││                                                       │        page preview          │ │
│ Qayta tiklash││  PRINTER                                              │        (paper white,         │ │
│ Saqlash      ││  [ HP LaserJet (standart)                ▾] Xususiyatlar…  shadow, on `desk`)     │ │
│ Boshqa nomda ││   …list also has: Brauzer oynasi · PDF sifatida saqlash  │                              │ │
│  saqlash     ││   Holat: tayyor                                        │                              │ │
│▌Chop etish   ││                                                       │                              │ │
│ Eksport      ││  SAHIFALAR                                            │                              │ │
│ Yopish       ││  [ Barcha sahifalar                     ▾]            │                              │ │
│              ││  Diapazon: [ 1-3, 5            ]  (only for "Maxsus") └──────────────────────────────┘ │
│              ││  Toq/juft:  (•) Barchasi ( ) Toq ( ) Juft              ‹  Sahifa 1 / 3  ›     − ──○── + │
│              ││                                                                                      │
│              ││  SAHIFA                                                                              │
│              ││  Yoʻnalish: [ Kitob | Albom ]                                                        │
│              ││  Qogʻoz:    [ A4 (210 × 297 mm)                 ▾]                                    │
│              ││  Masshtab:  [ Asl oʻlcham                       ▾]   (Sheets: + "Varaqqa sigʻdirish") │
│              ││                                                                                      │
│              ││  ▸ Qoʻshimcha   (collapsed: app-specific rarely used options)                        │
│              ││                                                                                      │
│ Parametrlar  ││  3 ta sahifadan 3 tasi chop etiladi · Sahifa parametrlari…                            │
│ Dastur haqida││                                                                                      │
└──────────────┘└──────────────────────────────────────────────────────────────────────────────────────┘
```

**Group rows per app** (the same order everywhere; a row is hidden if the app has no such setting):

| Group | Docs | Sheets | Slides | PDF (dialog) |
|---|---|---|---|---|
| Header | Print (primary), Copies | Print, Copies (craft_ui printer block's copies move up) | Print, Copies (hidden on the web: it prints through a PDF) | Print / "PDF sifatida saqlash" (footer stays right-aligned, as dialogs do) |
| Printer | system printer list + "Brauzer oynasi" + "PDF sifatida saqlash"; Properties | craft_ui printer block (printer, two-sided, grayscale) | (web/PDF path: note text "PDF orqali chop etiladi") | as today |
| Pages | All / Current / Selection / Custom + range; Odd/even as labelled radios | Active sheets / Whole workbook / Selection | All / Current / Custom + range | All / Current / Pages / Selected + range; reverse under "Qoʻshimcha" |
| Page | Orientation, Paper, Scale ("Asl oʻlcham" / "Qogʻozga moslash") | Orientation, Paper, Scaling ("Asl" / "Varaqni bitta sahifaga" / "Ustunlarni bitta sahifaga") | Layout (Full slide / Notes / Handouts N), Orientation, Paper | Size tabs (Fit / Actual / Shrink / Custom), Orientation, Paper |
| More (collapsed) | Print markup | Gridlines, headings (from Page Setup) | Hidden slides, frame slides, header & footer… | Poster/booklet, print as image, comments & forms |
| Footer line | "N of M pages will print" · Page Setup… | Page Setup… | Header & Footer… | (scale note, as today) |

**Default paper.** A4 when the UI language is uz-* or ru. Slides (`backstage.rs:1097`) and PDF currently default to Letter.

**Export page** (Docs, Sheets and Slides; it replaces Docs' list of 11 big buttons):
- "PDF yaratish" is the first card, with a primary button.
- Under it, "Fayl turini oʻzgartirish" is one single-select list of formats (name plus extension, one line each).
- One "Saqlash…" button follows the list.

**Save As** keeps the native formats chooser only.

---

## 8. Implementation order (phase 2 of this work, after the owner's answers)

1. **`crates/office-theme` and the shell CSS.**
   - Build the `Palette` and `Metrics` consts from `brand.toml`, the `style()` helper and the contrast test (§5.3).
   - Add `cargo xt theme css`, which writes `web/shell/theme.css`, plus the gen-check.
   - The shell uses the generated variables.
   - Done when: `cargo test -p office-theme` passes; `ci.sh gen-check` passes; `shell.spec.ts` passes.
2. **suite-core `theme` key.**
   - The shell's Settings page gets a theme picker. The desktop host passes it on, and the web shell passes `?theme=` to the editor iframes.
   - The bridge patches are touched only for the query read. Coordinate with the A2 bridge work (`a11y-baseline.md`).
3. **Four `0150-*-theme-tokens-hook` patches.** These are independent, so run one builder agent per app in parallel.
   - Each runs `scripts/verify-queue.sh` for its app, the app's tests and `ci.sh sizes`.
4. **Docs backstage fixes** (rail colour, Recover, rail width 220) inside wordcraft 0150, then **`0151` Print and Export** in Docs, Sheets and Slides, and the PDF alignment.
5. **After-screenshots.** Use the same script and names as §1 under `E:/office-target/design-audit/after/`, and do a side-by-side review.
   - Also check on the MSI desktop build.
   - Run an a11y re-check: contrast spot checks and keyboard paths through Print.
6. **STATUS lines, `patches/REGISTER.md` entries, and the owner review** of the before/after pairs.

**Not in this work** (listed so it is not forgotten):
- one shared title bar and toolbar row layout across the apps;
- unified icon sets;
- the translation gaps and name leaks listed in §1.1.

---

## 9. Questions for the owner

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Default appearance for all apps and the shell | Light · Dark · **System** (follow Windows) | **System**. Docs, Sheets and the shell already do this, and the theme is one shared setting the user can change. |
| Q2 | Accent colour | **One brand blue `#1E4FA3` / `#6C9BF0` everywhere** · one colour per app (Docs blue, Sheets green, Slides orange, PDF red), with the same neutrals | **One colour**. That is what "one design language" means, and the app icons already tell the apps apart by pictogram. Per-app colours are a later one-line change in `brand.toml` if wanted. |
| Q3 | Backstage nav rail | **Neutral rail** (`rail` grey) with an accent selection bar · solid accent rail, as Docs and Sheets have today | **Neutral**. It works in dark mode, and it can never turn black or blue per page. |
| Q4 | Font sizes | Unify now (13 body, Docs goes from 12.5 to 13) · **later** | **Later.** A 0.5 pt change is barely visible but can move ribbon collapse points. Colours, buttons and the backstage carry the owner's complaint. |
| Q5 | Multi-colour icons in the Sheets and Slides full ribbons | keep their colours · recolour to `icon` and `accent` | **Keep for now.** The simple toolbars, which are the default, are already monochrome and tinted. |
| Q6 | Default paper size | **A4 for uz/ru**, Letter for en-US · keep Letter | **A4.** |
| Q7 | PDF's pill-shaped buttons | **become standard 4-radius buttons** · keep pills | Standard, for one button shape everywhere. |

**Decided (owner, 2026-10-29): every recommendation above is accepted** — Q1 System, Q2 one brand colour, Q3 neutral rail, Q4 font sizes later, Q5 keep the coloured icons, Q6 A4 for uz/ru, Q7 standard buttons.

**Revised (owner, 2026-10-29, later the same day): no single design language.** Each app keeps its own look (upstream
colours, rail, buttons, fonts). This replaces Q2, Q3 and Q7 above and the "one token set" of sections 4-5; options B/C of
section 3 stay rejected. What stays:
- **One global theme, changed in one place:** a shared `theme` setting (System · Light · Dark, default System, Q1) in
  suite-core, set from the shell's settings; every app applies it to its own light/dark palette, like the shared `lang`.
- **Our own additions follow the host app's design language:** anything our layer adds inside an app (version history,
  recovery, transliteration, XDFU, the simple toolbars, the web History panel) draws with that app's own `Tokens`,
  widgets and spacing, never with colours or shapes of its own.
- Q4 (font sizes later), Q5 (keep the coloured icons) and Q6 (A4 for uz/ru) stand. Print pages are made clear inside
  each app's own style (section 7's common layout is dropped).
- Default document font (owner, 2026-10-29): Times New Roman 12 pt for new documents in all four apps (Slides: the
  typeface only, its slide sizes stay).
