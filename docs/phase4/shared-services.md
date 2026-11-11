# Shared services (`crates/suite-core`, Phase 4 WP5)

One small crate that the four apps, the desktop host and the shell share. Permissive dependencies only
(`toml_edit`, `serde_json`, `ron`; `zip` and `roxmltree` in tests), `#![forbid(unsafe_code)]`, no GUI, no network. It
lives in the root workspace (`crates/suite-core`); the domain workspaces and `services/` reach it by path
(`../../crates/suite-core`). Every name (product, data folders, font files) is read from `brand/brand.toml` by
`build.rs`; none is written in the code.

## Using it from an app lane

```toml
# in the lane's own crate (a path dependency; the path is relative to that crate)
suite-core = { path = "<repo>/crates/suite-core" }
```

```rust
use suite_core::settings::{self, ToolbarMode};

let mode: ToolbarMode = settings::toolbar_mode();          // Simple unless settings.toml says toolbar = "full"
settings::set_toolbar_mode(ToolbarMode::Full)?;            // io::Result<()>; atomic; other apps see it at their next start
let all: settings::Settings = settings::load();            // every key, with defaults; never fails
let loaded = settings::load_in(&dir);                      // explicit folder, with warnings (tests, the host)
```

Rules for lanes:

- **Read once at start-up** and keep the value in the app (the plan's `office_overlay::toolbar_simple()` flag). `load()`
  reads the file every call. Do not call it per frame.
- **Never fails.** A missing file, an unknown key, a bad value or a corrupt file all give defaults (see below). There is
  nothing to unwrap. `set_toolbar_mode` returns an error only when the folder cannot be written; show nothing for it, log it.
- **Web (`wasm32`).** There is no file. `toolbar_mode()` is the user's saved choice (`localStorage` key `suite.toolbar`, written by
  `set_toolbar_mode`), else the URL's `?toolbar=full|simple` (the document server passes it), else `Simple`
  (`settings::resolve_toolbar`). The other keys read as the defaults.
- **PDF ignores `toolbar`.**
- Tests of an app must not touch the real user file: debug builds honour `SUITE_SETTINGS_DIR=<folder>` (a release build
  ignores it), or call `settings::load_in` / `settings::set_in` on a temp folder.

## 1. Settings (`suite_core::settings`)

File: `<settings dir>/settings.toml`, where `<settings dir>` is `%APPDATA%\<product>` on Windows (`~/Library/Application
Support/<product>` on macOS, `$XDG_CONFIG_HOME/<product>` or `~/.config/<product>` elsewhere), and `<product>` is
`brand.toml` `[product] name`. `settings::dir()` returns it.

