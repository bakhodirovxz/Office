#!/usr/bin/env bash
# Desktop-JVM check that Signatures.java (the Android Ed25519 verifier) agrees with crates/suite-core/src/plugins.rs: RFC 8032 vectors and the
# ci/plugins/signing fixture. Same JDK as build.sh. Usage: packaging/android/sigcheck.sh
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd -W)"; ROOT="$(cd "$HERE/../.." && pwd -W)"
export JAVA_HOME="${JAVA_HOME:-/c/Program Files/Android/Android Studio/jbr}"; J="$JAVA_HOME/bin"
W="${TMP:-E:/office-target/tmp}/sigcheck"; rm -rf "$W"; mkdir -p "$W"
"$J/javac" --release 11 -d "$W" "$HERE/src/office/android/Signatures.java" "$HERE/sigcheck/SigCheck.java"
"$J/java" -cp "$W" office.android.SigCheck "$ROOT/ci/plugins/signing/pkg" "$ROOT/ci/plugins/signing/test-a.pub"
