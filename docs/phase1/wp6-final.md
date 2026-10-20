# WP6 final: size budgets, clippy ratchet, quarantine review, CI results

Run 2026-10-18/19 on branch `phase-1` (BASE: wordcraft eae2421, gridcraft f1894c6, deckcraft 1dc6972, pdfcraft 924c4da).
Machine: this PC, one build at a time, `-j 4`, `CARGO_HTTP_CHECK_REVOKE=false`, `source scripts/env.sh`.

## 1. Web size budgets (`web/budgets.toml`)

### What was measured
- **Our side:** the four release web dists were stale (built before the Uzbek language hooks and the catalog resync), so all four were rebuilt
  with `scripts/build.sh <d> web --release` (`RUSTC_WRAPPER` unset for web), overlay on, Uzbek catalogs exported, fallback fonts in.
  Build times: docs 1,301 s, sheets 573 s, pdf 1,034 s, slides 1,475 s.
- **Pristine side:** the old `base_br` values came from the Phase 0 base, so the "delta" of STATUS (+47..117 KB) was an upper bound that mixed our
  layer with upstream growth between the two bases. For this step the pristine web wasm was measured **at the current BASE** with the same
  tool: a scratch worktree (`E:/office-target/p1/wt/pristine`, not committed) with `upstream/<app>` replaced by the pristine vendor commit (no queue,
  no overlay, no catalogs), the same domain wrapper, `OFFICE_NO_OVERLAY=1 OFFICE_NO_I18N=1 scripts/build.sh <d> web --release`.
  Two scratch-only adjustments: `--locked` removed (the pristine lock differs from the queue's), and the sheets CLI feature `bundled-pdf-font`
  (added by the queue) dropped from the scratch manifest. slides is built with the craft-fonts embedded, as in Phase 0 and as `build.sh` does
  (a first pristine slides build ran without the fonts, 6.8 MB brotli, and was discarded: the fonts directory is untracked and was missing in the worktree).
- All numbers: `brotli -q 11 --lgwin=24`, `gzip -9`, bytes, wasm only unless noted.

### Result

| domain | wasm raw (ours / pristine) | gzip -9 (ours / pristine) | brotli ours | brotli pristine (`base_br`) | **delta** | delta % | JS brotli (ours / pristine) |
|---|---|---|---|---|---|---|---|
| docs (wordcraft) | 42,621,161 / 42,219,232 | 15,916,854 / 15,735,172 | 9,482,712 | 9,377,090 | **+105,622** | +1.13 % | 20,778 / 20,548 |
| sheets (gridcraft) | 19,328,641 / 18,979,436 | 7,209,238 / 7,044,454 | 4,855,241 | 4,733,055 | **+122,186** | +2.58 % | 20,050 / 20,041 |
| slides (deckcraft, craft-fonts on) | 35,545,666 / 35,135,104 | 14,060,278 / 13,902,137 | 9,412,479 | 9,302,764 | **+109,715** | +1.18 % | 19,649 / 19,644 |
| pdf (pdfcraft) | 39,565,728 / 39,099,122 | 14,560,284 / 14,393,230 | 9,493,807 | 9,395,853 | **+97,954** | +1.04 % | 20,495 / 20,495 |

What the delta contains: the carried queue (patches: wordcraft 10, gridcraft 29, deckcraft 8, pdfcraft 4), the overlay hooks and
placeholder logo (deckcraft and pdfcraft embed a 64 KiB RGBA logo), the two Noto fallback faces (+148,316 B raw per app), and the two Uzbek catalogs.
It is the whole layer of Phase 1 on top of upstream, which is what the ratchet has to guard. The earlier STATUS figures were lower for docs, slides and
pdf because the Phase 0 base was a larger upstream build (wordcraft pristine fell 64 KB between the bases).

### Final budgets (plan rule: `delta_br_max` = measured delta + 10 %, rounded up to 16 KiB)

| domain | `base_br` | `delta_br_max` | budget (wasm brotli) | our brotli | headroom | JS `js_br_max` |
|---|---|---|---|---|---|---|
| docs | 9,377,090 | 131,072 | 9,508,162 | 9,482,712 | 25,450 | 24,576 (20,778) |
| sheets | 4,733,055 | 147,456 | 4,880,511 | 4,855,241 | 25,270 | 24,576 (20,050) |
| slides | 9,302,764 | 131,072 | 9,433,836 | 9,412,479 | 21,357 | 24,576 (19,649) |
| pdf | 9,395,853 | 114,688 | 9,510,541 | 9,493,807 | 16,734 | 24,576 (20,495) |

- The temporary 512 KiB guard is removed from every domain (`web/budgets.toml` header and rows rewritten). `base_sha` is now the current BASE of each app,
  `base_raw/base_gzip/base_br/js_br` are the pristine numbers above, and slides now says `craft_fonts = true` (it was `false`, but the Phase 0 base and
  the dist both embed the fonts; docs, sheets and pdf build without them).
- `scripts/ci.sh sizes` on the rebuilt dists: **exit 0**, table in section 4.
- Not measured: the effect of `CRAFT_FONTS_DIR` on docs, sheets and pdf (the plan's "measure both settings"; WP3 recommended parity with Phase 0, and no
  owner decision has changed it).

## 2. Clippy ratchet (`ci/clippy-baseline/<app>.txt`)

`scripts/ci.sh lint` now **fails on any upstream clippy warning that is not in the baseline** (it used to be advisory and printed `baseline none`).
A warning is keyed by file and message, without line numbers, so edits elsewhere do not move it; a second warning of the same kind in the same file is also
new. A baselined warning that disappears is only noted (shrink the file). `cargo clippy` itself failing to compile also fails the job (before, a compile
error was counted as "0 warnings"). `CI_CLIPPY_REFRESH=1` rewrites the files from the current output.

Counts at the current BASE (rustc 1.97.1 clippy, `cargo clippy --workspace --all-targets`, lint names from `--message-format json`):

| app | warnings | lint | location | origin |
|---|---|---|---|---|
| wordcraft | 1 | `clippy::explicit_auto_deref` | `crates/ui-egui/src/office_overlay.rs:68` (`return *done;`) | ours: carried patch 0100 |
| gridcraft | 2 | `clippy::manual_is_multiple_of` | `crates/pdf/src/ttf.rs:378` | ours: carried patch 0020 (pdf-fonts, new file) |
| | | `clippy::explicit_auto_deref` | `crates/ui-egui/src/office_overlay.rs:68` | ours: carried patch 0100 |
| deckcraft | 2 | `clippy::manual_range_contains` | `crates/model/src/trendline.rs:468` | upstream |
| | | `clippy::explicit_auto_deref` | `crates/ui-egui/src/office_overlay.rs:68` | ours: carried patch 0100 |
| pdfcraft | 1 | `clippy::explicit_auto_deref` | `crates/ui-egui/src/office_overlay.rs:68` | ours: carried patch 0100 |

- Only **one** of the six is upstream's own (deckcraft `trendline.rs`). Five sit in code our patches add. They are baselined and not fixed here because
  `upstream/` must not be hand-edited: the fix is one line per patch (`return done;` in 0100 for all four apps, `% 4 != 0` to `.is_multiple_of(4)` in
  gridcraft 0020) and belongs in the next patch refresh through the cache clones (`office/queue-*` branches), where the register rows are updated at the same time.
  After that the baselines drop to 0/1/1/0. Until then the ratchet cannot hide a new warning in the same files, because the key includes the message.
- Our own crates (root workspace, `xtask` and `crates/office-brand`): `cargo clippy --workspace --all-targets -- -D warnings` exits 0 (the nine
  `collapsible_if` errors of the first lint run were fixed in `82778a1`).
- The lint run of 2026-10-18 03:33 had reported `baseline none` and a gridcraft count taken while `gridcraft-xlsx` lib tests did not compile; the
  numbers above are from clean runs after patch 0103.

## 3. Quarantine review (`ci/quarantine.toml`, 54 entries, `cargo xt quarantine check` = OK, 54 entries)

> **Note (gate review 2026-10-19).** Now 56 entries (98c0a02 added 2 pdfcraft `flaky`). "All evidence files exist" holds only on this PC: `*.log` is gitignored, so 12 of the
> 16 evidence files (cited by 47 entries, including `pc-on.log` and `wc-on.log`) are not in git and are missing in a clean checkout. See `docs/phase1/gate-review.md`.

Criteria: every entry has a class, a reason, evidence that exists (all 54 evidence files and anchors were checked), and a review trigger. New optional
field `reviewed = "<date>@WP6-final[: verdict]"` records this review; the checker ignores it. The review also re-read the failing assertion of every
pdfcraft entry in `pc-on.log` and ran diagnostics on the entries that looked weak (`docs/phase1/evidence/tests/wp6-diag-*.log`, warm targets,
overlay on, same environment as `ci.sh test`).

Counts by app and class (unchanged): wordcraft 19 (env 7, brand-overlay 11, i18n-incomplete 1), gridcraft 1 (brand-overlay), deckcraft 5 (env 1, flaky 1,
brand-overlay 3), pdfcraft 29 (env 1, flaky 4, brand-overlay 20, i18n-incomplete 4). No entry covers a failure caused by our patches except the two
allowed classes; none was added or dropped (the WP6b run re-verified each against the BASE; this review changed metadata, not selectors).

### Weak reasons found and what was done

| entry | what was weak | outcome |
|---|---|---|
| pdfcraft brand-overlay x19 (#36-#54) | the same generic sentence ("asserts upstream branding ... patch 0101") copied onto 19 entries with different assertions; one of them (`every_registered_command_is_implemented`) stops at the first failing command, so it could mask a later regression | each reason now names the failing assertion (`file:line`) and the cause (name rebrand, community chrome off by 0101, update check removed, fallback face appended by 0102, product version 0.1.0 vs crate 0.8.0). The masking risk is covered by the overlay-off run, where the test passes (3,516 passed) |
| wordcraft `breaks_menu` x2 | cause "probably localised, unverified" | **confirmed**: `WORDCRAFT_LANGUAGE=en` makes both pass, default (ru system language) fails at `breaks_menu.rs:31` (`wp6-diag-wc-breaks-*.log`). Reason, evidence and trigger rewritten; drop the entries on an English-locale runner |
| deckcraft `replacefonts_tests::ribbon_button_opens_the_dialog_and_replaces` | reason said "label lookup, left Some(replaceFonts)": wrong. The log shows left `None` at `replacefonts_tests.rs:46`; the button label is found but the dialog does not open | reason corrected; env label changed to `win10-overlay-off-undiagnosed`; **still WEAK**: cause unknown, fails only with the overlay off (pristine-like baseline, not the shipping build). A diagnosis needs an overlay-off rebuild (about 20 min) and is left for the next test night |
| pdfcraft perf x2 (`panels_with_hundreds_of_items...`, `scrolling_a_500_page_document...`) | "fails under load, passes alone (2 of 2)" | re-run alone, `--test-threads 1`, 5 iterations: panels 1 of 5 pass, scrolling 4 of 5 (average frame 41.6-58.8 ms against upstream's 40 ms debug budget; upstream skips on load average only on Linux/macOS). A single-thread brotli job overlapped the run, so the numbers are slightly pessimistic. Reasons rewritten; trigger: move both to a nightly perf job on a quiet or faster runner |
| pdfcraft `hundreds_of_xfa...`, `text_background_is_cached...` (flaky) | isolated evidence thin (2 of 2) / wrong review text copied from another entry | isolated: xfa 5 of 5 pass, text_background 10 of 10 pass: load-sensitive only, as claimed. Review texts fixed |
| pdfcraft `store_keys` (`name = "*"`) | `env = "any"` although the cause is this PC's Defender | `env = "win10-defender"` (still applies here: label starts with `win10`; a clean runner runs the binary) |
| 5 `i18n-incomplete` entries | trigger "shrinks when WP9e adds the row" although WP9e is closed | trigger now "Phase 2 uz catalog completion; CI fails after two stale runs" and the reasons say the row is outside the P1 string set |
| wordcraft emoji x2, alt_numpad | cause only hypothesised ("no or a different colour emoji font", OEM code page) | kept, marked `ok-with-caveat`: they fail at the pristine Phase 0 build and with the overlay off (7 wordcraft env entries fail in both modes), so they are independent of our patches |

### Result

| verdict | entries |
|---|---|
| ok (specific reason, evidence, trigger) | 48 |
| ok with caveat (cause hypothesised, independence from our patches shown) | 3 |
| evidence corrected, reclassification pending (perf budgets) | 2 |
| **weak: cause unknown** | **1** (deckcraft `replacefonts`) |
| removed | 0 |
| stale (passes) | 0 at the last WP6b run |

Full table (verdict column from the `reviewed` field):

| # | app | class | test | env | verdict |
|---|---|---|---|---|---|
| 1 | wordcraft | env | `lib` `tests_color_glyphs::emoji_are_drawn_in_colour` | win10-no-colour-emoji-font | ok-with-caveat (fails at pristine too, cause only hypothesised) |
| 2 | wordcraft | env | `lib` `tests_color_glyphs::emoji_keep_their_colours_on_a_dark_page` | win10-no-colour-emoji-font | ok-with-caveat (fails at pristine too, cause only hypothesised) |
| 3 | wordcraft | env | `lib` `...:tests::alt_numpad_code_types_its_character_without_keytips` | win10-ru-oem866 | ok-with-caveat (locale-specific by design, not re-diagnosed) |
| 4 | wordcraft | env | `breaks_menu` `page_break_from_the_menu_adds_a_page` | win10-ru-locale-labels | ok |
| 5 | wordcraft | env | `breaks_menu` `section_break_from_the_menu_splits_sections` | win10-ru-locale-labels | ok |
| 6 | deckcraft | env | `lib` `...acefonts_tests::ribbon_button_opens_the_dialog_and_replaces` | win10-overlay-off-undiagnosed / overlay off | WEAK (cause undiagnosed, old reason was wrong; overlay-off baseline only) |
| 7 | wordcraft | brand-overlay | `lib` `i18n::audit::untranslated_interface_text` | any | ok |
| 8 | wordcraft | brand-overlay | `lib` `...sts::bulgarian_covers_the_entire_existing_interface_catalog` | any | ok |
| 9 | wordcraft | brand-overlay | `lib` `...e_existing_interface_catalog_and_keeps_count_labels_neutral` | any | ok |
| 10 | wordcraft | brand-overlay | `lib` `...sts::hungarian_covers_the_entire_existing_interface_catalog` | any | ok |
| 11 | wordcraft | brand-overlay | `lib` `...tests::swedish_covers_the_entire_existing_interface_catalog` | any | ok |
| 12 | wordcraft | brand-overlay | `lib` `...e_entire_existing_interface_catalog_and_preserves_templates` | any | ok |
| 13 | wordcraft | i18n-incomplete | `lib` `i18n::tests::every_tab_and_command_is_translated` | any | ok |
| 14 | gridcraft | brand-overlay | `lib` `...n::tests::translations_are_unique_and_preserve_unknown_text` | any | ok |
| 15 | pdfcraft | brand-overlay | `lib` `i18n::tests::arabic_lookups_are_in_display_order` | any | ok |
| 16 | pdfcraft | i18n-incomplete | `lib` `...oard_key_names_have_a_separate_context_and_english_fallback` | any | ok |
| 17 | pdfcraft | i18n-incomplete | `lib` `...gotor_confirmation_is_translated_in_all_supported_languages` | any | ok |
| 18 | pdfcraft | i18n-incomplete | `lib` `...lection_labels_and_refusals_are_translated_in_every_catalog` | any | ok |
| 19 | deckcraft | brand-overlay | `lib` `backstage::tests::every_page_renders` | any | ok |
| 20 | deckcraft | brand-overlay | `lib` `...::tests::palette_has_one_export_entry_that_opens_the_dialog` | any | ok |
| 21 | wordcraft | env | `lib` `..._sysfonts::fallback_lookups_are_remembered_until_fonts_load` | any | ok |
| 22 | wordcraft | env | `lib` `...ache_is_dropped_when_a_face_loads_by_its_legacy_family_name` | any | ok |
| 23 | wordcraft | brand-overlay | `passthrough` `...t_words_counts_and_personal_ones_follow_the_privacy_setting` | any | ok |
| 24 | wordcraft | brand-overlay | `lib` `tests::latex_round_trip` | any | ok |
| 25 | wordcraft | brand-overlay | `lib` `tests::the_default_app_button_shows_only_with_the_hosts_hook` | any | ok |
| 26 | wordcraft | brand-overlay | `lib` `...::tests::general_start_up_turns_the_start_screen_off_and_on` | any | ok |
| 27 | wordcraft | brand-overlay | `lib` `...ts::start_screen_shows_file_home_and_esc_leaves_no_document` | any | ok |
| 28 | deckcraft | flaky | `bin/deckcraft` `..._server::tests::blocked_ui_times_out_and_full_queue_is_busy` | any | ok |
| 29 | deckcraft | brand-overlay | `lib` `tests::metadata_retains_values_and_omits_blank_fields` | any | ok |
| 30 | pdfcraft | env | `store_keys` `*` | win10-defender | ok |
| 31 | pdfcraft | flaky | `lib` `...f_xfa_calculations_run_in_one_engine_within_the_open_budget` | any | ok |
| 32 | pdfcraft | flaky | `perf` `panels_with_hundreds_of_items_stay_within_the_frame_budget` | any | reclassified evidence (fails alone, not only under load); move to nightly perf |
| 33 | pdfcraft | flaky | `perf` `scrolling_a_500_page_document_stays_within_the_frame_budget` | any | reclassified evidence (borderline alone, not only under load); move to nightly perf |
| 34 | pdfcraft | flaky | `lib` `...background_is_cached_without_editing_and_released_on_cancel` | any | ok |
| 35 | pdfcraft | i18n-incomplete | `lib` `...ts::the_preference_overrides_the_system_for_every_animation` | any | ok |
| 36 | pdfcraft | brand-overlay | `accessibility` `checking_fixing_skipping_and_reporting` | any | ok |
| 37 | pdfcraft | brand-overlay | `commands` `the_about_dialog_shows_the_running_version` | any | ok |
| 38 | pdfcraft | brand-overlay | `commands` `every_registered_command_is_implemented` | any | ok |
| 39 | pdfcraft | brand-overlay | `fonts` `japanese_ui_text_uses_craft_fonts` | any | ok |
| 40 | pdfcraft | brand-overlay | `pickers` `...tup_url_preserves_home_even_after_all_other_tabs_are_closed` | any | ok |
| 41 | pdfcraft | brand-overlay | `signing` `windows_store_identity_does_not_ask_for_a_file_password` | any | ok |
| 42 | pdfcraft | brand-overlay | `ui` `...sing_the_last_active_tab_selects_the_previous_one_then_home` | any | ok |
| 43 | pdfcraft | brand-overlay | `ui` `home_removes_one_recent_file_or_clears_them_all` | any | ok |
| 44 | pdfcraft | brand-overlay | `ui` `home_shows_welcome_and_tools` | any | ok |
| 45 | pdfcraft | brand-overlay | `ui` `...o_fit_then_arrows_step_through_them_and_open_stays_in_sight` | any | ok |
| 46 | pdfcraft | brand-overlay | `ui_group` `links::help_commands_open_each_link` | any | ok |
| 47 | pdfcraft | brand-overlay | `ui_group` `links::about_dialog_shows_the_brand_and_links` | any | ok |
| 48 | pdfcraft | brand-overlay | `ui_group` `links::discord_button_in_the_top_bar_opens_discord` | any | ok |
| 49 | pdfcraft | brand-overlay | `ui_group` `links::home_screen_links` | any | ok |
| 50 | pdfcraft | brand-overlay | `ui_group` `updates::a_newer_release_is_offered_for_download` | any | ok |
| 51 | pdfcraft | brand-overlay | `ui_group` `updates::an_up_to_date_or_failed_check_says_so` | any | ok |
| 52 | pdfcraft | brand-overlay | `ui_group` `updates::nothing_is_asked_until_the_user_checks` | any | ok |
| 53 | pdfcraft | brand-overlay | `viewer` `...laceholder_title_leaves_the_file_name_on_the_tab_and_window` | any | ok |
| 54 | pdfcraft | brand-overlay | `viewer` `tab_and_window_show_the_document_title_when_asked` | any | ok |

## 4. CI results

Run `scripts/ci.sh --run wp6-final quick sizes` (profile `quick` = queue, lint, gen-check, deny, brand, i18n; no domain is affected by the diff, so no per-app
`check`/`test`) and `--run wp6-final2 gen-check sizes` after rebuilding `xtask` (see the note). Logs: `E:/office-target/p1/ci/wp6-final*/`.

| job | result | detail |
|---|---|---|
| queue | **ok** (10 s) | verify-queue all, register check, quarantine format |
| lint | **ok** (277 s) | fmt root and 4 upstream trees clean, `clippy -D warnings` on our crates exit 0, layers and assets ok, upstream clippy ratchet: wordcraft 1, gridcraft 2, deckcraft 2, pdfcraft 1 warnings, all baselined |
| gen-check | **ok** | `4 domains up to date`; lockdiff x4: `0 version changes, 0 foreign additions` |
| deny | **ok** (34 s) | root plus 4 domains |
| brand | **ok** (359 s) | brand-grep 0 hits, brand-scan-bin on the rebuilt web wasm and exes |
| i18n | **ok** (8 s) | lint, roundtrip, check-stale |
| sizes | **ok** | table below |

```
domain  (src)              raw         gzip       brotli       budget     headroom js_br/max
docs    dist          42621161     15916854      9482712      9508162        25450 20778/24576
sheets  dist          19328641      7209238      4855241      4880511        25270 20050/24576
slides  dist          35545666     14060278      9412479      9433836        21357 19649/24576
pdf     dist          39565728     14560284      9493807      9510541        16734 20495/24576
```

Notes:
- The first `gen-check` failed (`path '../craft-libs/vendor/krilla' leaves the upstream tree`): the shared `xtask.exe` in `E:/office-target/p1/core` had been rebuilt from the
  WP4 scratch worktree (its `cargo xt overlay` writes the same shared target dir and the executable bakes in its source root). Rebuilding `xtask` in the main checkout fixed it; main sources were never changed. The
  WP4 worktree should use its own `target-dir` for the root workspace if it is ever reused.
- `ci.sh test` was not re-run in main: nothing it selects changed (quarantine selectors are unchanged, only metadata fields); the last full run is `docs/phase1/tests.md`.
- Evidence of the unpatched baselines stays in `docs/phase1/evidence/tests/`.
