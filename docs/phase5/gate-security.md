# Phase 5 gate, security part (WP18): checklist S1-S25 and the attacker review

Date 2026-11-24/25. Branch `p5-gate-sec` from `phase-5` at e81a8a12. Architect review (opus), this PC only: local Docker, no HRM test
server, no `<test-host>`, no production system, no `corpus/` file. The dry run was `security-rehearsal.md` (2026-10-30); this is the gate
run on the gate commit. Owner decision 2026-11-24: the gate is not tied to the HRM, so the review also covers the server as any
integrator installs it (the Docker image configured by env like an ONLYOFFICE Document Server), section 4.

**Result in one line:** 23 PASS, 1 PARTIAL (S19: the real proxy chain measured here with Traefik, but not behind a front proxy or a
real host), 1 owner/HRM (S25). Two rows failed on the gate commit and are fixed on this branch: S21 (three code comments named the
product) and S22 (a new RustSec advisory in the services lock). The attacker review found three medium-or-higher problems, all
reproduced and fixed with a test that fails without the fix: **G1 (high)** anonymous upload bodies were buffered before the token check,
eight requests OOM-killed a 512 MB container; **G2 (medium)** a trusted proxy named by host (the drop-in's Traefik) was capped at 32
connections, one client with slow uploads took the server from everyone; **G3 (medium)** a history restore skipped the drop-in's
fetch-before-change rule (D1), so a replayed `exp`-less config token could save any bytes to the provider.

## 1. Runs

Env (Git Bash): `PATH="$HOME/.cargo/bin:/e/office-tools/bin:$PATH" CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_HTTP_CHECK_REVOKE=false
TMP=TEMP=E:/office-target/tmp CARGO_TARGET_DIR=E:/office-target/p5-gate-sec`.

| Run | Gate commit e81a8a12 | This branch |
|---|---|---|
| `cargo test --no-fail-fast -p docserver -p fake-hrm -p suite-serve -p convert-api` (services) | 189 passed, **1 failed** (`no_hrm_names_in_code`) | 192 passed, 1 failed (193 = 190 + 3 new). The one failure is a clock bound under load (`outbound.rs:367`, the retry time within now+55..65 s; this PC ran several other agents' builds): it passed alone, as did the four such bounds that failed in an earlier full run here (`auth.rs:942`, `outbound.rs:367`, `:576`; `outbound.rs:926` fails the same way on the unfixed code, 15.7 s against an 8 s bound) |
| the three new tests alone, before their fixes | — | all three fail: `a_body_is_read_only_after_its_token` (413, want 401), `restore_waits_for_the_providers_document_like_a_save` (200, want 409 `no_document`), `a_trusted_proxy_named_by_host_is_not_capped_per_address` (third connection gets no answer) |
| `cargo clippy -p docserver -p convert-api -p suite-serve --tests -- -D warnings` | — | `cargo fmt --check` and clippy `-D warnings`: clean |
| `cargo deny check` on `services/Cargo.toml` with `ci/deny/services.toml`, host and `x86_64-unknown-linux-gnu` | host: **advisories FAILED** (RUSTSEC-2026-0594, `cookie` 0.18.2); linux: ok | host and linux: `advisories ok, bans ok, licenses ok, sources ok` (duplicate-version warnings only) |
| `scripts/ci.sh deny` (root, docs, sheets, slides, pdf) | — | `ok root, docs, sheets, slides, pdf` |
| Playwright `playwright.docserver.config.ts` (`OFFICE_TARGET=E:/office-target/p5-gate-sec`, `DIST=E:/office-target/p5/dist`, dists of 2026-11-21/23) | — | 35 passed, 2 failed, 1 skipped (5.4 min). Every S row's case passed (S13, S14 twice, S16/S17 full run, S23, D3b). The two failures are not security rows and are load flakes: `WP9 letter ... Latin to Cyrillic` passed on rerun; `HRM Back 300 ms after an edit, xlsx` (a typed key lost, the test's own comment calls it an engine matter under load) failed 3 of 9 runs on this branch and 2 of 8 on the unfixed binary, alternated |
| Docker probes on the image `hoshiya-docserver:p5-5b752c69` (built 2026-11-11; the services code differs from the gate commit only in the zip feature line of two `Cargo.toml`s) | section 4 | — |
| Memory probe, debug `hoshiya-serve.exe` on this PC, 8 parallel anonymous 100 MB save bodies through the trusted-proxy address (`scratchpad memprobe.sh`) | peak working set **720 MB**, every answer 401 | peak working set **28 MB**; each request answered 401 at once (the client is cut off while still sending; with a 1 KB body the status line reads `401 Unauthorized`) |

## 2. The checklist (PLAN §10)

PASS = the named test, or a test that proves the row under another name, ran green on this branch. Paths are under `services/` unless
they start with `ci/` or `packaging/`.

| # | Status | Evidence | Note |
|---|---|---|---|
| S1 | PASS | `docserver/tests/auth.rs:662` `every_route_checks_its_token`, `auth.rs:852` `dirty_and_history_routes_check_their_token`, `docserver/tests/history.rs:259` | each route without, tampered, expired, other session's token |
| S2 | PASS | `convert-api/src/jwt.rs:181` `alg_none_rejected` (and HS512); `auth.rs:180` `config_token_refusals` (none, HS512, RS256 headers at the route) | `alg` must be exactly `HS256` before the MAC is computed (`jwt.rs` `verify_with`) |
| S3 | PASS (named test absent) | `auth.rs:180` cases "missing exp", 13 h, 11 h; `docserver/src/config.rs` `the_docsapi_compat_mode` (the D1 alternative: `compat = "docsapi"` accepts no `exp`, 12 h session cap) | Q4 alternative recorded in `dropin.md` D1; `config_token_without_exp_is_refused` is the "missing exp" case of `config_token_refusals` |
| S4 | PASS | `docserver/src/config.rs` `refuses_without_a_server_secret_or_with_a_short_one`, `secrets_must_differ`, `the_docsapi_compat_mode`; `suite-serve/src/lib.rs:421`; Docker: `JWT_SECRET=secret` refused, "providers[0].secret_file must hold at least 16 bytes", no value printed | compat mode: 16 bytes minimum, warning under 32 (D4) |
| S5 | PASS | `auth.rs:631` `outer_fields_are_ignored` | |
| S6 | PASS | `docserver/tests/outbound.rs:810` (302 not followed, `ftp:`, `user:pw@`, wrong port, https), `outbound.rs:912` (stalled body) | |
| S7 | PASS | `outbound.rs:690`, `outbound.rs:751`, `docserver/src/outbound.rs:309` `every_deny_range` | 169.254.169.254, `[::ffff:127.0.0.1]`, `2130706433`, a name to 10.0.0.5, the rebinding stub |
| S8 | PASS | `auth.rs:299` `config_field_refusals_and_modes` (a callback URL only on the fetch list) | the env path defaults `DOCSERVER_CALLBACK_ALLOW` to the fetch list (section 4) |
| S9 | PASS | `outbound.rs:846` (60 MB header, lying and absent length, zip bomb, renamed xlsx), `outbound.rs:930` (51 MB save and autosave), new `outbound.rs:1003` | |
| S10 | PASS (named test absent) | `auth.rs:299` (save, autosave, dirty 403 in a view session), `outbound.rs:313` `a_view_session_reads_and_sends_no_callback` | |
| S11 | PASS | `auth.rs:960` `file_token_bindings` | |
| S12 | PASS | `docserver/src/routes.rs:222` `internal_listener_serves_files_only`; `suite-serve/tests/docserver.rs:372` (shared port); Docker: a stranger on the internal port gets 403 | |
| S13 | PASS | `suite-serve/tests/docserver.rs:149` `framing_and_corp_rules_with_the_docserver_on`; `ci/web-smoke/docserver.spec.ts:314` | |
| S14 | PASS | `docserver.spec.ts:263`, `docserver.spec.ts:287` | read again: `api.js` checks origin and window; `editor.js` returns early when not framed (`window.parent === window`), so a popup of `/ds/editor#o=<any>` hears nothing |
| S15 | PASS | `outbound.rs:1200` `session_creation_and_failed_auth_are_limited_per_client_address` | behind a proxy named by host see G2 |
| S16 | PASS | `outbound.rs:1073` (chain verifies, a rewritten line fails); `docserver.spec.ts:133` | env mode keeps the audit key on the data volume unless mounted (section 4, L3) |
| S17 | PASS | `docserver/tests/no_ids_in_logs.rs:41`; `docserver/src/atrest.rs:158`; `docserver.spec.ts:133` (stderr and audit of a full run) | |
| S18 | PASS | `docserver/src/retention.rs` tests (`sweep_deletes_closed_and_abandoned_sessions_with_their_history` and four more); `docserver/tests/wp14.rs:264`, `wp14.rs:322` | |
| S19 | PARTIAL | built-in: `convert-api/tests/tls.rs`, `ci/web-smoke/tls.spec.ts`; upstream in-process: `convert-api/tests/hardening.rs:757`; **real Traefik v3.6 → image, measured here** (section 4): HSTS `max-age=31536000` on the https answer, plain http → 308 to the https `public_url`, a direct (non-proxy) request → 403 | not measured: a front proxy before Traefik (F4) and a real host name with a real certificate |
| S20 | PASS | `suite-serve/tests/docserver.rs:134` `no_providers_no_docserver_routes` | |
| S21 | PASS (was FAIL) | `convert-api/tests/no_hrm_names.rs:39` | on e81a8a12 it failed on three comment lines in `docserver/src/config.rs:56` and `sessions.rs:542,563` (merged 2026-11-09/11 after the last green run); reworded |
| S22 | PASS (was FAIL) | `cargo deny` services, host and linux: ok; `scripts/ci.sh deny`: ok (root, docs, sheets, slides, pdf) | RUSTSEC-2026-0594 (`cookie` 0.18.2, nonce reuse in private cookies after `fork`) came in through `wry` (suite-desktop, Windows only); not reachable (the `private` feature is off: no `aes-gcm` in its lock entry, and Windows does not fork), fixed anyway by `cargo update -p cookie --precise 0.18.3` (lock only). The same entry is in phase-4's services lock |
| S23 | PASS | `outbound.rs:516` (500, `{"error":1}`, a silent provider = unknown), `outbound.rs:339`; `docserver.spec.ts:237` | the 70 s sleep runs as an 8 s hang against a short `callback_timeout_s` |
| S24 | PASS | `auth.rs:662`, `auth.rs:852`, `history.rs:259` | |
| S25 | owner/HRM | — | no longer a gate condition (owner, 2026-11-24): the gate is the generic drop-in, not an HRB flow |

## 3. Attacker review: findings

Severity is for a generic install: the image behind a TLS proxy, one or more providers, configured by env (`entrypoint.sh`) or TOML.
"Verified" names how; nothing below is reported from reading alone unless it says "unverified".

### High

**G1. Anonymous upload bodies were buffered before the token check (fixed).**
- `POST /ds/session/{sid}/save` and `.../autosave` took the body as `Bytes` (`docserver/src/api.rs:238`, `:271` on e81a8a12). axum
  extracts the whole body (up to `max_document_mb`, 100 in the env config) before the handler runs, so `authorize` came after the
  buffering. Any client that reaches the proxy, with no token and any 32-hex sid, could make the server hold `max_document_mb` per
  request; the proxy is exempt from the per-address cap, so the bound was the total of 1024 connections.
- Verified twice: (1) Docker, image `p5-5b752c69`, `--memory 512m`, env config, Traefik's place taken by a client on the trusted-proxy
  address: eight parallel 100 MB POSTs with no token → the container was killed, `OOMKilled=true`, exit 137. (2) Native debug build:
  the same eight requests, each answered 401, peak working set 720 MB.
- Fix: the two handlers take `Body` and read it with `read_body` only after `authorize` and `editable`, capped at `max_document_mb` + 1
  (`api.rs` `read_body`); the inert `DefaultBodyLimit` layers on those routes are gone. `close` already read its body after the token.
  What stays pre-auth by design: `POST /ds/session` (the token is in the body), 128 KB; history `name`, 4 KB.
- Test: `docserver/tests/outbound.rs:1003` `a_body_is_read_only_after_its_token` (an over-cap body without or with a junk token: 413
  before, 401 after). After the fix the memory probe peaks at 28 MB (was 720 MB).

### Medium

**G2. A trusted proxy named by host was capped like one client (fixed).**
- `ConnLimits::from_settings` exempted only the CIDR entries of `trusted_proxies` from the per-address connection cap
  (`convert-api/src/tls.rs:73` on e81a8a12); names (`NameSet`) were left out. The drop-in and the env config name the proxy by container
  name (`DOCSERVER_TRUSTED_PROXIES=<traefik>`, `dropin.md` D2, D8), so every user behind Traefik shared `max_conn_per_ip` = 32
  connections, and `dropin.md` D2b's "Traefik is exempt from the per-address count" was not true.
- Verified with real Traefik v3.6 in front of the image (`DOCSERVER_TRUSTED_PROXIES=gsec-traefik`): one client opened 34 slow uploads
  (4 KB at 100 B/s to `/ds/session`) through Traefik; while they ran, every other request through Traefik, `/healthz` included, got
  **502** (3 of 3); before and after, 200.
- Fix: `ConnLimits` gets `exempt_names` (the live `NameSet` of `trusted_proxies`), checked next to `exempt` at accept (`tls.rs`).
- Test: `convert-api/tests/hardening.rs:607` `a_trusted_proxy_named_by_host_is_not_capped_per_address` (`trusted_proxies =
  ["localhost"]`, `per_ip = 2`, three connections: the third got no answer before, all three are answered after).
- Left: the total (`max_connections` 1024) is shared by everyone behind the proxy; slow clients are then the proxy's to cut (Traefik v3's
  entry point `readTimeout` defaults to 60 s).

