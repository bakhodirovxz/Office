# Phase 5 gate, security part: rehearsal (WP18 dry run)

Date 2026-10-30. Branch `p5-secreview` from `phase-5` at 0e3b630. Architect review (opus), local machine only: no HRM test
server, no real provider, no Docker run. This is a dry run of PLAN §10 and of the WP18 attacker review; the real gate runs
them again on the gate commit.

**Result in one line:** 19 of 25 rows green, 3 partial, 1 missing (S18, retention sweep: not built), 1 not run (S22, tools
missing), 1 owner/HRM (S25). Two rows changed in this rehearsal: S1 was partial (now green, test added) and S17 was failing on
two error paths (now green, fixed). The biggest open item is data retention: nothing on the server is ever deleted (F1).

**Update 2026-10-30 (branch `p5-retention`):** F1 fixed (the retention sweep), so S18 is green: 20 green, 3 partial, 1 not run, 1
owner/HRM. F4's deployment files are done; the real proxy chain is still checked in WP17.

**Update 2026-10-30 (branch `p5-sec2`):** S14 and S21 are green, S19 is narrower (built-in TLS half done), F7 and F8 are fixed, and
the crash gap between `close_session` and `callback_closed` is closed: 22 green, 1 partial (S19), 1 not run (S22), 1 owner/HRM (S25). **2026-10-31:** S22 green, so 23 green.

## 1. Test runs

| Run | Before (0e3b630) | After (this branch) |
|---|---|---|
| `cargo test -p docserver -p fake-hrm -p suite-serve -p convert-api` (target `E:/office-target/p5-secreview/services`) | 161 passed, 0 failed | 165 passed, 0 failed (4 new tests) |
| `cargo clippy -p docserver --tests` | — | no warning |
| Playwright `playwright.docserver.config.ts` (`DIST=E:/office-target/p5/dist`) | 11 passed, 1 skipped (1.7 min) | 11 passed, 1 skipped |

New tests: `auth.rs` `dirty_and_history_routes_check_their_token`, `history.rs`
`compare_copies_a_crash_left_behind_are_removed_at_start`, `no_ids_in_logs.rs` `no_session_id_token_or_url_in_log_lines`,
`atrest.rs` `log_names_hide_the_session_id`; plus two cases in `config.rs` `frame_origins_are_origins_only`. Each of the three
fixes' tests failed before its fix (output of the red run: `left: 1, right: 0` for the leftovers; the log scan named
`atrest.rs` `self.sealed_name(path)` and `history.rs` `rec.sid`; frame origins kept `https://app.test:443`).

## 2. The §10 checklist

Paths are under `services/` unless they start with `ci/` or `crates/`. "Named test absent" means the row names a test that
does not exist under that name, but a test under another name proves the row; it is counted green.

