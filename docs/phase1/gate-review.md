# Phase 1 exit gate: adversarial review

Reviewer: architect agent (opus), 2026-10-19, on `phase-1` at `98c0a02`. Read-only, apart from the small doc fixes listed in section 5.
No build was started. The final uninterrupted `ci.sh full` in the clean clone (`E:/office-target/p1/clean/office`, run `final-run`, started 23:18)
was still in the `lint`/`brand` jobs when this review was written. Its result decides gate item 2 (section 1).

Commands run for this review (cheap, no rustc):
- `bash scripts/verify-queue.sh all`: wordcraft 10, gridcraft 29, deckcraft 8, pdfcraft 4, craft-libs 0 patches, every tree OK.
- `cargo xt register check`: `51 patches, 51 rows, OK`. No REGISTER row has an empty cell.
- `cargo xt quarantine check`: `OK, 56 entries`.
- `cargo xt i18n lint`: 0 errors, 1,878 P1 rows `agent-reviewed`, 0 draft, 0 owner-reviewed. `check-stale`: ok.
- `cargo fmt --all --check` for the root and the four upstream trees: 0 diffs.
- Licence files: `LICENSE*|NOTICE*|ATTRIBUTION*|COPYING*` under `upstream/` = 70. That equals the BASE tree of every repo
  (wordcraft 5, gridcraft 6, deckcraft 6, pdfcraft 24, craft-libs 29). No patch deletes a file.
- Phase 1 commits: 0 `Co-Authored-By` trailers, no git remote.

## 1. Verdict per exit-gate item

| # | Gate item (BRIEF Phase 1) | Verdict | Evidence |
|---|---|---|---|
| 1 | A clean checkout builds desktop and web for all four apps | **MET-WITH-CAVEAT** | `docs/phase1/clean-checkout.md:29-30` (desktop and CLI release x4 in clean3, web x4 in clean4) |
| 2 | CI is green | **NOT MET yet (pending)**. It becomes MET-WITH-CAVEAT if `final-run` exits 0 with `web-smoke` ok | `clean-checkout.md:8,33`; the `final-run` summary in `E:/office-target/p1/clean/target/ci/final-run/summary.md` |
| 3 | A dry run of the upstream sync succeeds | **MET-WITH-CAVEAT** | `docs/phase1/sync-reports/2026-10-19.md:132` (run 1 `DRY RUN INCOMPLETE`), the run 2 outcome (`DRY RUN OK`, gridcraft only), `rebase-cost.md` (i18n reproduction) |
| 4 | The UI shows the new brand and switches to uz-Latn / uz-Cyrl | **MET-WITH-CAVEAT** | `docs/phase1/evidence/brand/` (16 PNG), `docs/phase1/evidence/i18n/` (37 PNG: 4 apps x desktop and web x Latn and Cyrl), tests in patches `*/0110` |

### Item 1: clean-checkout build

**What is proven.** A fresh `git clone` (origin = the local repo, `HEAD` 98c0a02) builds every target in its own target dir after six script fixes:
desktop, CLI and web release builds for all four apps.

**No hidden reliance on the main checkout.** This was checked:
- After the fixes (8d749ae, b12c208, 643ae33, 5ed9d41, 7e50ec2, 2d44732), the clone's only ignored files are `upstream/craft-fonts/` (fetched) and the three
  generated `index.html` files (`git status --ignored`).
- The root target dir and the trunk dist dir now follow `OFFICE_TARGET`.

**Caveats.**
- **Stitched evidence.** The proof is stitched together from runs clean1 to clean5 on one warm target dir, with fixes applied between the runs. The desktop builds
  (clean3) ran at a commit older than the web fixes.
- **No cold, uninterrupted run yet.** `final-run` reuses that warm target, so even when it is green it proves "uninterrupted", not "cold".
- **It relies on machine-level state that a new machine would not have:**
  - the craft-fonts cache clone in `E:/office-upstream` (bootstrap took 5 s from it);
  - `E:/office-tools`, the cargo registry and git caches, the trunk/wasm-bindgen cache;
  - the Playwright browsers in `%LOCALAPPDATA%\ms-playwright` (`ci.sh web-smoke` never runs `playwright install`).
