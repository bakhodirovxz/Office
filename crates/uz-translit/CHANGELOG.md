# Changelog

## 0.1.0
- First implementation of docs/phase2/rules.md (R1-R17, run-aware API, exceptions dictionary).

## Unreleased (rules/3)
- WP16 (2026-10-21, no output change): `DEFAULT_LATIN_ALPHABET` is the one default switch (owner decision O13, still
  `Y1995`); `LatinAlphabet::default()` and `Options::default()` follow it, `LatinAlphabet::name()` gives `1995|2026|auto`.
  Tests pin 1995 explicitly (`tests/common::o95`). Golden SRC-017..025 and a test from the first official text written
  in the 2026 alphabet (rules.md section 15, S16).
- Gate review M1: `tog’’` and `'bog''` keep the okina (the second apostrophe is the quote closer). Dictionary: `sirk` is a stem, new stem `sirka`.
- rules/2 (below):
- `Options.latin_alphabet: LatinAlphabet { Y1995 (default), Y2026 }`: ->Latin can write the pending 2026 alphabet
  (ö ğ ş ç). ->Cyrillic always reads both alphabets, incl. decomposed forms (rules.md section 15).
- Dictionary changes from docs/phase2/source-verification.md: `pochtalon` variant withdrawn, `QR` keep promoted,
  `karyera` -> `karyer`, new `kuryer`, `kirillits`, `marganets`, `slanets`, `slans`, `kvars`, `radiostansi`,
  `oʻzbekkonsert`, `adʼyunktura`; JShShIR confidence lowered to medium.
