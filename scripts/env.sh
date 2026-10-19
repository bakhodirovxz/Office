# Sourced by every script. Phase 1 environment (PLAN section 1.6).
export PATH="$HOME/.cargo/bin:/e/office-tools/cargo/bin:/e/office-tools/bin:$PATH"
export CARGO_HTTP_CHECK_REVOKE=false CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_INCREMENTAL=0
# git CLI fetches (revocation check fails on this machine)
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=http.schannelCheckRevoke GIT_CONFIG_VALUE_0=false
OFFICE_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || (cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd))"
export OFFICE_ROOT; export OFFICE_TARGET="${OFFICE_TARGET:-E:/office-target/p1}"   # override for a clean checkout: OFFICE_TARGET=E:/office-target/p1/clean/target
export UPSTREAM_CACHE=E:/office-upstream OFFICE_CACHE=E:/office-cache
export RUSTC_WRAPPER=sccache SCCACHE_DIR=$OFFICE_CACHE/sccache SCCACHE_CACHE_SIZE=40G
export CRAFT_FONTS_DIR="$OFFICE_ROOT/upstream/craft-fonts"   # one copy for all domains (F9)
export CARGO_BUILD_TARGET_DIR="$OFFICE_TARGET/core"          # root workspace (xtask, root tests); overrides .cargo/config.toml, so a clean checkout never shares it
export OFFICE_OVERLAY_ROOT=$OFFICE_TARGET/overlay            # per-app OFFICE_OVERLAY_DIR is set by build.sh
# curl is always used with --ssl-no-revoke; Gradle is not needed in Phase 1
