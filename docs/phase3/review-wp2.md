# WP2 architect review: job model, executor, planner table (2026-10-21)

Scope: `services/convert/` at commit 4dd1741. This review is read-only: no build and no test run. Line numbers refer to
4dd1741, and the builder is editing these files for WP3, WP4 and the sidecar step. I checked against PLAN §1, §2,
§5.4, §5.5, §8 WP2/WP3/WP7/WP15, `docs/phase3/census.md`, and the upstream CLI sources the plan cites.

Severity:
- **Blocker:** change this before the next WP builds on the seam.
- **Major:** a wrong or silent result, or a security gap. Fix it in WP2/WP3.
- **Minor:** a real defect with a cheap fix or a small blast radius.

Verdict: the plan-then-execute split, the fixed relative paths in argv and JSON, and the cleared environment are sound.
The planner table matches §1, with the gaps listed below. The weak spots are:
- the boundary where outputs leave the job;
- two seams that WP7 and WP15 need and do not have yet;
- several ways an exit 0 can still be wrong.

---

## Blockers (seams)

### B1. No cancel or job deadline, and child processes outlive the parent
`exec.rs:246` (`run_tool`), the poll loop at `exec.rs:301-329`, and `job.rs:95` (`run_plan`).
- **Scenario:** WP7 `DELETE /jobs/{id}` must kill a running step (§5.1), and §5.3 needs a 30-minute job timeout.
  Neither is possible: `run_tool` blocks until the step's own timeout (up to 15 min per step), and a docx → png job of N
  pages runs N steps back to back with no overall bound.
- **Windows:** `child.kill()` (`exec.rs:331`) terminates the direct child only. If the API process dies (service stop,
  crash, forced kill in the WP7 done-check), Windows does not kill its children. The tools keep running for up to 15 min
  and keep writing into `work/` while the restarted server sweeps it.
- **Process trees today:** the four domain CLIs, on the subcommands we call, spawn no children. The only `Command::new`
  hits are the pdf-cli corpus `check` harness (`pdfcraft-cli/src/main.rs:749`), printing and tests. So the direct-child
  watchdog is enough for now. But if a tool ever spawns a child that inherits our pipes, the joins at `exec.rs:334-335`
  hang after the kill.
- **Minimal fix:**
  - Add `cancel: &AtomicBool` and `deadline: Option<Instant>` to `run_tool` and `run_plan`. Check them in the poll loop
    and between steps, and map them to a new "cancelled" exit, or to exit 4.
  - Put each step in a Windows Job Object with `KILL_ON_JOB_CLOSE` and a per-process memory limit. That also fixes M6
    exactly. Use a safe wrapper crate (for example `win32job`), with its licence checked by cargo-deny.
  - The PLAN §2.5 reason against Job Objects ("needs unsafe FFI, which our lints forbid") does not hold:
    `unsafe_code = "forbid"` (`services/Cargo.toml`) applies to our crates, not to a vetted dependency.
  - Linux (WP11): put the child in its own process group, kill the group, and set `PR_SET_PDEATHSIG` via the same
    wrapper.

### B2. The `PostOutput` hook cannot carry WP15 or WP7 encryption
`exec.rs:125-130`, call site `job.rs:192-194`.
- **Placement:** after every step validated and before anything leaves the job. That placement is right, and the veto
  works (test `fault.rs:144`). The trouble is what the hook receives and when it runs.
- **No context:** the hook gets `(job_dir, files)` only. It does not get the input kind, the target, the options, the
  principal (`sub` or OS user), the client address, the job id, or a way to add warnings or report fields. WP15 needs
  all of these: the watermark text (copy number, address, user, time), the audit line, and `xdfu: true` with evidence.
  In WP7 the principal differs per job, while `post_output` lives in the process-wide `Config`.
- **No pre-run phase:**
  - WP15 `refuse` mode must refuse before any tool runs.
  - "A direction that cannot carry a watermark is refused" must be decided at plan time.
  - Today the only hook runs after the conversion.
- **Unwatermarked copy can leak:** `files` is listed before the hooks run and is never re-listed. Suppose a watermark
  hook writes `output-wm.pdf` next to the original instead of rewriting in place. Then the original, unwatermarked
  file is returned, and the new one is dropped. Outputs are also not re-validated after a rewrite, so a hook that
  corrupts a PDF still returns exit 0.
