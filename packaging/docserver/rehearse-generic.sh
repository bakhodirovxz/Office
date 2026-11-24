#!/usr/bin/env bash
# The quick-start rehearsal (packaging/docserver/README.md): a FRESH `docker run` of the document server configured only by the
# documented env (no install.sh, no profile, no config file), a generic ONLYOFFICE-API host (ci/web-smoke/generic-host/host.mjs in
# node:alpine) on the same Docker network, and a Chromium run: per docx, xlsx, pptx, pdf open -> edit -> save -> reopen -> download,
# and a view-only open. Secrets and the throwaway CA are made at run time. Needs the image (build.sh) and node:22-alpine locally.
# Usage: rehearse-generic.sh [playwright args, e.g. --grep docx]   Env: IMAGE (default hoshiya-docserver:quickstart), OFFICE_TARGET, KEEP=1, OPENSSL, SHOTS.
set -euo pipefail
export MSYS_NO_PATHCONV=1
HERE=$(cd "$(dirname "$0")" && pwd -W); REPO=$(cd "$HERE/../.." && pwd -W)
T=${OFFICE_TARGET:-E:/office-target/p5-quickstart}; D=$T/rehearse-generic; SSL=${OPENSSL:-openssl}
IMAGE=${IMAGE:-hoshiya-docserver:quickstart}
NET=qs_net DS=qs-docserver APP=qs-app DSPORT=18543 APPPORT=13000
cleanup() { [ "${KEEP:-0}" = 1 ] && return; docker rm -f $DS $APP > /dev/null 2>&1 || true; docker volume rm qs-data > /dev/null 2>&1 || true; docker network rm $NET > /dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup
rm -rf "$D"; mkdir -p "$D/certs" "$D/docs"
cd "$D/certs"
$SSL ecparam -name prime256v1 -genkey -noout -out ca.key
$SSL req -x509 -new -key ca.key -sha256 -days 2 -subj /CN=rehearsal-ca -out ca.pem
$SSL ecparam -name prime256v1 -genkey -noout -out leaf.key
$SSL pkcs8 -topk8 -nocrypt -in leaf.key -out server.key
$SSL req -new -key leaf.key -subj /CN=localhost -out leaf.csr
printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' > ext.cnf
$SSL x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 2 -sha256 -extfile ext.cnf -out server.pem 2> /dev/null
PIN=$($SSL x509 -in server.pem -pubkey -noout | $SSL pkey -pubin -outform der | $SSL dgst -sha256 -binary | base64)
chmod 644 ./*.key ./*.pem
cd "$D/docs"   # synthetic fixtures only
cp "$REPO/domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx" sample.docx
cp "$REPO/domains/sheets/tests/translit-fixtures/xlsx/01-styles.xlsx" sample.xlsx
cp "$REPO/domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx" sample.pptx
cp "$REPO/docs/phase4/wp23/fixtures/namuna-hisobot.pdf" sample.pdf
SECRET=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')   # 48 hex characters, shared by host and document server

docker network create $NET > /dev/null
# THE QUICK START: the settings an integrator types, nothing else (the volume and the certificate mount are not settings)
docker run -d --name $DS --network $NET --read-only --cap-drop ALL --security-opt no-new-privileges:true --tmpfs /tmp:size=256m,uid=10002 -p 127.0.0.1:$DSPORT:8787 -v qs-data:/data -v "$D/certs:/certs:ro" \
  -e JWT_ENABLED=true -e "JWT_SECRET=$SECRET" \
  -e DOCSERVER_PUBLIC_URL=https://localhost:$DSPORT -e DOCSERVER_TLS_CERT=/certs/server.pem -e DOCSERVER_TLS_KEY=/certs/server.key \
  -e DOCSERVER_FETCH_ALLOW=http://$APP:3000 -e DOCSERVER_FRAME_ORIGINS=http://localhost:$APPPORT \
  -e DOCSERVER_INTERNAL_PORT=8780 -e DOCSERVER_INTERNAL_URL=http://$DS:8780 -e DOCSERVER_INTERNAL_ALLOW=$APP \
  "$IMAGE" > /dev/null
docker run -d --name $APP --network $NET -p 127.0.0.1:$APPPORT:3000 -v "$HERE/../../ci/web-smoke/generic-host:/host:ro" -v "$D/docs:/docs:ro" \
  -e PORT=3000 -e DS_URL=https://localhost:$DSPORT -e SELF_URL=http://$APP:3000 -e "JWT_SECRET=$SECRET" node:22-alpine node /host/host.mjs > /dev/null
for _ in $(seq 40); do st=$(docker inspect -f '{{.State.Health.Status}}' $DS); [ "$st" = healthy ] && break; sleep 2; done
echo "docserver health: $st"; [ "$st" = healthy ] || { docker logs $DS 2>&1 | tail -20; exit 1; }
docker logs $DS 2>&1 | head -8
echo "--- api.js and the healthz answer through the published port"
curl -sk --cacert "$D/certs/ca.pem" -m 5 -o /dev/null -w 'api.js %{http_code}\n' https://localhost:$DSPORT/web-apps/apps/api/documents/api.js || true
echo "--- the internal listener is not published"; curl -s -m 3 -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8780/ || true

cd "$REPO/ci/web-smoke"
set +e
PIN=$PIN DS_URL=https://localhost:$DSPORT HOST_URL=http://localhost:$APPPORT SHOTS=${SHOTS:-$REPO/docs/phase5/evidence/generic} \
  npm exec -- playwright test --config playwright.generic.config.ts "$@" 2>&1 | tee "$D/playwright.log"
RC=${PIPESTATUS[0]}
set -e
echo "--- log hygiene: tokens in the server log (must be 0)"
docker logs $DS 2>&1 | grep -c -E "eyJ[A-Za-z0-9_-]{10,}|$SECRET|\?t=" || true
echo "RESULT: playwright exit $RC"
exit "$RC"
