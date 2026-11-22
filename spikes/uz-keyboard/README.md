# uz-keyboard spike (throwaway, Phase 0)

The smallest TSF text input processor that turns s+h into ş while typing, with Backspace undo. It also holds the
registration experiments. The plan and the findings are in [docs/keyboard/PLAN.md](../../docs/keyboard/PLAN.md).

Build (MSVC):

    CARGO_TARGET_DIR=E:/office-target/kbd-spike cargo +stable-x86_64-pc-windows-msvc build --release --offline

`kbdctl` subcommands are listed in the header of `src/bin/kbdctl.rs`. The TIP writes its log to `%TEMP%\uzkbd-spike.log`.

## What the spike created on the machine, and what was removed (2026-11-22)

Everything was under HKCU or in the spike's own files. Nothing under HKLM, no certificate store, and no change to
the user's language list (read before: en-US 0409:00000409, ru 0419:00000419; unchanged).

| Created | By | Removed |
|---|---|---|
| `HKCU\Software\Classes\CLSID\{71A6784F-8ECA-4FC0-92C0-0B1756DE11CB}` (+ `InprocServer32`) | `kbdctl reg-com` | `kbdctl unreg` (deleted) |
| `HKCU\Software\Microsoft\CTF\TIP\{71A6784F-8ECA-4FC0-92C0-0B1756DE11CB}` (LanguageProfile, Category) | `kbdctl hkcu-tip` | `kbdctl unreg` (deleted) |
| Process-local TSF profile (`TF_RP_LOCALPROCESS`) | `kbdctl host … tip` | gone when the process exits; nothing persisted |
| `%TEMP%\uzkbd-spike.log` | the TIP | deleted |
| Build output `E:\office-target\kbd-spike\` | cargo | kept (build cache, outside the repo) |

`kbdctl api-register` (system RegisterProfile and RegisterCategory) failed with E_FAIL and wrote nothing.
HKCU\Software\Microsoft\CTF was byte-identical before and after (reg export diff).

After `kbdctl unreg`, `reg export` of HKCU\Software\Microsoft\CTF, HKCU\Keyboard Layout and
HKCU\Control Panel\International\User Profile was byte-identical to the snapshots taken before the spike.
`Get-WinUserLanguageList` still shows en-US (0409:00000409) and ru (0419:00000419). The spike's run copy
(`E:\office-target\kbd-spike\run`) was deleted.
