# Adversarial review of `docs/ARCHITECTURE-DECISION.md` (Phase 0 exit gate)

Date: 2026-10-16. Reviewer: architect sub-agent. Method: read-only. Every number in the ADR was traced to its cited
Phase 0 source, and a few upstream facts were re-counted in `upstream/` at the Phase 0 HEADs. Nothing was built, run
or committed.

**ADR locations** refer to the section and to the line numbers of the ADR **before** this review's edits (672
lines). **Evidence** is `file:line`, with paths relative to `docs/` unless they start with `upstream/` or `spikes/`.

**Counts:** 1 blocker, 13 major, 24 minor. The blocker and all major findings are fixed in the ADR. Minor findings:
20 fixed, 4 left (marked LEFT).

---

## Blockers

### B1. The mobile recommendation is not tested against D1 and D5, and its deciding cost is hidden

**ADR location.** §0 row 4 (l.30); §2.2.1 matrix "Remaining cost" (l.191); recommendation (l.194–208); B-3 (l.619).

**What is wrong.**
1. **D1 is a hard criterion.** "Fails the Android touch tests → rejected" (l.46). No option passes it today:
   - A-web **FAILS** the 360 dp test (l.181).
   - A-web and B are both PARTIAL on long-press, with no selection handles (l.180).
   - The Phase 6 checklist the ADR adopts requires both handles and 360 px.
   
   The re-decision trigger, "A-web fails an item that B passes" (l.206), cannot fire on handles, because B fails that
   item too.
2. **The recommendation weighs a sized cost against an unsized one.** B is put at 7–11 engineer-months (ESTIMATE). A-web
   is "**UNSIZED**". The ADR still treats A-web as the cheap option.
3. **The A-web UI cost sits where D5 bites, and the ADR does not say so.**
   - The compact 360 dp layout, the handles and the simplified desktop toolbar are new code inside each app's
     upstream `ui-egui`: 53.8k / 25.6k / 30.9k / 58.7k lines.
   - No extension point exists. The only ribbon mode is `ribbon_collapsed`.
   - Upstream calls phones out of scope.
   
   So unless upstream accepts a layout-mode hook, this becomes the largest carried patch set, ×4 apps, in the hottest
   files. That conflicts with D5 ("thin layers + minimal patches", l.50). B's UI, by contrast, is our own code and
   complies with D5.
4. **"One effort serves both desktop and mobile" (l.201–202) is a reasoning, not evidence.** BRIEF Phase 6 task 3 asks
   for "a compact, contextual bottom toolbar, **not the desktop ribbon**". That is a different artefact from a desktop
   simplified toolbar.

**Evidence.**
- `BRIEF.md:426-430` (handles, 360 px, no overflow); `BRIEF.md:431` (bottom toolbar); `BRIEF.md:63` (phones out of
  scope).
- `phase0/mobile-a0.md:50, 48, 69` ("egui-core limits: no native selection handles").
- `phase0/mobile-b.md:76` (B: no handles).
- `phase0/wordcraft-patches.md:87` (no native selection handles).
- `phase0/workspace-conflicts.md:149` (ui-egui sizes).
- Read for this review: `upstream/wordcraft/crates/ui-egui/src/lib.rs:236`, `upstream/gridcraft/crates/ui-egui/src/lib.rs:55`,
  `upstream/deckcraft/crates/ui-egui/src/lib.rs:242`. `ribbon_collapsed` is the only mode; a grep for
  simplified/compact/touch modes finds none.
- `phase0/mobile-b.md:110-115` (B's estimate, and A's "compact layout ... real UI work").

**Fix (applied).**
- §0 row 4 now reads: "**v1 = A-web, conditionally.** … **No option passes D1 today:** A-web fails the 360 dp test,
  and no option has selection handles. A-web's remaining UI cost is **UNSIZED** and lands inside each app's upstream
  `ui-egui` crate …".
- §2.2.1 "Remaining cost" (A-web) gains the location, the line counts, the `ribbon_collapsed` evidence, and "an
  upstream PR or a large carried patch, ×4 apps", plus selection handles.
- §2.2.1 recommendation gains a lead paragraph, "**Read this first: no option passes D1 today.** … a **sized** cost
  for B … against an **UNSIZED** cost for A-web …".
