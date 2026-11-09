#!/usr/bin/env bash
# Image entrypoint (docs/phase5/dropin.md D6). With JWT_SECRET or JWT_SECRET_FILE set, the server config is built from the environment:
# the ONLYOFFICE Docker names (JWT_ENABLED, JWT_SECRET, JWT_HEADER, JWT_IN_BODY) plus DOCSERVER_* extras, so a plain `docker run` or
# compose swap needs no hand-written TOML. Without them the server runs with the given arguments (the image CMD: --config ...).
# Secrets are never printed. The config goes to /tmp (the tmpfs of a read-only container).
set -euo pipefail
BIN=(/app/hoshiya-serve --dist /app/dist --brand /app/brand/brand.toml)
if [ -z "${JWT_SECRET:-}${JWT_SECRET_FILE:-}" ]; then exec "${BIN[@]}" "$@"; fi

die() { echo "entrypoint: $*" >&2; exit 64; }
warn() { echo "entrypoint: warning: $*" >&2; }
case "${JWT_ENABLED:-true}" in true | 1 | yes) ;; *) die "JWT_ENABLED=$JWT_ENABLED: this server never runs without a token" ;; esac
[ "${JWT_HEADER:-Authorization}" = Authorization ] ||
  warn "JWT_HEADER=$JWT_HEADER is ignored: callbacks carry the token in the body (which the backend reads) and in Authorization"
case "${JWT_IN_BODY:-true}" in true | 1 | yes) ;; *) warn "JWT_IN_BODY is ignored: the callback token is always in the body" ;; esac

umask 077
R=/tmp/docserver
mkdir -p "$R" /tmp/work
if [ -n "${JWT_SECRET_FILE:-}" ]; then
  [ -s "$JWT_SECRET_FILE" ] || die "JWT_SECRET_FILE: cannot read $JWT_SECRET_FILE"
  PS=$JWT_SECRET_FILE
else
  printf %s "$JWT_SECRET" > "$R/provider-secret"
  PS=$R/provider-secret
fi
unset JWT_SECRET

S=${DOCSERVER_SECRETS_DIR:-/run/secrets}
secret() { # <name>: the file in $S, else one generated once on the data volume
  if [ -s "$S/$1" ]; then printf %s "$S/$1"; return; fi
  mkdir -p /data/secrets
  if [ ! -s "/data/secrets/$1" ]; then
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "/data/secrets/$1"
    warn "generated $1 in /data/secrets; mount $S with it instead (the audit key belongs outside the data volume)"
  fi
  printf %s "/data/secrets/$1"
}
# TOML string and list of strings from a value / a comma list; a quote, backslash or control character is refused, not escaped
q() {
  case "$1" in *[\"\\]* | *[[:cntrl:]]*) die "a DOCSERVER_* value holds a quote, backslash or control character" ;; esac
  printf '"%s"' "$1"
}
list() { # <comma list> [table]: ["a", "b"], or [{ url = "a", internal = true }, ...] with table
  local out="" x
  IFS=',' read -ra xs <<< "$1"
  for x in "${xs[@]}"; do
    x=${x// /}
    [ -n "$x" ] || continue
    if [ "${2:-}" = table ]; then x="{ url = $(q "$x"), internal = true }"; else x=$(q "$x"); fi
    out="${out:+$out, }$x"
  done
  printf '[%s]' "$out"
}
need() { [ -n "${!1:-}" ] || die "$1 is required (docs/phase5/dropin.md D6)"; }
# checked here, in this shell: a die inside the $(...) of the heredoc below would only end that subshell
for v in "${!DOCSERVER_@}"; do q "${!v}" > /dev/null; done
for v in DOCSERVER_PUBLIC_PORT DOCSERVER_INTERNAL_PORT DOCSERVER_MAX_DOCUMENT_MB DOCSERVER_KEEP_CLOSED_HOURS DOCSERVER_KEEP_ABANDONED_DAYS DOCSERVER_CLOSE_IDLE_MINUTES; do
  case "${!v:-1}" in *[!0-9]*) die "$v must be a number" ;; esac
done
for v in DOCSERVER_PUBLIC_URL DOCSERVER_FETCH_ALLOW DOCSERVER_TRUSTED_PROXIES DOCSERVER_INTERNAL_URL DOCSERVER_INTERNAL_ALLOW; do need "$v"; done
case "${DOCSERVER_ALLOW_OPAQUE_ORIGIN:-0}" in   # dropin.md D3b: off unless the installer's --allow-opaque-origin set it
  1 | true | yes) OPAQUE=true ;;
  0 | false | no | "") OPAQUE=false ;;
  *) die "DOCSERVER_ALLOW_OPAQUE_ORIGIN must be 1 or 0" ;;
esac
PUB=${DOCSERVER_PUBLIC_URL%/}
# the public origin frames itself: a mobile WebView with baseUrl = this server has it as its page origin
FRAMES="${DOCSERVER_FRAME_ORIGINS:+$DOCSERVER_FRAME_ORIGINS,}$PUB"
P=${DOCSERVER_PROVIDER:-docsapi}
# PUBLIC_PORT = INTERNAL_PORT is the shared layout (dropin.md D2b): one listener, its saved-file route for INTERNAL_ALLOW only
[ "${DOCSERVER_PUBLIC_PORT:-8787}" != "${DOCSERVER_INTERNAL_PORT:-80}" ] || echo "entrypoint: shared layout: port ${DOCSERVER_PUBLIC_PORT:-8787} serves both sides" >&2

cat > "$R/api.toml" <<EOF
listen = "0.0.0.0:${DOCSERVER_PUBLIC_PORT:-8787}"
auth = "jwt"
secret_file = $(q "$(secret converter-secret)")
tls = "upstream"
trusted_proxies = $(list "$DOCSERVER_TRUSTED_PROXIES")
public_url = $(q "$PUB")
work_dir = "/tmp/work"
store_dir = "/data/store"

[docserver]
server_secret_file = $(q "$(secret server-secret)")
audit_key_file = $(q "$(secret audit-key)")
data_dir = "/data/docserver"
diff_bin_dir = "/app/diff"
internal_listen = $(q "0.0.0.0:${DOCSERVER_INTERNAL_PORT:-80}")
internal_allow = $(list "$DOCSERVER_INTERNAL_ALLOW")
internal_url = $(q "$DOCSERVER_INTERNAL_URL")
max_document_mb = ${DOCSERVER_MAX_DOCUMENT_MB:-100}
keep_closed_hours = ${DOCSERVER_KEEP_CLOSED_HOURS:-168}
keep_abandoned_days = ${DOCSERVER_KEEP_ABANDONED_DAYS:-7}
close_idle_minutes = ${DOCSERVER_CLOSE_IDLE_MINUTES:-5}
allow_opaque_origin = $OPAQUE

[[docserver.providers]]
name = $(q "$P")
issuer = $(q "$P")
compat = "docsapi"
secret_file = $(q "$PS")
fetch_allow = $(list "$DOCSERVER_FETCH_ALLOW" table)
callback_allow = $(list "${DOCSERVER_CALLBACK_ALLOW:-$DOCSERVER_FETCH_ALLOW}" table)
frame_origins = $(list "$FRAMES")
edit_types = $(list "${DOCSERVER_EDIT_TYPES:-docx,xlsx,pptx,pdf}")
$([ -z "${DOCSERVER_UI_LANG:-}" ] || echo "ui_lang = $(q "$DOCSERVER_UI_LANG")")
history = "session"
EOF
exec "${BIN[@]}" --config "$R/api.toml"
