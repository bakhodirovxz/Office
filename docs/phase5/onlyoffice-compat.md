# ONLYOFFICE DocsAPI config compatibility (Phase 5, brief task 3)

**What this is.** For each field of an ONLYOFFICE Docs API editor config, callback and event, what our document server
does: **Supported**, **Partial** (works with a stated limit), or **Not supported** (ignored or refused, as stated).
"HRB" says whether hr-backend sends or needs it today (`docs/phase0/hrm-current-state.md` §4, owner confirmed unchanged
2026-10-25). Plan: `docs/phase5/PLAN.md`.

**Status of this file (2026-10-25).** WP5 (`api.js`, the wrapper page) is built; the rows it implements name their test in "Verified" (the spec is `ci/web-smoke/docserver.spec.ts`, Chromium, two sites, TLS). Everything else is still the plan's intent. "Target" is the Phase 5 plan's intent; "Verified"
stays "—" until a named test passes, and WP16 fills it in. A row's target changes only with the plan.

**Clean-room.** Field names come from ONLYOFFICE's **public** Docs API documentation (the editor config, events,
methods and callback reference pages) and from HRB's own use of them. No ONLYOFFICE source code (AGPL) was read, copied
or vendored, and our `api.js` is written from scratch. WP16 re-checks every name below against the public pages before
the gate; a name not found there is removed.

Legend: **S** supported, **P** partial, **N** not supported. HRB: **sent** (HRB emits it), **uses** (HRB listens or
relies on it), **—** (not used).

## 1. Top level

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `documentType` (`word`, `cell`, `slide`, `pdf`) | sent (`hrm-current-state.md:252`) | P | Selects the editor (`/docs/`, `/sheets/`, `/slides/`, `/pdf/`). Only `word`/docx is in the gate; the others open but lack read-only and save acknowledgement until their bridges get them. The wrapper picks the frame from the session's `editor` and the shell's prefix table | WP5, WP6 | docx only: docserver.spec (xlsx, pptx, pdf not run) |
| `type` (`desktop`, `mobile`, `embedded`) | sent as `mobile` on `device=mobile` (`:264`) | P | `mobile` accepted and **ignored** (`api.js` makes the same frame; layout comes from the viewport); editing is **not** forced to view; `embedded` treated as view (not yet) | WP5 | — |
| `width`, `height` | — (client-side) | S | Applied to the iframe by `api.js` (number = px, a CSS length, default 100%) | WP5 | — (implemented, no test) |
| `token` | sent: HS256 over the whole config, no `exp` (`:265`) | S | HS256 only, `alg` pinned; the verified payload is the only source of truth; `exp` required (PLAN Q4) | WP3 | — |
| `events` | uses | S | Kept on the page, never sent to the server, never signed (`:312`); `api.js` strips it from the config it posts | WP5 | docserver.spec: the POST to `/ds/session` has no `events` key |

## 2. `document`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `fileType` | sent (`:253`) | P | Allowlist docx (gate), xlsx, pptx, pdf; must match the OOXML main part of the fetched bytes; `doc`, `rtf`, `odt`, `txt` … refused | WP3 | — |
| `key` | sent: `{kind}_{id}_v{ts}_{8hex}`, new per open (`:254`) | S | Echoed unchanged in callbacks; validated (charset `[0-9A-Za-z._=-]`, ≤ 128 chars); a hash of it goes to the audit log | WP3 | — |
| `title` | sent (`:255`) | S | The file name shown and sent to the editor; sanitised | WP3 | — |
[redacted: HRM security detail]
| `info.*` (`owner`, `uploaded`, `favorite`, `folder`, `sharingSettings`) | — (`:267`) | N | Ignored | — | — |
| `referenceData` | — (`:267`) | N | Ignored | — | — |
| `permissions.edit` | sent | S | `false` → view session: the server refuses save, autosave and dirty (403) and sends no callback (not even status 4 on close); the wrapper opens the editor with `readOnly: true`, so typing, Save and exports are refused | WP3, WP7 | auth.rs `config_field_refusals_and_modes`; outbound.rs `a_view_session_reads_and_sends_no_callback`; docserver.spec: WP7 view session |
| `permissions.download` | sent `true` | P | Server: the version-history download route needs it. Editor: **no bridge flag exists** (0127 has only `readOnly`), so `false` is not enforced for an edit session. A view session refuses exports through `readOnly` whatever `download` says (stricter than asked; a view session with `download:true` cannot export) | WP7 | — |
| `permissions.print` | sent `true` | P | **Not enforced**: the editors have no bridge flag for print. Accepted and stored on the session | WP7 | — |
| `permissions.review` | sent `false` | P | Not supported yet: `review:true` with `edit:false` opens as a plain view session (read-only). Forced tracked changes need a wasm-safe policy layer (PLAN F9), after the gate | WP7 | — |
| `permissions.comment` | sent `= edit` | P | With `edit:true`: part of editing. Comment-only (`edit:false`) is **not supported**: opened as view (read-only) | WP7 | — |
| `permissions.copy`, `chat`, `fillForms`, `modifyContentControl`, `modifyFilter`, `protect`, `commentGroups`, `reviewGroups`, `userInfoGroups`, `editCommentAuthorOnly`, `deleteCommentAuthorOnly` | — | N | Ignored | — | — |

