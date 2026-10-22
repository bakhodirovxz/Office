# Phase 4 security review: the shell, `suite-serve` and the desktop host (2026-10-22)

**Scope.** This review covers four parts of the code:
- `services/suite-serve/` (WP7, 5bbc34d);
- `web/shell/` (`shell.js`, `index.html`);
- `services/suite-desktop/` (WP9, e7d923d);
- the WP19 changes to `services/convert-api` (4384b50).

It also covers the parts of `convert-api` these depend on: `routes.rs` (guard, headers, `router_with`), `app.rs` (the allowed hosts) and
`runner.rs`. I read the editor bridges on the far side of the postMessage channel:
- wordcraft patch `0001`;
- gridcraft `apps/gridcraft-web/src/host.rs`;
- pdfcraft `apps/pdfcraft-web/src/main.rs`.

I also read the vendored `wry 0.57.0`, `tower-http 0.6.11` and `webview2-com-sys 0.39.1` sources. Line numbers refer to e7d923d.

The review is read-only. I built nothing and ran no product binary. The only commands I ran were `cargo deny` for two targets (§9 below)
and an Authenticode check of `WebView2Loader.dll`.

**Threat model** (from the task):
- a hostile web page in the user's normal browser, with the server on loopback;
- a malicious document opened in an editor (wasm on the web, native exe on the desktop);
- another local process, run by the same user or by a different one;
- a crafted path from Explorer's "Open with".

The same user's own processes are **not** a security boundary. They can already run anything as that user, so I note them only where
they cross something else (a lower integrity level, another user).

**Severity:**
- **Blocker:** someone in the threat model crosses a trust boundary in the default install. Fix it before the Phase 4 gate and before
  any pilot MSI.
- **Major:** a gap that is reachable, or plaintext at rest that grows without a bound. Fix it in Phase 4.
- **Minor:** defence in depth, a cheap fix, or a trap for a later WP.

**Verdict.** The core design holds:
- The postMessage bridge checks origin and source on both sides.
- Desktop actions never travel over HTTP.
- The IPC parser is strict.
- Process launch cannot be injected.
- The Host/Origin guard covers every `suite-serve` route.
- The per-route CSP is correct.
- WP19 does not regress any Phase 3 fix.

The weak spots are all **cross-user on a shared Windows host** (RDS, fast user switching) or **data at rest**:
- the single-instance pipe can be squatted (B1);
- the in-process server has no authentication for other local users (M1);
- the shell copies documents into a WebView2 profile that gets a new origin on every launch (M2).

---

## Blocker

### B1. The single-instance pipe can be squatted by another user, who then receives every file path and blocks the app
`host.rs:206-211` (the name), `host.rs:220-226` (`first_pipe_instance`), `host.rs:227-249` (the fallback), `host.rs:62-71` (`send`).

**Status: fixed (2026-10-22).** The reviewer's fix as written: client `send` (`host.rs` `SQOS_IDENTIFICATION`, `server_is`) opens with SQOS identification and checks the server's SID before writing; the server creates every instance with `D:P(A;;GA;;;<SID>)` (`suite_winsec::create_user_only_pipe`); the name is `<id>-host-<SID>-<session>`. Tests: `host::tests::a_server_run_by_another_identity_is_refused` (injected foreign SID and an error), `host::tests::send_writes_only_to_a_server_of_our_own_identity` (real pipe, matching SID delivers, foreign SID writes nothing), `suite-winsec/tests/pipe.rs` (SID round trip, a client sees the real server SID, a second first-instance is refused), and the `a_second_launch_hands_its_file_to_the_running_instance` process test. The only `unsafe` is in `services/suite-winsec/src/sys.rs`. The SID comparison is by bytes, which is what `EqualSid` does. A real second account was not available, so the DACL refusing another user is by construction (SDDL), not observed.

- **The name is predictable:** `\\.\pipe\suite-host-<USERNAME>`. `USERNAME` is no secret. `query user` lists every session on an RDS
  host, and domain account names are public inside the domain.
- **What `first_pipe_instance(true)` checks:** only that *no* pipe with that name exists. It does not check who created it.
- **Squatting:** another user B creates the name first, with a DACL that grants Everyone write (one `CreateNamedPipe` call). B can do
  this at logon, or any time A has no `suite.exe` running.
- **What A's `suite.exe` then does:**
  1. Its own `create` fails (`ERROR_ACCESS_DENIED`).
  2. `send()` opens B's pipe for writing. It succeeds.
  3. It writes `{"type":"open","path":"C:\\Users\\A\\Documents\\<file name>"}` for every file on its command line.
  4. It exits 0 (`host.rs:238-239`).
