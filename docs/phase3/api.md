# The converter HTTP API (`services/convert-api`, WP7)

Status: implemented and tested on 2026-10-21. Plan: [PLAN.md](PLAN.md) section 5. The UI page is WP8; `services/convert-api/ui/` holds
placeholders that WP8 replaces. XDFU handling is WP15 and is not implemented (see "XDFU" below).

Run: `convert-api [--config FILE]`. With no file it is **local mode** on `127.0.0.1:8787`. The tools are found like the CLI does
(`CONVERT_DOCS_CLI`, ..., `CONVERT_TOOLS_DIR`), the sidecar through `CONVERT_PDF2DOCX_URL`, fonts through `CRAFT_FONTS_DIR`.

## The two modes

| | Local (default) | Server |
|---|---|---|
| `auth` | `"none"` | `"jwt"` (HS256) |
| Address | loopback only. **Any other address refuses to start** (exit 2) | any |
| Transport | plain HTTP | HTTPS only (rustls: TLS 1.3, TLS 1.2 with AEAD suites). HSTS is sent. A plain-HTTP listener only redirects |
| Login | none anywhere | `Authorization: Bearer <token>`; never from the URL |
| Files at rest | plain files in `work_dir` | AES-256-GCM, a key per job |
| Required keys | none | `secret_file`, `key_file`, `tls_cert`, `tls_key` |

Token: HS256, `alg` must be exactly `HS256`, claims `{sub, exp, scope:"convert"}`, `exp` is required, `iss` must equal the configured
`iss` when one is set. `sub` is the job owner: another user's job id (or any id that does not exist) gives 404. Any organisation can issue
these tokens; nothing in the code names one. Code: `src/jwt.rs` (ported from `spikes/docserver/src/jwt.rs`, with its 4 tests, plus `exp`
required and the claim rules).

## Endpoints

All under `/api/v1` need the mode's auth. Errors are `{"error": {"code", "message"}}`.

| Method, path | Answer |
|---|---|
| `GET /api/v1/formats` | `convert formats --json` plus `limits` |
| `POST /api/v1/jobs` | multipart: `files` (1..30, repeated, also `files[]`) and `options` (JSON). `202 {job, files:[{n, name, kind, target} or {n, name, kind, error:{code, message}}]}` and `Location`. A file that is unsupported, password-protected, a zip bomb or an unreadable type is rejected **on its own**; the rest run |
| `GET /api/v1/jobs/{id}` | `{job, state: queued\|running\|done, progress:{done,total}, files:[{n, name, kind, target, state: queued\|running\|done\|failed, step, code, message, warnings, outputs:[{id, name, size}]}]}` |
| `GET /api/v1/jobs/{id}/files/{n}/{id}` | one output, `Content-Disposition: attachment` with an RFC 5987 `filename*` |
| `GET /api/v1/jobs/{id}/zip` | every output plus `report.json` (the per-file reports, schema `report.schema.json`), once the job is `done`; built once, then served |
| `DELETE /api/v1/jobs/{id}` | cancels (the running step is killed through the convert cancel flag and the Windows Job Object) and deletes now. `204` |
| `GET /healthz` | open: `{ok, tools:{name:bool}, sidecars:{pdf2docx:null\|bool}, queue:{active,max}}`, no paths |
| `GET /`, `/app.js`, `/app.css`, `/i18n.json` | the UI files (WP8) |

`options` (unknown keys are a 400): `to` (required), `translit` (`cyrl\|latn\|auto`), `alphabet` (`1995\|2026\|auto`), `apostrophe`
(`official\|ascii`), `sheet`, `pages`, `dpi` (72-600), `pdfEngine` (`native\|sidecar`). Same meaning as the CLI flags.

Statuses of `POST`: 202; 400 bad multipart, options, no files; 409 `too_large_for_server` (the file needs more memory than the whole
budget); 413 `file_too_large`, `batch_too_large`, `too_many_files`, or a `Content-Length` over the request limit; 408
`upload_timeout`; 429 `rate_limited` or `too_many_active_jobs` (with `Retry-After`); 503 `queue_full` (with `Retry-After`).

**Reports carry ids and names, never server paths.** `input` is the sanitised upload name, `outputs` are the download names. Every
string in a report (messages, tool stderr tails, warnings) goes through `Redactor`, which replaces the configured work, store, temp
and tool directories with `<server>` and any remaining `X:\...` path with `<path>`.

