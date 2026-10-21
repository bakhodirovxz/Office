# The converter HTTP API (`services/convert-api`, WP7)

Status: implemented and tested on 2026-10-21; hardened after the architect security review the same day
([review-wp7.md](review-wp7.md), every finding has a status there). Plan: [PLAN.md](PLAN.md) section 5. The UI page is WP8
(`services/convert-api/ui/`). XDFU handling (WP15, opt-in, off by default) is in the converter core; see "XDFU" below.

Run: `convert-api [--config FILE]`. With no file it is **local mode** on `127.0.0.1:8787`. The tools are found like the CLI does
(`CONVERT_DOCS_CLI`, ..., `CONVERT_TOOLS_DIR`), the sidecar through `CONVERT_PDF2DOCX_URL`, fonts through `CRAFT_FONTS_DIR`.

## The two modes

| | Local (default) | Server |
|---|---|---|
| `auth` | `"none"` | `"jwt"` (HS256) |
| Address | loopback only (`127.0.0.0/8`, `::1`). **Any other address refuses to start** (exit 2), `::ffff:127.0.0.1` included | any |
| Transport | plain HTTP | HTTPS only: TLS built in (`tls = "builtin"`), or ended by a trusted reverse proxy (`tls = "upstream"`). HSTS is sent on every answer that travelled over TLS |
| Login | none anywhere | `Authorization: Bearer <token>`; never from the URL |
| Files at rest | plain files in `work_dir` and `store_dir` | AES-256-GCM, a key per job, held in memory only |
| Required keys | none | `secret_file`, `public_url`, and either `tls_cert` + `tls_key` or `tls = "upstream"` + `trusted_proxies` |
| `public_url` | refused | required (the `Host` check and the redirect target) |

Local mode is for one person on one computer. Things the loopback rule cannot see, and the operator must not do: a port forward
(`netsh portproxy`, `ssh -R`, a published container port) to a loopback-bound local-mode server hands `auth = "none"` to the
network, because a non-browser client chooses its own `Host`. On a shared Windows host (RDS) every session shares loopback and
the principal `local`: run server mode there.

Token: HS256, `alg` must be exactly `HS256`; claims `{sub, exp, scope:"convert"}`, plus:
- `exp` is required, with 60 s of leeway for clock drift, and must not be further away than `max_token_lifetime_s` (default 12 h):
  a token cannot be revoked, so a leaked one must run out;
- `nbf`, when present, must not be more than 60 s in the future;
- `iss` must equal the configured `iss` when one is set; `aud` must equal (or list) the configured `aud` when one is set;
- use a secret that only the issuer and this service share, so that no other service's holders can mint convert tokens.

`sub` is the job owner: another user's job id (or any id that does not exist) gives 404. Any organisation can issue these tokens;
nothing in the code names one. Code: `src/jwt.rs` (ported from `spikes/docserver/src/jwt.rs`, plus the claim rules).

## Endpoints

All under `/api/v1` need the mode's auth. Errors are `{"error": {"code", "message"}}`.

| Method, path | Answer |
|---|---|
| `GET /api/v1/formats` | `convert formats --json` plus `limits` |
| `POST /api/v1/jobs` | multipart: `files` (1..30, repeated, also `files[]`) and `options` (JSON, at most 64 KiB, checked while it streams). `202 {job, files:[{n, name, kind, target} or {n, name, kind, error:{code, message}}]}` and `Location`. A file that is unsupported, password-protected, a zip bomb or an unreadable type is rejected **on its own**; the rest run |
| `GET /api/v1/jobs/{id}` | `{job, state: queued\|running\|done, progress:{done,total}, files:[{n, name, kind, target, state: queued\|running\|done\|failed, step, code, message, warnings, outputs:[{id, name, size}]}]}`; a file also has `xdfu` when the XDFU hook found a grif |
| `GET /api/v1/jobs/{id}/files/{n}/{id}` | one output, `Content-Disposition: attachment` with an RFC 5987 `filename*` |
| `GET /api/v1/jobs/{id}/zip` | every output plus `report.json` (the per-file reports, schema `report.schema.json`), once the job is `done`; built once (it takes a CPU slot), then served |
| `DELETE /api/v1/jobs/{id}` | cancels (the running step is killed through the convert cancel flag and the Windows Job Object) and deletes now. `204`. The job can no longer be fetched, but it counts against the limits until its work has stopped |
| `GET /healthz` | local mode, or a valid token: `{ok, tools:{name:bool}, sidecars:{pdf2docx:null\|bool}, queue:{active,max}}`, no paths. Server mode without a valid token: `{"ok":true}` only |
| `GET /`, `/app.js`, `/app.css`, `/i18n.json`, ... | the UI files (WP8) |

