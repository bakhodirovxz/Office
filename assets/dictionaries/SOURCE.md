# Hunspell dictionaries: provenance

Data only (no code). Read by the Slides spell checker (`deckcraft_engine::spell`, Hunspell subset) and by Docs through the shared
`dictionaries/` folder (PLAN phase 4 section 5.4). The suite installs this folder once, next to the executables.

| | `en_US` | `ru_RU` |
|---|---|---|
| Origin | SCOWL "en_US" Hunspell dictionary (2020.12.07, Kevin Atkinson) | `ru_RU` by Alexander I. Lebedev, `TRY` line by Laszlo Nemeth (2012-08-24) |
| Copy taken from | `wooorm/dictionaries` @ `836d7c2032167880e639a1fe522748733e528c2d`, `dictionaries/en/index.{aff,dic}` | the same commit, `dictionaries/ru/index.{aff,dic}` |
| Licence | MIT-style permission notice (SCOWL) plus the Ispell BSD notice; Moby and UK wordlists are public domain. Text: `LICENSE-en_US.txt` | BSD-style, with the clause "modified versions must be clearly marked". Text: `LICENSE-ru_RU.txt` |
| Modified by us | no (byte-identical to the source files) | no (byte-identical to the source files) |
| sha256 `.aff` | `8ae1f19d4840d957728ad90555d5a8dff6cc5c046279c95ff0c00fc0a0136c7b` | `38ce7d4af78e211e9bafe4bf7e3d6a2c420591136cb738ec6648f8fdf6524cd7` |
| sha256 `.dic` | `f0b1a234bd178bdd01875b2a392a9647f888b8fe879f79c52aae62c2759b3647` | `f6047416a0204adbecf3a451b874ec8a97ee37e2cbc714466ef04d8dbcc0d6fc` |

The `wooorm/dictionaries` repository itself is MIT (its own `license` file); the dictionaries keep their original notices, which are
copied here unchanged. Reproduce:

```
base=https://raw.githubusercontent.com/wooorm/dictionaries/836d7c2032167880e639a1fe522748733e528c2d/dictionaries
curl -L $base/en/index.aff -o en_US.aff   # and index.dic, ru/index.aff, ru/index.dic, en/license, ru/license
sha256sum -c <<< ...   # the hashes above
```

Uzbek proofing is "later" (BRIEF section 4.8): no Uzbek dictionary is shipped.
