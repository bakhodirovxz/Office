# Phase 2 (Transliteration, `uz-translit`): execution plan

> **Owner decisions 2026-10-20**
> - **Language-tagged text:** text tagged ru or en **is converted**, because Uzbek is commonly typed under those
>   layouts. This is a deliberate deviation from the literal Appendix A wording. It is reversible as a command option.
> - **Codes such as `PF-1234` / `ПФ-1234` stay exactly as typed**, in either script.
> - The golden list and the exceptions dictionary are approved by the agent (`reviewed:agent-*`), delegated by the
>   owner.


| | |
|---|---|
| **Status** | Plan only, written 2026-10-20 on branch `phase-1`. No code, no builds (the Phase 1 gate CI run had the machine). Execution starts on branch `phase-2` (from `phase-1`) **after the owner's "go" at the Phase 1 gate**. |
| **Gate decisions in force** | ADR Appendix C: MIT OR Apache-2.0; working name from `brand/brand.toml` only; W2 domains + pristine snapshot + patch queue; **no upstream PRs, every upstream change is a carried patch with a `patches/REGISTER.md` row**; **golden list and exceptions dictionary review delegated to the agent** (2026-10-20): rows are marked `reviewed:agent-<model>:<date>`, never as owner approval, and the owner reports problems found in use. |
| **Inputs** | `docs/BRIEF.md` (Phase 2, Appendix A), `CLAUDE.md`, ADR App. C, `docs/STATUS.md`, `docs/phase1/SIZING.md` §2.1, `docs/phase0/layer-map-*.md`, `vendor-notes/uz-translit-onlyoffice/scripts/{translit.js,code.js}` (reference only; nothing is copied). |
| **Exit gate (brief)** | (1) Golden tests pass. (2) A 108-page .docx converts in under 2 s in the editor; report the number (ONLYOFFICE plugin: 62 s). (3) Formatting is identical before and after, apart from the text. |
| **Estimate** | **17 stream-days likely (range 12.5–24)**, about 4.5 calendar weeks. SIZING said 15.75; the difference is in §10. |

How to use this plan. Each WP in §9 can be handed to one sub-agent as written. §1–§8 hold the designs the WPs
implement. The `file:line` references were read for this plan on 2026-10-20 at the current BASE (wordcraft `eae2421`,
gridcraft `f1894c6`, deckcraft `1dc6972`), mostly by sonnet Explore agents. I re-checked the load-bearing ones myself
(marked **[checked]**). Re-check every reference before writing a patch, because they move with every sync.

---

## 0. New facts found while planning (they change the design)

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| P1 | **wordcraft's command registry can be replaced at runtime.** `Session.registry: Arc<Registry>` is a pub field, `Registry::new(Vec<CommandSpec>)` and `Registry::all()` are pub, and `CommandSpec` is `Clone`. | `crates/engine/src/session.rs:364`, `crates/engine/src/lib.rs:108-122` **[checked]** | The Docs command can live in **our own crate**. Only a gated one-line install call per entry point is carried (§3.2). |
| P2 | **gridcraft and deckcraft registries are closed.** Each is a `static OnceLock` built from a hard-coded module list. gridcraft's `edit()`/`commit()` mutation helpers are `pub(crate)`. | gridcraft `crates/engine/src/cmd/mod.rs:110,123,151`; deckcraft `crates/engine/src/cmd/mod.rs:148,161,197` **[checked]** | Sheets and Slides need a carried engine module: one new file plus 2 lines (§4, §5). The rule logic stays in `uz-translit`, so the module is a thin adapter. |
| P3 | **gridcraft has no rich-text model.** The XLSX reader flattens `<r><t>` runs into one string, and the writer emits `<si><t>`. | `crates/xlsx/src/xml.rs:67-75` `rich_text`, called at `sheet_read.rs:446`; `crates/xlsx/src/write.rs:378` **[checked]** | Rich-text runs in cells are **already lost by any upstream save**, so "run-level" does not apply to Sheets. It is a Phase 8 fidelity finding, not ours to fix here. The preservation test compares against a plain open→save baseline (§8). |
| P4 | **All three engines make one command one undo step,** using whole-document copy-on-write snapshots (Arc-shared blocks, slides or sheets). | wordcraft `session.rs:1167-1303` (two Arc clones per mutating command); gridcraft `lib.rs:327, 360-370`; deckcraft `lib.rs:579-628` | "One undo step" comes for free if the whole conversion is **one command**. No transaction API is needed, and none exists. |
| P5 | **wordcraft paragraph = one `String` plus `Vec<Run{len: bytes, props}>`.** Inline objects are U+FFFC in the text. Hyperlink targets are `CharProps.link`, the language is `CharProps.lang`, and tracked-change marks are `CharProps.ins/del`. | `crates/doc/src/para.rs:17, 40, 767`; `crates/doc/src/props.rs:348, 377, 380, 383` **[checked]** | This is exactly the shape of the run-aware API (§1.3). Changing run text never touches a hyperlink target. Field codes live in objects (`Field{instr,…}`) or `FieldStart{instr}` markers, never in the text (`para.rs:445-479`). |
| P6 | **The closest analogue in wordcraft is `format.changeCase`, not Replace All.** It rewrites characters per run, keeps the formatting, has a tracked-change recipe, and maps the selection afterwards. | `crates/engine/src/cmd/format.rs:383` `change_case`, `:500` `live_chars`, `:517` `case_groups`, `:541` `map_offset` **[checked]**; tracked recipe `:446-463` | Model the Docs command on it. But `case_groups` converts per run, which breaks digraphs split across runs. Ours converts the joined live text of the paragraph (§3.3). |
| P7 | **wordcraft's column/cell dispatch rewrites the selection** for any mutating id it does not know: the selection collapses to the first segment. `para.*` and `format.*` ids pass through, but the protection check then treats them as "direct formatting", which a styles-only lock blocks. | `crates/engine/src/cmd/column.rs:178-190`, `crates/engine/src/cmd/protect.rs:243-245` **[checked]** | Use a neutral id (`tools.translit*`) and add it to the `per_row` arm at `column.rs:178` (one carried line). Without that line, a column selection would collapse to a caret, and "caret = whole document" would convert the whole file. This is a real data-safety trap. |
| P8 | **Layout time, not the algorithm, is the 108-page risk on web.** Phase 0 measured open+layout of a 4-page document in emulator wasm at 64–90 ms warm, about 16–22 ms per page. Linear extrapolation gives about 1.7–2.4 s for 108 pages. wordcraft lays out the whole document on every pass and caches shaping per paragraph `rev` only. | `docs/phase0/mobile-b/b4-load-warm.jsonl`; `crates/layout/src/lib.rs:485, 804-819`; `para.rs:829` `touch` | A conversion changes every paragraph, so every paragraph is reshaped. **Measure the layout time on day 1 of the Docs work** (WP10a), before any design is frozen. The emulator figure is a PROJECTION; desktop Chromium and native will be faster. |
| P9 | **The owner's seed has arrived.** `translit.js` is 152 lines, and `code.js` drives ONLYOFFICE through `ReplaceTextSmart` per paragraph in ~1 s batches. | `vendor-notes/uz-translit-onlyoffice/scripts/` | SIZING's "seed missing" blocker is closed. §2 lists where it differs from Appendix A. |
| P10 | **Lots of parallel uz-Latn ↔ uz-Cyrl text already exists in the repo.** The i18n catalogs hold `reviewed:agent-opus` Uzbek UI strings in both scripts (`i18n/*.tsv`). | `i18n/wordcraft.tsv` rows such as `Navigatsiya paneli` / `Навигация панели` | A free **regression corpus** (not truth: the Cyrillic was drafted by machine and reviewed by an agent). Every disagreement is either a catalog error or a rule gap (WP3). |
| P11 | **Our own crates and domain crates already have CI homes.** `crates/*` is a root workspace member, and `test-ours` runs the root workspace plus every domain workspace under nextest. The i18n extractor scans `crates/ui-egui/src` and `crates/engine/src` of each upstream tree. | `Cargo.toml:3`; `scripts/ci.sh:345-358`; `xtask/src/i18n/extract.rs:258-260, 530, 635` | No new CI job is needed for the crate. Labels added by our ui-egui patches are extracted automatically. Labels inside our domain crate are not (§6). |

