# Slides command hook (WP4): the seam, the guarded ids and every bypass

Patch `deckcraft/0141-deckcraft-office-command-hook.patch`. Read with `hooks-docs.md` (Docs) for the same table in the other app.

## The seam

`deckcraft_engine::office_hook` (a new engine file) holds one process-wide `CommandHook`:

| | |
|---|---|
| `before(id, &Session) -> Option<String>` | a reason to refuse `id`, or `None`. `Session::execute` asks it first and returns `EngineError::Disabled(id, reason)` when it refuses. The UI asks it through `office_hook::refusal` to grey a control out (`ribbon::enabled`). |
| `after(id, &Session, &params, &Result<Value>)` | the outcome, for a command that ran (a failed one too). This is where the snapshot after `file.save` hangs. |
| `set_hook(Option<Arc<dyn CommandHook>>)` | attach or remove. Nothing is attached by default: `execute` then behaves as upstream's. |

Only the **outermost** command of an action is offered (`depth == 0`): a command that runs other commands is one action, so a policy
sees `file.export`, not the steps inside it. Upstream hunks: `Session::execute` (+9 lines), the `pub mod` line, `ribbon::enabled` (one
condition) and one guard in each of `export_as`, `print_pdf` and `save_as_dialog` (4 lines each).

Tests: `crates/engine/tests/office_hook.rs` (`hook_refuses_print`, `hook_sees_save`, `without_a_hook_nothing_changes`: the bytes of
`file.saveBytes` are identical with no hook and with an allow-all hook) and `office_toolbar_tests::hook_refuses_print` (the UI's print path).

## Guarded ids (checked against the registry)

| id | registry at BASE `fe510bd` | hook sees it |
|---|---|---|
| `file.save` | `cmd/file.rs` | after-hook: the snapshot (WP10) |
| `file.saveAs` | `cmd/file.rs` | before, after |
| `file.saveTemplate` | `cmd/file.rs` | before, after |
| `file.export` | `cmd/file.rs` (PDF, PNG, PPTX, outline, GIF) | before, after |
| `edit.copy`, `edit.cut` | `cmd/edit.rs` | before, after |
| `file.print` | **not a registry id**: a UI-level name the Print page asks about (`print_pdf`), because print is a PDF written through `file.export` and a policy may allow export but not print | before |

## Paths and bypasses (WP4 step 1)

Every user path to a guarded action, and what happens on it.

| Path | Goes through `Session::execute`? | Handling |
|---|---|---|
| Ctrl+S, QAT Save, File › Save, with a file path (desktop) | yes: `app.save()` runs `file.save` | hooked |
| Save of an unsaved presentation, Save As, File › Save a Copy (desktop) | yes: `save_as_dialog` runs `file.saveAs` | hooked |
| Export to PDF, PNG, JPEG, PPTX, outline, GIF (desktop) | yes: `export_as` runs `file.export` | hooked |
| Print (desktop) | yes: `print_pdf` runs `file.export` into the temp folder | hooked, and before-hook `file.print` |
| Ctrl+C, Ctrl+X, context menus, thumbnail menus | yes: `publish_clipboard` runs `edit.copy` / `edit.cut`; the system clipboard is set only from the command's result | hooked |
| Control channel (desktop TCP) | yes: `app.run` | hooked |
| **Web: Save, Save As, export, print** | **no**: the web branch calls `save_bytes` / `pdf_bytes` and `download` directly | a one-line guard at the top of `save_as_dialog`, `export_as`, `print_pdf` asks `refusal("file.saveAs" / "file.export" / "file.print")`. **No after-hook on the web** (a stated limit: the snapshot is a desktop feature). In the web shell the bridge (`deckcraft:save`) is the save path, and the shell, not the editor, stores it. |
| Text copied out of a text field (notes pane, dialogs, the command palette) | no: egui's own `TextEdit` copies it | **limit**. Slide content is covered; notes text is not. |
| `backstage.rs:831` copies a recent file's path | no | not content; left alone |
| Print Screen, screenshots, a photo of the screen, a second window | no | **limit** (R11): the OS, not the editor |
| Drag a shape or slide out to another app | not offered by egui | none |
| Autosave / crash recovery (`file.recovery.save`) | yes, but not a user export | not guarded |

The bridge's own Save (`deckcraft:requestSave`, Ctrl+S in the web shell) does not run `file.save`: it writes `.pptx` bytes through
`save_bytes` and posts them to the shell. It is a web path in the table above.

## What 0143 does with the seam (WP10 and WP11, Slides)

XDFU in the editors is **watermark only** (owner, 2026-10-22): nothing is refused. The hook trait gained `wrap(id, session, params, run)`
(default: run the command; offered for the outermost command only, like `before` and `after`), because a stamp has to be in the
presentation while the command runs and be taken out afterwards, and the engine gained `register_command` (a command found by id, not
listed). `crates/ui-egui/src/office_history.rs` (desktop only, `office_history::init()` from `main`, only with the brand overlay) installs
the hook:

- **Snapshot.** `after` of `file.save` / `file.saveAs`, when the command saved the document's own path: `Store::snapshot`. Errors go to
  stderr. Slides has no timed autosave of the file (AutoRecover writes its own folder through `file.recovery.save`, which is not hooked),
  so every save is an explicit one and always snapshots (the store skips bytes it already has); there is no throttle. The Version
  History button calls `open_panel(path)` through `office_toolbar::set_history_handler`; an unsaved presentation gets the toast
  "Save the file first, then open its version history.".
- **Watermark** (only with `xdfu = on` and a grif: XDFU, ХДФУ or ДСП in the title of slide 1 or in a text box on slide 1 whose top edge is
  in the top quarter of the slide; a grif lower down or on another slide is not recognised). Stamped, with the copy number from
  `Issuer::issue` (the audit line exists only if the copy was made): `file.export` in every format (pdf, png, jpeg, gif, pptx, deckcraft,
  outline), the Print page's PDF (`file.export` with `print: true`, audit event `print`), `file.saveAs` and `file.save` to a path other than
  the document's own, `file.saveTemplate` and `file.saveBytes`. The stamp is a red 8 pt text box along the bottom of every slide, present
  only while the command runs (the working presentation, its saved state and dirty flag are put back; pdf, png, jpeg and gif use the ASCII
  form of the line); an outline gets the line as its first line. A plain `file.save` of the working file is not a copy and is not stamped.
- `office.xdfu.insertGrif {grif?}` (registered only when XDFU is on) puts a red text box named "XDFU grif" at the top right of slide 1
  with the grif and `Nusxa N` (the audit counter); a second call replaces it. It is undoable.
- **Limits that remain:** the web build installs no hook (desktop only), so the web branches of `save_as_dialog`, `export_as`, `print_pdf`
  (`save_bytes` / `pdf_bytes` + download, the table above) and the shell bridge's Save are not stamped; `edit.copy` / `edit.cut` are not
  blocked and carry no mark; the headless CLI and MCP never install the layer; a copy whose audit log cannot be opened is still made, with
  copy number 0 and a message on stderr; after a stamped Save As the document is bound to the stamped file, and its next plain save
  rewrites that file without the stamp (the in-memory working copy is never stamped).
- Tests (own binaries, the hook is process-wide): `office_history.rs`, `office_xdfu.rs`, `office_xdfu_off.rs` in `crates/ui-egui/tests`.
