# Plugins (API v1)

How to write, package, install and administer a plugin for the Docs, Sheets and Slides editors. Sections 1-8 are the contract between a plugin and
a host (the rules `crates/plugin-api` enforces: when this text and the crate disagree, the crate is right and this text is a
bug). Section 9 is for the person who installs plugins or runs a server. Sections 10-13 are the command catalog, the Rust SDK
with a copy-and-edit template, the version rules and the security model. Appendix A is the editor/page bridge for people who
write a host; a plugin author can skip it. Design background: `docs/phase7/PLAN.md`.

Plugins run in Docs, Sheets and Slides (`apps = ["docs", "sheets", "slides"]`, any of them; Phase 7 WP13 added Sheets and
Slides, section 6a). PDF has no plugin adapter. Signature checking and an administration page are not built (section 9.7).

## 1. Concepts

- **Package.** A zip read in memory with exactly these flat entries: `plugin.toml` (required), `plugin.wasm` (required),
  `strings.tsv` (optional: en, ru, uz-Latn, uz-Cyrl), `SIGNATURE` (optional, not checked yet). Any other name, a directory, a path or a
  duplicate refuses the package. Limits: 8 MiB in all, `plugin.wasm` 4 MiB, `plugin.toml` 64 KiB, 64 entries.
- **Manifest.** `plugin.toml` declares who the plugin is, what it may touch (permissions) and what it adds to the UI
  (commands, toolbar buttons, menu items, panels, events). Everything it declares is checked before the binary is read.
- **Binary.** One core WebAssembly module, the same file on every platform (desktop: wasmtime; web and Android: the browser's
  WebAssembly in a Worker). It has no way out of its sandbox except the messages below.
- **Rounds.** The host calls the plugin with a message; the plugin answers with *actions* (data); the host checks and runs them;
  if the answer asked for more input (`next`) the host calls again with the results. At most 16 rounds per invocation.
- **Effects are data.** A plugin never edits the document itself. It returns actions such as `run`, `textmap.apply`,
  `fetch`, `ui.panel`. All mutating actions of one invocation are one undo step; an error in any round puts the document back
  as it was before the first action.
- **Stateless calls.** A native call may start a fresh instance of the module, and a web Worker may be restarted after a timeout.
  Keep nothing between calls; everything a call needs comes in the call (params, settings, reads, results).
- **Permissions.** Declared in the manifest, shown in the install dialog, enforced per action by the host (section 8).
- **Plugin commands are registry commands** (`plugin.<plugin id>.<command id>`) and show in the command palette. Do not rely on
  macro recording or Repeat to replay them: on the web a plugin command returns before it has finished.

## 2. Manifest reference (`plugin.toml`)

Unknown keys are refused. Top-level keys (`events` among them) must come **before** the first `[table]` or `[[table]]` header,
because TOML puts every key after a header into that table.

| Key | Rule |
|---|---|
| `id` | `[a-z0-9.-]`, 3-64 characters, starts and ends with a letter or digit, no `..`. Unique per install. Never the product name. |
| `version` | `MAJOR.MINOR.PATCH` with an optional `-prerelease`; numbers without leading zeros; no `+build`. |
| `api` | The API major the plugin was built for. The host lists the majors it runs (v1 hosts: 1); anything else is refused. |
| `apps` | One to four of `docs`, `sheets`, `slides`, `pdf`; no repeats. `docs`, `sheets` and `slides` host plugins (`pdf` does not yet); an editor loads only the plugins that list it. |
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

Command `params` are defaults. When the command runs, the caller's params are merged over them and the result is the call's
`params`. Two names are read by the host itself: `scope` (`auto`, `selection` or `document`: which paragraphs a text map read
covers) and `fieldResults` (boolean, default true: whether field results appear in the text map as non-editable segments).

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
| Imports | Exactly `env.memory` (required; the host creates it with `maximum = memory_mb`) and optionally the debug `plugin_v1.log(ptr: i32, len: i32)`. Anything else refuses the load. |
| Exports | `api_version() -> i32` (= 1), `alloc(len: i32) -> i32`, `free(ptr: i32, len: i32)`, `call(ptr: i32, len: i32) -> i64` (the answer as `ptr << 32 | len`). No exported memory. |
| Message | `u32 header_len, header (UTF-8 JSON), u32 blob_count, then per blob: u32 len, bytes`. Little endian. |
| Limits per round | Input and output 32 MiB each; header 1 MiB; 16 blobs; 16 rounds per invocation; 5 s per round; memory `memory_mb`. |

The Rust SDK (section 11) writes the exports and the codec; write them by hand only for another language. The host calls `alloc`,
writes the call there, calls `call`, reads the answer at the returned `ptr << 32 | len` and calls `free` on both.

The parser checks every length against the bytes left before it allocates, refuses trailing bytes and invalid UTF-8, and
treats a truncated message as an error. A host call is read leniently by the plugin (unknown fields are ignored, which is how
the API grows inside a major); a plugin answer is read strictly by the host (an unknown action or field refuses the whole
invocation).

**The call header** (host to plugin):

| Field | Meaning |
|---|---|
| `kind` | `command`, `ui` (a click or edit in a panel), `event`, or `resume` (the next round of the same invocation). |
| `api`, `api_minor` | The API major (1) and the minor of this host (section 12). |
| `app`, `lang` | `docs`; the UI language (`en`, `ru`, `uz-Latn`, `uz-Cyrl`). |
| `command` | `command`: the command id (without the `plugin.<id>.` prefix). |
| `widget` | `ui`: the widget id. The text of a `textField` or the chosen option of a `select` is in `params.value`. |
| `event` | `event`: the event name. |
| `params` | `command`: the manifest's `params` merged with the caller's. `resume`: empty. |
| `settings` | The plugin's settings: the manifest's `[settings]`, replaced (not merged) by the admin's or the user's. |
| `results` | `resume`: one result per action of the previous round (section 4). |
| `reads` | What was read, as `name -> index of the blob that holds it` (`textmap`, `selection`, `paragraph`). |

