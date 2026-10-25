# Phase 5 (Document server + HRM integration): execution plan

> **Owner decisions and requirements in force (gathered 2026-10-15 … 2026-10-25)**
> - **Phase 5 starts early, in parallel** (owner, 2026-10-25; `CLAUDE.md:4-6`). Branch `phase-5`, worktree
>   `E:/office-wt/phase-5`. The Phase 4 gate stays open until WP23 and its report. Phase 4 fixes land on `phase-4` and are
>   **merged forward** into `phase-5`, never the other way. The Phase 5 gate comes after the Phase 4 "go".
> - **The HRM integration target is hr-backend (HRB)** (owner, 2026-10-16; `docs/STATUS.md:884-887`). The sandbox
>   prompt is superseded (`docs/phase0/hrm-backend-prompt.md:1`). The HRM-side work order is
>   `docs/phase0/hr-backend-prompt.md` §4 (PR-0 … PR-8).
> - **The HRM test server is unchanged since the Phase 0 survey** (owner confirmed unchanged, 2026-10-25).
>   `docs/phase0/hrm-current-state.md` (HRB @ `7f27f79`, 2026-10-15/16) is treated as current. A consequence: none of the
>   HRB PRs (PR-0 … PR-3) is on the test server yet. We have **no access** to the server now; §9 keeps only the questions
>   the Phase 0 analysis itself left open.
> - **Accounts are optional** (owner, 2026-10-21). Local mode needs no login. A server connection is optional. The HRM is
>   one configured token issuer or SSO provider, never required or named in code (`docs/phase4/PLAN.md:319-329`;
>   test `services/convert-api/tests/no_hrm_names.rs`).
> - **Version history independent of the HRM** (owner, 2026-10-21; `docs/STATUS.md:9-30`): Phase 5 = the server store,
>   the diff engines, restore, and the HRM backend of the same interface.
> - **XDFU is opt-in and off by default everywhere**, the docserver included (`docs/phase3/PLAN.md:950-952`).
> - **Pilot flow: HRB `files` docx — assumed, owner may change** (owner, 2026-10-25: "start the work without waiting for my
>   answer"; recommendation Q1, §13.4). Only **WP1** (the key prefix and builder the harness mimics), **WP9** (moves onto the
>   gate path for a decree, letter or report flow) and **WP17** (pilot config) change if the owner picks another flow.
> - **Pilot is Docs-only first** (`docs/ARCHITECTURE-DECISION.md:31`, `:522`): `OFFICE_CRAFT_FILE_TYPES=docx`, behind a
>   flag, ONLYOFFICE stays the instant fallback.
> - **Standing rules:** product name only from `brand/brand.toml`; MIT OR Apache-2.0; permissive deps only (`cargo-deny`,
>   `ci/deny/services.toml`); every upstream change a carried patch with a `patches/REGISTER.md` row; HRM test environment
>   only (<test-server>, `<test-host>`), read-only analysis, no deploy without the owner's explicit OK; never copy `corpus/`.
> - **Clean-room:** the DocsAPI-shaped embed API is designed from ONLYOFFICE's **public API documentation** and from what
>   HRB itself sends and expects (`hrm-current-state.md` §4). No ONLYOFFICE (AGPL) source is read, copied or vendored.
>   The spike's `api.js` is our own code (`spikes/docserver/static/api.js:1-38`).

| | |
|---|---|
| **Status** | Plan only, written 2026-10-25 on branch `phase-5`. No product code. |
| **Inputs** | `docs/BRIEF.md` Phase 5 (:389-413) and App. B (:571); `docs/phase0/hrm-current-state.md`, `hrm-integration-design.md`, `hr-backend-prompt.md`, `docserver-results.md`, `spike-plans.md` §4; `spikes/docserver/`; `docs/ARCHITECTURE-DECISION.md` §2.4 (:430-468), :777; `docs/STATUS.md:9-30`, `:524-533`; `docs/phase1/SIZING.md` §2.4 (:211-282); `docs/phase3/PLAN.md` §5.5 (:430-450), §9.4 O10-O11 (:922-923), §10 (:928-963); `docs/phase4/PLAN.md` F2/F3/F7/F8 (:79-85), §4 (:247-262), §5.5-5.6 (:299-329), :523; `docs/phase4/gate-review.md` §8 (:461-478); `services/convert-api`, `services/suite-serve`, `crates/suite-core`, `crates/xdfu-core`. |
| **Exit gate (brief)** | (1) One real HRM document flow works end to end on the test environment: open → edit → save → reopen → download. (2) The security checklist passes (§10). |
| **Estimate** | **About 42.5 stream-days likely (range 30.5–64.5)**: 24.5 on the gate path, 18 beside it (§13.1). The HRM side is a separate stream: the pilot subset PR-0 … PR-3 plus `editor=auto` is 4.75 / 6.25 / 9 d (`SIZING.md:262`). |

How to use this plan. §0 holds the facts found while planning. §1–§8 are the designs. §9 lists the open HRM
questions. §10 is the security checklist the gate runs. §11 holds the work packages; each can be handed to one
sub-agent as written. §12 is the order and what starts without the owner. §13 has the estimate, risks and the owner
decisions. The ONLYOFFICE field table is its own file: `docs/phase5/onlyoffice-compat.md`.

---

