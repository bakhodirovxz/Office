# Desktop host (WP9)

`services/suite-desktop` builds `suite.exe` (no console window). It starts `suite-serve` in-process on `127.0.0.1:<free port>` and shows the shell in a WebView2 window (wry 0.57, tao 0.37, rfd 0.17: MIT/Apache; all Windows-only in `Cargo.toml`).

- Command line: `suite.exe [--dist D] [--shell D] [--brand F] [--bin-dir D] [--browser] [--] [FILE...]`. A file opens in its editor exe (`brand.toml` `bin`, extension table of PLAN 7.2) found in `--bin-dir` (default: next to `suite.exe`).
- IPC (`window.ipc.postMessage(JSON)`, only from `http://127.0.0.1:<port>`, 8 KiB max, exact fields, anything else is refused): `open{path}`, `pick{app?}`, `recent{}`, `history{}`, `settings_get{}`, `settings_set{key,value}` (`lang` only). Replies arrive as the `suite-host` DOM event. Paths from the page: absolute, drive-letter only (no UNC), no `..`, known extension, existing file. Navigation is limited to our origin, new windows are denied, devtools are off.
- Single instance (F14): the first `suite.exe` owns the named pipe `\.\pipe\<product.id>-host-<USERNAME>` (`first_pipe_instance`, local clients only). A second launch sends `open`/`focus` lines and exits 0; if nobody answers it opens the files itself.
- No WebView2 (or `--browser`, or `SUITE_FORCE_NO_WEBVIEW2=1`): the default browser opens the local URL and a dialog says so; the server lives while the dialog is open. Dialog text is English only (not yet in the i18n catalogs).
- Test hooks (env): `SUITE_DATA_DIR`, `SUITE_INSTANCE`, `SUITE_BROWSER`, `SUITE_TEST_EXIT_AFTER_MS`.
- windows-gnu imports `WebView2Loader.dll`: ship it next to `suite.exe`. The msvc build links it statically. Data: `%LOCALAPPDATA%\<product>\Host` (history.json, settings.json, host.json, webview2/).
- Smoke: `scripts/smoke-desktop.ps1` (release, this PC, WebView2 155): window in about 1.9 s, server about 1.4 s, host 28 MB, WebView2 6 processes about 285 MB, second launch handed to the running instance.
- Not done: `GATE-FLOW desktop` (needs the editors' control channels), IPC `new from template`, `search folders`, a real History (WP10; `history` is the host's open log).
