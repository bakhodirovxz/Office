# The Android app (PLAN phase 6 WP8, 2026-11-03, branch `p6-android`)

A thin WebView host: one Activity, the shell and the four editor dists in the APK, local mode, **no INTERNET permission**
(`aapt2 dump permissions` lists none). Server mode is a later config: it will add the permission and a network security config.

## Build
`packaging/android/build.sh [dist-root] [out.apk]` (Git Bash). No Gradle: aapt2 + javac + d8 + zipalign + apksigner, the method of
`ci/android-harness/build.sh`. `dist-root` holds `docs-web sheets-web slides-web pdf-web` (default `E:/office-target/p6-android/dist`); the
shell is assembled from `web/shell` exactly as `scripts/shell-dist.sh` does (without `.br`). The first APK used the four dists of
`E:/office-target/p6-mobileux/dist` (built 2026-11-02; Docs is the current phone-layout build, the others are newer than `p5/dist` of
2026-10-25). Output: `E:/office-target/p6-android/office.apk`.
`gen-brand.py` writes, from `brand/brand.toml`: the label (`res/values/strings.xml`), `shell-config.json` (what suite-serve's `brand_json`
builds, with `local: true`) and the manifest MIME filters. Icons come from `brand/logo/android/res`. The product name is in no Java or XML source.

## How assets are served
`MainActivity.Client.shouldInterceptRequest` answers every request of the virtual origin `https://appassets.androidplatform.net/`
(another host or a non-GET: 404; `shouldOverrideUrlLoading` refuses to leave the origin). Routes as suite-serve's: `/` and `/shell/*` from
`assets/web/shell`, `/shell-config.json`, `/docs|sheets|slides|pdf/*` from `assets/web/<dist>`. MIME by extension (`application/wasm`,
`text/javascript`, ...). Headers as suite-serve sends: the shell gets convert-api's baseline CSP; an editor `index.html` gets the editor CSP
built as `editor_csp` does (`'self' 'wasm-unsafe-eval'` plus the SHA-256 of each inline script and style, computed at first request;
`frame-ancestors 'self'`); all get `nosniff`, `Referrer-Policy: no-referrer`, `Cross-Origin-Resource-Policy: same-origin`. No COOP/COEP is
sent for the editors by suite-serve either, and the editors ran without them. The wasm is not precompressed (the APK deflates it).
Note: aapt2 `-A` on Windows writes backslashes into asset names, so `build.sh` adds `assets/` with `jar uf`.

## Open and save (shell bridge, `web/shell/shell.js`, 25 added lines)
- **In:** VIEW and SEND (MIME filters for docx xlsx pptx pdf doc xls ppt odt ods odp) and the shell's Open buttons (`SuiteHost.pick`, the
  system Files picker, `ACTION_OPEN_DOCUMENT` with a write grant) read the `content://` bytes on a thread and park them; the page fetches
  them once from `/__open` (name in `X-Name`), so no base64 goes in. `window.hostOpen()` runs on arrival and once at the end of `main()` (cold start).
- **Out:** the editor's `<prefix>:save` makes the shell call `SuiteHost.save(name, base64)`. The host writes to the opened URI with
  `openOutputStream(uri, "wt")` when the extension is unchanged, else (or when the write is refused: a read-only source) starts
  `ACTION_CREATE_DOCUMENT` (Save as). It answers `hostSaved({ok, name, error})`; only `ok` clears the dirty flag, an error is shown by the shell
  (`fail`) and the document stays dirty. After a Save as, the new URI is the write-back target.