- **Effect:**
  - Confidentiality: B learns the full path of every file that A opens through `suite.exe`. In this organisation, HR file names carry
    personal data (an order about a named employee).
  - Availability: A's file does not open and A's home window never appears. A plain launch sends only `focus`, so it does nothing.
- **Impersonation:** `std::fs::OpenOptions` does not set `SECURITY_SQOS_PRESENT` by default. The std docs of
  `OpenOptionsExt::security_qos_flags` warn about exactly this. A squatter that holds `SeImpersonatePrivilege` can therefore
  `ImpersonateNamedPipeClient` at impersonation level and act as A. Ordinary users lack that privilege, but service accounts have it
  (IIS app pools, LocalService, NetworkService, SQL Server). So a compromised service on the box can escalate to every user who
  starts the suite.
- **A smaller variant:** user A (local) and user A (domain) have the same `USERNAME`, so they collide on the name. The second one falls
  back after 2 s.
- **Answer to "is the pipe ACL default-secure across users?":** half of it is.
  - **Writes into our pipe:** yes. The default named-pipe DACL gives other users and anonymous *read* only, so they cannot write.
    Low-integrity and AppContainer processes cannot write either: the default medium label blocks write-up, and the DACL has no
    package ACE. `reject_remote_clients` is set on every instance (`host.rs:84`, `:224`). tokio 1.54 carries the RUSTSEC-2023-0001 fix.
  - **Squatting and impersonation:** no. Read-only connects from other users can also tie up instances (a small DoS).
- **Fix (about 40 lines, `windows-sys` is already in the lock file):**
  1. **Client:** open the pipe with `.security_qos_flags(SECURITY_IDENTIFICATION)`. Before writing, check that the server runs as us:
     - `GetNamedPipeServerProcessId`;
     - `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`;
     - `OpenProcessToken`, then `GetTokenInformation(TokenUser)`;
     - `EqualSid` against our own token's SID.

     Any failure counts as "not ours". In that case send nothing and take the existing stand-alone path (`host.rs:241-248`) without the
     pipe.
  2. **Server:** create every instance with an explicit, protected DACL `D:P(A;;GA;;;<own SID>)` through
     `ServerOptions::create_with_security_attributes_raw`. Other users can then no longer even connect.
  3. **Name:** use `<id>-host-<user SID>-<session id>` instead of `USERNAME`. This removes the local/domain collision. The name may stay
     guessable, because step 1 makes guessing useless.

  I recommend this over the other option, a random name kept in a user-only file such as `host.json`. That option also works, but it
  adds a first-start race (two launches both find no file) and a stale-file case. The SID check holds no state.

  **Test:** a unit test that a pipe created by the test with a foreign owner is refused by the client check (pass a SID that does not
  match). The smoke test also checks that a second launch is still handed off.

---

## Major

### M1. The desktop server has no authentication and every local user can reach it
`host.rs:128-174` (local mode, `auth = "none"`, `127.0.0.1:<random port>`), `app.rs:474-481` (the allowed hosts are the loopback
names), `routes.rs:271` (every client is the principal `local`). Phase 3 noted the same thing for `convert-api` as documentation
only (review-wp7 m2). Now `suite.exe` runs that server on every desktop for as long as the home window is open.

**Status: fixed (2026-10-22).** The reviewer's design: `suite_serve::Layout.local_key`, the `local_gate` middleware outside the guard (`suite-serve/src/lib.rs`), constant-time compare, the key from `getrandom` in `host.rs` `new_key`, the WebView and the browser fallback start at `/_k/<key>`. Tests: `desktop_key_gates_every_request` (403 without or with a wrong cookie, on `/`, the API, `/healthz`, the editors and unknown paths; 303 plus the exact `HttpOnly; SameSite=Strict` cookie for the right key; 200 with the cookie; no key configured gives the old behaviour and `/_k/` is a 404), and the process test in `suite-desktop/tests/host.rs`. `scripts/smoke-desktop.ps1` now treats a 403 without the key as "the server is up" and prints it.

- **Who can connect:** `netstat -ano` shows the port to every user. A user B in another RDS session, or a session left signed in by
  fast user switching, then has the same access as the owner. The Host/Origin guard does not stop B, because B sends whatever `Host`
  B likes.
- **What B can do:**
  - B can run conversions **in A's security context**. A's process stages and parses B's crafted files. Any parser bug, or any path
    that follows an external reference, then runs as A.
  - B can fill A's `%LOCALAPPDATA%\<product>\Host\store` up to the quotas.
  - B can hold A's `local` slots (`per_user_active` = 3), so A gets 429.
- **What B cannot do:**
  - B cannot read A's jobs. The ids are 128-bit, and there is no list endpoint.
  - B cannot use the IPC: it is WebView-only. The host test checks this at `tests/host.rs:126-127`.
