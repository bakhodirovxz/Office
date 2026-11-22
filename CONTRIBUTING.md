# Contributing

Thank you for helping. This is an open-source office suite (Documents, Spreadsheets, Presentations, a PDF viewer) with Uzbek
Latin <-> Cyrillic transliteration and an add-in for Microsoft Office. Every kind of help is welcome: bug reports, testing on
real documents, translations, documentation and code.

## Ways to help

- **Report a bug.** Open an issue with the "Bug report" form: what you did, what you expected, what happened, the version and
  Windows version. Attach a document that shows the problem only if it holds no personal or confidential data (better: a small
  synthetic file that reproduces it).
- **Fidelity problems.** A .docx/.xlsx/.pptx that looks different than in Microsoft Office is very useful: a screenshot of both and
  the smallest file that shows the difference.
- **Transliteration.** Wrong Latin <-> Cyrillic results (including the 2026 Latin alphabet) go in an issue with the input text and
  the expected output. The rules are in `crates/uz-translit/RULES.md`, the word exceptions in `crates/uz-translit/data/`.
- **Translations.** The UI is in uz-Latn, uz-Cyrl, ru and en; the catalogs are the `.tsv` files in `i18n/`. Corrections by native
  speakers are especially welcome.
- **Code.** Pick an issue labelled `good first issue` or `help wanted`, or open an issue first for anything larger, so we can agree
  on the approach before you spend time on it.

## Ground rules

- **Licence.** The project is licensed under MIT OR Apache-2.0. By submitting a contribution you agree that it is licensed the same
  way (inbound = outbound). Dependencies must use permissive licences (checked by `cargo-deny`).
- **Clean room.** Do not copy code from Microsoft Office, ONLYOFFICE (AGPL) or LibreOffice (MPL), and do not paste code you do not
  have the right to share. Public specifications (ECMA-376, ISO 32000, ...) are fine.
- **Upstream engines.** `upstream/` holds pristine copies of the Craft engines. Never edit them directly: changes are carried as
  numbered patches in `patches/<repo>/` and listed in `patches/REGISTER.md` (see `docs/UPSTREAM.md`).
- **No personal data.** Never commit real documents with personal data, credentials or internal server details.
- **Language.** Code, comments, commit messages and docs are in English. Issues may be written in Uzbek, Russian or English.
- **Evidence.** Performance, fidelity or mobile claims need a measurement or a test, not just a statement.

## Building and testing

The "For developers" part of [README.md](README.md) explains the toolchain and the build. In short: `scripts/bootstrap.sh` checks the
tools, `scripts/ci.sh <job>` runs one CI job and `scripts/ci.sh full` runs the whole gate. Add or update tests with every change;
run at least the tests of the crates you touched and `cargo fmt` before you open a pull request.

## Pull requests

- Open pull requests against the newest `phase-N` branch (currently `phase-7`); development happens in phases, and the maintainer
  carries accepted changes to the other branches.
- Keep a pull request to one topic, describe what and why, and link the issue.
- The public repository is published from the maintainer's working tree. An accepted pull request is applied there and appears in
  the next publish with your authorship kept, so the pull request itself may be closed instead of merged.
- Be kind and constructive; we follow the usual open-source etiquette.

## Security

Please do not report security problems in a public issue. Use GitHub's "Report a vulnerability" (Security tab) so it stays private
until it is fixed.
