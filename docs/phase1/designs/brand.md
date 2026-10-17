# WP7a: branding design (brand config, overlay hook, per-app sites, assets, CI check)

| | |
|---|---|
| **Status** | Design, frozen for WP7b–g. Written 2026-10-17 on branch `phase-1`. No builds, no edits under `upstream/` or `patches/`. |
| **Snapshot** | Re-grepped at the **old BASE** that is vendored today (wordcraft `fd0bb2f`, gridcraft `e6b6b34` + queue 0001–0022, deckcraft `9859da3`, pdfcraft `afa8e18`). WP5 moves to a new BASE. **Every `file:line` here must be re-checked before a patch is written** (the `brand-grep` in §7 finds every site again mechanically). |
| **Gate decisions applied** | Working name from `brand/brand.toml` only, never hard-coded. Our layer is MIT OR Apache-2.0. No upstream PRs: every change below is a carried patch registered in `patches/REGISTER.md`. |
| **Inputs** | PLAN.md WP7, §3; CLAUDE.md; four read-only inventory scans (one per app), with the load-bearing lines spot-checked by hand. |

## 0. Findings that change the plan

| # | Finding | Evidence | Consequence |
|---|---|---|---|
| B1 | **No app has telemetry, analytics, crash upload or an automatic update check.** The only outbound HTTP call in any shipped app is pdfcraft's **user-initiated** "Help › Check for updates…". It is not a startup check: upstream documents "there is no check at start (the owner's decision)". | `pdfcraft/crates/ui-egui/src/updates.rs:1-6`; `apps/pdfcraft/src/updates.rs:7` (`api.github.com/repos/storytold/pdfcraft/releases/latest`, `ureq`); wired at `apps/pdfcraft/src/main.rs:389` | PLAN F5 overstated the risk. We still disable it (§6), because it would offer our users upstream PdfCraft downloads. |
| B2 | **The ArtCraft marks are compiled into pdfcraft only.** `include_bytes!` of `docs/brand/artcraft-{logo,logo-white,mark}.svg` (`widgets.rs:506,508,516`). The other three apps never compile `docs/brand/`. deckcraft instead *paints* its own DeckCraft logo in code (`dialogs.rs:2640-2653`). | `widgets.rs:503-520` | Removing pdfcraft's marks is a **licence obligation**, not a choice: `docs/brand/LICENSE-brand.txt` ("Forks and modified versions … must remove the ArtCraft Marks"). |
| B3 | The brand licence **explicitly allows** a plain-text line: "You may say, in plain text, that your work is based on PdfCraft by the ArtCraft team." | `pdfcraft/docs/brand/LICENSE-brand.txt` (same file in all four repos) | The attribution wording in `brand.toml` should use exactly that form (§2). It must not use a logo, and must not suggest endorsement. |
| B4 | Brand-bearing **catalog keys**: wordcraft has 24 distinct English keys containing WordCraft/ArtCraft (about 26 rows in each of 22 TSVs), pdfcraft has 29 (590 rows), gridcraft has 5 (14 entries). Translations embed the name too (`ja`: "GridCraft について"). | Inventory scans | Rebrand **after** lookup, at the one choke point per app (§3.3). Catalog keys stay upstream's, so every existing translation keeps working. |
| B5 | Names are also **written into users' files**: docx/xlsx/pptx `Application`, PDF `Producer`/`Creator`, ODT/HTML/RTF generator, gridcraft `INFO("osversion")`, default author names. | §4 per app, "file metadata" rows | Not chrome, but recipients see it. A separate droppable patch `0102-<app>-file-metadata` (§3.6). Owner decision O3. |
| B6 | The upstream **app icons are MIT OR Apache-2.0** (e.g. `gridcraft/assets/app-icon/LICENSE.txt`, `pdfcraft/assets/app-icon/LICENSE.txt`). Only `docs/brand/*` is trademark-restricted. | Licence files | Replacing the app icons is a product-identity choice, not a legal must. We replace them anyway (WP7 goal), and nothing breaks if a size is missed. |
| B7 | pdfcraft's strict `cargo xtask assets` check rejects any new asset-like file (png, svg, ico, ttf, **tsv**…) that has no `ATTRIBUTION.toml` entry with a matching SHA-256. | `pdfcraft/xtask/src/assets.rs:53-56,243-260` | Our icons, catalogs and fonts must **never enter the upstream tree**. They live in the overlay directory (outside the tree) and are reached through `include_bytes!` with absolute paths from `OUT_DIR`. This is the main reason for the overlay design. |

---

## 1. Design summary (options, choice, trade-off)

Options for carrying a brand:
- **(A) Edit the literals in place** (rename "WordCraft" → "Suite Docs" in each file). Simplest patch, but it hard-codes the name in the patch queue (forbidden: B-1 is deferred), and it conflicts on every catalog edit upstream makes.
- **(B) Overlay hook** (PLAN §3.2). Each app carries one generated-constants module plus one-token call-site edits. Brand data (name, icons, catalogs, fonts) lives in our tree and enters the build through `OFFICE_OVERLAY_DIR`.
- **(C) Post-build binary patching** of strings. Fragile, length-limited, cannot remove UI. Rejected.

**Choice: (B)**, with one refinement over PLAN §3.2: **rebrand at the translation choke point** (`i18n::t`/`tr`, gridcraft `Language::tr`), not at each `tl!` call site. That single edit per app covers all of B4's catalog strings (tens of sites) at once. Only the raw literals that do not pass through i18n need their own one-token edits.

Decisive trade-off: (B) costs one new file plus one small `build.rs` hook per app and a runtime `contains()` per looked-up string. In return, the name never appears in `patches/`, choosing B-1 later touches only `brand.toml`, and the hunks in hot files stay at a token or two each. A pristine-like build (overlay unset) behaves exactly like upstream, so upstream's own tests remain the regression net.

---

## 2. `brand/brand.toml`: schema (v1)

The only file that contains the working name. `crates/office-brand` parses it (with `toml_edit`, per PLAN §1.2) and validates it. `cargo xt overlay <app>` renders it into the overlay (§3.1).