| # | Status | Evidence | Note |
|---|---|---|---|
| S1 | green (was partial) | `docserver/tests/auth.rs:569` (renew, document, save, autosave, close); `auth.rs:759` new (dirty and the 5 history routes); `docserver/tests/history.rs:204` | Before: `dirty` and 4 of 5 history routes had no no-token/tampered/expired/other-session case. |
| S2 | green | `convert-api/src/jwt.rs:181` (`alg_none_rejected`, HS512); `docserver/tests/auth.rs:180` (none, HS512, RS256 headers at the route) | |
| S3 | green, named test absent | `auth.rs:180` cases "missing exp", "too long exp" (13 h), "11 h exp"; `require_exp` defaults to true (`docserver/src/config.rs:682`) | `config_token_without_exp_is_refused` does not exist. |
| S4 | green | `config.rs:689`, `config.rs:780` (distinct, no value in the message); `suite-serve/src/lib.rs:399` | |
| S5 | green | `auth.rs:538` | |
| S6 | green | `docserver/tests/outbound.rs:809` (302 not followed, `ftp:`, `user:pw@`, wrong port, https), `outbound.rs:911` (stalled body) | |
| S7 | green | `outbound.rs:689`, `outbound.rs:750`, `docserver/src/outbound.rs:309` | 169.254.169.254, `[::ffff:127.0.0.1]`, `2130706433`, a name to 10.0.0.5, the rebinding stub. |
| S8 | green | `auth.rs:363` | |
| S9 | green | `outbound.rs:845` (60 MB header, lying and absent length, zip bomb, renamed xlsx), `outbound.rs:929` (51 MB save and autosave) | The bomb is 40 MB of zeros in < 1 MB, not "1 KB to 2 GB"; the ratio rule is the same. |
| S10 | green, named test absent | `auth.rs:413` (save, autosave, dirty 403 in a view session), `outbound.rs:312` (no callback) | `view_session_cannot_save` does not exist. |
| S11 | green | `auth.rs:867` | |
| S12 | green | `docserver/src/routes.rs:188` | |
| S13 | green | `suite-serve/tests/docserver.rs:143` (headers, no CORS); `ci/web-smoke/docserver.spec.ts:277` (unlisted site cannot frame the wrapper or `/docs/`) | |
| S14 | green (was partial) | `docserver.spec.ts:253` (a foreign sibling frame posts `docs:event`, `docs:config`, `docs:close`, `docs:ready`); new `docserver.spec.ts:284` (the same frame posts `<prefix>:save`, `:dirty`, `:failed`, `:ready`, `:opened` for all four prefixes to the wrapper: no version, no status 1, no callback, state stays `ready`) | The wrapper checks `ev.source === S.frame.contentWindow` (`static/editor.js:301`). |
| S15 | green | `outbound.rs:1156` | |
| S16 | green | `outbound.rs:1029` (chain verifies, a rewritten line fails); `docserver.spec.ts:201` | |
| S17 | green (was failing) | `docserver.spec.ts:201` (stderr and audit of a full run); new `docserver/tests/no_ids_in_logs.rs` (every log macro of the crate) | Fixed F3. |
| S18 | green (was missing) | sweep: `docserver/src/retention.rs:242` (clock-injected: closed+acked 25 h gone, 23 h kept, abandoned 7 d gone, status 2 queued kept at 60 d, a given-up status 2 under the 7-day rule, history doc and its own blobs gone, a shared blob kept, one count-only audit line), `retention.rs:311` (a key still open elsewhere keeps its history), `retention.rs:326` (`history = "keep"`); `crates/suite-core/src/history.rs:1340` (`forget_id`); encryption part: `docserver/tests/wp14.rs:264`, `wp14.rs:317` | Fixed F1 (branch `p5-retention`). |
| S19 | partial (built-in TLS half done) | `convert-api/tests/tls.rs:69` (converter routes); new `ci/web-smoke/tls.spec.ts:129` (`/ds/editor` over built-in TLS: 200, HSTS `max-age=31536000`; plain HTTP to `/ds/editor?x=1` gets 308 to the https `public_url`) | `tls = "upstream"` behind Traefik is only in `docserver-compose.spec.ts` (Docker, not run: TLS-intercepting proxy) and asserts no header. |
| S20 | green | `suite-serve/tests/docserver.rs:128` | |
| S21 | green (was partial) | `convert-api/tests/no_hrm_names.rs:36` now scans `docserver/static/` too; there, and only there, the word `onlyoffice` is allowed on comment lines (clean-room note, API-compatibility remarks). Code, strings and every other name still fail the scan. | |
| S22 | green (2026-10-31) | `scripts/ci.sh deny`; `cargo deny` on `services/Cargo.toml` with `ci/deny/services.toml` (host and `x86_64-unknown-linux-gnu`) | The pinned `cargo-deny` 0.20.2 (`scripts/tools.lock`) was already in `E:/office-tools/bin`, only off the PATH. root, docs, sheets, slides, pdf: ok; services (docserver, fake-hrm, suite-serve, convert): `advisories ok, bans ok, licenses ok, sources ok` on both targets, duplicate-version warnings only. `cargo-audit` is not used: `cargo deny check advisories` reads the same RustSec database (owner left the choice to the agent, 2026-10-31). |
| S23 | green | `docserver.spec.ts:227` (401 `{"error":1}`); `outbound.rs:515` (500, `{"error":1}`, a silent provider = unknown); `outbound.rs:338` | The "sleeps 70 s" case runs as an 8 s hang against a short configured `callback_timeout_s`. |
| S24 | green | `auth.rs:569`, `auth.rs:759`, `history.rs:204` | |
| S25 | owner/HRM, not ours | — | HRB PR-1, PR-2 on `<test-host>`. |

## 3. Attacker review: findings

Severity is for the pilot set-up (one provider, Traefik in front, internal network). "Fixed" = on this branch with a test.

### High

**F1. Nothing the server stores is ever deleted (S18; fixed on `p5-retention`).**

