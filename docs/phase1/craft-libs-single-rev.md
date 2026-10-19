# WP4: craft-libs on ONE revision (experiment)

Run 2026-10-19 (night slot). Nothing in the main branch changed except this report and its evidence. The decision is an **owner decision** (section 6).

## 1. Question and setup

The four domains pin craft-libs at five different revs (lock files of `domains/*`). Can all four use one?

| domain | current craft-libs sources in `domains/<d>/Cargo.lock` |
|---|---|
| docs (wordcraft) | `craft-print`, `craft-ui` d27646a; `craft-pdf` and the krilla patch c348732 |
| sheets (gridcraft) | `craft-print`, `craft-ui` d27646a |
| slides (deckcraft) | `craft-pdf` and the krilla patch c348732 |
| pdf (pdfcraft) | `craft-print` 666c358, `craft-quit` f58bc1a, `craft-ui` 5c874d5 |

Dates: 5c874d5 Oct 10, d27646a Oct 12, 666c358 Oct 12, f58bc1a Oct 12, c348732 Oct 13, **BASE 15ed713 Oct 16** (newest of all, and newer than any rev an app uses).
**Tested revision: BASE 15ed713**, the vendored snapshot `upstream/craft-libs` (patch queue empty). The alternative "newest rev any app uses" (c348732) was
**not** run: it has the print stack and `craft-pdf` shared writer of the newer revs too, and slides already sits on it, so the 15ed713 result below
shows where the breaks are (one, in slides); c348732 would be a second experiment of the same size if the owner wants it.

