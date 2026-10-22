# The deckcraft spell-check access violation (WP6)

**Result: 0 of 20 GUI runs crashed (en-US) and 0 of 5 (ru-RU). It did not reproduce in the real app. No fix is carried.**

## What was known
`docs/phase1/tests.md`: the `deckcraft-ui-egui` lib test binary dies with 0xC0000005 when run multi-threaded in a debug build (6 of 10 runs),
never with `--test-threads 1`, and not under nextest (13 of 13 runs clean). Narrowed to the two `squiggles_*` tests together with any other
spelling test; both call `spell::install` / `set_user_dictionary_path` (process-wide `OnceLock<Mutex<..>>`) and settle on a worker thread.
Root cause never found.

## How it was tried
- Build: `scripts/build.sh slides desktop --release` at BASE `fe510bd` with the queue through 0142 (2026-10-23, after the fonts incident of
  2026-10-22), `E:/office-target/p1/slides/release/slides.exe`.
- Dictionaries: the shipped `assets/dictionaries` (en_US 551 KB, ru_RU 3.5 MB; `docs/phase4/licences.md`), through `DECKCRAFT_DICTIONARIES`.
- Driver: `scripts/spell-crash-repro.mjs`, over the control channel (`--control <port>`), one fresh process and `APPDATA` per run:
  new presentation, blank layout, two text boxes side by side, then 606 characters (534 for Russian) typed one at a time with random 15-55 ms gaps,
  alternating between the boxes (`edit.select`, `text.edit`, `ui.text`) three rounds of three phrases, so the background checker restarts with
  every keystroke; 2.5 s settle; screenshot; `app.quit`. A run counts as a crash when the process exits before the script asks it to quit.
- Evidence that the checker really ran: squiggles under the misspelt words in the screenshots
  (`evidence/toolbar/slides-spell-en-squiggles.png`, `slides-spell-ru-squiggles.png`), 600+ keystrokes each.

## Verdict
Test-harness only, as far as this goes: the crash needs the debug test binary's parallel tests (several `Harness`es and the shared statics
in one process). The GUI has one UI thread and one checker thread per slide check and was clean in 25 of 25 runs.
**The quarantine stays** (nextest, one process per test); the in-process crash is still unexplained. Not measured: a debug GUI build, a real
user's hours of typing, or other dictionaries. Spell-check may be on by default in Slides (the PLAN gate: "only if the GUI run is clean").
