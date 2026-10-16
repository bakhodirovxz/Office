Spike 3 approach B (hybrid). Results: docs/phase0/mobile-b.md. Throwaway, nothing here is product code.
bind/            wordcraft-bind crate (own workspace; path deps into E:/office-work/wordcraft-b, branch spike/mobile-b from fd0bb2f). cargo test --release
web/             Trunk page (index.html, app.js, serve.py, desktop-check.js). Build: trunk build --release  (dist E:/office-target/mobile-b/dist)
android-webview/ B5: plain Android WebView APK that serves the dist from its assets (copy dist into app/src/main/assets first)
scripts/         emulator harness (adapted from spikes/mobile-a/probe/scripts), CDP helpers, bench.js, typeburst.js
fixtures/        sample.docx (HRM fixture), long.docx (4 pages, made by bind/examples/gen_fixture.rs)
Env: CARGO_TARGET_DIR=E:/office-target/mobile-b, -j 4; wait for no rustc and 3 GB free before building.
