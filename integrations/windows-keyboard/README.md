# Windows keyboard: the TSF text input processor (kbd.dll)

The Uzbek 2026 keyboard for Windows, as planned in [docs/keyboard/PLAN.md](../../docs/keyboard/PLAN.md) (WP2: the TIP,
WP3: registration). The conversion rules live in [crates/kbd-core](../../crates/kbd-core); this crate is the Windows
side. The workspace is its own, like the spike's: it needs the MSVC toolchain and the `windows` crate.

## Build and test

    export CARGO_TARGET_DIR=E:/office-target/kbd-tip
    cargo +stable-x86_64-pc-windows-msvc build --release --offline
    cargo +stable-x86_64-pc-windows-msvc build --release --offline --target i686-pc-windows-msvc
    cargo +stable-x86_64-pc-windows-msvc test --release --offline
    cargo +stable-x86_64-pc-windows-msvc test --release --offline --test inproc -- --ignored   # activation inside TSF
    cargo clippy --offline --all-targets --target x86_64-pc-windows-msvc -- -D warnings
    cargo deny --manifest-path Cargo.toml --config ../../ci/deny/keyboard.toml check

The tests need no registration and change nothing on the machine. The ignored test registers the class object and a
profile for its own process only (`CoRegisterClassObject`, `TF_RP_LOCALPROCESS`), activates the TIP through TSF, and
checks the key sink, the preserved key, the mode button, the toggle and the teardown.

The C runtime is linked statically (`.cargo/config.toml`): the DLL loads into every process that takes text input and
must not depend on the VC++ redistributable.

## What the DLL contains

