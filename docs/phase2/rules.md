# Uzbek Latin ⇄ Cyrillic transliteration rules (`uz-translit`)

| | |
|---|---|
| **Status** | `reviewed:agent-opus:2026-10-20` (owner delegation, 2026-10-20). Not an owner review. The owner reports problems found in use. |
| **Scope** | The authoritative rule set for the `uz-translit` crate (Phase 2, WP1). It is precise enough to implement and test without making any language decisions. Adapter behaviour (Docs, Sheets, Slides) is named in §7.2 but specified in `PLAN.md` §3–§5. |
| **Precedence** | 1. Owner decisions (PLAN.md header, 2026-10-20). 2. BRIEF Appendix A. 3. The official orthography: *Oʻzbek tilining asosiy imlo qoidalari* (Latin; Cabinet of Ministers resolution No. 339, 24 Aug 1995) and *Ўзбек орфографиясининг асосий қоидалари* (Cyrillic, 1956). 4. Agent decisions, listed in §12. The owner's `translit.js` is a reference only; Appendix A wins wherever they differ (PLAN.md §2). |
| **Location** | PLAN.md WP1 named `crates/uz-translit/RULES.md`. This file is the spec; WP2 makes the crate's `RULES.md` a one-line pointer here. |
| **Rule numbers in the sources** | The official texts are cited by name, not by paragraph. WP3 adds paragraph numbers when it quotes a rule in a golden row, and it does not guess them. |

Notation:
- `→C` means Latin → Cyrillic (`Direction::ToCyrillic`); `→L` means Cyrillic → Latin (`Direction::ToLatin`).
- `ʻ` is always U+02BB (MODIFIER LETTER TURNED COMMA, oʻ/gʻ). `ʼ` is always U+02BC (MODIFIER LETTER APOSTROPHE, tutuq belgisi). Where an input uses another apostrophe, the example names the code point.
- `ў Ў` = U+045E/U+040E, `қ Қ` = U+049B/U+049A, `ғ Ғ` = U+0493/U+0492, `ҳ Ҳ` = U+04B3/U+04B2.

---

## 1. Appendix A → rule map

| Appendix A item | Rule ids |
|---|---|
| Apostrophes: accept every variant; normalise the output | R1 |
| Tutuq belgisi ʼ ↔ ъ (maʼno, sanʼat) | R2, R15 |
| Digraphs sh/ch/ng; s'h → сҳ (Isʼhoq); masʼul ↔ масъул | R3, R4, R2 |
| Ye / е / э (word start, after a vowel, otherwise) | R5 |
| ё ю я ↔ yo yu ya, context-aware | R6 |
| ц ↔ s / ts, with the dictionary | R7, R7a, R11 |
| Letter case (Sh/SH/sh, SHAHAR ↔ ШАҲАР, mixed case) | R10 |
| Exceptions dictionary (sentabr, oktabr, aktyor …) | R11, with R8 (ъ) and R9 (ь) |
| Leave untouched: URLs, e-mail, codes, numbers | R12 |
| Leave untouched: field codes, hyperlink targets, other-language text | §7.2 (adapter level); the language policy is the owner decision D1 |
| Run boundaries | R13 |
| (not in Appendix A) foreign Latin words; quotes; character normalisation; non-Uzbek Cyrillic letters | R14, R15, R16, R17 |

### 1.1 Rule index
| R | Name | Section |
|---|---|---|
| R1 | Apostrophe variants and output forms | §4.1 |
| R2 | Tutuq belgisi ʼ ↔ ъ | §4.2 |
| R3 | Digraphs sh, ch; ng | §4.3 |
| R4 | Separated s + h (sʼh ↔ сҳ) | §4.4 |
| R5 | е / э / ye | §4.5 |
| R6 | ё ю я and йў | §4.6 |
| R7 | ц ↔ s / ts | §4.7 |
| R7a | ц from vowel + ts + i + (y, a, o, e) | §4.7 |
| R8 | Russian ъ and ўъ | §4.8 |
| R9 | ь | §4.9 |
| R9a | ьо → yo | §4.9 |
| R10 | Letter case | §5 |
| R11 | Exceptions dictionary | §6 |
| R12 | Protected spans | §7 |
| R13 | Run boundaries | §8 |
| R14 | Foreign Latin word guard | §4.10 |
| R15 | Quote guard | §4.11 |
| R16 | Character normalisation (combining marks, invisible characters) | §2.4 |
| R17 | Non-Uzbek Cyrillic letters (щ, ы, others) | §3.2 |

### 1.2 PLAN.md §2 conflicts: resolution
| # | Resolution in this spec |
|---|---|
| C1 codes | R12 CODE-A/CODE-B in **both** scripts; codes stay as typed (owner, 2026-10-20). |
| C2 ц | R7 (rule for →L), R7a (a narrow rule for →C), R11 (dictionary for the rest). |
| C3 dictionary | R11, §6.4: 154 loanwords + 4 acronyms + 33 keep entries. aktyor needs no entry (R6). |
| C4 language tags | Owner decision: ru- and en-tagged text **is converted**. Other non-Uzbek tags are skipped by the adapter (PLAN.md D1). An option also skips ru/en. |
| C5 fields, links | §7.2 (adapter level). |
| C6 word-final apostrophe | R2: ъ only between two Latin letters. R15: the quote guard. |
| C7 c, w | R14, on by default. |
| C8 ъ/ь in loanwords | R8, R9 + the dictionary. |
| C9 Roman numerals | R12 ROMAN. |
| C10 йо in loanwords | Dictionary (rayon, mayor, konveyer …). |
| C11 scope | Adapter level (PLAN.md §3–§5). |

### 1.3 Expected disagreements with the owner's plugin (for the WP3 three-way diff)
These examples were checked against an unmodified `translit.js` run under Node on 2026-10-20. Every other rule-only example in §10 agrees with it.

| Input | Ours | Plugin | Rule |
|---|---|---|---|
| koeffitsiyent | коэффициент | коэффитсиент | R7a |
| маъюс | maʼyus | mayus | R8 / D-09 |
| Ш. РАШИДОВ | SH. RASHIDOV | Sh. RASHIDOV | R10 (Single) |
| почтальон | pochtalyon | pochtalon | R9a |

Expected by design, but not listed row by row: C1, C2, C3, C6, C7, C8, C9 and C10 (PLAN.md §2), plus R12 paths and @mentions, R15 quotes, and the dictionary.

---

## 2. Definitions

### 2.1 Character classes
- **Apostrophe set `A`** (input): U+02BB ʻ, U+02BC ʼ, U+2018 ‘, U+2019 ’, U+0027 ', U+0060 `, U+02BF ʿ, U+00B4 ´.
  - U+02BF and U+00B4 are added beyond Appendix A. Both occur in typed Uzbek: ´ comes from keyboard layouts that have no ʻ key, and ʿ comes from Arabic-style transcription. They have no other use in Uzbek prose.
  - Not in `A`: U+2032 ′ (prime: feet, minutes), U+02B9 ʹ, U+201B ‛. They pass through unchanged and act as boundaries.
- **Letter**: a char with `char::is_alphabetic()` that is **not** in `A`.
  - Implementer note: U+02BB, U+02BC and U+02BF are Unicode category Lm, so `is_alphabetic()` returns true for them. The exclusion is required.
- **Latin letter**: ASCII `a–z`, `A–Z`.
- **Foreign Latin letter**: a letter in U+00C0–U+024F or U+1E00–U+1EFF (é ü ö ş ç ñ ß …).
- **Cyrillic letter**: a letter in U+0400–U+052F.
- **Latin vowel `V_L`**: a e i o u and their capitals. The cluster oʻ counts as a vowel.
- **Cyrillic vowel `V_C`**: а е ё и о у ў э ю я ы and their capitals.
- **Cyrillic consonant**: a Cyrillic letter that is not in `V_C` and is not ъ, ь, Ъ or Ь.
- **Hyphens**: U+002D -, U+2010 ‐, U+2011 ‑, U+2012 ‒, U+2013 –, U+2014 —. All of them are boundaries.
- **Transparent `T`**: U+00AD (soft hyphen), U+200C (ZWNJ), U+200D (ZWJ), U+2060 (word joiner), U+FEFF. See R16.
- **Combining marks**: U+0300–U+036F. See R16.
- U+200B (zero-width space) and U+FFFC (inline object) are **boundaries**, not transparent.

### 2.2 Word, boundary, word start
- **Segment**: the text the engine sees in one call. It is either the whole input of `convert`, or a maximal sequence of consecutive `convert: true` runs in `convert_runs` (§8). Segment edges are hard boundaries: no rule looks across them.
- **Word**: a maximal sequence of letters, plus these word-internal characters:
  - (a) an apostrophe from `A` that R1 consumes as part of oʻ/gʻ;
  - (b) an apostrophe from `A` between two Latin letters (R2, R4);
  - (c) transparent characters between two letters;
  - (d) combining marks after a letter.

  Everything else is a **boundary**: whitespace, digits, hyphens, all other punctuation, apostrophes not covered by (a) or (b), U+200B, U+FFFC, protected spans (R12) and segment edges.
  - Consequences: `5-sinf` contains the word `sinf`, `ijtimoiy-iqtisodiy` contains two words, `2024yil` contains the word `yil` (unless R12 protects the token).
