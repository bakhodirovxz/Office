# "New user, no instructions" check (WP23, PLAN 12.3 gate item 1)

Status 2026-10-25: **protocol and kit ready; sessions not run yet.** The results tables below are empty on purpose: the owner fills them in
during and after the sessions. Nothing here is a claim that a task passed.

Kit (all in this repo, copied to `E:\office-target\wp23-kit\` with the MSI):

| File | What |
|---|---|
| `docs/phase4/wp23/task-cards.uz-Latn.html`, `task-cards.ru.html` | printable A4, one card per task plus an intro card, for the participant |
| `docs/phase4/wp23/observer-sheet.html` | printable A4 landscape, 3 desktop pages (P1-P3) and 1 web page |
| `docs/phase4/wp23/fixtures/namuna-hisobot.pdf` | the T4 input: 1 page, heading, paragraph, 3x4 table, 31 KB, made by our own converter from invented text (no corpus, no personal data) |
| `scripts/wp23-reset.ps1` | resets the current user's product state between participants (section 6) |

Uzbek and Russian text in the cards and in `README.uz-Latn.txt` of the kit: drafted `agent-sonnet:2026-10-25`, re-reviewed
`reviewed:agent-opus:2026-10-25` (the intro said "Xato qilish mumkin emas" / "Ошибиться нельзя", i.e. "you must not make
mistakes"; now "there are no wrong actions here").

## 1. Limit, stated plainly

The participants are IT staff (owner decision 2026-10-22: the pilot is tested in IT only). IT staff are not representative of office
users, so a "no instructions" result **overstates** ease of use. Record it as such, and repeat the check with 2-3 non-IT office staff before
any wider rollout (Phase 5 pilot or Phase 10). With 2-3 people and one session each, the numbers show *where* people get stuck, not how often.

## 2. Setup

Desktop (P1-P3), one test PC, the same PC for everyone, reset between participants:

1. Install the MSI (kit README): per-machine, needs one UAC click. It is not signed, so SmartScreen shows "Windows protected your PC":
   More info, then Run anyway. Check the SHA-256 first (it is in the kit `SHA256.txt`; it starts `80E7C9B0`, ends `C317`; MSI built 2026-10-25 with the F10 fix 5e2e4b1).
2. Run `wp23-reset.ps1 -Yes` once before the first participant (creates the `WP23` folder on the Desktop and puts the PDF in it).
3. Leave the desktop empty: no app is open. The participant sees the Start menu and the Desktop with the `WP23` folder.
4. Screen recording only if the participant agrees; do not keep it with a name. Otherwise take notes on the observer sheet.

Web (one task set), see section 5.

## 3. Rules for the observer

- Read nothing to the participant except the card (the intro card first: this tests the program, not you; think aloud; no help;
  say "skip" when stuck). Do not name an app, a menu or a button. Do not answer "how do I". Silence is the default.
- Allowed: "please keep talking", "what are you looking for?", telling the time limit once, repeating the task text.
- An **assist** is any hint, pointing, reading a label aloud, answering a how-question, or doing a step for them. Count each one. A
  crash or a hang is an *incident*, not an assist: note it, restart the app, and continue (it is a finding too).
- When the time limit is reached, stop the task and mark it N (or P if the result is partly right), then go to the next one. Do not
  coach afterwards until all five tasks are done.
- Fixed order T1 to T5 (T5 needs the letter of T1; if T1 failed, open any document for them before T5 and note it).
- Write only: P-id, start/end times, outcome Y/P/N, assist count, where they got stuck (screen and control, not names), short quotes
  without names. No names, no document text, no screenshots that show a name. P-ids are not linked to names anywhere.

## 4. Tasks

Time limits are per task (PLAN suggests 5-8 minutes). "Start state" = what is on the screen when the task card is handed over.

| Task | Card (plain words, no hints) | Start state | Success criterion (observable) | Limit |
|---|---|---|---|---|
| T1 Letter | Write a short letter (3-4 sentences) to a colleague about a meeting next Monday at 10:00; save it as "Xat" in the WP23 folder | nothing open; Desktop and Start menu visible | A file named like the card asks exists in `WP23`, opens again, and holds 3 or more sentences that mention Monday and 10:00 (observer opens it; content not recorded) | 8 min |
| T2 Table with a sum | A table with 3 columns (name, count, price) and 4 rows of invented data; the program must calculate the sum of the prices; save as "Jadval" | nothing open | File exists, 3 columns and 4 data rows, one cell below the prices is a formula (`=SUM(...)`, observer looks at the cell) and its value equals the sum. Typing the total by hand = P | 8 min |
| T3 Deck | A 3-slide deck: slide 1 a title ("Chorak natijalari"), slide 2 two or three short points, slide 3 "Rahmat"; save as "Taqdimot" | nothing open | File exists and opens with exactly 3 slides carrying those contents | 8 min |
| T4 PDF to Word | Get a Word document (.docx) out of `namuna-hisobot.pdf` in `WP23`; find the result and open it | `namuna-hisobot.pdf` in `WP23`; nothing open | A `.docx` exists whose content shows the heading, the paragraph and the 3x4 table (observer opens it in Docs); the participant opened it themselves | 6 min |
| T5 Show/Hide paragraph marks | In the open letter make paragraph marks and spaces visible, then hide them again | the T1 letter open in Docs | The observer sees the marks (pilcrow at the paragraph ends, dots for spaces) on screen, and later the document without them. Both states needed for Y; only showing = P | 5 min |

### Expected paths (the observer knows them, the participant is not told)

Found by reading the product; not yet walked with a stranger.

- **T1:** Start menu "Hoshiya Docs" (or the Hoshiya home shell, card Docs, then New / a blank document), type, Ctrl+S, Save As dialog,
  folder `WP23`. On the web: the same, and "save" is a browser download (Downloads, not `WP23`).
- **T2:** "Hoshiya Sheets": type the cells, `=SUM(` over the price cells (or an AutoSum command if the toolbar has one), save.
- **T3:** "Hoshiya Slides": title slide, add slides (New slide), type, save.
- **T4:** two real paths exist.
  1. **The converter** (primary; this is what the home screen offers): the Hoshiya home shell has a card "Converter" (hint "Convert between
     document formats and work with PDF pages", `web/shell/i18n.json` `home.convert`; route `/convert/`, page `services/convert-api/ui`).
     On the page: "Choose files" (or drop the file), pick the file `namuna-hisobot.pdf`, "Convert to" = DOCX (`ui.opt.target`),
     "Convert", then download the result (on the desktop host a download lands in `%USERPROFILE%\Downloads`, not in `WP23`; the observer
     tells nobody, the participant has to notice where it went; if they look in `WP23`, that is a finding).
     The MSI ships the engine: `convert\pdf-cli.exe` and `convert\hoshiya-convert.exe`. I ran the same conversion on this PC with the
     staged `hoshiya-convert.exe` (`namuna-hisobot.pdf --to docx`): it exits 0 and the docx holds the table and the word "Qalam".
     This was not clicked through the real shell page.
  2. **The PDF app:** open the PDF in "Hoshiya PDF" (double-click is not set as default; use Start menu, then open), File menu, "Export
     to Word..." (`export.docx`, `upstream/pdfcraft/crates/engine/src/commands.rs:324`; the wrapper does `export_office_dialog`).
     Not clicked through either.
  Either path counts as success. Record which one they took.
- **T5:** In Docs, Home tab, Paragraph group, the pilcrow button "Show/Hide ¶" (`upstream/wordcraft/crates/ui-egui/src/ribbon.rs:1160`, command
  `view.marks`; also a key tip). Found in the source; not clicked on a running app for this kit.

## 5. The web session

What the web shell is: `hoshiya-serve.exe` (crate `suite-serve`, `services/suite-serve/src/main.rs`) serves the home shell, the converter
and the four editors on one origin. It is **in the MSI** (`%ProgramFiles%\Hoshiya\hoshiya-serve.exe`) and in the build output.

**Local mode binds 127.0.0.1 only, and that cannot be changed by a flag.** `auth = "none"` is refused on any non-loopback address
(`services/convert-api/src/config.rs:344`), and the host guard serves only `localhost:<port>` and `127.0.0.1:<port>` (checked: a request with
`Host: 10.0.0.5:8787` gets 421). Opening it to the LAN needs server mode: `auth = "jwt"`, a TLS certificate (`tls_cert`, `tls_key`),
`public_url`, a secret, and a token for each browser (`docs/phase4/tls.md` is the tested setup). That is not prepared for tomorrow and
would test the token flow, not the usability of the editors. So the web session is **held on the owner's PC itself**: the participant sits
at it, in Microsoft Edge or Chrome.

Command (PowerShell or cmd, from the repo root, so `brand/brand.toml` is found; checked 2026-10-25: `GET /` and `/docs/` answer 200):

```
E:\office-target\p1\services\release\hoshiya-serve.exe --dist E:\office-target\p1\dist
```

Then open `http://127.0.0.1:8787/` in the browser (it prints `hoshiya-serve listening on http://127.0.0.1:8787`). Stop it with Ctrl+C in
that window. If the MSI is installed on that PC instead, `"%ProgramFiles%\Hoshiya\hoshiya-serve.exe" --dist "%ProgramFiles%\Hoshiya\dist"`
does the same (run it from a folder that has no other `brand` folder; the default `--brand` is `brand/brand.toml` relative to the current
folder, so pass `--brand "%ProgramFiles%\Hoshiya\brand.toml"` too).

Web differences to expect (known, not findings by themselves): saving is a browser download into Downloads; "open file" uses the browser's
file chooser; the editors are canvases (no screen-reader tree, A1) and keyboard focus can get trapped in them (A2), see `a11y-baseline.md`.
Use a fresh browser profile (a private window) per participant. Use a **different participant** for the web set than the one who did the
desktop set, if you have one; if not, run the web set with P3 after a break and write down that P3 had seen the tasks.

## 6. Between participants

`scripts\wp23-reset.ps1` (dry run by default; `-Yes` deletes). It removes only the product's own per-user folders, derived from
`brand/brand.toml`: under `%APPDATA%` and `%LOCALAPPDATA%` the folder named like the product, and the upstream-named folder of each app (the apps
still keep their preferences, recent list and recovery files under their upstream names: `WordCraft`, `GridCraft`, `DeckCraft`, `PdfCraft`).
It also empties the `WP23` folder on the Desktop and copies the fixtures back in. It refuses to run while a product program is running
(it never kills one). It does not touch `Downloads` (the converter result of T4 lands there: delete `*.docx` made by the previous participant by hand,
or say so in the notes), the registry, file associations, or the installed program. Details are in the script header.

## 7. Results (fill in)

One cell: `Y/P/N, mm:ss, assists`. Copy times from the observer sheet. Desktop = the MSI on the test PC; W = the web set.

| P | T1 Letter | T2 Table + SUM | T3 Deck | T4 PDF to Word | T5 Show/Hide | Incidents (crash, hang) |
|---|---|---|---|---|---|---|
| P1 desktop | | | | | | |
| P2 desktop | | | | | | |
| P3 desktop | | | | | | |
| W web | | | | | | |

For T4 also write which path: converter / PDF app / none. Session date and PC (Win10 or Win11): ____________

### Findings (every failure needs a fix commit or a gate note)

| # | Task, P | What happened (where they got stuck, no names) | Severity | Fix commit or gate note |
|---|---|---|---|---|
| 1 | owner's own use of the installed MSI (2026-10-27, Docs, uz-Latn) | Every start of Docs (also opening a .docx) showed File › Recover with old copies, also of other files ("Untitled draft" 6-7 days old). Cause: the page opened whenever any copy existed, and a copy was removed only when its still-open document was saved, so Don't Save, Save + Open/Close in one step and Continue without restoring all left copies that came back at every start. Fixed (wordcraft/0127): offered once per copy, a clean exit removes the window's own copies, an offered copy expires after 7 days. Built exe, isolated profile: before, the page opened at starts 2, 3 and 5 and Don't Save + exit kept the draft; after, only start 2 (after the kill), and Don't Save + exit removes it. PDF gets the 7-day expiry (pdfcraft/0113); Slides reopens leftovers as tabs and Sheets has no recovery | high | branch `p4-recovery` (see STATUS, WP23 findings) |
| 2 | same session | Save (Saqlash) froze the window ("Не отвечает"), here from the Recover page. Cause: the version-history snapshot (docs-office `layer.rs`) ran inside `file.save` on the UI thread and pruned the whole history store, which held 2,672 documents, nearly all converter test outputs (the converter's CLI runs had recorded history in the real profile). Measured with the built exe on a copy of that store: first save 29.8-51.8 s, next ones 1-4 s (local disk, mapped drive, from the Recover page); after the fix (snapshot on its own thread; no history in `WORDCRAFT_NO_PREFS` runs) 0.1-0.65 s. The untitled-document save opens Save As without blocking (0.3 s) | high | branch `p4-recovery` (see STATUS, WP23 findings) |

## 8. Keyboard and Narrator walk (owed by `a11y-baseline.md` A12, A1, A2)

The baseline could not run these (the Windows session was locked). Do them on the test PC after the participants, on an unlocked desktop,
and write the result in the table. Narrator: Win+Ctrl+Enter starts and stops it.

| # | Check | Result (ok / fail / note) |
|---|---|---|
| K1 | Hoshiya home shell (Edge or the desktop host): Tab goes through search, language, 4 app cards, Open file, templates, converter link; every stop has a visible focus ring; Enter on a card opens it | |
| K2 | Converter page by keyboard only: choose a file (Enter on "Choose files"), pick "Convert to", Convert, reach the download | |
| K3 | Docs (desktop app): Alt starts key tips, a toolbar command is reachable by keys; if Docs has a command search (Slides has Ctrl+Shift+P), a command runs from it | |
| K4 | Sheets and Slides (desktop): same; Slides has no key tips (A7), so use the command search and shortcuts | |
| K5 | Dialogs (Save As, Open): Tab, Enter and Escape work; focus returns to the document | |
| K6 | Web editor in the browser: Tab into the editor and back out. Expected fail (A2: focus stays in the editor); note whether F6 gets out | |
| N1 | Narrator on the desktop Docs: reads the window name, toolbar buttons with their names (UIA lists 44 unnamed interactive nodes in Docs; note which ones are announced as just "button") | |
| N2 | Narrator on the home shell and the converter page (names, the file chooser, the status after Convert) | |
| N3 | Narrator on a web editor in Edge: expected to announce one unnamed text box (A1); note what it says | |
| N4 | `scripts\uia-dump.ps1 -Tabs 40` on an unlocked desktop, for each of the four apps (the keyboard-walk script the baseline left ready) | |

Do not report Narrator speech as verified unless somebody listened to it.
