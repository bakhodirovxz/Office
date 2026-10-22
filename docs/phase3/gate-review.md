# Phase 3 gate review (WP13): adversarial review of the converter

Date 2026-10-22, branch `phase-3`, HEAD `d67ed47`. The review started at `21cade8`. The builder's flaky-test fixes landed
during the review (`15f542b`, `a119148`, `d67ed47`), before any services job of the `ci.sh full` run below started, so
that run tested them. Architect tier (opus), with two read-only sub-agents: a security review of the new code and the
sync dry run. No product code was changed.

Gate items (BRIEF Phase 3, PLAN 9.3): (1) every supported direction works from CLI, API and UI; (2) the fidelity table
exists; (3) failures are reported to the user and never fail silently.

**Close-out (2026-10-22, section 10): READY for the owner's "go".** B1 and B2 are fixed, as are M1 and M3-M5. M2 is
the Phase 4 sync. `ci.sh full` is 28 of 28 ok at `d7b9582`, and the ratchet shows 0 new silent losses.

Original verdict at `d67ed47`: **NOT READY for "go" yet.** Two findings block the gate (B1: the PDF operations are CLI-only; B2: a silent date swap
in CSV input). B1 needs an owner decision or 1-2 days of work; B2 is about half a day. `ci.sh full`: exit 0, 28 of 28 jobs (section 5);
the silent-loss ratchet: 0 new (section 5). Licences: signed off
with conditions (section 3). Sync: the patch queue does not rebase cleanly on today's upstream (13 conflicts), but every CLI
flag and command id the converter uses still exists (section 6).

