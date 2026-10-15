# Spike 4 results: document server (S0-S7)

Ran against the **official release wordcraft-web v0.7.0** (`E:/office-tools/releases/wordcraft-web-0.7.0.zip`,
fresh extraction at `E:/office-target/spike4/wc-stock/wordcraft-web-0.7.0/`), **not the upstream HEAD build**.
S8-S9 (host-bridge patch, wasm rebuild) were out of scope and not run. Adapter tested: `shim` only.
Browser: Playwright 1.56.1 (pinned exactly), headless Chromium, 1 worker, WebGL via SwiftShader
(`--use-angle=swiftshader --enable-unsafe-swiftshader`), inner URL `/wc/index.html?webgl`.

## What was built (all under `spikes/docserver/`)
- `Cargo.toml` (own `[workspace]`), `src/main.rs` (router, endpoints), `src/jwt.rs` (hand-rolled HS256 + 4 tests),
  `src/config.rs`, `src/fetch.rs` (SSRF guard + 1 test), `src/session.rs`, `src/callback.rs`,
  `src/bin/fake_hrm.rs`
- `static/api.js` (DocsAPI.DocEditor shim), `static/editor.html` (wrapper, shim adapter), `hrm/index.html`
- `fixtures/sample.docx`, `scripts/smoke.sh`
- `e2e/package.json` (+lock), `e2e/playwright.config.ts`, `e2e/spike4.spec.ts`, `e2e/verify_docx.py`, `e2e/results.jsonl` (raw run log)
- Screenshots: `docs/phase0/docserver/` (`s6-1-before-edit`, `s6-2-after-edit`, `s6-3-saved-banner`, `s7-1-tampered-token`, `s7-2-ssrf`, `s7-3-wrong-callback-secret`, all `.png`)

Deviations from the plan:
1. HS256 is hand-rolled (hmac 0.12 + sha2 0.10 + base64 0.22) instead of `jsonwebtoken`, so no `rsa`/RUSTSEC-2023-0071 dependency. No `tracing`/`tracing-subscriber` (eprintln only).
2. Playwright does not start the servers (no `webServer`); the caller starts `fake_hrm` and `docserver`.
3. Negative test (iii) uses a test-only fake_hrm endpoint `POST /admin/callback-secret?value=...` that overrides the HRM's callback-verification secret, instead of restarting docserver with another secret. The effect is the same (docserver signs with S, HRM verifies with S').
4. Servers were debug builds (`cargo build -j 2`, `CARGO_TARGET_DIR=E:/office-target/docserver`).
5. Shim "ready" event fires after drop + 2 rAF + 500 ms (there is no completion signal in shim mode), so `readyMs` includes that 500 ms.

## Done-check outcomes
| Step | Result | Evidence |
|---|---|---|
| S0 | PASS | v0.7.0 dist extracted: `index.html` + `wordcraft-web-8f83828293ff7c4d_bg.wasm` (39,785,016 B); `npx playwright --version` -> `Version 1.56.1`; Chromium installed in the default cache |
| S1 | PASS | Fixture made with `wordcraft-cli run --cmd 'text.insert=...' --save` (6,527 B); `python -I e2e/verify_docx.py fixtures/sample.docx "HRM fixture"` -> `OK`, exit 0 |
| S2 | PASS | `cargo test -j 2`: 4 jwt tests (round-trip, tampered, alg=none rejected, wrong secret/expired) + 1 SSRF allowlist test, all pass |
| S3 | PASS | `smoke.sh`: config JSON has a 3-part token; `/files/sample.docx` sha equals the fixture |
| S4 | PASS | `scripts/smoke.sh` printed `SMOKE OK`: session -> document sha equal -> save -> `received.jsonl` gained `status:6` line with the same sha256 as the fixture; tampered `/files/..?t=` gave 401. `.wasm` served as `application/wasm` |
| S5 | PASS (automated, not manual) | Headless run showed the fixture text in the editor (screenshot s6-2); Ctrl+S is delivered to the canvas, the app handles it (no browser "Save as" dialog), and it adds a jsonl line |
| S6 | PASS, 3/3 | table below |
| S7 | PASS, 3/3 | table below |