- **Word start**: the position of a word's first letter.
- **Previous letter**: the nearest preceding letter in the same word, skipping transparent characters and combining marks. A word's first letter has none.
- **Token**: a maximal run of non-whitespace characters. A **token start** is a position whose previous char is whitespace, a segment edge, or one of `( [ { « " “ „`. R12 uses tokens.

### 2.3 Processing order (both directions)
For each segment:
1. **R16 view.** Build the character view: Cyrillic combining pairs are composed for matching, and transparent characters are skipped for context. The source characters are never altered except as specified.
2. **R12.** Find the protected spans at token starts. They are copied verbatim and act as boundaries.
3. **R15** (→C only). The quote pass classifies each apostrophe that follows o/g.
4. **Segment into words** (§2.2). For each word, in order:
   1. a dictionary `keep` or `acro` entry, matched with exact case (R11) → done;
   2. the foreign guard (R14, →C) → copied verbatim;
   3. a dictionary `word` entry, then the longest `stem` entry (R11);
   4. otherwise the letter rules (§3, §4).
5. **R10.** Case is computed per word on the whole segment text.
6. **R13.** Emit clusters with their source ranges, so that `convert_runs` can split them.

Characters that are not inside a word and not in a protected span (punctuation, digits, spaces, apostrophes used as punctuation) are copied unchanged.

### 2.4 R16: character normalisation
- **Cyrillic combining pairs** are treated as the precomposed letter, as one source cluster: у/У + U+0306 → ў/Ў; и/И + U+0306 → й/Й; е/Е + U+0308 → ё/Ё. The output goes to the base letter's run (R13).
- **Other combining marks** pass through. Each is emitted right after the output of the letter it follows.
- **Transparent characters** do not break words, clusters or context. Examples:
  - `s` U+00AD `h` is still the digraph sh;
  - inside a cluster, the transparent character is emitted **after** the cluster's output (`s`+SHY+`h` → `ш`+SHY);
  - between clusters, it stays where it is.
- **Dictionary lookup skips any word that contains a transparent character**; such a word is converted by the letter rules only. This is a deliberate simplification (it is rare, and it keeps soft-hyphen positions exact).
- No other Unicode normalisation is done. The crate is `no_std` and has no tables; NFC of other text is not our concern.

---

## 3. Character tables

### 3.1 Latin → Cyrillic (`→C`)
Clusters are matched longest first, in the order given in §4.12. Capitals map to the Cyrillic capital (case per R10).

| Latin | Cyrillic | Condition / rule |
|---|---|---|
| a A | а А | |
| b B | б Б | |
| ch Ch CH cH | ч Ч | R3 |
| d D | д Д | |
| e E | е Е **or** э Э | R5 |
| f F | ф Ф | |
| g G | г Г | |
| gʻ Gʻ (any variant in `A`) | ғ Ғ | R1 |
| h H | ҳ Ҳ | |
| i I | и И | |
| j J | ж Ж | |
| k K | к К | |
| l L | л Л | |
| m M | м М | |
| n N | н Н | ng → нг, two letters (R3) |
| o O | о О | |
| oʻ Oʻ (any variant in `A`) | ў Ў | R1, R15 |
| p P | п П | |
| q Q | қ Қ | |
| r R | р Р | |
| s S | с С | |
| sh Sh SH sH | ш Ш | R3 |
| s + apostrophe + h | с + ҳ | R4 (the apostrophe is dropped) |
| t T | т Т | ts → тс; ц only by R7a or R11 |
| u U | у У | |
| v V | в В | |
| x X | х Х | |
| y Y | й Й | ye → е, yo → ё, yu → ю, ya → я; y before oʻ → й (R6) |
| z Z | з З | |
| apostrophe between two Latin letters | ъ Ъ | R2 |
| c (not in ch), w, foreign Latin letters | (word untouched) | R14. With the guard off: c → к, w → в, foreign letters unchanged. |

### 3.2 Cyrillic → Latin (`→L`)
| Cyrillic | Latin | Condition / rule |
|---|---|---|
| а А | a A | |
| б Б | b B | |
| в В | v V | |
| г Г | g G | |
| ғ Ғ | gʻ Gʻ | R1 |
| д Д | d D | |
| е Е | ye **or** e | R5 |
| ё Ё | yo | R6 (always) |
| ж Ж | j J | |
| з З | z Z | |
| и И | i I | |
| й Й | y Y | |
| к К | k K | |
| қ Қ | q Q | |
| л Л | l L | |
| м М | m M | |
| н Н | n N | |
| о О | o O | |
| п П | p P | |
| р Р | r R | |
| с С | s S, **or** sʼ before ҳ/Ҳ | R4 |
| т Т | t T | |
| у У | u U | |
| ў Ў | oʻ Oʻ | R1 |
| ф Ф | f F | |
| х Х | x X | |
| ҳ Ҳ | h H | |
| ц Ц | ts **or** s | R7 |
| ч Ч | ch | R3 |
| ш Ш | sh | R3 |
| ъ Ъ | ʼ **or** nothing | R8 |
| ь Ь | nothing, **or** y before о/О | R9, R9a |
| э Э | e E | R5 |
| ю Ю | yu | R6 (always) |
| я Я | ya | R6 (always) |
| **R17** щ Щ | sh | not Uzbek; Russian names and loans (щётка → shyotka) |
| **R17** ы Ы | i I | not Uzbek |
| **R17** every other Cyrillic letter (і ї є ґ ә ө ү ұ ң һ …) | unchanged | not Uzbek; passed through, and they count as letters for word boundaries |

Multi-character outputs take their case from R10. A Latin letter inside Cyrillic text is never changed by `→L`, and a Cyrillic letter inside Latin text is never changed by `→C`.

---

## 4. Letter rules

### 4.1 R1: apostrophe variants
- **→C.** A Latin `o`/`O` or `g`/`G` immediately followed by **any** apostrophe in `A` forms one cluster, which gives ў/Ў or ғ/Ғ.
  - This holds before a vowel, before a consonant and at the end of a word (togʻ, bogʻ, yogʻ).
  - The one exception is R15 (the apostrophe closes a quotation).
- **Two apostrophes after o/g** (`bog''`): the first belongs to the letter, and the second is ordinary punctuation.
- **Never tutuq after o/g.** An apostrophe after o/g is never read as a tutuq. The 1995 Latin spelling writes oʻ in every word with Cyrillic ўъ (moʻjiza), and Uzbek has no гъ/оъ. The "g' before a vowel" case (`g'alaba`) is therefore always ғ: ғалаба.
- **→L output forms.** ў → `oʻ` and ғ → `gʻ` use U+02BB. The tutuq ъ → `ʼ` and the R4 separator use U+02BC.
  - With `Options.apostrophe = Ascii`, all three are U+0027 instead.
- **→L leaves existing Latin alone.** Latin text already present in the input keeps its apostrophes; →L does not normalise text it does not convert.
- Source: Appendix A; the 1995 alphabet (Oʻ oʻ, Gʻ gʻ, and the tutuq belgisi ʼ as separate signs). The choice of U+02BB/U+02BC follows the plugin and current official practice.

### 4.2 R2: tutuq belgisi
- **→C.** An apostrophe from `A` gives ъ when **all** of these hold:
  - its previous char is a Latin letter other than o/g/O/G (after o/g, R1 applies);
  - its next char is a Latin letter;
  - it is not an R4 separator (s + apostrophe + h).

  ъ is upper case (Ъ) when both neighbouring letters are upper case.
- **Every other apostrophe is punctuation** and is copied unchanged: at a word start, at a word end, next to a digit, or between non-letters.
  - This fixes plugin bug C6 (`'olma'` → `'олма'`, not `'олмаъ`).
- Examples:
  - maʼno → маъно; san'at (U+0027) → санъат; mas’ul (U+2019) → масъул;
  - taʼlim → таълим; eʼlon → эълон; sheʼr → шеър;
  - qatʼiy → қатъий; maʼyus → маъюс; MAʼNO → МАЪНО.
- **→L.** ъ → ʼ, except in the R8 cases.
- Source: Appendix A; the 1995 rules (the tutuq belgisi corresponds to Cyrillic ъ in Arabic loans).

### 4.3 R3: digraphs sh, ch, and ng
- **→C.** `s`+`h` → ш; `c`+`h` → ч. Each is one cluster. The case of the output is the case of the **first** letter (R10): Sh, SH → Ш; sH → ш.
- **→L.** ш → sh and ч → ch. For upper case, R10 decides SH or Sh.
- **ng: decision D-05.** The engine **never** treats ng as a unit, in either direction.
  - Cyrillic writes нг as two letters, and Latin writes ng as two letters. The mapping is n ↔ н, g ↔ г, letter by letter.
  - Case therefore transfers per letter (NG ↔ НГ, Ng ↔ Нг).
  - ngʻ is н + ғ (R1 consumes the gʻ), so no ambiguity arises.
  - The 1995 rules call ng a "letter combination" (harf birikmasi) for one sound. That only matters for hyphenation and alphabetical sorting, which are outside this crate.
  - Uzbek has no separator for n + g, unlike s'h.
- Examples: yangi → янги; tong → тонг; ming → минг; NG at the end of an all-caps word → НГ; ТОНГ → TONG.

### 4.4 R4: separated s + h
- **→C.** `s` + one apostrophe from `A` + `h` (case-insensitive) → с + ҳ.
  - The cluster is (s, apostrophe) → с; the apostrophe produces no output. `h` is then its own cluster (ҳ).
  - The apostrophe is a separator, so it does **not** produce ъ: Cyrillic does not write ъ here.
