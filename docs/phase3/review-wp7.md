# WP7 architect security review: the HTTP API (2026-10-21)

Scope: `services/convert-api/` at commit 7bfe056, and the parts of `services/convert/` it relies on (`job.rs`, `exec.rs`,
`http.rs`, `scan.rs`, `names.rs`). Read-only: nothing was built or run. Line numbers refer to 7bfe056. A builder is
adding the WP8 UI to `convert-api` at the same time, so the UI files were not reviewed. I checked the code against
PLAN §5 (in particular §5.5), §8 WP7, §10, `docs/phase3/api.md` and `docs/phase3/review-wp2.md`.

Threat model: a login-less server on loopback (local mode), and a JWT plus TLS server on a LAN (server mode). Clients are
hostile in both modes. A hostile browser on the same machine is in scope (DNS rebinding, CSRF).

Severity:
- **Blocker:** fix before the Phase 3 gate. For a blocker marked "O1", fix it before the HRM test-server deploy.
- **Major:** a security or availability gap that a hostile client can reach. Fix it in Phase 3.
- **Minor:** a real defect with a cheap fix or a small blast radius.

Verdict: the main defences are sound:
- `alg` is pinned, the MAC compare is constant-time, and `exp` is required.
- The loopback rule fails closed.
- The Host and Origin guard stops DNS rebinding and cross-site posts.
- Ids are 128-bit OS random, and the owner is checked on every route.
- Client names never become server paths, and `Content-Disposition` cannot be injected.
- The AES-GCM framing catches truncation and reordering.
- The baseline headers sit on the outermost layer.

The weak spots are availability and the at-rest story:
- a dropped request leaks an active job;
- the store has no disk bound;
- the zip scan amplifies CPU outside the gate;
- the LAN listeners have no per-IP connection limits;
- the plaintext-window table in api.md is incomplete.

The WP2 review items that touch the API are closed:
- B1: cancel, deadline and the Job Object (`exec.rs:97-106`, `exec.rs:155-176`);
- B2: `JobHook`, `InputSource` and `OutputSink`;
- M1: temp name plus rename in the CLI sink;
- M2: `safe_name` in the crate;
- M6: commit sampling;
- M8: the caps;
- M9: a fail-closed scan with a measured inflate;
- m1: `sweep_stale`;
- m3: staging copy first.

Two WP2 items are only partly closed. Both are below (M1 and M4):
- B1: the job deadline is per file, not per API job.
- B2: the wrapped key and the tmpfs claim.

---

## Blockers

### B1. A request dropped after its body was read leaks an active job forever
`routes.rs:387-401` (the cleanup runs only in the `Err` arm), `routes.rs:415` (an unbounded wait for a CPU slot),
`routes.rs:419-424` (the precheck), `app.rs:237-246` (`admit` counts `active`), `app.rs:322-327` (the sweeper only
looks at finished jobs).
- **Mechanism:** a client may close the connection once the body is fully read, while `create_inner` waits for a CPU
  slot or for the precheck. hyper then sees EOF mid-message and ends the connection (`hyper-1.12.0`
  `proto/h1/conn.rs:493-510`, `mid_message_detect_eof`; polled from `dispatch.rs:225`). That drops the handler future
  at its `.await`. `app.delete` and `app.finish` never run.
- **Effect:** the job stays `active = true` in `jobs` with no finish time. The sweeper never removes it. It counts
  against `per_user_active` and `queue_max` until the next restart, and its directory stays on disk.
- **Scenario, local mode:** every client is the principal `local`. The user uploads while a long conversion holds the
  slots, then reloads the tab, three times. After that every `POST` answers 429 `too_many_active_jobs` until a restart.
  This needs no attacker: it is ordinary use.
- **Scenario, server mode:** a tab closed during the precheck takes one of the user's 3 slots. About 100 such events
  across users fill `queue_max`, and the whole server answers 503.
- **Fix:**
  - Put an RAII guard in `create_job` that runs `delete` and `finish` on `Drop`. Disarm it right after the supervisor
    task is spawned (`routes.rs:455`).
  - Bound the slot wait. Either return 503 with `Retry-After` after a few seconds, or answer 202 before the precheck
    and report the precheck results through `GET /jobs/{id}`.
  - Test: drop the `create_job` future while the CPU semaphore is exhausted, then assert that `queue_depth()` is back
    to 0.

