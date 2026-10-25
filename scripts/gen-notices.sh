#!/usr/bin/env bash
# Third-party crate notices for the MSI (gate finding F3). cargo-about 0.9.2 (pinned in scripts/tools.lock), config about.toml.
#   scripts/gen-notices.sh           write packaging/windows/THIRD-PARTY-NOTICES.txt
#   scripts/gen-notices.sh --check   fail when the committed file differs from a fresh run, or a shipped crate lacks a licence
# One cargo-about run per shipped binary (the 4 domain desktops and CLIs, hoshiya, hoshiya-serve, convert, convert-api), merged by
# scripts/gen-notices.mjs. --offline: only the crate sources already in the cargo cache, so the output does not depend on the network.
set -euo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
want=$(awk '$1=="cargo-about"{print $2}' scripts/tools.lock)
got=$(cargo about --version 2>&1 | awk '{print $2}')
[ "$got" = "$want" ] || { echo "cargo-about: want $want got '$got' (cargo install cargo-about --version $want --locked --features cli)" >&2; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
args=()
run() { # <label> <manifest>
  cargo about generate --format json --locked --offline -c about.toml --manifest-path "$2" -o "$tmp/$1.json" >/dev/null
  args+=("$1=$tmp/$1.json")
}
for d in docs sheets slides pdf; do run "$d-desktop" "domains/$d/apps/$d-desktop/Cargo.toml"; run "$d-cli" "domains/$d/apps/$d-cli/Cargo.toml"; done
for d in docs sheets; do run "$d-diff" "domains/$d/crates/$d-diff/Cargo.toml"; done
for p in suite-desktop suite-serve convert convert-api; do run "$p" "services/$p/Cargo.toml"; done
out=packaging/windows/THIRD-PARTY-NOTICES.txt
if [ "${1:-}" = "--check" ]; then
  node scripts/gen-notices.mjs "$tmp/new.txt" "${args[@]}" || exit 1
  cmp -s "$tmp/new.txt" "$out" || { echo "STALE: $out differs from a fresh cargo-about run; run scripts/gen-notices.sh" >&2; exit 1; }
  echo "ok $out is current"
else
  node scripts/gen-notices.mjs "$out" "${args[@]}"
fi