---

## 1. Crate `crates/uz-translit`

### 1.1 Shape
- A root workspace member (`crates/uz-translit`). It is MIT OR Apache-2.0, `publish = false`, and has **zero dependencies, including dev-dependencies**. The property tests use a 15-line xorshift PRNG with fixed seeds instead of proptest.
- `#![no_std]` + `extern crate alloc`. This costs nothing: the code needs only `String`, `Vec`, `Cow` and `char::to_{upper,lower}case`, which are all in core and alloc. Tests run with std.
- Built native, and checked with `cargo check -p uz-translit --target wasm32-unknown-unknown`. The wasm-bindgen layer is a **separate crate** (§7), so the core stays dependency-free.
- `build.rs` (std only) compiles `data/exceptions.tsv` into sorted static tables (`&[(&str, &str, Kind)]`, one keyed by Latin and one by Cyrillic). There is no parsing at run time and no `OnceLock`, which keeps it no_std. **Only `reviewed:*` rows are compiled in**, as with the i18n release export.

### 1.2 String-level API
```rust
pub enum Direction { ToCyrillic, ToLatin }
pub enum Script { Latin, Cyrillic }
pub struct Options {
    pub apostrophe: Apostrophe,        // Official (ʻ U+02BB / ʼ U+02BC, default) | Ascii (')
    pub foreign_latin_guard: bool,     // ToCyrillic: leave words with c (not in ch), w untouched (default true; §2 C7)
    pub roman_numerals: bool,          // protect XXI, IV ... (default true; §2 C9)
}
pub fn convert(text: &str, dir: Direction, opts: &Options) -> String;
pub fn detect(text: &str) -> Option<Script>;   // letter majority, like the plugin's detect()
pub const RULES_VERSION: &str;                 // bumped with any output-changing rule or data change
```