### B2 (O1). The job store has no disk bound, and the inputs are kept for the whole retention time
`app.rs:234-246` (admission counts active jobs only), `app.rs:64-66` with `app.rs:291-297` (`in/<n>` is removed only
by `purge`), `routes.rs:455-462` (a job whose files all fail the precheck finishes at once), `config.rs:160-161`
(`store_dir` defaults to `work_dir`, which defaults to `%TEMP%` on the system drive).
- **Scenario:** an authenticated client posts 30 jobs a minute (`rate_user_per_min`), each 300 MB of random bytes.
  Every file fails detection, so each job finishes at once and stops counting as active. The uploads stay on disk for
  `retention_s` = 1 h.
  - Per `sub`: about 9 GB a minute.
  - Per IP (4 subs): 36 GB a minute.
  - The system drive fills in minutes. Every conversion then fails, and Windows itself degrades.
- **Plan status:** api.md lists "a global disk quota" as deferred. It must not be deferred past O1, and part of the
  fix is free.
- **Fix:**
  1. Delete `in/<n>` as soon as file `n` ends: at the end of `run_file`, and in the precheck failure branch. Inputs are
     never served back. This alone changes the bound from upload volume to output volume.
  2. Add a store quota at `admit`. Keep a counter of stored bytes, and refuse with 507 or 503 when
     `stored + max_batch_mb > store_quota_mb` (a new config key) or when the free space on `store_dir` is below a
     floor.
  3. Add a per-`sub` cap on retained bytes or retained jobs (429).

---

## Major

### M1. The zip scan amplifies CPU about 1000:1, runs twice, and the zip build bypasses the CPU gate
`routes.rs:379` and `job.rs:598` (the scan at `POST` and again at conversion), `scan.rs:75` (the ratio rule only for
entries over 10 MB), `scan.rs:121-145` (a full measured inflate up to 1 GiB per file), `routes.rs:415` (one CPU slot for
the whole batch precheck), `routes.rs:577-625` (`build_zip` on the blocking pool with no slot and no rate limit).
- **Amplification:**
  - Take about 107 entries of 9.9 MB of zeros. They deflate to roughly 10 KB each, so the file is about 1 MB.
  - Each entry is under `RATIO_FROM_BYTES`, and the total is under 1 GiB, so the file passes the scan and is
    inflated in full.
  - The scan inflates about 1 GiB per 1 MB uploaded: a few CPU-seconds.
  - 30 such files in one 30 MB request cost about 30 GiB of inflate (minutes of one core). That happens at `POST`,
    and again for every file at conversion.
- **Scenario:** one user keeps 3 requests in precheck (the `per_user_active` limit). That holds 3 of the default 4 CPU
  slots (`clamp(12/3, 1, 4)`). Two users hold all four, and every conversion waits.
- **Second path:** `GET /zip` on each finished job deflates up to 1 GiB per call on the blocking pool (512 threads),
  with no slot and no rate limit.
- **Who can do it:**
  - Unauthenticated: no. Auth runs before the body is read.
  - A browser page against local mode: no. The `Origin` check stops it.
  - Any token holder in server mode: yes.
- **Fix:**
  - Count the measured inflate across the whole request against a budget, for example 2 GiB or 50x the upload size,
    and reject the rest of the batch per file with exit 9.
  - Keep the precheck `Scan` in `FileState` and pass it to the run, so the second scan goes. The staged copy comes
    from an authenticated AES-GCM blob in server mode and from the user's own plain file in local mode, so the TOCTOU
    reason for the second scan does not apply.
  - Allow at most one precheck at a time per `sub`.
  - Take a CPU slot in `build_zip`.

### M2. One user can hold every slot for hours: the job deadline is per file, and the queue is FIFO across users
`job.rs:457` (a new deadline in every `run_job`, that is, per file), `app.rs:424-428` (one task per file, a FIFO tokio
semaphore), `routes.rs:455-460`.
- **Scenario:** a user's 3 jobs of 30 files each are 90 file tasks queued ahead of everyone who comes later. Each file
  may take up to `job_timeout_s` = 30 min, so one API job may legally run 30 x 30 min.
