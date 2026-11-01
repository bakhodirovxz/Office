# WP4: WebView embed proof in the HRM-app shape (2026-11-02, branch `p6-webview`)

Question: does the editor work when it is embedded the way the HRM mobile app (MOB, Expo `<hrm-app>`) embeds ONLYOFFICE today
(F10, `docs/phase0/hrm-current-state.md:315-319`): inline HTML in an Android WebView, `baseUrl` = the Document Server, running
`new DocsAPI.DocEditor("editor", config)`? No real HRM system was touched; the backend is the Phase 5 fake HRM.

## What was built (test-only, never in the product)

`ci/android-harness/` (our code, MIT OR Apache-2.0), no Gradle and no downloads: `build.sh` = aapt2 + javac (Android Studio's JDK 21,
`--release 11`) + d8 + zipalign + apksigner with a per-run debug key, against `platforms/android-36/android.jar` (build-tools 36.0.0), minSdk 26,
targetSdk 33. `MainActivity.java` fetches the editor config natively over HTTP (like MOB), builds the inline HTML (api.js from the
Document Server, `cfg.events`, `new DocsAPI.DocEditor`) and calls `loadDataWithBaseURL(ds, html, "text/html", "utf-8", null)`; events go to
logcat tag `HARNESS`. `run.mjs` drives the whole run on AVD `HRM_Dev_33` (API 33): throwaway CA and leaf for `10.0.2.2` (the CA is baked into
the APK's network-security trust anchors, never committed), the fake HRM (`FAKE_HRM_MOBILE_EDIT=1`, a new env switch in
`services/fake-hrm/src/main.rs`, so `device=mobile` stays editable as after HRB PR-3), and `hoshiya-serve` from the Phase 5 build with TLS on
`https://10.0.2.2:<port>`. Typing is real Gboard: taps on the soft keys through `adb shell input tap`, the globe key for the Russian layout (no
`keyevent`); the layout is checked on the screencap (`png.mjs`). `cdp.mjs` talks to the WebView's devtools socket (Playwright's
`connectOverCDP` does not work on a WebView).
Run: `P6_DIR=E:/office-target/p6-webview node scripts/mobile/rig.mjs start HRM_Dev_33`, then `node ci/android-harness/run.mjs`.
`HRM_Dev_33` was added to the rig's AVD table (console port 5560).

Environment of the run: Android System WebView 109.0.5414.123 (this AVD image's; old), Gboard 12.4 with Russian added in Gboard's language settings
(a precondition on the AVD), `-gpu swiftshader_indirect`, 420 dpi, 1080x2400.

## Result per step (evidence in `docs/phase6/evidence/webview/`)

| Step | Result |
|---|---|
| open | The editor is ready ("ready" event) in the WebView with the inline HTML; the document renders (Latin and Cyrillic runs, canvas 1082x1997). |
| type | Latin `salom ` on the English layout, then `привет` on the Russian layout, by Gboard key taps into the last empty paragraph (`typed.png`); the first change gives callback status 1. |
| save | `__editor.requestSave()`: `onSaved` v2; the fake HRM received exactly one status 6 (its download from the internal file route succeeded, nothing rejected). |
| close | `__editor.close()`: `onClosed` status 2; callback status 2 sent. |
| saved docx | `v2.bin` of the session: the text of `word/document.xml` ends with `...YOSHLARsalom привет`, so both the Latin and the Cyrillic text are in it (`webview-embed.json`, step `docx`). |

Callback log (`ds-log.txt`): `callback status 1`, `6`, `2` to `/files/1/save-callback: ok`, one each. Recording: `webview-embed.mp4`
(540x1200, sha256 `7ac58438f0a98b902c46543ea26d8baffdaf4cf38f5a6f455b87e1893b3c782e`).

## Origin and `frame-ancestors` (feeds `docs/phase5/hrb-handoff.md`)

What Android WebView reports for `loadDataWithBaseURL(baseUrl, ...)`: the document's URL and origin are the **baseUrl's**. In the page:
`location.href` = `https://10.0.2.2:<port>/`, `location.origin` = `window.origin` = `https://10.0.2.2:<port>`; `isSecureContext` is true (https);
`location.ancestorOrigins` in the wrapper frame = `[<DS origin>]`, in the editor frame = `[<DS origin>, <DS origin>]`. (The devtools target list shows
the page as `about:blank`; ignore that, it is the data URL.) So with baseUrl = the Document Server, the "host page" has the **Document Server's own
origin**, not the HRM's origin and not `null`.

Consequences:
- The wrapper's `frame-ancestors` list always contains the server itself, so CSP never blocks the framing in this shape.
- The gate that matters is the session create: the wrapper sends its parent's `event.origin` and the server compares it with the provider's
  `frame_origins` (`sessions.rs:270-280`). The wrapper URL carries `#o=<DS origin>`. **The HRM operator must put the Document Server's own public
  origin into the provider's `frame_origins` in addition to the SPA origin**, for example
  `frame_origins = ["https://hrm.example.org", "https://docs.example.org"]`, where the second is `public_url` (scheme, host, port exactly as in MOB's
  `baseUrl`, no path). If MOB used another `baseUrl` (say the HRM API host), that origin is the value to list: it is whatever origin MOB passes.
- Verified negatively: with `frame_origins = ["https://hrm.example.test"]` only, the editor never opens (events: only `boot`), the server logs
  `auth.fail create: origin is not a frame origin` (403), and the harness gets no `ready`.
- Side effect: api.js, wrapper and editor are same-origin with the host, so the cross-site storage and cookie partitioning of the browser case
  (Phase 5 S13/S14) does not apply in the app.

## WebGL in the nested iframes

The probe ran in all three frames (host document, wrapper, editor): `webgl` and `webgl2` contexts are created in each
(WebGL 2.0 OpenGL ES 3.0 Chromium; renderer "Android Emulator OpenGL ES Translator (Google SwiftShader)"). The editor's own canvas
is a `webgl2` canvas (`getContext('2d')` on it returns null): the editor renders through WebGL2 inside the doubly nested iframe in WebView 109.

## Findings and limits

1. The WebView needs view focus (`wv.requestFocus()`), or the soft keyboard never shows although the editor's hidden input is focused. MOB (React
   Native WebView) must be checked for this; not verified on the real app.
2. After the keyboard opens the page resizes, but the canvas does not scroll the caret into view (the new last paragraph stays below the visible
   part until more text is typed): the resize handling of PLAN section 5.3, owned by WP6. Typing still works.
3. The layout in the WebView is the desktop one (ribbon, tabs); the phone layout is WP6 (not merged here).
4. The emulator is slow with the editor running next to other emulators: "System UI isn't responding" dialogs appeared, so the run sets
   `hide_error_dialogs 1`; open takes 15-40 s here.
5. Not verified: the real MOB app (React Native WebView props such as `originWhitelist`, `domStorageEnabled`), OEM keyboards (Samsung, SwiftKey),
   a newer WebView than 109, a real TLS chain (the harness trusts a throwaway CA), a `baseUrl` with a path or trailing slash (same origin by the
   URL rules, not run). `hoshiya-serve` is the Phase 5 build in `E:/office-target/p5/services/debug/`, not rebuilt on phase-6.