- A new "**The condition (D5)**" paragraph.
- A new step 3, the "Phase 4 sizing check": (i) the 360 dp layout size and a handle design, (ii) upstream's answer to
  a layout-mode hook PR, (iii) A-web in the Expo WebView on a real phone. B is re-decided at the Phase 4 gate if
  these fail.
- The Phase 6 trigger gains: "the handles and the 360 dp layout cannot be done without a large carried patch".
- B-3, §3.1 item 5, the §3.2 Phase 4 row, risk 5, and the §2.6 "Still to write" row are updated to match.

---

## Major

### M1. The headline Excel effort is mislabelled, and the links sum is wrong in its source

**ADR location.** §0 row 6 (l.32); §1.2 (l.68); §2.3 table (l.255–256); §3.1 (l.376); §3.2 E row (l.395); §6 risk 2
(l.566); B-7 (l.623).

**What is wrong.**
1. The ADR's legend defines PROJECTION as "arithmetic on measured unit costs" (l.13). "31.5–51 … (PROJECTION from
   spike estimates)" (l.32) and "PROJECTION" (l.623) are sums of **estimates**, so by the ADR's own legend they are
   ESTIMATEs.
2. The links figure "17–25" comes from the source's own total, but the source's component rows add up to **18–27**:
   3–4 + 1–2 + 3–4 + 3–5 + 2–3 + 2–3 + 2–3 + 2–3.
3. The ADR's L row also leaves out "source loading" (2–3 d).

The memory subtotal 14.5–26 checks out: 1–1.5 + 5–9 + 3.5–6.5 + 2–4 + 3–5. With the corrected links figure the total
becomes **32.5–53 d (≈6.5–10.6 engineer-weeks)**.

**Evidence.** `phase0/gridcraft-scaling-and-links.md:306-315` (the rows) against `:317` ("≈17–25");
`phase0/gridcraft-templates-spike.md:394`; `STATUS.md:686-694`.

**Fix (applied).**
- §2.3 L row now reads: "syntax 3–4, model 1–2, I/O 3–4, calc 3–5, source loading 2–3, commands 2–3, consent/security
  2–3, verification 2–3 | **18–27** | Sum of the component rows … That report's own total says "≈17–25", which does
  not match its rows."
- Total row: "**32.5–53 (≈6.5–10.6 engineer-weeks)** | ESTIMATE: a sum of estimates, for "someone new to the code".
  Upstream review latency and rework come on top (unknown)".
- Every other occurrence is updated: §0, §1.2, §3.1, §3.2, §2.6, risk 2, B-7.

### M2. Excel correctness is overstated, and Excel interop is missing from the top risks

**ADR location.** §2.3 correctness bullet (l.239); §6 (absent).

**What is wrong.**
1. "Excel 16 reads the same formulas and values in every cell" holds for **w10 and the w10 edit-script result only**.
   COM opened them with alerts off, and w40–w120 were not checked in Excel.
2. The spike found that Excel **refuses** shared formulas that hold sheet-qualified references. It was found by
   bisection, and other rules are untested.
3. `check` reports that a 0.7.0 round trip loses the `core-properties` and `sheetMetadata` relationships.

The ADR's own cutover rule is per file, so files cross between Excel and gridcraft users. That makes round-trip
compatibility a top-tier risk, yet it appears only as one line in §7.

**Evidence.** `phase0/gridcraft-templates-spike.md:238, 240-249, 328`; `phase0/xlsx-bench.md:38`;
`phase0/fidelity-baseline.md:20`.

**Fix (applied).**
- The bullet now reads: "Excel 16 reads the same formulas and values in every cell, **on w10 and the w10 edit-script
  result only**. COM opened them with alerts off, so a repair prompt would not have been seen. w40–w120 were not
  checked in Excel".
- A new "**Excel compatibility hazards found**" bullet.
- A new §1.2 row "Files exchanged with Excel users during the cutover".
- A new **risk 11**, "Excel round-trip compatibility during the per-file cutover" (M/H).
- The track E gate now requires interactive Excel opens of w40–w120 and the census files.

### M3. Interactive editing at the owner's sizes is unmeasured but presented through open peaks

