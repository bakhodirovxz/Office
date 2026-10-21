# Phase 3 (Converter, "Konvertor"): execution plan

> **Owner decisions 2026-10-21**
> - **O1:** the converter runs **on this PC only** for now. WP11 (Linux build and server image) is deferred; the server
>   comes later.
> - **O2: no .ppt support.** The user is told to re-save the file as .pptx in PowerPoint. WP6 (the LibreOffice sidecar)
>   is dropped for .ppt.
> - **O3:** the proposed limits are accepted: 50 MB per file (xlsx 100 MB), 30 files / 300 MB per batch, 500 PDF pages,
>   outputs kept for 1 h. They are configurable.
> - **O6 (revised 2026-10-21):** the product is fully open source and most users have no HRM, so **no account is ever
>   required**. Two modes (§5.5):
>   - **Local (default):** the converter runs on the user's own PC with no login, like any desktop tool.
>   - **Server (optional):** the user may connect to a converter server. That server checks tokens from whatever issuer
>     its operator configures. The HRM is one such issuer for our organisation's deployment, configured rather than
>     hard-coded. Phase 5 SSO plugs in here.
> - **O4:** `merge_lines.py` is licensed **MIT OR Apache-2.0**, the same as our layer. It is the owner's code and the
>   owner allowed fully open source.
> - **O5:** Docker Desktop licensing is not an issue for now.
> - **O7: the NumLocale patch IS in Phase 3** (about 6–8 days). gridcraft becomes locale-aware for display, CSV, PDF
>   and typed input (uz/ru: decimal comma, NBSP grouping, `;` list separator, localized month names, dd.mm.yyyy input),
>   per `docs/phase1/locale-check.md`.
> - **Added 2026-10-21 (owner):**
>   - web security and encryption rules (§5.5), with XDFU ("for official use only") handling per Cabinet of Ministers
>     Resolution No. 766 of 04.12.2025 (§10, WP15);
>   - the 2026 Latin alphabet law, reported approved today (WP16).
> - **Licence facts verified by the orchestrator:** pdf2docx is MIT since 0.5.12 (PyPI shows 0.5.13, 2026-05-01). It
>   depends on PyMuPDF (AGPL), so the sidecar stays an isolated container.


| | |
|---|---|
| **Status** | Plan only, written 2026-10-20 on branch `phase-2` while the Phase 2 gate CI ran. No code, no cargo builds, nothing committed. Execution starts on branch `phase-3` (from `phase-2`) **after the owner's "go" at the Phase 2 gate**. |
| **Gate decisions in force** | ADR Appendix C and `CLAUDE.md`: MIT OR Apache-2.0; working name only from `brand/brand.toml`; W2 domains + pristine snapshot + patch queue; no upstream PRs (every upstream change is a carried patch with a `patches/REGISTER.md` row); MPL-2.0 only for the symphonia crates as a *linked* dependency; Uzbek UI strings and the translit lists reviewed by the agent (`reviewed:agent-<model>:<date>`); **Android deferred (2026-10-20): the converter and the UI translations come first**; the 1995 Latin alphabet is the default, 2026 is an option. |
| **Inputs** | `docs/BRIEF.md` Phase 3, §3, §5; `docs/ARCHITECTURE-DECISION.md` §2.1.2 (converter = upstream CLIs as subprocesses, one job per process), §3.2, §5.2, App. B/C; `docs/STATUS.md`; `docs/phase0/{inventory,fidelity-baseline,xlsx-bench,gridcraft-memory-r2,docserver-results}.md`; `docs/phase1/locale-check.md`; `docs/phase2/PLAN.md`; `vendor-notes/{README.md,merge_lines.py}`; the four upstream CLI sources. |
| **Exit gate (brief)** | (1) Every supported direction works from CLI, API and UI. (2) The fidelity table exists. (3) Failures are reported to the user and never fail silently. |
| **Estimate** | **25 stream-days likely (range 16.5–38; 22.5 without the Linux image WP11)**; with three parallel streams about **13–15 working days** of calendar (§10). |

How to use this plan. §1–§7 are the designs; §8 holds the work packages, each of which can be handed to one sub-agent
as written. `file:line` references were read for this plan on 2026-10-20 at the current BASEs (wordcraft `eae2421`,
gridcraft `f1894c6`, deckcraft `1dc6972`, pdfcraft `924c4da`). Paths are under `upstream/<repo>/` unless stated.
Re-check every reference in WP0: upstream moves 100–140 commits a day.

---