**Names.** A client name is never a path. Uploads are stored as `in/<n>` and outputs as `out/<n>/<k>`. The name goes through
`convert::names::safe_name` (no separators, `..`, control or bidi characters, no device names, 200 characters); equal output names get
` (2)`, ` (3)`. Downloads are `attachment`, with a fixed content type per extension and `application/octet-stream` for markup, so a
downloaded html file can never run in our origin.

## Configuration (`convert-api --config FILE`, TOML; unknown keys are an error)

| Key | Default | Meaning |
|---|---|---|
| `listen` | `127.0.0.1:8787` | `ip:port` (an IP literal) |
| `auth` | `none` | `none` or `jwt` |
| `secret_file`, `iss` | - | server mode: the issuer's HS256 secret (at least 32 bytes) and the accepted `iss` |
| `key_file` | - | server mode: the operator key, 32 raw bytes or 64 hex characters; wraps every job key |
| `tls_cert`, `tls_key` | - | server mode: PEM files |
| `redirect_listen`, `public_url` | - | server mode: a plain-HTTP listener that redirects (308) to `public_url`, never to the request's `Host`. `public_url` also fixes the allowed `Host` |
| `work_dir` | `<tmp>/convert-api-work` | scratch for the tools and (by default) the job store. **Dedicated to this server** |
| `store_dir` | `work_dir` | where the job store lives |
| `xdfu` | `off` | any other value is refused: "not implemented yet" |
| `cpu_slots` | `clamp(cores/3, 1, 4)` | concurrent conversions |
| `mem_budget_mb` | 12288 | each conversion reserves its estimate first (sheets 20 MB per input MB, others 40, plus 256, capped at the tool cap) |
| `queue_max`, `per_user_active` | 100, 3 | active jobs in total, and per `sub` |
| `max_file_mb`, `max_xlsx_mb` | 50, 100 | per file (sheet kinds by extension get the larger one) |
| `max_batch_files`, `max_batch_mb` | 30, 300 | per request |
| `retention_s` | 3600 | results are deleted this long after the job is done |
| `upload_timeout_s`, `job_timeout_s` | 900, 1800 | |
| `rate_user_per_min`, `rate_ip_per_min` | 30, 120 | job creation per `sub` and per IP; failed logins per IP are limited to 30 a minute |

## Web security baseline (always on)

