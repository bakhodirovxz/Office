# Sizing of Phases 2–10 (WP12, the B-15 week-1 promise)

| | |
|---|---|
| **Status** | Draft 1, written 2026-10-17, before Phase 1 day 1. **To be refined at the Phase 1 gate** with Phase 1 actuals (WP3 build times, WP5 and WP14 rebase cost, the real duration of Phase 1 against its 20-day plan). |
| **Work package** | `docs/phase1/PLAN.md` WP12. The plan names the file `sizing.md`; this is the same file (Windows paths are case-insensitive). |
| **Inputs** | `docs/BRIEF.md`; `docs/ARCHITECTURE-DECISION.md` §2–§7 and App. B/C; `docs/STATUS.md`; every report in `docs/phase0/`; `docs/phase1/PLAN.md`; spike patch headers and file timestamps under `spikes/`. |
| **Decisions in force** | A-web for mobile (conditional, re-decided at the Phase 4 gate); priority fidelity (incl. Excel) → mobile → co-editing; W2 build domains + vendored snapshot + patch queue; **no upstream PRs, so every upstream change is carried**; our layer MIT OR Apache-2.0. |

**Labels used for every number**
- **MEASURED**: from a Phase 0 run or artefact; the source is cited.
- **P0-ESTIMATE**: an estimate written in a Phase 0 report, cited. It is not a measurement.
- **JUDGEMENT**: my estimate for this document. Most numbers here are JUDGEMENT. No Phase 1 unit cost exists yet.
- **PROJECTION**: arithmetic on the above.
- **UNSIZED**: no basis for any number; the owning phase sizes it first.

---

## 0. Summary

| Phase | Low | Likely | High | Risk buffer | Plan-to (likely + buffer) | Main driver of the range |
|---|---|---|---|---|---|---|
| 2 Transliteration | 11 | 15.75 | 23.5 | 4 | 19.75 | Owner approval of the golden list; run-level edits in 3 engines |
| 3 Converter | 15 | 21 | 29 | 5 | 26 | HTTP job API; fidelity table needs the corpus |
| 4 Shell (web + desktop) | 30.5 | 44.5 | 63 | 12 | 56.5 | Simplified toolbar ×4 apps as carried `ui-egui` patches; MSI and signing |
| 5 Document server + HRM (our side) | 24.5 | 35.5 | 51.5 | 10 | 45.5 | Desktop sign-in and sync; permissions mapping |
| 6 Mobile (A-web, Android, 4 apps) | 33.5 | 51 | 82 | 20 | 71 | **Compact layout and handles are UNSIZED until the Phase 4 sizing check** |
| 7 Plugins | 24 | 35 | 51 | 10 | 45 | No Phase 0 evidence at all |
| 8 Fidelity (continuous) | 31 | 51.5 | 87 | 15 | 66.5 | A budget: the corpus has not been run |
| 9 Co-editing (Docs only) | 38 | 57.5 | 87 | 20 | 77.5 | Engine has no operation model; Sheets/Slides UNSIZED |
| 10 Release and operations | 18 | 26.5 | 37.5 | 6 | 32.5 | Security-review findings |
| Track E Excel engine | 15 | 34 | 69.5 | 15 | 49 | P0-ESTIMATE 32.5–53 d (human-scale), converted |
| **Total, our stream** | **240.5** | **372.25** | **581** | **117** | **489.25** | |
| HRM side (hr-backend PR-0–PR-8 + client changes), separate stream | 16.75 | 22.75 | 30 | — | — | Run by the hr-backend agent; add it if our stream does it |
| Not included | iOS (4–10 + 2–4 packaging, needs a Mac); macOS desktop (needs a Mac); approach B (51–231); co-editing for Sheets/Slides; multi-threaded recalc | | | | | UNSIZED or conditional |

All figures are **stream-days** (§1.1), i.e. JUDGEMENT or converted P0-ESTIMATE, never measurements.

**Calendar** (§4, one agent-driven stream, carried-patch tax and owner latency included):

| | Optimistic | Likely | Plan-to | Pessimistic |
|---|---|---|---|---|
| Phases 2–10 + E done | **≈ Jan 2028** (≈59 weeks) | **≈ Jul 2029** (≈139 weeks) | ≈ May 2030 (≈182 weeks) | **≈ 2034** (≈400 weeks): does not converge, the plan would have to change first |
| First pilot (§5: Docs-only HRM embed with uz-translit, on the HRM test env) | **mid-Dec 2026** | **late Jan 2027** | — | **mid-May 2027** |