*Fix.*
- `docserver/src/retention.rs`: a sweep at start and hourly (`spawn_sweep_loop`, `suite-serve/src/main.rs`).
  - A session folder goes `keep_closed_hours` (24) after a close the provider acknowledged (`Closed::acked`, set from the close's
    callback answer in `callback.rs`). Any other folder goes `keep_abandoned_days` (7) after its last activity.
  - A status 2 still queued (`queue/<sid>.json`, not given up) is never swept; an unreadable queue entry counts as queued.
  - Each decision and deletion runs under the session lock that every request's change holds.
- History: a swept session's history document and its unshared blobs are deleted too (`Store::forget_id`, new and narrow in
  `suite-core`), unless another session of the key is still on disk or the provider has `history = "keep"` (default `"session"`).
  The reasons and the trade-off are in PLAN §4 "Retention".
- Audit: one `retention.sweep` line with counts per sweep that deleted something; no ids (S17 scan still green).
- Not addressed: the history store's own limits for `history = "keep"` providers (last 50 or 90 days, newest kept) are unchanged.

*The finding as reported:*
- `sessions/<sid>/` (every version, `autosave.bin`, `session.json` with the provider's document URL and its token) has no
  sweep at all: no code in `docserver/src` deletes a session folder.
- The history store keeps the newest version of every document id for ever: the per-document rule keeps the last 50 or the
  last 90 days, whichever keeps more, and the 20 GiB rule never takes a document's newest version
  (`crates/suite-core/src/history.rs:804-845`; limits at `docserver/src/history.rs:76`).
- The HRM sends a new random `document.key` on every open (`docs/phase0/hrm-current-state.md:254`). So every open is a new
  history document (`doc_id`, `history.rs:156`) whose copy is never pruned.
- Failure scenario: a year of pilot use leaves every HR document ever opened on the docserver volume. With
  `encrypt_at_rest = false` (the default) it is plaintext, and a copy or backup of the volume holds all of it. That breaks Q10
  ("24 h after a good close, 7 days for an abandoned session, no backups") and PLAN §4 "Retention".
- Also: disk use only grows. Every snapshot prunes by reading every `history/docs/*.jsonl`, so saves get slower as documents
  pile up.
- Needs a choice, so not fixed here:
  - a sweep thread (24 h after `closed.at` and an acknowledged status 2; 7 days after `last_activity`; never while a status 2
    is queued);
  - what happens to the history of a swept session: history per key is useless with per-open keys, so delete the doc id with
    its session, or keep history only for providers with stable keys;
  - the clock-injected test S18 names.

### Medium

**F2. Plaintext compare copies survive a crash (fixed).** History > Compare writes both versions as plaintext into
`sessions/<sid>/compare-<rnd>/` (`history.rs:424`). They are removed by a drop guard (`history.rs:283`), which does not run
when the process is killed (OOM, power loss, `docker kill` during the 120 s compare timeout). With `encrypt_at_rest = true`
they stay readable on disk, and F1 means nothing removes them later. Fix: `sweep_compare_leftovers` at start
(`history.rs:172`, called from `config.rs:535`). Test: `history.rs:524`.

**F4. The rate-limit key collapses behind an untrusted proxy chain (deployment files fixed on `p5-retention`; the real chain is
checked in WP17).** `traefik.yml` now sets `entryPoints.websecure.forwardedHeaders.trustedIPs`, marked `OPERATOR` (loopback only in
the rehearsal: no front proxy). `deploy.md` "Client addresses behind proxies" says how the two lists line up. Correction to the fix
below: with a front proxy, our `trusted_proxies` must list Traefik **and** the front proxy, because Traefik appends the front proxy's
address after the browser's, and the docserver stops at the first untrusted address from the right. The client key is the rightmost
`X-Forwarded-For` entry that is not a trusted proxy (`convert-api/src/routes.rs:183-190`), counted per IPv4 address or IPv6
/64 (`convert-api/src/app.rs:120`). This is correct with browser → Traefik → docserver. If another proxy (an HRM nginx, a
load balancer) sits in front of Traefik, Traefik's own default discards the client's forwarded header and reports that proxy
as the client (`packaging/docserver/traefik/traefik.yml` sets no `forwardedHeaders.trustedIPs`).
- Failure scenario: all users share one key. 30 opens a minute across the whole organisation are then the limit
  (`rate_create_per_min`), and 20 bad tokens from anyone give everyone 429 for a minute.
- To fix in the deployment:
  - set `entryPoints.websecure.forwardedHeaders.trustedIPs` to the front proxy;
  - keep `trusted_proxies` = Traefik;
  - say so in `docs/phase5/deploy.md`;
  - test it on `<test-host>` (WP17).
- Related, low: the internal listener has no guard layer, so every caller of the file route is the one client "internal"
  (`docserver/src/api.rs:60-64`). 20 bad or expired file tokens a minute from anywhere in `internal_allow` refuse the
  provider's own downloads for that minute. Status 2 is retried, status 6 is not.

**F5. History is shared by every session of a provider and `document.key` (open, design note).** Any session of a key,
view-only included (`download` defaults to true), lists and downloads every past version of it (`history.rs:209`), including
versions made before that user was given access. ONLYOFFICE leaves history to the host (`setHistoryData`), so a host written
for it does not expect this. With the HRM's per-open keys nothing crosses sessions today. A provider with stable keys would
expose old versions across users. Options: a per-provider `history = "off" | "session" | "key"` switch, or tie it to F1's
decision. *F1 added `history = "session" | "keep"` (retention only): with `"session"` a key's history lives as long as one of its
sessions does. Sharing between concurrent sessions of one key is unchanged, so F5 stays open.*

### Low

**F3. Session ids in stderr on two error paths (S17; fixed).** Two lines printed the raw session id:
- a failed history snapshot (`history.rs:163-166`);
- a sealed file that does not open, whose name is `sessions/<sid>/v<N>.bin` (`docserver/src/atrest.rs:129-132`).

The e2e grep (`docserver.spec.ts:201`) sees only a good run's lines. Fix: `sid_ref` in both (`history.rs:165`, `atrest.rs:124`,
`log_name`). Test: `docserver/tests/no_ids_in_logs.rs` reads every `eprintln!`/`println!` of the crate and refuses a session
id, a token, a provider URL or `sealed_name`; and `atrest.rs:158`.

