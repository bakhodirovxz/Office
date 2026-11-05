# Microsoft Office add-in: Uzbek Latin <-> Cyrillic transliteration — plan

Design notes and status, 2026-11-05: **the Word, Excel and PowerPoint adapters work on the test PC** (section 9), a
WiX MSI installs and removes the add-in for all three apps, a signing script and a SignPath-ready CI workflow exist.
Decisions: section 11; the maintainer's SignPath steps: section 12.

Code: `crates/uz-translit-ffi` (C ABI of the engine), `integrations/ms-office/` (the COM add-in, build, register,
smoke test), `i18n/msaddin.tsv` (UI strings).

## 1. Goal and requirements

- One add-in for **Word, Excel and PowerPoint** (Outlook checked, section 4.4), two commands (decision of 2026-10-29: always the 2026 alphabet for writing): **Latin -> Cyrillic**, which
  reads each paragraph as 1995 or 2026 Latin (Auto), and **Cyrillic -> Latin**, which always writes the 2026 alphabet
  (ö ğ ş ç). No 1995 output in the add-in (the FFI keeps the alphabet parameter, like the CLI/API, for scripts).
  Selection, or the whole document.
- **Every Office from 2010 on**: 2010, 2013, 2016, 2019, 2021, 2024, Microsoft 365; 32-bit and 64-bit; MSI and
  Click-to-Run; Windows 10 and 11.
- Formatting is kept, one Undo reverts (where the host allows it), formulas and field codes are never touched.
- UI follows the **Office UI language**: uz-Latn, uz-Cyrl, ru, else en. Native look: a group on the built-in Review
  tab, Office icons, the standard Windows message box, the status bar.
- **One installer, one COM registration, all apps**; uninstall removes it from all; Group Policy deployment for IT.

## 2. Feasibility: which add-in technology

| Technology | Office versions | Verdict |
|---|---|---|
| **COM add-in** (IDTExtensibility2 + IRibbonExtensibility) | 2007 – 365, all builds, 32/64-bit, MSI and C2R | **Chosen.** The only kind that covers 2010 – 365 with one binary. |
| VSTO add-in | 2010 – 365 | Needs the VSTO 4.0 runtime on every PC (not on a clean Office 2010), ClickOnce trust prompts, and VS Office tools to build (absent here). Adds nothing a COM add-in cannot do. |
| Office.js web add-in | Word/Excel/PowerPoint 2016+ (requirement sets vary; 2016 MSI = API 1.1-1.3 with the IE11 webview) | No 2010/2013. IE11 has no WebAssembly, so the engine would need a JS port or a server. Possible **second target later** (Office on the web, Mac), not v1. |
| VBA / .dotm templates | all | Macro security policies block it in most organisations; per-app code; no shared engine. Rejected. |

### Host language: C# (.NET Framework 4.x) vs Rust (windows-rs)

What the test PC could build (checked 2026-11-04): Roslyn `csc.exe` (VS 2022 Build Tools, found with vswhere),
MSBuild, .NET Framework 4.8 runtime assemblies; **no** .NET Framework targeting pack, **no** `dotnet` SDK. `csc`
compiles against the runtime assemblies directly, so **nothing had to be installed** for C#. For Rust only the
`i686-pc-windows-msvc` std (about 25 MB) was added.

- **C# host (chosen).** .NET Framework 4.x is part of Windows 10/11 (4.8), Office loads managed COM add-ins in-process
  through `mscoree.dll` into CLR 4. Ribbon callbacks are IDispatch-by-name: the CLR gives that for free. The Office
  object models are used late-bound (`dynamic`), and the two Office interfaces are declared from their public IIDs, so
  **no PIA, no interop assembly** is shipped or referenced. Host size: 6 C# files (about 900 lines), 60 KB DLL, AnyCPU (one DLL for 32-
  and 64-bit Office).
- **Rust host (rejected for v1).** Would need hand-written IDispatch (GetIDsOfNames/Invoke) for every ribbon callback,
  late-bound calls through `IDispatch::Invoke` with VARIANT marshaling for every object-model call (hundreds of
  call sites across three apps), and an x86 and an x64 build of the host itself. Estimated 3-4x the code for the same
  behaviour, with no gain: the engine is already Rust behind the FFI.
- The engine stays in Rust: `crates/uz-translit-ffi`, a `cdylib` built for **x86 and x64** with a static CRT (no VC++
  redistributable). The C# host loads `x86\` or `x64\uz_translit_ffi.dll` by `IntPtr.Size` before the first call.

### FFI contract (`crates/uz-translit-ffi/src/lib.rs`)

- UTF-16 in and out with explicit `u32` lengths (same signatures on x86 and x64); output buffers are allocated by the
  library and freed by the caller with `uzt_free(ptr, len)`.
