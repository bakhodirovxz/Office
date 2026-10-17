# WP8 design: fallback UI and document font (Latin, U+02BB, U+02BC, Cyrillic incl. ўқғҳ)

| | |
|---|---|
| **Status** | Design, measured. Written 2026-10-17 on branch `phase-1`. No builds, no edits under `upstream/` or `patches/`. |
| **Test text** | `Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №` (PLAN WP8) |
| **Evidence** | Candidate fonts downloaded with `curl --ssl-no-revoke` to `E:/office-tools/fonts-eval/<family>/` (one directory per download). Coverage was read from each font's `cmap` with fontTools 4.61.1 in `E:/office-tools/venv-bench` (`python -I`). The full report is `E:/office-tools/fonts-eval/coverage-report.txt`; the script is copied next to it (`fontcov.py.txt`, reproduced in §6). |
| **Note** | `venv-bench` had no fontTools. `fonttools==4.61.1` was installed into it with pip, the same version as the system Python's (PLAN F11). |

## 1. Decision

**Noto Sans Regular** and **Noto Sans Mono Regular** (notofonts, v2.015 / v2.014, **unhinted** TTF, SIL OFL 1.1, **no Reserved Font Name**), subset to the "uz" range set in §3. Each app appends them **last** in every interface family. Bold is not shipped in Phase 1.

Why this one (options considered, all OFL):

