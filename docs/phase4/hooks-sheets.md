# Command hook: Sheets (WP4 step 1 and 2, gridcraft)

Patch `gridcraft/0141-gridcraft-office-command-hook.patch` adds the seam. Nothing is installed by default, so with no hook
`Session::execute` takes exactly its former path (no clone, no call). WP10 (version snapshot) and WP11 (XDFU policy) fill it.

## The seam

`crates/engine/src/office_hook.rs` (new file) and 9 lines in `crates/engine/src/lib.rs`:

```rust
pub trait Hook: Send + Sync {
    fn before(&self, session: &Session, id: &str, params: &Json) -> Result<(), String> { Ok(()) } // Err(reason): refused
    fn after(&self, session: &Session, id: &str, params: &Json, result: &Result<Json>) {}
}
pub fn install(hook: Box<dyn Hook>) -> bool   // once per process; false when one is already installed
```

- `execute(id, params)` is now: no hook, run the old body (`execute_inner`); with a hook, `before`, then the old body on a
  clone of `params`, then `after`.
- A refusal is `EngineError::Disabled(id, reason)`: the same error the UI already shows for a disabled command, so a refused
  Print raises the usual box. A refused command never runs, and `after` is not called for it.
- `after` also runs when the command failed (it gets the `Result`). It sees the session after the command, so the snapshot
  hook reads `session.active().path` of a just-saved workbook.
- Every caller goes through `execute`: the UI (`SheetApp::run` and `run_or_alert`), the CLI and MCP (`execute_headless`), the
  control channel, the journal replay. The hook therefore also sees nested calls a command makes on its own session.
- Tests (`crates/engine/tests/office_hook.rs`, its own binary because a hook cannot be removed): `hook_refuses_print`,
  `hook_sees_save`, `guarded_ids_exist` (every guarded id of PLAN section 4 is in this registry, so a sync that renames one fails here).

## Bypass list (WP4 step 1: every UI path for a guarded action, read at BASE `2ebdf16`)

| Action | UI path | Reaches `execute`? | Handling |
|---|---|---|---|
| Print | ribbon/backstage Print and Ctrl+P: `print_now` -> `submit_print` -> `self.run("file.print")` (`lib.rs:597-607`); `ui.printSubmit` the same | yes | guarded by id |
| Export PDF / CSV / HTML | backstage Export, `file.exportPdf`, `file.exportCsv`, `file.exportHtml` | yes | guarded by id |
| Save, Save As | backstage, Ctrl+S, `file.save` / `file.saveAs` through `run_command` (`lib.rs:403` does the after-save bookkeeping) | yes | `file.save` after-hook = snapshot |
| AutoSave | `lib.rs:1230-1239` `self.session.run("file.save")` every 2 s while dirty | yes | the after-hook runs per autosave; the snapshot policy must throttle (WP10) |
| Copy, Cut | ribbon, Ctrl+C/X, context menu: `copy_to_clipboard` -> `run_command("edit.copy" / "edit.cut")` (`lib.rs:451-470`); drag-move `session.run("edit.cut")` (`grid.rs:2320`) | yes | guarded by id; the text reaches `ctx.copy_text` only after a successful `execute`, so a refusal puts nothing on the clipboard |
| Download (web host, Backstage "download a copy") | `file.saveBytes` (`backstage.rs:911`, `lib.rs:783`) is a **query** that returns the whole file | yes | stamped like an export (patch 0143; `guarded_ids_exist` lists it). The web build installs no hook (desktop only), so the web download stays unstamped: stated limit |
| Text copied out of the cell editor or the formula bar | egui's own `TextEdit` Ctrl+C/Ctrl+X handling | **no** | stated limit: it copies the text of one cell being edited, not a range. Not closed in Phase 4 |
| Operating-system screenshots, Print Screen, a phone camera | outside the process | no | stated limit (R11) |
| `ui.screenshot {path}` on the control channel | `control.rs:362`: a PNG of the window | no | the control channel exists only when the app is started with `--control` / `GRIDCRAFT_CONTROL_PORT`; the host never passes it. Stated limit |
| Drag a cell range out of the window, OLE | not implemented by the app | n/a | none |

Reading of the result: the one real gap (`file.saveBytes`) is closed by 0143, and four stated limits remain. No UI path needed a
one-line carried call: the dispatcher is the only door for copy, cut, print, export and save.

## What 0143 does with the seam (WP10 and WP11, Sheets)

XDFU in the editors is **watermark only** (owner, 2026-10-22): nothing is refused. The hook trait gained `wrap(session, id, params, run)`
(default: run the command), because a stamp has to be in the workbook while the command runs and be taken out afterwards.
`crates/ui-egui/src/office_history.rs` (desktop only, `init()` from `main`) installs the hook:

- **Snapshot.** `after` of `file.save` / `file.saveAs`, when the saved path is the document's own path: `Store::snapshot`. Errors go to
  stderr. The autosave timer passes `{"autosave": true}`; such a save snapshots at most every 5 minutes per file. An explicit save
  always snapshots (the store skips bytes it already has). `office.history` calls `open_panel`, or says to save the file first.
- **Watermark** (only with `xdfu = on` and a grif in the first five rows of the first sheet or in its headers and footers). Stamped, with
  the copy number from `Issuer::issue` (the audit line exists only if the copy was made): `file.print`, `file.exportPdf`,
  `file.exportCsv`, `file.exportHtml`, `file.saveAs` and `file.save` to a path other than the document's own (xlsx, json, pdf, csv, ...)
  and `file.saveBytes`. xlsx, json, pdf and print get the line as the first line of the centre footer of every sheet while the command
  runs (restored afterwards; the document stays clean); csv, tsv, txt and html get it as the first line. A plain `file.save` of the
  working file is not a copy and is not stamped.
- **Limits that remain:** `edit.copy` / `edit.cut` are not blocked and carry no mark (clipboard text has nowhere to put one); the
  bypasses of the table above; the headless CLI and MCP never install the layer; a copy whose audit log cannot be opened is still made,
  with copy number 0 and a message on stderr (nothing is refused); a workbook with the grif only below row 5 or on another sheet is not
  recognised.
- `office.xdfu.insertGrif {grif?}` (registered only when XDFU is on) writes the grif and `Nusxa N` (the audit counter) into the right
  section of the first sheet's header, above what was there, through `pageLayout.headerFooter` (so it is undoable).
- Tests (own binaries, the hook is process-wide): `office_history.rs`, `office_xdfu.rs`, `office_xdfu_off.rs` in `crates/ui-egui/tests`.