- **→L.** с immediately followed by ҳ/Ҳ (no intervening character other than transparent ones) → `sʼ`; then ҳ → `h`.
- Examples: Isʼhoq → Исҳоқ; Is'hoq → Исҳоқ; Asʼhob → Асҳоб; IS'HOQ → ИСҲОҚ; Исҳоқ → Isʼhoq.
- **Known input error.** A Latin `Ishoq` (written without the separator) gives Ишоқ. This is not corrected: it could equally be a real ш.
- Source: Appendix A; the 1995 rules (the tutuq belgisi separates s and h when they are pronounced separately).

### 4.5 R5: е, э, ye
**→C.** The order matters:
1. `y`/`Y` + `e`/`E` → е/Е (one cluster), in any position. Examples: yer → ер, qayer → қаер, Yevropa → Европа, kliyent → клиент.
2. A lone `e`/`E` → **э/Э** if any of these holds:
   - (a) it is at a word start (after a space, a hyphen, a digit or any boundary);
   - (b) its previous letter is a vowel in `V_L` (a e i o u, including the o of oʻ);
   - (c) it directly follows a tutuq apostrophe (R2), giving ъэ.
3. Otherwise → е/Е.

Examples:
- ekin → экин; el → эл; erkak → эркак;
- poeziya → поэзия; aeroport → аэропорт; koeffitsiyent → коэффициент;
- sotsial-ekonomik → социал-экономик; 3-etaj → 3-этаж;
- kel → кел; Toshkent → Тошкент.

**→L.**
1. е/Е → `ye` if any of these holds:
   - (a) it is at a word start;
   - (b) its previous letter is in `V_C`;
   - (c) its previous letter is ъ/Ъ or ь/Ь.
2. Otherwise → `e`.
3. э/Э → `e` always.

Examples: ер → yer; қаер → qayer; Европа → Yevropa; объект → obyekt; пьеса → pyesa; кел → kel; эл → el; поэзия → poeziya.

**Compounds written solid (decision D-07).** The rules do not detect compound boundaries.
- A Cyrillic э after a consonant inside a solid compound (Ўзбекэнерго) gives Latin `e`, and that comes back as е.
- Such words round-trip only through a dictionary entry; none are seeded, because the known cases are proper names.
- Hyphenated compounds work by rule, because the hyphen is a boundary.

**No exceptions to the Latin word-start rule are needed.** Every Cyrillic word that starts with е is written ye in Latin. The dictionary may still override a word if one is found.

Source: Appendix A; the 1995 rules (ye for Cyrillic е at word start and after vowels).

### 4.6 R6: ё, ю, я and йў
**→C.**
- `y` + `o` where that o is followed by an R1 apostrophe (the oʻ cluster) → й. The oʻ then gives ў.
  - Examples: yoʻq → йўқ; yoʻl → йўл; boyoʻgʻli → бойўғли.
- Otherwise `y` + `o` / `u` / `a` → ё / ю / я (one cluster), in any position, after a consonant too.
  - Examples: dunyo → дунё; aktyor → актёр; tayyor → тайёр; yangi → янги; Yusuf → Юсуф; iyun → июн(ь) (ь via R11).
- Other `y` → й. Examples: choy → чой; qiyin → қийин; seyf → сейф.

**→L.** ё → yo, ю → yu, я → ya **everywhere**: at a word start, after vowels, after consonants, after ъ/ь.
- Examples: ёз → yoz; дунё → dunyo; актёр → aktyor; объём → obyom; компьютер → kompyuter.
- й → y. So йў → yoʻ, and that round-trips through the first →C bullet.

**Loanwords with Cyrillic й + vowel** (район, майор, конвейер, фойе) give Latin `yo`/`ye` sequences. →C reads these back as ё/е, so they need dictionary entries (§6.4, group F).

Source: Appendix A; the 1995 rules (yo yu ya as letter combinations).

### 4.7 R7 and R7a: ц
**→L (R7), fully by rule.**
- ц/Ц → `ts` if its previous letter is in `V_C`.
- Otherwise (at a word start, after a consonant, after ъ/ь) → `s`.
- Examples:
  - after a vowel: операция → operatsiya; лицей → litsey; абзац → abzats;
  - at a word start: цирк → sirk;
  - after a consonant: концерт → konsert; функция → funksiya;
  - after a consonant or ь: кальций → kalsiy; сцена → ssena.
- Source: the 1995 rules (s at a word start and after consonants, ts after vowels) and Appendix A. The plugin agrees.

**→C (R7, default).** `ts` → тс and `s` → с. Native Uzbek words have т+с across morpheme boundaries (ketsa → кетса, otsiz → отсиз, aytsin → айтсин), so ц is never produced by default.

**→C (R7a, decision D-08).** `t`+`s` → ц/Ц (one cluster) when **both** hold:
- its previous letter is in `V_L`;
- the next two letters are `i` followed by one of `y a o e`.

This covers the productive Russian suffix family -tsiya / -tsion / -tsial / -tsiyent: operatsiya, konstitutsiya, investitsiya, militsiya, sotsial, ratsion, patsiyent, koeffitsiyent, initsiativa, ofitsiant.
- No native Uzbek word or suffix has vowel + t + s + i + (y | a | o | e). The native -tsin and -tsiz forms are followed by n and z.
- It round-trips: R7 gives ts after a vowel.
- Cases not covered (ts + e, ts + a, initial or post-consonant ц) need the dictionary (R11, §6.4, groups G–I).

### 4.8 R8: Russian ъ, and ўъ
**→L.**
1. ъ/Ъ after ў/Ў → nothing. Example: мўъжиза → moʻjiza. The 1995 Latin spelling writes oʻ without a tutuq.
2. ъ/Ъ whose previous letter is a **Cyrillic consonant** and whose next letter is е ё ю я (any case) → nothing. The following vowel already starts with y.
   - Examples: объект → obyekt; субъект → subyekt; съезд → syezd; конъюнктура → konyunktura.
3. Otherwise ъ → ʼ (R2). This includes ъ **after a vowel** before an iotated vowel (маъюс → maʼyus), because Uzbek tutuq words have a vowel there, while the Russian separating ъ always follows a consonant (a prefix).

**→C.** By rule, ъ is produced only from a tutuq apostrophe (R2). Russian ъ and the ъ of ўъ are restored only by the dictionary (§6.4, groups D and E). Without an entry: obyekt → обект, moʻjiza → мўжиза.

**Decision D-09.** Rule 2 assumes that no Uzbek word has consonant + tutuq + е/ё/ю/я. None is known. A counter-example would lose its ʼ in →L.

### 4.9 R9 and R9a: ь
**→L.**
- **R9a:** ь/Ь immediately before о/О → `y`.
  - Examples: почтальон → pochtalyon; батальон → batalyon; павильон → pavilyon.
  - This sequence occurs only in Russian loans, and the Latin y keeps the sound. Decision D-10.
- Otherwise ь/Ь → nothing. Its effect on a following е is R5 (пьеса → pyesa).
  - Examples: фильм → film; апрель → aprel; компьютер → kompyuter.

**→C.** ь is never produced by rule. It comes only from the dictionary (§6.4, groups B and C).

**The ь of a stem before a suffix (decision D-11).** In Cyrillic, a loanword that ends in ь normally drops the ь before an Uzbek suffix, for example 1 январдан, сентябрда.
- `word` entries apply to the bare word only, so the inflected form falls back to the rules (yanvardan → январдан), which already gives the ь-less form.
- `stem` entries drop a final ь whenever a remainder follows (§6.3).
- →L ignores ь either way, so сентябрьда and сентябрда both give sentabrda.

### 4.10 R14: foreign Latin word guard (→C, `Options.foreign_latin_guard`, default on)
- A **word** (§2.2) is copied verbatim if it contains any of:
  - `c`/`C` not immediately followed by `h`/`H`;
  - `w`/`W`;
  - any foreign Latin letter.
- Uzbek Latin has no c outside ch, no w, and no diacritic letters, so such a word is not Uzbek.
- Examples: Microsoft, Excel, Word, Windows, McDonald's, Müller, Facebook.
- **Not caught:** foreign words made only of Uzbek letters (Google, Zoom, YouTube, iPhone). Common ones are `keep` entries (§6.4, group K); the rest are converted.
- Hyphen parts are separate words: in `e-mail`, `e` → э and `mail` → маил. Known limitation; Uzbek writes `elektron pochta`.
- With the guard off, the plugin mapping applies: c → к, w → в, and foreign letters stay unchanged.

### 4.11 R15: quote guard (→C)
An apostrophe used as a single quotation mark must not turn the o/g before it into ў/ғ, and must never become ъ. R2 already handles ъ. For o/g, a pass over the segment runs before conversion:

1. **Opener:** an apostrophe that is U+0027, U+0060 or U+2018, is at a token start, and is followed by a letter.
2. **Closer candidate:** an apostrophe that is U+0027, U+0060 or U+2019, is preceded by a Latin letter, and is followed by a non-letter or the segment end.
   - U+2018, U+02BB, U+02BC, U+02BF and U+00B4 are **never** closers. A closing single quote is typographically U+2019, and many Uzbek sites write oʻ with U+2018.
