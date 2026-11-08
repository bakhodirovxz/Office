#!/usr/bin/env bash
# Drop-in install of the document server in place of a running ONLYOFFICE-style Document Server container (docs/phase5/dropin.md D7).
# Run by the operator on the Docker host. Nothing in the backend changes: our container takes the old one's name, network aliases,
# Traefik labels, restart policy and published ports; the old one is kept, stopped and disconnected, for `install.sh rollback`.
#
#   install.sh [--dry-run] --profile FILE --image REF [--etc DIR] [--from NAME] [--backend NAME] [--public-url URL]
#              [--frame-origin URL]... [--trusted-proxy NAME|IP]... [--secret-file FILE] [--edit-types LIST]
#   install.sh rollback [--etc DIR]
#
# The profile (KEY=value lines, e.g. profiles/hrm.env) names the backend's env variables; the code knows no backend. Anything
# ambiguous is a refusal that names the flag settling it. Secrets are never printed: --dry-run shows their length only.
set -euo pipefail
export MSYS_NO_PATHCONV=1   # Git Bash (the rehearsal on Windows): keep container paths as written

ETC=/opt/docserver DRY=0 PROFILE="" IMAGE="" FROM="" BACKEND="" PUBLIC_URL="" SECRET_FILE="" EDIT_TYPES=docx,xlsx,pptx,pdf   # the types whose edit configs edit (docs/phase5/r6.md); --edit-types docx keeps the others view-only
FRAMES=() PROXIES=() CMD=install PUBLIC_PORT=8787
die() { echo "install.sh: refused: $*" >&2; exit 2; }
say() { echo "install.sh: $*"; }
[ "${1:-}" = rollback ] && { CMD=rollback; shift; }
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1 ;;
    --profile) PROFILE=$2; shift ;;
    --image) IMAGE=$2; shift ;;
    --etc) ETC=$2; shift ;;
    --from) FROM=$2; shift ;;
    --backend) BACKEND=$2; shift ;;
    --public-url) PUBLIC_URL=${2%/}; shift ;;
    --frame-origin) FRAMES+=("${2%/}"); shift ;;
    --trusted-proxy) PROXIES+=("$2"); shift ;;
    --secret-file) SECRET_FILE=$2; shift ;;
    --edit-types) EDIT_TYPES=$2; shift ;;
    *) die "unknown argument $1 (see the head of this file)" ;;
  esac
  shift
done
STATE=$ETC/state.env
LINUX=1; case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) LINUX=0 ;; esac
command -v docker > /dev/null || die "no docker CLI"
insp() { docker inspect -f "$2" "$1"; }
ours() { [ "$(docker inspect -f '{{index .Config.Labels "docserver.dropin.replaces"}}' "$1" 2> /dev/null)" != "" ]; }
envof() { # <container> <NAME>: the value of one env variable of a container ("" when unset)
  insp "$1" '{{range .Config.Env}}{{println .}}{{end}}' | sed -n "s/^$2=//p" | head -n1
}