**ADR location.** §2.3 (l.224–234); §1.2 row (l.69); risk 4 (l.568); §7.

**What is wrong.**
1. Every patched peak is a CLI `info` run. The patched GUI was never run.
2. On w10 the edit script peaked at **871 MB, 4.5× the 194 MB open peak**.
3. The 0.7.0 GUI on w40 used 2,795 MB against 2,525 MB for the CLI.
4. Recalc latency per edit on w80 was never measured, and `crates/calc` has no threading.
5. The open times at the owner's sizes are left out. Patched w80 takes 73.7–80.4 s against 57.8 s for Excel. w120
   takes 182.4 s with rustc running, against 86.5 s.

The owner's workload is editing 80 MB+ files, not opening them from a CLI.

**Evidence.** `phase0/gridcraft-templates-spike.md:283-284, 308-310`; `phase0/xlsx-bench.md:48`; `STATUS.md:647`
("no threading in crates/calc").

**Fix (applied).**
- §2.3 gains a bullet "**Not measured: interactive editing at the owner's sizes.** … So the open peaks above are a
  floor for a user editing an 80 MB workbook, not a forecast".
- The open-time bullet gets the w80 and w120 times.
- §1.2: "**Editing in the GUI at this size is unmeasured**".
- The track E gate now includes a scripted w80 GUI session.
- Risk 4 is raised to M–H.
- New §7 row "Editing an 80 MB workbook in the GUI".

### M4. The HRM mobile embed is claimed as "the real HRM embed path"

**ADR location.** §2.2.1 recommendation (b) (l.197–198); matrix row (l.183); §1.2 HRM embed (l.76).

**What is wrong.**
1. The mobile HRM flow ran in **Chrome** on the emulator, against our fake HRM page and our docserver. The A0 run also
   used Chrome command-line flags.
2. The real path is a `react-native-webview` inside the Expo app, and A-web was never run in Android System WebView.
3. Only B's page was run in a WebView APK.

The claim "already passes the real HRM embed path on mobile, which is a WebView inside the Expo app" is therefore
untrue as written.

**Evidence.** `phase0/mobile-a0.md:4-5, 16, 51`; `phase0/wordcraft-patches.md:112`; `phase0/hrm-current-state.md:315-319`;
`phase0/mobile-b.md:104-107`.

**Fix (applied).**
- Recommendation (b) now reads: "passes the HRM embed flow (fake HRM page → docserver → status-6 callback) in
  **Chrome** on the emulator. The real path is a `react-native-webview` inside the Expo app …, and it is **not
  tested**".
- The matrix cell now reads "**PASS in Chrome** … **Not run in Android System WebView or in the HRM Expo app**".
- The B cell notes its WebView APK run.
- The §1.2 row is corrected.
- New §7 row; it is also part of the Phase 4 evidence task.

### M5. W2 is called a "proven pattern", and what it gives up is not stated

**ADR location.** §2.1.1 row (l.96); §2.1.2 W2 row (l.105); B-5 (l.621).

**What is wrong.**
1. Both examples link only engine-level crates, from wordcraft and gridcraft. Neither app carries a `[patch]`
   (verified: `[patch.crates-io]` entries are 0 for both).
2. No full app (desktop GUI, or web through trunk) has been built from a domain.
3. No domain has used deckcraft's or pdfcraft's `[patch]` blocks.
4. W2 also does not meet BRIEF §5's "deduplicate shared crates; resolve version conflicts explicitly". It **defers**
   both: craft-libs stays at 5 revs, and the conflicts stay unresolved.

**Evidence.** `phase0/mobile-b.md:12`; `phase0/gridcraft-scaling-and-links.md:23-27`; `phase0/workspace-conflicts.md:112-121`;
`upstream/{wordcraft,gridcraft}/Cargo.toml` (no `[patch.crates-io]`, counted for this review); `BRIEF.md:129-131`.

**Fix (applied).**
- §2.1.1 gains "**Limits of this evidence:** both examples link engine-level crates only, and both apps carry no
  `[patch]` …".
- The W2 row now reads: "The pattern is shown for engine crates only … A full-app domain build, and the copied
  `[patch]` sets, are UNVERIFIED; they are the first Phase 1 task." and "**It does not meet BRIEF §5 "one
  workspace"**, and it does not meet the brief's dedupe goals: it **defers** them …".