**The answer header** (plugin to host): `{"actions": [...], "next": {"reads": [...]}}`. Both keys are optional. An answer without
`next` ends the invocation.

## 4. Actions

| Action | Needs | Effect and result |
|---|---|---|
| `run {id, params}` | `document = write` (`read` for the document reads) and an id on the allowlist (sections 8, 10) | Runs a catalog command. Result: the command's own result (JSON). |
| `textmap.apply {blob}` | `write` | Applies edits to the text map read in this invocation (section 5). Result: `{paragraphs, runs}`. |
| `read {what, scope?, index?}` | `read` | `what` = `textmap`, `selection`, `paragraph`. Result: `{blob}`; the bytes are in the next call's `reads`. The web host does not serve it (the result is an error): use `next.reads`, which works everywhere. |
| `fetch {url, method, headers, body}` | a granted network origin | Done by the host. Result: `{status, body}`, or `{error}` when the request failed. |
| `ui.panel {panel, widgets}` | the panel is declared | Replaces the panel's widgets; an empty list closes the panel. Result: null. |
| `ui.status {text}` | none | One status line (up to 256 characters, no control characters). Result: null. |

`next: {reads: [{what, scope?, index?}]}` asks the host for reads before the next round; it is the way to read in an `event`
invocation (which may not run `read` actions). **Every read needs `document = read` or `write`**, however it is asked for (a
command's `reads`, `next.reads` or a `read` action); a plugin with `document = none` is stopped with "reading the document needs
the document permission". `paragraph` needs `index` and a text map read earlier in the same invocation. `selection` is the
selected text only, UTF-8, and empty for a collapsed caret.

At most 256 actions per answer. All actions of an answer are checked before the first one runs. An `event` invocation may run only
`ui.*` actions.

`fetch` rules: the URL must be plain `https` and its origin must be one the user or admin granted, byte for byte. A URL with a
user name, a backslash, whitespace, a control character or more than 2048 characters is refused (clients disagree about where
the host ends in such URLs). Methods: GET, POST, PUT, PATCH, DELETE. Up to 16 headers (names `[A-Za-z0-9-]`, values up to
4096 characters without control characters); `Host`, `Cookie`, `Content-Length`, `Connection`, `Transfer-Encoding`, `Origin`,
`Referer`, `Upgrade`, `TE`, `Expect`, `Proxy-*` and `Sec-*` are refused. The body is a string of at most 1 MiB. The host sends no
credentials of its own and applies the response size and time limits (section 8).

A `fetch` to an origin the manifest does not list (or that nobody granted, natively) is a permission violation like a refused command: the
whole invocation is aborted, the document is restored and the plugin gets no further round. The one recoverable case is the web grant
dialog: when the user says no, the next round sees `{error: "network not permitted"}`. A `next.reads` list (or `read` actions) of more than 16
reads in one round is refused the same way.

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
invocation; the host refuses an apply when the document changed since the read (`stale revision`). The scope of the read is the
command's `scope` param (`auto` = the selection when there is one, else the whole document) or the `scope` of the `read`.
The SDK has `TextMap::decode`, `Apply::encode` and `Reply::textmap_apply`; `plugins/uz-translit` is the worked example.

## 6. Contributions and widgets

A toolbar button goes into a "Plugins" group of the simple ribbon (a "Plugins" custom tab in the full ribbon). A menu item goes
into the Plugins group of the Tools menu or the text context menu. Every command is also a registry command, so it shows in the
command palette. A panel is a side panel on the desktop, a panel beside the editor on the web, a bottom sheet on a phone. A
panel opens when a command, a click or a resume call answers `ui.panel` with widgets; an event refreshes a panel that is open
and never opens one.

Widgets (the fixed list; at most 64 per panel; text up to 1024 characters, no control or bidi characters, line breaks only in
`text`): `heading {text}`, `text {text}`, `list {items: [{id?, text}]}` (up to 256), `button {id, label}`,
`textField {id, value, placeholder?}`, `select {id, options: [{value, label}], value?}` (up to 256). Widget ids are letters,
digits, `_`, `-`, up to 48. A click on a `button` calls the plugin with `kind = ui` and `widget` = its id; editing a `textField`
or choosing a `select` option does the same with the text or value in `params.value`. Panel content is declarative: no markup,
no script, text is always shown as text.

## 6a. Sheets and Slides

The same API, package, permissions and limits as Docs; the adapter is `crates/plugin-adapter` with the engine sides
`domains/sheets/crates/sheets-plugins` and `domains/slides/crates/slides-plugins`. The call's `app` is `sheets` or `slides`. What
differs:

- **Commands.** Every command of an enabled plugin is listed in a "Plugins" menu of the simplified toolbar, one submenu per plugin
  (there is no command palette for plugin commands in these editors, so `[[toolbar]]` and `[[menu]]` entries add nothing). The
  menu is not in the full ribbon. A finished command's `ui.status` is the toast (Sheets) or the status bar (Slides). At most 64
  distinct plugin command ids per process (the engines' registries cannot remove a command, so an id keeps its slot until the
  app restarts).
- **Allowlist** (`plugin_api::allow::SHEETS` and `SLIDES`, deny by default, tested against the real registries in the two
  crates). Sheets: `cell.`, `range.`, `home.`, `selection.` minus `home.addIns`, `home.lockCell`, `home.insertCopiedCells`; reads
  `document.inspect`, `sheet.read`, `cell.get`, `selection.stats`. **No formula**: a params string that Sheets would read as a formula (`=...`, `+A1`, `-SUM(`,
  `@...`, leading blanks skipped) refuses the `run`, because a formula can reach other files and programs (external references,
  `WEBSERVICE`, `HYPERLINK`, DDE). Slides: `text.`, `format.`, `table.`, `selection.` minus the two dialog commands; reads
  `document.inspect`, `slide.inspect`, `shape.inspect`; the keys `hover` and `action` (hyperlinks that can start a program) are
  refused like `link`.
- **Text map, Sheets.** One paragraph per constant text cell in scope (row by row, sheet by sheet), one editable segment each (a
  cell holds one plain string; the engine keeps no rich text). Formulas and their results, numbers, booleans, table header cells,
  locked cells of a protected sheet, spill and in-cell picture cells are not in the map. `scope`: `auto` (the selection when it is
  more than one cell, else the sheet), `selection`, `sheet`, `workbook`, `document` (= `workbook`). The apply checks every cell
  against the read, writes the changed ones as one edit and recalculates the formulas that read them.
- **Text map, Slides.** The shapes, groups, table cells and speaker notes of the scope, one paragraph per text paragraph, one
  segment per piece: a run, or the part of one inside or outside a selected text range (`outside the selection`). Field, break and
  equation runs are `object` segments. The run's language tag is the segment's. `scope`: `auto` (the selected text range, else the
  selected shapes, else the deck), `selection`, `slide`, `deck`, `document` (= `deck`). Masters and layouts are not in it. The
  apply writes the pieces back as runs (properties and link targets kept), merges what `normalize` merges, and keeps a selected
  range on the same text.
- **Protected documents.** A workbook with a protected sheet or a protected structure is protected: a plugin that may write is
  refused on it ("the document is protected"); a reading plugin runs. Slides has no protection.
- **Undo, restore, stale.** As in Docs: one undo step per invocation (labelled with the command), any error restores the
  document, the selection and the undo and redo stacks, and an apply after the content changed is refused. In Slides the
  revision is the content's (a selection change does not make a read stale).
- **Not in v1:** events are never sent; a panel is drawn on the web (by the page host) but not in the desktop apps (a
  `ui.panel` action is accepted and kept).
- **Web.** The bridge of Appendix A with the prefix `gridcraft` or `deckcraft` (no `pluginEvent`). While a call waits for the
  page, the input lock is a command hook: every command that changes the document (and `file.*`, `edit.*`, `plugin.*`,
  `office.*`) is refused with "another plugin is still running"; moving the selection and changing the sheet or slide stay
  possible.

## 7. Events

`documentOpened`, `documentSaved` and `selectionChanged` (debounced 200 ms) are sent only to plugins that list them, in Docs
only (section 6a). An event
call can only answer with `ui.panel` and `ui.status`; it cannot run commands or change the document. It may ask for reads in
`next.reads` (needs `document = read` or `write`); the resume call carries them and may answer `ui.*` again. A failed event call
is recorded on the plugin and never shown to the user.

## 8. Permissions, limits and what is refused

- **Document**: `none` / `read` / `write`, declared. Every read (section 4) needs `read`; `run` of a document read needs `read`;
  every other allowed command, and `textmap.apply`, needs `write`.
- **Network**: per origin. In server mode only the admin grants; in local mode the user ticks the address on the Plugins page
  (desktop) or the operator lists it (local web server); on the Android local app never (the app has no INTERNET permission). The
  manifest cannot ask for an IP address or `localhost`, and the native host refuses to connect to a private address behind a
  public name. Reading the document and reaching the network are separate grants; a plugin that has both can send document text
  to a granted origin, and the install dialog shows both.
- **Native fetch** (`crates/plugin-host/src/net.rs`, used by the desktop and CLI hosts through `docs-office::plugin_native`): only a granted
  origin, plain https; the name is resolved in the host and one refused address among the answers refuses the name (loopback, 10/8,
  172.16/12, 192.168/16, 169.254/16, CGNAT 100.64/10, unspecified, multicast, reserved v4; v6 loopback, ULA, link-local, multicast
  and every v6 form that carries a v4 address, judged by that address); the connection goes to the checked address, TLS uses the
  operating system's roots; redirects are followed inside the same origin only (3 hops, each resolved and checked again); 10 s
  and 1 MiB per call; the answer is `{status, body}`. Private targets are always refused; there is no admin switch for them.
- **Command allowlist** (deny by default; the data is `plugin_api::allow`, the generated list is `docs/plugins-commands.md`,
  section 10). Allowed families: `caret.`, `select.`, `text.`, `format.`, `font.`, `para.`, `styles.`, `list.`, `table.`,
  `fields.`, `developer.`, `insert.`, `review.`, minus the named exceptions, plus the reads `document.text`,
  `document.paragraph`, `document.selection`, `document.inspect`. Never allowed: `file.*`, `edit.*` (the system clipboard and the
  user's undo stack), `tools.*` (macros), `mailings.*`, `link.*`, `view.*`, `readAloud.*`, `ui.*`, `office.*`, `plugin.*` (no
  plugin runs another), `document.setText`, and every family not listed (`layout.`, `arrange.`, `hf.`, `design.`, `picture.`,
  `shape.`, `outline.`, `references.`, ...). Also refused inside the allowed families because they take a file, a path or
  bytes: `insert.textFromFile`, `insert.picture`, `insert.link`, `insert.object`, `font.add`, `font.loadSystem`, `font.remove`,
  `review.compare`, `review.combine`, `review.acceptAll`, `review.rejectAll`, `styles.organizer`, `styles.copyFrom`.
  Refused for their effect: `insert.field` and `fields.setCode` (an instruction can be `HYPERLINK`, `INCLUDEPICTURE`,
  `INCLUDETEXT` or `DDEAUTO`, which reach a file or another program once the document is opened in Word),
  `review.addToDictionary` (writes the user's dictionary file, outside the undo step), `review.accept` and `review.reject` (a loop
  of them is accept all or reject all), `review.restrict` (a plugin must not lock the document or set a password).
  Of the 633 Docs commands, 300 are allowed (2026-11-03). A `run` whose `params` hold a key named `path`, `dir`, `file`, `url`,
  `uri`, `href`, `src`, `link`, `ins`, `del`, `fmtChange` or `numChange` (any depth, any case; so also inside `styles.*` and
  `format.set`: `link` is a hyperlink target, the others forge tracked-change attribution) is refused, except a value that is an array of non-negative integers (a document
  node path, so `document.paragraph {path: [3]}` works). A test checks that every exception exists in the registry and that no
  allowed command's `params` description mentions a path or URL.
- **Refused at install**: a manifest that breaks section 2, an `api` the host does not run, a binary that imports anything but
  section 3 lists or exports its memory, a package that breaks section 1.
- **Refused at run time**: a message that breaks section 3, an answer with an unknown action, an action without its permission,
  a text map apply that breaks section 5, a round over its time or memory budget, more than 16 rounds, a document that changed
  while the plugin ran. The document is then restored and the status line says the plugin was stopped.
- **Limits** (the numbers live in `plugin-api/src/wire.rs`; `web/plugins/host.js` repeats them):
  - *Memory*: `memory_mb` (at most 64) for the one memory the host creates. A module that defines a memory of its own, or any wasm GC
    type, is refused at load (natively multi-memory is off; on the web `worker.js` reads the section headers before compiling).
  - *Stack*: the wasm call stack is 256 KiB; deeper recursion is a trap. Natively each round runs on its own 1 MiB thread, so the
    editor's stack does not matter.
  - *Time*: 5 s per round (50 ms tick natively); **10 s per invocation**, all rounds and all host-side work together, checked before
    each round, action and fetch (a fetch that starts just before the end can still take its own 10 s); at most **8 fetches** per
    invocation; an `event` gets **one round of 100 ms** and cannot fetch. Running out of time fails the invocation and restores the
    document. A plugin that runs out of time 3 times in a session is switched off until it is enabled again (native) or the page is
    reloaded (web).

## 9. Installing, enabling and administering

Code: `crates/suite-core/src/plugins.rs` (the store, shared by the desktop, the local web server and the docserver),
`services/suite-desktop` (IPC), `services/suite-serve` (`/plugins/<sha256>`), `services/docserver/src/{config,plugins}.rs`,
`web/plugins/*` (the page host and the Plugins page). The rules behind it: users may install unless a machine policy says
otherwise; a package is pinned by the SHA-256 of its `plugin.wasm`; signatures are optional and not checked yet; administrators
control plugins through configuration files.

### 9.1 What an install is

A zip (or, installed, a folder) with the flat entries of section 1. The installed copy is
`<settings dir>/plugins/<id>/<version>/` with `install.toml` next to the files: `sha256` of `plugin.wasm`, `manifest_sha256` of
`plugin.toml`, and a reserved `signature` field that nothing reads yet. IT's copies go in
`%PROGRAMDATA%\<product>\plugins\<id>\<version>\` (a debug build honours `SUITE_MACHINE_DIR`). Installing never enables. A new
install of an id whose manifest differs from the installed one turns that id off and withdraws its grants (it needs fresh
consent), and the other versions of the id are removed.

**Integrity.** A package whose files no longer match its record is refused: not listed for the page, not served, said on stderr
and (docserver) audited. A package without a record is refused in the user folder and accepted in a machine or operator folder.
`%PROGRAMDATA%` is writable by standard users by default, so the MSI creates `%PROGRAMDATA%\<product>` with a protected DACL
(SYSTEM and Administrators full, Users read and execute), and the desktop host ignores the machine folder and `policy.toml`
(said once on stderr) unless that folder is owned by Administrators or SYSTEM and no one else holds a write right
(`suite-winsec::folder_is_admin_only`). The `icacls` custom actions are in `scripts/build-msi.ps1`; they have not been exercised
on an installed MSI yet.

### 9.2 Desktop

The Plugins page lists installed packages with their permissions; the user installs from a `.zip` (the host shows the file
dialog; a page cannot name a path), enables, and ticks network origins. `settings.toml`:

```toml
[plugins]
enabled = ["id"]

[plugins.grants]
"id" = ["https://origin"]

[plugins.settings.id]      # optional: the user's values for the plugin's [settings] (they replace the manifest's)
```

IPC (exact fields, 8 KiB, own origin only, like `open`): `plugins_list{}`, `plugins_install{}`, `plugins_enable{id, enabled}`,
`plugins_grant{id, origin, granted}`. Every answer is the event `plugins`
`{ok, error?, installed?, items: [{id, version, name, publisher, apps, document, network, grant, enabled, machine, sha256, problem}], policy}`.
A grant must be an origin the manifest declares. The shell reads the enabled list on every request: no restart. Native editor
windows load the enabled, verified plugins at start (`docs-office::plugin_native`).

**Machine policy.** `%PROGRAMDATA%\<product>\policy.toml`:

```toml
[plugins]
user_install = false   # the user folder is not read at all: nothing in it is listed or enabled
user_grants = false    # no user grant counts (existing ones are ignored; withdrawing stays possible)
```

A policy file that cannot be read or parsed switches both off. No file: both allowed.

### 9.3 Web, local (`<product>-serve`)

In the server's config file: `[plugins] dir = "..."`, `enabled = [ids]`, `[plugins.grants] "id" = [origins]`, optionally
`[plugins.settings.<id>]` and `[plugins.sha256] "id" = "<hex of plugin.wasm>"` (a package whose wasm differs is left out). The
operator is the only one who enables or grants. The list is worked out once at start (restart to change it); the wasm is read
again and compared on each serve. `/shell-config.json` gets `plugins` (descriptors); `/plugins/<sha256>` serves an enabled
package's wasm. The home page's CSP is `connect-src 'self' <granted origins>` (computed per request).

### 9.4 Docserver (per organisation = per provider)

`[docserver.plugins] dir` names the folder of packages; per `[[docserver.providers]]`:

```toml
plugins = [{ id = "id", version = "1.0.0", sha256 = "<hex of plugin.wasm>", grant = ["https://host"], settings = { } }]
```

At start the folder is read; a malformed entry, a `grant` that is not an `https://host[:port]` origin the manifest declares, or
`plugins` without `dir` refuses the start. An entry whose package is missing or whose SHA-256 differs is left out and audited
(`plugin.refused`). The entry's `settings` replace the manifest's. The session answer has `plugins: [descriptor]` of that
provider only, `wasm` = `/ds/plugins/<sha256>?s=<session id>`, `grant` = the admin's list.

- `GET /ds/plugins/<sha256>?s=<sid>` serves the file for a live session whose provider lists that digest (404 otherwise, the same
  for every other case), reading the file again on each request (409 `plugin_changed` and `plugin.refused` when it changed). It
  takes only 64 lower-case hex digits.
- `POST /ds/session/{sid}/plugin-event {plugin, kind, origin}` (session token) lets the wrapper report `network`,
  `network.refused`, `network.denied` from the page host. These lines are claims of the page, marked `"source":"client"`: a
  `network` event is accepted only for an origin in that plugin's `grant` (else 400, not logged).
- **Audit lines:** `plugin.loaded`, `plugin.refused`, `plugin.network` (the first two at most once a minute per session and sha).
- The config token is never read for plugins. The wrapper page's CSP is `connect-src 'self' <the origins granted to any
  provider's plugins>` (one list for all providers, because the page's policy is fixed before its session is known; the page
  host still lets a plugin reach only its own descriptor's `grant`).

### 9.5 Grants at run time

In the desktop, local web and docserver modes the descriptor always carries `grant` (possibly empty), so the page host never
asks at run time: the user ticks the address on the Plugins page (desktop) or the operator lists it (web, docserver). A granted
`fetch` passes the host check and the page's CSP; an ungranted one is stopped by both. Only a page host that gets a descriptor
without `grant` (someone embedding the host) asks the user once per origin and keeps the answer in `localStorage` `plg.grants`.

### 9.6 Android

The Android app runs the same web page host inside its WebView, with the same Worker CSP. It has no INTERNET permission, so no
network grant can work there. Installing a plugin on Android (a file picker) is not built, and no plugin is bundled in the app yet.

### 9.7 Disabling, and what is not done

Remove the id from `enabled` (local) or the entry from the provider (server); the editors drop the commands and the panel at
the next page load (local) or the next document open (server). Not built: the signature check (the `SIGNATURE` entry is
accepted and ignored), an admin page (configuration files only), uninstall, update-in-place prompts, events and native panels in
Sheets and Slides (section 6a), a PDF adapter.

## 10. The command catalog

`run` can call 306 of the 633 Docs registry commands. `docs/plugins-commands.md` lists them: id, access (`read` or `write`),
label and the registry's own `params` description. It is generated from the allowlist data and the Docs command registry
(`cargo xt plugin commands`; `cargo xt plugin commands --check` fails when the file is stale and runs in `scripts/ci.sh
gen-check`). Do not edit it. `params` is the registry's free-text description of the JSON object the command takes
(`{"size": number}`); a command that needs nothing takes `{}`.

A `run` result is the command's own result. A command that fails (disabled in the current state, bad params) ends the
invocation and the document is restored; read first when the state is uncertain.

## 11. The Rust SDK and the template

`crates/plugin-sdk` is the glue for the ABI of section 3: the `alloc`/`free`/`call` exports, the message codec, `Reply` and the
typed actions and widgets of `plugin-api` (re-exported: `Action`, `Call`, `CallKind`, `Next`, `ReadReq`, `ReadWhat`, `Scope`,
`Widget`, `TextMap`, `Apply`). A plugin is a `cdylib` for `wasm32-unknown-unknown` with one handler,
`fn(Call, Vec<Vec<u8>>) -> Result<Reply, String>` (the call and the blobs that came with it), exported with
`plugin_sdk::export!(handler)`. `Reply::status(text)` is a status line; build a `Reply { actions, next, blobs }` for the rest;
`plugin_sdk::read(&call, &blobs, "textmap")` returns the bytes of a named read. An `Err` becomes a `ui.status` line and changes
nothing.

### 11.1 Quick start

The smallest plugin is the folder `plugins/template` (one command that opens one panel, whose button shows a status line).
Copy it, rename it, change the code.

```
plugins/template/
  Cargo.toml     package name, dependency on plugin-sdk, crate-type = ["cdylib", "rlib"]
  build.rs       passes --import-memory to the linker (the host creates the memory; section 3); copy it unchanged
  plugin.toml    the manifest (section 2)
  strings.tsv    optional: your UI texts in en, ru, uz-Latn, uz-Cyrl
  src/lib.rs     the handler, plugin_sdk::export!(handler), unit tests
```

1. Copy the folder to `plugins/<name>` and change `name` in `Cargo.toml`. The root workspace already includes `plugins/*`.
   For a folder **outside** the repository, also: add an empty `[workspace]` table to its `Cargo.toml`; replace
   `edition.workspace = true` and `license.workspace = true` by `edition = "2021"` and a licence; set
   `plugin-sdk = { path = "<repo>/crates/plugin-sdk" }`; keep `[lib] crate-type = ["cdylib", "rlib"]` and `build.rs`.
2. In `plugin.toml` set a new `id` and `version`, name the commands, and declare only the permissions the code needs.
3. Write the handler in `src/lib.rs`. Match on `call.kind` (`command`, `ui`, `event`, `resume`) and `call.command` or
   `call.widget`. Return `Reply::status(...)` or a `Reply` with actions.
4. Test natively: `cargo test` calls the handler with a `Call` built from JSON (the tests in the template do; they need
   `serde_json` as a dev-dependency). The `export!` macro defines the ABI only on `wasm32`, so the crate also builds and tests as
   a normal library.
5. Build and pack with the repo's `xtask`, run from the repository:

```
cargo xt plugin build <name>      # plugins/<name>, `plugin` profile (speed, no unwinding, stripped)
cargo xt plugin pack <name>       # also checks plugin.toml and the entry list with plugin-api; writes <target>/plugins/<id>-<version>.zip
cargo xt plugin pack <folder>     # a name with a path separator (../hello, E:/work/hello) is a folder outside the workspace; built with --release
```

   `<target>` is `CARGO_TARGET_DIR` (for a folder outside the workspace without it: `<folder>/target`). `pack` stores the entries
   in a fixed order with a fixed timestamp and mode, so equal inputs give equal bytes, and prints the SHA-256 of the zip. The
   `wasm32-unknown-unknown` target must be installed (`rustup target add wasm32-unknown-unknown`). The wasm file is
   `<target>/wasm32-unknown-unknown/<plugin|release>/<package name with _>.wasm`.
6. Install the zip (section 9) and enable it. To load it in a native test instead, copy
   `domains/docs/crates/docs-translit/tests/template.rs`: it starts the real module in wasmtime, registers it, runs the command
   and checks the result. It reads the module from `TEMPLATE_PLUGIN_WASM`; with `TEMPLATE_PLUGIN_MANIFEST` (your `plugin.toml`)
   and `TEMPLATE_PLUGIN_COMMAND` (a command id) it loads your plugin and checks that the command runs and shows a status line:

```
TEMPLATE_PLUGIN_WASM=<wasm> TEMPLATE_PLUGIN_MANIFEST=<plugin.toml> TEMPLATE_PLUGIN_COMMAND=<id> \
  cargo test --manifest-path domains/docs/Cargo.toml -p docs-translit --test template
```

   (the docs domain needs `upstream/craft-fonts`; see `docs/UPSTREAM.md`).

`strings.tsv` has a header row (`key`, `en`, `ru`, `uz-Latn`, `uz-Cyrl`, then a status column) and one tab-separated line per
text. It is shipped in the package and the plugin reads its own copy at build time (`include_str!`); the template's `tr` picks
the column from `call.lang`. Texts of a third-party plugin are the author's responsibility; they do not pass the project's
Uzbek review.

`build.rs` is required: without `--import-memory` the module defines its own memory and the host refuses it. It must pass the
flag **only for `wasm32`**, or a native `cargo test` fails to link. This is the whole file:

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg-cdylib=--import-memory");
    }
}
```

(`plugins/template/build.rs` also sets `-zstack-size=262144` and `--no-entry`; both are optional.) The smallest `src/lib.rs`, a
plugin whose only command shows a status line:

```rust
use plugin_sdk::{Call, Reply};

