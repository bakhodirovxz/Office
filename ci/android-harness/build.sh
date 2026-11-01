#!/usr/bin/env bash
# Test-only APK, no Gradle and no downloads: aapt2 + javac + d8 + zipalign + apksigner from the local SDK and Android Studio's JDK.
# Usage: ci/android-harness/build.sh <test-ca.pem> [out.apk]   (the CA is baked into the APK's trust anchors; generated per run, never committed)
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd -W)"; CA="$(cygpath -m "$1")"; OUT="${2:-E:/office-target/p6-webview/harness/harness.apk}"
SDK="$(cygpath -m "${ANDROID_SDK_ROOT:-$LOCALAPPDATA/Android/Sdk}")"; BT="$SDK/build-tools/36.0.0"; AJ="$SDK/platforms/android-36/android.jar"
export JAVA_HOME="${JAVA_HOME:-/c/Program Files/Android/Android Studio/jbr}"; J="$JAVA_HOME/bin"
OUT="$(cygpath -m "$OUT")"; D="$(dirname "$OUT")"; W="$D/work"; rm -rf "$W"; mkdir -p "$W/res" "$W/cls" "$W/dex"
cp -r "$HERE/res/." "$W/res/"; mkdir -p "$W/res/raw"; cp "$CA" "$W/res/raw/test_ca.pem"
"$BT/aapt2" compile --dir "$W/res" -o "$W/res.zip"
"$BT/aapt2" link -o "$W/base.apk" -I "$AJ" --manifest "$HERE/AndroidManifest.xml" --min-sdk-version 26 --target-sdk-version 33 "$W/res.zip"
"$J/javac" --release 11 -cp "$AJ" -d "$W/cls" $(find "$HERE/src" -name '*.java')
(cd "$W/cls" && "$J/jar" cf ../classes.jar .)
"$BT/d8.bat" --lib "$AJ" --min-api 26 --output "$W/dex" "$W/classes.jar"
(cd "$W/dex" && "$J/jar" uf ../base.apk classes.dex)
"$BT/zipalign" -f 4 "$W/base.apk" "$W/aligned.apk"
[ -f "$D/debug.keystore" ] || "$J/keytool" -genkeypair -keystore "$D/debug.keystore" -storepass android -keypass android -alias d -keyalg RSA -keysize 2048 -validity 3650 -dname "CN=harness" 2>/dev/null
"$BT/apksigner.bat" sign --ks "$D/debug.keystore" --ks-pass pass:android --key-pass pass:android --out "$OUT" "$W/aligned.apk"
echo "built $OUT"
