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
