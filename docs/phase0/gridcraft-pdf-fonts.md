# gridcraft PDF export: Cyrillic and Uzbek text (Phase 0 spike)

Worktree `E:/office-work/gridcraft-pdf`, branch `spike/pdf-fonts` (3 commits on `e6b6b34`, not pushed). Patches: `spikes/gridcraft-pdf-patches/0001..0003`. Build target `E:/office-target/gridcraft-pdf`. Throwaway spike code.

## Result in one paragraph

`file.exportPdf` now embeds TrueType subsets (Identity-H CID fonts with ToUnicode). On the four synthetic Excel files the `?` and `####` are gone from the text layer, and the mean text similarity (round trip) goes from **0.415 to 0.754**. Pixel diff did not improve (**3.77% to 3.90%**), see the table. Wasm grows by **+65,891 bytes raw (+0.35%), +8,487 bytes gzip (+0.12%)**.

## Approach and rationale

Chosen: **(a)**, embed and subset in gridcraft's own writer (`crates/pdf`). Not (b), krilla.

Upstream's direction:
- There is no decision record for PDF fonts. `docs/decisions.md` has none.
- The roadmap names it: `docs/gaps.md:53-57` (gap 5, "non-Latin PDF", 12-20 h, "PDF writer uses 14 standard fonts only"); `ROADMAP.md:127` ("Text in every script ... in cells and PDF"); `docs/localization-parity.md:36`; `docs/file-format-parity.md:59`.
- The standing rules point at (a): `crates/pdf/src/lib.rs:1-14` ("tiny, self-contained", no dependencies, `#![forbid(unsafe_code)]`); `docs/architecture.md:22` (pdf is a small L2 writer); `CLAUDE.md:34` (fonts never in the repo, system fonts at runtime); `CLAUDE.md:Never break wasm`.

Why not krilla:
- It would replace the writer (about 730 lines plus 12 `Font` variants used at about 20 call sites in `print.rs`) with a large dependency.
- It is not in gridcraft's lock file. `craft-pdf`'s `write` feature is pinned by git rev (`craft-libs`), and krilla brings its own font stack (skrifa, subsetter, flate).
- It adds wasm weight on a path that has none today.
- It conflicts with the "tiny self-contained" identity of the crate.
- Cost of (a): about 900 new lines, and uncompressed output (see limits).

Metrics source:
- The GUI binds installed workbook fonts (`crates/ui-egui/src/fonts.rs:bind_faces`) and falls back to egui's Ubuntu Light. The engine now uses the same files (installed family by name) and the same default face.
- Native: a `name`-table-only scan of the system font folders (`crates/engine/src/pdf_fonts.rs`).
- Web: no file system. The default face comes from egui's own bytes (`ui-egui/src/theme.rs` hands them over, so no second copy is linked). Hosts can add faces with `pdf_fonts::register_font`.

Design:
- `Font::Face(u16)` is a new variant. `text_width`, `char_width` and `Page::text*` keep their signatures, so `print.rs` changes stay small.
- Faces live in a process-wide registry (`embed.rs`, `register_face`), capped at 512 faces.
- Per-character fallback: a face may name a fallback face. Text is split into runs, one `Tf`/`Tj` pair per run.
- Subsetting keeps the original glyph ids (`CIDToGIDMap /Identity`). Unused glyphs become empty, and `hmtx` is trimmed to the highest used id.
- The TrueType reader is bounds-checked (returns `None`, never panics).
- `text_width_snapped`: Excel measures with integer-pixel advances at 96 dpi (11 pt Calibri digit = 7 px, not 7.44). The "does the number fit / `####`" test uses it. Without it, `3500000.0` in a 9.57-char column still showed `#`. Standard fonts are unaffected.
- Arial, Times New Roman and Courier New stay on the standard fonts when the text is Latin-1, so those PDFs do not change. Non-Latin-1 text switches to an installed Arial or equivalent, then the default face.
- `GRIDCRAFT_PDF_SYSTEM_FONTS=0` turns system font lookup off (byte-reproducible output). The crate's unit tests behave this way.

