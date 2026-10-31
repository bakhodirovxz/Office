# Phase 4 gate review (WP24): adversarial review of the unified shell (web + desktop)

Date 2026-10-24, branch `phase-4`. The review started at `2047fe2`. Architect tier (opus), with four sub-agents: three
read-only security reviews (desktop host; history and XDFU; the MSI sources) and the end-of-phase sync dry run. Every
sub-agent finding used below was re-checked against the code by me; the ones I could not re-check say so. `file:line`
references are at `2047fe2` (before the fixes) unless a fix commit is named; `history.rs` moved by up to 17 lines in `00c7f5c`.

Gate items (BRIEF Phase 4; PLAN §12.3): a new user creates, edits, saves and reopens a document, a spreadsheet and a
presentation, and converts a PDF, on the web and on the desktop, without instructions; the bundle budgets are met.

**Verdict: NOT READY for "go" yet, for owner-side items only.** No code finding blocks the gate. The rows that are open need
the owner: "Always" by hand on this PC (the MSI test itself passed, 0 failed checks), Windows 11 and the domain PC, the WP23 "no instructions"
sessions, and the desktop keyboard/Narrator walk. The one **not met** gate row (the four apps did not read the language from the shared settings, section 7, row "Brief task 5") is met in 6413f7a. `ci.sh full`: **29 of 29 ok,
exit 0, 6 h 38 min** (section 5). Web and desktop gate flows: 11/11 and 14/14 (section 4). MSI on Win10: 0 failed checks.

Fixed during this review (each its own commit, explicit pathspec):

| Commit | What |
|---|---|
| `a881fff` | `scripts/msi-test.ps1`: never drive "Always" over an existing `.docx` UserChoice; revert with `reg.exe` and check it; survive a fresh profile; uninstall check counts only lines the install added; screenshots and machine-noise diffs stay out of the repo (F2) |
| `00c7f5c` | `crates/suite-core/src/history.rs`: one damaged index no longer deletes that document's stored versions; an index `sha256` must be a hash (F1, F9); two tests |
| `a59dffb` | `web/shell/history.js`: a version list for another document is not this request's answer (F8) |
| `4d0e6ca` | `packaging/windows/README.md`: the GPO default-associations policy applies at every sign-in, not once (F6); `windows-verification.md` result lines |

Evidence that is not in the repo (synthetic files only, no corpus) is under `E:/office-target/p4/`: `gate-flow/` (web flow:
downloads, screenshots), `gate-flow-desktop9/`, `gate-flow-desktop10/` (desktop flow: the files the host wrote, moved out
of Downloads, and its data dir), `gate-flow-desktop*.log`; the two flow scripts are in the review's scratchpad and are
described in section 4.

---

## 1. Findings

Severity: **major** = fix before the feature is used for real (or before the named event); **minor** = real, small blast
radius; **info** = recorded, no action needed now. "XDFU" = matters only when `xdfu = "on"`; it is off by default
(section 2.4). Status: **fixed** (commit), **open**.

### Major

**F1. One damaged history index deleted every stored version of that document. Fixed `00c7f5c`.**
- Where: `crates/suite-core/src/history.rs` `prune_locked` (before the fix: lines 584-590 built the list of documents from
  the indexes that parse; step 3, lines 651-664, deleted every object no parsed index used).
- Scenario: a power cut or full disk while `append` writes a line (`history.rs:397-402`) leaves a torn last line in
  `docs/<id>.jsonl` (an AV scanner holding the file has the same effect). The next save of *any other* document runs
  prune, the torn document is not in the list, and all of its version bytes are deleted.
- Fix: step 3 runs only when every index was read. Test `a_torn_index_keeps_its_objects_when_another_document_prunes`.
- Still open (minor): the torn document's own history stays unreadable (`add_version` fails, the editors only log it)
  until its last line is cut by hand. Repair (truncate an unterminated last line before appending) is a Phase 5 item.