3. **Matching.** For each opener, take the closer candidates after it, up to the next opener or the segment end:
   - **None:** nothing happens.
   - **At least one candidate is not preceded by o/g/O/G:** the first such candidate closes the quote. Every o/g-final candidate before it stays a letter (ў/ғ).
   - **All candidates are preceded by o/g:** the **last** of them closes the quote. Its apostrophe is copied unchanged, and its o/g converts as plain о/г. The others stay ў/ғ.
4. **Not involved:** an apostrophe after o/g that is followed by a letter (koʻcha) is always R1, and the guard does not consider it.

Examples:
- `‘radio’ dedi` → `‘радио’ деди`;
- `‘Bog’ ko’chasi’` → `‘Боғ кўчаси’`;
- `Men 'yo'q' dedim` → `Мен 'йўқ' дедим`.

Without an opener, a word-final o/g apostrophe is always the letter: `tog'` → тоғ.

**Genuinely ambiguous (listed as `ambiguous` in the golden list):** `'bog'` with U+0027 throughout gives `'бог'`. The rule reads the last apostrophe as the closing quote. Input with U+02BB is never ambiguous.

### 4.12 →C cluster order (implementation contract)
At each letter position, the first matching item wins:
1. `o`/`O` + an R1 apostrophe that R15 did not mark as a closer → ў. Consumes 2.
2. `g`/`G` + an R1 apostrophe that R15 did not mark as a closer → ғ. Consumes 2.
3. `s` + apostrophe + `h` → с. Consumes 2; the h is handled next.
4. `s` + `h` → ш. Consumes 2.
5. `c` + `h` → ч. Consumes 2.
6. `y` + (`o` + an R1 apostrophe) → й. Consumes 1.
7. `y` + `o` / `u` / `a` / `e` → ё / ю / я / е. Consumes 2.
8. `t` + `s` under R7a → ц. Consumes 2.
9. `e` → э or е (R5).
10. An apostrophe between Latin letters → ъ (R2).
11. A single letter → the table in §3.1.

All comparisons are case-insensitive. The case of the output comes from R10. In a stem remainder (§6.3), "previous letter" includes the stem's last source letter.

---

## 5. R10: letter case

### 5.1 Word classes
These are computed on the letters of the word (§2.2) in the source, over the whole segment, so a word split across runs is classified as one word.
- **lower:** no letter is upper case.
- **UPPER:** at least 2 letters, and every cased letter is upper case.
- **Title:** the first letter is upper case and every other letter is lower case.
- **Mixed:** anything else (OʻzR, ToshDU, MChJ, SHahar).
- **Single:** exactly 1 letter. It is treated as UPPER if it is upper case and in an **all-caps context**; otherwise as Title (upper case) or lower.
  - All-caps context: the nearest preceding word or the nearest following word in the segment has class UPPER.

### 5.2 →L (one Cyrillic letter → 0, 1 or 2 Latin characters)
- A lower-case source letter gives lower-case output.
- An upper-case source letter whose output has 2 or more characters (Ш Ч Ё Ю Я Ў Ғ, Е → Ye, Ц → Ts, С → Sʼ) is **all upper** (SH, YO, OʻZ…; the apostrophe has no case) when any of these holds:
  - (a) the next letter of the word is upper case;
  - (b) it is the last letter of the word and the previous letter is upper case;
  - (c) the word is Single in an all-caps context.

  Otherwise only the first output character is upper case (Sh, Yo, Oʻ).
- This one rule gives the expected result for every class:
  - Title Шаҳар → Shahar;
  - UPPER ШАҲАР → SHAHAR and АҚШ → AQSH;
  - Mixed ТошДУ → ToshDU and ЎзР → OʻzR;
  - Single Ш. Рашидов → Sh. Rashidov and Ш. РАШИДОВ → SH. RASHIDOV.
- **The "SH" vs "Sh" question at a word start:** an upper-case Ш at the start of an UPPER word gives SH (ШАҲАР → SHAHAR). At the start of a Title word inside an all-caps line (ТОШКЕНТ Шаҳри), it gives Sh, because that word's next letter is lower case.

### 5.3 →C (1 or 2 Latin characters → 1 Cyrillic letter)
- The output is upper case iff the **first source letter of the cluster** is upper case: Sh, SH → Ш; sH → ш; Yo → Ё; Oʻ → Ў.
- Tutuq ъ is upper case (Ъ) iff both neighbouring letters are upper case.
- The class does not matter for →C, except for dictionary output (§5.4).

### 5.4 Case transfer for dictionary output
The output is the entry's target form, stored in lower case. It is re-cased by the source word's class:
- **lower:** unchanged.
- **Title:** the first character upper case.
- **UPPER**, or **Single** in an all-caps context: all upper case.
- **Mixed:** per output character. Output character j gets the case of the source character k whose proportional range contains j (§8.3).

`acro` and `keep` entries are matched with exact case and emitted literally (§6.2).

### 5.5 Not guaranteed
Mixed-case words do not always round-trip. For example, SHahar →C Шаҳар →L Shahar. §9 excludes them from the guarantee.

---

## 6. R11: exceptions dictionary

### 6.1 File and format
- File: `crates/uz-translit/data/exceptions.tsv`. UTF-8, LF, tab-separated, with a header row. Lines starting with `#` are comments.
- Columns: `latin  cyrillic  match  dir  conf  status  source  note`. (This adds `conf` to PLAN.md §9 WP3.)
  - **`latin`, `cyrillic`:** lower case, except in `acro` and `keep` rows. In Latin, oʻ/gʻ use U+02BB and the tutuq uses U+02BC. A `stem` row holds the stem, not necessarily a whole word (`funksi` / `функци`).
  - **`match`:** one of
    - `word`: the whole word only;
    - `stem`: the word, or the stem followed by any remainder;
    - `acro`: an acronym, matched with exact case, both directions;
    - `keep`: Latin, matched with exact case, copied verbatim in →C; `cyrillic` = `-`.
  - **`dir`:** one of
    - `both`: keyed in both tables;
    - `l2c`: Latin key only (→L already gives `latin` by rule; the test asserts it);
    - `c2l`: Cyrillic key only;
    - `variant`: Latin key only, for a non-standard input spelling. →L never produces it, and it is excluded from round-trip tests.
  - **`conf`:** `high | medium | low`.
  - **`status`:** `candidate | reviewed:agent-<model>:<date> | ambiguous | rejected`.
  - **`source`:** `rules.md` for the seed in §6.4, or as in WP3.
- Seed rows from §6.4 enter the file with `status = reviewed:agent-opus:2026-10-20` when conf is high or medium, and `ambiguous` when conf is low. They still go through the WP3 three-way diff and the corpus grep, and any disagreement demotes a row to `ambiguous`.
- `build.rs` compiles only `reviewed:*` rows.

### 6.2 Lookup
The lookup unit is a **word** (§2.2). Hyphen parts are looked up separately.

**Key normalisation:**
- lower-case with `char::to_lowercase`;
- in Latin, the apostrophe after o/g → U+02BB, and an apostrophe between letters → U+02BC;
- in Cyrillic, the combining pairs are composed (R16).

**Order, first hit wins:**
1. `keep` (→C only) and `acro`: an exact-case comparison of the word against the source column.
2. R14 guard (→C).
3. `word`: the normalised word equals the key.
4. `stem`: the **longest** key that is a prefix of the normalised word. The remainder may be empty.
   - **c2l keys** for a stem row whose Cyrillic ends in ь: both the full form (`сентябрь`) and the form without ь (`сентябр`).

**Minimum stem length.** A stem key under 5 letters needs an explicit "no false hit" note in `note`. WP3 greps the catalog corpus and the golden list for words starting with it. Seeded examples: `film`, `rubl`, `pult`, `stil`, `sikl`, `sex`, `volt`, `otel`.

### 6.3 Output for a stem hit
- **Stem part:**
  - in →C, with a non-empty remainder and a Cyrillic stem ending in ь, the stem is emitted **without** its ь (D-11);
  - otherwise the Cyrillic (→C) or Latin (→L) column is emitted as written.
- **Remainder:** converted by the letter rules, with the stem's last source letter as the previous-letter context for R5, R7, R7a and R8.
  - vaksina|tsiya → вакцина + ция, because R7a sees the a.
  - funksi|yasi → функци + яси.
- **Case:** the whole word's class is applied (§5.4). The remainder takes per-cluster case as usual.
- **Runs:** the stem part is split proportionally; the remainder is split by clusters (§8).
- A remainder is never looked up again in the dictionary.

### 6.4 Initial list (154 loanwords, 4 acronyms, 33 keep entries)
All `dir` = `l2c` unless stated. For `l2c` rows, →L of the Cyrillic by rule gives the Latin exactly; WP2's tests assert this for every row.

**A. Months (11)**
| # | latin | cyrillic | match | dir | conf | note |
|---|---|---|---|---|---|---|
| 1 | yanvar | январь | word | l2c | high | inflected forms by rule: yanvardan → январдан (D-11) |
| 2 | fevral | февраль | word | l2c | high | |
| 3 | aprel | апрель | word | l2c | high | |
| 4 | iyun | июнь | word | l2c | high | |
| 5 | iyul | июль | word | l2c | high | |
| 6 | sentabr | сентябрь | stem | both | high | →L by rule would give "sentyabr"; the official Latin is sentabr |
| 7 | oktabr | октябрь | stem | both | high | as above |
| 8 | noyabr | ноябрь | word | l2c | high | |
| 9 | dekabr | декабрь | word | l2c | high | |
| 10 | sentyabr | сентябрь | stem | variant | medium | common non-standard Latin |
| 11 | oktyabr | октябрь | stem | variant | medium | common non-standard Latin |

