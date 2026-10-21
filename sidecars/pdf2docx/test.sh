#!/usr/bin/env bash
# Run the unittest suite inside the image, under the same limits as production.
set -euo pipefail
docker run --rm --read-only --tmpfs /tmp:rw,size=512m --memory 2g --cpus 2 --pids-limit 128 \
  --network none "${1:-office/pdf2docx:dev}" python -m unittest discover -s /app/tests -v