- **Plan:** PLAN §5.3 says "job timeout 30 min", meaning the API job.
- **Queue wait:** the queue wait itself has no bound either.
- **Fix:**
  - Create one `Instant` deadline per API job in `create_inner`, pass it through `JobMeta` into `Ctl`, and start it at
    admission.
  - Cap the files of one `sub` that run at once, for example `max(1, cpu_slots / 2)`. A per-user semaphore taken
    before `app.cpu` is enough.
  - Fail files that waited longer than the job deadline.

### M3 (O1). The LAN listeners can be exhausted by unauthenticated connections
`tls.rs:51`, `tls.rs:64`, `tls.rs:73`, `main.rs:59-62`.
- **TLS listener:**
  - The cap is 1024 connections in total, with no per-IP limit.
  - The 10 s header timeout runs for each request, including the wait on keep-alive. So a client that sends
    `GET /healthz` (no token needed) every 9 s keeps its slot forever.
  - 1024 such connections cost an attacker about 110 requests a second. After that every new connection is dropped
    (`try_acquire_owned` fails, then `continue`).
- **Redirect listener:**
  - It is plain `axum::serve`. axum 0.8.9 builds the hyper connection without a timer (`serve/mod.rs:391`), so no
    header timeout applies, and there is no connection cap.
  - It is on the LAN, unauthenticated, and shares the process with the API. A classic slowloris grows tasks and
    sockets without bound.
- **Plan status:** api.md lists this as deferred. It must not be deferred past O1.
- **Fix:**
  - Add a per-IP connection cap in the accept loop, for example 16, using a `HashMap<IpAddr, count>` with a guard that
    decrements it.
  - Add a maximum connection age, or a maximum number of requests per connection.
  - Serve the redirect router through the same loop, with a plain TCP acceptor, so it gets the same timeouts and caps.
  - The local listener (`main.rs:81`) can wait. See m13.

### M4 (O1). The plaintext on disk goes beyond what api.md lists, and tmpfs is not optional in server mode
api.md lines 101-118, `config.rs:160-161`, `http.rs:55-75`, `exec.rs:341-347`.

**Plaintext stays after normal operation.** On a disk-backed `work_dir`, every job leaves its plaintext (the precheck
copy, the staging copy, `in`, `mid`, `out` and `tmp`) in free clusters after deletion. On an SSD, wear levelling keeps
it out of reach of any wipe. api.md says only that "a crash leaves plaintext until the next start". The default
`work_dir` is `%TEMP%` on the system drive.

**The sizing advice conflicts with the store.** api.md sizes the tmpfs at `cpu_slots x 2 x max_batch_mb` = 2.4 GB. But
`store_dir` defaults to `work_dir`, and the store holds up to 100 active uploads of 300 MB each, plus 1 h of results. A
tmpfs of that size fills, and conversions then fail.

**Copies the table misses:**
- **The pdf2docx sidecar.**
  - The whole PDF goes over plain HTTP (`http.rs:55-64`).
  - The sidecar keeps its own temp files and memory.
  - The DOCX answer, up to 1 GiB, sits in API memory (`http.rs:75`).
  - The WP6 LibreOffice sidecar has the same exposure.
  - This also touches §10, clause 11: network exchange.
- **Crash dumps.**
  - Windows Error Reporting writes a report and minidump for a tool that dies with an exception (exit `0xC...`), and
    may upload it, depending on the machine's consent policy.
  - On Linux (WP11), core dumps can land on the host through `core_pattern`.
- **The page file and the hibernation file.**
  - Tool memory (multi-GB for gridcraft) and the job keys can be paged out.
  - api.md says only "swap is not handled".
- **Antivirus quarantine and sample submission.** Defender copies a flagged file, for example a docx with a macro,
  into its quarantine store.
