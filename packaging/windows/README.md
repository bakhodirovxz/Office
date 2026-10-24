# Windows packaging (PLAN phase 4, WP14)

`suite.wxs` and `default-associations.xml` are **generated** by `scripts/build-msi.ps1` from `brand/brand.toml`. Do not edit them; change the script or the brand file.
The MSI is built with WiX **5.0.2** (pinned in `scripts/tools.lock`; fetch with `scripts/fetch-wix.ps1`). WiX 6 and later carry the Open Source Maintenance Fee, so they are not used.

```
powershell -File scripts/fetch-wix.ps1                       # once
scripts/build.sh all all --release --msvc                    # the MSVC exes (hoshiya.exe, hoshiya-serve.exe, 4 editors, 4 CLIs, convert)
scripts/shell-dist.sh && scripts/build.sh all web --release  # the web dists the host serves (when stale)
powershell -File scripts/build-msi.ps1                       # -> E:\office-target\p1\msi\<id>-<version>-x64.msi
```

The pilot MSI is **unsigned** (owner, 2026-10-22). `-SignThumbprint <sha1>` signs the exes and the MSI with `scripts/sign.ps1` when a certificate exists.

## Default associations XML (documented, NOT deployed)

`default-associations.xml` is the file for the Group Policy setting *Computer Configuration > Administrative Templates > Windows Components > File Explorer >
Set a default associations configuration file* (or `Dism /Online /Import-DefaultAppAssociations:<file>`). Owner decision Q2: **no defaults are changed now.** The
MSI registers "Open with" only and never writes `UserChoice`. This file exists so IT can switch defaults on later, per OU, without a new build.

Content (PLAN Q2 / B-11): Docs for `.docx .doc .rtf .odt`, Slides for `.pptx`, PDF for `.pdf`. **No Sheets row**: Excel stays the `.xlsx` default until the
Excel-workload track is done. The identifiers are the ProgIDs the MSI registers (`<aumid>.Document`), so the policy only has an effect on a PC where the MSI is installed.

How it behaves, so nobody is surprised:
- Two different mechanisms (Windows behaviour, not ours):
  - the **GPO** is processed on domain-joined PCs only, at **every sign-in of every user**: it sets the listed types to our apps again each time, so a
    user's own choice for those types lasts only until the next sign-in. Enable it only in an OU where that is wanted;
  - **DISM import** changes the defaults of **new** profiles only; existing users keep their `UserChoice`.
  (Corrected in the Phase 4 gate review; to be confirmed on the domain test VM, docs/phase4/windows-verification.md.)
- Windows 11: the same file works; the Settings UI shows our app only after the MSI is installed.
- To try it on one PC (needs an elevated prompt, changes the defaults of *new* profiles): `Dism /Online /Import-DefaultAppAssociations:packaging\windows\default-associations.xml`;
  undo with `Dism /Online /Remove-DefaultAppAssociations`.
- Not deployed anywhere by us. Verified here only as well-formed XML naming existing ProgIDs (docs/phase4/windows-verification.md).

## What the MSI registers (all under HKLM; `ALLUSERS=1`)

Per app (`docs`, `sheets`, `slides`, `pdf`; names, exes and AUMIDs from `brand.toml`):
- `Software\Classes\<aumid>.Document` : friendly name, `AppUserModelID`, `DefaultIcon`, `shell\open\command = "<exe>" "%1"`.
- `Software\Classes\Applications\<bin>.exe` : `FriendlyAppName`, `DefaultIcon`, `shell\open\command`, `SupportedTypes\.<ext>`.
- `Software\<data_dir>\Capabilities` (`ApplicationName`, `ApplicationDescription`, `ApplicationIcon`, `FileAssociations\.<ext> = <ProgID>`) and
  `Software\RegisteredApplications\<name>` pointing at it, so the app appears in Settings > Default apps.
- `Software\Classes\.<ext>\OpenWithProgids\<ProgID>` per extension. The `.ext` key's own default value is never written.
- `Software\Microsoft\Windows\CurrentVersion\App Paths\<bin>.exe`.

Extensions: Docs `docx docm dotx dotm doc dot odt rtf txt md markdown html htm`; Sheets `xlsx xlsm xltx xltm xls xlt xlsb ods csv tsv`; Slides `pptx potx ppsx`
(not `odp`, not `ppt`); PDF `pdf`. Start menu: one shortcut per exe (the host and four editors) in the all-users Start menu folder, each with its AUMID.
