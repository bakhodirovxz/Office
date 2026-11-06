# Plugins on Android (Phase 7 WP9)

How the Android app carries plugins (PLAN section 8, Android row) and how it was checked. Code: `packaging/android/src/office/android/`
(`Plugins.java` store, `Toml.java` manifest reader, `MainActivity.java` loader and bridge), `packaging/android/build.sh`,
`web/plugins/page.js`. Evidence: `docs/phase7/evidence/android/` (JSON and screenshots per item), recordings in `E:/office-target/p7-evidence/evidence/p7-android/`
with their SHA-256 in `docs/phase7/evidence/SHA256SUMS`. Runner: `scripts/mobile/android-plugins.mjs`; test packages: `scripts/mobile/fixtures/make-test-packages.py`.

## What is in the app

- **Bundled plugins.** `build.sh` runs `cargo xt plugin pack <name>` for every `plugins/*/plugin.toml` and unpacks the package to
  `assets/web/plugin-pkg/<id>/` (plugin.toml, plugin.wasm, strings.tsv). A new first-party plugin needs no code change. Needs cargo on PATH.
- **Loader.** `/shell-config.json` is served with the enabled plugins' descriptors added (`id`, `version`, `sha256`, `wasm: /plugins/<sha256>`, manifest as JSON,
  `grant: []`). `/plugins/<sha256>` serves the wasm of an enabled plugin after hashing it again; anything else is 404. `/shell/plugins/*.js` (the Worker
  script) keeps the Worker CSP header.
- **Install from file (O3: allowed locally).** The Plugins page (shell nav button, shown when `window.SuiteHost` exists) calls `SuiteHost.plugins(json)`:
  `plugins_install` opens the system picker (`ACTION_OPEN_DOCUMENT`), the zip is checked (entry names, sizes, wasm magic, id, `api`, `apps`) and kept in
  `files/plugins/<id>/` with `install.toml` (`sha256`, `manifest_sha256`, the same record as `suite-core::plugins`). A package changed on disk later is listed
  with a problem and never served. A bundled id cannot be replaced.
- **Enable / disable.** `SharedPreferences("plugins")`, key `on.<id>`; bundled default on, installed default off (the user turns it on). A change applies when
  the app restarts (the page says so: `plg.page.restart`).
- **No network.** The manifest has no INTERNET permission, `shouldInterceptRequest` answers 404 for any other host, descriptors carry `grant: []`, the page
  shows no network rows (`network: []`, `policy.user_grants: false`) and `plugins_grant` is refused.

## Shortcuts (known limits)

- `Toml.java` reads the TOML subset a manifest uses (no multi-line strings, no dates); `Plugins.parse` checks id, api, apps and sizes, not every rule of
  `plugin-api`'s manifest parser. Bundled packages were validated by `xt plugin pack`; the page host re-checks the descriptor and enforces permissions at run time.
  Upgrade path: a wasm build of `plugin-api::manifest` called from the page.
- The signature (O4) is not checked, as on desktop.
- A change of the enabled set needs an app restart (the shell reads `shell-config.json` once).

## Found while running it

The Docs phone layout never draws the ribbon, and the ribbon's per-frame hook (`preset::sync` -> `docs_plugins::frame`) is what registers the plugin
commands in the editor's registry, so in the app `wordcraft:run plugin.<id>.<command>` answered "unknown command". Fixed in our layer:
`docs-office::plugin_web::sync` (called every frame by the web host) now calls `docs_plugins::frame` itself (cheap when nothing changed).

## Results (emulator `HRM_Pixel_8`, API 36 image, 540x1200 rig, 2026-11-03)

JSON and screenshots: `docs/phase7/evidence/android/`; recordings and their SHA-256: `docs/phase7/evidence/SHA256SUMS`.

