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

## 9. The web bridge (editor iframe and page host)

On the web the editor (the wasm in the iframe) is the plugin **executor** (`docs-plugins`), and the **page** that hosts the iframe
owns the plugin's Worker. Rounds therefore cross `postMessage`. Every message is a `type` plus the fields below, to and from the
same origin as the other `wordcraft:*` messages (the `?host=parent` protocol, `host.rs`). A text map and every other blob are
transferred `ArrayBuffer`s, never base64 or JSON. Numbers `call` and `rev` are integers.

| Message | Direction | Fields |
|---|---|---|
| `wordcraft:plugins` | page to editor | `{plugins: [{manifest, settings?, origins?, enabled?}], timeoutMs?}`. `manifest` is the text of `plugin.toml` (the editor parses and validates it; no TOML parser is needed in JS). `settings` overrides the manifest's `[settings]` defaults, `origins` are the network origins that were granted, `enabled` defaults to true. The list is the **whole** set the page hosts: plugins not listed are unregistered, an empty list unregisters all. `timeoutMs` (optional, default 7000) is how long the editor waits for one round. |
| `wordcraft:pluginsSet` | editor to page | `{ids: [registered plugin ids], errors: [one text per refused entry]}`, the answer to every `wordcraft:plugins`. A refused entry is skipped, the others register. |
| `wordcraft:pluginCall` | editor to page | `{call, plugin, rev, round, header, blobs}`. `header` is the call header of section 3 as an object (`kind` is `command`, `ui`, `event` or `resume`; for `resume` it carries `results` and `reads`); `blobs` is an array of `ArrayBuffer`s in the order the header's `reads` index (the text map is blob 0). `rev` is the document revision this call belongs to; `round` counts from 1. One `call` id covers a whole invocation, its rounds and its resumes. Events are calls with `kind: "event"`. |
| `wordcraft:pluginActions` | page to editor | `{call, rev, header, blobs}`: the plugin's answer. `header` is the answer header of section 3 as an object (`{actions: [...], next?: {reads: [...]}}`), `blobs` the answer's blobs (for `textmap.apply`), `rev` the value from the call, echoed. Or `{call, error: "text"}` when the page has no answer (a trap, a refused message, the Worker's own time budget). The editor parses the header strictly (section 3) and checks every action (sections 4 and 8). |
| `wordcraft:pluginDone` | editor to page | `{call, plugin, ok, error}`: the invocation is over. `ok` false means it failed and the document is as it was (`error` says why: `no answer in time`, `stale`, `the document was replaced`, a refused action, the page's own `error`); the page ends the invocation's Worker round. |

A plugin command (the ribbon, a menu, the palette, `wordcraft:run`) returns at once with `{"pending": call}`. From then on the editor
is **locked for input** (Restrict Editing's read-only protection, lifted while the answer's actions run and put back between rounds,
never left on) until `wordcraft:pluginDone`. The editor ends an invocation itself, with the same restore, when a round gets no answer
within `timeoutMs`, when the document revision moved while the plugin ran (`stale`), when `rev` of an answer is not the call's, and
when another document is opened. An answer for a call that is not pending (finished, timed out) is ignored. Event calls neither lock
nor snapshot (they can only answer with `ui.*` actions); one event of a kind is outstanding per plugin, later ones are dropped until
the page answers it. All mutating actions of an invocation, over all its rounds, are one undo step.

`fetch` actions are not served by the editor yet: the result of one is `{"error": "network is not available in this host"}`
(WP5 decides whether the page runs them).
