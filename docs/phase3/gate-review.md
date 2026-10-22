# Phase 3 gate review (WP13): adversarial review of the converter

Date 2026-10-22, branch `phase-3`, HEAD `d67ed47`. The review started at `21cade8`. The builder's flaky-test fixes landed
during the review (`15f542b`, `a119148`, `d67ed47`), before any services job of the `ci.sh full` run below started, so
that run tested them. Architect tier (opus), with two read-only sub-agents: a security review of the new code and the
sync dry run. No product code was changed.

Gate items (BRIEF Phase 3, PLAN 9.3): (1) every supported direction works from CLI, API and UI; (2) the fidelity table
exists; (3) failures are reported to the user and never fail silently.

**Verdict: NOT READY for "go" yet.** Two findings block the gate (B1: the PDF operations are CLI-only; B2: a silent date swap
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
