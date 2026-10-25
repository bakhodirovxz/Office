#!/usr/bin/env bash
# The local rehearsal (WP10): compose up (Traefik, document server, fake HR backend on backend_net), isolation probe, the browser flow 3x, down.
# Needs the images from build.sh. Usage: rehearse.sh [runs]   Env: OFFICE_TARGET, KEEP=1 (leave the containers up), OPENSSL.
set -euo pipefail
export MSYS_NO_PATHCONV=1 PATH="$PATH"
HERE=$(cd "$(dirname "$0")" && pwd -W); REPO=$(cd "$HERE/../.." && pwd -W)
T=${OFFICE_TARGET:-E:/office-target/p5}; D=$T/rehearse; RUNS=${1:-3}; SSL=${OPENSSL:-openssl}
export REHEARSE_DIR=$D
C="docker compose -f $HERE/compose.yml"
cleanup() { [ "${KEEP:-0}" = 1 ] || $C down -v --remove-orphans >/dev/null 2>&1 || true; }
trap cleanup EXIT
rm -rf "$D"; mkdir -p "$D/certs" "$D/config" "$D/secrets"
cd "$D/certs"
$SSL ecparam -name prime256v1 -genkey -noout -out ca.key
$SSL req -x509 -new -key ca.key -sha256 -days 2 -subj /CN=rehearsal-ca -out ca.pem
$SSL ecparam -name prime256v1 -genkey -noout -out leaf.key
$SSL pkcs8 -topk8 -nocrypt -in leaf.key -out server.key
$SSL req -new -key leaf.key -subj /CN=localhost -out leaf.csr
printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' > ext.cnf
$SSL x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 2 -sha256 -extfile ext.cnf -out server.pem 2>/dev/null
PIN=$($SSL x509 -in server.pem -pubkey -noout | $SSL pkey -pubin -outform der | $SSL dgst -sha256 -binary | base64)
chmod 644 *.key
cd "$D"
head -c 48 /dev/urandom > secrets/server-secret; head -c 48 /dev/urandom > secrets/converter-secret; head -c 48 /dev/urandom | od -An -tx1 | tr -d ' \n' > secrets/audit-key
cp "$HERE/api.rehearsal.toml" config/api.toml
chmod 644 secrets/* config/*

$C up -d traefik fakehrm
# the harness makes its secret at start; the provider entry of the server reads it from a file (HRB: CRAFT_DOCSERVER_JWT_SECRET)
for i in $(seq 30); do S=$(docker logs hoshiya-rehearsal-fakehrm-1 2>&1 | sed -n 's/.*(secret \([0-9a-f]*\),.*/\1/p'); [ -n "$S" ] && break; sleep 1; done
[ -n "$S" ] || { echo "harness did not print its secret"; exit 1; }
printf %s "$S" > secrets/provider-secret; chmod 644 secrets/provider-secret
$C up -d docserver
for i in $(seq 40); do st=$(docker inspect -f '{{.State.Health.Status}}' hoshiya-rehearsal-docserver-1); [ "$st" = healthy ] && break; sleep 2; done
echo "docserver health: $st"; [ "$st" = healthy ] || { docker logs hoshiya-rehearsal-docserver-1 2>&1 | tail -20; exit 1; }
docker logs hoshiya-rehearsal-docserver-1 2>&1 | head -5

echo "--- isolation: the internal listener from a container that is not the harness (must be refused)"
docker run --rm --network backend_net_rehearsal alpine:latest sh -c 'wget -q -S -T 5 -O- http://172.29.50.10:8780/ 2>&1 | head -3; echo "wget exit: $?"' || true
echo "--- and from the host (the port is not published; must fail)"
curl -s -m 3 -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8780/ || true
echo "--- the harness reaches it only for its own flow (checked by the browser runs below)"

cd "$REPO/ci/web-smoke"
for n in $(seq "$RUNS"); do
  echo "=== run $n/$RUNS"
  PIN=$PIN npm exec -- playwright test --config playwright.docserver-compose.config.ts
done
echo "--- audit and log hygiene"
docker logs hoshiya-rehearsal-docserver-1 2>&1 | grep -c -E 'eyJ[A-Za-z0-9_-]{10,}|Bearer|\?t=' || true