## Diffstat (`git diff --stat e6b6b34`)

```
 Cargo.lock                     |   4 +
 apps/gridcraft-cli/Cargo.toml  |   3 +-
 crates/engine/Cargo.toml       |   8 +
 crates/engine/src/cmd/print.rs | 108 ++++++++-
 crates/engine/src/lib.rs       |   1 +
 crates/engine/src/pdf_fonts.rs | 360 ++++++++++++++++++++++++++++++
 crates/pdf/Cargo.toml          |   6 +-
 crates/pdf/src/embed.rs        | 228 +++++++++++++++++++
 crates/pdf/src/lib.rs          |  98 ++++++++-
 crates/pdf/src/tests.rs        | 205 +++++++++++++++++
 crates/pdf/src/ttf.rs          | 487 +++++++++++++++++++++++++++++++++++++++++
 crates/ui-egui/src/theme.rs    |   5 +
 docs/*.md (3 files)            |   8 +-
 15 files changed, 1503 insertions(+), 18 deletions(-)
```

The `pdf` crate stays dependency-free (the Ubuntu Light dev-dependency is for tests only). Engine gets an optional `bundled-pdf-font` feature, enabled by `gridcraft-cli`.

Not done: the harness change (`--excel-exe`, `--no-reopen` in `spikes/fidelity/run.py`) is in our repo, not in the patches.

## Before / after (Excel files only, 4 files, round trip, Office 16 PDF as reference)

Before = release 0.7.0 (`fidelity-baseline.md`). After = patched `gridcraft-cli` (dev profile; output is profile-independent). Harness: `python -I spikes/fidelity/run.py --input .../synthetic-excel --out E:/office-target/fidelity-pdf-fix/out --builds head --apps excel --excel-exe E:/office-target/gridcraft-pdf/debug/gridcraft-cli.exe --no-reopen` (the Office reference PDFs were reused, so no Excel COM was run).

| File | Text sim before | Text sim after | Pixel diff before / after (mean %) | `?` / `#` chars in text layer before / after | PDF bytes before / after |
|---|---|---|---|---|---|
| hrm_staff.xls | 0.469 | 0.892 | 4.82 / 4.92 | many / 0 | 17,760 / 84,231 |
| hrm_staff.xlsx | 0.490 | 0.862 | 3.79 / 4.07 | many / 0 | 23,934 / 145,014 |
| hrm_timesheet.xls | 0.362 | 0.733 | 2.95 / 3.02 | many / 0 | 5,208 / 49,587 |
| hrm_timesheet.xlsx | 0.339 | 0.529 | 3.51 / 3.61 | many / 0 | 16,560 / 81,560 |
| **Excel mean** | **0.415** | **0.754** | **3.77 / 3.90** | | |

