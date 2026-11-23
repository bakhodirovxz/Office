# Hoshiya: an open-source office suite for Uzbek

Hoshiya is a free office suite: **Documents**, **Spreadsheets**, **Presentations** and a **PDF viewer**. It runs on Windows 10/11, in
a web browser and (as a preview) on Android. It opens and saves the Microsoft Office formats (.docx, .xlsx, .pptx), needs no account and
no internet connection, and has Uzbek built in: a Latin <-> Cyrillic transliterator that already knows the **2026 Latin alphabet**
(ö ğ ş ç), and a user interface in Uzbek Latin, Uzbek Cyrillic, Russian and English. An add-in brings the same transliteration to
Microsoft Word, Excel and PowerPoint.

Open source under MIT OR Apache-2.0. **Status: preview / pre-release. Expect rough edges** (see [Project status](#project-status-and-roadmap)).

**[Code signing policy](CODE-SIGNING-POLICY.md)**: free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/) (applied for; releases are unsigned until it is granted).

## Contents

1. [Why Hoshiya exists](#why-hoshiya-exists)
2. [What is in it](#what-is-in-it)
3. [Download and install](#download-and-install)
4. [Project status and roadmap](#project-status-and-roadmap)
5. [How it is built](#how-it-is-built)
6. [Building from source](#building-from-source)
7. [Testing](#testing)
8. [Contributing](#contributing)
9. [Licence, brand and acknowledgements](#licence-brand-and-acknowledgements)

## Why Hoshiya exists

Organisations in Uzbekistan mostly depend on proprietary office software, and Uzbek text constantly moves between two scripts
(Latin and Cyrillic). The Latin alphabet itself is changing: the 2026 reform replaces `oʻ`, `gʻ`, `sh`, `ch` with `ö`, `ğ`, `ş`, `ç`.
Hoshiya aims to give such organisations an office suite that is:

- **Open source and free**, with permissive licences only, so it can be audited, adapted and self-hosted.
- **Uzbek-first**: transliteration and the 2026 alphabet are part of the product, not an afterthought.
- **Offline and account-free**: local mode needs no login and no server. Connecting to a server is optional.
- **Compatible**: it reads and writes the Microsoft Office file formats that offices already use (Office 2010 and newer files).
- **Simple**: an interface meant for non-technical office staff, in four UI languages.

## What is in it

### The four editors

| App | What it is | Formats |
|---|---|---|
| **Hoshiya Docs** | Word processor | opens and saves .docx; imports .doc, .odt, .rtf, .html, .md, .txt; exports PDF |
| **Hoshiya Sheets** | Spreadsheet | opens and saves .xlsx; imports .xls, .xlsb, .ods |
| **Hoshiya Slides** | Presentations | opens and saves .pptx; exports PDF and PNG |
| **Hoshiya PDF** | PDF viewer and editor | view, text selection and copy, combine, split, extract |

These are the Craft engines (see [How it is built](#how-it-is-built)) with our layer on top: fallback fonts and dictionaries for
Uzbek, UI catalogs in four languages, the transliteration commands, a version history, and one shared shell that starts any of the
apps. On Windows the apps run natively; the same editors also run in a browser as WebAssembly. The [converter](#converter-command-line-and-http-api)
handles more formats (csv and others) than the editors import directly.

### Uzbek transliteration

The `uz-translit` crate converts text between Uzbek Latin and Cyrillic. It is built into Docs, Sheets and Slides (and into the
converter and the Office add-in) as the "Latin to Cyrillic" and "Cyrillic to Latin" commands, and it keeps the formatting.

- Going to Cyrillic, it reads both the 1995 Latin alphabet and the 2026 one (`ö ğ ş ç`, including decomposed forms).
- Going to Latin, it can write the 2026 letters. With nothing selected, "Cyrillic to Latin" converts the whole file and also rewrites
  1995 Latin in the 2026 letters. The reform was signed on 2026-10-21 and applies six months after official publication, so the
  default output alphabet is one switch in the crate and is still 1995.
- A paragraph or cell that looks English keeps its Latin letters.
- A reviewed exceptions dictionary ([crates/uz-translit/data/](crates/uz-translit/data/)) handles words that rules alone get wrong.
  The written rules cover the tutuq (ʼ), `ye`/`е`, `yo`/`ё`, `ts`/`ц`, `ь`/`ъ` and abbreviations such as AQSH and MChJ.
- The rules, with their sources, are in [docs/phase2/rules.md](docs/phase2/rules.md); a short pointer is in
  [crates/uz-translit/RULES.md](crates/uz-translit/RULES.md).
- The Uzbek UI strings, the golden test list and the exceptions were reviewed by an AI agent (marked `reviewed:agent-*`), not by a
  human linguist. **Wrong results are bugs: please report them** (there is an issue form for transliteration).

### Microsoft Office add-in

A separate small installer (about 330 KB) that adds its own ribbon tab to **Word, Excel and PowerPoint 2010 or newer** with the same
transliteration. It installs per user, without administrator rights, and does not need the suite. See [Download](#download-and-install).

### Converter: command line and HTTP API

`convert` turns documents into other formats and `convert-api` offers the same over HTTP: docx, doc, odt, rtf, html, md, txt, xlsx,
xls, xlsb, ods, csv, pptx, ppt and pdf into the formats `convert formats` lists, plus transliteration and PDF combine/split/extract.
The desktop installer ships it with the four editor CLIs, so it works offline. Server mode is HTTPS only, uses bearer tokens and
encrypts files at rest. Commands are under [Converter commands](#converter-commands-services); the full reference is
[docs/phase3/api.md](docs/phase3/api.md).

### Web version

The editors run in a browser as WebAssembly, loaded one at a time: opening Docs downloads only Docs. A small shell page starts the
apps. Each editor is one WebAssembly bundle of roughly 5 to 9.5 MB compressed with brotli (measured, see
[docs/STATUS.md](docs/STATUS.md)), and its size budget is enforced in CI ([web/budgets.toml](web/budgets.toml)). The Windows
installer also hosts this web shell locally inside a desktop window.

### Document server

`services/docserver` embeds the editors in other systems: open a document by URL, edit it, and save it back through a callback,
with signed tokens (JWT), version history, autosave and data encrypted at rest. It is built to be **drop-in compatible with systems
that already integrate the ONLYOFFICE Document Server API**: such a system can switch to it by changing server-side settings only
(a Docker image and an install script). See [packaging/docserver/](packaging/docserver/) and
[docs/phase5/dropin.md](docs/phase5/dropin.md). A scripted rehearsal of that switch (two scenarios) passed 67 of 67 checks on
2026-11-21. No ONLYOFFICE code is used: the API was built clean-room from its public documentation.

### Mobile

Mobile is **web-based** for version 1: the editors run in a phone browser with a phone layout and touch handles. An Android app
exists as a thin WebView host with the editors inside (local mode, no internet permission). It has been tested on an Android
**emulator** only, not on a real phone, and it is not published yet. iOS is not available (no Mac for development). See
[docs/phase6/android-app.md](docs/phase6/android-app.md).

### Plugins

Docs, Sheets and Slides can be extended with plugins: sandboxed WebAssembly modules that declare what they may touch (the document,
network, files) and add commands, buttons, menu items and panels. The same plugin file runs on desktop, web and Android. Two
first-party plugins are included: transliteration ([plugins/uz-translit](plugins/uz-translit)) and template fields
([plugins/template-fields](plugins/template-fields)); [plugins/template](plugins/template) is a copy-and-edit starting point.

- A plugin gets only the permissions it declares.
- Installing requires a SHA-256 pin; an Ed25519 signature is optional (unsigned plugins are allowed only in local mode).
- Plugins do not run on protected documents.
- A test with a deliberately malicious plugin (network or file access without permission) is part of CI.

Authoring and administration guide: [docs/plugins.md](docs/plugins.md).

## Download and install

| What | Version | Download | Size |
|---|---|---|---|
| Desktop suite (Windows 10/11, x64) | 0.7.0 | [hoshiya-0.7.0-x64.msi](https://github.com/bakhodirovxz/Office/releases/download/v0.7.0/hoshiya-0.7.0-x64.msi) | 231 MB |
| Office add-in (Word, Excel, PowerPoint 2010+) | 0.2.1 | [hoshiya-office-addin-0.2.1-x64.msi](https://github.com/bakhodirovxz/Office/releases/download/addin-v0.2.1/hoshiya-office-addin-0.2.1-x64.msi) | 328 KB |
| Windows keyboard for the 2026 Uzbek Latin alphabet (sh → ş, ch → ç, o' → ö, g' → ğ as you type) | 0.4.0 | [hoshiya-keyboard-0.4.0-x64.msi](https://github.com/bakhodirovxz/Office/releases/download/keyboard-v0.4.0/hoshiya-keyboard-0.4.0-x64.msi) | 332 KB |

Release notes and checksums: [suite 0.7.0](https://github.com/bakhodirovxz/Office/releases/tag/v0.7.0),
[add-in 0.2.1](https://github.com/bakhodirovxz/Office/releases/tag/addin-v0.2.1),
[keyboard 0.4.0](https://github.com/bakhodirovxz/Office/releases/tag/keyboard-v0.4.0) (each has a `SHA256SUMS` file).

- The installers are not signed yet, so Windows SmartScreen warns: "More info" > "Run anyway". Check the download against `SHA256SUMS`.
  Signing will be done by SignPath Foundation, see the [code signing policy](CODE-SIGNING-POLICY.md).
- The add-in installs per user without admin rights and adds its own ribbon tab. With nothing selected, "Cyrillic to Latin"
  converts the whole file and rewrites 1995 Latin in the 2026 letters (English paragraphs keep their Latin).
- The keyboard installs per machine (administrator rights) and appears as "Oʻzbek 2026 (ş ç ö ğ)" under Uzbek (Latin): switch
  with Win+Space; Ctrl+Shift+` turns it off and on; Backspace right after ş gives back sh. Source: branch `keyboard-release`.
- If Word disabled the add-in after a freeze in 0.2.0: File > Options > Add-ins > Manage: Disabled Items > Go > Enable.

**System requirements.** Desktop suite: Windows 10 or 11, 64-bit (the installer is large because it includes the offline converter).
Add-in: Office 2010 or newer. Web version: a current browser with WebAssembly. UI languages: uz-Latn, uz-Cyrl, ru and en (set in the
settings; the apps read it at start).

The installer registers the apps under "Open with" and does not change your default file associations. IT staff can find an optional
default-associations file in [packaging/windows/README.md](packaging/windows/README.md).

There are no screenshots in the repository yet.

## Project status and roadmap

Hoshiya is built in phases, each ending with a review gate. Current state, from [docs/STATUS.md](docs/STATUS.md):

| Phase | Scope | State |
|---|---|---|
| 0 | Research, spikes, architecture decision | done |
| 1 | The four editors in one workspace, brand, four UI languages, size budgets | done |
| 2 | Uzbek transliteration (rules, exceptions, 2026 alphabet) | done |
| 3 | Converter, CLI and HTTP API | done |
| 4 | Unified shell: web and Windows desktop, installer | in progress (gate open) |
| 5 | Document server and drop-in integration | in progress, in parallel |
| 6 | Mobile (web-based, Android app) | in progress, in parallel |
| 7 | Plugins (Docs, Sheets, Slides) | in progress, in parallel |

Not available yet: macOS, Linux and iOS builds; real-time co-editing; signed installers; a published Android app; a plugin
administration page. Priorities are fidelity with Microsoft Office files first, then mobile, then co-editing. Fidelity is checked
against reference renders and reported in the phase notes; no overall percentage is claimed here.

## How it is built

Hoshiya is **not** a rewrite. Its editors are the open-source **WordCraft, GridCraft, DeckCraft and PdfCraft** applications by the
ArtCraft team (Rust, drawn with egui, MIT OR Apache-2.0). We add thin layers around them and carry the smallest possible changes:

- `upstream/` is a pristine vendored snapshot of each engine and is never edited by hand.
- Every change to an engine is a numbered patch in `patches/<engine>/`, with its reason in
  [patches/REGISTER.md](patches/REGISTER.md). Builds apply the queue to the snapshot ([docs/UPSTREAM.md](docs/UPSTREAM.md)).
- Our own code is in new crates and services: transliteration, brand, shared shell logic, the plugin host, the converter and the
  document server.
- The product name and look live in one brand file, `brand/brand.toml`.
- On the web each editor is a separate, lazily loaded WASM bundle; budgets are checked by `scripts/ci.sh sizes`.
- Dependencies are permissive-only, checked by `cargo-deny`; third-party notices are generated for the installer.

| Path | What is there |
|---|---|
| `upstream/` | pristine vendored engines (never hand-edited) |
| `patches/` | the patch queue for each engine, plus `REGISTER.md` |
| `domains/` | build domains (`docs`, `sheets`, `slides`, `pdf`): one workspace per editor with our layer applied |
| `crates/` | our shared crates: `uz-translit`, `suite-core`, `plugin-*`, `office-brand` and others |
| `services/` | `convert`, `convert-api`, `docserver`, `suite-desktop`, `suite-serve` (serves the web shell) |
| `web/` | the web shell, the plugin host page, size budgets |
| `plugins/` | first-party plugins and the plugin template |
| `packaging/` | Windows MSI, Android host, document-server Docker image and installer |
| `sidecars/` | optional Docker sidecars (`pdf2docx`, `soffice`), used only over HTTP/CLI |
| `i18n/` | UI string catalogs (`.tsv`): uz-Latn, uz-Cyrl, ru, en |
| `assets/`, `brand/` | fonts, icons, dictionaries; the brand config and logos |
| `scripts/`, `ci/`, `xtask/` | build, packaging and CI tooling |
| `docs/` | plans, status, design notes, the architecture decision |
| `spikes/` | throwaway experiments |

Background reading: [docs/BRIEF.md](docs/BRIEF.md) (mission and constraints), [docs/ARCHITECTURE-DECISION.md](docs/ARCHITECTURE-DECISION.md)
and the phase plans `docs/phase1/PLAN.md` to `docs/phase7/PLAN.md`.

## Building from source

Most people only need the [installers](#download-and-install). To build, use `scripts/build.sh <docs|sheets|slides|pdf|all> <native|web|cli> [--release] [--msvc]`
for a domain and `scripts/build-msi.ps1` for the installer ([packaging/windows/README.md](packaging/windows/README.md)). The
developer setup below is for Windows 10 with Git Bash.

### Clean checkout (Windows 10, Git Bash)

Prerequisites (`scripts/bootstrap.sh` only checks them, it installs nothing):
- rustup with the `stable-x86_64-pc-windows-gnu` toolchain (MSYS2 ucrt64 gcc), the `wasm32-unknown-unknown` target, rustfmt and clippy;
- `cargo-deny`, `cargo-nextest`, `sccache`, `actionlint`, `trunk` and `brotli` on PATH (exact versions in `scripts/tools.lock`);
- Node with Playwright for the `web-smoke` job;
- network access for the `craft-fonts` fetch.

The scripts keep build output, caches and tools on a second drive: by default `E:/office-target`, `E:/office-cache`,
`E:/office-upstream` and `E:/office-tools/bin` ([scripts/env.sh](scripts/env.sh)). Set `OFFICE_TARGET` to put the build output
elsewhere; the other paths are set in `scripts/env.sh`.

```bash
git clone https://github.com/bakhodirovxz/Office.git hoshiya && cd hoshiya
export OFFICE_TARGET=E:/office-target/hoshiya   # optional; set it before the scripts source env.sh
scripts/bootstrap.sh                            # checks tools, creates the dirs, fetches craft-fonts (not in git), starts sccache
scripts/ci.sh full                              # the whole gate, hours when cold; one job: scripts/ci.sh <job> [args]
```
Notes:
- Run `scripts/ci.sh` with exclusive use of the machine: one rustc-heavy job at a time, a 3 GB free-RAM gate, and a build lock at `$OFFICE_TARGET/BUILD.lock`
  (remove a stale lock after a killed run).
- `bootstrap.sh` prints "sccache server did not start" when a server is already running; that is harmless.
- Results of a run: `$OFFICE_TARGET/ci/<run>/summary.md` (`--run <id>` names the run).
- The sccache cache is `E:/office-cache/sccache`, shared by all checkouts. It does not hit across checkouts at different paths (0 hits in the clean-checkout run).

### Converter commands (`services/`)

A document converter on the four domain CLIs: docx, doc, odt, rtf, html, md, txt, xlsx, xls, xlsb, ods, csv, pptx, ppt and pdf to the
formats `convert formats` lists, plus Uzbek Latin/Cyrillic transliteration (`--translit`) and pdf combine/split/extract. It links no
engine: it runs the release CLIs as processes. Build: `scripts/build.sh <docs|sheets|slides|pdf> cli --release` for each domain, then
`cargo build --release --manifest-path services/Cargo.toml` (binaries `convert` and `convert-api`). Point it at the CLIs with
`CONVERT_DOCS_CLI`, `CONVERT_SHEETS_CLI`, `CONVERT_SLIDES_CLI`, `CONVERT_PDF_CLI` or `CONVERT_TOOLS_DIR`. Full reference: [docs/phase3/api.md](docs/phase3/api.md).

```bash
# CLI: one file or a batch; exit code and --json report say what happened (0 ok, 3 unsupported, 10 warnings under --strict)
convert report.docx --to pdf
convert letter.docx --to docx --translit cyrl            # Latin to Cyrillic, formatting kept
convert book.xlsx --to csv --sheet all --out-dir out/
convert scan.pdf --to png --pages 1-2 --dpi 150
convert pdf combine a.pdf b.pdf -o all.pdf
convert formats --json                                    # every supported direction;  convert check: which tools are found

# Local server: loopback only, no login, plain HTTP; the page is served at the same address
convert-api                                               # http://127.0.0.1:8787/

# Server mode: HTTPS only, bearer tokens (HS256), files encrypted at rest; refuses to start without its keys
convert-api --config convert-api.toml                     # auth = "jwt", tls_cert, tls_key, secret_file, key_file
```

Optional sidecars (Docker): `sidecars/pdf2docx` (layout-preserving pdf to docx, `--pdf-engine sidecar`, `CONVERT_PDF2DOCX_URL`) and
`sidecars/soffice` (legacy formats). Tests: `scripts/ci.sh convert`, `convert-e2e`, `sidecars` (see [docs/phase3/ci.md](docs/phase3/ci.md)).

## Testing

`scripts/ci.sh <job>` runs one CI job and `scripts/ci.sh full` runs the whole gate: formatting and lint, licence and advisory checks,
tests of our crates and of each engine with our patches applied, web size budgets, browser smoke tests, plugin tests (including the
malicious-plugin test), the converter end to end, and notices. The job list is in the header of `scripts/ci.sh`; GitHub workflows are in
`.github/workflows/`.

## Contributing

Contributions are welcome: bug reports, testing on real documents, transliteration fixes, translations (uz-Latn, uz-Cyrl, ru, en)
and code. Start with [CONTRIBUTING.md](CONTRIBUTING.md), then [open an issue](https://github.com/bakhodirovxz/Office/issues/new/choose)
or look for issues labelled `good first issue`. Issues can be written in Uzbek, Russian or English. Never attach documents that
contain personal data.

Working notes for coding agents are in `CLAUDE.md`; plans and status are in `docs/`.

## Licence, brand and acknowledgements

- **Our code** (everything outside `upstream/`, except the third-party material and brand assets listed in [NOTICE](NOTICE),
  including the patches): MIT OR Apache-2.0, at your option
  ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
- **Upstream engines** keep their own licences and notices (MIT OR Apache-2.0); every change to them is a recorded patch.
- **One exception:** the `symphonia` audio crates are MPL-2.0. They are used unmodified and listed in [NOTICE](NOTICE) with a source
  link. Other third-party material, such as the Noto fonts (SIL OFL 1.1), is listed there too.
- **Brand:** the Hoshiya name and logo (`brand/logo/`) are not covered by the licence above. The **ArtCraft** name and logos are
  trademarks of the ArtCraft team, are not open source and are not used here. In plain text: *Based on WordCraft, GridCraft,
  DeckCraft and PdfCraft by the ArtCraft team (MIT OR Apache-2.0).*
- **Acknowledgements:** the ArtCraft team for the engines; the Noto Project for the fonts; the authors of the Rust crates we build
  on; everyone who tests and reports problems.