- New Phase 1 first task: a full-app build per domain, with W2 re-decided at the Phase 1 gate if it fails.
- B-5 is rewritten to match.

### M6. Upstream tracking deviates from a BRIEF §5 hard constraint without being flagged

**ADR location.** §2.1.3 (l.142); §0 row 7 (l.33).

**What is wrong.** BRIEF §5 says "Track upstream as **pinned git subtrees or submodules**". The ADR rejects both and
recommends vendored snapshots plus a patch queue, calling it "a squashed subtree in spirit". That is a deviation from
a hard constraint, but it is not framed as an owner decision. Only the workspace deviation is.

**Evidence.** `BRIEF.md:127-128`.

**Fix (applied).**
- §0 row 7: "Upstream is tracked as **vendored pristine snapshots plus a patch queue**, which is neither a subtree nor
  a submodule. **This deviates from BRIEF §5** (§2.1.3)."
- §2.1.3 verdict: "**But it is neither of the mechanisms BRIEF §5 names** …, so the owner decides it with B-5."
- B-5 now names both deviations.

### M7. The HRM test-environment constraint is missing, and the test environment holds production data

**ADR location.** §2.4 (l.299–331); §6; B-9 (l.625).

**What is wrong.** BRIEF §5 "HRM test environment only" appears nowhere in the ADR. The sources show the test-only
boundary is weak:
[redacted: HRM security detail]
- the main server uses the **same compose project name and path**;
- the test host's Traefik config carries public production routes.

So the pilot processes real personal data, and an operator can mistake one host for the other. B-9 also asks for a
rotation on "main" without saying who does it.

**Evidence.** `BRIEF.md:143-144`; `phase0/hr-backend-prompt.md:84-89`; `STATUS.md:758-760`.

**Fix (applied).**
- §2.4 gains a bullet "**HRM test environment only (BRIEF §5).**", with the three facts and the pilot rules
  (`hostname` check, no document bytes or names in logs, pilot documents treated like `corpus/`, main-server changes
  are the owner's or the HRM team's).
- New **risk 12**, "HRM test environment holds production data" (M/H).
- The Phase 5 gate adds "The pilot runs on `vds-test` only".
[redacted: HRM security detail]
  `vds-test` is a copy of production".

### M8. Windows 10 is marked as having no gap

**ADR location.** §1.2 "Windows 10" (l.71).

**What is wrong.**
1. "All four apps build, run and pass the CLI smoke tests" overstates the evidence. Only the **CLIs** were
   smoke-tested.
2. Of the GUIs, only the gridcraft 0.7.0 one was launched, and the session had no interactive desktop.
3. All Phase 0 builds were `windows-gnu`, and the shipping MSVC build was never made here.

The gap column says "—" for a primary target.

**Evidence.** `phase0/inventory.md:29, 55, 72`; `phase0/xlsx-bench.md:47-49`; `STATUS.md:489`.

**Fix (applied).** The row now reads: "All four apps build at HEAD with `windows-gnu`, and all four **CLIs** pass smoke
tests … The **GUIs were not exercised interactively** … | A GUI smoke test of all four apps, on an MSVC build". This
is added to the Phase 4 evidence task and to §7.

### M9. The ADR does not say plainly what v1.0 will not do

**ADR location.** §1 (absent).

**What is wrong.** The owner requirements imply exclusions that are scattered across the document or missing: macros,
`.doc`/`.xls` save-back, `.ppt`, external links before L, files over about 150 MB, upstream's silent chart, SmartArt
and OLE drops, iOS, and Sheets, Slides or PDF on phones. **Outlook and the other Office apps** are not mentioned at all,
although STATUS lists Outlook use as an open question.

**Evidence.** `STATUS.md:541-544, 569-571`; `BRIEF.md:72`; ADR §2.3.

**Fix (applied).** New §1.3 "What v1.0 will not do (on the Phase 0 evidence)", with 10 explicit bullets.

### M10. Accessibility (BRIEF §4.3) is absent

**ADR location.** Absent: §1.2, §6, §7.