fn handler(_call: Call, _blobs: Vec<Vec<u8>>) -> Result<Reply, String> {
    Ok(Reply::status("Hello"))
}

plugin_sdk::export!(handler);
```

and the `Cargo.toml` of a plugin outside the repository:

```toml
[package]
name = "hello"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
plugin-sdk = { path = "<repo>/crates/plugin-sdk" }

[dev-dependencies]
serde_json = "1"

[workspace]
```

On a native build the compiler warns that the handler is unused (the export exists only on `wasm32`); ignore it or add
`#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]`.

Testing a handler: `Call` implements `Deserialize` (build it with `serde_json::from_value` from the header fields of
section 3; `kind`, `api`, `api_minor`, `app` and `lang` are required, the rest optional). `Reply` has public fields `actions`,
`next` and `blobs` and implements neither `Debug` nor `Serialize`; check `reply.actions`, for example
`matches!(&reply.actions[..], [Action::UiStatus { text }] if text == "Hello")`. `pack` rebuilds the module first, so you do not
need to run `build` before it.

Keep `serde_json` out of the normal dependencies unless you need it (it is most of a small plugin's size).

Debugging: a module may import `plugin_v1.log(ptr: i32, len: i32)` (UTF-8 text) itself; the SDK has no helper in v1. The host
keeps up to 64 lines of 256 bytes per session for the audit log and never shows them to the user. The user sees the `Err`
text of the handler, or "plugin stopped" with the reason when the host refused the answer.

### 11.2 The first-party plugins

- `plugins/uz-translit` (`document = write`, `apps = ["docs", "sheets", "slides"]`): the three transliteration commands as one
  text map round: the command `reads` `textmap`, converts the editable segments with the same `uz-translit` crate as the built-in
  command, and answers `textmap.apply` plus a status line with the counts ("cells" in Sheets). `auto` finds the direction from the
  text the app's built-in reads: in Slides only the segments it would convert (not fields, not other languages). It is the model
  for any bulk text plugin.
- `plugins/template-fields` (`document = write`, one event, `apps = ["docs"]`: content controls and bookmarks are Docs only): a panel, `ui` calls, catalog commands with parameters, an event
  with a read, and settings. The field list is configuration only: `settings.fields = [{name, label, kind}]` (`kind` is `text`,
  the default, or `bookmark`). The manifest carries a synthetic default list; an administrator replaces it per provider and, in
  local mode, the user does with `[plugins.settings.<id>]`. No real field name is in the code, tests or docs.
  - **Panel `fields`:** heading, filter box, "Check fields" button, and a `list` whose items (`id` = `ins-<position>`) are the
    "Insert" buttons. Every call is stateless, so every answer redraws the whole panel.
  - **Insert** (`ui` call): a `text` field runs `developer.plainText {title: label, tag: name}`, then
    `developer.properties {placeholder: label, lockDelete: true}` (a content control, saved as `w:sdt` with `w:tag` = the name).
    A `bookmark` field runs `text.insert {text: label}`, one `caret.left {extend: true}` per character and
    `insert.bookmark {name}` (so the bookmark covers exactly the label).
  - **Highlight:** `selectionChanged` answers with `next.reads = [selection]` and no action; the resume call carries the selected
    text and the answer is `ui.panel` with the field whose label equals the text marked. The selection read is text only and
    empty for a collapsed caret, so a field is recognised while the user has selected its placeholder.
  - **Check fields:** `run document.inspect {text: false}`, then the tags of the `controlStart` objects and the names of
    `bookmarkStart` objects are compared with the list. The inspect read covers body paragraphs (the first 2,000 blocks), not
    objects inside tables, headers or footers.
  - **Limits:** at most 256 fields; names up to 64 characters (bookmark names follow `insert.bookmark`: a letter first, no
    spaces, up to 40); an invalid entry is skipped and counted in the panel.

## 12. Versioning and compatibility

- `api` in the manifest is the major. A host lists the majors it runs (v1 hosts: `[1]`); a plugin with another `api` is refused at
  install.
- Additions inside v1 (a new action, context field, widget or event) raise `api_minor`, which every call carries
  (`plugin_api::API_MINOR`, 0 today). A plugin reads the call leniently and ignores what it does not know, so it keeps running; a
  plugin that needs a newer minor should check `call.api_minor` and say so in a status line.
- A change of meaning or a removal is `api = 2`. A host then runs 1 and 2 side by side for at least one release.
- The host reads a plugin's answer strictly: an action the host does not know stops the invocation, so a plugin that uses a newer
  minor than its host fails cleanly instead of being guessed at.
- The allowlist may grow inside a major (a new registry command in an allowed family is allowed at once; an exception may be added
  when a command turns out to take a file or bytes). Treat `docs/plugins-commands.md` as the list for the host you run.

## 13. Security model and reporting

**Trust.** A plugin is code from a third party running in the user's session. The user or the administrator decides which plugins
exist (section 9); the host decides what each one may do.

**Layers.**

1. The package is checked before anything runs (sections 1, 2); the wasm is pinned by SHA-256 and read again on use.
2. The module can only import `env.memory` and `plugin_v1.log`; no WASI, no filesystem, no network import exists. The browser
   check (`WebAssembly.Module.imports()`) and the native check are the same rule.
3. Runaway code is stopped: an epoch deadline natively, `Worker.terminate()` on the web (5 s per round); memory is capped at
   `memory_mb` by the host-made memory.
4. Every answer is parsed strictly and checked as a whole before its first action runs: permissions, the command allowlist,
   path-like params, network origins and fetch rules, widget limits.
5. Network: the worker has a CSP of `default-src 'none'`; `fetch` runs in the host after the grant check; native fetch refuses
   private addresses; the page's own CSP lists only granted origins.
6. The document is restored on any error; on the web it is locked for input while a plugin runs; plugin text reaches the screen
   as text only, with control and bidirectional characters removed.

**What it does not protect against.** A plugin with `document = read` sees the text it reads; with a network grant too, it can
send it to that origin. The SHA-256 pin proves the file is the one the administrator named, not that it is safe; signatures are
not checked yet. A flaw in wasmtime or in the browser's WebAssembly engine would let a plugin out of the sandbox; both are
pinned and covered by the project's advisory checks, and an out-of-process host is the hardening if third-party plugins at scale
are wanted. Plugin strings are not reviewed by the project.

**Reporting.** Report a suspected sandbox escape, allowlist hole or permission bypass privately by e-mail to
bohodirovasadbek1@gmail.com, not in a public issue.

## Appendix A. Host implementers: the web page host and the editor bridge

On the web the page that hosts the editor iframe owns the plugin's Worker and builds every call; the editor (the Docs wasm,
`docs-plugins::bridge` and `docs-office::plugin_web`; patch `wordcraft/0148` only forwards the messages) does what only it can:
read the document, run the **editor actions** (`run`, `textmap.apply`) after the allowlist and grant check of sections 4 and 8,
keep the undo step, the revision and the snapshot. The page keeps `ui.*` and `fetch` and never forwards them.

