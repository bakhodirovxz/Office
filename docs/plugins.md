# Plugins (API v1) - DRAFT

Status: draft of sections 1-8 (Phase 7 WP1). Sections 9-13 (installing, the generated command catalog, the Rust SDK and
samples, versioning, security reporting) come with WP8 and WP11. The rules here are the ones the `plugin-api` crate enforces;
when this text and the crate disagree, the crate is right and this text is a bug. Design background: `docs/phase7/PLAN.md`.

## 1. Concepts

- **Package.** A zip read in memory with exactly these flat entries: `plugin.toml` (required), `plugin.wasm` (required),
  `strings.tsv` (optional: en, ru, uz-Latn, uz-Cyrl), `SIGNATURE` (optional). Any other name, a directory, a path or a duplicate
  refuses the package. Limits: 8 MiB in all, `plugin.wasm` 4 MiB, `plugin.toml` 64 KiB, 64 entries.
- **Manifest.** `plugin.toml` declares who the plugin is, what it may touch (permissions) and what it adds to the UI
  (commands, toolbar buttons, menu items, panels, events). Everything it declares is checked before the binary is read.
- **Binary.** One core WebAssembly module, the same file on every platform. It has no way out of its sandbox except the
  messages below.
- **Rounds.** The host calls the plugin with a message; the plugin answers with *actions* (data); the host checks and runs them;
  if the answer asked for more input (`next`) the host calls again with the results. At most 16 rounds per invocation.
- **Effects are data.** A plugin never edits the document itself. It returns actions such as `run`, `textmap.apply`,
  `fetch`, `ui.panel`. All mutating actions of one invocation are one undo step; an error in any round puts the document back
  as it was before the first action.
- **Permissions.** Declared in the manifest, shown in the install dialog, enforced per action by the host (section 8).

## 2. Manifest reference (`plugin.toml`)

Unknown keys are refused. Top-level keys (`events` among them) must come **before** the first `[table]` or `[[table]]` header,
because TOML puts every key after a header into that table.

| Key | Rule |
|---|---|
| `id` | `[a-z0-9.-]`, 3-64 characters, starts and ends with a letter or digit, no `..`. Unique per install. Never the product name. |
| `version` | `MAJOR.MINOR.PATCH` with an optional `-prerelease`; numbers without leading zeros; no `+build`. |
| `api` | The API major the plugin was built for. The host lists the majors it runs (v1 hosts: 1); anything else is refused. |
| `apps` | One to four of `docs`, `sheets`, `slides`, `pdf`; no repeats. |
| `publisher` | Free text, 1-128 characters, shown in the install dialog. |
| `name.<lang>` | `en` required; `ru`, `uz-Latn`, `uz-Cyrl` optional; a missing language falls back to `en`. Any other language key is refused. |
| `events` | Any of `documentOpened`, `documentSaved`, `selectionChanged`; no repeats. |
| `[permissions]` | `document` = `none` (default) / `read` / `write`; `network` = up to 8 origins; `memory_mb` = 1-64 (default 16). |
| `[[command]]` | Up to 16. `id` (letter first; letters, digits, `_`, `-`; no dot; up to 48), `label.<lang>`, `reads` (`textmap`, `selection`, `none`), `params` (up to 16 keys, 1 KiB encoded). The registry id is `plugin.<plugin id>.<command id>`. |
| `[[toolbar]]` | Up to 8. `command` (a declared one), `icon` (`[a-z0-9_-]`, up to 32; unknown names show the label only), `group.<lang>`. |
| `[[menu]]` | Up to 8. `command`, `place` = `tools` or `context.text`, `group.<lang>`. |
| `[[panel]]` | Up to 2. `id` (as a command id), `title.<lang>`. |
| `[settings]` | Defaults the admin or user may override: up to 32 keys, 4 KiB encoded. |

