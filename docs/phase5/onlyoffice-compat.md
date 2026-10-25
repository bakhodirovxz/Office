# ONLYOFFICE DocsAPI config compatibility (Phase 5, brief task 3)

**What this is.** For each field of an ONLYOFFICE Docs API editor config, callback and event, what our document server
does: **Supported**, **Partial** (works with a stated limit), or **Not supported** (ignored or refused, as stated).
"HRB" says whether hr-backend sends or needs it today (`docs/phase0/hrm-current-state.md` §4, owner confirmed unchanged
2026-10-25). Plan: `docs/phase5/PLAN.md`.

**Status of this file (2026-10-25).** "Verified" names a test that exists and passed in the WP16 run (`scripts/ci.sh docserver`):
Rust tests are `file::test_name` under `services/docserver/tests/` (or `services/<crate>/...`); browser tests are titles in
`ci/web-smoke/docserver.spec.ts` (Chromium, two sites, TLS; written **spec: "title"**) and `bridge-docs.spec.ts`.
"—" means no test asserts the row: the behaviour is implemented (or only planned) but nothing proves it. "Target" is the Phase 5
plan's intent; a row's target changes only with the plan.

**Verified 2026-10-25 against commit 7f10ad8** (every test name below was grepped in that tree; pass/fail = the WP16 job run, see
`docs/STATUS.md`). The field names are **not** yet re-checked against the public DocsAPI pages (WP16 had no network step for it):
that stays open for the gate review.

**Clean-room.** Field names come from ONLYOFFICE's **public** Docs API documentation (the editor config, events,
methods and callback reference pages) and from HRB's own use of them. No ONLYOFFICE source code (AGPL) was read, copied
or vendored, and our `api.js` is written from scratch. WP16 re-checks every name below against the public pages before
the gate; a name not found there is removed.

Legend: **S** supported, **P** partial, **N** not supported. HRB: **sent** (HRB emits it), **uses** (HRB listens or
relies on it), **—** (not used).

## 1. Top level

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `documentType` (`word`, `cell`, `slide`, `pdf`) | sent (`hrm-current-state.md:252`) | P | Selects the editor (`/docs/`, `/sheets/`, `/slides/`, `/pdf/`). Only `word`/docx is in the gate; the others open but lack read-only and save acknowledgement until their bridges get them. The wrapper picks the frame from the session's `editor` and the shell's prefix table | WP5, WP6 | docx only: spec: "cross-site ..." (xlsx, pptx, pdf never opened); a `documentType` that does not match the file type is refused: auth.rs::config_field_refusals_and_modes |
| `type` (`desktop`, `mobile`, `embedded`) | sent as `mobile` on `device=mobile` (`:264`) | P | `mobile` accepted and **ignored** (`api.js` makes the same frame; layout comes from the viewport); editing is **not** forced to view; `embedded` treated as view (not yet) | WP5 | — (no test) |
| `width`, `height` | — (client-side) | S | Applied to the iframe by `api.js` (number = px, a CSS length, default 100%) | WP5 | — (implemented, no test) |
| `token` | sent: HS256 over the whole config, no `exp` (`:265`) | S | HS256 only, `alg` pinned; the verified payload is the only source of truth; `exp` required by default (PLAN Q4; see the `exp` row in section 8) | WP3 | auth.rs::config_token_refusals (bad signature, issuer, `alg` none/HS512/RS256, missing token, a session token as config token); auth.rs::outer_fields_are_ignored (unsigned fields never win); fake-hrm contract.rs::jwt_pins_hs256 |
| `events` | uses | S | Kept on the page, never sent to the server, never signed (`:312`); `api.js` strips it from the config it posts | WP5 | spec: "cross-site ..." (the POST to `/ds/session` has no `events` key); auth.rs::outer_fields_are_ignored |