Every response, errors and static files included, carries: `Content-Security-Policy: default-src 'self'; script-src 'self'; object-src
'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
`Permissions-Policy: camera=(), microphone=(), geolocation=()`, `Cache-Control: no-store`, and in server mode `Strict-Transport-Security:
max-age=31536000`. No CORS header is ever set and there is no third-party origin. Tested by `every_response_carries_the_security_headers`
(202, status, output, zip, formats, healthz, the four UI files, 404, 405, 400, 421, 403, 401, 204, in both modes) and, for the redirect
listener, `the_plain_http_listener_only_redirects_to_the_configured_url`.

Also on: a `Host` check (local mode: loopback names with the port; server mode: `public_url`) against DNS rebinding, and an `Origin` check
(an `Origin` header must be this very host) against cross-site posts to the local server, which has no login.

TLS (`src/tls.rs`): rustls with the ring provider, TLS 1.3 and TLS 1.2, cipher suites filtered to AEAD (GCM, ChaCha20-Poly1305).
Tested with real handshakes pinned to 1.3 and to 1.2 (the 1.2 suite is ECDHE with GCM or ChaCha). Handshake and request-header timeouts are
10 s and at most 1024 connections are open. `aws-lc` is not used: its licence includes OpenSSL.

## Data at rest (server mode) and exactly what is plaintext

Design: a random 256-bit key per job, held in memory (`JobKey`, zeroed on drop) and wrapped with the operator key (AES-256-GCM, the job
id as AAD) into `<job>/key`. Blobs are streamed in 64 KiB AES-256-GCM frames; the nonce is file id plus frame index, the AAD marks the last
frame, so a truncated, reordered or tampered blob fails to read. `src/crypt.rs` (3 tests). The sweeper and the delete path remove `key`
first, then the data. The operator can unwrap `key` with the operator key to recover a crashed server's files; nobody else can.

The tools cannot read ciphertext, so **plaintext exists in these places, for this long**:

| Where | Holds | From - until |
|---|---|---|
| The request, in memory | each upload chunk | encrypted into `in/<n>` before the next chunk is read; nothing else on disk |
| `work_dir/stage-<pid>-pre-<job>-<n>/` | a decrypted copy of one upload | the per-file check at `POST` (kind, zip scan, plan): seconds; deleted when the check returns, also on error |
| `work_dir/stage-<pid>-<seq>/input.<ext>` | the decrypted input | convert's staging copy: until `convert_source` returns |
| `work_dir/job-<pid>-<id>/{in,mid,out,tmp,home}` | input, intermediate files, outputs, the tools' temp files | from the start of the run until `run_job` ends. Outputs leave `out/` encrypted (`JobSink::put`) before the directory is deleted |
| Process memory | whatever the tools and the 64 KiB frames hold | while a request or step runs; swap is not handled |
| `out/<n>/<k>`, `zip` | **ciphertext** | the retention time; downloads decrypt frame by frame into the response, and the zip is written through the encrypting writer, so there is no plaintext zip |

Not encrypted: sizes, timestamps, the number of files, and the directory layout (names on disk are numbers; client names live in memory only).

A kill mid-run leaves the scratch directories above in plaintext (the tools were killed with the server through the Job Object, but the
files stay). So: **put `work_dir` on a tmpfs or a RAM disk** (container: `--tmpfs /work:size=...`; Windows: a RAM disk), sized for
`cpu_slots x 2 x max_batch_mb` of scratch. `store_dir` may stay on disk (it holds ciphertext) or share the tmpfs. On a disk-backed
`work_dir`, a crash leaves plaintext until the next start, which deletes it first thing (`convert::job::sweep_stale`, which also catches
`stage-<pid>-api-*`: the store directories are named that way on purpose). Full-disk encryption of the volume is the operator's second line.

## Queue, limits, isolation

- CPU slots, then the memory budget, in that order (a holder of both always finishes, so the gates cannot deadlock). The upload check also
  takes a CPU slot, because it inflates the upload to scan it.
- Admission is counted before the body is read, so a full queue or a user at 3 active jobs costs no disk. The body limit is applied by
  `RequestBodyLimitLayer` (it refuses a large `Content-Length` before the handler runs and counts a streamed body); each part is counted
  while it is written, and the whole upload has `upload_timeout_s`.
- Every step is a subprocess in a Windows Job Object with kill-on-close; the job has a 30 minute deadline; `DELETE` sets the job's cancel
  flag. A restart loses the jobs (in-memory queue, PLAN 5.2 shortcut): their ids answer 404 and their files are swept at start.
- Zip bombs: `convert::scan` (central directory plus a bounded inflate: 1 GiB total, ratio 100:1 above 10 MB, 10,000 entries, overlapping
  or out-of-file parts) runs at `POST` and again at conversion. The test fixture is a 40,755-byte zip of overlapping entries that declares
  4.5 GB (`zip_bombs_are_rejected_per_file_and_the_rest_of_the_batch_runs`), plus a 12 MB part with a 1000:1 ratio.

## XDFU

Not implemented here (WP15, section 10, opt-in). WP7 only does the plumbing: `JobMeta.principal` (the `sub`, or `local`) and
`JobMeta.client` (the peer IP) reach every `JobHook` through `JobCtx`, and `xdfu = "off"` is the only accepted value of the config key.

## Tests (`cargo test -p convert-api`, run from `services/`)

`src/*` unit tests (9: jwt 5, crypt 3, config 1), `tests/api.rs` (18, in-process router against a stand-in tool), `tests/tls.rs` (3,
real sockets), `tests/e2e.rs` (2, the binary: refusals; every direction against the real tools, then a hard kill and a restart).
The e2e test needs the release tools in `E:/office-target/p1/<domain>/release` and the WP1 inputs in `E:/office-target/p3/census/in`; it
skips with a message without them (`CONVERT_API_E2E_REQUIRED=1` turns that into a failure).

## Not done (deferred)

- WP15 XDFU; SSO (OIDC) as a third `auth` value; a mode for TLS terminated upstream (a container behind Traefik). Server mode is
  HTTPS-only today and needs the owner's call before an upstream-TLS escape hatch exists.
- A persistent queue (jobs survive restarts); a global disk quota for in-flight uploads (today: per request 300 MB, 100 active jobs).
- Rate limits use the peer address; `X-Forwarded-For` is not trusted, so behind a proxy all clients share one IP bucket.
- Key rotation for the operator key, a key from an HSM or the environment, token revocation (`jti`).
- Header-read timeout and connection cap exist on the TLS path only. The local listener and the redirect listener use plain `axum::serve`.
- Unix absolute paths in tool messages are scrubbed only under the configured roots.
- `Range` requests and `HEAD` on downloads.
