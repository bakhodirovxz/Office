#!/usr/bin/env bash
# Download the wheels named in requirements.lock into ./wheelhouse (pip checks every sha256).
# Done on the host because the Docker build network here sits behind a TLS-intercepting proxy.
set -euo pipefail
cd "$(dirname "$0")"
PY=${PYTHON:-python}
args=()
for p in manylinux2014_x86_64 manylinux_2_17_x86_64 manylinux_2_27_x86_64 manylinux_2_28_x86_64 any; do args+=(--platform "$p"); done
"$PY" -m pip download --no-deps --require-hashes --only-binary=:all: --python-version 3.12 --implementation cp \
  "${args[@]}" -r requirements.lock -d wheelhouse