### A.1 The page host

`web/plugins/host.js`, `worker.js`, `host.css`, `i18n.json`. The shell (`web/shell/shell.js`) and the docserver wrapper
(`services/docserver/static/editor.js`) fetch `host.js` **only when a plugin is enabled** (a non-empty `plugins` list in
`/shell-config.json`, or in the docserver session answer); with none, no request for any of these files is made.

- **Descriptor** (what the server lists per enabled plugin; produced from `plugin.toml` with `plugin-api`, which has validated it):
  `{id, version, sha256, wasm: <same-origin URL of plugin.wasm>, manifest: <plugin.toml as JSON, same key names>, grant?: [origins], settings?}`.
  The host refuses a wasm whose SHA-256 differs. `grant` present: only those origins are granted and nobody is asked; absent:
  the user is asked once per origin.
- **One Worker per plugin**, made at the first call. `worker.js` compiles the module, refuses it unless its imports are exactly
  `env.memory` and optionally `plugin_v1.log` and it exports no memory (error text `imports not allowed: ...`, `the module exports
  a memory`), creates the memory itself with `maximum = memory_mb * 16` pages, and answers `call` messages. The page measures the
  time budget (5 s, an admin may lower it with `pluginBudgetMs`) from posting a round until the answer; on expiry it calls
  `terminate()`, tells the editor to abort, and starts a new Worker at the next call.
