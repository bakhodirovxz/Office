#!/usr/bin/env bash
# Run the sidecar with hard limits: no egress (internal network), read-only root, capped tmpfs, non-root.
# Usage: run.sh [image] ; env PORT (default 18081) is published on 127.0.0.1 only.
set -euo pipefail
export MSYS_NO_PATHCONV=1  # Git Bash would rewrite /app/... into a Windows path
IMAGE=${1:-office/soffice:dev}
NAME=${NAME:-soffice-sidecar}
PORT=${PORT:-18081}
NET=${NET:-soffice-internal}
# An internal network has no route out but cannot publish ports. So the sidecar sits on it alone, and a relay
# container (same image, dev only) is on the default bridge (published to 127.0.0.1 only) and on the internal net.
docker network inspect "$NET" >/dev/null 2>&1 || docker network create --internal "$NET" >/dev/null
docker rm -f "$NAME" "$NAME-relay" >/dev/null 2>&1 || true
docker run -d --init --name "$NAME" --network "$NET" --memory 2g --memory-swap 2g --cpus 2 --pids-limit 256 \
  --read-only --tmpfs /tmp:rw,nosuid,size=512m --cap-drop ALL --security-opt no-new-privileges "$IMAGE" >/dev/null
docker run -d --name "$NAME-relay" -p "127.0.0.1:$PORT:8080" -e "TARGET=$NAME:8080" \
  --memory 64m --pids-limit 64 --read-only --cap-drop ALL --security-opt no-new-privileges \
  "$IMAGE" python3 -I /app/relay.py >/dev/null
docker network connect "$NET" "$NAME-relay"
echo "sidecar $NAME on internal net $NET (no egress); http://127.0.0.1:$PORT -> $NAME:8080"
