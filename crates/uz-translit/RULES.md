# Rules

The authoritative rule set for this crate is [`docs/phase2/rules.md`](../../docs/phase2/rules.md).
The exceptions dictionary is `data/exceptions.tsv` (format: section 6.1 of that file).

`Options::upgrade_skip_english` (with `latin_upgrade`): a segment (one paragraph) with at least 2 distinct English function words
is left in its Latin; Cyrillic in it is still converted. Word list: `engine.rs` `ENGLISH` (no word that is common Uzbek Latin).
