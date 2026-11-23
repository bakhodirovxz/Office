# Fallback fonts (Noto Sans, Noto Sans Mono, Tinos), subset

| File | Source | SHA-256 of the unmodified source |
|---|---|---|
| `NotoSans-Regular-uz.ttf` | Noto Sans Regular v2.015, unhinted TTF | `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8` |
| `NotoSansMono-Regular-uz.ttf` | Noto Sans Mono Regular v2.014, unhinted TTF | `87f8ce0522a6c99b743ee5fc75b4073cfdd575639119672828b7b9944b65b4f4` |
| `Tinos-Regular-uz.ttf` | Tinos Regular v1.340 (google/fonts commit `5bb28733cec313b63fbd415b574b5ae35c8aa8b8`, `ofl/tinos/Tinos-Regular.ttf`) | `60a0e8ef0c04dd5dd69ffe91025fa2ae5836cbd35600a82ba031977557e2cb61` |

- Download URL (pinned to the notofonts.github.io commit `5c3913063bb189df4edc94b766584dfb4c04a187`, 2026-10-14):
  `https://raw.githubusercontent.com/notofonts/notofonts.github.io/5c3913063bb189df4edc94b766584dfb4c04a187/fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf`
  and the same path with `NotoSansMono/.../NotoSansMono-Regular.ttf`.
- Licence: SIL Open Font License 1.1 (`OFL.txt`, from `notofonts/latin-greek-cyrillic`). The OFL header declares no Reserved
  Font Name, so the subset keeps the family names "Noto Sans" / "Noto Sans Mono" (OFL section 3).
- **These files are modified**: they are subsets (fontTools `subset`), produced by `scripts/subset-fallback-font.sh`
  with the Unicode ranges listed in that script (Latin, Latin-1, Latin Extended-A/B, U+02BB/U+02BC, Cyrillic block, punctuation,
  currency). The script verifies the cmap against the required Uzbek/Russian set.
- Used only as the **last** face of every interface font family (`office-fallback-sans` / `office-fallback-mono`).
- **Tinos** (Times New Roman's metric twin; Steve Matteson, Copyright 2026 The Tinos Project Authors) is licensed under the
  SIL OFL 1.1, text in `OFL-Tinos.txt` (from `googlefonts/tinos` commit `4106aab86ddefe26d58a69745e04ae9479d22993`). The font declares no Reserved
  Font Name, so the subset keeps the family name "Tinos". Source URL:
  `https://raw.githubusercontent.com/google/fonts/5bb28733cec313b63fbd415b574b5ae35c8aa8b8/ofl/tinos/Tinos-Regular.ttf`.
  The subset also drops hinting (`--no-hinting --desubroutinize`) to save size. Since 2026-11-23 its ranges (`TINOS_RANGES` in the script) are narrower than the Noto ones: Latin Extended-B and the Cyrillic extensions outside uz/kk/ky/tg/uk are dropped (40,576 B raw, 21,839 B brotli; was 74,024 / 37,251); those letters draw in the sans fallback. It is bundled in the Sheets and Slides builds only
  (`office-fallback-serif`: the serif face of the grid, and the slide font database's stand-in for Times New Roman); Docs bundles Source Serif 4.
