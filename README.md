# Hoshiya Office add-in — Uzbek Latin ↔ Cyrillic

A COM add-in for **Microsoft Word, Excel and PowerPoint** (Office 2010 and later, 32- and 64-bit) that converts Uzbek
text between the Latin and Cyrillic scripts in place, keeping the formatting.

**[Code signing policy](docs/ms-addin/CODE-SIGNING-POLICY.md)** — Free code signing provided by SignPath.io,
certificate by SignPath Foundation (applied for; releases are unsigned until it is granted).

## What it does

A tab of its own, named after the product, with two commands. Each converts the selection, or the whole file when nothing is
selected (in Excel one selected cell counts as nothing, and converts the whole workbook without asking):

- **Latin to Cyrillic.** Reads each paragraph as 1995 Latin (oʻ gʻ sh ch) or 2026 Latin (ö ğ ş ç), whichever it is
  written in.
- **Cyrillic to Latin.** Always writes the **2026 alphabet** (ö ğ ş ç). Text already in the 1995 Latin alphabet is
  rewritten in the 2026 letters too. When nothing is selected, a paragraph with at least two English function words (the,
  and, is, ...) keeps its Latin; an English sentence inside an Uzbek paragraph may still be changed. A real selection
  upgrades everything in it.

- **Word:** every story (main text, tables, headers and footers, footnotes, comments, text boxes). Field codes,
  hyperlink addresses, bookmarks, comments and content controls are kept. One Undo reverts the whole command.
- **Excel:** cell values only, **never formulas**. Rich-text cells keep their per-character formatting. Also shapes,
  text boxes and notes. Text Excel would otherwise turn into a date or number stays text.
- **PowerPoint:** shapes, groups, tables, SmartArt and notes. Slide masters and layouts are converted only when asked.
- **Undo transliteration** (Excel, PowerPoint): those apps clear their own Undo after an add-in change, so the add-in
  keeps what it changed and puts it back. Anything edited since is left as it is.

The ribbon follows the Office UI language: Uzbek (Latin or Cyrillic), Russian, or English otherwise. The conversion
engine is [`crates/uz-translit`](crates/uz-translit) (Rust, no dependencies, golden-list tested), called through a C ABI
([`crates/uz-translit-ffi`](crates/uz-translit-ffi)). The host is C# on .NET Framework 4.x, with no Office interop
assemblies. Design notes and measurements: [docs/ms-addin/PLAN.md](docs/ms-addin/PLAN.md).

## Supported Office versions

| Office | Status |
|---|---|
| Office 2019 Click-to-Run, 32-bit, on Windows 10 22H2 | **verified**: Word, Excel and PowerPoint smoke tests, the MSI end to end |
| Office 2010, 2013, 2016, 2021, 2024, Microsoft 365; 64-bit Office; MSI installs; Windows 11 | **expected, not tested**: only 2010-era APIs are used, newer ones are feature-detected (PLAN.md section 5) |

## Build

Needs Windows 10 or later with:
- Rust (rustup), targets `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc`;
- Visual Studio or the Build Tools (MSVC linker; MSBuild's Roslyn `csc.exe`, found with vswhere or given as `CSC`);
- the Windows SDK (`rc.exe`);
- WiX 5.0.2 for the MSI (`dotnet tool install --global wix --version 5.0.2`).

```powershell
cargo test --locked                                        # engine and C ABI (golden list)
$env:OFFICE_TARGET = 'D:\build\office-addin'               # any output folder
bash integrations/ms-office/build.sh                       # DLLs -> $env:OFFICE_TARGET\ms-office\addin
powershell -File integrations/ms-office/build-msi.ps1      # MSI  -> $env:OFFICE_TARGET\ms-office\msi
```

The product name, vendor and version come from one place, [`brand/brand.toml`](brand/brand.toml), and are stamped
into every binary and the MSI. CI: [`.github/workflows/ms-addin.yml`](.github/workflows/ms-addin.yml).

## Install

- **Per user** (no admin rights): `msiexec /i <name>-office-addin-<version>-x64.msi`.
- **Per machine:** `msiexec /i <msi> ALLUSERS=1 MSIINSTALLPERUSER=""`, or the `-PerMachine` build for Group Policy.
- **Uninstall:** "Apps & features", or `msiexec /x <msi>`. Running Office apps are never closed by the installer.

Details, signing for IT, and the Office policy that keeps the add-in enabled: [docs/ms-addin/install.md](docs/ms-addin/install.md).
For development, `integrations/ms-office/register.ps1` and `unregister.ps1` register the built add-in for the current
user only.

## Tests

- `cargo test --locked`: the golden list and the rules, also through the C ABI.
- `integrations/ms-office/test/run-smoke.ps1 -Out <OFFICE_TARGET>`: drives its own Word, Excel and PowerPoint instances
  through COM on generated test files. It checks text, formatting runs, field codes, anchors and Undo. The add-in is
  registered only for the length of the run.
- `integrations/ms-office/test/msi-smoke.ps1 -Out <OFFICE_TARGET>`: installs the MSI, tests the installed add-in in
  Excel, then uninstalls it and checks that nothing is left.

## Licence

MIT OR Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), [NOTICE](NOTICE)).

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user or
the person installing or operating it.
