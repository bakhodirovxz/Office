# Source verification: dictionary changes for WP2

| | |
|---|---|
| **Date** | 2026-10-20 |
| **Status** | `reviewed:agent-opus:2026-10-20`. This is an agent check under the owner's delegation, not an owner review. |
| **Evidence** | `docs/phase2/rules.md` §14. Source ids such as `lex-4674902` are defined there. |
| **Scope** | Changes for `crates/uz-translit/data/exceptions.tsv`. This agent did not edit that file; WP2 applies these changes. |

Column order for new rows: `latin cyrillic match dir conf status source note`. Every new row gets `status = reviewed:agent-opus:2026-10-20`.

## Required changes

**SV-01. No change for the months (D-11 is confirmed).**
- Keep `sentabr`, `oktabr` (stem, both) and the `word` rows for the other months.
- Optional: set their `source` to `lex-parallel`.

**SV-02. Do not demote the `sentyabr` and `oktyabr` variants.**
- They are correctly `variant` rows: official Latin never writes them (0 hits in S5; S11).

**SV-03. Withdraw the `pochtalon` → `почтальон` variant row.**
- Set the row `pochtalon	почтальон	stem	variant	…` to `rejected`, or delete it.
- Reason: the Statistics Agency (`stat-366`) pairs Uzbek Cyrillic **почталон** with Latin **pochtalon**, so pochtalon → почталон by rule is the official pair. The variant row would turn it into почтальон.
- Keep `pochtalyon	почтальон	stem	l2c` (R9a, Russian-style input), and add to its note "official Uzbek form: почталон / pochtalon (stat-366)".
- Then golden AMB-007 can become `both`.

**SV-04. Promote `QR` (keep).**
- Change the row to: `QR	-	keep	l2c	medium	reviewed:agent-opus:2026-10-20	lex-4674902	Tax Code: QR-кодларнинг / QR-kodlarning`.
- Residual risk: a Latin `QR` meaning Qoraqalpogʻiston Respublikasi would stay Latin. No such use was found.
- Then golden AMB-008 can become `both`.

**SV-05. Add `kuryer`.**
- Row: `kuryer	курьер	stem	l2c	high	reviewed:agent-opus:2026-10-20	lex-6058066	kuryerlik 159x in the postal law; ь is internal, so D-11 does not apply`.
- Today the rules give kuryerlik → курерлик.
- Then golden SRC-011 can become `both`.

**SV-06. Replace `karyera` with `karyer`.**
- Row: `karyer	карьер	stem	l2c	high	reviewed:agent-opus:2026-10-20	lex-4674902	karyer, karyerda (Tax Code); karyera = karyer + a`.
- Delete `karyera`: the shorter stem covers it (karyer + a → карьер + а).
- Then golden SRC-012 can become `both`. Golden SOFT-038 (karyera) must still pass.

## Recommended (official pairs found; the rules alone get the l2c direction wrong)

| # | latin | cyrillic | match | dir | conf | source | note |
|---|---|---|---|---|---|---|---|
| SV-07 | kirillits | кириллиц | stem | l2c | high | lex-112286 | ts + a after a vowel; R7a does not cover it (golden SRC-013) |
| SV-08 | marganets | марганец | stem | l2c | high | lex-4674902 | word-final ц after a vowel (golden SRC-007) |
| SV-09 | slanets | сланец | word | l2c | high | lex-4674902 | `word`, because the plural is сланцлар / slanslar (golden SRC-009) |
| SV-10 | slans | сланц | stem | l2c | medium | lex-4674902 | 5 letters; slanslar. Grep for a false hit first (golden SRC-010) |
| SV-11 | kvars | кварц | stem | l2c | high | lex-4674902 | kvarsit → кварцит (golden SRC-008) |
| SV-12 | radiostansi | радиостанци | stem | l2c | high | lex-1625271 | 1995 rules item 48; `stansi` only matches at a word start (golden SRC-014) |
| SV-13 | oʻzbekkonsert | ўзбекконцерт | word | l2c | high | lex-2831618 | organisation name (golden SRC-015) |

Golden rows SRC-007 to SRC-015 are `c2l` for now. When one of these rows lands, WP2 changes the golden row to `both`.

## Notes only (no row change)

- **SV-14. Add `source` ids to rows confirmed by an official parallel pair.** Optional:
  - `sex`, `sirk`, `ssena`, `konsert`: `lex-6257288`, `lex-6600413`, `lex-2831618`;
  - `sement`, `sentner`, `sikl`, `senzura`, `sivilizats`: `lex-4674902`;
  - `funksi`, `aksiya`, `konsepsi`, `stansi`, `litsenz`, `protsess`, `dotsent`, `ofitser`, `obyekt`, `subyekt`, `kompyuter`, `film`, `medal`: `lex-parallel`;
  - `MChJ`: `lex-22525`.
- **SV-15. JShShIR (acro): lower the confidence to medium.** gov.uz writes both JShShIR and JSHSHIR; D-14 by rule would give JSHSHIR. YaTT and YaIM were not found in an official text, so they stay as they are.
- **SV-16. Keep `moʻmin` as `ambiguous`.** No online dictionary was found.
- **SV-17. `sink` (zinc).** The official texts use rux / рух. The row is harmless, so keep it.
- **SV-18. Candidate class: prefix + ц-root.**
  - The lexicographer R. Jomonov writes антициклон → antisiklon, эпицентр → episentr and геоцентрик → geosentrik; R7 gives antitsiklon.
  - No official text was found.
  - If WP2 adds these rows, it adds them as `ambiguous`, and they are not compiled.
- **SV-20. Optional: add a `c2l` row for adʼyunktura.**
  - Row: `adʼyunktura	адъюнктура	stem	c2l	medium	…	lex-5013007`.
  - The Law on Education writes the tutuq here. This is the only official exception to D-09 rule 2 that was found; obyekt and subyekt follow the rule hundreds of times.
  - Golden SRC-016 is `l2c` until this row lands.
- **SV-19. Do not add `yod`.** Both йод / yod (iodine) and ёд / yod (memory) occur in ПФ-158; D-24 stands.
