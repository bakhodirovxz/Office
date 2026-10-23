# Docs command hooks (WP4, the Docs part)

`docs-office` re-registers the guarded ids (`hooks::GUARDED`) with wrappers (F2), through the existing `office-ext` install hook:
no new upstream patch for this. `hooks::set_hook(Some(Arc<dyn CommandHook>))` attaches a policy; WP10 (snapshot after
`file.save`) and WP11 (XDFU) fill it, Phase 5 adds permissions.
- `before(id, &Session) -> Option<&'static str>` runs inside the command's `enabled`. A reason greys the control out and
  `Session::run` refuses with `Disabled("<id>: <reason>")`.
- `after(id, &Session, params, &result)` runs after the original, also for a failed result.
- With no hook set a wrapper only calls the original (tests: `without_a_hook_nothing_changes`, `hook_refuses_print`,
  `hook_sees_save` in `domains/docs/crates/docs-office/src/tests.rs`).
- Guarded: `file.print`, `file.printPdf`, `file.exportPdf`, `file.exportPng`, `file.saveAs`, `file.save`, `file.saveTemplate`,
  `edit.copy`, `edit.cut` (a test checks that every id is registered at the BASE).

## Bypasses (every path that does not go through `Session::run`), read at wordcraft 2e6db0b
| Path | Where | Handling |
|---|---|---|
| Web download branch: Save, Save As, Export PDF/PNG become downloads built with `io::save_bytes_with` | `ui-egui/src/lib.rs` `execute`, the `services.download` branch (about line 970) | **Bypasses `enabled` and `after`.** Stated limit for Phase 4 (XDFU is off by default and the web has no snapshots). WP11 must either add the one-line call there (a carried hunk, patch 0122's `office_overlay` already holds the seam) or enforce the watermark on the server side of the shell. |
| Read mode: the Copy key event copies `selected_text()` to the system clipboard | `ui-egui/src/keys.rs` (the `egui::Event::Copy` arm of the read-mode branch) | Bypass. Stated limit; same one-line fix. |
| System clipboard and screenshots outside the app | OS | Cannot be guarded by any in-app hook. Stated limit (R11). |
| Background and browser-picker saves (`async_saves.rs`) | settle a save that `execute` already started | Not a bypass: the save began in `execute` (so the wrapper ran, except in the web download branch above). |
| Autosave and recovery files (`file.autosave`) | `execute` handles it before the registry | Writes a recovery copy, not a user output; WP11 decides whether XDFU documents may be autosaved. |
| Keyboard shortcuts, menus, QAT, context menus | all call `app.run` / `execute` | Go through the registry. |

## WP10 and WP11: the layer on the seam (`domains/docs/crates/docs-office/src/layer.rs`, desktop only)

`CommandHook` gained `wrap(id, session, params, run)` (default: run the command), called between `before` and `after`. `layer::init()` (from `install`,
once per process, never in this crate's own tests) reads the shared settings and installs `Layer`; tests call `layer::configure(store, audit)`.
No upstream patch except `wordcraft/0123` (AutoSave passes `{"autosave": true}` to `file.save`, so an explicit save can be told from the timer).

- **Snapshot.** After `file.save` / `file.saveAs`, when the saved path is the document's own file: `suite_core::history::Store::snapshot`. A pdf/txt
  copy or a save that only opened the Save As dialog makes no version. An autosave makes at most one version per document in 5 minutes; an explicit
  save always does (the store skips identical bytes). Errors go to stderr, a save never fails because of them.
- **History panel.** `office.history` and upstream's `file.versions` (Backstage Version History, with no `save`/`restore` parameter) call
  `history::open_panel(path)`: the desktop host opens the shell's History panel for the file. An unsaved document gets "Save the file first, then
  open its version history." from `office.history`, and upstream's in-memory list from `file.versions`. Web builds keep the old behaviour.
- **XDFU (watermark only, owner decision 2026-10-22).** With `xdfu = on` and a grif (XDFU, ХДФУ, ДСП) in the first 25 body paragraphs or the first
  5 lines of a header, footer or text box: `file.printPdf`, `file.exportPdf`, `file.exportPng`, `file.saveAs`, `file.saveTemplate` and `file.save`
  to another path run with the cl. 12 line as the document's diagonal watermark (restored afterwards; txt, md and html copies also get it as their
  first line). Copy number = the next `seq` of the keyed audit chain (`xdfu-audit.jsonl` and `.key` in the data folder); a line is written only if
  the copy was made. Nothing is refused: an unusable audit log gives copy number 0 and a message on stderr. A plain save of the working file is not
  a copy. `office.xdfu.insertGrif {grif?}` (registered only when XDFU is on) writes the right-aligned grif paragraph with "Nusxa N" at the start of
  the body; there is no menu entry yet (command search and the control channel only).
- **Off means absent.** No `office.xdfu.*` command, no detection, no audit file, byte-identical copies: test `xdfu_off_registers_nothing`.
- **Bypasses and limits that remain.** The web download branch (`services.download`) and the web host bridge install no layer (no settings, no
  store in wasm): no snapshots and no stamp there. A save-as binds the document to the copy, so its next plain save rewrites that file without the
  stamp. `edit.copy` / `edit.cut` and the read-mode Copy key carry no mark (clipboard text has no place for one) and are not blocked. Screenshots and
  the OS clipboard are outside the editor. A grif below the 25th paragraph is not recognised. The save-as dialog's `file.save` with no path only
  asks for a path and makes no copy.
- Tests: `layer_tests.rs` (`save_snapshot_list_restore_round_trip`, `xdfu_off_registers_nothing`, `watermark_carries_copy_number_and_user_and_nothing_is_refused`,
  `insert_grif_writes_a_right_aligned_paragraph_with_the_copy_number`, `version_history_opens_the_panel_only_for_a_saved_document`); the PDF text is read
  with hayro-syntax through each font's ToUnicode map, the same way as in the Slides test.
