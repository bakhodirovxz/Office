# i18n review: how the owner reviews Uzbek strings (WP9b)

Rule: **no Uzbek string is trusted until the owner has marked it `reviewed`.** Release exports contain only reviewed rows
(plus upstream's own ru). Review builds contain drafts too and are labelled as drafts. Nothing in the tooling writes or
"fixes" Uzbek text by itself. Design: `docs/phase1/designs/i18n.md`.

## 1. What you receive

| File | What it is |
|---|---|
| `docs/phase1/i18n/glossary-batch0.xlsx` (and `.csv`) | **Batch 0**: 150 core UI terms (File, Edit, Save, Table, Cell, Slide, Page, ...) with a DRAFT uz-Latn and uz-Cyrl term, 1-2 alternatives and a usage note. Review this first. No catalog drafting starts before batch 0 has come back. |
| `i18n/review/batch-NN.csv` | Later batches, produced by `cargo xt i18n review-export --batch N`. At most 400 rows each. |

Columns (A to H are the review columns; the rest are helpers and are ignored on import):

| Col | Name | Meaning |
|---|---|---|
| A | key | Stable row id. Do not edit. |
| B | context | Usually empty. Tells apart two senses of the same English word. |
| C | en | English source. Do not edit. |
| D | ru | What the upstream app already ships in Russian (reference). |
| E | uz-Latn draft | Proposed Latin text. |
| F | uz-Cyrl draft | Proposed Cyrillic text. |
| G | status | `draft-unreviewed` (initial), `reviewed`, `rejected`. |
| H | owner comment | Free text. It is stored in the row note. |

Spelling rules checked on import: `oʻ` and `gʻ` use U+02BB; the tutuq belgisi (`maʼlumot`) uses U+02BC. ASCII `'`,
U+2018, U+2019 and the backtick next to a letter are reported as errors.

## 2. How to review a row

1. Read C (English) and D (Russian). Check E and F. In the glossary, J and K show alternatives and L says why a term was chosen.
2. Draft is right: set G to `reviewed`.
3. You prefer other text: type it over E and/or F, then set G to `reviewed`. Your text is kept exactly as typed.
4. Wrong and no replacement: set G to `rejected` and say why in H. The row is re-drafted with your comment and returns in a later batch.
5. Not sure yet: leave G as `draft-unreviewed`. The row is untouched.

Review passes (`--pass`): `both` (default; glossary uses it), `latn` (meaning, uz-Latn only), `cyrl` (spelling, only rows whose
uz-Latn is already reviewed). With `latn` the Cyrillic column is shown for context and is not changed by the import.

## 3. How to return a batch

1. Excel: File > Save As > **CSV UTF-8 (Comma delimited)**. A `;` delimiter, which Excel uses in some regional settings, is detected.
2. `cargo xt i18n review-import <file.csv>` (optional `--date YYYY-MM-DD`).

Import rules:
- `en_hash` changed (English changed since the export): the row is skipped and listed as **stale**; it comes back in the next batch.
- `reviewed` and the text unchanged: `reviewed:owner:<date>`.
- `reviewed` and the text changed: your text replaces the draft and becomes `reviewed:owner:<date>`, **unless it fails the apostrophe
  lint**. Then your text is still kept as typed, but its status is `draft:owner-fix:<date>`, with the lint message in the row note.
  It returns in the next batch for confirmation.
- Text changed but status not `reviewed`: stored as `draft:owner-fix:<date>`.
- `rejected`: recorded as `rejected:owner:<date>`.
- Every import appends one line to `i18n/review/LOG.md` (counts per verdict, stale rows). Keep the returned CSV next to the export as evidence.

## 4. Status vocabulary

`todo` (no text yet), `draft:<who>:<date>` (shown to you as `draft-unreviewed`), `reviewed:owner:<date>` (trusted),
`rejected:owner:<date>`, `upstream` (Russian text that ships with the upstream app; trusted as shipped), obsolete rows (priority `obsolete`; the English no longer exists upstream; never exported).

## 5. Commands

```
source scripts/env.sh
cargo xt i18n extract [--dry-run]                       # refresh i18n/*.tsv from upstream/ (read-only)
cargo xt i18n review-export --batch 0                   # glossary -> i18n/review/batch-00.csv
cargo xt i18n review-export --batch 1 [--pass latn|cyrl|both] [--size 400]
cargo xt i18n review-import <returned.csv>
cargo xt i18n coverage                                  # keys / P1 / todo / draft / reviewed per app and script
cargo xt i18n export <app|all> --mode review|release    # native files into $OFFICE_OVERLAY_ROOT/<app>/
cargo xt i18n roundtrip                                 # extract -> export (no Uzbek) must equal upstream's en/ru
cargo xt i18n check                                     # row ids match scope+context+English
```

Batch order: glossary (batch 0), then shared P1, then P1 per app (wordcraft, gridcraft, pdfcraft, deckcraft), then P2. A batch
lists rows that already have a draft; rows without text are not sent.

## 6. Fallback chain (owner decision, 2026-10-17)

`uz-Cyrl -> ru -> en` and `uz-Latn -> en`, applied by the exporter, not at run time. A missing uz-Cyrl row shows upstream's Russian
(only where upstream has it); everything else shows English. English text is never written into an Uzbek catalog. The chain is in `i18n/config.toml`.