**G3. A history restore skipped the fetch-before-change rule (D1) (fixed).**
- `dropin.md` D1 bounds a replayed config token without `exp` (`compat = "docsapi"`, the env config's mode) by refusing save, autosave
  and dirty until the provider's document was fetched (`api.rs` `editable`). `POST .../history/{v}/restore` checked only the `edit`
  permission (`docserver/src/history.rs:236`), and its `put_version` made the restored bytes version 1 of the new session.
- Attack: within the history's life (with `history = "session"` as long as any session of the key is on disk, 7 days after its close by
  default), a config token of that key, replayed after its `document.url` stopped working, opens a session (no fetch needed), restores an
  old version (the provider gets status 2 with it at close), and from then on `editable` passes, so `save` sends any bytes to the provider
  as status 6.
- Verified by the test before the fix: restore answered 200 in a session whose document was never fetched.
- Fix: `restore` calls `editable` after the permission check (`history.rs`, one line). Test: `docserver/tests/history.rs:207`
  `restore_waits_for_the_providers_document_like_a_save` (restore 409 `no_document`, the following save 409, no version stored).

### Low (not fixed; each is a recommendation)

- **L1. An authenticated edit session can fill the disk.** No cap on versions per session and no free-space floor in `put_version`
  (`sessions.rs`); each save is up to `max_document_mb` and is also copied into the history store. Needs a valid edit session (an
  employee, or a leaked config token whose `document.url` still works). Verified by reading only. Suggestion: a free-space floor (the
  converter has one) and a per-session version cap.