`options` (unknown keys are a 400): `to` (required), `translit` (`cyrl\|latn\|auto`), `alphabet` (`1995\|2026\|auto`), `apostrophe`
(`official\|ascii`), `sheet`, `pages`, `dpi` (72-600), `pdfEngine` (`native\|sidecar`). Same meaning as the CLI flags.

Statuses of `POST`: 202; 400 bad multipart, options, no files; 409 `too_large_for_server` (the file needs more memory than the whole
budget); 413 `file_too_large`, `batch_too_large`, `too_many_files`, or a `Content-Length` over the request limit; 408
`upload_timeout`; 429 `rate_limited`, `too_many_active_jobs` or `user_store_full` (all with `Retry-After`); 503 `queue_full`,
`store_full`, `disk_low` or `busy` (no CPU slot within `slot_wait_s` for the upload check; all with `Retry-After`).
`GET .../zip` answers 503 `busy` the same way.

**Reports carry ids and names, never server paths.** `input` is the sanitised upload name, `outputs` are the download names. Every
string in a report (messages, tool stderr tails, warnings) goes through `Redactor`, which replaces the configured work, store, temp
and tool directories and the sidecar's address with `<server>` and any remaining `X:\...` path with `<path>`. The sidecar's address
is also left out of the messages themselves.

**Names.** A client name is never a path. Uploads are stored as `in/<n>` and outputs as `out/<n>/<k>`. The name goes through
`convert::names::safe_name` (no separators, `..`, control, bidi or zero-width characters, no device names including `COM¹` and
`CONIN$`, 200 characters); equal output names get ` (2)`, ` (3)`. Downloads are `attachment`, with a fixed content type per extension
and `application/octet-stream` for markup, so a downloaded html file can never run in our origin.

## Configuration (`convert-api --config FILE`, TOML; unknown keys are an error)

| Key | Default | Meaning |
|---|---|---|
| `listen` | `127.0.0.1:8787` | `ip:port` (an IP literal) |
| `auth` | `none` | `none` or `jwt` |
| `secret_file`, `iss`, `aud` | - | server mode: the issuer's HS256 secret (at least 32 bytes) and the accepted `iss` and `aud` (each optional) |
| `max_token_lifetime_s` | 43200 | server mode: the longest remaining life a token may have when it arrives |
| `tls` | `builtin` | `builtin`: this server speaks TLS. `upstream`: a reverse proxy ends TLS and talks plain HTTP to this server; allowed only with a non-empty `trusted_proxies`, and then only those proxies are answered |
| `trusted_proxies` | `[]` | IPs and CIDRs (`"10.1.0.0/16"`). Only a peer in this list may set `X-Forwarded-For` and `X-Forwarded-Proto`; for every other peer both are ignored. The client is the rightmost `X-Forwarded-For` address that is not itself a trusted proxy |
| `tls_cert`, `tls_key` | - | `tls = "builtin"`: PEM files |
| `redirect_listen` | - | `tls = "builtin"`: a plain-HTTP listener that redirects (308) to `public_url`, never to the request's `Host` |
| `public_url` | - | server mode, required: `https://host[:port]`, no path, query or user info. Fixes the allowed `Host` |
| `key_file` | - | not used any more (the job keys are not written anywhere, WP7 review M5). Still accepted; if set it must hold 32 random bytes (raw, base64 or 64 hex characters), not a passphrase |
| `work_dir` | `<tmp>/convert-api-work` | scratch for the tools: **plaintext**. Dedicated to this server. Server mode: put it on a RAM disk |
| `store_dir` | `<work_dir>-store` | where the job store lives (ciphertext in server mode) |
| `xdfu` | `off` | `off`, `watermark` or `refuse` (see "XDFU"). Off installs nothing |
| `xdfu_audit_file` | - | the XDFU audit log. Required in server mode, on storage that survives a restart (not the RAM disk of `work_dir`); local mode defaults to `<user data dir>/convert/xdfu-audit.jsonl` |
| `xdfu_redact` | `false` | leave file names out of the audit log |
| `cpu_slots` | `clamp(cores/3, 1, 4)` | concurrent conversions |
| `mem_budget_mb` | 12288 | each conversion reserves its estimate first (sheets 20 MB per input MB, others 40, plus 256, capped at the tool cap) |
| `queue_max`, `per_user_active` | 100, 3 | active jobs in total, and per `sub` |
| `max_file_mb`, `max_xlsx_mb` | 50, 100 | per file (sheet kinds by extension get the larger one) |
| `max_batch_files`, `max_batch_mb` | 30, 300 | per request |
| `store_max_mb` | 20480 | the store never grows beyond this: a new job is refused (503 `store_full`) when what the store holds plus one batch reservation (`max_batch_mb`) would pass it |
| `user_store_mb` | 2048 | the most one `sub` keeps in the store at once (429 `user_store_full`), reservation included |
| `min_free_mb` | 2048 | a new job is refused (503 `disk_low`) while the volume of `store_dir` has less than this plus `max_batch_mb` free. Skipped, with no error, when the volume cannot be found |
| `slot_wait_s` | 30 | how long the upload check (and a zip build) waits for a CPU slot before it answers 503 `busy` |
| `max_connections`, `max_conn_per_ip` | 1024, 32 | open connections in total, and per client address (an IPv6 /64; trusted proxies are not counted per address) |
| `retention_s` | 3600 | results are deleted this long after the job is done |
| `upload_timeout_s`, `job_timeout_s` | 900, 1800 | `job_timeout_s` is for the whole API job, all its files together, from admission |
| `rate_user_per_min`, `rate_ip_per_min` | 30, 120 | job creation per `sub` and per client address (an IPv6 /64); failed logins per address are limited to 30 a minute |