rollback() {
  [ -f "$STATE" ] || die "no $STATE: nothing was installed from this --etc"
  # shellcheck disable=SC1090
  . "$STATE"
  docker inspect "$OLD_NAME-replaced" > /dev/null 2>&1 || die "the replaced container $OLD_NAME-replaced is gone"
  if docker inspect "$OLD_NAME" > /dev/null 2>&1; then
    ours "$OLD_NAME" || die "$OLD_NAME exists and is not ours: not removing it"
    docker rm -f "$OLD_NAME" > /dev/null
  fi
  docker rename "$OLD_NAME-replaced" "$OLD_NAME"
  local line net aliases ip a args
  while IFS='|' read -r net aliases ip; do
    [ -n "$net" ] || continue
    args=()
    for a in ${aliases//,/ }; do args+=(--alias "$a"); done
    [ -n "$ip" ] && args+=(--ip "$ip")
    docker network connect "${args[@]}" "$net" "$OLD_NAME" 2> /dev/null || say "note: $OLD_NAME already on $net"
  done <<< "$OLD_NETS"
  docker update --restart="$OLD_RESTART" "$OLD_NAME" > /dev/null
  [ "$OLD_RUNNING" = true ] && docker start "$OLD_NAME" > /dev/null
  mv "$STATE" "$STATE.rolled-back.$(date +%s)"
  say "rolled back: $OLD_NAME is the original container again (restart=$OLD_RESTART); the data volume $OLD_NAME-docserver-data is kept"
}
[ "$CMD" = rollback ] && { rollback; exit 0; }

[ -n "$PROFILE" ] && [ -f "$PROFILE" ] || die "--profile FILE is required (see packaging/docserver/profiles/)"
[ -n "$IMAGE" ] || die "--image REF is required"
docker image inspect "$IMAGE" > /dev/null 2>&1 || die "image $IMAGE is not on this host"
prof() { sed -n "s/^$1=//p" "$PROFILE" | tail -n1; }
FROM_IMAGE=$(prof FROM_IMAGE) PROVIDER=$(prof PROVIDER)
[ -n "$FROM_IMAGE" ] && [ -n "$PROVIDER" ] || die "the profile needs FROM_IMAGE and PROVIDER"
if [ "$LINUX" = 1 ] && [ "$DRY" = 0 ] && [ "$(id -u)" != 0 ]; then die "run as root: the secret files are given to uid 10002 (a --dry-run writes nothing and needs only docker)"; fi

# ---- 1. the old container: from the state of an earlier run, or found by its image
RERUN=0
if [ -f "$STATE" ]; then
  # shellcheck disable=SC1090
  . "$STATE"
  docker inspect "$OLD_NAME-replaced" > /dev/null 2>&1 || die "$STATE exists but $OLD_NAME-replaced is gone (rollback by hand)"
  RERUN=1
  OLD=$OLD_NAME-replaced
else
  if [ -z "$FROM" ]; then
    mapfile -t c < <(docker ps --format '{{.Names}} {{.Image}}' | awk -v p="$FROM_IMAGE" 'index($2, p) == 1 {print $1}')
    [ "${#c[@]}" = 1 ] || die "${#c[@]} running containers of image $FROM_IMAGE*; name the one to replace with --from"
    FROM=${c[0]}
  fi
  ours "$FROM" && die "$FROM is already ours"
  OLD=$FROM
  OLD_NAME=$(insp "$OLD" '{{.Name}}'); OLD_NAME=${OLD_NAME#/}
  OLD_RESTART=$(insp "$OLD" '{{.HostConfig.RestartPolicy.Name}}')
  OLD_RUNNING=$(insp "$OLD" '{{.State.Running}}')
  # net|aliases (no short ids)|fixed IPv4 (only when configured)
  OLD_NETS=$(insp "$OLD" '{{range $k,$v := .NetworkSettings.Networks}}{{$k}}|{{join $v.Aliases ","}}|{{if $v.IPAMConfig}}{{$v.IPAMConfig.IPv4Address}}{{end}}{{println}}{{end}}' |
    awk -F'|' 'NF { n=split($2, a, ","); s=""; for (i=1;i<=n;i++) if (a[i] !~ /^[0-9a-f]{12}$/) s = s (s?",":"") a[i]; print $1 "|" s "|" $3 }')
  OLD_PORTS=$(insp "$OLD" '{{range $p,$b := .HostConfig.PortBindings}}{{range $b}}{{$p}}|{{.HostIp}}|{{.HostPort}}{{println}}{{end}}{{end}}' | sed '/^$/d')
  OLD_LABELS=$(insp "$OLD" '{{range $k,$v := .Config.Labels}}{{$k}}={{$v}}{{println}}{{end}}' | grep '^traefik\.' || true)
fi
[ -n "$OLD_NETS" ] || die "$OLD_NAME is on no network"
NETS=$(cut -d'|' -f1 <<< "$OLD_NETS")
NAMES=$( (echo "$OLD_NAME"; cut -d'|' -f2 <<< "$OLD_NETS" | tr ',' '\n') | sed '/^$/d' | sort -u)

case "${OLD_RESTART:-no}" in no | always | unless-stopped | on-failure) ;; *) OLD_RESTART=unless-stopped ;; esac
OLD_ENV_JWT_ENABLED=$( [ "$RERUN" = 1 ] && echo true || envof "$OLD" JWT_ENABLED)
case "${OLD_ENV_JWT_ENABLED:-true}" in true | 1 | yes | "") ;; *) die "$OLD_NAME runs with JWT_ENABLED=$OLD_ENV_JWT_ENABLED: the backend's tokens would not verify there either" ;; esac

