#!/usr/bin/env bash
# Image entrypoint (docs/phase5/dropin.md D6). With JWT_SECRET or JWT_SECRET_FILE set, the server config is built from the environment:
# the ONLYOFFICE Docker names (JWT_ENABLED, JWT_SECRET, JWT_HEADER, JWT_IN_BODY) plus DOCSERVER_* extras, so a plain `docker run` or
# compose swap needs no hand-written TOML. Without them the server runs with the given arguments (the image CMD: --config ...).
# Secrets are never printed. The config goes to /tmp (the tmpfs of a read-only container).
set -euo pipefail
BIN=(/app/hoshiya-serve --dist /app/dist --brand /app/brand/brand.toml)
die() { echo "entrypoint: $*" >&2; exit 64; }
if [ -z "${JWT_SECRET:-}${JWT_SECRET_FILE:-}" ]; then
  # fail closed: no secret in the environment and no operator config file mounted means there is nothing safe to run
  cfg=""; prev=""
  for a in "$@"; do [ "$prev" = --config ] && cfg=$a; prev=$a; done
  [ -r "$cfg" ] || die "JWT_SECRET or JWT_SECRET_FILE is required (the secret the host application signs its editor config with), or mount a config file and pass --config; see packaging/docserver/README.md"
  exec "${BIN[@]}" "$@"
fi
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
V=/data/secrets
# secret <name> [dir]: the file in $S, else one an earlier start generated in $V, else a new one in dir (none without dir). Run it
# as `X=$(secret ...)` in this shell: its die then stops the script (gate review L5: a failed write said "generated" and went on).
secret() {
  local f
  for f in "$S/$1" "$V/$1"; do if [ -s "$f" ]; then printf %s "$f"; return; fi; done
  [ -n "${2:-}" ] || return 0
  mkdir -p "$2" || die "cannot store a generated $1 in $2; mount $S/$1 (DOCSERVER_SECRETS_DIR) or a volume writable by uid 10002 on /data"
  head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$2/$1" || die "cannot write $2/$1"
  [ "$2" != "$V" ] || warn "generated $1 in $V on the data volume; mount $S with it instead (a copy of the volume holds it)"
  printf %s "$2/$1"
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
need() { [ -n "${!1:-}" ] || die "$1 is required (packaging/docserver/README.md, docs/phase5/dropin.md D6)"; }
# checked here, in this shell: a die inside the $(...) of the heredoc below would only end that subshell
for v in "${!DOCSERVER_@}"; do q "${!v}" > /dev/null; done
for v in DOCSERVER_PUBLIC_PORT DOCSERVER_INTERNAL_PORT DOCSERVER_MAX_DOCUMENT_MB DOCSERVER_KEEP_CLOSED_HOURS DOCSERVER_KEEP_ABANDONED_DAYS DOCSERVER_CLOSE_IDLE_MINUTES; do
  case "${!v:-1}" in *[!0-9]*) die "$v must be a number" ;; esac
done
for v in DOCSERVER_PUBLIC_URL DOCSERVER_FETCH_ALLOW DOCSERVER_INTERNAL_URL DOCSERVER_INTERNAL_ALLOW; do need "$v"; done
# TLS: the server's own (a certificate and key given as files) or a trusted proxy's (the drop-in); one of the two is required
if [ -n "${DOCSERVER_TLS_CERT:-}" ]; then
  need DOCSERVER_TLS_KEY
  [ -r "$DOCSERVER_TLS_CERT" ] && [ -r "$DOCSERVER_TLS_KEY" ] || die "DOCSERVER_TLS_CERT / DOCSERVER_TLS_KEY: cannot read the files (they must be readable by uid 10002)"
  TLS="tls = \"builtin\"
tls_cert = $(q "$DOCSERVER_TLS_CERT")
tls_key = $(q "$DOCSERVER_TLS_KEY")"
else
  [ -n "${DOCSERVER_TRUSTED_PROXIES:-}" ] || die "TLS is required: set DOCSERVER_TLS_CERT and DOCSERVER_TLS_KEY (the server's own), or DOCSERVER_TRUSTED_PROXIES (TLS ends at a proxy)"
  # without a certificate of its own, TLS must end at a proxy that is listed here
  TLS="tls = \"upstream\"
trusted_proxies = $(list "$DOCSERVER_TRUSTED_PROXIES")"
fi
[ -n "${DOCSERVER_FRAME_ORIGINS:-}" ] || warn "DOCSERVER_FRAME_ORIGINS is empty: only $DOCSERVER_PUBLIC_URL may embed the editor; list the origin of the host application's pages"
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

# Gate review L3: only the server secret is generated on the data volume (the history chain and open sessions need it across
# restarts). The converter's secret is used by nobody outside and is made anew in the tmpfs each start. The audit key is never
# generated: next to the log it would protect nothing; without a mounted one the chain is unkeyed and the server says so.
SS=$(secret server-secret "$V")
CS=$(secret converter-secret "$R")
AK=$(secret audit-key)
[ -n "$AK" ] || warn "no $S/audit-key: the audit log's chain is not keyed; mount one (outside the data volume) to key it"

cat > "$R/api.toml" <<EOF
listen = "0.0.0.0:${DOCSERVER_PUBLIC_PORT:-8787}"
auth = "jwt"
secret_file = $(q "$CS")
$TLS
public_url = $(q "$PUB")
work_dir = "/tmp/work"
store_dir = "/data/store"

[docserver]
server_secret_file = $(q "$SS")
$([ -z "$AK" ] || echo "audit_key_file = $(q "$AK")")
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