Text the user sees (names, labels, titles, groups, publisher) is at most 64 characters (publisher 128), not empty, and holds no
control character and no invisible or bidirectional-override character (U+200B-200F, U+2028-202E, U+2066-2069, U+FEFF), so a
label cannot read differently from its bytes.

A `network` entry is `https://host[:port]` and nothing else: lowercase ASCII host with at least two labels, no IP address,
no `localhost`, no user name, path, query or trailing slash. A command that `reads` the document needs `document` of `read`
or `write`.

```toml
id = "uz-translit"
version = "1.0.0"
api = 1
apps = ["docs"]
publisher = "Example Publisher"
events = ["selectionChanged"]
name.en = "Transliteration (plugin)"

[permissions]
document = "write"
memory_mb = 32

[[command]]
id = "toCyrillic"
label.en = "Latin to Cyrillic"
reads = ["textmap"]
params = { scope = "auto" }

[[toolbar]]
command = "toCyrillic"
icon = "translit"
```

## 3. ABI and message format

| Item | Rule |
|---|---|
| Imports | Exactly `env.memory` (required; the host creates it with `maximum = memory_mb`) and optionally the debug `log(ptr, len)`. Anything else refuses the load. |
| Exports | `api_version() -> i32` (= 1), `alloc(len: i32) -> i32`, `free(ptr: i32, len: i32)`, `call(ptr: i32, len: i32) -> i64` (the answer as `ptr << 32 | len`). No exported memory. |
| Message | `u32 header_len, header (UTF-8 JSON), u32 blob_count, then per blob: u32 len, bytes`. Little endian. |
| Limits per round | Input and output 32 MiB each; header 1 MiB; 16 blobs; 16 rounds per invocation; 5 s per round; memory `memory_mb`. |

The parser checks every length against the bytes left before it allocates, refuses trailing bytes and invalid UTF-8, and
treats a truncated message as an error. A host call is read leniently by the plugin (unknown fields are ignored, which is how
the API grows inside a major); a plugin answer is read strictly by the host (an unknown action or field refuses the whole
invocation).

The host call header: `kind` (`command`, `ui`, `event`, `resume`), `api`, `api_minor`, `app`, `lang`, then as the kind needs
`command`, `widget`, `event`, `params`, `settings`, `results` (one per action of the previous round), `reads` (what was read,
to the index of the blob that holds it). The answer header: `{"actions": [...], "next": {"reads": [...]}}`.

## 4. Actions

| Action | Needs | Effect |
|---|---|---|
| `run {id, params}` | `document = write` (`read` for the document reads) and an id on the allowlist (section 8) | Runs a catalog command; the result comes back in the next round. |
| `textmap.apply {blob}` | `write` | Applies edits to the text map read in this invocation (section 5). |
| `read {what, scope?, index?}` | `read` | `what` = `textmap`, `selection`, `paragraph`; the result comes back in the next round. |
| `fetch {url, method, headers, body}` | a granted network origin | Done by the host; the response comes back in the next round. |
| `ui.panel {panel, widgets}` | the panel is declared | Replaces the panel's widgets. |
| `ui.status {text}` | none | One status line (up to 256 characters). |

At most 256 actions per answer. An `event` invocation may run only `ui.*` actions.

`fetch` rules: the URL must be plain `https` and its origin must be one the user or admin granted, byte for byte. A URL with a
user name, a backslash, whitespace, a control character or more than 2048 characters is refused (clients disagree about where
the host ends in such URLs). Methods: GET, POST, PUT, PATCH, DELETE. Up to 16 headers; `Host`, `Cookie`, `Content-Length`,
`Connection`, `Transfer-Encoding`, `Origin`, `Referer`, `Upgrade`, `TE`, `Expect`, `Proxy-*` and `Sec-*` are refused. The host
sends no credentials of its own and applies the response size and time limits.

## 5. Text map

