# deckcraft interface languages (WP9d)

Design: `docs/phase1/designs/i18n.md` section 2.5 (unchanged). This file records what was built.

deckcraft has no i18n upstream: every string is an English literal, there is no `sys-locale` dependency and `ui-egui` has no `web-sys`.
The mechanism copies WordCraft's model (English-key TSV, `t(&str) -> &str`, thread-local current language) and is wrapped at the
helper choke points, so coverage grows with the catalog alone.

## Carried patches

| Patch | What |
|---|---|
| `patches/deckcraft/0110-deckcraft-i18n-mechanism.patch` | `crates/ui-egui/src/i18n.rs` (new): registry (`en`, then `uz-latn` and `uz-cyrl` only when the overlay supplies their catalogs), a strict TSV parser (WordCraft's grammar: `\n \t \\` escapes, placeholder and `…` checks, a bad line is skipped and logged), `t()`/`tr()` (the result goes through `office_overlay::rebrand`), `from_pref`, `normalize_pref` (`uz` is an alias for `uz-latn`). `UiState.language` (`auto` by default; an older `ui.json` still loads), command `app.language.set {language}`, `i18n::set_current` at the start of every frame, a language picker on File > Account. `build_office.rs` embeds `catalog-uz-*.tsv` (cfg `office_uz`). `office_overlay::app_name()` follows the current language. |
| `patches/deckcraft/0111-deckcraft-i18n-p1-strings.patch` | Display-site wraps, no label changes: the ribbon tab row and the helpers `big()`, `small()`, `tip()` and `menu_item()`; the command palette labels; the dialog title, `buttons()` (OK/Cancel), `field()` labels and the About blurb; the Backstage helpers (`nav_item`, page heading, `heading`, `dim`, `arrow_link`, `search` hint, `tile`, `square_button`, `place_row`, `task`) and the former `rebrand("...")` sites. |

## Behaviour

- Without `OFFICE_OVERLAY_DIR` (or without the catalogs) `LANGUAGES` holds English only, the picker is hidden and every string is upstream's.
- `auto` resolves to English. There is no system-language detection in Phase 1: adding `sys-locale` changes deckcraft's lockfile and
  deny surface (it is already in the other three locks). The gate item "OS locale picks Uzbek" therefore holds for wordcraft, gridcraft
  and pdfcraft; deckcraft follows the language the user picks (or `app.language.set`, or `"language"` in `ui.json`).
- The picker names are `Oʻzbekcha` and `Ўзбекча`, with the draft marker `(qoralama)` / `(қоралама)` while any exported Uzbek row is an unreviewed draft
  (`catalog-meta.env`, written by `cargo xt i18n export`).
- Plural: none. `DRAFTING.md` requires count-neutral wording.
- ru for deckcraft is not exported yet (owner decision O4).

## Coverage

Only strings that reach a wrapped helper are translated; everything else stays English until a later phase wraps it. The catalog can already
hold any string that reaches a helper. Gaps seen in the evidence screenshots are listed in `docs/phase1/evidence/i18n/README.md`.