**What is wrong.** "Accessible" is a v1.0 target. Phase 0 did not measure it on any platform. The web and mobile path
draws everything on one canvas, so screen-reader support there is unknown. The only evidence touching it is the
`uiautomationcore` import seen in the exes.

**Evidence.** `BRIEF.md:105`; `STATUS.md:529-530`; `phase0/mobile-b.md:112` ("accessibility text layer").

**Fix (applied).** A new §1.2 row "Accessibility … **Not measured in Phase 0** …", a new §7 row, and an entry in
"Other risks".

### M11. Code signing, SmartScreen and mass deployment are absent

**ADR location.** §2.5 (absent); §3.2 Phase 10 (l.404).

**What is wrong.** Replacing Office on office PCs means a GPO-deployed, per-machine MSI. An unsigned MSI, or one whose
certificate has no reputation yet, triggers SmartScreen warnings. The ADR mentions signing only at Phase 10, but the
Phase 4 pilots on office PCs need it, and the certificate is an owner procurement with lead time.

**Evidence.** `BRIEF.md:530, 576`; `STATUS.md:515-517` (installer required); ADR l.337 (GPO plan).

**Fix (applied).**
- A new §2.5 row "**Code signing, SmartScreen, mass deployment**", with the plan "The owner procures a code-signing
  certificate before the first Phase 4 pilot on office PCs (B-10) …".
- B-10 now includes the certificate.
- The Phase 4 row adds "A signed MSI on one domain PC".

### M12. Phase 0 closes with unmet tasks that are not framed as a gate decision

**ADR location.** §3.2 "Honest scope note" (l.406–407); Appendix B.

**What is wrong.**
1. BRIEF task 5 (corpus run, LibreOffice and x2t references) was not done.
2. Task 3B asked for "at most two options, for example Tauri v2 … vs Flutter". Neither was evaluated, and iOS was
   impossible.
3. Task 6 asks for "an updated phase plan **with estimates**", but phases 1–10 are UNSIZED apart from track E and B.

The ADR is honest about each gap in places, but it never asks the owner to accept closing the gate with them.

**Evidence.** `BRIEF.md:225-231, 242-247, 248-253`; `phase0/mobile-b.md:108`; `phase0/fidelity-baseline.md:3, 22`.

**Fix (applied).**
- A new §0 row 9, "Phase 0 gaps".
- A new **B-15**, "Closing Phase 0 with known gaps", with options (a) close now and (b) hold the gate.
- The scope note now ends: "**This falls short of BRIEF Phase 0 task 6** … and the owner decides at this gate whether
  to accept that (B-15)".
- New §7 rows for the Tauri/Flutter evaluation and for Office 2010–2019 files.

### M13. Product UI changes in upstream `ui-egui` are missing from the patch strategy

**ADR location.** §2.6 "Still to write" (l.363); §2.1.2 consequences (l.116–118).

**What is wrong.** The ADR says branding goes through "small carried patches or upstream product-config hooks". The
desktop product needs more than that, all inside four upstream `ui-egui` crates with no extension point:
- the simplified toolbar with Show/Hide ¶ on it (BRIEF §4.3, an owner requirement);
- "Sign in with HRM";
- "Open from / Save to HRM".

These are likely the largest patch series, but they are not listed in §2.6 or in the rebase-cost discussion.

**Evidence.** `BRIEF.md:102`; `STATUS.md:577-582`; `phase0/hrm-integration-design.md:255`; the `ribbon_collapsed`
evidence in B1.

**Fix (applied).**
- §2.6 "Still to write" adds "**All four `ui-egui` crates, likely the largest series:** the simplified toolbar with
  Show/Hide ¶ on it (desktop), the compact 360 dp layout and selection handles (mobile), and the "Sign in with HRM" and
  "Open from / Save to HRM" UI. No extension point exists for any of them".
- §4.3 now notes that a carried `ui-egui` layout series could cost more than the templates carry.

---

## Minor