- **Fix:** add a per-launch secret in `suite-serve` (about 30 lines, plus one test).
  - `Layout` gets `local_key: Option<[u8; 32]>`.
  - A middleware wraps the router outside the guard. `GET /_k/<hex key>` sets `k=<key>; Path=/; HttpOnly; SameSite=Strict` and answers
    303 to `/`. Every other request without that cookie gets 403. Compare the key in constant time.
  - The host opens `http://127.0.0.1:<port>/_k/<key>` in the WebView (`host.rs:288`) and in the browser fallback (`host.rs:345`).
  - The key is never written to `host.json`.
  - `suite-serve` alone (`main.rs`) keeps `None`, so the Phase 3 behaviour stays the same.
  - `referrer-policy: no-referrer` keeps the key out of `Referer`. The fallback's `explorer <url>` command line is readable only by the
    same user.
- **Bigger alternative:** serve the router to the WebView through a wry custom protocol, with no TCP at all, and open a port only for
  the fallback. That is more code and changes the origin, so I do not recommend it for Phase 4.

### M2. The shell keeps document copies in a WebView2 profile that gets a new origin on every launch, with no bound and no cleanup
`shell.js:7`, `:28-51` (IndexedDB "Recent", 200 MB **per origin**), `:99` (every save is copied), `host.rs:135-139` (the port is random
on every start), `host.rs:284` (a persistent `WebContext` under `%LOCALAPPDATA%\<product>\Host\webview2`).

- **Desktop:**
  - The origin is `http://127.0.0.1:<port>`, and the port changes on every launch. So every launch starts a new IndexedDB, and up to
    200 MB of saved documents pile up under the old ones. The browser fallback does the same in the user's default browser.
  - Plaintext HR documents therefore pile up on disk, one origin per launch:
    `...\webview2\EBWebView\Default\IndexedDB\http_127.0.0.1_<port>.indexeddb.*`.
  - The user cannot see them. "Remove" in Recent only reaches the current origin.
  - The shell never cleans them up. Chromium evicts whole origins, least recently used first, but only under global quota pressure
    (UNVERIFIED for WebView2 155).
  - A per-machine MSI uninstall cannot remove per-user profile data.
  - Functionally, Recent and the language (`localStorage` `sh.lang`) look empty after every restart. The host's own `recent`,
    `settings_get` and `settings_set` IPC exist (`lib.rs:330-336`), but the shell does not use them (`shell.js:12`, `:35-38`).
- **Server mode:**
  - Recent is per browser profile, not per `sub`, and "Disconnect" clears only `sessionStorage`.
  - On a PC or browser profile that two staff share, the second user sees and reopens the first user's documents.
  - PLAN §3.2 chose browser storage on purpose. The leak between users needs an owner decision.
- **Status: fixed (2026-10-22).** `host.rs`: `.with_incognito(true)`, the old `webview2/` profile folder is deleted at start, and a download handler (`download_target`, `Ctx::saved` in `lib.rs`) writes into the folder WebView2 chose, only for our file types, never over an existing file (`name (1).ext`), and records the path in `history.json`. **Deviation:** no native Save dialog. A blocking modal dialog inside the WebView2 download callback could not be tested here; the file goes to the default downloads folder and the shell shows the status line. `shell.js` picks the Recent source by mode: desktop = the host's `recent` IPC, the language from `settings_get`/`settings_set`, no IndexedDB and no `localStorage`; server mode (a token in `sessionStorage`) and the desktop host's browser fallback (`shell-config.json` carries `"local": true`) = names only in `sessionStorage` (`sh.recent`, 50 names), reopening asks for the file again, and `convert-api/ui/app.js` `disconnect()` clears it; the plain web host keeps the WP7 IndexedDB copies. The orchestrator's wording "server job ids" has no case yet: the shell starts no server job, so only names are stored. The old `shell` IndexedDB is deleted in the first two modes. Tests: `a_download_target_is_checked_and_never_overwrites`, `suite-serve` `shell_config_comes_from_the_brand_file` (the `local` flag), Playwright `shell.spec.ts` (desktop IPC stand-in, names mode), `converter.spec.ts` (Disconnect clears the names). Not tested: the real WebView2 download and incognito on disk (the smoke script checks that no `IndexedDB` folder appears).
- **Fix:**
  1. `host.rs:287`: add `.with_incognito(true)` (WebView2 InPrivate, `wry-0.57.0/src/webview2/mod.rs:414`). Nothing from the web layer
     is then persisted. The smoke test checks that the `webview2` folder holds no `IndexedDB` after a save.
  2. `shell.js`: when `window.ipc` exists, take Recent from the host's `recent` IPC, take the language from `settings_get` and
     `settings_set`, and skip `remember()`.
  3. Add a host download handler (`with_download_started_handler`). It shows a native Save dialog and records the chosen path in
     `history.json`. Desktop "New, then Save, then reopen" then works across restarts. This overlaps the open `GATE-FLOW desktop`.
  4. Server mode, an owner decision: either keep no copies, or key them by `sub`, clear them on Disconnect, and say so in the Recent
     note.

