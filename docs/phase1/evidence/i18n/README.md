# i18n evidence (WP9c, WP9d, 2026-10-18)

Phase 1 gate item: the UI switches to uz-Latn and uz-Cyrl. All Uzbek text here is the **unreviewed draft** (`draft-unreviewed`); the language pickers say so
(`Oʻzbekcha (qoralama)`, `Ўзбекча (қоралама)`). Builds: `scripts/build.sh <d> desktop|web --release` (review-mode export, overlay on). Fallback chain at export:
uz-Cyrl to ru to en, uz-Latn to en, so Cyrillic screens show Russian words where no Uzbek row exists, and Latin screens show English.

## How each app was switched

| App | Web (Playwright, headless Chromium, `scripts/web-brand-evidence.mjs`) | Desktop (release exe) |
|---|---|---|
| wordcraft (docs) | browser language `uz-Latn-UZ` / `uz-Cyrl-UZ` (OS-locale path, `sys-locale` js); picker in File > Options > Language | `WORDCRAFT_LANGUAGE=uz-Latn-UZ`: window title `Suite Hujjatlar`, `uz-Cyrl-UZ`: `Suite Ҳужжатлар` (MainWindowTitle) |
| gridcraft (sheets) | browser language (`navigator.language`); picker on the View tab | `ui.json` `{"language":"uz-latn"}` in a scratch `%APPDATA%`; window title is the workbook name (upstream); About shows the Uzbek app name |
| pdfcraft (pdf) | the web build never follows the browser (upstream, finding I3); picked in Preferences > Interface language | `pdf.exe --language uz-cyrl`; About screenshot through the control channel |
| deckcraft (slides) | picked on File > Account (no OS-locale detection in Phase 1, design 2.5) | `ui.json` `{"language":"uz-latn"}` in a scratch `%APPDATA%`; ribbon and Account page through the control channel |

The real settings under `%APPDATA%` were not touched (scratch `APPDATA` for gridcraft and deckcraft; pdfcraft wrote no settings file).

## Files

`<app>-web-*` and `<app>-desktop-*`: screenshots. `test-<app>-overlay-on.log`: the ui-egui i18n tests with the overlay on (`scripts/overlay-tests.sh <d> ...`).
Quarantine entries for what fails are in `ci/quarantine.toml` (class `brand-overlay` or `i18n-incomplete`).

## Findings (looked at every screenshot)

No tofu in any Uzbek text (U+02BB, U+02BC, ўқғҳ draw) in all four apps, except upstream's own CJK language names in gridcraft's web picker (CJK fonts are not bundled on web).
Labels inside buttons did not clip in the screens sampled (11 wordcraft tabs in uz-Latn, 5 in uz-Cyrl, the sheets and slides ribbons). The 386 long-label lint
warnings were not checked one by one. Clipping that was seen:

1. **Group overflow at the right edge** (wordcraft web 1100 px, both scripts; gridcraft desktop uz-Cyrl 1456 px): the ribbon scrolls, wider labels push whole groups out of view
   (`Sahifa e...`, `Расмлар рўйхатини кири...`, gridcraft's Find group). Same mechanism as English at a narrower width.
2. **wordcraft web uz-Cyrl, tab row at 1100 px**: `Ишлаб чиқувчи` and `Ёрдам` fold into the `»` overflow.
3. **Backstage side menu**: `Бошқа ном билан сақлаш` (wordcraft uz-Cyrl) and `Boshqa nom bilan saqlash` (deckcraft web uz-Latn, `Boshqa nom bila`) are cut at the menu width.
4. **gridcraft web View tab at 1100 px**: the `Interfeys tili` combobox sits at the ribbon edge, so the draft marker is cut off.
5. **deckcraft uz-Cyrl** (desktop and web): the interface face had no Inter at our craft-fonts pin (risk F-R2), so Cyrillic came from BIZ UDPGothic and was drawn with CJK full-width
   advances (absurd letter spacing: the tab row ran past the window, `Ёзиб о...`, the Account side menu was cut, `Параметрл`). **Fixed 2026-10-18 by deckcraft patch 0112**: our Noto Sans
   subset now comes before the CJK face in the proportional and bold families (CJK text still falls through to BIZ). Evidence: `slides-desktop-ribbon-uz-cyrl-before-fix.png` (before),
   `slides-desktop-ribbon-uz-cyrl.png`, `slides-desktop-ribbon-uz-latn.png`, `slides-web-fixed-{cyrl,latn}-{account,deck}.png` (after, release builds): normal spacing, all ribbon tabs fit, no tofu.
   What remains there is plain width clipping (`Boshqa nom bilan ...` in the Account side menu, the ribbon scrolling at the right edge), the same mechanism as findings 1 and 3.
6. wordcraft uz-Cyrl style tile `Без интерв...` is elided by the tile.

## Gaps seen (draft coverage, not hooks)

- Strings that are not in the P1 draft set stay English (uz-Latn) or Russian (uz-Cyrl): wordcraft `Recover`, `About`, `Good evening`, `Blank document`, `Read Mode`, `Print Layout`;
  deckcraft `Layout`, `Section`, `Present`, `Shape Fill`; the new picker heading `Interface Language`.
- deckcraft translates only what reaches a wrapped helper (0111).
- Tests: `every_tab_and_command_is_translated` (wordcraft, `Controls`) and three pdfcraft tests need rows that the P1 set lacks (keyboard key names, the GoToR confirmation,
  object selection labels). `cargo xt i18n roundtrip` fails for pdfcraft because the catalog was extracted at the old BASE: upstream ru now has two more rows
  (`You are viewing revision ...`); `cargo xt i18n extract` would add them (a trial run changed about 2,300 TSV lines for 17 new rows, so it was reverted and left to the catalog owner).
