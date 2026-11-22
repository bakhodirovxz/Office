# Uzbek 2026 keyboard for Windows: Phase 0 plan

Status: Phase 0 done 2026-11-22 (owner accepted all recommendations, section 13); product work on branch `keyboard`.
Phase 0 history: plan plus a throwaway spike. Branch `kbd-spike`, spike in `spikes/uz-keyboard/`.
Written 2026-11-22. Machine: Windows 10 Pro 19045, x64, standard user at Medium integrity (S-1-16-8192). The
product name is not written here; like every other surface, it comes from `brand/brand.toml`.

## 1. Goal

The goal is a new entry, "Uzbek (Latin)", in the Windows input list. While the user types:
- `sh`, `ch`, `oʻ`, `gʻ` become the 2026 letters `ş`, `ç`, `ö`, `ğ`;
- case follows R10 (rules.md 4.3): `Sh`, `SH` → `Ş`; `sH` → `ş`;
- Backspace right after a conversion gives back the typed digraph (`ş` → `sh`), so a real s + h can still be typed;
- it works in Word, browsers, Telegram Desktop and Notepad, on Windows 10 and 11, on x64 first and ARM64 later;
- it installs per user without admin rights if at all possible.

Reference for behaviour only: alifly.uz/klaviatura, a free per-user .NET tray app. Nothing of it was downloaded or
examined; this plan uses public Microsoft documentation and our own measurements only.

## 2. Architecture decision

**Decision: (a) a TSF text input processor (TIP), an in-process COM DLL written in Rust with the `windows` crate.
It keeps only the pending tail of a digraph in a TSF composition. It needs a one-time per-machine (admin)
registration (section 3).** The no-admin alternative (b) is described so the owner can choose it knowingly; see
O1 in section 13.

| | (a) TSF TIP | (b) low-level hook tray app | (c) KLC keyboard layout |
|---|---|---|---|
| Rewrites `s` after `h` | yes: `s` waits in a composition, then becomes `ş` | yes: swallows `h`, injects Backspace + `ş` | **no** (below) |
| Backspace undo | yes: exact, the composition still holds `ş` | approximate: blind injection, no view of the text | no |
| Word-level exceptions (Ishoq) | possible: composition until word end, or reading the text where the app allows it | key history only; a mouse click breaks it | no |
| Elevated apps | yes (HKLM registration) | **no**: UIPI blocks hooks and SendInput into higher-integrity windows unless uiAccess (signed, in Program Files, admin) | yes |
| UWP / Store apps | yes, with the IMMERSIVESUPPORT category; DLL in Program Files (readable by app containers) | yes | yes |
| Chromium / Electron / Qt (Telegram) | yes: Chromium uses TSF; Qt uses IMM32, so the composition goes through CUAS | yes (VK_PACKET input) | yes |
| Lock screen / UAC prompt | possible: SECUREMODE category + "copy settings to welcome screen" (admin) | no | yes, if copied |
| Per-user install, no admin | **no** (section 3, measured) | **yes** | no: layouts live in HKLM\SYSTEM |
| Failure mode | a TIP bug crashes the host app (in-process) | Windows 7+ silently removes a hook that exceeds LowLevelHooksTimeout (max 1 s since 1709), and the app cannot tell | none |
| Security perception | a standard Windows IME | a global keyboard hook plus injection, the same pattern as a keylogger; AV heuristics flag unsigned ones | none |

Why not (c). A KLC/`kbd*.dll` layout maps one key event to characters at the moment the key goes down. The only
mechanism with state is the dead key (kbd.h `DEADKEY` table), and it works only as a prefix that outputs nothing:
- `s` would have to be a dead key, invisible until the next key;
- `s` + an unmatched key outputs both characters;
- `ş` is one code point, so Backspace cannot give back `sh`;
- nothing can change a character after it has been emitted.

So "rewrite `s` after `s` was typed" is impossible; only "hide `s` until the next key" is possible. That is a UX
regression, and it still needs admin, because layouts are registered under
HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts. **Confirmed: (c) is rejected.**

Why (a) over (b), even at the cost of admin:
- (a) is the documented Windows mechanism for exactly this job.
- Its conversion is exact (no injected Backspace racing fast typing).
- It works in elevated apps and can reach the lock screen.
- Microsoft's IME requirements page says "use TSF to implement IMEs" and lists the registration steps:
  RegisterProfile, RegisterCategory IMMERSIVESUPPORT, InstallLayoutOrTip.
- (b) cannot type into elevated windows at all. That matters on the HRM/IT machines, where admin tools run elevated.

### 2.1 Conversion model inside the TIP (spike-proven shape)

The TIP holds only the pending tail of a digraph in a composition:

| State | Key | Effect |
|---|---|---|
| idle | `s` / `c` / `o` / `g` (any case) | start a composition showing that letter; the key is eaten |
| pending `s` | `h` | the composition now shows `ş` / `Ş` and stays open; eaten |
| pending `c` | `h` | `ç` / `Ç` (as above) |
| pending `o` / `g` | `'` `ʻ` `’` `` ` `` (rules.md set A) | `ö` / `ğ` (as above) |
| pending `s` | `'` then `h` | `sh` (1995 separator s'h = 2026 plain sh, rules.md 15.2) |
| pending converted letter | Backspace | commit the typed keys (`sh`, `Sh`, `oʻ`, …) instead; eaten |
| pending single letter | Backspace | remove it; eaten |
| pending anything | any other key, click, focus change | commit what is shown; the key goes on to the app |
| any | Ctrl / Alt chords | commit, then pass through (shortcuts never change) |
| any | Shift / CapsLock alone | ignored (so `sH` works) |

Why the converted letter stays in the composition until the next key: Backspace undo then needs no access to
committed text. Committed text cannot always be read or edited. In CUAS (IMM32-emulated apps such as Notepad's
Edit control and Qt), a TIP can only write inside its own composition. The alternative is "commit `ş` at once, and
on Backspace read and replace the character before the caret". It works only in fully TSF-aware apps (Word,
RichEdit, Chromium), so it would make the behaviour differ per app.

The composition needs a display attribute with no underline (`ITfDisplayAttributeProvider`, `TF_LS_NONE`), so
that the pending letter looks like ordinary text. This still has to be checked visually per app (section 10).

## 3. Per-user vs per-machine: findings (measured on this machine, 2026-11-22)