- **L2. `trusted_proxies` accepts `0.0.0.0/0` and `::/0`** (`convert-api/src/config.rs` `Cidr::parse`; `internal_allow` refuses a /0).
  An operator who writes it turns the "proxy only" rule off and lets any client choose its rate-limit bucket by `X-Forwarded-For`.
  Verified by reading. Suggestion: refuse a /0 as `internal_allow` does.
- **L3. Env mode keeps generated secrets on the data volume.** With no `/run/secrets` mount the entrypoint writes `server-secret`,
  `converter-secret` and `audit-key` to `/data/secrets` (warned at start; verified in Docker). A copy of the volume then holds the key of
  the session and file tokens, and the audit key sits next to the log it protects. The installer always mounts them; a hand `docker
  run` does not. For the quick-start doc: mount `/run/secrets`.
- **L4. F5 (rehearsal) is still open:** every session of one provider and `document.key`, view-only included, lists and downloads every
  past version of that key. With per-open keys (the HRM) nothing crosses; an integrator with stable keys exposes old versions across
  users. ONLYOFFICE co-editors of one key see each other's live edits, so the exposure is close to that level.
- **L5. `entrypoint.sh` says "generated" when it could not write** (`/data` read-only, no volume): `secret()` runs inside `$(...)`, so
  its `mkdir` failure does not stop the script. The start still fails closed ("cannot read /data/secrets/server-secret"), verified in
  Docker. Cosmetic.