**F2. `msi-test.ps1` could change the owner's `.docx` default and not restore it. Fixed `a881fff`.**
- Where: `scripts/msi-test.ps1` "Always" step (before the fix, lines 228-264).
- Scenario: the dialog was driven even when `.docx` already had a UserChoice; only the revert was skipped. On a PC where
  Word is the user's UserChoice, `.docx` stayed on Hoshiya Docs, and after uninstall Windows resets it (the hashed,
  deny-ACL'd key cannot be put back by a script). Where there was no UserChoice, the revert was `Remove-Item
  -ErrorAction SilentlyContinue`, which the deny-SetValue ACE on the key can make fail silently; nothing checked it.
- Fix: the step runs only when `.docx` has no UserChoice; the revert is `reg.exe delete` and the result line
  `ALWAYS revert` checks it. Same commit:
  - the `reg export` of a missing `FileExts\.docx` (a fresh profile, i.e. Windows Sandbox or a new IT test user) threw under
    `$ErrorActionPreference='Stop'` (reproduced by the sub-agent in Windows PowerShell 5.1);
  - `UNINSTALL` diffed the whole of HKLM and HKCU A→C, so the shell's own per-user traces of opening files (OpenWithList,
    MuiCache, AppCompat store, association toasts) would have failed it with a perfect MSI. It now fails only on a
    registry line the install added that is still there; the rest is info. An empty ProductCode no longer turns the
    filter into match-all;
  - whole-screen screenshots of a work PC and the "other machine noise" diff (MRU file names) were written into
    `docs/phase4/evidence/openwith/`, which is not git-ignored. They stay in `-Work` now, and the whole-HKCU dumps are deleted.
- I parse-checked the script (PowerShell parser, 0 errors) and unit-checked the residue logic with sets. The owner's
  elevated run at 09:45 (section 7) used this version: the Always step was skipped by the automation (no dialog found),
  `ALWAYS revert` ok, and the run then sat in the registry diff for over an hour. The main session found the cause and
  fixed it: `RegSet` joined continued values quadratically (`d2ee004`, now 75 s for the A/B/C diff; that commit also
  carries my `[Shell]::Minus` replacement of a PowerShell pipeline over millions of lines). `427b61d` (main session):
  `$PSScriptRoot` is empty in param defaults under 5.1 `-File`/RunAs, so earlier owner runs never started. The re-run at
  14:38 passed with 0 failed checks (section 7).

**F3. The MSI ships about 900 crates' code without their MIT/Apache notices; the WebView2 loader's licence text is still missing.** **Fixed in bdb77d6** (cargo-about 0.9.2: `packaging/windows/THIRD-PARTY-NOTICES.txt`, 702 crates, installed as `licenses/THIRD-PARTY-NOTICES.txt` with `WebView2-LICENSE.txt`; `ci.sh notices` fails on a stale file or an unlicensed crate).
- Where: `NOTICE` lists our layer, fonts, icons, dictionaries, the MPL crates and the upstream NOTICE files, but no
  per-crate copyright/permission notices; `scripts/build-msi.ps1:51-78` stages only ours, the logo licence, the upstream
  LICENSE/NOTICE/ATTRIBUTION files and a VC runtime note. `NOTICE:157-163` says the WebView2 SDK licence text must be
  copied in "when the MSI is built" and is UNVERIFIED; the MSI is built (`windows-verification.md`).
- Scenario: MIT and Apache-2.0 require the notice to travel with binary copies. The MSI is the first binary we hand to
  someone (WP15: the owner's IT, Windows 11 and domain PCs).
- Fix (not small, not done): generate a third-party notices file from the lockfiles (for example `cargo about`, a new
  dev tool: owner OK needed) into `licenses\` at MSI build time, and copy the Microsoft.Web.WebView2 `LICENSE.txt` next to
  it. **Before the MSI leaves this PC.** `cargo deny` (licences) is green, so this is attribution, not an incompatible licence.

**F4. XDFU: Docs printing to a physical printer gets no watermark and no audit line.** Open, XDFU.
- Where: upstream `wordcraft/crates/ui-egui/src/lib.rs:1693` routes `ui.print`/`ui.printSubmit` to `print_to_system`,
  which calls `app.session.print_pdf` directly (`printing.rs:165`). Our hook guards `file.print`
  (`domains/docs/crates/docs-office/src/hooks.rs:43-53`), which only opens the dialog.
- Scenario: with `xdfu = "on"`, a grif document printed on paper is not stamped and not counted; WP11 says every print is.
  The bypass list in `hooks-docs.md` (WP10/WP11 section) does not name it. Sheets and Slides prints do go through their hooks.
- Fix: a one-hunk carried wordcraft patch so `print_to_system` gets its PDF through the layer, plus a test; add the line
  to the bypass list until then. **Before XDFU is switched on** (with m13 of Phase 3 and F5).
- Fixed in `5905293`: carried patch `wordcraft/0125` routes the Print dialog's PDF through a hook; `docs-office` `print_wrap` stamps it and writes the `print` audit line, never blocks (test `printing_to_paper_carries_the_stamp_and_an_audit_line`). The bypass line in `hooks-docs.md` was not added (no longer a bypass).

**F5. XDFU: the audit lock is held for a whole export, and a 30 s "stale" takeover breaks the chain.** Open, XDFU.
- Where: `crates/xdfu-core/src/audit.rs:199-209`, `:310-326` (sub-agent; I read the lock code, not a run).
- Scenario: a print or export longer than 30 s lets a second editor process take the lock; both write the same `seq`, the
  chain breaks, and from then on every copy is "copy 0" with no audit line (stderr only). A waiter that gives up after 10 s
  also issues copy 0.
- Fix: reserve the seq under the lock (`<event>_begin`), release, produce, then append the result line under the lock.
- Fixed in `5905293` (partly): `append_after` re-reads the tail after `produce` and chains onto it, with `copy` = the stamped number when it differs from the line's `seq`, so a takeover no longer breaks the chain (test `a_lock_taken_over_during_production_keeps_the_chain_intact`). The lock is still held during `produce`; the `_begin` design was not done.

**F6. The GPO for default associations was described as one-time; it applies at every sign-in. Fixed `4d0e6ca` (docs).**
- Where: `packaging/windows/README.md:25-26` (before).
- Scenario: IT enables the policy for a test OU expecting "new profiles only"; every user's `.docx/.doc/.rtf/.odt/.pptx/.pdf`
  goes back to Hoshiya at each sign-in. The corrected text separates the GPO (domain PCs, every sign-in) from DISM import
  (new profiles). To confirm on the domain test VM.

**F7. History View opens a writable copy whose saves are recorded under a temp path that is deleted at the next start.** Open.
- Where: `services/suite-desktop/src/lib.rs:580-592` (`history_view`), `host.rs:463-464` (temp deleted at start).
- Scenario: the user edits and saves the "view" copy; the editor's after-save hook snapshots it under the temp path; the
  host deletes `temp` at its next start, and every History action on that entry is then refused (`check_file` needs the
  file). The string `hist.viewed` promises the copy does not touch history. Nothing of the original document is lost.
- Fix: make the copy read-only and skip snapshots of paths under the data folder's `temp`. Needs one GUI check that all
  three editors open a read-only file and refuse its save cleanly, so not done here.
- Fixed in `39e77a4`: the View copy is written read-only and `Ctx::saved` takes no snapshot of a file under `temp` (test: `the_history_messages`). Not checked: that each editor refuses a save of a read-only file cleanly (needs one GUI run).

**F8 (was minor, raised): the History panel could act on the wrong document. Fixed `a59dffb`.**
- Where: `web/shell/history.js:20-28`, `:72-79` (before).
- Scenario: click document A, then B before A's list arrives: B's request resolved with A's reply, A's versions showed
  under B's heading, and Restore/Name acted on B with A's version numbers (a "Before restore" version is always taken, so
  nothing is lost, but the wrong content lands in B).
- Fix: a `history_versions` reply must echo the requested path, and a reply that arrives after another document was
  chosen is dropped. The shell spec's History test (`ci/web-smoke/shell.spec.ts:355`) passes against it (section 5, `shell`).

### Minor

- **F9. The index `sha256` was joined under `objects/` unchecked. Fixed `00c7f5c`:** 64 lowercase hex digits or the line
  is refused (test `an_index_sha256_that_is_not_a_hash_is_refused`). Needed write access to the user's own store, so no
  privilege boundary was crossed.
- **F10. History of documents on network or mapped drives was refused. Fixed in the commit that follows `5798231`, measured on a `net use` mapped drive.** Before: opening the panel for `H:\a.docx` answered a 'no saved versions' text (the store lists `\?\UNC\srv\share\a.docx`, compared as a string), and once matched, `check_file(.., Source::Page)` refused any canonical UNC path; a Recent reopen from the page failed the same way. Saving and opening from the editor were never affected (history errors are ignored there). Fix in `services/suite-desktop/src/lib.rs`: a drive-letter path matches a stored document by `doc_id`; a path the host already knows (history store, Recent) is checked as a user path. An unknown share path from the page is still refused.
- **F11. History IPC runs on the WebView UI thread** (`host.rs:499-507`). Listing verifies by hashing every live version
  (`history.rs:538-546`, up to the 2 GB cap); the page gives up after 20 s (`history.js:25`) while the host may still
  finish a restore, so the user may retry. Fix: run `history_*` on a worker as `Pick` does; list with size checks only.
  Fixed in `f488c00`: every `history_*` IPC message runs on a worker thread (`host.rs`, the IPC handler) and answers through the event-loop proxy; the listing uses `Store::verify_quick` (presence and size, no hashing; test in `a_tampered_line_is_detected`). Restore, copy and view still hash what they read. Not measured on a 2 GB document; not run in a real window.
- **F12. Restoring while the document is open in our editor is silently undone** by the editor's next save (Sheets
  autosaves every ~2 s while dirty). Both states stay in history. Fix (smallest): "close the document first" in
  `hist.restore.body`.
- Fixed in `39e77a4` (documentation choice, no code): `hist.restore.body` now says to close the document in the editor first, in all four languages (`i18n/shell.tsv`, status `reviewed:agent-sonnet:2026-10-24`). An open editor is still not told about the restore.
- **F13. Patch `gridcraft/0144` has no effect:** `history_cmd` returns `EngineError::BadParams`, whose Display adds
  "invalid parameters for `office.history`: ", so the whole-string catalog lookup never matches and the toast stays
  English (`patches/gridcraft/0143-...patch:407`; upstream `crates/engine/src/lib.rs:49`). The test checks `contains`.
  Fix: `EngineError::Other(msg)`.
  Fixed in `5905293`: `gridcraft/0144` now also makes the message an `EngineError::Other`; catalog key at `i18n/gridcraft.tsv:219` matches the string exactly. Not covered by a UI test (the toast path needs egui).
- **F14. History lock and retention** (`history.rs:219-243`, `:628-661`): a stale-lock takeover by delete can remove a
  fresh lock, and `Drop` removes whatever lock is there; the newest version of every document ever saved is kept forever
  (deleted files, View copies, `name (n).ext` downloads) with no way to delete a document's history (personal data on
  shared PCs); `.tmp` leftovers in `objects/` are never cleaned. Phase 5 with the server store.
- Fixed in `851c00b` (Phase 5, `p5-f14`): the lock file holds its owner's token; `Drop` removes it only when the token is still ours, and a stale takeover renames the file aside and puts it back if it was not the stale one (test `a_lock_is_removed_only_by_its_owner`). The `.tmp` sweep already existed (`sweep_tmp`, from WP11, run by every prune; test `stale_tmp_files_are_swept`), so no change there. New `history_forget` IPC (worker thread like the other `history_*`) calls `Store::forget_id`, also for a file that is gone; the History panel has a "Delete history" button with a confirm dialog (5 shell strings, 4 languages; test `history_forget_deletes_the_history_only`, spec case in `shell.spec.ts`). Not changed: the newest version of a document is still kept until the user deletes its history. Not run: the Playwright shell spec (needs the release `hoshiya-serve` and the shell dist).
- **F15. Release builds honour `WEBVIEW2_*` environment variables**, and the comment at `host.rs:116-117` says no variable
  can pick an executable. `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=...` opens an unauthenticated CDP
  port on loopback (our own `scripts/desktop-editors-check.mjs:36` uses exactly that against the debug build); another
  local user could then drive the trusted page and its IPC. Needs the variable set in the victim's environment. Fix: in
  release builds refuse (or fall back to the browser) when `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` or
  `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` is set; correct the comment.
- Fixed in `39e77a4`: a release build refuses to open the window while either variable is set (`host.rs` `start`, `webview2_env_override`, test `webview2_env_that_changes_the_browser_is_found`); the comment at `host.rs:116-117` is corrected. The debug build still honours them (the desktop checks use them). Not run: a release build with the variable set.
- **F16. Another local user can stop the host from starting** by creating the predictable pipe name first
  (`host.rs:352-357`, `:409-411`): `send` correctly refuses the foreign server, then a launch with no files returns an
  error and no window opens. Fix: carry on as a full instance with no pipe server.
  Fixed in `f488c00`: `hand_over` returns `CarryOn` when nobody answers and there is nothing to open, and the host runs with no pipe server (`host.rs`, test `a_squatted_pipe_name_does_not_stop_a_launch`). With files it still opens them in-process and exits, as before.
- **F17. Browser fallback only: the `k` cookie is not port-scoped** (`services/suite-serve/src/lib.rs:201`): a page of
  another local user on another 127.0.0.1 port is same-site and can overwrite it (DoS) or ride it to our static files and
  converter. The WebView path is unaffected (no other origin loads in it). Fix: a per-launch cookie name.
  Fixed in `f488c00`: the cookie is `k<hash of the launch key>` (`suite_serve::cookie_name`); the old `k=` is no longer accepted (tests `desktop_key_gates_every_request`, desktop `tests/host.rs`, fixed in `39d8a52`).
- **F18. XDFU: Docs Save As onto its own working path is treated as a copy** (`docs-office/src/layer.rs:135`, no
  `path == s.path` check; Sheets and Slides have it), and the `file.save` comparison at `:137` is case-sensitive on Windows.
- **F19. XDFU: text-output stamping fails closed** (the copy is deleted and the command fails: `layer.rs:173-183`,
  gridcraft 0143 `:574-577`, deckcraft 0143 `:535`), inside a "nothing is ever refused" design; Docs reads `params.path`
  where the real output may differ. deckcraft records an unstamped copy as stamped when `alloc_id` runs out (0143 `:509`).
  Docs `mailings.finish {path}` and the desktop host's in-window (wasm) editors are not stamped: add both to the limits list.
- **F20. MSI build and test details:** `build-msi.ps1:81,188` sign from `LocalMachine\My` while `signing.md:23,36` puts the
  certificates in `CurrentUser\My`; ProductCode is UUIDv5 of id+version with no `AllowSameVersionUpgrades`
  (`suite.wxs:5`), so a second 0.1.0 pilot build cannot install over the first (error 1638): bump the version per pilot
  build; UpgradeCode derives from `[product] id`, not a frozen `upgrade_code` (PLAN §7.1); no ICE validation
  (`wix msi validate`); the VC runtime DLL list (`build-msi.ps1:62`) is from a one-off dumpbin, not checked per build;
  `msi-test.ps1`: OPENWITH uses filter 0 (all handlers) where the right-click menu shows the recommended ones (`:164`),
  `AppActivate('Рабочий стол')` is not checked before keystrokes (`:181-184`), no try/finally around install..uninstall,
  the editors run elevated in the test, and the PDF fixture comes from git-ignored `spikes/` (`:37`).
  Fixed in `f488c00` (partly): (1) `MajorUpgrade AllowSameVersionUpgrades="yes"` with `ProductCode="*"` (a new code per build), so a rebuilt MSI replaces the installed one with no uninstall; (2) `UpgradeCode` is frozen in `brand.toml [product] upgrade_code` (the value the pilot MSIs already carry, `{2EA291EA-...}`), the script no longer derives either code; (3) `build-msi.ps1 -SignStore` defaults to `CurrentUser` like `sign.ps1` and `signing.md`; (4) try/finally around install..uninstall in `msi-test.ps1`; (5) `wix msi validate` runs (below). ICE result: ICE43/ICE57 errors on the five shortcut components (non-advertised shortcuts keyed on a file; a per-machine install with Start-menu shortcuts), ICE60 warnings, and ICE61 (expected: same-version upgrades). Still open: ICE43/57, the VC runtime list check, the OPENWITH filter, the `AppActivate` check, elevated editors, the `spikes/` fixture. Not run: any install or upgrade (needs elevation).
  Further (branch `p4-msi`, test MSI `E:\office-target\p4-msi-test\hoshiya-0.1.0-x64.msi`, not the owner's file): (1) ICE43/ICE57 gone: `wix msi validate` rc 0, only ICE60 (3, non-font files) and ICE61 (same-version upgrade, intended) warnings remain. Each shortcut now sits in its own component `SC_*` with an HKCU registry KeyPath (`Software\<data dir>\Shortcut\installed`); an HKLM KeyPath was tried first and still fails both ICEs (they treat ProgramMenuFolder shortcuts as per-user data even in a per-machine package). Upgrade safety, read from the MSI tables: the 16 file and `OW_*` components have the same ComponentId, attributes and KeyPath as the a638aef MSI; the 5 `SC_*` components are new (new GUIDs); UpgradeCode is the frozen one; `RemoveExistingProducts` is at sequence 1401, right after `InstallValidate` (1400), so the old product is fully uninstalled before the new one installs and the moved shortcut does not conflict with the old `C_*` component rule. Cost: the HKCU value exists for the installing user only; non-advertised shortcuts never trigger repair, so nothing else depends on it. (2) `build-msi.ps1` runs `dumpbin /dependents` on every staged exe and fails if the VC runtime imports (vcruntime/msvcp/concrt/vccorlib/mfc/vcomp) differ from the shipped list; it also fails if dumpbin is missing or finds none. Today: vcruntime140.dll, vcruntime140_1.dll, as shipped. (3) `msi-test.ps1`: OPENWITH uses filter 1 (recommended), prints the all-handlers count; `AppActivate` result is checked (throws; the existing try/catch reports it); the PDF fixture is the tracked `docs/phase4/wp23/fixtures/namuna-hisobot.pdf`; the elevated editors cannot be avoided without changing what is measured and are noted in the script header. New `build-msi.ps1 -Out <dir>` builds beside an installed MSI. Still open: none of this was installed or upgraded (needs elevation and the owner); the new OPENWITH filter 1 has not been run, so it may need a look if the recommended list omits our handlers.
- **F21. The converter page drops a file chosen before its script has attached its listeners.** Found while scripting the
  desktop flow: right after navigating to `/convert/`, `setInputFiles` was lost in 4 of 6 host runs (the page showed "no
  files yet", `#o-to` empty and disabled); waiting for the page's load plus 3 s made 2 of 2 runs pass. `init()` attaches
  the `#pick` listener only after `await fetch('/i18n.json')` (`services/convert-api/ui/app.js:374-377`). A person is very
  unlikely to pick that fast; a test is not. This may be the "`#o-to` stayed disabled" `tls.spec.ts` flake of
  `integration.md` (WP22 table), **not proven**. Moving the listeners above the first `await` did **not** make my host runs
  reliable (the event can also fire before the script runs at all), so I reverted that change; the fix belongs in the specs
  (wait for the page to be initialised).
  Fixed in `5905293`: product-side guard, after `init()` has attached its listeners it adds any `#pick.files` already chosen (`app.js`, after `await start()`), which also covers the event that fired before the script ran.
  Further in `f488c00`: all listeners are now attached before the first `await` (the i18n fetch moved below them), so a file chosen while the catalog or the format list loads is added at once and `start()` renders again; the post-`start()` guard stays for a change event that fired before the script ran. New spec `a file chosen before the page texts arrive...` (`converter.spec.ts`, i18n delayed 1.5 s) passes, but it also passes with the earlier guard alone, so it is regression cover, not proof. The cause of the 4-in-6 host losses is still not found.