---

## Minor

**m1. IPC `open` takes a raw path from the page** (`lib.rs:312`, `lib.rs:90-125`).
- **Status (2026-10-22): fixed. `lib.rs` `Ctx::open`: from the page only a path in `history.json` (`Store::knows`); `check_file` refuses `:` after the prefix and DOS device names for every source (`dos_device`), and a page path whose canonical form is a share (`is_network_target`). Tests: `open_checks_the_file_and_starts_the_brand_editor`, `streams_and_device_names_are_refused`. The junction case itself (a real directory symlink to a share) is covered by the pure `is_network_target` test only.**
- **What the checks are:** the page-source rules (drive letter, no UNC, no `..`, a known extension, the file exists) are the only thing
  between page JavaScript and "start an editor on any local file".
- **Gaps:**
  - A directory symlink to a share passes, so an `\\host\...` target is reached through a drive letter.
  - `C:\x\a.txt:s.docx` (an ADS) passes.
  - On Windows 10, `C:\x\COM1.docx` makes `is_file()` open the COM1 device. It then fails, but only after the open.
- **Not a gap:** `p.is_file()` (`lib.rs:121`) and the editor opening the file later are not checked together. Only the same user can
  race them.
- **Fix:**
  - Accept `open{path}` from the page only for a path that is already in `history.json`. The shell only reopens host-known files, and
    picks happen host-side. The page-source path rules then become a second layer.
  - For both sources: reject `:` after the prefix and reserved DOS names. For page-source paths, `std::fs::canonicalize` and reject a
    result that starts `\\?\UNC\`.

**m2. The editors are same-origin with the shell, so splitting the CSP by route is cosmetic between them.**
- **Status (2026-10-22): fixed on the shell side. `shell.js` `save`: the file name is reduced to its last component and must have one of the app's own extensions, else `err.type`; the host's `download_target` applies the same allowlist again. Tests: `a_download_target_is_checked_and_never_overwrites`. SmartScreen is on again (m6). The sibling-origin split stays Phase 5.**
- **The real boundary is the origin.** Any script in `/docs/` can call `top.ipc.postMessage`, read the shell's IndexedDB and drive the
  shell. The IPC origin test even accepts `/docs/?host=parent` (`lib.rs:480`).
- **Today this needs script execution in our origin, which nothing grants:**
  - document hyperlinks follow only http, https and mailto (wordcraft `canvas.rs:2306-2318`);
  - new windows are denied;
  - no editor message reaches the IPC (`shell.js:84-103`).
- **Fix now:** in `shell.js:94-96`, accept `save` names only with the app's known save or export extensions. An editor fed a hostile
  document then cannot make the shell download `x.lnk` or `x.hta`. wry's default arguments turn SmartScreen off
  (`wry-0.57.0/src/webview2/mod.rs:303`).
- **Later (Phase 5):** serve the editors from a sibling origin (a second port), so that an editor compromised through a document cannot
  reach the IPC or Recent.

**m3. The editor cache rule makes a file immutable unless it is the page** (`suite-serve/src/lib.rs:145-156`).
- **Status (2026-10-22): fixed. `suite-serve/src/lib.rs` `hashed`: `immutable` only for trunk-hashed names, `no-cache` for everything else, 304 included. Tests: `only_trunk_hashed_names_are_immutable`, `cache_frame_and_guard_rules_of_the_routes` (`/docs//`, `/docs/INDEX.HTML`). The harness dist names now carry a 16-digit hash.**
- **The rule:** `page` is only `/docs/` or `*/index.html`. Everything else that answers 200 gets `max-age=31536000, immutable`.
- **Names that reach the same `index.html`:** `/docs//`, `/docs/INDEX.HTML` (NTFS is case-insensitive) and `/docs/index.html::$DATA`
  (an ADS). Each is cached for a year. After an upgrade, such a page still names the old `*.js` and `*.wasm`, while the CSP hashes are
  new, so it fails to load.
- **304s:** the middleware sets `Cache-Control` only on 200 (`:149`), so 304 answers get the baseline `no-store`. This only costs
  performance.
- **Fix:** mark `immutable` only for names that match trunk's hash pattern (`-[0-9a-f]{15,16}` before the extension, as every file in
  the current dists does). Answer `no-cache` for everything else, 304 included.