- **Encryption at rest (§5.5):**
  - `collect` copies plaintext to `Dest.out_dir` (`job.rs:213`), and inputs are copied from a plaintext path
    (`job.rs:255`). So WP7 cannot encrypt on the way in or out without changing `run_job`.
  - Separately, the tools need plaintext in `in/`, `mid/` and `out/` while a step runs. So the claim that leftover files
    are unreadable after a crash only holds if `/work` is tmpfs. Write that into §5.5 and WP7.
- **Minimal fix:**
  - Replace `PostOutput` with
    `trait JobHook { fn before(&self, &JobCtx) -> Result<Vec<Warning>, Failure>; fn after(&self, &JobCtx, &[PathBuf]) -> Result<Vec<Warning>, Failure>; }`.
  - `JobCtx` carries `{ job_id, job_dir, input (in-job path), kind, target, opts, principal, client }`. The principal
    and client come in through `JobSpec`.
  - Call `before` after detection and before `plan`.
  - After `after` returns, re-list `out/` and run `validate_file` on each output.
  - Replace the `Dest` copy with an `OutputSink` (`put(name, src) -> Result<String, Failure>`). The CLI sink does
    reserve plus atomic rename (M1); the WP7 sink encrypts.
  - Accept inputs as a source the caller materialises into the job dir.

---

## Major