```toml
schema = 1

[product]
id      = "suite"                 # REQUIRED. [a-z][a-z0-9-]{1,31}. Machine id: data dirs, app ids. Changed once, before Phase 4 installs.
name    = "Suite"                 # REQUIRED. Display name. Owner decision B-1 (deferred). 1..40 chars, no control chars.
version = "0.1.0"                 # REQUIRED. SemVer. Shown in About, VERSIONINFO.
vendor  = "Suite contributors"    # REQUIRED. VERSIONINFO CompanyName, MSI Manufacturer (Phase 4).
copyright = "Copyright (c) the Suite contributors. MIT OR Apache-2.0."   # VERSIONINFO LegalCopyright, About footer.

[attribution]                     # REQUIRED. Plain text only (brand licence B3). Owner reviews the wording.
en      = "Based on WordCraft, GridCraft, DeckCraft and PdfCraft by the ArtCraft team (MIT OR Apache-2.0)."
ru      = "Основано на WordCraft, GridCraft, DeckCraft и PdfCraft от команды ArtCraft (MIT OR Apache-2.0)."
uz-Latn = "WordCraft, GridCraft, DeckCraft va PdfCraft (ArtCraft jamoasi, MIT OR Apache-2.0) asosida."   # DRAFT, unreviewed
uz-Cyrl = "WordCraft, GridCraft, DeckCraft ва PdfCraft (ArtCraft жамоаси, MIT OR Apache-2.0) асосида."   # DRAFT, unreviewed
# Per-app override (optional): [attribution.docs] en = "Based on WordCraft by the ArtCraft team …"

[apps.docs]                       # One table per app; keys docs|sheets|slides|pdf are fixed.
upstream = "wordcraft"            # REQUIRED. Which upstream repo.
upstream_names = ["WordCraft"]    # REQUIRED. Tokens rebrand() replaces (case-sensitive, whole token).
bin      = "suite-docs"           # REQUIRED. Desktop exe / web bin name.
cli      = "suite-docs-cli"
app_id   = "suite.docs"           # Wayland/X11 app id (with_app_id), single-instance names.
aumid    = "Suite.Docs"           # Windows AppUserModelID / RegisteredApplications name (Phase 4 MSI must match).
data_dir = "Suite/Docs"           # Relative to %APPDATA% / ~/Library/Application Support / $XDG_CONFIG_HOME.
storage_key = "suite-docs"        # eframe storage / localStorage key where upstream uses one (pdfcraft).
pictogram = "lines"               # lines | grid | slide | page: icon generator input.
accent_light = "#0F766E"          # Optional per-app override of [colors].
accent_dark  = "#5EC4B6"
names = { en = "Suite Docs", ru = "Suite Документы", uz-Latn = "Suite Hujjatlar", uz-Cyrl = "Suite Ҳужжатлар" }   # uz forms DRAFT

[apps.sheets]
upstream = "gridcraft"
upstream_names = ["GridCraft"]
bin = "suite-sheets"
cli = "suite-sheets-cli"
app_id = "suite.sheets"
aumid = "Suite.Sheets"
data_dir = "Suite/Sheets"
pictogram = "grid"
names = { en = "Suite Sheets", ru = "Suite Таблицы", uz-Latn = "Suite Jadvallar", uz-Cyrl = "Suite Жадваллар" }

[apps.slides]
upstream = "deckcraft"
upstream_names = ["DeckCraft"]
bin = "suite-slides"
cli = "suite-slides-cli"
app_id = "suite.slides"
aumid = "Suite.Slides"
data_dir = "Suite/Slides"
pictogram = "slide"
names = { en = "Suite Slides", ru = "Suite Презентации", uz-Latn = "Suite Taqdimotlar", uz-Cyrl = "Suite Тақдимотлар" }

[apps.pdf]
upstream = "pdfcraft"
upstream_names = ["PdfCraft"]
bin = "suite-pdf"
cli = "suite-pdf-cli"
app_id = "suite.pdf"
aumid = "Suite.Pdf"
data_dir = "Suite/Pdf"
storage_key = "suite-pdf"
pictogram = "page"
names = { en = "Suite PDF", ru = "Suite PDF", uz-Latn = "Suite PDF", uz-Cyrl = "Suite PDF" }

[colors]                          # Placeholders until the owner chooses (ADR App. B).
accent_light = "#0F766E"
accent_dark  = "#5EC4B6"
icon_fg      = "#FFFFFF"

[chrome]                          # Upstream community chrome. All false in our builds.
community      = false            # Discord pills/buttons/cards, "Join the … community" links
share          = false            # wordcraft title-bar Share button (it opens Export; see §4.1)
zotero         = false            # wordcraft Zotero tab and panel (loopback 127.0.0.1:23116 on use)
update_check   = false            # pdfcraft Help › Check for updates (GitHub API)
upstream_links = false            # getartcraft.com, github.com/storytold/*, "Learn More"
contributors_tab = true           # About › Contributors/Models (upstream credits; attribution, kept; O2)

[links]                           # Our own links. Empty string = the item is hidden.
website = ""
help    = ""
source  = ""

[i18n]                            # Consumed by WP9 (see i18n.md §5).
locales  = ["en", "ru", "uz-Latn", "uz-Cyrl"]
default  = "auto"                 # "auto" = upstream behaviour (follow the system, else en). O1 in i18n.md.
mode     = "review"               # review = reviewed + draft rows; release = reviewed only.

[fonts]                           # Consumed by WP8 (see font.md).
fallback = ["fallback/NotoSans-Regular-uz.ttf", "fallback/NotoSansMono-Regular-uz.ttf"]

[file_metadata]                   # Consumed by 0102-<app>-file-metadata (O3).
application = "{product.name}"    # Writer "Application"/"Producer" string; "{app.names.en}" also allowed.
```

Validation rules (`office-brand`, run by `cargo xt brand check` and by every `overlay` call):
1. Required keys present; unknown keys are an error (catches typos such as `comunity`).
2. `product.name` and every `names.*` must not contain any `upstream_names` token, `ArtCraft`, `Craft` as a word, or `storytold`.
3. Colours must be `#RRGGBB`. The accent must have contrast ≥ 4.5:1 against `icon_fg` (WCAG AA for the icon pictogram) and against the theme's panel colour.
4. `attribution.*` must mention every `upstream_names` token of the apps (attribution completeness) and must not contain a URL (plain text only, B3).
5. `app_id`, `aumid`, `bin` and `data_dir` must be unique across apps.
6. Uzbek values are drafts until the owner reviews them. Their status lives **in brand.toml itself** (`[review] names_uz-Latn = "draft"`, `attribution_uz-Cyrl = "reviewed:2026-10-20"`), so the working name never leaves this file (Rule W, §7.1). `brand check --release` fails while any `uz-*` value is not `reviewed`. `cargo xt i18n review-export` adds these rows to batch 0 (i18n.md §6) by reading brand.toml, and `review-import` writes the verdicts back with `toml_edit`, so the format and comments are preserved.

---

## 3. The overlay contract (frozen)

### 3.1 What `cargo xt overlay <app>` writes

`$OFFICE_OVERLAY_ROOT/<app>/` (outside every tree; `E:/office-target/p1/overlay/<app>/` locally):

| File | Content |
|---|---|
| `overlay.env` | `KEY=value` lines, UTF-8, `\n`-escaped. Parsed by a 20-line function in each `build.rs`, so no dependency is added. |
| `icon-256.png`, `icon.ico`, `icon-document.ico` (pdf only), `favicon.ico`, `icon-192.png`, `icon-512.png`, `icon-maskable-512.png` | §5 |
| `catalog-<code>.tsv` (wordcraft, pdfcraft, deckcraft), `catalog-<code>.tsv` rendered for gridcraft's build-time `match` | i18n.md §4 |
| `dates-<code>.tsv` (pdfcraft only) | i18n.md §2.4 |
| `fallback-sans.ttf`, `fallback-mono.ttf` | font.md §3 |

`overlay.env` keys (the complete list; adding a key is a contract change and needs a new WP7a revision):
```
OVERLAY_SCHEMA=1
PRODUCT_NAME=Suite                    APP_NAME=Suite Docs                  APP_ID=suite.docs
APP_NAME_en=… APP_NAME_ru=… APP_NAME_uz-latn=… APP_NAME_uz-cyrl=…        (lowercase codes, as the apps use)
UPSTREAM_NAMES=WordCraft              AUMID=Suite.Docs                     DATA_DIR=Suite/Docs
STORAGE_KEY=suite-docs                VERSION=0.1.0                        VENDOR=… COPYRIGHT=…
ATTRIBUTION_en=… ATTRIBUTION_ru=… ATTRIBUTION_uz-latn=… ATTRIBUTION_uz-cyrl=…
ACCENT_LIGHT=0F766E                   ACCENT_DARK=5EC4B6
COMMUNITY=0  SHARE=0  ZOTERO=0  UPDATE_CHECK=0  UPSTREAM_LINKS=0  CONTRIBUTORS_TAB=1
LINK_WEBSITE=  LINK_HELP=  LINK_SOURCE=
ICON_PNG=icon-256.png
CATALOGS=uz-latn:catalog-uz-latn.tsv,uz-cyrl:catalog-uz-cyrl.tsv     (ru/en overrides only if non-empty)
DEFAULT_LANGUAGE=auto
FONTS=fallback-sans.ttf,fallback-mono.ttf
FILE_APPLICATION=Suite
```

### 3.2 The generated module `$OUT_DIR/office_overlay_gen.rs`

Written by the hook's `build.rs` part. It emits `cargo::rerun-if-env-changed=OFFICE_OVERLAY_DIR` and `cargo::rerun-if-changed=<each overlay file>`. **When `OFFICE_OVERLAY_DIR` is unset, every value is upstream's own** (upstream name, all flags `true`, no catalogs, no fonts, upstream icon bytes are not touched: `ICON_PNG` is `None`).