**m4. The CSP hashes are computed once, from `index.html`, and that is not always what is served** (`lib.rs:120-126`).
- **Status (2026-10-22): not in the Phase 4 list; left open (fails closed). Restart after updating the dists.**
- **When they differ:**
  - `ServeDir::precompressed_br` serves `index.html.br` whenever the client accepts br. A stale `.br` (`scripts/shell-dist.sh --check`
    exists but nothing at start enforces it) therefore serves an older loader with new hashes.
  - A dist replaced while the server runs: an MSI upgrade with the app open, or a server deploy without a restart.
- **This fails closed.** The editor does not start, and nothing is bypassed. A hash only ever allows its exact text.
- **Fix:** refuse to mount an editor whose `index.html.br` is older than `index.html`, or hash the bytes that are served. Document
  "restart after updating the dists".
- **Checked and fine:** the hashes cover exactly the text between the tags. The current dists are LF and every asset name is hashed. The
  11 shell specs pass (STATUS:1151-1155).

**m5. `baseline_headers` now keeps any value a route set** (`routes.rs:113-116`). WP7 made this generic so the editor CSP survives.
- **Status (2026-10-22): fixed. `routes.rs` `baseline_headers`: only `content-security-policy`, `cache-control` and `x-frame-options` may be set by a route; `X-Frame-Options: DENY` is in the baseline and the editor routes send `SAMEORIGIN`. The shell-and-guard test on `suite-serve`'s own routes is `cache_frame_and_guard_rules_of_the_routes` (`Host: evil.example` gives 421, a foreign `Origin` on `POST /api/v1/jobs` gives 403). `x-frame-options` is a third overridable name because the editor layer must loosen it.**
- **The risk:** any later `extra` route can replace the CSP, COOP or CORP without anyone noticing.
- **Fix:** let only `content-security-policy` and `cache-control` be overridden, and only by the editor layer. Insert the others
  unconditionally.
- **Also:**
  - Add `X-Frame-Options: DENY` to the baseline and `SAMEORIGIN` on editor routes, for engines without `frame-ancestors`.
  - Add a `suite-serve` test for its own routes: `Host: evil.example` on `/docs/` gives 421, and `Origin: https://evil.example` on
    `POST /api/v1/jobs` gives 403. Today the guard is tested only through `convert-api`.

**m6. The WebView2 settings left at their defaults** (`host.rs:287-318`; the defaults are in `wry-0.57.0/src/lib.rs:1768-1769`).
- **Status (2026-10-22): fixed, except where noted. `host.rs`: accelerator keys off, default context menu off, a download handler (M2), SmartScreen on (`--disable-features=msWebOOUI,msPdfOOUI`). Why: WebView2's own Print and Save as would make a copy without the editor's watermark and would bypass our UI; the editors' own print and export commands are not WebView2 accelerators and stay. Side effect to check in use: browser zoom and reload keys are off too. `blob:` stays in `nav_ok`: whether a blob download link raises NavigationStarting is unverified here, so it was not dropped. Not run against a real WebView2 in this session: the cases in `smoke-desktop.ps1` only start the window and check the server.**
- **Browser accelerator keys and the default context menu are on.** Ctrl+P prints the page, including a framed editor's canvas.
  Ctrl+S and "Save as" save the page, and F5 reloads it. This matters little today, but WebView2's own print and "Save as" would make a copy without the editor's watermark
  (owner decision: XDFU is watermark only, WP11), so the accelerators and the menu must go; the fix prevents unwatermarked copies.
- **Downloads have no handler.** WebView2's default flow applies: the default folder, no extension allowlist, and no SmartScreen,
  because wry passes `--disable-features=...msSmartScreenProtection`.
- **`nav_ok` allows top-level `about:blank` and `blob:` of our origin** (`lib.rs:169-175`). It does not raise capability today:
  - a blob document inherits the creator's CSP;
  - the IPC origin test refuses `blob:` and `about:blank`.

  But it is wider than needed once downloads go through a handler.
- **Fix:**
  - `.with_browser_accelerator_keys(false)` and `.with_default_context_menus(false)` (`WebViewBuilderExtWindows`).
  - The download handler of M2, with an extension allowlist.
  - Drop `blob:` from `nav_ok` if the smoke test shows that downloads still work.
- **Fine:**
  - devtools are off (`:289`);
  - new windows are denied (`:291`);
  - top-level navigation stays on our origin;
  - remote iframes are blocked by the documents' `default-src 'self'` (`frame-src`);
  - the user-data folder inherits the user-only ACL of `%LOCALAPPDATA%`.