- **F22. Slides' window title has no file name** (owner's MSI run: `hoshiya-slides: title [Hoshiya Slides]`, also for the
  second window), unlike Docs, Sheets and PDF. With two presentations open the taskbar cannot tell them apart. Open; a
  title-setting line in the deckcraft overlay (carried patch) is the likely fix, not investigated.
- Fixed in `39e77a4`: carried patch `deckcraft/0144` sends `<file name> — <product>` as the window title (test `the_window_title_has_the_file_name_once_a_file_is_open`). Not re-run on the MSI.

### Info

- Network-share check after the file is touched: `suite-desktop/src/lib.rs:173` (`is_file`) follows a junction or mapped
  drive before the network check at `:177`, and a failing `canonicalize` counts as allowed. Only paths the user opened before.
- `nav_ok` allows top-level `blob:` of our origin (`lib.rs:255-257`); IPC from it is refused (`lib.rs:1046`). Drop it if
  Ctrl+S still works without it.
- Editor iframes are same-origin and can call `parent.ipc.postMessage` with the top page's origin. The IPC surface is
  narrow (only known paths), so a hostile document that gets JS in an editor gains little.
- Release builds accept `--dist`, `--shell`, `--brand`, `--bin-dir` (`lib.rs:716-723`): a crafted shortcut can run the
  signed host with another shell. No privilege gain; AppLocker still checks the child exes. Threat-model note.