| # | ADR location | What is wrong | Evidence | Fix | Status |
|---|---|---|---|---|---|
| m1 | §2.2 web row (l.164) | HEAD brotli "9.4 / 4.7 / 9.3 / 9.4 MB" is decimal MB, but it sits next to release sizes in MiB, which inflates the apparent growth | `phase0/inventory.md:24`; `STATUS.md:20-27` | "HEAD wasm brotli: **9.0 / 4.5 / 8.9 / 9.0 MiB** (9,441,597 / 4,731,289 / 9,300,367 / 9,390,759 B)" | Fixed |
| m2 | §2.1.1 (l.98) | wordcraft desktop glue is 3,191 lines, not 2,886: the count missed `src/animations/desktop.rs` (305) | `upstream/wordcraft/apps/wordcraft/src/**` (recounted) | "3,191 / 1,768 / 1,622 / 3,138" | Fixed |
| m3 | §2.1.2 (l.133) | "up to 2.2 h per app": pdfcraft took 8,810 s = 2.4 h (an upper bound), and deckcraft 0.9 h | `phase0/inventory.md:27` | "0.9–2.4 h per app (pdfcraft's 2.4 h is an upper bound)" | Fixed |
| m4 | §2.3 (l.231) | "19–20 after", but w120 measured 20.4 and w80 18.9 | `phase0/gridcraft-templates-spike.md:288-293` | "18.9–20.4 after" | Fixed |
| m5 | §2.3 table (l.228–229); §1.2 (l.69) | "aborted above 4 GB": our harness killed the runs at 3,993 / 4,011 MB when free RAM fell below 1 GB. gridcraft's own outcome is unknown | `phase0/xlsx-bench.md:35`; `phase0/gridcraft-templates-spike.md:283-284` | "killed by our RAM guard at 3,993 MB", plus a note that peaks are CLI and in MiB | Fixed |
| m6 | §4.4 (l.476–477) | The deckcraft crash rate leaves out that **both** `cargo test` runs crashed. 1 of 8 is the direct-binary rate | `phase0/inventory.md:87, 96-97` | "It crashed in **both** `cargo test` runs, in 1 of 8 direct multi-threaded runs, and in 0 of 4 single-threaded runs" | Fixed |
| m7 | §2.4 (l.317) | "`_hrm_*` survived 4/4": only `_hrm_desc_` was tested, through the release CLI. `_hrm_sig_` and the letter and report bookmarks were not | `phase0/fidelity-baseline.md:115`; `phase0/hr-backend-prompt.md:113-116` | Rewritten to name what was and was not tested; the Phase 5 gate now covers decree, letter and report | Fixed |
| m8 | §3.2 Phase 5 (l.399) | "9 PRs (PR-0–PR-8)": the steps are PR-0 (baseline) plus 9 code PRs (1, 2, 3, 4a, 4b, 5, 6, 7, 8) | `phase0/hr-backend-prompt.md:378-791` | "a PR-0 baseline plus 9 code PRs (PR-1–PR-8, with PR-4 split into 4a/4b)" | Fixed |
| m9 | §4.3 (l.448–449) | "PROJECTION from its 27-file footprint" is a judgement, and the 27 files are commit 2 only | `phase0/gridcraft-templates-spike.md:181` | "(ESTIMATE: commit 2 alone touches 27 files)" | Fixed |
| m10 | §2.5 (l.340) | Only the decimal separator is listed. Dates (`12.10` against `12/10`) and grouping formats (`#.##0`) also differ | `phase0/gridcraft-pdf-fonts.md:78` | The row is renamed "Locale number and date display" with both facts | Fixed |
| m11 | §2.2.1 A-web column | The column mixes A0 results (v0.7.0, Chrome flags, Chrome's own software GL) for pinch, rotation, 360 dp, PSS and the HRM flow with post-patch results in stock Chrome. The memory comparison with B is not like for like | `phase0/wordcraft-patches.md:114`; `phase0/mobile-a0.md:16` | A "Mixed conditions in the A-web column" note under the matrix heading; "measured under different Chrome GL settings" in step 2 | Fixed |
| m12 | §2.2.1 step 2 (l.204–205) | B's "lifecycle robustness" PASS ran with DevTools attached, which keeps the page "visible" | `phase0/mobile-b.md:79` | The caveat is added to step 2 | Fixed |
| m13 | B-8 (l.624); §3.2 Phase 3 | The LibreOffice `.ppt` sidecar is recommended, but LibreOffice was never installed or tested | `STATUS.md:494` | §1.3: "That sidecar is untested: LibreOffice was not installed in Phase 0" | Fixed |
| m14 | Absent | C: has about 34 GB free; per-domain target directories ×5 need a disk plan | `STATUS.md:492`; `CLAUDE.md` | §3.2 Phase 1: "Domain target directories go on E: …"; added to "Other risks" | Fixed |
| m15 | §4.4 | No nightly duration is given. The four cold release test runs sum to about 6.6 h on this PC | `phase0/inventory.md:27` | A nightly bullet with the PROJECTION | Fixed |
| m16 | §2.1.1 (l.93) | "33 for the union" cannot be reproduced from saved artefacts. Only the merged `dups.txt` was kept: 32 names, recounted. Per-app `cargo tree -d` outputs were not saved | `spikes/workspace/README.md`; `spikes/workspace/dups.txt` | Add "(per-app lists not saved; re-run `cargo tree -d` per app in Phase 1)" | LEFT |
| m17 | §2.3 cutover (l.262–263) | "gridcraft 0.7.0 already handles about 40 MB" leaves out the 2.5 GB CLI / 2.8 GB GUI peak and the 45 s open | `phase0/xlsx-bench.md:29, 48` | Qualifier added | Fixed |
| m18 | §6 risk 4 (l.568) | L = M is optimistic, given the unmeasured editing peaks and the unknown fleet RAM | M3 evidence | L = M–H, with the evidence and mitigation updated | Fixed |
| m19 | §0 row 6 (l.32) | The source says the Excel work conflicts with BRIEF §5 "minimal upstream patches" unless upstream accepts it; the ADR does not say so | `phase0/gridcraft-scaling-and-links.md:343-345` | "carrying it ourselves would break BRIEF §5 "minimal upstream patches"" | Fixed |
| m20 | §2.3 | The memory patches change the xlsx reader only. `.xls`, `.xlsb` and `.ods` still build one tree per formula | `phase0/gridcraft-templates-spike.md:334` | A "Scope of the memory patches" bullet | Fixed |
| m21 | §6 risk 1 (l.565) | The deckcraft HEAD wasm (35.1 MB) against release (25.5 MB) is put down to drift. The release was built by upstream CI and HEAD by us (`windows-gnu` host, trunk 0.21.14), so the comparison is unverified | `phase0/inventory.md:22`; `STATUS.md:26` | Add "(build environments differ; growth attribution UNVERIFIED)" | LEFT |
| m22 | §2.2.1 B "Remaining cost" (l.191) | "**Sheets is blocked**" overstates it: gridcraft has no egui-free grid renderer, so B for Sheets needs one built (upstream work) | `phase0/layer-map-gridcraft.md:50-51, 125-126` | Fixed in risk 5 ("one would have to be built"). The matrix cell still says "blocked"; suggested text: "**Sheets needs a new grid renderer**" | Partly fixed |
| m23 | B-9 (l.625) | The rotation on "main" has no actor | `BRIEF.md:143-144` | Done as part of M7 | Fixed |
| m24 | §6 risk 8 | Impact "**Critical**" is outside the table's H/M/L scale | ADR l.561 | Use "H (critical)" | LEFT |

Checks that passed:
- D4 ratios (96 / 98 / 97 / 71%).
- Patched/Excel ratios (0.9 / 1.5 / 1.6 / 1.9×; both scripts report MiB: `spikes/xlsx-bench/excel_ref.ps1:26,36`,
  `measure.py:18`).
- GUI and CLI footprint sums (314.3 / 224.1 MB).
- 505/523/496 functions = 94.8%.
- The templates diffstat (+1,667/−132 = production +865/−106 + tests +773/−25 + docs +29/−1; 39 unique files).
- wordcraft patch sizes and wasm deltas (+9,263 B brotli; gridcraft +8,487 B gzip).
- pdf-fonts (15 files, +1,503/−18; 0.415 → 0.754).
- The docserver timings.
- The mobile B numbers.
- The workspace facts (97 members, 923 lock entries, 5 craft-libs revs, 27 differing hayro files).
- `[patch.crates-io]` counts (deckcraft 1, pdfcraft 8; recounted).
- `members` globs.
- `show_discord` at `ui-egui/src/lib.rs:259`.
- `default_apps.rs:1-14`.

The `<!-- R2: ... -->` placeholder in §2.3.1 is untouched.

---

## What was changed in `ARCHITECTURE-DECISION.md`

1. **§0:**
   - row 4 (mobile) rewritten as conditional (B1);
   - row 6: 32.5–53 d ESTIMATE, the BRIEF §5 conflict and the unmeasured items (M1, M3, m19);
   - row 7: the upstream-tracking deviation (M6);
   - new row 9, "Phase 0 gaps" (M12).
2. **§1.2:**
   - links row 18–27;
   - the 80 MB row (RAM guard, open times, GUI editing unmeasured);
   - new row "Files exchanged with Excel users";
   - Windows 10 row corrected;
   - Android row (360 dp FAIL, no handles);
   - HRM embed row (Chrome, not the Expo WebView);
   - new row "Accessibility" (M1–M4, M8, M10, m5).
3. **New §1.3 "What v1.0 will not do"** (M9, m13).
4. **§2.1:**
   - limits of the W2 evidence;
   - the W2 row (engine crates only; does not meet the brief's one-workspace and dedupe goals);
   - glue line count 3,191;
   - test-build hours;
   - §2.1.3 verdict flags the BRIEF §5 tracking deviation (M5, M6, m2, m3).
5. **§2.2:**
   - web row in MiB with byte counts;
   - the mixed-conditions note on the A-web column;
   - the HRM-embed cell;
   - the A-web "Remaining cost" cell (`ui-egui` location, no extension point, handles);
   - the recommendation rewritten: the "no option passes D1" lead, the D5 condition, a new Phase 4 sizing check
     (step 3), Phase 6 triggers, A-native renumbered to step 5 (B1, M4, m1, m11, m12).
6. **§2.3:**
   - RAM-guard labels and units note;
   - 18.9–20.4 MB/MB;
   - w80/w120 open times;
   - qualified Excel correctness;
   - new "Excel compatibility hazards" and "Not measured: interactive editing" bullets;
   - L row itemised as 18–27;
   - total 32.5–53 ESTIMATE;
   - cutover qualifier;
   - scope of the memory patches (M1–M3, m4, m5, m17, m20).
7. **§2.4:**
   - the HRM test-environment bullet (production-data clone, shared compose name, Traefik routes, pilot rules);
   - bookmark wording (M7, m7).
8. **§2.5:** locale row widened to dates; new code-signing / SmartScreen / GPO row (M11, m10).
9. **§2.6:** "Still to write" gains links 18–27 d and the four `ui-egui` series (M13, M1).
10. **§3:**
    - §3.1 track E figure and item 5 (Phase 4 sizing);
    - Phase 1 first task (full-app domain builds; E: target directories);
    - the E row (estimate and gate: GUI editing session, interactive Excel at scale);
    - the Phase 4 row (sizing and evidence tasks, MSVC GUI smoke, signed MSI, the B-3 re-check);
    - the Phase 5 row (PR count, all bookmarks, `vds-test` only);
    - the scope note references B-15 (B1, M1, M3, M5, M8, M11, M12, m8, m14).
11. **§4:**
    - §4.3 carry-cost wording (ESTIMATE, the `ui-egui` comparison);
    - §4.4 deckcraft crash rate;
    - nightly duration (m6, m9, m15).
12. **§6:**
    - risk 2 (18–27 d, the layout hook);
    - risk 4 (M–H, editing peaks);
    - risk 5 (layout location, Phase 4 re-decision);
    - new risks 11 (Excel round trip) and 12 (HRM test env holds production data);
    - "Other risks" adds accessibility, signing and C: disk (M2, M7, M10, M11, m14, m18, m22).
13. **§7:**
    - the Excel auto-repair row is sharpened;
    - new rows: GUI editing at 80 MB, A-web in the Expo WebView, W2 full app, accessibility, desktop GUIs on Windows,
      B's desktop shell, Office 2010–2019 files.
14. **Appendix B:**
    - B-3 (conditional);
    - B-5 (both BRIEF §5 deviations, W2 confirmed at the Phase 1 gate);
    - B-7 (32.5–53 d ESTIMATE);
    - B-9 (actor, Traefik question);
    - B-10 (code-signing certificate);
    - new B-15 (closing Phase 0 with known gaps).