**F6. A frame origin written with its default port refused every session (fixed).** `frame_origins = ["https://hr.example:443"]`
was kept as written. The browser's `event.origin` never carries `:443`, so the create check (`docserver/src/sessions.rs:267-269`)
refused every open with 403 (onError -20), while `frame-ancestors` allowed the frame. This is a correctness bug, not a hole,
but the kind that would cost a day in WP17. Fix: `config.rs:468` drops `:443` for https and `:80` for http. Test:
`config.rs:882`.

**F7. Fetch errors reach the browser verbatim (fixed).** *Fix: `sessions.rs:485-500` answers every failed provider fetch (except TooLarge) with the fixed text "the document could not be fetched" (502 `fetch_failed`); the log and audit line carry only `unreachable` for a transport error (ureq's text may hold the URL and query). Tests in `outbound.rs` match the fixed text.* The finding as reported: `SessErr::Upstream(e.to_string())` (`sessions.rs:492`, answered as 502
by `api.rs:42`) carries ureq's error text: "host not found", "connection refused", timeouts. The user learns the state of the
provider's internal host. The URL is provider-signed, so the user cannot pick the target. A fixed text ("the document could
not be fetched") for `Unreachable` would do. Not changed here because `outbound.rs` tests match parts of these texts.

**F8. The compare tool's stderr goes to the browser (fixed).** *Fix: `history.rs` `run_diff`: the browser gets "the compare tool could not compare these versions" (and a fixed text when the tool cannot start); stderr and the spawn error go to the log with the session folder path replaced by `<session>` (no sid). Test: `history.rs` compare case.* The finding as reported: On a tool failure the first 2 KB of its stderr is the error message
(`history.rs:351`). It may name server paths (`.../sessions/<sid>/compare-.../a.docx`). Prefer a fixed text and log the
stderr (with `sid_ref`).

### Info (checked, no action)

- **Token confusion.** The three token kinds cannot be swapped:
  - session and file tokens use their own HMAC subkeys of `server_secret` with a `purpose` check (`tokens.rs`);
  - config tokens are verified against provider secrets that must differ from `server_secret`;
  - `authorize` checks sid, provider and `sub` against the record (`api.rs`);
  - the file token is bound to sid, ver and sha256, checked in constant time.

  Our callback tokens are signed with the provider secret, but their payload has no `document`, so they never open a session.
  The HRB side must keep `CRAFT_DOCSERVER_JWT_SECRET` apart from its other secrets (S25, `hr-backend-prompt.md:349`).
- **The body field `origin` of `POST /ds/session` is client-supplied.** The cross-provider check (`sessions.rs:267`) guards the
  honest wrapper flow only. Anyone holding a config token can call the route with any origin. The barrier is
  `frame-ancestors` plus the wrapper's own origin check. This is as designed.