**B. ь inside or at the end of the word (40)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 12 | kompyuter | компьютер | stem | high | |
| 13 | film | фильм | stem | high | 4 letters; no native word starts with film |
| 14 | rol | роль | word | high | roli → роли by rule |
| 15 | nol | ноль | word | high | |
| 16 | parol | пароль | stem | high | |
| 17 | avtomobil | автомобиль | stem | high | |
| 18 | mebel | мебель | stem | high | |
| 19 | model | модель | stem | medium | modellashtirish → моделлаштириш |
| 20 | portfel | портфель | stem | high | |
| 21 | spektakl | спектакль | stem | high | |
| 22 | rubl | рубль | stem | high | 4 letters |
| 23 | dizel | дизель | stem | high | |
| 24 | albom | альбом | stem | high | |
| 25 | medal | медаль | stem | high | |
| 26 | detal | деталь | stem | medium | |
| 27 | shtempel | штемпель | stem | high | |
| 28 | kalkulyator | калькулятор | stem | high | |
| 29 | kalkulyats | калькуляц | stem | high | kalkulyatsiya → калькуляция (ь + ц) |
| 30 | fakultet | факультет | stem | high | |
| 31 | apelsin | апельсин | stem | high | |
| 32 | asfalt | асфальт | stem | high | |
| 33 | gospital | госпиталь | stem | medium | |
| 34 | kontrol | контроль | stem | medium | kontrolyor → контролёр |
| 35 | pult | пульт | stem | high | 4 letters |
| 36 | volt | вольт | stem | medium | 4 letters; voltmetr → вольтметр |
| 37 | tabel | табель | stem | high | HR timesheet |
| 38 | profil | профиль | stem | high | |
| 39 | stil | стиль | stem | medium | 4 letters; stilistika → стилистика (D-11 drops the ь) |
| 40 | modul | модуль | stem | high | |
| 41 | otel | отель | stem | medium | 4 letters |
| 42 | spiral | спираль | stem | medium | |
| 43 | dvigatel | двигатель | stem | medium | |
| 44 | ansambl | ансамбль | stem | medium | |
| 45 | sekretar | секретарь | stem | medium | sekretariat → секретариат (ь dropped) |
| 46 | kalendar | календарь | stem | medium | |
| 47 | lager | лагерь | word | medium | |
| 48 | konsult | консульт | stem | high | konsultant, konsultatsiya (R7a in the remainder) |
| 49 | devalv | девальв | stem | medium | devalvatsiya |
| 50 | alyans | альянс | stem | medium | |
| 51 | polsha | польша | word | medium | proper name, Title case by R10 |

**C. ь before a vowel: ье, ья, ью, ьо (11)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 52 | premyer | премьер | stem | high | premyer-ministr: hyphen parts |
| 53 | karyera | карьера | stem | high | HR |
| 54 | intervyu | интервью | stem | high | |
| 55 | pyesa | пьеса | stem | high | |
| 56 | atelye | ателье | word | medium | |
| 57 | pochtalyon | почтальон | stem | medium | R9a |
| 58 | batalyon | батальон | stem | medium | R9a |
| 59 | pavilyon | павильон | stem | medium | R9a |
| 60 | bilyard | бильярд | stem | low | |
| 61 | kalsiy | кальций | word | medium | ь + ц |
| 62 | selsiy | цельсий | word | medium | initial ц + ь |

**D. Russian ъ (8)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 63 | obyekt | объект | stem | high | obyektiv, obyektlar |
| 64 | subyekt | субъект | stem | high | |
| 65 | inyeksi | инъекци | stem | high | inyeksiya (ъ + ц) |
| 66 | syezd | съезд | stem | high | |
| 67 | podyezd | подъезд | stem | high | |
| 68 | konyunktura | конъюнктура | stem | high | |
| 69 | adyutant | адъютант | stem | medium | |
| 70 | obyom | объём | stem | medium | |

**E. ўъ (5)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 71 | moʻjiza | мўъжиза | stem | high | moʻjizaviy |
| 72 | moʻtabar | мўътабар | stem | high | |
| 73 | moʻtadil | мўътадил | stem | high | |
| 74 | moʻjaz | мўъжаз | stem | medium | |
| 75 | moʻmin | мўъмин | stem | medium | |

**F. Cyrillic й + vowel, and э after a consonant (7)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 76 | rayon | район | stem | high | not ёд/йод: `yod` is deliberately **absent** (yod = ёд, "memory") |
| 77 | mayor | майор | stem | high | military rank (HR) |
| 78 | konveyer | конвейер | stem | high | |
| 79 | foye | фойе | word | medium | |
| 80 | york | йорк | word | medium | Nyu-York → Ню-Йорк |
| 81 | mayonez | майонез | stem | low | |
| 82 | mer | мэр | word | medium | the only seeded э-after-consonant word |

**G. Initial ц (Latin s) (14)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 83 | sirk | цирк | word | high | **word only**: sirka = сирка (vinegar) |
| 84 | sex | цех | stem | medium | 3 letters; checked: "magic" is sehr, so no native word starts with sex |
| 85 | sement | цемент | stem | high | |
| 86 | sentr | центр | stem | high | sentral → централ |
| 87 | sentner | центнер | stem | medium | |
| 88 | sitata | цитата | stem | high | |
| 89 | sikl | цикл | stem | medium | 4 letters; siklon → циклон |
| 90 | silindr | цилиндр | stem | high | |
| 91 | sivilizats | цивилизац | stem | high | sivilizatsiya, sivilizatsion |
| 92 | senzura | цензура | stem | high | |
| 93 | sisterna | цистерна | stem | medium | |
| 94 | sirkulyar | циркуляр | stem | medium | longer than `sirk`, which is word-only |
| 95 | sink | цинк | word | low | |
| 96 | ssena | сцена | stem | high | covers ssenariy → сценарий |

**H. ц after a consonant (Latin s) (46)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 97 | funksi | функци | stem | high | funksiya, funksional |
| 98 | aksiya | акция | stem | high | **not** `aksi` (aksincha = аксинча); aksiyador |
| 99 | aksiz | акциз | stem | high | excise |
| 100 | seksiya | секция | stem | high | |
| 101 | inspeksi | инспекци | stem | high | inspektor is unaffected |
| 102 | konsert | концерт | stem | high | |
| 103 | konsern | концерн | stem | medium | |
| 104 | konsepsi | концепци | stem | high | |
| 105 | konsept | концепт | stem | high | konseptual |
| 106 | prinsip | принцип | stem | high | prinsipial |
| 107 | korrupsi | коррупци | stem | high | anti-corruption HR documents |
| 108 | stansi | станци | stem | high | |
| 109 | kvitansi | квитанци | stem | high | |
| 110 | instansi | инстанци | stem | high | |
| 111 | distansi | дистанци | stem | high | distansion taʼlim |
| 112 | tendensi | тенденци | stem | high | |
| 113 | kompetensi | компетенци | stem | high | HR |
| 114 | konferensi | конференци | stem | high | |
| 115 | preferensi | преференци | stem | high | |
| 116 | konvensi | конвенци | stem | high | ILO conventions |
| 117 | intervensi | интервенци | stem | medium | |
| 118 | konkurensi | конкуренци | stem | high | |
| 119 | potensial | потенциал | stem | high | |
| 120 | sanksi | санкци | stem | high | |
| 121 | fraksi | фракци | stem | high | |
| 122 | reaksi | реакци | stem | high | |
| 123 | kolleksi | коллекци | stem | high | |
| 124 | leksiya | лекция | stem | medium | **not** `leksi` (leksika = лексика) |
| 125 | direksiya | дирекция | stem | high | |
| 126 | korreksi | коррекци | stem | medium | |
| 127 | proyeksi | проекци | stem | medium | |
| 128 | konstruksi | конструкци | stem | high | konstruktor is unaffected |
| 129 | rekonstruksi | реконструкци | stem | high | |
| 130 | instruksi | инструкци | stem | high | |
| 131 | produksi | продукци | stem | high | |
| 132 | redaksi | редакци | stem | high | |
| 133 | tranzaksi | транзакци | stem | high | |
| 134 | infeksi | инфекци | stem | high | |
| 135 | vaksina | вакцина | stem | high | vaksinatsiya (R7a in the remainder) |
| 136 | konsess | концесс | stem | medium | konsessiya |
| 137 | fransi | франци | stem | high | Fransiya |
| 138 | fransuz | француз | stem | high | |
| 139 | shveysar | швейцар | stem | medium | Shveysariya |
| 140 | konsentr | концентр | stem | medium | konsentratsiya |
| 141 | aksent | акцент | stem | medium | |
| 142 | opsiya | опция | stem | medium | |

**I. ц after a vowel that R7a does not cover (Latin ts) (13)**
| # | latin | cyrillic | match | conf | note |
|---|---|---|---|---|---|
| 143 | litsenz | лиценз | stem | high | litsenziya, litsenziat |
| 144 | litsey | лицей | stem | high | |
| 145 | abzats | абзац | stem | high | legal documents |
| 146 | ofitser | офицер | stem | high | |
| 147 | dotsent | доцент | stem | high | academic rank (HR) |
| 148 | protsent | процент | stem | high | |
| 149 | protsess | процесс | stem | high | |
| 150 | protsedur | процедур | stem | high | |
| 151 | retsept | рецепт | stem | high | |
| 152 | retsenz | реценз | stem | high | retsenziya, retsenzent |
| 153 | spetsifik | специфик | stem | medium | |
| 154 | matritsa | матрица | stem | medium | |