Evidence that is not in the repo (synthetic files only, no corpus) is under `E:/office-target/p3/gate/`:
`cli-matrix.tsv`, `api-matrix.tsv`, `cli/<case>/{report.json,stderr.txt}`, `api/*.server.log`, `ui/*.png`, `scripts/`
(the generators, the stub sidecar, the bad tools' sources).

---

## 1. Findings

Severity: **blocker** = fix (or decide) before the owner's "go"; **major** = fix in Phase 3 or before the feature is
switched on; **minor** = real, cheap, small blast radius. "XDFU" marks findings that matter only when `xdfu` is not
`off`; it is off by default everywhere (section 2.3).

### Blockers

**B1. PDF combine, split and extract pages exist in the CLI only: no API route, no UI control.**
- Where: `services/convert/src/main.rs` (`convert pdf combine|split|extract`); `services/convert-api/src/routes.rs` has no
  such route (grep: no `combine`, `split` or `extract` in `routes.rs`, `ui/app.js`, `ui/index.html`); `GET /api/v1/formats`
  lists 80 rows and none is a PDF operation (`docs/phase3/evidence/formats.json`).
- Scenario: the brief's task 1 lists "PDF combine / split / extract pages / text", and the gate asks that every supported
  direction works from CLI, API and UI. A user of the page cannot combine two PDFs; neither can an API client (Phase 5).
- Minimal fix, one of:
  - (a) the owner scopes the three operations as CLI-only for Phase 3 and moves API/UI to Phase 4 (the shell), and the
    brief's gate row is read with that exclusion recorded;
  - (b) `POST /api/v1/jobs` takes `options.op = combine|split|extract` (a combine job is one output from N files; split
    takes `every`/`before`; extract takes `pages`), planned with the existing `pdf_op` code, plus one UI option group
    and one e2e row each. Estimate 1-2 days.

**B2. A `;`-separated CSV with `dd.mm.yyyy` dates read under the en-US locale swaps day and month, with status ok and no warning.**
- Measured (`E:/office-target/p3/fidelity/fixtures/staff.csv`, Excel 16's own ru CSV): `convert staff.csv --to xlsx
  --locale en-US --json` gives `status ok, code 0, warnings []`; `--to csv` writes `1/3/2019` for `01.03.2019` and keeps
  `3500000,0` as text. Under `--locale ru` (and the default on this PC) the date stays `01.03.2019`.
- When the job locale is en-US:
  - an API client that does not send `options.locale` (api.md: the default is `en-US`);
  - the page used in English (`ui/app.js:194` sends the UI language as `locale`);
  - the CLI on a PC whose Windows *display language* is English but whose regional format is ru or uz:
    `services/convert/src/main.rs:205-216` takes `sys_locale::get_locale()`, which on Windows is the UI language list,
    not the regional format. On this PC both are ru-RU, so the default happens to be right.
- This is defect D6 coming back. `defects.md` D6 planned an interim warning ("warn on every csv/tsv input that is not
  `,`-delimited"), but it was never added.
- Minimal fix: (1) a warning row: csv/tsv input with a `;` or tab delimiter, or cells shaped like `dd.mm.yyyy`, read in
  an en-US job, warns "dates and decimal commas are read as en-US"; (2) the CLI default from the regional format
  (`HKCU\Control Panel\International\LocaleName`, or `GetUserDefaultLocaleName` through the already-linked `windows`
  crate); (3) a test with `staff.csv` under en-US that expects the warning.

### Major

**M1. The page shows warning placeholders literally: `{copy}`, `{missing} of {total}`, `{encoding}`.**
`services/convert-api/ui/app.js:76-83` (`warningText`) fills only `{text}` (and `{kind}`); the report's `Warning` carries
no arguments. The affected catalog rows are `warning.text_transcoded` ("The text was in {encoding}"), `warning.pdf_text_missing`
(the D5 guard: "{missing} of {total} letters are missing") and `warning.xdfu_copy` ("Copy {copy} was entered"), in all
five languages. The warning still shows, so it is not silent, but the number the user needs is missing. A cp1251 CSV,
common for Cyrillic data, hits it. Fix: add `args` to `report::Warning` (the values `text_with` already has) and call
`t(key, Object.assign({text: w.text}, w.args))`; add one Playwright assertion.

**M2. The patch queue does not rebase on today's upstream: 13 conflicts, two of them in the NumLocale patches.**
The dry run (section 6) gives 53 patches clean, 6 3-way and 13 conflicting. gridcraft `0028` (display/CSV) and `0029`
(PDF/print) collide with upstream's new `value_text_as` (#1163). The other NumLocale patches (0023-0027, 0030, 0130)
apply. Not a Phase 3 defect: upstream moved ~700 commits since BASE. But the next sync is a hand-merge of 13 patches
(estimate 1-2 days), and NumLocale must be re-measured after it. Fix: schedule the sync as its own work package after
the gate. Re-run `ci.sh convert-e2e` and the WP14 matrix on the result.

**M3 (XDFU; blocks switching XDFU on, not the gate). The docx and xlsx watermark replaces the document's own header and footer.**
- Where: `services/convert/src/xdfu.rs:853-876` (docx `insert.header`/`insert.footer` with `text`) and `:912-925` (xlsx
  `pageLayout.headerFooter {"header": "&C…", "footer": …}`).
- Upstream: wordcraft `crates/engine/src/cmd/insert.rs:540-563`. `hf_part` takes the default header of the caret's
  section (section 1), and `set_story` replaces its content. gridcraft replaces all three sections. Read and confirmed
  by this review.
- Scenario: a letter whose letterhead, page number or the grif itself sits in the header is converted with
  `xdfu = "watermark"`. The output's header becomes the stamp line only: the letterhead is lost, and so is the grif that
  clause 32 requires on the copy. Exit 0, no warning. With a different first page, even-page headers or several
  sections, page 1 or later pages get no stamp at all. pptx `design.headerFooter` probably replaces existing footers
  too (UNVERIFIED).
- Fix:
  - xlsx: use the section form (`center` only) and set `firstHeader`/`evenHeader`.
  - docx: either a carried wordcraft patch (`insert.header` with `append`, over every header part of every section), or
    our own zip pass that prepends one paragraph to every `word/header*.xml` and `footer*.xml`.
  - Test: a grif in the header plus `titlePg`; both the grif and the stamp must be on page 1.

**M4 (XDFU). Detection misses common real layouts, and the job then converts unmarked with exit 0.**
- The output check for kinds that are not scanned on input (`xdfu.rs:1277-1307`, `1354-1394`) is blind to headers. The
  odt reader drops headers for every target (defects.md E5), and txt, md and csv outputs drop them by design. So an .odt,
  .doc, .rtf or .xls whose grif is in the header is never found.
- "Top of page 1" is the first 8 paragraphs or lines in reading order (`xdfu.rs:401`, `407-414`, `576-598`). In a
  two-column letterhead table, the left cell's 8 or more lines push the upper-right grif out of the window.
- An image-only PDF (a scanned paper original) yields no evidence and counts as "not marked" (`xdfu.rs:586-607`,
  `1313-1315`). That contradicts the module's own rule that not knowing is not the same as not marked.
- Fix:
  - scan odt and ods natively (`content.xml`, `styles.xml`, `meta.xml`);
  - for doc, rtf, xls and xlsb, detect on a probe conversion to docx or xlsx in `mid/`;
  - scan every cell of a table that starts in the window, and widen the window to about 25 paragraphs;
  - treat a blank page-1 text layer as `xdfu_cannot_check`, or write that limit in api.md (owner choice).

**M5 (XDFU). The audit chain is an unkeyed SHA-256 chain.** `services/convert/src/audit.rs:66-72`, `81-112`. Anyone who
can write the file can rewrite lines k..n and recompute the hashes; `xdfu-verify` then passes. api.md says "a changed,
deleted, inserted or reordered line breaks the chain", which holds only against a naive edit. Fix: HMAC-SHA256 with an
operator key kept outside the log (the API already accepts an unused `key_file`), or append each `seq`/`hash` to a
second sink (the service log); correct the api.md wording either way.

### Minor

| # | Where | Scenario | Minimal fix |
|---|---|---|---|
| m1 | `services/convert/src/exec.rs:161-179` (`STDERR_PATTERNS`) | A password-protected `.doc` gives exit 1 "docs-cli failed: wordcraft-cli: this document is protected with a password"; an encrypted PDF gives exit 1 "pdf-cli failed: …protected by a password". A password-protected `.docx` gives exit 9 "password-protected". The message is right, but the code and the UI headline ("The conversion tool could not convert this file") differ. Measured, section 2 rows 15-16, 24-26 | add the exact phrases `this document is protected with a password` (wordcraft) and `the document is protected by a password` (pdfcraft) as `Rejected`, with tests against the real strings |
| m2 | `services/convert/src/http.rs:117-128` | The sidecar is down: the message reads "the pdf2docx sidecar did not answer: io: ����…(os error 10061)". The OS text arrives in the console code page and is decoded wrongly. It shows the same way in the API and the UI details | map `ConnectionRefused`/`TimedOut` to our own sentence and drop the OS text (keep it in the step's stderr tail) |
| m3 | `services/convert/src/kind.rs` (zip detection) | A cut-off `.docx` (right magic, no central directory) is reported as "unsupported input: the content is not a document…" (exit 3, UI headline "This file type is not supported"). The user is told the format is wrong, when the file is damaged | when the magic is `PK\x03\x04` and the central directory is unreadable, fail with exit 9 "the file is damaged (the zip directory is missing or cut off)" |
| m4 | sidecar `worker.py` messages relayed by `http.rs` | A garbage PDF through the sidecar: "cannot open the PDF: Failed to open file '/tmp/job-jye3px_i/in.pdf'". A container path reaches the client (api.md lists Unix paths as deferred to WP11) | strip `/tmp/job-*` in the sidecar's message, or extend the `Redactor` now |
| m5 | `services/convert/src/job.rs:332-345` (D5 `PdfTextGuard`) | If `pdf-cli text` fails, the guard is skipped and the native docx is returned with no warning, so the D5 loss can be silent again in that case. Other D5 notes, not silent: the fallback sends the PDF to the sidecar even when the user picked `native` (`job.rs:376-401`), and a sidecar timeout past the job deadline fails a job whose native docx exists | on a probe failure add a `pdf_text_unchecked` warning; make the fallback an operator switch, and never use it for a job the XDFU hook flagged |
| m6 | `crates/uz-translit/src/engine.rs:310-316` | `--alphabet auto` towards Latin always writes 1995. The O13 switch (`DEFAULT_LATIN_ALPHABET`, `lib.rs:59`) does not reach it | use `DEFAULT_LATIN_ALPHABET` there; a test with the constant flipped |
| m7 | `docs/UPSTREAM.md:130-147` | The converter-interface list leaves out what `xdfu.rs:850-993` calls (docs-cli `insert.header`, `insert.footer`, `design.watermark`, `file.properties`; slides-cli `design.headerFooter`; sheets-cli `pageLayout.headerFooter`; pdf-cli `doc_header_footer`, `doc_watermark`, `doc_save`, `doc_create`). It also describes `--cmd` as `{id, args}` JSON; the real form is `id={params}` (`plan.rs:355`) | add the rows and correct the form |
| m8 | `services/convert-api/ui/app.js:108-112`, `227-233`, `311`, `127-130` | Local-mode download links are plain `<a download>`, so a 404 after the retention time is handled by the browser, not by the page. An `i18n.json` load failure is swallowed, and the page then shows raw keys. `connect()` accepts an `http:` origin for `#token=` | use the existing `download()` in both modes; show an error when the catalog fails to load; allow https or loopback only |
| m9 | `services/convert/src/job.rs:1244`, `scan.rs:53` | An ods with charts converted to pdf under ru loses the `pdf_en_us` note: ODF charts are not counted, and the chart labels stay en-US | count ODF chart objects, or keep the note for ods |
| m10 | `services/convert/src/exec.rs:270-318` (honour-locale probe) | The probe checks one CSV round trip. A sheets-cli that ignores the locale but echoes the field would be judged "honoured", and `pdf_en_us` / `csv_one_sheet_en_us` would be dropped. The PDF path is not probed | probe en and ru and require the outputs to differ |
| m11 | `services/convert/src/text.rs:59` | A UTF-8 file with one invalid byte is decoded wholly as cp1252 (mojibake). It is reported (`text_transcoded`), so not silent | decode as lossy UTF-8 when nearly all high bytes form valid sequences |
| m12 | `services/convert/src/audit.rs:119-130`, `xdfu.rs:1401` (XDFU) | The lock is `with_extension("lock")`, so a log named `*.lock` is its own lock. The stale-lock removal is check-then-delete (two writers can get the same seq). The `copy` line is written before marking, so a failed mark leaves a copy that never existed | lock `path + ".lock"`; create the lock with `create_new` only; write the line after marking succeeds, or add a `failed` event |
| m13 | `services/convert/src/xdfu.rs:466`, `441-513`, `601`, `933` (XDFU) | Blank shared strings shift xlsx string indexes, so a grif cell can be missed. Prefixed SpreadsheetML (`x:c`) and xlsx text boxes are not scanned. pdf `subject`/`keywords` are read but pdf-cli `info` emits only title and author (api.md overclaims). The PDF mark rewrites the file (`full: true`), which invalidates an existing signature (clause 34) | keep empty `<si>` entries; correct api.md; prefer an incremental save if pdfcraft has one |
| m14 | `sidecars/pdf2docx/SOURCES.md:15-16` | The LGPL FFmpeg libraries inside the opencv wheel point to ffmpeg.org, not to the exact source. LGPL-2.1 section 6 wants the corresponding source (or an offer) when the image is distributed | add the exact FFmpeg build (from the wheel's `LICENSE-3RD-PARTY.txt`) to `scripts/sidecar-sources.sh` before Phase 10 publishes an image |

Not findings: the review-wp2 and review-wp7 items still listed as deferred in `api.md` "Not done" are not repeated here.

---

## 2. Security (PLAN 5.5)

### 2.1 Earlier fixes are in the code (spot check at HEAD)

| Item | Where at HEAD |
|---|---|
| WP2 B1: cancel, deadline, Job Object with kill-on-close | `services/convert/src/exec.rs:101-110` (`contain`, `win32job`, `limit_kill_on_job_close`), `:210-211` (`cancel`, `deadline` in `Ctl`), `:225-227` (job deadline → exit 4); measured: the hang tool was stopped at 5 s (CLI) and at the 8 s job deadline (API, UI) |
| WP2 B2: hooks with context, input source, output sink | `services/convert/src/hook.rs:18` (`JobCtx`), `:40-47` (`JobHook::before/after`), `job.rs:613` (`OutputSink`); outputs re-listed and re-validated after the hooks, test `fault.rs:277` |
| WP2 M1: temp-then-rename | `job.rs:753` (`.<name>.<pid>.part`) |
| WP2 M2: names safe in the crate | `names.rs:19` (`safe_name`) |
| WP2 M3: case-insensitive `warning:` capture | `exec.rs:694`, pdf `info` warnings `job.rs:555-569` |
| WP2 M4: slides `--pages`, expected output counts | `plan.rs:789-830`, `exec.rs:804` |
| WP2 M6: commit sampling every 100 ms | `exec.rs:347` (`sample_ms: 100`), `:603` (`memory().max(virtual_memory())`); measured: cap 150 MB, seen 152 MB, exit 5 |
| WP2 M7: xml/tex/json not taken as text | `kind.rs:136-138`, `:323-324` |
| WP2 M8: fan-out caps | `plan.rs:281-292` (`max_pages_in` 500, `max_png_out` 200, `max_sheets` 50), checks `:732-746` |
| WP2 M9 / WP7 M1: zip scan to the §5.5 limits, fail closed | `scan.rs:14-20` (1 GiB, 10,000 entries, 100:1 from 1 MiB), `:77-94`; measured: the 1.5 MB → 1.5 GiB bomb is rejected at `POST` with exit 9 |
| WP2 m5: exact password phrases | `exec.rs:161-179` (but see m1: two upstream phrases are missing) |
| WP2 m12: undeclared files in `out/` | `job.rs:817-825` |
| WP7 B1: a dropped request leaks nothing | `services/convert-api/src/routes.rs:752-765` (`Admitted` with `Drop`), bounded slot wait `routes.rs:838` |
| WP7 B2: disk bound | `app.rs:480-491` (store quota, user cap, free-space floor), `app.rs:733-752` (`DropInput`: an input is deleted when its file ends) |
| WP7 M3: per-address connection caps on every listener | `tls.rs:60-74`, `:150-167` |
| WP7 M5: no job key on disk | api.md "Data at rest"; test `server_mode_stores_nothing_readable_and_no_key_anywhere` |
| WP7 M6: trusted proxies, IPv6 by /64 | `routes.rs:118-152` (`ip_key`, limiter) |

### 2.2 New since those reviews (WP9, WP15, D2-D6, WP16)

All new findings are in section 1: B2, M1, M3-M5 and m1-m13. Checked and sound:
- the refuse path returns nothing (`job.rs:833-834`); every output is marked or the job fails (`xdfu.rs:1118-1126`);
- the client address is parsed to an `IpAddr` before it reaches the stamp (`routes.rs:155-160`);
- `sub` and the stamp reach the tools only as JSON values in argv, with no shell; control characters become `_`;
- D2-D4 loss rows match `defects.md`;
- 0-byte input is rejected (`job.rs:1105-1106`);
- the page renders server text with `textContent` only and uses no inline script or style;
- the token is read once from the fragment and removed with `replaceState`.

### 2.3 XDFU is off by default everywhere

| Surface | Default | Evidence |
|---|---|---|
| CLI | `off` | `services/convert/src/main.rs:122` (`--xdfu` absent → `Mode::Off`); `:126-128` refuses `--xdfu-audit`/`--redact` without a mode. Measured: `tests/golden/xdfu/grif.doc --to docx` with no flag → exit 0, no `xdfu` object in the report, no audit file created (`%APPDATA%\convert` and `%LOCALAPPDATA%\convert` do not exist). With `--xdfu refuse`: exit 9 "This document is marked XDFU … Nothing was converted", one `refused` audit line |
| API | `"off"` | `services/convert-api/src/config.rs:93`; the hook is installed only when the mode is not off (`app.rs:346-367`) |
| UI | no control | no `xdfu` in `ui/app.js` or `ui/index.html`; the 20 `xdfu` rows in `ui/i18n.json` are warning texts, shown only when a report carries them |

---

## 3. Licences: architect sign-off

`docs/phase3/licences.md` is **signed off** (architect opus, 2026-10-22), with the conditions below.

| Item | Verdict | Checked |
|---|---|---|
| pdf2docx 0.5.13 (MIT) | allowed | `requirements.lock` pins `pdf2docx==0.5.13` with a hash; the live sidecar's `/healthz` reports 0.5.13 |
| PyMuPDF 1.26.7 + MuPDF (AGPL-3.0) | **allowed, only as now**: in the isolated container, over HTTP, never linked or vendored, unmodified | pinned `PyMuPDF==1.26.7` with a hash; `run.sh` puts the worker on an `--internal` network, read-only root, tmpfs, `--cap-drop ALL`; no `mupdf`/`pymupdf`/`fitz` crate in any tracked `Cargo.lock`; `worker.py:39` is the only import. Conditions: no patches to PyMuPDF/MuPDF ever; moving to PyMuPDF 2.x needs a new licence check (`pymupdf-layout`, `pymupdf4llm`, onnxruntime); a distributed image ships `SOURCES.md` plus the `scripts/sidecar-sources.sh` bundle |
| opencv-python-headless 4.14 with bundled FFmpeg (LGPL-2.1+) | allowed in the container (unmodified, replaceable shared objects; no GPL x264/x265) | `sidecars/pdf2docx/LICENSES.md:24`. Condition (m14): name the exact FFmpeg source before any image is distributed |
| LibreOffice 7.4.7 (MPL-2.0, LGPL parts), `sidecars/soffice` | allowed, **test-only** reference renderer | the product never calls it: no `soffice`/`x2t` reference in `services/`; `.ppt` is refused (exit 3, "re-save the file as .pptx", O2); `Dockerfile:1` says test-only |
| ONLYOFFICE x2t (documentserver 8.2.3, AGPL-3.0) | allowed as a **local test reference only**, never shipped | used only by `scripts/fidelity/fid/refs.py` (an HTTP client to a container the operator starts); the image is local (`onlyoffice/documentserver:8.2.3`), not in any compose file of ours, not pushed. Condition: `ALLOW_PRIVATE_IP_ADDRESS=true` stays a local-only setting |
| `merge_lines.py` | allowed, MIT OR Apache-2.0 (O4) | byte-identical, sha `9563dd3a` in `/healthz` |
| New Rust deps of `services/` | allowed | `win32job` 2.0.3, `ureq` 3.4.2, `toml` 1.1.8, `sys-locale` 0.3.2, `png` 0.18.1, `aes-gcm` 0.10.3 / `aes` 0.8.4: MIT OR Apache-2.0; `encoding_rs` 0.8.42: (Apache-2.0 OR MIT) AND BSD-3-Clause; `rustls` 0.23.46: Apache-2.0 OR ISC OR MIT; `ring` 0.17.14: Apache-2.0 AND ISC; `rustls-webpki`: ISC; `zip` 9.0.5, `sysinfo` 0.37.2, `axum`, `hyper`: MIT. `aws-lc` is not used |
| `cargo deny` | pass | services: `cargo deny --manifest-path services/Cargo.toml --config ci/deny/services.toml check` → "advisories ok, bans ok, licenses ok, sources ok" (2026-10-22); root + 4 domains: `ci.sh full` job `deny` ok (section 5) |
| Clean-room | clean | `git grep` over `services/`, `sidecars/`, `i18n/converter.tsv`, `scripts/fidelity/`, the converter spec and `xtask/src/i18n` for ONLYOFFICE/Ascensio, LibreOffice/Document Foundation, Microsoft copyright, MPL and AGPL headers: every hit is a call to a separate process, a comment or a licence table. No copied code or translations. The uz-Cyrl strings come from `uz-translit`, the uz-Latn-2026 ones are derived, and ru/en were written by agents (`converter.tsv` review marks) |

Not legal advice. The AGPL "aggregate" reading (`PLAN 3.1`) is the usual one; Phase 10 publishing is where it must hold.

---

## 4. Never silent: every tool and sidecar broken on purpose

Binaries: `E:/office-target/p1/services/release/convert.exe` and `convert-api.exe`, rebuilt under the build lock at
2026-10-21 23:54 (they predated the WP16 `routes.rs` change), and the release domain CLIs. Bad tools (`panic.exe`: panics;
`av.exe`: reads a null pointer; `hang.exe`: sleeps 600 s) are 3-line Rust programs in `E:/office-target/p3/gate/scripts/`.
Sidecar: my own instance of `office/pdf2docx:dev` (`sidecars/pdf2docx/run.sh`, internal network) and a Python stub that
answers 503, 504 or 500. Inputs: `hrm_*` and `scan.pdf` from the synthetic census set; `corrupt.docx` (the first third of a
docx), `garbage.pdf` (a PDF header + random bytes), `empty.docx` (0 B), `bomb.docx` (1.5 MB, 1.5 GiB inflated),
`locked.docx` and `locked.doc` (Word 16 COM `SaveAs2` with a password; my own Word PID, killed after), `locked.pdf`
(AES-256, made by PyMuPDF inside the sidecar image).

Surface: **CLI** = exit code, `--json` report, stderr; **API** = `POST /api/v1/jobs` answer and `GET /jobs/{id}` file
state; **UI** = the page in Chromium (Playwright), the per-file card. "UI headline" is the localized text; "Details" is the
engine message.

| # | Case | CLI exit | CLI message (report `message`) | API | UI |
|---|---|---|---|---|---|
| 1 | Missing CLI (`CONVERT_DOCS_CLI` → a path that does not exist) | 7 | "docs-cli was not found (set CONVERT_DOCS_CLI or CONVERT_TOOLS_DIR; looked at: …)" | file `failed` 7, paths shown as `<path>`; `/healthz` `tools.docs-cli:false, ok:false` | "A conversion tool is missing or cannot be reached." + details |
| 2 | Crashing tool: panic (exit 101) | 6 | "docs-cli crashed (panic, exit 101): bad-tool: deliberate panic in the engine" | `failed` 6, same text | "The conversion engine crashed on this file." + details |
| 3 | Crashing tool: access violation | 6 | "docs-cli crashed (exception 0xC0000005): no message" | `failed` 6 | same headline + details |
| 4 | Timeout (hanging tool) | 4 | "docs-cli did not finish within 5 s and was stopped" (`--timeout 5`, stopped at 5 s) | `failed` 4 "the job exceeded its overall time limit" (`job_timeout_s = 8`, 8.3 s) | uz-Latn: "Aylantirish juda uzoq davom etdi va toʻxtatildi." + details |
| 5 | Memory cap (real sheets-cli, `big300k.xlsx`, `--max-mem 150`) | 5 | "sheets-cli used more than 150 MB of memory (saw 152 MB) and was stopped" | not run (same executor) | not run |
| 6 | Sidecar down (nothing on the port) | 7 | "the pdf2docx sidecar did not answer: io: ����… (os error 10061)" (m2: the OS text is garbled) | `failed` 7, same | the "Separate service" engine option is disabled and labelled "(not available)" from `/healthz`; a job sent anyway would show the API text |
| 7 | Sidecar not configured | 7 | "the pdf2docx sidecar is not configured (set CONVERT_PDF2DOCX_URL …), or use --pdf-engine native" | - | as row 6 |
| 8 | Sidecar 503 busy (stub, `Retry-After: 1`) | 1 | "the PDF converter stayed busy (pdf2docx sidecar 503 busy): stub sidecar says 503" (after the backoff retries, 7 s) | `failed` 1, same | same path as the API |
| 9 | Sidecar 504 | 4 | "the PDF converter timed out (pdf2docx sidecar 504 timeout): …" | `failed` 4 | same path |
| 10 | Sidecar 500 | 6 | "the PDF converter's worker crashed (pdf2docx sidecar 500 worker_crashed): …" | `failed` 6 | same path |
| 11 | Real sidecar, scanned PDF | 1 | "the PDF has no text layer (pdf2docx sidecar 422 no_text_layer): … OCR is not supported" | `failed` 1 | same path |
| 12 | Real sidecar, garbage PDF | 3 | "the PDF cannot be read (pdf2docx sidecar 400 bad_pdf): cannot open the PDF: Failed to open file '/tmp/job-…/in.pdf'" (m4: container path) | - | - |
| 13 | Real sidecar, encrypted PDF | 9 | "the PDF is encrypted (pdf2docx sidecar 422 encrypted): … remove the protection first" | `failed` 9 | - |
| 14 | Corrupt input: cut-off docx | 3 | "unsupported input: the content is not a document, spreadsheet, presentation or PDF this converter reads" (m3: says unsupported, not damaged) | rejected at `POST`, 3 | "Rejected. This file type, or this conversion, is not supported." + details |
| 15 | Corrupt input: garbage PDF → txt / → docx (native) | 3 / 3 | "the input is not a readable PDF (pdf-cli: … no readable page tree or cross-reference data was found)" | `failed` 3 | uz-Latn: "Bu fayl turi yoki bu aylantirish qoʻllab-quvvatlanmaydi." + details |
| 16 | Empty file (0 B docx) | 9 | "the file is empty (0 bytes), so there is nothing to convert" | rejected at `POST`, 9 | "Rejected. The file was rejected: it is too large, password-protected or unsafe to open." + details |
| 17 | Password: docx (Word, encrypted package) | 9 | "the file is password-protected; passwords are not supported" | rejected at `POST`, 9 | "Rejected …" + details |
| 18 | Password: .doc (Word 97 RC4) | 1 | "docs-cli failed (exit 1): wordcraft-cli: this document is protected with a password" (+ `doc_import_loss` warning) (m1) | `failed` 1 | with docs-cli missing: row 1's text (the tool check comes first) |
| 19 | Password: encrypted PDF → txt / docx / png (native) | 1 / 1 / 1 | "pdf-cli failed (exit 1): pdfcraft-cli: the document is protected by a password" (m1) | `failed` 1 | uz-Latn: "Aylantirish vositasi bu faylni aylantira olmadi." + details |
| 20 | Zip bomb (1.5 MB → 1.5 GiB) | 9 | "the package expands to 1536 MB, over the limit of 1024 MB" | rejected at `POST`, 9, in 0.1 s | "Rejected …" + details |
| 21 | Scanned PDF → txt (native) | 0 + warning | `warn`, `empty_text`: "No text came out. A scanned PDF has no text layer, and OCR is not part of this converter." | `done` + `empty_text` | uz-Latn: "Ogohlantirish: Matn olinmadi …" |
| 22 | Control: `hrm_letter.docx` → pdf | 0 | ok, `hrm_letter.pdf` | `done` | "Done", download link |

Totals: CLI 29 runs, API 19 runs (`E:/office-target/p3/gate/{cli,api}-matrix.tsv`), UI 3 batches with 13 broken files
(`E:/office-target/p3/gate/ui/ui-never-silent-{broken-en,timeout-uz-Latn,sidecar-down-en}.png`). **No case was silent**:
every failure has a non-zero exit, a `failed`/`rejected` state and a message on all three surfaces, and the one success
with a loss carries its warning. The defects on this path are about the quality of the message (m1, m2, m3, m4) and the
B2 silent date swap, which is a wrong result and not a failure.

Real-tool UI run (closes the gap that `ci/web-smoke/converter.spec.ts` drives the page against the fake tool only):
`hrm_letter.docx`, `hrm_staff.xlsx` and `hrm_deck.pptx` → PDF: 3 of 3 done, three valid PDFs (104-378 KB), and the xlsx
carries `pdf_en_us` in an English job. `uz_latn.docx` → docx with "Latin to Cyrillic": 2,135 Cyrillic letters and 0
Latin in `word/document.xml`. `uz_cyrl.pdf` → docx (native) with the same option: done, with the `translit_nothing`
warning ("Transliteration found no text to convert"). Screenshot `ui/ui-real-tools-translit-en.png`.

---

## 5. `ci.sh full`

`scripts/ci.sh --run wp13-full full`, started 2026-10-21 23:52 on the shared tree (HEAD `21cade8`; each job reads the tree
when it starts, so the services jobs ran on `d67ed47` with the flaky-test fixes). Finished 2026-10-22 05:09. **Exit 0:
all 28 jobs ok**, 5 h 17 min. Summary `E:/office-target/p1/ci/wp13-full/summary.md`, one log per job next to it.

| Job | Result | Seconds | Notes |
|---|---|---|---|
| queue | ok | 31 | |
| lint | ok | 2,982 | upstream clippy: 1/2/2/1 warnings, all baselined |
| gen-check | ok | 5 | |
| deny | ok | 16 | root + 4 domains |
| brand | ok | 576 | |
| i18n | ok | 6 | |
| test-ours | ok | 1,903 | nextest summaries 178/93/44/43/71 passed (1, 2, 0, 0, 4 skipped) |
| test wordcraft | ok | 977 | |
| test gridcraft | ok | 4,352 | includes NumLocale 0023-0030/0130 |
| test deckcraft | ok | 1,548 | |
| test pdfcraft | ok | 454 | |
| check docs / sheets / slides / pdf | ok | 120 / 842 / 153 / 156 | desktop + wasm32 |
| bind | ok | 124 | |
| convert | ok | 93 | services clippy, nextest **171 of 171** (the two formerly flaky tests included), cargo-deny services |
| convert-e2e | ok | 270 | API e2e 2 of 2 (every `formats` row over HTTP, then a hard kill → empty `work/`); `directions.sh` 43 PASS / 0 FAIL, combine, split and extract included; Playwright converter spec 8 passed, 1 skipped (screenshots need `UI_SHOTS`) |
| build docs / sheets / slides / pdf | ok | 912 / 510 / 1,093 / 976 | desktop, web, cli |
| translit-npm | ok | 19 | |
| translit-preserve | ok | 127 | 8 of 8 IDENTICAL |
| sizes | ok | 447 | brotli headroom: docs 19,271 B, sheets 21,869, slides 26,935, pdf 22,771, npm 18,364 |
| brand (binaries) | ok | 321 | |
| web-smoke | ok | 11 | 4 passed |
| actionlint | ok | 2 | |

The builder's own `ci.sh convert` at `d67ed47` (`E:/office-target/p1/ci/20261022-004337/summary.md`): ok, exit 0, 1,448 s.
Not re-run here: `convert` passed inside the full run.

Disk: E: 87 → 86 GB free over the run. C: 8.2 → 6.7 GB free (98 % used). Nothing of this review is on C: except the
scratchpad and the `C:\c` leftover (section 9). C: is close to full and should be cleared before the next long run.

The silent-loss ratchet (`fidelity-directions`) is nightly, not in `full`. It was run once separately:

`scripts/ci.sh --run wp13-fid fidelity-directions` at `d67ed47` (it waited for build slots between the full run's steps): **ok, exit 0,
4,681 s**. It rebuilt the services (50 s) and the four CLIs (docs 186 s, sheets 126 s, slides 206 s, pdf 56 s), then ran
`run.py --directions all`: **90 directions, 117 cases, 4 silent losses** in 2,822 s, MS Office 16 reference. Then
`silent_baseline.py check`: "4 now, 4 in the baseline, 0 new, 0 gone". The 4 are D1 (txt → docx/odt/rtf/pdf page count),
accepted in `defects.md`. Log: `E:/office-target/p1/ci/wp13-fid/fidelity-directions.log`; JSON
`E:/office-target/p1/fidelity-directions/summary.json`. The ratchet cannot see B2. It passes no `--locale`, so on
this PC its csv/tsv jobs take the ru default from the Windows UI language. Its loss checks also do not compare date
cells: WP10 needed `csv_recheck.py` to find D6. The B2 fix should add an en-US csv case with a cell comparison.

---

## 6. Upstream sync dry run

`scripts/sync-upstream.sh --dry-run --survey --gates quick all`, 2026-10-21 23:55, exit 1, 51 min. The monorepo tree,
index and HEAD were untouched. Report `E:/office-target/p1/sync/reports/2026-10-21.md`; rebase log
`…/2026-10-21-rebase-log.tsv`; scratch worktrees `E:/office-target/p1/sync/wt-<repo>-<sha>`.

| Repo | BASE → upstream | Upstream commits | Patches | Clean / 3-way / conflict |
|---|---|---|---|---|
| wordcraft | `eae2421` → `2e6db0b` | 281 | 13 | 10 / 2 / 2 (`0001` web Cargo.toml and web.rs; `0101` ribbon.rs) |
| gridcraft | `f1894c6` → `2ebdf16` (0.10.0) | 77 | 42 | 31 / 2 / 9 (`0003`, `0009`, `0010`, `0015`, `0018`, `0020` Cargo.lock, `0022`, **`0028`**, **`0029`**) |
| deckcraft | `1dc6972` → `fe510bd` | 60 | 14 | 11 / 1 / 2 (`0103` format/lib.rs; `0124` cmd/file.rs) |
| pdfcraft | `924c4da` → `b77d570` | 258 | 4 | 3 / 1 / 0 |
| craft-libs | `15ed713` → `1f80537` | 26 | 0 | - |

- **NumLocale (gridcraft 0023-0030, 0130):** 0023-0027, 0030 and 0130 apply. 0028 conflicts in `crates/engine/src/display.rs`
  (upstream's new `value_text_as`, #1163) and `io.rs`. 0029 conflicts in `crates/engine/src/cmd/print.rs:1098`. Both need
  a hand-merge (M2).
- **Gates:** only pdfcraft got through the rebase. Its quick gates: verify-queue, register (73 patches / 73 rows),
  i18n-lint, gen-check, lockdiff, deny and check (native + wasm32, 466 s) pass. `i18n-stale` and `i18n-roundtrip` fail
  on 2 new upstream keys; `--apply` runs the extract that fixes them.
- **The CLI flags `convert` depends on (`docs/UPSTREAM.md`): all present at the new shas.** docs-cli `convert`, `run
  --file --cmd --save --print` (`wordcraft-cli main.rs:393, 456, 306-315`); sheets-cli `info --json`, `convert --sheet`,
  `run --in --cmd --out` (`gridcraft-cli main.rs:74-78, 229, 270, 385`); slides-cli `info`, `render --all/--slide
  --scale`, `convert`, `run` (`deckcraft-cli main.rs:53-58, 270-273, 464-476`); pdf-cli `info`, `text --page`, `combine
  --out`, `extract --pages --out`, `split --every/--before --out-dir`, `run --script --root` (`pdfcraft-cli main.rs:96-107,
  240-267`). Command ids present: `file.exportPdf`, `pageLayout.headerFooter`, `insert.header`, `design.watermark`,
  `file.properties`, `design.headerFooter`, `doc_open`, `doc_save`, `page_render`, `doc_create`, `doc_export_office`,
  `doc_header_footer`, `doc_watermark`. `tools.translit*` come from our own patches (0120/0122), which rebased cleanly.
- UPSTREAM.md is incomplete (m7).

---

## 7. The matrix against the brief

`GET /api/v1/formats` (`docs/phase3/evidence/formats.json`) has 80 rows: docx, doc, odt, rtf, html, md and txt each to
docx/odt/rtf/html/md/txt/pdf/png (56); xlsx, xls, xlsb, ods, csv and tsv each to pdf/xlsx/csv (18); pptx to pdf/png/pptx
(3); pdf to docx/txt/png (3). Transliteration is an option of the docs, sheets and slides rows.

| Brief Phase 3 task 1 | In `formats` | Gap |
|---|---|---|
| docx ↔ odt/rtf/html/md/txt | yes, both ways | - |
| .doc import; → pdf/png | yes (`.dot` read as doc) | - |
| xlsx; .xls/.xlsb/.ods import; → pdf/csv | yes (+ csv/tsv input) | - |
| pptx → pdf/png | yes | **.ppt: excluded by the owner (O2)**; refused with exit 3 and "re-save the file as .pptx". `.odp` is refused too |
| PDF combine / split / extract pages | **no** (CLI subcommands only) | **B1** |
| PDF text | yes (pdf → txt) | - |
| PDF → editable DOCX (task 2) | yes (`pdfEngine` native or sidecar) | O8 (default engine) is still open; WP10 recommends native |
| Transliteration as an option (task 3) | yes | - |

Not in the brief, said plainly in PLAN 1.5: no writer for ods/odp/doc/xls/ppt; no OCR; no URL input; no password input.

---

## 8. PLAN 9.3: the gate table

| Gate item | Evidence | Status |
|---|---|---|
| Every supported direction works from the **CLI** | the planner golden (`services/convert/tests/plan_golden.rs`, job `convert`); `services/convert/tests/directions.sh` (job `convert-e2e`); all 80 `formats` rows run through `convert.exe` in `docs/phase3/fidelity.md` 3.3; PDF operations in `fidelity.md` "Not rows of formats" (`pdf.combine`, split, extract: hash-equal pages); census `docs/phase3/census.md` | **proven**: `convert` and `convert-e2e` ok at `d67ed47` (section 5) |
| … from the **API** | `services/convert-api/tests/e2e.rs:246` `every_direction_over_http_then_a_hard_kill_leaves_work_empty_after_restart` (job `convert-e2e`, `CONVERT_API_E2E_REQUIRED=1`); section 4 API column | **proven for the 80 rows; open for PDF combine/split/extract (B1)** |
| … from the **UI** | `ci/web-smoke/converter.spec.ts` (9 tests, fake tool; job `convert-e2e`); the real-tool UI run in section 4 (docx/xlsx/pptx → pdf, pdf → docx, translit); screenshots `docs/phase3/evidence/ui/converter-*.png` (5 languages, no tofu) | **proven for the formats rows (UI = the same `POST /jobs`); open for the PDF operations (B1)** |
| **The fidelity table exists** | `docs/phase3/fidelity.md`: one row per `formats` row (80), Office 16, LibreOffice 7.4.7 and x2t 8.2.3 references, the PDF → DOCX candidates; `docs/phase3/evidence/fidelity-summary.json` | **proven, on the SYNTHETIC set only**: corpus (O9) not delivered |
| **Failures are reported, never silent** | fault tests `services/convert/tests/fault.rs` (exit 1/4/5/6/7/8, hooks), `convert-api/tests/{api,hardening}.rs`; section 4 (22 cases × CLI/API/UI, none silent); the silent-loss ratchet (section 5, `ci/fidelity/silent-baseline.json`: D1 only); `defects.md` D2-D6 fixed | **open: B2** (silent CSV date swap under en-US); m1-m4 are message quality |
| Brief task 2: PDF → DOCX sidecar, isolated, never linked | `sidecars/pdf2docx/` image, `run.sh` (internal network, read-only, limits); section 3 licence sign-off; `cargo deny` with no AGPL crate in any lock | **proven** |
| Brief task 3: transliteration, formatting kept | WP4 preserve route (`services/convert/tests/translit_e2e.rs`); `ci.sh full` job `translit-preserve`; the UI translit run (section 4) | **proven**: `translit-preserve` 8 of 8 IDENTICAL at this HEAD |
| Brief task 4: batch upload, queue with progress, zip, limits, timeouts, cleanup | `convert-api/tests/api.rs`, `hardening.rs` (limits, quota, dropped request, cancel, connection caps), `e2e.rs` (hard kill → empty `work/`); section 4 rows 4 and 20 | **proven** |

Open rows: the API and UI rows for the PDF operations (B1), and "never silent" (B2). The fidelity row stands with the
synthetic-only caveat that the owner already knows (O9).

---

## 9. What I did not verify

- Memory-cap (row 5) and the sidecar 503/504/500 cases were not driven through the UI. The UI calls the same `POST /jobs`, and
  the API rows show the texts.
- Server mode (JWT, TLS) was not exercised end to end in this review; its tests are in `convert-api/tests/{tls,hardening}.rs`
  and ran in the `convert` job.
- deckcraft `design.headerFooter` replace semantics (M3), and whether pdfcraft can save incrementally (m13).
- PyMuPDF 1.27/1.28 and 2.x licences.
- A run of the suite with `DEFAULT_LATIN_ALPHABET` flipped to 2026 (WP16 could not do it either).
- Housekeeping: my API runs wrote their `work_dir` under `C:\c\Users\uge226\AppData\Local\Temp\claude\…\scratchpad\`
  (an MSYS path written into a TOML file, 850 KB of synthetic outputs). The safety check stopped me from removing
  `C:\c`. The owner can delete that folder.

---

## 10. Close-out (2026-10-22)

This section re-verifies every finding of this review against the code at `d7b9582`. The fix work since `93dc8b2` is
`73d8e67`, `c73eb91`, `827eced`, `3d5fea8`, `a2df6f9`, `4b9adf6`, `6cc3623`, `23ce913` and `d7b9582`. Architect tier (opus).
No product code was changed in the close-out. The evidence that is not in the repo (synthetic files only) is under
`E:/office-target/p3/gate/close/`:
- `cli-matrix.tsv` and `api-matrix.tsv`;
- the two UI screenshots and `ui-combined.pdf`;
- `sidecar-real-garbage.report.json`;
- the scripts that made them.

### 10.1 Status of every finding

| # | Status | Commit | Where (at `d7b9582`) | Test / measurement |
|---|---|---|---|---|
| **B1** | **fixed** | `73d8e67` | `options.op` = `combine`/`split`/`extract`. Parsing: `services/convert-api/src/routes.rs:452-522`; file counts: `:938-948`; `formats` lists the 3 operations: `:388-390`. `app.rs:820-930` (`PdfOp`, `pdf_op` through the same `job::run_job`). Page: `ui/index.html:78-90`, `ui/app.js:185-243` (mode select, order buttons, Alt+Up/Down) | `convert-api/tests/api.rs:1248` (combine keeps the upload order), `:1287` (split, extract), `:1327` (21 bad option and file cases), `:1402` (caps), `:1452` (server mode: token, nothing readable at rest); `ci/web-smoke/converter.spec.ts:148`, `:187`. Measured with real tools: 10.2 rows 12-15 |
| **B2** | **fixed** | `827eced`, `6cc3623` | Warning `csv_locale_mismatch`: `services/convert/src/csvloc.rs:50-64`, `job.rs:1235-1241` (csv/tsv in an en-US job). CLI default from the regional format: `main.rs:213-245` (`HKCU\Control Panel\International\LocaleName`). Page: a Number format select, defaulting to the page language, `ui/app.js:44`, `:246`, `:382` | `csvloc.rs` unit tests (4), `tests/warnings.rs:176`, `main.rs:547`, `converter.spec.ts:207`. Measured: 10.2 rows 8-11 and 18 |
| **M1** | **fixed** | `73d8e67` | `convert-api/src/app.rs:198-225` (`warning_args`: the values read back from the filled English template), `:709-716`; `ui/app.js:79-83` | `app.rs:1074` (every placeholder of every code), `api.rs:1553`, `converter.spec.ts:129` (5 languages, no `{x}` left). A `shortcut` is recorded at `app.rs:201`: `report::Warning` should carry `args` itself |
| **M2** | **deferred to Phase 4** | - | the sync is a hand-merge of 13 patches (gridcraft `0028`/`0029` against upstream `value_text_as`) | carry-forward, 10.6 |
| **M3** (XDFU) | **fixed** | `827eced` | **docx:** the stamp paragraph is prepended to the body and to every `word/header*.xml`/`footer*.xml` (`xdfu.rs:1050-1088`); `insert.header`/`insert.footer` are no longer used. **xlsx:** merged into the centre section of every header and footer, first-page and even-page ones included (`:1090-1225`). **pptx:** a text box; the slide's own footers stay (`:1247-1296`) | `tests/xdfu.rs:797`: a letterhead header, a first-page header holding the grif, the xlsx left section and a pptx footer are all kept and stamped. Unit tests `xdfu.rs:2199-2260` |
| **M4** (XDFU) | **fixed** | `827eced` | Window: 25 paragraphs, plus every paragraph of a table that starts in it (`xdfu.rs:407-459`). odt/ods read natively, `styles.xml` headers included (`:638-676`). doc/rtf/xls/xlsb probed through docx/xlsx in `mid/` (`:776-789`). An image-only PDF is refused with `xdfu_no_text_layer` (`:1724-1760`). Every input of a combine is checked (`:1680`) | `tests/xdfu.rs:662`, `:725`, `:754`, `:912`, `:947`, `:1060`, `:1091`. The remaining limit is stated as a `shortcut` (`xdfu.rs:33-38`): a header that the legacy importer drops is missed |
| **M5** (XDFU) | **fixed** | `827eced`, `4b9adf6` | HMAC-SHA256 under an operator key file (`audit.rs:71-121`). A keyed log is never accepted without its key, and there is no downgrade. The key is `xdfu_audit_key_file` in the API config (`config.rs:363`) and `--xdfu-audit-key` in the CLI. An unkeyed log is announced at start (`audit.rs:124`). api.md wording corrected (`api.md:326-327`), including the limit that lines cut off the end are not detected | `audit.rs:443` (RFC 4231 case 2), `:452` (a consistent rewrite without the key fails), `tests/xdfu.rs:998` |
| m1 | fixed | `827eced` | `exec.rs:175-182`: the two exact upstream phrases are `Rejected` | `exec.rs:850`. Measured exit 9 for `.doc` and for PDF to txt/docx/png: 10.2 rows 1-4 |
| m2 | fixed | `827eced` | `http.rs:33`, `:150-170`: refused or timed out gives our own sentence; the OS text stays in the step's stderr tail | 10.2 row 7, CLI and API |
| m3 | fixed | `827eced`, `4b9adf6` | `kind.rs:124-125`, `:192` (`damaged`); `job.rs:1169-1176`; rejected at `POST`: `routes.rs:807` | `tests/warnings.rs:251`, `api.rs:1562`; 10.2 row 6 |
| m4 | fixed | `827eced`, `73d8e67` | `http.rs:46-60` cuts container paths out of sidecar messages. The API `Redactor` also removes Unix roots (`app.rs:170-172`) | `http.rs:230`, `app.rs:1100`. Measured with my own real sidecar on `garbage.pdf`: CLI exit 3, message "…Failed to open file '<path>'"; through the API, 0 occurrences of `/tmp/` in `GET /jobs/{id}`. The local CLI report keeps the raw path in the step's `stderr_tail`, by design |
| m5 | fixed (the silent part); rest deferred to Phase 5 | `827eced` | `job.rs:343-348`: a failed `pdf-cli text` probe now warns `pdf_text_unchecked` | `tests/warnings.rs:551` (the code set). Deferred to Phase 5 (server config): make the sidecar fallback an explicit operator switch, never used for an XDFU-flagged job |
| m6 | still open (latent) | - | `crates/uz-translit/src/engine.rs:309-316` still hard-codes 1995 for `auto` to Latin | No effect while `DEFAULT_LATIN_ALPHABET` is 1995 (`lib.rs:59`). It must be fixed before O13 flips the default. Carried (10.6) |
| m7 | fixed | `827eced` | `docs/UPSTREAM.md:137-140`: the XDFU commands and tools; `--cmd` is `id={params}` | read |
| m8 | fixed | `73d8e67` | `ui/app.js:99-113`, `:283-284` (`download()` in both modes, so a 404 is shown); `:371-397` (a catalog load failure is shown); `:119` (`#token=` only for https or loopback) | `converter.spec.ts:71`, `:271` |
| m9 | deferred to Phase 8 | - | `scan.rs:53` counts only `xl/charts/chart*.xml`, so an ods with charts converted to pdf under ru loses the `pdf_en_us` note | fidelity item |
| m10 | deferred to the Phase 4 sync | - | `exec.rs:287-330`: still one CSV probe; the PDF path is not probed | re-check when the synced sheets-cli is probed |
| m11 | fixed | `827eced` | `text.rs:59`: lossy UTF-8 when nearly all high bytes form valid sequences | `text.rs:178` |
| m12 | fixed | `827eced`, `4b9adf6` | `audit.rs:187-215`: `<log>.lock`, `create_new`, and a stale lock is taken over by rename. `:303-311`: the `copy` line is written only after the mark is produced | `audit.rs:509` |
| m13 (XDFU) | partly fixed; the rest is deferred until before XDFU is switched on (O10) | `827eced` | Fixed: empty `<si>` entries are kept (`xdfu.rs:517-521`, test `:2176`). Open: `x:`-prefixed SpreadsheetML and xlsx text boxes are not scanned; `api.md:291` still claims the pdf subject and keywords; the PDF mark is a full save (`xdfu.rs:1354`, `"full": true`), which breaks an existing signature | - |
| m14 | deferred to Phase 10 | - | `sidecars/pdf2docx/SOURCES.md:15-16` unchanged | before any image is distributed |

**New findings of the close-out.** All are minor, and none blocks the gate.

| # | Where | Finding | Fix |
|---|---|---|---|
| n1 | `docs/phase3/api.md:69-70` | Still says "with XDFU on, the hook inspects the first file of a combine". Stale since `827eced`: the hook reads every file of `in/` (`xdfu.rs:1680`, test `tests/xdfu.rs:1091`), and the API's `pdf_op` uses the same `job::run_job` (`app.rs:914-929`) | delete the sentence |
| n2 | `services/convert-api/ui/app.js:321`, `:331-334` | In combine/split/extract mode the file table's Status column stays "Waiting" after the job is done. The upload rows are set to `queued`, and `poll` updates only `S.op`. The result card is right ("Done", the download link). Seen in `close/ui-combine-real-en.png` | in `poll`, copy `S.op.state` onto the `S.sent` rows; add one Playwright assertion |
| n3 | `services/convert/src/main.rs:231-245` | The CLI default locale runs `reg` by bare name on every run without `--locale` | call `%SystemRoot%\System32\reg.exe`, or `GetUserDefaultLocaleName` through the already-linked `windows` crate |
| n4 | `scripts/fidelity/fid/directions.py` | The ratchet still has no en-US csv case with a cell comparison (section 5 asked for one with the B2 fix). B2 is guarded by `tests/warnings.rs:176` and `converter.spec.ts:207` instead, so a regression is caught in `convert` and `convert-e2e`, not by the ratchet | add the case in Phase 4 |

### 10.2 Never silent: the changed cases, re-run

**Binaries.** The release `E:/office-target/p1/services/release/convert.exe` and `convert-api.exe` were rebuilt under the
build lock at 07:36, by the `fidelity-directions` run's `build-services` step. The `convert-api.exe` from 07:20 predated
the page changes of `6cc3623`/`23ce913`, which it embeds. The domain CLIs are the release builds.

**Setup.** My own `convert-api` on 127.0.0.1:18791 (API), :18793 (UI) and :18794 (real sidecar), each stopped by its PID
afterwards. The sidecar URL points at a closed port, except for the m4 run, which used my own `office/pdf2docx:dev`
container (removed afterwards).

**Inputs.** The review's `locked.doc`, `locked.docx`, `locked.pdf`, `corrupt.docx` and `garbage.pdf`; `staff.csv` (Excel 16's
own ru CSV, synthetic); `uz_cyrl.pdf` and `uz_latn.pdf` (2 pages each).

| # | Case | CLI | API | UI |
|---|---|---|---|---|
| 1 | password `.doc` to pdf | **exit 9** "the input is password-protected (docs-cli: wordcraft-cli: this document is protected with a password)" (+ `doc_import_loss`) | `failed` 9, same | - |
| 2-4 | password PDF to txt / docx (native) / png | **exit 9** / 9 / 9 "the input is password-protected (pdf-cli: …protected by a password)" | `failed` 9 (txt, docx) | - |
| 5 | password `.docx` (control) | exit 9 | - | - |
| 6 | cut-off docx | **exit 9** "the file is damaged (the zip directory is missing or cut off); open it in its application and save it again" | **rejected at POST**, 9, same text | - |
| 7 | sidecar down | exit 7 "the pdf2docx sidecar did not answer: it is not running or refused the connection". **No OS text and no path** in the report or on stderr | `failed` 7, same, no path | - |
| 8 | `;` CSV, `--locale en-US`, to xlsx | exit 0, `warn`, **`csv_locale_mismatch`** | `done` + `csv_locale_mismatch`, both with no `locale` (the API default is en-US) and with `locale: en-US` | - |
| 9 | the same, to csv | exit 0, `warn`, `csv_locale_mismatch` | - | - |
| 10 | the same, `--locale ru` | ok, no warning | `locale: ru`: `done`, no warning | - |
| 11 | the same, no `--locale` (this PC's regional format is ru-RU) | ok, no warning (ru is read from the registry) | - | - |
| 12 | `pdf combine` uz_cyrl + uz_latn | exit 0, `combined.pdf`, 4 pages | `op: combine`: `done`, `combined.pdf` 159,694 B, `%PDF-` | **Real tools, Chromium.** The order was changed with the up button (uz_latn first). The card shows `Done`; the download is `combined.pdf`, 159,697 B, 4 pages. Page 1 reads "BUYRUQ" (Latin) and page 3 "БУЙРУҚ": the order the user set. No console error. `close/ui-combine-real-en.png` |
| 13 | `op: split`, `every: 1` | - | `done`: `uz_cyrl-p1.pdf`, `uz_cyrl-p2.pdf` | - |
| 14 | `op: extract`, `pages: "2"` | - | `done`: `uz_latn-pages.pdf` | - |
| 15 | `op: combine` with one file | - | **400** `bad_options` "combine needs at least two PDF files" | - |
| 16 | control: `hrm_letter.docx` to pdf | ok | `done`, 104,150 B | - |
| 17 | reverse probe: an en-US `,` CSV (`3/1/2019`, `"3,500,000.50"`) read under `--locale ru`, to xlsx | ok, no warning. The cells are 43525 (1 March 2019), 3500000.5 and 1200.75: **the same cells as under en-US**, so this direction does not swap | - | - |
| 18 | `;` CSV in the page | - | - | The Number format select follows the page language (uz-Latn gives `uz-Latn`, en gives `en-US`). In English the card shows "Warning: This CSV looks like it is in the Russian or Uzbek format …". `close/ui-csv-mismatch-en.png` |
| 19 | m4: real sidecar, `garbage.pdf` | exit 3, "…Failed to open file '<path>'" | `failed` 3, the same message, no `/tmp/` anywhere in the job JSON | - |
| 20 | real sidecar, `locked.pdf` | exit 9 "the PDF is encrypted (pdf2docx sidecar 422 encrypted)…" | - | - |

No case was silent. Every changed case now has the exit code and the wording that the review asked for.

### 10.3 `ci.sh full` on `d7b9582`

`scripts/ci.sh --run p3close-full full`, started 2026-10-22 07:35 on the shared tree at HEAD `d7b9582` (clean; the only
edits during the run were to the four docs files of this close-out). Finished 09:09. **All 28 jobs ok**, 1 h 34 min (sccache
warm; the WP13 run took 5 h 17 min). Summary `E:/office-target/p1/ci/p3close-full/summary.md`, one log per job next to it.

| Job | Result | Seconds | Notes |
|---|---|---|---|
| queue | ok | 8 | |
| lint | ok | 339 | upstream clippy: 1/2/2/1 warnings, all baselined |
| gen-check | ok | 5 | |
| deny | ok | 13 | root + 4 domains |
| brand | ok | 305 | |
| i18n | ok | 6 | the converter rows of `c73eb91`, `3d5fea8`, `6cc3623`, `23ce913` included |
| test-ours | ok | 644 | nextest 178/93/44/43/71 passed (1, 2, 0, 0, 4 skipped) |
| test wordcraft / gridcraft / deckcraft / pdfcraft | ok | 250 / 143 / 100 / 376 | |
| check docs / sheets / slides / pdf | ok | 34 / 23 / 58 / 36 | desktop + wasm32 |
| bind | ok | 109 | |
| convert | ok | 208 | services clippy `-D warnings`; nextest **206 of 206** (171 at WP13; the new tests of the fix commits); cargo-deny services "advisories ok, bans ok, licenses ok, sources ok" |
| convert-e2e | ok | 244 | API e2e 2 of 2 (every `formats` row over HTTP, then a hard kill leaves `work/` empty); `directions.sh` 43 PASS / 0 FAIL (combine, combine-3, split, split-before, extract included); Playwright converter spec **12 passed**, 1 skipped (screenshots need `UI_SHOTS`) |
| build docs / sheets / slides / pdf | ok | 358 / 123 / 1,073 / 264 | desktop, web, cli |
| translit-npm | ok | 14 | |
| translit-preserve | ok | 127 | 8 of 8 IDENTICAL |
| sizes | ok | 447 | brotli headroom unchanged: docs 19,271 B, sheets 21,869, slides 26,935, pdf 22,771, npm 18,364 |
| brand (binaries) | ok | 303 | |
| web-smoke | ok | 11 | 4 passed |
| actionlint | ok | 1 | |

Disk over both runs: C: 18 → 17 GB free, E: 83 GB free (never near the 5 GB stop).

### 10.4 The silent-loss ratchet (`fidelity-directions`, nightly)

`scripts/ci.sh --run p3close-fid fidelity-directions` at `d7b9582`, started 07:36 next to the full run: **ok, exit 0,
866 s**.
- **Builds:** services 27 s; CLIs docs 3 s, sheets 2 s, slides 127 s, pdf 5 s.
- **`run.py --directions all`:** **90 directions, 117 cases, 4 silent losses**, 689 s, with the MS Office 16 reference (14 new
  Office conversions, the rest from its cache, in its own PIDs).
- **`silent_baseline.py check`:** "4 now, 4 in the baseline, 0 new, 0 gone". The 4 are D1 (txt to docx/odt/rtf/pdf page count).
- **Files:** log `E:/office-target/p1/ci/p3close-fid/fidelity-directions.log`; JSON
  `E:/office-target/p1/fidelity-directions/summary.json`.
- **Office:** the owner's WINWORD (PID 24216, started 2026-10-21 15:03) was not touched, and no Office process of the run is
  left.

### 10.5 PLAN 9.3: the final gate table

| Gate item | Evidence | Status |
|---|---|---|
| Every supported direction works from the **CLI** | `services/convert/tests/plan_golden.rs` (job `convert`); `services/convert/tests/directions.sh` (job `convert-e2e`, combine, split and extract included); all 80 `formats` rows in `docs/phase3/fidelity.md` 3.3; 10.2 rows 1-12 | **proven** (10.3 `convert`, `convert-e2e`) |
| … from the **API** | `convert-api/tests/e2e.rs:246` (every `formats` row over HTTP, job `convert-e2e`); PDF operations `api.rs:1248`, `:1287`, `:1327`, `:1402`, `:1452`; 10.2 API column (real tools: combine, split, extract) | **proven**, PDF operations included (B1 fixed) |
| … from the **UI** | `ci/web-smoke/converter.spec.ts` (job `convert-e2e`; PDF operations `:148`, `:187`; number format `:207`); the real-tool UI runs (section 4, and 10.2 rows 12 and 18) | **proven**, PDF operations included (B1 fixed) |
| **The fidelity table exists** | `docs/phase3/fidelity.md` (one row per `formats` row; Office 16, LibreOffice 7.4.7 and x2t 8.2.3 references; the PDF to DOCX candidates); `docs/phase3/evidence/fidelity-summary.json` | **proven on the SYNTHETIC set only. Open: the corpus (O9) was not delivered**, so no claim is made about real HRM documents |
| **Failures are reported, never silent** | `services/convert/tests/fault.rs`, `convert-api/tests/{api,hardening}.rs`; section 4 (22 cases × 3 surfaces) and 10.2 (20 changed cases); the ratchet 10.4 (4 = D1, 0 new); `defects.md` D2-D6 fixed, D1 accepted (Phase 8) | **proven**: B2 fixed (`csv_locale_mismatch`, 10.2 rows 8-11 and 18), m1-m4 fixed |
| Brief task 2: PDF to DOCX sidecar, isolated, never linked | `sidecars/pdf2docx/` (internal network, read-only, limits); section 3 licence sign-off; `cargo deny` (10.3) | **proven**. O8 (the default engine) is still the owner's; the code default is `native` (`plan.rs:107-111`), as WP10 recommends |
| Brief task 3: transliteration, formatting kept | `services/convert/tests/translit_e2e.rs`; job `translit-preserve` (10.3); the UI translit run (section 4) | **proven** (10.3) |
| Brief task 4: batch, queue with progress, zip, limits, timeouts, cleanup | `convert-api/tests/api.rs`, `hardening.rs`, `e2e.rs` (a hard kill leaves `work/` empty); section 4 rows 4 and 20 | **proven** |
| XDFU (PLAN 10, opt-in) | off by default everywhere (section 2.3); M3-M5 fixed (10.1) | **built, off**. Switching it on needs O10 and the m13 rest |

### 10.6 Carry-forward to Phase 4

1. **The upstream sync with hand-merge (M2).** 13 conflicting patches, 6 3-way. Among them are gridcraft `0028` (display/CSV)
   and `0029` (PDF/print) against upstream's new `value_text_as` (#1163). After the merge: re-run the NumLocale tests, the
   WP14 matrix, `ci.sh convert-e2e`, and the m10 locale probe.
2. **D7.** Upstream pdfcraft's ru "Move down" (`Переместить вdownarrow`) needs a one-line carried patch with the sync,
   unless upstream fixes it first (`defects.md` D7).
3. **To Phase 8:** D1 (txt page count, the 4 ratchet entries), D8 (uz AM/PM designators), D9 (uz auto formats of typed
   dates and times, a `parse_input` design item), and m9 (ODF charts and `pdf_en_us`).
4. **Deferred minors:**
   - m5 rest (the fallback as an operator switch, never under XDFU), Phase 5;
   - m6 (fix before O13 flips the alphabet default);
   - m10 (with the sync);
   - m13 rest (before XDFU is switched on, O10);
   - m14 (Phase 10, before an image is distributed);
   - the close-out's n1-n4 (small; n1 is a one-line doc fix, n2 a few lines of `app.js`).
5. **M3 from Phase 2:** opening on an Android phone (`docs/phase2/gate-review.md`). Android is deferred by the owner
   (2026-10-20).
6. **Server mode** (JWT plus TLS) has not been exercised end to end in a real browser over TLS. Only
   `convert-api/tests/{tls,hardening}.rs` and the token tests of the page (`converter.spec.ts:271`, `:285`) cover it.
7. **The corpus (O9) is not delivered.** Every fidelity number is on the synthetic set.
8. Housekeeping: `C:\c` (850 KB, synthetic outputs of section 4) still exists; the owner can delete it. The config guard
   against MSYS paths (`convert-api/src/config.rs:376`, `:427`) stops it from happening again.

### 10.7 Verdict

**READY for the owner's "go".**
- Both blockers are fixed and measured on all three surfaces: B1 (PDF operations in the API and the page) and B2
  (`csv_locale_mismatch`, and the CLI default taken from the regional format).
- M1, M3, M4 and M5 are fixed. M2 is the Phase 4 sync, by design.
- `ci.sh full` is 28 of 28 ok on `d7b9582`.
- The ratchet shows 4 silent losses, which is the D1 baseline: 0 new.
- No re-run case was silent.

What remains does not block the gate, and is listed in 10.6:
- **Owner items:** O8 (the default PDF to DOCX engine; the code default is native, as recommended), O9 (corpus, so every
  fidelity number stays synthetic) and O10 (before XDFU is switched on anywhere).
- **Small fixes:** n1-n4, m6 and the deferred minors.
