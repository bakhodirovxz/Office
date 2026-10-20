# Golden list: drafting notes (Phase 2 WP3, drafting half)

Status: **reviewed** (2026-10-20, `reviewed:agent-opus:2026-10-20`, owner delegation; not an owner review). See the
[Review](#review-wp3-review-half-2026-10-20) section at the end. The drafting sections below describe the candidate list as
drafted, before review.
Drafter: sonnet. Rules checked against `docs/phase2/rules.md`.

## Files
| File | Rows | Columns |
|---|---|---|
| `crates/uz-translit/data/golden.tsv` | 1044 | `id latin cyrillic dir rule status source conf note` (header row) |
| `crates/uz-translit/data/variants.tsv` | 83 | `id input latin_canonical cyrillic variant rule status source conf note` |

- `dir`: `both` (l2c(latin)==cyrillic and c2l(cyrillic)==latin), `l2c` (only the first), `c2l` (only the second). In a `c2l` row, `latin` is the **expected output** and `cyrillic` is the input.
- `rule`: comma-separated rule ids from rules.md. `tables` = plain letter tables (§3).
- `source`: `agent` (drafted from orthography knowledge, rule cited in `rule`), `official-name` (region, city, ministry, public figure), `rules.md-§10` (a worked example copied from the spec).
- `conf`: `high | medium | low`. The latin column always holds the canonical output form (U+02BB for oʻ/gʻ, U+02BC for tutuq and sʼh). Rows whose input uses other apostrophes are `l2c` only (R15 quote rows) or live in `variants.tsv`.
- The id prefix is the category. Dictionary-dependent rows carry `R11` in `rule`.

## Self-check done
A throw-away Python model (not committed) of the rules (R1–R7a, R10, c2l tables) was run over all 1559 assertions of the rows that need no dictionary, no protected span, no quote guard and no R8/R9/R16/R17: **0 mismatches**. The 99 rows that differ from the rule-only result are exactly the dictionary-dependent ones (checked by listing them). The Rust crate has not been run against them (no build in this task).

## Coverage per category
| Category (prefix) | Rows | Notes |
|---|---|---|
| Letters and digraphs (`LET`) | 72 | the 26 letters, ch sh ng oʻ gʻ ye yo yu ya ts, then words |
| oʻ / gʻ (`APO`) | 71 | plus **83** apostrophe-variant rows in `variants.tsv` (8 variants x 8 words, plus caps and mixed rows) |
| Tutuq belgisi and sʼh (`TUT`, `TUT2`) | 59 | maʼno, sanʼat, taʼlim, masʼul, Isʼhoq (R4: 10 golden rows plus 10 in variants) |
| ye / е / э (`YE`) | 69 | word start, after a vowel, middle, after a tutuq |
| ё / ю / я (`YO`) | 60 | includes й + vowel loans (rayon, mayor, konveyer) |
| ц (`TS`) | 84 | R7a (34 tagged rows in all categories; 26 in `TS`), native ts (R7, 10), dictionary ц (48) |
| ь / ъ (`SOFT`) | 70 | months, inflected forms (sentabrda, oktabrdan), ўъ, vowel + ъ + iotated |
| Letter case (`CASE`) | 72 | lower/Title/UPPER, Single letter, Mixed acronyms |
| Names (`NAME`) | 156 | people, surnames, regions, cities, districts, ministries |
| HR and office vocabulary (`HR`) | 177 | |
| Calendar and numbers (`CAL`) | 31 | |
| Protected spans (`SPN`) | 70 | codes, e-mail, URL, domains, numbers, Roman numerals, paths, keep entries, R14 |
| Quotes (`QUO`) | 10 | R15, plus 3 ambiguous rows in `AMB` |
| Ambiguous and one-way (`AMB`) | 23 | 19 are `conf=low` |
| Non-round-trip classes (`NRT`) | 15 | ц, ь, ъ, щ, ы without entries |
| Combining marks R16 (`U16`) | 5 | `c2l` only |
| **Total** | **1044** | |

By direction: `both` 994, `l2c` 19, `c2l` 31.
By confidence: high 973, medium 52, low 19.

Quota notes against `PLAN.md` and rules.md §11:
- R4 sʼh: 10 golden rows (quota 10) plus the variants file.
- R9a (ьо): only 3 `both` rows (pochtalyon, batalyon, pavilyon) plus one `l2c` (pochtalon). Quota 5 is short by 2. I know no further safe words; `bilyard` is `low`.
- R8 vowel + ъ + iotated: 5 rows (maʼyus, maʼyuslik, taʼyin, taʼyinlash, taʼyinlandi). Quota met.
- R15: 10 `QUO` rows plus 3 ambiguous `AMB`. Quota (8, 3 ambiguous) met.
- R14: 10 rows. R16: 5 rows. R12: 53 rows tagged.
- §9.4 one-way classes: ц 3, ь 3, ъ 3, щ/ы 6, Mixed 3, variant spelling 2, missing separator 1. The **solid-compound э class has only 1 row** (Ўзбекэнерго) because I know no further real compounds.
- R17 `щ`/`ы` rows are medium: the rule is the plugin's, not an orthography citation.

## Low-confidence list (`conf=low`, 19 rows)
| Id | Row | Why |
|---|---|---|
| AMB-001 | `'bog'` to `'бог'` (l2c) | R15: the last U+0027 is read as the closing quote. Inherently ambiguous. |
| AMB-002 | `Ular 'tog' dedi` to `Улар 'тог' деди` | same |
| AMB-003 | `Ular 'bog' va 'tog' dedi` | same, two quoted words |
| AMB-004 | `Ishoq` to `Ишоқ` | missing separator; could mean Isʼhoq |
| AMB-007 | `pochtalon` to `почталон` | D-10: the official Latin of почтальон is pochtalyon |
| AMB-008 | `QR-kod` to `ҚР-код` | QR vs ҚР (§13.6) |
| AMB-009 | `AQSh` to `АҚШ` (l2c) | D-14 |
| AMB-010, 011 | `yod` to `ёд`; `йод` to `yod` | D-24: ёд "memory" vs йод "iodine" |
| AMB-012 to 014 | `SHahar`, `ToShkent`, `TOshkent` (l2c) | Mixed case; not guaranteed to round-trip (§5.5) |
| AMB-015 | `Ўзбекэнерго` to `Oʻzbekenergo` (c2l) | D-07 |
| AMB-016, 017 | `I. Karimov`, `V. Aliyev` (l2c) | D-16: I/V/X + dot at a paragraph start is a Roman numeral |
| AMB-018 | `e-mail` to `э-маил` | R14 hyphen limitation |
| AMB-020 to 022 | `bilyard`, `mayonez`, `sink` | the dictionary seed marks them `low` |

Medium rows worth a second look: AQSH (D-14, `both`), pochtalyon/batalyon/pavilyon (D-10), moʻjaz and moʻmin (ўъ seeds), Shveysariya, McDonald's restorani (the guard covering a word with an apostrophe), the `FHDYo` and `TDYuU` l2c rows (the reverse gives all caps, D-14), mashʼal and Qurʼon (Arabic loans I am fairly, not fully, sure of), Asʼhob/musʼhaf/Isʼhoqov (rare s'h words).

