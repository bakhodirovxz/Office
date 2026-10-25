#!/usr/bin/env bash
# Builds the two rehearsal images (hoshiya-docserver, hoshiya-fakehrm) with nothing fetched inside a container: the crates are vendored
# on the host (cargo vendor --locked), the Linux binaries are compiled in an offline rust container, the dists come from the host builds.
# Usage: packaging/docserver/build.sh        Env: OFFICE_TARGET (default E:/office-target/p5), DOCS_DIST, SHELL_DIST.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH" CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_HTTP_CHECK_REVOKE=false MSYS_NO_PATHCONV=1
HERE=$(cd "$(dirname "$0")" && pwd -W); REPO=$(cd "$HERE/../.." && pwd -W)
T=${OFFICE_TARGET:-E:/office-target/p5}
DOCS_DIST=${DOCS_DIST:-$T/dist/docs-web}      # patch 0127 (the bridge) is in this one
SHELL_DIST=${SHELL_DIST:-$T/dist-wp7/shell}
RUST_IMAGE=${RUST_IMAGE:-rust:1-slim-bookworm}
CTX=$T/image-ctx
mkdir -p "$T/linux" "$CTX/dist"

[ -d "$T/svc-vendor" ] || (cd "$REPO/services" && cargo vendor --locked "$T/svc-vendor" >/dev/null)
printf '[source.crates-io]\nreplace-with = "vendored"\n[source.vendored]\ndirectory = "/vendor"\n[net]\noffline = true\n' > "$T/svc-vendor.toml"

docker run --rm -v "$REPO:/repo:ro" -v "$T/svc-vendor:/vendor:ro" -v "$T/svc-vendor.toml:/usr/local/cargo/config.toml:ro" \
  -v "$T/linux:/target" -e CARGO_TARGET_DIR=/target -w /repo/services "$RUST_IMAGE" \
  cargo build --release --locked --offline -p suite-serve -p fake-hrm

cp "$T/linux/release/hoshiya-serve" "$T/linux/release/fake-hrm" "$CTX/"
rm -rf "$CTX/dist" && mkdir -p "$CTX/dist" && cp -r "$DOCS_DIST" "$CTX/dist/docs-web" && cp -r "$SHELL_DIST" "$CTX/dist/shell"
cp "$REPO/brand/brand.toml" "$CTX/brand.toml"
cp "$REPO/domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx" "$CTX/sample.docx"
docker build -t hoshiya-docserver:rehearsal -f "$HERE/Dockerfile" "$CTX"
docker build -t hoshiya-fakehrm:rehearsal -f "$HERE/Dockerfile.fake-hrm" "$CTX"
docker images --format '{{.Repository}}:{{.Tag}} {{.Size}}' | grep -E '^hoshiya-(docserver|fakehrm)'