- The single-instance DACL is by SID only (`suite-winsec/src/sys.rs:152`): an elevated first instance accepts messages
  from the same user's medium-integrity processes. Optional: add `S:(ML;;NW;;;HI)` when elevated.
- Mark-of-the-Web: whether WebView2 writes `Zone.Identifier` for `blob:` downloads is not verified; restore by rename
  (`history.rs:189-207`) drops the stream and replaces a symlinked path with a regular file.
- The XDFU audit records the file name unredacted (`xdfu-core/src/lib.rs:265`, `redact = false`); HRM file names can
  carry personal names. The audit key sits beside the log in the user-writable folder (no protection against the user).
- User data after uninstall: `%APPDATA%\Hoshiya` (settings), `%LOCALAPPDATA%\Hoshiya\Host\webview2`,
  `%LOCALAPPDATA%\Hoshiya\history` (document snapshots) stay. Correct for a per-machine MSI; now stated in
  `windows-verification.md` step 7; add a shared-PC note to the pilot instructions.
- VC runtime: app-local `vcruntime140.dll`, `vcruntime140_1.dll`, unmodified, Microsoft-signed (checked in
  `build-msi.ps1:60-67`), under the Visual Studio distributable-code terms; not updated by Windows Update. The static CRT
  would remove both the redistribution and the update question; owner's call.
- Docs' upstream "Make default" button opens `ms-settings:defaultapps?registeredAppMachine=WordCraft`
  (`upstream/wordcraft/apps/wordcraft/src/default_apps.rs:87`): user-driven, writes nothing; the query names the upstream app.

---

## 2. Security (PLAN WP24: CSP, IPC, TLS, XDFU off)

### 2.1 `services/suite-winsec` (the only `unsafe`, `src/sys.rs`)
Read in full. Sound:
- Every handle is an `OwnedHandle` (`sys.rs:124`, `:141`); `GetCurrentProcess` is a pseudo-handle, not closed (`:91`).
- Every `LocalAlloc`'d buffer is copied out and freed once, also on the error paths (`:54-57`, `:76-83`, `:175-177`).
- `token_info` sizes the buffer from the first call and keeps it `u64`-aligned for the pointer inside `TOKEN_USER`; a
  failing size query leaves `len = 0` and the second call fails cleanly (`:99-109`). `token_user` keeps the buffer alive
  while the SID inside it is copied (`:111-115`).