| Part | Per-user (HKCU) works? | Evidence |
|---|---|---|
| COM class (`CLSID\{..}\InprocServer32`) | **yes, for Medium-integrity processes only** | `kbdctl reg-com` wrote HKCU\Software\Classes\CLSID\{71A6784F-…}; `kbdctl cocreate` → `CoCreateInstance` S_OK. Elevated processes ignore per-user COM registration by design (Microsoft, "Per-User COM on Vista for elevated token processes"; COM Elevation Moniker page). |
| TIP profile (`ITfInputProcessorProfileMgr::RegisterProfile`, system scope) | **no** | From a Medium-IL process: `RegisterProfile` → `0x80004005` E_FAIL. HKCU\Software\Microsoft\CTF was byte-identical before and after (`reg export` diff), so there is no silent per-user fallback. |
| Category (`ITfCategoryMgr::RegisterCategory`, GUID_TFCAT_TIP_KEYBOARD) | **no** | → `0x80004005` E_FAIL. The ACL of HKLM\SOFTWARE\Microsoft\CTF\TIP gives BUILTIN\Users and ALL APPLICATION PACKAGES `ReadKey` only. |
| Hand-written mirror of the profile and category keys under HKCU\Software\Microsoft\CTF\TIP\{clsid} (same shape as HKLM) | **no: TSF ignores it** | After `kbdctl hkcu-tip`, `EnumProfiles(0x0443)` still lists 3 profiles, none ours; `GetProfile(spike)` → E_FAIL. That HKCU key holds only per-user Enable overrides (it was empty here). |
| Process-local profile (`TF_RP_LOCALPROCESS`) | yes, but only inside our own process | `RegisterProfile(..., TF_RP_LOCALPROCESS)` → S_OK, `ActivateProfile(FORPROCESS)` → S_OK. Under 0x0443 the TIP never activated (uz-Latn is not in this user's language list); under 0x0409 (en-US, in the list) TSF did call `Activate`. Useful as a test harness, not for Notepad or Word. |
| Adding the installed profile to a user's input list (`InstallLayoutOrTip`, input.dll) | yes, per user, no admin | Documented: "If the IME must be usable immediately after installation, call InstallLayoutOrTip". Not executed here: nothing was added to the owner's language list. |

**Conclusion: a TIP cannot be installed without admin rights.** The profile and the category are read from HKLM only.
There is also a security reason to want HKLM: a DLL that loads into elevated processes must sit in an admin-only
folder (Program Files). Otherwise any user-level malware could replace it and run inside the admin's elevated apps.

What needs admin, exactly, and only once per machine (install, upgrade, uninstall):
1. Copy the DLLs to `%ProgramFiles%\<brand>\Keyboard\{x64,x86,arm64}`.
2. Register COM under HKLM\Software\Classes\CLSID, in the 64-bit view and the 32-bit (WOW6432Node) view.
3. Register the profile and categories: `RegisterProfile`, plus `RegisterCategory` for TIP_KEYBOARD,
   TIPCAP_IMMERSIVESUPPORT, TIPCAP_SYSTRAYSUPPORT, and (optionally) TIPCAP_SECUREMODE and TIPCAP_UIELEMENTENABLED.

What does not need admin:
- adding the keyboard to each user's list (`InstallLayoutOrTip`, or Settings → Language → Uzbek (Latin) → Add a keyboard);
- the on/off state and the options (HKCU).

The no-admin route that exists is (b), the hook tray app (alifly's model, judging by its description). See O1.

## 4. How it appears under Uzbek (Latin)

The profile is registered for LANGID **0x0443** (uz-Latn-UZ), with `hklsubstitute` = the US layout (00000409).
Unconverted keys therefore type exactly as on today's Uzbek Latin keyboard, which Windows maps to the US layout.

Windows then shows it as a keyboard of "Oʻzbek (lotin)" in Settings and in the Win+Space list. If the user does not
yet have Uzbek (Latin), `InstallLayoutOrTip("0443:{clsid}{profile}", 0)` adds both the language and the keyboard.

Display name:
- `RegisterProfile` takes one plain string. `ITfInputProcessorProfilesEx::SetLanguageProfileDisplayName` points to a
  MUI string resource in the DLL, so the name follows the Windows display language (uz-Latn, uz-Cyrl, ru, en
  catalogs).
- The brand name, if it appears at all, is rendered from `brand/brand.toml` at build time.
- Proposed: "Oʻzbek 2026 (ş ç ö ğ)". Owner decision O4.

The LANGID was confirmed only indirectly: `EnumProfiles(0x0443)` works on this machine and lists 3 system
profiles. The owner's language list (en-US 0409:00000409, ru 0419:00000419) was read and **not changed**.

## 5. Behaviour in specific hosts (from documentation and design; to be measured in WP6)

| Host | Expected | Basis |
|---|---|---|
| Notepad, WordPad, classic Edit controls | composition through CUAS | TSF Cicero-unaware app support; spike harness |
| Word 2010+ (TSF-aware) | full TSF text store | Word implements TSF |
| Chrome / Edge / Electron | TSF (Chromium's Windows text input uses TSF) | to measure; Chromium's third-party DLL blocking is believed to exempt registered TIPs (unverified) |
| Telegram Desktop (Qt) | IMM32 → CUAS composition; Qt draws the preedit inline | to measure |
| UWP / Store apps, Settings search | loaded into the app container; needs IMMERSIVESUPPORT and readable DLLs | IME requirements page |
| Elevated apps (admin cmd, regedit) | works only with HKLM COM and profile | per-user COM is ignored above Medium IL |
| Office Protected View (low IL) | loads; DLL in Program Files is readable | to measure |
| Lock screen / UAC secure desktop | only with the SECUREMODE category and "copy settings to welcome screen" | v1 out of scope (O5) |
| Console (conhost, Windows Terminal) | conhost via IMM; Terminal via TSF | to measure |
| Password fields | TSF disables TIPs there (input scope) | standard; nothing to do |

Interaction with Word AutoCorrect:
- The TIP commits the pending letter **before** the space or punctuation reaches Word. AutoCorrect, which triggers on
  word boundaries, therefore sees the finished word.
- Smart quotes: an apostrophe that the TIP consumes (after `o`, `g`, or the `s'h` separator) never reaches Word.
  Every other apostrophe passes through, and Word may turn it into ’ (U+2019), which our engine accepts (set A).
- Word tags typed text with the input language (uz-Latn). That matches the docs-translit language policy (D1).
- Word's Uzbek proofing does not know the 2026 letters, so it will mark words as misspelled. This is outside our scope.
- Ctrl+Z right after a commit: whether Word undoes `ş` and `sh` as one unit is to be measured.

## 6. Conversion rules: reuse uz-translit vs a small state machine

**Recommendation: a small, pure-Rust incremental state machine (`kbd-core`, section 2.1).** It has no Windows
dependencies, so it is unit-testable on any OS. Its data and tests come from `crates/uz-translit`.

- **Why not call `uz_translit::convert` per key.** The engine works on words and paragraphs: it needs the whole
  word, and R11 dictionary lookups match stems. Its 1995 → 2026 path (`Options::latin_upgrade`) rewrites a finished
  word. A keyboard must decide at the second key of a digraph and must be able to undo it.
- **What to reuse.**
  - The letter mapping and code points: rules.md 15.1, `chars.rs` (`is_new_letter`, `compose`; ş U+015F, ç U+00E7,
    ö U+00F6, ğ U+011F and the upper-case forms).
  - The apostrophe set A (`chars.rs::is_apos`).
  - The R10 case rule.
  - The s + h word list: golden rows TUT-046..058 (Isʼhoq, Asʼhob, musʼhaf, Isʼhoqov, …). No row of today's
    `data/exceptions.tsv` is an s + h word, so that list must be exported or extended.
  - Both need a small public API in uz-translit, made through the normal process; the crate is not edited in this
    spike.
- **Where the exceptions apply.**
  1. The typed separator `s'h` → `sh` (no `ş`). This is the 1995 rule R4, which in 2026 spelling is plain `sh`
     (rules.md 15.2). It covers users who type "Is'hoq" from 1995 habit, at zero risk.
  2. Backspace undo covers any other real s + h at the moment of typing.
  3. Optional (WP7), word-level fixing of "Ishoq" without a separator:
     - while the typed word is a prefix of a listed s + h word, keep the composition open to the word end;
     - at the boundary, commit `sh` for a listed word and `ş` otherwise.
     - The cost: the composition spans part of the word (spell-check and AutoCorrect run only at commit), and some
       common prefixes would stay open longer (`ish…` covers both `işla` and `Ishoq`).
     - rules.md 4.4 classes "Ishoq" without a separator as a known input error, so this is a convenience, not a
       correctness requirement.
  4. `ch` in foreign words (Michael, chat): the keyboard converts them, and the user presses Backspace. An R14-style
     guard is impossible at the second key. Owner decision O3.
- **Differential test.** Type every Latin cell of golden.tsv through `kbd-core` as key events. Compare the result
  with `uz_translit::convert(…, ToLatin, latin_upgrade, Y2026)` of the same text. List the expected differences: the
  foreign-word guard, and no-separator s + h.

## 7. UX

- **Backspace undo.** As in 2.1: one Backspace right after a conversion restores the digraph. A second Backspace
  deletes normally.
- **On/off.**
  - The Win+Space switch to another keyboard is the primary "off".
  - Inside the keyboard, a mode toggle (`GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`) turns conversion off and leaves
    the plain Uzbek Latin layout.
  - The toggle is bound to a preserved key (proposed: Ctrl+Shift+` or Right-Alt+`; owner decision O4).
- **Indicator.** The Input Indicator (system tray, not the old language bar) shows:
  - a brand icon from the DLL's resources (black-and-white, 16 to 48 px, as the IME requirements specify);
  - a mode icon, through `GUID_LBI_INPUTMODE`: "ş" when on and "s" when off.
- **No candidate window, no popups.** Nothing is drawn except the composition, which has no underline.
- **Options.** HKCU only, set from a small settings page:
  - convert `sh`/`ch` only, or also `oʻ`/`gʻ`;
  - the tutuq output form.

## 8. Packaging

- **One MSI per machine** (WiX 5, like `integrations/ms-office/installer/addin.wxs` on branch `ms-addin`, but built
  `-PerMachine` only).
  - Scope: Program Files plus HKLM, for the reasons in section 3.
  - The add-in's per-user default cannot be reused here.
- **Payload.** `x64\kbd.dll`, `x86\kbd.dll` (32-bit Office 2010–2016 and other 32-bit apps load the x86 TIP), and
  `arm64\kbd.dll` later. Plus NOTICE and the licences.
- **Registration as the IME documentation prescribes** ("call RegisterProfile … don't write registry entries directly").
  - A deferred, elevated custom action runs `kbd.dll`'s `DllRegisterServer`/`DllUnregisterServer` (once per
    architecture). Those functions call RegisterProfile and RegisterCategory; COM keys may be WiX `RegistryValue`s.
  - A non-elevated, impersonated custom action calls `InstallLayoutOrTip` for the installing user.
  - Other users add the keyboard themselves in Settings, or a per-user first-run entry does it. Owner decision O6.
- **Uninstall.** Unregister the profile and the categories, then remove the files. A DLL in use is replaced at the
  next restart (MSIRESTARTMANAGERCONTROL=Disable, as in the add-in).
- **Versioning and UpgradeCode.** From `brand/brand.toml` (a new frozen UpgradeCode for this MSI).

## 9. Signing

- **Phase 0 and pilot: unsigned.**
  - Consequences: SmartScreen warns on a downloaded MSI (Mark of the Web), and Defender may score an unsigned
    in-process DLL higher.
  - IT installs the MSI on the pilot PCs (AppLocker and certificates are the owner's own call, as for the add-in).
- **Later:** the same SignPath Foundation route as the Office add-in (`docs/ms-addin/CODE-SIGNING-POLICY.md` on
  `ms-addin`), signing the DLLs and the MSI.
- **Unknown, to measure on a test PC:** whether any host refuses an unsigned TIP. Candidates: Chromium's
  third-party module blocking, apps with Microsoft-signed-only code-integrity policies, and Windows 11 Smart App
  Control (which blocks unsigned binaries when on).

## 10. ARM64

- **Builds needed.** Native ARM64 apps need an ARM64 DLL. Emulated x64 apps need the x64 DLL; ARM64EC is an option
  to evaluate. x86 apps need the x86 DLL. So ARM64 Windows needs all three, each registered.
- **The `windows` crate.** It supports `aarch64-pc-windows-msvc`.
- **This machine.** It has neither the Rust aarch64 target nor the MSVC ARM64 libraries (VS Build Tools here has x64
  and x86 only). It has no ARM64 hardware either.
- **The plan.**
  - Cross-build in CI (GitHub `windows-11-arm` or a cross toolchain).
  - Real-device testing waits for ARM64 hardware. **Blocked here; said plainly.**
- **x86 is proven.** The spike DLL builds for `i686-pc-windows-msvc` (143,872 B) as well as x64 (160,768 B).

## 11. Testing strategy

1. **`kbd-core` unit tests.**
   - Every row of the 2.1 table, both cases, Caps Lock.
   - A property test: without Backspace, the output never contains a digraph that has a 2026 letter, except after
     `s'h`.
   - The differential test against uz-translit (section 6).
2. **In-process TSF harness.**
   - The spike's `kbdctl host`: our own window with a classic Edit control (CUAS path) and a RichEdit with
     SES_USECTF (TSF-aware path).
   - The profile is registered with `TF_RP_LOCALPROCESS`, and keys are fed by SendInput.
   - It runs without any machine registration, so it can run in CI. It needs an unlocked interactive desktop (below).
3. **System tests on a test PC or VM with the MSI installed.** SendInput into each host app, reading the text back
   with WM_GETTEXT or UI Automation. Hosts:
   - Notepad, Word (32-bit and 64-bit), Edge, Chrome, Telegram Desktop, Windows Terminal;
   - a UWP text box, an elevated Notepad, Office Protected View.
   - Each run records per-key latency, and also the visible composition, by screenshot.
4. **Manual checklist.** Lock screen (if O5), IME switch hotkeys, focus change while pending, mouse click while
   pending, Remote Desktop.
5. **Not CI-safe.** SendInput needs the foreground and an unlocked desktop (measured below). Whether GitHub Windows
   runners give an interactive desktop is unknown. Plan it as a self-hosted or manual job.

### 11.1 Spike results (2026-11-22)

What was built:
- `spikes/uz-keyboard/src/lib.rs`: the TIP, about 370 lines.
  - Contents: ITfTextInputProcessor, ITfKeyEventSink and ITfCompositionSink; sync edit sessions; and a composition
    that holds the pending `s`/`ş`.
  - Exports: DllGetClassObject, DllCanUnloadNow.
- `src/bin/kbdctl.rs`: registration experiments, the test host, and the SendInput and ITfKeystrokeMgr drivers with
  latency.
- MSVC release builds: x64 DLL 160,768 B; x86 DLL 143,872 B. The first build took 66 s, and later ones a few seconds.
  The `windows` 0.62.2 crate (MIT OR Apache-2.0) was already in the cargo cache.

Registration: see section 3. Per-user COM works. Profile and category registration need HKLM.

**Typing into Notepad: not done. Stopped as instructed.** Notepad only gets a TIP that TSF can activate for the
user, which needs the HKLM profile and category registration (section 3). No per-machine registration or admin
action was attempted.

In-process harness:
- **Locked desktop.** The workstation was locked for the whole session (foreground window: LockApp.exe "Windows
  Default Lock Screen"). SendInput is therefore impossible: `kbdctl host` aborts with "cannot take the foreground"
  and sends no keys anywhere.
- **Fallback: feeding keys through `ITfKeystrokeMgr::TestKeyDown/KeyDown`,** as a TSF-aware app's message loop does.
  - The baseline without a TIP passed 10/10 cases in the Edit control.
  - Harness overhead (KeyDown → text change, same thread): median 0.46 ms, p95 0.53 ms, 63 keys.
- **The TIP never received a key event.**
  - The TIP activated: log `activate tid=18` and key-sink `OnSetFocus(true)` during the feed.
  - But `TestKeyDown` returned `Ok(false)` without calling the sink, while the window had Win32 focus and the thread
    manager had a focus document.
  - Probable cause: TSF does not route keystrokes to a thread that is not the foreground input thread. This is an
    inference, not proven.
  - Manually activating the TIP with a non-TSF client id is rejected: `AdviseKeyEventSink` → E_INVALIDARG.
- So the conversion, Backspace undo and per-key latency through a real TIP are **not yet measured**. They need an
  unlocked desktop. An unlock-triggered run of the same harness (`kbdctl wait-unlock` + `host edit|rich tip`) was
  queued; any result is recorded in 11.2.

### 11.2 Unlock-triggered run

The desktop stayed locked: `kbdctl wait-unlock 45` ended with "still locked after 45 min", and no keys were sent.
The first job of WP5 is therefore this run on an unlocked desktop:
`UZKBD_LANG=0409 kbdctl host edit tip` and `kbdctl host rich tip`. It covers the CUAS path and the TSF-aware path,
the ten cases in `CASES`, Backspace undo, and per-key latency against the `plain` baseline. It needs about one
minute with nobody typing, and no admin.

### 11.3 Per-machine test on this PC: prepared, not run (2026-11-22)

The owner decided O1 = A and O2 = all four letters. The coordinator relayed an approval to register the spike per
machine on this PC.

What changed:
- The spike TIP now handles all four letters, as a pure state machine (`decide()` and `shown()` in
  `spikes/uz-keyboard/src/lib.rs`).
- That covers sh, ch, oʻ (typed `o'`), gʻ, the R10 case rules, Backspace undo, the `s'h` → `sh` rule, and Ishoq
  (→ Işoq without a separator; `Ish⌫oq` → Ishoq).
- A unit test plays all 23 shared `CASES` through the state machine against a model document: **23/23 pass**.
- Builds: x64 DLL 163,840 B and x86 DLL 145,920 B.

The machine test is in `spikes/uz-keyboard/machine-test/`:
- `run.sh` (not elevated) does the following, in order:
  1. waits up to 20 min for an unlocked desktop, and sends no keys while LockApp is in the foreground;
  2. snapshots HKCU CTF, Keyboard Layout, Control Panel\International, Software\Microsoft\Input and the language list;
  3. starts `elevated.ps1` through **one UAC prompt**;
  4. adds `0443:{clsid}{profile}` to the list (`lang-add.ps1`);
  5. runs `kbdctl suite notepad wordpad edge`. Each app gets a plain run and then a TIP run, with a session-wide
     profile switch that is restored afterwards. Edge uses a temporary `--user-data-dir` on E: and mirrors its
     textarea into the window title;
  6. restores the list exactly (`lang-restore.ps1`: en-US 0409:00000409, ru 0419:00000419), signals cleanup, and
     compares every snapshot.
- `elevated.ps1` does the following:
  1. snapshots HKLM\SOFTWARE\Microsoft\CTF in both registry views;
  2. copies the x64 and x86 DLLs to `C:\Program Files\uzkbd-spike`;
  3. writes the HKLM COM keys in both views;
  4. runs `kbdctl machine-register` (x64 and x86): RegisterProfile 0x0443, plus the categories TIP_KEYBOARD,
     IMMERSIVESUPPORT and SYSTRAYSUPPORT;
  5. waits for the runner;
  6. unregisters, deletes the keys and the folder, and lists any process that still holds the DLL;
  7. snapshots again.

  Cleanup needs no second UAC prompt, because the same elevated process does it.

**Not executed.** The agent's permission system denied starting the run, because the approval reached the agent
only as a relayed message. Nothing was registered: `C:\Program Files\uzkbd-spike`, the HKLM keys and the HKCU
CLSID are all absent, and the language list is unchanged.

To run it, the owner either runs `bash spikes/uz-keyboard/machine-test/run.sh` himself (after building, see the
README), or grants the agent this permission explicitly. The output answers the open points of 11.1:
- conversion in Notepad (CUAS), WordPad (TSF-aware RichEdit) and Edge (Chromium TSF);
- per-key latency (median and p95, against the plain baseline), and the TIP-side time per key;
- whether the HKLM, HKCU and language-list snapshots are byte-identical after cleanup.

### 11.4 Per-machine test: results (owner ran it, 2026-11-22)

First run: `elevated.ps1` stopped before registering anything. Its helper `R` was shadowed by the built-in alias
`r` (Invoke-History); it is renamed `RegRun`. The snapshots were unchanged.

Second run (Windows 10 22H2, x64):
- Registration: RegisterProfile 0x0443 and the three categories returned Ok in x64 and x86. The profile was
  visible (flags 0x2) and was added to the user's list as uz-Latn.
- **WordPad (TSF-aware RichEdit): TIP 23/23**, plain 23/23. Latency from SendInput to the text change: median
  8.83 ms, p95 14.91 ms (plain: 6.16 / 13.02 ms).
- **Notepad (CUAS): TIP 4/23**, plain 23/23. Every failure lost the key that only commits, for example the `a` in
  `shahar` → `şhar`, the space or Enter after a word, or the `y` in `choy` → `ço`. Cause: `OnTestKeyDown` claimed
  the key and `OnKeyDown` then returned FALSE. Under CUAS a claimed key has already become VK_PROCESSKEY, so it is
  lost. This is the risk "CUAS ordering" in section 12. **Fix (not yet machine-tested):** a commit-only key is no
  longer claimed. `OnTestKeyDown` commits synchronously and lets the key through. Latency (TIP): median 5.30 ms,
  p95 8.83 ms.
- **Edge: not measured.** The plain run passed 12 cases, then AnyDesk took the foreground and the harness stopped
  sending keys, as designed.
- TIP-side time per key (158 keys): median 2.9 ms, p95 7.4 ms, max 8.5 ms. All keys are below one 16 ms frame.
- Cleanup:
  - HKLM CTF snapshots in both views: byte-identical. The language list: identical.
  - HKCU: Windows left `CTF\SortOrder\AssemblyItem\0x443` and `CTF\TIP\{clsid}` (Enable=0). The coordinator deleted
    both by hand. Windows also dropped its `CTF\Assemblies\0x419` cache entry, and `Control Panel\International`
    lists the same values in a new order.
  - The installer must remove the two HKCU keys on uninstall, for every profile that has them.
  - The DLL stayed loaded in Explorer, VS Code, Docker Desktop, Element, mintty and msrdc, so
    `C:\Program Files\uzkbd-spike` could not be deleted until restart. The product MSI must expect this, for
    example with a restart-pending delete. The machine test now uses its own run folder each time.

Third run (with the "commit in OnTestKeyDown and pass the key" fix):
- **Caps Lock was on for the whole run**, so every case came out with its case inverted, the plain baseline too
  (0/23). The harness now switches Caps Lock off for the run and restores it afterwards.
- Read with the case inverted back, **WordPad 23/23 and Edge 23/23** (Edge measured for the first time). Edge
  latency is about 215 ms both plain and with the TIP. That is the harness reading the window title, not the TIP.
- **Notepad still fails.** The key is no longer lost, but it lands before the committed text: `shahar` → `aşhar`,
  `tosh⏎` → `to⏎ş`. Under CUAS, a key that is let through overtakes the composition result.
- **Fix: the section 12 fallback (not yet machine-tested).**
  - A commit-only key is always claimed.
  - A printable key is committed with the composition in one edit session, as `Finish(shown + char)`, using
    ToUnicodeEx with the thread's key state.
  - Any other key (Enter, Tab, arrows, shortcuts) commits the composition and is then sent again with SendInput,
    so Enter still sends in a chat.
- TIP-side time per key (217 keys): median 0.15 ms, p95 5.6 ms.
- Cleanup bugs, both fixed:
  - `$P` was overwritten by the loop variable `$p`, because PowerShell names are case-insensitive. It is now
    `$Root`.
  - The two HKCU keys came back. `lang-restore.ps1` now removes them.

Fourth run (claim commit keys; commit printable keys with the composition, re-send the others):
- Notepad plain: 23/23 with Caps Lock off. Caps Lock was on again from the Notepad TIP run onwards. The cause is
  not known: the TIP never sends VK_CAPITAL, and AnyDesk was running.
- Read with the case inverted back, **all three hosts are 23/23 with the TIP: Notepad (CUAS), WordPad and
  Edge**. Notepad is now in the right order: `toş⏎`, `Işoq`, `sş `, `qusx `. Latency from SendInput to the text
  change, TIP on: Notepad median 4.47 ms (p95 7.66 ms), WordPad 6.12 ms (9.88 ms). TIP-side time per key: median
  0.15 ms, p95 5.1 ms (237 keys).
- **Cleanup was complete for the first time.** Every HKCU, HKLM and language-list snapshot is byte-identical, no
  process held the DLL, and the run folder was deleted. Only the folders from runs 2 and 3 remain, held until restart.
- The harness now reads Caps Lock through the target window's input thread (the console's own GetKeyState was
  stale), switches it off before each case and logs when it does, and restores the user's state at the end. A
  confirmation run with no case inversion is still open.

Fifth run (the product TIP, 2026-11-22 23:39; Windows 10, ru UI):
- Registration Ok (regsvr32 x64 and x86, `KbdInstallForUser`). The display name shows in the ru UI language.
- **The TIP activated in Notepad, WordPad and Edge but converted nothing: 3/23.** The 3 passing cases are the ones with
  no digraph. The log has no handled key at all.
- **Cause (most likely, measured in part):** the product read every character with
  `ToUnicodeEx(..., GetKeyboardLayout(0))`. ToUnicodeEx gives 0 characters for an HKL that is not loaded in the session
  (measured on this PC: 0x04430443 and 0x04090443 give nothing; the loaded 0x04090409 gives `s`). The profile has no
  substitute layout, so the thread's HKL under the TIP is a uz-Latn one that is not loaded. Every letter then became
  "no character" and started nothing. The spike read letters from the key code, so it never hit this. The thread's HKL
  under the TIP could not be read here: uz-Latn is not in this PC's list, so a process-local 0x0443 profile does not
  activate.
- **Fix:** the character comes from the thread's HKL, then the thread's active layout, then US, then the key code itself
  (A–Z with Shift xor Caps Lock, the `'` and `` ` `` keys, Space). The debug log now shows the HKL at activation and
  every key let through, with the character and its source.
- `hklSubstitute` stays empty for now. Setting the US layout (0x04090409) would change the input language that apps see
  to en-US (Word may switch its proofing language; not measured), and it is a registration change. Decide after the
  next run logs the real HKL.
- `KbdInstallForDefaultUser` → 1603 and nothing changed in the default-user hive; the failing step was not logged. Now
  every step logs its Win32 error. The documentation does not give the format of `pszUserReg`, so three forms are tried
  (`HKEY_USERS\<mount>`, `HKU\<mount>`, `\Registry\User\<mount>`) and each result is logged with what the hive shows.
  If none works, the documented alternative for O6 is Active Setup (`HKLM\SOFTWARE\Microsoft\Active Setup\Installed
  Components\{id}`, `StubPath` = a per-user call of `InstallLayoutOrTip` at each user's next logon).
- After `regsvr32 /u`, `HKLM\SOFTWARE\Microsoft\CTF\TIP\{clsid}` was left in the 64-bit view. `DllUnregisterServer`
  now deletes it.
- Edge lost the first key of the first case (`hahar `). The harness now types a warm-up key until the page reacts.
- Cleanup: every snapshot byte-identical.

## 12. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Admin needed for install (measured) | users without admin cannot install; deployment goes through IT | per-machine MSI via IT or GPO; per-user enablement without admin; option (b) if the owner wants no-admin (O1) |
| In-process DLL crash takes down Word, Telegram, … | data loss in the host | `catch_unwind` at every COM entry, so a bug commits the composition and passes the key through instead of aborting the host (the spike uses `panic = "abort"`, not acceptable for the product); fuzz `kbd-core`; tiny surface (no UI, no I/O on the key path) |
| CUAS ordering: a committed composition vs the next key's WM_CHAR | wrong order such as "oş" → "o" before "ş" | measure in Notepad and Qt (WP6); fallback: eat the key and insert it in the same edit session |
| Composition looks different per app (underline, IME window) | visual noise | `TF_LS_NONE` display attribute; screenshots per app in WP6 |
| Foreign `ch` words converted | annoyance | Backspace undo; optional user exception list (O3) |
| Unsigned DLL blocked by Smart App Control, Chromium or AV | does not load | SignPath before any wide rollout (section 9) |
| ARM64 untested | unknown | CI build; device test when hardware exists |
| Windows 11 changes to TSF/CUAS | regressions | test matrix includes Windows 11 23H2/24H2 |
| Lock screen | the 2026 letters are not available for passwords there (rarely needed) | out of v1 (O5) |

## 13. Open decisions for the owner

- **O1. Admin requirement.** DECIDED 2026-11-22: A (per-machine TIP).
  - (A) TIP with a per-machine MSI that IT installs: recommended; full fidelity, works in elevated apps.
  - (B) a per-user hook tray app, no admin: alifly-like; no elevated apps; approximate Backspace undo; AV risk.
  - (C) both: about +6 days, and two behaviours to support.
- **O2. Letters in v1.** DECIDED 2026-11-22: all four (ş ç ö ğ).
- **O3. Foreign `ch`.** DECIDED 2026-11-22: accept the conversion; Backspace undo restores `ch`. No exception list in v1.
- **O4. Display name and on/off hotkey.** DECIDED 2026-11-22: "Oʻzbek 2026 (ş ç ö ğ)" (uz-Latn; other locales from the
  i18n catalogs) and Ctrl+Shift+`.
- **O5. Lock screen and UAC desktop support.** DECIDED 2026-11-22: not in v1.
- **O6. Enable for every user of a PC at install.** DECIDED 2026-11-22: every user (IT installs), through the
  default-user hive (`InstallLayoutOrTipUserReg`) plus the installing user.
- **O7. Word-level s + h fixing for "Ishoq" without a separator (WP7).** DECIDED 2026-11-22: skip in v1 (later).
- **O8. Signing.** DECIDED 2026-11-22: the same SignPath project as the add-in.

## 14. Work packages and effort (agent-days, excluding the owner's test time)

| WP | Content | Effort |
|---|---|---|
| WP1 | `kbd-core` state machine, unit, property and differential tests; small public data API in uz-translit | 2.5 |
| WP2 | TIP DLL: composition, `TF_LS_NONE` display attribute, Backspace undo, preserved-key toggle, Input Indicator icons, MUI display name, panic safety | 4 |
| WP3 | Registration: DllRegisterServer/Unregister (profile, categories), InstallLayoutOrTip helper, clean uninstall, x86 + x64 | 2 |
| WP4 | Per-machine WiX MSI, NOTICE, brand rendering, build script (MSVC x64/x86) | 2 |
| WP5 | Test harness made CI-able: in-process TSF host, SendInput driver, latency report | 1.5 |
| WP6 | App matrix on a test PC or VM (Notepad, Word 32/64, Edge, Chrome, Telegram, Terminal, UWP, elevated, Protected View), Windows 10 and 11 | 3 (+ owner PC time) |
| WP7 | Optional: word-level s + h exceptions | 2 |
| WP8 | ARM64 build in CI; device test when hardware exists | 1 (+ hardware) |
| WP9 | Signing via SignPath (owner steps) | 0.5 (+ owner) |
| — | **Total** | **about 16.5 (18.5 with WP7)** |
| (B) | If O1 = B or C: hook tray app (per-user, no admin) | +6 |

## 15. Licences and clean-room

- **Dependencies.** The spike uses only the `windows` and `windows-core` crates (MIT OR Apache-2.0); `cargo-deny`
  compatible.
- **No code was derived from Windows-classic-samples SampleIME** (MIT). The spike was written from the public TSF
  API reference only. If product code later derives from SampleIME, NOTICE must carry its MIT notice.
- **No code from ONLYOFFICE, LibreOffice or alifly** was looked at.

## 16. Spike cleanup record

The record of what was created and removed on this machine is kept in `spikes/uz-keyboard/README.md`.
