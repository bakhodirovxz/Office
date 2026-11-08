#!/usr/bin/env bash
# The drop-in rehearsal (docs/phase5/dropin.md section 3): an HRM shape on this PC, then install.sh, the browser flow and the rollback.
# Two scenarios, one after the other (SCENARIO=labels|file|both, default both):
#   labels  network backend_net_dropin | Traefik (docker provider) routes the stand-in by its labels | no published port
#           -> two listeners: public :8787 (labels rewritten), saved-file :80
#   file    network backend_net_dropin_file | Traefik with --providers.docker=false and a file provider directory (/dynamic) whose route
#           sends Host(127.0.0.1) to http://docs-alias:80 | the stand-in has no labels, publishes 127.0.0.1:18082:80 and
#           127.0.0.1:18444:443, has a 512 MB memory limit and compose labels: the test server as surveyed on 2026-11-08
#           -> the shared layout: one listener :80 for Traefik's route and the backend's saved-file download
# In both: dropin-onlyoffice is a stand-in for the old Document Server (nginx:alpine tagged locally like ONLYOFFICE's image, aliases
# docs-server and docs-alias, JWT_SECRET in its env, a marker api.js), restart unless-stopped; dropin-api is the fake HR backend
# as `api`, as surveyed (no exp, body token, allowlist by host name, rewrite to http://docs-server, i.e. port 80, FRONTEND_URL).
# Needs the images from build.sh. Never touches a network or container without the dropin prefix.
# Usage: rehearse-dropin.sh   Env: OFFICE_TARGET, SCENARIO, KEEP=1 (leave the last one up), OPENSSL.
set -euo pipefail
export MSYS_NO_PATHCONV=1
HERE=$(cd "$(dirname "$0")" && pwd -W); REPO=$(cd "$HERE/../.." && pwd -W)
T=${OFFICE_TARGET:-E:/office-target/p5}; D=$T/rehearse-dropin; SSL=${OPENSSL:-openssl}
IMAGE=hoshiya-docserver:rehearsal STANDIN=onlyoffice/documentserver:rehearsal-standin
TRAEFIK_IMAGE=traefik:v3.6@sha256:31267173a15b4944e797a76ffd9c419707c8d8b32fe5b610f80cd0cfa05f372d   # v3.1 cannot talk to Docker 29's API
NETS="backend_net_dropin backend_net_dropin_file"
PASS=0 FAILN=0
ok() { echo "PASS [$SC] $*"; PASS=$((PASS + 1)); }
bad() { echo "FAIL [$SC] $*"; FAILN=$((FAILN + 1)); }
check() { local what=$1; shift; if "$@"; then ok "$what"; else bad "$what"; fi; }
down() {
  docker rm -f dropin-onlyoffice dropin-onlyoffice-replaced dropin-api dropin-traefik > /dev/null 2>&1 || true
  docker volume rm dropin-onlyoffice-docserver-data > /dev/null 2>&1 || true
  for n in $NETS; do docker network rm "$n" > /dev/null 2>&1 || true; done
}
cleanup() {
  [ "${KEEP:-0}" = 1 ] && return
  down
  docker rmi "$STANDIN" > /dev/null 2>&1 || true
}
trap cleanup EXIT
SC=setup
cleanup
rm -rf "$D"; mkdir -p "$D/certs"
cd "$D/certs"
$SSL ecparam -name prime256v1 -genkey -noout -out ca.key
$SSL req -x509 -new -key ca.key -sha256 -days 2 -subj /CN=rehearsal-ca -out ca.pem
$SSL ecparam -name prime256v1 -genkey -noout -out leaf.key
$SSL pkcs8 -topk8 -nocrypt -in leaf.key -out server.key
$SSL req -new -key leaf.key -subj /CN=localhost -out leaf.csr
printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' > ext.cnf
$SSL x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 2 -sha256 -extfile ext.cnf -out server.pem 2> /dev/null
PIN=$($SSL x509 -in server.pem -pubkey -noout | $SSL pkey -pubin -outform der | $SSL dgst -sha256 -binary | base64)
chmod 644 ./*.key
SECRET=$(head -c 20 /dev/urandom | od -An -tx1 | tr -d ' \n')   # 40 bytes: the secret ONLYOFFICE and the backend share
docker tag nginx:alpine "$STANDIN"
apijs() { curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js || true; }
code() { curl -sk -m 5 -o /dev/null -w '%{http_code}' "$1" || true; }
STANDIN_CMD='d=/usr/share/nginx/html/web-apps/apps/api/documents; mkdir -p $d && echo "/* stand-in */ window.STANDIN = true;" > $d/api.js && exec nginx -g "daemon off;"'
api() { # <network>: the HRB env as surveyed: BACKEND_DOCS_URL and BACKEND_SELF_URL unset (their defaults), its allowlist, its secret
  docker run -d --name dropin-api --network "$1" --network-alias api -p 127.0.0.1:8790:8000 --read-only --cap-drop ALL \
    -e FAKE_HRM_ADDR=0.0.0.0:8000 -e FAKE_HRM_DOCSERVER=https://127.0.0.1:18443 -e FAKE_HRM_BACKEND_URL=http://api:8000 \
    -e FAKE_HRM_INTERNAL_BASE=http://docs-server -e FAKE_HRM_ALLOWED_HOSTS=docs-server,docs-alias,minio \
    -e "FAKE_HRM_DS_SECRET=$SECRET" -e FAKE_HRM_TEST_API=1 \
    -e BACKEND_CALLBACK_HOSTS=docs-server,docs-alias,minio -e "BACKEND_DOCS_SECRET=$SECRET" \
    -e FRONTEND_URL=http://localhost:8790 hoshiya-fakehrm:rehearsal > /dev/null
}
# the install, the rerun, the isolation probes and the browser flow; the arguments are the scenario's extra install.sh flags
common() {
  local E=$D/$SC/etc
  INSTALL=(bash "$HERE/install.sh" --profile "$HERE/profiles/hrm.env" --image "$IMAGE" --etc "$E" "$@")
  for _ in $(seq 30); do apijs | grep -q STANDIN && break; sleep 1; done
  check "before: Traefik serves the stand-in's api.js" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q STANDIN"
  echo "=== [$SC] install.sh --dry-run"
  "${INSTALL[@]}" --dry-run > dry.txt 2>&1 || { cat dry.txt; bad "dry run"; }
  cat dry.txt
  check "dry run: no secret in its output" bash -c "! grep -q '$SECRET' dry.txt"
  check "dry run: changed nothing" bash -c "[ \"\$(docker inspect -f '{{.State.Running}}' dropin-onlyoffice)\" = true ] && [ ! -e '$E/state.env' ]"
  check "dry run: the frame origin comes from the backend's FRONTEND_URL" grep -q 'frame origins    http://localhost:8790 + ' dry.txt
  check "dry run: reports the image size and the free space" grep -qE '^  image  .*B on disk \(loaded\); ' dry.txt
  check "dry run: the opaque origin is refused by default" grep -q '^  opaque origin    refused' dry.txt
  echo "=== [$SC] install.sh"
  "${INSTALL[@]}" 2>&1 | tee install.txt
  check "install: no secret in its output" bash -c "! grep -q '$SECRET' install.txt"
  check "install: ours is healthy under the old name" bash -c "[ \"\$(docker inspect -f '{{.State.Health.Status}} {{index .Config.Labels \"docserver.dropin.replaces\"}}' dropin-onlyoffice)\" = 'healthy dropin-onlyoffice' ]"
  check "install: the old one is stopped, restart=no, on no network" bash -c "[ \"\$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}} {{len .NetworkSettings.Networks}}' dropin-onlyoffice-replaced)\" = 'false no 0' ]"
  check "install: the secret is not in our container's env" bash -c "! docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dropin-onlyoffice | grep -q '$SECRET'"
  for _ in $(seq 20); do apijs | grep -q DocsAPI && break; sleep 1; done
  check "install: Traefik serves our api.js on the old route" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q 'DocsAPI.DocEditor'"
  echo "=== [$SC] install.sh again (idempotent)"
  "${INSTALL[@]}" > rerun.txt 2>&1 && ok "rerun: succeeds" || { cat rerun.txt; bad "rerun"; }
  check "rerun: recreated ours, kept the replaced one" bash -c "grep -q 'rerun: recreate ours' rerun.txt && docker inspect dropin-onlyoffice-replaced > /dev/null"
  for _ in $(seq 20); do apijs | grep -q DocsAPI && break; sleep 1; done

  echo "=== [$SC] isolation: the saved-file route (port 80 under the old alias)"
  local r
  r=$(docker run --rm --network "$NET" --read-only --cap-drop ALL --entrypoint bash "$IMAGE" -c \
    'exec 3<>/dev/tcp/docs-server/80 && printf "GET /ds/files/x/1 HTTP/1.0\r\nHost: docs-server\r\n\r\n" >&3 && head -n1 <&3' 2>&1 || true)
  check "a stranger on the network gets 403 ($r)" grep -q ' 403 ' <<< "$r"
  r=$(code https://127.0.0.1:18443/ds/files/x/1)
  check "through Traefik the saved-file route is $VIA_PROXY ($r)" [ "$r" = "$VIA_PROXY" ]

  echo "=== [$SC] the browser flow (Chromium on the host): docx open, edit, save, close, reopen, compare; xlsx and pptx edit, save, close"
  cd "$REPO/ci/web-smoke"
  if PIN=$PIN DROPIN=1 node_modules/.bin/playwright test --config playwright.docserver-compose.config.ts; then ok "browser flow"; else bad "browser flow"; fi
  cd "$D/$SC"
  local n
  n=$(docker logs dropin-onlyoffice 2>&1 | grep -c -E "$SECRET|eyJ[A-Za-z0-9_-]{10,}|Bearer|\?t=" || true)
  check "our log holds no secret, token or file URL ($n hits)" [ "$n" = 0 ]
  docker logs dropin-onlyoffice 2>&1 | grep -E "warning|WARNING|shared" | sed 's/^/  log: /' || true
}
# dropin.md D3b: --allow-opaque-origin through the installer, kept by a plain rerun (an upgrade), undone by --no-allow-opaque-origin.
# The browser side of the switch is docserver.spec.ts "D3b"; here: the plan line, the container env and the headers through Traefik.
opaque() {
  local E=$D/$SC/etc
  local I=(bash "$HERE/install.sh" --profile "$HERE/profiles/hrm.env" --image "$IMAGE" --etc "$E" --public-url https://127.0.0.1:18443)
  envv() { docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dropin-onlyoffice | sed -n 's/^DOCSERVER_ALLOW_OPAQUE_ORIGIN=//p'; }
  up() { # wait for ours, then save the wrapper page's headers through Traefik (lower case) to $1
    for _ in $(seq 20); do apijs | grep -q DocsAPI && break; sleep 1; done
    curl -sk -m 5 -D - -o /dev/null https://127.0.0.1:18443/ds/editor | tr -d '\r' | tr 'A-Z' 'a-z' > "$1" || true
  }
  echo "=== [$SC] install.sh --allow-opaque-origin"
  "${I[@]}" --allow-opaque-origin > opq1.txt 2>&1 || { cat opq1.txt; bad "opaque: rerun with the flag"; }
  up h1.txt
  check "opaque: the plan says ALLOWED" grep -q '^  opaque origin    ALLOWED' opq1.txt
  check "opaque: the container has the switch on" [ "$(envv)" = 1 ]
  check "opaque: /ds/editor through Traefik has a CSP without frame-ancestors and no X-Frame-Options" \
    bash -c "grep -q '^content-security-policy: ' h1.txt && ! grep -q 'frame-ancestors' h1.txt && ! grep -q '^x-frame-options' h1.txt"
  check "opaque: the server warns at start" bash -c "docker logs dropin-onlyoffice 2>&1 | grep -q 'allow_opaque_origin = true'"
  echo "=== [$SC] install.sh (a plain rerun keeps the switch)"
  "${I[@]}" > opq2.txt 2>&1 || { cat opq2.txt; bad "opaque: plain rerun"; }
  up h2.txt
  check "opaque: a plain rerun keeps it" bash -c "grep -q '^  opaque origin    ALLOWED.*(kept from the running container)' opq2.txt && ! grep -q frame-ancestors h2.txt"
  check "opaque: ... and the container still has it" [ "$(envv)" = 1 ]
  echo "=== [$SC] install.sh --no-allow-opaque-origin"
  "${I[@]}" --no-allow-opaque-origin > opq3.txt 2>&1 || { cat opq3.txt; bad "opaque: rerun with --no-allow-opaque-origin"; }
  up h3.txt
  check "opaque: --no-allow-opaque-origin puts the frame check back" \
    bash -c "grep -q '^  opaque origin    refused' opq3.txt && grep -q 'frame-ancestors .*http://localhost:8790' h3.txt && [ \"\$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dropin-onlyoffice | sed -n 's/^DOCSERVER_ALLOW_OPAQUE_ORIGIN=//p')\" = 0 ]"
}
rollback() {
  echo "=== [$SC] install.sh rollback"
  bash "$HERE/install.sh" rollback --etc "$D/$SC/etc"
  for _ in $(seq 30); do apijs | grep -q STANDIN && break; sleep 1; done
  check "rollback: the stand-in runs under its name with restart=unless-stopped" bash -c "[ \"\$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}} {{.Config.Image}}' dropin-onlyoffice)\" = 'true unless-stopped $STANDIN' ]"
  check "rollback: its aliases are back" bash -c "docker inspect -f '{{range .NetworkSettings.Networks}}{{join .Aliases \" \"}}{{end}}' dropin-onlyoffice | tr ' ' '\n' | grep -qx docs-server && docker inspect -f '{{range .NetworkSettings.Networks}}{{join .Aliases \" \"}}{{end}}' dropin-onlyoffice | tr ' ' '\n' | grep -qx docs-alias"
  check "rollback: Traefik serves the stand-in's api.js again" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q STANDIN"
  check "rollback: the data volume is kept" bash -c "docker volume inspect dropin-onlyoffice-docserver-data > /dev/null"
}

scenario_labels() {
  SC=labels NET=backend_net_dropin VIA_PROXY=404   # Traefik reaches the public listener, which has no such route
  echo "=== [$SC] the HRM shape before the install: Traefik routes the stand-in by its labels"
  down; mkdir -p "$D/$SC"; cd "$D/$SC"
  docker network create "$NET" > /dev/null
  docker run -d --name dropin-traefik --network "$NET" -p 127.0.0.1:18443:443 --read-only --cap-drop ALL \
    -v /var/run/docker.sock:/var/run/docker.sock:ro -v "$HERE/dropin/traefik.yml:/etc/traefik/traefik.yml:ro" \
    -v "$HERE/dropin/dynamic.yml:/etc/traefik/dynamic.yml:ro" -v "$D/certs:/certs:ro" "$TRAEFIK_IMAGE" > /dev/null
  docker run -d --name dropin-onlyoffice --network "$NET" --network-alias docs-server --network-alias docs-alias \
    --restart unless-stopped -e JWT_ENABLED=true -e "JWT_SECRET=$SECRET" -e JWT_IN_BODY=true \
    -l traefik.enable=true -l 'traefik.http.routers.docs.rule=Host(`127.0.0.1`)' -l traefik.http.routers.docs.entrypoints=websecure \
    -l traefik.http.routers.docs.tls=true -l traefik.http.services.docs.loadbalancer.server.port=80 \
    "$STANDIN" sh -c "$STANDIN_CMD" > /dev/null
  api "$NET"
  common --public-url https://127.0.0.1:18443
  check "layout: two listeners, the labels' port rewritten to 8787" bash -c "grep -q 'layout           two listeners: public :8787, saved-file :80' dry.txt && docker inspect -f '{{index .Config.Labels \"traefik.http.services.docs.loadbalancer.server.port\"}}' dropin-onlyoffice | grep -qx 8787"
  check "port 80 is not published" bash -c "[ -z \"\$(docker port dropin-onlyoffice 80 2> /dev/null)\" ]"
  check "install: says nothing brings the stand-in back (no compose labels)" grep -q 'was not made by docker compose' install.txt
  rollback
}

scenario_file() {
  SC=file NET=backend_net_dropin_file VIA_PROXY=403   # Traefik reaches the shared port, whose file route lets internal_allow only
  echo "=== [$SC] the HRM shape before the install: no labels, a file-provider route to http://docs-alias:80, loopback publishes"
  down; mkdir -p "$D/$SC/dynamic"; cd "$D/$SC"
  # the route the test server has (names as there), a second router for the SPA in another file, and the TLS store
  cat > dynamic/doc-editor.yml << 'Y'
http:
  routers:
    doc-editor:
      rule: "Host(`127.0.0.1`)"
      entryPoints: [websecure]
      service: doc-editor
      tls: {}
  services:
    doc-editor:
      loadBalancer:
        servers:
          - url: "http://docs-alias:80"
Y
  cat > dynamic/hr.yml << 'Y'
http:
  routers:
    hr:
      rule: "Host(`localhost`)"
      entryPoints: [websecure]
      service: hr
      tls: {}
  services:
    hr:
      loadBalancer:
        servers:
          - url: "http://api:8000"
Y
  printf 'tls:\n  stores:\n    default:\n      defaultCertificate:\n        certFile: /certs/server.pem\n        keyFile: /certs/server.key\n' > dynamic/tls.yml
  local before; before=$(cat dynamic/* | sha256sum)
  docker network create "$NET" > /dev/null
  docker run -d --name dropin-traefik --network "$NET" -p 127.0.0.1:18443:443 --read-only --cap-drop ALL \
    -v "$D/$SC/dynamic:/dynamic:ro" -v "$D/certs:/certs:ro" "$TRAEFIK_IMAGE" \
    --providers.docker=false --providers.file.directory=/dynamic --entrypoints.websecure.address=:443 \
    --entrypoints.websecure.forwardedHeaders.trustedIPs=127.0.0.1/32 --log.level=INFO > /dev/null
  standin() { # [publish args]: the old container as on the test server, compose labels included
    docker rm -f dropin-onlyoffice > /dev/null 2>&1 || true
    docker run -d --name dropin-onlyoffice --network "$NET" --network-alias docs-server --network-alias docs-alias \
      --restart unless-stopped --memory 512m -e JWT_ENABLED=true -e "JWT_SECRET=$SECRET" -e JWT_IN_BODY=true "$@" \
      -l com.docker.compose.project=rehearsal -l com.docker.compose.service=docs-server \
      -l com.docker.compose.project.working_dir=<server-path> -l com.docker.compose.project.config_files=<server-path> \
      "$STANDIN" sh -c "$STANDIN_CMD" > /dev/null
  }
  api "$NET"
  local DRY=(bash "$HERE/install.sh" --dry-run --profile "$HERE/profiles/hrm.env" --image "$IMAGE" --etc "$D/$SC/etc")
  echo "=== [$SC] refusals"
  standin -p 18445:443
  if "${DRY[@]}" > r1.txt 2>&1; then bad "a 443 publish on every address is refused"; else
    check "a 443 publish on every address is refused" grep -q 'publishes 443/tcp on every address' r1.txt; fi
  standin -p 127.0.0.1:18082:80 -p 127.0.0.1:18444:443
  printf 'http:\n  services:\n    other:\n      loadBalancer:\n        servers:\n          - url: "http://docs-server:8080"\n' > dynamic/extra.yml
  if "${DRY[@]}" > r2.txt 2>&1; then bad "two ports in the file routes are refused"; else
    check "two ports in the file routes are refused" grep -q '2 different scheme:port pairs' r2.txt; fi
  rm dynamic/extra.yml
  echo "=== [$SC] discovery (no --public-url, no --trusted-proxy)"
  "${DRY[@]}" > disc.txt 2>&1 || { cat disc.txt; bad "discovery dry run"; }
  cat disc.txt
  check "discovery: the file route and its one Host() rule" bash -c "grep -q 'doc-editor.yml:[0-9]*:http://docs-alias:80' disc.txt && grep -q 'public           https://127.0.0.1  listener :80  trusted proxies dropin-traefik' disc.txt"
  check "discovery: the shared layout, 443 dropped, 80 mapped to ours, the memory limit" bash -c "grep -q 'layout           shared: one listener :80' disc.txt && grep -q 'ports -p 127.0.0.1:18082:80;' disc.txt && grep -q '127.0.0.1:18444->443 dropped' disc.txt && grep -q 'memory           512 MB' disc.txt"
  # the rehearsal's Traefik listens on 18443, which a Host() rule does not say: the real install gets --public-url
  common --public-url https://127.0.0.1:18443
  check "install: the memory limit carried over" bash -c "[ \"\$(docker inspect -f '{{.HostConfig.Memory}}' dropin-onlyoffice)\" = 536870912 ]"
  check "install: published 127.0.0.1:18082 -> our :80 only" bash -c "[ \"\$(docker port dropin-onlyoffice | tr -d '\r')\" = '80/tcp -> 127.0.0.1:18082' ]"
  local c1 c2; c1=$(code http://127.0.0.1:18082/ds/files/x/1) c2=$(code http://127.0.0.1:18082/ds/api.js)
  check "the loopback publish: the saved-file route is 403 ($c1)" [ "$c1" = 403 ]
  check "the loopback publish: the public routes answer the proxy only ($c2)" [ "$c2" = 403 ]
  check "install: prints the compose override and writes nothing of it" bash -c "grep -q '^  docs-server:' install.txt && grep -q 'profiles: \[\"disabled\"\]' install.txt && grep -q '<server-path>' install.txt"
  check "Traefik's files are unchanged" [ "$(cat dynamic/* | sha256sum)" = "$before" ]
  opaque
  rollback
  check "rollback: the loopback publishes are the stand-in's again" bash -c "docker port dropin-onlyoffice | tr -d '\r' | grep -q '127.0.0.1:18444' && docker port dropin-onlyoffice | tr -d '\r' | grep -q '127.0.0.1:18082'"
}

case "${SCENARIO:-both}" in
  labels) scenario_labels ;;
  file) scenario_file ;;
  both) scenario_labels; scenario_file ;;
  *) echo "SCENARIO is labels, file or both" >&2; exit 64 ;;
esac
echo "=== $PASS passed, $FAILN failed"
[ "$FAILN" = 0 ]
