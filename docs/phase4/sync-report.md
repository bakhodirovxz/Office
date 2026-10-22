# Phase 4 WP0: upstream sync of 2026-10-22

Branch `phase-4`. All five repos moved to current upstream with `scripts/sync-upstream.sh` (`--apply --resolve`, gates
`quick`), one repo at a time, from the cache clones `E:/office-upstream/<repo>`. Per-repo script reports:
`E:/office-target/p1/sync/reports/2026-10-22-<repo>.md`; the last one is committed as `docs/phase1/sync-reports/2026-10-22.md`.
Per-patch rows: `patches/rebase-log.tsv` (date `2026-10-22`).

## 1. BASE per repo

NEW is the script's default: the newest release tag that contains BASE, or `origin/HEAD` when the repo has no tags.

| repo | old BASE | new BASE | upstream commits | commits since the tag on `origin/HEAD` (not synced) |
|---|---|---|---|---|
| wordcraft | `eae2421` (v0.7.0-274) | **`2e6db0b` v0.9.0** (2026-10-19) | 281 | 562 (`8e98636`; includes "Merge 178 open PRs (backlog round 1)", #3555) |
| gridcraft | `f1894c6` (v0.7.0-7) | **`2ebdf16` v0.10.0** (2026-10-21) | 77 | 605 (`06920de`; a merge of the `next` branch, #2226, 293 commits dated 2026-10-22) |
| deckcraft | `1dc6972` (v0.7.0-721) | **`fe510bd` v0.9.0** (2026-10-19) | 60 | 14 (`128cb0b`) |
| pdfcraft | `924c4da` (v0.8.0-220) | **`3e30645` v0.11.0** (2026-10-21) | 264 | 17 (`88dad98`) |
| craft-libs | `15ed713` | **`1b9cf19`** `origin/HEAD` (2026-10-21, no tags) | 29 | 0 |
| craft-fonts | `3d9ab3a` | unchanged (moved by hand only, never by the script) | - | 6 (`43a9944`), not needed: every slides check and build passed on the pin |

Why the tags and not `origin/HEAD`: the script's documented default, and the Phase 3 dry run (gate review section 6) measured
exactly these targets (only pdfcraft moved on from v0.10.1 to v0.11.0, and craft-libs from `1f80537`). wordcraft and gridcraft
`origin/HEAD` are hours-old, untagged mass merges (1,167 commits between them); syncing to them is the next sync, not this one.

## 2. Patches

73 patches rebased (wordcraft 13, gridcraft 42, deckcraft 14, pdfcraft 4); the Phase 3 dry run said 53 clean, 6 3-way, 13 conflicting.

| | clean | 3-way | resolved (hand-merge) | amended | regenerated | dropped | new |
|---|---|---|---|---|---|---|---|
| wordcraft | 9 | 2 (0003, 0004) | 2 (0001, 0101) | - | - | **1 (0003)** | - |
| gridcraft | 30 | 1 (0005) | 9 (0003, 0009, 0010, 0015, 0018, 0020, 0022, 0028, 0029) | **1 (0130)** | 1 (0111 rustfmt) | - | - |
| deckcraft | 11 | 1 (0121) | 2 (0103, 0124) | - | - | - | - |
| pdfcraft | 3 | 1 (0100) | - | - | - | - | **1 (0111, D7)** |
| **total** | **53** | **5** | **13** | **1** | **1** | **1** | **1** |

After the sync the queue holds 73 patches (wordcraft 12, gridcraft 42, deckcraft 14, pdfcraft 5), 73 REGISTER rows.
Two more patches were regenerated after the apply, when the full test runs found semantic conflicts: wordcraft 0101 (a new
upstream ribbon test counts the Share button, which the overlay hides) and pdfcraft 0111 (upstream pins ru.tsv by SHA-256
in ATTRIBUTION.toml). Both are separate commits with a REGISTER note.

### How each conflict was resolved (patch intent kept, tests kept, REGISTER row kept and annotated)

| patch | conflict | resolution | interdiff lines |
|---|---|---|---|
| wordcraft 0001 host bridge | `web.rs` `WebShell`, `web-sys` features | upstream added stored preferences (`prefs: PrefsSync`, #2132) to the same struct and update loop; kept both: `WebShell { app, inbox, messages, prefs, bridge }`, preferences stored before `bridge.sync`; feature list = upstream's + `MessageEvent`, `UrlSearchParams` | 22 |
| wordcraft 0101 chrome off | `ribbon.rs` Share button | upstream moved the buttons into a width plan (`plan_strip`, #2026); the Share button block and its share of `button_widths` (plus its 6 px gap) are both gated on `office_overlay::SHARE` | 97 |
| deckcraft 0103 file metadata | `format/src/lib.rs` | upstream's save now separates chart parts first; the generator string keeps `OFFICE_PRODUCT_NAME` inside the new code | 18 |
| deckcraft 0124 whole strings | `cmd/file.rs` damage notice | upstream now also reports slides it could not read (`Damage { media, slides }`). Five whole templates instead of two: media one/many, slides one/many (both exactly upstream's English), and both at once with `{n}` and `{m}`. `localize_message` matches any `{x}` count (`fill_template`); the toast test covers all five; `xtask` `OWN_DECK_0124` lists them | 80 |
| gridcraft 0003 | `xlsx/src/tests/mod.rs` | module list: both modules | 2 |
| gridcraft 0009 | `calc/src/tests.rs` | both tests (upstream's new MEDIAN test, ours) | 2 |
| gridcraft 0010 | `cmd/file.rs` open | `drop(bytes)` kept after upstream's new `open_bytes_with(.., csvAsText)` | 2 |
| gridcraft 0015 | `cmd/file.rs`, `io.rs` | `open_bytes_calculated(name, bytes, password, csv_as_text)`; upstream's `open_bytes_with` wraps it; `open_plain` takes `csv_as_text` and returns `calculated` | 33 |
| gridcraft 0018 | `cmd/file.rs` | the read-from-file branch kept; the bytes branch passes `csvAsText` | 5 |
| gridcraft 0020 | `Cargo.lock` | `gridcraft-pdf` 0.10.0 plus our `epaint_default_fonts` dependency (third sync in a row: a hot file) | 4 |
| gridcraft 0022 | `docs/localization-parity.md` | upstream's new "names in any script" bullet kept, its "standard fonts only" line replaced by ours | 6 |
| **gridcraft 0028** NumLocale | `display.rs`, `io.rs`, `cmd/file.rs` | see section 3 | 53 |
| **gridcraft 0029** NumLocale | `cmd/print.rs:1098` | see section 3 | 5 |
| **gridcraft 0130** NumLocale UI | no textual conflict; a **semantic** one: upstream's new `grid.rs` test (`#1163`) calls `display_text` with 7 arguments, 0130 adds the 8th (`NumLocale`). Found by `cargo check --workspace --all-targets`; amended with `NumLocale::EnUs` through the new `--amend 0130` | 14 |

Resolution method: a manual `git am -3` per repo in `E:/office-target/p1/sync/wp0-<repo>`, each fix rustfmt-checked, then
compile-checked and tested in a scratch monorepo; the real run used `--resolve` and answered each stop with the prepared tree
(`git read-tree --reset -u <commit>`), so the script did the rebase, rustfmt patch, materialisation, register and log.

### Dropped: wordcraft 0003 (upstream does it)
Upstream handles `ImeEvent::DeleteSurrounding` since 82bf806 (#1949, part of #1480): `keys.rs` `delete_surrounding` erases
by characters on both sides of the caret, joins the typing undo step, keeps the erased text's formatting, and ignores the
event inside an equation on purpose. After the 3-way apply our arm was unreachable (the clippy ratchet reported `unreachable
pattern` in `keys.rs`), and our equation arm contradicted upstream's choice. Dropped; the queue rebuilt without it (only the
`[PATCH n/12]` headers of the other 12 files change); REGISTER has a new "Dropped patches" section. Upstream's
`delete_surrounding_*` tests cover our measured case (`teh` to `the`). Not re-measured on a phone (Android is deferred).

### New: pdfcraft 0111 (D7)
Upstream's `crates/ui-egui/src/i18n/ru.tsv` still read `Move down` = `Переместить вdownarrow` at v0.11.0 and at `origin/HEAD`
`88dad98`. One-line carried patch, REGISTER row `pdfcraft/0111`, `i18n/pdfcraft.tsv` re-extracted (ru `Переместить вниз`,
status `upstream`), D7 marked fixed in `docs/phase3/defects.md`.

## 3. NumLocale against `value_text_as` (0028, 0029)

Upstream's #1163 split `value_text(wb, sheet, c)` into `value_text` plus `value_text_as(wb, v, code)` so print can pass the
code of the cell's effective look (a conditional format may override it). Smallest re-threading:

- `display.rs`: `value_text_in(wb, sheet, c, loc)` now calls a new `value_text_as_in(wb, v, code, loc)`, which holds the
  former body with `format_in(.., loc)`. Upstream's `value_text_as(wb, v, code)` is a one-line wrapper with `NumLocale::EnUs`,
  exactly like every other `_in` pair (D8). `value_text` is unchanged (`value_text_in(.., EnUs)`).
- `print.rs`: the cell-text branch calls `value_text_as_in(wb, &v, code, b.loc)`, so upstream's conditional-format code is
  kept and the PDF text follows the locale.
- `io.rs` / `cmd/file.rs`: `open_bytes_calculated_in(name, bytes, password, csv_as_text, loc)`; `open_plain` and the two
  `CsvOptions` carry both upstream's `parse_values: !csv_as_text` and our `locale: loc`.

en-US byte-identical: by construction every en-US entry point is the `_in` function with `EnUs`, and `format_in(EnUs)`
is `format_value` with the unchanged code (the built-in date swap returns `None` for `EnUs`).

Re-validation:
- **Scratch tree** (manual queue on the new BASEs): `cargo test` of gridcraft core, numfmt (including `tests/en_us_golden.rs`,
  1,500 en-US rows recorded before NumLocale), formula (`decimal_comma_input`), engine `tests/numlocale.rs` and `tests/translit`,
  engine `print` lib tests (PDF text by locale, `&D`/`&T`), ui-egui `tests/num_locale.rs`, `tests/formula_locale.rs` and the
  `grid` lib tests: **192 passed, 0 failed**.
- **Synced tree**, `ci.sh test gridcraft` (whole upstream workspace, overlay on, quarantine): **ok**.
- **WP14 matrix re-measured** with the synced release `sheets-cli` (`E:/office-target/p3/bin/sync-sheets-cli.exe`,
  `E:/office-target/p3/matrix-p4/matrix.md`, same generators and driver as Phase 3): the output is **byte-identical** to the
  Phase 3 `matrix.md`. ru screen equals Excel ru in 21 of 24 and 19 of 20 rows (unchanged); for ru, uz-Latn and uz-Cyrl every
  row's screen text is in the CSV and in the PDF text layer; the en column is unchanged, so en-US stays byte-identical on the
  CLI surfaces (screen, CSV, PDF).
- **m10** (the honour-locale probe) re-checked only as an observation: the synced CLI gives different CSV and PDF text for
  `en` and `ru` in every number row of the matrix, so the probe's premise holds. The probe's code change (probe the PDF path,
  require en and ru to differ) is still open: PLAN WP20 lists it.

## 4. i18n

| catalog | new rows | uz-Latn / uz-Cyrl | ru |
|---|---|---|---|
| wordcraft | 58 | drafted, `draft:agent-opus:2026-10-22` | upstream's Russian (status `upstream`); 1 upstream ru change merged (`Accessibility` = `Доступность`) |
| deckcraft | 8 (5 upstream strings, 3 our new 0124 templates) | drafted | not kept for deckcraft (no upstream catalog; every deckcraft row has ru `todo`) |
| pdfcraft | 2 | drafted | upstream's Russian; D7 row corrected |
| gridcraft | 0 | - | - |

68 new rows, 136 Uzbek cells drafted, all for review (release exports skip drafts until reviewed). `cargo xt i18n lint`:
0 errors (645 warnings, 643 before); `check`: 8108 rows, all ids match; `check-stale` ok; `roundtrip` OK.
`i18n/converter.tsv` was re-sorted by the extract (the Phase 3 "Number format" row had been appended out of order; no value changed).

## 5. Interfaces the converter depends on (`docs/UPSTREAM.md`)

Checked at the new BASEs (`upstream/<app>/apps/*-cli/src`): every subcommand and flag of the table in `docs/UPSTREAM.md`
exists. docs-cli `convert`, `run --file --cmd --print --save`; sheets-cli `info --json` (`Args::parse(.., &["json"])`),
`convert --sheet`, `run --in --cmd --out`, `cat`; slides-cli `convert`, `run --in --cmd --print --save`, `render --all/--slide
--scale`, `info`; pdf-cli `text --page`, `info`, `run --script --root`, `split --every/--before --out-dir`, `combine --out`,
`extract --pages --out`, `render --page --dpi --out`. Command and tool ids found in the sources: `file.exportPdf`,
`pageLayout.headerFooter`, `insert.header`, `design.watermark`, `file.properties`, `design.headerFooter`, `tools.translit*`
(our 0120/0122 hooks), `doc_open`, `doc_save`, `page_render`, `doc_create`, `doc_export_office`, `doc_header_footer`,
`doc_watermark`, `page_add_text`. The converter is proven against them by `convert-e2e` (section 6).

## 6. Gates

Run on the synced tree (worktree `E:/office-target/p1/sync/wp0-main`, branch `wp0-sync`), then rebased onto `phase-4`
(only `i18n/converter.tsv` conflicted, phase-4's version kept; `web/budgets.toml` merged cleanly) and the fast gates re-run there.

| gate | result |
|---|---|
| `scripts/verify-queue.sh all` | wordcraft 12, gridcraft 42, deckcraft 14, pdfcraft 5, craft-libs 0 patches: **tree OK** (also on the rebased branch) |
| `cargo xt register check` | **73 patches, 73 rows, OK** |
| `sync-upstream.sh --apply --gates quick`, per repo | craft-libs, pdfcraft, gridcraft: **PASS**. deckcraft, wordcraft: every gate PASS except `i18n-lint` (`missing-p1`: new upstream P1 rows need at least a draft); finished by hand with the drafts, as section 7 of their reports says |
| `--gates full` | not run as such (it rebuilds everything in a cold `$OFFICE_TARGET/sync/full`). Closest documented equivalent, on the merged tree with the warm targets: `ci.sh build <d> all` x4 and `ci.sh test <app>` x4 (in `quick`) |
| `ci.sh quick` (`CI_DIFF_BASE` = the pre-sync commit, so all four domains count as affected) | queue, gen-check, deny, brand, i18n, check x4, bind, convert-ui, test gridcraft (1,765 passed, 1 quarantined), test deckcraft (1,740 passed, 4 quarantined): **ok**. First-run failures, all fixed and re-run green: `lint` (2 new clippy warnings: wordcraft `keys.rs` unreachable pattern, which retired patch 0003; gridcraft `desktop_theme.rs` upstream `let_unit_value`, baselined), `test wordcraft` (3: the 0101 Share test fix, two new upstream env tests quarantined; re-run 3,454 passed, 20 quarantined), `test pdfcraft` (1: the D7 patch changed a hash-pinned catalog; 0111 now updates `ATTRIBUTION.toml`; `xtask assets` 13/13 pass). pdfcraft otherwise 3,795 passed, 28 quarantined (22 still failing, 6 passed-flaky), 1 unrunnable binary (`store_keys`, Defender, as before) |
| `ci.sh build docs/sheets/slides/pdf all` (release desktop, web, CLI) | **ok** (sheets desktop re-run: the first run was killed by another agent's `kill` and lock removal at 15:44) |
| `ci.sh convert-e2e` | **ok**: API e2e 2/2, `directions.sh` **43 PASS / 0 FAIL**, converter Playwright 12 passed, 1 skipped (screenshots) |
| `ci.sh translit-preserve` | **ok** |
| `ci.sh sizes` | **ok** after the base re-measure (below) |
| `cargo xt gen-domain --check all`, `cargo xt i18n check-stale / roundtrip / lint / check` | ok, round trip OK, 0 errors (645 warnings), 8,109 rows all ids match (rebased branch) |

### Web size budgets: `base_br` re-measured at the new BASEs

The first `sizes` run failed: docs 9,845,949 B against a budget of 9,573,698 (old `base_br` + `delta_br_max`), slides 754 B
of headroom. The old `base_br` was the pristine build at the old BASE, so the excess was upstream growth, not ours. Re-measured
as in `docs/phase1/wp6-final.md`: a scratch worktree with the four pristine trees at the new BASEs, `OFFICE_NO_OVERLAY=1
OFFICE_NO_I18N=1 scripts/build.sh <d> web --release`, scratch-only adjustments (no `--locked`, no docs extension crate, no
sheets `bundled-pdf-font`, an index.html without the overlay favicon), slides with the craft-fonts embedded. Bytes, wasm,
`brotli -q 11 --lgwin=24`:

| domain | old `base_br` | new `base_br` (pristine) | upstream growth | ours (patched dist) | **our delta** | delta at Phase 2 WP12 | `delta_br_max` | new budget | headroom |
|---|---|---|---|---|---|---|---|---|---|
| docs | 9,377,090 | **9,675,040** | +297,950 (+3.2 %) | 9,845,949 | **170,909** | 177,150 | 196,608 (unchanged) | 9,871,648 | 25,699 |
| sheets | 4,733,055 | **4,737,395** | +4,340 | 4,909,511 | **172,116** | 168,712 | 196,608 (unchanged) | 4,934,003 | 24,492 |
| slides | 9,302,764 | **9,327,609** | +24,845 | 9,465,850 | **138,241** | 139,894 | 163,840 (unchanged) | 9,491,449 | 25,599 |
| pdf | 9,395,853 | **9,435,622** | +39,769 | 9,569,362 | **133,740** | 137,654 | 163,840 (unchanged) | 9,599,462 | 30,100 |

Our delta stayed inside its limit everywhere (docs, slides and pdf shrank a little; sheets +3,404 B, the NumLocale and sync
changes), so no `delta_br_max` was raised. Upstream growth is under the 5 % `upstream_growth_warn` everywhere. JS glue brotli:
20,027-20,785 B (max 24,576). `web/budgets.toml` carries the new `base_sha`, `base_raw`, `base_gzip`, `base_br`, `js_br`.

## 7. Lock and dependency changes

`cargo xt lockdiff` (domain lock against upstream lock), each domain: **0 version changes, 0 foreign additions**
(removed dev/xtask entries: docs 34, sheets 18, slides 38, pdf 21). Domain locks against the Phase 3 locks: only the upstream
workspace crates' own version bumps, plus in pdf `rustls-webpki` 0.103.15 to 0.103.16, `webpki-root-certs` 1.0.9 to 1.0.10
and the new `craft-docpath` (craft-libs). No egui, wgpu, winit or wasm-bindgen change; no rust-version change. `cargo deny`
passes for all four domains. No version conflict needed a decision.

## 8. Hours (input to the sync-cost measurement, PLAN 5.4)

Agent wall time, one agent (opus), 2026-10-22, started 11:45. Machine time ran on two build slots shared with another
agent's MSVC and services builds, so the gate hours are longer than an idle machine would need.

| step | wall time | of which agent work |
|---|---|---|
| read the rules, scripts, Phase 3 findings; fetch; choose NEW | 0.2 h | 0.2 h |
| manual rebase and hand-merge of the 13 conflicts (deckcraft 2, wordcraft 2, gridcraft 9) | 0.4 h | 0.4 h |
| scratch compile check of all four workspaces (all targets, native + wasm32) and targeted tests; found the 0130 semantic conflict | 0.6 h | 0.1 h |
| sync tooling fixes found on the way (`--amend`, empty-register bug, exclusive rustc wait) | 0.4 h | 0.4 h |
| five `--apply` runs (gates quick), two finished by hand for i18n-lint | 1.4 h | 0.3 h |
| D7 patch, 68 i18n drafts, REGISTER notes | 0.6 h | 0.6 h |
| gates (ci quick, release builds of 4 domains, convert-e2e, translit-preserve), the WP14 matrix, base_br re-measure, fixes they found (wordcraft 0003 drop, 0101 test, quarantine, clippy baseline) | about 10 h (shared slots: every CI job waited for the same lock key; the pdfcraft and gridcraft test builds took 1.5-3 h each) | about 1.3 h |

Resolver time for the 13 hand-merges plus the amend, from the prepared resolutions: about **38 minutes** (deckcraft 0103 2,
0124 6; wordcraft 0001 3, 0101 4; gridcraft 0003 1, 0009 1, 0010 2, 0015 4, 0018 2, 0020 1, 0022 1, 0028 6, 0029 2;
0130 amend 3). Upstream distance: 711 commits (281 + 77 + 60 + 264 + 29). That is about **5.3 resolver minutes per 100
upstream commits**, against 2.3 at the 67-commit sync of 2026-10-19. The `minutes` column of `rebase-log.tsv` for the
resolved rows is the script's polling time with the prepared answer (seconds), not resolver time; the numbers above are.
The total agent time for the whole sync including gates and fixes is about 4.5 h; the plan's estimate for WP0a was 1.5 d.

Hot files (conflicted in two consecutive syncs): gridcraft `Cargo.lock` (0020, third time) and gridcraft `crates/engine/src/io.rs`
/ `cmd/file.rs` (0010, 0015, 0018, 0028: all four patches meet upstream's file-open path). wordcraft `ribbon.rs` (0101).

## 9. New upstream features worth knowing

- **craft-libs:** a shared `craft-window` crate for window decorations (relevant to the Phase 4 unified shell), `craft-i18n`
  (catalog lookup with locale fallback, plurals, placeholder filling, a pseudo locale: a possible future replacement for our
  per-app catalog plumbing), `craft-docpath` (checks paths stored in documents; pdf links it now), `craft-media`.
- **wordcraft 0.9.0:** input-method rewrites (`DeleteSurrounding`) done right, which retired our 0003; stored web preferences
  (`localStorage`, #2132); a tab strip that gives way in order (#2026); many dialogs (Drop Cap, Convert Text to Table,
  AutoFit, Multilevel List Library, Spike, font import), ligature caret, RTF heading fidelity, content-control fixes.
- **gridcraft 0.10.0:** right-to-left sheets (`.xls`, HTML export, text boxes), `value_text_as` (conditional-format number
  format in print, #1163), CSV "open as text" (`csvAsText`), Ctrl+Enter number formats, a Windows desktop-theme watcher.
- **deckcraft 0.9.0:** a PPTX slide that cannot be read is reported instead of dropped (#203), PNG export with a transparent
  background (#784), chart parts kept through native saves (#851), many open-time caps against absurd files.
- **pdfcraft 0.11.0:** fills and border widths for text boxes and callouts, redaction clears image pixels and evaluates OCMD
  visibility, Office export follows deep forms and says where it stops, a Windows-store certificate filter for signing.

## 10. Risks and findings

- **The next sync is big.** wordcraft and gridcraft `origin/HEAD` are 562 and 605 commits past the tags synced here, from
  two mass merges on 2026-10-22 (178 backlog PRs; the `next` branch). Expect more conflicts in the same hot files.
- **Semantic conflicts are invisible to `git am`.** This sync had three: gridcraft 0130 against a new `grid.rs` test, wordcraft
  0101 against a new ribbon test, pdfcraft 0111 against upstream's catalog hash pin. Only `cargo check --all-targets` and the
  full test run found them. `--amend` handles the first kind in the script now; a quick gate that also runs the tests of the
  crates a patch touches would find the others before `--apply`.
- **`--apply` cannot pass when upstream adds P1 strings.** The i18n merge adds them as `todo` and `lint` then fails
  (`missing-p1`). Finished by hand twice here. Fix: an `--i18n-drafts <tsv>` hook, or a sync gate that treats new `todo` P1 rows
  as a warning and leaves the error to the next CI run. A PLAN decision.
- **68 Uzbek drafts await review** (`draft:agent-opus:2026-10-22`); release exports skip drafts, so these strings show in English
  until reviewed.
- **Tooling found on the way** (fixed and committed): a craft-libs-only sync blanked `REGISTER.md` (empty-log awk bug);
  `wait_no_rustc` stalled every gate behind any other agent's build. Not fixed: `cargo xt` bakes the repo root in at compile
  time (`CARGO_MANIFEST_DIR`), and cargo does not rebuild xtask for another worktree with the same layout, so a shared
  `CARGO_TARGET_DIR` can run one worktree's xtask on another tree. The sync script is safe (a fresh scratch checkout has newer
  mtimes, so xtask rebuilds); manual runs in a second worktree need their own target dir.
- **An incident on this machine, caused by me and repaired:** `git worktree remove --force` on my pristine scratch worktree
  followed its `upstream/craft-fonts` junction and emptied the real `C:/Users/uge226/Desktop/office/upstream/craft-fonts`
  (about 19:05). Restored within minutes from the cache at the pinned sha; `scripts/fetch-fonts.sh --verify`: 31 files OK.
  A slides build by another agent that read the fonts in that window may have embedded none: rebuild any slides artifact
  made between 19:00 and 19:15. Lesson: remove a junction with `rmdir` before removing a worktree.
- **Another agent killed one of my builds** (`kill` plus removal of `BUILD.lock.1`, 15:44); the sheets desktop build was re-run.
- **wordcraft 0003 is dropped** on upstream's word; the Android Gboard case was not re-measured on a phone (Android is deferred).
- **craft-fonts** stays at `3d9ab3a` (6 commits behind); every slides check and build passes on it.
- **Not done here:** WP0b (re-grep of the PLAN references at the new BASEs), the m10 probe code change (WP20), a `--gates full`
  run in the cold sync target.
