# Code signing policy — Office add-in

This page is the code signing policy for the Microsoft Office add-in (Uzbek Latin ↔ Cyrillic transliteration for
Word, Excel and PowerPoint). The project-wide policy, which also covers the Windows keyboard, is
[CODE-SIGNING-POLICY.md on the main branch](https://github.com/bakhodirovxz/Office/blob/main/CODE-SIGNING-POLICY.md).
Status: **no code signing certificate yet.** Releases are unsigned; verify downloads against the `SHA256SUMS` file of
each release. The SignPath Foundation declined the project's application for now on 2026-11-24 (the project is new and
not yet widely known); this policy describes how signing will work once a certificate is in place.

## What is signed

Only our own binaries, built from this repository by the CI workflow `.github/workflows/ms-addin.yml` on a
GitHub-hosted runner (no local builds are signed for release):

| File | What it is |
|---|---|
| `UzTranslit.OfficeAddin.dll` | the add-in host (C#, .NET Framework 4.x) |
| `x86\uz_translit_ffi.dll`, `x64\uz_translit_ffi.dll` | the transliteration engine (Rust) for 32- and 64-bit Office |
| `<id>-office-addin-<version>-x64.msi`, `…-x64-machine.msi` | the per-user and the per-machine (Group Policy) installers |

The DLLs are signed first, the MSIs are then built from the signed DLLs and signed. Nothing else is signed: no
third-party binary is shipped. The engine links the Rust standard library and, statically, the Microsoft C runtime of
the MSVC toolchain (redistributable, as in any MSVC-built program; it avoids a VC++ redistributable on the client);
licences in `NOTICE`. The add-in host uses only the .NET Framework that is part of Windows.
Product name and version in every file come from one place, `brand/brand.toml` (`[product]`), stamped by
`integrations/ms-office/build.sh` (DLL version resources) and `build-msi.ps1` (MSI), and must match the signing
configuration. Windows shows the publisher named in the certificate.

All components are licensed MIT OR Apache-2.0 (OSI-approved); there is no proprietary code.

## Team roles

| Role | Members |
|---|---|
| Authors (commit to the repository) | `bakhodirovxz` (maintainer) |
| Reviewers (review every change from a non-committer before it is merged) | `bakhodirovxz` (maintainer) |
| Approvers (approve every signing request by hand) | `bakhodirovxz` (maintainer) |

Every member uses multi-factor authentication on GitHub and on the signing service.

## Privacy statement

This program will not transfer any information to other networked systems unless specifically requested by the
user or the person installing or operating it.

(The add-in converts text inside the open document on the user's computer. It has no network code, no telemetry and
no update check.)

## What the installer changes

The MSI installs, and its uninstaller (Windows "Apps & features" / `msiexec /x`) removes:
- the add-in files in `%LOCALAPPDATA%\Programs\<product>\OfficeAddin\` (per user) or
  `%ProgramFiles%\<product>\OfficeAddin\` (per machine);
- the COM registration of one class (`Software\Classes\CLSID\{B1F8F147-98CA-4DF8-8C5D-E2E2D4E4E3A8}` and the ProgId
  `UzTranslit.OfficeAddin`) and one add-in key per Office app (`Software\Microsoft\Office\<Word|Excel|PowerPoint>\Addins\UzTranslit.OfficeAddin`),
  under HKCU (per user) or HKLM (per machine), in both the 64-bit and the 32-bit registry view.

It never closes or restarts running Office applications (`MSIRESTARTMANAGERCONTROL=Disable`); the add-in loads at the
next start of an Office app. Details: `docs/ms-addin/install.md`.
