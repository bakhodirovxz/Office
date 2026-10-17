# Fallback fonts (Noto Sans, Noto Sans Mono), subset

| File | Source | SHA-256 of the unmodified source |
|---|---|---|
| `NotoSans-Regular-uz.ttf` | Noto Sans Regular v2.015, unhinted TTF | `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8` |
| `NotoSansMono-Regular-uz.ttf` | Noto Sans Mono Regular v2.014, unhinted TTF | `87f8ce0522a6c99b743ee5fc75b4073cfdd575639119672828b7b9944b65b4f4` |

- Download URL (pinned to the notofonts.github.io commit `5c3913063bb189df4edc94b766584dfb4c04a187`, 2026-10-14):
  `https://raw.githubusercontent.com/notofonts/notofonts.github.io/5c3913063bb189df4edc94b766584dfb4c04a187/fonts/NotoSans/unhinted/ttf/NotoSans-Regular.ttf`
  and the same path with `NotoSansMono/.../NotoSansMono-Regular.ttf`.
- Licence: SIL Open Font License 1.1 (`OFL.txt`, from `notofonts/latin-greek-cyrillic`). The OFL header declares no Reserved
  Font Name, so the subset keeps the family names "Noto Sans" / "Noto Sans Mono" (OFL section 3).
- **These files are modified**: they are subsets (fontTools `subset`), produced by `scripts/subset-fallback-font.sh`
  with the Unicode ranges listed in that script (Latin, Latin-1, Latin Extended-A/B, U+02BB/U+02BC, Cyrillic block, punctuation,
  currency). The script verifies the cmap against the required Uzbek/Russian set.
- Used only as the **last** face of every interface font family (`office-fallback-sans` / `office-fallback-mono`).