```rust
// GENERATED by build.rs from $OFFICE_OVERLAY_DIR/overlay.env. Do not edit.
pub const ACTIVE: bool = true;
pub const PRODUCT_NAME: &str = "Suite";
pub const UPSTREAM_NAMES: &[&str] = &["WordCraft"];
pub const APP_NAMES: &[(&str, &str)] = &[("en", "Suite Docs"), ("ru", "…"), ("uz-latn", "…"), ("uz-cyrl", "…")];
pub const APP_ID: &str = "suite.docs";
pub const AUMID: &str = "Suite.Docs";
pub const DATA_DIR: &str = "Suite/Docs";
pub const STORAGE_KEY: &str = "suite-docs";
pub const ATTRIBUTION: &[(&str, &str)] = &[("en", "Based on …"), /* … */];
pub const ACCENT_LIGHT: [u8; 3] = [0x0F, 0x76, 0x6E];
pub const ACCENT_DARK: [u8; 3] = [0x5E, 0xC4, 0xB6];
pub const COMMUNITY: bool = false;
pub const SHARE: bool = false;
pub const ZOTERO: bool = false;
pub const UPDATE_CHECK: bool = false;
pub const UPSTREAM_LINKS: bool = false;
pub const CONTRIBUTORS_TAB: bool = true;
pub const LINK_WEBSITE: &str = "";
pub static ICON_PNG: Option<&[u8]> = Some(include_bytes!("E:/office-target/p1/overlay/docs/icon-256.png"));
pub const CATALOG_UZ_LATN: &str = include_str!("…/catalog-uz-latn.tsv");   // "" when absent; a const, so
pub const CATALOG_UZ_CYRL: &str = include_str!("…/catalog-uz-cyrl.tsv");   // static LANGUAGES can use it
pub const CATALOG_RU: &str = "";                                           // deckcraft only (no upstream ru)
pub const EXTRA_LANGS: usize = 2;                                          // 0 when no uz catalogs (matches cfg office_uz)
pub static CATALOGS: &[(&str, &str)] = &[("uz-latn", CATALOG_UZ_LATN), ("uz-cyrl", CATALOG_UZ_CYRL)];
pub const DEFAULT_LANGUAGE: &str = "auto";
pub static FALLBACK_FONTS: &[(&str, &[u8])] = &[("office-fallback-sans", include_bytes!("…")), ("office-fallback-mono", include_bytes!("…"))];
pub const FILE_APPLICATION: &str = "Suite";
```

The build script also emits two cfgs (each with its `cargo::rustc-check-cfg`):
- `office_overlay` whenever `OFFICE_OVERLAY_DIR` is set. It is used **only** where the code must not even be compiled in our builds: pdfcraft's ArtCraft-mark `include_bytes!` (§4.4).
- `office_uz` when `CATALOGS` names uz locales. The language registries use it to add their entries (i18n.md §3), so a pristine-like build has exactly upstream's language list.

Everything else is a plain `if office_overlay::FLAG` on a `const`, which keeps both paths type-checked in every build.

### 3.3 The hand-written module `crates/ui-egui/src/office_overlay.rs` (new file, one per app)

New files never conflict on rebase, so all logic lives here. Its API is the same in all four apps:

```rust
include!(concat!(env!("OUT_DIR"), "/office_overlay_gen.rs"));

/// The app's display name in the current interface language (falls back to en).
pub fn app_name() -> &'static str;
/// The attribution line in the current interface language (falls back to en).
pub fn attribution() -> &'static str;
/// Replace every UPSTREAM_NAMES token by app_name(); drop known upstream "Part of …" sentences.
/// Returns `s` itself (no allocation) when nothing matches; otherwise an interned &'static str
/// (bounded: one entry per distinct (language, source) pair, about 30 per app).
pub fn rebrand<'a>(s: &'a str) -> &'a str;
/// Accent colours as egui colours.
pub fn accent(dark: bool) -> egui::Color32;
/// The window icon (overlay icon, else None so the caller keeps upstream's).
pub fn icon_png() -> Option<&'static [u8]>;
```

Rules for `rebrand`:
- Token replacement is whole-token and case-sensitive (`WordCraft`, not `wordcraft`, which is an identifier shape).
- These upstream sentences are **removed** (replaced by the empty string, and the caller hides an empty label): "Part of the Crafting Apps from the ArtCraft team.", "Part of ArtCraft.", "Join the ArtCraft community on Discord", "Join the ArtCraft Discord", "Join us on Discord: discord.gg/artcraft", "Join our community on Discord", and the copyright lines "… ArtCraft Team and the … contributors." (upstream copyright moves to our NOTICE; §8).
- `rebrand` never touches strings without a match, so ru and other translations pass through unchanged.
- The interning cache is a `Mutex<HashMap<(lang, &'static str), &'static str>>`. Keys come from the catalogs, so the set is bounded. The cost on a miss is one `memchr`-based `contains` per upstream token. That is negligible next to egui text layout, and WP7c measures it with a kittest frame-time comparison.
- Current language: wordcraft and pdfcraft read `crate::i18n::current().code()`. gridcraft's language lives in egui context data (`i18n.rs:166-174`), so its hook adds one line in `set_current` (`i18n.rs:172`) that mirrors the code into a thread-local inside `office_overlay`. deckcraft uses the new mechanism's own `current()` (i18n.md §2.5).

### 3.4 The `build.rs` hook

All four `crates/ui-egui/build.rs` exist today (105 lines each; they generate the credits). The hook adds **two lines** to each:
```rust
#[path = "build_office.rs"] mod build_office;   // top of file
build_office::run();                            // first line of main()
```
plus the new file `crates/ui-egui/build_office.rs` (about 120 lines: env parse, `include_bytes!` path rendering with forward slashes and `\\` escaping, cfg emission, upstream defaults).

Crates below ui-egui (wordcraft `control-key`, deckcraft `engine` recovery/spell paths, pdfcraft `engine` dates, writer crates for 0102) must not depend on ui-egui (`xtask layers`). They use `option_env!("OFFICE_DATA_DIR")` / `option_env!("OFFICE_PRODUCT_NAME")` in a `const` `match` with the upstream value as fallback. rustc tracks `option_env!` for rebuilds. `scripts/build.sh` exports those variables from `overlay.env`.

App crates (`apps/<app>/src/main.rs`) reach the overlay through `<app>_ui_egui::office_overlay::*`, because ui-egui is already their dependency.

### 3.5 Patch layout per app

| Patch | Content | Droppable alone? |
|---|---|---|
| `0100-<app>-office-overlay-hook` | `build_office.rs`, `office_overlay.rs`, `mod office_overlay;` in `lib.rs`, the 2-line `build.rs` hook, the choke-point `rebrand`, name/title/icon/data-dir/accent one-token edits, `office_overlay_tests.rs` | No: it is the base. |
| `0101-<app>-community-chrome-off` | Gates on `COMMUNITY`, `SHARE`, `ZOTERO`, `UPDATE_CHECK`, `UPSTREAM_LINKS`; removing pdfcraft's ArtCraft marks; replacing deckcraft's painted logo | Yes: when it is dropped, the chrome comes back, but pdfcraft then **violates the brand licence** (B2). REGISTER.md marks it "must carry". |
| `0102-<app>-file-metadata` | Writer `Application`/`Producer`/generator strings via `option_env!("OFFICE_PRODUCT_NAME")` | Yes (O3). |
| `0110-<app>-office-languages` (WP9) | i18n.md | Yes. |

Patch size budget: 0100 at most 150 changed lines in existing upstream files (new files do not count). Above that, split it.

### 3.6 What stays upstream (identifiers, not brand)