- The pipe: `D:P(A;;GA;;;<owner SID>)`, protected, no inheritance, nobody else (not even read); the descriptor outlives
  `CreateNamedPipe`, which copies it (`:147-178`). Low-integrity and AppContainer processes of the same user are refused
  by the default mandatory label and the missing package SID. `first_pipe_instance` and `reject_remote_clients` are set
  in `suite-desktop/src/host.rs:205`, `:383`. The client checks the server's SID before writing (`pipe_server_sid`,
  `:129-143`) and uses SQOS identification (`host.rs:157-191`). A PID-reuse race in `GetNamedPipeServerProcessId` would
  need the server to exit during the call and only makes the check fail closed.
- `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` (`:182-188`), plus `DependentLoadFlag` on the msvc exe
  (`windows-verification.md`).

### 2.2 `services/suite-desktop` (WebView2 host)
Checked and correct (sub-agent, spot-checked by me):
- IPC: a tagged enum with `deny_unknown_fields`, 8 KiB cap, generic error text (`lib.rs:186-240`); no exec or
  arbitrary-path message. Only brand editors start: `bin` is `[A-Za-z0-9._-]` (`lib.rs:72-80`), the exe is
  `bin_dir/<bin>.exe` (`:468`), `Command::new(exe).arg(file)`, no shell (`host.rs:367-373`); the page may open only paths
  in `history.json` (`lib.rs:464`, `:502`) or the history store (`:524-536`).
- `check_file` refuses NUL, over-length, relative, `\\?\` and `\\.\` prefixes, UNC from the page, `..`, `:` streams, DOS
  device names (also `COM¹`), unknown extensions, missing files (`lib.rs:133-181`; tests `:1003-1029`, `:1110-1124`).
- Settings: only `lang`, `toolbar`, `latin_alphabet`, values checked by suite-core (`lib.rs:19`, `:401-410`); `xdfu`,
  `server.url`, `spell_dictionaries` are refused (test `lib.rs:1169-1194`).
- Origin: exact `http://127.0.0.1:<port>` prefix followed by `/`, `?` or `#` (`lib.rs:243-249`; tests `:822-837`,
  `:1032-1059`). Navigation limited to our origin, new windows denied (`lib.rs:252`, `host.rs:497-498`); no ShellExecute of
  a page-supplied URL anywhere in the host. InPrivate, devtools off, accelerator keys and context menu off (`host.rs:470-476`).
- Downloads: our extensions only, no `:`/device names, never overwrite (`lib.rs:268-296`, `host.rs:479-489`).
- Desktop key: 32 bytes from `getrandom`, 64 hex digits (`host.rs:146-150`); not in `host.json`, the title, the editors'
  command lines, or the fallback dialog (`host.rs:419-422`, `:560`). Test hooks are debug-only (`host.rs:118-125`).
- Port: 127.0.0.1 only (`host.rs:263-276`).
- Open: F7, F10, F11, F15, F16 and the info items of section 1.

### 2.3 `services/suite-serve`
Read in full (`src/lib.rs`, `src/main.rs`):
- The desktop gate is the outermost layer, so it covers every route including the converter and its 403s
  (`lib.rs:265-276`). `/_k/<key>` answers 303 to `/` with `Referrer-Policy: no-referrer`, `Cache-Control: no-store`; the
  cookie is `HttpOnly; SameSite=Strict; Path=/` (no `Secure`: plain loopback HTTP); the compare is constant-time for equal
  lengths (`:187-227`); the key must be 32+ alphanumerics (`:267-270`). Logging: the server prints only its listen line
  and refusals (`convert-api/src/runner.rs:13,60,109`); no request path is logged, so the key never reaches a log. Test
  `services/suite-serve/tests/serve.rs:262-300` (no key → 403, `?k=` is not accepted).
- Static serving: tower-http 0.6.11 `ServeDir` (`services/Cargo.lock:3017`) percent-decodes, then refuses any component
  that is not `Normal` (so `..`, `..%5c..` on Windows, `C:` and roots) and DOS device names on Windows (read in the crate
  source). Test `serve.rs:199-200` (`/docs/..%2f..%2fCargo.toml`). An NTFS stream name (`index.html::$DATA`) serves the
  same bytes with a non-HTML type and `nosniff`, revalidated, not immutable (`lib.rs:158-160`).
- CSP per route: the shell and the converter keep the strict baseline (`script-src 'self'`, `frame-ancestors 'none'`);
  an editor adds only `'wasm-unsafe-eval'` and the sha256 of each inline script/style of its own `index.html`,
  `frame-ancestors 'self'`, `object-src 'none'`, `base-uri 'none'` (`lib.rs:96-115`). Spec
  `shell.spec.ts:102` and the per-editor "no violation" tests (`:235`); both flows of section 4 saw no CSP violation.
- Host/Origin guard against DNS rebinding sits inside (`convert-api/src/routes.rs:194-260`); tests `serve.rs:238-241`.

### 2.4 XDFU is watermark-only and off by default
- Default `off` (`crates/suite-core/src/settings.rs:114`); the only switch is `xdfu = "on"` in `settings.toml`, read once at
  start (docs `layer.rs:52`; the gridcraft/deckcraft 0143 `init`). The page cannot set it (`suite-desktop/src/lib.rs:19`,
  test `:1181-1194`); `SUITE_SETTINGS_DIR` is debug-only (`settings.rs:326-331`); no flag and no document field turns it on.
  A grif only matters when XDFU is already on (PLAN §10).
- Off means absent: no command, no detection, no audit file, byte-identical output (tests `xdfu_off_registers_nothing` in
  Docs and in both 0143 patches; `layer.rs:59-61`, `:74-77`).
- Never blocks: no hook overrides `before` (`docs-office/src/hooks.rs:16-19`, `layer.rs:86-125`, both 0143 layers);
  `edit.copy`/`edit.cut` pass through; no clipboard or screen-capture API in our code; an unusable log still makes the copy
  (copy 0, stderr: `xdfu-core/src/lib.rs:314-358`). Exception: F19 (text output fails closed).
- Before anyone sets `xdfu = "on"`: F4, F5, F18, F19, and Phase 3 m13.

### 2.5 History store
Checked and correct: the document id is SHA-256 of the canonical path, never the raw path (`history.rs:173-182`);
objects are named by their own hash and written atomically (`:189-207`, `:417-437`); restore writes only to a path in
the store and re-checks it as a page path (`suite-desktop/src/lib.rs:524-536`); restore snapshots the current file first
("Before restore", `history.rs:503-512`, test `restore_snapshots_unsaved_work_first`); a read-only or locked target fails
the rename cleanly; retention keeps at least 1 (`settings.rs:149-153`), always the newest and named versions; malformed
input is an `Err`, never a panic (`:270-340`); messages and names are length-capped with control characters removed;
`history.js` builds with `textContent`/`createElement` only (no `innerHTML`) and stores nothing in the browser. Open: F7,
F10-F12, F14.