The bulk read and write for text plugins. Read blob (host to plugin):
`u32 lang_count, per lang (u32 len, text); u32 paragraph_count, per paragraph: u32 index, u32 segment_count, per segment:
u8 flags, u16 lang, u32 len, UTF-8 text`.

- A **segment** is a piece of a run that never crosses a run, an object or a keep boundary. Tracked deletions are left out.
- `lang` indexes the language table; entry 0 is always the empty tag ("not set"). Paragraph indexes strictly increase and may
  skip (a selection scope).
- `flags`: bit 0 *editable*; when it is clear, one or more of bit 1 *outside the selection*, bit 2 *object*, bit 3 *locked
  control*, bit 4 *field result* says why. An editable segment carries no reason bit; other bits are refused.

Apply blob (plugin to host), positional against the read:
`u32 paragraph_count, per paragraph: u32 segment_count, per segment: u8 changed (0 or 1), then if 1: u32 len, UTF-8 text`.

The host refuses the apply when the paragraph or segment counts differ from the read, when a segment that is not editable
changed, or when new text holds a line break (`\n`, `\r`, U+2028, U+2029) or a control character other than a tab. Otherwise
it runs the editor's own apply (tracked changes included) and maps the selection. The read and the apply belong to one
invocation; on the web the editor refuses an apply when the document changed since the read and reads once more.

## 6. Contributions and widgets

A toolbar button goes into a "Plugins" group of the simple ribbon (a "Plugins" custom tab in the full ribbon). A menu item goes
into the Plugins group of the Tools menu or the text context menu. Every command is also a registry command, so it shows in the
command palette. A panel is a side panel on the desktop, a panel beside the editor on the web, a bottom sheet on a phone.

Widgets (the fixed list; at most 64 per panel; text up to 1024 characters, no control or bidi characters, line breaks only in
`text`): `heading {text}`, `text {text}`, `list {items: [{id?, text}]}` (up to 256), `button {id, label}`,
`textField {id, value, placeholder?}`, `select {id, options: [{value, label}], value?}` (up to 256). Widget ids are letters,
digits, `_`, `-`, up to 48.

## 7. Events

`documentOpened`, `documentSaved` and `selectionChanged` (debounced 200 ms) are sent only to plugins that list them. An event
call can only answer with `ui.panel` and `ui.status`; it cannot read or change the document.

## 8. Permissions, limits and what is refused

- **Document**: `none` / `read` / `write`, declared. `run` of a document read needs `read`; every other allowed command, and
  `textmap.apply`, needs `write`.
- **Network**: per origin. In server mode only the admin grants; in local mode the user is asked once per origin; on the
  Android local app never. The manifest cannot ask for an IP address or `localhost`; the host also refuses to connect to a
  private address behind a public name unless the admin allowed it.
- **Native fetch** (`crates/plugin-host/src/net.rs`, used by the desktop and CLI hosts through `docs-office::plugin_native`): only a granted
  origin, plain https; the name is resolved in the host and one refused address among the answers refuses the name (loopback, 10/8,
  172.16/12, 192.168/16, 169.254/16, CGNAT 100.64/10, unspecified, multicast, reserved v4; v6 loopback, ULA, link-local, multicast
  and every v6 form that carries a v4 address, judged by that address); the connection goes to the checked address, TLS uses the
  operating system's roots; redirects are followed inside the same origin only (3 hops, each resolved and checked again); 10 s
  and 1 MiB per call; the answer is `{status, body}`. The admin switch for private targets named in the PLAN is not built: private
  targets are always refused.