- **windows-gnu only.** MSVC (the shipping toolchain, ADR 2.5) and Linux were never built. The GitHub workflow has never run.
- **Build, not launch.** Desktop exes are built but never launched in CI. Only WP3 did a one-off 15 s alive check.

### Item 2: CI green

**What exists today.**
- Every job passed at least once in the clean clone (`clean-checkout.md` table).
- `web-smoke` has **never executed**: the spec landed in 98c0a02, after the last clean run.
- "CI" here means `scripts/ci.sh` on one development PC. `.github/workflows/ci.yml` is `workflow_dispatch` only and has never run, because the owner rule forbids a remote.

**How honest "green" is:**
- **The quarantine is large.** It has 56 entries: env 9, flaky 7, brand-overlay 35, i18n-incomplete 5. That means 40 of 56 entries (71 %) hide the effect of our own overlay on
  upstream tests. The plan allows this, but finding M2 shows these tests are not exercised anywhere in CI.
- **Its evidence is not in git.** 47 of the 56 entries cite logs that are not in git (finding M1).
- **Skipped steps that pass silently:**
  - `brand-scan-bin` runs before the builds (finding M3).
  - `fidelity` returns "ok" when `corpus/` is absent (`scripts/ci.sh:447-448`; nightly only).
  - No job is skipped in `full` otherwise.
- **No stubs in `full`:** `gen-check` and `i18n` have real implementations (`ci.sh:211-222,265-273`).
- **Flaky tests are rising** (finding M6). Two of them were quarantined after a single observation.

**Acceptance condition for this item.** `final-run` exits 0, and `summary.md` shows `web-smoke: ok` and `test pdfcraft: ok` with no non-quarantined failure.

### Item 3: sync dry run

**What succeeded.** Against the current upstream HEAD (67 commits, about 2 days), all 51 patches rebase:
- 49 clean;
- 1 conflict, resolved in 92 s (gridcraft 0020, a `Cargo.lock` version line);
- 1 regenerated (0111 rustfmt).

The quick gates pass: verify-queue, register, gen-check x4, lockdiff x4, deny x4, and `cargo check` native and wasm32. The monorepo stayed untouched.

**Caveats.**
- **No single all-repo pass.** No single invocation printed `DRY RUN OK` for all repos. Run 1 (all repos) ended `DRY RUN INCOMPLETE`: the conflict, plus `i18n-stale` and
  `i18n-roundtrip` FAIL (`sync-reports/2026-10-19.md:110-114,132`). The i18n gates were shown to pass after `cargo xt i18n extract` only by a hand reproduction
  (`rebase-cost.md`, "i18n-roundtrip finding").
- **The i18n design guarantees this failure** (finding M4).
- **`--gates full` has never been run** (finding M5).
- **Not a weekly sample.** The distance is 2 days, not 7, so the weekly cost (risk 1) is still unmeasured. The owner moved that measurement to Phase 2.

### Item 4: brand and uz-Latn / uz-Cyrl

**What is proven.**
- All four apps, on desktop and web, show the working name from `brand/brand.toml`.
- About shows the attribution line. The ArtCraft marks are not compiled in.
- The binary scan reported 0 leaks in the WP6-final run in the main checkout.
- All four apps switch to both scripts, and every combination has screenshots.
- The patch tests `uzbek_is_offered_only_with_the_overlay_catalogs` run with the overlay on in `ci.sh test`.

**Caveats.**
- **Partial coverage.** Only the P1 subset is translated. Share of keys: wordcraft 784/2,616 (30 %), gridcraft 447/970 (46 %), pdfcraft 601/2,672 (22 %),
  deckcraft 389/1,559 (25 %) (`cargo xt i18n coverage`).