### 2.6 Installer (`packaging/windows/suite.wxs`, `scripts/build-msi.ps1`, `scripts/msi-test.ps1`)
Read by the sub-agent and spot-checked: per-machine (`suite.wxs:4`), Program Files 64-bit (`:10`), all-users Start menu
(`:24`). Every registry write is HKLM; **no UserChoice, no `.ext` default value**: only
`.<ext>\OpenWithProgids\<ProgID>`, Capabilities, `RegisteredApplications`, App Paths (`suite.wxs:65-227`). ProgID commands
are quoted `"…exe" "%1"`. Every value sits in a component; `ForceDeleteOnUninstall` only on our own keys; RemoveFolder
for the Start menu folder (`:32`). No CustomAction, ServiceInstall, Run key, scheduled task or download in the MSI; no
WixUI, so no WiX runtime is shipped. MajorUpgrade with a stable UpgradeCode (`build-msi.ps1:23-30`). The WiX download is
SHA-256-checked (`fetch-wix.ps1:6,11-12`, `scripts/tools.lock`). Default-associations XML: Docs, Slides, PDF only, no
Sheets (`packaging/windows/default-associations.xml`). Install and uninstall on Win10: the owner's run, 0 failed checks, residue 0 of 400 (section 7).

---

## 3. Licences: architect sign-off

- `ci.sh deny` (root + 4 domains) **ok** in the full run; the services deny (`ci/deny/services.toml`) runs inside
  `convert` (section 5). New Phase 4 crates are permissive: wry, webview2-com, windows/windows-sys, getrandom, ron,
  toml_edit, winresource, tokio named pipes (MIT and/or Apache-2.0). The single exception stays MPL-2.0 symphonia
  (`NOTICE:200-222`).
- NOTICE covers: Fluent UI System Icons, MIT, 84 SVGs, provenance and hashes (`NOTICE:124-149`,
  `assets/icons/fluent/SOURCE.md`); Hunspell en_US/ru_RU, permissive, unmodified (`NOTICE:151-155`,
  `docs/phase4/licences.md`); the WebView2 loader (`NOTICE:157-163`, text still owed: F3).
- WiX 5.0.2 (MS-RL) is a build tool, never shipped (no extension DLLs, no custom actions); WiX 6+ OSMF avoided by the pin
  (PLAN F12).
- VC runtime: app-local, Microsoft-signed, distributable-code terms (info in section 1).
- Brand: `brand/logo/LICENSE.txt` says all rights reserved and "not MIT OR Apache-2.0"; `NOTICE:5-8` excludes
  `brand/logo/` from our licence; the MSI ships it as `licenses\logo-LICENSE.txt` (`build-msi.ps1:75`). Correct.
- **Condition:** F3 (third-party crate notices and the WebView2 licence text) before the MSI leaves this PC.

---

## 4. The gate flows, run here

Both scripts drive the real binaries with **real release CLIs** behind the converter (the shell spec uses a fake tool).
Synthetic inputs only: `web/shell/templates/tabel.xlsx`, a blank New document, `E:/office-target/p3/census/in/uz_latn.pdf`.

**Web** (`hoshiya-serve` release, Chromium, 1500×900): **`GATE-FLOW web: 11/11 ok`**.
- Docs: New → type "Salom gate" → Ctrl+S → the download is a docx whose `word/document.xml` holds the text (10.6 s from
  New) → Recent lists it → reopen from Recent (`wordcraft:opened` again).
- Sheets: open `tabel.xlsx` → type 424242 + Enter → `gridcraft:dirty` → Ctrl+S → the xlsx holds 424242 (19.6 s) → Recent →
  reopen, no failure, title kept; the screenshot shows the saved value in the reopened sheet.
- Converter: PDF → DOCX through `/convert/`, the downloaded docx has text runs (4.6 s, 2,795 B).
- No page error, no CSP violation. This closes the gap that `shell.spec.ts` has no Sheets round trip and no real
  PDF→DOCX (its converter test is txt→pdf with a fake tool).

**Desktop** (`hoshiya.exe` debug build from the full run's services target, driven over CDP; release editors as
`--bin-dir`; real `%USERPROFILE%\Downloads`, every file the host wrote moved out afterwards, every editor it started
stopped): **`GATE-FLOW desktop: 14/14 ok`, twice in a row** (runs 9 and 10).
- For Docs, Sheets and Slides: New in the shell → an edit (text, a number, New Slide) → Ctrl+S → the host's download
  handler writes the file and its content holds the edit → Recent (from the host) lists it → Recent > open starts the
  native editor (`hoshiya-docs.exe` etc.) on that path (Cyrillic name included) and it is still running 6 s later.
- PDF → DOCX through `/convert/` inside the host, the docx written by the download handler (9.4 s, 9.8 s).
- Earlier runs failed on my script, not the product: process matching by a Cyrillic file name (fixed by matching the exe),
  and F21 (fixed by waiting for the page). Run 4 was cut by a closed pipe and left two of its editors and three files; I
  stopped those two PIDs (started by my run, checked by path) and moved the files.
- Not covered here: the MSI-installed binaries (the owner's MSI run opens all four types from Explorer's command line, section 7), the PDF editor's "Open" path (the flow converts a PDF; opening a
  PDF in the viewer is in `smoke-desktop.ps1` and the templates check).

---

## 5. `ci.sh full`

`scripts/ci.sh full`, run `gate4-20261024-0808`, started 2026-10-24 08:08 on the shared tree at `2047fe2`, finished 14:46.
**Exit 0: all 29 jobs ok, 397 min 54 s (6 h 38 min).** Summary `E:/office-target/p1/ci/gate4-20261024-0808/summary.md`,
one log per job next to it. Each job reads the tree when it starts: `test-ours` compiled `suite-core` before `00c7f5c`
(its two new tests ran separately: `cargo test -p suite-core --lib history`, 9 of 9, plus clippy `-D warnings`, in
`E:/office-target/p4/review`); every later job (check, bind, convert, shell, build) compiled the fixed `suite-core`, and
`shell` ran `history.js` with `a59dffb`. `msi-test.ps1` is not part of any CI job. The machine was shared with the sync
dry run, the two gate flows, a debug host build and the owner's MSI runs.

| Job | Result | Seconds | Notes |
|---|---|---|---|
| queue | ok | 8 | |
| lint | ok | 73 | upstream clippy 1/3/2/1 warnings, all baselined |
| gen-check | ok | 18 | |
| deny | ok | 11 | root + 4 domains: advisories, bans, licences, sources ok |
| brand | ok | 309 | |
| i18n | ok | 6 | |
| test-ours | ok | 1,919 | nextest 227 / 133 / 47 / 46 / 74 passed (1, 2, 0, 0, 4 skipped) |
| test wordcraft / gridcraft / deckcraft / pdfcraft | ok | 2,121 / 4,388 / 2,164 / 4,913 | |
| check docs / sheets / slides / pdf | ok | 132 / 831 / 123 / 158 | desktop + wasm32 |
| bind | ok | 471 | |
| convert | ok | 327 | services clippy, nextest **236 of 236**, cargo-deny services |
| convert-e2e | ok | 561 | API e2e 2/2; `directions.sh` **43 PASS / 0 FAIL** (incl. `pdf2docx-native`, combine, split, extract); converter Playwright 12 passed, 1 skipped |
| shell | ok | 581 | suite-serve nextest 8/8; shell Playwright **21 of 21** (History panel `:355`, `:461` included); templates 5/5 |
| build docs / sheets / slides / pdf (desktop, web, cli) | ok | 1,042 / 345 / 1,212 / 1,165 | |
| translit-npm | ok | 28 | |
| translit-preserve | ok | 136 | 8 of 8 IDENTICAL |
| sizes | ok | 475 | see below |
| brand (binaries) | ok | 344 | |
| web-smoke | ok | 12 | 4 passed |
| actionlint | ok | 1 | |

