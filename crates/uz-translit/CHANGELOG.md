# Changelog

## 0.1.0
- First implementation of docs/phase2/rules.md (R1-R17, run-aware API, exceptions dictionary).

## Unreleased (rules/2)
- `Options.latin_alphabet: LatinAlphabet { Y1995 (default), Y2026 }`: ->Latin can write the pending 2026 alphabet
  (ö ğ ş ç). ->Cyrillic always reads both alphabets, incl. decomposed forms (rules.md section 15).
- Dictionary changes from docs/phase2/source-verification.md: `pochtalon` variant withdrawn, `QR` keep promoted,
  `karyera` -> `karyer`, new `kuryer`, `kirillits`, `marganets`, `slanets`, `slans`, `kvars`, `radiostansi`,
  `oʻzbekkonsert`, `adʼyunktura`; JShShIR confidence lowered to medium.