- **Command allowlist** (deny by default; the data is `plugin_api::allow`). Allowed families: `caret.`, `select.`, `text.`,
  `format.`, `font.`, `para.`, `styles.`, `list.`, `table.`, `fields.`, `developer.`, `insert.`, `review.`, minus the named
  exceptions, plus the reads `document.text`, `document.paragraph`, `document.selection`, `document.inspect`. Never allowed:
  `file.*`, `edit.*` (the system clipboard and the user's undo stack), `tools.*` (macros), `mailings.*`, `link.*`, `view.*`,
  `readAloud.*`, `ui.*`, `office.*`, `plugin.*` (no plugin runs another), `document.setText`. Also refused inside the allowed families because they take a file, a
  path or bytes: `insert.textFromFile`, `insert.picture`, `insert.link`, `insert.object`, `font.add`, `font.loadSystem`,
  `font.remove`, `review.compare`, `review.combine`, `review.acceptAll`, `review.rejectAll`, `styles.organizer`, `styles.copyFrom`.
  Of the 633 Docs commands, 306 are allowed (2026-11-03). A `run` whose `params` hold a key
  named `path`, `dir`, `file`, `url`, `uri`, `href` or `src` (any depth, any case) is refused, except a value that is an
  array of non-negative integers (a document node path).
- **Refused at install**: a manifest that breaks section 2, an `api` the host does not run, a binary that imports anything but
  section 3 lists, a package that breaks section 1.
- **Refused at run time**: a message that breaks section 3, an answer with an unknown action, an action without its permission,
  a text map apply that breaks section 5, a round over its time or memory budget, more than 16 rounds. The document is then
  restored and the status line says the plugin was stopped.

## 9. The web bridge (editor iframe and page host; Phase 7 WP4)

On the web the page that hosts the editor iframe owns the plugin's Worker and builds every call (section 9a); the editor (the Docs
wasm, `docs-plugins::bridge` and `docs-office::plugin_web`, patch `wordcraft/0148` only forwards the messages) does what only it can:
read the document, run the **editor actions** (`run`, `textmap.apply`) after the allowlist and grant check of sections 4 and 8, keep the
undo step, the revision and the snapshot. The page keeps `ui.*` and `fetch` and never forwards them. Messages are the `?host=parent`
protocol (prefix `wordcraft`), same origin. `call` is an integer for an invocation the editor starts and a text (`h<n>`) for one the
page starts (a panel click). Buffers are transferred `ArrayBuffer`s, never base64.

