# Document server: deployment and the local rehearsal (Phase 5, WP10)

The image is `hoshiya-serve` (Linux build, with the `[docserver]` section) plus the Docs dist (with patch 0127) and the shell.
Everything is in `packaging/docserver/`. Nothing here is deployed anywhere but the developer PC; the HRM test server (`<test-host>`)
needs the owner's OK (PLAN section 7, WP17).

## Image

- `hoshiya-docserver:rehearsal`: `debian:bookworm-slim` + one 12 MB static-ish binary + `docs-diff` and `sheets-diff` in `/app/diff` (History > Compare) + `dist/docs-web` + `dist/shell`. Runs as uid 10002
  (`USER`), with `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges`; writable: the `/data` volume and a `/tmp` tmpfs only.
  `HEALTHCHECK` = `GET /healthz` over the container's own loopback (the one request `tls = "upstream"` answers without the proxy).
- Build offline-in-container (the dev PC's proxy breaks TLS inside containers): `packaging/docserver/build.sh` runs `cargo vendor --locked`
  on the host, compiles `suite-serve` and `fake-hrm` in `rust:1-slim-bookworm` with `--offline`, copies the dists, runs `docker build`.
  Sheets, Slides and PDF dists are not in the image (the server logs that those routes are not served).
- **Size (2026-10-25):** 49.7 MB compressed content / 193 MB on disk (`docker images`); the test harness image `hoshiya-fakehrm:rehearsal`
  is 29.4 MB / 118 MB. Both are on this PC only; no registry push.
- **`cargo deny`** (`ci/deny/services.toml`, `--target x86_64-unknown-linux-gnu`, the graph of this image): `advisories ok, bans ok,
  licenses ok, sources ok`, exit 0.

## Topology of the rehearsal (`compose.yml`, network `backend_net_rehearsal`, 172.29.50.0/24)

```
browser --https 127.0.0.1:18443--> traefik .5 --http--> docserver .10:8787   (public side, tls = "upstream")
browser --http  localhost:8790 --> fakehrm .20:8790     (host page + /__test/ routes; the stand-in for the HR backend)
fakehrm .20 --http--> docserver .10:8780                (internal listener: saved-file route only, internal_allow = ["172.29.50.20/32"])
docserver .10 --http--> fakehrm:8790                    (document fetch and callbacks; provider entry marked internal = true)
```

- The name `backend_net` already exists on this PC (another project's network, 172.18.0.0/16): the rehearsal uses `backend_net_rehearsal`
  and never touches it. On `<test-host>` the service joins the real `backend_net` (`external: true`).
- Traefik: file provider only (no docker socket), one TLS entry point, a throwaway CA made by `rehearse.sh`. The certificate is also the
  default one, because a client that connects to an IP address sends no SNI.
- The internal listener has no `ports:` entry; from another container on the network it answers 403, from the host the connection fails
  (both checked by `rehearse.sh`).
- Run: `packaging/docserver/build.sh && packaging/docserver/rehearse.sh 3` (`KEEP=1` leaves the containers up).
  The browser flow is `ci/web-smoke/docserver-compose.spec.ts` (Playwright/Chromium on the host, certificate pinned by public key): open the
  document, type, save, close (callback status 2), reopen, download the stored document from the backend and compare its hash.

## Example operator config

Shape used by the rehearsal (`packaging/docserver/api.rehearsal.toml`). The HRM appears only here, as one example provider; the code
knows no HRM names.

```toml
listen = "0.0.0.0:8787"
auth = "jwt"
secret_file = "/run/secrets/converter-secret"     # >= 32 bytes
tls = "upstream"                                   # TLS ends at Traefik
trusted_proxies = ["<traefik container IP>/32"]    # plus every entry of Traefik's trustedIPs (see below)
public_url = "https://docs.example.org"            # the public host name (the Host check and redirect target)
work_dir = "/tmp/work"
store_dir = "/data/store"

[docserver]
server_secret_file = "/run/secrets/server-secret"  # >= 32 bytes, different from every provider secret
audit_key_file = "/run/secrets/audit-key"          # kept outside the data volume
data_dir = "/data/docserver"
diff_bin_dir = "/app/diff"                         # docs-diff and sheets-diff (History > Compare); unset = 501
internal_listen = "<this container's IP on backend_net>:8780"   # never publish with -p
internal_allow = ["<the HR backend container IP>/32"]          # not a /16 that contains the bridge gateway
keep_closed_hours = 168                            # retention: a session 7 days after a close the backend acknowledged (default)
keep_abandoned_days = 7                            # any other session 7 days after its last activity (default)

[[docserver.providers]]
name = "hrm"                                        # example: any backend that signs the config token (HS256)
issuer = "hrm"
secret_file = "/run/secrets/provider-secret"       # = the backend's CRAFT_DOCSERVER_JWT_SECRET
fetch_allow = [{ url = "http://api:8000", internal = true }]
callback_allow = [{ url = "http://api:8000", internal = true }]
frame_origins = ["https://hr.example.org"]         # the SPA origin that may frame the editor
history = "session"                                # default: a key's history goes with its last session (new key per open)
```

Compose service (the parts that matter): `image`, `read_only: true`, `cap_drop: [ALL]`, `tmpfs: /tmp`, a volume on `/data`, the config and
secrets mounted read-only, a fixed IPv4 address on `backend_net` (so `internal_listen`, `internal_allow` and `trusted_proxies` can name
addresses), no `ports`. Traefik routes the public host name to `http://<service>:8787` with `tls`. The backend is told the internal URL
(`http://<service IP>:8780`) as its `CRAFT_DOCSERVER_INTERNAL_URL`.

## Client addresses behind proxies (security rehearsal F4)

The rate limits (`rate_create_per_min`, `rate_auth_fail_per_min`) and the audit log's `client` count per client address (an IPv4
address, or an IPv6 /64). The docserver takes it from `X-Forwarded-For` only when the connection comes from an address in its own
`trusted_proxies`, and then reads the header from the right: the first address that is not itself in `trusted_proxies` is the client.
Traefik builds that header. It appends the address it was connected from, and keeps what came before only when that peer is in its
`entryPoints.websecure.forwardedHeaders.trustedIPs` (`packaging/docserver/traefik/traefik.yml`); from anyone else it drops the
incoming header.

The two lists must describe the same chain:

| Chain | Traefik `trustedIPs` | docserver `trusted_proxies` |
|---|---|---|
| browser → Traefik → docserver (the rehearsal) | none (the file lists only the loopback) | Traefik |
| browser → front proxy (an HRM nginx, a load balancer) → Traefik → docserver | the front proxy | Traefik **and** the front proxy |

What goes wrong when they do not:
- **Front proxy missing from Traefik's `trustedIPs`** (Traefik's default): Traefik drops the browser's address and reports the front
  proxy as the client. Every user then has **one shared rate-limit key**: 30 session opens a minute for the whole organisation, and 20
  bad or expired tokens from anyone give everyone 429 for a minute. The audit log names the proxy for every request.