- **VSS snapshots and backup agents.** They capture `work_dir` while files exist.
- **The SYSTEM account.** A service running as LocalSystem gets `C:\Windows\SystemTemp` from `GetTempPath2`, whatever
  `TEMP` says. So any engine temp file would bypass `<job>/tmp` and the sweeper. UNVERIFIED: whether any engine writes
  temp files at all.

**Fix:**
- In server mode, refuse to start unless `work_dir` is RAM-backed. On Linux, check `statfs` for `TMPFS_MAGIC`. On
  Windows, require an explicit `work_dir_ram = true` and print a startup warning.
- In server mode, default `store_dir` to a path separate from `work_dir`.
- Make full-disk encryption of the OS volume a server-mode requirement, not a "second line". It also covers the page
  file and the crash dumps.
- In WP11, set `--ulimit core=0`.
- Run the service as a dedicated non-SYSTEM account.
- Put the sidecars on the same no-egress internal network, with a tmpfs `/tmp`.
- Rewrite the api.md table with every row above.
- The antivirus policy for `work_dir` is a trade-off for HRM security to decide, so list it as an owner item.

### M5. The wrapped job key is written next to the data, which voids "unreadable after a crash"
`app.rs:249-262` (`<job>/key`), `crypt.rs:44-50`, `app.rs:192-195` (`sweep_stale` deletes all of it at the next start,
unread).
- **What the plan says (§5.5):** "The key is held only in memory... the sweeper deletes the key first, so leftover
  files are unreadable after a crash."
- **What happens:** after a crash, the leftover directory holds the wrapped key next to the ciphertext. Anyone who also
  has `key_file`, which sits on the same host and is readable by the service account, reads everything. That covers a
  stolen disk, a backup or VSS snapshot of both, and another admin.
- **The copy buys nothing:** a restart drops the in-memory queue, and the next start deletes these directories without
  reading them. There is no recovery tool.
- **Fix:** do not write `key`. The job key then lives only in memory, as the plan says, and a crash leaves ciphertext
  with no key anywhere.
  - `key_file` stops being needed. Drop it from the required server keys, or keep it for a future persistent queue.
  - Update the api.md sentence "the operator can unwrap `key`".
  - This is the smallest change: it is a deletion.

### M6 (O1). Per-IP limits collapse behind a proxy, and IPv6 rotation defeats them
`routes.rs:109` (the IP from `ConnectInfo`), `routes.rs:388-390`, `app.rs:88-100`.
- **Behind Traefik:** the HRM side runs Traefik. Server mode is HTTPS-only (there is no upstream-TLS mode), so Traefik
  can only pass TCP through. Every peer is then Traefik's address.
  - The 120 job creations a minute become one shared server-wide bucket. Four users exhaust it for everyone.
  - The 30 failed logins a minute are also shared. That is harmless for valid tokens: only failures count.
- **IPv6 rotation:** each IPv6 address is its own bucket, and a LAN host can use many addresses from its /64. Once the
  map holds more than 4096 keys, every hit runs an O(n) `retain` under the mutex (`app.rs:91-93`).
- **Fix:** an owner decision before O1, one of:
  - expose the port directly, with no proxy;
  - Traefik TCP passthrough with PROXY protocol v2, parsed in the accept loop, accepted only from a configured proxy
    address;
  - the deferred upstream-TLS mode, with a trusted-proxy list for `X-Forwarded-For`.

  Also key IPv6 addresses by /64, and give the map a hard cap.

### M7. The `options` part is buffered whole before its 64 KiB check, outside the memory gate
`routes.rs:284-285`.
- **Mechanism:** `field.text()` reads the whole part into memory before the check. multer has no per-field limit, so
  the part can be up to the request limit of 301 MB.
- **Scale:** every admitted request may do this. Admission allows 3 per `sub` and 100 in total, so up to about 30 GB in
  the API process, which the 12 GB memory budget does not see.
- **Effect:** an OOM of the API kills every running job.
- **Same class, outside the budget:** the sidecar answer (`http.rs:75`, up to 1 GiB per step) and the whole PDF read
  for the sidecar (`http.rs:55`).
- **Fix:**
  - Read `options` with `field.chunk()` and a 64 KiB counter.
  - Cap the sidecar answer by the target's expected size, for example `max_file_mb` x 4, or stream it to `out/`.