## 2. `document`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `fileType` | sent (`:253`) | P | Allowlist docx (gate), xlsx, pptx, pdf; must match the OOXML main part of the fetched bytes; `doc`, `rtf`, `odt`, `txt` … refused | WP3 | auth.rs::config_field_refusals_and_modes (`exe`, type mismatch); ooxml.rs::a_main_part_of_the_right_type_and_nothing_else; outbound.rs::size_cap_zip_bombs_and_wrong_packages_are_refused_on_fetch (the other refused types are not each tried) |
| `key` | sent: `{kind}_{id}_v{ts}_{8hex}`, new per open (`:254`) | S | Echoed unchanged in callbacks; validated (charset `[0-9A-Za-z._=-]`, ≤ 128 chars); a hash of it goes to the audit log (`key_ref`, `api.rs:178`) | WP3 | auth.rs::config_field_refusals_and_modes (space, > 128, missing); outbound.rs::the_audit_chain_covers_a_full_flow_verifies_and_holds_no_secret; echo in callbacks: the HRB harness checks the key on every callback in outbound.rs::open_save_close_reopen_against_the_hrb_harness |
| `title` | sent (`:255`) | S | The file name shown and sent to the editor; validated as plain text (1 to 255 characters, no control characters), not otherwise rewritten | WP3 | spec: "cross-site ..." (`rec.title`); wp14.rs::encrypted_files_round_trip_hold_no_plaintext_and_survive_a_restart (survives a restart); the validation limits themselves: — |
[redacted: HRM security detail]
| `info.*` (`owner`, `uploaded`, `favorite`, `folder`, `sharingSettings`) | — (`:267`) | N | Ignored | — | — |
| `referenceData` | — (`:267`) | N | Ignored | — | — |
| `permissions.edit` | sent | S | `false` → view session: the server refuses save, autosave and dirty (403) and sends no callback (not even status 4 on close); the wrapper opens the editor with `readOnly: true`, so typing, Save and exports are refused | WP3, WP7 | auth.rs::config_field_refusals_and_modes; outbound.rs::a_view_session_reads_and_sends_no_callback; spec: "WP7 view session ..."; bridge-docs.spec: "2 readOnly ..." |
| `permissions.download` | sent `true` | P | Server: the version-history download route needs it. Editor: **no bridge flag exists** (0127 has only `readOnly`), so `false` is not enforced for an edit session. A view session refuses exports through `readOnly` whatever `download` says (stricter than asked; a view session with `download:true` cannot export) | WP7 | — (the history download route checks the session token and permissions: history.rs::history_routes_check_the_session_token_and_permissions; `download:false` itself is not asserted) |
| `permissions.print` | sent `true` | P | **Not enforced**: the editors have no bridge flag for print. Accepted and stored on the session | WP7 | — (no test) |
| `permissions.review` | sent `false` | P | Not supported yet: `review:true` with `edit:false` opens as a plain view session (read-only). Forced tracked changes need a wasm-safe policy layer (PLAN F9), after the gate | WP7 | — (no test for `review:true`; `edit:false` alone: auth.rs::config_field_refusals_and_modes) |
| `permissions.comment` | sent `= edit` | P | With `edit:true`: part of editing. Comment-only (`edit:false`) is **not supported**: opened as view (read-only) | WP7 | auth.rs::config_field_refusals_and_modes (`edit:false` is a view session) |
| `permissions.copy`, `chat`, `fillForms`, `modifyContentControl`, `modifyFilter`, `protect`, `commentGroups`, `reviewGroups`, `userInfoGroups`, `editCommentAuthorOnly`, `deleteCommentAuthorOnly` | — | N | Ignored | — | — |