- **Screenshots predate the agent review.** The wording shown has changed since (doc fixed, see section 5).
- **pdf and slides web need the picker.** They do not follow the browser language.
- **The brand is incomplete** (finding m1):
  - the brand colours are designed but not applied;
  - exe names, settings folders (`%APPDATA%\WordCraft`), app ids and eframe storage names are still upstream's (STATUS WP7 "Not rebranded");
  - the icons are placeholders.

## 2. Findings

### Blocker

**B1. Gate item 2 is unproven until `final-run` is green.**
- **Fix:** wait for `final-run`.
- **If it is green:** record the job table under "final run" in `clean-checkout.md`, then the gate can be reported.
- **If `web-smoke` fails:** fix it, then re-run `ci.sh build web-smoke` at least. Re-run the whole run if any code changed.
- **If only a pdfcraft timing test fails:** do not quarantine it on the spot. Apply M6 first.

### Major

**M1. Quarantine evidence is not in the repository.**
- **What is wrong:** `.gitignore:7` ignores `*.log`.
- **How much is missing:** 12 of the 16 evidence files cited by `ci/quarantine.toml` are untracked. They back 47 of the 56 entries, among them `pc-on.log` (20 entries) and the
  `test-*-overlay-on.log` files (14 entries).
- **Why it matters:** in a clean checkout, or for the owner, the "every entry has evidence" claim cannot be checked (`wp6-final.md` §3, `tests.md:5`). The same applies to the
  WP6b result logs (`final-*.log`, `*-off.log`).
- **Fix:** `git add -f` the small logs. Cut `pc-on.log` (2.3 MB) down to the failing-test excerpts, or rename the evidence to `.txt`. Make
  `cargo xt quarantine check` fail when an evidence path is not tracked.

**M2. The functional coverage of 40 overlay-quarantined tests is not exercised in CI.**
- **What is quarantined:**
  - functional UI tests that fail only on a brand string, among them pdfcraft `every_registered_command_is_implemented`, `home_removes_one_recent_file_or_clears_them_all`,
    `closing_the_last_active_tab_selects_the_previous_one_then_home`, `a_late_startup_url_preserves_home...` and `checking_fixing_skipping_and_reporting`, deckcraft
    `every_page_renders`, and wordcraft start-screen and options tests;
  - five wordcraft catalog-completeness tests.
- **Why the stated mitigation does not hold:** wp6-final says "the masking risk is covered by the overlay-off run". That run was a single manual run on 2026-10-18.
  `ci.sh` never runs `OFFICE_NO_OVERLAY=1` (`ci.sh:282-287,476-489`), so a regression in those code paths after any sync, or from any later patch (0112 came after it), goes unseen.
- **Fix, short term:** add `OFFICE_NO_OVERLAY=1 scripts/ci.sh test all` to `nightly` (and to `sync-upstream --gates full`).
- **Fix, Phase 2:** carry test patches that read the brand name instead of a literal, as gridcraft 0104 already does. That shrinks the brand-overlay class.

**M3. The brand binary scan runs before the builds.**
- **What happens:** `profile_jobs` emits `queue lint gen-check deny brand i18n` before `build` (`scripts/ci.sh:478,486`).
  - In a clean checkout, `job_brand` finds no binaries, prints a note and returns 0 with no skip counted (`ci.sh:257-261`); clean2 shows exactly this.
  - In a warm target dir it scans the previous run's binaries: `final-run` was running `brand-scan-bin` at 23:32, before any build.
- **Why it matters:** the "0 leaks" claim is therefore not re-proven by `full`.
- **Fix:** move `brand-scan-bin` into a job after `build`/`sizes`. Count "no binaries" as `SKIPPED` so the summary shows `ok-with-skips`.

**M4. A sync dry run cannot pass whenever upstream touched a catalog.**
- **What happens:** the dry run already works in a scratch monorepo worktree, but it refuses to merge the catalogs (`scripts/sync-upstream.sh:298-307`). So `i18n-stale`
  and `i18n-roundtrip` fail in every such dry run, and `DRY RUN OK` for `all` is unreachable.
