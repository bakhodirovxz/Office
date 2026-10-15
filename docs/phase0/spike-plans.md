# Phase 0 spike plans: document server (task 4) and mobile (task 3)

Status: **design only**. Nothing here has been built or run. Written 2026-10-15 from a static read of
`upstream/` (HEADs listed in `docs/STATUS.md`), the lockfiles, the crate sources already extracted in
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`, and the external docs cited inline.
Paths like `wordcraft/...` are relative to `upstream/`. Anything not backed by a file:line, a
lockfile version or a URL is marked **UNVERIFIED**.

Shared rules for both spikes:
- Build output goes to `E:/office-target/<name>`. Use `-j 2` while another build or the emulator is
  running (11.8 GB RAM). Never run the emulator during a wordcraft release build.
- Env vars on this machine: `PATH="$HOME/.cargo/bin:/e/office-tools/cargo/bin:$PATH"`,
  `CARGO_HTTP_CHECK_REVOKE=false`, `CARGO_NET_GIT_FETCH_WITH_CLI=true`. Trunk is already at
  `E:/office-tools/cargo/bin/trunk.exe`. See `E:/office-target/run.sh`.
- Never edit `upstream/<repo>` in place, because the inventory builds use it. Upstream patches go in a
  git worktree, for example `git -C upstream/wordcraft worktree add E:/office-src/wordcraft-spike4 -b spike4-host-bridge`,
  and are exported as `.patch` files.

---

## Spike 4: document server (run first)

### 4.0 Goal and definition of done
The flow: the HRM page GETs a **signed config**, the embedded web editor loads the document from
`document.url`, the user types, and the document server POSTs to `editorConfig.callbackUrl` with a
JWT. The fake HRM verifies the JWT, fetches the saved `.docx` and finds the typed marker in it. A
Playwright run proves the whole flow, plus three negative tests: tampered token, SSRF, and a wrong
callback secret.

### 4.1 Findings that shape the design (verified by reading the code)

| Fact | Evidence |
|---|---|
| wordcraft-web opens files only through `Services::inbox`. A `(name, bytes)` pushed there runs `file.open {path:name, data:base64}` | `wordcraft/crates/ui-egui/src/lib.rs:2111-2145` (`drain_inbox`). The engine sets `s.path = name` at `wordcraft/crates/engine/src/cmd/file.rs:234` |
| Save on web means `file.save` turns into `Services::download(name, bytes)`. The bytes come from `wordcraft_engine::io::save_bytes_with(&name, &doc, password, &opts)` | `wordcraft/crates/ui-egui/src/lib.rs:842-891` (inside `WordApp::execute`, which starts at :811). Engine API at `wordcraft/crates/engine/src/io.rs:328`, with `save_bytes` at :305 |
| The save name is `<title stem>.docx`. A `.docx` save marks the document clean (`keeps_everything`) | `lib.rs:1658-1664`, `lib.rs:876-882`, `lib.rs:2268-2271` |
| The web shell wires `download`, `inbox` and `on_dirty` in `services()`. `app.services` is a pub field, so it can be overridden after `WordApp::new` | `wordcraft/apps/wordcraft-web/src/web.rs:178-220`, `web.rs:64-66`, `lib.rs:423` |
| `download` uses `window.showSaveFilePicker`, which is **looked up at save time**, and falls back to an anchor download | `web.rs:347-367`, `web.rs:369-445` (`Reflect::get(&window,"showSaveFilePicker")` at :354) |
| An async save picker can report its result later through `AsyncSaves` (`start`/`finish`) | `wordcraft/crates/ui-egui/src/async_saves.rs:52,63`, `web.rs:415-441` |
| wordcraft-web has no `?url=` and no host bridge. Its only query flags are `?webgl` and `?sample` | `web.rs:36,59,85-87`, `main.rs:9` |
| gridcraft's bridge: `gridcraft:ready/open/save/dirty/failed`, same-origin parent only, replaces `services.download` | `gridcraft/apps/gridcraft-web/src/host.rs:1-166`, wired in `gridcraft/apps/gridcraft-web/src/web.rs:30,64,95` |
| pdfcraft: `?file=<url>` fetch and `pdfcraft:open/save/dirty` | `pdfcraft/apps/pdfcraft-web/src/main.rs:30-53, 57-73, 109-120, 122-246` |
| eframe's drop handler is on the canvas and reads `dataTransfer.files`. It does not check `isTrusted` | `eframe-0.36.2/src/web/events.rs:106, 976-1000` |
| The `?webgl` flag forces WebGL2. This matters for headless or iGPU Chromium | `web.rs:36-40` |

### 4.2 Architecture

```
HRM page  http://127.0.0.1:8790  (fake_hrm bin)
  │ 1. GET /config/sample.docx  → {document:{key,url,…}, editorConfig:{callbackUrl,…}, token}
  │ 2. <script src="http://localhost:8780/api.js">; new DocsAPI.DocEditor("ph", config)
  ▼
iframe  http://localhost:8780/editor          (docserver; "wrapper", our JS, ~150 lines)
  │ 3. postMessage(config) from api.js → wrapper → POST /api/session {config}
  │    server: verify HS256 token → SSRF-checked GET document.url → store → {sid,name}
  │ 4. wrapper GET /api/session/{sid}/document (same origin, no CORS)
  ▼
iframe  http://localhost:8780/wc/index.html?host=parent&webgl   (wordcraft-web dist, same origin)
  │ 5. wrapper → editor: open(name, bytes)      [adapter: "shim" or "bridge", §4.4]
  │ 6. user types; Ctrl+S (or wrapper requestSave)
  │ 7. editor → wrapper: save(name, bytes) → POST /api/session/{sid}/save (raw body)
  ▼
docserver 8. store version → POST callbackUrl  {key,status:6,url,filetype,users,forcesavetype:1}
             body.token = JWT(body); header Authorization: Bearer JWT({payload: body})