Method (scratch only; main checkout untouched):
- Worktree `E:/office-target/p1/wt/craftlibs` on branch `wp4/craftlibs` (not merged), own target dir `E:/office-target/p1/wt/craftlibs-target`, shared build lock.
- Every `domains/<d>/Cargo.toml` gets `[patch."https://github.com/storytold/craft-libs"]` with `craft-ui`, `craft-print`, `craft-quit`, `craft-pdf` as path
  dependencies on `upstream/craft-libs/crates/*`. For docs and slides the existing `[patch.crates-io] krilla = { git ... c348732 }` becomes
  `path = ../../upstream/craft-libs/vendor/krilla` (craft-libs' own `[patch]` set is ignored when it is a dependency, so the domain must carry it).
  sheets and pdf have no krilla patch and get none.
- Lock files re-resolved (`cargo metadata`, no `--locked`); `cargo xt lockdiff` of the new lock against the current one.
- Per domain: `cargo check --all-targets -p <d>-desktop -p <d>-cli` (native, windows-gnu) and `cargo check -p <d>-web --target wasm32-unknown-unknown`.
- Tests: the same upstream app in the worktree with the same `[patch]` in its root `Cargo.toml` (`craft-ui|print|quit|pdf` to `../craft-libs/crates/*`, the krilla git patch of
  wordcraft and deckcraft to the vendored path), run through the real CI job: `scripts/ci.sh test <app>` (overlay ON, quarantine applied, nextest profile `ci`),
  whole upstream workspace, no doctests (all four apps have zero). Compared with the WP6b run on the unpatched tree (docs/phase1/tests.md).

## 2. Result matrix

| domain | resolve | check native | check wasm32 (web) | tests (nextest, quarantine) | verdict at 15ed713 |
|---|---|---|---|---|---|
| docs | PASS | PASS, 0 errors, 840 s | PASS, 0 errors, 548 s | PASS: 2748 passed, 0 failed, 19 quarantined (19 still failing, 0 stale), identical to the baseline | **PASS** |
| sheets | PASS | PASS, 0 errors, 686 s | PASS, 0 errors, 460 s (1 warning, dead code in `gridcraft-ui-egui backstage.rs:416`, not craft-libs) | PASS: 1665 passed, 0 failed, 1 quarantined (still failing), identical | **PASS** |
| slides | PASS | PASS, 0 errors, 459 s | **FAIL, 4 errors**, 531 s | PASS: 1676 passed, 0 failed, 4 quarantined (4 still failing), identical (run with the fix below) | **FAIL on wasm32, fixable by one line** |
| pdf | PASS | PASS, 0 errors, 1,113 s (2 dead-code warnings in pdfcraft) | PASS, 0 errors, 496 s (7 dead-code warnings in pdfcraft and the overlay stub) | PASS with 3 timing failures: 3,488 passed, 3 failed (not quarantined), 28 quarantined (25 still failing, 3 passed-flaky, 0 stale), store_keys binary unrunnable (Defender), as before. The 3: `xfa_scripts_run_for_buttons_and_field_changes_through_tools` (`automation.rs:5422`, left 10 right 20), `check_scans_overlapping_directories_once` (`pdfcraft-cli check.rs:73`, 2 s per-file timeout), `a_formcalc_calculate_result_is_cut_before_it_is_saved` (`engine tests.rs:4605`). Re-run alone x3: xfa 3/3 pass, formcalc 3/3 pass, check_scans 2/3 pass. None touches craft-ui, craft-print or craft-quit | **PASS** (timing flakes of the same kind as the quarantined pdfcraft ones; the baseline full run had 0 non-quarantined failures, so these 3 should be watched, and added to the quarantine as `flaky` only if they recur) |

Baseline for the test columns (WP6b, same BASE, overlay on): wordcraft 2748 / 19, gridcraft 1665 / 1, deckcraft 1676 / 4, pdfcraft 3491 / 28 + 1 unrunnable binary.
Compile errors in the tests: 0 in all runs. The check logs are in `docs/phase1/evidence/craftlibs/`.

### The one failure: slides, wasm32 only

```
upstream/deckcraft/crates/pdf/src/lib.rs:32:48   E0432 unresolved import `krilla::text::TextDirection`: no `TextDirection` in `text`
upstream/deckcraft/crates/pdf/src/lib.rs:562:23  E0599 no method named `draw_text` found for `&mut Surface`
upstream/deckcraft/crates/pdf/src/lib.rs:573:19  E0599 (same)
upstream/deckcraft/crates/pdf/src/lib.rs:617:11  E0599 (same)
```

Cause: craft-libs commit **f39922b "fix(pdf): make backend text shaping opt-in"** (Oct 14, after c348732) changed the workspace dependency to
`krilla = { default-features = false, features = ["raster-images"] }` (`upstream/craft-libs/Cargo.toml:17`) and added the craft-pdf feature
`write-simple-text = ["write", "krilla/simple-text"]` (`crates/pdf/Cargo.toml:20`). `Surface::draw_text` and `TextDirection` exist only with krilla's `simple-text`
(`vendor/krilla/src/surface.rs:381`, `src/text/mod.rs:25`). deckcraft-pdf uses both and got the feature by accident: native builds get it from
deckcraft-pdf's own `krilla = { version = "=0.8.2", features = ["rayon"] }` (default features) under `cfg(not(target_arch = "wasm32"))`
(`upstream/deckcraft/crates/pdf/Cargo.toml:31-32`), which does not exist on wasm32. So the break is invisible to native checks and tests, and appears only in the web build.
wordcraft-pdf calls neither `draw_text` nor krilla's `TextDirection` (its own text direction type is a different one), hence docs passes.

Fix, verified in the scratch tree: `upstream/deckcraft/Cargo.toml:60`, `craft-pdf ... features = ["write"]` becomes `["write", "write-simple-text"]`.
`cargo check -p slides-web --target wasm32-unknown-unknown` then passes (0 errors, 64 s incremental). On native the feature set is unchanged (already
`simple-text` through the default features), which is why the deckcraft test run above is valid with and without it. As a **carried patch** it is one line and
upstreamable (deckcraft is already depending on a behaviour craft-pdf now documents as opt-in).

## 3. Lock changes (`cargo xt lockdiff <new> <current>`; files in `docs/phase1/evidence/craftlibs/lockdiff-*.txt`)

| domain | packages in the lock before / after | extra versions of existing crates | new crates | removed (old craft-libs revs, dev/xtask) |
|---|---|---|---|---|
| docs | 528 / 637 | 11 (base64 0.23, block-buffer, const-oid, core-foundation, cpufeatures, crypto-common, digest, getrandom 0.2, sha1, sha2, syn 1) | 102 | 10 |
| sheets | 440 / 588 | 9 | 136 | 2 |
| slides | 543 / 543 | 0 | 0 | 2 |
| pdf | 754 / 755 | 0 | 1 (`craft-winprint-gdi`, git) | 3 |

Why docs and sheets grow: they move from craft-print d27646a to 15ed713. The newer `craft-print` is the Windows printing stack (`winprint`, `craft-winprint-gdi`,
`winsafe`, `windows 0.62`), and `winprint` has a **build-dependency** on `reqwest` with default features (a PDFium download that craft-print switches off), so
`reqwest`, `hyper`, `rustls`, `aws-lc-rs` and `aws-lc-sys` (a C/cmake build) enter the lock. pdfcraft already has all of it (pdf grows by one crate).
Counted with `cargo tree` for x86_64-pc-windows-gnu:

| domain | runtime graph before / after | runtime + build graph before / after | web (wasm32) runtime graph |
|---|---|---|---|
| docs | 291 / 303 (+12) | 305 / 407 (+102) | 214 / 208 (-6) |
| sheets | 191 / 209 (+18) | 204 / 349 (+145) | 149 / 149 (0) |
| slides | 291 / 291 | 305 / 305 | 222 / 222 |
| pdf | 457 / 458 | 547 / 548 | 383 / 384 |

So for the web bundles there is no growth; for the desktop apps the shipped code grows by 12 (docs) and 18 (sheets) crates and the **build** by about 100-145 crates
(+ one C build, which compiled here on windows-gnu: the pdf domain already needs it). `cargo deny` with the existing per-domain configs: slides and pdf stay green;
docs and sheets fail on RUSTSEC-2024-0370 (`proc-macro-error` unmaintained, reached through `winprint -> fmt-derive`, compile-time only), the same advisory
`ci/deny/pdf.toml` already ignores with that reason. Adopting means adding the same ignore line to `ci/deny/docs.toml` and `ci/deny/sheets.toml`.

## 4. Dedupe gain

Counted over the union of the four domain locks (the future merged workspace), craft-libs-sourced packages:

| | before | after |
|---|---|---|
| craft-libs git revs in the locks | 5 (d27646a, c348732, 5c874d5, 666c358, f58bc1a) | **0 (one path source, 15ed713)** |
| craft-libs-sourced package entries | 7 (`craft-pdf`, `craft-print` x2, `craft-quit`, `craft-ui` x2, `krilla`) | **5** |
| duplicate craft-* crates that disappear | | **2** (`craft-print` and `craft-ui` each collapse from two revs to one) |

Honest accounting: the craft-* gain is **2 crates and 4 pins (5 revs to 1)**. Union lock size goes 928 to 931 packages and duplicate crate *names* 88 to 90, because
docs and sheets pick up the newer print stack with older major versions of some crates next to the ones they already have (base64 0.22/0.23, syn 1/2, ...). The gain is
therefore not a size or build-time win; it is that one craft-libs checkout, one lock source and one set of API revisions replace five pins, and the apps stop drifting apart.

## 5. Cost and risk of adopting

- slides needs the one-line deckcraft carried patch above (web only); without it adoption is not possible.
- docs and sheets take a newer craft-print/craft-ui (behaviour changes on Windows printing, see the 14 print commits d27646a..15ed713), and the build gets about 100-145 crates and a cmake/C step heavier. Tests do not exercise printing end to end.
- Whether the 15ed713 UI/print/pdf code behaves the same in the running apps was not examined beyond tests and checks (no GUI run of the patched builds).
- `domain.toml` has no `craft_libs` key yet; the plan wants `craft_libs = "vendored"` and gen-domain emitting the `[patch]` if the owner adopts.
- A future craft-libs sync moves all four apps at once, so a break such as f39922b shows up in one place (good) but blocks all four (bad); the sync gate (`cargo check` of every domain, wasm included) already catches it.

## 6. Recommendation and owner decision

**Recommendation: adopt, after the owner accepts two things: the deckcraft one-line carried patch and the heavier docs/sheets build.** The experiment found no other break: all four
domains compile native and wasm, and the app suites pass with the same counts as before and the same quarantine (pdfcraft: three load-sensitive timing tests failed in the full run and passed or mostly passed alone).

OWNER DECISION (nothing adopted in the main branch): "Move all four domains to one craft-libs revision (BASE 15ed713, vendored `upstream/craft-libs`) so that five git pins become one, at the price of
a deckcraft carried patch (`craft-pdf/write-simple-text`), a heavier docs/sheets desktop build, and a per-domain `[patch]` emitted by gen-domain." Until decided, the current pins stay.

Files: this report; `docs/phase1/evidence/craftlibs/` (lockdiff outputs, check log tails, test summaries); scratch tree `E:/office-target/p1/wt/craftlibs` on branch `wp4/craftlibs` (uncommitted edits: six manifests,
four upstream manifests, `scripts/{env.sh,ci.sh,lib/lock.sh}` redirected to a scratch target dir). `git status --porcelain domains/` in the main checkout is empty.