Reading:
- Text: the `?` and `####` were the dominant loss, and both are gone (checked with `pdfcraft-cli text`). All Cyrillic and Uzbek text (including `oʻ`, U+02BB) extracts correctly.
- Pixel diff is flat or slightly worse. Before, a `?` or `#` was ink in about the right place. Now real glyphs are drawn where Excel draws them, so the remaining difference is glyph anti-aliasing, heavier Excel table borders and page layout, which this patch does not touch. The pixel metric does not reward the fix.
- Remaining text-sim gap: extraction order, dates (`12.10` in Office's locale vs `12/10`) and number formats such as `#.##0`, none of which are font problems.
- Side by side (Office | before | after), under 0.2 MB in total: `docs/phase0/fidelity-pdf-fix/hrm_staff_xlsx_p01_before_after.png`, `hrm_staff_xlsx_p02_before_after.png`, `hrm_timesheet_xlsx_p01_before_after.png`. Visible: the `Maosh` and date columns no longer show `#######`, and Cyrillic headers are readable.

## Tests

`cargo test -p gridcraft-pdf -p gridcraft-engine --lib`: 294 engine and 23 pdf tests, all pass.

New in `crates/pdf/src/tests.rs`:
- coverage (Cyrillic, Uzbek Cyrillic present; Helvetica lacks them);
- Cyrillic embedded with no `(?`;
- ToUnicode round trip for Uzbek Latin with U+02BC, Uzbek Cyrillic, and U+02BB (the last only when the machine has a face with it);
- mixed scripts through per-character fallback, with width equal to the fallback's;
- the face's own widths;
- subset size and sfnt checksum;
- standard-font output unchanged;
- one font object across pages, with rotation;
- damaged or truncated fonts rejected without panic.

New in the engine:
- `pdf_fonts::tests` (5): standard fonts stay, Cyrillic gets a covering face, unknown family, host-registered face, okina.
- `print::tests` (3): Cyrillic and Uzbek cells export as real text (extracted through ToUnicode), Latin-1 text stays on the standard fonts, and a column that fits Arial but shows `#` in a wider face (metrics are the font's own).

Separately, `pdfcraft-cli text` on the patched PDFs shows the Cyrillic and Uzbek text (see the table).

Not run: `cargo fmt` and `clippy`. The toolchain has neither component installed. `cargo xtask ci` was not run. The native `gridcraft` desktop app was not built (only `ui-egui` for wasm, through trunk).

## Size impact

`trunk build --release` of `gridcraft-web` (baseline from the inventory build of the same HEAD):

| | raw | gzip -9 |
|---|---|---|
| baseline | 18,973,730 | 7,041,161 |
| patched | 19,039,621 | 7,049,648 |
| delta | +65,891 (+0.35%) | +8,487 (+0.12%) |

A first attempt linked Ubuntu Light from the engine and cost +392,880 raw, because the egui crate has its own copy of the bytes. Handing egui's bytes over from `theme.rs` removed it. The CLI links its own copy (+362 KB), which only affects the native binary.

## Upstreamability

Good fit:
- no new runtime dependency in `crates/pdf`;
- layering preserved (`cargo xtask layers` was not run, but only `ui-egui -> engine -> pdf` edges exist, all previously there);
- wasm-safe, the fonts rule respected (no font files in the repo), and the gap-5 docs updated.

Probable review points:
- the process-wide face registry (a handle in the document would be cleaner but touches about 20 call sites);
- system-font scan code duplicated from `ui-egui/src/fonts.rs` (could move into the engine and be shared);
- PDFs are 3-10x larger until streams are compressed;
- output now depends on installed fonts (switch `GRIDCRAFT_PDF_SYSTEM_FONTS=0`);
- the pixel-snapped width is Windows/GDI-specific behaviour.

Suggest splitting into two PRs: the pdf crate (0001) and the engine wiring (0002).

## Remaining limits

- **No compression.** Subset fonts and content are uncompressed, so PDFs grow (table above). Needs `FlateDecode` (miniz_oxide is already in the lock file).
- **RTL, shaping, bidi:** none. Arabic, Hebrew and Devanagari are drawn in logical order, unshaped.
- **CJK:** most CJK system fonts are CFF/OTF or collections. The reader supports `glyf` only, so those fall back to notdef. craft-fonts' Noto CJK is a CFF OpenType font and would need `FontFile3` embedding.
- **Emoji / colour fonts:** no (no `CBDT`/`COLR`, no emoji font is chosen).
- **U+02BB (oʻ, gʻ) and ₽:** egui's Ubuntu Light has no glyph for U+02BB, so on a machine with no installed face that has it (web, a bare Linux box) it prints as notdef. The same gap exists in the web GUI. Calibri, Arial, Segoe UI, Times, Tahoma and DejaVu cover it. A craft-fonts entry with Latin extended and modifier letters would fix both. ToUnicode cannot represent a notdef glyph, so that character is lost in extraction.
- **Font fallback:** the default face has only a regular weight (bold falls back to regular). Variable fonts embed the default instance only. Kerning, ligatures and hinted widths are not applied except the pixel snapping above.
- **Bold/italic:** exact style files only (Regular, Bold, Italic, Bold Italic); Light, Semibold and similar are not selected.
- **Charts:** chart labels and shape or header text go through the same fallback, but charts always start from Helvetica, not the cell font.
- The font catalog is scanned once per process (new installs need a restart), and a face file over 48 MB is skipped.