## 0. New facts found while planning (they change the design)

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **HRB config tokens carry no `exp`.** Our ported JWT code refuses a token without `exp`. | `hr-backend-prompt.md:829` ("HS256 over the whole config, no `exp`"); `services/convert-api/src/jwt.rs:75-77` (`NoExpiry`) | Either HRB adds `exp` (its PR-3 already offers it as a separate commit, `hr-backend-prompt.md:513-514`) or we accept a replayable config. Owner decision Q4; recommendation: require `exp`. |
| F2 | **`jwt::claims` hard-codes `scope == "convert"`.** | `services/convert-api/src/jwt.rs:91-94` | The docserver reuses `jwt::sign` / `jwt::verify` only and writes its own claim rules. No change to the converter. |
| F3 | **The spike uses one allowlist for both the document URL and the callback URL, and checks only `host:port`** (no DNS or IP check). HRB's `document.url` host is `api:8000`, a private Docker address. | `spikes/docserver/src/main.rs:105-106`; `spikes/docserver/src/fetch.rs:11-24`; `hrm-current-state.md:227-230`, `:256` | Separate per-provider lists for fetch and callback. A resolver check refuses private, loopback, link-local and metadata addresses **unless the operator marked that entry `internal`** (brief task 5: "no internal addresses unless configured"). We connect to the address we checked (no DNS rebinding). |
| F4 | **With `tls = "upstream"` the server answers only its trusted proxy**, and redirects plain HTTP. HRB downloads a saved file **directly** from our internal URL: it rewrites the callback `url` host to `CRAFT_DOCSERVER_INTERNAL_URL` and fetches over the Docker network. | `services/convert-api/src/routes.rs:206-240`; `hrm-current-state.md:289-290`; `hr-backend-prompt.md:351`, `:519-524` | The docserver needs a second, **internal listener** that serves only the token-bound file route (`/ds/files/{sid}/{ver}`), restricted to configured CIDRs. Without it every HRB save download gets 403 `not_via_proxy`. |
| F5 | **Our header baseline forbids exactly what an embed needs.** Every response gets `frame-ancestors 'none'`, `X-Frame-Options: DENY`, `Cross-Origin-Resource-Policy: same-origin`, and only CSP, cache-control and XFO may be overridden per route. The editor routes allow `frame-ancestors 'self'` only. The HRM SPA loads `api.js` from another origin and frames our page; `frame-ancestors` is checked against **every** ancestor, so the nested editor iframe is blocked too. | `services/convert-api/src/routes.rs:96-123`; `services/suite-serve/src/lib.rs:97-113`; `hrm-current-state.md:226`, `:302-319` | Per-route overrides: `api.js` gets `Cross-Origin-Resource-Policy: cross-origin`; the wrapper page **and** the editor routes get `frame-ancestors 'self' <provider frame origins>` and no XFO when the docserver is on. `cross-origin-resource-policy` joins the routable set. The review-m5 rule ("a later route cannot weaken it") is kept by making the set an explicit list with a test. |
| F6 | **The editor runs inside a cross-site iframe**, where browsers block or partition third-party cookies. | design reasoning; the spike ran both pages on `localhost` (same site), so it could not show this (`docserver-results.md:6-7`) | Per-request auth is a **bearer session token held in the wrapper page's memory**, never a cookie. The cross-origin Playwright run uses two different site names (WP5). |
| F7 | **Session routes are capability URLs**: `/api/session/{sid}/*` is authorised by the UUID alone; `get_document` checks nothing. | `docs/STATUS.md:524-533`; `spikes/docserver/src/main.rs:146-160` | Session token on every call (brief task 5, §3.2). Session ids are kept out of logs (the spike prints them, `main.rs:132`, `:186`). |
| F8 | **All four web editors now have a `?host=parent` bridge** (wordcraft 0001, deckcraft 0002; gridcraft and pdfcraft upstream), and the shell already drives all four by their prefixes. **None has a read-only open or a save acknowledgement**, so a failed host upload still shows as saved. | `patches/REGISTER.md:15`, `:60`; `docs/phase4/PLAN.md:85` (F7); `web/shell/shell.js:2`, `:92-95`; message list `patches/wordcraft/0001-…patch:11-18`; `docs/STATUS.md:309-311` | WP6 adds two messages to the Docs bridge, as a carried patch: `open` with `readOnly`, and `saved {ok}` (dirty stays on until the server confirms). The deckcraft bridge (SIZING 5.5) is already done in Phase 4. |
| F9 | **Web builds install no office layer.** No snapshots, no XDFU stamp, and the web download branch bypasses `enabled`/`after`. | `docs/phase4/hooks-docs.md:17`, `:44-45`; `docs/STATUS.md:1188` | On the web, **the server is the enforcement point** for permissions and XDFU. In-editor gating (print, download) is best-effort UI and is said so. |
| F10 | **wordcraft's Restrict Editing (readOnly / comments / trackedChanges) is enforced in `Session::run`, but it writes `w:documentProtection` into the saved file.** | `upstream/wordcraft/crates/engine/src/cmd/protect.rs:1-8`, `:19`; `crates/docx/src/write/mod.rs:1027` | Not usable for session permissions: it would change the HRM's document. Read-only comes from the bridge flag (WP6). Review and comment-only modes are **partial** (WP7). |
| F11 | **The Phase 4 history store is path-keyed and its chain is unkeyed.** xdfu-core already has a keyed (HMAC) chain, which the converter opens. | `crates/suite-core/src/history.rs:12-17`, `:472-541`; `crates/xdfu-core/src/audit.rs:6-9`, `:239`; `services/convert-api/src/app.rs:416` | The server store adds an id-keyed entry point to `suite-core::history` and uses the keyed chain. The audit log reuses `AuditLog::open_keyed` as is. |
| F12 | **The services workspace links no engine; it runs the domain CLIs as processes.** | `services/Cargo.toml:1-2` | Diff engines are small binaries in the domain workspaces (`domains/docs`, `domains/sheets`), called as processes, like the converter calls its tools (W2 kept). |
| F13 | **Transliteration is already in the web Docs build** (patch 0120 touches `apps/wordcraft-web`). | `patches/wordcraft/0120-…patch:56-71` | The loss of HRB's ONLYOFFICE `uz-translit` plugin (OPEN-Q1, `hr-backend-prompt.md:885-886`) is covered. WP9 checks it in the embedded editor. |
| F14 | **HRB downloads the saved file inside the callback handler (30 s timeout) and then post-processes the docx; the spike waits only 10 s for the reply.** | `hr-backend-prompt.md:851`; `spikes/docserver/src/callback.rs:35` | Default callback timeout 60 s, configurable. A timeout is "unknown", not "failed": the editor stays dirty and the user is told (WP4). |
| F15 | **Only `_hrm_desc_` was ever round-tripped (4/4 synthetic files, CLI only)**, but the `files` kind has no Word↔DB sync, so a `files` pilot does not depend on it. | `docs/ARCHITECTURE-DECISION.md:452-455`; `docs/phase1/SIZING.md:223` | The recommended pilot (Q1) does not wait for the bookmark work; a decree pilot does (WP9). |
| F16 | **The no-HRM-names test covers `convert-api`, `suite-serve`, `web/shell` and `crates/` only.** | `docs/phase4/accounts.md:45-48` | WP16 adds `services/docserver` and `ci/web-smoke/docserver*` to its scan list. |
| F17 | **The spike defaults are unsafe for anything but a spike**: a built-in secret, read-only "not enforced" in session creation, a 100 MB cap. | `spikes/docserver/src/main.rs:46`, `:48`, `:112-114` | Nothing is carried as code. §8 says per file what is reused as design. |

---

## 1. Shape

```
HRM SPA (other site) ──<script src>──▶ /ds/api.js  (alias /web-apps/apps/api/documents/api.js)
   │ new DocsAPI.DocEditor(id, config)   config = HRM-signed JSON (token = HS256 over the config)
   ▼
 iframe ▶ /ds/editor  (wrapper page, our origin; frame-ancestors = the provider's SPA origins)
   │  POST /ds/session {config}  ─────────▶ verify token (provider secret) ─▶ fetch document.url (SSRF guard) ─▶ session on disk
   │  ◀── {sid, session token, editor, perms}
   │  iframe ▶ /docs/?host=parent  (the Phase 4 editor dist, same origin as the wrapper)
   │  postMessage bridge: open(bytes, readOnly) / dirty / save / saved
   │  POST /ds/session/{sid}/save  (Bearer session token) ─▶ new version ─▶ callback status 6 ─▶ HRM save-callback
   │  POST /ds/session/{sid}/close ─▶ callback status 2 or 4
   ▼
HRM api (Docker network) ──GET http://<internal listener>/ds/files/{sid}/{ver}?t=<file token>──▶ the saved bytes
```

- **One process, one origin.** The docserver is a new library crate `services/docserver`, mounted by `suite-serve`
  next to the shell, the editors and the converter. It is **off** unless the config has a `[docserver]` section (D1).
- **Local mode is untouched.** With no providers configured, no `/ds/` route exists and nothing asks for a login.
- **Standalone server mode (no HRM)** uses the same sessions with a provider whose issuer is any organisation's
  system. The server version store (§6) serves that case. The HRM is one `[[docserver.providers]]` entry in an
  operator's config file, and appears only as an example in docs.

---

## 2. Design decisions (options, recommendation, deciding trade-off)