## S6 happy path (3 consecutive green runs, shim adapter)
| Run | Result | HRM page load -> editor ready | Ctrl+S -> callback line visible at HRM (poll 50 ms) | Saved docx |
|---|---|---|---|---|
| 1 | pass | 4931 ms | 327 ms | 6,355 B |
| 2 | pass | 1927 ms | 446 ms | 6,353 B |
| 3 | pass | 2164 ms | 147 ms | 6,355 B |

- Each run typed ` SPIKE4-<ts> Салом`; `verify_docx.py` found the marker and `Салом` in the HRM-received file every time (Cyrillic typing via `keyboard.type` works).
- wasm transferred: 39,785,016 B (Content-Length; uncompressed, localhost). Playwright `body()` length was not obtainable, so the header value was used.
- Saved docx 6,353-6,355 B vs fixture 6,527 B (re-serialised by the engine).
- Run 1 was slower to start (cold). The numbers are SwiftShader software WebGL on loopback, so treat them as an upper bound for rendering and a lower bound for network.

## S7 negative tests
| Test | Observed |
|---|---|
| (i) one char of `config.token` flipped | `POST /api/session` -> **401** `invalid token: BadSignature`; HRM got `onError {status:401}` |
| (ii) correctly signed `document.url=http://169.254.169.254/latest` | **403** `host not allowed: 169.254.169.254:80`; `onError {status:403}` |
| (iii) HRM verifies callbacks with a different secret | HRM answered the callback **401** `{"error":1}` (rejected.jsonl: `bearer BadSignature`); `/save` returned 200 with `callback:"failed"`, detail `callback receiver answered 401 Unauthorized`; wrapper raised `onError`; no new line in `received.jsonl` |

## What failed / caveats
- Nothing in S0-S7 failed. First attempts hit two bugs that were fixed: (a) `api.js` posted the config including `events` functions (DataCloneError); (b) the shim returned outer-realm Promises, which the wasm rejected with "showSaveFilePicker failed: not a promise", so every promise handed to the wasm is now created from the inner window's `Promise`.
- Shim limits: no dirty-state events, no host-initiated save, relies on internals (DOM ids, drop handler, picker lookup). Not upstreamable.
- Stock v0.7.0 UI shows its own chrome (Discord button, Share, Zotero tab, title "sample - Edited"), which a product build must remove.
- A failed callback leaves the app showing the document as saved (the close rejection path was not inspected in the UI beyond the banner). Async-save ticket handling is Phase 5 (see plan 4.4).
- SSRF guard is allowlist-only (no DNS/IP resolution checks). `exp` in config is checked when present; `document.key` versioning, read-only enforcement and `/close` (status 2/4) are not implemented.
- Debug binaries, session data at `E:/office-target/docserver/data`, received files at `E:/office-target/docserver/received`.

## ONLYOFFICE config-field compatibility (what the spike supports)
Supported: `document.{key,url,title,fileType,permissions.edit}`, `editorConfig.{callbackUrl,mode,user.id}`, `token` (HS256, verified; the payload is the only trusted source), `documentType` (ignored), `events.{onDocumentReady,onDocumentStateChange (API wired, never fired in shim mode),onError}` plus non-standard `onSaved`.
Callback sent: `{key,status:6,url,filetype,users,forcesavetype:1,token}` with `Authorization: Bearer JWT({payload:body})`; HRM must reply `{"error":0}`.
Not supported: everything else (`editorConfig.{lang,customization,coEditing,...}`, `document.info`, `height/width/type`, `permissions` other than `edit`, status 2/4 on close, inbox/outbox/browser separate secrets, `exp` required).

## What S8-S9 still need
- S8: host-bridge patch to wordcraft-web (`host.rs`, ~175 lines, `?host=parent`), wasm rebuild with `trunk build --release` to `E:/office-target/spike4/wc-bridge`, export to `spikes/docserver/patches/`. Needs a build against upstream HEAD (not the v0.7.0 release).
- S9: bridge adapter in `editor.html` (open/save/dirty/requestSave messages), `WORDCRAFT_WEB_DIST` -> wc-bridge, HRM-triggered `requestSave`, `onDocumentStateChange` on typing, `/close` sending status 2, spec re-run with `adapter=bridge` 3x.
- S10: copy these numbers into `docs/STATUS.md`.