| Message | Direction | Fields |
|---|---|---|
| `wordcraft:plugins` | page to editor | `{commands, toolbar, menu, plugins: [{manifest, settings?, enabled?}], timeoutMs?}`, sent on every editor `ready`. The editor builds its slot commands, ribbon group and menus from `plugins[].manifest` (the descriptor's manifest as JSON; section 2 names, parsed and validated by `plugin-api`), so `commands`, `toolbar`, `menu` are informational. The list is the **whole** set: plugins not listed are unregistered, an empty list unregisters all. `timeoutMs` (optional, default 20,000) is how long the editor waits for the page between two messages of an invocation (the Worker's load 10 s plus one round 5 s). |
| `wordcraft:pluginsSet` | editor to page | `{ids, errors}`: the plugins registered and one text per refused entry (the others register). |
| `wordcraft:pluginCall` | editor to page | `{call, plugin, command, params, reads: {textmap?, selection?}}`: a plugin command ran (ribbon, menu, palette, `wordcraft:run`). `reads` are the command's `reads` as transferred `ArrayBuffer`s (the text map is the section 5 blob, the selection UTF-8 text); `params` are the manifest defaults merged with the caller's. |
| `wordcraft:pluginActions` | page to editor | `{call, plugin, actions, blobs, reads, more, abort?}`: the editor actions of one answer (`blobs` are the answer's blobs, `textmap.apply` names one by index), `reads` the reads the next call needs (`next.reads`), `more` whether another round follows. `abort: true` (the page gave up: a trap, a refused answer, the time budget) restores the snapshot; nothing is sent back. `plugin` is the plugin id; the editor needs it for a `call` it did not start. |
| `wordcraft:pluginResults` | editor to page | `{call, results, reads, error?}`: one result per action, one `ArrayBuffer` per requested read, in order. `error` means the invocation failed, the document is as it was and the page ends it (an unknown or ended `call` gets `error` too). |
| `wordcraft:pluginEvent` | editor to page | `{event}` for `selectionChanged` (debounced 200 ms), `documentOpened`, `documentSaved`, sent when a plugin of the page lists the event; the page relays it. |

A plugin command returns at once with `{"pending": call}`. From then on the document is **locked for input** (Restrict Editing's
read-only protection, lifted while an answer's actions run, put back between rounds and never left on) until the answer with
`more: false`. The editor ends an invocation itself, with the same restore and a "Plugin stopped: ..." status line, when no message
arrived within `timeoutMs`, when the revision moved while the plugin ran (`stale revision`), and when another document is opened. An
answer for an ended invocation gets a `pluginResults` with `error`. All mutating actions of an invocation, over all its rounds, are
one undo step. A panel click's invocation begins at its first forwarded answer (snapshot and undo step from there). `read` actions are
not served (the result says to use `next.reads`).

## 9a. The web page host (Phase 7 WP5; the rest of section 9 comes with WP8)

`web/plugins/host.js`, `worker.js`, `host.css`, `i18n.json` (ours, no patch). The shell (`web/shell/shell.js`) and the docserver wrapper
(`services/docserver/static/editor.js`) fetch `host.js` **only when a plugin is enabled** (a non-empty `plugins` list in
`/shell-config.json`, or in the docserver session answer); with none, no request for any of these files is made.

- **Descriptor** (what the server lists per enabled plugin; WP8 produces it from `plugin.toml` with `plugin-api`, which has already validated it):
  `{id, version, sha256, wasm: <same-origin URL of plugin.wasm>, manifest: <plugin.toml as JSON, same key names>, grant?: [origins], settings?}`.
  The host refuses a wasm whose SHA-256 differs. `grant` present means server mode: only those origins are granted and nobody is asked;
  absent means local mode: the user is asked once per origin and the answer is kept in this browser (`localStorage` `plg.grants`).
- **One Worker per plugin**, made at the first call. `worker.js` compiles the module, refuses it unless its imports are exactly `env.memory`
  and optionally `plugin_v1.log` and it exports no memory (error text `imports not allowed: ...`, `the module exports a memory`), creates the
  memory itself with `maximum = memory_mb * 16` pages, and answers `call` messages. The page measures the time budget (5 s, an admin may
  lower it with `pluginBudgetMs`) from posting a round until the answer; on expiry it calls `terminate()`, tells the editor to abort, and
  starts a new Worker at the next call.
- **CSP**: `/shell/plugins/*.js` is served with `default-src 'none'; script-src 'wasm-unsafe-eval'` (suite-serve `WORKER_CSP`; the Android
  asset loader has the same constant). A Worker takes the policy of its own response, so the plugin code cannot fetch or import anything.
  The page's own policy (`connect-src 'self'` through `default-src`) also stops the `fetch` action to a foreign origin, so the servers
  (WP8, section 9b) widen it to exactly the granted origins: `connect-src 'self' <origins>` on the home page (suite-serve) and on the
  wrapper page (docserver), nothing more.
- **Page-side checks** (the same rules as `plugin-api`): the wire format (lengths before allocation, no trailing bytes, UTF-8), a strict
  answer (unknown action, key or widget refuses it), document access per action, events may run `ui.*` only, panels must be declared,
  `fetch` URL/method/header rules, widget text cut at 1,024 characters (64 widgets, 256 items), and every plugin string reaches the DOM
  through `textContent` (control and bidi characters stripped). The command allowlist is checked by the editor (Rust), not here.
- **Editor bridge** (`<prefix>` = the editor's prefix; PLAN 4.6, section 9 is the contract; WP4 added `manifest`/`settings` to `plugins` and `plugin` to `pluginActions`): page to editor
  `plugins {commands: [{slot, id, label, mutates, reads, params}], toolbar: [{slot, icon, group}], menu: [{slot, place, group}]}` on every
  editor `ready` (slots are 0..63, assigned by the host in manifest order); editor to page `pluginCall {call, plugin, command, params, reads:
  {textmap?: ArrayBuffer, selection?: ...}}`; page to editor `pluginActions {call, actions, blobs: [ArrayBuffer], reads: [{what}], more}`
  with only the editor's actions (`run`, `textmap.apply`, `read`), answered by `pluginResults {call, results: [one per action], reads:
  [one per requested read, an ArrayBuffer or JSON], error?}`; `more:false` closes the invocation, `abort:true` (failure or time
  budget) asks for the snapshot to be restored. UI clicks and events start a host-made call id (`h<n>`). `pluginEvent {event, data?}`
  from the editor is relayed to plugins that list the event.
- **Panel**: opens when a plugin sends `ui.panel`; `role=region` named "<plugin name>: <panel title>", Close button, Escape closes; a
  side panel (the editor frame narrows) or, up to 600 px wide, a bottom sheet with a grabber and 48 px targets. `ui.status` goes to the
  shell status line (and the toast on a phone).
- **Strings** are a catalog of their own (`i18n/plugins.tsv`, scope `plugins`, exported to `web/plugins/i18n.json` by `cargo xt i18n
  export plugins --format json`) so the shell's size budget does not carry them.
- **Tests**: `ci/web-smoke/plugins-host.spec.ts` (stub editor, fixture plugin in `ci/plugins/webhost-test`).


## 9b. Installing, enabling and admin control (Phase 7 WP8)

Code: `crates/suite-core/src/plugins.rs` (the store, shared by all three hosts), `services/suite-desktop` (IPC), `services/suite-serve`
(`/plugins/<sha256>`, `PluginSource`), `services/docserver/src/{config,plugins}.rs`, `web/plugins/page.js` (the Plugins page).
Owner decisions behind it (PLAN 13): **O3** (who may install), **O4** (SHA-256 pin, signatures optional and not checked yet), **O7** (config file, no admin UI).

- **Package.** A zip (or, installed, a folder) with the flat entries of section 1. Installed copy: `<settings dir>/plugins/<id>/<version>/`
  with `install.toml` next to the files: `sha256` of `plugin.wasm`, `manifest_sha256` of `plugin.toml`, and a reserved `signature` field that
  nothing reads yet (O4: `shortcut:` in `plugins.rs`; the check comes with the project signing key). IT's copies go in
  `%PROGRAMDATA%\<product>\plugins\<id>\<version>\` (a debug build honours `SUITE_MACHINE_DIR`). A new install of an id whose manifest differs from the installed one
  turns that id off and withdraws its grants (it needs fresh consent), and the other versions of the id are removed.
- **Integrity.** A package whose files no longer match its record is refused: not listed for the page, not served, said on stderr and (docserver) audited.
  A package without a record is refused in the user folder and accepted in a machine or operator folder. `%PROGRAMDATA%` is writable by standard users by default, so the MSI creates `%PROGRAMDATA%\<product>` with a protected
  DACL (SYSTEM and Administrators full, Users read and execute), and the desktop host ignores the machine folder and `policy.toml` (said once on stderr) unless that
  folder is owned by Administrators or SYSTEM and no one else holds a write right (`suite-winsec::folder_is_admin_only`). The `icacls` custom actions are in
  `scripts/build-msi.ps1`; they are not exercised until an MSI is built and installed.
- **Desktop.** `settings.toml`: `[plugins] enabled = ["id", ...]` and `[plugins.grants] "id" = ["https://origin", ...]`. IPC (exact fields, 8 KiB, own origin only, like `open`):
  `plugins_list{}`, `plugins_install{}` (the host shows the file dialog; a page cannot name a path, and the dialog's path follows the rules of `open`: absolute,
  a drive letter, no `..`, no stream or device name, an existing `.zip`), `plugins_enable{id, enabled}`, `plugins_grant{id, origin, granted}`. Every answer is the event
  `plugins` `{ok, error?, installed?, items: [{id, version, name, publisher, apps, document, network, grant, enabled, machine, sha256, problem}], policy}`.
  A grant must be an origin the manifest declares. Installing does not enable. With `user_install = false` the user folder is not read at all (nothing in it is listed or enabled; effective from the next host start on the desktop). The shell reads the enabled list on every request: no restart.
- **Machine policy (O3).** `%PROGRAMDATA%\<product>\policy.toml`: `[plugins] user_install = false` and/or `user_grants = false`. With `user_grants = false` no
  user grant counts (existing ones are ignored; withdrawing stays possible). A policy file that cannot be read or parsed switches both off. No file: both allowed.
- **Web local (`hoshiya-serve`).** `[plugins] dir = "..."`, `enabled = [ids]`, `[plugins.grants] "id" = [origins]` and optionally `[plugins.sha256] "id" = "<hex of plugin.wasm>"` (a package whose wasm differs is left out) in the
  config file; the operator is the only one who enables or grants. The list is worked out once at start (restart to change it); the wasm is still read again and compared on each serve. `/shell-config.json` gets `plugins` (descriptors); `/plugins/<sha256>` serves an enabled package's wasm (read again and compared with the digest).
- **Grants are not asked at run time in these two modes.** The descriptor always carries `grant` (possibly empty), so the page host's own prompt never appears; the
  user ticks the address on the Plugins page (desktop) or the operator lists it. The home page's CSP is `connect-src 'self' <granted origins>` (computed per request),
  so a granted `fetch` is not blocked by it and an ungranted one is, twice (host check, CSP).
- **Docserver (per organisation = per provider).** `[docserver.plugins] dir` and, per `[[docserver.providers]]`,
  `plugins = [{ id, version, sha256, grant = ["https://..."], settings = { ... } }]`. At start the folder is read; a malformed entry, a `grant` that is not an
  `https://host[:port]` origin the manifest declares, or `plugins` without `dir` refuses the start. An entry whose package is missing or whose SHA-256 differs is left out
  and audited (`plugin.refused`). The session answer has `plugins: [descriptor]` of that provider only, `wasm` = `/ds/plugins/<sha256>?s=<session id>`, `grant` = the admin's
  list. `GET /ds/plugins/<sha256>?s=<sid>` serves the file for a live session whose provider lists that digest (404 otherwise, the same for every other case), reading the file again
  on each request (409 `plugin_changed` and `plugin.refused` when it changed). `POST /ds/session/{sid}/plugin-event {plugin, kind, origin}` (session token) lets the wrapper report
  `network`, `network.refused`, `network.denied` from the page host (`S.onAudit`) for the audit line `plugin.network`. These lines are claims of the page, marked `"source":"client"`: a `network` event is accepted only for an origin in that plugin's `grant`
  (else 400, not logged), and a session is limited like failed authentications. `/ds/plugins/<sha>` takes only 64 lower-case hex digits; `plugin.loaded` and `plugin.refused` are
  written at most once a minute per session (and sha). Audit lines: `plugin.loaded`, `plugin.refused`, `plugin.network`.
  The config token is never read for plugins (`onlyoffice-compat.md`, M12). The wrapper page's CSP is `connect-src 'self' <the origins granted to any provider's plugins>`
  (`shortcut:` one list for all providers, because the page's policy is fixed before its session is known; the page host still lets a plugin reach only its own descriptor's `grant`).
- **Disable.** Remove the id from `enabled` (local) or the entry from the provider (server); the editors drop the commands and the panel at the next document open (server)
  or the next page load (local).
- **Not done:** signature check (O4), an admin page (O7), uninstall, update-in-place prompts, Android install.

## 10. The Rust SDK and packing (Phase 7 WP6; the full guide and the template come with WP11)

`crates/plugin-sdk` is the glue for the ABI of section 3: `alloc`/`free`/`call` exports, the message codec, `Reply` and the typed
actions of `plugin-api`. A plugin is a `cdylib` for `wasm32-unknown-unknown` with one handler, `fn(Call, Vec<Vec<u8>>) -> Result<Reply, String>`,
exported by `plugin_sdk::export!(handler)`; an `Err` becomes a `ui.status` line and changes nothing. The crate's `build.rs` passes
`--import-memory` to the linker (the host creates the memory). `plugins/uz-translit` is the first-party example (PLAN section 6).

`cargo xt plugin build <name>` builds `plugins/<name>` with the `plugin` profile (speed, no unwinding, stripped). `cargo xt plugin pack <name>`
checks `plugin.toml` and the entry list with `plugin-api` and writes `<target>/plugins/<id>-<version>.zip` (stored entries in a fixed order, fixed
timestamp and mode, so equal inputs give equal bytes) and prints its SHA-256. `plugins/<name>/strings.tsv` (header row, then
`key, en, ru, uz-Latn, uz-Cyrl, status` per line) is shipped in the package; the plugin reads its own copy at build time.

## 11. The template-fields plugin (Phase 7 WP7; PLAN section 7)

`plugins/template-fields` is the second first-party plugin and uses a different part of the API than transliteration: a panel, `ui` calls, catalog
commands with parameters, an event with a read, and settings. The field list is configuration only: `settings.fields = [{name, label, kind}]`
(`kind` is `text`, the default, or `bookmark`). The manifest carries a synthetic default list; an administrator replaces it per provider
(`[[docserver.providers]] plugins = [{ ..., settings = { fields = [...] } }]`; the entry's `settings` replace the manifest's, they are not merged)
and, in local mode, the user does with `[plugins.settings.<id>]` in `settings.toml` (desktop) or in the web host's config file (`suite-core::plugins::State`).
No real field name is in the code, tests or docs.

- **Panel `fields`:** heading, filter box, "Check fields" button, and a `list` whose items (`id` = `ins-<position>`) are the "Insert" buttons. Every call is
  stateless, so every answer redraws the whole panel.
- **Insert** (`ui` call): a `text` field runs `developer.plainText {title: label, tag: name}`, then `developer.properties {placeholder: label, lockDelete: true}`
  (a content control, saved as `w:sdt` with `w:tag` = the name). A `bookmark` field runs `text.insert {text: label}`, one `caret.left {extend: true}` per
  character and `insert.bookmark {name}` (so the bookmark covers exactly the label). All of these are on the allowlist (families `developer.`, `text.`,
  `caret.`, `insert.`).
- **Highlight:** `selectionChanged` answers with `next.reads = [selection]` and no action (an event may run `ui.*` only); the resume call carries the
  selected text and the answer is `ui.panel` with the field whose label equals the text marked. The selection read is **text only** and empty for a collapsed
  caret, so a field is recognised while the user has selected its placeholder (click the control, or Select All in a paragraph holding only the control);
  a caret inside a filled control is not recognised. An event refreshes a panel that is open and never opens one (`host.js`; `docs-plugins::exec`).
- **Check fields** (command or button): `run document.inspect {text: false}`, then the tags of the `controlStart` objects and the names of `bookmarkStart`
  objects are compared with the list: missing fields, and control tags that are not in the list. The inspect read covers body paragraphs (the first 2,000
  blocks), not objects inside tables, headers or footers.
- **Limits:** at most 256 fields; names up to 64 characters (bookmark names follow `insert.bookmark`: a letter first, no spaces, up to 40); an invalid entry is
  skipped and counted in the panel. Typing in the filter box redraws the list; the next selection event resets the filter (no state between calls).
