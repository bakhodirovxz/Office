# deckcraft strings that need a code change (not fixed here)

The translations keep the placeholders, so lint and the runtime reader accept the rows, but on screen an English fragment still shows.
Line numbers are the current `upstream/deckcraft` tree (patched). Fix them in `patches/`, not in `upstream/`.

| Catalog id | English | Site | Problem | Suggested fix |
|---|---|---|---|---|
| 2198b175545c | `{n} Column{}` | `upstream/deckcraft/crates/ui-egui/src/ribbon.rs:772` | `{}` is the English plural `s` | count-neutral template, for example `Columns: {n}` |
| 101d8cc1dee7 | `{n} Slide{}` | `upstream/deckcraft/crates/ui-egui/src/ribbon.rs:3327` | same | `Slides: {n}` |
| 8d97086a708a | `{names} {it} installed on this computer. Text that uses {them} is shown in a similar font.` | `upstream/deckcraft/crates/ui-egui/src/toast.rs:27-28` | `{it}` = isn't/aren't and `{them}` = it/them are English words spliced in | two whole-string templates (one font / several fonts) |
| (no row) | `{a} and {b}`, `{a}, {b}, {c} and {} more` | `upstream/deckcraft/crates/ui-egui/src/toast.rs:23-25` | the `names` list is built with English "and" / "more" and never goes through `t()` | translate the list joiner, or template the three cases |
| cc386cbf509d | `Align text to the {l} of the cell` | `upstream/deckcraft/crates/ui-egui/src/ribbon.rs:3038` (label from `:3021`) | `{l}` = Top/Center/Bottom and the whole string is lower-cased, so it only reads correctly if `{l}` is translated first | run `{l}` through `t()` before `format!`, and do not `to_lowercase()` the Uzbek |
| (no row yet) | `{n} picture{} or media file{} ... couldn't be read and {} empty.` | `upstream/deckcraft/crates/engine/src/cmd/file.rs:250-254` | three English grammar splices (`s`, `s`, `is/are`); the string is not in the catalog | whole-string templates for singular and plural before it is extracted |