**m7. Child processes inherit the current directory, and the host does not restrict the DLL search** (`host.rs:216`).
- **Status (2026-10-22): fixed. Editors start with `current_dir(bin_dir)` (`host.rs` spawn), and `suite.exe` calls `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` first thing (`suite_winsec::set_default_dll_directories`, tested). The call inside each editor's start-up stays WP14/WP15 work.**
- **The scenario:** when Explorer starts `suite.exe` with a file, the current directory can be the document's folder (Downloads). An
  editor that does `LoadLibrary("x.dll")` for a DLL missing from System32 (an optional GPU or compiler DLL) then searches the current
  directory after the system directories.
- **Fix:**
  - `Command::new(exe).current_dir(&self.bin_dir)`;
  - in `suite.exe`'s `main`, call `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)`;
  - the same call in each editor's start-up belongs in the overlay patches. That is outside this review; I note it for WP14 and WP15.

**m8. `WebView2Loader.dll` for windows-gnu builds** (`docs/phase4/desktop-host.md:10`, `scripts/smoke-desktop.ps1:28-33`).
- **Status (2026-10-22): fixed in the repo: `.cargo/config.toml` links every windows-msvc build with `/DEPENDENTLOADFLAG:0xA00`; `scripts/gen-legal.sh` (so NOTICE) has a row for the WebView2 loader with the licence named but not copied (the text is in the NuGet package, not in the crate: UNVERIFIED). Checked: the msvc `suite.exe` (debug, `cargo build --target x86_64-pc-windows-msvc -p suite-desktop`) has `DependentLoadFlags` = 0xA00 in its PE load configuration. The editors' msvc builds pick the flag up from the same config file but were not rebuilt here. Still to do at MSI time: ship the msvc build or put the DLL next to `suite.exe` in Program Files, check its signature and pin its SHA-256.**
- **The import:** the gnu `suite.exe` imports the loader statically, so the Windows loader resolves it before `main`. It looks in the
  application directory first.
- **Where the risk is:** under `Program Files` (per-machine MSI, admin-only writes) there is none. There are two risky cases:
  1. **A portable or zip copy run from a user-writable folder** (upstream ships portable zips). A planted `WebView2Loader.dll`, or any
     non-KnownDLL such as `dwmapi.dll`, `uxtheme.dll` or `version.dll`, next to the exe is loaded. This is the classic "Downloads
     folder" planting.
  2. **A missing DLL.** The search then falls through to the current directory and to `PATH`.
- **The file itself:** the shipped DLL comes from `webview2-com-sys-0.39.1/x64/`. It is version 1.0.3800.47, and I checked that its
  Authenticode signature is valid (Microsoft Corporation).
- **Fix:**
  - Ship the **MSVC** build for the pilot. It links `WebView2LoaderStatic.lib` and needs no DLL.
  - If a gnu build ships, the MSI installs the DLL in the same directory and component as `suite.exe`, never on `PATH`, and never as a
    portable zip.
  - MSVC builds link with `/DEPENDENTLOADFLAG:0xA00` (application directory plus System32 for static imports). Do this for `suite.exe`
    and the editors.
  - The MSI build checks the DLL's signature and pins its SHA-256.

**m9. Supply chain** (`ci/deny/services.toml:8-13`, `scripts/ci.sh:534`).
- **Status (2026-10-22): fixed. `scripts/ci.sh` `job_convert` runs `cargo deny` for `x86_64-unknown-linux-gnu` as well (`deny_one` takes a target), and `services/Cargo.lock` has tokio 1.54.1 (only that package changed).**
- **The Linux run the config describes does not exist.** The comment says "CI also runs `--target x86_64-unknown-linux-gnu` for the
  server", but `job_convert` runs one `deny_one` with the Windows-only `[graph] targets`. WP11's Linux image is no longer deny-checked.
- **I ran it** (cargo-deny 0.20.2, advisory DB of 2026-10-22). Both `--target x86_64-unknown-linux-gnu` and
  `--target x86_64-pc-windows-gnu` give "advisories ok, bans ok, licenses ok, sources ok".
  - The Linux graph does not contain wry, tao or rfd at all: they are `cfg(windows)` dependencies of `suite-desktop`. So the MPL crates
    the comment worries about (`selectors`, `option-ext`) are absent from both graphs.
  - **Fix:** add the Linux line to `job_convert`. It is cheap, and it is clean today.
- **`tokio 1.54.0` is yanked** (a warning in both graphs, `services/Cargo.lock:139`). Fix: `cargo update -p tokio`.
- **The licences of the new dependencies:**
  - wry: Apache-2.0 OR MIT;
  - tao: Apache-2.0;
  - webview2-com and rfd: MIT;
  - tower-http and mime_guess: MIT;
  - dunce: CC0-1.0, MIT-0 or Apache-2.0.

  All are permissive. One caveat: `webview2-com-sys` declares `MIT` but ships Microsoft's prebuilt `WebView2Loader.dll` and
  `WebView2LoaderStatic.lib`. The static library is linked into the MSVC `suite.exe`. They are under the WebView2 SDK licence, which I
  believe is BSD-3-style (UNVERIFIED), and cargo-deny cannot see that.