- **CSP**: `/shell/plugins/*.js` is served with `default-src 'none'; script-src 'wasm-unsafe-eval'` (suite-serve `WORKER_CSP`; the
  Android asset loader has the same constant). A Worker takes the policy of its own response, so the plugin code cannot fetch or
  import anything. The page's own policy (`connect-src 'self'`) also stops a `fetch` action to a foreign origin, so the servers
  widen it to exactly the granted origins.
- **Page-side checks** (the same rules as `plugin-api`): the wire format, a strict answer (unknown action, key or widget refuses
  it), document access per action, events may run `ui.*` only, panels must be declared, `fetch` URL/method/header rules, widget
  text cut at 1,024 characters (64 widgets, 256 items), and every plugin string reaches the DOM through `textContent`. The
  command allowlist is checked by the editor (Rust), not here.
- **Panel**: `role=region` named "<plugin name>: <panel title>", Close button, Escape closes; a side panel (the editor frame
  narrows) or, up to 600 px wide, a bottom sheet with a grabber and 48 px targets. `ui.status` goes to the shell status line (and
  the toast on a phone).
- **Strings** of the host are a catalog of their own (`i18n/plugins.tsv`, scope `plugins`, exported to `web/plugins/i18n.json` by
  `cargo xt i18n export plugins --format json`).