**Key messages**
1. **The carried-patch tax is the swing factor.** At weekly sync it is 0.4–2.6 stream-days a week at the Phase 1 gate and grows to 1–10 a week by v1.0, against a capacity of 5 (JUDGEMENT model, §3; it is replaced by WP5's measurement). Each 0.5 stream-day a week of tax moves the likely end date by about 15–20 weeks.
2. **Splitting Phase 5** so that a Docs-only HRM pilot follows Phase 2 brings the first pilot forward by about 5 months (likely: late Jan 2027 instead of mid-June 2027). This is an owner decision (§6).
3. **Mobile is the largest unknown.** A-web is 33.5–82 stream-days; if the Phase 4 gate switches to B, Docs alone is 51–231 and Sheets is blocked (§2.5).

---

## 1. Units, evidence and conversion

### 1.1 The unit: one stream-day

**One stream-day = one working day of the single AI-agent-driven engineer stream**: the orchestrator plus its parallel sub-agents, about 9 agent-hours, sharing **one** 11.8 GB build PC under a serial build lock, with heavy builds and tests in night slots. This is the unit of `PLAN.md` §8 (≈185 agent-hours + ≈45 machine-hours ≈ 20 working days). A week is 5 stream-days of capacity before the carried-patch tax (§3) and owner latency.

### 1.2 Measured Phase 0 effort data

The durations below come from file modification times and `git format-patch` `Date:` headers. Modification times bound the **end** of a spike well; the **start** is often not recorded. So treat them as approximate. MEASURED in that sense only.

| Spike | Output (MEASURED) | Wall time (MEASURED from timestamps) | What the Phase 0 plan expected (P0-ESTIMATE) |
|---|---|---|---|
| Docserver S0–S7 | Rust/axum crate (JWT, SSRF guard, sessions, callback, fake HRM) + Playwright e2e, 3/3 green, negative tests ([docserver-results]) | Plan last edited 17:10, first spike file 17:24, results 17:43 (2026-10-15). Under about 1 h after the plan; start not recorded | Not given in days ([spike-plans] §4.6) |
| wordcraft patches 0001–0005 | 9 files, +651/−18 (≈200 test lines), verified on the emulator ([wordcraft-patches]) | Commits 19:50–20:38, emulator evidence to 01:34, report 01:49 → ≈6–8.5 h | PLAN WP12: "about one day" |
| gridcraft templates round 1 | 39 files, +1,667/−132 (production +865/−106), 25 new tests, bit-identical values, Excel COM check ([gridcraft-templates-spike] §2–4) | Previous report 20:03 → patches 02:25 → report 02:51 → ≈7 h | Steps 1–2: **14–27 engineer-days** ([gridcraft-templates-spike] §8). Remaining to production after the spike: **5–9 d** |
| gridcraft memory round 2 | 30 files, +1,368/−244, 11 commits, re-measured after an accidental overwrite ([gridcraft-memory-r2] §4.1) | Probe 08:29 → patches 11:35–18:44 → report 22:27 → ≈14 h | Remaining to production: 5–8 d |
| gridcraft PDF fonts | 15 files, +1,503/−18 ([gridcraft-pdf-fonts]) | Finding at 09:26–09:29 → patches 11:30 → report 11:31 → ≈2 h if it started at the finding | — |
| Fidelity harness + synthetic set | `run.py`, Office COM generator, 10 files ([fidelity-baseline]) | 07:36 → 09:41 → ≈2 h | — |
| Mobile A kill test | Probe app, 170-line shim, Gradle, emulator matrix ([mobile-a-kill]) | 10:51 → 13:24 → ≈2.5 h | A1–A3 "about 1 day" ([spike-plans] §3A.6) |
| Mobile B | 281-line bind crate, 440-line page, WebView APK, full matrix ([mobile-b]) | Evidence 13:42–14:24, report 14:27; ≥1 h, start not recorded | B1–B4 "about 2 days" + B5 0.5–1 day ([spike-plans] §3B.4) |
| **All of Phase 0** | 6 tasks, ≈10 spikes, ADR, adversarial review | First report 16:48 (10-15) → last spike report 22:27 (10-16) → ADR 10-16 → owner gate 10-17. ≈30 h of round-the-clock agent work | — |

Other measured unit costs that the estimates below use:
- Cold release builds per app: GUI 323–612 s, web 420–897 s; first release test runs 3,213–8,810 s per app; ≈6.6 h for all four (PROJECTION) ([inventory], ADR §4.4).
- Excel-scale runs: w80 opens in 67–80 s and saves in 102–157 s; some fuzz seeds exceed 2.5 GB ([gridcraft-templates-spike] §5–6, [gridcraft-memory-r2]).
- **Owner gate latency, one data point:** ADR finalised 2026-10-16, gate decisions recorded 2026-10-17 (ADR App. C): ≈1 working day. B-7…B-14 are still open, so some answers take longer.

**What this data shows.** Prototype-grade spikes ran **5–20× faster** than the human-scale estimates written next to them (A kill: 1 day planned, ≈2.5 h taken; B: 2.5–3 days planned, hours taken; templates: 14–27 days planned for steps 1–2, a measured and tested but unhardened implementation in ≈7 h). **What it does not show:** production hardening, review, verification on real files, soak, rebase cost and owner latency. Phase 0 measured none of these. Phase 1 is the first measurement of production-grade throughput.

### 1.3 Converting Phase 0 human-scale estimates (JUDGEMENT)

The Phase 0 reports estimate production work in "engineer-days for someone new to the code" ([gridcraft-scaling-and-links] §2, [gridcraft-templates-spike] §6). I convert those to stream-days with factor **×0.35 (low end of the source range) / ×0.6 (midpoint) / ×1.0 (high end)**:
- ×0.35 takes part of the measured 5–20× spike speed-up;
- ×1.0 assumes none of it, because a large share of production work is machine-bound (one PC, serial builds, w80/w120 runs, soak that does not fit 11.8 GB) or owner-bound.

The factor is JUDGEMENT. Recalibrate it at the Phase 1 gate: if Phase 1 finishes in N working days against its 20-day plan, scale the JUDGEMENT items by roughly N/20.

**Items marked JUDGEMENT** are written directly in stream-days, anchored as follows: a prototype spike produced roughly 650–1,700 changed lines, tests included, in 0.25–1.5 stream-days (§1.2). Production work costs ×2–4 that, for hardening, real-file verification and machine time.

### 1.4 What the item numbers exclude
- **The carried-patch tax** (§3): it is added per calendar week in §4.
- **Owner latency** (gates, reviews, hardware): added in §4.
- **Work in other teams' repos:** the hr-backend, hr-frontend and hrm-mobile changes are sized separately (§2.4).

---

## 2. Per-phase sizing

Each table gives low / likely / high stream-days and the basis of each item. "Carried" means the item adds patches to `patches/` (no upstream PRs, B-6).

### 2.1 Phase 2: Transliteration (`uz-translit`)

**Scope as it now stands**
- A pure-Rust crate with no dependencies, built for native and wasm, covering BRIEF Appendix A, plus the U+02BB/U+02BC forms that Gboard emits ([mobile-a0]).
- A Docs command at run level: selection or whole document, either direction, one undo step, optionally tracked changes, digraphs across run boundaries, field codes and hyperlink targets skipped.
- The same command in Sheets (cell text, not formulas) and Slides (text frames, notes).
- A WASM/npm package for the HRM web app.
- A golden list of ≥500 pairs, **owner-approved**; a round-trip property test; formatting-preservation tests.
- A 108-page .docx in under 2 s (ONLYOFFICE plugin: 62 s, [BRIEF] §3).
- Owner-added context: the HRM pilot users lose the ONLYOFFICE `uz-translit` plugin when they switch editors ([hr-backend-prompt] OPEN-Q1). So Phase 2 must precede the pilot (ADR §3.1 item 4).
- The seed (`vendor-notes/uz-translit-onlyoffice/`) **is still missing** (checked 2026-10-17). HRB's untracked `deploy/onlyoffice-plugins/` is an alternative seed if the owner agrees.

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 2.1 | Crate: rules, exceptions dictionary, case handling, untouched tokens (URLs, codes, numbers), property test; native + wasm | 1.5 | 2 | 3 | JUDGEMENT. ≈1.5–2.5k lines with tests; spike rate in §1.2 |
| 2.2 | Draft the golden list (≥500 pairs, both directions) and the exceptions dictionary; export for owner review | 1 | 1.5 | 2 | JUDGEMENT. Owner review time is separate |
| 2.3 | Docs command: run walk, digraphs across runs, skip fields and hyperlinks, one undo step, optional tracked changes (carried engine patch) | 2.5 | 3.5 | 5 | JUDGEMENT. Tracked-change recording through the engine is unexplored |
| 2.4 | Sheets command: cell text including rich-text runs, never formulas (carried) | 1 | 1.5 | 2.5 | JUDGEMENT |
| 2.5 | Slides command: text frames, tables, notes (carried; deckcraft's engine owns text, [layer-map-deckcraft] §4) | 1 | 1.5 | 2.5 | JUDGEMENT |
| 2.6 | WASM/npm package with TS types and an HRM web example (not published without the owner's OK) | 0.5 | 1 | 1.5 | MEASURED anchor: the B bind crate went to wasm through trunk inside one spike ([mobile-b] B1) |
| 2.7 | Menu/ribbon entry and dialog in 3 apps, i18n strings (carried `ui-egui`) | 1 | 1.5 | 2 | JUDGEMENT |
| 2.8 | Formatting-preservation tests (XML diff apart from text) for docx, xlsx and pptx fixtures | 1 | 1.5 | 2 | JUDGEMENT |
| 2.9 | 108-page measurement and any optimisation needed | 0.5 | 0.75 | 1.5 | JUDGEMENT. The 62 s is API overhead ([BRIEF] §3); our layout time on 108 pages is unmeasured |
| 2.10 | Gate: review and report | 1 | 1 | 1.5 | JUDGEMENT |
| | **Subtotal** | **11** | **15.75** | **23.5** | |

- **Dependencies:** Phase 1 (domains build, patch queue, i18n catalogs, the U+02BB fallback font from WP8). The owner's seed and list approval.
- **From the owner:**
  - Deliver `vendor-notes/uz-translit-onlyoffice/`, or allow the HRB plugin as the seed.
  - Approve the golden list and the exceptions dictionary (BRIEF App. B): about 4–8 owner-hours (JUDGEMENT), ideally within one week.
  - Provide a real 108-page document, or approve a synthetic one.
- **Risk buffer:** 4 stream-days (≈25%). Technical risk is low. The swing is the owner's list review and disagreements on edge cases (ц, е/э, loanwords).
- **Carry added:** about 6–9 patches (3 engine commands, 3 UI entry points, a possible tracked-change hook).

### 2.2 Phase 3: Converter ("Konvertor")

**Scope as it now stands**
- A `convert` CLI over the upstream CLIs **as subprocesses** (W2, ADR §2.1.2), one job per process. This also contains gridcraft's multi-GB peaks.
- Directions: docx ↔ odt/rtf/html/md/txt, .doc import, → pdf/png; xlsx, .xls/.xlsb/.ods import, → pdf/csv; pptx → pdf/png; PDF combine/split/extract/text.
- Owner requirement "files from Office 2010 onward, legacy formats included":
  - .doc and .xls convert to OOXML with an explicit warning (they cannot be saved back);
  - **.ppt only through a LibreOffice headless sidecar** if the owner keeps .ppt (B-8, still open).
- A PDF→DOCX sidecar: pdf2docx with `parse_stream_table=False`, then `merge_lines.py`, in a container, never linked. **`merge_lines.py` is missing** (`vendor-notes/` absent).
- Transliteration as an option. An HTTP API (batch, queue, progress, zip, limits, timeouts, cleanup). A UI page. A fidelity table per direction against LibreOffice and x2t.
- 80 MB+ workbooks go through the converter too, so job concurrency must respect RAM (1.5–2.6 GB peaks per job, [gridcraft-templates-spike] §5).

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 3.1 | Orchestrator CLI (`services/`): format matrix, subprocess per job, exit codes, warning capture | 2 | 3 | 4 | JUDGEMENT |
| 3.2 | All directions with tests | 2 | 3 | 4 | MEASURED anchor: all four CLIs pass smoke tests, including docx→odt→docx and PDF export ([inventory]); gridcraft PDF needs `run --cmd file.exportPdf` |
| 3.3 | PDF→DOCX sidecar container and HTTP wrapper | 1.5 | 2 | 3 | JUDGEMENT. The pipeline itself is the owner's, already tested ([BRIEF] §3) |
| 3.4 | .ppt sidecar (only if B-8 keeps .ppt) | 1 | 1.5 | 2 | JUDGEMENT. LibreOffice was never installed in Phase 0 |
| 3.5 | Transliteration option (calls the Phase 2 commands through `run --cmd`) | 0.5 | 0.5 | 1 | JUDGEMENT |
| 3.6 | HTTP API: batch upload, job queue, progress, zip, size/page limits, timeouts, temp cleanup, auth | 3 | 4 | 5 | MEASURED anchor: the axum/JWT/SSRF docserver prototype took under a day ([docserver-results], §1.2); production ×3–5 |
| 3.7 | UI page (standalone web page; moved into the shell in Phase 4), i18n | 1.5 | 2 | 3 | JUDGEMENT |
| 3.8 | Fidelity table per direction against LibreOffice and x2t, on the corpus | 2 | 3 | 4 | JUDGEMENT. Needs WP11's references and the corpus |
| 3.9 | Large workbooks: RAM-aware concurrency, timeouts | 0.5 | 1 | 1.5 | JUDGEMENT |
| 3.10 | Gate | 1 | 1 | 1.5 | JUDGEMENT |
| | **Subtotal** | **15** | **21** | **29** | |

- **Dependencies:** Phase 1 and Phase 2 (BRIEF §7). The corpus and the LibreOffice/x2t references (PLAN WP11). Docker.
- **From the owner:** B-8 (.ppt: how common, keep or drop); deliver `merge_lines.py`; the corpus (B-14); accept running the AGPL/MPL sidecars as separate containers. Review time ≈2 h (JUDGEMENT).
- **Risk buffer:** 5 stream-days. Most of the risk is in the fidelity table (corpus) and the PDF→DOCX weak spots, which stay out of scope.
- **Carry added:** few or none. The converter is our code, over upstream CLIs.

### 2.3 Phase 4: Unified shell (web + desktop)

**Scope as it now stands**
- Home screen; lazy-loaded wasm per editor; budgets enforced in CI.
- **Owner requirement "replace MS Office on Windows 10/11":**
  - one suite MSI with our own ProgIDs, UpgradeCode, AppUserModelID and file associations;
  - a GPO default-association XML for domain PCs (B-11);
  - a **signed** build on MSVC (Phase 0 built only `windows-gnu`; [inventory]);
  - a GUI smoke test of all four apps;
  - WARP and RDP measurements;
  - a Windows 11 run, on owner hardware.
- **Simplified toolbar by default, with Show/Hide ¶ visible** (the `view.marks` command exists, `wordcraft/crates/engine/src/cmd/view.rs:18`), plain Uzbek labels, and the full ribbon as a toggle. **There is no extension point** (`ribbon_collapsed` is the only mode, ADR §2.2.1). So this is a carried patch in each of four `ui-egui` crates of 53.8k / 25.6k / 30.9k / 58.7k lines.
- Shared services: settings, recent files, fonts, spell-check, i18n.
- **The mobile evidence task** (ADR §2.2.1 step 3): A-web in the HRM Expo WebView on a real mid-range phone; the size of the 360 dp layout; a selection-handle design. Under B-6 there can be no "upstream answer to a layout-mode hook PR". **I replace it with a measured carry cost of the hook patch across two syncs.**
- Accessibility baseline (ADR §1.2 says unmeasured). Linux packages. macOS is UNSIZED (no Mac).

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 4.1 | Home launcher (desktop) and static web shell: new per type, templates, recent files, search | 3 | 4 | 6 | JUDGEMENT |
| 4.2 | Web lazy-load assembly, per-editor dist, CI budgets, brotli precompression, headers | 2 | 3 | 4 | MEASURED anchor: web builds 420–897 s cold each ([inventory]); the budget ratchet comes from Phase 1 |
| 4.3 | Toolbar design, done once for desktop and mobile; owner review | 1.5 | 2 | 3 | JUDGEMENT |
| 4.4 | Simplified toolbar + ribbon toggle, carried: wordcraft 3/4/6, gridcraft 2/3/4, deckcraft 2/3/4, pdfcraft 2/3/4 | 9 | 13 | 18 | JUDGEMENT. 500–1,500 new lines per app in hot files; anchors: the touch-scroll patch (+328, ≈190 non-test lines) took part of one spike ([wordcraft-patches]); PLAN WP7c–f sizes a one-token brand hook at 5 h per app |
| 4.5 | Shared services crate + per-app hooks (carried) | 2 | 3 | 4 | JUDGEMENT |
| 4.6 | Suite MSI (WiX), ProgIDs, GPO association XML, upgrade/uninstall tests | 3 | 4 | 6 | JUDGEMENT. Upstream WiX exists to copy from (`wordcraft/packaging/windows/wordcraft.wxs:79-98`) |
| 4.7 | MSVC build and code-signing pipeline | 1 | 2 | 3 | JUDGEMENT. Needs VS Build Tools here or a CI runner |
| 4.8 | Windows verification: GUI smoke ×4 on MSVC, WARP and RDP, Win11, a GPO install on a domain PC | 2 | 3 | 4 | JUDGEMENT. Needs owner hardware |
| 4.9 | Mobile evidence task (real phone in the Expo WebView, 360 dp size, handle design, hook carry cost) | 3 | 4 | 6 | JUDGEMENT. A0 + patches ran on the emulator in Chrome only ([mobile-a0], [wordcraft-patches]) |
| 4.10 | Accessibility baseline (Narrator/NVDA, keyboard-only, web canvas) | 1 | 1.5 | 2 | JUDGEMENT |
| 4.11 | Linux packages (macOS UNSIZED) | 1 | 2 | 3 | JUDGEMENT |
| 4.12 | "New user, no instructions" check with 2–3 office staff | 1 | 1.5 | 2 | JUDGEMENT |
| 4.13 | Gate, including the B-3 re-decision pack | 1 | 1.5 | 2 | JUDGEMENT |
| | **Subtotal** | **30.5** | **44.5** | **63** | |

- **Dependencies:** Phase 1; Phase 3 (the Converter page). The owner's hardware.
- **From the owner:**
  - Hardware (B-10): a Win11 PC or VM, a mid-range Android phone, a domain-joined PC for the GPO test.
  - **A code-signing certificate.** Procurement can take weeks: start it during Phase 2 (ADR §2.5).
  - Decisions B-11 (default-handler policy), B-12 (fleet facts), B-3 (mobile re-decision at the gate).
  - Toolbar design review and Uzbek label review: about 6–14 owner-hours (JUDGEMENT).
  - 2–3 staff for the usability check.
- **Risk buffer:** 12 stream-days. The toolbar carry is in the hottest files of four apps.
- **Carry added:** about 10–16 patches, 2–6k lines, mostly hot `ui-egui` files. **This is the second-largest carry after track E.**

### 2.4 Phase 5: Document server + HRM integration (split into 5a and 5b, see §6)

**Scope as it now stands**
- The spike's server, productionised:
  - a per-request session token, JWT on every call, DocsAPI-shaped config, the api.js alias at `/web-apps/apps/api/documents/api.js`;
  - callback statuses 1/2/4/6, configs without `callbackUrl`, a callback timeout above 10 s;
  - periodic autosave, async-save tickets, DNS/IP SSRF checks, size limits, audit log, TLS, a Docker image.
  
  The backlog is ADR §2.4, [hrm-integration-design] §5 and [hr-backend-prompt] §5.
- Permissions view/comment/review/edit and read-only mode; version history; compatibility notes.
- **HRM SSO and sync (owner requirement):** a desktop OAuth 2.1 + PKCE client (loopback, system browser, Windows Credential Manager); "Sign in with HRM", "Open from / Save to HRM", ETag/If-Match, leases, a conflict dialog, an offline queue. That UI is carried `ui-egui` work (ADR review M13).
- The pilot: HRB `files` docx behind PR-3's flag, on `vds-test` only, with ONLYOFFICE as the instant fallback.
- The `_hrm_*` bookmark gate on a real decree, letter and report. The `files` kind has no Word↔DB sync, so the gate is not needed for a `files` pilot.

**5a: Docs-only HRM pilot**

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 5.1 | Docserver productionisation (list above) | 4 | 6 | 8 | MEASURED anchor: spike S0–S9 is green (3/3; bridge 8/8 ×3) and took under about 1.5 stream-days in all ([docserver-results], [wordcraft-patches]); production ×3–5 |
| 5.4 | Security: SSRF DNS/IP checks, limits, audit, rate limits, TLS (rustls) on the target OS, Docker image, checklist tests | 2 | 3 | 4 | JUDGEMENT. The TLS stack is UNVERIFIED ([spike-plans] §4.3) |
| 5.7 | Compatibility notes (supported / partial / unsupported fields) | 0.5 | 0.5 | 1 | The matrix is already drafted ([hr-backend-prompt] §5) |
| 5.8 | Pilot on `vds-test`: container on `hr_network`, env, TLS/route, open → edit → save → reopen → download 3× green, flip back without a restart | 2 | 3 | 4 | JUDGEMENT. Owner presence is required for test-stack deploys ([hr-backend-prompt] §1) |
| 5.11 | uz-translit reachable inside the embedded editor (OPEN-Q1) | 0.5 | 0.5 | 1 | JUDGEMENT |
| 5a-g | Gate and security checklist | 1 | 1 | 1.5 | JUDGEMENT |
| | **5a subtotal** | **10** | **14** | **19.5** | |

**5b: the rest of Phase 5**

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 5.2 | Permissions → editor features (review = forced track changes, comment-only gating), carried | 2 | 3 | 5 | JUDGEMENT. The spike enforces `edit` only ([docserver-results]) |
| 5.3 | Version history UI on HRB's `/versions` (PR-4a, PR-7) | 1.5 | 2 | 3 | JUDGEMENT |
| 5.5 | Host bridges: adapt gridcraft's and pdfcraft's `?host=parent`; a new deckcraft bridge (carried) | 2 | 3 | 4 | MEASURED anchor: wordcraft 0001 (+213/−8) inside a ≈1-day spike ([wordcraft-patches]) |
| 5.6 | `_hrm_*` bookmark round trip on real HRM documents through the web editor, and a fix if any are lost | 1 | 2 | 4 | MEASURED: only `_hrm_desc_` tested, 4/4 synthetic files, CLI only ([fidelity-baseline]); the others are UNVERIFIED |
| 5.9 | Desktop OAuth client, sign-in, Open from / Save to HRM, sync with leases, conflict dialog and offline queue; Docs first, then Sheets and Slides (carried UI) | 5 | 7 | 10 | JUDGEMENT on the design in [hrm-integration-design] §1.5, §2.2, §5 |
| 5.10 | Web BFF for the `craft-web` client (optional) | 2 | 3 | 4 | JUDGEMENT |
| 5b-g | Gate and security checklist | 1 | 1.5 | 2 | JUDGEMENT |
| | **5b subtotal** | **14.5** | **21.5** | **32** | |
| | **Phase 5 total (our side)** | **24.5** | **35.5** | **51.5** | |

**HRM side (separate stream; nothing below is sized in Phase 0)**

| PR | Low | Likely | High | Basis |
|---|---|---|---|---|
| PR-0 baseline | 0.25 | 0.25 | 0.5 | JUDGEMENT on the fully specified steps and tests in [hr-backend-prompt] §4 |
| PR-1 callback robustness, SSRF | 1 | 1.5 | 2 | JUDGEMENT |
| PR-2 secrets | 1 | 1 | 1.5 | JUDGEMENT |
| PR-3 Craft provider behind a flag | 1.5 | 2 | 3 | JUDGEMENT |
| PR-4a versioned store (files) | 2 | 2.5 | 3 | JUDGEMENT |
| PR-4b versioned store (orders, letters; every decree writer) | 2 | 3 | 4 | JUDGEMENT |
| PR-5 session registry, leases | 1 | 1.5 | 2 | JUDGEMENT |
| PR-6 OAuth 2.1 server | 3 | 4 | 5 | JUDGEMENT |
| PR-7 `/office/*` sync API | 3 | 4 | 5 | JUDGEMENT |
| PR-8 hardening, docs | 0.5 | 0.75 | 1 | JUDGEMENT |
| hr-frontend (v2, v1): `editor=auto`, `editorUrl`, `/oauth/consent` | 1 | 1.5 | 2 | JUDGEMENT |
| hrm-mobile: three WebView screens | 0.5 | 0.75 | 1 | JUDGEMENT |
| **HRM side total** | **16.75** | **22.75** | **30** | **Pilot subset (PR-0–PR-3 + `editor=auto`): 4.75 / 6.25 / 9** |

- **Dependencies:**
  - 5a: Phase 2; Phase 1 branding (no ArtCraft chrome in the embed); **PR-0 to PR-3 merged and deployed on `vds-test`**; the owner's secret actions.
  - 5b: Phase 4 (the desktop apps carry the sign-in UI); PR-4a to PR-8.
- **From the owner:**
[redacted: HRM security detail]
  - Answer OPEN-Q1 to Q8 ([hr-backend-prompt] §8). Choose the pilot flow (BRIEF App. B, Phase 5) and the pilot users.
  - Be present for test-stack deploys. Provide test hostnames, DNS and TLS.
  - Approve the client changes in hr-frontend and hrm-mobile.
  - Provide one HRM-generated decree, letter and report for 5.6.
  - About 6–12 owner-hours, spread out (JUDGEMENT).
  - **Start the hr-backend agent on PR-0 to PR-3 now** (ADR §3.1 item 3). If that slips, it becomes the pilot's critical path.
- **Risk buffer:** 5a 3 stream-days, 5b 7 stream-days.
- **Carry added:** 5a about 0–2 patches (bridge fixes); 5b about 8–12 patches (permissions, deckcraft bridge, sign-in and sync UI in 3 apps).

### 2.5 Phase 6: Mobile (A-web, conditional)

**Scope as it now stands**
- Owner requirement: editing on mobile, web and desktop, rejected if the Android touch and keyboard tests fail ([STATUS], D1).
- On Android: open from the device, the share sheet and HRM links; the touch checklist with recordings (handles, long-press menu, autocorrect, swipe typing, Cyrillic, pinch, momentum, rotation, 360 px with no overflow); a compact contextual bottom toolbar; performance on a mid-range phone; the HRM Expo embed; mobile sign-in (`craft-mobile`, app links, Keystore).
- **Today no option passes D1** (ADR §2.2.1).
- What exists: wordcraft patches 0001–0005 (+651/−18, MEASURED on the emulator, [wordcraft-patches]).
- What is missing: the compact layout (ribbon fails at 360 dp), selection handles (egui core has none), caret above the keyboard, the keyboard reappearing after a swipe, finger-drag of objects.
- Sheets, Slides and PDF were never tested on mobile.
- iOS needs a Mac (B-10).

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 6.1 | Productionise wordcraft 0001–0005; caret above the keyboard; keyboard after swipe; finger-drag hit-test hook | 2 | 3 | 5 | MEASURED anchor: the 5 patches came from one spike; remaining limits are listed in [wordcraft-patches] |
| 6.2 | **Docs compact layout at 360 dp + contextual bottom toolbar (carried)** | 5 | 8 | 12 | **UNSIZED band** (ADR §2.2.1). JUDGEMENT until 4.9 measures it |
| 6.3 | **egui-drawn selection handles + long-press menu (Docs)** | 3 | 5 | 8 | **UNSIZED band**. JUDGEMENT |
| 6.4 | Sheets mobile: A0 matrix, touch selection and handles on the grid, in-cell editing with the soft keyboard, formula bar, compact toolbar | 6 | 9 | 15 | JUDGEMENT. Grid, hit-testing and editor live only in `ui-egui` ([layer-map-gridcraft] §4) |
| 6.5 | Slides mobile | 5 | 7 | 12 | JUDGEMENT. deckcraft's engine owns keys and IME, which helps ([layer-map-deckcraft] §4) |
| 6.6 | PDF mobile (viewing, annotation, forms) | 4 | 6 | 10 | JUDGEMENT |
| 6.7 | Android wrapper written from scratch (the agdk scaffold has no licence, ADR §5.2), share sheet, file open, HRM app links; or a PWA | 3 | 4 | 6 | MEASURED anchor: a 40-line WebView APK built in the B spike ([mobile-b] B5) |
| 6.8 | Mobile sign-in (`craft-mobile`, app links, Keystore), reusing 5.9's sync | 1.5 | 2 | 3 | JUDGEMENT |
| 6.9 | HRM Expo `react-native-webview` embed: test and fix | 1 | 2 | 3 | JUDGEMENT. Never tested ([hrm-current-state] §4.4) |
| 6.10 | Real-device performance (start-up, memory, scroll FPS) and fixes | 1 | 2 | 4 | JUDGEMENT. Emulator numbers only: PSS 284–326 MB ([mobile-a0]) |
| 6.11 | Checklist recordings per app | 1 | 1.5 | 2 | JUDGEMENT |
| 6.12 | Gate | 1 | 1.5 | 2 | JUDGEMENT |
| | **Subtotal (Android, 4 apps)** | **33.5** | **51** | **82** | |
| — | iOS (WKWebView wrapper + checklist), only with a Mac | 4 | 6 | 10 | JUDGEMENT. UNVERIFIED end to end |
| — | **Approach B instead, if chosen at the Phase 4 gate (Docs only)** | 51 | 113 | 231 | P0-ESTIMATE 7–11 engineer-months ([mobile-b]) = 147–231 days at 21 a month, converted ×0.35/0.6/1.0. **Sheets is blocked:** there is no egui-free grid renderer ([layer-map-gridcraft]) |

- **Dependencies:** the B-3 re-decision at the Phase 4 gate (4.9 evidence); 5b (sign-in, sync); a real phone.
- **From the owner:** a mid-range Android phone (needed already at Phase 4); a Mac or cloud Mac, or an explicit iOS deferral (B-10); the Android package name and app-link domain (ADR §5.1); the HRM mobile team's change; about 4–8 hours of device testing or review (JUDGEMENT).
- **Risk buffer:** 20 stream-days. That is the largest relative buffer, because 6.2 and 6.3 are unsized.
- **Carry added:** about 15–25 patches across four `ui-egui` crates, in the hottest files. With 4.4, **this could be the largest carry of the project** (ADR §4.3).

### 2.6 Phase 7: Plugin system

**Scope as it now stands:** WASM plugins (wasmtime natively; the browser runtime on web and therefore on A-web mobile); a manifest with permissions and UI contributions; an API over the command catalogs (629 / ≈340 / 320 / ≈200 + 229 commands, [STATUS] task 2); versioned docs; a sandbox; uz-translit ported with the same behaviour and speed; a second sample plugin (an HRM template field); admin control on the docserver; a malicious-plugin test. **Phase 0 has no evidence for any of it.**

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 7.1 | API and manifest design, versioning | 2 | 3 | 4 | JUDGEMENT |
| 7.2 | Native host (wasmtime; fuel/epoch and memory limits) | 3 | 4 | 6 | JUDGEMENT |
| 7.3 | Web host (Worker + postMessage bridge into the egui wasm app) | 3 | 4 | 6 | JUDGEMENT |
| 7.4 | Integration ×4 apps: commands, selection and events, toolbar/menu/panel contributions (carried `ui-egui` and engine hooks) | 6 | 9 | 14 | JUDGEMENT |
| 7.5 | Sandbox and permissions; malicious-plugin tests on both runtimes | 2 | 3 | 4 | JUDGEMENT |
| 7.6 | uz-translit as a plugin at the same speed (likely needs a bulk run-level API) | 2 | 3 | 5 | JUDGEMENT |
| 7.7 | Second sample plugin | 1 | 1.5 | 2 | JUDGEMENT |
| 7.8 | Admin control per organisation on the docserver; distribution | 2 | 3 | 4 | JUDGEMENT |
| 7.9 | Mobile verification | 1 | 1.5 | 2 | JUDGEMENT |
| 7.10 | `docs/plugins.md`, SDK template | 1 | 1.5 | 2 | JUDGEMENT |
| 7.11 | Gate and sandbox security review | 1 | 1.5 | 2 | JUDGEMENT |
| | **Subtotal** | **24** | **35** | **51** | |

- **Dependencies:** Phase 4 (a stable host); Phase 5 (admin control); Phase 6 (the mobile runtime).
- **From the owner:** who may install plugins (BRIEF App. B, Phase 7); about 2 h of review.
- **Risk buffer:** 10 stream-days.
- **Carry added:** about 8–12 patches (four apps' hooks).

### 2.7 Phase 8: Fidelity hardening (continuous from Phase 1)

**Scope as it now stands**
- The harness exists (`spikes/fidelity/run.py`). The corpus has never been run, and `corpus/` is still missing (checked 2026-10-17).
- Owner requirements: Office 2010+ files including legacy formats; no silent loss (warn); Excel files that move between Excel and our users during the cutover (risk 11).
- Known top losses on the synthetic set ([fidelity-baseline]):
  - gridcraft PDF fonts (patch ready);
  - Word layout, 5–11% pixel diff;
  - .doc comments and revisions lost;
  - .xls charts and conditional formats lost;
  - .ppt does not open.
- Also: locale number and date display (PLAN WP10 measures it).

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 8.1 | Harness in nightly CI, trend per gate, interactive Excel repair-prompt detection, public sample sets | 3 | 4 | 6 | MEASURED anchor: the harness took ≈2 h to build in Phase 0 (§1.2) |
| 8.2 | No-silent-loss detection and warnings ×3 apps (charts, SmartArt, OLE, macros, .doc/.xls downgrade) (carried) | 3 | 4 | 6 | JUDGEMENT |
| 8.3 | Keep charts, SmartArt and OLE as opaque parts in docx/pptx where the model allows (carried engine) | 5 | 9 | 15 | JUDGEMENT. It may be infeasible in places |
| 8.4 | gridcraft PDF fonts to production (FlateDecode first, fmt/clippy, tests) | 1 | 1.5 | 3 | MEASURED anchor: the spike, +1,503 lines in ≈2 h ([gridcraft-pdf-fonts]) |
| 8.5 | Word layout offset (≈10 px body shift; font and line metrics) | 2 | 4 | 8 | JUDGEMENT. Cause not isolated |
| 8.6 | .doc comments and revisions; .xls charts and conditional formats | 5 | 8 | 13 | JUDGEMENT |
| 8.7 | Locale numbers, dates and entry (re-size after WP10) | 2 | 3 | 6 | JUDGEMENT. gridcraft hard-codes `.` and `,` (PLAN F10) |
| 8.8 | **Corpus-driven top-N fixes: a budget, not an estimate** | 10 | 18 | 30 | JUDGEMENT. Unknown until the corpus runs |
| | **Subtotal** | **31** | **51.5** | **87** | |

- **Dependencies:** the corpus (B-14). LibreOffice/x2t references (WP11).
- **From the owner:** the corpus, with files saved by Office 2010, 2013, 2016/2019 and 365 and real linked workbooks; a ranking of loss categories by impact; about 2 h per gate to read the loss trend.
- **Risk buffer:** 15 stream-days.
- **Carry added:** about 10–20 small patches in importers and layout, mostly cold files.

### 2.8 Phase 9: Real-time co-editing

**Scope as it now stands:** a CRDT vs OT spike; presence, cursors, merging and offline merge in the docserver; comments and review under co-editing; the exit test with 3 users on web and mobile for 10 minutes until all copies converge. The engines have command catalogs and journals (gridcraft records `(id, params)`, [layer-map-gridcraft] §2), but no operation model that can be merged. Co-editing in a server means linking an engine there (ADR §2.1.2). HRM does not co-edit today (ADR §2.4), and B-4 ranks co-editing last.

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 9.1 | Spike: CRDT (yrs/automerge) vs OT on commands, for text, formatting and tables; conflicts, bandwidth, latency | 5 | 7 | 10 | JUDGEMENT |
| 9.2 | Engine change: operation log, position mapping, deterministic apply (carried, wordcraft core) | 15 | 25 | 40 | JUDGEMENT. The largest single item in this document |
| 9.3 | Server: sessions, WebSocket broadcast, persistence, presence, offline merge | 8 | 11 | 15 | JUDGEMENT |
| 9.4 | Remote cursors and presence in `ui-egui` (carried) | 3 | 4 | 6 | JUDGEMENT |
| 9.5 | Comments and review under co-editing | 3 | 5 | 8 | JUDGEMENT |
| 9.6 | Convergence harness, soak | 3 | 4 | 6 | JUDGEMENT |
| 9.7 | Gate | 1 | 1.5 | 2 | JUDGEMENT |
| | **Subtotal (Docs only)** | **38** | **57.5** | **87** | Sheets and Slides co-editing: **UNSIZED** |

- **Dependencies:** Phase 5 (docserver) and Phase 6 (mobile users in the exit test).
- **From the owner:** accept Docs-only co-editing for v1.0, or fund Sheets and Slides later.
- **Risk buffer:** 20 stream-days.
- **Carry added:** a deep series in wordcraft's engine. It may be impractical to carry at all; see §6 item 5.

### 2.9 Phase 10: Release and operations

| # | Item | Low | Likely | High | Basis |
|---|---|---|---|---|---|
| 10.1 | Signed desktop installers (most work is done in Phase 4) | 2 | 3 | 4 | JUDGEMENT |
| 10.2 | Signed Android APK/AAB (iOS +2/3/4 if there is a Mac) | 2 | 3 | 4 | JUDGEMENT |
| 10.3 | Docker images: docserver, converter, PDF sidecar, LibreOffice sidecar; compose; health checks | 2 | 3 | 4 | JUDGEMENT |
| 10.4 | Static web assets with headers (wasm MIME, brotli, caching, CSP) | 1 | 1.5 | 2 | JUDGEMENT |
| 10.5 | Ops documentation: install, upgrade, backup, monitoring | 2 | 3 | 4 | JUDGEMENT |
| 10.6 | User guide in Uzbek with screenshots | 3 | 4 | 6 | JUDGEMENT. Owner review is separate |
| 10.7 | Whole-system security review and fixes | 3 | 5 | 8 | JUDGEMENT. The fix budget is unknown |
| 10.8 | Release notes, versioning policy, third-party notice bundle | 1 | 1.5 | 2 | JUDGEMENT |
| 10.9 | Fresh install on a clean server from the docs | 1 | 1.5 | 2 | JUDGEMENT |
| 10.10 | Gate | 1 | 1 | 1.5 | JUDGEMENT |
| | **Subtotal** | **18** | **26.5** | **37.5** | |

- **From the owner:** release channels and signing identities (BRIEF App. B, Phase 10); a clean test server; review of the Uzbek user guide, about 6–12 h (JUDGEMENT).
- **Risk buffer:** 6 stream-days.

### 2.10 Track E: Excel engine (part of the "fidelity" priority, B-4)

**Scope as it now stands:** the owner's workload is heavy formulas, many sheets, links to other workbooks and files of 80 MB and more that keep growing ([STATUS]).
- **Measured today (round 2 patches):** w80 opens with the engine below Excel's peak (817 vs 851 MB); the desktop app is still +14% to +46% above Excel because of a fixed 226 MB start-up cost.
- **Not built or measured:**
  - links (never refreshed; a typed link gives `#REF!`);
  - interactive editing at w80;
  - Excel's repair prompt on our w40–w120 output.

  Sources: [gridcraft-memory-r2], [xlsx-bench], ADR §2.3.
- F9 takes 10 s against Excel's 1.3 s on w80, MEASURED ([gridcraft-memory-r2] §6). Multi-threaded recalc is UNSIZED and not included.

| # | Item | P0-ESTIMATE (human-scale, d) | Low | Likely | High | Source |
|---|---|---|---|---|---|---|
| E0 | Census of real workbooks; raise `MAX_TOTAL` | 1–1.5 | 0.35 | 0.75 | 1.5 | ADR §2.3 |
| E1 | Round 1 (templates + run graph) to production | 5–9 | 1.75 | 4.2 | 9 | [gridcraft-templates-spike] §8 |
| E2 | Round 2 to production (incl. 1 d on real owner files) | 5–8 | 1.75 | 3.9 | 8 | [gridcraft-memory-r2] §7 |
| E3 | Close the GUI baseline gap (fonts on demand, renderer, allocator) | 3–4.5 (UNVERIFIED) | 1.05 | 2.25 | 4.5 | [gridcraft-memory-r2] §7 |
| E4 | Streaming writer + bounded undo | 3.5–6.5 | 1.2 | 3 | 6.5 | [gridcraft-memory-r2] §7 |
| E5 | Streaming zip (the 1 GiB wall at ≈150 MB on disk) | 3–5 | 1.05 | 2.4 | 5 | ADR §2.3 |
| E6 | **External links** (syntax, model, I/O, calc, source loading, commands, consent, verification) | **18–27** | 6.3 | 13.5 | 27 | [gridcraft-scaling-and-links] §3.2 (sum of rows) |
| E7 | GUI editing session on w80 (peak RAM, recalc latency per edit); interactive Excel check of w40–w120 and census files | — | 1.5 | 2 | 3 | JUDGEMENT (the ADR's track E gate) |
| E8 | Fixes that E7 finds (Excel compatibility, editing memory) | — | 0 | 2 | 5 | JUDGEMENT, a budget |
| | **Subtotal** | **38.5–61 + E7/E8** | **15** | **34** | **69.5** | Conversion per §1.3 |

- **Dependencies:** Phase 1 (queue, `xlsx-bench` in CI). Real workbooks for the census. Excel 16 on this PC (present). A machine that can soak: some fuzz seeds exceed 2.5 GB.
- **From the owner:** B-7 (fund E, and keep Excel per file until it lands); access to real large and linked workbooks (B-14); fleet RAM (B-12); macro use. Optionally, a bigger build or soak machine.
- **Risk buffer:** 15 stream-days.
- **Carry added:** E1–E6 are about 6–10k lines in gridcraft's hottest engine files (`cell.rs`, `recalc.rs`, lexer/parser). With B-6 standing, **this is the most expensive carry we take on** (ADR §4.3).

---

## 3. The ongoing carried-patch sync cost

**Status: JUDGEMENT model. Replace its inputs with WP5's measurement** (`docs/phase1/rebase-cost.md`, "minutes per 100 upstream commits"), **and again after WP14.** Two data points are not a trend (PLAN R17).

**Why it is real.** B-6 sends no PRs, so nothing ever leaves the queue. Upstream moves 701 / 993 / 859 / 823 commits a week (MEASURED, [STATUS]), and 96–98% of each repo's history is from the last week. So the hot files we patch change many times a week.

**Model (per weekly sync).** Weekly cost = F + Σ over hot-file patches (p_h × t_h) + Σ over cold patches (p_c × t_c).

| Input | Low | Likely | High | Basis |
|---|---|---|---|---|
| F: fixed agent time per sync (fetch, signals review, regenerate, register, quarantine triage) | 0.25 sd | 0.35 sd | 0.5 sd | JUDGEMENT. The machine-only gates (quick ≤30 min, full ≤4 h warm; PLAN §6.1) run at night and occupy the build lock, not stream-days |
| p_h: chance a hot-file patch conflicts in a week | 0.4 | 0.6 | 0.8 | JUDGEMENT on the velocity above |
| t_h: agent-hours to resolve and re-verify one hot conflict | 0.25 | 0.5 | 1.5 | JUDGEMENT. Consistent with PLAN WP5's own estimate: 4–20 h for gridcraft's 22 patches over 4–7 days of upstream |
| p_c × t_c for cold patches (new files, one-token hooks) | 0.05 × 0.1 h | 0.1 × 0.2 h | 0.2 × 0.3 h | JUDGEMENT |

**Queue growth and weekly cost** (PROJECTION from the model, 9 agent-hours per stream-day):

| Point in the plan | Patches (hot) | Carried lines | Weekly cost, low / likely / high (sd) | Share of the 5 sd/week capacity |
|---|---|---|---|---|
| Phase 1 gate | ≈35 (≈15) | ≈7–9k (the 27 spike patches are ≈+5.2k/−0.4k MEASURED; brand, i18n and font ≈1.5–3k) | 0.43 / 0.9 / 2.6 | 9% / 18% / 53% |
| After 2, 5a, 3 | ≈45 (≈18) | ≈9–12k | 0.47 / 1.0 / 3.1 | 9% / 20% / 62% |
| After E, 4, 5b | ≈75 (≈35) | ≈18–28k | 0.66 / 1.6 / 5.4 | 13% / 32% / >100% |
| After 6, 7 | ≈110 (≈55) | ≈28–42k | 0.89 / 2.3 / 8.2 | 18% / 46% / >100% |
| After 9 | ≈130 (≈65) | ≈35–55k | 1.0 / 2.7 / 9.6 | 20% / 54% / >100% |

**What follows**
1. At the high end, from track E and Phase 4 onward, **weekly sync costs more than the whole stream**. The plan cannot survive that unchanged. The levers, in order of effect:
   - (a) **Revisit B-6** for the two big series: track E and a `ui-egui` layout-mode hook. Accepted upstream, they stop costing anything.
   - (b) **Change the cadence** from weekly to a pin per phase gate (every 4–8 weeks), plus security syncs. Whether a 4-week gap costs less than 4 weekly syncs is **unmeasured**: WP5 and WP14 both cross about one week. I recommend adding a third dry run across a 2–3-week gap in WP14 or right after the Phase 1 gate. A dry run is cheap; it runs in scratch.
   - (c) **Design for low carry**: new files, one-token hooks, our own crates (PLAN §3.2), and review every patch's "drop when" condition.
2. **The roll-up in §4 uses the "likely" column per period, and caps the pessimistic case at 2.4–3.5 sd/week.** That cap assumes lever (b) or (a) is pulled. Uncapped, the pessimistic case does not converge.
3. **Machine time is a second, hidden tax.** Each sync's full gates take up to 4 h of the build lock, and the nightly up to 8 h (PLAN §6.1), on a PC that is already RAM-bound. A second build machine reduces the calendar risk (owner hardware, §7).

---

## 4. Calendar roll-up

**Assumptions**
- One stream, 5 stream-days a week, starting the day after the owner says "go" at the Phase 1 gate.
- Phase 1 gate: **Fri 2026-11-13** likely (PLAN D20), Tue 2026-11-10 optimistic (17 days), Tue 2026-11-24 pessimistic (27 days).
- Order as recommended in §6: 2 → 5a → 3 → E → 4 → 5b → 6 → 7 → 9 → 10, with Phase 8 spread across everything after 5a in proportion to work.
- **Carry tax per period** (§3 table): optimistic 0.43 → 1.0; likely 1.0 → 1.6 → 2.3 → 2.7; pessimistic capped at 2.6 → 3.0 → 3.5 sd/week.
- **Owner latency per gate** (10 gates): optimistic 1 working day (the one MEASURED data point, Phase 0), likely 3, pessimistic 10. Plus blocking waits that are not overlapped (list approvals, hardware and certificate lead times): 0 / 4 / 12 weeks.
- One New Year week off. Other public holidays are not modelled.
- The HRM side runs in its own stream and is off our critical path only if PR-0 to PR-3 start now.

| Scenario | Work (sd) | Calendar weeks (work ÷ capacity after tax) | + gate latency | + blocking waits | **Total** | **Phases 2–10 + E done** |
|---|---|---|---|---|---|---|
| Optimistic | 240.5 | ≈57 | ≈2 | 0 | **≈59 weeks** | **≈ Jan 2028** |
| Likely | 372.25 | ≈129 | ≈6 | ≈4 | **≈139 weeks** | **≈ Jul 2029** |
| Plan-to (likely + 117 sd buffer) | 489.25 | ≈172 | ≈6 | ≈4 | ≈182 weeks | ≈ May 2030 |
| Pessimistic | 581 | ≈371 | ≈20 | ≈12 | **≈403 weeks** | **≈ 2034. A signal, not a schedule:** at this tax and size, one stream without PRs does not finish, and §3's levers must be pulled first |

**Sensitivity (PROJECTION)**
- **Carry tax:** each ±0.5 sd/week of average tax moves the likely total by roughly ±15–20 weeks.
- **A second stream** (a second agent stream and a second build PC) does not halve the calendar, because gates, owner reviews and the serial phase order stay. JUDGEMENT: −30 to −40%.
- **Scope cuts with the largest effect:** co-editing (−57.5 sd likely), plugins (−35), Sheets/Slides/PDF on mobile (−22 of Phase 6's 51).
- **Calibration:** if Phase 1 takes N working days against 20, scale the JUDGEMENT items by about N/20 (§1.3).

**Likely milestones** (coarse, month granularity; PROJECTION)

| Milestone | Likely date |
|---|---|
| Phase 2 gate (uz-translit, npm package) | mid-Dec 2026 |
| **5a: Docs-only HRM pilot on `vds-test`** | **late Jan 2027** |
| Phase 3 gate (Converter) | Mar 2027 |
| Track E done (80 MB+ editing, links) | Jun 2027 |
| Phase 4 gate (suite MSI, simplified toolbar, B-3 re-decision): **first Windows office-PC pilot possible** | Sep–Oct 2027 |
| 5b gate (HRM sign-in and sync on desktop) | Nov 2027 |
| Phase 6 gate (Android, 4 apps) | Apr–May 2028 |
| Phase 7 gate | Aug 2028 |
| Phase 9 gate (Docs co-editing) | Mar 2029 |
| Phase 10 gate (v1.0), including ≈4 weeks of blocking waits | Jun–Jul 2029 |

---

## 5. Critical path to a first useful pilot

**Recommended first pilot: Docs-only HRM embedding with uz-translit.**
- **The flow:** HRB `files` docx; pilot users on `vds-test`; behind PR-3's flag (`OFFICE_CRAFT_FILE_TYPES=docx`); ONLYOFFICE as the instant fallback.
- **Why this one:**
  - It is the business driver (BRIEF §7).
  - Its path is already MEASURED end to end on the spike: 3/3 green with the bridge, a 200–290 ms save → callback, green in Chrome on the Android emulator ([wordcraft-patches], [mobile-a0]).
  - The fallback makes it low risk for users.
  - The `files` kind has no Word↔DB bookmark sync, so the unmeasured `_hrm_*` gate is not on its path.
  - uz-translit keeps the one feature pilot users would otherwise lose (OPEN-Q1).
- **Honest limit:** this is a pilot on the **test** environment (BRIEF §5). A production pilot needs PR-1/PR-2 on the main server and the owner's go. That is outside Phases 2–5 as written.

**Critical path:** Phase 1 gate → Phase 2 (15.75 sd likely) → owner go → 5a (14 sd likely) → pilot. **In parallel and off the path only if started now:** hr-backend PR-0 to PR-3 (6.25 sd likely in the HRM stream), the `JWT_SECRET` rotation, test DNS and TLS, and the hr-frontend `editor=auto` change.

| | Optimistic | Likely | Pessimistic |
|---|---|---|---|
| Phase 1 gate + go | Nov 10 + 1 d | Nov 13 + 3 d | Nov 24 + 10 d |
| Phase 2 | 11 sd at 4.55/week → ≈Nov 27 | 15.75 sd at 4.0/week → ≈Dec 16 | 23.5 sd at 2.4/week + 2 weeks waiting for list approval + holiday → ≈Mar 3, 2027 |
| 5a | 10 sd → ≈Dec 15, 2026 | 14 sd + New Year week → **≈Jan 20, 2027** | 19.5 sd → ≈mid-May 2027 |
| **Pilot running on `vds-test`** | **mid-Dec 2026** | **late Jan 2027** | **mid-May 2027** |

**Earliest realistic date: late January 2027.** Mid-December 2026 is possible only if every range lands low, the owner approves the golden list within days, and the HRM side is deployed by early December.

**Without the 5a split** (the ADR order 2 → 3 → 4 → 5), the same pilot is likely in **mid-June 2027**, or **late August 2027** if track E is interleaved before it as the ADR's "parallel" track.

**"Pilot zero", nearly free:** the uz-translit npm package at the Phase 2 gate (mid-Dec 2026 likely), if the HRM web team wants transliteration in its own forms.

**Not recommended as the first pilot: a Windows desktop Docs pilot on office PCs.** It needs the MSI, a code-signing certificate and Word fidelity on the real corpus, and all three are unmeasured or unprocured today. Its earliest realistic point is the Phase 4 gate (≈Sep–Oct 2027 likely).

---

## 6. Where the ADR's phase order should change

| # | Change | Reason | Who decides |
|---|---|---|---|
| 1 | **Split Phase 5. Put 5a (the Docs-only HRM pilot) right after Phase 2**, before Phases 3 and 4. 5b (permissions, versions UI, desktop sign-in and sync, other bridges) stays after Phase 4. | The pilot uses one per-editor wasm and the existing bridge. It needs none of Phase 4: no shell, MSI, toolbar or lazy loading. It moves the first pilot from about mid-June 2027 to about late January 2027 (§5). **This deviates from BRIEF §7** ("5 depends on 4"; "never merge two phases"). Splitting a phase is not merging one, but the owner must approve it. | Owner |
| 2 | **Schedule track E as a block between Phases 3 and 4**, not as a "parallel track". Run E0 (census) as soon as the corpus arrives. | With one stream, "parallel" means it competes for the same days. Unmerged E patches pay the weekly carry for every week they sit in the queue, so land them in one concentrated block. Phase 4's MSI pilot on office PCs needs E for the Excel users. | Owner (with B-7) |
| 3 | **Phase 4's mobile evidence task:** replace "upstream's answer to a layout-mode hook PR" (impossible under B-6) with **the measured carry cost of that hook across two syncs**. That measurement decides B-3 at the Phase 4 gate. | Without it, the B-3 re-decision has no input on the D5 condition (ADR §2.2.1). | Architect (plan change), owner at the gate |
| 4 | **Sync cadence: weekly → a pin per phase gate plus security syncs**, if a dry run across a 2–3-week gap shows that cost grows more slowly than the gap. **Revisit B-6 before track E and Phase 4**, at least for those two series. | §3: at the high end, weekly sync exceeds the stream's capacity from track E onward. | Owner (ADR §4.2, B-6) |
| 5 | **Treat Phase 7 (plugins) and Sheets/Slides co-editing as the first scope cuts** if the schedule slips. Do not reorder them now. | B-4 does not rank plugins. uz-translit is built in by then. Phase 9.2 may be impractical to carry at all. | Owner |
| 6 | **Start owner-side lead times now:** hr-backend PR-0 to PR-3 (ADR §3.1 item 3), the code-signing certificate, a mid-range Android phone, a Win11 PC or VM, a Mac decision, and a second build or soak PC. | Each one becomes a critical-path wait if requested at the phase that needs it (the §4 blocking waits). | Owner |

Unchanged: Phase 8 runs continuously from Phase 1 (ADR §3.1 item 1); Phase 2 precedes the pilot; co-editing comes last.

---

## 7. Top schedule risks

| # | Risk | Effect on the schedule | Evidence | Mitigation |
|---|---|---|---|---|
| 1 | **The carried-patch tax** (B-6, weekly sync) | 0.4–2.6 sd/week now, 1–10 sd/week by v1.0, against 5 of capacity. Likely total ±15–20 weeks per 0.5 sd/week | Velocity MEASURED ([STATUS]); cost UNMEASURED until WP5 | WP5, WP14, a 2–3-week-gap dry run; §3 levers (B-6 revisit, cadence, low-carry design) |
| 2 | **Mobile UI cost is unsized** (compact layout, handles ×4 apps in hot `ui-egui`), with the B fallback at 51–231 sd for Docs only and Sheets blocked | Phase 6 swings between 33.5 and 82 sd on A-web, or more than 230 sd on B | ADR §2.2.1, [mobile-b], [layer-map-gridcraft] | The Phase 4 evidence task (§6 item 3); a phone in hand by Phase 4 |
| 3 | **Owner-side and HRM-side inputs**: golden list and Uzbek reviews, the corpus (Phase 8's 10–30 sd is a placeholder budget), HRM security PRs and secret rotation (pilot blocker), hardware and certificate lead times | 0–12 weeks of blocking waits in §4; the pilot slips one-for-one with HRM PR-0 to PR-3 | `corpus/` and `vendor-notes/` still missing (checked 2026-10-17); B-7…B-14 open | Request everything in §6 item 6 now; batch the reviews (PLAN §4.2) |

Also tracked:
- **Track E's size:** 15–69.5 sd; Excel interop at scale (risk 11).
- **Estimation calibration:** the JUDGEMENT numbers rest on prototype speed, not production speed (§1.3).
- **The single 11.8 GB build PC:** machine time and soak capacity.
- **Co-editing engine work** that may be impractical to carry.

---

## 8. What updates this document

| When | What changes |
|---|---|
| After WP3 (Phase 1 days 2–4) | Domain build times feed the machine-time model |
| **After WP5** (Phase 1 day 8) | **§3 inputs (p_h, t_h, F) from `rebase-cost.md`; then §4's tax and dates** |
| After WP10 and WP11 (if the corpus arrives) | Item 8.7 (locale); 8.8 (corpus budget); E0 (census); Phase 3's fidelity table |
| After WP14 | The second rebase point (with the brand, i18n and font patches); and, if added, the 2–3-week-gap dry run |
| Phase 1 gate | Recalibrate the JUDGEMENT items by Phase 1's actual duration against 20 days (§1.3); re-issue §0 and §4 |
| Each later gate | Replace the finished phase's estimate with its actuals, and re-scale the rest |

## Sources

[BRIEF]: ../BRIEF.md
[STATUS]: ../STATUS.md
[inventory]: ../phase0/inventory.md
[spike-plans]: ../phase0/spike-plans.md
[docserver-results]: ../phase0/docserver-results.md
[wordcraft-patches]: ../phase0/wordcraft-patches.md
[mobile-a0]: ../phase0/mobile-a0.md
[mobile-a-kill]: ../phase0/mobile-a-kill.md
[mobile-b]: ../phase0/mobile-b.md
[xlsx-bench]: ../phase0/xlsx-bench.md
[gridcraft-scaling-and-links]: ../phase0/gridcraft-scaling-and-links.md
[gridcraft-templates-spike]: ../phase0/gridcraft-templates-spike.md
[gridcraft-memory-r2]: ../phase0/gridcraft-memory-r2.md
[gridcraft-pdf-fonts]: ../phase0/gridcraft-pdf-fonts.md
[fidelity-baseline]: ../phase0/fidelity-baseline.md
[layer-map-gridcraft]: ../phase0/layer-map-gridcraft.md
[layer-map-deckcraft]: ../phase0/layer-map-deckcraft.md
[hrm-current-state]: ../phase0/hrm-current-state.md
[hrm-integration-design]: ../phase0/hrm-integration-design.md
[hr-backend-prompt]: ../phase0/hr-backend-prompt.md

ADR = `docs/ARCHITECTURE-DECISION.md`; PLAN = `docs/phase1/PLAN.md`. Timestamps in §1.2 are file modification times and
`git format-patch` dates read on 2026-10-17 from `docs/phase0/`, `spikes/*/` and `spikes/*-patches/`.
