#!/usr/bin/env bash
# Builds the keyboard MSI (docs/keyboard/PLAN.md section 8): both DLLs (MSVC, static CRT), stage, WiX 5.0.2. Installs nothing.
# Output: $KBD_MSI_OUT (default E:/office-target/kbd-msi)/<brand id>-keyboard-<version>-x64.msi
# Env: CARGO_TARGET_DIR (DLL build output; default E:/office-target/kbd-tip), WIX (wix.exe), KBD_OFFLINE (cargo flag, default --offline).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-E:/office-target/kbd-tip}"
out="${KBD_MSI_OUT:-E:/office-target/kbd-msi}"
wix="${WIX:-E:/office-tools/wix-5.0.2/PFiles64/WiX Toolset v5.0/bin/wix.exe}"
# FROZEN, generated once (2026-11-22): never change it, or a new MSI no longer replaces the installed one.
# Not the suite's brand.toml upgrade_code: this is a separate product.
upgrade_code='{DB93EEF6-7035-47C3-8F65-D3536D2FBD68}'

brand() { sed -n '/^\[product\]/,/^\[/{s/^'"$1"' = "\(.*\)"/\1/p}' "$root/brand/brand.toml"; }
name="$(brand name)"; id="$(brand id)"; vendor="$(brand vendor)"; version="$(brand version)"
[ -n "$name" ] && [ -n "$id" ] && [ -n "$vendor" ] || { echo "brand/brand.toml [product] name/id/vendor missing" >&2; exit 1; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "brand.toml version must be X.Y.Z, got '$version'" >&2; exit 1; }
[ -x "$wix" ] || { echo "wix.exe not found: $wix (set WIX; scripts/fetch-wix.ps1 on branch ms-addin)" >&2; exit 1; }

cd "$here/.."
cargo +stable-x86_64-pc-windows-msvc build --release ${KBD_OFFLINE---offline}
cargo +stable-x86_64-pc-windows-msvc build --release ${KBD_OFFLINE---offline} --target i686-pc-windows-msvc
stage="$out/stage"
rm -rf "$stage"; mkdir -p "$stage/x64" "$stage/x86"
cp "$CARGO_TARGET_DIR/release/kbd.dll" "$stage/x64/"
cp "$CARGO_TARGET_DIR/i686-pc-windows-msvc/release/kbd.dll" "$stage/x86/"
cp "$root/NOTICE" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$stage/"

msi="$out/$id-keyboard-$version-x64.msi"
tmp="$out/tmp"; mkdir -p "$tmp"; export TEMP="$tmp" TMP="$tmp"   # wix scratch off C:
"$wix" build -arch x64 -o "$msi" "$here/keyboard.wxs" \
  -d "Stage=$stage" -d "ProductName=$name keyboard" -d "Brand=$name" -d "Vendor=$vendor" -d "Version=$version" -d "UpgradeCode=$upgrade_code"
rm -rf "$stage" "$tmp" "${msi%.msi}.wixpdb"
echo "built $msi ($(( $(stat -c %s "$msi") / 1024 )) KB, version $version)"