## Open questions for the reviewer
1. **Quotation rows with variants.** Rows such as `‘Bog’ ko’chasi’` are `l2c` only because their latin column is a non-canonical input. Should they be moved into `variants.tsv` instead, to keep `golden.tsv` strictly canonical?
2. **`sirka` (SPN/TS-037) and `sirk`.** Check the word-only rule for `sirk` against `sirka` in the real table.
3. **`kontrol`/`kontrolyor`, `profilaktika`, `sekretariat`.** These rely on D-11 (the stem drops ь before any remainder). Confirm with the real dictionary compiler; if `kontrolyor` should be `контролёр` the entry must not eat the `y`.
4. **R7a native counter-examples.** None of my 10 native ts rows (ketsa, aytsin, otsiz, ...) is followed by `i` + y/a/o/e. Please grep lex.uz-derived data for a native `tsia/tsio/tsiy/tsie`.
5. **`Nyu-York`** relies on the `york` word entry and the hyphen boundary; `Nyu` converts by R6 (`Ню`).
6. **Roman numerals.** `A. Karimov` is high confidence (A is not a Roman letter); `X. Rahimov` is mid-text only (SPN row).
7. **Arabic loan rows** (`mashʼal`, `Qurʼon`, `sharʼiy`, `qalʼa`, `sunʼiy`): spellings follow current Cyrillic usage as I know it; I did not check lex.uz pairs (no network source was used). All rows from my knowledge carry `source=agent`; none was verified against a parallel official text.
8. **Row count.** The list is deliberately larger than the 600 requested so that at least 500 survive review; trim `HR`/`NAME` first if the review budget is tight.
9. **Plugin and lex.uz diff.** Not run: `scripts/translit/diff-plugin.mjs` does not exist yet, so none of the rows has a plugin comparison.

## Review (WP3 review half, 2026-10-20)
Reviewer: opus, `reviewed:agent-opus:2026-10-20` (owner delegation 2026-10-20; not an owner review; the owner reports
problems found in use). Every row was read for: correct official Latin (U+02BB in oʻ/gʻ, U+02BC for tutuq and sʼh), correct
Uzbek Cyrillic, direction, rule tags, and whether the word is real.

