# Android app: licences (WP8)

`packaging/android/` is written from scratch (MIT OR Apache-2.0). It has **no third-party dependency**: no Gradle, no androidx, no
Kotlin, no support library, no Maven artifact. The APK contains our `classes.dex` (one Java class, `MainActivity`, compiled against the
platform `android.jar`), the generated resources, and the assets: our shell and the four editor dists (their licences are covered by the
repo's NOTICE and the `cargo-deny` output, unchanged). Nothing from ONLYOFFICE, LibreOffice or WPS was used.

Build tools used (not shipped):

| Tool | Source | Licence |
|---|---|---|
| Android SDK build-tools 36.0.0: `aapt2`, `d8`, `zipalign`, `apksigner` | local Android SDK | Apache-2.0 |
| `platforms/android-36/android.jar` (compile-time API only, not packaged) | local Android SDK | Apache-2.0 (Android SDK licence) |
| JDK 21 (`javac`, `jar`, `keytool`), Android Studio's bundled JBR | local | GPLv2 with Classpath Exception (build output is not covered) |

The launcher icons are `brand/logo/android/res` (the Hoshiya logo pack, `brand/logo/LICENSE.txt`).