## 3. `editorConfig`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `mode` (`edit`, `view`) | sent (`:258`) | S | `view` → read-only session | WP3, WP7 | auth.rs::config_field_refusals_and_modes ("mode view"; a bad mode is refused); spec: "WP7 view session ..." |
| `callbackUrl` | sent for edit configs, omitted for view (`:259`) | S | Through the per-provider callback allowlist; a config without it is accepted as view only (`mode` and `permissions.edit` are then reported as view and the editor opens read-only) | WP4, WP7 | outbound.rs::a_view_session_reads_and_sends_no_callback (the fake backend's `mode=view` config); auth.rs::config_field_refusals_and_modes ("no callback", a callback off the list, a URL on the fetch list only); spec: "WP7 view session ..." |
| `user.id` | sent `emp-{id}` / `user-{id}` (`:260`) | S | The session `sub`; echoed in callback `users[]`; the audit actor | WP3 | session `sub`: auth.rs::every_route_checks_its_token; `users[]` in the callback and the audit actor: — (the harness receives `users` but no test asserts it) |
| `user.name` | sent | P | Reaches the session; **not yet** passed on to the editor (no author name in tracked changes until the bridge has a message for it); no presence list | WP5, WP6 | — (no test) |
| `user.group`, `user.image` | — | N | Ignored | — | — |
| `lang` | sent `ru` (`:261`) | P | Used for the wrapper's own messages (ru, uz-Latn, uz-Cyrl, en) through the shell catalog; **not yet** passed to the editor (no bridge message) | WP5 | — (not asserted anywhere; the shell catalog's ru/uz keys are complete: suite-serve serve.rs::shell_catalog_is_complete) |
| `region`, `location` | — | N | Ignored (number formats follow the language) | — | — |
| `coEditing.mode`, `coEditing.change` | — (`:267`) | N | One editor per session; no co-editing (Phase 9) | — | — |
| `recent`, `templates`, `createUrl` | — (`:268`) | N | Ignored | — | — |
| `actionLink` | — | N | Ignored | — | — |
| `plugins` | — (the `uz-translit` plugin is mounted server-side in ONLYOFFICE, `hr-backend-prompt.md:833`) | N | Not supported; Latin ⇄ Cyrillic is built into our editor (PLAN F13) | WP9 | — (ignored field, no test) |
| `embedded.*` | — | N | Ignored | — | — |

## 4. `editorConfig.customization`

| Field | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `forcesave` | sent `true` (`:262`) | P | Ignored as a switch: every explicit save and host `requestSave` sends status 6 | WP4 | spec: "cross-site ..." (`requestSave()` gives callback status 6); outbound.rs::open_save_close_reopen_against_the_hrb_harness |
| `uiTheme` | sent (`:263`) | P | `theme-dark` → dark, anything else → light. **Not wired in WP5**: the wrapper does not pass it on (no bridge message) | WP5 | — (no test) |
| `autosave` | — | P | Autosave is a recovery copy on our server only; the HRM sees saves (6) and the close (2) | WP4, WP6 | bridge-docs.spec: "5 autosave ..." (bytes are handed over after the period only while dirty); outbound.rs::save_bodies_are_capped_and_checked (the autosave route); the HRM seeing no callback for an autosave: — |
| `anonymous`, `comments`, `compactHeader`, `compactToolbar`, `compatibleFeatures`, `customer`, `features`, `feedback`, `goback`, `help`, `hideNotes`, `hideRightMenu`, `hideRulers`, `integrationMode`, `logo`, `macros`, `macrosMode`, `mentionShare`, `mobile`, `review`, `submitForm`, `toolbarHideFileName`, `toolbarNoTabs`, `unit`, `zoom` | — | N | Ignored; the product chrome and toolbar come from our own settings | — | — |

## 5. Events (from the editor to the host page)

