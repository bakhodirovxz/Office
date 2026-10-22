# Accounts: local by default, a provider seam for later (Phase 4, WP19)

PLAN section 5.6. Local mode asks for no login and is the default. A server connection is optional. Phase 5 adds SSO; Phase 4 only
leaves the seam that SSO plugs into.

## The seam

Two things, both in `services/convert-api/src/config.rs`:

| Piece | What it is | Phase 4 behaviour |
|---|---|---|
| `auth` mode (`Settings.auth` -> `Resolved.auth: Auth`) | `"none"` (local, loopback only) or `"jwt"` (server mode: TLS, secret, `public_url`) | unchanged. A new mode (for example `"oidc"`) is a new arm of the one `match` in `Settings::resolve` and a new `Auth` variant. |
| `[[providers]]` (`Settings.providers: Vec<Provider>`) | `name`, `issuer`, `kind = "jwt" \| "oidc"`. Configuration only. | None by default. One `kind = "jwt"` entry is accepted in server mode and its `issuer` becomes the required `iss` (the same as `iss = "..."`; if both are set they must be equal). `kind = "oidc"` is refused at start: `kind = "oidc" is available in a later version`. More than one entry, an empty `name` or `issuer`, or any entry in local mode is refused. |

The converter page already works with any provider that hands the user an HS256 bearer token: `#token=` fragment or the paste field, kept
in `sessionStorage`, sent in the `Authorization` header and never in a URL. "Connect to server" navigates to the server's own page and
never fetches across origins. The Settings entry "Connect to server" of the shell and the desktop `settings.toml [server] url = ""` belong
to the shell work packages (WP9, WP10); the target they navigate to is the server's `/` (the converter) or `/convert/` with
`#token=...`.

## Example (documentation only)

An HR system that issues the tokens is just a provider. It is never named in code; the names below are placeholders.

```toml
auth = "jwt"
secret_file = "/run/secrets/convert-secret"   # shared with the issuer, at least 32 bytes
tls_cert = "/etc/convert/server.pem"
tls_key = "/etc/convert/server.key"
public_url = "https://convert.example.org"
redirect_listen = "0.0.0.0:80"

[[providers]]
name = "staff-portal"
issuer = "https://portal.example.org"
kind = "jwt"
```

The issuer signs `{ "sub": "...", "scope": "convert", "iss": "https://portal.example.org", "exp": <now + at most max_token_lifetime_s> }`
and sends the user to `https://convert.example.org/#token=<jwt>`.

## Tests

- `services/convert-api/src/config.rs` unit test `providers_are_a_seam_...`: the rules above.
- `services/convert-api/tests/no_hrm_names.rs` `no_hrm_names_in_code`: the HR system's product and host names (whole words, any case; the list
  comes from `docs/phase0/hrm-current-state.md`) do not appear under `services/convert-api/{src,ui}`, `services/suite-serve/src`,
  `web/shell` or `crates/`. It runs in the services nextest run (`scripts/ci.sh convert`), not in `test-ours`, because that is where the
  seam lives. Older strings in `services/convert/` (fixture names in tests and the loss evidence) predate it and are not scanned.
- `ci/web-smoke/tls.spec.ts`: server mode over a real TLS handshake in Chromium (see `docs/phase4/tls.md`).