> Count: 154 entries (11 + 40 + 11 + 8 + 5 + 7 + 14 + 46 + 12). WP3 appends new rows from lex.uz after row 154.

**J. Acronyms (`acro`, `dir` = both, exact case, literal output) (4)**
| # | latin | cyrillic | conf | note |
|---|---|---|---|---|
| A1 | MChJ | МЧЖ | high | →L by rule would give MCHJ |
| A2 | YaTT | ЯТТ | high | |
| A3 | JShShIR | ЖШШИР | high | personal ID number |
| A4 | YaIM | ЯИМ | medium | GDP |

Other Uzbek acronyms convert by rule (BMT ↔ БМТ, AQSH ↔ АҚШ, OʻzR ↔ ЎзР, ToshDU ↔ ТошДУ, IIV ↔ ИИВ). Decision D-14: an UPPER acronym gives SH/CH/YA in →L (AQSH, not AQSh).

**K. Keep (`keep`, →C only, exact case, copied verbatim) (33)**

PDF, USB, ID, HR, URL, HTML, XML, JSON, IBAN, IELTS, TOEFL, PhD, USD, EUR, UZS, Google, YouTube, iPhone, Gmail are conf **high**.

IT, API, IP, VPN, GPS, SMS, ISO, RUB, Apple, Zoom, Skype, Outlook, Linux are conf **medium**.

`QR` is conf **low**, status `ambiguous`: QR-kod vs ҚР = Qoraqalpogʻiston Respublikasi. It is not compiled.

Exact case means that lower-case `it` (dog, ит) and `id` still convert. Words with c or w (Excel, Word, CD, kW) need no entry because of R14.

---

## 7. R12: protected spans

Spans are found at **token starts** (§2.2) before word segmentation. They are copied byte for byte and act as boundaries. Patterns are case-insensitive unless stated.

