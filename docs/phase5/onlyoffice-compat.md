# ONLYOFFICE DocsAPI config compatibility (Phase 5, brief task 3)

**What this is.** For each field of an ONLYOFFICE Docs API editor config, callback and event, what our document server
does: **Supported**, **Partial** (works with a stated limit), or **Not supported** (ignored or refused, as stated).
"HRB" says whether hr-backend sends or needs it today (`docs/phase0/hrm-current-state.md` §4, owner confirmed unchanged
2026-10-25). Plan: `docs/phase5/PLAN.md`.

**Status of this file: skeleton (2026-10-25).** Nothing is built yet. "Target" is the Phase 5 plan's intent; "Verified"
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
| `documentType` (`word`, `cell`, `slide`, `pdf`) | sent (`hrm-current-state.md:252`) | P | Selects the editor (`/docs/`, `/sheets/`, `/slides/`, `/pdf/`). Only `word`/docx is in the gate; the others open but lack read-only and save acknowledgement until their bridges get them | WP5, WP6 | — |
| `type` (`desktop`, `mobile`, `embedded`) | sent as `mobile` on `device=mobile` (`:264`) | P | `mobile` accepted, layout from the viewport; editing is **not** forced to view; `embedded` treated as view | WP5 | — |
| `width`, `height` | — (client-side) | S | Applied to the iframe | WP5 | — |
| `token` | sent: HS256 over the whole config, no `exp` (`:265`) | S | HS256 only, `alg` pinned; the verified payload is the only source of truth; `exp` required (PLAN Q4) | WP3 | — |
| `events` | uses | S | Kept on the page, never sent to the server, never signed (`:312`) | WP5 | — |

## 2. `document`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `fileType` | sent (`:253`) | P | Allowlist docx (gate), xlsx, pptx, pdf; must match the OOXML main part of the fetched bytes; `doc`, `rtf`, `odt`, `txt` … refused | WP3 | — |
| `key` | sent: `{kind}_{id}_v{ts}_{8hex}`, new per open (`:254`) | S | Echoed unchanged in callbacks; validated (charset `[0-9A-Za-z._=-]`, ≤ 128 chars); a hash of it goes to the audit log | WP3 | — |
| `title` | sent (`:255`) | S | The file name shown and sent to the editor; sanitised | WP3 | — |
[redacted: HRM security detail]
| `info.*` (`owner`, `uploaded`, `favorite`, `folder`, `sharingSettings`) | — (`:267`) | N | Ignored | — | — |
| `referenceData` | — (`:267`) | N | Ignored | — | — |
| `permissions.edit` | sent | S | `false` → view session (server refuses save) | WP3, WP7 | — |
| `permissions.download` | sent `true` | P | `false` → the editor's Save a copy / export refused through the bridge flag; best effort on the web | WP7 | — |
| `permissions.print` | sent `true` | P | `false` → print refused through the bridge flag; best effort | WP7 | — |
| `permissions.review` | sent `false` | P | `true` with `edit:false` → Docs opens with tracked changes forced on (after the gate) | WP7 | — |
| `permissions.comment` | sent `= edit` | P | With `edit:true`: part of editing. Comment-only (`edit:false`) is **not supported**: opened as view | WP7 | — |
| `permissions.copy`, `chat`, `fillForms`, `modifyContentControl`, `modifyFilter`, `protect`, `commentGroups`, `reviewGroups`, `userInfoGroups`, `editCommentAuthorOnly`, `deleteCommentAuthorOnly` | — | N | Ignored | — | — |

## 3. `editorConfig`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `mode` (`edit`, `view`) | sent (`:258`) | S | `view` → read-only session | WP3, WP7 | — |
| `callbackUrl` | sent for edit configs, omitted for view (`:259`) | S | Through the per-provider callback allowlist; a config without it is accepted as view only | WP4, WP7 | — |
| `user.id` | sent `emp-{id}` / `user-{id}` (`:260`) | S | The session `sub`; echoed in callback `users[]`; the audit actor | WP3 | — |
| `user.name` | sent | P | Shown as the author of tracked changes and comments; no presence list | WP5 | — |
| `user.group`, `user.image` | — | N | Ignored | — | — |
| `lang` | sent `ru` (`:261`) | P | Sets the editor language when the user has no own choice | WP5 | — |
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
| `uiTheme` | sent (`:263`) | P | `theme-dark` → dark, anything else → light | WP5 | — |
| `autosave` | — | P | Autosave is a recovery copy on our server only; the HRM sees saves (6) and the close (2) | WP4, WP6 | — |
| `anonymous`, `comments`, `compactHeader`, `compactToolbar`, `compatibleFeatures`, `customer`, `features`, `feedback`, `goback`, `help`, `hideNotes`, `hideRightMenu`, `hideRulers`, `integrationMode`, `logo`, `macros`, `macrosMode`, `mentionShare`, `mobile`, `review`, `submitForm`, `toolbarHideFileName`, `toolbarNoTabs`, `unit`, `zoom` | — | N | Ignored; the product chrome and toolbar come from our own settings | — | — |

## 5. Events (from the editor to the host page)

| Event | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `onAppReady` | uses (`:307`) | S | When the editor bridge is ready | WP5 | — |
| `onDocumentReady` | uses | S | When the document is open | WP5 | — |
| `onDocumentStateChange` | uses (stub) | S | `{data: true|false}` on dirty changes | WP5 | — |
| `onError` | uses (`:307`) | S | `{data: {errorCode, errorDescription}}` (the spike sent `{status}`, `hr-backend-prompt.md:830`) | WP5 | — |
| `onWarning` | uses | S | Callback failures and size refusals | WP5 | — |
| `onReady` (old name) | — | N | Not emitted | — | — |
| `onRequestClose` | — | P | Emitted when the user closes; the host then calls `destroyEditor()` | WP5 | — |
| `onRequestHistory`, `onRequestHistoryData`, `onRequestHistoryClose`, `onRequestRestore` | — (`:268`) | N | Version history is ours (PLAN §6), not driven by the host | — | — |
| `onRequestRename`, `onRequestSaveAs`, `onRequestInsertImage`, `onRequestOpen`, `onRequestCreateNew`, `onRequestEditRights`, `onRequestSharingSettings`, `onRequestUsers`, `onRequestSendNotify`, `onRequestReferenceData`, `onRequestSelectDocument`, `onRequestSelectSpreadsheet`, `onMakeActionLink`, `onMetaChange`, `onDownloadAs`, `onInfo`, `onOutdatedVersion`, `onCollaborativeChanges`, `onPluginsReady` | — | N | Not emitted | — | — |

## 6. Methods (on the `DocEditor` object)

| Method | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `destroyEditor()` | uses (`:308`) | S | Removes the iframe and listeners; an open session is closed (status 2 or 4) | WP5 | — |
| `requestSave()` (ours, non-standard) | — | S | Asks the editor to save now (status 6) | WP5 | — |
| `close()` (ours, non-standard) | — | S | Closes the session (status 2 or 4) | WP5 | — |
| `downloadAs`, `insertImage`, `refreshHistory`, `setHistoryData`, `requestClose`, `setActionLink`, `setFavorite`, `setReferenceData`, `setRequestedDocument`, `setRequestedSpreadsheet`, `setRevisedFile`, `setSharingSettings`, `setUsers`, `showMessage`, `denyEditingRights`, `startFilling` | — | N | Not present (calling one throws "not supported") | — | — |

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
| Script path `/web-apps/apps/api/documents/api.js` | uses (`:305`) | S | Alias of `/ds/api.js` | WP5 | — |