Sizes (brotli, bytes; budget = `base_br + delta_br_max`, the shell has its own row):

| Bundle | Brotli | Budget | Headroom | JS brotli / max |
|---|---|---|---|---|
| docs | 9,854,669 | 9,871,648 | 16,979 | 20,760 / 24,576 |
| sheets | 4,942,722 | 4,966,771 | 24,049 | 20,040 / 24,576 |
| slides | 9,494,503 | 9,524,217 | 29,714 | 19,915 / 24,576 |
| pdf | 9,571,586 | 9,599,462 | 27,876 | 20,543 / 24,576 |
| shell | 54,728 | 65,536 | 10,808 | |
| npm (uz-translit) | 30,817 | 49,152 | 18,335 | 3,142 / 16,384 |

Docs has the least headroom (17 KB, 0.17 %); the shell grew from 48,443 B (WP22) to 54,728 B with the History panel.
`uz-Latn-2026` in the apps (an estimated 60-90 KB) does not fit any editor's headroom (STATUS).

Disk: E: 241 → 163 GB free over the run (CI targets, the dry run's scratch, my `E:/office-target/p4` scratch); C: 8.1 GB
free (97 % used). Nothing was deleted.

---

## 6. Upstream sync dry run (PLAN §2.5, the carry cost of the second sync)

`scripts/sync-upstream.sh --dry-run --survey --gates quick all`, started 2026-10-24 08:28:52, **9 min 40 s**, exit 1
("DRY RUN INCOMPLETE"). The monorepo tree, index and HEAD were untouched. Report `E:/office-target/p1/sync/reports/2026-10-24.md`,
log `…/2026-10-24-rebase-log.tsv`, scratch `E:/office-target/p1/sync/mono-2026-10-24-082852`. Targets are `origin/HEAD`
(no newer tags), two days past the WP0 BASEs.

| Repo | BASE → upstream | Upstream commits | Patches | Clean / 3-way / conflict |
|---|---|---|---|---|
| wordcraft | `2e6db0b` → `9130181` | 565 | 16 | 13 / 1 / 2 (`0004` canvas.rs, `0110` i18n/mod.rs) |
| gridcraft | `2ebdf16` → `ac8da7d` | 614 | 47 | 17 / 7 / 19 (many after `0002` are probably a cascade; not separated) |
| deckcraft | `fe510bd` → `b47195f` | 29 | 19 | 18 / 0 / 1 (`0001` recovery.rs) |
| pdfcraft | `3e30645` → `53199fb` | 876 | 6 | 4 / 0 / 2 (`0110` dates.rs and i18n, `0111` ATTRIBUTION.toml) |
| craft-libs | `1b9cf19` → `0073927` | 198 | 0 | - |

The script's own totals (54 / 9 / 24) do not match the per-repo tally from its report; I quote the per-repo rows.

**The Phase 4 carry (the number that replaces "upstream's answer to a layout hook", PLAN §2.5):**

| Patch | Sync 1 (WP0, landing) | Sync 2 (this dry run) |
|---|---|---|
| wordcraft 0122 (ribbon hook), 0123 (AutoSave mark) | new on the WP0 BASE | clean, clean |
| wordcraft 0124 (history message catalog) | new | 3-way |
| gridcraft 0140 (simple toolbar) | new | **conflict**, `crates/ui-egui/src/ribbon.rs:567`, 2 hunks (the one-line call-site branch) |
| gridcraft 0141 (command hook), 0142 (test), 0143 (history/XDFU), 0144 | new | clean ×4 |
| deckcraft 0002 (host bridge), 0140, 0141, 0142, 0143 | new | clean ×5 |

So over a 2-day, 565/614/29-commit upstream move, 1 of the 12 Phase 4 patches conflicts, and in the expected place (the
ribbon call site, PLAN §2.3); our new files carried clean. Minutes per patch were not spent (dry run, no resolution).
Gates: register (88/88), i18n stale/roundtrip/lint pass; verify-queue fails on `craft-libs` ("pristine commit subtree !=
BASE tree", unexplained, probably the dry run's own snapshot step since craft-libs has no patches); the cargo gates did
not run because the rebase did not complete. craft-fonts moved upstream (`3d9ab3a` → `43a9944`) and is pinned by hand.
Upstream velocity is the R1 risk as before: the next real sync is a hand-merge of about 20 gridcraft patches.

---

## 7. PLAN §12.3: the gate table

| Gate item | Status | Evidence |
|---|---|---|
| **Web:** a new user creates, edits, saves and reopens a document, a spreadsheet and a presentation | **met** (the "new user" part: WP23, pending owner) | `GATE-FLOW web: 11/11 ok` (section 4: Docs from New, Sheets with a typed number); `shell.spec.ts:139` Docs and `:176` Slides round trips, `bridge-slides.spec.ts` 9 tests, `templates.spec.ts` 5/5, in `ci.sh shell` (section 5) |
| **Desktop:** the same | **met (scripted)**; WP23 desktop sessions **pending owner** | `GATE-FLOW desktop: 14/14 ok` ×2 (section 4); `integration.md` desktop checks (`desktop-editors-check.mjs` 16/16, templates 8/8, toolbar shared); the installed MSI opens all four types from paths with spaces and Cyrillic (owner's run 14:38, row "Open with" below) |
| … and converts a PDF | **met** | real PDF→DOCX through `/convert/` in Chromium and inside `hoshiya.exe` (section 4); `directions.sh` `pdf2docx-native` in `convert-e2e` (section 5) |
| … without instructions | **pending owner** | WP23: `docs/phase4/usability.md` does not exist yet |
| Bundle budgets are met | **met** (Docs headroom 16,979 B) | `ci.sh sizes` (section 5), the `[shell]` row `web/budgets.toml:82-83`; lazy load: `shell.spec.ts:68` (no wasm on the home screen), `:117` (Docs fetches only the Docs bundle), and the web flow fetched one wasm per opened editor |
| Brief task 1: home screen (new, templates, recent, search) | **met**; desktop "search folders" **not done** | New cards, templates, Recent, search over Recent: `web/shell/index.html:21-48`, `shell.spec.ts:139` (search narrows Recent), `:276` (templates); desktop Recent from the host (both flows); `desktop-host.md:30` lists "search folders" as not done |
| Brief task 3: simplified toolbar by default, full-ribbon toggle on desktop, plain Uzbek labels; Show/Hide ¶ visible | **partly met** | toolbar screenshots `docs/phase4/evidence/toolbar/` (17: Docs and Slides in uz-Latn only, Sheets in en/ru/uz-Latn/uz-Cyrl), toggle shared through settings (`integration.md`, `toolbar-share-check.py`), ¶ in the Docs preset (`domains/docs/crates/docs-office/src/preset.rs:18`, tests in `tests.rs`); missing: the 5-language set for Docs and Slides, `uz-Latn-2026` in the apps (deferred, STATUS), WP23 "find ¶" (owner) |
| Brief task 4: single installer (Windows; macOS none; Linux optional) | **met on Win10 (this PC)**; Windows 11 and domain **pending owner** | `windows-verification.md` (175.8 MB MSI, WiX 5.0.2, decompile check), `msvc.md`; `msi-test.ps1` fixed for the owner's run (F2); F3 before it leaves this PC; Linux (WP21, optional) not done |
| Brief task 5: shared services | **met** (language: met in 6413f7a (code d3deade, register 8f64651)) | met: `suite-core` settings/recent/fonts/templates/history (`test-ours` ok; history 9/9 incl. the two new tests), toolbar setting shared Sheets↔Slides, dictionaries en_US/ru_RU (deckcraft 0142, `licences.md`), one font copy (`fonts.rs`); the four apps read `lang` from `settings.toml` at start (`shared-services.md` section 6.1; `office_overlay_tests::shared_lang_picks_the_catalog_language` in each app, overlay ON) |
| Owner: "Open with" for every type, defaults by user or GPO, single instance | **met on Win10 (this PC)**; "Always" by hand, Win11 and the GPO on a domain VM **pending owner** | owner's elevated re-run 2026-10-24 14:38-14:42 with the fixed script, **0 failed checks** (`docs/phase4/evidence/openwith/msi-test.log`, transcript redacted, `6ec6c20`; `install-added.txt`, `uninstall-residue.txt`, `bd13971`): `INSTALL` ok; `REGISTRY` 27 extension registrations, 4 ProgIDs, 4 Capabilities, 5 shortcuts, no default changed; `OPENWITH` docx/xlsx/pptx/pdf ok, ours listed as recommended; `OPEN spaces+cyrillic 4/4`; `SINGLE` docs/sheets/slides new window, pdf tab (second launch exits, 1 process); `UNINSTALL` ok, **residue 0 of 400 installed registry lines, 0 files left**; `ALWAYS docx NOT DRIVEN` (UI Automation found no dialog: manual check owed). The first run (09:45) gave the same rows and stalled in the diff (F2). Defect: Slides' window title shows no file name (F22). Static: `suite.wxs:65-227` (OpenWithProgids, Capabilities, no UserChoice); GPO XML `packaging/windows/default-associations.xml`, README corrected (F6); host single instance `smoke-desktop.ps1` (second launch handed over) and `suite-winsec/tests/pipe.rs` |
| Owner: history (Phase 4 part), XDFU (off by default), accounts optional | **met** | history: `history.rs` tests 9/9, shell History spec `shell.spec.ts:355`, `:461`; F1/F8/F9 fixed, F7/F10-F12/F14 open (none loses data now); XDFU off: section 2.4; accounts: `docs/phase4/accounts.md`, `tls.spec.ts` (run by `ci.sh convert-ui`, not part of `full`: WP19 commit `4384b50`), local mode needs no login |
| B-3 re-decision pack | **partly met** | `docs/phase4/mobile-evidence.md` (360/412 viewports, the 360 dp fit reading, M3 stage timing, the recommendation: keep A-web conditional, decide at Phase 6 start); the carry table: section 6 here; **missing: the handle design** (`mobile-evidence.md` §5); (iii) a real phone cannot be produced (B-10) |
| Carry-forward closed or re-homed | **met** | section 8 |

---

## 8. Carry-forward from Phase 3 (§10.6) and to Phase 5

| Phase 3 item | Status |
|---|---|
| 1. Sync with hand-merge (M2), NumLocale re-run | closed in WP0 (`sync-report.md`; NumLocale tests and WP14 matrix unchanged) |
| 2. D7 (pdfcraft ru "Move down") | closed: `pdfcraft/0111` (`c3edebb`, `d703595`) |
| 3. To Phase 8: D1, D8, D9, m9 | re-homed, unchanged |
| 4. Minors: n1-n4, m6 | closed: `3d1841c` (n1, n3, n4, m6), `4384b50` (n2) |
| 4. m10 (PDF locale probe code change) | **re-homed to Phase 5**: the premise was re-checked after the sync (`sync-report.md:105-107`), the probe change is not made |
| 4. m5 rest (Phase 5), m13 rest (before XDFU on), m14 (Phase 10) | re-homed, unchanged; m13 now sits with F4, F5, F18, F19 |
| 5. M3 (Android open) | diagnosed on desktop (`mobile-evidence.md` §3); the phone run moves to the B-3 decision at Phase 6 start |
| 6. Server mode over TLS in a real browser | closed: WP19 `tls.spec.ts`, `docs/phase4/tls.md` |
| 7. Corpus (O9) not delivered | open, owner |
| 8. `C:\c` housekeeping | open: the folder still exists; the owner can delete it |

New carry to Phase 5: F7, F10-F17, F20, F21; history repair of a torn index (F1 remainder); the third-party notices
(F3) if not done before the pilot; the XDFU set (F4, F5, F18, F19) before XDFU is switched on; `GATE-FLOW` scripts of
section 4 into `ci/` (the desktop one needs a desktop session, like the `desktop` job).

---

## 9. What I did not verify

- "Always" (the dialog automation found no window in either owner run), Windows 11,
  a domain PC, AppLocker/WDAC (owner hardware).
- The WP23 sessions and the desktop keyboard/Narrator walk (`a11y-baseline.md`: not run; the web editors fail the
  keyboard walk inside the iframe, the desktop apps expose unnamed UIA nodes).
- F10 (mapped drive) and F11 (timings) are read from code, not run. F5 and the deckcraft revision side effect are from the
  sub-agent's reading.
- Whether WebView2 policy registry keys can also add browser arguments (F15), and Mark-of-the-Web on downloads.
- The sync dry run's cascade separation and the craft-libs verify-queue failure (section 6).

## 10. Decisions for the owner

1. "Always" by hand on this PC (right-click a .docx > Open with > Hoshiya Docs > Always, check a double-click, then put Word back); then Windows 11 and the domain VM with the same script.
2. Brief task 5, the apps' language from settings: done in 6413f7a (owner 2026-10-24); nothing to decide.
   Follow-up (main session, 2026-10-24): one rule for all four apps, a fixed shared `lang` always wins at start, the saved language applies only for `auto`, an in-app change is session-only (shared-services.md 6.1); `ci.sh deny` pdf red on RUSTSEC-2026-0387 fixed by crypto-bigint 0.7.6 (+ ctutils 0.4.5) in the carried lock patch pdfcraft/0900.
3. F3: OK to add `cargo about` (dev tool, permissive) for the third-party notices before the MSI goes to IT.
4. WP23 staff sessions.
5. The handle design for the B-3 pack: write it now (architect, ~0.5 d) or with the B-3 decision at Phase 6.