### M1. A failed or interrupted collect leaves real-looking files in the destination
`job.rs:166-182`, `job.rs:206-215`.
- `reserve` creates an empty file with the final name, then `fs::copy` writes over it.
- **Scenario A:** with `--sheet all` or a multi-page png, the 3rd copy fails (disk full, permission denied, or "no free
  name"). Files 1-2 and the empty placeholder stay in `--out-dir`, while the report says `failed` with `outputs: []`.
- **Scenario B:** Ctrl-C, or a kill during `fs::copy` of a large PDF, leaves a truncated `report.pdf` under its final
  name.
- In both cases the "never silent" rule is broken at the last step.
- **Fix:**
  1. Copy into `<out_dir>/.<name>.<pid>.part`, then `fs::rename` it onto the reserved name. Windows `rename` replaces.
  2. On any error, remove the files already placed and the placeholder.
  3. For `-o`, use the same temp-then-rename, so an existing target is never half-overwritten.

### M2. Output names are not made safe inside the crate (§5.5)
`job.rs:244` (`JobSpec.stem: String`), `job.rs:150-163` (`final_name`), `job.rs:211`.
- **Scenario:** WP7 builds a `JobSpec` with the client's file name as `stem`.
  - `..\..\x` is joined into `out_dir`.
  - `NUL`, `CON` or `COM1` (`NUL.docx` uploaded) makes `reserve` open the Windows device. `fs::copy` then "succeeds",
    and the report lists `...\NUL.pdf` as an output that does not exist: silent loss.
  - Bidi or control characters reach the download and zip names.
- Today the CLI is safe only because `file_stem()` of a local path cannot contain separators.
- **Fix:** sanitise `stem` once, in `run_job`:
  - no separators, `..`, control or bidi characters;
  - Windows reserved base names (CON, PRN, AUX, NUL, COM1-9, LPT1-9, also with trailing dots or spaces) get a `_`
    prefix;
  - at most 200 chars;
  - empty becomes `output`.

  This makes the §5.5 rule a property of the crate, not of WP7. Also, the report puts absolute local paths in
  `input` and `outputs` (`job.rs:263`, `:329`); WP7 must map them to ids (§5.4: clients never see server paths).

### M3. pdf-cli's loss warning is dropped, so exit 0 hides truncated content
`exec.rs:359` keeps only lines that start with `Warning:`.
- pdf-cli prints lower-case `warning: page N: content past the page's safety budget was skipped`
  (`upstream/pdfcraft/apps/pdfcraft-cli/src/main.rs:613`) on `render`.
- **Scenario:** pdf → png or docx → png of a heavy page silently drops content.
- The `warnings` array of `pdf-cli info` (`main.rs:325`) is also discarded at `job.rs:123-127`, and so are the
  sheets-cli `file.open` warnings printed during the `info` step.
- **Fix:** match `warning:` case-insensitively after trimming. Pass the `info` JSON `warnings` and `ran.warnings` from
  the Expand step into `p.warn`. Dedupe by text with a count (see m6).

### M4. Slides → png ignores `--pages`, and no output count is checked
`plan.rs:359-366` never reads `o.pages`, but `row_options` (`plan.rs:256-258`) advertises `pages` for every png row.
The golden file freezes the bug: `pages+dpi pptx->png: slides-cli render in/input.pptx --all out --scale 4.00`.
- **Scenario:** `--to png --pages 2` on a 30-slide deck returns 30 PNGs, exit 0.
- `Outputs::Dir` (`exec.rs:434-443`) accepts "at least one png". A render that stops after 3 of 5 slides with exit 0
  passes, and the same holds for `pdf split`. §2.6 requires the page or slide count to match.
- **Fix:**
  - Either reject `--pages` for slides at plan time, or filter `slide-NN.png` in `out/` to the spec before collect.
  - Give `Outputs::Dir` an expected count:
    - slides: from `slides-cli info`, or the render's own stdout;
    - split: `ceil(pages / N)`, or `len(before) + 1`, from `pdf-cli info`.

### M5. Redirecting `LOCALAPPDATA` hides per-user fonts, so `convert` output differs from the measured CLIs
`exec.rs:266-268` sets `USERPROFILE`, `APPDATA` and `LOCALAPPDATA` to `<job>/home`.
- Both font scanners read `%LOCALAPPDATA%\Microsoft\Windows\Fonts`:
  - wordcraft: `upstream/wordcraft/crates/fonts/src/fontdb.rs:730`;
  - gridcraft PDF embedding: `upstream/gridcraft/crates/engine/src/pdf_fonts.rs:166`.
- On Windows 10 1809 and later, a font installed without admin rights lands there.
- **Scenario:** a user who installed a Cyrillic or Uzbek font per-user gets substituted fonts in every docx/xlsx → pdf,
  with no warning. The census and Phase 0 numbers were taken with the full environment, so they do not describe
  `convert`.
- Also, `GRIDCRAFT_PDF_SYSTEM_FONTS` is an on/off switch (`pdf_fonts.rs:245`), not a path as §2.4 says.
- **Fix:**
  - In local mode, pass the real `LOCALAPPDATA` (prefs are already off via `WORDCRAFT_NO_PREFS`), or point the redirect
    at a directory containing the real `Microsoft\Windows\Fonts`.
  - On servers, fonts come only from `CRAFT_FONTS_DIR` or the image.
  - Correct §2.4.
  - Measure one direction with a per-user font, through `convert` and through the raw CLI.

### M6. The memory watchdog measures the wrong quantity and overshoots
`exec.rs:316-319`.
- **Wrong quantity:** sysinfo's `memory()` on Windows is the working set. Windows trims working sets under pressure,
  so the cap reads low exactly when the box is short of memory. A tool that commits much more than its resident set is
  not stopped. Commit is `virtual_memory()` (PrivateUsage; verify against the sysinfo 0.37 source).
- **Overshoot:** the fault tool's own rate (64 MB per 20 ms = 3.2 GB/s, `bin/fake_tool.rs:21-23`) gives about 800 MB
  of overshoot per 250 ms sample, and more for multi-threaded engines.
- **No budget for `-j`:** the CLI has no global budget, so `-j 4` on sheets is 4 × (6 GB + overshoot) on a 31.8 GB box.
- **Fix:** a Job Object memory limit (B1) makes the cap exact. Otherwise:
  - sample `virtual_memory()` on Windows;
  - sample every 100 ms;
  - assert `peak_mb < cap + 512` in `memory_cap_is_exit_5`, so the overshoot is measured, not assumed;
  - cap `-j` by `free RAM / tool cap`.

### M7. Word XML, LaTeX and JSON documents are converted as plain text, with exit 0
`kind.rs:60-82` (`from_ext` has no `xml`, `tex`, `latex`, `ltx` or `json`) and `kind.rs:197-206`. Any other UTF-8 text
becomes `Kind::Txt`, and `ext_mismatch` stays false because the extension maps to no kind. The file is then copied to
`in/input.txt` (`plan.rs:188-190`).
- wordcraft dispatches by extension and does open `xml` (Word 2003 XML / Flat OPC), `tex` and `json`
  (`upstream/wordcraft/crates/engine/src/io.rs:9-29`).
- **Scenario:** a Word 2003 XML document, still common in legacy office workflows, becomes a docx or pdf full of raw
  XML tags, with no warning: a wrong output that looks successful.
- **Fix:** either add these kinds, keeping the original extension in `input_path` and detecting `<?xml` with
  `w:wordDocument` / `pkg:package`, or `\documentclass`. Or return `DetectError::Unknown` for text whose head starts
  with `<?xml` and for those extensions, until §1 lists them.

### M8. The Expand steps have unbounded fan-out
`plan.rs:413-426` (`expand_pages`) and `plan.rs:430-463` (`expand_csv`).
- **Page fan-out:** a PDF with 50,000 tiny pages (cheap to make) gives 50,000 `render` processes. There is no input
  page limit (§5.3: 500) and no PNG output limit (§5.3: 200). Each process reopens the whole PDF.
- **Deviation from the plan:** §2.3 and census F4 say "one `pdf-cli run --script` with N `page_render` steps". The
  census measured 108 pages in 12.6-16.3 s in one process. The implementation spawns one process per page.
- **Sheet fan-out:**
  - `--sheet all` runs `info` (a full open), then one full reopen per sheet. The census measured 6.3 s per open for
    300k rows, so 20 sheets take about 2 min.
  - Hidden and veryHidden sheets (`visibility` in the `info` JSON, `gridcraft-cli/src/main.rs:241`) are exported with
    no note. In an HR workbook, a hidden sheet often holds data that was hidden on purpose.
- **Fix:**
  - Put `max_pages_in`, `max_png_out` and `max_sheets` in `Config`, checked in both expanders (exit 9).
  - Emit one `pdf-cli run --script` for the pages (the script builder is the `write_files` mechanism that already
    exists for native docx).
  - Skip veryHidden sheets with a warning, and warn on hidden ones.

### M9. The zip check is weaker than §5.5 and fails open (WP3 must not inherit it)
`job.rs:219-234`.
- **Weaker limits:** 4 GiB and 100,000 entries, against §5.5's 1 GiB, 10,000 entries and a 100:1 ratio.
- **Fails open:**
  - `ZipArchive::new` failure gives `Ok(())`, so an inconsistent central directory passes.
  - Per-entry errors are skipped.
- **Trusts the archive:** declared sizes are attacker-controlled. A central directory that lies small, while the local
  streams inflate large, passes, and then the engine inflates it.
- **Fix in WP3's scanner:**
  - Reject an unreadable or inconsistent directory for zip-kind inputs.
  - Check the ratio.
  - Build the plan's 42 KB → 4 GB fixture both honest and lying.
  - Verify and cite each engine's own inflate cap. gridcraft's is at `package.rs:13`; wordcraft's and deckcraft's are
    UNVERIFIED.

  Until then, the only backstop is the memory watchdog, which has the weaknesses in M6.

---

## Minor

- **m1. A crash or Ctrl-C leaks job directories** (`job.rs:66-73`). `Drop` runs on panic (there is no `panic = "abort"`),
  but not on Ctrl-C or a kill. `%TEMP%\convert-work\job-*` keeps plaintext copies of inputs.
  - **Fix:** at startup, remove `job-<pid>-*` directories whose pid is not alive (sysinfo is already a dependency).
    WP7's sweeper can reuse this.
- **m2. A panic in one `-j` job loses every report** (`job.rs:353-370`). `thread::scope` re-panics after the join, so
  the batch exits 101 with no report, while the other jobs' outputs are already in the destination.
  - **Fix:** wrap `f(item)` in `catch_unwind` and turn a panic into a `Crash` report.
- **m3. Checks run on the source, then the source is copied again** (`job.rs:296-317` against `job.rs:255`). Size,
  detection and the zip check read the source path; the copy happens later. A file that changes in between (sync
  folder, network share) is checked as one file and converted as another.
  - **Fix:** copy first, then check the copy. B2's input source needs this anyway.
- **m4. `pdf_op` skips the input size limit** (`job.rs:333-351`).
  - **Fix:** apply the `max_input_mb` check used in `convert_file`.
- **m5. False positives in `STDERR_PATTERNS`** (`exec.rs:111-118`). Bare `password` and `encrypted` are substring
  matches over all of stderr, including `Warning:` lines.
  - Example: gridcraft's `The sheet is protected with a password. Unprotect it first`
    (`gridcraft/crates/engine/src/cmd/review.rs:139`) would be reported as "the input is password-protected" (exit 9)
    for a file that opens fine. Today's routes do not hit it: translit skips locked cells (patch 0120).
  - **Fix:** match exact upstream phrases per tool, ignore `Warning:` lines, and test against the real upstream
    strings, so a wording change upstream breaks a test instead of the classification.
- **m6. Duplicate upstream warnings.** `--sheet all` on an `.xls` repeats the import warning once per sheet.
  - **Fix:** dedupe by `(code, text)` and increment `count`.
- **m7. Internal csv names come from document content** (`plan.rs:454-459`).
  - A sheet named `NUL` or `CON` writes `out/NUL.csv` (the Windows device). The job then fails with exit 8 for a valid
    workbook.
  - **Fix:** use index-based internal names (`out/s01.csv`), and map the sanitised sheet name only in `final_name`.
- **m8. OLE kind is decided by a whole-file UTF-16 search** (`kind.rs:140-156`). An `.xls` with an embedded Word object
  has a `WordDocument` stream in an `MBD*` storage, so it is routed to docs-cli and fails loudly.
  - **Fix:** when more than one app stream name is found, let the extension decide.
- **m9. Legacy-encoded text is rejected as "not a document"** (`kind.rs:188-193`). cp1251 and UTF-16 (Excel's
  "Unicode Text") CSV and TXT files get exit 3 with a misleading message. These encodings are common for Cyrillic
  data.
  - **Fix:** at least say "not UTF-8 text". Supporting these encodings is a product decision.
- **m10. Panic messages show the wrong line** (`exec.rs:347`). For a Rust panic, `last_line` is
  `note: run with RUST_BACKTRACE=1 ...`.
  - **Fix:** prefer the line after `panicked at`.
- **m11. `convert check` litters the system temp dir** (`main.rs:231-240`). It runs the tools with the system temp dir
  as the job dir and creates `%TEMP%\tmp` and `%TEMP%\home`.
  - **Fix:** use a `JobDir`.
- **m12. Unexpected files in `out/` are returned without validation** (`job.rs:185-187`). `collect` returns every file
  there, not just the declared, validated outputs.
  - **Fix:** fail (or warn) on files that are not declared. Use `symlink_metadata` and reject anything that is not a
    regular file.
- **m13. Missing or unmeasured rows.**
  - pdf → docx `--translit` (§1.4; census PASS) is refused (`plan.rs:248`). This is WP4's row; keep it on WP4's list.
  - Docs translit to pdf/html/md/txt/png is planned (`docs-cli run … --save out.<ext>`; the `.pdf` path goes to
    `file.save`, `wordcraft-cli/src/main.rs:395-407`) but UNMEASURED. The census ran translit only to docx, odt and rtf.
  - `--dpi` on non-png targets is accepted and ignored.
- **m14. Golden test coverage** (`tests/plan_golden.rs`).
  - It is a self-snapshot, so it freezes bugs as readily as fixes (M4 is in it). The Expand fixtures are invented; I
    checked that the real shapes match (`sheets[].name`, `gridcraft-cli/src/main.rs:239`; `pages`,
    `pdfcraft-cli/src/main.rs:315`).
  - The only check against the real CLI grammar is `tests/directions.sh`. It needs the census inputs on E:, so it
    cannot run in CI.
  - **Add to `directions.sh`:**
    - `--sheet NAME` success;
    - pptx `--pages`;
    - docs translit → pdf/png;
    - pdf → txt `--pages N`;
    - combine of 3 files;
    - split `--before`;
    - one `.xml` input (M7).
  - **Add to the golden:** a captured real `info` output as the fixture for both expanders.

---

## What I did not verify
- Nothing was built or run, so the existing tests and `directions.sh` were not executed.
- Which counters sysinfo 0.37 uses on Windows (M6) comes from memory of its source; check it before relying on it.
- Whether `pdf-cli extract` or `split` rejects out-of-range pages or silently drops them. `page_list`
  (`pdfcraft-cli/src/main.rs:400-419`) does not check against the page count; `organize::extract_pages` was not read.
- The inflate caps in wordcraft and deckcraft (M9).
- Whether wordcraft's html and md exports write images to a side folder. `collect` ignores directories, so images in a
  side folder would be lost; `formats/src/html.rs` appears to inline images as data URIs.