The following are **kept** and listed in `brand/allowlist.txt`, each with a reason:
- crate names `wordcraft-*` and others, module paths, `WORDCRAFT_*`/`GRIDCRAFT_*`/`DECKCRAFT_*`/`PDFCRAFT_*` env var names, log targets (`gridcraft*=info`), thread names;
- egui widget ids (`gridcraft.formula_bar`), web DOM ids (`wordcraft_canvas`);
- the web embedding wire protocol `wordcraft:ready|save|dirty|…`, `gridcraft:*`, `pdfcraft:*`. These are an API contract that the HRM docserver will speak (Phase 5), and renaming them is a separate decision;
- file-format ids: `.deckcraft` extension, `application/vnd.storyteller.deckcraft+zip`, deckcraft theme id `"DeckCraft"` (saved in files; only the *display* name is rebranded), pdfcraft `%PdfCraft` content-stream markers (`edit/src/lib.rs:329-330`, needed to recognise its own edits in saved PDFs);
- MCP server ids (`"name":"wordcraft"`). The MCP *instructions text* carries Discord/website URLs (`wordcraft/crates/mcp/src/server.rs:138`, `deckcraft/crates/mcp/src/server.rs:143-145`, `pdfcraft/crates/automation/src/mcp.rs:29,134`); 0101 empties them when `!COMMUNITY`, because an agent may repeat them to a user;
- CLI `--help`/`--version` text. It prints Discord/website lines (`pdfcraft-cli/src/main.rs:82,161-163`; `deckcraft-cli/src/main.rs:76-78,101`; `help.rs:153-154`). These are gated in 0101 at priority P2. The CLI binary name is ours (domain `bin`).

---

## 4. Per-app inventory (old BASE; `file:line`; action)

Action legend: **N** = `office_overlay::app_name()` (one token); **R** = through `rebrand` (covered by the choke point unless marked "explicit"); **G:FLAG** = gated on that overlay flag in 0101; **I** = overlay icon; **D** = overlay data dir/key; **A** = accent; **ours** = file owned by our domain build (D-A), no patch; **keep** = identifier (§3.6); **P4** = packaging, not built in Phase 1 (our own MSI/manifests in Phase 4).

### 4.1 wordcraft (hot files: `lib.rs` 6764, `dialogs.rs` 4505, `ribbon.rs` 2990, `chrome.rs` 667 lines)

| Item | Where | Action |
|---|---|---|
| Window title at creation | `apps/wordcraft/src/main.rs:374` `.with_title("WordCraft")` | N |
| App id | `apps/wordcraft/src/main.rs:375` `.with_app_id("ai.storyteller.wordcraft")` | `APP_ID` |
| eframe app name | `apps/wordcraft/src/main.rs:414` `run_native("WordCraft", …)` | N (`PRODUCT_NAME`-derived, stable across languages: use `APP_NAMES[en]`) |
| Runtime window title | `crates/ui-egui/src/lib.rs:2038` `"{stem}{•} - WordCraft"`, `:2040` `"WordCraft"` | N (explicit; two tokens) |
| Title-bar text without a document | `crates/ui-egui/src/chrome.rs:350` | N |
| Window icon | `apps/wordcraft/src/main.rs:260` (macOS 512), `:262` (hicolor 256) | I: `icon_png().unwrap_or(include_bytes!(…))` |
| Exe icon + VERSIONINFO | `apps/wordcraft/build.rs:16-22` (ProductName, FileDescription, CompanyName "Learning Machines LLC", LegalCopyright) | **ours** (domain `build.rs` from overlay) |
| About title | `crates/ui-egui/src/dialogs.rs:1266` "About WordCraft" | R (title passes through the dialog-title lookup) |
| Save-changes message title | `dialogs.rs:1267` `"WordCraft"` | N |
| About heading | `dialogs.rs:2383` `tl!("WordCraft")` | R |
| About blurb with "Part of the Crafting Apps from the ArtCraft team" | `dialogs.rs:2386-2388` | R (sentence removed) + explicit `attribution()` label added after it |
| About links: website, Discord, GitHub | `dialogs.rs:2390`, `:2391`, `:2392` | G:UPSTREAM_LINKS / G:COMMUNITY (0101); our `[links]` shown instead when non-empty |
| About copyright "ArtCraft Team and the WordCraft contributors" | `dialogs.rs:2395` | Replaced by `COPYRIGHT` (0101). The upstream copyright is kept in our NOTICE (§8). |
| About tabs Contributors/Models | `dialogs.rs:2397-2400`; `credits.rs:263,286` (`github.com/{login}` links) | keep (attribution; G:CONTRIBUTORS_TAB) |
| Discord pill | `chrome.rs:296` `if app.ui.show_discord {` … `:306-308` | G:COMMUNITY: `if office_overlay::COMMUNITY && app.ui.show_discord` |
| Discord user pref | `lib.rs:260` field, `lib.rs:355` default `true` | keep (the gate makes it inert) |
| Options checkbox "Show the community button in the title bar" | `options.rs:193` | G:COMMUNITY (hide) |
| `ui.discord` command | `lib.rs:1496-1497` (opens `https://discord.gg/artcraft`), allowed list `:2394` | G:COMMUNITY: no-op + status message (the control channel can still name it) |
| Help ribbon "Community Discord" | `ribbon.rs:2006` | G:COMMUNITY |
| Backstage "Discord community" | `backstage.rs:121`, `:129-130` | G:COMMUNITY |
| Keytip "Community Discord" | `keytips.rs:342` | G:COMMUNITY |
| Share button (opens Export; not social) | `ribbon.rs:228-245` | G:SHARE (flag kept for parity with the plan; owner may set `share = true`, O2) |
| Zotero tab dispatch / panel | `ribbon.rs:371`, `:1734`; panel `zotero.rs:228`; tab list in engine `crates/engine/src/ribbon.rs:19` | G:ZOTERO, filtered at render in ui-egui (no engine edit) |
| Status texts with the name (`tl!`) | `lib.rs:1563,1567,1572`; `options.rs:199,201,393,688,690,692,698`; `backstage.rs:375,376`; `chrome.rs:307,455`; `dialogs.rs:2552`; `file_browser.rs:331` | R (choke point) |
| Raw (non-`tl!`) strings | `apps/wordcraft/src/print.rs:43` print-job title; `clipboard.rs:102`; `default_apps.rs:109`; `lib.rs:1492,1559`; `zotero.rs:92,110,173` | R explicit (P2 for print/clipboard) |
| Default-apps registration | `apps/wordcraft/src/default_apps.rs:87` `registeredAppMachine=WordCraft`; `:35` desktop id | `AUMID` / `APP_ID` (Phase 4 MSI must register the same name) |
| Settings, control key, ui.json, logs | `crates/control-key/src/lib.rs:7-22` (`%APPDATA%\WordCraft`, macOS, XDG) | D via `option_env!("OFFICE_DATA_DIR")` |
| Dictionaries dir | `crates/proof/src/hunspell.rs:60` `%LOCALAPPDATA%\WordCraft\dictionaries` | D (`option_env!`) |
| Log file names | `apps/wordcraft/src/logging.rs:33,123` | keep (file names inside our data dir) |
| Accent | `crates/ui-egui/src/theme.rs:8` `APP_COLOR` (0x3B5BDB), used `:299` and `backstage.rs:6,36`, `file_browser.rs:15,445` | A: `APP_COLOR` becomes `office_overlay::accent(false)`. Note the guard test `theme.rs:813-817` bans literal colour forms. The const must stay a `const`, so the generated module provides `ACCENT_LIGHT` as `Color32::from_rgb(…)` const. |
| Web start error | `apps/wordcraft-web/src/web.rs:108` | R explicit |
| Web title, favicon, loading text | `apps/wordcraft-web/index.html` (`<title>`, favicon `wordcraft.ico`, "Loading WordCraft…") | **ours** (`index.html.in`) |
| File metadata | `docx/src/write/mod.rs:1267` `<Application>WordCraft`; `pdf/src/lib.rs:366` creator/producer; `formats/src/html.rs:1587`, `odt.rs:795`, `rtf.rs:508`, `latex.rs:55` (also writes getartcraft URL); `engine/src/session.rs:611` default author "WordCraft User"; `engine/src/sample.rs:33,115` | 0102 (O3) |
| MCP instructions with community URLs | `crates/mcp/src/server.rs:136,138` | G:COMMUNITY (P2) |
| Packaging | `packaging/windows/wordcraft.wxs` (20-158: Name, Manufacturer, ARP URLs `:36-38` incl. Discord help link, UpgradeCode/GUIDs `:69,86`), `macos/Info.plist.in`, `linux/*.desktop`, `metainfo.xml.in:21-24,38`, `nfpm.yaml`, flatpak, `sign.ps1:58` | P4 (our own suite MSI; new GUIDs) |
| Tests that assert brand | `lib.rs:2702` (title "WordCraft"), `lib.rs:4523`, `options.rs:1027,1030`, `theme.rs:813-817`, `i18n/audit.rs:175` (allow-list "WordCraft"/"Discord"), `formats/src/tests.rs:252`, `engine/src/tests.rs:2144`, `control-key/src/lib.rs:84`, `apps/wordcraft/tests/help.rs:36` | Pass with the overlay **unset** (pristine-like). With it set: quarantine class `brand-overlay` (PLAN §6.5), each listed. |