### 1.3 Run-aware API
```rust
pub struct Run<'a> { pub text: &'a str, pub convert: bool }
/// Converts the runs of ONE paragraph as one logical text and returns each run's new text.
/// Borrowed = unchanged (the caller skips that run: fewer writes, and unchanged paragraphs stay cached).
pub fn convert_runs<'a>(runs: &[Run<'a>], dir: Direction, opts: &Options) -> Vec<Cow<'a, str>>;
```
- **Join, convert once, split back.** The engine walks the concatenated text of the runs that have `convert: true`. Internally it records `starts[i]`, the output byte offset where the output of source char `i` begins. Each run's output is `out[starts[first char] .. starts[end]]`.
- **Ownership rule for multi-char clusters.** The output of a source cluster goes to the run of its **first** source char. Bold "S" + plain "h" → bold "Ш" + plain "" (an empty run, which the engines' `normalize()` drops). The plugin does the same in `convertSegments` (`translit.js:142-150`).
  - One char → two chars (ш → sh) stays inside one run.
  - A dictionary hit (sentabr → сентябрь) is spread proportionally: source char `k` of `n` gets output chars `[k·m/n, (k+1)·m/n)`. That keeps a half-bold word half bold. In the common case the whole word is in one run, and then it makes no difference.
- **`convert: false` runs** (field results we keep, foreign-language runs, locked content controls) are copied verbatim and act as a **word boundary**.
- **Runs that are not text at all** (tracked-deleted runs) are simply **left out of the slice** by the caller. The join then sees the live text only ("s[del:x]h" reads as "sh"), which is what Change Case does with `live_chars`.
- Inline objects are U+FFFC in wordcraft text. Like every non-letter, U+FFFC is a boundary that passes through unchanged.
- Invariant (property-tested): `concat(convert_runs(split(t))) == convert(t)` for **every** split of `t`.

### 1.4 Rule engine (one pass, no regex, hand-written scanners)
Per paragraph:
1. **Protected spans** are found at token starts and copied verbatim (Appendix A "leave untouched"):
   - a URL: `scheme://…` or `www.…` up to whitespace, with trailing `.,;:!?)»"` trimmed;
   - an e-mail address;
   - a bare domain with a known TLD (`uz ru com org net gov edu info io`, the plugin's list);
   - **codes** (either script): `UPPER{1,5}[-/]digits` (PF-1234, ПФ-5050, PQ-4545), or a letter adjacent to 3 or more digits (BI72373344, the plugin's rule);
   - Roman numerals (strict pattern, all caps; length ≥ 2, or a single I/V/X that is not followed by "." so that initials still convert).

   Numbers have no letters, so they are untouched by construction. Units like `m3` still convert (the plugin's behaviour).
2. **Words** are maximal sequences of letters plus apostrophe variants. An apostrophe counts as part of a word only between letters, or after o/g.
3. **Exceptions dictionary.** The word is lowercased and its apostrophes normalised. It is looked up first as an exact `word` entry, then as the longest `stem` entry (a stem plus any remainder, e.g. `oktabr|da`). The remainder is converted by the rules, with the stem's last letter as context. The original case pattern is re-applied.
4. **Rules** for words without a dictionary hit:
   - **ToCyrillic** is a small state machine with lookahead ≤ 2, over the rules R1–R9 in §1.5.
   - **ToLatin** maps per char using `prev`/`next` context.
5. **Case.** Each word is classed `lower | Title | UPPER | Mixed`.
   - UPPER: all output upper (ШАҲАР ↔ SHAHAR).
   - Title: first output char upper (Sh, Ch, Yo, Oʻ).
   - Mixed: per source char. A capital whose next source letter is also a capital gives an all-caps cluster (SH); otherwise only its first char is a capital.
   - A one-letter capital word inside an all-caps run of words (two or more words of length ≥ 2) is upper. This is the plugin heuristic `caseLike` (`translit.js:17-23`), made word-aware.

**Throughput target:** ≥ 50 MB/s native. A 108-page document is about 0.3 MB of text, so conversion takes milliseconds. The 2 s budget belongs to the editor (§7).

### 1.5 The rules (Appendix A). WP1 writes them out as `crates/uz-translit/RULES.md`, numbered, with examples and sources
| # | Rule | Latin → Cyrillic | Cyrillic → Latin |
|---|---|---|---|
| R1 | Apostrophes: accept ʻ U+02BB, ʼ U+02BC, ‘ U+2018, ’ U+2019, ' U+0027, ` U+0060 | o+any → ў, g+any → ғ | ў → oʻ, ғ → gʻ (U+02BB; `Ascii` option → ') |
| R2 | Tutuq belgisi | apostrophe **between two letters** (not after o/g) → ъ: maʼno → маъно, sanʼat → санъат, masʼul → масъул | ъ → ʼ (U+02BC) |
| R3 | Digraphs | sh → ш, ch → ч; ng → нг (two letters both ways) | ш → sh, ч → ch, щ → sh |
| R4 | Separated s'h | s + apostrophe + h → сҳ (Isʼhoq → Исҳоқ) | сҳ → sʼh |
| R5 | ye / е / э | ye → е; e at word start → э; e after a vowel → э (poeziya → поэзия); otherwise e → е | е → ye at word start, after a vowel, after ъ/ь; otherwise e. э → e |
| R6 | ё ю я | yo/yu/ya → ё/ю/я; but y + oʻ → й+ў (yoʻq → йўқ) | ё/ю/я → yo/yu/ya everywhere (after a consonant too: дунё → dunyo); exceptions in the dictionary |
| R7 | ц | none by rule (ts → тс, s → с); **only through the dictionary** (litsey → лицей, sirk → цирк) | ц → s at word start and after a consonant, ts after a vowel (the 1995 rule; the plugin agrees) |
| R8 | ъ before iotated / after ў | (dictionary: obyekt → объект, moʻjiza → мўъжиза) | ъ before е/ё/ю/я → "" (объект → obyekt); ўъ → oʻ (мўъжиза → moʻjiza) |
| R9 | ь | none by rule; **dictionary** (kompyuter → компьютер, iyul → июль, aprel → апрель) | ь → "" |
| R10 | Case | §1.4 step 5 | §1.4 step 5 |
| R11 | Exceptions dictionary | §1.4 step 3 | §1.4 step 3 |
| R12 | Untouched spans | §1.4 step 1, plus the adapter-level skips in §3–§5 (field codes, hyperlink targets, language tags) | same |
| R13 | Run boundaries | §1.3 | §1.3 |
| R14 | Foreign Latin guard (ours, §2 C7) | a word with c (not in ch) or w stays as typed (Microsoft, Excel, Word) | n/a |
| R15 | Quote guard (ours, §2 C6) | a word-final apostrophe after a vowel is **not** ъ; o + an apostrophe that closes a quote opened with the same character in that paragraph is not ў | n/a |

---

## 2. The owner's `translit.js` against Appendix A (Appendix A wins)

**Where they agree** (our rules match the plugin's behaviour, re-implemented from Appendix A):
- the apostrophe variant set (`translit.js:4`) and official output forms U+02BB/U+02BC (`:2-3`);
- sh/ch, s'h ↔ сҳ (`:45-46, :95-97`), tutuq between letters (`:85`);
- е → ye at word start, after a vowel, after ъ/ь (`:38`); Latin e → э at word start or after a vowel (`:104-107`);
- ё/ю/я and yo'q → йўқ (`:98-102`); the ц rule for Cyrillic → Latin (`:40`);
- ъ before iotated vowels and after ў (`:41-44`); the case heuristic (`:17-23`);
- the per-paragraph, run-preserving split (`:141-150`).

**Where they differ or the plugin is incomplete:**

| # | Appendix A says | translit.js does | Effect | Resolution |
|---|---|---|---|---|
| C1 | Leave codes such as "PF-1234" untouched | `SKIP` (`:127`) protects only a **Latin** letter directly adjacent to ≥ 3 digits. "PF-1234" (with a hyphen) becomes "ПФ-1234". Cyrillic codes ("ПФ-5050") are never protected in Cyrillic → Latin. | Decree numbers change script | R12 code patterns in both scripts. **Flag:** real Uzbek Cyrillic documents write "ПФ-5050", while Appendix A keeps the code as typed. We follow Appendix A and mark it `ambiguous` in the golden list (owner reports in use). |
| C2 | ц ↔ s / ts through an exceptions dictionary | Latin → Cyrillic never produces ц; there is no dictionary | litsey → литсей, sirk → сирк | R7 + dictionary |
| C3 | An exceptions dictionary (sentabr ↔ сентябрь, oktabr ↔ октябрь, aktyor ↔ актёр) | none | сентябрь → "sentyabr" (official: sentabr); sentabr → "сентабр"; dekabr → "декабр"; iyul → "июл" | R11. **Note:** aktyor ↔ актёр already comes out right by rule R6; it needs no entry. The months need entries in both directions, and the inflected forms (sentabrda) need stem matching. |
| C4 | Leave text tagged with a non-Uzbek language untouched | Not possible through the ONLYOFFICE plugin API | Russian quotations and English names get transliterated | Adapter-level skip using the run language (§3.4). **This conflicts with real-world tagging** (see D1 below). |
| C5 | Leave field codes and hyperlink targets untouched | Implicit in `ReplaceTextSmart` | n/a | Explicit adapter skips (§3.4), with tests |
| C6 | Tutuq ʼ ↔ ъ (between letters) | A word-final apostrophe after a vowel becomes ъ (`:86`). A closing quote after a vowel ('olma') becomes "олмаъ". | Corrupts quoted words | R15: ъ only between letters. Plugin bug, not an Appendix A rule. |
| C7 | (silent) | Maps the non-Uzbek letters c → к and w → в (`:62`) | Microsoft → "Микрософт", Windows → "Виндоwс"/"Виндовс" | R14 foreign-word guard, on by default. Appendix A does not cover this; we decide it (reversible option). |
| C8 | ё/ю/я "context-aware ... after ъ"; the dictionary covers loanwords | Latin → Cyrillic cannot restore ъ/ь (obyekt → обект, kompyuter → компютер) | Wrong loanwords | Dictionary (R8, R9) |
| C9 | (silent) | Roman numerals convert (XXI asr → ХХИ аср) | Wrong | R12 roman-numeral span (our addition) |
| C10 | (silent) | Cyrillic йо in loanwords (район, майор) round-trips to раён/маёр | Wrong on Cyrillic → Latin → Cyrillic | Dictionary (`rayon`, `mayor`) |
| C11 | Text boxes, headers, notes and every story | Works only on the selection (`code.js:205`: "Avval o‘giriladigan matnni belgilang"), and Sheets/Slides lose run formatting through `GetSelectedText` + `ReplaceTextSmart` | Behaviour, not a rule | Ours: selection, or the whole document when the selection is empty (brief) |

**D1, the language-tag policy (an agent decision, reversible, recorded in STATUS).** Uzbek offices commonly type Uzbek Cyrillic on a Russian keyboard layout and Uzbek Latin on an English one. Word then tags the runs `ru-RU` and `en-US`. A literal "skip every non-Uzbek tag" would therefore skip most real text.

Default policy:
- skip runs whose **effective** language (`styles.resolve_char`) is set and is **not** `uz*`, `ru*` or `en*`;
- the dialog has an option "Also skip text marked as Russian / English" (off by default);
- the result message reports how many runs were skipped for their language.

This needs a measurement on the corpus (absent, B-14). The owner may override the default.

---

## 3. Docs (wordcraft): the command

### 3.1 Recommendation: our domain crate plus a gated install hook. Rejected: an engine-module patch
| Option | What is carried | Trade-off |
|---|---|---|
| **D1 (recommended)** `domains/docs/crates/docs-translit` (depends on `wordcraft-engine`, `wordcraft-doc`, `uz-translit`) holds the whole command. `install(&mut Session)` appends two `CommandSpec`s to `Session.registry`. | **Patch `wordcraft/0120`** (about 12 lines): a `#[cfg(feature = "office-ext")] docs_translit::install(&mut session);` line at `apps/wordcraft/src/main.rs:422` (and `:497` if that path is reachable), `apps/wordcraft-web/src/web.rs:79` and `apps/wordcraft-cli/src/main.rs:103` **[checked]**; an empty `[features] office-ext = []` in those three app manifests (so the upstream workspace compiles it out with clean check-cfg); and our two ids in the `per_row` arm at `crates/engine/src/cmd/column.rs:178` (P7). **Ours, not carried:** `cargo xt gen-domain` learns a `domain.toml` `[extensions] crates = ["docs-translit"]` key. It writes the path dependency and `default = ["office-ext"]` into the generated app manifests. | The logic is ours: reviewed, tested in `test-ours`, never a rebase conflict. API drift shows up as a compile error in our crate. The install hook is also the seed of Phase 7's "plugins register commands". The cost is a small `gen-domain` change. |
| D2 `crates/engine/src/cmd/translit.rs` as a carried new file, plus the registry line and an engine path dependency on `crates/uz-translit` | about 400 lines in the queue, plus Cargo.lock lines | Simplest mechanically, and the CLI and MCP get the command for free. But the logic would live in the patch queue, although the engine exposes everything needed (P1). Rejected for Docs; it is the right shape for Sheets and Slides (P2). |

**Pub API the crate uses** (all pub; the agent's report, P1/P5/P6 checked):
- `Session.{doc, sel, registry, author}`;
- `Document::{parts, para_paths (lib.rs:835), para (:806), para_mut (:813), settings.track_changes (:450), styles}`;
- `Paragraph::{text, runs, objects, normalize (para.rs:1047), touch (:829), props_of_char (:950), insert_text (:996), format (:1141)}`;
- `cmd::{selected_paths (cmd/mod.rs:139), new_revision (:162), para_lang (:456), controls::check_edit (controls.rs:353)}`;
- `CommandSpec::new(…).params(…).when(…)` (`lib.rs:61-101`) and the `p::*` param readers (`lib.rs:139-165`).
- It does **not** use the `pub(crate)` helpers (`track_delete`, `each_para*`).

### 3.2 Commands
- **`tools.translitCyrl`** and **`tools.translitLatn`**. Two ids, so that context menus without params can call them (gridcraft's needs this; §4). The `tools.` prefix avoids the protection and dispatch traps in P7.
- Params: `{scope?: "auto"|"selection"|"document", skipLangs?: bool}`. `auto` means the selection if it is not collapsed, otherwise the whole document (all stories).
- Result JSON: `{paragraphs, runs, skipped: {lang, field, locked}, ms: {convert, apply}}`. The UI shows a one-line status message (§6).
- `.when()` disables the commands on a read-only document.

### 3.3 Algorithm (one `Session::run` = one undo step, P4)
1. **Collect targets.**
   - Document scope: `[Body] + doc.parts.keys()` (headers, footers, footnotes, endnotes, comments, text boxes; `lib.rs:252-260, 662`), then `para_paths(story)` for each, which reaches into tables.
   - Selection scope: `selected_paths(s)` with per-paragraph bounds `[a.off or 0, b.off or len)`, as `format.rs:421-422` does.
   - The selection bounds are hard word boundaries: we never convert text outside the selection.
2. **Classify each paragraph's runs** into `Run{text: &p.text[range], convert}`:
   - `convert = false` for: a field result between `FieldStart`/`FieldEnd` markers when the user chose to keep results (see 3.4); a run whose effective language is skipped (D1); a locked content control (`check_edit`).
   - Tracked-deleted runs (`del.is_some()`) are left out of the slice.
3. Call `uz_translit::convert_runs`. **If every output is Borrowed, leave the paragraph alone:** no `para_mut`, no copy-on-write, no `touch`, and its layout stays cached.
4. Otherwise `para_mut`. Rebuild `p.text` in **one pass** from the outputs plus the untouched slices, keeping every `Run.props`. Set `Run.len` to the new byte lengths (Cyrillic is 2 bytes per letter, Latin 1, so offsets change). Then `p.normalize()` and `p.touch()`.
   - Never splice with `delete`/`insert_text` per match: that is O(n²), and `delete` removes any objects in its range (`para.rs:1077`).
   - U+FFFC objects stay at the same character index, so `objects[k]` still matches the k-th U+FFFC.
5. **Selection.** For selection scope the new selection is `[from, from + new_len)` in the same paragraphs. For document scope the caret is mapped through the run table (map_offset analogue, `format.rs:481-485`).
6. **Tracked changes.** When `doc.settings.track_changes` is on, use Change Case's recipe instead of step 4 (`format.rs:446-463`):
   - create a `new_revision(s, Delete)` and a `new_revision(s, Insert)`;
   - mark the old text `del`, and insert the new text after it with `ins` set and the same props.
   - Same-author, same-timestamp revisions are reused, so `doc.revisions` grows by 2, not by thousands.
   - Granularity is **per changed run** in v1. `shortcut: per-run tracked replacements, upgrade to per-word spans if reviewers find them coarse.`

### 3.4 What is skipped (Appendix A, brief)
- **Field codes.** `InlineObject::Field{instr, result, …}` (`para.rs:445-465`) is never touched: the code and its cached result live in the object, not in the text. `FieldStart{instr}` (`:470-479`) markers are never touched.
  - Decision: the **result text between `FieldStart`/`FieldEnd`** (e.g. an imported TOC or REF) is visible text and **is converted**. This keeps a TOC consistent with its converted headings until the next field update.
  - It is a param (`fieldResults: true`), and a fixture asserts both settings.
- **Hyperlink targets:** `CharProps.link` is a prop, never text, so it is untouched by construction. The display text converts.
- **URLs, e-mails, codes and numbers inside text:** R12.
- **Language-tagged runs:** D1.
- **Tracked-deleted text:** excluded (3.3 step 2). Equations, Ruby, Opaque objects and shape alt text: untouched.

---

## 4. Sheets (gridcraft): the command
- **Carried patch `gridcraft/0120`** adds the new file `crates/engine/src/cmd/translit.rs` (about 150 lines), `pub mod translit;` and `v.extend(translit::specs())` in `command_specs()` (`cmd/mod.rs:123`), and a `uz-translit` path dependency (monorepo-relative; the sync gates compile in a scratch monorepo worktree, `scripts/sync-upstream.sh:250, 319-327`) plus its Cargo.lock lines.
- **Ids:** `edit.translitCyrl` and `edit.translitLatn`, defined with `cmd!` (`cmd/mod.rs:110`), undoable.
- **Targets:** `target_ranges(s, p)` (the selection areas, `selection.rs:8-15`). With a single-cell selection, the used range of the active sheet. With `{"within": "workbook"}`, every sheet (as `replace` does, `edit.rs:1514`).
- **Cell rule:**
  - convert only `cell.formula.is_none() && Value::Text` (`model/src/cell.rs:276`; `core/src/value.rs:146-154`);
  - change `.value` on a clone of the cell and `set_cell` it, keeping `style`, pictures and the rest;
  - mutate inside `edit(s, |cx| …)` / `cx.touch` (`cmd/mod.rs:319, 522`), so one sheet copy and one incremental recalc happen (`commit`, `:328-368`).
  - Do **not** reuse `edit.replace`: it rewrites formula cells through `input_to_cell` (`edit.rs:1519-1534`).
- **Skipped (reported in the result):** formulas, numbers, booleans, errors, **table header cells** (renaming them would break structured references, and `tables::sync_headers` runs inside `edit()`), sheet names, defined names, number-format literals, chart titles, comments, headers and footers.
- **Semantic risk (warned, not fixed).** Formulas that compare against text literals (`=IF(A1="ha";…)`, VLOOKUP keys) do not change, so their results may change. The result JSON counts formulas that contain letter literals. The UI shows "N formulas contain text and were not changed".
- Row heights: `auto_row_height` (`commit`, `:362-364`) may resize auto-height rows with wrapped text. This is layout, not formatting (§8.2).

## 5. Slides (deckcraft): the command
- **Carried patch `deckcraft/0120`:** a new file `crates/engine/src/cmd/translit.rs` (about 200 lines), `pub mod translit;` (`cmd/mod.rs:3-32`), `v.extend(translit::specs())` in `command_specs()` (`:161`), and the path dependency.
- **Ids:** `text.translitCyrl` and `text.translitLatn`.
  - `text.` here is deckcraft's namespace. deckcraft has no wordcraft-style column dispatch. Re-check this in WP0.
  - Also re-check that deckcraft has no styles-only protection check.
- **Model:** `TextBody → Paragraph{runs: Vec<Run{text, props, kind}>}` (`model/src/text.rs:438-460, 494, 597`). Convert only `RunKind::Text`. `Field`, `Break` and `Math` runs are `convert: false` boundaries. `props.link` (the target) and `props.lang` are untouched. `normalize()` (`text.rs:542`) drops the empty digraph-tail runs.
- **Targets.** Mutate through `s.edit(|doc, sel| …)` (`lib.rs:643`), cloning only the slides that change, as `replace` does (`cmd/find.rs:161`, swap at `:196-198`).
  - Text selection: `Selection.text: TextSel{anchor, caret, cell, notes}` (`lib.rs:73-94`), the range inside one body.
  - Selected shapes (`Selection.shapes`): their bodies, including groups and table cells.
  - Otherwise, the whole deck: slide shapes (recursing into groups), table cells (`table.rs:11`), and speaker notes (`Slide.notes`, `model/src/lib.rs:621`).
  - We write our own walker. find.rs's `bodies` (`find.rs:43`) is private and skips layouts.
- **Skipped (reported):** layout and master placeholder prompt text, comments, SmartArt nodes, alt text. `shortcut: slides + notes only, add SmartArt (DiagramNode.text) if users ask.`

---

## 6. UI: menu, ribbon and context-menu entries, i18n
**Carried patch `<app>/0121` in each app** (ui-egui only, one-token wraps, labels through the existing `t`/`tl!`):

| App | Ribbon | Context menu |
|---|---|---|
| Docs | Review › Language group (`crates/ui-egui/src/ribbon.rs:1757` `review`, group at `:1771-1774`): a "Transliterate" menu button with two items | `canvas.rs:2835` `context_menu` **[checked]**: two `item(…)` lines, shown only with a non-empty selection |
| Sheets | Review tab (`ribbon.rs:2017` **[checked]**): `menu_items(app, ui, &[(label, id, params)])` (`:481-503`) | `grid.rs:2859` **[checked]**: two `(label, id)` tuples in the vec at `:2865-2886` |
| Slides | Review › Language (`ribbon.rs:2148` **[checked]**, `:2158`), with `big`/`small` | `canvas.rs:1075` **[checked]**: two `ui.button` lines |

- **Labels.** One English set, identical in all three apps, so that `shared.tsv` serves all of them:
  - "Transliterate", "To Cyrillic", "To Latin";
  - result messages: "{n} paragraphs converted", "Nothing to convert", "{n} items skipped (other language, locked or field)", "{n} formulas contain text and were not changed".
- **Uzbek wording.** Start from the owner's known plugin labels (`code.js:274-283`: "Lotinga o‘girish", "Kirillga o‘girish", "Lotin ⇄ Kiril"), aligned with the glossary. Note the plugin spells "Kiril" in one place and "Kirill" in another; the reviewer picks one. Use U+02BB, not the plugin's U+2018.
  - The uz-Cyrl column is produced with `uz-translit` itself (dogfooding), then reviewed.
- **Catalog flow** (the Phase 1 pipeline):
  1. `cargo xt i18n extract` picks up the strings from the patched `ui-egui/src` (P11).
  2. A builder drafts uz-Latn and uz-Cyrl; ru is drafted ours (ru override column).
  3. An opus reviewer sets `reviewed:agent-opus:<date>` (UI-string review was delegated on 2026-10-18).
  4. `lint`, `roundtrip` and `check-stale` must pass.
  - The `CommandSpec.label` inside our Docs crate is **not** extracted. Keep it byte-identical to the ribbon label, so the palette lookup hits the same row. A test asserts the label is in the catalog.
- **Keyboard shortcuts:** none by default (no Word convention exists). The commands appear in the palette and the keyboard-customise list automatically (`keytips.rs:108`, `dialogs_keyboard.rs:47`). gridcraft's keytip tables are generated and tested, so WP7 checks whether a new ribbon id needs a keytip row.

---

## 7. WASM/npm package for the HRM web app
- **Crate `crates/uz-translit-wasm`** (root workspace, `crate-type = ["cdylib"]`). Its only dependency is `wasm-bindgen = "=0.2.129"`, the pin docs-bind already uses.
- Release profile override: `opt-level = "s"`, `lto = true`, `panic = "abort"`, `codegen-units = 1`.
- **API (JS, TypeScript types from wasm-bindgen):**
  ```ts
  toCyrillic(text: string, opts?: {apostrophe?: "official"|"ascii", foreignGuard?: boolean}): string
  toLatin(text: string, opts?): string
  convertRuns(runs: string[], to: "cyrl"|"latn", opts?): string[]   // HRM rich-text editors
  detect(text: string): "latn"|"cyrl"|null
  version: string; rulesVersion: string
  ```
- **Build** (`scripts/build-npm.sh`, a new `ci.sh npm` job, also in `full`):
  1. `cargo build --release -p uz-translit-wasm --target wasm32-unknown-unknown` (sccache off for wasm, as in Phase 1).
  2. `wasm-bindgen --target web --out-dir dist/npm/uz-translit`.
     - Pin `wasm-bindgen-cli 0.2.129` as a prebuilt download in `scripts/tools.lock`; today only trunk's private copy exists.
     - The `web` target works in Vite (HRM web uses Vite, `VITE_ONLYOFFICE_URL`, `docs/phase0/hrm-current-state.md:310`) without a plugin, and in Node via `initSync(bytes)`.
  3. A hand-written `package.json` template gets its version from the crate.
  4. `npm pack` writes `dist/npm/uz-translit-<ver>.tgz`. **Nothing is published.** The HRM consumes the tarball (`npm install ./uz-translit-0.1.0.tgz`) until the owner OKs a registry and a scope.
- **Name:** `uz-translit`, the component name from the brief, not the product brand. The scope is decided at publish time.
- **Versioning:** semver, starting at 0.1.0, one version for crate and package.
  - An output-changing rule or dictionary change bumps the minor version, and so does an API change while at 0.x. `CHANGELOG.md` lives in the crate.
  - `rulesVersion` lets the HRM record which rules produced a stored text.
- **Size budget.** PROJECTION: under 64 KB raw, about 20 KB brotli. The budget is the measured value + 10 %, rounded up to 16 KiB, the `web/budgets.toml` rule. It goes in a new `[npm.uz-translit]` section, enforced by `ci.sh sizes`.
- **Tests.**
  - `node --test ci/npm/uz-translit.test.mjs` runs the golden list through the wasm, so JS and Rust must agree.
  - A Vite smoke page (`ci/npm/vite-smoke/`, Playwright, reusing `ci/web-smoke` infrastructure) imports the tarball.
- **Not covered: HRM mobile (Expo / React Native).** Hermes has no WebAssembly. Options are a WebView or a later JS port, which would be a second implementation to keep in sync. Recorded as a risk; out of Phase 2 scope (the brief says web app).

---

## 8. Formatting-preservation tests: what "identical" means

### 8.1 The baseline
All three engines re-serialise on save, so the **original file is never the comparison target**:
- `B` = open(F) → save, with no command;
- `R` = open(F) → command (document scope) → save.
- We compare B with R. This isolates our change from writer normalisation and from upstream losses such as P3.

### 8.2 Definitions
**DOCX** (`R` vs `B`):
1. Every package part **except** `word/document.xml`, `header*.xml`, `footer*.xml`, `footnotes.xml`, `endnotes.xml` and `comments*.xml` is **byte-identical**: styles, numbering, settings, theme, fontTable, media, rels, `[Content_Types].xml`. `docProps/core.xml` is excluded only if the writer stamps the current time (check in WP9).
2. In text parts, the XML trees are equal (element names, order, all attributes and values) after two normalisations:
   - (a) the character content of each `w:t` is ignored;
   - (b) a `w:r` whose `w:t` text became empty may be absent from `R`, the digraph tail (§1.3).

   So `w:rPr` (including `w:lang`), `w:pPr`, `w:sectPr`, tables, numbering references, bookmarks, comment ranges, existing `w:ins`/`w:del` with their `w:delText` text, `w:hyperlink` `r:id`, `w:fldSimple@w:instr` and `w:instrText` text must all match exactly.
3. **Text check:** for every paragraph, the concatenated `w:t` text in `R` equals `uz_translit::convert_runs` applied to `B` with the same skips. Protected strings (URLs, codes, foreign-language runs, kept field results) are byte-identical.
4. **Model check** (fast, run first): for every paragraph in every story, the sequence of `(CharProps, len > 0)` runs and the `ParaProps` and `objects` are equal before and after, modulo empty runs. Run lengths follow the mapping.
5. Rendering is reported, not asserted: Latin and Cyrillic widths differ, so page counts may shift.

**XLSX:** every part except `xl/sharedStrings.xml` is byte-identical. In particular:
- sheet XML `<f>` formulas, `s=` style indices, numbers and cell types are identical;
- a row `ht` without `customHeight` is reported, not asserted.

In `sharedStrings.xml`, only `<t>` text changes. Rich-text loss (P3) is identical in B and R, so it does not appear in the diff; it is recorded as a Phase 8 fidelity finding.

**PPTX:** every part except `ppt/slides/slide*.xml` and `ppt/notesSlides/*.xml` is byte-identical. In those parts the trees are equal, ignoring the `a:t` text of `a:r`, with empty `a:r` allowed to vanish. `a:rPr` (`lang`, `hlinkClick r:id`) is identical, and `a:fld` elements (including their `a:t`) are identical.

### 8.3 Fixtures
- Generated by **MS Word, Excel and PowerPoint 16 on this PC through COM**. Phase 0 did this already (`docs/phase0/fidelity-baseline.md`), and Office-written XML is realistic: rsid and proofErr run splits are exactly the stress we need.
- Scripts: `scripts/translit/make-fixtures.ps1`. Outputs are committed (synthetic text from the golden list, no personal data) under `domains/docs/crates/docs-translit/tests/fixtures/` and `tests/translit-fixtures/{xlsx,pptx}/`.

**docx, 8 files:**
1. a digraph split across bold/plain runs (S|h, C|h, O|ʻ, Y|e) in both scripts;
2. hyperlinks (target and display text);
3. fields: PAGE, DATE, TOC, REF, MERGEFIELD, a complex field;
4. tables, nested;
5. headers, footers, footnotes, endnotes, comments, a text box;
6. existing tracked changes;
7. lang tags uz-Latn, ru-RU, en-US, de-DE;
8. a locked content control, mixed apostrophe variants, URL/e-mail/PF-1234/XXI.

**xlsx, 2:** text, numbers and formulas with text literals, a table with headers, rich-text cells.

**pptx, 2:** text frames, groups, a table, notes, a slide-number field, a hyperlink.

The XML comparison test code may use an XML reader already in the docs domain lock (no new lock entries; WP9 checks).

---

## 9. Work packages

Tiers: **scout** = haiku, **builder** = sonnet, **architect** = opus. "Build" means a cargo build of a domain or an upstream workspace. Only **one build at a time** (11.8 GB RAM, the build lock). The zero-dependency crate compiles in seconds, so its tests may run beside text-only work, but never beside a domain build.

### WP0. Kick-off and re-verification
- **Goal:** start from facts at the BASE in force after the Phase 1 gate.
- **Steps:**
  - create branch `phase-2`;
  - re-grep every `file:line` in §3–§6 (and note any sync that moved them);
  - check deckcraft for a column-dispatch or protection trap like P7;
  - check whether wordcraft `main.rs:497` is a real second session path;
  - pin `wasm-bindgen-cli 0.2.129` in `scripts/tools.lock` and install it.
- **Files:** `docs/phase2/wp0-notes.md`, `scripts/tools.lock`.
- **Done-check:** `wasm-bindgen --version` → `0.2.129`, and every reference in the notes is marked confirmed or moved.
- **Estimate:** 0.25 d. **Tier:** scout (the tools.lock edit by builder).

### WP1. Rule specification and the conflict decisions
- **Goal:** turn Appendix A plus §2 into numbered, testable rules before code exists.
- **Steps:**
  - write `crates/uz-translit/RULES.md`: R1–R15, each with 3+ examples, the source (the 1995 orthography rules *Oʻzbek tilining asosiy imlo qoidalari* by rule number, the Cyrillic orthography rules, or Appendix A), and the decision for C1–C11 and D1;
  - define the golden-list quotas per rule (§9 WP3);
  - record D1 and C7 as agent decisions in `docs/STATUS.md`.
- **Files:** `crates/uz-translit/RULES.md`, `docs/STATUS.md`.
- **Done-check:** every Appendix A bullet maps to a rule id (a table at the top of RULES.md); `grep -c '^| R' RULES.md` ≥ 15.
- **Estimate:** 0.75 d. **Tier:** architect.

### WP2. The crate (rules, run-aware API, tests)
- **Goal:** §1, complete, with a frozen public API.
- **Steps:**
  - (half-day 1) API stubs plus `convert_runs` invariant tests, so that WP4–WP8 can start against the signatures;
  - scanners for protected spans; the ToCyrillic state machine; the ToLatin context mapper; case; the dictionary `build.rs`;
  - unit tests per rule (≥ 5 cases each);
  - the property tests: split invariance, case transforms, the apostrophe-variant equivalence, and round trip;
  - a throughput test (`#[ignore]`, release);
  - `no_std` check; wasm32 check.
- **Files:** `crates/uz-translit/{Cargo.toml, build.rs, src/lib.rs, src/{scan,l2c,c2l,case,dict}.rs, tests/{rules,props}.rs, CHANGELOG.md}`.
- **Done-check:**
  - `cargo nextest run -p uz-translit` passes;
  - `cargo check -p uz-translit --target wasm32-unknown-unknown` passes;
  - `cargo tree -p uz-translit -e normal,dev | wc -l` = 1 (zero dependencies);
  - `cargo test --release -p uz-translit -- --ignored throughput` prints ≥ 50 MB/s.
- **Estimate:** 2 d. **Tier:** builder.

### WP3. Golden list and exceptions dictionary (agent-reviewed, delegated)
- **Goal:** ≥ 500 reviewed pairs, valid in both directions, plus the dictionary, each row with a source.

**Sources, in order of trust:**
1. Examples in the official orthography rules (Latin 1995, Cyrillic), cited by rule number.
2. **Parallel official texts on lex.uz:** the same act published in uz-Cyrl and uz-Latn. Use the Labour Code first: it is HRM vocabulary.
   - Align by article and paragraph; extract word pairs with frequencies; store only the pairs and the document id, never the texts.
   - lex.uz Latin versions mix apostrophe characters and contain typos, so normalise R1 first, and send a row that disagrees with the rules to review, not straight in.
3. Official names published in both scripts (regions, cities, ministries); month and weekday names; common Russian loanwords from the rules' own lists.
4. The Phase 1 catalog pairs (P10), as a **regression corpus only**.
5. **Agent-generated candidates only to fill a rule quota** that the sources leave short. They carry `source=agent` and need a rule citation.

**Quotas (rows with `dir=both`, about 520 total):**

| Rule area | Rows |
|---|---|
| R1 apostrophes | 40 (each variant) |
| R2 tutuq | 40 |
| R3 digraphs | 60 |
| R4 s'h | 10 |
| R5 e/ye/э | 60 |
| R6 ё/ю/я | 50 |
| R7 ц | 30 |
| R10 case | 40 |
| R11 exceptions (incl. months, inflected) | 80 |
| Everyday and HRM vocabulary | ≥ 110 |

Plus `sentences.tsv`, about 80 rows, for context rules: R12 spans, R15 quotes, case runs, codes.

**Formats** (UTF-8, LF, TSV):
- `crates/uz-translit/data/golden.tsv`: `id  latin  cyrillic  dir(both|l2c|c2l)  rule  status  source  note`
- `crates/uz-translit/data/sentences.tsv`: the same columns.
- `crates/uz-translit/data/exceptions.tsv`: `latin  cyrillic  match(word|stem)  dir  status  source  note`
- Status values: `candidate | reviewed:agent-<model>:<date> | ambiguous | rejected`. Never `owner`.

**Review procedure:**
1. A builder (sonnet) drafts the candidates.
2. A differential script `scripts/translit/diff-plugin.mjs` runs the **owner's unmodified `translit.js` under Node as an oracle** next to our crate on every row and on the catalog corpus.
   - It writes a three-way table: expected / ours / plugin.
   - Running the owner's own code for comparison copies nothing. It is not ONLYOFFICE code.
3. An architect (opus, a different model from the drafter) reviews every row in batches of ≤ 100 against RULES.md and the source:
   - sets `reviewed:agent-opus:<date>`, fixes the row, or marks it `ambiguous` with a note;
   - a disagreement between sources is always `ambiguous`.
4. `ambiguous` rows are excluded from the pass/fail gate, but the gate report lists them, so the owner knows what to watch in use.
5. Stem entries shorter than 5 letters need a "no false hit" check: the reviewer greps the catalog corpus and the golden list for words that start with the stem.

**Tests** (in WP2's test files):
- `golden.rs` asserts `l2c(latin) == cyrillic` and `c2l(cyrillic) == latin` for every reviewed row, by `dir`;
- it asserts ≥ 500 reviewed `both` rows;
- `props.rs` checks the round trip `c2l(l2c(w)) == w` for every reviewed Latin form in golden **and** in the dictionary, plus generated sentences (case variants, apostrophe variants, random run splits, inserted protected spans), with 20,000 seeded cases.
- **Files:** the three TSVs, `scripts/translit/diff-plugin.mjs`, `docs/phase2/golden-review.md` (counts, ambiguous list, plugin disagreements by class).
- **Done-check:**
  - `cargo nextest run -p uz-translit golden` passes;
  - `awk -F'\t' '$4=="both" && $6 ~ /^reviewed:agent-/' crates/uz-translit/data/golden.tsv | wc -l` ≥ 500;
  - no row has `status` = `owner…`.
- **Estimate:** 2 d (1.25 drafting and harness, 0.75 review). **Tier:** builder drafts, architect reviews. Runs in parallel with WP2 and WP4 (no builds).

### WP4. Docs command (D1)
- **Goal:** §3, fully.
- **Steps:**
  1. `gen-domain` `[extensions]` support, with an xtask unit test.
  2. Patch `wordcraft/0120` (install lines, empty features, the `column.rs:178` arm), with a REGISTER row (category: new `translit`).
  3. Write `domains/docs/crates/docs-translit`: the collect, classify, convert, rebuild and selection-map steps; the tracked mode; the result JSON.
  4. Engine-level tests on `Session`:
     - one undo restores the document exactly (`doc == before`);
     - redo works;
     - a column selection converts only the selection;
     - a locked control is skipped;
     - field codes, `instr` and hyperlink targets are unchanged;
     - in a document whose Track Changes is on, the conversion produces one Delete and one Insert revision;
     - the digraph split across runs works.
  5. `cargo xt gen-domain --check docs`; lockdiff (new local packages only).
- **Files:** `xtask/src/domain.rs`, `domains/docs/domain.toml`, `patches/wordcraft/0120-*.patch`, `patches/REGISTER.md`, `domains/docs/crates/docs-translit/**`.
- **Done-check:**
  - `scripts/verify-queue.sh wordcraft` and `cargo xt register check` are green;
  - `cargo nextest run --manifest-path domains/docs/Cargo.toml -p docs-translit` passes;
  - `scripts/ci.sh check docs` is green (native and wasm32);
  - `docs-cli commands --json | grep tools.translitCyrl` finds the command (proves the install hook in the CLI).
- **Estimate:** 3 d. **Tier:** builder; an architect reviews the patch and the undo/tracked tests.

### WP5. Sheets command
- **Goal:** §4.
- **Steps:**
  - patch `gridcraft/0120` with the module, registry line and path dependency;
  - engine tests: a formula cell is untouched, a number is untouched, a table header is skipped, one undo step, workbook scope, the warning count;
  - check that the `0111` rustfmt patch regeneration still works with a new file in the queue.
- **Done-check:** `scripts/verify-queue.sh gridcraft`; `scripts/ci.sh test gridcraft` green, apart from the existing quarantine; `scripts/ci.sh check sheets`.
- **Estimate:** 1 d. **Tier:** builder.

### WP6. Slides command
- **Goal:** §5.
- **Steps:**
  - patch `deckcraft/0120`;
  - our own walker over shapes, groups, tables and notes;
  - engine tests: a text selection range, the selected shapes, the whole deck, a field run untouched, a link target untouched, one undo step.
- **Done-check:** `scripts/verify-queue.sh deckcraft`; `scripts/ci.sh test deckcraft` green with the quarantine (run with `--test-threads 1` if the known ui-egui crash appears); `scripts/ci.sh check slides`.
- **Estimate:** 1.25 d. **Tier:** builder.

### WP7. UI entries and i18n (3 apps)
- **Goal:** §6.
- **Steps:**
  - patches `<app>/0121`;
  - `cargo xt i18n extract`, then drafts, then the opus review;
  - status-message plumbing (each UI shows the result JSON as a status line);
  - check gridcraft keytips;
  - screenshots in uz-Latn and uz-Cyrl of the ribbon entry and the context menu, through the Phase 1 evidence method (Playwright web plus desktop window capture).
- **Files:** `patches/*/0121-*.patch`, `i18n/{shared,wordcraft,gridcraft,deckcraft}.tsv`, `docs/phase2/evidence/ui/`.
- **Done-check:**
  - `scripts/ci.sh i18n` green (lint, roundtrip, check-stale);
  - `scripts/overlay-tests.sh <d> office_overlay i18n` passes or the failure is quarantined with a reason;
  - the screenshots exist and show no tofu.
- **Estimate:** 1.5 d. **Tier:** builder; architect for the Uzbek review.

### WP8. WASM/npm package
- **Goal:** §7.
- **Files:** `crates/uz-translit-wasm/**`, `scripts/build-npm.sh`, `ci/npm/**`, `web/budgets.toml`, `scripts/ci.sh` (an `npm` job, and `sizes` extended).
- **Done-check:**
  - `scripts/ci.sh npm` builds `dist/npm/uz-translit-0.1.0.tgz`;
  - `node --test ci/npm` passes the golden list;
  - `scripts/ci.sh sizes` reports the package within budget;
  - the Vite smoke screenshot shows converted text.
- **Estimate:** 1 d. **Tier:** builder. Can start once WP2's API is frozen; it runs in parallel with WP4 because it is a small build.

### WP9. Formatting-preservation harness
- **Goal:** §8.
- **Steps:**
  - fixture scripts (Office COM);
  - the B/R save harness per app: `docs-translit` tests use Session directly; Sheets and Slides use `run --cmd` through their CLIs (`gridcraft-cli` `main.rs:153`, `deckcraft-cli` `main.rs:437-492`);
  - the XML comparer (§8.2) with the allowlists in code and a clear diff report.
- **Files:** `scripts/translit/make-fixtures.ps1`, fixture dirs, `domains/docs/crates/docs-translit/tests/preserve.rs`, `scripts/translit/preserve-check.sh` (Sheets and Slides).
- **Done-check:** `cargo nextest run --manifest-path domains/docs/Cargo.toml -p docs-translit preserve` passes for all 8 docx fixtures and both directions; `scripts/translit/preserve-check.sh` prints `IDENTICAL` for 2 xlsx and 2 pptx.
- **Estimate:** 1.5 d. **Tier:** builder.

### WP10. The 108-page document and performance
- **Goal:** the gate number, measured three ways.
- **Steps:**
  - **WP10a (day 1 of WP4, before the Docs design is frozen):**
    - generate the document with Word COM (`scripts/translit/make-108.ps1`). Seeded text from the golden vocabulary in each script, with headings, numbered lists, tables every few pages, headers and footers with PAGE, footnotes, hyperlinks, a TOC, mid-word bold/italic splits, lang tags and a text box;
    - Word's `ComputeStatistics(wdStatisticPages)` must be **108**; our engine's page count is recorded too;
    - two files: Latin and Cyrillic, each about 0.3 MB of text;
    - measure wordcraft's full-layout time on it with `docs-bind`'s `layout_ms()`, native and in wasm (P8).
  - **N1, headless native** (the reference): `docs-translit` `#[ignore]` test `perf_108`. Release build, idle machine, one warm-up, median of 5. It reports open (excluded), command, layout after the command, undo and redo.
  - **N2, desktop GUI:** the release `docs` exe, driven through the existing control channel. Run the command id, then wait for the next rendered frame (`ui.*` automation, `crates/ui-egui/src/control.rs`). Median of 5.
  - **N3, web:** the `docs-web` dist in Chromium (Playwright), the file opened through the 0001 host bridge. Run the command from the palette, then time from the keypress to the next presented frame (`requestAnimationFrame`), plus the command's own `ms` log line.
  - **Optional (0.5 d), for an apples-to-apples comparison:** run the owner's plugin on the same file in an ONLYOFFICE Docs container (Docker, run separately, nothing linked) and time it.
  - **If N2 or N3 is ≥ 2 s:**
    - first check that our part (convert + apply) is < 100 ms;
    - if layout is the cost, the options are an upstream layout patch (carried; architect-designed) or reporting honestly;
    - the owner decides. Nothing is hidden.
- **Files:** `scripts/translit/make-108.ps1`, the two committed docx files, `domains/docs/crates/docs-translit/tests/perf.rs`, `ci/web-smoke/translit-perf.spec.*`, `docs/phase2/perf.md`.
- **Done-check:** `docs/phase2/perf.md` has N1, N2 and N3 with the raw runs, the machine state and the commit; the gate line reads "108 pages: X s (desktop), Y s (web); ONLYOFFICE plugin 62 s".
- **Estimate:** 1.25 d (+0.5 optional; an optimisation, if needed, is a new decision). **Tier:** builder; architect if optimisation is needed.

### WP11. CI, register, docs
- **Goal:** everything Phase 2 adds runs in CI and is documented.
- **Steps:**
  - add the `npm` job to `quick` when `crates/uz-translit*` changes;
  - `test-ours` already covers the crate and `docs-translit`;
  - add a `translit` REGISTER category (`register check` knows the category list);
  - `docs/UPSTREAM.md`: the 0120/0121 carry notes (hot files: `ribbon.rs`, `canvas.rs`, `grid.rs`, `column.rs`);
  - README: a usage section.
- **Done-check:** `scripts/ci.sh quick` green; `cargo xt register check` OK.
- **Estimate:** 0.5 d. **Tier:** builder.

### WP12. Gate: adversarial review and report
- **Goal:** prove the three gate items; report to the owner in Uzbek.
- **Steps:**
  - an architect (opus) adversarial review: rules against Appendix A, a sample of 50 golden rows re-checked, the patches, undo/track safety, and data loss risks (P7, gridcraft header cells);
  - `scripts/ci.sh full` (the npm job included);
  - a second sync dry run with the new patches (rebase cost of 0120/0121);
  - update STATUS; the gate report.
- **Files:** `docs/phase2/gate-review.md`, `docs/STATUS.md`.
- **Done-check:** `ci.sh full` exit 0; the gate table in `gate-review.md` has evidence links for every item.
- **Estimate:** 1 d. **Tier:** architect.

### Parallel vs serial
```
day:   1    2    3    4    5    6    7    8    9   10   11   12   13   14   15   16   17
WP0 ▌
WP1  ▐█▌
WP2     ███████▌                                         (crate builds: seconds)
WP3       ████████████                                   (text work, no builds; runs beside everything)
WP10a        ██                                          (Word COM + one docs build: first domain build)
WP4            ███████████▌                              (docs builds, serial)
WP8            ███▌                                      (tiny build; slot between docs builds)
WP5                        ████                          (sheets builds)
WP6                            █████                     (slides builds)
WP7                                 ██████               (3 ui-egui rebuilds, serial)
WP9                   ███▌           ███                 (docx part after WP4; xlsx/pptx after WP5/6)
WP10 (N1-N3)                              █████
WP11                                           ██
WP12                                              ████
```
**Critical path:** WP0 → WP1 → WP2 (API stub) → WP10a → WP4 → WP9 (docx) → WP10 → WP12, about 11 of the 17 days. Everything else fills build gaps. WP3 is the off-path risk: its review must finish before WP12.

---

## 10. Estimate, risks, schedule

### 10.1 Estimate (stream-days)
| WP | Low | Likely | High |
|---|---|---|---|
| WP0 | 0.25 | 0.25 | 0.5 |
| WP1 | 0.5 | 0.75 | 1 |
| WP2 | 1.5 | 2 | 3 |
| WP3 | 1.5 | 2 | 3 |
| WP4 | 2 | 3 | 4.5 |
| WP5 | 0.75 | 1 | 1.5 |
| WP6 | 0.75 | 1.25 | 2 |
| WP7 | 1 | 1.5 | 2 |
| WP8 | 0.75 | 1 | 1.5 |
| WP9 | 1 | 1.5 | 2 |
| WP10 | 1 | 1.25 | 2.5 |
| WP11 | 0.25 | 0.5 | 0.5 |
| WP12 | 1 | 1 | 1.5 |
| **Total** | **12.5** | **17** | **24** |

Against SIZING §2.1 (15.75 likely):
- **Down:** owner review hours are gone (delegated); Sheets is simpler (no rich text, P3).
- **Up:** the run-aware design and its property tests; the agent review of the list (0.75 d); the gen-domain extension; three perf measurements (web included); the P7 safety work.
- The risk buffer stays at 4 days for an un-optimisable layout (R1).

Calendar at about 4 stream-days a week: **about 4.5 weeks** from the Phase 1 "go".

### 10.2 Risks
| # | Risk | Likelihood / impact | Mitigation | Proven by |
|---|---|---|---|---|
| R1 | Full relayout of 108 pages exceeds 2 s on web (P8 projects 1.7–2.4 s in emulator wasm) | medium / gate | Measure in WP10a before the design freezes; skip unchanged paragraphs; report N1/N2/N3 honestly; a layout patch only with the owner's say | WP10 |
| R2 | Language-tag policy D1 is wrong for the real corpus | medium / users | A reversible default plus a dialog option; counts in the result; re-measure when the corpus arrives (B-14) | WP4 tests, STATUS |
| R3 | Selection-collapse trap (P7): a column selection becomes "whole document" | high if missed / data | The `per_row` line in 0120 plus a test; an explicit `scope` param | WP4 |
| R4 | Sheets: converting text breaks formulas with text literals, lookups or table headers | medium / data semantics | Skip table headers; never touch formulas; a warning count in the UI | WP5 |
| R5 | gridcraft drops cell rich text on any save (upstream) | certain / fidelity | Out of scope; recorded for Phase 8; the B/R baseline keeps it out of our diff | WP9 |
| R6 | Ambiguous orthography (ь in inflected month names, ц, e/э, ПФ- codes) | certain / correctness | `ambiguous` status, listed at the gate; the owner reports in use | WP3, WP12 |
| R7 | Golden sources are noisy (lex.uz Latin apostrophes and typos) | medium | Normalise; rule disagreement goes to review; three-way diff against the plugin | WP3 |
| R8 | Dictionary stems give false hits (a short stem prefixing an unrelated word) | medium | `word` vs `stem` entries; a minimum stem length of 5 for stems unless reviewed; corpus grep | WP3 |
| R9 | Carry cost: 2 new patches per app; `ribbon.rs`/`canvas.rs`/`grid.rs`/`column.rs` are hot | certain / weekly | One-token edits, a new file for engine modules, measured in the WP12 dry run | WP12 |
| R10 | HRM mobile (Hermes) cannot run wasm | certain / scope | Out of Phase 2 (web only); WebView or a JS port later | report |
| R11 | No corpus: perf and preservation are proven on synthetic Office-written files only | certain | Office COM files (realistic run splits); re-run when the corpus arrives | WP9, WP10 |
| R12 | `gen-domain` extension or check-cfg trouble in the upstream workspace | low | Empty `[features]` in the upstream app manifests; the `ci.sh test wordcraft` upstream build | WP4 |
| R13 | Phase 1 gate not yet passed | — | Phase 2 starts only after the owner's "go" | — |

### 10.3 Exit-gate items → proving WP
| Gate item (brief) | Evidence | WP |
|---|---|---|
| Golden tests pass | `cargo nextest run -p uz-translit` (≥ 500 reviewed `both` rows, round-trip property test); the same list through the npm wasm | WP2, WP3, WP8 |
| 108-page .docx < 2 s in the editor; number reported vs 62 s | `docs/phase2/perf.md`: N1 headless, N2 desktop, N3 web | WP10 (WP10a early) |
| Formatting identical apart from text | the docx preservation tests on 8 fixtures + the 108-page file, both directions; xlsx/pptx checks | WP9 |
| Brief task 2: Docs command (selection/whole, both directions, one undo, optional tracked, cross-run digraphs, skip fields/links) | docs-translit engine tests | WP4 |
| Brief task 3: Sheets (cell text, not formulas) and Slides (text frames, notes) | engine tests + preserve-check | WP5, WP6, WP9 |
| Brief task 4: WASM/npm package | `dist/npm/uz-translit-*.tgz`, node tests, Vite smoke, size budget | WP8 |
| UI in 4 languages | screenshots, `ci.sh i18n` | WP7 |

### 10.4 Decisions for the owner (few: the list review is delegated)
1. **Phase 2 start** after the Phase 1 gate.
2. **Override D1 if needed** (language tags: the default converts ru-/en-tagged text, because Uzbek is commonly typed on those layouts).
3. **C1:** codes such as PF-1234 / ПФ-5050 stay as typed (Appendix A), although Cyrillic documents usually write ПФ-. Confirm or change.
4. **Later:** an npm registry and scope before any publish. Whether to add an ONLYOFFICE side-by-side timing on the same 108-page file (optional, WP10).