## 3. `editorConfig`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `mode` (`edit`, `view`) | sent (`:258`) | S | `view` → read-only session | WP3, WP7 | — |
| `callbackUrl` | sent for edit configs, omitted for view (`:259`) | S | Through the per-provider callback allowlist; a config without it is accepted as view only (`mode` and `permissions.edit` are then reported as view and the editor opens read-only) | WP4, WP7 | outbound.rs `a_view_session_reads_and_sends_no_callback` (the fake backend's `mode=view` config), auth.rs; docserver.spec: WP7 view session |
| `user.id` | sent `emp-{id}` / `user-{id}` (`:260`) | S | The session `sub`; echoed in callback `users[]`; the audit actor | WP3 | — |
| `user.name` | sent | P | Reaches the session; **not yet** passed on to the editor (no author name in tracked changes until the bridge has a message for it); no presence list | WP5, WP6 | — |
| `user.group`, `user.image` | — | N | Ignored | — | — |
| `lang` | sent `ru` (`:261`) | P | Used for the wrapper's own messages (ru, uz-Latn, uz-Cyrl, en) through the shell catalog; **not yet** passed to the editor (no bridge message) | WP5 | docserver.spec: the wrapper's error text is Russian for `lang: ru` (seen in the page snapshot, not asserted) |
| `region`, `location` | — | N | Ignored (number formats follow the language) | — | — |
| `coEditing.mode`, `coEditing.change` | — (`:267`) | N | One editor per session; no co-editing (Phase 9) | — | — |
| `recent`, `templates`, `createUrl` | — (`:268`) | N | Ignored | — | — |
| `actionLink` | — | N | Ignored | — | — |
| `plugins` | — (the `uz-translit` plugin is mounted server-side in ONLYOFFICE, `hr-backend-prompt.md:833`) | N | Not supported; Latin ⇄ Cyrillic is built into our editor (PLAN F13) | WP9 | — |
| `embedded.*` | — | N | Ignored | — | — |

## 4. `editorConfig.customization`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `forcesave` | sent `true` (`:262`) | P | Ignored as a switch: every explicit save and host `requestSave` sends status 6 | WP4 | — |
| `uiTheme` | sent (`:263`) | P | `theme-dark` → dark, anything else → light. **Not wired in WP5**: the wrapper does not pass it on (no bridge message) | WP5 | — |
| `autosave` | — | P | Autosave is a recovery copy on our server only; the HRM sees saves (6) and the close (2) | WP4, WP6 | — |
| `anonymous`, `comments`, `compactHeader`, `compactToolbar`, `compatibleFeatures`, `customer`, `features`, `feedback`, `goback`, `help`, `hideNotes`, `hideRightMenu`, `hideRulers`, `integrationMode`, `logo`, `macros`, `macrosMode`, `mentionShare`, `mobile`, `review`, `submitForm`, `toolbarHideFileName`, `toolbarNoTabs`, `unit`, `zoom` | — | N | Ignored; the product chrome and toolbar come from our own settings | — | — |

## 5. Events (from the editor to the host page)

| Event | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `onAppReady` | uses (`:307`) | S | When the editor bridge is ready (`<prefix>:ready`) | WP5 | docserver.spec (event reaches the host; ready is implied by the document opening) |
| `onDocumentReady` | uses | S | When the editor reports the document open (`<prefix>:opened`) | WP5 | docserver.spec: host banner `ready` |
| `onDocumentStateChange` | uses (stub) | S | `{data: true|false}` on every `<prefix>:dirty` of the editor | WP5 | docserver.spec: events logged after a save (dirty values themselves not asserted) |
| `onError` | uses (`:307`) | S | `{data: {errorCode, errorDescription}}` (the spike sent `{status}`, `hr-backend-prompt.md:830`). Codes: -20 token or origin refused (401, 403), -4 document not loaded, -1 anything else | WP5 | docserver.spec: tampered token → `-20`, no editor frame, no session |
| `onWarning` | uses | S | Wired: `{warningCode, warningDescription}`, 1 = host did not accept a save (needs WP4's callback), 2 = recovery copy not stored | WP5 | — |
| `onSaved`, `onClosed` (ours, non-standard) | — | S | `onSaved {version, sha256, callback}` after the server stored a save (`callback`: `unknown` until WP4); `onClosed {status}` after the session closed | WP5 | docserver.spec |
| `onReady` (old name) | — | N | Not emitted | — | — |
| `onRequestClose` | — | P | `api.js` relays it; **nothing in the editors triggers it yet** (no close control in the editor chrome), so the host has no way to see it today | WP5 | — |
| `onRequestHistory`, `onRequestHistoryData`, `onRequestHistoryClose`, `onRequestRestore` | — (`:268`) | N | Version history is ours (PLAN §6), not driven by the host | — | — |
| `onRequestRename`, `onRequestSaveAs`, `onRequestInsertImage`, `onRequestOpen`, `onRequestCreateNew`, `onRequestEditRights`, `onRequestSharingSettings`, `onRequestUsers`, `onRequestSendNotify`, `onRequestReferenceData`, `onRequestSelectDocument`, `onRequestSelectSpreadsheet`, `onMakeActionLink`, `onMetaChange`, `onDownloadAs`, `onInfo`, `onOutdatedVersion`, `onCollaborativeChanges`, `onPluginsReady` | — | N | Not emitted | — | — |

## 6. Methods (on the `DocEditor` object)

| Method | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `destroyEditor()` | uses (`:308`) | S | Closes the session first (status 2 or 4; a second close answers the first), waits for `onClosed` for at most 3 s with the frame hidden, then puts the original placeholder element back and removes the listeners | WP5 | docserver.spec |
| `requestSave()` (ours, non-standard) | — | S | Asks the editor to save now; the server stores the version (status 6 callback is WP4) | WP5 | docserver.spec: `onSaved`, version 2 on disk |
| `close()` (ours, non-standard) | — | S | Closes the session (status 2 or 4); the host gets `onClosed {status}` (non-standard event) | WP5 | docserver.spec: status 2 after a save |
| `downloadAs`, `insertImage`, `refreshHistory`, `setHistoryData`, `requestClose`, `setActionLink`, `setFavorite`, `setReferenceData`, `setRequestedDocument`, `setRequestedSpreadsheet`, `setRevisedFile`, `setSharingSettings`, `setUsers`, `showMessage`, `denyEditingRights`, `startFilling` | — | N | Present and throws "not supported" | WP5 | docserver.spec: `downloadAs` |

## 7. Save callback (our server → the host)

| Item | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `status` 1 | ignored (`:284-285`) | S | Once, on the first dirty event | WP4 | — |
| `status` 2 | saved | S | On close after a save; `url` = the last version; retried on failure | WP4 | — |
| `status` 3, 7 | ignored | N | Never sent (a failure is answered to the editor, PLAN §3.3) | — | — |
| `status` 4 | ignored | S | On close with nothing saved | WP4 | — |
| `status` 6 | saved | S | Every save; `forcesavetype: 1` | WP4 | — |
| `key`, `url`, `filetype`, `users` | uses (`:276`) | S | As above; `url` is our file route with a 10-min file token | WP4 | — |
| `actions`, `changesurl`, `history`, `lastsave`, `notmodified`, `userdata` | `actions` read, extra fields allowed (`:276-277`) | P | `actions` = `[{type, userid}]` for open/close; the others not sent | WP4 | — |
| body `token` | uses (the only place HRB reads it, `:279`) | S | `JWT(body)` with the provider secret | WP4 | — |
| `Authorization: Bearer` | not read today; PR-1 adds it | S | `JWT({payload: body})` | WP4 | — |
| Reply `{"error":0}` | answered | S | Required; anything else → the save is reported failed and the document stays dirty | WP4 | — |
| Timeout | HRB downloads in the handler (30 s) | S | 60 s default, configurable | WP4 | — |

## 8. Security and other services

| Item | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| JWT secret per direction (inbox / outbox / browser) | one secret | P | One secret per provider for config and callback; our own derived keys for session and file tokens | WP2, WP3 | — |
| JWT header name configurable | — | N | `Authorization` only | — | — |
| `exp` in the config token | not sent | S | Required (PLAN Q4) | WP3 | — |
| Document server fetch over TLS with verification | — | S | rustls; verification on | WP4 | — |
| Command service (`drop`, `forcesave`, `info`, `meta`, `version`, …) | — | N | Not provided; `requestSave()`/`close()` from the page instead | — | — |
| Conversion API | — | N | Our converter has its own API (`docs/phase3/api.md`) | — | — |
| Document Builder | — | N | — | — | — |
| WOPI | — | N | Rejected for now (`hrm-integration-design.md:313-314`) | — | — |
| Script path `/web-apps/apps/api/documents/api.js` | uses (`:305`) | S | Alias of `/ds/api.js` (same bytes, `Cross-Origin-Resource-Policy: cross-origin`, cached 5 min on 200 only) | WP5 | `services/suite-serve/tests/docserver.rs`; docserver.spec loads the host page through it |
