# Windows verification (Phase 4 WP14/WP15)

Status 2026-10-24: **MSI built and statically checked; the install test has NOT run yet** (needs one UAC click, see "Blocked"). Nothing below is claimed as passed unless it says so.

## The MSI
- `scripts/build-msi.ps1` -> `E:\office-target\p1\wp14\msi\hoshiya-0.1.0-x64.msi`, 175.8 MB (stage 525 MB), SHA-256 `7D31CC54F9CF001C282DF8A39BBE1E261B855F9C48845D14B6191D396FE0C7A2`. Per-machine, x64, unsigned (Q4).
- WiX **5.0.2** (`wix --version` = 5.0.2+aa65968c), official `wix-cli-x64.msi`, SHA-256 `097383B2773BDD3D76A6A2CD3F4DE6357BAE59172E1B6FA5AD87CE062074E8D0` = the publisher's `wix-cli-x64.metadata.json`. The msi itself is not Authenticode-signed. Pinned in `scripts/tools.lock`, fetched by `scripts/fetch-wix.ps1` (unpacked with `msiexec /a`, nothing installed).
- Codes from `brand.toml` (UUIDv5): UpgradeCode `{2EA291EA-35AE-59D4-AB3E-09D1AE815A49}`, ProductCode `{DC308512-70D2-51C0-9CEE-8F05B50749D0}` (changes with the version; MajorUpgrade).
- Layout under `%ProgramFiles%\Hoshiya`: `hoshiya.exe`, `hoshiya-serve.exe`, `hoshiya-docs|sheets|slides|pdf.exe`, `brand.toml`, `vcruntime140.dll`, `vcruntime140_1.dll`, `dist\{shell,docs-web,sheets-web,slides-web,pdf-web}`, `dictionaries\`, `fonts\`, `icons\`, `licenses\` (logo licence, upstream NOTICE/ATTRIBUTION/LICENSE, VC runtime note), `NOTICE`, `LICENSE-*`, `convert\` (`hoshiya-convert.exe` + `docs|sheets|slides|pdf-cli.exe`; the converter looks for the upstream CLI names next to itself, so the brand `cli` names are not used there).
- Built from a clean checkout of HEAD 06e6669 (the shared tree held other agents' unfinished work, which broke `suite-core`) with `scripts/build.sh all all --release --msvc` into `E:\office-target\p1\wp14`. Times: slides 801+508 s, pdf 1343+582, docs 1142+429, sheets 1031+122, services (hoshiya, hoshiya-serve, convert, convert-api) 1418 s. No WebView2Loader.dll import; `DependentLoadFlag` 0x0A00 on hoshiya.exe (dumpbin).
- **Found:** every MSVC exe imports `VCRUNTIME140.dll` (hoshiya.exe also `_1`), which a clean Windows 10 lacks. Shipped app-local (Microsoft-signed, checked in the script) beside the exes and in `convert\`.
- Registration keys: `packaging/windows/README.md`. Extensions per PLAN 7.2; no `.odp`, no `.ppt`; no `UserChoice`, no `.ext` default value. GPO XML: `packaging/windows/default-associations.xml` (well-formed; not deployed, not applied by DISM here).
- Static check done: the msi decompiles (`wix msi decompile`) with registry values, 5 shortcuts with `System.AppUserModel.ID`, MajorUpgrade. Not yet checked: any install behaviour.

## Blocked: elevation and Windows Sandbox
- Windows Sandbox is **not available** on this PC (`WindowsSandbox.exe` absent; the optional feature needs admin and a reboot). Fallback: install/uninstall on this PC with registry snapshots.
- The session is not elevated. The user is a local administrator (UAC consent, Yes/No, no password). `Start-Process powershell -Verb RunAs ... msi-test.ps1` showed a UAC prompt that nobody answered in 8 minutes, so it was cancelled. **The owner must click "Yes" on the UAC prompt**, then watch: the script drives the mouse/keyboard for about 5 minutes (Explorer window, Open with dialogs, 4 + 4 editor windows).
- Command: `Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','<repo>\scripts\msi-test.ps1','-Msi','E:\office-target\p1\wp14\msi\hoshiya-0.1.0-x64.msi','-Work','E:\office-target\msitest-work'`

## What `scripts/msi-test.ps1` does
Prints `INSTALL`, `REGISTRY`, `OPENWITH docx|xlsx|pptx|pdf`, `OPEN spaces+cyrillic n/4`, `SINGLE ...`, `ALWAYS docx`, `UNINSTALL registry diff N`; exit code = failed checks. Log and screenshots go to `docs/phase4/evidence/openwith/`.
1. Snapshot (`reg export` of HKLM\SOFTWARE and HKCU\Software) A, install, snapshot B.
2. Registry assertions for every extension of every app, the 5 Start menu shortcuts, files present, no default changed (`UserChoice` and `.ext` default of docx/xlsx/pptx/pdf before = after).
3. "Open with": `SHAssocEnumHandlers` (the shell's own list) must contain our exe; screenshots of the Open-with dialog per type and of the Explorer context menu for .docx.
4. Opens the four fixtures in `...\Рабочий стол\` (spaces + Cyrillic) with the registered command line; the window title must show the file name (Slides' title carries only the app name: accepted when the window is up, screenshot as evidence).
5. Single instance: Docs/Sheets/Slides give a second process and window with the first untouched; PDF: the second launch exits and the process count stays 1 (the tab is visible only in the screenshot).
6. "Always": drives the shell's dialog with UI Automation (never writes UserChoice), then a plain ShellExecute of the docx must start our editor. Reverted afterwards (on this PC `.docx` has no UserChoice; the script will not touch one that existed). If the automation cannot find the controls it prints `NOT DRIVEN`; do it by hand: right-click a .docx > Open with > Hoshiya Docs > Always.
7. Uninstall, snapshot C, diff A against C. "Ours" = lines mentioning the product, ProgIDs, exes, product code, or our extension keys (this catches empty leftover keys). Everything else is reported separately as machine noise in `uninstall-diff-other-machine-noise.txt`. Files left behind count too. User data the apps create (`%APPDATA%\Hoshiya`, `%LOCALAPPDATA%\Hoshiya`) is deliberately not removed by uninstall.

## Steps for owner IT (Windows 11, domain PC)
1. Clean user, elevated prompt. Copy the repo `scripts\`, `brand\`, the fixtures named in the script (`-Fixtures <repo root>`) and the MSI.
2. Run the command above. Win11's "Open with" UI differs; the registry and `SHAssocEnumHandlers` checks are the same. If the Explorer-menu screenshot misses, take it by hand.
3. Domain: also try `Dism /Online /Import-DefaultAppAssociations:packaging\windows\default-associations.xml` on a test VM, then the GPO in a test OU (`packaging/windows/README.md`). Check AppLocker/WDAC allow unsigned exes under `%ProgramFiles%\Hoshiya`.
4. Send back `docs\phase4\evidence\openwith\` and `msi-test.log`.

## Result rows
| Check | Win10 22H2 (this PC) | Windows 11 | Domain PC |
|---|---|---|---|
| MSI builds, WiX 5.0.2 | done, 175.8 MB | n/a | n/a |
| INSTALL, REGISTRY, OPENWITH, OPEN, SINGLE, ALWAYS, UNINSTALL diff | not run: UAC click owed | not run: owner hardware | not run: owner hardware |
| GPO XML applied | not run | not run | not run: owner hardware |
| Windows Sandbox | not available (feature disabled) | | |