## 0. New facts found while planning (they change the design)

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **pdf2docx is MIT now, not GPL/AGPL.** The LICENSE commit "This project will now become MIT licensed" landed on 2026-03-09; PyPI marks 0.5.12 (2026-03-09) and 0.5.13 (2026-05-01) `MIT`. Earlier releases were GPL-3.0 (2020) and then AGPL-3.0 (2024). | [pdf2docx LICENSE history](https://github.com/ArtifexSoftware/pdf2docx/commits/master/LICENSE), [PyPI JSON](https://pypi.org/pypi/pdf2docx/json), [docs licence page](https://pdf2docx.readthedocs.io/en/latest/license.html) | The brief's "pdf2docx is GPL/AGPL" is out of date. Pin **pdf2docx ≥ 0.5.12** (an older pin would bring AGPL code back). **The sidecar stays mandatory anyway**, because pdf2docx requires PyMuPDF (F2). |
| F2 | **PyMuPDF is AGPL-3.0** (or the Artifex commercial licence), and it bundles MuPDF (AGPL). pdf2docx 0.5.13 requires `PyMuPDF>=1.26.7`. | [PyMuPDF COPYING](https://github.com/pymupdf/PyMuPDF/blob/main/COPYING), [PyPI JSON](https://pypi.org/pypi/PyMuPDF/json) | The sidecar image is an AGPL work as a whole. It is a separate container called over HTTP, never linked or vendored (BRIEF §5). Distributing the image means shipping the AGPL source offer (§3.2). |
| F3 | **pdfcraft already exports PDF → DOCX natively** (also PPTX, HTML, RTF): the `doc_export_office` automation tool (command `export.docx`): "text, headings, bold and italic … borderless inferred tables and positioned editable shapes on pages with artwork … Font matching and vector graphics remain limited". | `crates/automation/src/tools.rs:1063-1065`; `crates/engine/src/commands.rs:324` | The brief says no Craft app converts PDF to DOCX; at our BASE that is no longer true. It is permissive and needs no container. **Measure it next to pdf2docx** (WP9). The owner picks the default from the numbers (O8). The sidecar is still built, as the brief requires. |
| F4 | **wordcraft `convert` to `.png` writes page 1 only.** | `apps/wordcraft-cli/src/main.rs:322-323` (`file.exportPng` with `"page": 1`) | docx → png for all pages goes docx → pdf (wordcraft) → `pdf-cli run --script` with one `page_render` step per page (one process, `crates/automation/src/tools.rs:161`). WP1 measures how much this route differs from wordcraft's own renderer. |
| F5 | **gridcraft `convert` has no PDF target**, and CSV is one sheet. PDF goes through `run --in F --cmd file.exportPdf={"path":…,"sheets":"all"}`. | `apps/gridcraft-cli/src/main.rs:269-284` (message at `:275`); spec `crates/engine/src/cmd/print.rs:20-28`; harness `spikes/fidelity/run.py:142-143` | The planner (§2.3) uses `run` for xlsx → pdf. xlsx → csv runs once per sheet (`convert in out.csv --sheet NAME`) when the user asks for all sheets. |
| F6 | **Number formatting in CSV and PDF is en-US** (`3,500,000.50`, English month names); the NumLocale patch (6–8 d) was proposed and not done. | `docs/phase1/locale-check.md` §1, §5; CSV `crates/engine/src/io.rs:241-251`; PDF `crates/engine/src/cmd/print.rs:1067,1084` | Every xlsx → csv/pdf job carries a **warning**, not a silent difference. Funding the patch is owner decision O7. |
| F7 | **The CLIs read the paper size of HTML/MD/TXT input from the locale variables** (`Region::from_locale_vars`). On a Linux server with `LANG=C` that gives Letter. | `apps/wordcraft-cli/src/main.rs:63-71, 111-113` | The executor sets the locale variables per job (A4 for uz/ru users; §2.4). |
| F8 | **gridcraft prints importer losses on stderr** (`Warning: …`, e.g. what `.xls` import drops); wordcraft, deckcraft and pdfcraft print none for lossy conversions. | `apps/gridcraft-cli/src/main.rs:153-163` | Warnings come from three sources: upstream stderr, our input scanner and our static loss table (§2.6). |
| F9 | **None of the upstream CLIs has ever been built for Linux.** Phase 0 and Phase 1 built `windows-gnu` only. gridcraft PDF export embeds installed system fonts (patch `gridcraft/0021`), and a Linux container has no Calibri, Arial or Times. | `docs/phase0/inventory.md`; `patches/gridcraft/0021-*` | A server image (WP11) is new work and needs measuring: a Linux build, a font set, and pixel diffs between Linux and Windows. |
| F10 | **The engines contain no HTTP client.** Only the pdfcraft GUI links `ureq`/`reqwest` (its update check); the CLIs do not. | `upstream/pdfcraft/apps/pdfcraft/Cargo.toml`; no hit in `*/crates/*/Cargo.toml` or `*/apps/*-cli/Cargo.toml` | A hostile HTML or DOCX cannot make our tools fetch URLs. Defence in depth: the tool processes run without egress (§5.6). |
| F11 | **Docker's daemon is not running on the dev PC** (`dockerDesktopLinuxEngine` pipe missing), and Docker Desktop is a paid product for large organisations. | `docker version`, 2026-10-20 | WP0 starts the engine. The HRM organisation's licence position for Docker Desktop is owner question O5. Docker Engine on Linux, or inside WSL2, is Apache-2.0. |
| F12 | **The services workspace slot exists:** the root `Cargo.toml` excludes `services`, and ADR §2.1.3 puts the converter API there. | `Cargo.toml` (`exclude = [..., "services"]`); ADR §2.1.3 | No layout decision is needed. `services/` is a sixth Cargo workspace with its own lock and its own `ci/deny/services.toml`. |

---

## 1. The conversion matrix

**Notation.** `docs-cli`, `sheets-cli`, `slides-cli` and `pdf-cli` are our domain builds of the four upstream CLIs
(`domains/*/apps/*-cli`, sources in `upstream/<app>/apps/<app>-cli/src/main.rs`; built today at
`E:/office-target/p1/<d>/release/<d>-cli.exe`). "Measured" means `docs/phase0/fidelity-baseline.md` (10 synthetic
Office-16 files, release 0.7.0 builds, before our patches) unless stated. **NOT MEASURED** means nobody has run it;
WP1 runs each direction once and WP9 measures it.

### 1.1 Docs (wordcraft)
Open: `docx docm dotx dotm doc dot txt md markdown html htm rtf odt tex latex ltx xml json` (`crates/engine/src/io.rs:9-29`).
Save: `docx docm dotx dotm pdf txt md html rtf odt tex png json xml` (`io.rs:31`).

| Direction | Today's command | Missing | Known fidelity limits |
|---|---|---|---|
| docx → odt / rtf / html / md / txt | `docs-cli convert in.docx out.<ext>` (`main.rs:308-334`, `file.save`) | Our wrapper only | NOT MEASURED. docx → odt → docx passed a smoke test ([inventory]). md and txt lose page layout, headers/footers and images by design; html keeps images inline (unverified). |
| odt / rtf / html / md / txt → docx | `docs-cli convert in.<ext> out.docx` | Locale per job (F7) | NOT MEASURED. Paper size for html/md/txt comes from the region (F7). |
| .doc / .dot → docx (import) | `docs-cli convert in.doc out.docx` | Warning text | Measured: **comments and tracked changes lost** (1 → 0 each); paragraph count inflated (99 → 113, 46 → 53); pixel diff 7.5–11.5 %. Password-protected .doc opens only with a password (`io.rs:132-134`), which the converter does not offer: it reports "password-protected". |
| docx / doc / odt / rtf / … → pdf | `docs-cli convert in out.pdf` | — | Measured docx → pdf: page count 4/4, text similarity 0.96, pixel diff **mean 6.2 %, max 11.3 %**; a constant vertical body offset; Times New Roman substituted. HEAD builds with our font patches: NOT MEASURED. |
| docx → png (all pages) | `convert` = page 1 only (F4); `render <f> <png> --page N` reopens per page (`main.rs:353-361`) | Route via pdf + `pdf-cli` page_render script | NOT MEASURED. The pdf route's rasteriser differs from wordcraft's own (F4, WP1). |
| docx (+translit) → docx/odt/rtf/… | `docs-cli run --file in --cmd 'tools.translitCyrl={…}' --save out.<ext>` (`main.rs:362-412`; the command is in `domains/docs/crates/docs-translit`) | Our wrapper | Phase 2 gate: formatting identical apart from the text on 9 Office-made fixtures; layout dominates the time (0.35–0.44 s for 108 pages headless). |

### 1.2 Sheets (gridcraft)
Open by extension: `xlsx xlsm xltx xltm`, `xls xlt` (import only), `xlsb`, `ods`, `csv`, `tsv tab txt`, `json`, `html htm`
(`crates/engine/src/io.rs:12-39`). `convert` writes `xlsx csv tsv json html` (`main.rs:275`).

| Direction | Today's command | Missing | Known fidelity limits |
|---|---|---|---|
| xls / xlsb / ods → xlsx | `sheets-cli convert in out.xlsx` | Capture `Warning:` lines (F8) | Measured `.xls`: **charts and conditional formats lost** (1 → 0); fonts, fills, borders and pictures are not imported (upstream warning). xlsb and ods: NOT MEASURED. Cell rich text is lost by any save (Phase 2 P3). |
| xlsx (any input) → pdf | `sheets-cli run --in F --cmd 'file.exportPdf={"path":"…","sheets":"all"}'` (F5) | Our wrapper | With patches 0020–0022 (embedded fonts): text similarity 0.415 → **0.754**, pixel diff 3.9 %. The `.xls` route lost a page (4 → 3) with the chart. **en-US number/date text** (F6). |
| xlsx → csv | `sheets-cli convert in out.csv [--sheet NAME]` (`main.rs:269-284`) | Loop over sheets for "all" | One sheet per file, **displayed** values, `,` delimiter, **en-US separators** (F6; Excel ru writes `;` and `3500000,5`). General-format numbers keep full precision (`io.rs:243-249`). |
| xlsx → xlsx (+translit) / → pdf (+translit) | `sheets-cli run --in F --cmd 'tools.translitCyrl={"scope":"workbook",…}' --out out.xlsx` (`main.rs:384-433`; patch `gridcraft/0120`, `0122`) | Our wrapper | Phase 2: IDENTICAL on 2 fixtures after two documented normalisations; formulas never touched; text literals inside formulas reported as a count. |
| Large workbooks | any of the above | Admission control (§5.4) | CLI peak after the memory rounds: **w80 (79 MB) 817–828 MB**; round 1 had w120 at 2.4 GB at open and 2.6 GB at save ([gridcraft-memory-r2], [STATUS]). Open time 37 s (w40) and more; timeouts must scale with size. |

### 1.3 Slides (deckcraft)
Open: `.deckcraft`, `.pptx`, `.potx`, `.ppsx` (`.ppt`: "not a presentation DeckCraft can read", measured). Write:
`deckcraft pptx potx ppsx pdf png jpg jpeg gif txt md` (`crates/engine/src/cmd/file.rs:340`).

| Direction | Today's command | Missing | Known fidelity limits |
|---|---|---|---|
| pptx → pdf | `slides-cli convert in.pptx out.pdf` (`apps/deckcraft-cli/src/help.rs`, `convert`; format check `main.rs:425-432`) | — | Measured: 5/5 slides; shapes, table, picture and notes survive; pixel diff **mean 0.61 %, max 1.67 %**; chart survival NOT MEASURED. |
| pptx → png (all slides) | `slides-cli render in.pptx --all OUTDIR` (`main.rs:240-290`; names `slide-01.png`…) | — | Same renderer as pdf; `--scale 2` default. Notes are not drawn. |
| pptx (+translit) → pptx/pdf | `slides-cli run --in F --cmd 'tools.translitCyrl={"scope":"deck",…}' --save out.<ext>` (patch `deckcraft/0120`, `0122`) | Our wrapper | Phase 2: IDENTICAL on 2 fixtures (adjacent equal runs merged). |
| **.ppt → pptx/pdf/png** | none | **LibreOffice sidecar** (§1.6) | Today: fails. |

### 1.4 PDF (pdfcraft)
| Direction | Today's command | Missing | Known fidelity limits |
|---|---|---|---|
| combine | `pdf-cli combine a.pdf b.pdf … --out c.pdf` (`main.rs:525-535`) | — | Expected lossless; NOT MEASURED (WP9: per-page render hashes). |
| split | `pdf-cli split in.pdf (--every N \| --before 3,7) --out-dir D` (`main.rs:549-570`; names `<stem>-p<a>-<b>.pdf`) | — | As above. |
| extract pages | `pdf-cli extract in.pdf --pages 1,3,5-8 --out o.pdf` (`main.rs:537-547`) | — | As above. |
| text | `pdf-cli text in.pdf [--page N]` → stdout (`main.rs:~350-370`) | Write to a file; detect "no text layer" | Scanned PDFs give empty text. pdfcraft has an OCR crate (ocrs), but Cyrillic support is UNVERIFIED, so OCR is **out of scope**. An empty result is reported as a warning, never as success-with-nothing. |
| pdf → png | `pdf-cli render in.pdf --page N --out p.png` per page, or one `run --script` with `page_render` steps (`main.rs:880-909`, `tools.rs:161`) | Script builder | Lossless rasterisation at a chosen dpi (≤ 600). |
| **pdf → docx** | none in the CLI subcommands; `run --script` with `doc_open` + `doc_export_office` (F3) | **Sidecar** (§3) and/or the native route | Brief §3 facts: pdf2docx needs `parse_stream_table=False`; Cyrillic body text comes out one paragraph per line (fixed by `merge_lines.py`); text drawn over images (signature, QR, stamp) is lost; table column widths are wrong; x2t gives a faithful copy built from absolutely positioned text boxes. **None of this is measured in this repo yet** (WP9). |
| pdf → docx (+translit) | sidecar → `docs-cli run … tools.translitCyrl … --save` | Chain | Inherits both sets of limits. |

### 1.5 Not in the matrix (said plainly, so nothing fails silently)
- **xlsx → ods, pptx → odp, → .doc/.xls/.ppt**: no writer upstream. The planner answers "unsupported direction" (exit 3).
- **OCR of scanned PDFs**: out of scope (§1.4).
- **URL input**: not in Phase 3 (§5.5).
- Password-protected inputs: reported as "password-protected"; no password field in Phase 3.

### 1.6 .ppt: the legacy option (ADR B-8)
- **Mechanism.** A second sidecar: LibreOffice headless in its own container, called over HTTP. It does exactly one
  thing, `.ppt → .pptx` (`soffice --headless --convert-to pptx`, with a fresh `-env:UserInstallation` profile per job,
  because one shared profile makes concurrent conversions fail). Our engines then do pptx → pdf/png, so every output
  goes through one renderer.
- **Licence.** LibreOffice is MPL-2.0 (with an LGPLv3+ option for older code)
  ([Debian copyright](https://metadata.ftp-master.debian.org/changelogs/main/libr/libreoffice/stable_copyright)). As a
  separate process it is not a linked dependency, so the "MPL only for symphonia" rule (which governs linked crates)
  does not apply. ADR §5.2 already lists "LibreOffice headless (MPL-2.0) for .ppt, if kept" under the sidecar
  rule.
- **Clean-room.** The rule forbids copying LibreOffice code into this repo. Running its binary in a container copies
  nothing: our sidecar is a 100-line HTTP wrapper of our own around a command line. **Verdict: allowed**, under the
  same conditions as pdf2docx: separate image, never linked, never vendored, no LibreOffice source read for our code.
- **Still the owner's call (O2):** whether .ppt is in scope at all (B-8 asked how common .ppt files are). The image
  is about 0.6–1 GB (ESTIMATE). LibreOffice parsing hostile files is a security surface, so it gets no network, a
  read-only root, tmpfs and resource limits.
- The same image also renders the LibreOffice **reference** PDFs for the fidelity report (§7). That use is test-only
  and needs no owner decision.

---

## 2. The `convert` CLI (our crate, thin)

### 2.1 Where it lives
```
services/Cargo.toml                 # sixth workspace (F12): own lock, ci/deny/services.toml
services/convert/                   # lib (planner, executor, scanner, report) + bin `convert`
services/convert-api/               # bin `convert-api` (axum), ui/ (static page)
sidecars/pdf2docx/                  # Dockerfile, server.py, worker.py, merge_lines.py (owner's, unmodified), requirements.lock, LICENSES.md, SOURCES.md
sidecars/soffice/                   # Dockerfile, worker.py (server.py shared by copy at build time)
i18n/converter.tsv                  # UI strings and error/warning messages
scripts/fidelity/                   # the harness, promoted from spikes/fidelity (§7)
```
It links **no engine**. It finds the four CLIs through `convert.toml` (or `CONVERT_TOOLS_DIR`), and the sidecars
through URLs (`CONVERT_PDF2DOCX_URL`, `CONVERT_SOFFICE_URL`). Dependencies: `serde`, `serde_json`, `toml`, `zip` (input
scanning), `sysinfo` (memory watchdog), a blocking HTTP client without TLS (`ureq` with default features off, or
`reqwest` with features off; WP2 picks the one with fewer crates). All MIT/Apache.

### 2.2 Interface
```
convert <inputs…> --to <fmt> [-o FILE | --out-dir DIR] [options]
    fmt: docx odt rtf html md txt pdf png xlsx csv pptx
    --translit cyrl|latn|auto      --alphabet 1995|2026|auto (default 1995)   --apostrophe official|ascii
    --sheet NAME|active|all        (csv; default active)
    --pages 1,3-5                  (pdf → png/txt; docx → png)
    --dpi N                        (png; default 150)
    --pdf-engine sidecar|native    (pdf → docx; default from config, O8)
    --timeout SECS  --max-mem MB   (override the per-tool defaults, §2.5)
    -j N                           (parallel files; default 1)
    --json                         (machine report on stdout, §2.6)
    --strict                       (a warning becomes exit 10)
convert pdf combine <a.pdf> <b.pdf>… -o out.pdf
convert pdf split <in.pdf> (--every N | --before 3,7) --out-dir DIR
convert pdf extract <in.pdf> --pages 1,3-5 -o out.pdf
convert formats [--json]           (the matrix: inputs, targets, options; the API and UI read this)
convert check                      (finds every tool and sidecar, prints versions; exit 7 if one is missing)
```
- The input type comes from **magic bytes first** (`PK\x03\x04` + the `[Content_Types].xml` main part, OLE
  `D0 CF 11 E0` + the stream names, `%PDF-`, then text heuristics), and from the extension only as a tie-break. A
  `.docx` that is really an `.xlsx` is routed by content. A mismatch is a warning.
- Output names: `<stem>.<ext>`; multi-output gets `<stem>-p01.png`, `<stem>-<sheet>.csv`, `<stem>-p1-3.pdf`;
  collisions get ` (2)`. Inputs are never overwritten (`-o` equal to an input is a usage error).

### 2.3 Job model: plan, then execute
- **Planner** (pure function, table-driven): `(input kind, target, options) → Vec<Step>`. A `Step` is either
  `Tool { tool, argv, outputs }` or `Http { sidecar, route, body, output }`. Nothing runs during planning.
  Examples:
  - `doc → pdf`: `[Tool docs-cli convert in.doc out.pdf]`
  - `docx → docx --translit cyrl`: `[Tool docs-cli run --file in --cmd tools.translitCyrl={"scope":"document","latinAlphabet":"1995"} --save out.docx]`
  - `xlsx → pdf --translit cyrl`: `[Tool sheets-cli run --in in --cmd tools.translitCyrl={"scope":"workbook",…} --cmd file.exportPdf={"path":out,"sheets":"all"}]`
  - `xlsx → csv --sheet all`: `[Tool sheets-cli info --json] → one Tool per sheet` (expansion after step 1; the only dynamic plan).
  - `docx → png`: `[Tool docs-cli convert in mid.pdf] → [Tool pdf-cli run --root JOB --script pages.json]`
  - `ppt → pdf`: `[Http soffice ppt→pptx] → [Tool slides-cli convert mid.pptx out.pdf]`
  - `pdf → docx --translit cyrl`: `[Http pdf2docx] → [Tool docs-cli run --file mid.docx --cmd tools.translitCyrl=… --save out.docx]`
- **JSON in `--cmd` is built only from validated enums and paths we generated.** No user string reaches a command
  argument or the JSON, so there is no injection path. Processes start from an argv vector, never through a shell.
- **Executor:** one subprocess per Tool step (ADR: one job per process), in a job directory
  `<work>/<job-id>/{in,mid,out}`, with `cwd` = the job directory. Intermediate files stay in `mid/`; only `out/` is
  returned. Each step's outputs are checked before the next step starts (§2.6).
- The upstream tools write their output in place. A crash can leave a partial file. That file stays in `mid/` or
  `out/` of a job marked failed, and a failed job never returns outputs.

### 2.4 Process environment
- **Cleared environment.** Only these are passed: `PATH` (the tools dir), `SystemRoot` and `TEMP`/`TMP` = `<job>/tmp`
  (Windows), `HOME`/`USERPROFILE`/`APPDATA` = `<job>/home`, `LANG`/`LC_ALL`/`LC_PAPER` from the job's paper region
  (default `uz_UZ.UTF-8` → A4; F7), `WORDCRAFT_NO_PREFS=1` (no preferences read or written), `CRAFT_FONTS_DIR`
  (config), and `GRIDCRAFT_PDF_SYSTEM_FONTS`, which is an **on/off switch**, not a path (`pdf_fonts.rs:245`).
- **`LOCALAPPDATA`** (review M5): both font scanners read `%LOCALAPPDATA%\Microsoft\Windows\Fonts` (wordcraft
  `fontdb.rs:730`, gridcraft `pdf_fonts.rs:166`), where Windows 10 1809+ puts fonts installed without admin rights. In
  **local mode** the tools therefore get the real `LOCALAPPDATA`, so `convert` renders like the measured raw CLIs;
  preferences stay off through `WORDCRAFT_NO_PREFS`. In **server mode** (`CONVERT_LOCALAPPDATA=isolated`) it points into the
  job directory, and the fonts come from `CRAFT_FONTS_DIR` or the image only.
- **Process containment (Windows)**: each tool step runs in a Job Object with *kill on close*, through the safe `win32job` crate (it offers no
  per-process commit limit, so the memory cap stays with the watchdog; exact caps would need `unsafe` FFI). The tools die
  with `convert`, and `unsafe_code = "forbid"` still holds for our crates. Linux (WP11): process group, kill the group,
  `PR_SET_PDEATHSIG`.
- `pdf-cli run` always gets `--root <job>`, so its script can only touch the job directory.

### 2.5 Timeouts and memory
- **Timeout per step** = `base(tool) + per_mb(tool) × input MB`, capped at 15 min. Defaults (ESTIMATES until WP1
  measures them): docs 60 s + 2 s/MB; sheets 60 s + 3 s/MB (w40 opened in 37 s); slides 60 s + 2 s/MB; pdf 30 s +
  1 s/MB; pdf2docx 60 s + 3 s/page; soffice 120 s. On timeout the process is killed (`kill_on_drop` plus an explicit
  kill and wait), and the step reports exit 4.
- **Memory.** A watchdog samples the child's commit (private bytes, `sysinfo` `virtual_memory`; the working set reads low under
  memory pressure) every 100 ms and kills it above the cap (exit 5).
  Defaults: docs 3 GB, slides 3 GB, pdf 2 GB, sheets 6 GB. `shortcut: polling can overshoot by one sampling interval;
  the container cgroup limit (§5.6) is the hard backstop.` The cgroup limit (WP11) is the hard backstop on Linux; on Windows there is none
  beyond the watchdog.
- The peak working set of every step goes into the report. That is how WP1 and WP9 measure peaks for free.

### 2.6 Errors, warnings and exit codes (never fail silently)
**Output validation after every step:** the file exists, is non-empty, and starts with the right magic (`PK` for
docx/xlsx/pptx/odt, `%PDF-`, the PNG signature, `{\rtf`, UTF-8 for text). For PDF and PNG, the page or slide count
must match what was asked. `--verify` also re-opens each output with its own CLI (`info`). It is on by default in the
API and costs one more process.

**Warnings**, three sources merged into one list, each with a stable code and catalog text:
1. **Upstream stderr:** `Warning:` lines (F8), passed through verbatim with the code `upstream`.
2. **Input scanner** (zip central directory only, no extraction, no XML parser): the counts of `word|ppt|xl/charts/`,
   `*/diagrams/` (SmartArt), `*/embeddings/` (OLE), `comments*.xml`, `vbaProject.bin` (macros), `footnotes.xml`, and a
   byte search for `<w:ins `/`<w:del ` (tracked changes).
3. **A static loss table** `(feature, target) → loss`, from §1 and the WP9 measurements. For example: comments in
   `.doc` input are lost; md/txt drop headers, footers, images and page layout; csv keeps only one sheet, displayed
   values and en-US numbers (F6); macros are dropped when docm/xlsm is saved as docx/xlsx; pptx → png has no notes;
   pdf → txt gives an empty result on a scanned file.

A warning is emitted only when the scanner finds the feature **and** the table says the target loses it, so a plain
letter → pdf stays quiet.

**Report** (`--json`, and always in the API): per file `{input, kind, target, status: ok|warn|failed, code,
message, steps:[{tool, exit, ms, peak_mb, stderr_tail}], warnings:[{code, count, text}], outputs:[…]}`. `stderr_tail`
is at most 2 KB. In corpus mode (`--redact`) file names are replaced by ids and stderr tails are dropped (CLAUDE.md
corpus rule).

**Exit codes** (per file in the report; the process exits with the most severe one across the batch):

| Code | Meaning |
|---|---|
| 0 | all outputs written and valid (warnings may exist; they are printed to stderr and in the report) |
| 1 | a tool returned an error (an engine message is passed on) |
| 2 | usage error |
| 3 | unsupported input type or direction |
| 4 | timeout |
| 5 | memory cap exceeded |
| 6 | a tool crashed (panic exit 101, access violation, signal) |
| 7 | a tool or sidecar is missing or unreachable |
| 8 | the output failed validation (missing, empty, wrong magic, wrong page count) |
| 9 | input rejected by a limit (size, pages, zip-bomb check, password-protected) |
| 10 | `--strict` only: done, but with warnings |

The upstream CLIs all exit 1 on error (`wordcraft-cli main.rs:560-569`, `gridcraft-cli main.rs:62-92`,
`deckcraft-cli main.rs:105-111`, `pdfcraft-cli main.rs:116-121`). Exit 101 or an NTSTATUS/signal is mapped to 6.

---

## 3. The PDF → DOCX sidecar

### 3.1 Licence verdicts (verified 2026-10-20)

| Component | Licence | Role | Verdict |
|---|---|---|---|
| pdf2docx 0.5.13 | **MIT** since 0.5.12 (F1); older releases GPL-3.0 / AGPL-3.0 | in the sidecar | Allowed. Pin `==0.5.13` (≥ 0.5.12). |
| PyMuPDF (+ MuPDF) | **AGPL-3.0** or Artifex commercial (F2) | in the sidecar | Allowed **only** as a separate container over HTTP. Never linked, never vendored, never in our repo. |
| python-docx | MIT | sidecar (pdf2docx dependency and `merge_lines.py`) | Allowed |
| numpy, fonttools, fire, opencv-python-headless | BSD-3, MIT, Apache-2.0, Apache-2.0 (the opencv wheel bundles FFmpeg, LGPL; UNVERIFIED for the headless wheel) | sidecar | Allowed in the container; listed in `LICENSES.md` |
| `merge_lines.py` | **The owner's own code, no licence statement** (`vendor-notes/README.md`) | sidecar | Owner decision O4: license it MIT OR Apache-2.0 so the image can ship it. Until then it runs on this PC only. |
| LibreOffice | MPL-2.0 (+ LGPLv3+ option) | .ppt sidecar and test reference | Allowed as a separate container (§1.6) |
| ONLYOFFICE Docs (x2t) | AGPL-3.0 | test reference only | Run locally in Docker for references; never shipped, never called by the product |

Our sidecar glue (`server.py`, `worker.py`) is our code, MIT OR Apache-2.0. It imports pdf2docx and PyMuPDF, so the
image **as a whole is distributed under AGPL-3.0 terms**. MIT is compatible with that. The Rust converter talks to it
only over HTTP, as a separate program. (This is the usual reading of "aggregate" vs "derivative work"; it is not
legal advice.)

**AGPL obligations when the image is distributed** (Phase 10 publishes; Phase 3 builds locally only):
- `LICENSES.md` holds every licence text.
- `SOURCES.md` gives the exact versions and the source archives (sdists) for PyMuPDF/MuPDF.
- `scripts/sidecar-sources.sh` downloads those sdists into a source bundle that is kept with every image tag.
- We do not modify AGPL code, so the §13 network clause (which applies to modified versions) is not triggered. We keep
  it that way: no patches to PyMuPDF or MuPDF, ever.

### 3.2 Image
- Base: `python:3.12-slim-bookworm`, **pinned by digest** (PyMuPDF 2.x needs Python ≥ 3.11).
- `pip install --no-deps --require-hashes -r requirements.lock`: every package pinned with its hash, including
  PyMuPDF at an exact version ≥ 1.26.7 chosen in WP5.
- Non-root user, read-only root filesystem, `/tmp` as tmpfs (size-capped), no compilers in the final stage.
- `merge_lines.py` is copied from `vendor-notes/` **unmodified**; the build checks its sha256 (`9563dd3ae3ff705d…`).
  Any fix to it is a separate, measured change that the owner approves (it is the owner's code).
- Image tag = `pdf2docx-<pdf2docx ver>-pymupdf-<ver>-<our rev>`. Phase 3 never pushes it to a registry.

### 3.3 API
```
GET  /healthz                      → 200 {"ok":true,"versions":{"pdf2docx":"0.5.13","pymupdf":"…","python-docx":"…","merge_lines":"9563dd3a"},"busy":0,"slots":1}
POST /v1/pdf2docx?merge=1&pages=1-5   body: application/pdf (raw bytes, ≤ limit)
     200 → application/vnd.openxmlformats-officedocument.wordprocessingml.document
           header X-Convert-Report: {"pages":N,"merged":M,"paragraphs":P,"warnings":[…]}
     4xx/5xx → {"code":"…","message":"…"}   400 bad PDF · 413 too large · 422 encrypted | no_text_layer | too_many_pages · 503 busy (Retry-After) · 504 timeout · 500 worker crashed
```
- `server.py`: Python stdlib `ThreadingHTTPServer` only, so the HTTP layer adds no dependency. A
  `BoundedSemaphore(slots)` admits jobs; when it is full the answer is 503.
- Each job runs `python -I worker.py in.pdf out.docx opts.json` as a **child process** with a timeout, in its own
  temp dir that is deleted in `finally`. Memory: `resource.setrlimit(RLIMIT_AS)` in the worker, plus the container
  limit.
- `worker.py`:
  - opens the PDF with PyMuPDF; refuses an encrypted file (422) and too many pages (422);
  - runs `Converter(pdf).convert(docx, pages=…, parse_stream_table=False)`;
  - runs `merge_lines.py` as a subprocess (it reads `sys.argv`, so it is not imported);
  - computes the report: pages, merged paragraphs (from `merge_lines.py`'s stdout line), and `no_text_layer` pages
    (PyMuPDF `get_text()` empty).
- Run limits: `--memory 2g --cpus 2 --pids-limit 128`, its own Docker network marked `internal` (no egress), not
  published to the host in production (dev: 127.0.0.1 only).

### 3.4 The weak spots, and what is measured (WP9)
| Weak spot (brief §3) | Fixture (built by construction, so the truth is known) | Metric |
|---|---|---|
| Cyrillic one paragraph per line | Word-made HRM-style order (uz-Cyrl, ru, uz-Latn), justified body, exported by Word to PDF | paragraph count of the output docx vs the source docx (ratio, before and after `merge_lines.py`); text similarity |
| Fake tables from justified text | same | count of `w:tbl` in the output vs the source (`parse_stream_table=False` vs default, measured once to confirm the brief) |
| Text over images lost (stamp, signature, QR) | Word docx with a known word drawn over a picture, and a stamp image behind text | the known words present in the output docx text (yes/no per word) |
| Table column widths | Word tables with known `w:gridCol` widths, merged cells | per-column relative width error vs the source |
| Editability (x2t's weakness) | all | the share of text inside absolutely positioned text boxes (`wps:txbx`, `v:textbox`) per page |
| Visual fidelity | all | output docx opened in Word (COM) → PDF, vs the source PDF: page count, text similarity, pixel diff |

**Candidates in one table:** pdf2docx raw; pdf2docx + `merge_lines.py`; pdfcraft native (F3); Word 16's own PDF
reflow (Word opens a PDF through COM; feasibility checked in WP9) as an "Office reference"; x2t (if feasible).

**Edge cases** that must produce an error, not an empty docx: a scanned PDF (422 `no_text_layer`), an encrypted PDF,
a 0-byte file, a truncated PDF, and a 1,000-page PDF over the page limit.

---

## 4. Transliteration as a conversion option

- **The same Phase 2 commands, unchanged**, through `run --cmd`:
  - Docs: `tools.translitCyrl|translitLatn|translitAuto`, params `{"scope":"document","latinAlphabet":"1995|2026|auto","apostrophe":"official|ascii"}` (`domains/docs/crates/docs-translit/src/lib.rs:100-130`);
  - Sheets: `{"scope":"workbook",…}` (patch `gridcraft/0122`);
  - Slides: `{"scope":"deck",…}` (patch `deckcraft/0122`).
- The scope is **always explicit**. A headless session has no selection, but relying on `auto` would repeat the Phase
  2 P7 trap if a default ever changes.
- Defaults: alphabet **1995**; apostrophe official; language policy as decided in Phase 2 (ru/en-tagged text is
  converted).
- **Any input the engine opens can be transliterated:** docx, doc, odt, rtf, html, md and txt via `docs-cli run`;
  xlsx, xls, xlsb, ods and csv via `sheets-cli run`; pptx (and .ppt via the sidecar) via `slides-cli run`; pdf via the
  pdf → docx route, then `docs-cli`. The target format is free. Two examples: Latin .docx → Cyrillic .docx; Latin .doc
  → Cyrillic .pdf.
- **The command's result JSON** (`paragraphs`, `runs`, `skipped{lang,field,locked}`, `rulesVersion`;
  `docs-translit/src/lib.rs:203-214`) is captured from `run --print` / the gridcraft JSON line and copied into the job
  report. "Nothing to convert" (0 paragraphs) is a **warning** (`translit_nothing`), so a mistaken direction is never
  silent. Text literals inside formulas (Sheets) are a warning count too.
- Small fix found while planning: the `PARAMS` help string at `docs-translit/src/lib.rs:28` does not list
  `latinAlphabet`, although the parser accepts it (`:119-128`). One line, in WP4.
- **Tests:** the planner table test; an end-to-end test per app on the Phase 2 fixtures through `convert`,
  comparing with `scripts/translit/preserve-check.sh` and the docx `preserve` test, so the converter route equals the
  Phase 2 route byte for byte in the non-text parts.

---

## 5. The HTTP API (`services/convert-api`, axum)

### 5.1 Endpoints (versioned under `/api/v1`)
| Method, path | Purpose |
|---|---|
| `GET /api/v1/formats` | the matrix (`convert formats --json`), plus the server's limits |
| `POST /api/v1/jobs` | multipart: `files[]` (1..N) + `options` (JSON: `to`, `translit`, `alphabet`, `sheet`, `pages`, `dpi`, `pdfEngine`) → `202 {job, files:[{n, name, kind, target|error}]}`. Files with an unsupported direction are rejected **per file**, and the rest of the batch runs. |
| `GET /api/v1/jobs/{id}` | `{state: queued|running|done, progress: {done, total}, files:[{n, state, step, code, message, warnings, outputs}]}` |
| `GET /api/v1/jobs/{id}/files/{n}/{output}` | one output (`Content-Disposition` with an RFC 5987 `filename*`) |
| `GET /api/v1/jobs/{id}/zip` | every output plus `report.json`; built once into the job dir, then served |
| `DELETE /api/v1/jobs/{id}` | cancel (kills running steps) and delete now |
| `GET /healthz` | the tools found, the sidecars reachable, queue depth |
| `GET /` | the UI page (§6) |

**Progress: polling, not SSE.** The UI polls `GET /jobs/{id}` every second. The engines report no progress inside a
step, so progress is honest step counts. SSE would add push latency of about 1 s that nobody needs, plus proxy and
keepalive behaviour to test (Traefik on the HRM side). Polling works through anything. SSE is a later, additive
change if the shell wants it.

### 5.2 The queue
- An in-memory queue: one tokio task per job, plus two admission gates:
  1. **CPU slots** (default `min(4, cores/3)`);
  2. **a memory budget** (default 12 GB on this PC). Each step reserves its estimate before it starts. The sheets
     estimate is `20 MB × input MB` (from the measured 19–20 MB per file-MB at open; round 2 is lower), capped at the
     tool cap.
- A job that cannot ever fit is rejected up front (409 `too_large_for_server`). It is never queued forever.
- Queue length cap: 100 jobs, then 503 with `Retry-After`. Per user: at most 3 active jobs.
- A restart loses queued jobs. They are marked failed with `server_restarted`, never left "running". `shortcut:
  in-memory queue, upgrade to a persistent one only if the docserver (Phase 5) needs jobs that survive restarts.`

### 5.3 Limits (defaults are proposals; owner decision O3)
| Limit | Default | Enforced where |
|---|---|---|
| File size | 50 MB (xlsx 100 MB) | axum body limit per part; streaming to disk with a byte counter |
| Batch | 30 files, 300 MB | request limit |
| PDF pages (input) | 500 | `pdf-cli info` before planning |
| PNG pages (output) | 200 | after the pdf step |
| Step timeout / job timeout | §2.5 / 30 min | executor |
| Result retention | 1 h after done, then deleted | sweeper |
| Zip input | **not accepted** as a container of files (multipart carries the batch) | upload kind check |

### 5.4 Job isolation
- Every step is a subprocess, per the ADR. An engine panic, hang or the gridcraft multi-GB peak kills one step of one
  job, never the server.
- Each job has its own directory. Clients never see server paths.
- **In a container** (WP11): the API container has no egress (it reaches only the sidecars' internal network), a
  read-only root, `/work` as a size-capped tmpfs or a volume, non-root, and a cgroup memory limit = the memory budget
  + 1 GB. Tool processes inherit all of it (defence in depth for F10).

### 5.5 Security
- **Auth: JWT HS256**, reusing the spike-4 code (`spikes/docserver/src/jwt.rs`: hand-rolled HMAC-SHA256, so there is no
  `rsa` crate and no RUSTSEC-2023-0071; `alg` must be HS256; `exp` required; 4 tests to carry over). Claims `{sub,
  exp, scope:"convert"}`. Jobs belong to `sub`: another user's job id gives 404.
- **Two modes (owner decision O6, revised):**
  - **Local (the default).** `auth = "none"` with the server bound to `127.0.0.1`. The UI opens with no login, and
    there is no account anywhere in this path. The server refuses `auth = "none"` on any non-loopback address.
  - **Server (optional).** `auth = "jwt"`. The operator sets the issuer's shared secret (`secret_file`) and an optional
    `iss` to accept, in the server config. Any organisation can issue these tokens; our HRM is just one configured
    issuer, so nothing names the HRM in code. Phase 5 adds SSO (OIDC) as a further `auth` value, not a replacement.
- **UI:** in local mode the UI never asks for anything. A "Connect to server" setting (server URL plus a token) is
  optional. The token comes from the URL fragment (`#token=…`, never sent to the server in logs) or from a paste field.
  Clearing the setting returns the UI to local mode.
- **Path traversal:** client file names are never used as paths. They are stored as `in/<n>.<ext>` with `ext` from the
  allowlist. The original name is used only for the download name and zip entry names, sanitised: no separators, no
  `..`, no control or bidi characters, 200 characters max, deduplicated.
- **Zip bombs** (every OOXML/ODF input is a zip; the engines unzip it): before planning, read only the central
  directory and reject the file (exit 9) if the total uncompressed size is over 1 GiB (gridcraft's own cap,
  `package.rs:13`), the ratio is over 100:1 for any entry over 10 MB, there are more than 10,000 entries, or the
  central directory is inconsistent. The same check guards the zip we send back (it is our own output).
- **SSRF:** Phase 3 accepts **uploads only, no URLs**. If URL input is added later, reuse `spikes/docserver/src/fetch.rs`
  (scheme plus exact host:port allowlist, no redirects, size and time caps), and add resolve-then-check against
  private ranges, which the spike lacked.
- **Logs:** job id, user sub, kinds, sizes, codes and timings. No file names in server logs when `--redact` is on, and
  never any document content (the corpus rule).
- **Web security baseline** (owner request, 2026-10-21; checked by a test that reads every response's headers):
  - **Transport.** Server mode serves HTTPS only, via rustls: TLS 1.3, plus 1.2 with AEAD suites only. HSTS is set
    (`max-age=31536000`). Plain HTTP only redirects. Local mode stays plain HTTP, because it is bound to `127.0.0.1`.
  - **CSP.** `default-src 'self'`, `script-src 'self'` (no inline scripts, no `eval`), `object-src 'none'`,
    `frame-ancestors 'none'`, `base-uri 'none'`, `form-action 'self'`.
  - **Other headers.** `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
    `Permissions-Policy` denying camera, microphone and geolocation, and `Cache-Control: no-store` on every API
    response and output.
  - **No third-party origins.** Fonts, scripts and styles are all self-hosted, with no CDN, so the UI works on an
    air-gapped network. CORS is closed (same origin only).
  - **Cookies, if any appear later:** `Secure`, `HttpOnly`, `SameSite=Strict`. Tokens never go in URLs sent to the
    server (the fragment only).
  - **Rate limits** per `sub` and per IP on uploads and job creation.
  - **Data at rest (server mode).** Job files are encrypted with AES-256-GCM (RustCrypto `aes-gcm`, MIT/Apache) under
    a per-job key. The key is held only in memory and wrapped by an operator key file. The sweeper deletes the key
    first, so leftover files are unreadable after a crash.
- **XDFU documents:** see §10.
- **Body limits** are applied before multipart parsing, so a slow or huge upload cannot fill the disk.

---

## 6. The UI page (standalone; Phase 4's shell is not ready)

### 6.1 Shape
- `services/convert-api/ui/{index.html, app.js, app.css, i18n.json}`, served by the API at `/`. Vanilla JS, no
  framework, no build step, about 400–600 lines. The Phase 4 shell can later host it as an iframe or reuse the API
  directly.
- Flow:
  1. drag and drop, or pick files (several);
  2. each file shows its detected kind;
  3. choose the target format, filtered by `GET /formats` for the kinds present;
  4. options: transliteration (none, Latin → Cyrillic, Cyrillic → Latin, auto), alphabet (1995 default, 2026),
     CSV sheet (active or all), PNG dpi;
  5. Convert;
  6. per-file progress (queued, running step i of n, done, failed);
  7. per-file download, plus "Download all (zip)".
- **Every failure is visible.** A failed file shows the reason from the error code in the user's language plus the
  engine message ("details"). Warnings show as a yellow line per file with the count ("3 comments will be lost").
  Network or server errors are shown, never swallowed. Polling stops on 4xx and shows the error.
- Accessibility: real `<button>` and `<input type=file>` for keyboard users; labels on every control; a live region
  announcing per-file state changes.
- The title and product name come from `brand/brand.toml` (a new `[apps.convert]` entry with names in the 4 languages;
  `cargo xt brand check` validates it). The page has no hard-coded name.

### 6.2 i18n
- `i18n/converter.tsv`, in the same format and columns as the other catalogs. Strings: UI labels, format names, option
  names, and every error and warning code of §2.6.
- `cargo xt i18n export converter --format json` (a small export target) writes `ui/i18n.json` with the fallback
  chains of `i18n/config.toml` (uz-Cyrl → ru → en; uz-Latn → en).
- Language: from a picker, defaulting to `navigator.language` mapped to en / ru / uz-Latn / uz-Cyrl; remembered in
  `localStorage`.
- uz-Cyrl drafts are made with `uz-translit` itself, as in Phase 2 WP7. Review is by an opus sub-agent, marked
  `reviewed:agent-opus:<date>` (delegated, never an owner review).

### 6.3 Evidence
- A Playwright spec in `ci/web-smoke/converter.spec.ts`: one batch with docx + xlsx + pptx + pdf → pdf, a translit
  job, a failing file (a corrupt docx) and a rejected file (oversize).
- Screenshots in all four languages to `docs/phase3/evidence/ui/`, checked for no tofu.

---

## 7. The fidelity report (`docs/phase3/fidelity.md`)

**One table per conversion direction**, with columns: files, ok / failed, page-count match, text similarity, pixel
diff mean / max, structural deltas (kept / lost), peak RAM and time, warnings emitted vs losses observed. The last
column checks the "no silent loss" claim: **a loss observed without a warning is a bug**, and is listed.

**Sets:**
1. The synthetic Office-16 set (`spikes/fidelity/gen_office.ps1`, 10 files), plus new fixtures: odt/rtf/html/md/txt
   written by Word; xlsb and ods written by Excel; a .ppt; the PDF → DOCX fixtures of §3.4; and the Phase 2 translit
   fixtures.
2. `corpus/` **if the owner delivers it** (B-14; still missing), run with ids instead of names (`run.py` already
   anonymises).

**References:**
- **MS Office 16 via COM** (installed here; `spikes/fidelity/office_com.ps1`). Primary. For a direction X → Y:
  - Office converts the source to Y itself (Word `SaveAs2` odt/rtf/html/txt/pdf; Excel `SaveAs` CSV with
    `Local:=False` for an apples-to-apples en-US CSV, and `ExportAsFixedFormat` PDF; PowerPoint `SaveAs` PDF/PNG);
  - our Y is opened in Office (for editable Y) → PDF;
  - both are compared with the Office PDF of the source.
- **LibreOffice** (our soffice image, §1.6): the same source → Y and → PDF as a second reference. The test is a
  separate process, so it is licence-safe.
- **ONLYOFFICE x2t** in a local `onlyoffice/documentserver` container (pinned tag, JWT off, never shipped), through
  its conversion API (`POST /converter`, fields `filetype, key, outputtype, url`;
  [ONLYOFFICE conversion API](https://api.onlyoffice.com/docs/docs-api/additional-api/conversion-api/request/)). If
  feasible within 0.5 d in WP9; if not, the report says so.
- All PDFs are rasterised by the same `pdf-cli render` at 96 dpi, as in Phase 0.

**Metrics by target kind:**
| Target kind | Metric |
|---|---|
| pdf, png | page count; text similarity; pixel diff vs the Office PDF (Phase 0 method) |
| editable (docx, odt, rtf, xlsx, pptx) | Office reopen ok; Office → PDF pixel diff and text similarity; structural counts before/after (`office_com.ps1`) |
| text (html, md, txt, csv) | text similarity vs Office's own export; csv cell-by-cell equality vs Excel CSV (`Local:=False`), plus the en-US vs ru separator difference measured separately |
| PDF ops | per-page render hash equality (lossless), page count |
| pdf → docx | §3.4 |
| translit | Phase 2 preserve checks on the converter's outputs |

The harness is promoted to `scripts/fidelity/` (`run.py`, `report.py`, `office_com.ps1`, `gen_office.ps1`), because
spikes are throwaway by rule. The `libreoffice_reference` and `x2t_reference` hooks (`spikes/fidelity/run.py:123-131`)
are implemented. Candidates are run **through `convert`**, so the report measures the product, not the raw CLIs.

---

## 8. Work packages

Tiers: **scout** = haiku, **builder** = sonnet, **architect** = opus. The machine has **31.8 GB RAM**, so **two
cargo builds may run in parallel**, which needs the WP0 change to the build lock. Docker builds and Office COM runs
count as one build slot each.

### WP0. Kick-off and re-verification
- **Goal:** start from facts after the Phase 2 gate.
- **Steps:**
  - create branch `phase-3` from `phase-2`;
  - re-grep every `file:line` in §0–§4;
  - turn `scripts/lib/lock.sh` into a 2-slot lock (`BUILD.lock.1`, `BUILD.lock.2`; one per target dir) and raise the
    RAM gate accordingly;
  - start the Docker engine (F11) and record `docker version`;
  - check that the domain CLIs exist at HEAD (`scripts/build.sh all cli --release`);
  - add `[apps.convert]` to `brand/brand.toml`.
- **Files:** `docs/phase3/wp0-notes.md`, `scripts/lib/lock.sh`, `brand/brand.toml`.
- **Done-check:** two `build.sh` runs in parallel both finish; `docker run --rm hello-world` works;
  `cargo xt brand check` OK; every reference marked confirmed or moved.
- **Estimate:** 0.5 d. **Tier:** scout (re-grep), builder (lock, brand).

### WP1. Direction census
- **Goal:** today's truth for every row of §1 before the design freezes: pass/fail, time, peak RAM, stderr warnings.
- **Steps:**
  - a throwaway script (`scripts/fidelity/census.ps1`) runs each direction of §1 once on the synthetic set with the raw
    domain CLIs, recording the exit code, wall time and peak working set;
  - docx → png: the pdf route vs `docs-cli render --page N` on one 4-page file (pixel diff between the routes, F4);
  - pdf → docx with pdfcraft native (F3) on 2 Word-made PDFs;
  - the timeouts of §2.5 are re-derived from the measured times (3× the measured p95, rounded).
- **Files:** `docs/phase3/census.md`.
- **Done-check:** every §1 row has a measured status; §2.5's defaults are replaced by measured ones.
- **Estimate:** 1 d. **Tier:** builder runs, scout gathers.

### WP2. `convert` core: planner, executor, CLI
- **Goal:** §2.1–§2.5 and the exit codes of §2.6.
- **Steps:**
  1. `services/` workspace, `ci/deny/services.toml` (copy of the root policy);
  2. kind detection (magic first), with a unit test per kind, including a mislabelled file;
  3. the planner table and `convert formats --json`; a table test: every (kind, target, options) → argv, golden;
  4. the executor: job dir, cleared env, timeouts, memory watchdog, output validation, cleanup guard;
  5. **fault injection with a fake tool:** a test-only bin `convert-fake-tool` that sleeps, exits 1, panics, aborts,
     allocates 4 GB, or writes an empty or truncated file; each must produce its exit code (1/4/5/6/8) and a message;
  6. the PDF subcommands and the `-j` batch.
- **Files:** `services/{Cargo.toml,Cargo.lock}`, `services/convert/**`, `ci/deny/services.toml`.
- **Done-check:**
  - `cargo nextest run --manifest-path services/Cargo.toml` passes, including the fault-injection tests;
  - `cargo deny … services` passes;
  - `convert check` finds the 4 CLIs;
  - each §1 direction without a sidecar runs once through `convert` on the synthetic set (script in WP9).
- **Estimate:** 3.5 d. **Tier:** builder; an architect reviews the job model and the planner table (0.25 d).

### WP3. Warnings and validation (no silent loss)
- **Goal:** §2.6: upstream warning capture, the input scanner, the static loss table, `--verify`, the report schema.
- **Steps:**
  - the scanner over the zip central directory (and the zip-bomb checks of §5.5, shared code);
  - the loss table as data (`services/convert/data/losses.toml`), each row citing its evidence (fidelity-baseline,
    census, WP9);
  - warning codes added to `i18n/converter.tsv`;
  - tests: one fixture per warning code, plus "a plain letter → pdf gives no warning".
- **Files:** `services/convert/src/{scan,losses,report}.rs`, `services/convert/data/losses.toml`, `i18n/converter.tsv`.
- **Done-check:** the tests pass; on the synthetic set, `.doc → docx` warns comments and revisions, `.xls → xlsx`
  passes the upstream warnings through, and `xlsx → csv` warns en-US.
- **Estimate:** 1.5 d. **Tier:** builder.

### WP4. Transliteration option
- **Goal:** §4.
- **Steps:**
  - the planner rows for translit on all three apps and on the pdf chain;
  - capture the command's result JSON into the report;
  - the `translit_nothing` warning;
  - the one-line `PARAMS` fix in `docs-translit`;
  - end-to-end tests that reuse the Phase 2 fixtures and comparers.
- **Files:** `services/convert/src/plan.rs`, `domains/docs/crates/docs-translit/src/lib.rs:28`, tests.
- **Done-check:** `convert in.docx --to docx --translit cyrl` on the 9 Phase 2 docx fixtures gives the same result
  as the Phase 2 `preserve` test (non-text parts byte-identical); the same for 2 xlsx and 2 pptx via
  `preserve-check.sh`; `--alphabet 2026` round-trips the Phase 2 2026 rows.
- **Estimate:** 1 d. **Tier:** builder.

### WP5. PDF → DOCX sidecar
- **Goal:** §3.
- **Steps:**
  1. `sidecars/pdf2docx/` with the Dockerfile, `server.py`, `worker.py`, `requirements.lock` (hashes),
     `LICENSES.md`, `SOURCES.md`, and `merge_lines.py` with a sha check;
  2. `scripts/sidecar-sources.sh`;
  3. `docker build`; the run script with limits;
  4. Python tests (stdlib `unittest`, run **inside** the image) for the error codes, the timeout, 503 when busy, and
     the scanned and encrypted cases;
  5. the `Http` step in `convert`, with a contract test against a fake server.
- **Files:** `sidecars/pdf2docx/**`, `scripts/sidecar-sources.sh`, `services/convert/src/http.rs`.
- **Done-check:**
  - `curl /healthz` shows the pinned versions;
  - a Word-made Cyrillic PDF converts, with `merged > 0`;
  - the scanned PDF gives 422 `no_text_layer` and `convert` exit 1 with that message;
  - an architect signs off the licence table (§3.1) in `docs/phase3/licences.md`.
- **Estimate:** 2 d. **Tier:** builder; architect licence review (0.25 d).

### WP6. LibreOffice sidecar (.ppt), conditional on O2
- **Goal:** §1.6. It also serves WP9 as the LibreOffice reference renderer, so the image is built even if O2 = no
  (test-only use).
- **Steps:**
  - `sidecars/soffice/` (`debian:bookworm-slim` pinned by digest + the Debian `libreoffice-impress`/`-writer`/`-calc`
    packages at pinned versions);
  - the same `server.py`, with a worker using a per-job profile; route `POST /v1/ppt2pptx`;
  - the planner rows for `.ppt`.
- **Files:** `sidecars/soffice/**`.
- **Done-check:** the synthetic `hrm_deck.ppt` → pptx → pdf has 5 slides; it is in the fidelity table with its pixel
  diff vs PowerPoint; two parallel jobs both succeed.
- **Estimate:** 1 d. **Tier:** builder.

### WP7. HTTP API
- **Goal:** §5.
- **Steps:**
  - axum routes;
  - multipart streaming to disk with limits;
  - the queue with the CPU and memory gates;
  - the polling status;
  - the zip;
  - the sweeper;
  - JWT (ported from spike 4, with its tests);
  - the security tests:
    - a traversal name such as `..\..\x.docx`;
    - a bidi file name;
    - a zip bomb (a 42 KB → 4 GB fixture generated in the test);
    - oversize;
    - an expired, tampered or `alg=none` token;
    - local mode works end to end with no token, and `auth = "none"` on a non-loopback address refuses to start;
    - another user's job id;
    - the queue full;
    - a cancel that kills a running step;
  - an API e2e test that runs **every direction of `GET /formats`** on one synthetic file each.
- **Files:** `services/convert-api/**`, `docs/phase3/api.md`.
- **Done-check:** the tests pass; the e2e matrix test is green; after the e2e run plus a forced kill of the server,
  the next start leaves `work/` empty (the sweeper).
- **Estimate:** 3 d. **Tier:** builder; architect security review (0.5 d).

### WP8. UI page and catalog
- **Goal:** §6.
- **Steps:**
  - the page;
  - `converter.tsv` drafts, with uz-Cyrl via `uz-translit`;
  - the `i18n export … --format json` target with an xtask test;
  - opus review of the uz rows;
  - the Playwright spec and the 4-language screenshots.
- **Files:** `services/convert-api/ui/**`, `i18n/converter.tsv`, `xtask/src/i18n/export.rs`,
  `ci/web-smoke/converter.spec.ts`, `docs/phase3/evidence/ui/`.
- **Done-check:** `scripts/ci.sh i18n` green; the Playwright spec passes (including the failing-file and
  rejected-file cases, which must show their messages); screenshots in en/ru/uz-Latn/uz-Cyrl with no tofu.
- **Estimate:** 2 d. **Tier:** builder; architect for the Uzbek review.

### WP9. Fidelity harness for directions
- **Goal:** §7, the machinery.
- **Steps:**
  - promote `spikes/fidelity` to `scripts/fidelity`;
  - new fixture generators (odt/rtf/html/md/txt via Word, xlsb/ods via Excel, .ppt, the §3.4 PDF → DOCX fixtures);
  - the direction runner through `convert --json`;
  - the Office COM reference per direction;
  - the LibreOffice reference (WP6 image);
  - x2t (time-boxed to 0.5 d);
  - the PDF → DOCX metrics;
  - the "loss without warning" check.
- **Files:** `scripts/fidelity/**`.
- **Done-check:** one command, `python -I scripts/fidelity/run.py --directions all --out E:/office-target/p3/fidelity`,
  produces the per-direction JSON for the synthetic set.
- **Estimate:** 3 d. **Tier:** builder.

### WP10. Fidelity runs and report
- **Goal:** `docs/phase3/fidelity.md`: one table per direction, the PDF → DOCX candidate comparison (input to O8),
  the warnings-vs-losses list, and the corpus run if delivered.
- **Done-check:** every direction in `GET /formats` has a table row with measured numbers or an explicit "not
  measurable because …"; every "loss without warning" is either fixed in WP3's table or listed as a defect.
- **Estimate:** 1.5 d. **Tier:** builder runs; architect writes the conclusions.

### WP11. Linux build and the converter image, conditional on O1
- **Goal:** prove the converter runs on a Linux server (F9).
- **Steps:**
  - build the four domain CLIs and `services/` in a `rust:1.97-bookworm` container (pinned digest; the source copied
    into a volume, not bind-mounted, for speed; `CARGO_NET_GIT_FETCH_WITH_CLI=true`);
  - a runtime image with a font set (Carlito, Caladea, Liberation, the U+02BB fallback font; all OFL/Apache;
    Microsoft fonts never);
  - `compose.yaml` with the three containers on internal networks;
  - re-run the WP9 synthetic directions in the container and report the pixel diff of Linux vs Windows per direction.
- **Files:** `services/Dockerfile`, `compose.yaml`, `docs/phase3/linux.md`.
- **Done-check:** `docker compose up` → the UI converts the WP7 e2e matrix; `linux.md` has build times, image sizes
  and the Linux-vs-Windows diff table.
- **Estimate:** 2.5 d (high 5: the first Linux build of four engines is an unknown). **Tier:** builder; architect if
  the build breaks inside upstream.

### WP12. CI, register, docs
- **Goal:** everything runs in CI.
- **Steps:**
  - `ci.sh` jobs:
    - `convert` (services tests + deny);
    - `convert-e2e` (API matrix on the synthetic set, release CLIs);
    - `sidecars` (docker build + the in-image tests; nightly and on changes under `sidecars/`);
    - `fidelity-directions` (nightly);
  - add `quick` triggers on `services/**`;
  - mirror the workflow; actionlint;
  - the README usage section; `docs/UPSTREAM.md`: the CLI flags we depend on (a sync that changes them must fail
    `convert-e2e`, not production).
- **Done-check:** `scripts/ci.sh quick` and `full` green.
- **Estimate:** 1 d. **Tier:** builder.

### WP13. Gate: adversarial review and report
- **Goal:** prove the three gate items; report to the owner in Uzbek.
- **Steps:**
  - an architect review: security (§5.5 list), licences (§3.1), "never silent" (break each tool and sidecar on purpose
    through CLI, API and UI and check the message reaches the user), the matrix vs the brief;
  - `ci.sh full`;
  - a sync dry run (do the CLI flags still hold?);
  - update STATUS.
- **Files:** `docs/phase3/gate-review.md`, `docs/STATUS.md`.
- **Done-check:** `ci.sh full` exit 0; every gate row in §9.3 has an evidence link.
- **Estimate:** 1.5 d. **Tier:** architect.

### WP14. gridcraft NumLocale (owner decision O7)
- **Goal:** gridcraft is locale-aware for display, typed input, CSV and PDF, per `docs/phase1/locale-check.md` §5:
  - uz and ru use a decimal comma, NBSP thousands grouping, the `;` list separator and localized month names;
  - `12.10.2026` typed in a uz/ru locale is 12 October 2026, not 10 December;
  - en-US behaviour stays byte-identical.
- **Steps:**
  - a design note (which locale source, where it threads through, file storage stays invariant) reviewed by an
    architect before code;
  - carried gridcraft patches plus REGISTER rows;
  - tests per locale for parse, display, CSV and PDF, plus an en-US no-change test;
  - the xlsx round-trip keeps the stored values invariant, so a file saved in uz opens identically in Excel.
- **Files:** `patches/gridcraft/*`, `docs/phase3/numlocale.md`. The F6/R7 warning in WP3 is dropped for locales the
  patch covers.
- **Done-check:**
  - the `locale-check.md` §2 matrix is re-measured, with uz and ru rows correct;
  - Excel opens a uz-saved file with identical values (COM check);
  - `ci.sh quick` is green.
- **Estimate:** 6–8 d. **Tier:** architect (design and review), builder (patches). Stream E runs in parallel with
  A–C on the second cargo slot.

### WP15. XDFU handling in the converter (§10)
- **Goal:** an XDFU-marked document is never converted silently or anonymously.
- **Steps:**
  - **Detect the grif.** Look for "XDFU", "ХДФУ", "ДСП" or "Для служебного пользования" in the upper-right of page 1,
    in a header, or in the document properties. The result goes in the job report as `xdfu: true`, with the evidence.
  - **Watermark** every output of an XDFU input, per cl. 12: copy number, the client's network address, the user
    (`sub` in server mode, the OS user in local mode), and date and time. Text outputs (csv, txt, md) get a header line.
    Any direction that cannot carry a watermark is refused with a clear message.
  - **Audit log** (append-only JSON lines, hash-chained) of every XDFU job: who, when, from where, which copy number.
    No content and no file names when `--redact` is on.
  - **Policy (owner, 2026-10-21): XDFU support is opt-in and OFF by default.** The product is open source and most
    users don't need it. The setting is `xdfu = "off" | "watermark" | "refuse"`, in the server config and the local
    settings. When it is `off`, there is no detection, no watermark, no audit log and no XDFU UI. When it is on, the
    operator picks watermark or refuse. The UI shows the setting only to whoever controls the config: the local user
    or the server operator.
  - Server mode with `auth = "none"` is already impossible on the network (§5.5).
- **Files:** `services/convert/src/xdfu.rs`, `services/convert/src/audit.rs`, tests, `i18n/converter.tsv` (warnings).
- **Done-check:**
  - with `xdfu = "off"` (the default), a grif document converts exactly like any other: no watermark, no audit
    entry, byte-identical output;
  - a grif fixture per language (uz-Latn, uz-Cyrl, ru) is detected, and a plain document is not;
  - every output direction either carries the watermark (text extracted from the output) or is refused;
  - the audit chain verifies, and tampering with one line breaks it.
- **Estimate:** 2 d. **Tier:** builder; architect review together with the WP7 security review.

### WP16. 2026 Latin alphabet adoption
- **Context:** the owner reports the new alphabet law (Ş Ç Ö Ğ replacing sh, ch, oʻ, gʻ) was approved on
  2026-10-21. Phase 2 already supports it behind `latinAlphabet = 2026`, from the Senate text (`docs/phase2/rules.md`
  §15). The default stays 1995.
- **Steps:**
  - Once the signed text is published on lex.uz, diff it against rules.md §15: letters, case pairs, the separating
    sign and its code point, the `ng` status, and the entry-into-force date and transition rules. Fix any mismatch,
    with golden rows, `reviewed:agent-*`.
  - Generate a **uz-Latn-2026 UI catalog** from uz-Cyrl through `uz-translit` (Y2026), with no new translation work.
    The language menu then offers "Oʻzbekcha (1995)" and "Oʻzbekcha (2026)".
  - Prepare the default switch (O13) as a one-line config change.
- **Done-check:** the §15 diff is recorded with the source link; the 2026 catalog passes lint; the UI screenshot shows
  2026 Latin with no tofu (the Noto fallback covers Ş Ç Ö Ğ).
- **Estimate:** 1 d. **Tier:** builder; opus for the text diff.

### Parallel streams
```
day:        1    2    3    4    5    6    7    8    9   10   11   12   13   14   15
A Rust      WP0 ▌WP2 █████████▌WP3 ████▌WP4 ██▌WP7 ████████▌WP8 █████▌         WP13 ███▌
B Docker         WP5 █████▌WP6 ██▌          WP11 ███████▌(O1)
C Fidelity     WP1 ██▌         WP9 ████████▌                 WP10 ████▌
D CI                                                    WP12 ██▌
```
- **Critical path:** WP0 → WP2 → WP3 → WP7 → WP8 → WP13. That is **0.5 + 3.5 + 1.5 + 3 + 2 + 1.5 = 12 stream-days**,
  so about 13 working days with integration slack. WP4 slots in between WP3 and WP7 (or in parallel with WP7's
  first day).
- Stream B needs Docker only, no cargo, so it runs beside two cargo builds.
- Stream C needs the release CLIs (built once in WP0), Office COM and the WP6 image. **WP10 needs WP5** (pdf → docx
  rows) **and WP4** (translit rows).
- Two cargo builds at a time at most: stream A (services, small) plus one domain rebuild (WP4's docs-translit line,
  WP11's Linux build).

---

## 9. Risks, schedule, gate mapping

### 9.1 Estimate (stream-days)
| WP | Low | Likely | High |
|---|---|---|---|
| WP0 | 0.25 | 0.5 | 1 |
| WP1 | 0.75 | 1 | 1.5 |
| WP2 | 2.5 | 3.5 | 5 |
| WP3 | 1 | 1.5 | 2 |
| WP4 | 0.5 | 1 | 1.5 |
| WP5 | 1.5 | 2 | 3 |
| WP6 | 0.5 | 1 | 2 |
| WP7 | 2 | 3 | 4.5 |
| WP8 | 1.5 | 2 | 3 |
| WP9 | 2 | 3 | 4 |
| WP10 | 1 | 1.5 | 2 |
| WP11 | 1.5 | 2.5 | 5 |
| WP12 | 0.5 | 1 | 1.5 |
| WP13 | 1 | 1.5 | 2 |
| **Total** | **16.5** | **25** | **38** |

Without WP11 (O1 = no): 22.5 likely. Calendar with three streams: **about 13–15 working days likely, 20 high**.

### 9.2 Risks
| # | Risk | Likelihood / impact | Mitigation | Proven by |
|---|---|---|---|---|
| R1 | The upstream CLIs do not build or behave the same on Linux (F9): fonts, paths, locale | medium / deployment | WP11 measures it; the Windows dev deployment works regardless; the owner decides deployment (O1) | WP11 |
| R2 | An engine hangs or crashes on real files (upstream lists open crash and hang reports) | high / per job | Subprocess per step, timeout, watchdog, exit 6 with a message; fault injection | WP2, WP7, WP13 |
| R3 | gridcraft memory peaks exhaust the server | medium / server | Memory-weighted admission; refuse a job that can never fit; cgroup backstop | WP7 |
| R4 | AGPL obligations for the sidecar image are missed when it is distributed | low if planned / legal | `LICENSES.md`, `SOURCES.md`, source bundle per tag; no AGPL modifications; Phase 3 never publishes | WP5, WP13 |
| R5 | `merge_lines.py` has no licence statement | certain until answered / shipping | O4; run on this PC only until answered | WP5 |
| R6 | pdf2docx quality on HRM Cyrillic PDFs is worse than expected | medium / feature | Measured against pdfcraft native, Word reflow and x2t; the owner picks (O8); weak spots warned per file | WP9, WP10 |
| R7 | CSV and PDF show en-US numbers and dates (F6) | certain / users | A warning on every such job; NumLocale patch (6–8 d) is O7 | WP3 |
| R8 | No corpus: every number is synthetic (Office-16-written) | certain | Office COM fixtures; re-run the one command when `corpus/` arrives | WP10 |
| R9 | LibreOffice and the Craft parsers on hostile files (CVE history) | medium / security | No egress, read-only root, tmpfs, limits, non-root, per-job profile; upload size limits; auth | WP6, WP7, WP11 |
| R10 | An upstream sync changes CLI flags or messages (high velocity) | high / weekly | `convert-e2e` in CI runs every direction; the flags we depend on are listed in UPSTREAM.md | WP12 |
| R11 | Docker Desktop licensing for a large organisation (F11) | medium / cost | Docker Engine on Linux or inside WSL2; O5 | WP0 |
| R12 | The docx → png route rasterises wordcraft's PDF, not wordcraft's own renderer (F4) | low / fidelity | WP1 measures the difference; fall back to `render --page N` per page if it is large | WP1 |
| R13 | x2t is too heavy or too slow to wire | medium / report completeness | Time-boxed; the report says "not run" if so; Office and LibreOffice references suffice for the gate | WP9 |
| R14 | Polling misses fast jobs or creates load | low | 1 s interval, ETag/304 on unchanged status | WP7 |
| R15 | Phase 2 gate not yet passed | — | Phase 3 starts only after the owner's "go" | — |

### 9.3 Exit-gate items → proving WP
| Gate item (brief) | Evidence | WP |
|---|---|---|
| Every supported direction works from **CLI** | the planner table test; the synthetic run of every direction through `convert` (`docs/phase3/census.md` → `fidelity.md`) | WP2, WP4, WP5, WP6, WP10 |
| … from the **API** | the API e2e test over all of `GET /formats` (CI `convert-e2e`) | WP7, WP12 |
| … from the **UI** | the Playwright spec (one batch per input family + translit + failure cases), 4-language screenshots | WP8 |
| **The fidelity table exists** | `docs/phase3/fidelity.md`: one table per direction, Office COM and LibreOffice references (x2t if feasible), PDF → DOCX candidates | WP9, WP10 |
| **Failures are reported, never silent** | fault-injection tests (CLI and API); output validation; the "loss without warning" check; the gate review breaking each tool and sidecar on purpose and checking CLI, API and UI | WP2, WP3, WP7, WP8, WP10, WP13 |
| Brief task 2: PDF → DOCX sidecar, isolated, never linked | the image, the licence table, `cargo deny` (no AGPL crate in any lock) | WP5 |
| Brief task 3: transliteration as an option, formatting kept | the converter route equals the Phase 2 preserve results | WP4 |
| Brief task 4: batch upload, queue with progress, zip, limits, timeouts, cleanup | the API tests | WP7 |

### 9.4 What the owner must decide or provide
| # | Decision | Recommendation |
|---|---|---|
| **O1** | **Deploy the Docker images (converter, pdf2docx, soffice) on the HRM test server?** Who operates them; which host (`<test-host>`?); whether this is allowed before Phase 5. | Yes, on the test server only, after the gate, in server mode, with the HRM configured as the token issuer. It needs WP11. Until then the converter runs on this PC for evaluation. |
| **O2** | **The .ppt policy (B-8):** LibreOffice sidecar / no .ppt. How common are .ppt files? | Sidecar, if any are in use: the licence and clean-room are fine (§1.6), and the cost is about 1 d plus an image of about 1 GB. |
| **O3** | **File size limits:** per file, per batch, PDF pages, retention. | 50 MB per file (xlsx 100 MB), 30 files / 300 MB per batch, 500 PDF pages, 200 PNG pages, outputs deleted 1 h after done. |
| O4 | The licence of `merge_lines.py` (the owner's code) | MIT OR Apache-2.0, like our layer, so the image can ship it |
| O5 | Docker Desktop's licence for the organisation, or Docker Engine (WSL2/Linux) | Docker Engine on Linux for servers; on this PC, whatever the organisation is licensed for |
| O6 | Who may use the converter | **Decided (revised 2026-10-21):** anyone. Local mode needs no account. Server mode is optional and checks tokens from an operator-configured issuer (the HRM for our deployment). Anonymous access over the network stays off. |
| O7 | Fund the gridcraft NumLocale patch (6–8 d, `locale-check.md` §5) so CSV and PDF show `3 500 000,50` | Yes, before Sheets output is used for ru/uz documents; Phase 3 ships with the warning meanwhile |
| O8 | The default PDF → DOCX engine after WP10: pdf2docx sidecar (AGPL container) or pdfcraft native (permissive, built in) | Decide on the WP10 numbers |
| O9 | Deliver `corpus/` (B-14) | Still the biggest gap in every fidelity claim |
| O10 | Will XDFU documents go through the converter or our server at all, or only the HRM's own registry system? | Plan for yes (WP15). Before the HRM test-server deploy, the organisation's registry (devonxona) and security unit confirm it. |
| O11 | Which e-signature (EDS) for the cl. 34 QR + signature: E-IMZO? Does the organisation require certified national crypto (O‘z DSt) inside our server? | E-IMZO as a Phase 5 plugin. National crypto via a certified provider module, not our own implementation. |
| O12 | Who submits the deployed system for the cybersecurity expertise (cl. 11)? | The organisation, with the HRM. We deliver the security documentation pack (Phase 10). |
| O13 | 2026 alphabet: when does the default switch from 1995 to 2026? | On the law's entry-into-force date, once the published text is confirmed (WP16). Until then 2026 stays an option. |

---

## 10. XDFU ("Xizmat doirasida foydalanish uchun") documents

**Source:** Cabinet of Ministers Resolution No. 766 of 04.12.2025 and its Rules (lex.uz/docs/7883108), read on
2026-10-21. The Rules enter into force 3 months after publication. Clause numbers below are the Rules'.

**What the law requires of an electronic system, and who provides it:**

| Requirement | Clause | Our software | The organisation |
|---|---|---|---|
| Handle XDFU only in e-document systems that passed the **cybersecurity expertise** | 11, Resolution 2 | Security docs and a hardened server mode, so the deployed system can pass | Submits the deployed system (O12) |
| **Cryptographic protection and EDS** are the basic conditions | 11 | TLS + at-rest encryption (§5.5); a crypto-provider seam for certified national algorithms (O11) | Chooses the certified provider and EDS |
| Network exchange only via protected e-mail or expertised e-doc systems | 11 | No egress, no third-party origins; HTTPS only in server mode | Network design |
| Save, download, copy text and print are **restricted**; each electronic copy strictly identifiable | 11, 38 | A per-document policy: print, export and copy are blocked or need approval when the grif is present | Approvals (unit or registry head) |
| Every copy, print or download carries a **watermark**: copy number, the computer's network address, the user profile, date and time | 12 | Watermark on every output of an XDFU-marked input (WP15); in the editors in Phase 4 | — |
| The grif goes in the **upper-right corner of page 1**, with the copy number under it | 32 | Detection (WP15); an "Insert XDFU grif" command in Phase 4 | Marking decisions |
| E-documents are signed with an **EDS** and carry a **QR** (issuing body, number and date, signer) | 34 | Phase 5 plugin (E-IMZO, O11) | — |
| The registration index gets "-XDFU" | 36 | HRM registry side; our docserver passes it through | — |
| Storage only in protected systems; never taken off the premises | 41, 42 | Server mode with encryption at rest; local mode warns that the file is XDFU | Policy and endpoint control |
| Grif removal by a commission, reviewed every 2 years | 50–53 | Phase 5: the grif is removable only via the registry workflow | Commission |

**On/off (owner, 2026-10-21):** everything in this section is **opt-in and off by default** (`xdfu = "off"`), in
every phase: the converter, the editors and the docserver. An organisation that needs it switches it on. The general
web security baseline of §5.5 is not part of this switch; it is always on.

**Limits, said plainly.**
- Our open-source code cannot be certified on its own. The expertise covers the deployed system: our server, the
  HRM, the network and procedures.
- The resolution does not specify encryption algorithms, a log retention period, or the grif's exact electronic
  format. Where it is silent we follow the general cybersecurity regulations and record the choice.
- Local mode on an uncertified PC cannot make XDFU handling compliant. It only watermarks and warns.

**Phase mapping.**
- Phase 3, WP15: detection, watermark, audit log, policy hooks in the converter.
- Phase 4: the grif command and print, export and copy restriction in the editors.
- Phase 5: EDS and QR, registry integration, grif removal workflow, at-rest storage in the docserver.
- Phase 10: the security documentation pack for the expertise.

---

[inventory]: ../phase0/inventory.md
[fidelity-baseline]: ../phase0/fidelity-baseline.md
[gridcraft-memory-r2]: ../phase0/gridcraft-memory-r2.md
[STATUS]: ../STATUS.md