Network (wordcraft): **none outbound.** Loopback only: control server `apps/wordcraft/src/control_server.rs:40` (opt-in `--control`), MCP bridge `crates/mcp/src/backend.rs:113` (opt-in), Zotero client `crates/zotero/src/client.rs:91` → `127.0.0.1:23116` (on user action; gated off by `ZOTERO`). Keep the control server and MCP: they are the HRM integration path.

### 4.2 gridcraft (hot files: `lib.rs` 1261, `dialogs.rs` 2741, `ribbon.rs` 2516, `backstage.rs` 1088)

| Item | Where | Action |
|---|---|---|
| Window title, app id, eframe name | `apps/gridcraft/src/main.rs:312`, `:315` (`ai.storyteller.gridcraft`), `:347` | N, `APP_ID` |
| `--version`, log line | `main.rs:255`, `:284` | R explicit (P2) |
| Window icon | `main.rs:225` (macOS 512), `:227` (hicolor 256), decoded `:223-230` | I |
| Exe icon + VERSIONINFO | `apps/gridcraft/build.rs:10,16-22` | **ours** |
| About title / heading / blurb | `crates/ui-egui/src/dialogs.rs:677` "About GridCraft"; `:1486` heading; `:1489` "…Part of ArtCraft." | R explicit (gridcraft's English path skips `tr`) + attribution label |
| About links | `dialogs.rs:1490` (getartcraft), `:1491` (Discord) | G:UPSTREAM_LINKS / G:COMMUNITY |
| Ribbon "About\nGridCraft" button | `ribbon.rs:2261` | R (choke point `Language::tr`, `i18n.rs:119-131`; English labels go through `rsplit_once` at `:131`, so the hook applies `rebrand` to the final value of `tr`) |
| Start dialog title | `dialogs.rs:779` | N |
| Message-box titles (19 sites, first element of `app.message = Some((title, text))`) | `lib.rs:541,692,769,912,924,946,1078`; `backstage.rs:891,897,905,924`; `grid.rs:2272,2293`; `formula_bar.rs:130`; `ribbon.rs:1442` | N at each site (one token). Alternative (fewer hunks): rebrand the title where the modal draws it (one site). **Choose the draw site** (`dialogs.rs`, the message modal) and leave the 19 producers untouched. |
| Status/body strings | `lib.rs:804`; `dialogs.rs:1066-1068` (agent help, shows `gridcraft --control` CLI names), `:1500`; `ribbon.rs:2251` | R |
| Native Help menu (desktop) | `apps/gridcraft/src/native_menu.rs:177` (website), `:178` (Discord); app submenu `:204-205` "GridCraft"/"About GridCraft" | G:UPSTREAM_LINKS/G:COMMUNITY on the two tuples; N on `:204-205` |
| Config/log/data dirs | `apps/gridcraft/src/main.rs:76-87` (`%APPDATA%\GridCraft`, macOS, XDG), `:96,109,117` | D |
| Accent | `crates/ui-egui/src/theme.rs:65-67` (light accent/dark/soft #107C41…), `:78-81` (header/selection greens), `:107-109` (dark); hyperlink `:694` | A (light and dark sets; the soft and selection tints are derived from the accent by the same lightness steps upstream uses) |
| Ribbon "Share" (Export / save a copy) | `ribbon.rs:132-140`; backstage `Icon::Share` `backstage.rs:85` | **keep** (not social chrome) |
| Web start error | `apps/gridcraft-web/src/web.rs:78` | R explicit |
| Web index | `apps/gridcraft-web/index.html:6,7,18-19` | **ours** |
| Theme named "Craft" (Office-theme default) | `crates/model/src/style.rs:128`, `crates/xlsx/src/theme.rs:73,89`, `engine/src/cmd/view.rs:108,366,377` | keep in Phase 1 (allowlisted; a theme name, saved in files). Owner may rename the *display* later. |
| File metadata | `xlsx/src/write.rs:430` app.xml `Application`; `xlsx/src/pivot.rs:686` `refreshedBy`; `pdf/src/lib.rs:788-789` Creator/Producer; `functions/src/info.rs:106` `INFO("osversion")`; `engine/src/lib.rs:218` "GridCraft User"; `engine/src/cmd/solver.rs:1703`, `whatif.rs:406` (report sheet names); `print.rs:42` | 0102 (O3) |
| Packaging | `packaging/windows/gridcraft.wxs:17-35` (ARP URLs `:33-35`, Discord help link), `macos/Info.plist.in`, `linux/*` metainfo `:27-30` | P4 |
| Tests asserting brand | `apps/gridcraft/tests/mac_menu_appkit.rs:27`; `main.rs:404-411`; `engine/src/cmd/solver.rs:2336`; `crates/mcp/src/server.rs:157` | quarantine `brand-overlay` when the overlay is set |

Network (gridcraft): **none outbound** (no HTTP crate; `WEBSERVICE` returns `#VALUE!`, `crates/functions/src/info.rs:422`). Loopback opt-in only: `apps/gridcraft/src/control_server.rs:15`, `crates/mcp/src/backend.rs:48`.

### 4.3 deckcraft (hot files: `ribbon.rs` 3619, `dialogs.rs` 3631, `lib.rs` 3359, `backstage.rs` 1382)

deckcraft has **no i18n mechanism**, so there is no choke point until WP9's 0110 adds one. Order matters: **0100 for deckcraft introduces `office_overlay::rebrand` and calls it at each literal site**; 0110 later routes the same sites through `t()`, whose lookup then calls `rebrand`, which removes the per-site calls again. To avoid that churn, deckcraft's 0100 and 0110 are **written together** (§9), and 0100 only covers what 0110 does not wrap.

| Item | Where | Action |
|---|---|---|
| Window title, eframe name, app id | `apps/deckcraft/src/main.rs:200`, `:235`, `APP_ID :127` (applied `:210`) | N, `APP_ID` |
| File-dialog filter label | `main.rs:107` `add_filter("DeckCraft", ["deckcraft"])` | R explicit (label only; extension kept) |
| Title-bar fallback | `crates/ui-egui/src/ribbon.rs:79` | N |
| Presenter view title | `crates/ui-egui/src/show.rs:1757` | R explicit |
| About command label | `lib.rs:395` (`UI_COMMANDS`), `menus.rs:292` | R (at display: the palette and menu builders) |
| About dialog title / default titles | `dialogs.rs:50`, `:71`, `:74` | N |
| About body | `dialogs.rs:2609` `paint_logo`, `:2610` heading, `:2612-2613` blurbs, `:2616-2618` links, `:2620` copyright | heading N; logo → overlay icon image (0101); links G:COMMUNITY/UPSTREAM_LINKS; copyright → `COPYRIGHT`; attribution label added |
| Painted DeckCraft logo | `dialogs.rs:2640-2653` (`paint_logo`, F26B1D/C24E14), used `backstage.rs:693,1358`, `dialogs.rs:2609` | 0101: draw `icon_png()` instead (upstream code-art is MIT, but it is DeckCraft's identity) |
| Backstage Home header + Discord | `backstage.rs:693-695`, `:737` | N; G:COMMUNITY |
| Backstage Account page | `backstage.rs:1351`, `:1358-1359`, `:1367-1368`, `:1374-1375` | R; N; G:COMMUNITY/UPSTREAM_LINKS |
| Slide Show "Learn More" (opens getartcraft) | `ribbon.rs:2118` | G:UPSTREAM_LINKS (hide the button) |
| `app.links` command | `lib.rs:1052` | G:UPSTREAM_LINKS (returns `{}`) |
| Link constants | `crates/engine/src/links.rs:3-8` | keep (engine untouched; all UI uses gated) |
| Print-through-PDF text | `backstage.rs:1043,1045` | R |
| Error/status strings | `lib.rs:194,747`; `dialogs.rs:3220,3235`; `engine/src/cmd/file.rs:146,162`; `cmd/format.rs:1215`; `crates/format/src/lib.rs:37,155` | R (ui-egui sites); engine messages R explicit at the toast/alert draw site (one site, like gridcraft) |
| Built-in theme "DeckCraft" | display: `backstage.rs:540`, `dialogs.rs:277,285,468,485`; id: `crates/model/src/theme.rs:192,194` | R of the display name only; id kept (saved in files) |
| Native format label | `backstage.rs:216` "DeckCraft Presentation (*.deckcraft)" | R; extension kept |
| Settings/log/recovery/dictionary dirs | `apps/deckcraft/src/main.rs:57-67`; `engine/src/recovery.rs:29,32`; `engine/src/spell.rs:488,492,589,591` | D (`option_env!` in engine) |
| Window icon | `apps/deckcraft/src/main.rs:165` | I |
| Exe icon + VERSIONINFO | `apps/deckcraft/build.rs:16-22` | **ours** |
| Accent | `crates/ui-egui/src/theme.rs:63,65,69` (light), `:91,93,97` (dark); derived `:125-127` | A |
| Web start error / index | `apps/deckcraft-web/src/web.rs:59`; `index.html:6,8,18` | R explicit; **ours** |
| Sample deck "made in DeckCraft" | `engine/src/sample.rs:24,117` | 0102 |
| File metadata | `pdf/src/lib.rs:144-145`; `pptx/src/write/mod.rs:1255` `Application`; `pptx/src/tables.rs:229` table style names "DeckCraft {n}"; `format/src/lib.rs:68` generator | 0102 (table style names: keep, they are style ids in pptx) |
| Packaging | `packaging/windows/deckcraft.wxs:15-72` (ARP URLs `:31-33`), `macos/Info.plist.in`, `linux/*` | P4 |
| Tests asserting brand | `backstage_tests.rs:169,457`; `effects_tests.rs:30,38`; `theme_colors_tests.rs:133`; `model/src/tests.rs:14`; `pdf/src/tests.rs:96-97`; `mcp/src/tests.rs:26` | most use the theme **id** (kept), so they pass; the rest go to quarantine `brand-overlay` |

Network (deckcraft): **none outbound** (no HTTP crate, no web fetch feature). Loopback opt-in: `apps/deckcraft/src/control_server.rs:49`, `crates/mcp/src/backend.rs:47`.

### 4.4 pdfcraft (hot files: `lib.rs` 3185, `dialogs.rs` 1750, `chrome.rs` 904, `widgets.rs` 829, `home.rs` 258)

| Item | Where | Action |
|---|---|---|
| Window title at creation | `apps/pdfcraft/src/main.rs:310` | N |
| Per-document title | `crates/ui-egui/src/lib.rs:3091` `"PdfCraft"` / `"{name} — PdfCraft"` | N (two tokens) |
| APP_ID | `apps/pdfcraft/src/main.rs:49` (icon path `:56`) | `APP_ID` |
| eframe names / macOS app menu | `main.rs:249`, `:267`, `:397` | N |
| Software-renderer notice | `main.rs:459` `notify_tr("PdfCraft is drawing…")` | R (choke point) |
| Window icon | `main.rs:54` (macOS 1024), `:56` (hicolor 256) | I |
| Exe icons + VERSIONINFO | `apps/pdfcraft/build.rs:22-29` (app icon + document icon as resource 2); `apps/pdfcraft-cli/build.rs:16-20` | **ours** |
| **ArtCraft logo/mark (trademark, compiled in)** | `crates/ui-egui/src/widgets.rs:503-511` `artcraft_logo` (`include_bytes!` `:506`, `:508`), `:514-520` `artcraft_mark` (`:516`); used `dialogs.rs:1156` (About header), `dialogs.rs:1174` ("Part of" + logo), `home.rs:40` (community card) | 0101 (**must carry**): the upstream bodies stay under `#[cfg(not(office_overlay))]`; under `#[cfg(office_overlay)]` both functions are stubs that allocate no space and load no bytes, so **the `include_bytes!` is not compiled at all** in our builds (a `const false` branch would not guarantee that; verified by the binary scan, §7.2). The three call sites are also gated, so no empty gap is left. |
| About heading (raw literal, not `tl!`) | `dialogs.rs:1158` `"PdfCraft"` | N |
| About blurb / "Part of" | `dialogs.rs:1163-1164`, `:1170-1180` (incl. `community_links` `:1177`) | attribution label instead of "Part of" + logo; links G:COMMUNITY/UPSTREAM_LINKS |
| Home: welcome / recent / offline texts | `home.rs:25`, `:144`, `:210` | R |
| Home community card | `home.rs:34-56` (mark `:40`, heading `:42`, body `:43`, links `:46`) | G:COMMUNITY (whole card) |
| Top-bar Discord button (ungated, raw "Discord") | `chrome.rs:302-304`; width reservation `chrome.rs:69-70` | G:COMMUNITY (both sites, so the tab strip does not keep an empty gap) |
| `community_links` widget | `widgets.rs:524-537` (renders the whole `LINKS` table; first item via `tl!` at `:529`) | G: early return when `!COMMUNITY && !UPSTREAM_LINKS` |
| Help menu / palette entries | `crates/engine/src/commands.rs:373` (Discord), `:374` (web page), `:375` (GitHub), `:376` (ArtCraft website), `:377` (Check for updates), `:378` (About) | engine untouched; ui-egui hides the ids at render and the dispatcher no-ops them: `ui-egui/src/commands.rs:210-212` (`links::for_command`) and the `help.check_updates` arm |
| **Update check (network)** | `apps/pdfcraft/src/updates.rs:7` (`LATEST`, `ureq` 3.4, `User-Agent: PdfCraft/<ver>`), wired `main.rs:389`; UI `crates/ui-egui/src/updates.rs:15` `RELEASES_PAGE`, `:67-92` | 0101: `if office_overlay::UPDATE_CHECK { app.update_source = Some(…) }` at `main.rs:389`; hide `help.check_updates` when `!UPDATE_CHECK`, because without a source the command opens upstream's releases page (`updates.rs:69,92`). Overlay test asserts no source and no menu entry. |
| Update dialog texts | `crates/ui-egui/src/updates.rs:139,150` | R (moot while hidden) |
| Other user-visible "PdfCraft" (`tl!`) | `dialogs.rs:1067`; `lib.rs:1960,1962`; `canvas.rs:3938,4181`; `js_ui.rs:66,380`; `ocr_ui.rs:128`; `content_ui.rs:50,53`; `sign_ui.rs:1000,1002`; `fill_sign.rs:870` | R (choke point) |
| Storage dir / key | `apps/pdfcraft/src/main.rs:65,221` `eframe::storage_dir("PdfCraft")`; storage key `crates/ui-egui/src/lib.rs:2913` `"pdfcraft"` (read `main.rs:377`); legacy migration `main.rs:59,89,93` | D / `STORAGE_KEY` (keep the migration code; it simply finds nothing) |
| Other dirs | `apps/pdfcraft/src/settings.rs:23,175`; `crates/ui-egui/src/portable.rs:14,16`; `recovery.rs:154,156`; `image_pick.rs:82,84`; `single_instance.rs:162,173` | D |
| Accent | `crates/ui-egui/src/theme.rs:89-93` (light accent/text/soft/badge), `:113-117` (dark); selection `:88,112` | A |
| Web | `apps/pdfcraft-web/index.html:6` (no favicon upstream); `crates/ui-egui/src/web_print.rs:131` print-preview `<title>` | **ours** (adds a favicon); R explicit |
| File metadata | `crates/create/src/lib.rs:53` `Producer` (new documents only); `crates/sign/src/pdf.rs:1835,2008` `Prop_Build/App/Name`; `automation/src/comments.rs:15` default comment author; `edit/src/added.rs:711` "PdfCraft Shaped Text" font family; `preflight/src/icc.rs:51` | 0102 (keep the `%PdfCraft` markers, §3.6) |
| Packaging | `packaging/windows/pdfcraft.wxs:16-153` (ARP URLs `:38-40`), `installer-ui.wxs`, `sign.ps1:58`, `macos/Credits.rtf:8-9` (getartcraft + Discord), `linux/package.sh:155-156` (AppImage update info points at storytold releases) | P4. The AppImage zsync update info **is** an update channel. Our Phase 4 packaging must not copy it. |
| Tests asserting brand/links | `crates/ui-egui/tests/ui_group/links.rs:23-66`; `engine/src/links.rs:524-531`; `tests/viewer.rs:803,813,832`; `tests/accessibility.rs:94`; `tests/ui.rs:37,114,1118`; `tests/pickers.rs:577`; `tests/ui_group/updates.rs:58,102`; `organize/src/tests.rs:266`; `apps/pdfcraft-cli/tests/version_resource.rs:36-38` | quarantine `brand-overlay` with the overlay set; all pass pristine-like |
| xtask gates | `xtask/src/assets.rs:243-260` (asset needs ATTRIBUTION.toml + SHA-256); `:244` (brand licence only for `docs/brand/` logos); `xtask/src/layers.rs:30-88,185` (crate table) | Our hook adds **no asset files and no crate** to the tree (§0 B7), so both stay green. CI runs them from WP6a on (PLAN WP7 risk). |

Network (pdfcraft):
- **outbound**: the update check above (user-initiated; disabled by us).
- **web fetch**: `apps/pdfcraft-web/src/main.rs:110-120` `fetch_bytes` fetches the PDF named by the `?file=` query parameter (near `:44`). It is the embedding entry point (the host page decides the URL). Keep it, but Phase 5 (docserver) must restrict it to the same origin or an allowlist; recorded as risk R-B3.
- **loopback/IPC**: `crates/ui-egui/src/control.rs:866` (127.0.0.1, random port, token, opt-in `--control`), single-instance pipe/socket (`single_instance.rs:162,173`), MCP stdio.
- **none**: OCR model download (`crates/ocr/src/lib.rs:304-311` local only), TSA/OCSP/CRL (`sign/src/timestamp.rs:1-5`, `revocation.rs:1-3` "never fetches"), JS `submitForm` (`crates/js/src/lib.rs:502`, never sends), `launchURL` (confirmation + scheme allowlist).

---

## 5. Assets we must create (`cargo xt brand icons`, tiny-skia)

Generated, not hand-drawn: a rounded square (corner radius 22% of the side) in the app accent with a white pictogram (`lines`/`grid`/`slide`/`page`), rendered at each size from one vector description, so hinting at 16 px is controlled (pixel-snapped strokes at ≤ 32 px). All output goes to the overlay; **nothing is committed except the generator** (`brand/icons/README.md` documents it). The placeholder art is ours, MIT OR Apache-2.0.

| Asset | Sizes / format | Used by | Phase |
|---|---|---|---|
| `icon-<N>.png` per app | 16, 20, 24, 32, 40, 48, 64, 128, 256, 512, 1024 (RGBA) | window icon (256; macOS 1024 if ever built), ICO source | 1 |
| `icon.ico` per app | 16, 20, 24, 32, 40, 48, 64, 128, 256 (PNG-compressed ≥ 64; BMP < 64 for old shells) | exe resource (domain `build.rs`), taskbar | 1 |
| `icon-document.ico` (pdf) | same 9 sizes; page pictogram with a folded corner | pdfcraft resource id 2 (`apps/pdfcraft/build.rs:23`); Phase 4 file associations | 1 (placeholder) |
| `favicon.ico` per app | 16, 32, 48 | web `index.html.in` | 1 |
| `icon-192.png`, `icon-512.png`, `icon-maskable-512.png` per app | PNG; maskable with a 10% safe zone | web app manifest (A-web mobile, ADR) | 1 (manifest itself in Phase 4; icons are free) |
| `apple-touch-icon.png` per app | 180 | iOS home screen (A-web) | 1 |
| Suite mark (product-level) | SVG + 64/128 PNG | About header in all apps (replaces pdfcraft's ArtCraft mark at 40 px, deckcraft's painted logo) | 1 (reuse `icon-128.png` of the app; no separate mark until B-1) |
| `.icns` | 16–1024 | macOS | not needed (no macOS) |
| hicolor PNG tree, `.desktop`, metainfo | 16–512 | Linux packages | P4 |
| MSI dialog bitmaps | 493×312, 493×58 | WiX UI | P4 |

---

## 6. Telemetry and network: verdict and recommendation

| App | Outbound calls in shipped code | Recommendation |
|---|---|---|
| wordcraft | none | nothing to disable; Zotero loopback hidden with the Zotero chrome |
| gridcraft | none | nothing to disable |
| deckcraft | none | nothing to disable |
| pdfcraft | user-initiated GitHub release check (`ureq`, sends only `User-Agent: PdfCraft/<ver>`); web `?file=` fetch (host-driven) | **Disable the update check** (0101; `UPDATE_CHECK=false`): it would advertise upstream builds to our users, and it reveals the upstream version to GitHub. Keep the web fetch (embedding), harden in Phase 5. |

No analytics, crash-report upload, licence check, font/model download or "ping" exists in any app (inventory scans, each category checked explicitly). Every URL that opens does so only on a user click, through the OS browser.

Proof in CI (§7.2): the release binaries and wasm contain no `api.github.com`, `discord.gg`, `getartcraft` or `github.com/storytold` strings. Because `UPDATE_CHECK` is a `const false`, the optimiser removes `updates::latest_release` and its URL; the string scan proves it.

---

## 7. CI brand-leak check

### 7.1 Source scan: `cargo xt brand-grep` (job `brand`, seconds, offline)

- **Scope**: the *patched* trees `upstream/{wordcraft,gridcraft,deckcraft,pdfcraft}/{apps,crates}/**/*.rs`, plus `domains/**` and our `crates/**`, `i18n/**`, `brand/` (except `brand.toml`).
- **Lexing**: a small Rust string-literal lexer (normal, byte, raw `r#"…"#` strings, escapes), so code identifiers and comments are not scanned. `#[cfg(test)]` modules, `tests/` dirs and `*_tests.rs` are skipped (tests may assert upstream text).
- **Rule U (upstream brand)**: a literal matches `(?i)\b(word|grid|deck|pdf|art|print|photo)craft\b|storytold|getartcraft|discord\.gg|storyteller\.ai|learning machines`. A hit passes only if:
  - (a) the same expression routes it through `office_overlay::` or a choke-point function patched to call `rebrand` (the scanner knows the four choke points by file and function name), or
  - (b) it is identifier-shaped (`^[a-z0-9_.:\-/]+$`, no spaces, e.g. `gridcraft.formula_bar`, `wordcraft:save`, `.deckcraft`), or
  - (c) it is inside `log::*!`/`tracing::*!`/`panic!`/`expect(` (not user-visible), or
  - (d) it matches an entry in `brand/allowlist.txt`: `path-glob  literal-regex  # reason` (no line numbers, so it survives syncs).
- **Rule W (working name)**: reads `product.name` and every `names.*` from `brand.toml` and fails on **any** occurrence (case-sensitive, whole word, in literals *and* comments) in `patches/**`, `domains/**`, `crates/**`, `i18n/**`, `scripts/**`, `xtask/**`. This enforces "never hard-coded". Docs and the generated review exchange files `i18n/review/**` (which carry brand.toml's names to the owner) are excluded. Our catalogs must not contain the name either: translations keep the upstream token (`WordCraft haqida`) and `rebrand` replaces it at runtime (i18n.md §4.3).
- **Rule M (marks)**: fails on any `include_bytes!`/`include_str!`/`Image::from_bytes`/`include_image!` whose path contains `docs/brand/`, or any `artcraft_logo`/`artcraft_mark`/`paint_logo` call reachable outside `#[cfg(test)]` while `COMMUNITY=0`. The last check is textual: the call must sit under a gate.
- **Output**: `brand-grep: N literals scanned, K allowed (a:…, b:…, c:…, d:…), 0 unallowlisted hits`. Each hit prints `path:line: "literal"  [rule]`.

### 7.2 Binary scan: `cargo xt brand-scan-bin` (job `brand`, after `build`)

On each release exe and each `*_bg.wasm`:
- **zero** occurrences of `discord.gg`, `getartcraft`, `github.com/storytold`, `api.github.com/repos/storytold`, `storyteller.ai`;
- **zero** occurrences of a 48-byte signature of each `docs/brand/*.svg` (proves no mark is embedded);
- report (not fail) the counts of `WordCraft|GridCraft|DeckCraft|PdfCraft|ArtCraft`. They are expected: catalog keys and the attribution line. A count that rises by more than 10% over the previous run is flagged.

### 7.3 Runtime: `office_overlay_tests` (kittest, in each 0100)

- Open the main window, the About dialog, Home/Backstage, the Help ribbon/menu and the command palette. Walk the accesskit tree and assert that no label or value matches Rule U, except the attribution label (exact match against `attribution()`).
- Assert the window title equals `app_name()`, or `"<doc> - <app_name()>"`.
- pdfcraft: assert `update_source.is_none()`, and that `help.check_updates` is in neither the menu nor the palette.
- Snapshot PNGs to `$OFFICE_EVIDENCE_DIR` when set (8 evidence PNGs, PLAN WP7 done-check).

---

## 8. Attribution, LICENSE and NOTICE obligations

- **Upstream files stay untouched**: `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md` (and pdfcraft `ATTRIBUTION.toml`) in every snapshot. They are pristine by construction (PLAN F12).
- **Our root files (WP7g)**: `LICENSE-MIT`, `LICENSE-APACHE` (our layer), and `NOTICE`, which must:
  - reproduce each upstream copyright line exactly ("Copyright (c) 2026 ArtCraft Team and the WordCraft contributors." and likewise for GridCraft, DeckCraft, PdfCraft, plus craft-libs). Both MIT and Apache-2.0 §4(c) require the notices to be kept with copies;
  - state that the product is based on those apps, in the plain-text form the brand licence permits (B3), and that "ArtCraft" is a trademark of the ArtCraft Team that is not used for this product;
  - list the third-party material the binaries embed: Inter, JetBrains Mono, Source Sans 3/Serif 4, Dancing Script, Lucide (ISC), the hyphenation patterns, the optional craft-fonts faces, egui's default fonts (Ubuntu Font Licence, OFL Hack), and our fallback Noto Sans / Noto Sans Mono subset (OFL-1.1; font.md);
  - be shipped in every package (MSI, web zip) next to the binaries (Phase 4), and be reachable from About (a "Licences" button that shows NOTICE + each licence text). The About link is P2 in Phase 1; the file itself is P1.
- **In the app**: the About dialog shows `attribution()` in every app and keeps the Contributors/Models tabs (`CONTRIBUTORS_TAB=true`). Removing the "© ArtCraft Team …" line from About is allowed because the notice lives in NOTICE and the licence texts; but **do not** remove it until our NOTICE ships (WP7g before 0101 lands).
- **Apache-2.0 §4(b)**: modified files must carry prominent notices that they were changed. Each patch's commit message and REGISTER.md row records the change. Each *modified* upstream file gets a one-line comment at its first hunk: `// Modified by the office overlay patch queue (patches/<repo>/NNNN-…).` The new files (`office_overlay.rs`, `build_office.rs`) carry the header `// Added by the office overlay patch queue. MIT OR Apache-2.0.` These are neutral words with no product name, as Rule W requires for the queue.

---

## 9. Implementation order for WP7c–f (and why)

1. **WP7c pdfcraft first.** It has the only legal must-fix (B2), the only network call (B1) and the strictest in-tree gates (`xtask assets`, `xtask layers`, 590 brand catalog rows). If the overlay contract survives pdfcraft's gates, it survives everywhere. It also shares the TSV catalog model with wordcraft, so the choke-point `rebrand` is proven once for both.
2. **WP7d wordcraft.** Same TSV mechanism, so it reuses step 1's `office_overlay.rs` almost verbatim. It is the largest hot-file surface (`lib.rs` 6764 lines), so it gets the most rebase-cost data in the WP14 dry run.
3. **WP7e gridcraft.** Small and isolated (about 25 sites, one message-modal draw site). Its i18n is Rust tables, so the `Language::tr` choke point differs. It stacks on top of the 22-patch queue, so check that 0100 applies after 0022 without touching `crates/engine/src/lib.rs`.
4. **WP7f deckcraft last.** It has no choke point until 0110 exists, so write **0100 together with 0110-deckcraft-i18n-mechanism** (i18n.md §2.5) and let `t()` carry `rebrand`. It has the most literal sites and the highest carry risk (`ribbon.rs`/`dialogs.rs` above 3,600 lines each). The contract is frozen by then.

Patches can be *written* in parallel (four builders, separate cache worktrees), but **freeze `office_overlay.rs`'s API after step 1**. Steps 2–4 copy it, and a change after the freeze is applied to all four in one commit. Builds stay serial (RAM).

---

## 10. Owner decisions and risks

Owner decisions (to add to PLAN §9):
- **O1** Attribution wording (B3 form proposed in §2). The uz/ru variants are drafts.
- **O2** Keep the Contributors/Models tabs (recommended: yes, attribution at zero carry cost). Keep wordcraft's Share button (it only opens Export): recommended yes, set `share = true`. The plan's default is `false`.
- **O3** Rebrand file metadata (`Application`/`Producer`) in Phase 1 (patch 0102, about 1 h per app) or defer to Phase 2. Recommended: Phase 1 for docx/xlsx/pptx `Application` and PDF `Producer` only.
- **O4** The working name in the web embedding wire protocol (`wordcraft:*` message types): keep upstream's (recommended, an API contract) or rename in Phase 5.

Risks:
- **R-B1 carry cost.** Brand sites sit in the hottest files. Mitigation: the choke-point design reduces 0100 to an estimated 40–90 changed upstream lines per app (estimate, measured in WP14).
- **R-B2 interning in `rebrand`.** Leaked strings are bounded by the catalog key count. A pathological caller passing formatted strings (with a file name) through `rebrand` would grow the cache. Mitigation: `rebrand` is only called on catalog values and literals, never on `fmt` output. A debug assertion caps the cache at 4,096 entries.
- **R-B3 pdfcraft web `?file=` fetch** accepts any URL (§4.4). Harden in Phase 5.
- **R-B4 line drift.** All `file:line` values are at the old BASE. The WP5 rebase moves them; `brand-grep` re-finds every site, and the tables here are the checklist, not the patch.
