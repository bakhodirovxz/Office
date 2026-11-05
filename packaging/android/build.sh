#!/usr/bin/env bash
# The Android app (PLAN phase 6 WP8), no Gradle and no downloads: aapt2 + javac + d8 + zipalign + apksigner from the local SDK and Android Studio's JDK
# (same method as ci/android-harness/build.sh). Label, id and version come from brand/brand.toml; the icons from brand/logo/android.
# Usage: packaging/android/build.sh [dist-root] [out.apk]
#   dist-root: holds docs-web/ sheets-web/ slides-web/ pdf-web/ (default: E:/office-target/p6-android/dist, see docs/phase6/android-app.md)
# Debug signing key: $KEYSTORE (default E:/office-target/p6-android/signing/debug.keystore), generated on first run, never committed.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd -W)"; ROOT="$(cd "$HERE/../.." && pwd -W)"
DIST="$(cygpath -m "${1:-E:/office-target/p6-android/dist}")"; OUT="$(cygpath -m "${2:-E:/office-target/p6-android/office.apk}")"
KEYSTORE="$(cygpath -m "${KEYSTORE:-E:/office-target/p6-android/signing/debug.keystore}")"
SDK="$(cygpath -m "${ANDROID_SDK_ROOT:-$LOCALAPPDATA/Android/Sdk}")"; BT="$SDK/build-tools/36.0.0"; AJ="$SDK/platforms/android-36/android.jar"
export JAVA_HOME="${JAVA_HOME:-/c/Program Files/Android/Android Studio/jbr}"; J="$JAVA_HOME/bin"
W="$(dirname "$OUT")/work"; rm -rf "$W"; mkdir -p "$W/res" "$W/cls" "$W/dex" "$W/gen" "$(dirname "$KEYSTORE")"
read -r APPID VCODE VNAME < <(python3 "$HERE/gen-brand.py" "$ROOT/brand/brand.toml" "$HERE/AndroidManifest.xml" "$W/gen" | tr -d '\r')
cp -r "$ROOT/brand/logo/android/res/." "$W/res/"; cp -r "$W/gen/res/." "$W/res/"
# assets: the four dists (without the precompressed .br: the APK compresses, and we cannot send a Content-Encoding) and the shell, assembled from the repo
# exactly as scripts/shell-dist.sh does (web/shell + fallback font + the logo pack's web icons)
A="$W/assets/web"; mkdir -p "$A/shell"; cp "$W/gen/assets/web/shell-config.json" "$A/"
cp "$W/gen/assets/plugin-keys.txt" "$W/assets/"   # trusted publisher keys (Plugins.trustedKeys), not under web/
for d in docs-web sheets-web slides-web pdf-web; do
  [ -d "$DIST/$d" ] || { echo "build.sh: $DIST/$d is missing" >&2; exit 2; }
  mkdir -p "$A/$d"; (cd "$DIST/$d" && find . -type f ! -name '*.br' -print0 | tar --null -T - -cf -) | (cd "$A/$d" && tar xf -)
done
cp -r "$ROOT/web/shell/." "$A/shell/"; cp "$ROOT/assets/fonts/fallback/NotoSans-Regular-uz.ttf" "$A/shell/"
# first-party plugins (Phase 7 WP9): every plugins/<name> with a plugin.toml is packed by `cargo xt plugin pack` (cargo on PATH; CARGO_TARGET_DIR / OFFICE_TARGET as for any
# build) and unpacked to assets/web/plugin-pkg/<id>/; MainActivity's Plugins store lists, verifies and serves them. No code change when a plugin is added.
for pk in "$ROOT"/plugins/*/plugin.toml; do
  [ -f "$pk" ] || continue
  line="$(cd "$ROOT" && cargo xt plugin pack "$(basename "$(dirname "$pk")")" | tail -n 1)"
  python3 -I - "$line" "$A/plugin-pkg" <<'PY'
import re, sys, zipfile, pathlib
m = re.match(r"[0-9a-f]{64}  (.+) \(\d+ bytes\)$", sys.argv[1]); z = zipfile.ZipFile(m.group(1))
d = pathlib.Path(sys.argv[2]) / re.sub(r"-[^-]+\.zip$", "", pathlib.Path(m.group(1)).name); d.mkdir(parents=True)
for n in z.namelist(): (d / n).write_bytes(z.read(n))
PY
done
mkdir -p "$A/shell/plugins"; cp "$ROOT/web/plugins/host.js" "$ROOT/web/plugins/worker.js" "$ROOT/web/plugins/host.css" "$ROOT/web/plugins/page.js" "$ROOT/web/plugins/i18n.json" "$A/shell/plugins/"   # Phase 7 WP5 (MainActivity gives plugins/*.js the Worker CSP; page.js is the Plugins page)
L="$ROOT/brand/logo"; cp "$L/web/favicon.ico" "$L/web/favicon.svg" "$L/web/apple-touch-icon.png" "$L/web/icon-192.png" "$L/web/icon-512.png" "$L/web/icon-maskable-512.png" "$A/shell/"
cp "$L/svg/hoshiya-lockup-light.svg" "$A/shell/logo-light.svg"; cp "$L/svg/hoshiya-lockup-dark.svg" "$A/shell/logo-dark.svg"
sed 's#"/icon#"icon#' "$L/web/site.webmanifest" > "$A/shell/site.webmanifest"
"$BT/aapt2" compile --dir "$W/res" -o "$W/res.zip"
"$BT/aapt2" link -o "$W/base.apk" -I "$AJ" --manifest "$W/gen/AndroidManifest.xml" --rename-manifest-package "$APPID" --version-code "$VCODE" --version-name "$VNAME" \
  --min-sdk-version 26 --target-sdk-version 35 --debug-mode "$W/res.zip"
"$J/javac" --release 11 -cp "$AJ" -d "$W/cls" $(find "$HERE/src" -name '*.java')
(cd "$W/cls" && "$J/jar" cf ../classes.jar .)
"$BT/d8.bat" --lib "$AJ" --min-api 26 --output "$W/dex" "$W/classes.jar"
(cd "$W/dex" && "$J/jar" uf ../base.apk classes.dex)
# assets by jar, not aapt2 -A: aapt2 on Windows writes backslashes into the entry names
(cd "$W" && "$J/jar" uf base.apk assets)
"$BT/zipalign" -f 4 "$W/base.apk" "$W/aligned.apk"
[ -f "$KEYSTORE" ] || "$J/keytool" -genkeypair -keystore "$KEYSTORE" -storepass android -keypass android -alias debug -keyalg RSA -keysize 2048 -validity 3650 -dname "CN=debug" 2>/dev/null
"$BT/apksigner.bat" sign --ks "$KEYSTORE" --ks-pass pass:android --key-pass pass:android --out "$OUT" "$W/aligned.apk"
echo "built $OUT ($(stat -c %s "$OUT") bytes) id=$APPID"
