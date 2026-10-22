# Server mode over TLS in a real browser (Phase 4, WP19)

Carry-forward item 6 of `docs/phase3/gate-review.md` 10.6. Before this, TLS was covered only by `services/convert-api/tests/{tls,hardening}.rs`
(rustls clients) and the token tests of the converter page over plain loopback HTTP.

## Setup (`ci/web-smoke/tls.spec.ts`, config `playwright.tls.config.ts`)

- The test makes a throwaway CA and a server certificate (`localhost`, `127.0.0.1`; EC P-256; 2 days) with `openssl` in a temp directory and
  deletes the directory afterwards. Nothing is installed in any trust store.
- `suite-serve` runs with `auth = "jwt"`, its own TLS, `public_url = https://localhost:<port>`, `redirect_listen`, and no
  `trusted_proxies`. The converter tool is the test stand-in (`convert-api-fake-tool`).
- Chromium is launched with `--ignore-certificate-errors-spki-list=<sha256 of the server key>`: the handshake is real and pinned to that one
  key. A second Chromium without the flag must refuse the same server (`ERR_CERT_AUTHORITY_INVALID`): the negative control.
- The spec mints HS256 tokens with the server's shared secret (scope `convert`).
- Needs `openssl` on PATH (Git for Windows ships it; `OPENSSL=<path>` overrides). It runs in `scripts/ci.sh convert-ui`.

## Cases

| Case | Check |
|---|---|
| TLS is real | `securityDetails().protocol` is TLS 1.3; `Strict-Transport-Security: max-age=31536000...` and the strict CSP are on the page; no token -> the "needs an access token" message; an unpinned Chromium is refused |
| Plain HTTP | the redirect listener answers 308 to `public_url` + path + query whatever the `Host` header says; in the browser it lands on the https page |
| `#token=` flow | the fragment is read and removed from the address bar; the page shows server mode |
| Conversion + download | txt to pdf completes; the download is a fetch + blob (`%PDF-`) |
| PDF combine | two PDFs combined in the page's order (`combined.pdf`) |
| No token in a URL | in the whole network log no URL contains the token, its payload or `token=`; every `/api/v1/` request (the file downloads included) carries `Authorization: Bearer ...`; every request is https or blob |
| Paste field | a token pasted in "Connect to server" connects, and the field is emptied |
| Disconnect | back to local mode, `sessionStorage` empty, no `Authorization` header afterwards |
| Expired / wrong-secret token | the page says the server did not accept the token, that it may have expired, and opens the Connect panel (`ui.conn.token_rejected`) |

Not covered here: the shell's "Connect to server" setting (the shell has no Settings screen yet; WP9/WP10), cross-origin connect (the page
navigates and never fetches across origins; `converter.spec.ts`), TLS 1.2 and cipher suites (`services/convert-api/tests/tls.rs`).