### Checked, no finding (this run)

- Token confusion, alg confusion, `exp`/`nbf`, session fixation: as in the rehearsal (`security-rehearsal.md` §3 Info), read again on
  this commit. Session ids are server-made 128-bit random values bound to the session token; no cookie anywhere on the docserver routes.
- Callback errors in logs: `send_callback` logs `OutErr` text; ureq's `BadUri` texts that name the URL need a URL without scheme or host,
  which `url_allowed` never passes (read in ureq 3.4.2 `util.rs:330-337`).
- Races: save, autosave, close, idle close and the sweep take the one session lock; two concurrent saves of one session may send their
  status 6 out of order (only the session's own user can cause it; the wrapper serializes saves).
- Malicious documents on the server: nothing is inflated on the server except `docProps/core.xml` (capped 1 MB) and the XDFU checks
  (capped `take()`); the compare tools run with a 120 s timeout and a 16 MB output cap. Parsing in the editors (images, XML) is the
  browser's and was not reviewed here.
- Container: uid 10002, no capabilities, `/app` read-only to it; works with `--read-only --cap-drop ALL --security-opt
  no-new-privileges` (installer and compose set them).

## 4. The generic configuration surface (owner decision 2026-11-24)

Probed with `docker run` of `hoshiya-docserver:p5-5b752c69` (commands in the report of this branch; nothing outside local Docker).

| Situation | What happens | Verdict |
|---|---|---|
| No env, no config file | `server: cannot read /etc/hoshiya/api.toml`, exit 2 | fail closed |
| `JWT_ENABLED=false` | `this server never runs without a token`, exit 64 | fail closed (ONLYOFFICE would run open) |
| `JWT_SECRET` set, a required `DOCSERVER_*` missing | `DOCSERVER_PUBLIC_URL is required`, exit 64 | fail closed |
| `JWT_SECRET=secret` (6 bytes) | refused, "at least 16 bytes", no value printed | fail closed; 16-31 bytes run with a warning (D4) |
| No `/run/secrets` | server, converter and audit secrets generated once into `/data/secrets` with a warning | L3 |
| No volume and `--read-only` | refusal at start (cannot read the server secret) | fail closed, L5 |
| Built-in default secret | none exists | — |
| `require_exp` | off in env mode (`compat = "docsapi"`), warned at start | by design (D1); G3 closed its gap |
| `DOCSERVER_TRUSTED_PROXIES` | required; names and CIDRs; `0.0.0.0/0` accepted | L2 |
| `DOCSERVER_INTERNAL_ALLOW` | required; `/0` refused | ok |
| `DOCSERVER_CALLBACK_ALLOW` | defaults to the fetch list | the two lists are equal unless the operator splits them |
| `DOCSERVER_FETCH_ALLOW` | every entry `internal = true` (private addresses allowed, metadata never) | fine for an internal backend; the exact host:port still binds it |
| `DOCSERVER_ALLOW_OPAQUE_ORIGIN` | off unless `1` | ok (D3b) |
| Through Traefik (`gsec-traefik` by name) | https answer carries HSTS; plain http → 308 to `https://<public_url>/...`; a direct request to 8787 → 403; a stranger on the internal port → 403 | ok; G2 fixed the connection cap |
| Process | uid 10002, `CapEff` 0, `/app` not writable | ok |

### Merged image (phase-5 9af9a861)

Verified 2026-11-25 on branch `p5-verify` (= phase-5 9af9a861: G1-G3, S21/S22 and the quick-start entrypoint/README/rehearsal merged), image
`hoshiya-docserver:p5-verify` built by `packaging/docserver/build.sh` (new `TAG` env; default stays `rehearsal`). Local Docker only.

- Criterion (1): `rehearse-generic.sh` (fresh `docker run`, documented env only, generic ONLYOFFICE-API host, Chromium): **8 of 8** on the first
  run (docx, xlsx, pptx, pdf: open -> edit -> save -> reopen -> download, plus view-only; callbacks 1,6,2 each; no xlsx flake); log hygiene 0 tokens.
- Fail-closed probes (same image, `docker run --rm`; the secret value is never printed):

| Probe | Exit | Message |
|---|---|---|
| no env, no config | 64 | `JWT_SECRET or JWT_SECRET_FILE is required ... or mount a config file and pass --config` (was 2 on the gate image) |
| `JWT_ENABLED=false` | 64 | `this server never runs without a token` |
| `DOCSERVER_PUBLIC_URL` missing | 64 | `DOCSERVER_PUBLIC_URL is required` |
| `JWT_SECRET=secret` | 2 | `providers[0].secret_file must hold at least 16 bytes` (no value) |
| no TLS cert, no `DOCSERVER_TRUSTED_PROXIES` | 64 | `TLS is required: set DOCSERVER_TLS_CERT and DOCSERVER_TLS_KEY ..., or DOCSERVER_TRUSTED_PROXIES` |
| unreadable TLS cert path | 64 | `cannot read the files (they must be readable by uid 10002)` |
| `--read-only`, no volume | 2 | `mkdir: cannot create directory '/data/secrets': Read-only file system`, then `server_secret_file: cannot read /data/secrets/server-secret` |

- Tests (`services/`): `convert-api` 78 passed, 0 failed; `docserver` 85 passed, 0 failed (`--no-fail-fast`). The first `docserver` run under load had
  two timing failures in `tests/outbound.rs` (`a_failed_callback_keeps_the_document_dirty_and_the_retry_survives_a_restart`,
  `session_creation_and_failed_auth_are_limited_per_client_address`); both pass alone and in the full rerun.
- Not run: the "TLS certificate and no proxy keeps the per-address cap" check (section 5), Traefik rows, compose/drop-in rehearsals.
  The `--read-only` refusal prints shell errors before the server's own message; it still fails closed (exit 2).

## 5. Not run, and why

- The compose rehearsal (`packaging/docserver/rehearse.sh`, `docserver-compose.spec.ts`) and `rehearse-dropin.sh`: not run; the real
  Traefik checks of section 4 cover the proxy part. No new image was built from this branch (the build vendors and compiles in Docker,
  long, and E: was already under 60 GB from parallel builds); G1 and G2 were reproduced on the 2026-11-11 image and their fixes measured
  natively and by the tests.
- The entrypoint of branch `p5-quickstart` (built-in TLS by `DOCSERVER_TLS_CERT`/`KEY`, a refusal when there is neither a JWT secret nor
  a readable `--config`, `DOCSERVER_TRUSTED_PROXIES` required only without a certificate) is not on this branch: section 4 probed the gate
  commit's `entrypoint.sh` through the image tagged `p5-5b752c69`, not the re-tagged `:rehearsal`. After the merge, rerun section 4's
  rows on the merged image, and check that with a certificate and no proxy the per-address cap still applies (no proxy, no exemption).
- A front proxy before Traefik (F4), a real certificate and host name, and real HRM traffic: off limits here.
- The editors' own parsing of hostile documents (huge images, XML in the browser): not in the server's scope, not reviewed.
- Timing side channels: by reading only (HMAC verify and `ct_eq` are constant time).

## 6. For the owner

- Nothing needs a decision to pass this part of the gate. Recommended next (low): L1 (disk floor and version cap), L2 (refuse a /0
  `trusted_proxies`), and in the quick-start doc a `/run/secrets` mount (L3).
- `entrypoint.sh` and `install.sh` were not changed.
- The `cookie` bump should go to phase-4 too (the same lock entry), then merge forward as usual.

## 7. Commits

See `git log phase-5..p5-gate-sec`.