### Evidence
- **Code-point audit** (script, all rows): the Latin columns contain only ASCII letters, U+02BB and U+02BC, except the
  span, quote and R14 rows, which hold their intended characters. The Cyrillic columns contain no Latin homoglyphs. The
  U16 rows really contain U+0306 / U+0308. There are no duplicate ids.
- **Independent model.** A throw-away Python model of rules.md (R1–R17, R12 spans, R15 quote pass, R10 case) uses the
  real `exceptions.tsv`, with only its `reviewed:*` rows, as `build.rs` will. It was written by the reviewer, not reused
  from the drafter. Result: every reviewed row in both files matches in every direction it declares. The only rows that
  do not match are the 3 whose dictionary rows are not compiled (AMB-020/021/022), and they are now `ambiguous`. The
  Rust crate itself was not run (no builds in this task).
- **Plugin three-way diff** (the owner's unmodified `translit.js` under Node): 241 disagreements. Every one is in a
  class the spec marks as an intended difference: the dictionary (the plugin has none), R7a, R12 codes / keep entries /
  ROMAN / paths / @mentions / the `x` sign, R14, R15, R16, Single-letter case (Ш. РАШИДОВ) and the D-14 `acro` rows.
  None is in a plain letter-rule row.
- **lex.uz parallel texts: not used** (no network in this task). All rows remain `source=agent` or `official-name`
  from knowledge. This is the main remaining gap against PLAN WP3's source order.

### Statistics
| | golden.tsv | variants.tsv |
|---|---|---|
| Candidate rows reviewed | 1044 | 83 |
| Approved unchanged (conf adjusted on 8) | 1022 | 81 |
| Fixed, then approved | 4 | 0 |
| `ambiguous` | 13 | 0 |
| `rejected` | 5 | 2 |
| Added by the reviewer (all approved) | 22 | 0 |
| **Total rows / approved** | **1066 / 1048** | **83 / 81** |

Approved golden rows by direction: **`both` 999**, `c2l` 34, `l2c` 15. **Gate (≥ 500 approved `both`): met, 999.**
Approved rows by conf: high 981, medium 67, low 0.

**Rejected:**
- SOFT-068/069/070 `taʼyin…`: non-standard spelling. The standard form is tayin / тайин (the list's own HR-129 uses
  `tayinlash`).
- SPN-068, SPN-069: duplicates of `both` rows SPN-025 and SPN-010.
- VAR-074, VAR-075: duplicates of VAR-029 and VAR-011 (the same code points).

**Fixed:**
- NRT-007: `obyavleniye/объявление` is a Russian word that Uzbek does not use. It is replaced by `konyunktivit/конъюнктивит`.
- NRT-015: `yorsh/ёрш` has no щ or ы and round-trips by rule. It is replaced by `yashik/ящик` (R17).
- SPN-059: the R11 tag is removed.
- AMB-004 `Ishoq`: `both` is changed to `l2c`, because Ишоқ is not a word.

**Added** (each checked with the model):
- R8: SOFT-071..073.
- R9a: SOFT-074..078.
- R4: TUT-058.
- R12: SPN-071..078.
- §9.4: AMB-024..028.

### Open questions: resolutions
1. **Quote rows with variant apostrophes stay in `golden.tsv`, as `l2c`.** A quote-guard sentence needs U+0027,
   U+0060 or U+2019 by definition (R15 closers), so it can never be canonical. `variants.tsv` is per-word apostrophe
   normalisation with a canonical column. Clarification of the format: the "canonical latin" rule applies to the
   `both` and `c2l` rows; in an `l2c` row the latin column is the **input**.
2. **`sirk` / `sirka`: correct against the real table.** `sirk` is `word`, `sirkulyar` is a longer stem, and no
   stem prefixes `sirka`. Both rows pass.
3. **`kontrolyor`, `profilaktika`, `sekretariat`: correct.** The stem `kontrol` is followed by the remainder `yor`,
   which gives `ёр` by R6. The entry does not eat the y, and the result is контролёр, as in Russian. **New finding, the
   same mechanism in reverse:** the `medal` stem turns `medalyon` into **медалён** (Russian медальон). The same would
   happen to `modelyer` → моделер. This is a dictionary fix, not a spec change: add a longer stem `medalyon` →
   `медальон` (SOFT-076 is `c2l` until then).
4. **R7a native counter-examples: none found.**
   - The i18n catalogs (the P10 corpus) have 11 hits of vowel + `tsi` + y/a/o/e, and all of them are Russian loans
     (navigatsiya, annotatsiya, …).
   - Native t+s forms come from the suffixes -sa/-sin/-siz, so `i` is followed by n or z.
   - `corpus/` is not present on this machine, and lex.uz was not grepped. D-08 stays on the §13.7 watch list.
5. **`Nyu-York`: correct.** `Nyu` converts by R6, and `york` is a word entry, matched per hyphen part.
6. **Roman numerals: no change.** `A. Karimov` is safe. The paragraph-start `I.` / `V.` rows are now `ambiguous`
   (§13.3).
7. **Arabic loans: confirmed as the standard spellings.** mashʼal/машъал, Qurʼon/Қуръон, sharʼiy/шаръий,
   qalʼa/қалъа, sunʼiy/сунъий, daʼvo/даъво and musʼhaf/мусҳаф are correct, and TUT2-001/002 are raised to high. Two
   items are doubtful:
   - `taʼyin` is rejected (see above).
   - `moʻmin/мўъмин` is `ambiguous`, because Cyrillic usage is split (мўмин; the surname Мўминов).
8. **No trimming was needed.** 999 `both` rows are approved.
9. **The plugin diff was run** (see Evidence). The lex.uz diff was not, and it remains open.

### Quota gaps
- **R9a, 5 `both` rows: met.** pochtalyon, batalyon, pavilyon, plus pochtalyonlar and batalyonlar (stem +
  suffix).
  - Further real ьо words exist (медальон, бульон, лосьон, компаньон, шампиньон), but each needs a new
    `exceptions.tsv` row before it can be `both`. They are added as `c2l` rows (SOFT-076..078) and proposed for the
    dictionary.
- **R8 (vowel + ъ + iotated), 5 `both` rows: met** after the taʼyin rejection, with maʼyusona, maʼyuslanmoq and
  maʼyuslanib. The class has one productive root (maʼyus); no other standard word is known.
- **Solid-compound э (§9.4), 3 rows: met.** Ўзбекэнерго, Ўзэкспомарказ and Ўзэлтехсаноат, all `c2l` and all
  proper names.
  - The class is thin because Uzbek writes most such compounds with a vowel before э (Ўзбекгидроэнерго, which
    round-trips) or with a hyphen.
  - The consonant + э cases found are organisation names built on Ўз-/Ўзбек-.
- **Other §9.4 classes now at 3:** variant spelling (AMB-026 sentyabrda) and missing separator (AMB-027/028).
- **R4, 10 `both` rows: met**, with TUT-058.
- **R12, 3 sentences per span type: met** for URL, CODE-B, path and @mention (SPN-071..078).

### Remaining low-confidence and `ambiguous` items
| Rows | Why | Proposal |
|---|---|---|
| AMB-001/002/003 | R15 with U+0027 throughout (§13.4) | keep; watch in use |
| AMB-007 `pochtalon` → почталон | D-10: pochtalon is a widespread Latin spelling | `variant` row pochtalon → почтальон |
| AMB-008 `QR-kod` | §13.6 | keep |
| AMB-010 `yod` → ёд | D-24 (ёд vs йод) | keep; context only |
| AMB-016/017 `I.` / `V.` at a paragraph start | D-16 / §13.3 | keep |
| AMB-018 `e-mail` → э-маил | R14 hyphen limitation | consider a `keep` row `e-mail` |
| AMB-020/021 bilyard, mayonez | the pairs are correct; the dictionary rows are `ambiguous` | promote the `exceptions.tsv` rows, then these rows |
| AMB-022 sink/цинк | the pair is correct by R7; Uzbek says rux | decide together with the dictionary row |
| SOFT-065 moʻmin | мўъмин vs мўмин | demote the `exceptions.tsv` seed row to `ambiguous` |
| medium, approved | D-14 AQSH; D-10 pochtalyon/batalyon/pavilyon; rare sʼh words (Asʼhob, musʼhaf); FHDYo/TDYuU l2c | watch in use |

**Dictionary follow-ups** (in `exceptions.tsv`; the reviewer did not edit that file):
1. Promote bilyard and mayonez, and decide sink.
2. Demote moʻmin.
3. Add the stem `medalyon` → `медальон`, and consider `bulyon`, `losyon` and the variant `pochtalon`.

## Source check (2026-10-20)
The lex.uz and official-source check is in `rules.md` §14, and the dictionary follow-ups for WP2 are in `source-verification.md`.

Changes to `golden.tsv`:
- **Promoted:** AMB-007 and AMB-008, both now `c2l` (reviewed); they become `both` after SV-03 and SV-04.
- **Raised to high:** CASE-054 AQSH (D-14 confirmed).
- **Added:** 16 `SRC` rows, all official parallel pairs: 6 `both`, 9 `c2l` and 1 `l2c`.
- **Sources:** 334 rows got `source = lex-attested`, and about 30 rows got a specific `lex-<id>`.

Totals: 1082 rows, of which 11 are `ambiguous`.