| Span | →C | →L | Definition | Reason |
|---|---|---|---|---|
| **URL** | keep | keep | `http://`, `https://`, `ftp://`, `file://`, `mailto:` or `www.`, up to whitespace or one of `< > " « »`. Trailing `. , ; : ! ? ' " » ” ’ ) ] }` is trimmed, but a `)` is kept if the URL contains an unmatched `(`. | Appendix A. Changing a URL breaks it. |
| **E-mail** | keep | keep | `[A-Za-z0-9._%+-]+@` + labels `[A-Za-z0-9-]+` joined by `.`, at least 2 labels, with the same trailing trim. | Appendix A. |
| **Bare domain** | keep | keep | At least 2 labels `[A-Za-z0-9-]+` joined by `.`. The last label is one of `uz ru com org net gov edu info io kz tj kg tm de uk us eu int`, it is not followed by a letter or digit, and an optional `/…` path runs to whitespace. Examples: lex.uz, my.gov.uz, IJRO.GOV.UZ. | The plugin's list plus the region and common TLDs. A domain is an address. |
| **@mention** | keep | keep | `@` at a token start + `[A-Za-z0-9_]+`. | Telegram handles are identifiers. |
| **File path** | keep | keep | Any of: (a) `[A-Za-z]:\` or `[A-Za-z]:/`; (b) `\\` at the token start; (c) a token containing `\`; (d) a token starting with `/`, `./`, `../` or `~/` that contains another `/` or a `.ext`; (e) `name.ext` with `name` = `[A-Za-z0-9_.-]+` and `ext` in `doc docx xls xlsx ppt pptx pdf txt csv odt ods odp rtf jpg jpeg png gif zip rar 7z exe msi html htm xml json`. Each runs to whitespace. | Paths and file names are identifiers. `va/yoki` and `12/05/2024` are not paths under (a)–(e), so they convert. **Limit:** a path with spaces is cut at the first space. |
| **CODE-A** | keep | keep | At a token start: 1–4 **upper-case** letters of either script, where Latin Oʻ/Gʻ with any `A` apostrophe counts as one letter; then optionally one of `- – ‑ /`; then 1 or more digits; then any further groups of `[-/.]` + digits. The span ends at the last digit, so `PF-1234-son` → `PF-1234` + `-сон`. Examples: PF-1234, ПФ-5050, PQ-4545, ПҚ-4545, VMQ-123, OʻRQ-587, ЎРҚ-587, A4, Б-12. | **Owner decision 2026-10-20:** codes stay exactly as typed, in either script. The limit of 4 letters keeps all-caps words like `MODDA-5` convertible. |
| **CODE-B** | keep | keep | A token part made of letters and digits only (no apostrophes or hyphens), in which a run of 3 or more digits touches an **upper-case** letter. Examples: BI72373344, AA1234567, ВИ7237. | The plugin's rule, plus the upper-case condition so that `100ta` still converts to `100та`. |
| **Multiplication x** | keep | keep | `x`, `X` or `×` directly between two digits (3x4, 1920x1080). | It is a sign, not a letter. |
| **Numbers** | keep | keep | by construction: digits are not letters | Appendix A. |
| **ROMAN** (`Options.roman_numerals`, default on) | keep | not needed | An UPPER Latin word of length ≥ 2 that matches `^C{0,3}(XC\|XL\|L?X{0,3})(IX\|IV\|V?I{0,3})$` (values 1–399, letters I V X L C only). Or a single `I`, `V` or `X` that is either **not followed by `.`**, or is followed by `.` and is at a **paragraph start** (a section heading such as `I. Umumiy qoidalar`). **Paragraph start:** the first non-whitespace character of the input, or the first one after U+000A, U+000B, U+000D, U+2028 or U+2029; in `convert_runs`, the first non-whitespace character of `runs[0]`. | Uzbek Cyrillic documents write Roman numerals with Latin letters (XXI аср, II боб). A single letter + `.` elsewhere is an initial (X. Rahimov → Х. Раҳимов, I. Karimov → И. Каримов). D and M are left out because centuries, chapters, grades and quarters stay below 400, while DXX (ДХХ, the State Security Service) is a real Uzbek acronym. In →L, Latin letters are never changed anyway, and a Cyrillic Х used as a Roman numeral becomes X, which is correct. |
| **ALL-CAPS Latin acronyms** | **convert**, unless `keep` (§6.4 K) or R14 applies | n/a (Latin is untouched) | — | Uzbek acronyms (BMT, AQSH, MChJ, STIR, IIV, YaTT) are frequent in HR documents and must convert. Foreign ones are a finite list (`keep`) or contain c/w (R14). Protecting every all-caps word would break the Uzbek ones. |

### 7.1 Order and overlap
At each token start (§2.2), every span type is tried and the longest match wins; on a tie, the earlier row in the table wins. A span never starts inside a word. A token that starts with `№` is tried again after the `№` (`№PF-12`).

### 7.2 Adapter-level skips (named here, specified in PLAN.md §3–§5)
The crate receives these as `convert: false` runs, or never sees them:
- **Field codes:** `InlineObject::Field{instr}` and `FieldStart{instr}` (Docs), `a:fld` runs (Slides). Never touched.
- **Field results:** converted by default (`fieldResults: true`, PLAN §3.4), so a TOC matches its converted headings.
- **Hyperlink targets:** `CharProps.link` / `a:hlinkClick` are properties, never text. The display text converts.
- **Language tags (owner decision D1):** `ru*` and `en*` runs **are converted**, because Uzbek is commonly typed under those layouts. Runs tagged with any other non-`uz` language (de-DE, tr-TR …) are skipped. A command option also skips ru/en.
- **Not text:** tracked-deleted text (left out of the slice), locked content controls, equations, alt text.
- **Sheets:** formulas, numbers, table header cells and sheet names are never touched.

---

## 8. R13: run boundaries

### 8.1 Input and segments
- `convert_runs(runs, dir, opts)` takes one paragraph's runs in order.
- A maximal sequence of consecutive `convert: true` runs is one **segment**. Its text `S` is the concatenation of those runs.
- `convert: false` runs are copied verbatim and separate segments. They are hard boundaries: no context, word, digraph, quote or case decision crosses them.
- Selection edges are segment edges (the adapter cuts runs there).

### 8.2 Clusters
Converting `S` produces an ordered list of **clusters** that partitions `S`'s characters. Each cluster has a source range `[i, j)` (character indices into `S`) and an output string. The clusters are:

| Kind | Source → output | Examples |
|---|---|---|
| 1 → 1 | one letter | a → а, ъ → ʼ, ʼ → ъ (tutuq) |
| 1 → 0 | one letter | ь → "", ъ → "" (R8) |
| 1 → 2 | one letter | ш → sh, ў → oʻ, е → ye, ц → ts, с → sʼ (R4) |
| 2 → 1 | two characters | sh → ш, ch → ч, oʻ → ў, gʻ → ғ, yo yu ya ye → ё ю я е, ts → ц (R7a), (s, apostrophe) → с (R4) |
| verbatim | one character | punctuation, digits, spaces, protected-span characters, R14 words, `keep`/`acro` source where the output is the same |
| dictionary | n source letters → m output | split into n pseudo-clusters by §8.3 |

R16 additions:
- a combining Cyrillic pair is one 2 → 1 cluster;
- a transparent character inside a cluster belongs to that cluster, and its output goes after the cluster's letter output;
- other combining marks join the preceding cluster.

### 8.3 Proportional split for dictionary output
- A dictionary hit maps n source characters (stem part, or the whole word for `word`/`acro`) to m output characters.
- Source character k (0 ≤ k < n) gets output characters `[b(k), b(k+1))`, where `b(k) = ⌊(k·m + ⌊n/2⌋) / n⌋`. Then b(0) = 0, b(n) = m, and b is non-decreasing.
- Example: сентябрь (n = 8) → sentabr (m = 7) gives b = 0,1,2,3,4,4,5,6,7. So runs `сент|ябрь` → `sent|abr`.

### 8.4 Assignment
- Every cluster's output goes to the run that contains the cluster's **first source character**.
- The output of run r is the concatenation, in order, of the outputs of the clusters assigned to it.
- A run may come out empty, for example the tail of a digraph. The run stays in the result as `""`, and the engine's `normalize()` drops it.

| Source runs | Output runs |
|---|---|
| **S**·h·ahar (bold S) | **Ш**·""·аҳар |
| **O**·ʻzbekiston | **Ў**·""·збекистон |
| s·**ʼ**·hoq | с·**""**·ҳоқ |
| **Ш**·аҳар | **Sh**·ahar |
| сент·ябрь | sent·abr |

- **Borrowed vs owned:** a run whose output equals its input is returned as `Cow::Borrowed`.

### 8.5 Case across runs
- Case (R10) is decided on `S` before assignment: word class, next/previous letter and all-caps context are all computed on the joined text.
  - Example: bold `S` + plain `HAHAR` is one UPPER word, so the output is bold `Ш` + plain `АҲАР`.
  - Example: bold `Ш` + plain `АҲАР` is one UPPER word, so the bold run gets `SH`.
- A cluster's output is cased as a whole, then assigned to one run, so a 1 → 2 output never straddles two runs.

### 8.6 Invariant (property-tested)
For every split of `S` into `convert: true` runs:
`concat(convert_runs(split(S))) == convert(S)`.

It holds because conversion depends only on `S`, and assignment only distributes the cluster outputs in order.

---

## 9. Round-trip guarantees (basis of the property tests)

Let `N(x)` be `x` with every R1 apostrophe written in its output form (oʻ/gʻ = U+02BB, tutuq and the R4 separator = U+02BC).

### 9.1 Always
- **Idempotence:**
  - `→C(→C(x)) == →C(x)`, because Latin residue (protected spans, R14 words, keep entries, punctuation apostrophes) is classified identically the second time;
  - `→L(→L(x)) == →L(x)`.
- **Run split invariance:** §8.6.
- **Apostrophe equivalence:** replacing any R1/R2/R4 apostrophe in a Latin word by any other member of `A` does not change `→C(x)`.
  - Exception: R15 closers, which are by definition U+0027, U+0060 or U+2019.
- **Protected spans:** every R12 span is byte-identical after `→C` and after `→L`.

### 9.2 Latin → Cyrillic → Latin: `→L(→C(x)) == N(x)` guaranteed
The guarantee holds when `x` is official-orthography Latin and every Latin word in it satisfies **all** of these:
1. It has class lower, Title or UPPER (or Single), not Mixed. Mixed words are covered only by `acro` entries and by Mixed patterns whose upper-case clusters are single letters (OʻzR, ToshDU).
2. It contains `ye` only at a word start, after a vowel, or after a tutuq, unless a dictionary entry covers the word (obyekt, premyer).
3. It has no tutuq between a consonant and `y` + vowel (Cʼya …; D-09).
4. It has no tutuq at the end of a word (it would stay punctuation).
5. It has no `oʻ` + tutuq (`moʻʼjiza` normalises to `moʻjiza`).
6. It has no o/g + apostrophe that closes a quotation under R15.
7. It has no R14 trigger. Such words are untouched, so the guarantee is trivial for them, but they are excluded from the generator.

**Dictionary rows:** every reviewed `both` and `l2c` row round-trips (`→L(cyrillic) == latin`), and inflected forms with remainders from the suffix list `{da, dan, ga, ni, ning, lar, larni, lari, i, si, imiz, ingiz, dagi, gacha}` round-trip too. `variant` rows are excluded.

### 9.3 Cyrillic → Latin → Cyrillic: `→C(→L(y)) == y` guaranteed
The guarantee holds when every Cyrillic word in `y` satisfies **all** of these:
1. Its class is not Mixed (as in §9.2).
2. It has no ц, unless the word is a dictionary row or matches R7a (vowel + ц + и + я/о/а/е).
3. It has no ь, and no Russian ъ (consonant + ъ + iotated vowel), and no ўъ, unless a dictionary row covers it.
4. It has no э after a consonant (мэр, solid compounds), unless a dictionary row covers it.
5. It has no й + о/у/а/е (район, фойе), unless a dictionary row covers it.
6. It has no щ, ы or other R17 letters.
7. It has no ъ at the end of a word.
8. It has no тс after a vowel followed by и + я/о/а/е. No such Uzbek word is known; R7a would turn it into ц.
9. It contains no acronym whose →L case differs from the conventional Latin, unless an `acro` row covers it.

### 9.4 Known non-round-trip classes (each gets ≥ 3 `l2c`- or `c2l`-only golden rows)
- ц ↔ s/ts without an entry (сирка ↔ sirka is fine; цирк → sirk → сирк without the entry).
- е/э in solid compounds.
- ь and ъ without an entry.
- щ/ы → sh/i.
- Mixed case (SHahar).
- `sentyabr` (variant) → сентябрь → sentabr.
- Ishoq (missing separator).
- Ascii apostrophe output: `→C` accepts it, so the round trip still holds modulo `N`.

### 9.5 Generator for the property test (`props.rs`)
- **Latin words:** random sequences of syllables over the official alphabet, built from the clusters a b d e f g gʻ h i j k l m n o oʻ p q r s sh t u v x y z ch, plus yo yu ya ye only at a word start or after a vowel.
  - The generator applies the §9.2 exclusions: no `ts` (it would only test R7 vs R7a), no c/w, no word-final tutuq.
  - Words are lower case, then given a random class from lower/Title/UPPER.
- **Sentences:** words joined by spaces, hyphens, digits and punctuation, plus inserted protected spans (URL, e-mail, PF-1234, XXI).
- **Variation:** each word is converted with a random apostrophe variant, and the text is split into random runs.
- **Seeds:** 20,000 cases with fixed seeds (PLAN §9 WP3).

---

## 10. Worked examples (60; the first unit tests)

Input apostrophes are U+02BB/U+02BC unless the Rule column names another code point. Output apostrophes are always U+02BB (oʻ, gʻ) and U+02BC (tutuq, sʼh). `|` separates runs; **bold** marks a bold run.

| # | Dir | Input | Expected | Rule |
|---|---|---|---|---|
| E01 | →C | `shahar` | `шаҳар` | R3 |
| E02 | →C | `Shahar` | `Шаҳар` | R3, R10 |
| E03 | →C | `SHAHAR` | `ШАҲАР` | R10 |
| E04 | →C | `choy` | `чой` | R3, R6 |
| E05 | →C | `oʻzbek` | `ўзбек` | R1 |
| E06 | →C | `o'zbek` (U+0027) | `ўзбек` | R1 |
| E07 | →C | `O‘zbekiston` (U+2018) | `Ўзбекистон` | R1 |
| E08 | →C | `gʼalaba` (U+02BC after g) | `ғалаба` | R1 (never tutuq after g) |
| E09 | →C | ``to`g`ri`` (U+0060) | `тўғри` | R1 |
| E10 | →C | `bo´lim` (U+00B4) | `бўлим` | R1 |
| E11 | →C | `maʼno` | `маъно` | R2 |
| E12 | →C | `san'at` (U+0027) | `санъат` | R2 |
| E13 | →C | `mas’ul` (U+2019) | `масъул` | R2 |
| E14 | →C | `eʼlon` | `эълон` | R2, R5 |
| E15 | →C | `maʼyus` | `маъюс` | R2, R6 |
| E16 | →C | `Is'hoq` (U+0027) | `Исҳоқ` | R4 |
| E17 | →C | `yer` | `ер` | R5 |
| E18 | →C | `ekin` | `экин` | R5 |
| E19 | →C | `poeziya` | `поэзия` | R5 (e after a vowel) |
| E20 | →C | `sotsial-ekonomik` | `социал-экономик` | R7a, R5, hyphen boundary |
| E21 | →C | `yoʻq` | `йўқ` | R6 |
| E22 | →C | `dunyo` | `дунё` | R6 |
| E23 | →C | `yangi` | `янги` | R6, R3 (ng) |
| E24 | →C | `tayyor` | `тайёр` | R6 |
| E25 | →C | `ketsa` | `кетса` | R7 (ts → тс) |
| E26 | →C | `operatsiya` | `операция` | R7a |
| E27 | →C | `litsey` | `лицей` | R11 |
| E28 | →C | `funksiyasi` | `функцияси` | R11 stem + remainder |
| E29 | →C | `vaksinatsiya` | `вакцинация` | R11 stem, R7a in the remainder |
| E30 | →C | `sentabr` | `сентябрь` | R11 |
| E31 | →C | `1-sentabrdan` | `1-сентябрдан` | R11 stem, D-11 |
| E32 | →C | `SENTABR` | `СЕНТЯБРЬ` | R11, R10 |
| E33 | →C | `kompyuter` | `компьютер` | R11 |
| E34 | →C | `obyekt` | `объект` | R11 (R8) |
| E35 | →C | `moʻjiza` | `мўъжиза` | R11 (R8) |
| E36 | →C | `PF-1234-son farmon` | `PF-1234-сон фармон` | R12 CODE-A |
| E37 | →C | `Manba: https://lex.uz/docs/5013007.` | `Манба: https://lex.uz/docs/5013007.` | R12 URL |
| E38 | →C | `C:\Hujjatlar\hisobot.docx` | `C:\Hujjatlar\hisobot.docx` | R12 path |
| E39 | →C | `XXI asr, I bob` | `XXI аср, I боб` | R12 ROMAN |
| E40 | →C | `Buyruqni X. Rahimov imzoladi` | `Буйруқни Х. Раҳимов имзолади` | R12 (an initial mid-paragraph converts) |
| E41 | →C | `Microsoft Word` | `Microsoft Word` | R14 |
| E42 | →C | `PDF fayl, BMT` | `PDF файл, БМТ` | R11 keep; acronyms convert |
| E43 | →C | `‘Bog’ ko’chasi’` (U+2018 opener, U+2019 elsewhere) | `‘Боғ кўчаси’` | R15 |
| E44 | →C | `‘radio’ dedi` (U+2018 / U+2019) | `‘радио’ деди` | R15 |
| E45 | →C | `'olma'` (U+0027) | `'олма'` | R2 (no ъ at a word end) |
| E46 | →C | `va/yoki 3x4` | `ва/ёки 3x4` | R12 (not a path; x sign) |
| E47 | →C | runs `**S**\|h\|ahar` | runs `**Ш**\|""\|аҳар` | R13 |
| E48 | →L | `ШАҲАР`, `Шаҳар` | `SHAHAR`, `Shahar` | R10 |
| E49 | →L | `Ўзбекистон тўғри` | `Oʻzbekiston toʻgʻri` | R1 |
| E50 | →L | `маъно масъул Исҳоқ` | `maʼno masʼul Isʼhoq` | R2, R4 |
| E51 | →L | `ер қаер кел эл Европа` | `yer qayer kel el Yevropa` | R5 |
| E52 | →L | `объект съезд маъюс мўъжиза` | `obyekt syezd maʼyus moʻjiza` | R8 |
| E53 | →L | `цирк операция концерт абзац` | `sirk operatsiya konsert abzats` | R7 |
| E54 | →L | `сентябрь сентябрда сентябрьда октябрь` | `sentabr sentabrda sentabrda oktabr` | R11 |
| E55 | →L | `компьютер фильм почтальон пьеса` | `kompyuter film pochtalyon pyesa` | R9, R9a, R5 |
| E56 | →L | `дунё йўқ ЯНГИ Ёзги` | `dunyo yoʻq YANGI Yozgi` | R6, R10 |
| E57 | →L | `Ш. Рашидов` / `Ш. РАШИДОВ` | `Sh. Rashidov` / `SH. RASHIDOV` | R10 (Single) |
| E58 | →L | `ТошДУ ЎзР АҚШ МЧЖ` | `ToshDU OʻzR AQSH MChJ` | R10, R11 acro |
| E59 | →L | `ПФ-5050-сон, XXI аср, щётка` | `ПФ-5050-son, XXI asr, shyotka` | R12, R17 |
| E60 | →L | runs `**Ш**\|аҳар` and `сент\|ябрь`; also `Ўзбекистон` with `Ascii` | runs `**Sh**\|ahar` and `sent\|abr`; `O'zbekiston` | R13, §8.3, R1 Ascii |

