#!/usr/bin/env bash
# Run the unittest suite inside the image, under the same limits as production.
set -euo pipefail
export MSYS_NO_PATHCONV=1  # Git Bash would rewrite /app/tests into a Windows path
docker run --rm --init --read-only --tmpfs /tmp:rw,size=512m --memory 2g --cpus 2 --pids-limit 256 \
  --network none "${1:-office/soffice:dev}" python3 -I -m unittest discover -s /app/tests -v