fake_hrm  9. verify Bearer JWT (HS256, shared secret) → GET url → save .docx → reply {"error":0}
```

The browser never fetches the HRM document URL. The server proxies it, so arbitrary document hosts
need no CORS. The wordcraft-web dist is served from the docserver origin, so both host bridges (which
accept only a same-origin parent, `gridcraft/.../host.rs:13,116`) work unchanged.

The ONLYOFFICE shapes mirrored here come from public API docs, read clean-room:
- Config token: the JWT payload has the config's structure, and `document.key`, `document.url`,
  `document.permissions`, `editorConfig.callbackUrl` and `editorConfig.mode` must be signed. The
  token sits in `config.token`. Source: https://api.onlyoffice.com/docs/docs-api/additional-api/signature/browser/
- Callback body fields: `key`, `status` (2 = ready for saving, 4 = closed without changes, 6 = force
  save), `url`, `users`, `filetype`, `forcesavetype`. The required response is `{"error":0}`.
  Source: https://api.onlyoffice.com/docs/docs-api/usage-api/callback-handler/
- Outgoing token: `Authorization: Bearer <JWT>`, whose payload is `{"payload": <body>}`. HS256.
  Sources: https://api.onlyoffice.com/docs/docs-api/additional-api/signature/request/token-in-header/
  and https://api.onlyoffice.com/docs/docs-api/additional-api/signature/
- Decision: the spike sends the ONLYOFFICE shape, a JSON body with `url` pointing at the saved file
  (`GET /files/{sid}/{ver}?t=<short-lived JWT>`), and does not POST the bytes inline. That keeps
  the HRM-side change at about zero. The fake HRM downloads the file inside its callback handler
  before it replies, so "the saved .docx reached the callback receiver" is still proven. An
  inline-multipart variant is a 10-line change if the owner prefers it.

### 4.3 The `spikes/docserver` crate

The crate has its own `[workspace]`, so it never joins other workspaces. `CARGO_TARGET_DIR=E:/office-target/docserver`.

```
spikes/docserver/
  Cargo.toml                 # [[bin]] docserver (src/main.rs), [[bin]] fake_hrm (src/bin/fake_hrm.rs)
  src/main.rs                # router, state, env config
  src/jwt.rs                 # sign/verify HS256 + tests
  src/config.rs              # serde types for the DocsAPI subset
  src/fetch.rs               # SSRF-guarded GET (allowlist, no redirects, size+time limits)
  src/session.rs             # in-memory sessions + versions on disk (E:/office-target/docserver/data)
  src/callback.rs            # build + sign + POST callback, await {"error":0}
  src/bin/fake_hrm.rs        # fake HRM: page, files, signed config, callback receiver
  static/api.js              # DocsAPI.DocEditor shim (creates iframe, posts config, relays events)
  static/editor.html         # wrapper: session, fetch doc, adapters "shim" | "bridge", save upload
  hrm/index.html             # fake HRM page (served by fake_hrm)
  fixtures/sample.docx       # generated in step S1
  scripts/smoke.sh           # curl-only server flow (no browser)
  e2e/package.json, e2e/spike4.spec.ts, e2e/verify_docx.py
  patches/0001-wordcraft-web-host-bridge.patch     # from step S8; later moved to /patches register
```

**Dependencies.** Each is permissive.

| Crate | Version | Licence | Why |
|---|---|---|---|
| axum | 0.8 (0.8.3 is already in the local registry) | MIT | HTTP server |
| tokio | 1 (1.53.x in all four lockfiles) | MIT | runtime: `rt-multi-thread, macros, net, fs` |
| tower-http | 0.6 (0.6.2 local; 0.6.11 in the pdfcraft lock) | MIT | `ServeDir` for `/wc` and `/static`, plus `trace`, `limit` and `cors` (unused) |
| reqwest | 0.12 (0.12.15 local) | MIT/Apache-2.0 | `default-features=false, features=["json"]`. HTTP only for the spike, so no TLS C build. Phase 5 needs TLS: native-tls (schannel) or rustls with ring, **UNVERIFIED** on windows-gnu |
| jsonwebtoken | 11 (11.1.0 per https://docs.rs/crate/jsonwebtoken/latest/features) | MIT (https://docs.rs/jsonwebtoken/latest/jsonwebtoken/) | `default-features=false, features=["rust_crypto"]`. The README says at most one backend (`aws_lc_rs` or `rust_crypto`) must be selected (https://github.com/Keats/jsonwebtoken/blob/master/README.md). Pick `rust_crypto` because `aws_lc_rs` needs a C/CMake build, which is risky on windows-gnu (**UNVERIFIED**) |
| serde, serde_json, uuid (`v4`), url, sha2, base64, tracing, tracing-subscriber | as in the lockfiles | MIT/Apache-2.0 | |

Licence caveat: jsonwebtoken's `rust_crypto` feature pulls in `rsa ^0.9`
(https://docs.rs/crate/jsonwebtoken/latest/features). `rsa 0.9` has an open RustSec advisory
(Marvin timing attack, RUSTSEC-2023-0071, **UNVERIFIED** current status) that `cargo-deny` would
flag. That is fine for a spike. For Phase 5, either provide a custom `CryptoProvider` or hand-roll
HS256 with `hmac 0.13` + `sha2 0.11` + `base64 0.22`. All three are already in the wordcraft
lockfile, and the code is about 60 lines.

**Environment.** `DOCSERVER_ADDR=127.0.0.1:8780`, `DOCSERVER_PUBLIC_URL=http://localhost:8780`,
`DOCSERVER_JWT_SECRET` (shared with the HRM; the spike uses one secret, where ONLYOFFICE allows
browser, inbox and outbox secrets), `DOCSERVER_ALLOWED_HOSTS=127.0.0.1:8790` (used for both the
document URL and the callback URL), `WORDCRAFT_WEB_DIST=<dir>`, `DOCSERVER_MAX_BYTES=104857600`.

**Config JSON.** This is the subset the spike reads. Everything else passes through and is ignored.
```json
{
  "documentType": "word",
  "document": { "fileType": "docx", "key": "hrm-42-v1", "title": "sample.docx",
                "url": "http://127.0.0.1:8790/files/sample.docx",
                "permissions": { "edit": true, "download": true } },
  "editorConfig": { "callbackUrl": "http://127.0.0.1:8790/callback?doc=42",
                    "mode": "edit", "lang": "en", "user": { "id": "u1", "name": "Test User" } },
  "token": "<HS256 JWT whose payload is the object above without 'token'>"
}
```
Rule: the server trusts **only the verified token payload** and never the unsigned outer fields. That
is the ONLYOFFICE semantics. The validation is `Validation::new(Algorithm::HS256)`, so `none` and
other algorithms are rejected. `exp` is optional, as in ONLYOFFICE, and is checked when present.
`mode != "edit"` or `permissions.edit == false` sets a read-only flag, which the spike only logs.
Enforcing it is Phase 5.

**Docserver endpoints**