- **Tests**: `ci/web-smoke/plugins-host.spec.ts` (stub editor, fixture plugin in `ci/plugins/webhost-test`).

### A.2 The editor bridge

Messages are the `?host=parent` protocol (prefix `wordcraft`; `gridcraft` and `deckcraft` for Sheets and Slides, which send no
`pluginEvent`, section 6a), same origin. `call` is an integer for an invocation the editor
starts and a text (`h<n>`) for one the page starts (a panel click). Buffers are transferred `ArrayBuffer`s, never base64.

| Message | Direction | Fields |
|---|---|---|
| `wordcraft:plugins` | page to editor | `{commands, toolbar, menu, plugins: [{manifest, settings?, enabled?}], timeoutMs?}`, sent on every editor `ready`. The editor builds its slot commands, ribbon group and menus from `plugins[].manifest` (the descriptor's manifest as JSON, parsed and validated by `plugin-api`); `commands`, `toolbar`, `menu` are informational (slots 0..63, assigned by the host in manifest order). The list is the **whole** set: plugins not listed are unregistered, an empty list unregisters all. `timeoutMs` (default 20,000) is how long the editor waits for the page between two messages of an invocation. |
| `wordcraft:pluginsSet` | editor to page | `{ids, errors}`: the plugins registered and one text per refused entry (the others register). |
| `wordcraft:pluginCall` | editor to page | `{call, plugin, command, params, reads: {textmap?, selection?}}`: a plugin command ran (ribbon, menu, palette, `wordcraft:run`). `reads` are the command's `reads` as transferred `ArrayBuffer`s; `params` are the manifest defaults merged with the caller's. |
| `wordcraft:pluginActions` | page to editor | `{call, plugin, actions, blobs, reads, more, abort?}`: the editor actions of one answer (`blobs` are the answer's blobs, `textmap.apply` names one by index), `reads` the reads the next call needs (`next.reads`), `more` whether another round follows. `abort: true` (the page gave up: a trap, a refused answer, the time budget) restores the snapshot; nothing is sent back. |
| `wordcraft:pluginResults` | editor to page | `{call, results, reads, error?}`: one result per action, one `ArrayBuffer` per requested read, in order. `error` means the invocation failed, the document is as it was and the page ends it (an unknown or ended `call` gets `error` too). |
| `wordcraft:pluginEvent` | editor to page | `{event}` for `selectionChanged` (debounced 200 ms), `documentOpened`, `documentSaved`, sent when a plugin of the page lists the event; the page relays it. |

A plugin command returns at once with `{"pending": call}`. From then on the document is **locked for input** (Restrict Editing's
read-only protection, lifted while an answer's actions run, put back between rounds and never left on) until the answer with
`more: false`. The editor ends an invocation itself, with the same restore and a "Plugin stopped: ..." status line, when no
message arrived within `timeoutMs`, when the revision moved while the plugin ran (`stale revision`), and when another document is
opened. An answer for an ended invocation gets a `pluginResults` with `error`. All mutating actions of an invocation, over all its
rounds, are one undo step. A panel click's invocation begins at its first forwarded answer. `read` actions are not served (the
result says to use `next.reads`).

## Changes

- 2026-11-06 (Phase 7 WP13): Sheets and Slides host plugins (section 6a): per-app allowlists (`plugin_api::allow::list`), the text
  maps of both editors, the `gridcraft:` and `deckcraft:` bridges, the toolbar's Plugins menu; uz-translit lists all three apps.

- 2026-11-03: first complete version. Every read now needs the document permission (a plugin with `document = "none"` could read
  through `next.reads` before). The command catalog is generated (`docs/plugins-commands.md`). `plugins/template`, and
  `cargo xt plugin build|pack <folder>` for plugins outside the workspace.
