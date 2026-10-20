# Phase 2 WP0: re-verification of plan references

Checked 2026-10-20 on branch `phase-2` (HEAD d8bae2f) against the current tree. Upstream paths are shown under
`upstream/<repo>/` (the plan writes them repo-relative). `docs/`, `scripts/`, `xtask/`, `vendor-notes/` and other root-repo
files are outside this check. Files the plan asks us to create are listed as PLANNED, not MISSING.

Legend: OK = cited symbol or behaviour is at the cited line. MOVED = symbol still exists at a new line, or the cited line
now holds something else. MISSING = symbol not found.

| Reference in plan | Status | Current file:line | Note |
|---|---|---|---|
| §0 P1 `session.rs:364` (Session.registry) | OK | `upstream/wordcraft/crates/engine/src/session.rs:364` | `pub registry: Arc<Registry>` |
| §0 P1 `lib.rs:108-122` (Registry new/all) | OK | `upstream/wordcraft/crates/engine/src/lib.rs:110`, `:120` | `Registry::new` at 110, `all` at 120 |
| §0 P1 `lib.rs:61-101` (CommandSpec Clone) | OK | `upstream/wordcraft/crates/engine/src/lib.rs:60-61` | `#[derive(Clone)]` at 60, struct at 61 |
| §0 P4 `session.rs:1167-1303` (run, Arc snapshots) | OK | `upstream/wordcraft/crates/engine/src/session.rs:1167` | `pub fn run` at 1167; Arc snapshots at 850, 866 |
| §0 P5 `para.rs:17` (U+FFFC) | OK | `upstream/wordcraft/crates/doc/src/para.rs:17` | `pub const OBJ` = U+FFFC |
| §0 P5 `para.rs:40` (Run) | OK | `upstream/wordcraft/crates/doc/src/para.rs:40-41` | `pub struct Run`, `pub len` |
| §0 P5 `para.rs:767` (Paragraph) | OK | `upstream/wordcraft/crates/doc/src/para.rs:767-768` | `pub struct Paragraph`, `pub text` |
| §0 P5 `props.rs:348` (lang) | OK | `upstream/wordcraft/crates/doc/src/props.rs:348` | `pub lang: Option<String>` |
| §0 P5 `props.rs:377` (link) | OK | `upstream/wordcraft/crates/doc/src/props.rs:377` | `pub link: Option<String>` |
| §0 P5 `props.rs:380` (ins) | OK | `upstream/wordcraft/crates/doc/src/props.rs:380` | `pub ins: Option<u32>` |
| §0 P5 `props.rs:383` (del) | OK | `upstream/wordcraft/crates/doc/src/props.rs:383` | `pub del: Option<u32>` |
| §0 P5 / §3.4 `para.rs:445-479` (Field, FieldStart, FieldEnd) | OK | `upstream/wordcraft/crates/doc/src/para.rs:445`, `:470`, `:479` | Field 445-465, FieldStart 470, FieldEnd 479 |
| §0 P6 `format.rs:383` (change_case) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:383` | `fn change_case` |
| §0 P6 `format.rs:500` (live_chars) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:500` | `fn live_chars` |
| §0 P6 `format.rs:517` (case_groups) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:517` | `fn case_groups` |
| §0 P6 `format.rs:541` (map_offset) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:541` | `fn map_offset` |
| §0 P6 `format.rs:446-463` (tracked recipe) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:446-447` | `new_revision(Delete/Insert)` at 446 |
| §3.3 `format.rs:421-422` (per-paragraph bounds) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:421-422` | `from`/`to` per path |
| §3.3 `format.rs:481-485` (caret map) | OK | `upstream/wordcraft/crates/engine/src/cmd/format.rs:481-485` | `end.off = map_offset(...)` |
| §0 P7 `column.rs:178-190` (dispatch) | OK | `upstream/wordcraft/crates/engine/src/cmd/column.rs:178-190` | `per_row` arm at 178; caret fallback `_ if mutates` at 187 |
| §0 P7 / §3.1 `protect.rs:243-245` (direct formatting) | OK | `upstream/wordcraft/crates/engine/src/cmd/protect.rs:243-245` | `id.starts_with("format.")` at 243 |
| §0 P8 `layout/src/lib.rs:485` (cache) | OK | `upstream/wordcraft/crates/layout/src/lib.rs:485` | `pub struct LayoutCache` |
| §0 P8 `layout/src/lib.rs:804-819` (cache key) | OK | `upstream/wordcraft/crates/layout/src/lib.rs:804-819` | `Key { rev: p.rev, ... }` |
| §0 P8 / §1.3 `para.rs:829` (touch) | OK | `upstream/wordcraft/crates/doc/src/para.rs:829` | `pub fn touch` |
| §3.1 `para.rs:1047` (normalize) | OK | `upstream/wordcraft/crates/doc/src/para.rs:1047` | `pub fn normalize` |
| §3.3 `para.rs:1077` (delete drops objects) | OK | `upstream/wordcraft/crates/doc/src/para.rs:1077` | `pub fn delete` |
| §3.1 `para.rs` props_of_char, insert_text, format | OK | `upstream/wordcraft/crates/doc/src/para.rs:950`, `:996`, `:1141` | all pub |
| §3.1 `lib.rs:835` (para_paths) | OK | `upstream/wordcraft/crates/doc/src/lib.rs:835` | `pub fn para_paths` |
| §3.1 `lib.rs:806` / `:813` (para, para_mut) | OK | `upstream/wordcraft/crates/doc/src/lib.rs:806`, `:813` | |
| §3.1 `settings.track_changes (:450)` | OK | `upstream/wordcraft/crates/doc/src/lib.rs:450` | `pub track_changes` in `Settings` (449) |
| §3.1 `lib.rs:662` / §3.3 `lib.rs:252-260` (Document.parts, PartKind) | OK | `upstream/wordcraft/crates/doc/src/lib.rs:662`, `:252-260` | PartKind = Header, Footer, Footnote, Endnote, Comment, TextBox |
| §3.1 `cmd/mod.rs:139` (selected_paths) | OK | `upstream/wordcraft/crates/engine/src/cmd/mod.rs:139` | |
| §3.1 `cmd/mod.rs:162` (new_revision) | OK | `upstream/wordcraft/crates/engine/src/cmd/mod.rs:162` | |
| §3.1 `cmd/mod.rs:456` (para_lang) | OK | `upstream/wordcraft/crates/engine/src/cmd/mod.rs:456` | |
| §3.1 `controls.rs:353` (check_edit) | OK | `upstream/wordcraft/crates/engine/src/cmd/controls.rs:353` | `pub fn check_edit` |
| §3.1 `lib.rs:61-101` / `lib.rs:139-165` (CommandSpec builders, p readers) | OK | `upstream/wordcraft/crates/engine/src/lib.rs:61-101`, `:139-165` | `pub mod p` at 139 |
| §3.1 `apps/wordcraft/src/main.rs:422` (Session::new in main) | OK | `upstream/wordcraft/apps/wordcraft/src/main.rs:422` | inside `fn main` (319) |
| §3.1 `apps/wordcraft/src/main.rs:497` (second session path?) | OK (location), NOT a product path | `upstream/wordcraft/apps/wordcraft/src/main.rs:497` | Inside `#[cfg(all(test, unix, not(target_os = "macos")))] mod tests` (from 484). Test only. |
| §3.1 `apps/wordcraft-web/src/web.rs:79` | OK | `upstream/wordcraft/apps/wordcraft-web/src/web.rs:79` | in `pub fn start` (20) |
| §3.1 `apps/wordcraft-cli/src/main.rs:103` | OK | `upstream/wordcraft/apps/wordcraft-cli/src/main.rs:103` | in `fn session` (102) |
| §6 `ui-egui/src/ribbon.rs:1757` (Docs review) | OK | `upstream/wordcraft/crates/ui-egui/src/ribbon.rs:1757` | `fn review` |
| §6 ribbon group `:1771-1774` (Language) | OK | `upstream/wordcraft/crates/ui-egui/src/ribbon.rs:1771-1774` | `group(ui, "Language", ...)` at 1771 |
| §6 `canvas.rs:2835` (context_menu) | OK | `upstream/wordcraft/crates/ui-egui/src/canvas.rs:2835` | `fn context_menu` |
| §6 `keytips.rs:108` | OK | `upstream/wordcraft/crates/ui-egui/src/keytips.rs:108` | iterates `registry.all()` |
| §6 `dialogs_keyboard.rs:47` | OK | `upstream/wordcraft/crates/ui-egui/src/dialogs_keyboard.rs:47` | iterates `registry.all()` |
| §9 WP10 `ui-egui/src/control.rs` (ui automation) | OK | `upstream/wordcraft/crates/ui-egui/src/control.rs:152-157` | `ui.render`, `ui.inspect` handlers |
| §4 `cmd/mod.rs:110` (cmd macro) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:110` | `macro_rules` cmd |
| §4 / §0 P2 `cmd/mod.rs:123` (command_specs) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:123` | `OnceLock` at 124 |
| §0 P2 `cmd/mod.rs:151` (find_command) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:151` | `pub fn find_command` |
| §4 `cmd/mod.rs:319` (edit) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:319` | `pub(crate) fn edit` |
| §4 `cmd/mod.rs:522` (cx.touch) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:522` | `pub fn touch` |
| §4 `cmd/mod.rs:328-368` (commit) | OK | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:328-368` | `pub(crate) fn commit` at 328 |
| §4 `cmd/mod.rs:362-364` (auto_row_height) | MOVED | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:360` (call), `:458` (def) | 362-364 is the timing eprintln. auto_row_height is called at 360 and defined at 458. |
| §0 P4 `lib.rs:327` (execute) | OK | `upstream/gridcraft/crates/engine/src/lib.rs:327` | `pub fn execute` |
| §0 P4 `lib.rs:360-370` (snapshot swap) | OK | `upstream/gridcraft/crates/engine/src/lib.rs:360-370` | `Arc::ptr_eq(&d.wb, &wb)` at 360 |
| §4 `edit.rs:1514` (within workbook) | OK | `upstream/gridcraft/crates/engine/src/cmd/edit.rs:1514` | in `fn replace` (1505) |
| §4 `edit.rs:1519-1534` (replace rewrites formula cells via input_to_cell) | MOVED | `upstream/gridcraft/crates/engine/src/cmd/edit.rs:1536` | Lines 1519-1534 are the sheet loop and `replace_ci`. The `input_to_cell` call is at 1536. |
| §4 `selection.rs:8-15` (selection areas) | OK (struct) | `upstream/gridcraft/crates/engine/src/selection.rs:8-15` | `pub struct Selection`, `pub ranges` |
| §4 `target_ranges(s, p)` (cited as selection.rs:8-15) | MOVED | `upstream/gridcraft/crates/engine/src/cmd/mod.rs:237` | `pub(crate) fn target_ranges` lives in cmd/mod.rs, not selection.rs |
| §4 `core/src/value.rs:146-154` (Value) | OK | `upstream/gridcraft/crates/core/src/value.rs:146-154` | `pub enum Value`, `Text` at 150 |
| §4 `model/src/cell.rs:276` (Cell) | OK | `upstream/gridcraft/crates/model/src/cell.rs:276` | `pub struct Cell` |
| §0 P3 `xlsx/src/xml.rs:67-75` (rich_text) | OK | `upstream/gridcraft/crates/xlsx/src/xml.rs:67-75` | `pub fn rich_text` at 69 |
| §0 P3 `sheet_read.rs:446` | OK | `upstream/gridcraft/crates/xlsx/src/sheet_read.rs:446` | `xml::rich_text(is)` |
| §0 P3 `xlsx/src/write.rs:378` | OK | `upstream/gridcraft/crates/xlsx/src/write.rs:378` | `<si>` writer |
| §9 WP9 `gridcraft-cli main.rs:153` | OK | `upstream/gridcraft/apps/gridcraft-cli/src/main.rs:153` | `fn exec` |
| §6 `grid.rs:2859` (context_menu) | OK | `upstream/gridcraft/crates/ui-egui/src/grid.rs:2859` | `fn context_menu` |
| §6 `grid.rs:2865-2886` (item tuples) | OK | `upstream/gridcraft/crates/ui-egui/src/grid.rs:2865-2886` | `("Link…", "ui:insertLink")` at 2886 |
| §6 `ribbon.rs:2017` (Sheets review) | OK | `upstream/gridcraft/crates/ui-egui/src/ribbon.rs:2017` | `fn review` |
| §6 `ribbon.rs:481-503` (menu_items) | OK | `upstream/gridcraft/crates/ui-egui/src/ribbon.rs:481` | `fn menu_items` at 481 |
| §5 `cmd/mod.rs:3-32` (pub mod list) | OK | `upstream/deckcraft/crates/engine/src/cmd/mod.rs:3-32` | `pub mod animation` to `pub mod zoom` |
| §0 P2 `cmd/mod.rs:148` (cmd macro) | OK | `upstream/deckcraft/crates/engine/src/cmd/mod.rs:148` | `macro_rules` cmd |
| §0 P2 / §5 `cmd/mod.rs:161` (command_specs) | OK | `upstream/deckcraft/crates/engine/src/cmd/mod.rs:161-165` | `v.extend(file::specs())` at 165 |
| §0 P2 `cmd/mod.rs:197` (find_command) | OK | `upstream/deckcraft/crates/engine/src/cmd/mod.rs:197` | `pub fn find_command` |
| §0 P4 `lib.rs:579-628` (execute) | OK | `upstream/deckcraft/crates/engine/src/lib.rs:579-628` | `pub fn execute` at 579 |
| §5 `lib.rs:643` (Session.edit) | OK | `upstream/deckcraft/crates/engine/src/lib.rs:643` | `pub fn edit` |
| §5 `lib.rs:73-94` (TextSel) | OK | `upstream/deckcraft/crates/engine/src/lib.rs:73-94` | `pub struct TextSel` at 73 |
| §5 `cmd/find.rs:161` (replace) | OK | `upstream/deckcraft/crates/engine/src/cmd/find.rs:161` | `fn replace` |
| §5 `find.rs:196-198` (swap) | OK | `upstream/deckcraft/crates/engine/src/cmd/find.rs:196-198` | `*slp = Arc::new(sl);` at 196 |
| §5 `find.rs:43` (bodies, private) | OK | `upstream/deckcraft/crates/engine/src/cmd/find.rs:43` | `fn bodies` (private) |
| §5 `model/src/text.rs:438-460` (RunKind, Run) | OK | `upstream/deckcraft/crates/model/src/text.rs:438-460` | `pub enum RunKind` at 438, `kind` at 460 |
| §5 `text.rs:494` (Paragraph) | OK | `upstream/deckcraft/crates/model/src/text.rs:494` | `pub struct Paragraph` |
| §5 `text.rs:542` (normalize) | OK | `upstream/deckcraft/crates/model/src/text.rs:542` | `pub fn normalize` |
| §5 `text.rs:597` (TextBody) | OK | `upstream/deckcraft/crates/model/src/text.rs:597` | `pub struct TextBody` |
| §5 `model/src/lib.rs:621` (Slide.notes) | OK | `upstream/deckcraft/crates/model/src/lib.rs:621` | `pub notes: TextBody` |
| §5 `table.rs:11` (table cell text) | OK | `upstream/deckcraft/crates/model/src/table.rs:11` | `pub text: TextBody` |
| §6 `ui-egui/src/ribbon.rs:2148` (Slides review) | OK | `upstream/deckcraft/crates/ui-egui/src/ribbon.rs:2148` | `fn review` |
| §6 `ribbon.rs:2158` (Language big button) | MOVED | `upstream/deckcraft/crates/ui-egui/src/ribbon.rs:2159` | `big(app, ui, Icon::Language, "Language", ...)` at 2159; 2158 is a hover-text line |
| §6 `canvas.rs:1075` (context_menu) | OK | `upstream/deckcraft/crates/ui-egui/src/canvas.rs:1075` | `fn context_menu` |
| §9 WP9 `deckcraft-cli main.rs:437-492` (run) | OK | `upstream/deckcraft/apps/deckcraft-cli/src/main.rs:435-492` | `fn run` at 435; `--cmd` at 448 |
| §3.1 `domain.toml` `[extensions]` (planned key) | PLANNED | `domains/docs/domain.toml:4-12` | [desktop], [web], [cli] exist. `[extensions]` is WP4 work. |
| §3.1 `domains/docs/crates/docs-translit` | PLANNED | (none) | New crate, WP4 |
| §7 / WP8 `docs-bind` pin `wasm-bindgen = "=0.2.129"` | OK | `domains/docs/crates/docs-bind/Cargo.toml:27` | |

## Deckcraft trap check (P7 / §5)
- `upstream/deckcraft/crates/engine/src/cmd/` has no `column.rs` and no `protect.rs`. The text path has no wordcraft-style column dispatch and no styles-only protection check, so `text.translitCyrl` and `text.translitLatn` need no extra dispatch line. The only protection-like check is `crates/engine/src/cmd/file.rs:146` (password), which is unrelated.

## Other WP0 items (not done here, no builds run)
- `scripts/tools.lock` has no `wasm-bindgen-cli` entry. The pin is still to do.
- `wasm-bindgen` is not on the Bash PATH, so the done-check `wasm-bindgen --version` → `0.2.129` is still to do.
- `upstream/wordcraft/apps/wordcraft/src/main.rs:497` is a test path, not a second product session.

## Totals
- MOVED: 4 (gridcraft `cmd/mod.rs:362-364`, gridcraft `edit.rs:1519-1534`, gridcraft `target_ranges` cited as `selection.rs:8-15`, deckcraft `ribbon.rs:2158`)
- MISSING: 0
- PLANNED (new files, expected absent): 2