- **SSRF** (`outbound.rs`):
  - one resolution, every answer judged, connection to the judged addresses only: no rebinding window;
  - no redirects, no proxy (env proxies off);
  - exact `scheme://host:port` match after lower-casing;
  - `@ ? # \ %` refused in the authority; decimal and octal hosts only reach the resolver if listed.
  - Not denied (classified public): `::ffff:0:0/96` (IPv4-translated), `100::/64` (discard), `192.88.99.0/24`. None of them is
    routed to an internal address by a default Linux stack, so no change was made.
- **Headers.** `baseline_headers` lets a route set only CSP, cache-control, X-Frame-Options, and CORP `cross-origin`
  (`convert-api/src/routes.rs:102-145`). An empty X-Frame-Options is honoured only beside a CSP whose `frame-ancestors` is not
  `'none'`. Only `/ds/editor` and, with the docserver on, the editor routes widen `frame-ancestors`
  (`suite-serve/src/lib.rs:140`).
  - Side effect: the editor bridge takes `&parent_origin=` (patch `wordcraft/0001`), so a page on a provider frame origin can
    frame `/docs/?host=parent&parent_origin=<itself>` and drive a bare editor. It gets no token and no server data.
- **postMessage.**
  - `api.js` posts only to the docserver origin and hears only its own iframe window.
  - The wrapper hears the host only from `window.parent` at the `#o=` origin. That origin must also pass `frame-ancestors`
    and the server's check.
  - It hears the editor only from its own iframe at its own origin, and posts to the editor with a pinned target origin.
  - The session token stays in the wrapper's memory: no storage, no cookie (the spec checks storage).
- **Path traversal.**
  - `sid` is 32 lower-case hex before any path is built (`session_dir`, `queue_path`);
  - version numbers are `u64`, history ids are hex digests;
  - the file route loads by path sid only through `session_dir`;
  - compare file extensions are filtered to alphanumerics.
- **Zip bombs and types.** The central directory is checked (entry count, total, ratio) without inflating. The later inflations
  (xdfu grif check and stamp) read through capped `take()`. The type check needs `[Content_Types].xml` and the type's main
  part. A package holding both `word/document.xml` and `xl/workbook.xml` passes as docx, which is harmless: the editor decides.
- **At rest.** AES-256-GCM with the file name as AAD stops swaps. A rollback to an older sealed copy of the same file is not
  detected; that is outside the stated threat model (disk theft and copies).

## 4. Not checked, and why

- **Crash gap (data loss, R3), fixed on `p5-sec2`:** `callback.rs` `recover_unnotified`, run once by `spawn_retry_loop` before its loop. A session `closed` with status 2, `notified = false`, `acked = false`, edit rights, a version, and no queue entry gets `callback_closed` (queue entry, send, `notified` written). `notified` is written only after the attempt; `acked` is written when the provider answers `{"error":0}`. So an acked record or a queued one is never resent. Window left: a crash after the provider acted but before `acked` reaches disk can send status 2 twice (at-least-once; the provider gets the same version). Status 4 (no changes) is not resent. Test: `outbound.rs` `a_close_that_died_before_its_callback_sends_status_2_once_at_start`. Not checked: a crash during the retry loop's own send (the queue entry covers it, by reading).

- **S22:** green since 2026-10-31 (see the row): the pinned `cargo-deny` from `E:/office-tools/bin`; `cargo-audit` not needed.
- **The Docker compose rehearsal** (`packaging/docserver/rehearse.sh`, `docserver-compose.spec.ts`): not run. Docker's
  network sits behind the TLS-intercepting proxy, and the image build takes long. So `tls = "upstream"` behind real Traefik
  and the forwarded-header behaviour of F4 are reasoned from the code and the configs, not measured.
- **Real HRM traffic and `<test-host>`:** off limits. F4's proxy chain and S25 can only be checked there (WP17).
- **The editor bridges' message handling inside the wasm editors** (patches `wordcraft/0001`, `deckcraft/0002` and their Sheets
  and PDF peers): read only for the origin rule, not reviewed in depth.
- **Timing side channels** of the token checks: by reading only (HMAC verify is constant time, `ct_eq` for digests); not
  measured.
- **ureq error texts that might hold a URL** (a `BadUri` for a URL ureq cannot parse): only a malformed provider-signed URL
  could reach it; not exercised.

## 5. Commits

See `git log phase-5..p5-secreview`: the fixes F2, F3 and F6 with their tests, the S1 test, and this report.