- Every export catches panics (`UZT_PANIC`); invalid UTF-16 is rejected (`UZT_BAD_UTF16`), never replaced, so the host
  leaves that text untouched.
- `uzt_convert`, `uzt_convert_runs` (the paragraph-as-runs API of `uz_translit::convert_runs`), `uzt_abi_version`,
  `uzt_rules_version`, `uzt_free`. A `flags` word (ASCII apostrophe, foreign-word guard, Roman numerals) is in the ABI
  from v1 so options do not break it later.
- Tests (`tests/ffi.rs`): the golden list through the FFI (2106 conversions, both directions, 1995 alphabet), all three
  alphabet modes equal to the library on every golden row (6480), split invariance of `uzt_convert_runs` (3856 splits),
  the error contract.

## 3. Architecture

```
 Word / Excel / PowerPoint (32- or 64-bit, any version 2010+)
   │  HKCU|HKLM\Software\Microsoft\Office\<App>\Addins\UzTranslit.OfficeAddin  (LoadBehavior=3)
   ▼
 mscoree.dll ─► CLR 4 ─► UzTranslit.OfficeAddin.dll (AnyCPU, one COM class {B1F8F147-…})
   │   Connect: IDTExtensibility2 (load), IRibbonExtensibility (customUI 2009/07), IAddin (callbacks, test API)
   │   Catalog: i18n/msaddin.tsv (embedded), language = LanguageSettings.LanguageID(msoLanguageIDUI)
   │   WordAdapter (done: per-run path, + WordXml fast mode) │ ExcelAdapter (WP4) │ PowerPointAdapter (WP5)
   │   │ Outlook via WordEditor (WP6)
   ▼
 x86\ or x64\uz_translit_ffi.dll (Rust, static CRT)  ─►  crates/uz-translit (rules, golden lists)
```

- One COM class serves every app. `OnConnection` reads `Application.Name` and picks the adapter; `GetCustomUI` returns
  the ribbon for the ribbon IDs the adapters support (`Microsoft.Word.Document` today; `Microsoft.Excel.Workbook`,
  `Microsoft.PowerPoint.Presentation`, `Microsoft.Outlook.Mail.Compose` later) and nothing elsewhere, so a registration
  in Excel/PowerPoint before their adapter exists is harmless (verified: registered into all three during the tests).
- **Ribbon.** customUI **2009/07** schema (Office 2010+), a group on the built-in **Review** tab (`idMso="TabReview"`,
  exists in all three apps), two large buttons (to Cyrillic, to Latin) plus "Undo transliteration" in Excel and PowerPoint. Labels, screentips and supertips come only from `getLabel`,
  `getScreentip`, `getSupertip` callbacks (2007+ callbacks; nothing 2013-only).