| Item | Result | Evidence |
|---|---|---|
| uz-translit toCyrillic on `phone-ux.docx`, run from the editor, saved, pulled | pass (text saved with `Салом дунё`) | `translit-toCyrillic.json`, `translit-0..4*.png`, `translit.mp4` |
| uz-translit toLatin on the result | pass (Latin again; the plugin's Latin spelling of the Cyrillic text is its own, not the original) | `translit-toLatin.json` |
| Install from file through the picker (record, SHA-256, enable, restart) | pass | `install-from-file.json`, `install-*.png`, `install.mp4` |
| Disable uz-translit, restart | pass (not in `shell-config.json`, host lists only the other plugin) | `disable.json`, `disable-*.png`, `disable.mp4` |
| M1 import refused / M2 exported memory refused | pass (installed from file, load refused in the Worker) | `M1-load-refused.json`, `M2-load-refused.json` |
| M3 no network in manifest / M4 listed but Android grants none | pass (`network.refused` / `network.denied`, no dialog) | `M3-*.json`, `M4-*.json` |
| M6 infinite loop | pass (stopped at 5 s, a 100 ms page timer kept ticking) | `M6-time-budget.json`, `M6-stopped.png` |
| M7 memory growth | pass (capped at 2 MiB) | `M7-memory-cap.json` |
| M9 markup in a panel | pass (text only, cut at 1,024) | `M9-panel-is-text.json`, `M9-panel.png` |
| M11 Worker CSP | pass (header as suite-serve; `fetch` from inside the Worker blocked, same-origin and other) | `M11-worker-csp.json` |
| No network at all | pass (no INTERNET permission; page `fetch` to another host fails) | `no-network.json` |
| Tap-through fix in the app (`tapfix-app.mjs`) | before: picture gone from the autosave (5,750 B, no media); after: picture kept | `tapfix-before-*`, `tapfix-after-*` |

Run 2026-11-06 (re-check, `android-plugins.mjs recheck`, APK built from `p7-recheck` with the docs web rebuilt on phase-7 code, same emulator and rig; recording `recheck.mp4`, SHA-256 in `docs/phase7/evidence/SHA256SUMS`;
the fixture package `test.recheck` has the 15 commands of the rows, `test.nonet` is the second plugin of F5): all pass, evidence JSON in `docs/phase7/evidence/android/`.

| Item | Result | Evidence |
|---|---|---|
| M5 saveAs refused (again), M8 malformed answer, S1 recursion | pass (one audit `failed` each, document unchanged after save) | `M5-saveAs-refused.json`, `M8-malformed-answer.json`, `S1-recursion.json` |
| M10 edit from a `selectionChanged` event (a tap in the document) | pass ("an event may only run ui actions", for both plugins; document unchanged) | `M10-event-edit-refused.json`, `recheck-M10.png` |
| S2 8-fetch cap, 10 s invocation deadline | pass (no grant: 8 `network.denied` counted, the 9th ends it; a chain of rounds `stopped` at 10.7 s after 14 resume rounds) | `S2-fetch-cap.json`, `S2-deadline.json` |
| F2 links, field codes, forged revisions; F7a dictionary, accept, restrict | pass (each refused by the allowlist or by the denied-keys rule; document unchanged) | `F2-link.json`, `F2-chr.json`, `F2-incl.json`, `F2-tracked.json`, `F7a-vocab.json`, `F7a-accept.json`, `F7a-wall.json` |
| F5 busy wording | pass ("another plugin is still running"; the first plugin stopped; the next command ran) | `F5-busy.json`, `recheck-F5-busy.png` |
| F7b a document with its own protection (fixtures `protected.docx`, `protected-tracked.docx`) | pass (comments: "only comments are allowed in this document" from the engine; tracked changes: "the document is protected" from the bridge; the plugin was never called) | `F7b-comments.json`, `F7b-trackedChanges.json` |

Still not run: M12 (server only). The earlier `rows` item used the WP5 test plugin (`ci/plugins/webhost-test`) packed by `scripts/mobile/fixtures/make-test-packages.py`, and calls `PluginHost.last.invoke` through
the WebView's devtools; the translit item and the re-check rows run the command through the editor (`wordcraft:run`), not the palette or a toolbar button (the manifest has none).
Not a real phone: all of this is on the emulator. Found on the way (and fixed): the Plugins page showed "the app did not answer" when the file dialog stayed open for more than 20 s (`page.js`, `malicious.md` Finding 4; the fix is in this APK).

APK size (same four dists, same signing): 53,368,768 B before (`06b1999` sources) and 53,479,711 B after, +110,943 B (+0.21 %): the uz-translit package
(plugin.wasm 250,138 B raw, about 83 KB compressed) plus the three new classes and `page.js`.