- **Front proxy in `trustedIPs` but not in `trusted_proxies`**: the header reads `<browser>, <front proxy>`; the docserver stops at the
  front proxy, which it does not trust, and the result is the same single key.
- **A list wider than the real proxies** (a whole /16, or an address a user can reach Traefik from): that sender can write any
  `X-Forwarded-For` and pick its own rate-limit key, so the limits stop holding. Keep both lists to the proxies' own addresses.

The front proxy's address is site-specific: it is marked `OPERATOR` in `traefik.yml`. On `<test-host>` it is checked in WP17 (two clients
behind the front proxy must get separate keys: the audit line's `client` differs). Not checked here: there is no front proxy on this PC.

## Retention

A sweep thread deletes `sessions/<sid>/` `keep_closed_hours` (168, 7 days) after a close the provider acknowledged, and `keep_abandoned_days` (7)
after the last activity of any other session; never while a status 2 callback is still queued for retry
(`services/docserver/src/retention.rs`). It runs at start and then hourly, and writes one `retention.sweep` audit line with counts when
it deleted something. The session's history goes with it unless the provider has `history = "keep"`. Back-ups of `data_dir` would
defeat this (Q10: no back-ups of the data folder).

## Gaps and notes

- The rehearsal harness prints its random provider secret and `rehearse.sh` copies it into a file; a real deployment generates the secret
  once and gives the same file to both sides.
- `rehearse.sh` chmods the secrets 0644 so uid 10002 can read them; on a real host use a group or Docker secrets.
- The Windows host cannot run `--user`-mapped bind mounts the way Linux does; uid/permission behaviour of the volume is checked on Linux
  only when the image goes to `<test-host>`.
- Under heavy disk load on this PC one earlier attempt timed out waiting for "closed" (30 s); the recorded 3 runs were on an idle disk.