| Event | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `onAppReady` | uses (`:307`) | S | When the editor bridge is ready (`<prefix>:ready`) | WP5 | — (the test host page registers no `onAppReady` handler; the document opening implies the bridge, but nothing asserts the event) |
| `onDocumentReady` | uses | S | When the editor reports the document open (`<prefix>:opened`) | WP5 | spec: "cross-site ..." (host banner `ready`) |
| `onDocumentStateChange` | uses (stub) | S | `{data: true|false}` on every `<prefix>:dirty` of the editor | WP5 | spec: "cross-site ..." (`__dirty` becomes `true` after typing); "WP7 view session ..." (never `true` while read-only) |
| `onError` | uses (`:307`) | S | `{data: {errorCode, errorDescription}}` (the spike sent `{status}`, `hr-backend-prompt.md:830`). Codes: -20 token or origin refused (401, 403), -4 document not loaded, -1 anything else | WP5 | spec: "a tampered config token ends in onError -20 and no editor" (`-20`, no editor frame, no session); spec: "S14 ..." (a forged `onError` is not delivered). Codes -4 and -1: — |
| `onWarning` | uses | S | Wired: `{warningCode, warningDescription}`, 1 = host did not accept a save (needs WP4's callback), 2 = recovery copy not stored | WP5 | spec: "S23 ..." (`warningCode: 1`, the document stays dirty, nothing shown as saved); code 2: — |
| `onSaved`, `onClosed` (ours, non-standard) | — | S | `onSaved {version, sha256, callback}` after the server stored a save (`callback`: `unknown` until WP4); `onClosed {status}` after the session closed | WP5 | spec: "cross-site ..." (banners `saved`, `closed status 2`) |
| `onReady` (old name) | — | N | Not emitted | — | — |
| `onRequestClose` | — | P | `api.js` relays it; **nothing in the editors triggers it yet** (no close control in the editor chrome), so the host has no way to see it today | WP5 | — (no test) |
| `onRequestHistory`, `onRequestHistoryData`, `onRequestHistoryClose`, `onRequestRestore` | — (`:268`) | N | Version history is ours (PLAN §6), not driven by the host | — | — |
| `onRequestRename`, `onRequestSaveAs`, `onRequestInsertImage`, `onRequestOpen`, `onRequestCreateNew`, `onRequestEditRights`, `onRequestSharingSettings`, `onRequestUsers`, `onRequestSendNotify`, `onRequestReferenceData`, `onRequestSelectDocument`, `onRequestSelectSpreadsheet`, `onMakeActionLink`, `onMetaChange`, `onDownloadAs`, `onInfo`, `onOutdatedVersion`, `onCollaborativeChanges`, `onPluginsReady` | — | N | Not emitted | — | — |

## 6. Methods (on the `DocEditor` object)

| Method | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `destroyEditor()` | uses (`:308`) | S | Closes the session first (status 2 or 4; a second close answers the first), waits for `onClosed` for at most 3 s with the frame hidden, then puts the original placeholder element back and removes the listeners | WP5 | spec: "cross-site ..." (placeholder back, one frame left; the 3 s wait for `onClosed` and a second close answering the first: —) |
| `requestSave()` (ours, non-standard) | — | S | Asks the editor to save now; the server stores the version (status 6 callback is WP4) | WP5 | spec: "cross-site ..." (`onSaved`, version 2 on disk) |
| `close()` (ours, non-standard) | — | S | Closes the session (status 2 or 4); the host gets `onClosed {status}` (non-standard event) | WP5 | spec: "cross-site ..." (status 2 after a save) |
| `downloadAs`, `insertImage`, `refreshHistory`, `setHistoryData`, `requestClose`, `setActionLink`, `setFavorite`, `setReferenceData`, `setRequestedDocument`, `setRequestedSpreadsheet`, `setRevisedFile`, `setSharingSettings`, `setUsers`, `showMessage`, `denyEditingRights`, `startFilling` | — | N | Present and throws "not supported" | WP5 | spec: "cross-site ..." (`downloadAs` only; the other methods: —) |

## 7. Save callback (our server → the host)

| Item | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| `status` 1 | ignored (`:284-285`) | S | Once, on the first dirty event | WP4 | outbound.rs::the_audit_chain_covers_a_full_flow_verifies_and_holds_no_secret (the callbacks are exactly 1, 6, 2); spec: "cross-site ..." (`callback status 1 ... ok`). The HRB harness ignores 1, as HRB does |
| `status` 2 | saved | S | On close after a save; `url` = the last version; retried on failure | WP4 | outbound.rs::open_save_close_reopen_against_the_hrb_harness; ::a_failed_callback_keeps_the_document_dirty_and_the_retry_survives_a_restart; ::the_backoff_is_1_5_15_60_minutes_then_hourly_and_stops_after_a_day; auth.rs::sessions_survive_a_restart; spec: "cross-site ..." |
| `status` 3, 7 | ignored | N | Never sent (a failure is answered to the editor, PLAN §3.3) | — | — |
| `status` 4 | ignored | S | On close with nothing saved | WP4 | the status value: auth.rs::sessions_survive_a_restart; a view session sends **no** callback: outbound.rs::a_view_session_reads_and_sends_no_callback. The status-4 callback of an edit session to the provider: — |
| `status` 6 | saved | S | Every save; `forcesavetype: 1` | WP4 | status 6: outbound.rs::open_save_close_reopen_against_the_hrb_harness; spec: "cross-site ...". `forcesavetype`: — (set in `callback.rs:115`, asserted nowhere) |
| `key`, `url`, `users` | uses (`:276`) | S | `key` echoed, `users` = `[user.id]`, `url` is our internal file route with a 10-min file token. **`filetype` is not sent** (this row used to list it; `callback.rs:104-119` has no such field) | WP4 | `key` and `url` (the harness downloads it): outbound.rs::open_save_close_reopen_against_the_hrb_harness; file token life: auth.rs::file_token_bindings; `users`: — |
| `actions`, `changesurl`, `history`, `lastsave`, `notmodified`, `userdata` | `actions` read, extra fields allowed (`:276-277`) | N | **None is sent** (this row used to say `actions` was; `callback.rs:104-119` has none). The fake backend's own contract test sends `actions: []` (fake-hrm contract.rs). Whether hr-backend requires the field is **open** (HRB PR-1 / WP17) | WP4 | — |
| body `token` | uses (the only place HRB reads it, `:279`) | S | `JWT(body)` with the provider secret | WP4 | outbound.rs::open_save_close_reopen_against_the_hrb_harness (the harness verifies it and acts only then) |
| `Authorization: Bearer` | not read today; PR-1 adds it | S | `JWT({payload: body})` | WP4 | outbound.rs::the_callback_carries_both_tokens_and_the_file_url_of_our_internal_listener (header present and JWT-shaped; the payload shape and the harness's Bearer mode: —) |
| Reply `{"error":0}` | answered | S | Required; anything else → the save is reported failed and the document stays dirty | WP4 | outbound.rs::a_callback_is_ok_only_for_error_0_and_a_silent_provider_is_unknown, ::a_failed_callback_keeps_the_document_dirty_and_the_retry_survives_a_restart; spec: "S23 ..." |
| Timeout | HRB downloads in the handler (30 s) | S | 60 s default, configurable (1 to 600 s) | WP4 | outbound.rs::a_callback_is_ok_only_for_error_0_and_a_silent_provider_is_unknown (a silent provider is `unknown`); config.rs::limits_and_unknown_keys_are_checked; the 60 s default: — |

## 8. Security and other services

| Item | HRB | Target | Our behaviour | WP | Verified |
|---|---|---|---|---|---|
| JWT secret per direction (inbox / outbox / browser) | one secret | P | One secret per provider for config and callback; our own derived keys for session and file tokens | WP2, WP3 | config.rs::secrets_must_differ; auth.rs::every_route_checks_its_token, ::file_token_bindings; suite-serve lib.rs::the_docserver_secrets_differ_from_the_converters |
| JWT header name configurable | — | N | `Authorization` only | — | — |
| `exp` in the config token | not sent | S | Required by default (PLAN Q4); a provider with `require_exp = false` (needed for HRB as surveyed, F1) accepts a token without it, its session then ends after 12 h, and the server logs a warning | WP3 | auth.rs::config_token_refusals (missing, expired, > 12 h, not a number); ::an_hrb_token_without_exp_is_refused_unless_the_provider_allows_it; ::session_token_never_outlives_the_config |
| Document server fetch over TLS with verification | — | S | rustls; verification on | WP4 | browser side only: spec: "cross-site ..." and the compose rehearsal (pinned throwaway CA). The server's own outbound https verification: — (every Rust test uses http) |
| Command service (`drop`, `forcesave`, `info`, `meta`, `version`, …) | — | N | Not provided; `requestSave()`/`close()` from the page instead | — | — |
| Conversion API | — | N | Our converter has its own API (`docs/phase3/api.md`) | — | — |
| Document Builder | — | N | — | — | — |
| WOPI | — | N | Rejected for now (`hrm-integration-design.md:313-314`) | — | — |
| Script path `/web-apps/apps/api/documents/api.js` | uses (`:305`) | S | Alias of `/ds/api.js` (same bytes, `Cross-Origin-Resource-Policy: cross-origin`, cached 5 min on 200 only) | WP5 | suite-serve docserver.rs::api_js_is_a_plain_script_load, ::embed_files_are_served_with_the_right_types; spec: "cross-site ..." (loads through the alias, headers asserted) |
