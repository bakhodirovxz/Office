# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

This is the code signing policy of the Hoshiya project. It is linked from the project home page (the repository README)
and from every release page. Status: **applied for, not yet in force.** Until the SignPath Foundation grants the
certificate, releases are unsigned and Windows SmartScreen warns; verify downloads against the `SHA256SUMS` file of each
release.

## What is signed

Only our own binaries, built from this repository by a GitHub Actions workflow on a GitHub-hosted runner. No locally built
file is signed for release, and no third-party binary is signed.

| Product | Files | Built by |
|---|---|---|
| Office add-in (Word, Excel, PowerPoint) | `UzTranslit.OfficeAddin.dll`; `x86\uz_translit_ffi.dll`, `x64\uz_translit_ffi.dll`; the per-user and per-machine MSIs | `.github/workflows/ms-addin.yml` on branch `office-addin` |
| Windows keyboard (Uzbek 2026 Latin input) | `x64\kbd.dll`, `x86\kbd.dll`; the per-machine MSI | `.github/workflows/keyboard.yml` on branch `keyboard-release` |

The DLLs are signed first; the MSIs are then built from the signed DLLs and signed. The Rust DLLs link the Rust standard
library and, statically, the Microsoft C runtime of the MSVC toolchain (redistributable, as in any MSVC-built program);
licences are in `NOTICE`. The add-in host uses only the .NET Framework that ships with Windows. Product name and version
in every file come from one place, `brand/brand.toml` (`[product]`), and match the SignPath artifact configuration.
Windows shows the publisher as **SignPath Foundation** (the certificate subject).

All our code is licensed MIT OR Apache-2.0 (OSI-approved). There is no proprietary code.

## Team roles

The project has one maintainer, who owns the repository.

| Role | Members |
|---|---|
| Authors (commit to the repository) | [bakhodirovxz](https://github.com/bakhodirovxz) |
| Reviewers (review every change from a non-committer before it is merged) | [bakhodirovxz](https://github.com/bakhodirovxz) |
| Approvers (approve every signing request by hand in SignPath) | [bakhodirovxz](https://github.com/bakhodirovxz) |

Every member uses multi-factor authentication on GitHub and on SignPath.

## Privacy statement

This program will not transfer any information to other networked systems unless specifically requested by the user or
the person installing or operating it.

- The Office add-in converts text inside the open document on the user's computer. It has no network code, no telemetry
  and no update check.
- The keyboard changes letters as the user types, inside the application that has the focus. It has no network code, no
  telemetry and no update check. It writes nothing to disk unless the user turns on its diagnostic log
  (`HKCU\Software\Hoshiya\Keyboard`, `Debug` = 1): that log stays on the user's computer in `%TEMP%` and records key codes
  and timings for troubleshooting.

## What the installers change

Each MSI is removed completely by its own uninstaller (Windows "Apps & features" or `msiexec /x`). Neither closes or restarts
running applications.

**Office add-in** (per user, or per machine for Group Policy):
- files in `%LOCALAPPDATA%\Programs\Hoshiya\OfficeAddin\` (per user) or `%ProgramFiles%\Hoshiya\OfficeAddin\` (per machine);
- the COM registration of one class and one add-in key per Office application (Word, Excel, PowerPoint), under HKCU (per
  user) or HKLM (per machine), in both registry views. Details: `docs/ms-addin/install.md` on branch `office-addin`.

**Windows keyboard** (per machine; needs administrator rights):
- files in `%ProgramFiles%\Hoshiya\Keyboard\x64` and `\x86`;
- the COM registration of the input processor and its Text Services Framework profile for Uzbek (Latin), under HKLM, for
  64-bit and 32-bit applications;
- the keyboard is added to the installing user's input list, and through a Windows Active Setup entry
  (`HKLM\SOFTWARE\Microsoft\Active Setup\Installed Components`) to every other user's input list at their next logon;
- uninstall removes the files, the registration, the Active Setup entry and the keyboard from the installing user and from
  every logged-on user. Users who are not logged on keep an inert entry in their input list until they remove it in
  Settings > Time & language; the keyboard no longer exists for Windows, so it does nothing. Details:
  `integrations/windows-keyboard/README.md` on branch `keyboard-release`.