- A new document (no URI) goes through Save as. `<input type=file>` inside an editor (insert image) works through `onShowFileChooser`.
- `onJsConfirm`/`onJsAlert` are native dialogs (the shell's "leave unsaved changes?" needs them).
- Edge to edge: the root view pads for system bars, cutout and the IME, so the soft keyboard shrinks the viewport (the Docs phone layout relies on
  it); `adjustResize` is set too. `requestFocus()` in `onResume` and `onPageFinished`. `configChanges` keep the activity (and the document) across
  rotation; process death is WP12. The hardware Back goes to the shell's home first (`hostBack`), then closes.

## Recovery of unsaved work (WP12 recovery, 2026-11-03, branch `p6-recover`; `docs/phase6/lifecycle.md`)
- **Kept:** the shell hands each `wordcraft:autosave` (on hide, with a WebGL loss, every 30 s while dirty) to `SuiteHost.autosave(app, name, base64)`.
  The host writes `files/recover/<app>` (app-private: the name, a newline, the bytes) to `<app>.tmp`, `fsync`s it and renames it over the old one,
  so a kill mid-write leaves the previous copy whole. An unknown editor id or an empty name is refused (logged, the document stays dirty).
- **Reopened:** `GET /__recover/<app>` serves the copy (name in `X-Name`), 204 when there is none. On every page start the shell reopens it,
  **unsaved** (`wordcraft:open { dirty: true }`, wordcraft/0146), without a question (a browser can answer a `confirm()` by itself; lifecycle.md).
- **Dropped:** `SuiteHost.discard(app)` after a Save in the copy's own format (`hostSaved` ok) or when the user discards the changes (the
  "leave unsaved changes?" confirm when another document replaces it). Nothing else deletes it.
- **A dead renderer:** `Client.onRenderProcessGone` returns true (the app no longer dies with it), drops the WebView, builds a new one
  (`setUp()`, the same settings and bridge) and loads the shell, which reopens the copy. After a process death the relaunched app does the same.
- After process death the content URI the document came from is gone (not persisted; its grant ends), so a recovered document saves through
  Save as. After a renderer crash the app process lives on and Save writes back to that URI as before.

## Signing
Debug/test key only: `E:/office-target/p6-android/signing/debug.keystore` (alias `debug`, store and key password `android`), generated by
`build.sh` on first use with `keytool`, **never committed** (`*.keystore`, `*.jks` are gitignored). The release key is the Phase 4 item
(`docs/phase4/PLAN.md:23`, our own key, free) and waits for O5. The manifest is built `--debug-mode` (debuggable) for now.

## Provisional application id (O5)
`brand/brand.toml [android] application_id = "app.hoshiya.suite"`, `version_code = 1`. **Provisional: it never leaves this PC.** The owner chooses
the final reverse-DNS id before the first release build (PLAN 13.4 O5); it is permanent once published. `crates/office-brand` now accepts the
`[android]` table. Distribution (O10): sideload or MDM for IT staff only.

## Verified (AVD `HRM_Pixel_8`, API 36, Gboard 15.1; `scripts/mobile/android-app.mjs`, `android-type.mjs`)
Gboard needs Russian added and "Show language switch key" on (Gboard settings, a precondition on this AVD; the globe key is then at the key left of the space bar).

| Scenario | Result |
|---|---|
| open from the Files app (picker), type `salom` (EN) and `привет` (RU) with Gboard, More > Save, pull the file | `word/document.xml` text contains `Salom привет` (Gboard capitalised after a full stop) |
| open by VIEW (content URI, write grant), same, then force-stop and VIEW again | same bytes check; the reopened page shows `Salom привет` (`evidence/android/view-reopened.png`) |
| open by SEND | same (the uri is also in the intent data: `am start` cannot grant an `EXTRA_STREAM`; a real share sheet grants it in ClipData: not exercised) |
| read-only source (VIEW with only a read grant) | the write is refused, the Files app's Save as opens, Save writes `phone-ux (n).docx`, bytes checked (`readonly-save-as-dialog.png`) |
| rotation | landscape shows the desktop layout with the document (`view-landscape.png`); portrait again shows the same document, no reload |

Recordings: `E:/office-target/p6/evidence/<runId>/<scenario>.mp4`, SHA-256 in `docs/phase6/evidence/SHA256SUMS` (lines marked WP8).

## Not verified
- A real chooser share from another app, `ACTION_OPEN_DOCUMENT_TREE`, cloud-provider files (slow `openInputStream`, no progress shown).
- A write that fails mid-way: `"wt"` truncates first, so a short file can remain; the page is told, the document stays dirty, the user can Save as.
- Back-stack restore beyond the recovery above (WP12), API below 36 (minSdk 26 is only compiled for), release signing, a real phone, other WebView versions.
- The shell is the web build's: the History button and server calls do nothing useful in local mode.
- Whole files are held in memory (256 MB cap); base64 across the JS bridge makes a save about 2.3x the file size transiently.
