# Phase 2 gate review (adversarial)

| | |
|---|---|
| **Date** | 2026-10-20 |
| **Reviewer** | opus sub-agent (architect). Read-only for code. |
| **Branch / HEAD** | `phase-2` @ `82d21ac` (WP12 Russian catalog hooks landed during the review) |
| **Run by the reviewer** | `cargo test -p uz-translit --release`: all pass (alphabet2026 10, dict 6, golden 4, props 4, rules 88). A scratch binary over the crate (outside the repo, `E:/office-target/review-scratch`) for about 90 spot inputs. Raw perf JSON read from `E:/office-target/p2/`. |
| **Not run** | Any heavy build. `ci.sh full` on a clean checkout belongs to another agent; its result goes to `docs/phase2/clean-checkout.md`, which did not exist yet when this review was written. **The gate verdict depends on that run.** |

## 1. Exit gate verdicts

| # | Gate item (BRIEF Phase 2) | Verdict | Evidence and caveat |
|---|---|---|---|
| 1 | Golden tests pass | **MET-WITH-CAVEAT** | `golden.tsv` has 1,082 rows: 1,017 `both` + 15 `l2c` + 34 `c2l` reviewed, 11 `ambiguous`, 5 `rejected`. The test checks **1,032** Latin→Cyrillic and **1,051** Cyrillic→Latin rows, plus 81 variant rows and 952 rows under the 2026 alphabet. All pass. ≥500 both-direction rows: met. **The labelling is correct.** Every row is `reviewed:agent-opus:2026-10-20`. No doc says "approved by the owner"; rules.md, golden-notes.md, source-verification.md and PLAN.md all say "agent review under delegation, not an owner review". **Caveat (the brief's warning):** 577 of the 1,017 `both` rows have `source=agent`. A sonnet drafted them and an opus reviewed them, and the same agent family wrote the rules and the engine. Only **440** `both` rows (plus 10 one-way rows) are externally attested (`lex-attested`, `official-name`, `lex-<id>`), which is fewer than 500. Passing the list therefore proves internal consistency more than truth. Under the delegation this is acceptable, but the owner should know it. |
| 2 | 108-page .docx in under 2 s in the editor; report the number | **MET-WITH-CAVEAT** | **Measured, not projected:** desktop `docs.exe` command → next frame **0.40-0.42 s**, → painted new layout (screenshot, upper bound) **0.93-0.94 s**; web (docs-web wasm, headless Chromium, desktop PC) **1.26-1.27 s**; headless native N1 0.35-0.44 s; nightly `translit-perf` 344/303 ms. The reviewer re-derived every median and raw round in perf.md from `n2.json` and `n3.json`; they match, except one figure that is now fixed (§4). ONLYOFFICE plugin: 62 s. **Android is honestly reported as "not obtained"** (perf.md N3, STATUS). Caveats: (a) the 108-page files are synthetic, raw OOXML from a generator, **not written by Word**, so Word's run fragmentation (rsid, proofErr) is absent. Conversion is ~15 % of the time and layout dominates, so the risk is small. (b) Web was measured on a 12-core desktop, not on a typical HRM laptop. (c) The raw JSON was only in the E: scratch dir; it is now copied to `docs/phase2/evidence/perf/`. |
| 3 | Formatting identical before and after, apart from the text | **MET-WITH-CAVEAT** | Word, Excel and PowerPoint 16 (COM) wrote 9 .docx, 2 .xlsx and 2 .pptx fixtures. docx: 10 tests, both directions; xlsx/pptx: 8 comparisons. All report IDENTICAL. **What "identical" means here is weaker than it sounds.** (a) The baseline is **B = engine open→save**, not the Word original. Anything the engine already drops on a plain open/save is invisible, e.g. xlsx rich text (P3), which is lost by any save. (b) The docx XML diff ignores all `w:t` text and **lets any `w:r` with ≤ 2 characters vanish from R** (the "digraph tail" tolerance). The model check (run props, paragraph props, objects) runs on **body paragraphs only**. So in headers, footers, footnotes and text boxes, a short run that vanishes, with its formatting, would pass. (c) **U+2060 is a real formatting delta that was masked.** Word puts a symbol font on a word joiner. After conversion the joiner joins the previous letter's run and loses that font. The XML diff tolerates it (1-char run) and the model check explicitly excludes it. It is invisible, so its impact is nil, but strictly it is "identical except one known invisible delta". STATUS says that. The comment in `preserve.rs:267` says it is reported in perf.md, but perf.md does not mention it. (d) No converted file was re-opened in Word (validity or repair prompt) or compared visually. (e) There is no real-world corpus document (B-14: `corpus/` is still missing). |

**Overall:** no item is NOT MET. The gate is passable once the clean-checkout `ci.sh full` exits 0, provided the owner accepts the caveats above and the majors in §2. Each major is either fixed or explicitly carried forward.

## 2. Findings by severity

### Blockers
None found.

### Major

| Id | Finding | Evidence | Fix |
|---|---|---|---|
| M1 | **Wrong output: a quote ending in oʻ/gʻ plus a closing apostrophe loses the ʻ.** The R15 quote guard marks the apostrophe after g/o as the closer even when a second apostrophe follows it directly. Word typing `‘tog’’` produces exactly this. The result is a different word (тоғ "mountain" → тог; боғ "garden" → бог "god"). | `‘tog’’` → `‘тог’’` (want `‘тоғ’`); `'bog''` → `'бог''` (want `'боғ'`). Cause: `crates/uz-translit/src/l2c.rs:28-45` (`mark_closers`): the candidate after o/g is recorded and the following apostrophe is not a candidate, because its previous char is not Latin. | In `mark_closers`, when an o/g candidate at `j` is immediately followed by another apostrophe from `' ’ \``, mark `j+1` as the closer and leave `j` as ʻ. Add golden QUO rows for `‘tog’’`, `'bog''` and `‘Navroʻz’’`. Bump `RULES_VERSION`. |
| M2 | **Owner decision "1995 default with the 2026 option" is delivered only in the crate and the npm package, not in the editors.** `docs-translit`, `gridcraft/0120` and `deckcraft/0120` build `Options { apostrophe, foreign_latin_guard, ..defaults }` and have no `latinAlphabet` param or menu entry. Docs, Sheets and Slides therefore can never write 2026 Latin, and they can never read a 2026 document as 2026 only (`Ishoq` → Ишоқ). →C under the default does read ş ç ö ğ. | `domains/docs/crates/docs-translit/src/lib.rs:104-127`; `patches/gridcraft/0120…patch:186`; deckcraft likewise. | Add `latinAlphabet: "1995"\|"2026"\|"auto"` to the three command params (a few lines each) and a test each. A menu entry can wait until the law is signed. Or the owner explicitly accepts "crate + npm only until the reform is in force". |
| M3 | **Phones: the 108-page file never even opened** on the Android emulator (Chrome, `HRM_Pixel_8`, 240 s, with and without WebGL). This is reported honestly, but it is bigger than "translit not timed". It is an open/load failure of the web editor on a phone-class device, and mobile is priority #2. | perf.md N3; `E:/office-target/p2/emulator.log` | Carry forward as a Phase 4 risk with an owner-visible line. Diagnose the cause (memory, mobile layout or the open path) before the Phase 4 mobile decision. |

### Medium

| Id | Finding | Evidence | Fix |
|---|---|---|---|
| D1 | **Sheets cell context menu converts the whole sheet.** Right-click one cell → "Latin to Cyrillic" calls the command with `{}`. `scope: auto` with a single-cell selection means the used range of the active sheet. The menu location implies "this cell". One undo restores it, and the toast shows the count. | `upstream/gridcraft/crates/ui-egui/src/grid.rs:2888-2889, 2927` (`run_or_alert(other, json!({}))`); `patches/gridcraft/0120…patch:191-193` | Pass `{"scope":"selection"}` from the context menu (patch 0121). Keep `auto` on the Review tab. |
| D2 | **WP9 comparer tolerances are looser than the claim.** See gate item 3 (b). A ≤ 2-char run may vanish anywhere, and outside the body nothing checks run props. | `domains/docs/crates/docs-translit/tests/preserve.rs:155-201, 249-301` | Restrict the vanish tolerance to runs whose text, in B, is the second half of a digraph or cluster (h after s/c, an apostrophe after o/g) or invisible joiners. Run `model_check` over every story (`doc.parts`), not only `StoryRef::Body`. |
| D3 | **Slides ignores language tags.** A `de-DE` run is converted in Slides but skipped in Docs. rules.md 1.2 C4 / D1 says other non-Uzbek tags are skipped by the adapter. | STATUS WP5/WP6 ("deckcraft `RunProps.lang` … never skips") | In `deckcraft/0120`, pass `convert: lang_converts(run.props.lang, policy)` per run (one line plus a test). |
| D4 | **Golden tests can pass vacuously.** `table()` returns empty when a file is missing; `golden()` asserts ≥ 500 only `if f > 0`; `sentences()` checks `sentences.tsv`, which does not exist (0 rows checked on every run). | `crates/uz-translit/tests/golden.rs:6-12, 73-85` | Panic when `golden.tsv` or `variants.tsv` is missing. Delete the `sentences` test or add the file. |
| D5 | **Owner-visible budget change made by the agent:** docs wasm `delta_br_max` 131,072 → 163,840 B (Phase 2 added +146,051 B brotli). This follows the budgets.toml rule, but the owner has not seen it. | `docs/phase2/ci.md` gates table | List it in the exit-gate report under "decisions". Re-measure on the clean-checkout dist. |

### Minor

| Id | Finding | Fix |
|---|---|---|
| m1 | `sirk` is a `word` row (sirka "vinegar"), so `sirkda` → **сиркда** while `sirk` → цирк. Inflected circus forms are wrong. | A `stem` row `sirk` plus a `word` row `sirka` → сирка (exact match wins), or a stem with an exclusion. Golden rows `sirkda`, `sirklar`. |
| m2 | CODE-A protects short ALL-CAPS words + hyphen + digits: `BOB-3`, `QISM-2` stay Latin in Cyrillic output (`MODDA-12` converts: 5 letters). This is a by-product of "codes as typed". | Accept (Uzbek writes `3-BOB`), or require ≥ 2 consonant-only letters. Add a golden `ambiguous` row either way. |
| m3 | Roman numerals with D/M are not protected: `MMXXVI` → ММХХВИ. This is documented (D-16). | None; watch in use. |
| m4 | Tracked-deleted runs are passed as `convert:false`, so they are **hard boundaries**. PLAN 1.3 / 3.4 says they are left out of the slice, so that `s[del x]h` reads as `sh`. Now it gives сҳ. | Omit `del` segments from the `convert_runs` input in `classify`, and reinsert them verbatim. Add a test. |
| m5 | The npm template has no `"private": true`. One mistyped `npm publish` would publish under the unscoped name `uz-translit`. Nothing was published (no git remote; no `npm publish` in scripts or workflows). | Add `"private": true` until the owner decides on a registry and name. |
| m6 | The comment at `preserve.rs:267` says the U+2060 finding is in perf.md, but only STATUS has it. | Point the comment at STATUS, or add one line to perf.md. |
| m7 | `CLAUDE.md` "Language" still says "Uzbek strings are reviewed by the owner, never machine-trusted". This contradicts the delegations listed above it. Not changed (config file). | The owner or the lead edits it. |
| m8 | Simple fields (`InlineObject::Field{result}`) keep their cached result unconverted, while complex-field results (FieldStart…FieldEnd) are converted by default. Read from the code, not tested. | Document it, or convert both. They are regenerated on update either way. |
| m9 | perf.md and STATUS numbers that were wrong or stale. **Fixed in this review** (§4). | — |

## 3. Checklist against the brief

| Item | Status | Note |
|---|---|---|
| Pure Rust, zero dependencies, native + wasm | Yes | `crates/uz-translit/Cargo.toml` has no `[dependencies]`, `[dev-dependencies]` or `[build-dependencies]`. The crate is `#![no_std]` and `#![forbid(unsafe_code)]`, and `build.rs` uses std only. The wasm32 check is in `ci.sh check`. |
| Appendix A: apostrophe variants, normalised output | Yes | 8 input forms (6 + ʿ ´); output U+02BB/U+02BC; variants.tsv 81 rows. |
| Tutuq ʼ ↔ ъ, s'h → сҳ, masʼul | Yes | Spot checked in both directions, all four apostrophe forms and ALL CAPS (`IS'HOQ` → ИСҲОҚ → `ISʼHOQ`). |
| ye/е/э, ё/ю/я | Yes | `yer`, `poeziya`, `aeroport`, `dunyo`, `yoʻq`, `съезд`, `объект` are correct. |
| ц with the dictionary | Yes, with the m1 gap | `litsey`, `federatsiya`, `konsert`, `sex`, `цирк` → `sirk`, `ЦЕНТР` → `SENTR`. |
| Case (SH/Sh, ALL CAPS) | Yes | `SHAHAR`, `Ш. РАШИДОВ` → `SH. RASHIDOV`, `ЎЗБЕКИСТОН` → `OʻZBEKISTON`. |
| Exceptions dictionary | Yes | 200 reviewed rows plus 1 ambiguous. Suffixes on stems: `sentabrda`, `kompyuterlar`, `filmni`. |
| Untouched: URL, e-mail, codes, numbers | Yes | `https://…`, `info@gov.uz`, `www.my.gov.uz`, `PF-1234`, `ПФ-5050`, `BI72373344`, `10x20`. |
| Untouched: field codes, hyperlink targets | Yes | Field instructions live in objects and link targets in props, so the conversion never sees them. Tested (`command.rs:212`, `preserve_03`, `preserve_02`). |
| Untouched: non-Uzbek tagged text | Owner deviation, applied | ru/en are converted (owner decision). Other tags are skipped in Docs, but **not in Slides** (D3). Sheets has no tags. |
| Run boundaries | Yes | `convert_runs`, with the split invariance property-tested; S\|h, c\|h, o\|' tested in Docs and Slides. |
| One undo step; optional tracked changes | Yes | It is one `Session::run`. Tracked mode follows the document's Track Changes setting and adds 2 revisions; accept-all and reject-all are tested. |
| Sheets (cell text, not formulas) | Yes, with D1 | Formulas, cached values and table headers are skipped and counted. |
| Slides (frames and notes) | Yes | Shapes, groups, tables and notes. Layouts, masters, comments and SmartArt are not converted (documented). |
| WASM/npm package | Yes | Licence `MIT OR Apache-2.0`, both LICENSE files are included, 30,671 B brotli, node and Chromium tests, **not published**. |
| Tests: golden ≥500, round-trip property, docx fixtures | Yes, with D4 | The dictionary round trip is exhaustive per row (`tests/dict.rs`), not random; it is stronger than a property test. |
| Clean-room vs the owner's `translit.js` | Clean | The architecture differs (item view, protected-span scanners, word segmentation, dictionary, piece marks). The plugin identifiers (`caseLike`, `latToCyrChars`, `convertSegments`, `SKIP`, `OKINA`, `TUTUQ`) do not appear in `crates/`, `domains/docs/crates/`, the 012x patches or `ci/npm`. The only shared content is the letter tables, which are the alphabet itself. The plugin is the owner's own code (no ONLYOFFICE headers), and ONLYOFFICE's `plugins.js` is not vendored. |
| Patches registered | Yes | wordcraft/gridcraft/deckcraft 0120 and 0121 have REGISTER rows (category `translit`, "not sent"). |

## 4. Doc errors fixed in this review (uncommitted)

- `docs/phase2/perf.md`:
  - The N2 latn in-app convert was the warm-up value 55; it is now the median of the timed rounds, **57**.
  - "Our part 46-84 ms" is now **46-74 ms**. The 84 ms figure is the N2 client round trip, not convert + apply. The N3 bullet is corrected the same way.
  - The machine RAM line is annotated: CLAUDE.md records 31.8 GB from 19:56 that day.
  - The raw-rows pointer now names the committed copy.
- `docs/phase2/evidence/perf/n2.json` and `n3.json`: the raw N2/N3 rounds, copied from `E:/office-target/p2/`, so that the perf numbers have committed evidence.
- `docs/STATUS.md` WP2 bullet: an annotation with the gate-time counts (1,032 / 1,051 golden rows, `golden_2026` 952, 4 golden and 4 property tests, `sentences.tsv` absent).

Checked and consistent: N2 and N3 medians and raw rounds against the JSON; the ci.md size arithmetic (49,152 − 30,671 = 18,481; 146,051 − 131,072 = 14,979; 163,840 − 146,051 = 17,789); the golden totals in golden-notes.md (1,082 rows) against the file.

## 5. Carried forward

| To | Item |
|---|---|
| Phase 2 follow-up (before or just after the gate) | M1 quote-guard fix, M2 `latinAlphabet` param (or the owner's acceptance), D1 Sheets context-menu scope, D3 Slides language tags, D4 vacuous tests, m4 tracked-deleted boundary, m5 `"private": true` |
| Phase 3 (Converter) | Transliteration as a conversion option reuses `docs-cli run --cmd tools.translit*` (about 0.2 s for 108 pages, no layout). Add Word re-open validation of converted files (gate item 3 (d)). |
| Phase 4 (shell, mobile) | M3: the 108-page document does not open on the Android emulator. Phone timing for R1 is still unmeasured. Desktop window screenshots of the new entries. |
| Phase 5 / HRM pilot | Gate item 1 caveat: real HRM documents (`corpus/`, B-14) for both the golden behaviour and preservation. The owner reports problems found in use (delegation). |
| Phase 8 (fidelity) | xlsx rich-text loss on any save (P3); the WP9 baseline is the engine's own round trip. |
| Ongoing | 2026 alphabet: the law is not yet signed, and sources S14/S15 were not fetched (rules.md §15). Revisit when it is signed. |