| # | Question | Options | Recommendation | Why (the trade-off that decided) |
|---|---|---|---|---|
| D1 | Where the docserver lives | (a) library crate mounted by `suite-serve`; (b) its own binary on `convert-api`; (c) productionise the spike crate | **(a)** `services/docserver`, a member of the services workspace, mounted by `suite-serve` (`services/suite-serve/src/lib.rs:230-275`) | The editor dists, their CSP hashes (F8 of Phase 4), TLS, the trusted-proxy rules, connection limits and the header baseline already live there. A second binary would copy all of it. Cost: `suite-serve` grows; the converter routes stay mounted but keep their own JWT scope (`jwt.rs:91-94`). |
| D2 | Session state | in memory (spike, `session.rs:1-41`) / on disk | **On disk**: `<data_dir>/sessions/<sid>/` with `session.json` and the versions | An HRM session outlives a restart of our container; the close callback (status 2) must still be sent. In-memory loses the user's last save on restart. |
| D3 | Per-request auth inside the editor | cookie / capability URL / bearer token | **Bearer session token** in the wrapper page's memory, renewed while open | Cookies in a cross-site iframe are blocked or partitioned (F6). A capability URL is what the spike review rejected (F7). |
| D4 | Outbound HTTP client | `reqwest` (spike) / `ureq` (in tree, blocking, no TLS) / `hyper-util` client + `tokio-rustls` (both already in the services lock) | **`hyper-util` with our own connector**: resolve, check every address, connect to the checked address, TLS with rustls + ring | The SSRF guard needs control of resolution and connect. No new TLS stack and no `aws-lc-sys` (`services/convert-api/Cargo.toml` comment on rustls). Cost: about 150 more lines than `reqwest`. `cargo-deny` decides if a crate is needed after all. |
| D5 | Read-only in the editor | Restrict Editing (F10) / server-only / a bridge flag | **Bridge flag + server refusal** | Restrict Editing changes the HRM's file. Server-only would let a viewer type and then fail on save. |
| D6 | Version store in an HRM session | ours / the HRM's | **The HRM's, when configured** (HRB PR-4a/PR-7); ours in standalone server mode | One store of record per document. Two stores would disagree after a conflict. The gate needs neither (Q6). |
| D7 | Diff engines | in the server (links engines) / domain binaries as processes | **Domain binaries** (`docs-diff`, `sheets-diff`) | Keeps W2 and F12. Cost: one process start per diff (measured in WP12). |

---

## 3. Protocol

### 3.1 Routes

| Route | Listener | Auth | Notes |
|---|---|---|---|
| `GET /ds/api.js`, alias `GET /web-apps/apps/api/documents/api.js` | public | none | static, ours; `Cross-Origin-Resource-Policy: cross-origin`; short cache |
| `GET /ds/editor` | public | none | the wrapper page; `frame-ancestors 'self' <provider frame_origins>`; strict script CSP with hashes |
| `POST /ds/session` | public | **config token** (provider secret) | body `{config}`; rate-limited per client address; returns `{sid, token, editor, mode, perms, title}` |
| `POST /ds/session/{sid}/renew` | public | session token | a fresh session token; absolute cap = the config's `exp` or 12 h |
| `GET /ds/session/{sid}/document` | public | session token | the current version's bytes; the first call of a session fetches version 1 from the provider (WP4: 502 on a fetch failure, 413 over the cap, 415 for a bad package) |
| `POST /ds/session/{sid}/save` | public | session token, `edit` | body cap, zip and OOXML type check; new version; callback status 6 (`forcesavetype` 1); answers `{version, callback: ok\|failed\|unknown}` |
| `POST /ds/session/{sid}/dirty` | public | session token, `edit` | the editor's first change: callback status 1, once per session (WP4) |
| `POST /ds/session/{sid}/autosave` | public | session token, `edit` | recovery copy only, no callback (as the spike, `main.rs:190-200`) |
| `POST /ds/session/{sid}/close` | public | session token | callback status 2 (saved since open) or 4 (nothing saved); a failed status 2 goes to the retry queue |
| `GET /ds/files/{sid}/{ver}?t=` | **internal** (and public only if configured) | **file token** | what the HRM downloads after a callback (F4) |
| `GET /healthz` | both | none | unchanged |

Editor routing by `documentType` / `fileType`: `word`/docx → `/docs/`, `cell` → `/sheets/`, `slide` → `/slides/`,
`pdf` → `/pdf/`. Only docx is in the gate. The other three are **partial** until WP6's two bridge messages exist in
their bridges (§13.2 R6).

### 3.2 Tokens (brief task 5: JWT on every call)

| Token | Who signs | Key | Claims | Lifetime | Where it travels |
|---|---|---|---|---|---|
| Config token | the provider (HRM) | the provider's `secret_file` | the whole DocsAPI config | `exp` required (Q4) | `config.token`, POST body |
| Session token | us | `HMAC(server_secret, "session")` | `{purpose:"session", sid, sub, prov, perm, exp}` | 15 min, renewed | `Authorization: Bearer` from the wrapper page |
| File token | us | `HMAC(server_secret, "file")` | `{purpose:"file", sid, ver, sha256, exp}` | 10 min | the callback `url` query (the DocsAPI contract needs a fetchable URL) |
| Callback token | us | the provider's secret | `body.token = JWT(body)`, and `Authorization: Bearer JWT({payload: body})` | 10 min | the callback POST; HRB-compatible (`spikes/docserver/src/callback.rs:20-31`, `hrm-current-state.md:327-329`) |

- HS256 only, `alg` pinned (`jwt.rs:57-66`). Keys are at least 32 bytes. Each provider's secret must differ from
  `server_secret` and from every other provider's secret; the server refuses to start otherwise (the pattern of
  `services/convert-api/src/config.rs:363-415`).
- **Only the verified payload is trusted.** Unsigned outer fields (`events`, `width`, `height`, mobile `type`) are
  ignored (`hrm-current-state.md:312`).
- A session token for session A never reads session B: wrong sid → 404, the same answer as no session (no oracle).
- A file token is bound to `sid`, `ver` and the version's `sha256`.
- **`iss` is optional** (decision, 2026-10-25): HRB tokens carry none, and the provider is identified by its unique secret. When
  an `iss` is present it must equal the provider's configured issuer. Question for HRB in section 9: add `iss`?
- **Config-token replay within `exp` is accepted** (no one-time index): see section 4.

### 3.3 Callbacks

| Status | When we send it | HRB today (`hrm-current-state.md:284-285`) |
|---|---|---|
| 1 | once, on the first dirty event of a session | ignored (PR-5 renews a lease on it) |
| 2 | on close, when a save happened since open; `url` = the last version | saved |
| 4 | on close with nothing saved | ignored (PR-5 releases the lease) |
| 6 | on every explicit save and every host `requestSave` (`forcesavetype: 1`) | saved |
| 3, 7 | never: we have no server-side conversion step that can fail after a save was accepted; a failure is answered to the editor instead | ignored silently today |

- **Async-save ticket.** A save answers only after the callback reply. `{"error":0}` → `saved {ok:true}` to the editor.
  Anything else → `saved {ok:false}`, the document stays dirty, and the wrapper shows a plain message.
- **Retry (as built, WP4).** A status 2 is written to `<data_dir>/queue/<sid>.json` BEFORE its first try and removed when the
  provider answers `{"error":0}`. A failed try is retried after 1, 5, 15 and 60 min, then hourly, until 24 h after the close;
  then the entry is marked `gave_up` and the version stays on disk. A background thread (15 s tick, `spawn_retry_loop`) runs
  the due entries, also after a restart. Each try is a log line (host and path) and, with WP8, an audit line. `{"error":0}`
  is the only success: HTTP errors, `{"error":1}`, redirects and refused addresses are "failed"; only a timeout (default 60 s,
  `callback_timeout_s`) is "unknown". The record keeps `acked` (the highest acknowledged version); a version above it is dirty. Status 6 is not retried by the server: the user still has the dirty document and saves again.
- **Body and tokens (as built).** `{key, status, users:[user.id], url, forcesavetype}`; `url` (status 2 and 6) is
  `<internal_url>/ds/files/<sid>/<ver>?t=<file token>`, `internal_url` defaulting to the internal listener's address. `body.token`
  is a JWT of the body and `Authorization: Bearer` a JWT of `{"payload": body}`, both with the provider secret and `exp` 10 min.
- **Logging.** Host and path only (the fetch logs the host only: some providers put their file token in the path); never the query, a token or bytes (HRB rule `hr-backend-prompt.md:113-115`).

---

## 4. Security design (the checklist in §10 tests each item)

- **SSRF (F3, D4).** Per provider: `fetch_allow` and `callback_allow`, each a list of exact `scheme://host:port`. A
  bare `host` is not accepted (the port is explicit). An entry may carry `internal = true`; only then may it resolve to
  a private, loopback, link-local (169.254.0.0/16, fe80::/10), CGNAT, unique-local or IPv4-mapped address. Metadata
  addresses (169.254.169.254, fd00:ec2::254) are refused even for an `internal` entry. No userinfo, no redirects, a
  connect timeout (5 s) and a total timeout. Ranges reuse `convert_api::config::Cidr` (`config.rs:157-200`).