## Behind a reverse proxy (Traefik)

Server mode can sit behind Traefik (or any proxy that sets `X-Forwarded-For` and `X-Forwarded-Proto`) in one of two ways.

1. **TLS ends at Traefik** (`tls = "upstream"`). Traefik sees the client address and forwards it. Config of this server:
   ```toml
   auth = "jwt"
   secret_file = "/run/secrets/convert-jwt"
   listen = "0.0.0.0:8787"
   tls = "upstream"
   trusted_proxies = ["172.18.0.0/16"]      # the network Traefik is on, and nothing wider
   public_url = "https://convert.example.org"
   ```
   Traefik, dynamic configuration (file provider):
   ```yaml
   http:
     routers:
       convert:
         rule: Host(`convert.example.org`)
         entryPoints: [websecure]
         tls: {}
         service: convert
     services:
       convert:
         loadBalancer:
           servers:
             - url: http://convert-api:8787
   ```
   The `web` entry point redirects to `websecure` in Traefik (`redirections.entryPoint`); this server redirects too, to `public_url`,
   when the proxy says the client came in over plain HTTP. Rules of this mode:
   - Only a peer in `trusted_proxies` is answered (403 `not_via_proxy` for anyone else), except a health probe on the loopback
     address, which learns `{"ok":true}` only. A client cannot reach this port directly and choose its own address.
   - On an entry point that faces clients, leave Traefik's `forwardedHeaders.trustedIPs` empty, so that Traefik overwrites
     a client-supplied `X-Forwarded-For` instead of appending to it.
   - Do not put a buffering middleware on this route, and let `respondingTimeouts.readTimeout` cover `upload_timeout_s`.
   - HSTS is sent only on answers whose request carried `X-Forwarded-Proto: https`.
2. **TLS passes through Traefik** (a TCP router with `tls.passthrough`). The server keeps `tls = "builtin"`. It then sees Traefik's
   address for every client, so the per-address limits become one shared bucket for all users. Use the first way unless that is
   acceptable.

The rate limits and the logs use the address the rules above give. The per-address connection cap counts the peer (the proxy is
exempt from it, the total cap still applies to it).

## Web security baseline (always on)