### M8. Conflicts in WP8 (the UI being built now): CSP and CORS against PLAN §5.5's UI bullets
`routes.rs:71` (`CSP`), the no-CORS rule, PLAN §5.5 "UI".
- **"Connect to server" cannot work from the local page.** `default-src 'self'` makes `connect-src` `'self'`, and
  CORS is closed. So a page served from `127.0.0.1` cannot call a remote server. The risk is that someone loosens CSP
  or opens CORS to make the setting work.
  - **Fix:** make "Connect to server" a navigation to that server's own UI. There are then no cross-origin API calls,
    and the baseline stays as it is.
- **Server-mode downloads cannot carry the bearer token.** `<a href>` sends no `Authorization` header. A `?token=`
  would break "tokens never in URLs".
  - **Fix:** use `fetch` with the header, then a `Blob`, then `a[download]`. If image previews are wanted, `blob:` is
    blocked by `img-src` (it falls back to `default-src 'self'`); decide on that explicitly.
- **Clear the token fragment.** After `#token=` is read, call `history.replaceState` so it does not stay in history.
- **Render server text as text only.** File names, warnings and tool messages carry document content: sheet names,
  stderr tails. Render them with `textContent` only. CSP does not stop `<meta http-equiv=refresh>` or an injected
  `<a>`.
- **No inline styles.** `style-src` falls back to `'self'`, so inline `style=` attributes and `<style>` blocks are
  blocked. Keep all styling in `app.css`.

---

## Minor

- **m1. JWT claim rules are thin** (`jwt.rs:54-58`, `jwt.rs:62-74`, `config.rs:145-149`).
  - **Missing checks:**
    - `nbf` is not checked.
    - There is no maximum lifetime: a token with `exp` in 2099 is accepted. With no revocation, a leaked token lives
      until the secret rotates.
    - There is no clock-skew leeway, so issuer drift gives spurious 401s.
    - There is no `aud`. If the operator reuses another HRM service's secret, everyone who holds it can mint convert
      tokens.
  - **Not checked at start:** the secret's entropy and its file permissions.
  - **Fix:**
    - Add `max_token_ttl_s` (12 h by default) and reject `exp - now > ttl`.
    - Reject `nbf > now + 60`, and allow 60 s of leeway on `exp`.
    - Add an optional `aud`.
    - Document "use a dedicated secret".
  - **Fine as is:** `alg` confusion (only the exact `HS256`; no `kid`, `jku` or `x5u`), the compare (`verify_slice`,
    constant time) and leakage (tokens are never logged, never in URLs, and `no-referrer` is set).
- **m2. The mode switch fails closed; add tests and documentation** (`config.rs:135`, `config.rs:173-185`).
  - **Already fails closed:**
    - `listen` must be an IP literal, so `localhost` is never resolved.
    - `0.0.0.0`, `[::]` and LAN addresses are refused.
    - `[::ffff:127.0.0.1]` is refused too, because `Ipv6Addr::is_loopback` is `::1` only.
  - **Add tests** for `[::ffff:127.0.0.1]` (refused) and `127.0.0.2` (accepted), so this behaviour cannot drift.
  - **Refuse `public_url` in local mode:** today it replaces the loopback Host allowlist (`app.rs:206-215`).
  - **Document:**
    - A port forward (`netsh portproxy`, `ssh -R`, a container port published to a loopback-bound server) exposes
      `auth = "none"`. A non-browser client chooses its own `Host`.
    - On a shared Windows host (RDS), every session shares loopback and the principal `local`.
- **m3. Server mode without `public_url` has no Host check** (`app.rs:212`). DNS rebinding then reads `/healthz` from
  an attacker's origin.
  - **Fix:** require `public_url` in server mode.
  - **Validate it at start:** `https://host[:port]`, with no path or userinfo (`tls.rs:35`).
  - **The redirect is not an open redirect:** the target is `public_url` plus the path, never the request's `Host`. It
    still depends on a well-formed `public_url`.
- **m4. `/healthz` is open in server mode** (`routes.rs:175-200`). It tells any LAN client which tools exist, whether
  the sidecar is reachable, and the queue depth.
  - **Fix:** without a valid token, answer only `{ok}`.
