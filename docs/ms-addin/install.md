# Office add-in: build, install, sign

The add-in (Word, Excel, PowerPoint; see [PLAN.md](PLAN.md)) ships as its own MSI.
Everything here is in `integrations/ms-office/`. The product name, vendor and version come from `brand/brand.toml`
(MSI product name: `<brand> Office add-in`); identity and the frozen UpgradeCode are in `addin-ids.ps1`.

## Build

Needs: Rust (rustup) with the `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc` targets, Visual Studio or the
Build Tools (MSVC linker, MSBuild's Roslyn `csc.exe`, found with vswhere or given as `CSC`), the Windows SDK (`rc.exe`)
and WiX 5.0.2 (`dotnet tool install --global wix --version 5.0.2`, or `WIX` / `-Wix`). `OFFICE_TARGET` is the build
output root (any folder).

```powershell
$env:OFFICE_TARGET = 'D:\build\office-addin'   # any folder
# 1. the add-in itself (DLL + x86/x64 FFI DLLs) -> $env:OFFICE_TARGET\ms-office\addin
bash integrations/ms-office/build.sh
# 2. the MSI -> $env:OFFICE_TARGET\ms-office\msi\<id>-office-addin-<version>-x64.msi
powershell -File integrations/ms-office/build-msi.ps1
# per-machine-default variant for Group Policy (-machine suffix)
powershell -File integrations/ms-office/build-msi.ps1 -PerMachine
```

`build-msi.ps1` options: `-AddinDir`, `-Out`, `-Wix` (WiX 5.0.2; default `WIX`, PATH or the dotnet global tool), `-Version` (override
brand.toml, e.g. to test an upgrade), and the signing options below. It reads the assembly full name from the built DLL.
The package is built `-arch x64` (it must write both registry views), so **32-bit Windows is not supported**; 32-bit Office
on 64-bit Windows is (the 32-bit view is registered, and the add-in loads `x86\uz_translit_ffi.dll`).
`wix msi validate <msi>` works offline; its only message is ICE61 (same-version upgrades allowed).

## Install

| Case | Command | Files | Registry |
|---|---|---|---|
| Per-user (default, no admin) | `msiexec /i X.msi` | `%LOCALAPPDATA%\Programs\<brand>\OfficeAddin\` | HKCU |
| Per-machine (IT) | `msiexec /i X.msi ALLUSERS=1 MSIINSTALLPERUSER=""` (elevated) | `%ProgramFiles%\<brand>\OfficeAddin\` | HKLM |
| GPO Software Installation | assign `...-machine.msi` (built with `-PerMachine`) | `%ProgramFiles%\<brand>\OfficeAddin\` | HKLM |

GPO software installation cannot pass properties, hence the separate `-PerMachine` package. Silent install/uninstall with a log:

```powershell
msiexec /i X.msi /qn /l*v install.log
msiexec /x X.msi /qn /l*v uninstall.log      # or: msiexec /x {ProductCode}
```

A newer MSI (higher version) replaces the installed one (MajorUpgrade; the same version also reinstalls). Do not mix
scopes for the same user: uninstall the per-user copy before installing per machine.
Office loads the add-in at its next start; close Office before installing to see it at once.

## What the MSI writes

Same layout as `register.ps1` (the dev tool), under `HKMU` (HKCU per-user, HKLM per-machine), in **both** the 64-bit and the
32-bit registry view:

- `Software\Classes\CLSID\{B1F8F147-...}`: default, `ProgId`, `Implemented Categories`, `InprocServer32` (`mscoree.dll`,
  `ThreadingModel`, `Class`, `Assembly`, `RuntimeVersion`, `CodeBase` = `file:///<install dir>\UzTranslit.OfficeAddin.dll`).
- `Software\Classes\UzTranslit.OfficeAddin\CLSID`.
- `Software\Microsoft\Office\{Word,Excel,PowerPoint}\Addins\UzTranslit.OfficeAddin`: `FriendlyName`, `Description`, `LoadBehavior=3`.
- Files: `UzTranslit.OfficeAddin.dll`, `x86\` and `x64\uz_translit_ffi.dll`, `brand-name.txt`, `NOTICE`, `LICENSE-MIT`, `LICENSE-APACHE`.

Uninstall removes the files, the install folders and every key above (`ForceDeleteOnUninstall`).

## Signing

```powershell
# development certificate (files only, never put in a store; password in dev-codesign.pfx.password.txt, DEV ONLY)
powershell -File integrations/ms-office/new-dev-cert.ps1      # -> $env:OFFICE_TARGET\certs\dev-codesign.{pfx,cer} (or -OutDir)
# sign while building: the 3 DLLs (staged copies, never the build output) before packaging, the MSI after
powershell -File integrations/ms-office/build-msi.ps1 -Pfx <pfx> -PfxPasswordFile <txt>     # or -PfxPassword (SecureString)
powershell -File integrations/ms-office/build-msi.ps1 -Thumbprint <sha1> [-Store LocalMachine]
# sign any files
powershell -File integrations/ms-office/sign.ps1 -Pfx <pfx> -PfxPasswordFile <txt> a.dll b.msi
```

`sign.ps1`: SHA-256, RFC 3161 timestamp (default `http://timestamp.sectigo.com`, the one that answered from the test PC;, retries the timestamp server, prints signer and timestamp, then runs
`signtool verify /pa`. A self-signed certificate is expected to fail `verify /pa` (untrusted root): that prints
"signed, chain not trusted on this PC" and exits 0. Any other verify failure exits 1.

### Free signing for IT

No paid certificate is needed inside an organisation:

1. **Internal AD CS code-signing certificate** (preferred). Domain PCs
   already trust the enterprise root, so `verify /pa` passes and no further GPO is needed beyond publishing the signer to
   Trusted Publishers.
2. **Self-signed certificate** (`new-dev-cert.ps1` style, but generate it for real use on a protected machine): push the
   `.cer` by GPO into *Trusted Root Certification Authorities* and *Trusted Publishers* (Computer Configuration >
   Windows Settings > Security Settings > Public Key Policies). Never hand out the `.pfx`.

**AppLocker:** a publisher rule on the signing certificate's subject (a "Windows Installer" rule for the
MSI, a "DLL" rule for the add-in DLLs) allows the MSI and the three DLLs without hash rules, and survives updates.
The add-in's DLLs are signed, so a DLL rule collection with a publisher rule is satisfied.