- **Fix:** add a NOTICE row for the WebView2 SDK with the licence text from the matching NuGet version.

**m10. Test hooks are live in the release binary** (`host.rs:201`, `:210`, `:265`, `:347`, `:352`).
- **Status (2026-10-22): fixed. `host.rs` `hook()`: the `SUITE_*` variables are read in debug builds only (`cfg(debug_assertions)`); `tests/host.rs` is `cfg(debug_assertions)`. `scripts/smoke-desktop.ps1` therefore needs a debug `suite.exe` (its default).**
- **The hooks:** `SUITE_BROWSER` starts any executable on the URL, and `SUITE_INSTANCE` changes the pipe name. There are also
  `SUITE_DATA_DIR`, `SUITE_FORCE_NO_WEBVIEW2` and `SUITE_TEST_EXIT_AFTER_MS`.
- **Why it is minor:** only the same user can set them. Still, an environment variable that picks an executable is a needless
  primitive in a signed binary.
- **Fix:** compile them only under `cfg(debug_assertions)` or a `test-hooks` feature that the integration tests enable.

**m11. WP19: no regression, two notes for Phase 5** (`config.rs` `provider_issuer`).
- **Status (2026-10-22): Phase 5 (recorded, not changed): one HS256 secret per provider before a second provider is allowed; show "connected as `<sub>`" in the Connect panel.**
- **The seam is sound for one provider.** `kind = "oidc"` is refused, and only one entry is allowed. The issuer becomes the enforced
  `iss` (`jwt.rs:96-101`). The guard, the JWT rules, the limits and the TLS paths are unchanged.
- **Note 1, for Phase 5:** every provider shares the one HS256 `secret_file`. Before more than one provider is allowed, each one needs
  its own key. Otherwise `iss` is a label, not authentication: any issuer can mint tokens for another one's `iss`.
- **Note 2, a Phase 3 design point that WP19 now exercises:**
  - **The gap:** the `#token=` fragment is accepted from any navigation, so a link from anywhere can set the session's token (a
    login-CSRF).
  - **The impact is small:** there is no job list, so the attacker cannot retrieve the victim's uploads.
  - **Cheap fix:** show "connected as `<sub>`" in the Connect panel.

**m12. Small hardening on the command line and the brand config** (`lib.rs:61-67`, `:290`, `:365-390`).
- **Status (2026-10-22): fixed. `Brand::parse` accepts a `bin` only of `[A-Za-z0-9._-]+` starting with a letter or digit (`the_brand_bin_name_must_be_a_plain_file_name`). Nothing in the repo registers `suite.exe` as an "Open with" handler yet: when it does, use `"<exe>" -- "%1"`.**
- **`brand.toml` `bin`:** it is joined into a path without a check. The file is install-controlled, but reject anything that is not
  `[A-Za-z0-9._-]+`.
- **The "Open with" command:** register `"<exe>" -- "%1"` if `suite.exe` is ever registered. Explorer's `%1` is always absolute, so this
  is belt and braces.

**m13. The IPC test the plan asked for is missing.**
- **Status (2026-10-22): partly. `lib.rs` `ipc_message` is the one function the WebView handler calls; `a_page_of_another_origin_cannot_trigger_an_ipc_action` checks that a document of another origin (other port, `blob:`, `about:blank`, `file:`, lookalike hosts) gets no answer and starts nothing. The WebView-level case (a real second server, a real WebView2) was not run.**
- **What the plan asked for:** WP9's step "a test that a cross-origin page cannot trigger an IPC action".
- **What exists:** the string unit test `only_our_own_origin_may_send` (`lib.rs:476-492`).
- **Fix:** add a WebView smoke case. Navigate (by script) to a second loopback server on another port, post `{"type":"history"}`, and
  assert that no `suite-host` event arrives and nothing is started.

---

## The ten questions, answered