- **Why it matters:** this is why gate item 3 had to be assembled by hand.
- **Fix:** run `cargo xt i18n extract` in the scratch tree in dry runs too, report the merge diff, and gate on the post-merge check. The main tree is untouched either way.

**M5. `--gates full` has never been executed and probably fails for slides/deckcraft.**
- **Where the problem is:**
  - `ci.sh` re-sources `env.sh` in the scratch tree. That sets `CRAFT_FONTS_DIR=$SCR/upstream/craft-fonts`, but the fonts are fetched and not in the scratch worktree.
  - The quick gates work around this explicitly (`sync-upstream.sh:321` points them at `$MAIN`). The full gates do not (`:336-344`).
  - `test_fonts` then copies from a missing directory (`ci.sh:302-306`).
- **Why it matters:** `docs/UPSTREAM.md` described it as working. The doc is now marked "not exercised".
- **Fix:** export `CRAFT_FONTS_DIR` to main's copy (or fetch the fonts into the scratch tree), then run `--dry-run --gates full deckcraft` once before relying on it.

**M6. pdfcraft timing flakiness is growing, and green depends on machine load.**
- **The trend:**
  - 6 pdfcraft `flaky` entries;
  - 3 more failures not quarantined in WP4 (`craft-libs-single-rev.md` §2);
  - 2 more in the cold clean3 run, quarantined after one observation (98c0a02), although `clean-checkout.md` said "needs more runs".
- **One entry is misclassified:** the `perf` test `panels_with_hundreds_of_items...` fails 4 of 5 times when run alone. That is a hardware budget (class `env`), not `flaky`.
- **Fix:**
  - a nextest test group with `max-threads = 1` for the pdfcraft UI timing binaries (`combine_grid`, `comments`, `perf`, `edit_text_ui`);
  - retries only through the `flaky` profile;
  - a 3-observation rule before quarantining;
  - reclassify the perf entries as `env`.

### Minor

| # | Finding | Fix |
|---|---|---|
| m1 | The brand is incomplete against BRIEF task 3 ("name, icons and colours"). `brand.toml [colors]` (accent `#0F766E`) is designed (designs/brand.md §3.3) but no `0100` patch applies it; the upstream accents remain. Exe names (`docs.exe`…), `%APPDATA%\WordCraft` and other data dirs, AUMIDs and eframe storage names are still upstream's: an installed upstream app would share settings with ours. | Owner decision (BRIEF App. B, Phase 1: logo and colours). Apply the accent and the data-dir id in a 0100 refresh before any user install. |
| m2 | 5 of the 6 baselined upstream clippy warnings are in our own patches (`ci/clippy-baseline/*.txt`: `office_overlay.rs` in 0100 x4, gridcraft 0020 `ttf.rs`). | Fix them in the next patch refresh, as `wp6-final.md` §2 plans. |
| m3 | cargo-deny "duplicate crates" only warns (`multiple-versions = "warn"` in all 5 configs). The domain configs keep `wildcards = "warn"` "until the manifests set publish = false", but the generated manifests already set it. | Switch wildcards to `deny`. For duplicates, add a counted ratchet or an explicit skip list. |
| m4 | Desktop GUIs are never launched in CI; a broken window start would still be "green". | Add a 15 s alive check (WP3 step 6) after `build`. |
| m5 | `web-smoke` relies on Playwright browsers already cached on this PC. `tools.lock` has no node, brotli or Playwright entry. | Run `playwright install chromium` in `job_web_smoke`, and add the versions to `tools.lock`. |
| m6 | `release-tests`: its quarantined pass does not pass `--release` (`tests.md:192`). The `nightly` profile has never run. `fidelity` reports ok with no corpus. | Fix it when nightly is first used. Count a missing corpus as a skip. |
| m7 | The `pdfcraft-sign::store_keys` binary (3 tests) never runs here (Defender false positive). | Admin adds a Defender exclusion for `E:\office-target` (owner/admin). |
| m8 | CLAUDE.md "Language" still says Uzbek strings are "reviewed by the owner, never machine-trusted", which contradicts the gate decision a few lines above (agent review, `trust_agent_review = true`). | Owner reconciles the wording; agents may not edit CLAUDE.md on their own. |
| m9 | The agent reviewed the drafts written by agents of the same model family: an "independent" review in name only. 0 owner-reviewed rows. | Accepted by owner delegation. Report it plainly at the gate; owner feedback in use is the only human check. |
| m10 | Committed bytecode: `i18n/tools/__pycache__/*.pyc`, `spikes/xlsx-bench/__pycache__/*.pyc`. | `git rm --cached`, and add `__pycache__/` to `.gitignore`. |
| m11 | gridcraft 0103/0104 are test fixes (category `ci-fix`) numbered in the overlay range 0100-0199. PLAN WP2 step 3 reserves 0900-0999 for them. | Renumber at the next sync, or amend the convention. |
| m12 | gridcraft 0020 edits `Cargo.lock`, which is the only conflict in the second dry run and recurs with every version bump. | Drop the lock hunk from the patch (`rebase-cost.md` already proposes it). |
| m13 | NOTICE and the OFL texts are not yet part of any shipped artefact (the web dist has no licence page). | Phase 4 packaging. Track it in the licensing checklist. |