# ---- 2. the backend: the one container answering the backend URL's host on a network we share
who() { # <host>: running containers on our networks that answer to that name
  local net c
  for net in $NETS; do
    for c in $(docker network inspect -f '{{range .Containers}}{{.Name}} {{end}}' "$net"); do
      [ "$c" = "$OLD_NAME" ] && continue
      if [ "$c" = "$1" ] || insp "$c" "{{range \$k,\$v := .NetworkSettings.Networks}}{{if eq \$k \"$net\"}}{{join \$v.Aliases \" \"}}{{end}}{{end}}" | tr ' ' '\n' | grep -qx "$1"; then echo "$c"; fi
    done
  done | sort -u
}
hostof() { local h=${1#*://}; h=${h%%/*}; echo "${h%%:*}"; }
portof() { local h=${1#*://}; h=${h%%/*}; case "$h" in *:*) echo "${h##*:}" ;; *) case "$1" in https:*) echo 443 ;; *) echo 80 ;; esac ;; esac; }
BURL_DEFAULT=$(prof BACKEND_URL_DEFAULT)
if [ -z "$BACKEND" ]; then
  mapfile -t c < <(who "$(hostof "$BURL_DEFAULT")")
  [ "${#c[@]}" = 1 ] || die "${#c[@]} containers answer to $(hostof "$BURL_DEFAULT") on $NETS; name the backend with --backend"
  BACKEND=${c[0]}
fi
BURL=$(envof "$BACKEND" "$(prof BACKEND_URL_ENV)"); BURL=${BURL:-$BURL_DEFAULT}; BURL=${BURL%/}
case "$BURL" in http://*) ;; *) die "the backend URL $BURL is not plain http (our fetch has no TLS, dropin.md H:B9)" ;; esac
BHOST=$(hostof "$BURL") BPORT=$(portof "$BURL")
FETCH="http://$BHOST:$BPORT"
mapfile -t c < <(who "$BHOST")
[ "${#c[@]}" = 1 ] && [ "${c[0]}" = "$BACKEND" ] || die "the backend URL host $BHOST is not $BACKEND alone on $NETS"

# ---- 3. the saved-file URL: the backend rewrites the callback url to this (dropin.md D2), so it must be one of the old names
RW=$(envof "$BACKEND" "$(prof REWRITE_ENV)"); RW=${RW:-$(prof REWRITE_DEFAULT)}; RW=${RW%/}
case "$RW" in http://*) ;; *) die "$(prof REWRITE_ENV)=$RW: the internal listener is plain http" ;; esac
RWHOST=$(hostof "$RW") INTERNAL_PORT=$(portof "$RW")
grep -qx "$RWHOST" <<< "$NAMES" || die "$(prof REWRITE_ENV) points at $RWHOST, which is not a name of $OLD_NAME ($(echo $NAMES)); fix that env value first"
[ "$INTERNAL_PORT" != "$PUBLIC_PORT" ] || die "the rewrite target's port $INTERNAL_PORT is our public port"
AH=$(envof "$BACKEND" "$(prof ALLOWED_HOSTS_ENV)"); AH=${AH:-$(prof ALLOWED_HOSTS_DEFAULT)}
tr ',' '\n' <<< "$AH" | tr -d ' ' | tr 'A-Z' 'a-z' | grep -qx -e "$RWHOST" -e "$RWHOST:$INTERNAL_PORT" ||
  die "$RWHOST is not in the backend's $(prof ALLOWED_HOSTS_ENV): add it to that env value"

# ---- 4. the secret (never printed): --secret-file, the old container's JWT_SECRET, the backend's secret env; all present must agree
SECRET_TMP=$(mktemp); trap 'rm -f "$SECRET_TMP"' EXIT
if [ -n "$SECRET_FILE" ]; then cp "$SECRET_FILE" "$SECRET_TMP"
elif [ -s "$ETC/secrets/provider-secret" ]; then cp "$ETC/secrets/provider-secret" "$SECRET_TMP"
else printf %s "$(envof "$OLD" JWT_SECRET)" > "$SECRET_TMP"; fi
B_SECRET=""
for e in $(prof SECRET_ENVS); do B_SECRET=$(envof "$BACKEND" "$e"); [ -n "$B_SECRET" ] && break; done
[ -s "$SECRET_TMP" ] || { [ -n "$B_SECRET" ] && printf %s "$B_SECRET" > "$SECRET_TMP"; } || true
[ -s "$SECRET_TMP" ] || die "no secret: the old container has no JWT_SECRET and the backend none of $(prof SECRET_ENVS); give --secret-file"
[ -z "$B_SECRET" ] || [ "$B_SECRET" = "$(cat "$SECRET_TMP")" ] || die "the backend's secret differs from the document server's (not printed); HRM saves cannot work like this today either"
unset B_SECRET
SECRET_LEN=$(wc -c < "$SECRET_TMP" | tr -d ' ')
[ "$SECRET_LEN" -ge 16 ] || die "the shared secret has $SECRET_LEN bytes; rotate it to 32 random bytes or more on both sides first"
[ "$SECRET_LEN" -ge 32 ] || say "WARNING: the shared secret has only $SECRET_LEN bytes; rotate it to 32 random bytes or more"

# ---- 5. Traefik, the public URL, the trusted proxies
TRAEFIK=""
if [ -n "$OLD_LABELS" ]; then
  mapfile -t c < <(for net in $NETS; do docker network inspect -f '{{range .Containers}}{{.Name}} {{end}}' "$net" | tr ' ' '\n'; done | sed '/^$/d' | sort -u |
    while read -r n; do insp "$n" '{{.Config.Image}}' | grep -q traefik && echo "$n"; done)
  if [ "${#PROXIES[@]}" = 0 ]; then
    [ "${#c[@]}" = 1 ] || die "${#c[@]} Traefik containers share a network with $OLD_NAME; name the proxy with --trusted-proxy"
    TRAEFIK=${c[0]}; PROXIES=("$TRAEFIK")
  fi
  if [ -z "$PUBLIC_URL" ]; then
    mapfile -t h < <(grep -E '^traefik\.http\.routers\.[^.]+\.rule=' <<< "$OLD_LABELS" | grep -oE 'Host\([`"][^`"]+[`"]\)' | sed -E 's/Host\([`"]([^`"]+)[`"]\)/\1/' | sort -u)
    [ "${#h[@]}" = 1 ] || die "${#h[@]} Host() rules in $OLD_NAME's labels; give --public-url"
    grep -qE '^traefik\.http\.routers\.[^.]+\.tls(\.certresolver)?=' <<< "$OLD_LABELS" || die "no TLS label on $OLD_NAME's routers; give --public-url"
    PUBLIC_URL="https://${h[0]}"
  fi
  # routers may only use services defined in these labels (their port becomes ours) or none at all
  SVCS=$(grep -oE '^traefik\.http\.services\.[^.]+\.' <<< "$OLD_LABELS" | cut -d. -f4 | sort -u || true)
  for s in $(sed -n -E 's/^traefik\.http\.routers\.[^.]+\.service=//p' <<< "$OLD_LABELS"); do
    grep -qx "$s" <<< "$SVCS" || die "a router of $OLD_NAME uses service $s, which its labels do not define; move it to labels first"
  done
  LABELS=$(grep -v -E '^traefik\.http\.services\.[^.]+\.loadbalancer\.server\.(port|scheme)=' <<< "$OLD_LABELS")
  if [ -n "$SVCS" ]; then for s in $SVCS; do LABELS+=$'\n'"traefik.http.services.$s.loadbalancer.server.port=$PUBLIC_PORT"; done
  else LABELS+=$'\n'"traefik.http.services.$OLD_NAME-docserver.loadbalancer.server.port=$PUBLIC_PORT"; fi
else
  LABELS=""
  [ -n "$PUBLIC_URL" ] && [ "${#PROXIES[@]}" -gt 0 ] ||
    die "$OLD_NAME has no Traefik labels (a file route or a published port): give --public-url and --trusted-proxy (dropin.md section 2, last row)"
fi
case "$PUBLIC_URL" in https://*) ;; *) die "--public-url must be https://host[:port]" ;; esac
PORTS=()
while IFS='|' read -r cport hip hport; do
  [ -n "$cport" ] || continue
  [ "$cport" = 80/tcp ] || die "$OLD_NAME publishes $cport; only 80/tcp maps to our public listener"
  PORTS+=(-p "${hip:+$hip:}$hport:$PUBLIC_PORT")
done <<< "$OLD_PORTS"

# ---- 6. frame origins (dropin.md D3)
ok_origin() { grep -qE '^(https://[A-Za-z0-9.-]+(:[0-9]+)?|http://(localhost|127\.0\.0\.1)(:[0-9]+)?)$' <<< "$1"; }
if [ "${#FRAMES[@]}" = 0 ]; then
  for e in $(prof ORIGIN_ENVS); do
    for o in $(envof "$BACKEND" "$e" | tr ',;[]"' '     '); do o=${o%/}; ok_origin "$o" && FRAMES+=("$o"); done
  done
fi
if [ "${#FRAMES[@]}" = 0 ]; then
  echo "install.sh: Host() rules of other Traefik-routed containers (candidates):" >&2
  for n in $(docker ps --format '{{.Names}}'); do
    [ "$n" = "$OLD_NAME" ] && continue
    insp "$n" '{{range $k,$v := .Config.Labels}}{{$k}}={{$v}}{{println}}{{end}}' | grep -E '^traefik\.http\.routers\.[^.]+\.rule=' | grep -oE 'Host\([`"][^`"]+[`"]\)' | sed "s/^/  $n: /" >&2 || true
  done
  die "no frame origin: give --frame-origin https://<the page that embeds the editor> (repeatable)"
fi
for o in "${FRAMES[@]}"; do ok_origin "$o" || die "frame origin $o is not https://host[:port] (http only for localhost)"; done
FRAMES_CSV=$(printf '%s\n' "${FRAMES[@]}" | sort -u | paste -sd, -)
PROXIES_CSV=$(printf '%s\n' "${PROXIES[@]}" | sort -u | paste -sd, -)
# the backend by its alias and its container name: a recreated backend gets a new address, never a new name
ALLOW_CSV=$(printf '%s\n' "$BHOST" "$BACKEND" | sort -u | paste -sd, -)

# ---- the plan
FIRSTNET=$(head -n1 <<< "$NETS")
cat << EOF
install.sh: plan ($( [ "$RERUN" = 1 ] && echo "rerun: recreate ours" || echo "first install"))
  replace          $OLD_NAME (kept as $OLD_NAME-replaced, stopped, restart=no, disconnected; was restart=$OLD_RESTART)
  image            $IMAGE
  networks         $(echo $NETS)   names taken over: $(echo $NAMES)
  backend          $BACKEND  fetch/callback allow $FETCH  internal_allow $ALLOW_CSV
  saved-file URL   http://$RWHOST:$INTERNAL_PORT/ds/files/...  (the backend rewrites to $RW; $RWHOST is in its allowlist)
  public           $PUBLIC_URL  listener :$PUBLIC_PORT  trusted proxies $PROXIES_CSV  ports ${PORTS[*]:-none}
  frame origins    $FRAMES_CSV + $PUBLIC_URL
  provider         $PROVIDER  compat docsapi  edit types $EDIT_TYPES  secret <redacted, $SECRET_LEN bytes>
  traefik labels   $(grep -c . <<< "$LABELS" || true) (service port -> $PUBLIC_PORT)
  files            $ETC/secrets/{provider-secret,server-secret,converter-secret,audit-key} (0600, uid 10002), $STATE
  defaults         max_document_mb 100, keep_closed_hours 168, keep_abandoned_days 7, history session
EOF
[ "$DRY" = 1 ] && { say "dry run: nothing changed"; exit 0; }

# ---- apply
mkdir -p "$ETC/secrets"
chmod 700 "$ETC" "$ETC/secrets" 2> /dev/null || true
newkey() { [ -s "$ETC/secrets/$1" ] || head -c "$2" /dev/urandom | od -An -tx1 | tr -d ' \n' > "$ETC/secrets/$1"; }
cp "$SECRET_TMP" "$ETC/secrets/provider-secret"
newkey server-secret 48; newkey converter-secret 32; newkey audit-key 48
chmod 600 "$ETC"/secrets/*
if [ "$LINUX" = 1 ]; then chown 10002:10002 "$ETC/secrets" "$ETC"/secrets/*; chmod 500 "$ETC/secrets"; fi
if [ "$RERUN" = 0 ]; then
  # the state first: a failure from here on can be rolled back
  { printf 'OLD_NAME=%q\nOLD_RESTART=%q\nOLD_RUNNING=%q\nOLD_NETS=%q\nOLD_PORTS=%q\nOLD_LABELS=%q\n' \
      "$OLD_NAME" "$OLD_RESTART" "$OLD_RUNNING" "$OLD_NETS" "$OLD_PORTS" "$OLD_LABELS"; } > "$STATE"
  docker update --restart=no "$OLD_NAME" > /dev/null
  docker stop "$OLD_NAME" > /dev/null
  docker rename "$OLD_NAME" "$OLD_NAME-replaced"
  for net in $NETS; do docker network disconnect "$net" "$OLD_NAME-replaced"; done
else
  ours "$OLD_NAME" || die "$OLD_NAME is not ours"
  docker rm -f "$OLD_NAME" > /dev/null
fi
fail() { echo "install.sh: $*; rolling back" >&2; docker logs --tail 20 "$OLD_NAME" 2>&1 | sed 's/^/  | /' >&2 || true; rollback; exit 1; }
alias_args() { local a; for a in $(grep "^$1|" <<< "$OLD_NETS" | cut -d'|' -f2 | tr ',' ' '); do [ "$a" = "$OLD_NAME" ] || echo --network-alias "$a"; done; }
LABEL_ARGS=(--label "docserver.dropin.replaces=$OLD_NAME")
while IFS= read -r l; do [ -n "$l" ] && LABEL_ARGS+=(--label "$l"); done <<< "$LABELS"
SYSCTL=(); [ "$INTERNAL_PORT" -lt 1024 ] && SYSCTL=(--sysctl "net.ipv4.ip_unprivileged_port_start=$INTERNAL_PORT")
# shellcheck disable=SC2046
docker run -d --name "$OLD_NAME" --restart "$OLD_RESTART" \
  --read-only --cap-drop ALL --security-opt no-new-privileges:true --tmpfs /tmp:size=256m,uid=10002 "${SYSCTL[@]}" \
  -v "$OLD_NAME-docserver-data:/data" -v "$ETC/secrets:/run/secrets:ro" \
  --network "$FIRSTNET" $(alias_args "$FIRSTNET") "${PORTS[@]}" "${LABEL_ARGS[@]}" \
  -e JWT_SECRET_FILE=/run/secrets/provider-secret -e DOCSERVER_SECRETS_DIR=/run/secrets \
  -e "DOCSERVER_PUBLIC_URL=$PUBLIC_URL" -e "DOCSERVER_FETCH_ALLOW=$FETCH" -e "DOCSERVER_TRUSTED_PROXIES=$PROXIES_CSV" \
  -e "DOCSERVER_INTERNAL_URL=http://$RWHOST:$INTERNAL_PORT" -e "DOCSERVER_INTERNAL_PORT=$INTERNAL_PORT" \
  -e "DOCSERVER_INTERNAL_ALLOW=$ALLOW_CSV" -e "DOCSERVER_FRAME_ORIGINS=$FRAMES_CSV" -e "DOCSERVER_PROVIDER=$PROVIDER" \
  -e "DOCSERVER_EDIT_TYPES=$EDIT_TYPES" "$IMAGE" > /dev/null || fail "docker run failed"
for net in $(tail -n +2 <<< "$NETS"); do
  # shellcheck disable=SC2046
  docker network connect $(alias_args "$net" | sed 's/--network-alias/--alias/') "$net" "$OLD_NAME" || fail "cannot join $net"
done

# ---- health: healthy, then our 403 on the rewrite target from a throwaway container (it is us, and the peer gate holds)
st=""
for _ in $(seq 45); do st=$(insp "$OLD_NAME" '{{.State.Health.Status}}' 2> /dev/null || true); [ "$st" = healthy ] && break; sleep 2; done
[ "$st" = healthy ] || fail "the container is not healthy ($st)"
PROBENET=$(awk -F'|' -v h="$RWHOST" '{n=split($2,a,","); for(i=1;i<=n;i++) if (a[i]==h) {print $1; exit}}' <<< "$OLD_NETS")
probe=$(docker run --rm --network "${PROBENET:-$FIRSTNET}" --read-only --cap-drop ALL --entrypoint bash "$IMAGE" -c \
  "exec 3<>/dev/tcp/$RWHOST/$INTERNAL_PORT && printf 'GET / HTTP/1.0\r\nHost: $RWHOST\r\n\r\n' >&3 && head -n1 <&3" 2>&1 || true)
case "$probe" in *" 403 "*) ;; *) fail "the probe of http://$RWHOST:$INTERNAL_PORT/ got '$probe', not our 403" ;; esac
say "installed: $OLD_NAME is the document server now (healthy; $RWHOST:$INTERNAL_PORT answers 403 to a stranger)"
say "rollback: $0 rollback --etc $ETC"
say "NOTE: the backend's compose file still defines the old service. Do not run 'docker compose up' for it (bring up the backend's own"
say "      services by name), or add an override that disables it; otherwise it comes back and claims $OLD_NAME's names."