| key | values | default | read by |
|---|---|---|---|
| `lang` | `auto`, `en`, `ru`, `uz-Latn`, `uz-Latn-2026`, `uz-Cyrl` | `auto` | shell, apps (when the app's own language is "auto") |
| `latin_alphabet` | `1995`, `2026`, `auto` | `1995` (follows `uz-translit` owner decision O13) | transliteration surfaces |
| `toolbar` | `simple`, `full` | `simple` | Docs, Sheets, Slides |
| `theme` | `system`, `light`, `dark` | `system` (owner, 2026-10-29) | shell, all four apps at start (section 6.2) |
| `spell_dictionaries` | a folder path, up to 1024 bytes, no control characters | empty (the install folder's) | Docs, Slides (WP6) |
| `xdfu` | `off`, `on` | `off` | editors |
| `recent.limit` | 0 to 50 | 20 | shell, `recent` |
| `history.versions` / `.days` | 1 to 1000 / 1 to 3650 | 50 / 90 | history (WP10) |
| `history.max_mb` | 64 to 102400 | 2048 | history |
| `history.big_file_mb` / `.big_file_versions` | 1 to 4096 / 1 to 50 | 50 / 5 | history |
| `server.url` | empty, or `http(s)://...` without spaces, up to 2048 bytes | empty (local mode) | shell |

```toml
lang = "auto"
toolbar = "simple"
[recent]
limit = 20
```

Behaviour (each line has a test in `settings.rs`):

- **Typed.** A key has a type and a range. A wrong TOML type (`lang = 5`, `recent.limit = "5"`) or an out-of-range value is a
  warning in `Loaded::warnings` and that key falls back to its default. The other keys still apply, and the file is not moved.
- **Corrupt file.** A file that is not TOML, is not UTF-8, is over 1 MiB or is not a regular file is renamed to
  `settings.toml.corrupt` (the backup; a later one overwrites it) and the defaults apply. The next `set` starts a clean file.
- **Atomic.** A write goes to `settings.toml.<pid>.tmp` in the same folder and is renamed over the old file. A crash leaves
  the old file. `set_in` changes only the key it is given: other keys, unknown keys and comments are kept (`toml_edit`).
- **No lock.** Two processes writing different keys at the same instant can lose one change (last rename wins).
  Settings change on a click; a reader never sees a half-written file.
- **Unknown keys** are ignored on read and kept on write.

API summary (all in `suite_core::settings`):

| item | |
|---|---|
| `load() -> Settings` | the user's settings, defaults on any problem |
| `toolbar_mode() -> ToolbarMode` | `load().toolbar` |
| `set_toolbar_mode(ToolbarMode) -> io::Result<()>` | writes `toolbar` |
| `dir() -> Option<PathBuf>` | the settings folder |
| `load_in(&Path) -> Loaded { settings, warnings }` | explicit folder |
| `set_in(&Path, key, value) -> Result<(), SetError>` | one key by its dotted name, value in text form, validated |
| `validate(key, value) -> Result<(), SetError>` | the check alone |
| `Settings::to_map()` / `key_names()` | the flat text form (what the host sends the shell) |

## 2. Language (`suite_core::lang`)

`Lang` is `En | Ru | UzLatn | UzLatn2026 | UzCyrl` with `tag()` (`en`, `ru`, `uz-Latn`, `uz-Latn-2026`, `uz-Cyrl`).
`LangChoice` is the `lang` setting: `Auto` or `Fixed(Lang)`. `choice.resolve(system_locale)` gives the language to show:
a fixed choice wins; `Auto` follows the system locale name the caller passes (the app already detects it; this crate does
no FFI), mapping `uz`, `uz-UZ`, `uz-Latn-UZ` to `uz-Latn`, `uz-Cyrl-UZ` to `uz-Cyrl`, `ru-RU` to `ru`, `en-*` to `en`,
and anything else (or nothing) to `en`. `Lang::from_locale` is the mapping alone.

The shell shows `uz-Latn-2026` with the `uz-Latn` catalog until WP16 adds a 2026 catalog to it (the converter already has one).

## 3. Recent files (`suite_core::recent`)

Reads the four apps' own lists, read-only, tolerant, with **no upstream change** (PLAN 5.2). `recent::recent()` returns
`(Vec<Recent { path, app, ts }>, warnings)`, newest first, no duplicates (case-insensitive on Windows), only files that still
exist, at most `recent.limit`. `sources_in(base, profile)` and `merge(sources, limit, exists)` are the testable pieces.

| app | file (first folder that has it: `<base>/<brand data_dir>`, then the upstream folder) | shape |
|---|---|---|
| Docs | `ui.json` in `WordCraft` (or `WORDCRAFT_PROFILE_DIR`) | `{"recent": ["<path>", ...]}` |
| Sheets | `ui.json` in `GridCraft` | same |
| Slides | `ui.json` in `DeckCraft` | same |
| PDF | `data/app.ron` (then `app.ron`) in `PdfCraft` | RON map; key `app` is JSON `{"recent": [{"name","path","pages","size"}]}` |

(The plan named `prefs.json` for gridcraft and deckcraft; the real list is in `ui.json`, checked at the new BASEs.)

The lists carry no time per file, so entries get synthetic times: the list file's modified time minus the position. An app's
own order is never reversed; across apps, the app whose settings were saved last comes first.

Contract tests `recent_reads_{wordcraft,gridcraft,deckcraft,pdfcraft}` read the vendored upstream sources
(`upstream/<repo>/...`) and fail if the `recent` field, its serde naming, the file name or the folder name changes, then read a
fixture of the real shape. A sync that changes a list therefore fails `ci.sh test-ours` instead of silently emptying Recent.

## 4. Fonts (`suite_core::fonts`)

The shared Noto Sans fallback (covers U+02BB, U+02BC and Cyrillic): `fonts::SANS` and `fonts::MONO` are the file names from
`brand.toml` `[fonts]`, relative to `assets/fonts/`. `fallback_sans()` and `fallback_mono()` find them: `SUITE_FONTS_DIR`,
then `fonts/` or `assets/fonts/` next to the exe (where the installer puts the one shared copy), then `assets/fonts/` up the
tree (a development build). The apps keep using their own fallback (patches `*/0102`); this is the lookup the host, the shell
build and the MSI use so that there is one copy.

## 5. Templates (`suite_core::templates`, `web/shell/templates.json`)

Five clean-room Uzbek starters, made by `scripts/gen-templates.py` (hand-written OOXML, deterministic; `--check` fails on
drift). They are original text; nothing comes from `corpus/`, and no real name, number, address or organisation appears. Every
blank is a `[bracket]`. The names and the text are Uzbek UI strings: status `draft:agent-sonnet:2026-10-23` in the catalogue
until the review pass marks them `reviewed:agent-<model>:<date>`.

| file | app | content |
|---|---|---|
| `templates/buyruq.docx` | Docs | Order: organisation, number and date, basis, numbered orders, control clause, signature, acknowledgement lines |
| `templates/xizmat-xati.docx` | Docs | Official letter: addressee block, outgoing number and date, subject, three paragraphs, enclosure, signature, contact |
| `templates/tabel.xlsx` | Sheets | Monthly timesheet: 31 day columns, codes I K T S D, `COUNTIF` totals, daily hours, hours total, frozen panes, landscape |
| `templates/xodimlar-royxati.xlsx` | Sheets | Staff list: 30 numbered rows, date column, filter, `COUNTA` total |
| `templates/taqdimot.pptx` | Slides | Four 16:9 slides: title, plan, content with sub-points, thanks and questions |

The catalogue entry is `{app, file, status, name:{en,ru,uz-Latn,uz-Cyrl}}`; the shell fetches `shell/<file>`.
`templates::parse` checks it (known app, `templates/<plain name>.<ext of the app>`, all four names, a status) and the tests open
every package, parse every XML part and refuse e-mail addresses and long digit runs.

## 6. Desktop host and shell

`services/suite-desktop` keeps its allowlist; `settings_get` and `settings_set` now read and write the same
`settings.toml` through `suite-core`. The page may set only `lang`, `toolbar`, `latin_alphabet` and `theme`, each with the value
check above (anything else, such as `server.url` or `spell_dictionaries`, is refused: they decide where the shell navigates
and which files are read, and no page needs them yet). `settings_get` returns those four as strings. The shell reads `lang`
from it (`auto` means: follow the browser language) and `theme`, and writes either on a change.

Not done here: the dictionaries folder in Docs (WP6); the installer's single font copy.

### 6.1 The apps' language at start (WP24 gate review, owner 2026-10-24)

Each app's `office_overlay.rs` has `shared_lang_code()`: `suite_core::settings::load().lang`, read once, mapped to the code the
app registers (`en`, `ru`, `uz-latn`, `uz-cyrl`; `uz-Latn-2026` shows `uz-latn` until the apps get a 2026 catalog). It is `None`
for `auto`, with no file, in the web build and in tests. Where it applies:

| app | patch | where | rule |
|---|---|---|---|
| Docs | `wordcraft/0126` | `office_overlay::start_language`, called in `apps/wordcraft/src/main.rs` after `load_prefs` | a fixed shared `lang` replaces `ui.language` (even a saved fixed one); `auto` keeps it |
| PDF | `pdfcraft/0112` | the same, in `apps/pdfcraft/src/main.rs` after the saved preferences are restored | same |
| Slides | `deckcraft/0145` | the same, in `apps/deckcraft/src/main.rs` after `load_prefs`; Slides' own `auto` has always been English, so shared `auto` stays English (no system detection) | same |
| Sheets | `gridcraft/0145` | `load_prefs` in `apps/gridcraft/src/main.rs` (Sheets has no `auto` value: a first run saves the system language into `ui.json`) | same |

**One rule for all four apps** (main session, 2026-10-24): a fixed shared `lang` (anything but `auto`) always wins at start; the
app's own saved language applies only when the shared `lang` is `auto`; an in-app language change applies for the session and is
**not written back** to `settings.toml` (the shell is the only writer; the apps only read `lang`). A shared language the app
does not register (for example `uz-*` without the overlay catalogs) is ignored and the saved one stays. The web builds never
read the file. The in-app change still lands in the app's own `ui.json`, but a fixed shared `lang` overrides it at the next start.
Check: the `office_overlay` tests `shared_lang_picks_the_catalog_language` (the choice-to-code map and the app's registered language) and `shared_lang_wins_over_the_saved_language_at_start` (the precedence) in each app; tests never read the real settings file. Not checked: a real window opened with `lang = "ru"` (no desktop session in this run).

### 6.2 One theme for the shell and the apps (owner, 2026-10-29)

The owner kept each app's own look (`design-system.md`, "Revised") and asked for one place to choose light or dark. The key is
`theme = "system" | "light" | "dark"` (default `system` = follow the OS). The shell's header has a select for it next to the
language (`web/shell/index.html`, `#theme`). On the desktop the host writes it to `settings.toml`; on the web the browser keeps it
(`localStorage` `sh.theme`). The shell page applies it at once (`shell.js` `applyTheme`: `<html data-theme>`, the logo, the accent;
`system` follows `prefers-color-scheme` live). The apps read it once at start through `office_overlay::shared_theme_code()`
(native only; `None` in the web build and in tests) and apply it to their **own** light or dark palette:

| app | patch | where | what `system` does |
|---|---|---|---|
| Docs | `wordcraft/0132` | `office_overlay::start_theme`, `apps/wordcraft/src/main.rs` after `start_language` | upstream's `Appearance::System`: follows the OS live |
| Sheets | `gridcraft/0148` | `office_overlay::start_theme(&mut app.ui)`, `apps/gridcraft/src/main.rs` after `load_prefs` | upstream's `system_theme = true`: follows the OS live |
| Slides | `deckcraft/0149` | `main.rs` calls `office_overlay::load_start_theme`; `SlideApp::logic` applies `start_brightness` at the first frame | Slides has only Light and Dark: the OS answer at the first frame (not followed live; restart to follow a later OS change) |
| PDF | `pdfcraft/0115` | `office_overlay::start_theme`, `apps/pdfcraft/src/main.rs` after `start_language` (`set_theme_preference`) | upstream's `ThemePreference::System`: follows the OS live |

**Rule.** Unlike `lang`, every value wins at start, `system` included: `system` is a choice here (the owner's default), not
"no opinion", and the shell is the one place to change the theme. An in-app change (View or Options) holds for the session and
lands in the app's own `ui.json`, but the shared value replaces it at the next start. Nothing is written back to `settings.toml`.
With no file the default `system` applies, so Slides and PDF, whose upstream default is Light, now follow the OS on the desktop.
The web editors do not read the setting (no file; the shell does not pass it): Docs and Sheets follow the browser's
`prefers-color-scheme`, Slides and PDF their own default.
Check: `suite-core` `theme_defaults_to_system_and_a_bad_value_costs_only_the_theme` and `the_map_round_trips_through_set`;
`suite-desktop` `missing_editor_and_pick_and_settings`; in each app the `office_overlay` test `shared_theme_wins_over_the_saved_*`;
the shell smoke tests `desktop mode: the theme from settings.toml ...` and `web mode: the theme is kept by the browser ...`.