### Checked and found sound

- **Every patch has a REGISTER row with a reason:** 51 rows, no empty cell, every PR column "not sent (B-6)".
- **Upstream attribution files are intact:** 70, the same count as at BASE.
- **NOTICE:**
  - lists the 16 symphonia crates with the source link and MPL text URL (`NOTICE:156-178`);
  - lists the Noto OFL fonts;
  - reproduces the upstream NOTICE files verbatim.
- **MPL is confined.** It is allowed only through per-crate exceptions in `ci/deny/slides.toml:72-138`. Every other config says GPL/AGPL/LGPL/MPL are never allowed.
- **The advisory ignores are justified.** Spot-checked:
  - `rsa` is used only for public-key verification (`upstream/pdfcraft/crates/sign/src/rsa_pad.rs:6,35`);
  - no `ZipArchive::extract` call exists in `upstream/*`.
- **Our crates and domain manifests** are `MIT OR Apache-2.0` and `publish = false`.
- **No personal data or corpus contents in git.**
  - No `corpus/` path appears in any commit (`git log --all --name-only`).
  - The committed `.docx` fixtures are synthetic: `spikes/*/fixtures/sample.docx` contains only "HRM fixture paragraph". STATUS called it "HRM-derived", which is fixed.
  - The i18n `.xlsx` batches contain UI strings only.
- **HRM test-only rule respected.** Phase 1 touched no HRM system. HRM appears only in `SIZING.md` planning text.
- **Formatting is clean.** rustfmt is clean everywhere, and our crates pass `clippy -D warnings` (`wp6-final.md` §2).
- **Dedupe under W2 (ADR App. C B-5, an accepted deviation from "one workspace").**
  - This means five workspaces, not one.
  - craft-fonts and the Noto fallback are one copy shared by all domains.
  - craft-libs is still at 5 revs: WP4 shows one rev works with a one-line deckcraft fix, but the owner has not adopted it.

## 3. What Phase 1 did not do (carried forward)

