# Phase 4 data licences (WP6: the spell-check dictionaries)

Dictionaries are data, but they ship in our installer, so the permissive-only rule (CLAUDE.md) is applied to them as to code.
`cargo deny` does not see them (they are not crates); this file is the check. NOTICE lists them (`scripts/gen-legal.sh`).

| Dictionary | Source and version | Licence | Permissive? | Verdict |
|---|---|---|---|---|
| `en_US` | SCOWL 2020.12.07 (Kevin Atkinson), the Hunspell `en_US` build; affix file derived from Ispell (Geoff Kuenning) | MIT-style permission notice ("Permission to use, copy, modify, distribute and sell ... without fee, provided that the above copyright notice appears in all copies") plus the Ispell BSD notice (clause 4 removed with permission). The word lists inside are public domain (Moby, UK English Wordlist) or under the same notices. | yes | ships |
| `ru_RU` | Alexander I. Lebedev, 1997-2008; `TRY` line added 2012-08-24 by Laszlo Nemeth | BSD-style: retain the notice in source and binary redistributions, mark modified versions, no endorsement. | yes | ships |

- The files are byte-identical to the source files and keep their notices (`assets/dictionaries/LICENSE-en_US.txt`,
  `LICENSE-ru_RU.txt`). "Modified versions must be clearly marked" does not apply: we do not modify them.
- Where from, with hashes and the reproduction command: `assets/dictionaries/SOURCE.md`. The copy comes from
  `wooorm/dictionaries` (MIT) at a fixed commit, which repackages the LibreOffice/Hunspell dictionaries with their original notices.
- No LGPL, MPL or GPL dictionary is used. (`hunspell-ru` and several `en_US` builds elsewhere are LGPL or MPL: not these files.)
  If a later dictionary needs one of those, it needs an owner decision.
- The installer must carry the two `LICENSE-*.txt` files next to the dictionaries (the licence says the notice goes with binary
  redistributions).
- Uzbek proofing is "later" (BRIEF section 4.8); no Uzbek dictionary is shipped.