| Part | Where |
|---|---|
| Key handling, composition, Backspace undo, on/off toggle (Ctrl+Shift+\`, `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`) | `src/tip.rs` |
| Display attribute with no underline (`TF_LS_NONE`) | `src/attr.rs` |
| Input Indicator mode button (`GUID_LBI_INPUTMODE`: "ş" on, "s" off) | `src/langbar.rs` |
| Registration: `DllRegisterServer`, `DllUnregisterServer`, `KbdInstallForUser`, `KbdUninstallForUser`, `KbdActiveSetup` | `src/reg.rs` |
| Icons (black and white, 16 to 48 px) | `res/*.ico`, made by `res/make-icons.ps1` |
| Display name and mode strings (uz-Latn, uz-Cyrl, ru, en) | `i18n/keyboard.tsv`, compiled into string tables by `build.rs` |

Frozen identities (never change them): CLSID `{E384AABE-7F88-4BB2-BFB5-3AA607601E3E}`, profile
`{BB9F7540-B6BF-43AC-AC2C-0EE5FEC092A6}` under uz-Latn-UZ (0x0443), display attribute
`{508F6AA0-F1BF-406A-93C1-D4BC7FB71699}`, preserved key `{10A834EC-8648-4D1A-91AB-FD4E884F83B9}`.

## Registration, as the MSI will do it (WP4)

| Step | Who | Call |
|---|---|---|
| Per machine, per architecture (x64, x86) | elevated deferred custom action, or `regsvr32 /s` | `DllRegisterServer`: COM keys under HKLM, `RegisterProfile`, `SetLanguageProfileDisplayName`, categories TIP_KEYBOARD, IMMERSIVESUPPORT, SYSTRAYSUPPORT, DISPLAYATTRIBUTEPROVIDER (no SECUREMODE: O5) |
| Every other user, existing or new (O6) | the MSI writes `HKLM\SOFTWARE\Microsoft\Active Setup\Installed Components\{clsid}` (64-bit view); Windows runs its `StubPath` once per user at their next logon, as that user | `rundll32.exe "<dir>d\kbd.dll",KbdActiveSetup` = `InstallLayoutOrTip("0443:{clsid}{profile}", 0)`. (`InstallLayoutOrTipUserReg` on the default-user hive returned FALSE for every path form in machine run 6; dropped) |
| The installing user | impersonated, not elevated | `KbdInstallForUser`: `InstallLayoutOrTip("0443:{clsid}{profile}", 0)` |
| Uninstall | the reverse | `KbdUninstallForUser`, `DllUnregisterServer`; the Active Setup entry is removed with the MSI's registry component |

`KbdInstallForUser` and `KbdUninstallForUser` have the MSI DLL custom action signature (`UINT __stdcall f(MSIHANDLE)`) and return 0 or 1603; `KbdActiveSetup` has the rundll32 entry-point signature. With
the `Debug` setting (or the `KBD_DEBUG` environment variable, which `kbd-harness call` sets) they log every step and the
Win32 error of the one that failed.

Clean uninstall (PLAN 11.4: Windows leaves `HKCU\Software\Microsoft\CTF\TIP\{clsid}` and
`HKCU\Software\Microsoft\CTF\SortOrder\AssemblyItem\0x00000443`):
- `KbdUninstallForUser` removes both from the calling user (the SortOrder entries that name our CLSID only, then the
  keys left empty).
- `DllUnregisterServer` does the same in every user hive that is loaded (logged-on users), and deletes
  `HKLM\SOFTWARE\Microsoft\CTF\TIP\{clsid}` in its own registry view, which `UnregisterProfile` leaves (run 5).
- **Not possible:** other users who ran the Active Setup entry keep `0443:{clsid}{profile}` in their input list (and
  uz-Latn, if it was added for it), plus their Active Setup stamp under HKCU. Removing it would need a second Active
  Setup entry whose command still exists after uninstall; not done. Once the profile is unregistered TSF has no TIP to
  load for that entry (Microsoft does not document what Settings shows for it; not measured yet). The user can remove
  the language in Settings.
- **Not possible:** profiles that are not loaded at uninstall keep those per-user keys. They are inert once the
  profile is unregistered. Cleaning them would mean loading every NTUSER.DAT.
- A DLL still loaded in a running process cannot be deleted until it exits or the PC restarts (11.4); the MSI needs a
  restart-pending delete.

## MSI (WP4)

    bash integrations/windows-keyboard/installer/build.sh     # -> E:/office-target/kbd-msi/<brand id>-keyboard-<version>-x64.msi

Per machine only, one x64 package with both DLLs under `Program Files\<brand>\Keyboard\{x64,x86}`. Name, vendor and
version come from `brand/brand.toml` (the DLL's version resource too). Registration is the table above, run by custom
actions (`installer/keyboard.wxs`, sequence and conditions in its header); there is no SelfReg. WiX 5.0.2 as for the Office
add-in (`E:/office-tools/wix-5.0.2`, or `WIX`). The UpgradeCode is frozen in `installer/build.sh`. Unsigned (PLAN 9).

## Settings (HKCU only; no settings UI yet)

`HKCU\Software\<product name>\Keyboard` (the product name comes from `brand/brand.toml` at build time):
- `Letters` (DWORD): 0 or missing = all four letters (O2); 1 = sh and ch only.
- `Debug` (DWORD): 1 = append a log to `%TEMP%\<product name>-keyboard.log`: activations with the thread's keyboard
  layout (HKL), each handled key with its time, each key let through with the character the TIP read and which source
  answered (`via=`). Otherwise nothing is written to disk.

The TIP reads them at each activation and never writes them.

## Machine test (owner only: needs one UAC prompt)

`machine-test/run.sh` registers the DLLs per machine exactly as above (with the Active Setup entry), runs the entry's
`StubPath` as the owner (Active Setup itself only runs at another user's logon, which the test cannot do), then
`KbdInstallForUser`, types the 23 cases into Notepad, WordPad and Edge, runs the attach check (machine run 6), then
removes everything in the same elevated process and compares HKCU, HKLM (both views) and the language list before and
after. While the TIP is on, the harness never attaches to the target's input thread: that deactivated the TIP in run 6. Build
both architectures first (above), then:

    bash integrations/windows-keyboard/machine-test/run.sh

The result is in `E:/office-target/kbd-tip/run-last.log`, the TIP's own log in `E:/office-target/kbd-tip/m/tip.log`.

## Clean room

Written from the public Microsoft documentation (the Text Services Framework reference and "Input Method Editor (IME)
requirements"). No code from Windows-classic-samples SampleIME or any other IME.