| Item | State | Where |
|---|---|---|
| **WP11 corpus run, LibreOffice and x2t references, Excel census** | Not done. `corpus/` was never delivered (B-14), so the B-15 promise of "first corpus numbers in Phase 1 week 1" is unmet. The LibreOffice/x2t reference renders could have run on the Phase 0 synthetic set without the corpus; they did not. | PLAN WP11; ADR Phase 1 row (h), (i) |
| **Weekly rebase cost (risk 1)** | Not measured. Two low-distance points (57 and 67 commits, 1 conflict in total). Owner decision 2026-10-18: one sync per week in Phase 2, with a report each time. | `rebase-cost.md` |
| **deckcraft spell-check crash** | Not root-caused. `dialogs::spelling_tests` (the two `squiggles_*` tests with any other spelling test) crashes in-process with `cargo test` (6 of 10). It does not appear under nextest's process-per-test. The suspect is the process-wide spell state (`deckcraft/crates/engine/src/spell.rs:556-612`). It would bite any in-process host. | `tests.md` (crash section) |
| **Locale `NumLocale` patch** | Not started. gridcraft is hard-coded to en-US: `12.10.2026` is stored as 10 December and `3,5` stays text. ESTIMATE 6-8 days. It blocks v1 quality for uz/ru users. | `locale-check.md`; STATUS WP10 |
| **craft-libs single rev (WP4)** | Measured, not adopted. Gain: 5 revs to 1. Cost: +102/+145 crates in docs/sheets and one more RUSTSEC ignore. | `craft-libs-single-rev.md` §6 |
| **Owner items B-7 to B-14** | Open: B-7 Excel scope/track E, B-8 legacy .ppt, B-9 HRM security actions (JWT rotation, Traefik copy), B-10 Win11/Android/Mac devices and signing certificate, B-11 default-handler policy, B-12 fleet facts, B-13 Slides i18n scope (Phase 1 did mechanism + P1 strings), B-14 corpus. | ADR App. B/C |
| Also open | M1-M6 above. Brand colours, data dirs and exe names (m1). `vendor-notes/` is missing: Phase 2 task 1 seeds the translit rules from `vendor-notes/uz-translit-onlyoffice/`, so the owner must deliver it before Phase 2 starts. The Phase 2 golden list needs the owner's approval. | |

## 4. Recommendation

**The gate is passable once B1 clears, that is, when `final-run` is green including `web-smoke`.** Report items 1, 3 and 4 as met with the caveats above.

None of M1-M6 invalidates a gate item, but each makes "green" weaker than it reads. M1 (commit the evidence) and M3 (scan order) are cheap and should land before the gate report.
M2, M4, M5 and M6 can go into the first Phase 2 week, together with the first weekly sync.

**Before saying "go", the owner should:**
- deliver `vendor-notes/`, which Phase 2 needs;
- decide on WP4 (craft-libs single rev);
- decide on the brand colours and data dirs;
- give the Defender exclusion;
- decide on `NumLocale` timing.

## 5. Doc fixes applied by this review

- **`docs/STATUS.md`:**
  - last-updated date;
  - the Phase 0 "In progress" heading marked historical;
  - "Next step" replaced (it still said Phase 0 task 1);
  - docs-bind fixture described as synthetic, not "HRM-derived";
  - patch-count wording (49 at the time, 51 now);
  - second dry run: "50 of 51 clean" changed to 49 clean, 1 resolved, 1 regenerated, with the i18n gate failures stated;
  - WP14 line updated for 98c0a02 (web-smoke spec written but unexecuted, 56 quarantine entries, `--gates full` unexecuted).
- **`docs/phase1/rebase-cost.md`:**
  - gridcraft "apply pending" marked done;
  - `--gates full` status;
  - gridcraft and total counts (27 + regenerated; 49 of 51);
  - "all gates PASS" qualified for the i18n gates.
- **`docs/UPSTREAM.md`:** `--gates full` marked not exercised, with the suspected fonts-path issue.
- **`docs/phase1/clean-checkout.md`:** 98c0a02 update; brand-scan ordering note.
- **`docs/phase1/evidence/i18n/README.md`:** screenshots predate the agent review; the roundtrip issue is fixed.
- **`docs/phase1/i18n/review-agent-2026-10-18.md` §4:** the 19 draft rows were resolved later (0 drafts now).
- **`docs/phase1/wp6-final.md` §3 and `docs/phase1/tests.md`:** 56 entries; the evidence logs are untracked.
- **`docs/phase1/PLAN.md`:** the status line no longer says "Plan, not started".