- **m5. The sidecar's internal address reaches clients** (`http.rs:69`: "the pdf2docx sidecar at {base} did not
  answer: {e}"). The `Redactor` does not scrub URLs.
  - **Fix:** drop `{base}` from the client message.
  - **Before WP11:** also scrub `/`-rooted paths outside the configured roots. This deferred item must be done before
    the Linux image.
- **m6. The AAD binds only the LAST flag** (`crypt.rs:83-86`, `crypt.rs:97-101`, `crypt.rs:132`, `crypt.rs:187-206`).
  - **Whole-blob swaps:** the file id comes from the unauthenticated header. So with write access to `store_dir`, whole
    blobs can be swapped within one job (`in/0` and `in/1`, `out/0/0` and `zip`), and they still decrypt.
  - **Trailing bytes:** bytes after the LAST frame are ignored.
  - **Already correct:**
    - Nonces never repeat: the per-job key, the atomic file counter and the u64 frame index.
    - Truncation at a frame border is caught.
    - Reordered frames are caught.
    - The wrap is bound to its job id by the AAD.
  - **Fix:**
    - Make the AAD `job id || role ("in/3", "out/1/0", "zip") || last`.
    - Have the reader check the expected file id.
    - Reject data after LAST.
- **m7. Zeroize is partly cosmetic.**
  - **The AES key schedule stays in memory.** `aes` 0.8.4 is built without its `zeroize` feature (its `Cargo.lock`
    deps are `cfg-if`, `cipher` and `cpufeatures`). So the key schedule inside `Aes256Gcm` stays in freed memory, and
    so does GHASH H. For AES-256, the first two round keys are the key itself.
  - **More leftovers:**
    - `BlobReader` replaces `plain` without zeroizing the old frame (`crypt.rs:200`).
    - `BlobWriter` has no `Drop` on the error path.
    - `stream` keeps a 64 KiB buffer (`routes.rs:545`).
    - The operator key and the JWT secret are copied through `Resolved: Clone` (`config.rs:86-97`).
  - **Fix:**
    - Add `aes = { version = "0.8", features = ["zeroize"] }`, and enable the same feature for `ghash`.
    - Decrypt in place into one reused buffer.
  - **Limit:** this has little value without `mlock`, so pair it with full-disk encryption (M4).
- **m8. A failed zip build breaks the zip for good** (`routes.rs:592-593`). `zip.part` is created with `create_new` and
  is not removed on error, so every later `GET /zip` answers 500.
  - **Fix:** remove `zip.part` on error.
- **m9. `sweep_stale` can delete operator folders** (`job.rs:64-80`). Any `job-<digits>-*` or `stage-<digits>-*`
  directory of a dead pid is removed with `remove_dir_all`.
  - **Scenario:** a `work_dir` or `store_dir` set to `D:\Data` deletes a folder such as `stage-2024-photos`.
  - **Pid reuse:** it also keeps plaintext leftovers indefinitely when the pid was reused.
  - **Fix:**
    - Require a marker file in the directory, and refuse to start in a non-empty directory without it.
    - Match the exact patterns: `stage-<pid>-api-<32 hex>`, `stage-<pid>-pre-*`, `stage-<pid>-<hex>-<n>`, `job-...`.
    - Compare the process start time with the directory's mtime.