**Office Trust Center:** "Require that application add-ins are signed by a Trusted Publisher" is checked against the
registered server of the COM add-in; for a managed add-in that server is `mscoree.dll` (Microsoft-signed), not our DLL, so
the setting does not block it. Signing our DLLs matters for AppLocker/WDAC and SmartScreen, not for this Trust Center check.

### Office "List of managed add-ins" policy

To force the add-in on (and stop users disabling it), set per app, under
`HKCU\Software\Policies\Microsoft\Office\16.0\<app>\Resiliency\AddinList` the value **name = ProgId, data = "1"** (string):

| App | Key | Name | Data |
|---|---|---|---|
| Word | `...\16.0\word\Resiliency\AddinList` | `UzTranslit.OfficeAddin` | `1` |
| Excel | `...\16.0\excel\Resiliency\AddinList` | `UzTranslit.OfficeAddin` | `1` |
| PowerPoint | `...\16.0\powerpoint\Resiliency\AddinList` | `UzTranslit.OfficeAddin` | `1` |

Use the ADMX policy "List of managed add-ins" (Microsoft Office 2016 > Manage Restricted Permissions / per-app Miscellaneous)
or a GPO registry preference. `16.0` covers Office 2016, 2019, 2021 and Microsoft 365; Office 2010 is `14.0`, 2013 is `15.0`.

### Open-source signing service

SignPath Foundation offers free code signing for open-source projects. It is the chosen path for releases (2026-11-05): the CI workflow `.github/workflows/ms-addin.yml` and `docs/ms-addin/CODE-SIGNING-POLICY.md` are prepared; applying, MFA and the first release are the maintainer's steps (`docs/ms-addin/PLAN.md` section 12). Nothing has been sent to SignPath.

## Tested / not tested (2026-11-05)

Tested on the dev PC: build, `wix msi validate`, per-user install, upgrade, uninstall, signing with the dev certificate.
**Not tested:** per-machine install (needs admin), the GPO variant, 32-bit Windows (unsupported), Office actually loading
the installed add-in (Office was deliberately not started), a CodeBase under `Program Files` (path with a space).