- **Deny ranges (as built, `outbound.rs`).** The name is resolved once by our own resolver; if ANY answer is denied the whole name
  is refused, and the connection goes to the checked addresses only. Allowed only for an `internal = true` entry: 127/8, 10/8,
  172.16/12, 192.168/16, 169.254/16 (except the metadata address), 100.64/10, `::1`, fc00::/7, fe80::/10. Never, even then:
  169.254.169.254 and fd00:ec2::254, 0/8, `::`, `::/96`, multicast, 240/4 and 255.255.255.255, 192.0.0/24, 192.0.2/24,
  198.18/15, 198.51.100/24, 203.0.113/24, fec0::/10, 2002::/16 (6to4), 64:ff9b::/32 (NAT64), Teredo, 2001:db8::/32.
  IPv4-mapped addresses are judged as their IPv4 address. The outbound client is plain `http` only for now (no TLS feature in
  the workspace's ureq); an `https` entry is refused at request time (open question, section 9).
- **Config-token replay.** A config token may be presented again within its `exp` and opens another session. No one-time index:
  the token already grants access to that document for its whole life, and `exp` is at most 12 h, which bounds it (decision,
  2026-10-25).
- **Encryption at rest and XDFU stamping (WP14, as built).** `encrypt_at_rest = true` with `encrypt_key_file` (32 random bytes or more, kept
  outside `data_dir`, refused if equal to `server_secret_file`, `audit_key_file` or any provider secret) seals `session.json`, `v<N>.bin`,
  `autosave.bin` and the history objects (`history/objects/<sha>`) with AES-256-GCM (`services/docserver/src/atrest.rs`; the AEAD
  `convert-api` already uses). Format: `"DSAR1"`, a random 12-byte nonce, ciphertext and tag; the authenticated header is the magic plus
  the file's name under `data_dir`, so a file moved over another fails. A file that does not open (wrong key, damaged, plaintext) is
  refused, never served as it is. `at-rest.check` (sealed text) in `data_dir` is checked at start: a wrong key, encryption switched off
  over encrypted data, or switched on over plaintext data stops the start. Not sealed, and holding no document bytes: the history index
  (`history/docs/*.jsonl`: versions, authors, times), the retry queue and the audit logs. **Threat model:** this protects the data folder
  against theft of the disk or the volume, and against a copy or a backup of it. It does not protect against someone who is root on
  the running host (the key is in the server's memory and the key file must be readable by it), and not against anyone who can use
  the server's own routes with a valid session token. `xdfu = "watermark"` (default `"off"`: nothing is built, registered or
  stamped; `"refuse"` is a converter-server option only) needs `encrypt_at_rest` and `audit_key_file`. A copy that leaves the
  server (a history version downloaded with the `download` permission, or the saved file a callback hands to the provider) of a
  docx, xlsx or pptx that carries the grif gets the cl. 12 stamp line (copy number, the requester's network address, the session's
  user name, UTC time) through the converter's stamping, and one line in the keyed chain `<data_dir>/xdfu-audit.jsonl` whose `seq`
  is the copy number. A document without the grif is served as it is and logs nothing. A copy that cannot be checked or stamped
  (a pdf, a damaged package) or logged is refused. The editor's own load (`/document`) is the working copy, not a copy, and is
  not stamped. The sidecar fallback of the PDF to DOCX guard (gate review m5) is the operator's switch `pdf_fallback` (API config) or
  `CONVERT_PDF_FALLBACK=1` (CLI), off by default, and never used for a job that carries the grif or cannot be checked for it.
- **Size limits.** Fetch cap `max_document_mb` (default 50, the HRB cap `hr-backend-prompt.md:342`, and phase3 O3), by
  `Content-Length` first, then by counted bytes. The same cap on save and autosave bodies. Zip magic plus the
  converter's central-directory check against zip bombs (phase3 PLAN §5.5, `docs/phase3/PLAN.md:448-450`), and the
  OOXML main part must match `fileType`.
- **Rate limits.** Session creation and failed auth per client address, through the converter's limiter
  (`services/convert-api/src/routes.rs:295-318`).
- **WP8 as built.** Audit file `<data_dir>/audit.jsonl` (`audit_file`, `audit_key_file`, `redact`); an existing log that does not verify stops the start; no key = a warning. Sessions and document keys appear as a 16-hex digest (`session`, `key_ref`), never the id. Events: `session.create`, `document.fetch` (also failed/refused), `save`, `callback` (every try, retries included), `close`, `auth.fail`, `ssrf.block` (fetch and callback), `limit.hit`. Limits per client address (the guard layer's key): `rate_create_per_min` 30, `rate_auth_fail_per_min` 20; only failures count, over the limit the answer is 429 and one `limit.hit` line per client and kind per minute is written (a flood does not fill the log). A failed audit write is said on stderr and does not fail the request.
- **Audit log.** One keyed chain (`AuditLog::open_keyed`, F11) with: `session.create` (provider, `user.id`, a hash of
  `document.key`, mode and permissions, client address), `document.fetch` (host, size, sha256), `save` (version,
  sha256, size), `callback` (status, result, host+path), `close`, `auth.fail`, `ssrf.block`, `limit.hit`. Never tokens,
  query strings, document bytes, and file names only when not `redact`.
- **Headers (F5).** As §0 F5. `postMessage`: `api.js` answers only its own origin and its own iframe window (as
  `api.js:18`); the wrapper accepts the config only from `window.parent` on a configured frame origin.
- **TLS.** Built-in rustls, or `tls = "upstream"` behind the trusted Traefik (`docs/phase4/tls.md`). The internal
  listener is plain HTTP on the Docker network, bound to its interface, CIDR-restricted, and serves the file route only.
- **Retention.** A session's copies are deleted 24 h after a successful close, and 7 days after the last activity of an
  abandoned session (Q10). Encrypted at rest with an operator key when `encrypt_at_rest = true` or `xdfu != "off"`
  (reuses `services/convert-api/src/crypt.rs`).

---

## 5. Permissions (brief task 1)

| DocsAPI input | Server (authoritative) | Editor (best effort on the web, F9) | Gate | Status |
|---|---|---|---|---|
| `editorConfig.mode = "view"` or `permissions.edit = false` | save and autosave 403; no callbacks | opened with `readOnly` (WP6) | yes | supported (target) |
| config without `callbackUrl` | accepted only as view | read-only | no | supported (target); HRB keeps these on ONLYOFFICE until this ships (`hr-backend-prompt.md:503-504`) |
| `permissions.download = false` | no server download route exists to refuse | "Save a copy" / export refused through the bridge flag | no | partial |
| `permissions.print = false` | — | print refused through the bridge flag | no | partial |
| `permissions.review = true` with `edit = false` | as edit | Docs starts with tracked changes on and the toggle refused; needs a wasm-safe policy layer (F9) | no | partial, after the gate |
| `permissions.comment = true` with `edit = false` | as view | — | no | not supported in Phase 5 |
| `mode = "edit"`, `edit = true` | full | full | yes | supported (target) |

HRB sends `review:false` and `comment = edit` (`hrm-current-state.md:257`), so the pilot needs only the two
supported rows.

---

## 6. Version history, the server part, and diffs (owner requirement, `STATUS.md:9-30`)

- **Store.** `suite-core::history` gains an id-keyed API next to the path one: `snapshot_bytes(id, bytes, author,
  message)`, `list_id`, `read_id`, `restore_id`, `name_id`. The server opens it under `<data_dir>/history/` with the
  keyed chain (F11). Author = the token `sub`. Two Phase 4 leftovers are fixed here because the server is unattended:
  a torn last index line is repaired before an append (`docs/phase4/gate-review.md:48`), and `.tmp` leftovers in
  `objects/` are swept (`:147`).
- **Backend interface.** `trait VersionBackend { list, read, restore, name }` with two implementations: ours, and the
  HRM's `/office/documents/{id}/versions` (`hrm-integration-design.md:269`) once HRB PR-4a and PR-7 exist (D6). Until
  then the HRM implementation is a stub that answers "not available".
- **Routes** (server mode, the converter's JWT auth, `routes.rs:270-318`): list, view (opens read-only), compare,
  restore (always a new version), download, name.
- **Diffs (D7).** `docs-diff a.docx b.docx --json`: paragraph and run level, with the author per changed paragraph
  from `w:ins`/`w:del` where present ("blame"). `sheets-diff`: changed and added cells, sheets and formulas. Slides and
  PDF wait for Q7.
- **Not in Phase 5:** branches or merges of documents; live attribution (Phase 9).

---

## 7. Deployment on the test environment (WP10, WP17)

- **Image:** a Linux build of `suite-serve` (with the docserver) plus the four editor dists, built on this PC. Docker
  inside this PC goes through a TLS-intercepting proxy (`CLAUDE.md` local notes), so dependencies are vendored on the
  host (`cargo vendor`) and the build runs offline in the container.
[redacted: HRM security detail]
  compose project name and path, `hr-backend-prompt.md:85-88`): the container joins `backend_net`
  (`hrm-current-state.md:229-230`); the public route is a Traefik rule with TLS (`tls = "upstream"`, `trusted_proxies`
  = Traefik's address); the internal listener is reachable from `api` only.
- **Provider entry (operator config, never in code):** `issuer`, `secret_file` (= HRB's `CRAFT_DOCSERVER_JWT_SECRET`,
  generated on the box), `fetch_allow = ["http://api:8000"]` with `internal = true`, `callback_allow` the same,
  `frame_origins` = the SPA origin.
- **HRB env** (its PR-3, `hr-backend-prompt.md:340-353`): `OFFICE_CRAFT_ENABLED`, the system setting
  `office.craft_enabled`, `OFFICE_CRAFT_PILOT_USER_IDS`, `OFFICE_CRAFT_KINDS=file`, `OFFICE_CRAFT_FILE_TYPES=docx`,
  `OFFICE_EDITOR_PUBLIC_URL_CRAFT`, `CRAFT_CALLBACK_ALLOWED_HOSTS`, `CRAFT_DOCSERVER_INTERNAL_URL` (= our internal
  listener).
- **Internal listener:** never publish `internal_listen` with Docker `-p`; bind it to the Docker network address and allow only the HRB container's subnet or IP in `internal_allow`, not a /16 that contains the bridge gateway.
- **Fallback:** `office.craft_enabled=false` takes effect on the next config request with no restart; sessions already
  open keep saving (`hr-backend-prompt.md:526-527`).
- **Pilot documents are HRM data:** handled like `corpus/` (`docs/ARCHITECTURE-DECISION.md:430-432`); no document
  content in logs, issues, commits or reports.

---

## 8. What the spike gives (design reused, code not copied without review)

| Spike file | Phase 5 use |
|---|---|
| `src/jwt.rs` | **Superseded**: the converter's port (`services/convert-api/src/jwt.rs:1-3`, `exp` required, `nbf`, leeway, 9 tests) is reused. |
| `src/config.rs` | Design reference for the DocsAPI subset. Rewritten with validation: `key` charset `[0-9A-Za-z._=-]`, at most 128 characters (DocsAPI public docs); `fileType` allowlist; lengths. |
| `src/fetch.rs` | Design reference. Rewritten per §4 (per-provider lists, resolver check, pinned connect). Its test cases are kept and extended. |
| `src/session.rs` | Dropped (in memory; D2). |
| `src/callback.rs` | The body shape and the two tokens are kept (HRB-compatible). Rewritten with the timeout, the retry queue and host+path logging. |
| `src/main.rs` | The route list is the starting point. Not carried: capability-URL auth (F7), the default secret, the shared allowlist, sid logging (F17). |
| `src/bin/fake_hrm.rs` | Basis of the WP1 contract harness. |
| `static/api.js` | Basis of the WP5 `api.js` (our own code). |
| `static/editor.html` | The **shim adapter is dropped** (relies on wordcraft internals, `docserver-results.md:58`); the bridge adapter is the basis of the wrapper page. |
| `hrm/index.html`, `e2e/` | Basis of the cross-site test page and the Playwright spec pattern. |

---

## 9. Open questions left by the Phase 0 analysis (owner confirmed the server unchanged, 2026-10-25)

These were **unanswered or UNVERIFIED in Phase 0 itself**. Nothing here is about drift. We have no server access, so
each is answered by the owner, the HRM team, or WP17 on the box with the owner present.

| # | Question | Phase 0 source | Needed by |
|---|---|---|---|
[redacted: HRM security detail]
| H2 | Is `BACKEND_DOCS_SECRET` set and distinct on the **main** server? (PR-2 refuses to start otherwise.) | OPEN-Q3 `hr-backend-prompt.md:889-890` | HRB PR-2 on main (not our pilot) |
[redacted: HRM security detail]
| H4 | Test hostnames: our public host, the office web (BFF) host, the mobile app-link domain; is the custom scheme acceptable? | OPEN-Q5 `hr-backend-prompt.md:893-894`; design Q7 `hrm-integration-design.md:386-387` | WP17 |
| H5 | Are the client changes approved: `editor=auto` + `editorUrl` in hr-frontend-v2 (and v1), and later the `/oauth/consent` page? | OPEN-Q4 `hr-backend-prompt.md:891-892` | WP17 |
| H6 | Two users editing the same file today: does the later save really overwrite (new key per open)? | UNVERIFIED `hrm-current-state.md:357-360` | the "no regression" claim of the pilot |
[redacted: HRM security detail]
| H8 | Version retention (N versions or N days) and whether deleting a file purges its versions. | OPEN-Q7 `hr-backend-prompt.md:898-899`; design Q8 | HRB PR-4a, WP11 |
| H9 | Decree conflict policy and exclusive embedded editing. | OPEN-Q8 `hr-backend-prompt.md:900-902` | a decree pilot only |
| H10 | Which document is authoritative after an editor save of a decree (Word or the DB field)? | design Q5 `hrm-integration-design.md:382-383` | a decree pilot only |
| H11 | Is mobile editing allowed for HR documents once ONLYOFFICE is replaced (today forced to view)? | design Q9 `hrm-integration-design.md:389` | Phase 6 |
| H12 | One HRM-generated decree, letter and report (anonymised, into `corpus/`) for the bookmark test. | `SIZING.md:277` | WP9 second half |
| H13 | Can HRB add an `iss` claim to its config tokens? (We accept tokens without one; the secret names the provider.) | WP4 decision, PLAN 3.2 | optional |
| H14 | Is the document server reached over plain `http` on the Docker network (as today), or does any fetch or callback need `https`? The outbound client has no TLS yet. | WP4 | WP10 |

---

## 10. Security checklist (the exit gate runs every row; each names its test)

| # | Item | Test / evidence |
|---|---|---|
| S1 | Every `/ds/session*` call needs a valid token: the config token on create, the session token on every other call | `tests/auth.rs`: each route without, with a tampered, an expired and another session's token → 401 / 404 |
| S2 | HS256 pinned: `alg: none`, `HS512`, an RS/HS confusion header are refused | unit tests on `jwt::verify` (exist, `jwt.rs:162`) + docserver route tests |
| S3 | Config tokens without `exp` are refused (or the Q4 alternative is recorded); lifetime ≤ 12 h | `config_token_without_exp_is_refused` |
| S4 | Secrets ≥ 32 bytes, each provider's secret distinct from `server_secret` and from the others; start refused otherwise, with the variable named and no value printed | `config.rs` tests in the docserver crate |
| S5 | Only the signed payload is used; changing an unsigned outer field changes nothing | `outer_fields_are_ignored` |
| S6 | SSRF: scheme, userinfo, exact host:port, no redirect, connect and total timeouts | `fetch` tests: 302 not followed, `http://user@host`, `ftp:`, wrong port, slow body |
| S7 | SSRF: names resolving to private, loopback, link-local, CGNAT, IPv4-mapped IPv6 or metadata addresses are refused unless the entry is `internal`; metadata always | resolver-stub tests: `169.254.169.254`, `[::ffff:127.0.0.1]`, `2130706433`, a name that resolves to `10.0.0.5`, a rebinding stub (second answer differs) |
| S8 | Separate fetch and callback allowlists per provider | a callback URL that is only in the fetch list → refused |
| S9 | Size: fetch cap by header and by count, the same cap on save/autosave, zip-bomb check, OOXML type matches `fileType` | tests with a 51 MB body, a lying `Content-Length`, a 1 KB zip that inflates to 2 GB, a renamed xlsx as docx |
| S10 | View sessions cannot save or autosave and send no callback | `view_session_cannot_save` (403), the fake HRM sees no callback |
| S11 | File token bound to purpose, sid, ver and sha256; 10 min | tests: another ver, another sid, a session token used as a file token |
| S12 | Internal listener serves only the file route, only to configured CIDRs | `internal_listener_serves_files_only` |
| S13 | Headers: `frame-ancestors` lists only configured origins on the wrapper and editor routes; CORP cross-origin only on `api.js`; no CORS header anywhere; nosniff; the strict CSP elsewhere unchanged | header tests + `ci/web-smoke/docserver.spec.ts` framing from an unlisted origin fails |
| S14 | `postMessage`: `api.js` and the wrapper accept only their expected origin and window | Playwright: a foreign frame posting `docs:config` / bridge messages is ignored |
| S15 | Rate limits on session creation and failed auth | 429 after the configured count |
| S16 | Audit log: keyed chain, the §4 events, verifiable | `AuditLog::verify_keyed` on a run; a rewritten line fails |
| S17 | No token, query string, session id, file bytes or (with `redact`) file name in logs or audit | a grep test over the logs of a full e2e run |
| S18 | Retention: copies deleted after close + 24 h, abandoned after 7 days; encryption at rest when configured | clock-injected sweep test; `encrypt_at_rest` store holds no readable docx |
| S19 | TLS: builtin or upstream with trusted proxies; HSTS; plain HTTP redirected | the existing `tls.spec.ts` pattern, extended to `/ds/editor` |
| S20 | Local mode: no `/ds/` route without providers; no login anywhere | `no_providers_no_docserver_routes` |
| S21 | No HRM names in our code | `no_hrm_names_in_code` with `services/docserver` added (F16) |
| S22 | Licences and advisories | `cargo deny` with `ci/deny/services.toml`; `cargo audit` clean or each item justified |
| S23 | Callback failures are never silent: a failed or timed-out callback leaves the document dirty and tells the user | e2e with the fake HRM answering 500, `{"error":1}`, and sleeping 70 s |
| S24 | Every by-id route answers 404 for "not yours" (no existence oracle) | `tests/auth.rs` |
| S25 | HRM side (owner/HRM team, not our tests): HRB PR-1 and PR-2 merged and deployed on `<test-host>` before the first connection | the HRB PR descriptions; owner confirmation |

---

## 11. Work packages

Each WP lists scope, done-check (the measurement or test), estimate (stream-days, low/likely/high) and tier. Paths are
in this repo.

### WP0. Kick-off and merge-forward
- **Scope:** record the merge-forward rule in this plan's `docs/STATUS.md` section (`git merge phase-4` into `phase-5`
  after every Phase 4 fix; never the reverse); baseline `scripts/ci.sh quick` and the services tests on `phase-5`;
  send §9 and §13.4 to the owner.
- **Done-check:** baseline numbers in `docs/STATUS.md`; a first merge-forward commit, if Phase 4 moved.
- **Estimate:** 0.5 (0.5/0.5/1). **Tier:** builder.

### WP1. HRB contract harness (fake HRM)
- **Scope:** a test binary in `services/docserver/tests/support/` built from `spikes/docserver/src/bin/fake_hrm.rs`
  and made faithful to HRB as surveyed: the config shape of `hrm-current-state.md` §4.2 (with and without `exp`), the
  file route with a purpose-bound token, the callback rules of §4.3 (body token; key prefix `file_{id}_`; rejects
  `type`, `editorConfig`, `documentType`; host rewrite to the configured internal base; 30 s download; `{"error":0|1}`),
  plus PR-1's Bearer fallback. A cross-site host page (`hrm/index.html` pattern) served on a second site name.
- **Done-check:** the harness's own tests; the spike's S6/S7 cases reproduced against the harness rules.
- **Estimate:** 1.5 (1/1.5/2). **Tier:** builder.

### WP2. Docserver crate, config, mounting
- **Scope:** `services/docserver` (member of `services/Cargo.toml`), `forbid(unsafe_code)`, MIT OR Apache-2.0.
  `[docserver]` config: `server_secret_file`, `data_dir`, `max_document_mb`, `callback_timeout_s`, `internal_listen`,
  `internal_allow` (CIDRs), `encrypt_at_rest`, and `[[docserver.providers]]` (`name`, `issuer`, `secret_file`,
  `fetch_allow`, `callback_allow`, `frame_origins`, `require_exp`). Refuse-to-start rules (S4, S20). Mounted by
  `suite-serve` when the section exists; the internal listener (F4); the header overrides (F5).
- **Done-check:** config unit tests (each refusal), `no_providers_no_docserver_routes`, the header tests of S13.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder; the header-rule change gets an architect review.

### WP3. Sessions and tokens
- **Scope:** `POST /ds/session` (config token, payload-only, validation of §8 `config.rs` row), the on-disk session
  (D2), the session and file tokens of §3.2, renew, document, save, autosave, close; 404 for any other session.
- **Done-check:** `tests/auth.rs` (S1, S3, S5, S10, S11, S24); a restart test (create, save, restart, close → status 2).
- **Estimate:** 2.5 (2/2.5/4). **Tier:** builder.

### WP4. Outbound: SSRF-guarded fetch and the callback client
- **Scope:** D4 client; §4 SSRF rules; the §3.3 statuses, the async-save ticket, the status-2 retry queue on disk, the
  timeout (F14); host+path logging.
- **Done-check:** S6-S9, S23 against WP1; measured: callback round trip on loopback (median of 20).
- **Estimate:** 2.5 (2/2.5/3.5). **Tier:** architect for the resolver/connector, builder for the rest.

### WP5. Embed API: `api.js`, the wrapper page, framing
- **Scope:** clean-room `api.js`: `new DocsAPI.DocEditor(id, config)`, `destroyEditor()`, events `onAppReady`,
  `onDocumentReady`, `onDocumentStateChange`, `onError {errorCode, errorDescription}`, `onWarning`, `onRequestClose`;
  non-standard `requestSave()` / `close()` kept from the spike. The alias path. The wrapper page with the bridge
  adapter (the shell's prefix table, `web/shell/shell.js:92-95`), the session token in memory, renew, plain messages
  through the shell catalog (`i18n/shell.tsv`; Uzbek rows `reviewed:agent-<model>:<date>`).
- **Done-check:** `ci/web-smoke/docserver.spec.ts`: host page and docserver on **two different site names**
  (Chromium `--host-resolver-rules`) over TLS; open → type → save → callback received → close → status 2; S13, S14;
  3 runs in a row green. Measured: page load → `onDocumentReady`, save → callback (as `docserver-results.md:37-47`).
- **Estimate:** 2.5 (2/2.5/3.5). **Tier:** builder.

### WP6. Docs bridge: read-only open and save acknowledgement (carried patch)
- **Scope:** `patches/wordcraft/0127-…` (next free number): `wordcraft:open {readOnly}` and `wordcraft:saved {ok}`
  (dirty stays until `ok`); a periodic autosave while dirty (ADR :457) handed to the host as the 0005 lifecycle copy
  is. REGISTER row; `verify-queue.sh wordcraft`; `cargo xt register check`. wasm size against `web/budgets.toml`.
- **Done-check:** the bridge spec cases (read-only refuses typing; a failed save keeps the dirty mark); size delta
  inside `delta_br_max`.
- **Estimate:** 1.5 (1/1.5/2.5). **Tier:** builder.

### WP7. Permissions
- **Scope:** §5: the two supported rows end to end; the download/print bridge flags (partial); configs without
  `callbackUrl` as view. Review/comment rows stay documented as partial / not supported.
- **Done-check:** a test per supported row (server) and a Playwright case per row (editor); the compat file updated.
- **Estimate:** 1.5 (1/1.5/3). **Tier:** builder.

### WP8. Audit log and rate limits
- **Scope:** §4 audit events on `AuditLog::open_keyed`; the limiter on create and auth failures; the log grep (S17).
- **Done-check:** S15, S16, S17.
- **Estimate:** 1 (0.5/1/1.5). **Tier:** builder.

### WP9. `_hrm_*` bookmarks and translit in the embed
- **Scope:** synthetic decree, letter and report docx with every `_hrm_*` name of `hr-backend-prompt.md:119-122`,
  including zero-length bookmark paragraphs, generated by a script (no corpus content). Round trip through the CLI and
  through the web editor via the docserver (open, edit elsewhere, save). Then the real HRM samples (H12). Check that
  Latin ⇄ Cyrillic works in the embedded editor (F13).
- **Done-check:** a test asserting every bookmark name survives with its range; a Playwright case running the translit
  command in the embed. A loss → a carried fix, sized then.
- **Estimate:** 1.5 (1/1.5/4). **Tier:** builder; architect if a loss is found.

### WP10. Deployment package and the local rehearsal
- **Scope:** §7 image; an example operator config in `docs/phase5/deploy.md` (the HRM only as an example there);
  `docker compose` on this PC with WP1's harness on a network named like `backend_net`, Traefik in front, the internal
  listener used by the harness.
- **Done-check:** open → edit → save → reopen → download 3× green against the harness in containers; the image's
  `cargo deny` and size recorded.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP11. Server version store
- **Scope:** §6 store, routes and backend interface; retention (H8 defaults: last 50 or 90 days, as Phase 4
  `PLAN.md:308-311`); torn-line repair and `.tmp` sweep.
- **Done-check:** the Phase 4 history tests run against the id API; new `restore_is_a_new_version_server`,
  `torn_index_is_repaired`, `keyed_chain_rewrite_fails`.
- **Estimate:** 3 (2/3/4). **Tier:** builder.

### WP12. Diff engines
- **Scope:** `docs-diff` (paragraph and run, blame from tracked-change authors) in `domains/docs`; `sheets-diff` in
  `domains/sheets`; the compare route; a History panel "Compare" view. Slides and PDF per Q7.
- **Done-check:** golden diffs on synthetic pairs (insert, delete, move, format-only, a formula change); measured: diff
  time on the 108-page docx and on the w40 workbook.
- **Estimate:** 4 (3/4/6). **Tier:** architect for the Docs alignment, builder for Sheets.

### WP13. SSO and desktop sign-in (`kind = "oidc"`)
- **Scope:** generic, any provider: the server-side relying party (`kind = "oidc"` stops being refused,
  `services/convert-api/src/config.rs:291-298`), discovery and JWKS; the desktop client (system browser, loopback
  redirect on 127.0.0.1, PKCE S256, `keyring` for Windows Credential Manager, single-flight refresh;
  `hrm-integration-design.md` §1.3-1.5); "Open from / Save to server" with ETag/If-Match. HRB has no OAuth server until
  its PR-6/PR-7, so this is tested against a local test IdP. Signature algorithms: ES256 first (`p256`, permissive);
  RS256 only after the `rsa` advisory is checked for verify-only use.
- **Done-check:** a test IdP flow end to end; the state/PKCE negative tests of `hrm-integration-design.md:337-341`.
- **Estimate:** 7 (5/7/10). **Tier:** architect (design), builder (UI). **Not a gate item** (Q8).

### WP14. XDFU on the server (off by default)
- **Scope:** server stamping of a download or callback copy of a grif document (xdfu-core `stamp_text`, the keyed
  issuer), at-rest encryption, the phase 3 m5 remainder (the sidecar fallback as an operator switch, never for an XDFU
  job, `docs/phase3/gate-review.md:422`). EDS/E-IMZO, the registry and the grif-removal workflow stay design-only (Q9).
- **Done-check:** `xdfu_off_registers_nothing` for the docserver; stamped-copy tests as in gridcraft/0143.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP15. Phase 4 carry-forward
- **Scope:** `docs/phase4/gate-review.md:476-478`: F7, F11-F17, F20, F21 (F10 is already fixed, `STATUS.md:1193`),
  m10, the `GATE-FLOW` scripts into `ci/`. Any item fixed on `phase-4` meanwhile arrives by merge-forward and is ticked.
- **Done-check:** the per-item tests named in the gate review.
- **Estimate:** 2 (1.5/2/3). **Tier:** builder.

### WP16. CI, docs, compat file
- **Scope:** `scripts/ci.sh docserver` (unit, auth, fetch, the e2e with WP1); `docserver.spec.ts` into `web-smoke`;
  `no_hrm_names` scan list (F16); `ci/deny/services.toml` for new crates; keep `docs/phase5/onlyoffice-compat.md`
  current, and re-check every field name in it against the public DocsAPI pages.
- **Done-check:** `ci.sh docserver` exits 0 from a clean checkout.
- **Estimate:** 1 (0.5/1/1.5). **Tier:** builder.

### WP17. HRM pilot on `<test-host>` (owner-gated)
- **Prerequisites:** Q1, Q2, Q3 answered; HRB PR-0 … PR-3 merged and deployed on the test stack; H1, H4, H5 answered;
  the owner's explicit deploy OK and presence.
- **Scope:** §7 on the box; pilot users; the gate flow with a `files` docx.
- **Done-check:** open → edit → save → reopen → download, **3× green**, with a screen recording, timings and an audit
  excerpt (no content); flip `office.craft_enabled` off → the next open is ONLYOFFICE with no restart; H6 checked.
- **Estimate:** 3 (2/3/4). **Tier:** builder with the owner.

### WP18. Gate: security checklist, adversarial review, report
- **Scope:** run §10; architect review of the docserver as an attacker (token confusion, SSRF bypasses, header
  weakening, data left on disk); the Uzbek gate report.
- **Done-check:** every S-row green or an owner-accepted exception; review findings fixed or carried.
- **Estimate:** 1.5 (1/1.5/2). **Tier:** architect.

---

## 12. Order, parallel streams, and what starts now

**Implementation starts right after this plan's commit** (owner, 2026-10-25), on the assumed pilot flow. Order of start:
1. **WP0** (half a day), then at once **WP2** (stream A) and **WP1** (stream B): no owner input, no HRM access, no Phase 4
   dependency.
2. **WP3 → WP4** as soon as WP2's config lands; **WP5** as soon as WP3 answers sessions.
3. In parallel, beside the gate: **WP11**, **WP9** (synthetic half), **WP16**, **WP15** (items not touched on `phase-4`).
4. **WP17** waits only on the owner (deploy OK, H1/H4/H5) and HRB PR-0 … PR-3.

| Stream | Sequence | Waits on |
|---|---|---|
| A, server core | WP2 → WP3 → WP4 → WP8 | nothing |
| B, embed | WP1 → WP5 → WP6 → WP7 | WP3 for WP5's e2e |
| C, evidence and ops | WP9 (synthetic) → WP10 → WP17 → WP18 | WP4-WP7 for WP10; **owner + HRB PR-0…PR-3** for WP17 |
| D, history | WP11 → WP12 | Q7 for the Slides/PDF part only |
| E, beside the gate | WP13, WP14, WP15, WP16 | Q8, Q9 |

- **Critical path to the gate:** WP2 → WP3 → WP4 → WP5 → WP7 → WP10 → WP17 → WP18, about 17.5 stream-days of ours,
  **and in parallel the HRB stream PR-0 … PR-3 (6.25 d likely, not started)**. The later of the two sets the date.
- **Merge-forward:** WP6 and WP15 touch the same patch queue as Phase 4. Merge `phase-4` before starting either, and
  run `verify-queue.sh` after each merge.

---

## 13. Estimate, risks, gate mapping, owner decisions

### 13.1 Estimate (stream-days: low / likely / high)

| Group | WPs | Low | Likely | High |
|---|---|---|---|---|
| Gate path | WP0-WP10, WP16-WP18 | 17.5 | 24.5 | 38.5 |
| Beside the gate | WP11-WP15 | 13 | 18 | 26 |
| **Total (ours)** | | **30.5** | **42.5** | **64.5** |

Against `SIZING.md:249` (35.5 likely): +7, because the server history and diffs were added after SIZING
(`STATUS.md:30-31`, +8–12 minus the 3.5 spent in Phase 4), the cross-site framing and internal listener (F4, F5) were
unknown, and the deckcraft bridge (−3) is already done. Risk buffer as SIZING: 3 d (gate path) + 7 d (the rest).

### 13.2 Risks

| # | Risk | Effect | Mitigation |
|---|---|---|---|
| **R1** | **The gate's critical path is outside our repo**: HRB PR-0 … PR-3, the secret actions (H1) and a deploy with the owner present. None has started (server unchanged since Phase 0). | The gate cannot close on our work alone. | Start the HRB agent now (Q3). WP1 makes our side complete against HRB's documented rules, so WP17 is a deploy, not a debug session. |
| **R2** | **Cross-site embedding breaks** under our header baseline or browser policy (frame-ancestors chain, CORP, partitioned storage, WebGL in a nested iframe). The spike ran on one site and could not show it. | The editor is blank inside the HRM page. | F5/F6 designs; WP5 tests on two site names over TLS from day one; S13/S14. |
| **R3** | **Silent loss on save**: a failed or slow callback shown as saved (`STATUS.md:311`), HRB's 30 s download (F14), or HRB rewriting our URL to the wrong internal host (`hr-backend-prompt.md:519-522`). | Lost edits in HRM documents. | Async-save ticket + dirty until `ok` (WP4, WP6), retry queue for status 2, S23, and WP1 reproducing the HRB rewrite. |
| R4 | `_hrm_*` bookmarks lost on save | Decrees and letters regenerate and lose formatting | Not on the `files` pilot path (F15); WP9 before any decree pilot. |
| R5 | Merge-forward conflicts with Phase 4 fixes in `patches/wordcraft` | Rework | WP0 rule; merge before WP6/WP15. |
| R6 | Sheets, Slides and PDF bridges lack the two new messages | Those types stay on ONLYOFFICE (`OFFICE_CRAFT_FILE_TYPES=docx`) | Stated as partial; the same patch per bridge after the gate (about 1 d each, after WP6 measures the Docs one). |
| R7 | Office PCs' browsers and GPUs: software WebGL timings (`docserver-results.md:47`) are the only numbers | Slow open in the pilot | WP17 records load → ready on a pilot PC. |

### 13.3 Exit-gate items → proving WP

| Gate item / brief task | Proof | WP |
|---|---|---|
| One real HRM flow end to end (open → edit → save → reopen → download) | 3× green on `<test-host>`, recording, timings, audit excerpt; rehearsed in containers first | WP10, WP17 |
| The security checklist passes | §10 S1-S25 | WP18 |
| Task 1: sessions, signed config, open by URL, callback save, autosave, forced save | `tests/auth.rs`, restart test, callback statuses against WP1 | WP3, WP4 |
| Task 1: permissions, read-only, version history | §5 rows; WP11 tests (server store); the HRM backend as stub until HRB PR-4a/PR-7 | WP7, WP11 |
| Task 2: DocsAPI-shaped JS API | `docserver.spec.ts` | WP5 |
| Task 3: compatibility notes | `docs/phase5/onlyoffice-compat.md`, every row with a test or "not supported" | WP16 |
| Task 4: one flow behind a feature flag | HRB `office.craft_enabled` flip back with no restart | WP17 |
| Task 5: JWT on every call, SSRF, size limits, audit log | S1-S17 | WP2-WP4, WP8 |

### 13.4 Owner decisions (each with a recommendation)

| # | Decision | Options | Recommendation |
|---|---|---|---|
| **Q1** | **Which HRM document flow is the pilot** (`BRIEF.md:571`). **Assumed (a) until the owner says otherwise; a change affects WP1, WP9, WP17 only.** | (a) HRB `files` docx (folders module); (b) an order-act decree in draft; (c) letters or reports | **(a) `files` docx.** No Word↔DB sync, no locked statuses, no bookmark dependence (F15; `hrm-current-state.md:203-217`); HRB's defaults already match (`OFFICE_CRAFT_KINDS=file`, `OFFICE_CRAFT_FILE_TYPES=docx`, `hr-backend-prompt.md:345-346`). The decree comes second, after WP9 passes on a real HRM decree (H12). |
| **Q2** | Pilot users and client | which SPA gets `editor=auto` + `editorUrl`; who | **hr-frontend-v2 only** (it has the env URL and an e2e spec, `hrm-current-state.md:310-314`); 2–3 IT staff by `OFFICE_CRAFT_PILOT_USER_IDS`; the mobile app unchanged, so it stays on ONLYOFFICE. |
| **Q3** | Who runs HRB PR-0 … PR-3, and when | the HRM team / an agent with `hr-backend-prompt.md` / later | **Start now** with the prompt as written (`SIZING.md:279`). PR-4 … PR-8 are not needed for the gate. |
| **Q4** | Config token expiry | require `exp` / accept none per provider | **Require `exp`, at most 12 h** (F1). HRB's PR-3 `exp` commit becomes required. Without it a captured edit config is replayable for as long as its document URL token lives (1 h, `hrm-current-state.md:256`). |
| **Q5** | Deployment on `<test-host>` | container behind Traefik / a separate VM | **Container on `backend_net` behind the existing Traefik**, `tls = "upstream"`, internal listener for `api` only (§7). Needs the host name (H4) and your deploy OK. |
| **Q6** | Version history in HRM sessions | HRM store / ours | **The HRM's store is the record when configured**; ours in standalone server mode (D6). Neither is a gate item. |
| **Q7** | Diff scope in Phase 5 | all four / Docs + Sheets | **Docs + Sheets in Phase 5**; Slides and PDF later (Phase 8 or 9). |
| **Q8** | SSO and desktop sign-in/sync in the Phase 5 gate | in / after | **After the pilot gate** (WP13). HRB has no OAuth server until PR-6/PR-7; the seam stays. |
| **Q9** | XDFU server features and EDS (phase3 O10, O11) | all in Phase 5 / split | **Phase 5: server stamping and encryption at rest, off by default.** E-IMZO, the registry and the grif-removal workflow move to a later phase until O10/O11 are answered. |
| **Q10** | Retention of working copies on our server | — | **24 h after a good close, 7 days for an abandoned session**, no backups of the data folder. |
| Q11 | HRB-side names (`CRAFT_*` env names, `editorProvider: "craft"`) | keep / rename | HRM's own config, not our brand, so **the HRM team's call**. Our code names neither (B-1 note, ADR App. B). |