- **m10. `safe_name` misses some characters** (`names.rs:4-14`).
  - **Missing bidi and invisible characters:** U+061C (ALM); U+200B-U+200D and U+2060 (zero-width).
  - **Missing Windows device names:** `COM¹`-`COM³`, `LPT¹`-`LPT³`, `CONIN$`, `CONOUT$`.
  - **Impact:** client-side spoofing only, because names never become server paths.
  - **Content-Disposition is safe** (`routes.rs:509-520`). The ASCII fallback maps every non-graphic character,
    `"` and `\` to `_`. `filename*` percent-encodes everything outside RFC 5987 attr-char. CR and LF cannot reach the
    header.
- **m11. Gaps in the header test** (`tests/api.rs:554-605`).
  - **Status codes never exercised:** 413 (from tower-http's limit layer, a different response source), 408, 409, 429
    and 503.
  - **The UI list is hard-coded** (`tests/api.rs:579`), so new WP8 files are not covered.
  - **Fix:**
    - Keep one `UI_FILES` table that both the router and the test use.
    - Add the missing status codes.
    - Add `Cross-Origin-Resource-Policy: same-origin` and `Cross-Origin-Opener-Policy: same-origin` (cheap). They stop
      cross-site embedding and probing of API responses.
  - **Not testable at this layer:** hyper's own 400 and 431 responses never reach the middleware. That is acceptable:
    they have no body.
- **m12. Cancel and admission gaps.**
  - **Cannot be cancelled:**
    - Staging, detection and the scan run in `convert_source` before any `Ctl` exists (`job.rs:585-601`).
    - The sidecar call cannot be interrupted (`http.rs:61`).
  - **Admission stops counting too early:** `DELETE` removes the job from the map at once (`app.rs:312`), so work that
    is still running no longer counts. Cycling `POST` then `DELETE` gets past `per_user_active`.
  - **Fix:**
    - Keep counting until `finish`.
    - Check the cancel flag before the scan.
- **m13. The local listener has no header timeout and no connection cap** (`main.rs:81`). It is loopback-only, and
  browsers cap their connections, so this can wait until after Phase 3.
- **m14. Logs.** No file names, no content and no tokens are logged.
  - **`sub` is logged unquoted** (`app.rs:452`). Spaces in `sub` can forge `key=value` fields.
    - **Fix:** quote it.
  - **Panic messages still reach stderr.** `guarded` catches the panic, but the default panic hook prints first.
    - **Fix:** set a hook that prints only the location.
- **m15. `read_key` accepts any 32-byte file as a raw key** (`config.rs:107`). A 32-character passphrase becomes a
  low-entropy key without a warning. This becomes moot if M5 removes `key_file`.
  - **Fix:** accept only the 64-hex form, or warn when all 32 bytes are printable.

---

## The deferred list in api.md: what must not wait

| Deferred item (api.md "Not done") | Verdict |
|---|---|
| WP15 XDFU | Keep it in Phase 3, as planned. Not needed for an O1 test deploy on the anonymised corpus. Required before any real XDFU document reaches server mode. |
| SSO (OIDC) | Phase 5 is fine. |
| A mode for TLS terminated upstream (Traefik) | **Owner decision before O1** (M6). With HTTPS-only, Traefik can only pass TCP through, and that collapses the per-IP limits. |
| A persistent queue | Fine to defer. |
| A global disk quota | **Not deferrable** (B2). Deleting inputs after the run belongs in Phase 3. The quota is needed before O1. |
| Rate limits behind a proxy (`X-Forwarded-For`) | **Before O1** (M6). |
| Operator key rotation, HSM, `jti` revocation | Rotation and HSM are moot once M5 lands. Revocation can wait if `max_token_ttl_s` (m1) lands before O1. |
| Header timeout and connection cap on the local and redirect listeners | **The redirect listener: before O1** (M3). The local listener can wait (m13). |
| Unix absolute paths in tool messages | **Before the WP11 image**, so before O1 if O1 runs the Linux container (m5). |
| `Range` and `HEAD` on downloads | Fine to defer. |
| Not listed, but needed | B1 before the Phase 3 gate. M4 (tmpfs enforced, full-disk encryption, the full plaintext table) before O1. M7 and M8 now, while WP8 is open. |

---

## What I did not verify
- Nothing was built or run. The claim that hyper drops the handler future (B1) comes from reading hyper 1.12.0
  (`proto/h1/conn.rs:437-510`, `dispatch.rs:225-247`), not from a reproduction. The guard is correct either way.
- The inflate throughput behind M1 is an estimate (miniz_oxide runs at a few hundred MB/s per core). It was not
  measured.
- Whether any engine writes temp files, and so whether the SYSTEM-account `GetTempPath2` behaviour matters (M4).
- What Windows Error Reporting and Defender do on the HRM server depends on its policy. I did not check it.
- How O1 will put Traefik in front (M6).
- The WP8 UI files, which were being rewritten during this review.