Every response, errors and static files included, carries: `Content-Security-Policy: default-src 'self'; script-src 'self'; object-src
'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
`Permissions-Policy: camera=(), microphone=(), geolocation=()`, `Cache-Control: no-store`, `Cross-Origin-Resource-Policy: same-origin`,
`Cross-Origin-Opener-Policy: same-origin`, and in server mode `Strict-Transport-Security: max-age=31536000`. No CORS header is ever set
and there is no third-party origin. Tested by `every_response_carries_the_security_headers` (202, status, output, zip, formats, healthz,
the UI files, 404, 405, 400, 421, 403, 401, 204, in both modes) and by `error_statuses_413_408_409_429_503_carry_the_security_headers_too`
(`tests/hardening.rs`); for the redirect listener by `the_plain_http_listener_only_redirects_to_the_configured_url`.

Also on: a `Host` check (local mode: loopback names with the port; server mode: `public_url`) against DNS rebinding, and an `Origin` check
(an `Origin` header must be this very host) against cross-site posts to the local server, which has no login.

TLS (`src/tls.rs`): rustls with the ring provider, TLS 1.3 and TLS 1.2, cipher suites filtered to AEAD (GCM, ChaCha20-Poly1305).
Tested with real handshakes pinned to 1.3 and to 1.2. `aws-lc` is not used: its licence includes OpenSSL.

**Listener limits** (the TLS listener, the redirect listener and the local listener all run the same accept loop): the handshake and
the head of every request, the idle wait between keep-alive requests included, have 10 s; `max_connections` open in total; `max_conn_per_ip`
per client address; a connection ends after 100 requests (`Connection: close`) or after `max(upload_timeout_s, 3600) + 600` s. A client
that connects and stays silent, or drips a request, or keeps a connection alive with one request every few seconds, loses its slot.
Tested with real sockets (`tests/hardening.rs`).

## Data at rest (server mode) and exactly what is plaintext

Design: a random 256-bit key per job, held in memory (`JobKey`, zeroed on drop; the `aes` crate's `zeroize` feature is on, so its key
schedule is wiped too). **It is not written to disk anywhere**: a crash leaves ciphertext with no key on the machine, and the next start
sweeps it. Blobs are streamed in 64 KiB AES-256-GCM frames; the nonce is file id plus frame index; the AAD is `job id | role | last`
(role: `in/3`, `out/1/0`, `zip`), so a truncated, reordered, tampered or swapped blob fails to read, and so does data after the last
frame. `src/crypt.rs` (2 tests).

The tools cannot read ciphertext, so **plaintext exists in these places, for this long**:

| Where | Holds | From - until |
|---|---|---|
| The request, in memory | each upload chunk | encrypted into `in/<n>` before the next chunk is read; nothing else on disk. The `options` part is held up to 64 KiB |
| `work_dir/stage-<pid>-pre-<job>-<n>/` | a decrypted copy of one upload | the per-file check at `POST` (kind, zip scan, plan): seconds; deleted when the check returns, also on error |
| `work_dir/stage-<pid>-<hex>-<n>/input.<ext>` | the decrypted input | convert's staging copy: until `convert_source` returns |
| `work_dir/job-<pid>-<hex>-<n>/{in,mid,out,tmp,home}` | input, intermediate files, outputs, the tools' temp files | from the start of the run until `run_job` ends. Outputs leave `out/` encrypted (`JobSink::put`) before the directory is deleted |
| `store_dir/stage-<pid>-api-<id>/` | **ciphertext**: `in/<n>` until its file has run, `out/<n>/<k>`, `zip` | an input is deleted when its file ends; an output and the zip stay for the retention time |
| Process memory of the API and of every tool | whatever they hold: frames, documents (gigabytes for sheets) | while a request or step runs |
| The page file and the hibernation file | any of the memory above, at the system's choice | until overwritten; not handled by this software |
| Crash dumps | a tool's or the server's memory. Windows Error Reporting writes a minidump for a tool that dies with an exception, and may upload it according to the machine's consent policy; Linux writes a core file where `core_pattern` says | until deleted by hand |
| Free clusters | every deleted plaintext file above, until the disk reuses the space; on an SSD, wear levelling keeps it out of reach of any wipe | indefinitely |
| The pdf2docx sidecar and the WP6 LibreOffice sidecar | the whole PDF (or deck), in its own temp files and RAM, and the DOCX answer, which also sits in the API's memory (at most four times the input limit) | the sidecar's request; its temp files are the sidecar's to remove. The hop is plain HTTP on an internal network (PLAN 10 clause 11: network exchange) |
| Antivirus quarantine and sample submission | a copy of a flagged input (a docx with a macro, say), in the scanner's own store | the scanner's policy |
| Volume snapshots and backup agents (VSS, image backups) | everything on the volume while it exists | the snapshot's life |
| `C:\Windows\SystemTemp` | a file an engine writes to the temp directory when the service runs as LocalSystem: `GetTempPath2` ignores `TEMP` there, so the per-job `tmp` and the sweeper would not see it. UNVERIFIED whether any engine writes temp files at all | until deleted by hand |

Not encrypted: sizes, timestamps, the number of files, and the directory layout (names on disk are numbers; client names live in memory only).

What the operator must do, because the software cannot:
- **Put `work_dir` on a RAM disk.** Linux and containers: a tmpfs (`--tmpfs /work:size=3g`). Windows: a RAM disk driver. The server
  checks at every start (Linux: `/proc/mounts`) and prints a boxed WARNING when `work_dir` is not on tmpfs or ramfs, or cannot be
  told (Windows). On a RAM disk a crash or a power loss takes the plaintext with it.
- **Size it:** `cpu_slots x (3 x the largest file + 512 MB)` of scratch (3.2 GB with the defaults and a 100 MB sheet limit). A 200-page
  png export needs room for its pages.
- **`store_dir` may stay on disk**: it holds ciphertext, sized by `store_max_mb`. Its default is a path of its own, not `work_dir`,
  so each can be sized and placed alone. If it is on a tmpfs, size the tmpfs for `store_max_mb`, not for the scratch.
- **Turn on full-disk encryption of the OS volume** (BitLocker; LUKS on Linux). It is a requirement of server mode, not a second
  line: it is the only thing that covers the page file, the hibernation file, crash dumps and free clusters.
- **Switch crash dumps off for the service**: Windows Error Reporting `LocalDumps` and consent policy; in the WP11 image
  `--ulimit core=0`. Run the service as a dedicated, non-SYSTEM account.
- **Keep `work_dir` out of volume snapshots and backups**, and decide the antivirus policy for it (an owner item for the
  organisation's security staff: scanning the directory is safe for the data, quarantine and sample submission copy it).
- Put the sidecars on the same internal network without egress, with a tmpfs `/tmp` (WP11).

A kill mid-run leaves the scratch directories above in plaintext (the tools were killed with the server through the Job Object, but the
files stay). The next start deletes them first thing (`convert::job::sweep_stale`). It deletes only directories whose names have
exactly the shapes this software makes (`job-<pid>-<hex>-<n>`, `job-<pid>-check`, `stage-<pid>-<hex>-<n>`, `stage-<pid>-api-<32 hex>`,
`stage-<pid>-pre-<32 hex>-<n>`) and whose process is gone; a pid that a later process reused does not keep a directory alive, because
the owner must have started before the directory was last written. An operator's folder such as `stage-2024-photos` is never touched.

## Queue, limits, isolation

- A fair CPU gate, then the memory budget, in that order (a holder of both always finishes, so the gates cannot deadlock). The CPU
  slots are handed out round-robin by user (`src/gate.rs`), not first come first served: a user with ninety queued files takes one slot
  per turn like everybody else. The upload check also takes a CPU slot (it inflates the upload to scan it), but waits at most
  `slot_wait_s`; building a zip takes one too.
- **One deadline for the API job**: `job_timeout_s` from admission, for all its files together. A file that is still waiting for a
  slot or for memory when the deadline passes fails with exit 4; a running step is cut at the deadline.
- Admission is counted before the body is read, so a full queue or a user at 3 active jobs costs no disk. In order: the queue, the user's
  active jobs, the store quota (`store_max_mb`), the user's retained data (`user_store_mb`), the disk floor (`min_free_mb`). Every job
  holds a reservation of `max_batch_mb` from admission until its upload is stored, so simultaneous uploads cannot overshoot a quota.
- **What the store holds is bounded**: an input is deleted as soon as its file has run, or has failed its check at `POST`
  (inputs are never served back), so the store holds outputs and the zip. A job that is deleted or whose request is dropped (the client
  closed the connection while the upload check waited) is cancelled and removed at once; nothing is left counting against the limits.
- The body limit is applied by `RequestBodyLimitLayer` (it refuses a large `Content-Length` before the handler runs and counts a streamed
  body); each part is counted while it is written, and the whole upload has `upload_timeout_s`.
- Every step is a subprocess in a Windows Job Object with kill-on-close; `DELETE` sets the job's cancel flag (checked before staging
  work too). A restart loses the jobs (in-memory queue, PLAN 5.2 shortcut): their ids answer 404 and their files are swept at start.
- Zip bombs: `convert::scan` runs at `POST` (central directory, then a measured inflate) and its verdict is handed to the job, which
  skips a second inflate when its staged copy has the same SHA-256. Limits: 1 GiB inflated per file, 10,000 entries, overlapping or
  out-of-file parts, ratio 100:1 for every entry above 1 MiB, and one inflate budget per request: 50 times the upload, between
  256 MiB and 2 GiB, charged across all its files. The test fixtures are a 40,755-byte zip of overlapping entries that declares 4.5 GB,
  a 12 MB part with a 1000:1 ratio, and a 9.9 MB part with a ratio of the same size that the old 10 MB floor let through.
- **Empty files and legacy text encodings** (WP9 fixes): a 0-byte file of any claimed kind (`a.txt`, `a.docx`, `a.pdf`, no extension) is
  rejected at `POST` and by the CLI with exit 9 and "the file is empty (0 bytes)"; it is never converted as an empty text file. A `.txt`,
  `.csv`, `.tsv`, `.md` or `.html` file in cp1251, cp1252 or UTF-16 (BOM, little or big endian) is converted to UTF-8 in the job's private
  copy before the scan and before any tool, and the report carries `text_transcoded` naming the encoding. Detection order: a BOM, then
  valid UTF-8 (nothing to do), then cp1251 against cp1252 by how the high bytes sit in words (runs of three or more mean Cyrillic).
  Legacy text under any other extension (`x.docx` that holds cp1251 text) is still refused with exit 3. Limits: no other code page
  (cp1250, koi8-r) is guessed, and UTF-16 without a BOM is not recognised.
- **Damaged and scanned PDFs** (WP9 fixes): `pdf` to `txt` and to `docx` (native engine) first run `pdf-cli info`; what it lists as unread
  (a cut-off file: "failed parsing cross reference table") is shown as the engine said it (`upstream`) and the file is flagged
  `pdf_damaged`; `png` and `split` give the same flag from the `info` they ran anyway. A PDF with no text layer gives `empty_text` to
  `txt` (empty output) and to `docx` (the native docx holds no non-blank run); the sidecar engine already answers 422 for it.
- Logs: job id, `sub` (quoted, so a hostile value cannot forge fields), kinds, sizes, codes and timings; never a file name, a token or
  document content. A panic prints only its location, not its message.

## XDFU

Resolution No. 766 of 2025 and its Rules (PLAN section 10). **Opt-in and off by default.** `xdfu = "off"` installs no hook at all: no
detection, no watermark, no audit file, no `xdfu` in any report, and an output that is byte for byte what it was before (tested). The
setting is the operator's (the server config, or the local user's `--xdfu` flag of the CLI); the UI shows nothing about it.

`xdfu = "watermark"` or `"refuse"` installs the hook of `services/convert/src/xdfu.rs` for the API and for the CLI alike:

- **Detection** (before any tool runs): the grif "XDFU", "ХДФУ", "ДСП", "Для служебного пользования", or the Uzbek phrase in Latin or
  Cyrillic ("Xizmat doirasida foydalanish uchun", "Хизмат доирасида фойдаланиш учун"), in any case and spacing ("X D F U", "Х.Д.Ф.У.",
  a Latin x for a Cyrillic х), as a whole word. Looked for in the places the Rules name: the top of page 1 (the first 8 paragraphs of a
  docx, the first 5 rows of the first sheet, the top quarter of the first slide, the first 8 lines of page 1 of a pdf, the first 8 lines
  of a txt, md or csv), a header (docx headers, sheet headers and footers, slide masters and layouts) and the document properties
  (docx, xlsx, pptx core, app and custom properties; the pdf title, author, subject and keywords). The report gets
  `xdfu: {detected: true, mode, copy, refused, evidence: [{place, part, marker}]}`; the evidence names the place and the marker, never
  document text.
- **`refuse`**: the before-phase vetoes with exit 9 and a clear message. Nothing is converted.
- **`watermark`**: an output kind that cannot carry a mark (`odt`, `rtf`) is refused before any tool runs (exit 3). Otherwise the
  after-phase writes the audit line, whose sequence number is the **copy number**, and marks every output of the job with
  `XDFU copy N | <client address> | <user> | <date and time UTC>`. The user is the `sub` in server mode and the OS user in local mode;
  the client address is the address behind the trusted proxies, and in local mode this computer's address. The outputs are validated again
  after the hook.

  | Output | The mark |
  |---|---|
  | `pdf` | a footer line and a diagonal text on every page (`pdf-cli`: `doc_header_footer`, `doc_watermark`). Western European text only, so other characters in a user name become `_` with a short digest |
  | `png` | a strip with the line appended under the page (a one-line page rendered by `pdf-cli` at the png's dpi; the page pixels are never covered), and the line as a `tEXt` chunk `XDFU` |
  | `docx` | a header, a footer, a diagonal watermark and the custom properties `XDFU-copy` and `XDFU-stamp` (`docs-cli`) |
  | `xlsx` | a header and a footer on every sheet (`sheets-cli`); they show in the page layout and on paper |
  | `pptx` | a footer on every slide (`slides-cli`) |
  | `txt`, `md` | a first line (a first paragraph in md) |
  | `csv` | a first row of one quoted field |
  | `html` | a paragraph right after `<body>` |
  | `odt`, `rtf` | cannot carry one: refused |

  The matrix test marks all 30 directions the planner has from a docx, xlsx, pptx, pdf, txt, md or csv input and reads the mark back
  from each output; the 6 directions to odt or rtf are refused.
- **Audit log** (`services/convert/src/audit.rs`): JSON lines, append-only, hash-chained. Each line has `seq`, `ts` (UTC), `prev` (the
  hash of the line before, 64 zeros first), `event` (`copy`, or `refused` with a `reason` of `mode` or `cannot_carry`), `user`,
  `client`, `host`, `job` (the API job id and file number, or the converter's own id), `file` (left out with `xdfu_redact` or `--redact`),
  `kind`, `target`, `places`, `outputs`, and `hash`, which is SHA-256 over the line without its `hash`. Never any content. Verify with
  `convert xdfu-verify FILE`: it prints the line count and the last hash, or the first bad line (exit 1). A changed, deleted, inserted
  or reordered line breaks the chain from that line on. Two limits: lines cut off the END of the file are not detected (record the
  last hash elsewhere from time to time), and "append-only" is how this code writes the file: make the file append-only for the
  service account (an ACL that grants append but not write; `chattr +a`). A damaged log is not extended: the server and the CLI refuse
  to start with it. Two processes may share a log (a lock file serialises them).
- **Limits, said plainly.** Detection reads text, not layout, so a word that is also an ordinary word (ДСП is chipboard in Russian) in
  those places is a false positive, which errs on the safe side. Kinds that are not scanned: `doc`, `xls`, `xlsb`, `ppt`, `odt`,
  `ods`, `odp`, `rtf`, `html`, `tsv`; a grif in one of them is not found (an owner decision: convert them to a scanned kind first, or
  refuse them). Local mode on an uncertified computer only watermarks and warns; it cannot make XDFU handling compliant. The audit log
  and the marks say nothing about the Rules' print, copy and save restrictions (Phase 4) or the EDS and QR (Phase 5).

## Tests (`cargo test`, run from `services/`)

`convert-api`: `src/*` unit tests (15: jwt 6, crypt 2, config 4, gate 1, app 2), `tests/api.rs` (18, in-process router against a stand-in tool),
`tests/hardening.rs` (17: the review fixes), `tests/xdfu_api.rs` (5: WP15 through the API), `tests/tls.rs` (3, real sockets), `tests/e2e.rs`
(2, the binary: refusals; every direction against the real tools, then a hard kill and a restart). `convert`: `tests/xdfu.rs` (5, real tools:
detection per language, off is byte-identical, every direction marked or refused, refuse and redact, the audit chain and `xdfu-verify`),
`src/audit.rs` and `src/xdfu.rs` unit tests, `tests/verdict.rs` (3). The e2e test needs the release tools in `E:/office-target/p1/<domain>/release` and
the WP1 inputs in `E:/office-target/p3/census/in`; it skips with a message without them (`CONVERT_API_E2E_REQUIRED=1` turns that into
a failure).

## Not done (deferred)

- SSO (OIDC) as a third `auth` value; PROXY protocol v2 (the trusted-proxy list with `X-Forwarded-For` is used instead).
- A persistent queue (jobs survive restarts).
- Operator key rotation and an HSM do not apply any more (no key is stored); token revocation (`jti`) waits for SSO, with
  `max_token_lifetime_s` limiting the damage of a leaked token.
- Unix absolute paths in tool messages are scrubbed only under the configured roots. Do this before the Linux image (WP11).
- The sidecar call cannot be interrupted by a cancel (the job deadline caps it).
- `Range` requests and `HEAD` on downloads.
