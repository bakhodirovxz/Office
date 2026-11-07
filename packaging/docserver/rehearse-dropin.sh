#!/usr/bin/env bash
# The drop-in rehearsal (docs/phase5/dropin.md section 3): the HRM shape on this PC, then install.sh, the browser flow and the rollback.
#   network backend_net_dropin (no fixed subnet) | Traefik (docker provider, TLS on 127.0.0.1:18443)
#   dropin-onlyoffice: a stand-in for the old Document Server (nginx:alpine tagged locally like ONLYOFFICE's image, aliases docs-server and
#     docs-alias, JWT_SECRET in its env, Traefik labels, a marker api.js), restart unless-stopped
#   dropin-api: the fake HR backend as `api`, as surveyed (no exp, body token, allowlist by host name, rewrite to http://docs-server)
# Needs the images from build.sh. Usage: rehearse-dropin.sh   Env: OFFICE_TARGET, KEEP=1 (leave it up), OPENSSL.
set -euo pipefail
export MSYS_NO_PATHCONV=1
HERE=$(cd "$(dirname "$0")" && pwd -W); REPO=$(cd "$HERE/../.." && pwd -W)
T=${OFFICE_TARGET:-E:/office-target/p5}; D=$T/rehearse-dropin; SSL=${OPENSSL:-openssl}
NET=backend_net_dropin IMAGE=hoshiya-docserver:rehearsal STANDIN=onlyoffice/documentserver:rehearsal-standin
TRAEFIK_IMAGE=traefik:v3.6@sha256:31267173a15b4944e797a76ffd9c419707c8d8b32fe5b610f80cd0cfa05f372d   # v3.1 cannot talk to Docker 29's API
PASS=0 FAILN=0
ok() { echo "PASS $*"; PASS=$((PASS + 1)); }
bad() { echo "FAIL $*"; FAILN=$((FAILN + 1)); }
check() { local what=$1; shift; if "$@"; then ok "$what"; else bad "$what"; fi; }
cleanup() {
  [ "${KEEP:-0}" = 1 ] && return
  docker rm -f dropin-onlyoffice dropin-onlyoffice-replaced dropin-api dropin-traefik > /dev/null 2>&1 || true
  docker volume rm dropin-onlyoffice-docserver-data > /dev/null 2>&1 || true
  docker network rm "$NET" > /dev/null 2>&1 || true
  docker rmi "$STANDIN" > /dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup
rm -rf "$D"; mkdir -p "$D/certs" "$D/etc"
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
cd "$D"
SECRET=$(head -c 20 /dev/urandom | od -An -tx1 | tr -d ' \n')   # 40 bytes: the secret ONLYOFFICE and the backend share
{ cat "$HERE/profiles/hrm.env"; echo "ORIGIN_ENVS=CORS_ORIGINS"; } > profile.env

echo "=== the HRM shape before the install"
docker network create "$NET" > /dev/null
docker tag nginx:alpine "$STANDIN"
docker run -d --name dropin-traefik --network "$NET" -p 127.0.0.1:18443:443 --read-only --cap-drop ALL \
  -v /var/run/docker.sock:/var/run/docker.sock:ro -v "$HERE/dropin/traefik.yml:/etc/traefik/traefik.yml:ro" \
  -v "$HERE/dropin/dynamic.yml:/etc/traefik/dynamic.yml:ro" -v "$D/certs:/certs:ro" "$TRAEFIK_IMAGE" > /dev/null
docker run -d --name dropin-onlyoffice --network "$NET" --network-alias docs-server --network-alias docs-alias \
  --restart unless-stopped -e JWT_ENABLED=true -e "JWT_SECRET=$SECRET" -e JWT_IN_BODY=true \
  -l traefik.enable=true -l 'traefik.http.routers.docs.rule=Host(`127.0.0.1`)' -l traefik.http.routers.docs.entrypoints=websecure \
  -l traefik.http.routers.docs.tls=true -l traefik.http.services.docs.loadbalancer.server.port=80 \
  "$STANDIN" sh -c 'd=/usr/share/nginx/html/web-apps/apps/api/documents; mkdir -p $d && echo "/* stand-in */ window.STANDIN = true;" > $d/api.js && exec nginx -g "daemon off;"' > /dev/null
# the HRB env as surveyed: BACKEND_DOCS_URL and BACKEND_SELF_URL unset (their defaults), its allowlist, its secret, a CORS list
docker run -d --name dropin-api --network "$NET" --network-alias api -p 127.0.0.1:8790:8000 --read-only --cap-drop ALL \
  -e FAKE_HRM_ADDR=0.0.0.0:8000 -e FAKE_HRM_DOCSERVER=https://127.0.0.1:18443 -e FAKE_HRM_BACKEND_URL=http://api:8000 \
  -e FAKE_HRM_INTERNAL_BASE=http://docs-server -e FAKE_HRM_ALLOWED_HOSTS=docs-server,docs-alias,minio \
  -e "FAKE_HRM_DS_SECRET=$SECRET" -e FAKE_HRM_TEST_API=1 \
  -e BACKEND_CALLBACK_HOSTS=docs-server,docs-alias,minio -e "BACKEND_DOCS_SECRET=$SECRET" \
  -e CORS_ORIGINS=http://localhost:8790 hoshiya-fakehrm:rehearsal > /dev/null
apijs() { curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js || true; }
for _ in $(seq 30); do apijs | grep -q STANDIN && break; sleep 1; done
check "before: Traefik serves the stand-in's api.js" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q STANDIN"

INSTALL=(bash "$HERE/install.sh" --profile "$D/profile.env" --image "$IMAGE" --etc "$D/etc" --public-url https://127.0.0.1:18443)
echo "=== install.sh --dry-run"
"${INSTALL[@]}" --dry-run > dry.txt 2>&1 || { cat dry.txt; bad "dry run"; }
cat dry.txt
check "dry run: no secret in its output" bash -c "! grep -q '$SECRET' dry.txt"
check "dry run: changed nothing" bash -c "[ \"\$(docker inspect -f '{{.State.Running}}' dropin-onlyoffice)\" = true ] && [ ! -e '$D/etc/state.env' ]"
echo "=== install.sh"
"${INSTALL[@]}" 2>&1 | tee install.txt
check "install: no secret in its output" bash -c "! grep -q '$SECRET' install.txt"
check "install: ours is healthy under the old name" bash -c "[ \"\$(docker inspect -f '{{.State.Health.Status}} {{index .Config.Labels \"docserver.dropin.replaces\"}}' dropin-onlyoffice)\" = 'healthy dropin-onlyoffice' ]"
check "install: the old one is stopped, restart=no, on no network" bash -c "[ \"\$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}} {{len .NetworkSettings.Networks}}' dropin-onlyoffice-replaced)\" = 'false no 0' ]"
check "install: the secret is not in our container's env" bash -c "! docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dropin-onlyoffice | grep -q '$SECRET'"
for _ in $(seq 20); do apijs | grep -q DocsAPI && break; sleep 1; done
check "install: Traefik serves our api.js on the old route" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q 'DocsAPI.DocEditor'"
echo "=== install.sh again (idempotent)"
"${INSTALL[@]}" > rerun.txt 2>&1 && ok "rerun: succeeds" || { cat rerun.txt; bad "rerun"; }
check "rerun: recreated ours, kept the replaced one" bash -c "grep -q 'rerun: recreate ours' rerun.txt && docker inspect dropin-onlyoffice-replaced > /dev/null"
for _ in $(seq 20); do apijs | grep -q DocsAPI && break; sleep 1; done

echo "=== isolation: the internal listener (port 80 under the old alias)"
r=$(docker run --rm --network "$NET" --read-only --cap-drop ALL --entrypoint bash "$IMAGE" -c \
  'exec 3<>/dev/tcp/docs-server/80 && printf "GET /ds/files/x/1 HTTP/1.0\r\nHost: docs-server\r\n\r\n" >&3 && head -n1 <&3' 2>&1 || true)
check "a stranger on the network gets 403 ($r)" grep -q ' 403 ' <<< "$r"
check "port 80 is not published" bash -c "[ -z \"\$(docker port dropin-onlyoffice 80 2> /dev/null)\" ]"

echo "=== the browser flow (Chromium on the host): docx open, edit, save, close, reopen, compare; xlsx view-only"
cd "$REPO/ci/web-smoke"
if PIN=$PIN DROPIN=1 node_modules/.bin/playwright test --config playwright.docserver-compose.config.ts; then ok "browser flow"; else bad "browser flow"; fi
cd "$D"
n=$(docker logs dropin-onlyoffice 2>&1 | grep -c -E "$SECRET|eyJ[A-Za-z0-9_-]{10,}|Bearer|\?t=" || true)
check "our log holds no secret, token or file URL ($n hits)" [ "$n" = 0 ]
docker logs dropin-onlyoffice 2>&1 | grep -E "warning|WARNING" | sed 's/^/  log: /' || true

echo "=== install.sh rollback"
bash "$HERE/install.sh" rollback --etc "$D/etc"
for _ in $(seq 30); do apijs | grep -q STANDIN && break; sleep 1; done
check "rollback: the stand-in runs under its name with restart=unless-stopped" bash -c "[ \"\$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}} {{.Config.Image}}' dropin-onlyoffice)\" = 'true unless-stopped $STANDIN' ]"
check "rollback: its aliases are back" bash -c "docker inspect -f '{{range .NetworkSettings.Networks}}{{join .Aliases \" \"}}{{end}}' dropin-onlyoffice | tr ' ' '\n' | grep -qx docs-server && docker inspect -f '{{range .NetworkSettings.Networks}}{{join .Aliases \" \"}}{{end}}' dropin-onlyoffice | tr ' ' '\n' | grep -qx docs-alias"
check "rollback: Traefik serves the stand-in's api.js again" bash -c "curl -sk -m 5 https://127.0.0.1:18443/web-apps/apps/api/documents/api.js | grep -q STANDIN"
check "rollback: the data volume is kept" docker volume inspect dropin-onlyoffice-docserver-data
echo "=== $PASS passed, $FAILN failed"
[ "$FAILN" = 0 ]