Tests that need more than one assertion (E48, E54, E56–E60) are split into one test per string.

---

## 11. Golden-list quota additions (PLAN.md §9 WP3)
The PLAN quotas stand. Added for rules this spec introduces:

| Rule | Rows |
|---|---|
| R7a | 15 `both` |
| R9a | 5 `both` (with dictionary) |
| R8 маъюс-type (vowel + ъ + iotated) | 5 `both` |
| R12 per span type | 3 sentences each |
| R14 | 5 sentences |
| R15 | 8 sentences (3 of them `ambiguous`) |
| R16 | 5 `c2l` rows with combining Cyrillic |
| §9.4 non-round-trip | 3 one-way rows per class |

---

## 12. Decisions (where Appendix A is silent or ambiguous)

| # | Decision | Basis | Reversible by |
|---|---|---|---|
| D-01 | `A` adds U+02BF ʿ and U+00B4 ´; it excludes U+2032 and U+02B9 | in use in typed Uzbek; the prime has another meaning | data change |
| D-02 | Output: oʻ/gʻ = U+02BB, tutuq and the sʼh separator = U+02BC; `Ascii` option = U+0027 | 1995 alphabet; current official practice; the plugin | option |
| D-03 | An apostrophe after o/g is always oʻ/gʻ, never a tutuq (except R15 closers) | 1995 spelling: moʻjiza; Uzbek has no гъ/оъ | — |
| D-04 | Tutuq only between two Latin letters; elsewhere an apostrophe is punctuation | Appendix A example set; fixes plugin bug C6 | — |
| D-05 | ng is never a unit: n ↔ н, g ↔ г | both scripts write two letters; no separator exists | — |
| D-06 | Latin `e` → э at a word start, after a vowel, and after a tutuq (ъэ) | Appendix A; Cyrillic е after ъ reads ye | — |
| D-07 | Compound boundaries without a hyphen are not detected (Ўзбекэнерго does not round-trip) | no morphology in the crate | dictionary rows |
| D-08 | R7a: vowel + ts + i + (y, a, o, e) → ц | productive -tsiya/-tsion family; no native counter-example | option or rule change |
| D-09 | Cyrillic consonant + ъ + е/ё/ю/я → no tutuq in Latin (Russian ъ); vowel + ъ + iotated keeps ʼ (маъюс) | Russian ъ follows a prefix consonant; Uzbek tutuq words have a vowel there | — |
| D-10 | R9a: ьо → yo (почтальон → pochtalyon) | the y keeps the pronunciation; common Latin spelling | rule change |
| D-11 | A final ь of a stem is dropped before an Uzbek suffix (сентябрда, январдан); →L accepts both | prevalent Cyrillic usage in official texts | data change |
| D-12 | щ → sh, ы → i; other non-Uzbek Cyrillic letters unchanged | Uzbek Latin has no shch; the plugin agrees | — |
| D-13 | Mixed case: a capital is all-upper when its neighbour letter is a capital (§5.2) | Appendix A "as far as possible"; the plugin heuristic made word-aware | — |
| D-14 | UPPER acronyms → SH/CH/YA (AQSH); common Mixed ones via `acro` (MChJ, YaTT, JShShIR, YaIM) | consistent with SHAHAR; the Mixed forms are the dominant convention | data change |
| D-15 | All-caps Latin acronyms **convert** unless `keep` or R14 | Uzbek acronyms dominate HR documents | data change |
| D-16 | Roman numerals (I V X L C only, 1–399) protected in →C; a single I/V/X + `.` is protected only at a paragraph start | Cyrillic documents keep Latin numerals; initials mid-text; DXX = ДХХ | option |
| D-17 | Codes: CODE-A (1–4 upper-case letters + digits) and CODE-B (an upper-case letter touching ≥ 3 digits), in both scripts | owner decision 2026-10-20 (codes as typed) | owner |
| D-18 | File paths, @mentions and the multiplication `x` are protected | identifiers and signs, not words | data change |
| D-19 | R14 foreign guard on by default (c not in ch, w, diacritic Latin letters) | not Uzbek letters; PLAN C7 | option |
| D-20 | R15 quote guard as specified in §4.11 | Word smart quotes make ’ both a closer and an okina | — |
| D-21 | Cyrillic combining pairs composed; transparent characters ignored for context; words containing them skip the dictionary | real Word documents contain soft hyphens | — |
| D-22 | ru/en-tagged text converted; other non-uz tags skipped; an option skips ru/en | owner decision 2026-10-20; PLAN D1 | owner / option |
| D-23 | Field results converted; field codes and hyperlink targets never touched | PLAN §3.4 | param |
| D-24 | `yod` gets no entry (ёд "memory" vs йод "iodine"); `sirk` is word-only (sirka) | false-hit risk | data change |

---

## 13. Genuinely uncertain (watch in use; `ambiguous` rows at the gate)
1. **D-11, ь before suffixes.** Usage is mixed (сентябрьда and сентябрда both occur). Our →C picks the ь-less form.
2. **D-10, ьо → yo.** pochtalyon vs pochtalon. Both Latin spellings exist; the 1995 text gives no example we could cite.
3. **D-16, a single I/V/X with a dot.** At a paragraph start, `V. Aliyev` (an initial) stays Latin. Mid-text, a section `I.` would convert to `И.`.
4. **R15 with ASCII apostrophes throughout** (`'bog'`). This is inherently ambiguous.
5. **D-14, AQSH vs AQSh.** Both appear in official Latin texts.
6. **`QR`** (QR-kod vs ҚР). Left unprotected and marked `ambiguous`.
7. **D-08 (R7a) and D-09 (consonant + ъ + iotated).** They rest on the absence of counter-examples, not on a cited rule. The lex.uz parallel texts in WP3 are the test.
8. **CODE-A with 1–4 letters.** A short all-caps word + `-digit` (e.g. `BOB-2`) stays Latin.