| Candidate | Required coverage (§2) | Notes | Verdict |
|---|---|---|---|
| **Noto Sans 2.015 + Noto Sans Mono 2.014** | **PASS** (both; full Cyrillic block 256/256, Karakalpak Ǵǵ, Kazakh Әә Ңң Өө Үү) | A matching sans + mono pair. No RFN (`OFL.txt` header carries only the copyright line), so a subset may keep its name. Not Adobe-authored (deckcraft's asset policy excludes Adobe faces, `deckcraft/crates/fonts/build.rs:10`). | **chosen** |
| Fira Sans 4.203 | PASS | No equally complete mono partner checked; 16 of 256 Cyrillic block letters missing. | runner-up |
| Source Sans 3 3.052 | PASS | Already in wordcraft (`assets/fonts/SourceSans3-*`). **Adobe-authored**, so it breaks deckcraft's policy; Source Code Pro would be a second family. | rejected for a suite-wide fallback |
| PT Sans 2.003W | **FAIL: no U+02BB** | Declares Reserved Font Names "PT Sans" and "ParaType". | rejected |

Source: `https://raw.githubusercontent.com/notofonts/notofonts.github.io/main/fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf` and `…/NotoSansMono/unhinted/ttf/NotoSansMono-Regular.ttf`. That repo's `main` was at `5c3913063bb189df4edc94b766584dfb4c04a187` (2026-10-14). SHA-256 of the downloaded files: `NotoSans-Regular.ttf` f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8 (431,364 B); `NotoSansMono-Regular.ttf` 87f8ce0522a6c99b743ee5fc75b4073cfdd575639119672828b7b9944b65b4f4 (388,660 B). Licence text: `https://raw.githubusercontent.com/notofonts/latin-greek-cyrillic/main/OFL.txt`. WP8 pins the URL to the commit, not to `main`, and records all of this in `assets/fonts/fallback/SOURCE.md`.

Unhinted rather than hinted: egui rasterises with its own scaler and does not apply TrueType hinting, so hinting bytes are dead weight. The hinted Regular is 621,572 B; its subset is 74,784 B against 50,460 B unhinted (NARROW set).

## 2. What the current faces lack (measured from `cmap`)

Required set: ASCII; Latin-1 letters and punctuation (U+00AD soft hyphen excluded: it is invisible); **U+02BB** ʻ; **U+02BC** ʼ; Russian А–я, Ёё; **Uzbek Cyrillic Ўў Ққ Ғғ Ҳҳ**; – — ‘ ’ “ ” „ … • « »; ₽ № € ™.

| Face (where used) | Missing from the required set | Also missing (optional) |
|---|---|---|
| **Ubuntu Light 0.83** (egui default proportional; gridcraft web UI and PDF default face, deckcraft) | **U+02BB ʻ, U+20BD ₽** (it has U+02BC and all of ўқғҳ) | 42 of 256 Cyrillic-block letters; combining U+0300/U+0301 |
| **Hack 3.003** (egui default monospace) | **U+02BB ʻ, U+02BC ʼ, U+20BD ₽** | Karakalpak Ǵǵ; 76 Cyrillic-block letters |
| **Inter 4.001** Regular / Medium / SemiBold (wordcraft and pdfcraft UI) | **none** (it has U+02BB, U+02BC, ўқғҳ, ₽, №) | U+0149; historic Cyrillic U+047A–047F, U+049E–049F; U+00AD |
| **JetBrains Mono 2.305** (wordcraft and pdfcraft monospace) | **U+02BB ʻ, U+04B2 Ҳ, U+04B3 ҳ** | 134 Cyrillic-block letters; U+00AD |
| BIZ UDPGothic 1.051 (deckcraft UI first face when built with craft-fonts) | **U+02BB, U+02BC, Ққ Ғғ Ҳҳ, ₽** | — |
| Dancing Script 2.001 (pdfcraft typed signatures) | **all Cyrillic, U+02BB** | — |
| Source Sans 3 / Source Serif 4 (wordcraft document faces) | none | — |
| Segoe UI 5.62, Calibri 6.23, Consolas 7.00 (Windows system; gridcraft desktop picks system UI fonts) | none | — |
| **Noto Sans 2.015** Regular/Bold, **Noto Sans Mono 2.014** | **none** (full and subset) | none in the checked sets |

**Correction to PLAN WP8 risk 1**: Inter (as bundled, 4.001) does **not** lack Uzbek Cyrillic or U+02BB. The real gaps are the egui defaults (Ubuntu Light, Hack), JetBrains Mono (ʻ and Ҳҳ) and BIZ UDPGothic.

Effective gap per app (walking each family's chain at the old BASE):

| App | Proportional chain → missing | Monospace chain → missing | Documents |
|---|---|---|---|
| wordcraft | Inter, Ubuntu, emoji, SourceSans (`theme.rs:478-485`) → **nothing** | JetBrains, Hack, Ubuntu (`:482,487-488`) → **ʻ** | SourceSans/SourceSerif embedded (`crates/fonts/src/fontdb.rs:28-40`) → nothing |
| gridcraft | desktop: system UI face (Segoe UI) → nothing; **web / bare Linux: Ubuntu → ʻ ₽** (`theme.rs:388-447`; no system faces on wasm `:477-480`) | Hack, Ubuntu → **ʻ ₽** | **PDF default face = Ubuntu Light** (`theme.rs:390-394`) → **ʻ ₽ print as notdef and are lost in extraction** (Phase 0 finding) |
| deckcraft | with craft-fonts: BIZ UDPGothic, Noto Sans Arabic, Ubuntu (`theme.rs:169-193`) → **ʻ ₽**, and **қғҳ come from Ubuntu inside a BIZ word** (mixed faces); without craft-fonts: Ubuntu → ʻ ₽ | Hack, Ubuntu → ʻ ₽ | slides render through `deckcraft_fonts::FontDb`; on web only `CRAFT_FONTS` are embedded (`crates/fonts/build.rs:1-6`; craft-fonts 3d9ab3a has **no** `latin-manifest.txt`, so no Inter) → **ʻ missing on web** |
| pdfcraft | Inter, egui defaults, CJK… (`theme.rs:363-369`) → nothing | JetBrains, Hack, Ubuntu → **ʻ** | page text uses the PDF's own fonts; added text uses Inter (`crates/fonts/src/script.rs:11`) → nothing; typed signatures in Dancing Script fall back to Inter for Cyrillic (`theme.rs:363` comment) |

**Surprise**: deckcraft gets no Inter at all at our pinned craft-fonts (`3d9ab3a` has only `fonts/manifest.txt`). PLAN F9's "wasm gets Inter Regular/Bold" applies only to a craft-fonts with the Latin manifest. deckcraft's UI is therefore BIZ UDPGothic (a Japanese face) for Latin and Cyrillic, and Uzbek Cyrillic words mix two faces. This is upstream's state, not ours to fix in Phase 1. The evidence screenshots must show it, and it is recorded as risk F-R2.

## 3. The subset

Range set "uz/MEDIUM" (Latin, Latin-1, Latin Extended-A/B, the modifier letters ʹ ʺ ʻ ʼ, the accents used in names, the full Cyrillic block, general punctuation, currency):

```
U+0020-007E,U+00A0-024F,U+02B9-02BC,U+02C6-02C7,U+02D8-02DD,U+0300-0304,U+0306-0308,U+030C,
U+0400-04FF,U+2010-2027,U+2030-203A,U+20AC,U+20B8,U+20BD,U+2116,U+2122,U+2212,U+FFFD
```

Command (WP8 commits it as `scripts/subset-fallback-font.sh`):
```
python -I -m fontTools.subset <in.ttf> --unicodes="<ranges>" --layout-features='kern,liga,calt,ccmp,locl,mark,mkmk' \
  --name-IDs='*' --notdef-outline --output-file=<out.ttf>
```

Measured sizes (bytes; gzip -9; brotli was not measured because the `brotli` module is not installed):

| Set | Noto Sans Regular | Noto Sans Mono Regular | Regular + Mono | (Noto Sans Bold, not shipped) |
|---|---|---|---|---|
| full unhinted | 431,364 | 388,660 | 820,024 | 432,376 |
| BROAD (WP8 plan ranges, + U+2000-206F, arrows) | 122,004 / 64,670 gz | 99,568 / 54,926 gz | 221,572 / 119,596 gz | 123,032 |
| **MEDIUM (chosen)** | **83,684 / 43,777 gz** | **64,632 / 36,805 gz** | **148,316 / 80,582 gz** | 84,044 |
| NARROW (Uzbek + Russian only) | 50,460 / 26,229 gz | 37,668 / 21,575 gz | 88,128 / 47,804 gz | 50,556 |

Coverage proof of the chosen subsets (from `coverage-report.txt`):
```
## subsets/NotoSans-Regular-MEDIUM.ttf
   Noto Sans Regular | Version 2.015 | 83,684 B | glyphs 885 | cmap 846 | sha256:8e2fdf47abc2b545
   [req] ASCII printable: ok   [req] Latin-1 …: ok   [req] U+02BB okina: ok   [req] U+02BC apostrophe: ok
   [req] Russian Cyrillic A-ya: ok   [req] Yo/yo: ok   [req] Uzbek Cyrillic Ўў Ққ Ғғ Ҳҳ: ok
   [req] Typographic punct: ok   [req] Currency/symbols ₽ № € ™: ok
   [opt] Karakalpak Latin Ǵǵ: ok   [opt] Karakalpak/Kazakh Cyrillic Әә Ңң Өө Үү: ok   [opt] Cyrillic full block: ok (256)
   TEST TEXT: all present
   VERDICT: PASS
## subsets/NotoSansMono-Regular-MEDIUM.ttf
   Noto Sans Mono Regular | Version 2.014 | 64,632 B | glyphs 881 | cmap 845 | sha256:8574fbec17569231
   … every [req] and [opt] line ok …   TEST TEXT: all present   VERDICT: PASS
```

Why MEDIUM rather than NARROW (+60 KB raw, +33 KB gzip per app): the fallback is also **gridcraft's PDF document face on the web**, and HR files carry Karakalpak and Kazakh personal names (Ǵ, Әә, Ңң, Өө, Үү, Ққ, Ұұ) and Russian combining stress marks. A missing glyph there is lost text in the exported PDF, not a cosmetic box. +33 KB gzip is under 0.5% of gridcraft-web's 7.04 MB gzip (Phase 0 inventory build). Why not BROAD: the extra 73 KB raw buys arrows and IPA that no upstream face lacks.

Why no Bold: no interface family in any app reaches the fallback only for bold glyphs. wordcraft's and pdfcraft's bold families are Inter weights (complete). gridcraft's `UI_BOLD` role falls back to the regular UI face (`theme.rs:409-410`). deckcraft's `bold` family starts with Inter SemiBold, which is absent at our craft-fonts pin, so the family uses its regular faces. A regular-weight ʻ inside bold text is a thin mark whose weight difference is barely visible. Revisit if the owner's evidence review objects (+84 KB raw).

Naming: OFL-1.1 §3 restricts only Reserved Font Names, and Noto declares none. The subset keeps the internal family names "Noto Sans" / "Noto Sans Mono". `SOURCE.md` states that the files are modified (subset). The egui keys are `office-fallback-sans` / `office-fallback-mono`. The files are `assets/fonts/fallback/NotoSans-Regular-uz.ttf`, `NotoSansMono-Regular-uz.ttf`, `OFL.txt` and `SOURCE.md`, committed in **our** tree, never in an upstream tree (pdfcraft's `xtask assets` would reject them, brand.md B7).

## 4. How each app's font stack gets it

Mechanism: `cargo xt overlay <app>` copies the two subset files into the overlay. The generated module exposes `FALLBACK_FONTS: &[(&str, &[u8])]` (`include_bytes!`, brand.md §3.2). `office_overlay.rs` (new file) provides

```rust
/// Append the fallback faces after every other face of every family (never first: upstream metrics stay).
pub fn append_fallback_fonts(defs: &mut egui::FontDefinitions) {
    for (key, bytes) in FALLBACK_FONTS {
        defs.font_data.insert((*key).into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    for (family, list) in defs.families.iter_mut() {
        let key = if *family == egui::FontFamily::Monospace { "office-fallback-mono" } else { "office-fallback-sans" };
        if FALLBACK_FONTS.iter().any(|(k, _)| *k == key) && !list.iter().any(|n| n == key) {
            list.push(key.into());
        }
    }
}
```

Each app's carried hook is **one call** at the end of its single font-definition builder. The changes ride in `0100-<app>-office-overlay-hook`, or in `0102-<app>-fallback-font` if 0100 grows past about 150 lines (PLAN WP8):

| App | Call site (old BASE) | Patch lines | Notes |
|---|---|---|---|
| wordcraft | end of `font_definitions_language` (`crates/ui-egui/src/theme.rs:473-518`, just before `fonts` is returned at `:517`) | 1 | Every `set_fonts` path goes through it (`install_fonts_language_extra` `:411-421`; `widgets.rs:889`; `canvas.rs:3591` builds on it). Document fallback faces (`add_fallback_faces` `:424-436`) are still appended after ours, which is harmless: both are fallbacks. |
| gridcraft | end of `font_definitions_with` (`crates/ui-egui/src/theme.rs:388-447`), before `drop_unparseable_fonts` (`:442`) | 1 | Named role families (`UI`, `CELL`, …, `:401-419`) are in `families` and receive it too. **PDF**: hand the same static bytes to the engine as the default face's fallback, next to the existing `set_default_face(ubuntu…)` (`:392-394`), through the API that queue patches 0020–0022 add (`pdf_fonts::register_font` / per-face fallback). Passing egui's bytes avoids the second copy (the Phase 0 spike measured +392,880 B when a font was linked twice). 3–5 lines. |
| deckcraft | end of `font_definitions` (`crates/ui-egui/src/theme.rs:169-205`), after `add_fallback_faces(&mut defs, extra)` (`:193`) | 1 | **Documents**: also register the sans subset in the slide font database at start (`deckcraft_fonts::FontDb::global().add_font(bytes.to_vec())`, the API used at `crates/render/src/tests.rs:999`), so web slides draw ʻ. **WP8 must verify** that FontDb's per-character fallback consults a late-added face (not verified here). 2–3 lines. |
| pdfcraft | end of `font_definitions_for_platform` (`crates/ui-egui/src/theme.rs:352-…`), after the Arabic/Telugu/Hebrew/Thai loop | 1 | That is the single builder behind every `set_fonts` site (`bidi.rs:544`, `appearance.rs:162,201`, `widgets.rs:604,729,750`, `installed_font_definitions` `:244`). Installed system fallbacks (`:250-282`, e.g. `:257`, `:269`) are still placed after it on desktop, which is fine. Typed signatures keep Dancing Script (Latin only); Cyrillic signatures fall back to Inter as upstream designed. |

gridcraft CLI (`gridcraft-cli`, used for headless conversion): the CLI does not link ui-egui. Its PDF default face is the `bundled-pdf-font` feature's Ubuntu copy, so **ʻ is still lost in CLI-made PDFs**. This matters when the Phase 5 docserver converts with the CLI. Fix (P2, Phase 5 at the latest): a new `crates/engine/build.rs` that, when `OFFICE_OVERLAY_DIR` is set, emits `office_fonts.rs` with `include_bytes!` of the overlay sans subset and registers it as the default face's fallback in `pdf_fonts`. This mirrors the pdfcraft engine `build.rs` in i18n.md §2.3.

## 5. Size impact

| Target | Delta per app | Basis |
|---|---|---|
| wasm (`*_bg.wasm`, fonts are `include_bytes!` data) | **+148,316 B raw; ≈ +80.6 KB gzip-9** (brotli smaller; WP8 measures with `cargo xt sizes`) | MEDIUM Regular + Mono. Within PLAN WP8's "< 250 KiB brotli each" estimate. |
| desktop exe | +148,316 B per exe (×4 apps ≈ +0.59 MB) | Same bytes, embedded. CLIs are unchanged in Phase 1. |
| suite installer (Phase 4 MSI, compressed) | ≈ +0.32 MB for the four exes (gzip ratio 0.54) | ESTIMATE. One shared on-disk font file would save about 0.24 MB, but it needs a runtime loader and a web path anyway. Not worth it. |
| repo | +148 KB (two subset files) + OFL.txt + SOURCE.md | `assets/fonts/fallback/` |

## 6. Verification (WP8 done-checks)

1. **Build time**: `scripts/subset-fallback-font.sh` re-creates the subsets from the pinned sources and fails unless the cmap check passes:

   ```python
   # fontcov.py (abridged; full copy at E:/office-tools/fonts-eval/fontcov.py.txt). Run with python -I.
   from fontTools.ttLib import TTFont
   REQUIRED = {"U+02BB": [0x02BB], "U+02BC": [0x02BC], "Russian": range(0x0410, 0x0450), "Yo": [0x0401, 0x0451],
               "Uzbek Cyrillic": [0x040E, 0x045E, 0x049A, 0x049B, 0x0492, 0x0493, 0x04B2, 0x04B3],
               "punct": [0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x201E, 0x2026, 0x2022, 0xAB, 0xBB],
               "symbols": [0x20BD, 0x2116, 0x20AC, 0x2122], "ASCII": range(0x20, 0x7F),
               "Latin-1": [c for c in range(0xA0, 0x100) if c != 0xAD]}
   cmap = set((TTFont(path, lazy=True).getBestCmap() or {}).keys())
   missing = {k: [hex(c) for c in v if c not in cmap] for k, v in REQUIRED.items()}
   assert not any(missing.values()), missing
   ```
2. **Run time** (`office_overlay_tests::fallback_font_covers_uzbek`, all four apps): for every family in the app's own `FontDefinitions`, every character of the test text must be in the cmap of **some** face in that family's chain. Read the cmap with the parser already in the app's lock (pdfcraft uses `skrifa` in its tests, `tests/fonts.rs:298`). **Do not rely on `Fonts::has_glyphs` alone**: upstream warns that it cannot tell a real glyph from the replacement face (`pdfcraft/crates/ui-egui/tests/fonts.rs:293-295`).
3. **gridcraft PDF**: export a sheet containing the test text to PDF (native and web) and extract it with `pdfcraft-cli text`. The output must contain the test text, U+02BB included.
4. **Evidence**: `docs/phase1/evidence/font-<app>-web.png` shows `Oʻzbekiston` in the language picker and the test text in a monospace field (wordcraft/pdfcraft: the formula/code field; gridcraft: the formula bar).
5. `i18n lint` (i18n.md §6.2) refuses any catalog character that neither the fallback nor the app's primary faces can draw.
6. Upstream's `primary_ui_fonts_cover_latin_and_cyrillic_catalogs` (`pdfcraft/crates/ui-egui/tests/fonts.rs:297-…`) hard-codes its catalog list and does not include uz, so it is unaffected (PLAN F10 confirmed).

## 7. Risks

- **F-R1 mixed faces mid-word.** ʻ from Noto inside an Inter or Ubuntu Light word shows a slight weight and shape change (Ubuntu *Light* against Noto *Regular* is the most visible: gridcraft web). Accepted for Phase 1; show it to the owner in the evidence. Noto Sans Light (+≈84 KB) would match gridcraft better if the owner objects.
- **F-R2 deckcraft's UI is BIZ UDPGothic** (no Inter at our craft-fonts pin), with Uzbek Cyrillic қғҳ drawn by Ubuntu Light inside BIZ words. This is upstream's state. A later fix could pin a craft-fonts revision that has `latin-manifest.txt`, or prepend Inter for ru/uz interfaces (that breaks the "never first" rule, so it needs an owner decision).
- **F-R3 deckcraft FontDb fallback** for late-added faces is unverified (§4). If it does not consult them, web slides keep the ʻ gap until a patch in `crates/fonts`.
- **F-R4 gridcraft CLI PDFs** lose ʻ until the engine `build.rs` hook lands (§4).
- **F-R5 pinning.** The notofonts repo republishes files in place. Pin the commit in the download URL and verify the SHA-256 in `SOURCE.md` in `scripts/bootstrap.sh`.