| Method + path | Behaviour | Done-check |
|---|---|---|
| `GET /api.js` | Defines `window.DocsAPI.DocEditor(placeholderId, config)`. It creates `<iframe src="{public}/editor">` and waits for `{type:"docs:ready"}` from that iframe's origin. It then posts `{type:"docs:config", config}` with `targetOrigin={public}` and relays `docs:event` messages to `config.events.onDocumentReady / onDocumentStateChange / onError / onSaved` (names mirror DocsAPI events; only these four) | HRM page shows the iframe |
| `GET /editor` | Serves `static/editor.html` | |
| `GET /wc/*` | `ServeDir(WORDCRAFT_WEB_DIST)`. MIME types come from extensions (`.wasm` → `application/wasm`, **UNVERIFIED** for tower-http's mime_guess version) | `curl -I /wc/<hash>.wasm` shows the right type |
| `POST /api/session` | Body `{config}`. Steps: verify the token (401 on failure); SSRF-check and fetch `document.url` (403 if blocked, 502 on fetch error); store version 0; reply `{sid, name, mode}`. `sid` = UUIDv4 (a capability for the spike; Phase 5 uses a session JWT bound to the user) | `smoke.sh` |
| `GET /api/session/{sid}/document` | Returns the latest version's bytes (`application/vnd.openxmlformats-officedocument.wordprocessingml.document`) | |
| `POST /api/session/{sid}/save` | Raw body (limit `DOCSERVER_MAX_BYTES`), header `X-File-Name`. Check the `PK\x03\x04` zip magic, store version n+1, build the callback, POST it and await `{"error":0}` (10 s timeout). Reply `{version, callback:"ok"|"failed", detail}` | `smoke.sh` shows a callback line at the fake HRM |
| `GET /files/{sid}/{ver}?t=<jwt>` | `t` = HS256 JWT `{sid, ver, exp: now+600}` | The fake HRM's download succeeds; a tampered `t` gives 401 |
| `POST /api/session/{sid}/close` | Callback with status 2 if any version was saved, otherwise 4 (optional, S9) | |
| `GET /healthz` | `ok` | |

**SSRF guard** (`fetch.rs`; the same check applies to `callbackUrl`):
- the scheme must be `http` or `https`;
- `host:port` must be in `DOCSERVER_ALLOWED_HOSTS` (exact match);
- the response body is capped at `MAX_BYTES` and the request times out after 30 s;
- `reqwest` uses `redirect::Policy::none()`, so a 3xx is an error.

Phase 5 adds DNS resolution with private, loopback and link-local rejection unless allowlisted,
pinning the resolved IP for the connection, and audit logging.

**fake_hrm endpoints** (port 8790):
- `GET /` serves `hrm/index.html`. It fetches `/config/sample.docx`, then runs `new DocsAPI.DocEditor("ph", cfg)`.
- `GET /files/{name}` serves the fixtures.
- `GET /config/{name}` builds the config above with `key = "<name>-<unix>"` and signs it.
- `POST /callback` verifies `Authorization: Bearer`. It must be HS256 with the shared secret and its `payload.key` must equal `body.key`, otherwise the reply is 401 `{"error":1}`. On status 2 or 6 it GETs `body.url`, writes `E:/office-target/docserver/received/{key}-{n}.docx`, appends a JSON line `{ts,key,status,bytes,sha256,file}` to `received.jsonl` and replies `{"error":0}`.
- `GET /received` returns the jsonl for the test harness.

### 4.4 Getting bytes into and out of the wasm app: options compared

**(b) Zero-patch "shim" adapter.** It works with the **stock** dist the inventory is already
building, so there is no extra wasm build.
- *Open.* The wrapper is a same-origin parent, so it can reach `iframe.contentWindow`. Once the inner page's `#wordcraft_loading` element is removed (`web.rs:76-78`, which marks start-up done), the wrapper builds `new w.DataTransfer()`, adds `new w.File([bytes], name)` and dispatches `new w.DragEvent('drop', {dataTransfer, bubbles:true, cancelable:true})` on `#wordcraft_canvas`. The path from there: `events.rs:976-990` → `raw.dropped_files` → `WebShell::logic` reads the bytes (`web.rs:99-113`) → inbox → `file.open`.
- *Save.* Before the user saves, the wrapper sets `w.showSaveFilePicker = async () => ({createWritable: async () => ({write: async d => chunks.push(d), close: async () => upload(chunks)})})`. wordcraft calls it synchronously on save (`web.rs:354-357, 407`), writes a `Uint8Array` (`web.rs:460-462`) and awaits `close()`. A resolved `close` becomes `SaveOutcome::Saved`; a rejected `close` (callback failed) becomes `SaveOutcome::Failed`, and the app reports the error itself (`web.rs:418-441`). The save semantics are therefore correct with zero Rust changes.
- *Dirty state* cannot be observed, because it reaches only the `beforeunload` guard (`web.rs:211, 331-345`). A host-initiated save is not possible.
- *Size and risk.* About 60 lines of JS. It relies on internals (DOM ids, the picker lookup, the drop handler), and upstream moves at about 100 commits a day. **Not a contract and not upstreamable.** Use it to prove the server flow on day 1.

**(a) Host-bridge patch to wordcraft-web.** This is the durable, upstreamable path. It is modelled on
gridcraft's `host.rs` and keeps the same message semantics with a `wordcraft:` prefix:
`wordcraft:ready {version}`, `wordcraft:open {name, bytes}`, `wordcraft:save {name, bytes}` (with a
transferred `ArrayBuffer`), `wordcraft:dirty {dirty}`, `wordcraft:failed {error}`, plus one addition,
`wordcraft:requestSave`. The addition lets a host trigger a forced save, as DocsAPI hosts expect.

| File:line (upstream HEAD fd0bb2f) | Change | ~Lines |
|---|---|---|
| `apps/wordcraft-web/src/host.rs` (new) | Adapted copy of `gridcraft/apps/gridcraft-web/src/host.rs`. `Bridge::new` is unchanged. `connect(&mut WordApp, Inbox, egui::Context)` sets `app.services.download` to `post_save`, chains `app.services.on_dirty` (keeping the original closure from `web.rs:211` for the unload guard) and listens for `open` and `requestSave`. `requestSave` sets an `Rc<Cell<bool>>`. `sync(&mut WordApp)` runs in `logic` and, if the flag is set, calls `app.run("file.save", json!({}))` | 150 |
| `web.rs:15` | `mod host;` | 1 |
| `web.rs:85-87` | Add `fn query_param(key)` using `UrlSearchParams` (as `pdfcraft/.../main.rs:103-107`); keep `query()` | 5 |
| `web.rs:51-72` (app creator closure) | `let bridge = (query_param("host")=="parent").then(host::Bridge::new).flatten();` After `WordApp::new` (line 64): `if let Some(b)=&bridge { b.connect(&mut app, inbox.clone(), cc.egui_ctx.clone()) }`. Skip `app.start_screen()` (line 69-71) in host mode | 8 |
| `web.rs:76-81` | On `Ok`, `bridge.post_ready()`; on `Err`, `bridge.post_failed(..)`. The bridge has to be created outside the closure or created again, as gridcraft does at `web.rs:30` | 6 |
| `web.rs:91-119` (`WebShell`) | New field `bridge: Option<host::Bridge>`; call `bridge.sync(&mut self.app)` after `self.app.logic(ctx)` (line 118) | 5 |
| `apps/wordcraft-web/Cargo.toml:32` | Add `"MessageEvent", "UrlSearchParams"` to the web-sys features (gridcraft has both: `gridcraft/apps/gridcraft-web/Cargo.toml:29`) | 1 |
| **Total** | | **≈175** |

**Upstreamable: yes.** It is the third copy of an existing upstream pattern (pdfcraft and gridcraft,
both done for Nextcloud embedding). It is opt-in behind `?host=parent`, adds no new dependencies and
has no effect without a parent. It counts as `patches/` entry #1.

Optional (Phase 5, about 25 more lines): make the bridge's save asynchronous with
`AsyncSaves::start()` before posting and `finish(ticket, Saved|Failed)` on a `wordcraft:saved {ok}`
reply from the parent. Then a failed HRM callback leaves the document dirty, as `async_saves.rs:52,63`
and `lib.rs:884-889` already handle for the browser picker.

**(a2) `?url=` fetch inside wasm**, as pdfcraft does at `main.rs:30-53` (about 30 lines). It needs
CORS on the document host or a same-origin proxy, and it still needs a save path. The bridge
dominates it, so drop this option.

**Recommendation.** Run S5–S7 with the shim on the stock build to prove the server, JWT and SSRF parts
fast. Then build the bridge patch (S8) and re-run the same Playwright test with `?adapter=bridge`. The
bridge is the result we carry into Phase 5. `editor.html` implements both adapters behind one
interface: `open(name, bytes)`, an `onSave(name, bytes) → Promise` hook, an `onDirty(bool)` hook and
`requestSave()`. `requestSave` is a no-op in shim mode.

### 4.5 Test harness
- **Fixture.** Generate it with the CLI the inventory builds: `wordcraft-cli run --cmd 'text.insert={"text":"HRM fixture paragraph"}' --save fixtures/sample.docx`. The `run` subcommand is at `apps/wordcraft-cli/src/main.rs:15-54`, per the layer map. If the CLI is not built yet, use a 30-line Python `zipfile` script that writes a minimal OOXML package.
- **Playwright.** Use `e2e/` with `@playwright/test` pinned to an exact version and `npx playwright install chromium`. Run **headed** Chromium: the Intel UHD 630 works through ANGLE, while headless needs SwiftShader for WebGL2, which current Chromium only allows with `--enable-unsafe-swiftshader` (**UNVERIFIED**). Always use the `&webgl` flag on the inner URL.
  1. Start `fake_hrm` and `docserver` (Playwright `webServer` entries with `cargo run --release --bin …`).
  2. `page.goto('http://127.0.0.1:8790/')`. Wait for `document.body.dataset.state === 'ready'`, which the HRM page sets on `onDocumentReady`. The wrapper raises that event once the document is open: in bridge mode after it posts `open`; in shim mode after the drop plus one `requestAnimationFrame` round. Timeout 120 s, because the wasm is 37.9 MB.
  3. `const ed = page.frameLocator('#ph iframe').frameLocator('#wc')`, then `ed.locator('#wordcraft_canvas').click({position:{x:400,y:300}})`.
  4. `page.keyboard.press('Control+End')`, then `page.keyboard.type(' SPIKE4-' + Date.now() + ' Салом')`. The brief observed that keyboard typing reaches the egui canvas. Cyrillic goes through `insertText`, **UNVERIFIED**.
  5. Save. Bridge mode: post `requestSave` from the HRM page (deterministic). Shim mode: `Control+S`. If eframe does not `preventDefault` it, Chromium may open "Save page as" (**UNVERIFIED**); the fallback is clicking File › Save.
  6. Poll `GET http://127.0.0.1:8790/received` until a new line with `status:6` appears (timeout 30 s).
  7. Run `python -I e2e/verify_docx.py <file> <marker>`. It unzips `word/document.xml`, strips tags with a regex and asserts the marker and `Салом` are present. Stripping tags handles markers split across `<w:r>` runs. Cross-check with `E:/office-target/wordcraft/release/wordcraft-cli.exe text <file>` if that binary exists.
  8. Save screenshots to `docs/phase0/spike4/` (before edit, after edit, and an HRM page "saved" banner).
- **Negative tests** (the same spec file):
  - (i) Flip one character of `config.token`: `POST /api/session` returns 401 and the HRM gets `onError`.
  - (ii) `document.url = http://169.254.169.254/latest` (signed correctly): 403 "host not allowed".
  - (iii) Start docserver with a different `DOCSERVER_JWT_SECRET` than the HRM's verifier. The HRM rejects the callback with 401, the save endpoint returns `callback:"failed"` and the wrapper raises `onError`.
- **Measurements to record:**
  - time from `goto` to `ready`;
  - wasm bytes transferred (Playwright `response.body().length`);
  - save → callback-received latency;
  - size of the saved docx versus the fixture;
  - 3 consecutive green runs.

### 4.6 Step list for a sonnet-level implementer

| # | Step | Done-check |
|---|---|---|
| S0 | Wait for the inventory's `wordcraft web` build (`E:/office-target/logs/wordcraft-web.result`). Copy `upstream/wordcraft/dist/web` → `E:/office-target/spike4/wc-stock/`. `cd spikes/docserver/e2e && npm i -D @playwright/test@<exact> && npx playwright install chromium` | `index.html` and a `*.wasm` exist; `npx playwright --version` prints |
| S1 | Create the fixture docx (CLI or Python) | `verify_docx.py fixtures/sample.docx "HRM fixture"` exits 0 |
| S2 | Crate skeleton plus `jwt.rs` (sign and verify a config, sign and verify `{payload}`) | `cargo test` (target `E:/office-target/docserver`, `-j 2`): 4 tests pass (round-trip, tampered, alg=none rejected, wrong secret) |
| S3 | `fake_hrm` bin | `curl :8790/config/sample.docx` returns JSON with a 3-part `token`; `curl :8790/files/sample.docx` returns the fixture's bytes |
| S4 | `docserver`: session, document, save, files, callback and SSRF, without the browser | `scripts/smoke.sh` does session → document (sha equal to the fixture) → save (fixture bytes) → `received.jsonl` gains a line with `status:6` and the same sha256 |
| S5 | `api.js`, `editor.html` (shim adapter), `hrm/index.html` | Manual: the HRM page shows the editor with the fixture text; Ctrl+S adds a jsonl line |
| S6 | `spike4.spec.ts` happy path, adapter=shim | Green 3× in a row; screenshots saved; timings logged |
| S7 | Negative tests (i)–(iii) | 3 tests green |
| S8 | Host-bridge patch in the worktree (§4.4 table). Build with `CARGO_TARGET_DIR=E:/office-target/wordcraft trunk build --release --dist E:/office-target/spike4/wc-bridge` (shares the dependency cache; cargo's lock serialises it with the inventory) | wasm builds; `git diff --stat` ≈ 175 lines; patch exported to `spikes/docserver/patches/` |
| S9 | Bridge adapter in `editor.html`: `WORDCRAFT_WEB_DIST` points at wc-bridge, the HRM triggers `requestSave`, `onDocumentStateChange` fires on typing, `/close` sends status 2 | Spec green with `adapter=bridge` 3×; the dirty event is seen in the HRM log |
| S10 | Record the numbers in `docs/STATUS.md`. Write a 1-page results note with screenshots and the ONLYOFFICE-field compatibility list (supported: `document.{key,url,title,fileType}`, `editorConfig.{callbackUrl,mode,user}`, `token`, `events.{onDocumentReady,onDocumentStateChange,onError}`; everything else not supported) | Files exist |

**Effort:** S0–S7 take about 1.5–2 days. S8–S9 take about 1 day, mostly the wasm release build of
wordcraft-web plus the debug loop. **Total about 2.5–3 days.**

**Risks:**
1. A headed Chromium with a 38 MB wasm alongside builds risks RAM pressure. Run the tests only when no cargo build is running.
2. The Ctrl+S behaviour in Chromium (see step 5); bridge mode avoids it.
3. If the inventory's web build fails on windows-gnu, S5 is blocked. A fallback is a Linux Docker build of the dist (**UNVERIFIED**).
4. Typing fidelity: egui ignores `ImeEvent::DeleteSurrounding` (see §3A.3), which does not matter for Playwright typing.
5. `document.key` semantics (one key per document version) are not enforced in the spike.

---

## Spike 3: mobile (approaches A and B), Android only

macOS is not available, so **iOS is not tested**. The emulator is x86_64 (`abi.type=x86_64` in both
AVD `config.ini` files), so the Rust target is `x86_64-linux-android`. Use **HRM_Pixel_8**
(`system-images/android-36/google_apis_playstore/x86_64`), which has the Play Store and therefore
Gboard. HRM_Dev_33 is a Google APIs image; Gboard is **UNVERIFIED** there. Both AVDs declare
`hw.lcd.density=420` and 1080×2400 px, which is about 411×914 dp, and both list
`hw.keyboard`, `hw.gpu.enabled` and `hw.ramSize` twice in `config.ini`. Which value wins is
**UNVERIFIED**, so check in the emulator UI. Installed platforms are only `android-36` and
`android-36.1`, so use compileSdk 36. Build-tools 35–37 are present.

### 3A. egui everywhere on Android

#### 3A.1 Versions and Android support (from `upstream/wordcraft/Cargo.lock`)
eframe **0.36.2**, egui-winit 0.36.2, winit **0.30.13**, wgpu **30.0.1**, glow 0.17.0,
android-activity **0.6.1**, ndk 0.9.0, jni 0.22.4.

- eframe 0.36.2 supports Android through the `android-game-activity` and `android-native-activity` features (`eframe-0.36.2/Cargo.toml:58-59`). The app passes `NativeOptions::android_app` (`src/epi.rs:401-402`), which is required (`src/native/run.rs:37-41`).
- Both renderers handle suspend and resume:
  - wgpu: `src/native/wgpu_integration.rs:177-198, 449-486`;
  - glow: `src/native/glow_integration.rs:98-102, 1369-1383`.
- **`accesskit` must be off with NativeActivity.** It is a `compile_error!` at `eframe-0.36.2/src/lib.rs:146-153`.
- Renderer coupling: none. ui-egui uses only `load_texture`/`TextureOptions` (`wordcraft/crates/ui-egui/src/canvas.rs:1046-1147`) and has no wgpu, glow or paint callbacks (grep), so **both glow and wgpu are viable**. Build with both features and pick at runtime (`NativeOptions::renderer`). Try wgpu first: it uses Vulkan, or GLES if forced, the way `apps/wordcraft/src/graphics.rs:58-66` forces backends. Fall back to glow if wgpu fails on the emulator's host or SwiftShader GPU (**UNVERIFIED** which one works).
- **Activity choice decides text input.** android-activity's README says NativeActivity has "no built-in input method support (can only receive physical key events from soft keyboards that typically only allows basic ascii input)", and GameActivity "has built in support for input methods via the GameTextInput library" (`android-activity-0.6.1/README.md:180-222`). It needs `androidx.games:games-activity:4.4.0` (`README.md:211, 226-236`). NativeActivity's `set_text_input_state` is a no-op (`src/native_activity/mod.rs:434-446`). GameActivity emits `InputEvent::TextEvent` (`src/game_activity/mod.rs:1084-1091`) and supports polling with `AndroidApp::text_input_state()` (`src/lib.rs:1088-1094`, `game_activity/mod.rs:745-748`).
- **winit 0.30 drops IME on Android.** `set_ime_allowed` only shows or hides the soft keyboard; `set_ime_cursor_area` and `set_ime_purpose` are no-ops; `WindowEvent::Ime` is never emitted; `InputEvent::TextEvent` falls through to a warning. Source: https://github.com/rust-windowing/winit/blob/v0.30.x/src/platform_impl/android/mod.rs.
- egui-winit 0.36 toggles `set_ime_allowed` from `PlatformOutput::ime` and has no Android-specific code. Touch is forwarded as `Event::Touch`, and the first touch also emulates the mouse pointer. Source: https://raw.githubusercontent.com/emilk/egui/0.36.0/crates/egui-winit/src/lib.rs.
- **Prediction:** with stock eframe, Gboard shows when the page is focused (wordcraft sets `IMEOutput` at `canvas.rs:1380-1385`), but Cyrillic, autocorrect and swipe typing do not reach the document. At most, ASCII key events arrive. That is exactly what the kill test (§3.C) checks.
- **Workaround inside our shell crate, no upstream patch** (about 150–250 lines, **UNVERIFIED** that it works): keep a clone of `AndroidApp`. Each frame, poll `text_input_state()` (GameActivity), diff it against the previous state and inject events in `eframe::App::raw_input_hook`:
  - `egui::Event::Ime(Preedit{..})` for the compose region;
  - `Ime(Commit)` for committed text;
  - `Key{Backspace}` for deletions.

  wordcraft consumes Preedit and Commit at `crates/ui-egui/src/keys.rs:324-330`. Reset the state with `set_text_input_state` after a commit. Risk: Gboard autocorrect needs surrounding text, which this simple shim does not provide.

#### 3A.2 Packaging tool (windows-gnu host)

| Tool | Status | Fit |
|---|---|---|
| cargo-apk | README describes it for apps with a `NativeActivity` only (https://github.com/rust-mobile/cargo-apk). The brief calls it deprecated; the fetched README showed no notice (**UNVERIFIED**) | ✗: NativeActivity means no IME |
| xbuild | GitHub "About" says "(unmaintained)" (https://github.com/rust-mobile/xbuild); no Windows host or GameActivity docs | ✗ |
| cargo-mobile2 | Tauri's generator. Gradle-based, heavier (**UNVERIFIED** on gnu) | Possible, but overkill for A |
| **cargo-ndk + Gradle** | MIT/Apache-2.0; Windows is a supported host; `-t`/`-o` produce `jniLibs` (https://github.com/bbqsrc/cargo-ndk). The official `agdk-eframe` example uses exactly `cargo ndk -t … -o app/src/main/jniLibs build` plus `./gradlew installDebug` (https://github.com/rust-mobile/rust-android-examples/tree/main/agdk-eframe). That template predates eframe 0.36 and must be updated | **✓ Recommended.** GameActivity needs Java/Gradle anyway |

Cross-compiling looks low-risk: the wordcraft lock's only `-sys` crates are `jni-sys`, `ndk-sys`,
`wayland-sys`, `windows-sys` and others that are not C builds (grep of `Cargo.lock`). The NDK clang
is the target linker, and the host gnu toolchain only builds build scripts and proc-macros.

#### 3A.3 What in wordcraft must change for Android (file:line)
Do **not** build `apps/wordcraft`:
- its `cfg(all(unix, not(target_os="macos")))` dependencies (winit `x11`, `zbus`, `wl-clipboard-rs`, at `apps/wordcraft/Cargo.toml:48-57`) would apply to Android;
- `rfd`, `arboard` and `craft-print` are pulled in for all non-wasm targets (`:71-80`);
- Android support in rfd 0.17.2 and arboard 3.6.1 is **UNVERIFIED** and likely absent.

Instead, a new shell crate `spikes/mobile-a/wordcraft-android` (a cdylib exporting `android_main`)
mirrors `apps/wordcraft-web/src/web.rs`. It depends on `wordcraft-ui-egui`, `wordcraft-engine`,
`wordcraft-doc` and `wordcraft-fonts` by path. Their dependencies are Android-clean:
- `zbus` only for linux, at `crates/ui-egui/Cargo.toml:31-32`;
- `craft-ui` needs only egui, at `craft-libs/crates/ui/Cargo.toml`.

| Area | Where | Change (spike = in the shell crate, no upstream patch, unless noted) |
|---|---|---|
| Fonts | `crates/fonts/src/fontdb.rs:706-738`: on Android, `system_font_dirs` falls into the Linux branch (`/usr/share/fonts`) | Shell: read `/system/fonts/*.ttf` and call `FontDb::global().install_font(name, bytes)`, as `web.rs:305` does. Upstream patch later: add `/system/fonts` for `target_os="android"` (3 lines). The bundled Source Sans/Serif and Inter fonts (`fontdb.rs:28-37`) cover Latin and Cyrillic (**UNVERIFIED** for Cyrillic) |
| Settings, recovery | `crates/control-key/src/lib.rs:10-21` uses `HOME`/`XDG_CONFIG_HOME`; `fontdb.rs:711`, `proof/src/hunspell.rs:56`, `engine/src/cmd/options.rs:241` | Shell: `set_var("HOME"/"XDG_CONFIG_HOME", app.internal_data_path())` before start (needs `unsafe` in edition 2024; allowed in the spike crate) |
| File open/save | Services `pick_open`/`open_async`/`download` (`ui-egui/src/lib.rs:101-128`) | Shell: on start, `file.open {path}` for `<external_data_path>/test.docx` (pushed with `adb push … /sdcard/Android/data/<pkg>/files/`). `file.save` writes the same path through the engine. No pickers in the spike |
| Text input / IME | `canvas.rs:1380-1385` (IMEOutput) and `keys.rs:313-330` (Text/Preedit/Commit). `ImeEvent::DeleteSurrounding` is **not handled** anywhere in ui-egui (grep), but egui's web text agent emits it for corrections (`eframe-0.36.2/src/web/text_agent.rs:311-318`) | Shell: GameActivity text-state shim (§3A.1). Upstream patch candidate (about 10 lines in `keys.rs`): treat `DeleteSurrounding{before_chars}` as `text.backspace`×n. That fixes autocorrect on mobile **web** too (**UNVERIFIED**) |
| Touch scroll | The page area is a `ScrollArea::both()` with wheel input disabled (`canvas.rs:1007-1012`). Mouse-emulated touch drags go to selection (`canvas.rs:1496-1529, 2175-2233`) | Expect swipe to select, not scroll (matches the brief's mobile-web finding). Shell "touch adapter" in `raw_input_hook`: turn one-finger moves into `Event::MouseWheel{unit:Point}` (wordcraft scrolls these 1:1 with momentum, `scroll.rs:57-145, 167-210`) and keep a short tap as a click. About 100 lines, and it would fix mobile web too |
| Pinch zoom | `scroll.rs:249-253` (`multi_touch` → `zoom_delta`), applied at `canvas.rs:855-863` only if `rect_contains_pointer` | Expected to work. Test |
| Selection handles, long-press menu | None in ui-egui (no code found) | Expected **FAIL**. This is new UI work |
| DPI / layout width | `lib.rs:1787-1795` uses `native_pixels_per_point` (420 dpi gives 2.625). Desktop minimum is 760×480 pt (`apps/wordcraft/src/main.rs:377`) versus about 411 pt portrait | Expect the ribbon to overflow in portrait. That is the likely cause of the "wrong scale" on mobile web (**UNVERIFIED**). Test Interface size ≈ 60% (`theme.rs:119-124`) and landscape (914 pt) |
| Rotation | Activity recreation | Manifest `android:configChanges="orientation|screenSize|screenLayout|keyboardHidden|keyboard|density|uiMode"` and `windowSoftInputMode="adjustResize"` |
| Clipboard | egui-winit clipboard on Android (**UNVERIFIED**, likely a no-op) | Out of scope for the spike |

#### 3A.4 Steps (each with a done-check)
| # | Step | Done-check |
|---|---|---|
| A0 | **Mobile-web baseline (zero build).** Serve the inventory's `wordcraft-web` dist (Spike 4's docserver `/wc`, or `trunk serve` on 8771), run `adb reverse tcp:8780 tcp:8780` and open it in **Chrome on HRM_Pixel_8**. Run the Gboard test (A6) | Screen recording + pass/fail table. This shows how far egui's *web* text agent (`text_agent.rs:2, 73-93, 284-343`) gets with Gboard on Android, separating egui-core limits from winit-android limits |
| A1 | Tooling: `rustup target add x86_64-linux-android aarch64-linux-android`; `cargo install cargo-ndk --locked` (into `E:/office-tools/cargo`); Gradle uses JBR (`JAVA_HOME="C:/Program Files/Android/Android Studio/jbr"`); `ANDROID_HOME`, `ANDROID_NDK_HOME=…/ndk/27.1.12297006` | `cargo ndk --version`; `rustup target list --installed` |
| A2 | **IME probe app** `spikes/mobile-a/probe`: eframe 0.36 (`default-features=false`, `android-game-activity`, `wgpu`, `glow`, `default_fonts`; no accesskit) with one multiline `TextEdit` and a raw-event log panel. Gradle project from `agdk-eframe`, updated to games-activity 4.4.0 and compileSdk 36 | APK installs; the app shows the TextEdit on the emulator under wgpu **or** glow (record which works) |
| A3 | Probe + shim: add the GameActivity text-state polling shim | Kill test §3.C run, with and without the shim |
| A4 | wordcraft shell `spikes/mobile-a/wordcraft-android` (structure of `web.rs:19-128`, plus fonts, HOME, file path and touch adapter). `cargo ndk -t x86_64 -P 26 -o android/app/src/main/jniLibs build --release -j 2` with `CARGO_TARGET_DIR=E:/office-target/mobile-a` | `.so` built; record wall time and `.so` size (stripped) |
| A5 | Install and open `test.docx` | Document renders; screenshot to `docs/phase0/mobile/` |
| A6 | Test matrix and recordings (see measurement plan) | Table filled with pass/fail + video file per row |

#### 3A.5 Measurement plan
- **Startup.** `adb shell am force-stop <pkg>`, then `adb shell am start -W -n <pkg>/.MainActivity`, 5 times. Record the median `TotalTime`. Also log `WC_FIRST_FRAME` (first `ui()` call) and `WC_DOC_RENDERED` (first page texture uploaded) and read them with `adb logcat -v epoch`, because `am start -W` ends at the activity's first draw, not egui's.
- **Memory.** `adb shell dumpsys meminfo <pkg>`: record TOTAL PSS, Native Heap and Graphics at three points: blank, after opening a 10-page docx, and after scrolling to the end.
- **Frame time.** Log egui `frame.info().cpu_usage` over a 10 s scroll. Emulator numbers are indicative only; a real mid-range device is Phase 6.
- **Sizes.** APK size and stripped `.so` size.
- **Recording.** `adb shell screenrecord --bit-rate 4000000 --time-limit 120 /sdcard/a-<test>.mp4`, then `adb pull` into `docs/phase0/mobile/`. Screenshots with `adb exec-out screencap -p > x.png`.
- **Gboard test procedure** (HRM_Pixel_8):
  1. Show the soft keyboard although `hw.keyboard=yes`: `adb shell settings put secure show_ime_with_hard_keyboard 1` (**UNVERIFIED** key name), or use Settings › System › Keyboard › Physical keyboard › "Use on-screen keyboard".
  2. Select Gboard: `adb shell ime list -s`, then `adb shell ime set com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME`. Add the Russian keyboard (ЙЦУКЕН), and Uzbek Latin/Cyrillic if Gboard offers them (**UNVERIFIED**), in Gboard › Languages. Glide typing and auto-correction stay at their defaults (on).
  3. **Control run first:** repeat each test in a stock `EditText` (for example Chrome's address bar), so a failure can be attributed to our app and not to the keyboard setup.
  4. Tests: (a) the keyboard appears on tap and hides on Back; (b) Latin `hello world`; (c) Cyrillic `Салом дунё`; (d) Uzbek Latin `o‘g‘il` (apostrophe variants); (e) autocorrect `teh ` → `the `; (f) a glide-typed word; (g) backspace into a committed word; (h) Enter. Type by tapping keys with `adb shell input tap X Y` (coordinates from a screenshot of the keyboard). Glide with `adb shell input motionevent DOWN|MOVE|UP x y` sequences (Android 11+; **UNVERIFIED** that Gboard accepts injected glides), and fall back to a manual mouse glide that is screen-recorded.
  5. Deterministic commit path: the ADBKeyboard test IME (GPL-2.0; a test tool installed only on the emulator, never linked or shipped) with `am broadcast -a ADB_INPUT_B64 --es msg <b64 utf8>` exercises `InputConnection.commitText` with Cyrillic exactly (**UNVERIFIED** package and action names; check its README).
  6. Verify the document: save, `adb pull` the docx and run `verify_docx.py` (from Spike 4) for each string.
- Other rows: selection (long-press, drag), swipe scroll + momentum, pinch zoom, rotation keeps the document and caret, a 360 dp width (`adb shell wm size 1080x2280; wm density 480`, giving 360 dp), and save → reopen.

#### 3A.6 Risks (A)
1. IME: winit 0.30 has no Android IME (verified), so everything rests on our polling shim, which is **UNVERIFIED** and has no surrounding-text context. Autocorrect and glide may still break.
2. The UI is desktop-shaped (ribbon, minimum 760 pt). A mobile toolbar means new egui UI, which means forking ui-egui or building a big parallel UI.
3. There are no selection handles or long-press menu, and egui has no native text-selection affordances.
4. Emulator GPU: wgpu on the x86_64 emulator may fail. The glow fallback exists, but performance numbers will not represent a phone.
5. Build cost: a wordcraft release cross-build on 11.8 GB RAM, plus Gradle, plus the emulator.
6. Accessibility: accesskit on Android requires GameActivity (`eframe-0.36.2/src/lib.rs:146-153`) and is untested.
7. Upstream winit and eframe changes are outside our control.

**Effort (A):** A0 takes 0.5 day, A1–A3 take 1 day, A4–A6 take 1.5–2 days. **About 3–3.5 days.**

### 3B. Hybrid: engine renders pages, the UI lives in a cross-platform shell

#### 3B.1 Engine surface available today (no upstream patch)
- Commands: `Session::run(id, &Value) -> Result<Value, CmdError>` (`wordcraft/crates/engine/src/session.rs:1167`), with undo and rollback in the engine (layer map §2).
- Raster: `render_page(doc, page, scale, opts)` and `render_region(doc, page, w, h, view, opts)` (`crates/render/src/lib.rs:489, 497`). These produce premultiplied RGBA8. vello_cpu threads are 0 on wasm (`render/src/lib.rs:23-27`).
- Overlays: `DocLayout::caret/caret_on` (`crates/layout/src/hit.rs:436-440`) and `selection_rects` (`hit.rs:613`).
- **Pointer → selection is already reachable through commands.** `caret.set` accepts `{"page", "x", "y", "extend"?}` and calls `layout.hit` (`crates/engine/src/cmd/caret.rs:50-51, 268-285`). There are also `select.word`, `select.paragraph` and `select.dragUnit {unit,start,end,pos}` (`caret.rs:55-60`). Tap, drag-extend and double-tap word therefore need no patch. Behaviour that lives only in ui-egui (header/footer double-click, tables, objects; `canvas.rs:1496-1529, 2175-2233`) is out of scope for the spike.
- Typing: `text.insert`, `text.backspace`, `text.delete`, `text.newParagraph` (`crates/engine/src/cmd/text.rs:13-21`).
- Save: `io::save_bytes_with` (`crates/engine/src/io.rs:328`). Open: `io::open_bytes*` (`io.rs:128-164`) or the `file.open {path, data}` command.

**The gap:** the engine has no IME composition. The preedit is a UI string (`ui-egui/src/canvas.rs:86`, `keys.rs:324`). deckcraft's engine shows the model to copy: `compose_text(text, active)` and `commit_text(text)` (`deckcraft/crates/engine/src/keys.rs:231-246`), with composition as one undo step.

For the spike, the shell draws the preedit itself and commits with `text.insert`. Upstream later needs `text.compose {text, active}` and `text.commitComposition`, about 150 lines (**UNVERIFIED** estimate). A binding crate can also read `Session.doc` directly to give the IME surrounding text, so no command is needed for that.

#### 3B.2 The thin binding crate `wordcraft-bind`
It is the same Rust API for every shell. Only the export layer differs.
```rust
pub struct Editor { session: wordcraft_engine::Session }
impl Editor {
  pub fn blank(region) -> Editor;                 pub fn open(name: &str, bytes: &[u8]) -> Result<Editor>;
  pub fn run(&mut self, id: &str, params_json: &str) -> Result<String>;   // Session::run
  pub fn page_count(&self) -> u32;                pub fn page_size_pt(&self, page: u32) -> (f32, f32);
  pub fn render_tile(&self, page: u32, scale: f32, x: u32, y: u32, w: u32, h: u32) -> Rgba; // render_region
  pub fn tap(&mut self, page: u32, x_pt: f32, y_pt: f32, extend: bool);  // run("caret.set", …)
  pub fn caret(&self) -> Option<CaretRect>;       pub fn selection_rects(&self) -> Vec<(u32, Rect)>;
  pub fn text_context(&self, before: usize, after: usize) -> (String, usize); // for the IME mirror
  pub fn insert(&mut self, s: &str); pub fn delete_before(&mut self, n: usize); pub fn delete_after(&mut self, n: usize);
  pub fn save(&self, name: &str) -> Result<Vec<u8>>;   // io::save_bytes_with
}
```
- **Web and Tauri:** `#[wasm_bindgen]` exports (wasm-bindgen 0.2.129 is in the wordcraft lock), built with Trunk, already installed. Whether the exports are reachable from JS through Trunk is **UNVERIFIED**; the fallback is the `wasm-bindgen` CLI pinned to 0.2.129.
- **Flutter:** flutter_rust_bridge v2 with an opaque `Editor` handle. That needs `Session: Send` (**UNVERIFIED**; wrap it in a `Mutex`).
- **Native C ABI** is not needed for the spike.
- **Damage tracking gap:** nothing reports which pages a command changed. The spike re-renders the visible pages after each mutating command and measures the cost.

#### 3B.3 Shell candidates (at most two)
| | **Tauri v2 (Android System WebView) + web UI** | **Flutter + flutter_rust_bridge** |
|---|---|---|
| Licence | MIT OR Apache-2.0 (https://github.com/tauri-apps/tauri) | Flutter BSD-3-Clause (https://github.com/flutter/flutter); FRB MIT (https://github.com/fzyzcjy/flutter_rust_bridge) (**UNVERIFIED**, check LICENSE files) |
| Engine binding | wasm in the WebView (the same artifact as the web editor). Alternative: native Rust in the Tauri core, with RGBA returned as `tauri::ipc::Response` raw bytes (https://v2.tauri.app/develop/calling-rust/) | Native `.so` through FRB/cargokit; RGBA through `ui.decodeImageFromPixels` or a `Texture` |
| Text input / IME | Hidden `<textarea>` placed at the caret rectangle. `beforeinput` (insertText, insertCompositionText, deleteContentBackward, insertReplacementText) and `composition*` events go to `insert`/`delete_before` plus a preedit overlay. The textarea's value mirrors `text_context()` so Gboard autocorrect has context | Hidden `EditableText`/`DeltaTextInputClient` (insertion, deletion and replacement deltas) going to the same API |
| Scroll, zoom, rotation | **Native browser scrolling** (momentum), pinch (CSS/visualViewport) and rotation come free, because each page is a `<canvas>` in a scrolling `<div>` | Native through Flutter's `InteractiveViewer`/scrollables |
| Reuse | The same UI and wasm serve web (Phase 4) and mobile. The HRM mobile app is React Native/Expo (brief Phase 6.6), which can host the same page in a WebView | A separate Dart UI. The web editor would be a second UI |
| Machine fit | Tauri's docs ask for the MSVC host toolchain on Windows (https://v2.tauri.app/start/prerequisites/). **UNVERIFIED** with gnu. Use `@tauri-apps/cli` from npm (prebuilt) | The Flutter SDK is not installed (several GB, **UNVERIFIED** size) |

**Pick for the spike: Tauri/WebView.** Run the IME and scroll evidence first in **Chrome on the
emulator** (no Tauri needed), then wrap it in Tauri. If Tauri fails on gnu, wrap it in a 30-line Gradle
`WebView` activity instead.

#### 3B.4 Minimal spike scope (about 2 days, Android only)
| # | Step | Done-check |
|---|---|---|
| B1 | `spikes/mobile-b/bind` crate (API above, about 300 lines) plus a 2-test native harness | `cargo test`: open the fixture → `insert("X")` → `save` → the docx contains X |
| B2 | `spikes/mobile-b/web`: Trunk page, one `<canvas>` per visible page in an `overflow:auto` div, the IME bridge textarea, a 4-button toolbar (Bold, Undo, Save, Zoom). Build to `E:/office-target/mobile-b` | Works in desktop Chrome: tap places the caret, typing works, save downloads a valid docx |
| B3 | Emulator Chrome via `adb reverse` | The same Gboard matrix as A6, recorded |
| B4 | Measure: wasm size; `render_tile` ms per page at DPR 2.625 (`performance.now()`); memory (`dumpsys meminfo com.android.chrome` for the renderer); scroll smoothness (recording) | Numbers in a table |
| B5 (stretch) | Tauri v2 Android wrap (`tauri android init` / `dev`), or a plain Gradle WebView app | APK runs the same page; `am start -W` startup |

**Effort (B):** B1–B4 take about 2 days, and B5 adds 0.5–1 day.

**Risks (B):**
1. The whole UI must be rebuilt. The layer map counts about 51k lines of ui-egui (`docs/phase0/layer-map-wordcraft.md`, "Obstacles" §3), and command parameters have no schema.
2. The engine has no composition API (an upstream patch).
3. Without damage tracking, re-rendering after each keystroke may be slow in single-threaded wasm (`render/src/lib.rs:23-27`).
4. Wasm memory with large documents in a WebView.
5. Tauri on a windows-gnu host (**UNVERIFIED**).
6. Losing ui-egui-only behaviour (tables, objects, rulers) until it is rebuilt.

### 3.C Recommendation for Spike 3
1. **Run A0 first.** It costs half a day and no build: wordcraft-web in emulator Chrome. It answers whether egui's *web* text path (which has composition and DeleteSurrounding support, `text_agent.rs:284-343`) handles Gboard. It also reproduces the scale, scroll and autocorrect problems on a real Android keyboard.
2. **Then the A kill test** (A2–A3, about 1 day, a tiny eframe probe and not wordcraft): *Can a Gboard-committed Cyrillic word and a glide-typed word reach an egui `TextEdit` on the Android emulator, first with stock eframe 0.36.2 / winit 0.30.13, then with the GameActivity text-state shim?*
   - If both runs fail, approach A is dead for "native keyboard with Cyrillic, autocorrect and swipe typing" (brief §4.3) without patching winit or eframe.
   - If the shim works, build the full wordcraft Android shell (A4–A6).
3. **Run B1–B4 in parallel or right after.** They need only the wasm toolchain the inventory already uses. B's distinctive evidence is native scroll, pinch and IME inside a WebView, against a UI-rebuild cost.

**Single fastest measurement to kill A:** the Cyrillic-plus-glide Gboard test on the minimal eframe
GameActivity probe. On the code read, stock winit 0.30 cannot pass it, because it emits no Ime events
on Android and ignores `TextEvent`. So the real question is whether our shim can pass it within one
day.

### Sources (external)
- ONLYOFFICE API: callback handler https://api.onlyoffice.com/docs/docs-api/usage-api/callback-handler/; JWT overview https://api.onlyoffice.com/docs/docs-api/additional-api/signature/; browser token https://api.onlyoffice.com/docs/docs-api/additional-api/signature/browser/; token in header https://api.onlyoffice.com/docs/docs-api/additional-api/signature/request/token-in-header/
- jsonwebtoken: https://docs.rs/crate/jsonwebtoken/latest/features, https://docs.rs/jsonwebtoken/latest/jsonwebtoken/, https://github.com/Keats/jsonwebtoken/blob/master/README.md
- winit 0.30 Android backend: https://github.com/rust-windowing/winit/blob/v0.30.x/src/platform_impl/android/mod.rs; winit PR #2444 (wrapping input events for future IME): https://github.com/rust-windowing/winit/pull/2444
- egui-winit 0.36: https://raw.githubusercontent.com/emilk/egui/0.36.0/crates/egui-winit/src/lib.rs
- Packaging: https://github.com/bbqsrc/cargo-ndk, https://github.com/rust-mobile/cargo-apk, https://github.com/rust-mobile/xbuild, https://github.com/rust-mobile/rust-android-examples/tree/main/agdk-eframe
- Tauri: https://v2.tauri.app/start/prerequisites/, https://v2.tauri.app/develop/calling-rust/
