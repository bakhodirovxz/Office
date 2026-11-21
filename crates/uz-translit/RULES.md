# Rules

This file summarises the rule set. The full specification that the code comments cite as `docs/phase2/rules.md` (and
its section numbers) belongs to the project's design documents and is not part of this repository; the tests
(`tests/rules.rs`, `tests/golden.rs`, the golden lists in `data/`) pin the behaviour it specifies.
The exceptions dictionary is `data/exceptions.tsv` (format: the header comment and `build.rs`).

`Options::upgrade_skip_english` (with `latin_upgrade`): a segment (one paragraph) with at least 2 distinct English function words
is left in its Latin; Cyrillic in it is still converted. Word list: `engine.rs` `ENGLISH` (no word that is common Uzbek Latin).