- **Icons.** `getImage` (2007+ callback) returns an IPictureDisp drawn at run time (`src/Icons.cs`): the word
  "Uzbek" in the alphabet the button writes — "Ўз", "Öz" — in dark grey with a Word-blue bar, at 32 px x the
  screen DPI scale (40 px at 125 %, 64 px at 200 %), so high-DPI screens get a sharp image without image files.
  Verified: each callback returns a picture of the expected size (smoke test `icons`). **Not verified: how it looks**
  (alpha edges, Office's own scaling): the test Word is invisible, and showing windows on the user's desktop was
  avoided. Office's built-in `imageMso` icons were the first choice, but the test PC cannot validate names:
  `CommandBars.GetImageMso` returns E_UNEXPECTED in an automation instance, even in-process and visible-minimized (WP3).
- **Language.** `LanguageSettings.LanguageID(2)` at load. LCID primary 0x43 (Uzbek): sublanguage 2 = uz-Cyrl, any other
  = uz-Latn; 0x19 = ru; everything else = en. Office ships **one** Uzbek Latin UI and no separate 2026 one, so
  uz-Latn strings are written in the alphabet in force, 1995 (oʻ, gʻ with U+02BB). This is about UI text only: the
  conversion always writes 2026. Verified on the test PC: the mapping and the four catalogs
  (smoke test `catalog:` checks); this Office is English (LCID 1033), so the uz/ru ribbon was not seen on screen.
- **Messages.** Success goes to the status bar; refusals and errors use the standard Windows message box
  (`MessageBoxW`, owned by the Office window). No custom dialogs.
- **Scope rule.** A button converts the selection; with nothing selected (insertion point) it converts the whole
  document (all stories). (decision D6).

## 4. Formatting strategy per app

### 4.1 Word (implemented)

Evaluated: (a) `Range.WordOpenXML` -> edit `w:t` -> `Range.InsertXML`; (b) in-place edits per formatting run.
Both are implemented; **(b) is the default**, (a) is an opt-in "fast mode" for plain paragraphs (decision D8).
Measured (section 9, probes in the smoke test and `probe7`):
- (b) changes only characters that change; fields, bookmarks, comments, content controls, footnote marks and shapes
  are never re-created, and the command is **one Undo step**. Cost: about **3.5 ms per changed run** in-process
  (Word's own `Range.Text` write; 2.7 ms even in a 2-page document, so it is not the document size).
- (a) is about 50x faster on long documents, but **`Range.WordOpenXML` and `Range.InsertXML` each split a custom
  UndoRecord** (Word 2019, measured: edit A, then either call, then edit B, then one Undo leaves A in place).
  `Range.FormattedText` assignment splits it too. So the fast mode cannot be one Undo step (37 steps for the feature
  document, section 9). It is used only for whole paragraphs of the main text that pass a strict XML allow-list
  (`src/WordXml.cs`: only `w:pPr`/`w:r`/`w:proofErr`; runs with only `rPr`/`t`/`tab`/`br`/`cr`; no lists, section
  breaks or hidden text), never with Track Changes on, never for table cells or the story's last paragraph.
- Two InsertXML traps found and handled: (1) a package with the range's **last paragraph mark** merged that paragraph
  into the next one in a new document, so every XML range ends just before the last mark and the existing mark (with
  its paragraph properties) stays; (2) `XmlElement.SetAttribute("space", xmlNs, …)` without the `xml` prefix produces
  XML that InsertXML refuses (0x800A1801). After every insert the text is read back and must match exactly, else the
  command fails and is undone; a refused insert falls back to (b) for that span.

Algorithm (`integrations/ms-office/src/WordAdapter.cs`):
1. **Scope:** the selection range, or every story: `StoryRanges` + `NextStoryRange` chains (main text, footnotes,
   endnotes, comments, headers/footers of every section, text frames) plus text boxes in headers (not on the chain).
2. **Read pass:** walk formatting runs with `Range.MoveEnd(wdCharacterFormatting)`; cut them at bookmark, comment-scope
   and content-control edges; lock field codes (from `Range.Fields`: begin mark .. separator, and the end mark) and
   equations (`OMaths`). Every remaining piece is read with `Range.Text` and kept only if its length equals its
   position span (end-of-cell `\r\a` counts as one position); anything else (objects, field marks) is copied verbatim.
   Control characters (paragraph, cell, page, column, line breaks, object anchors) are hard boundaries.
3. **Convert:** pieces are grouped into paragraphs and converted with `uzt_convert_runs` (one call per paragraph: the
   context crosses run borders, e.g. a word bold in its first half).
4. **Write pass:** only changed pieces, unchanged head/tail trimmed, applied from the end of the story backwards with
   `Range.Text`, which keeps the formatting of the replaced characters. Where a bookmark/comment/content-control edge
   touches the edit, the new text is inserted inside the old one and the old characters deleted around it, so the
   anchor stays on the word (a plain replace collapsed a bookmark: found and fixed by the smoke test).
5. **Undo:** `Application.UndoRecord.StartCustomRecord` / `EndCustomRecord` (Word 2010+; feature-detected) makes the
   command one Undo step; on an error the record is closed and undone, so a failure leaves the document unchanged.
6. **Speed settings during the command:** `ScreenUpdating`, `Options.Pagination`, `CheckSpellingAsYouType` and
   `CheckGrammarAsYouType` off, restored in `finally` (240 s -> 184 s on the benchmark). Draft view was tried and
   dropped: it recomputed the footer's PAGE field (caught by the smoke test).
- **Fields:** codes never change; results (hyperlink text, cross-reference text) are converted; Word may rewrite a
  result on the next field update (expected Word behaviour, documented to users).
- **Hyperlinks:** address untouched (it is in the field code), display text converted.
- **Track changes:** existing revisions in scope -> refused with a message (converting deleted text is ill-defined).
  If "Track Changes" is on, the conversion is recorded as revisions (not tested yet: WP2).
- **Protected documents:** refused with a message.
- **Language tags:** v1 converts all text. The `LangPolicy` of docs-translit (skip text tagged with other languages)
  is WP2: one `LanguageID` read per piece.
- **Not handled yet (WP2):** text in grouped shapes, SmartArt, charts and WordArt in the main story; shapes anchored in
  header text frames inside groups; column (block) selections; content controls' placeholder text.

### 4.2 Excel (implemented 2026-11-05: `src/ExcelAdapter.cs`, `src/Text2.cs`)

- **Scope:** the selection (cell areas, or the selected shapes); with a single cell selected (Excel always has one) a
  standard Yes/No/Cancel box offers the whole workbook (Yes) or this sheet (No) — D6. A sheet = its `UsedRange`,
  shapes, notes and threaded comments; protected sheets are skipped.
- **Cells:** per area one `Value2` and one `Formula` array read; only string constants are converted. **Formulas are
  never written**: a cell whose `Formula` starts with `=` is skipped if `HasFormula` confirms it (text typed as
  `'=…` also shows `=…` in `Formula`: measured). Changed plain cells are written as one `Value2` array per row span,
  then read back: a text Excel parses into a date/number/formula (measured: "Май 5" -> "May 5" became a date) is
  written again with the text prefix apostrophe.
- **Rich-text cells:** a per-row font check (font properties are `DBNull` when mixed — `null` checks missed it, the
  smoke test caught a flattened cell); in mixed rows a per-cell check; runs are found by bisection on
  `Characters(start, len).Font`, converted as one text and rewritten run by run (`Characters().Text`), last first.
- **Shapes and text boxes:** `TextFrame2` paragraphs and runs (`Text2.cs`), addressed by character position (Excel
  re-splits runs after an edit, so run indexes are not stable). **Notes** (legacy comments): `TextFrame2` runs are
  "not implemented" on note shapes (measured), so they go through `TextFrame.Characters` like a rich cell.
  **Threaded comments** (365 builds): plain text via `CommentThreaded.Text`; not present in this 2019 build.
- **Not converted:** sheet names, headers/footers, chart titles, data validation lists, pivot captions (WP4b).
- **Undo (D5):** Excel clears its own undo list on any change by an add-in. Every change is recorded (row spans of old
  and new texts, rich-cell runs, shape-text edits) and "Undo transliteration" (ribbon, Excel and PowerPoint only)
  puts back exactly what is still as the conversion left it; a cell or shape edited since is left as it is
  (measured: 1 of 9 skipped after a manual edit). A failure part-way restores what was already changed.
- **Lifetime:** the snapshots hold COM references; the add-in drops them and runs the GC in `OnBeginShutdown` /
  `OnDisconnection` (measured: without it a hidden Excel stayed alive after `Quit`).

### 4.3 PowerPoint (implemented 2026-11-05: `src/PowerPointAdapter.cs`, `src/Text2.cs`)

- **Scope:** the window's selection (text, shapes or slides); nothing selected = every slide. Shapes, groups
  (recursively), tables (each cell), SmartArt nodes, and the notes body of each slide; **slide masters and layouts
  only when asked** (API scope `presentation+masters`; English placeholder text such as "Click to edit Master title
  style" is converted too, by the ru/en language policy — WP2's language policy applies here as well).
- Each paragraph's runs through `uzt_convert_runs`; changed runs rewritten through `Paragraph.Characters(start, len)`
  (only the differing middle), so run formatting stays (measured: a mid-word bold run and an italic run kept).
- **Undo (D5):** the same own "Undo transliteration"; `Application.StartNewUndoEntry` (2010+, feature-detected) is
  also called first. **Native Undo could not be measured here**: PowerPoint has no Undo in its object model and
  `ExecuteMso("Undo")` fails (E_FAIL) without a window. Check on a visible PC whether Ctrl+Z reverts the command in one
  step; if it does, the own button can be hidden in PowerPoint.
- **Not tested here:** SmartArt (`SmartArtLayouts` is not available in a windowless PowerPoint), the selection scope
  (no window), charts (not converted).

### 4.4 Outlook (checked, WP6)

- A message being composed is a Word document: `Inspector.WordEditor` (Outlook 2007+; the Word editor is the only
  editor since 2013). The Word adapter works on it unchanged; only the ribbon ID `Microsoft.Outlook.Mail.Compose`
  (Review tab exists there) and the routing `ActiveInspector().WordEditor` are new. Inline replies in the reading pane
  (2013+) use `Explorer.ActiveInlineResponseWordEditor`.
- **Cost: about 0.5 day of code + a test on a test PC.** It is cheap, but **not built**: testing needs Outlook with a
  mail profile, which the test PC did not offer. Planned as WP6.

## 5. Office version matrix

"Verified" = exercised by `integrations/ms-office/test/run-smoke.ps1` on the test PC. Everything else is **expected
(not tested)**; the "why" column is the reason we expect it to work.

| Office | Build type | Bitness | Status | Why it should work / what could break |
|---|---|---|---|---|
| **2019 ProPlus (16.0.19127.20860)** | **C2R** | **x86** | **Verified (Word, Excel, PowerPoint)** | The test PC, 2026-11-04 and 2026-11-05. |
| 2019 / 2021 / 2024 / 365 | C2R | x64 | Expected | Same Office 16 code; the add-in DLL is AnyCPU, `x64\uz_translit_ffi.dll` built and export-checked (dumpbin), but never loaded into a 64-bit Office. |
| 2016 / 2019 / 2021 / 2024 | MSI (volume) | x86/x64 | Expected | Same object model; MSI Office reads HKCU Addins natively. |
| 365 (current channels) | C2R | x86/x64 | Expected | As 2019 C2R; newer builds keep the COM add-in model. Risk: Microsoft's "new Outlook" has no COM add-ins (Outlook only). |
| 2013 (15.0) | MSI | x86/x64 | Expected | All APIs used exist in 2010. customUI 2009/07 supported. |
| 2013 (15.0) | C2R | x86/x64 | Expected, **highest risk** | 2013 C2R virtualised HKLM; **per-user HKCU** registration (what we write) is read; per-machine (HKLM) needs the keys inside the C2R virtual registry. Test first. |
| 2010 (14.0) | MSI | x86/x64 | Expected | UndoRecord, customUI 2009/07, `LanguageSettings`, `StoryRanges`, `MoveEnd(wdCharacterFormatting)` all exist in 2010. CLR 4 add-ins load in 2010 (it uses the mscoree shim). Risk: 2010 is out of support; no SP2 = some add-in loading bugs. |
| 2010 | C2R ("Click-to-Run 2010", beta-era virtualisation) | x86 | Expected, unverifiable | Rare; virtualised like 2013 C2R. |

Windows: Windows 10 22H2 x64 verified (the test PC). Windows 11 expected (same CLR 4.8, same Office). The Rust DLLs need
Windows 10+ (Rust's MSVC target minimum), which matches the supported Windows versions.

**To close the gaps** (IT can provide test PCs or VMs; one VM per row is enough, snapshots between runs):
1. Windows 10, Office 2010 SP2 x86 MSI. 2. Windows 10, Office 2013 x64 MSI. 3. Windows 11, Office 2013 C2R.
4. Windows 11, Office 2016 x86 MSI (the IE11-webview build; also the Office.js question). 5. Windows 11, Microsoft 365
x64 C2R current channel. 6. Windows 11, Office LTSC 2021 or 2024 x64. Run `build.sh` once, copy the output, and run
`run-smoke.ps1` on each (it needs only PowerShell, the built add-in and Office).

## 6. Install and deploy

### 6.1 What gets registered (`integrations/ms-office/register.ps1`, `unregister.ps1`)

One COM class, written into **both registry views** (64-bit and WOW6432Node), so 32- and 64-bit Office find it:
- `Software\Classes\CLSID\{B1F8F147-98CA-4DF8-8C5D-E2E2D4E4E3A8}`: `InprocServer32` = `mscoree.dll`, `Class`,
  `Assembly`, `RuntimeVersion=v4.0.30319`, `CodeBase=file:///…/UzTranslit.OfficeAddin.dll`, `ThreadingModel=Both`; `ProgId`.
- `Software\Classes\UzTranslit.OfficeAddin\CLSID`.
- Per app (Word, Excel, PowerPoint; Outlook later): `Software\Microsoft\Office\<App>\Addins\UzTranslit.OfficeAddin`
  with `FriendlyName` ("<product name> Uzbek transliteration"; the name comes from `brand/brand.toml` through the build
  output `brand-name.txt`), `Description`, `LoadBehavior=3`.
- Uninstall removes all of it from every app; `unregister.ps1` re-checks every key in both views and fails if one
  survived.

Office looks up the COM class through normal COM activation, so the same keys serve MSI and C2R Office 2010-365.
C2R 2016+ reads HKCU and HKLM Addins keys natively; C2R 2013 needs per-machine keys mirrored into
`HKLM\SOFTWARE\Microsoft\Office\15.0\ClickToRun\REGISTRY\MACHINE\…` (to verify on the 2013 C2R VM; per-user needs nothing).

### 6.2 Installers

- **Per-user MSI** (no admin): files under `%LOCALAPPDATA%\Programs\<product>\OfficeAddin\`, HKCU keys as above.
- **Per-machine MSI** (admin, for IT): files under `%ProgramFiles%\<product>\OfficeAddin\` (one AnyCPU DLL + `x86\` +
  `x64\`, so one MSI serves both Office bitnesses), keys under HKLM (both views).
- **Built** (D4): a separate WiX 5.0.2 MSI, `integrations/ms-office/installer/addin.wxs` + `build-msi.ps1`
  (single-package authoring: per-user by default, per machine with `ALLUSERS=1 MSIINSTALLPERUSER=""`, and a
  `-PerMachine` build for Group Policy). Frozen UpgradeCode in `addin-ids.ps1`. Commands and what it writes:
  `docs/ms-addin/install.md`.
- `MSIRESTARTMANAGERCONTROL=Disable`: install/uninstall never close and relaunch the user's Office apps (measured: with
  the default, a silent uninstall made Restart Manager shut down an Excel holding the add-in and relaunch it).
- Version and names: one source, `brand/brand.toml` `[product]` (`name`, `vendor`, `version`, `copyright`): the MSI
  (`<name> Office add-in`, version), the C# assembly attributes and a VERSIONINFO resource in both Rust DLLs
  (`build.sh`, checked after every build).

### 6.3 Group Policy / IT

- Deploy the per-machine MSI with GPO Software Installation (assigned to computers) or Intune/SCCM.
- Prevent Office from disabling the add-in for slow load: Office policy **"List of managed add-ins"**
  (`HKCU\Software\Policies\Microsoft\Office\<ver>\<app>\Resiliency\AddinList`, value `UzTranslit.OfficeAddin` = `1`,
  Office 2013+; set per app: Word, Excel, PowerPoint). Office 2010 has no such policy (users re-enable from
  File > Options > Add-ins).
- Trust Center "Require application add-ins to be signed by Trusted Publisher": for an unshimmed managed add-in Office
  checks `mscoree.dll` (Microsoft-signed), so it loads; we still Authenticode-sign both DLLs and the MSI (decision D3)
  for AppLocker/WDAC and SmartScreen. With AppLocker **DLL rules** on, a per-user install under `%LOCALAPPDATA%` is
  blocked: use the per-machine MSI (IT's call).

## 7. Tests

- Build: `OFFICE_TARGET=<out> bash integrations/ms-office/build.sh` (cargo x86 + x64 msvc, then Roslyn `csc`).
- `cargo test -p uz-translit-ffi` (engine through the FFI).
- `integrations/ms-office/test/run-smoke.ps1 -Out <OFFICE_TARGET> [-Big N]`: registers (HKCU, Word+Excel+PowerPoint),
  starts **its own** Word (`/Automation` process), builds a feature document under `<Out>\test-docs`, runs all three
  modes and the selection mode through the loaded add-in (`COMAddIn.Object`), and checks: every story's text against
  the engine, formatting runs before/after (COM walk and the `w:rPr` sequence of `WordOpenXML`), field codes,
  hyperlink address, bookmark/comment/content-control anchors, footnotes, one-step Undo and Redo, the tracked-changes
  refusal, the catalog languages. `-Big N` adds the timing document. It unregisters in `finally` and verifies no key is
  left, and quits only its own Word. `-Apps Excel,PowerPoint` (default: all three) runs `test/SmokeOffice.cs` on its own
  Excel / PowerPoint (PowerPoint is single-instance: skipped if one is already running). An instance holding a file the
  test did not open is never quit (made visible instead), and each test's own Office process must exit afterwards.
- `integrations/ms-office/test/msi-smoke.ps1 -Out <OFFICE_TARGET>`: builds the MSI, installs it per user, runs the
  Excel smoke test against the **installed** add-in, uninstalls, checks no key, no file and no Restart Manager action.
- CI: `.github/workflows/ms-addin.yml` (not enabled; section 12).

## 8. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Untested Office versions (section 5) | add-in fails to load on some PCs | VM matrix before pilot; the add-in only uses 2010 APIs, feature-detects UndoRecord |
| Large, densely formatted documents are slow on the one-Undo path (3.5 ms per changed run) | minutes on 100+ page documents with a format change every few words | measured in section 9; fast mode (WordXml) is ~50x faster but is many Undo steps (D8); WP7: progress in the status bar, Esc to cancel |
| Office disables a slow-loading add-in (resiliency) | buttons disappear | load is tiny (CLR start + catalog); managed add-ins list via GPO |
| Excel and PowerPoint undo | no one-step native undo | own snapshot undo (D5) |
| Field results re-generated by Word on update | converted hyperlink/ref text reverts | documented; WP2 option to convert the bookmarked source text instead |
| Unshimmed managed add-in shares the CLR default AppDomain with other add-ins | another add-in's crash can disable ours | COM shim (C++ loader) in WP8 if pilots show it |
| Unsigned binaries | AppLocker/WDAC/SmartScreen block | code signing (D3) |
| Icon look not verifiable here (invisible test Word) | blurry or haloed icons | WP3 on a visible test PC |
| Fast mode changes user options during the command (Pagination, proofing, SmartCutPaste) | a crash mid-command leaves them off | restored in `finally`; only the fast mode touches SmartCutPaste |

## 9. Measurements (the test PC, Word 2019 C2R x86, 2026-11-04)

From `run-smoke.ps1` (logs under `<OFFICE_TARGET>`: `smoke-final.log`, `smoke-big.log`, `smoke-fast.log`). The add-in
runs in-process in Word; times are the add-in's own (`ms=`), wall clock within 0.3 s of them.

**Feature document** (14 stories: main text, footnote, comment, headers/footers, text box; 27 formatting runs incl.
mid-word bold, a hyperlink field, a PAGE field, a bookmark, a comment, a content control, a table, a bulleted item,
a line break): 117 checks pass. Per mode (cyrl, latn, cyrl, latn, whole document; first run with three modes on 2026-11-04, rerun with two on 2026-11-05): every story's text equals
the engine's conversion; the formatting runs (COM walk) and the `w:rPr` sequence (WordOpenXML) are unchanged; field
codes, hyperlink address, bookmark, comment anchor and content control keep covering the converted word; **one Undo
restores every story and every run; Redo re-applies**. 0.6-1.0 s per conversion. Selection: only the selected
paragraph changes, one Undo restores (0.1 s). Tracked changes in scope: refused with the message.

**Benchmark document**: 159 pages, 75,630 words, 526,004 characters, **47,670 formatting runs** (a bold, italic,
coloured or half-bold word every ~1.6 words: a worst case; ordinary documents have a few runs per paragraph).

| Path | Latin -> Cyrillic | Cyrillic -> Latin 2026 | Text | Formatting (47,670 runs) | Undo |
|---|---|---|---|---|---|
| Per run (default), first build | 240 s | 266 s | exact | unchanged | 1 step, 0.26 s |
| Per run + pagination/proofing off (final) | 184-200 s (41,155 edits; read 39 s, write 145 s) | 268 s | exact | unchanged | **1 step, 0.26-0.40 s** |
| WordXml fast mode (opt-in, D8) | **20.9 s** (20 inserts, 999 paragraphs) | **20.2 s** | exact | unchanged | **21 steps** (7.9 s) |

- Per changed run, Word's `Range.Text` write costs 3.5 ms in-process; out of process 4.6 ms (big document) and 2.7 ms
  (2-page document) — it is Word's own cost, not ours. Engine time (Rust) is negligible against it.
- A smaller, ordinary document converts in about a second (the feature document: 39 edits, 0.6 s).
- Fast-mode findings (all handled, each caught by the smoke test first): WordOpenXML/InsertXML split the undo record;
  a range's last paragraph mark must stay outside; ending a range mid-paragraph makes InsertXML add a paragraph mark;
  **smart cut and paste drops a range's trailing space on export** (fast mode turns `Options.SmartCutPaste` off for
  the command and restores it); a package whose text differs from `Range.Text` is never written (falls back per run).

**Load and registration**: the add-in loads at Word start in an automation instance (`COMAddIn.Connect` was already
true), `onLoad` of the ribbon XML is called, getImage returns a 32 px picture per button (847x847 HIMETRIC at 96 DPI).
Registration into Word, Excel and PowerPoint at once and removal from all, verified key by key in both registry views.

**Not measured here**: other Office versions and 64-bit Office (section 5); how the icons and the uz/ru ribbon look
on screen (the test Word is invisible, this Office UI is English); add-in load time as Office's resiliency measures it.

### Excel and PowerPoint (2026-11-05, Excel/PowerPoint 2019 C2R x86, build 16.0.19127)

All three apps in one run (`run-smoke.ps1`, default apps): **163 checks pass**; every test's own Office process exits
2.0-3.3 s after its test; no key left.
- **Excel feature workbook** (plain cells in a row span, a rich-text cell with a mid-word bold run, a formula with a
  text literal, a number, a date, `'=…` text, a multi-line cell, a note, a text box with a bold run, a second sheet
  with "Май 5"): text equals the engine; the formula, number and date are unchanged; the rich cell and the text box
  keep their runs; "May 5" stays text (written with the prefix); scope sheet leaves the other sheet; workbook scope
  reaches it; selection A1:A2 only; Undo transliteration restores every cell, run, note and shape, and leaves a
  cell edited after the conversion (restored 8, skipped 1).
- **Excel 2,000 x 20 = 40,000 text cells** (200 of them rich text, a formula column): first version 308 s and Undo
  164 s (about 12 COM calls per cell); after per-row font checks and span snapshots **62.6 s, Undo 16.8 s**; every cell
  equals the engine, formulas unchanged, Undo restores all 40,000.
- **PowerPoint feature presentation** (title, body with a mid-word bold run, a text box with an italic run, a 2x2
  table, a group of two text boxes, notes): 13 edits in 0.34-0.40 s; text equals the engine; runs kept; masters
  untouched unless asked, converted when asked; Undo transliteration restores every text and run.
- **MSI** (`msi-smoke.ps1`): 324 KB; per-user install 0.5-1.1 s writes 10 keys; **the installed add-in loads in Excel
  and passes the Excel smoke test**; uninstall leaves 0 keys and 0 files; with `MSIRESTARTMANAGERCONTROL=Disable` no
  application is shut down or relaunched (the first run, before that property, did so — see section 6.2).

## 10. Work packages

| WP | What | Size | Status |
|---|---|---|---|
| WP0 | FFI crate, x86/x64 build, golden list through the FFI | 0.5 d | **done** |
| WP1 | Word prototype: COM host, ribbon, catalogs, Word adapter, register/unregister, smoke test | 2 d | **done** (this branch) |
| WP2 | Word completion: language policy, track-changes-on recording test, grouped shapes/SmartArt, column selections, speed (WP7) | 2 d | open |
| WP3 | Icons: `getImage` at 32 px x DPI (done); visual/alpha check on a visible test PC, or imageMso if IT prefers Office's own icons | 0.5 d | partly done |
| WP4 | Excel adapter (Value2 arrays, never formulas, rich text, shapes, notes, own undo) | 2-3 d | **done**; WP4b: headers/footers, sheet names, charts; faster rich-cell detection |
| WP5 | PowerPoint adapter (Runs, tables, groups, notes, masters on request, own undo) | 1.5-2 d | **done**; SmartArt and native Undo to check on a visible PC |
| WP6 | Outlook compose via WordEditor (+ inline response) | 0.5 d + pilot test | open |
| WP7 | Large documents: fast mode exists (WordXml, measured); still open: how to offer it (D8), progress and cancel | 1 d | partly done |
| WP8 | Installers: per-user and per-machine MSI (WiX 5.0.2), signing, GPO notes, optional COM shim | 2 d | **done** except per-machine/GPO install tests (need admin) and the COM shim |
| WP10 | SignPath readiness: CI workflow, code signing policy, one version source | 1 d | **done** (prepared); enabling needs the maintainer (section 12) |
| WP9 | Version matrix on IT's VMs (section 5) | 1-2 d | needs VMs |

## 11. Decisions

Taken on 2026-11-05.

- **D1 Host language: C#** (.NET Framework 4.x). (2026-11-05)
- **D2 Office 2010: kept.** Only 2010-era object-model and ribbon APIs; anything newer is feature-detected. No 2010 VM
  test for now: the matrix stays "expected, not tested". (2026-11-05)
- **D3 Code signing: SignPath Foundation** (free OSS signing; certificate subject "SignPath Foundation") for releases
  (2026-11-05). For local tests: `integrations/ms-office/sign.ps1`
  (signtool + RFC 3161, a .pfx or a thumbprint) with the self-signed development certificate kept as a file
  (`new-dev-cert.ps1`); for IT without SignPath, the AD CS / Group Policy path in `docs/ms-addin/install.md`.
  Prepared: `.github/workflows/ms-addin.yml`, `docs/ms-addin/CODE-SIGNING-POLICY.md`, one version source (section 6.2).
  The maintainer's steps: section 12. (2026-11-05)
- **D4 Installer: a separate WiX MSI**; per-user by default, per-machine through a property
  (or a per-machine build for GPO); Word, Excel and PowerPoint in one go; both registry views; clean uninstall; names
  from `brand/brand.toml`. (2026-11-05)
- **D5 Excel and PowerPoint undo: the add-in's own "Undo transliteration"** button, restoring a snapshot of what the
  add-in changed. (2026-11-05)
- **D6 Scope: "selection, else the whole document"** stays. (2026-11-05)
- **D7 Office.js: later**, no code now. (2026-11-05)
- **D8 Large documents: the normal mode (one Undo step) is the default; the fast mode is opt-in behind a standard
  Yes/No warning that Undo will take several steps.** (2026-11-05)

## 12. SignPath Foundation: the maintainer's steps

Prepared in this repository: the CI workflow `.github/workflows/ms-addin.yml` (builds from source
on `windows-latest`, uploads unsigned artifacts, two marked SignPath placeholder steps), `docs/ms-addin/CODE-SIGNING-POLICY.md`
(the required sentence, roles, privacy statement, what is signed, what the installer changes), one version source.
Only the maintainer can do these, in this order:

1. **Enable MFA** on the GitHub account (`bakhodirovxz`) — SignPath requires it for every team member — and on the
   SignPath account once it exists.
2. **Publish the repository** including `.github/workflows/ms-addin.yml` and the policy page, and link
   `CODE-SIGNING-POLICY.md` from the repository home page (README, done) and from the release page.
3. **Publish a first release** of the add-in in the form that will be signed (the MSI built by the workflow; SignPath
   requires an already released project).
4. **Apply** at signpath.org for the open-source programme with the maintainer's identity. State the project, the repository URL, the licence (MIT OR Apache-2.0) and the policy page.
5. After approval: create the SignPath project, two artifact configurations (the 3 DLLs; the MSIs) with the product
   name `<brand> Office add-in` and the version from `brand/brand.toml`, a release signing policy with the maintainer as
   approver; add the repository secret `SIGNPATH_API_TOKEN` and the variable `SIGNPATH_ORGANIZATION_ID`, fill in the
   `TODO(SignPath)` slugs in the workflow, then run it with `signpath_submit`.
6. Confirm the roles in the policy page (author, reviewer, approver: the maintainer for now) and keep them current.

Points to confirm with SignPath when applying: the statically linked MSVC C runtime in the Rust DLLs (toolchain
runtime, redistributable); that the MSIs contain only our signed DLLs plus licence texts. Publisher shown by Windows
after signing: "SignPath Foundation"; AppLocker publisher rules then use that publisher plus the product name
`<brand> Office add-in` (stamped into every file).