| # | Question | Answer |
|---|---|---|
| 1 | postMessage bridge, framing, clickjacking | Sound. **Shell side:** it checks `ev.origin === location.origin`, `ev.source` against its own iframe's window, and the app's prefix (`shell.js:85-88`). It posts with targetOrigin `location.origin` (`:63`) and renders only through `textContent`. **Editor side:** they check origin and source (gridcraft `host.rs:116`, pdfcraft `main.rs:183`, wordcraft `0001` +291). wordcraft's `&parent_origin=` can only aim saves at an origin that cannot frame it, so it is harmless. **The IPC:** no editor message reaches it. A framed editor *can* reach `top.ipc` by being same-origin (m2). **Framing:** the shell, the converter and the API answer with `frame-ancestors 'none'`, the editors with `'self'`. COOP `same-origin` on every answer cuts off cross-site popups, and CORP `same-origin` stops cross-site reads. There is no XFO (m5). |
| 2 | Editor CSP | The hashes are correct and minimal: each inline `<script>` and `<style>` of that dist, LF. `img-src data: blob:` adds no script or network capability. `'wasm-unsafe-eval'` sits only on the editor documents, but same-origin makes the split cosmetic (m2). A changed dist fails closed and is not a bypass (m4). The cache rule is too broad (m3). |
| 3 | Loopback server in desktop mode | The guard wraps every `suite-serve` route (`routes.rs:347`, `:380`). The desktop server's allowed hosts carry the real port (`host.rs:140-146`, `app.rs:474-481`; the host test gets 200 on it). A hostile site's POST is refused on `Origin`, and a rebinding attempt on `Host`. **Another local user is not stopped (M1).** |
| 4 | IPC | Sound: a tagged enum with `deny_unknown_fields`, 8 KiB, the serde error not echoed. The origin check is exact, port included. `settings_set` takes only `lang` with its 4 values. The replies are JSON, so a valid JS literal. The launch is the brand exe, joined to `bin_dir`, through `Command::arg`. Windows paths cannot contain `"`, and an absolute path cannot start with `-`, so there is no injection. The check and the launch can race, but only for the same user. Raw page paths: m1. |
| 5 | Named pipe | **B1.** Writes are default-secure. Squatting and impersonation are not. The fix: SQOS identification plus a server-SID check on the client, a user-only DACL on the server, a SID-based name. |
| 6 | "Open with" paths | UNC and `\\?\UNC\` are allowed from the user only. Device (`\\.\`) and other verbatim paths are refused. `..` is refused, verbatim included. The extension comes from the real final component, so RTLO, double extensions and a trailing dot or space are refused. Long paths are fine (std adds `\\?\`). ADS, DOS device names and symlinks: m1. |
| 7 | WebView2 | Devtools are off, new windows denied, top-level navigation locked to our origin, remote frames blocked by CSP, and the user-data folder has a user-only ACL. Remote content cannot load. Accelerators, context menus, downloads and SmartScreen: m6. Persistent storage: M2. |
| 8 | TLS and seam | No regression (m11). |
| 9 | Supply chain | Licences ok on Windows and on Linux (I ran both). The CI Linux run is missing, tokio is yanked, and the loader licence needs a NOTICE row (m9). |
| 10 | `WebView2Loader.dll` | Ship MSVC, use Program Files only, no portable zip, `/DEPENDENTLOADFLAG`, a signature and hash check (m8). |

## Checked and fine (not findings)
- `tower-http` `ServeDir` on Windows rejects drive prefixes, root and `..` components, and reserved DOS names
  (`tower-http-0.6.11/src/services/fs/serve_dir/mod.rs:455-490`).
- The bare editor route redirects to `/<route>/` with the query only. The path is fixed, so this is not an open redirect
  (`suite-serve/src/lib.rs:136-143`).
- `reply_script` embeds `serde_json` output, so a file name cannot break out of the JS literal (`lib.rs:178-180`).
- The IPC handler drops non-string messages, because `TryGetWebMessageAsString` fails first (`wry-0.57.0/src/webview2/mod.rs:968-972`).
- The accept loop never gives up the port while the process lives. A dead listener would let another process take the WebView's
  origin, and that cannot happen here: accept errors `continue` (`tls.rs:154-158`).

## What I did not verify
- I built nothing and ran no product binary. B1 and M1 come from reading the code and the Windows documentation. I did not reproduce
  either of them with a second account.
- Whether WebView2 155 delivers `chrome.webview.postMessage` from an iframe to the `CoreWebView2` handler. It does not matter here: a
  same-origin frame reaches `top.ipc` anyway (m2).
- Whether an `<a download>` of a `blob:` raises `NavigationStarting` in WebView2. This decides whether `nav_ok` needs `blob:` (m6).
- WebView2's Crashpad dumps under the user-data folder (renderer memory holds the open document) and their upload policy. It is the
  same class as Phase 3 M4.
- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` can add `--remote-debugging-port`. That is same-user only, and I did not check whether
  `AreDevToolsEnabled = false` blocks CDP.
- Chromium's quota and eviction numbers in WebView2 155 (M2).
- Whether the native editors resolve external references to UNC paths, which would leak NTLM credentials when a malicious document is
  opened. Examples: a docx `attachedTemplate`, linked images, `INCLUDEPICTURE`, an HTML `<img src="\\host\...">`. The engines carry no
  HTTP client, but file-system access to `\\host` paths is possible. This is outside this code, and worth a WP15 test.
- The exact licence text of the WebView2 SDK binaries (m9).
