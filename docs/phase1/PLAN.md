# Phase 1 (Foundation): execution plan

| | |
|---|---|
| **Status** | Plan, not started. Written 2026-10-17 on branch `phase-1`. |
| **Gate decisions in force** | ADR Appendix C: working name from one brand config; MIT OR Apache-2.0; W2 build domains + vendored pristine snapshot + patch queue; **no upstream PRs, everything is carried**; A-web (conditional); fidelity → mobile → co-editing. |
| **Inputs** | `CLAUDE.md`, `docs/BRIEF.md` §7 Phase 1, `docs/ARCHITECTURE-DECISION.md` (§2.1, §2.5, §3, §4, §5, §6, App. B/C), `docs/STATUS.md`, `docs/phase0/*` |
| **Exit gate (brief)** | (1) A clean checkout builds desktop and web for all four apps. (2) CI is green. (3) A dry run of the upstream sync succeeds. (4) The UI shows the new brand and switches to uz-Latn and uz-Cyrl. |
| **Estimate** | **≈185 engineer-hours of agent work plus ≈45 machine-hours of builds. About 20 working days (4 calendar weeks). Range 17–27 days.** See §8. |

How to use this plan. Every work package (WP) in §2 can be handed to one sub-agent as written. Each WP lists its
inputs, its steps, the files it creates, a done-check (a command and its expected output), an estimate, its risks and
the model tier. Sections §3–§7 hold the designs that the WPs implement. Put the deeper design notes in those
sections, not in the WPs. "Scout-reported" `file:line` references come from read-only haiku scans made for this
plan on 2026-10-17 at the Phase 0 HEADs. Re-check them before writing a patch, because they move with every sync.

---

## 0. New facts found while planning (they change the design)

Measured or read for this plan on 2026-10-17.

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | **The spike queue has 27 patches, not 16.** It has three series: wordcraft 0001–0005 on `fd0bb2f`; gridcraft 0001–0019 on `e6b6b34` (memory round 1 is 0001–0008; round 2 is 0009–0019, stacked on round 1 at `ac87f55`); and gridcraft pdf-fonts 0001–0003, also on `e6b6b34` as a separate branch. | `ls spikes/*-patches/`; patch `From` lines | The two gridcraft series must be merged into **one ordered queue**. They overlap in one file only, `crates/engine/src/lib.rs`. The ADR's "16 patches" figure is out of date. |
| F2 | **`core.autocrlf=true` is set globally and system-wide**, and the upstream working trees are CRLF (`git ls-files --eol`: `i/lf w/crlf`). `git archive HEAD` emits CRLF unless it runs with `-c core.autocrlf=false`. pdfcraft's own `.gitattributes` says `* -text` because CRLF "rewrote signed PDFs … and assets whose SHA-256 ATTRIBUTION.toml records". | `git config --get core.autocrlf` → `true`; `file upstream/wordcraft/Cargo.toml` → CRLF | Vendoring by copying the working tree, or with a default `git archive`, **corrupts the snapshot**. The monorepo needs `.gitattributes` `-text` for `upstream/**` and `patches/**`, plus `core.autocrlf=false`. |
| F3 | Upstream trees contain **7–9 executable files** per repo (`100755`) and no symlinks or submodules. | `git ls-tree -r HEAD \| grep ^100755` | On Windows (`core.filemode=false`), `git add` of copied files records `100644`, so the tree hash would no longer equal upstream's. Import with `git read-tree --prefix` (§1.4); then the subtree hash **equals upstream's tree hash exactly**, and that equality becomes the integrity check. |
| F4 | **No app has a branding hook.** `show_discord` is a *user preference* in `UiState` (`wordcraft/crates/ui-egui/src/lib.rs:259`, default `true` at `:354`, toggled by the user at `options.rs:193`). It is not a product config. gridcraft, deckcraft and pdfcraft have nothing at all. | Read for this plan; scouts | ADR §2.1.2 ("one hook already exists") is wrong. **Every brand item is a carried patch.** §3 keeps each one to a single token through one overlay module per app. |
| F5 | **pdfcraft checks GitHub for updates** (`apps/pdfcraft/src/updates.rs:7`, `api.github.com/repos/storytold/pdfcraft/releases/latest`; `crates/ui-egui/src/updates.rs:15`). pdfcraft also **draws the ArtCraft logo and mark** in About and Home (`widgets.rs:503,514`; `dialogs.rs:1156,1174`; `home.rs:40-43`, scout-reported). | pdfcraft scout | Both must go in our builds. Otherwise our product would offer users upstream PdfCraft downloads and show a non-open trademark. |
| F6 | Upstream has moved. HEADs on 2026-10-17: wordcraft `eae2421`, gridcraft `f1894c6`, deckcraft `1dc6972`, pdfcraft `924c4da`, craft-libs `15ed713`. craft-fonts is unchanged at `3d9ab3a`. There is no release tag newer than v0.7.0 / v0.8.0. | `git ls-remote` (works with `-c http.schannelCheckRevoke=false`) | The new BASE is a HEAD, not a tag (ADR §4.2). The rebase in WP5 spans 4–7 days of upstream, which is close to one weekly sync. |
| F7 | Snapshot sizes, tracked bytes (`git archive` tar / gzip -6): wordcraft 25.6 / 10.2 MB; gridcraft 9.7 / 4.2; deckcraft 11.9 / 4.0; pdfcraft 37.2 / 15.2; craft-libs 6.2 / 1.9; **craft-fonts 41.8 / 28.8**. The five code repos total **90.5 MB raw / 35.5 MB compressed**. | `git archive \| wc -c`, `\| gzip -6 \| wc -c` | Sets the vendoring cost (§1.3). |
| F8 | The upstream clones carry **linked worktrees and branches that exist nowhere else**: gridcraft `spike/formula-templates`, `spike/memory-r2` and `spike/pdf-fonts` (worktrees in `E:/office-work/gridcraft-{spike,pdf}`); wordcraft `spike/bridge-mobile` and `spike/mobile-b` (worktrees in `E:/office-work/wordcraft-{spike,b}`). | `git worktree list` | Moving the clones must not lose these branches. Run `git worktree repair` after the move (WP1). |
| F9 | deckcraft's `.cargo/config.toml` sets `[env] CRAFT_FONTS_DIR = "../craft-fonts"` (relative). With that variable set, the fonts crate **embeds craft-fonts into the binary**; wasm gets only Inter Regular/Bold and BIZ UDPGothic (`deckcraft/crates/fonts/build.rs:1-6`). Cargo reads `.cargo/config.toml` from the **current directory**, not from the manifest's directory. | Read for this plan | A domain build does not see upstream's `.cargo/config.toml`, so our env script must set `CRAFT_FONTS_DIR` explicitly. The setting changes the binary and wasm size, so it is part of the size-budget definition (§6.4). |
| F10 | pdfcraft's bundled **JetBrainsMono lacks U+02BB**, while Inter has it. The font test `tests/fonts.rs:297` requires every catalog character in all four UI faces. gridcraft's number formatter hard-codes `.` and `,` (`crates/numfmt/src/render.rs:158-159,198`). | Scouts (fontTools) | Feeds the fallback-font design (§4.5) and the decimal-separator check (WP10). |
| F11 | `rustfmt`, `clippy`, `cargo-deny`, `cargo-nextest`, `sccache` and `actionlint` are not installed. trunk 0.21.14 is at `E:/office-tools/cargo/bin/trunk.exe`. fontTools 4.61.1 is installed for Python 3.10. Python 3.10 has **no `tomllib`**. | `which`, `python -I -c` | Our tooling is a Rust `xtask` (it uses `toml_edit`), not Python (§1.2). Tools are downloaded as prebuilt binaries (WP0). |
| F12 | Only pdfcraft has a `deny.toml` (65 lines). All six repos ship `LICENSE-MIT`, `LICENSE-APACHE` and `ATTRIBUTION.md`; `NOTICE` exists in all except craft-fonts. | `ls upstream/*/` | Keeping them is automatic, because the snapshots are pristine. |

---

## 1. Target repository layout

### 1.1 Tree

```
office/                                    (git, branch phase-1; no remote)
├─ .gitattributes                          NEW: * text=auto eol=lf; /upstream/** -text; /patches/** -text; binaries
├─ .gitignore                              EDIT: drop "/upstream/"; add "/upstream/craft-fonts/", "/target/"
├─ Cargo.toml  Cargo.lock                  NEW: root workspace "core": members crates/*, xtask; exclude upstream, domains, spikes, services
├─ .cargo/config.toml                      NEW: alias xt = "run -q -p xtask --"
├─ rustfmt.toml  deny.toml                 NEW (our crates; deny policy shared by all 5 lockfiles)
├─ LICENSE-MIT  LICENSE-APACHE  NOTICE     NEW: our layer (MIT OR Apache-2.0); NOTICE credits upstream
├─ brand/
│  ├─ brand.toml                           THE one brand config (working name, ids, colours, flags, attribution)
│  └─ icons/README.md                      icons are generated by `cargo xt brand icons` into the overlay
├─ i18n/
│  ├─ glossary.tsv                         core terms en→uz-Latn→uz-Cyrl→ru (owner-reviewed first)
│  ├─ wordcraft.tsv gridcraft.tsv pdfcraft.tsv deckcraft.tsv   canonical catalogs (§4.2)
│  ├─ DRAFTING.md                          rules for draft producers (agents)
│  └─ review/                              batch exports to and imports from the owner (CSV)
├─ assets/fonts/fallback/                  NotoSans-{Regular,Bold}-uz.ttf, NotoSansMono-Regular-uz.ttf, OFL.txt, SOURCE.md
├─ crates/
│  ├─ office-brand/                        parses brand.toml; renders the overlay key=value file
│  └─ office-i18n/                         canonical TSV model, lint, native exporters (wordcraft/pdfcraft TSV, gridcraft table, deckcraft TSV)
├─ xtask/                                  `cargo xt …`: gen-domain, lockdiff, verify-queue helpers, register, overlay, i18n, sizes, brand-grep, quarantine
├─ upstream/                               COMMITTED pristine snapshot + queue applied; never hand-edited
│  ├─ wordcraft/ gridcraft/ deckcraft/ pdfcraft/ craft-libs/
│  └─ craft-fonts/                         FETCHED (ignored), pinned by patches/craft-fonts/BASE + SHA256SUMS
├─ patches/
│  ├─ REGISTER.md                          the register (§5.3)
│  ├─ rebase-log.tsv                       append-only rebase cost log (§5.4)
│  └─ <repo>/BASE  <repo>/NNNN-*.patch     BASE = source of truth (§1.5); craft-fonts/SHA256SUMS
├─ domains/
│  ├─ docs/   (wordcraft)  sheets/ (gridcraft)  slides/ (deckcraft)  pdf/ (pdfcraft)
│  │  ├─ domain.toml                       generator input (which upstream, which apps, our bin names)
│  │  ├─ Cargo.toml  Cargo.lock            GENERATED root (+ lock seeded from upstream); [patch]/[profile]/[workspace.lints] copied
│  │  ├─ apps/<d>-desktop/  <d>-web/  <d>-cli/   GENERATED Cargo.toml + OUR build.rs / index.html / Trunk.toml
│  │  └─ crates/                           our engine-bound crates (docs: docs-bind, moved from spikes/mobile-b/bind)
├─ scripts/
│  ├─ env.sh                               every script sources it (§1.6)
│  ├─ bootstrap.sh                         clean checkout → tools present, craft-fonts fetched, caches set up
│  ├─ fetch-fonts.sh  verify-queue.sh  sync-upstream.sh  build.sh  ci.sh  lib/lock.sh (build lock + RAM gate)
├─ ci/
│  ├─ quarantine.toml                      known failing tests with reasons (§6.5)
│  ├─ nextest.toml                         nextest profiles (retries, test groups)
│  └─ web-smoke/                           Playwright smoke spec (load each dist, no "failed to start", screenshot)
├─ .github/workflows/ci.yml                same jobs as scripts/ci.sh; `on: workflow_dispatch` only (ready to enable)
├─ web/budgets.toml                        size budgets (§6.4)
├─ services/                               NOT in Phase 1 (docserver arrives in Phase 5)
├─ spikes/                                 unchanged (Phase 0 evidence; *-patches stay as historical copies)
└─ docs/phase1/                            PLAN.md, domain-build.md, rebase-cost.md, craftlibs-single-rev.md,
                                           locale-numbers.md, deckcraft-i18n.md, sizing.md, evidence/ (screenshots)
```

**Five lockfiles in Phase 1** (= "cargo-deny ×5"): root `Cargo.lock` plus the four `domains/*/Cargo.lock`. `services/`
becomes the sixth in Phase 3 or 5. The root workspace and the domains are **separate Cargo workspaces**: each domain's
root `Cargo.toml` declares its own `[workspace]`, and the root lists `domains/` under `exclude`.

### 1.2 Committed vs ignored

| Committed | Ignored or outside the repo |
|---|---|
| `upstream/{wordcraft,gridcraft,deckcraft,pdfcraft,craft-libs}` (pristine + queue), `patches/**`, `domains/**` (including generated manifests and seeded locks), `brand/`, `i18n/`, `assets/fonts/fallback/`, `crates/`, `xtask/`, `scripts/`, `ci/`, `.github/`, `web/budgets.toml`, docs and evidence PNGs (each under 300 KB) | `upstream/craft-fonts/` (fetched); all build output: `E:/office-target/p1/<workspace>/`; the overlay dirs `E:/office-target/p1/overlay/<app>/` (generated); sccache `E:/office-cache/sccache`; the advisory DB `E:/office-cache/advisory-db`; upstream cache clones `E:/office-upstream/<repo>`; sync scratch `E:/office-target/p1/sync/`; `corpus/` (never) |

Tooling language: **Rust `xtask`** (in the root workspace) for anything that parses TOML or TSV or compresses files,
because Python 3.10 has no `tomllib` (F11) and xtask is portable to GitHub runners. **Bash** (`scripts/*.sh`) for
orchestration. Python only for the existing fidelity harness and for font subsetting with fontTools (`python -I`).

### 1.3 Size impact of vendoring

- **Initial commit.** About 90.5 MB raw / **35.5 MB compressed** for the five code repos (F7). The monorepo `.git`
  is 23 MB today, so it grows to about 60 MB.
- **Two BASEs land in Phase 1** (old BASE in WP1, new BASE in WP5). The second adds only the changed blobs. Expect
  +5–15 MB per weekly sync until `git gc` deltas them (ESTIMATE: at 700–1,000 commits a week, a large share of the hot
  files change). Rough PROJECTION: 0.3–0.8 GB after a year. Mitigation: `git gc --aggressive` monthly. If the repo
  ever passes 1 GB, re-baseline (squash old pristine commits into a new root). That decision is for later.
- **craft-fonts: fetched, not vendored.** Reasons:
  1. It is 41.8 MB of binary OFL fonts (28.8 MB compressed), which would be **45% of the repo**.
  2. We never patch it.
  3. It is not a Cargo dependency of any app; it enters only through `CRAFT_FONTS_DIR` (F9). So one fetched copy
     serves every domain, which is the ADR dedupe goal.
  4. Its content is mostly CJK, Arabic and Thai, which our uz/ru users rarely need. (It changed 10 times in its whole
     history.)

  A clean checkout needs the network anyway (crates.io and the craft-libs git deps), so fetching adds no new
  dependency.

  Integrity: `patches/craft-fonts/BASE` pins the SHA, and `patches/craft-fonts/SHA256SUMS` lists all 31 files.
  `scripts/fetch-fonts.sh` extracts it with `git -c core.autocrlf=false archive` from the cache clone (cloning first if
  needed) and verifies the hashes. A tarball backup is kept at `E:/office-cache/craft-fonts-<sha>.tar` in case
  upstream disappears.
- **craft-libs: vendored** (6.2 MB). The apps still fetch it by git rev through Cargo. The vendored copy is the target
  for the single-rev experiment (WP4) and for any future craft-libs patch.

### 1.4 Turning the existing `upstream/` clones into pristine snapshots

1. The clones become **cache clones** at `E:/office-upstream/<repo>` (moved, not re-cloned). They keep their full
   history, remotes, spike branches and linked worktrees (F8). All diffing and fetching happens there.
2. The monorepo receives **only the tree objects** of BASE, copied with git plumbing. No upstream history enters our
   `.git`, and nothing is shallow:
   ```bash
   # import_pristine <repo> <sha>   (inside scripts/sync-upstream.sh and WP1)
   C="$UPSTREAM_CACHE/$1"; tree=$(git -C "$C" rev-parse "$2^{tree}")
   { echo "$tree"; git -C "$C" ls-tree -r -t "$tree" | awk '{print $3}'; } \
     | git -C "$C" pack-objects -q --stdout | git unpack-objects -q
   git rm -r -q --cached --ignore-unmatch "upstream/$1"; rm -rf "upstream/$1"
   git read-tree --prefix="upstream/$1/" -u "$tree"
   ```
   `read-tree --prefix` writes exact modes and blob ids (F3). The `-text` attribute on `/upstream/**` makes the checkout
   byte-exact (F2).
3. **Two commits per repo per BASE.**
   - Commit **P** "`vendor(<repo>): pristine <sha7>`". Its `upstream/<repo>` subtree hash **equals** upstream's
     `<sha>^{tree}`.
   - Commit **Q** "`patches(<repo>): apply queue (N patches) on <sha7>`". Its tree is the queue applied in the cache
     worktree, imported the same way.

   So `git diff P_old P_new -- upstream/<repo>` is the pure upstream change between syncs, and `git diff P Q` is
   exactly our delta.

### 1.5 Source of truth for BASE

`patches/<repo>/BASE` is a key=value file, and it is the only source of truth for BASE:
```
repo=wordcraft
url=https://github.com/storytold/wordcraft
sha=fd0bb2fb1ec081e3eddc4d01917637fef0cafdfe
tree=<git rev-parse sha^{tree}>
date=2026-10-15T01:35:42-04:00          # upstream committer date
describe=v0.7.0-266-gfd0bb2f
synced=2026-10-19                        # when we took it
```
Consistency rules, enforced by `scripts/verify-queue.sh` in CI:
- (a) `git rev-parse <commit P>:upstream/<repo>` equals `tree`. P is found as the last commit whose message is
  `vendor(<repo>): pristine <sha7>`.
- (b) Replaying the queue onto `tree` gives exactly `HEAD:upstream/<repo>` (§5.2).
- (c) The working tree under `upstream/<repo>` is clean and has no untracked files.

`docs/STATUS.md` *cites* BASE; it never defines it.

### 1.6 Environment (`scripts/env.sh`, sourced by every script)

```bash
export PATH="$HOME/.cargo/bin:/e/office-tools/cargo/bin:/e/office-tools/bin:$PATH"
export CARGO_HTTP_CHECK_REVOKE=false CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_INCREMENTAL=0
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=http.schannelCheckRevoke GIT_CONFIG_VALUE_0=false   # git CLI fetches (F6)
export OFFICE_ROOT="$(git rev-parse --show-toplevel)" OFFICE_TARGET=E:/office-target/p1
export UPSTREAM_CACHE=E:/office-upstream OFFICE_CACHE=E:/office-cache
export RUSTC_WRAPPER=sccache SCCACHE_DIR=$OFFICE_CACHE/sccache SCCACHE_CACHE_SIZE=40G
export CRAFT_FONTS_DIR="$OFFICE_ROOT/upstream/craft-fonts"          # one copy for all domains (F9)
export OFFICE_OVERLAY_ROOT=$OFFICE_TARGET/overlay                     # per-app OFFICE_OVERLAY_DIR set by build.sh
# curl always with --ssl-no-revoke; Gradle not needed in Phase 1
```
`scripts/lib/lock.sh` gives two guards.
- `with_build_lock <cmd>`: one rustc-heavy job at a time, using `mkdir $OFFICE_TARGET/BUILD.lock` as the lock.
- `ram_gate <GB>`: waits until free physical memory is at least that many GB, read through
  `powershell -NoProfile -Command "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory"`.

It also honours the Phase 0 convention: a file `E:/office-target/PAUSE` pauses the queue. Job counts: `-j 6` for
debug, `-j 4` for release (thin-LTO links), `-j 2` while the emulator or Docker runs.

---

## 2. Work packages

**Order and parallelism at a glance.** Arrows show dependencies. `‖` means the work can run in parallel *as
agent work*. Every cargo build or test still goes through the single build lock, because of the 11.8 GB of RAM.

```
WP0 → WP1 → WP2 → WP3 (W2 go/no-go) → WP5 (rebase to new BASE, sync script) → WP7/WP8/WP9 hooks → WP6 final → WP14
                     ‖ WP12 sizing (week 1)   ‖ WP11 corpus (if delivered)   ‖ WP10 locale check
                     ‖ WP6a CI skeleton (scripts only, from WP2)             ‖ WP4 craft-libs (night slots after WP3)
                     ‖ WP7a/WP9a designs (architect, no builds)              ‖ WP13 bind crate (after WP5)
```

Global rules for every WP:
- Source `scripts/env.sh`. No build output on C:.
- Never hand-edit `upstream/`. Upstream code changes are made in a cache worktree, `git format-patch`ed into
  `patches/<repo>/` and materialised by the queue scripts.
- Commits go on `phase-1`, with English messages and **no `Co-Authored-By` trailers** (CLAUDE.md owner rule).
- Update `docs/STATUS.md` at the end of each WP.
- Reports are conclusions with `file:line`.

---

### WP0. Bootstrap, toolchain components and CI tools
- **Goal.** Every later WP finds the same tools at known paths.
- **Inputs / deps.** None.
- **Steps.**
  1. Write `scripts/env.sh` and `scripts/lib/lock.sh` (§1.6).
  2. `rustup component add rustfmt clippy` (this changes the shared toolchain; it was deliberately not done in
     Phase 0. Record it in STATUS). Confirm `rustup target list --installed` includes `wasm32-unknown-unknown`.
  3. Download prebuilt Windows binaries into `E:/office-tools/bin` with `curl --ssl-no-revoke -L`: cargo-deny,
     cargo-nextest, sccache, actionlint. Record the exact versions and SHA-256 in `scripts/tools.lock`
     (`name version url sha256`). Fall back to `cargo install --locked --root E:/office-tools/cargo` only if a binary
     is missing.
  4. Write `scripts/bootstrap.sh`. It checks tool versions against `tools.lock`, creates the E: dirs, runs
     `fetch-fonts.sh` (from WP1 on) and starts the sccache server.
- **Files.** `scripts/env.sh`, `scripts/lib/lock.sh`, `scripts/tools.lock`, `scripts/bootstrap.sh`.
- **Done-check.** `bash -c '. scripts/env.sh; cargo fmt --version; cargo clippy --version; cargo deny --version;
  cargo nextest --version; sccache --version; actionlint --version; trunk --version'` prints seven version lines and
  exits 0. `trunk --version` → `trunk 0.21.14`.
- **Estimate.** 3 h (+≈1 h of downloads).
- **Risks.** TLS revocation errors (use `--ssl-no-revoke`). sccache on windows-gnu: if rustc wrapping fails, drop
  `RUSTC_WRAPPER` and record it; CI then relies on persistent target dirs only.
- **Model.** builder (sonnet).
- **Parallel / RAM.** No builds. It can overlap with WP12 reading.

### WP1. Convert clones into cache clones; vendor pristine snapshots at the OLD BASE
- **Goal.** The `upstream/` snapshots are committed, byte-exact, with tree hashes equal to upstream, at the Phase 0
  SHAs (`fd0bb2f`, `e6b6b34`, `9859da3`, `afa8e18`, `8f5a283`; craft-fonts `3d9ab3a` fetched). The old BASE is used
  so that WP2's queues apply unchanged and WP3 can be compared with the Phase 0 inventory.
- **Inputs / deps.** WP0.
- **Steps.**
  1. For each repo: `git -C upstream/<r> status --porcelain` must be empty. Record `rev-parse HEAD` and
     `git branch -a`.
  2. `mkdir -p /e/office-upstream`. Move each clone with
     `robocopy upstream\<r> E:\office-upstream\<r> /E /MOVE /NFL /NDL` (cross-drive). Then:
     - `git -C /e/office-upstream/<r> fsck --connectivity-only`;
     - `git -C /e/office-upstream/gridcraft worktree repair /e/office-work/gridcraft-spike /e/office-work/gridcraft-pdf`;
     - the same for wordcraft (`wordcraft-spike`, `wordcraft-b`);
     - `git -C … config core.autocrlf false` in each cache clone.
  3. In the monorepo: `git config core.autocrlf false`.
     - Write `.gitattributes`:
       ```
       * text=auto eol=lf
       /upstream/** -text
       /patches/** -text
       *.patch -text
       *.ttf *.otf *.png *.ico *.icns *.pdf *.wasm binary
       ```
     - Run `git add --renormalize .`. It must show no content changes to the existing docs; if it does, commit them
       separately as "normalise line endings".
     - Edit `.gitignore`: remove `/upstream/`; add `/upstream/craft-fonts/` and `/target/`.
  4. Run `import_pristine` (§1.4) for the five code repos, write `patches/<r>/BASE` (§1.5), and commit
     `vendor: pristine snapshots at Phase 0 BASEs` (one commit holding all five P subtrees).
  5. Write `scripts/fetch-fonts.sh`. It runs `git -c core.autocrlf=false -C $UPSTREAM_CACHE/craft-fonts archive <sha>
     | tar -x -C upstream/craft-fonts`, then verifies against `patches/craft-fonts/SHA256SUMS` (generate it on first
     run with `sha256sum`; commit it).
- **Files.** `.gitattributes`, `.gitignore`, `patches/{wordcraft,gridcraft,deckcraft,pdfcraft,craft-libs,craft-fonts}/BASE`,
  `patches/craft-fonts/SHA256SUMS`, `scripts/fetch-fonts.sh`.
- **Done-check.**
  ```bash
  for r in wordcraft gridcraft deckcraft pdfcraft craft-libs; do
    . <(grep -E '^(sha|tree)=' patches/$r/BASE)
    [ "$(git rev-parse HEAD:upstream/$r)" = "$tree" ] && [ "$(git -C $UPSTREAM_CACHE/$r rev-parse $sha^{tree})" = "$tree" ] && echo "$r OK"
  done; git status --porcelain upstream | wc -l; scripts/fetch-fonts.sh --verify
  ```
  Expected: five `<r> OK` lines, `0`, and `craft-fonts 3d9ab3a: 31 files OK`. Also
  `git -C /e/office-work/gridcraft-spike rev-parse HEAD` → `3884ffa…` (the worktrees still work).
- **Estimate.** 4 h.
- **Risks.**
  - Losing the spike branches. Mitigation: move, never delete, and verify with `branch -a` before and after.
  - Renormalising existing docs creates noise. Mitigation: a separate commit.
  - A wrong `read-tree` prefix. Mitigation: the done-check fails loudly.
- **Model.** builder.
- **Parallel / RAM.** No builds.

### WP2. Import the spike queues and the register at the OLD BASE
- **Goal.** `patches/wordcraft/0001–0005` and one ordered gridcraft queue `0001–0022`, with a verifier that replays
  them offline, and `patches/REGISTER.md` with 27 rows.
- **Inputs / deps.** WP1.
- **Steps.**
  1. Copy `spikes/wordcraft-patches/000{1..5}*.patch` to `patches/wordcraft/`.
  2. gridcraft, in a cache worktree:
     - `git -C $UPSTREAM_CACHE/gridcraft worktree add --detach $OFFICE_TARGET/wt/gridcraft-q e6b6b34`;
     - `git am` the patches `spikes/gridcraft-patches/0001…0019`;
     - then `git am -3 spikes/gridcraft-pdf-patches/000{1,2,3}*`, resolving the `crates/engine/src/lib.rs` overlap (F1),
       which is expected to be adjacent `mod` lines.
     Regenerate the whole series with `git format-patch --zero-commit --binary --no-signature -o patches/gridcraft
     e6b6b34..HEAD`. Names come out as `0001…0022-*.patch`. pdf-fonts become 0020–0022.
  3. Numbering convention, from now on:
     - `0001–0099`: functional patches from the spikes (mobile, memory, PDF);
     - `0100–0199`: the product overlay (brand, i18n, fonts), always applied last;
     - `0900–0999`: temporary CI or test fixes.
  4. Write `scripts/verify-queue.sh` (§5.2). Materialise commit Q for wordcraft and gridcraft (import the cache
     worktree's `HEAD^{tree}` with the §1.4 plumbing).
  5. Write `patches/REGISTER.md` (§5.3): 27 rows. Upstream status is "not sent (gate decision B-6, 2026-10-17)" for
     every row. Measured effects come from `docs/phase0/wordcraft-patches.md`, `gridcraft-templates-spike.md`,
     `gridcraft-memory-r2.md` and `gridcraft-pdf-fonts.md`.
  6. Write the `cargo xt register check` stub. It parses REGISTER.md and fails if a patch file has no row, or a row
     has no file.
- **Files.** `patches/wordcraft/*.patch`, `patches/gridcraft/*.patch`, `patches/REGISTER.md`,
  `scripts/verify-queue.sh`, `xtask/` (initial: `register check`), root `Cargo.toml`.
- **Done-check.** `scripts/verify-queue.sh all` prints `wordcraft: 5 patches, tree OK`,
  `gridcraft: 22 patches, tree OK`, `deckcraft: 0 patches, tree OK`, `pdfcraft: 0 patches, tree OK`,
  `craft-libs: 0 patches, tree OK`, and exits 0. `cargo xt register check` → `27 patches, 27 rows, OK`.
  `git diff --stat <P> <Q> -- upstream/gridcraft | tail -1` shows about 64 files and +3,170 lines, the sum of both
  series.
- **Estimate.** 4 h.
- **Risks.** The lib.rs merge is trivial (ESTIMATE). If the pdf-fonts series conflicts beyond lib.rs, record it as
  the first rebase-cost data point.
- **Model.** builder.
- **Parallel / RAM.** No builds. WP6a (scripts) can start once this lands.

### WP3. Prove a full-app domain build for every app (the W2 go/no-go) — first build task
- **Goal.** Show that `domains/{docs,sheets,slides,pdf}` each build the **desktop GUI, the web app (trunk) and the
  CLI** from the unmodified upstream app sources, with the upstream lock, `[patch]` and profiles. deckcraft and
  pdfcraft are included with their copied `[patch]` blocks. All of this happens at the old BASE plus the queue, so
  the results compare with `docs/phase0/inventory.md`.
- **Inputs / deps.** WP2. Design: §2a below.
- **Steps.**
  1. Write `cargo xt gen-domain <d>` (`toml_edit`).
     - **Inputs:** `domains/<d>/domain.toml`, `upstream/<app>/Cargo.toml`, `upstream/<app>/apps/<app>{,-web,-cli}/Cargo.toml`.
     - **Root `Cargo.toml` it writes:**
       - `[workspace] members = ["apps/*", "crates/*"]`, `resolver = "3"`;
       - `[workspace.package]` with our version `0.1.0`, `edition = "2024"`, and `rust-version` = upstream's;
       - `[workspace.lints]` copied verbatim;
       - `[profile.*]` copied verbatim;
       - `[patch.*]` copied, with `path = "vendor/x"` rewritten to `path = "../../upstream/<app>/vendor/x"`. Git
         patches (deckcraft krilla) are copied unchanged.
     - **Per-app `Cargo.toml` it writes:**
       - `[[bin]] name = "<our bin>"` and `path = "../../../../upstream/<app>/apps/<app>/src/main.rs"`;
       - dependencies copied from the upstream app manifest. `workspace = true` is resolved against upstream's
         `[workspace.dependencies]`, keeping features and target cfg tables. Internal `<app>-*` crates become
         `path = "../../../../upstream/<app>/crates/<dir>"`;
       - `build = "build.rs"`, pointing at **our** build.rs.
     - **Header:** `# GENERATED by cargo xt gen-domain <d> from upstream/<app>@<sha7>; edit domain.toml instead`.
     - `--check` mode exits 1 on drift.
  2. Write `cargo xt lockdiff <domain-lock> <upstream-lock>`. It passes only if every `(name, source)` present in both
     locks has the same version, the only additions are path packages of ours, and removals are allowed (upstream
     dev-deps, xtask).
  3. Seed the lock: `cp upstream/<app>/Cargo.lock domains/<d>/Cargo.lock; cargo metadata --manifest-path
     domains/<d>/Cargo.toml --format-version 1 >/dev/null`, then run lockdiff.
  4. Our files per app:
     - `build.rs`: a Windows icon plus VERSIONINFO from upstream's `winresource` (already in the lock). In WP3 it uses
       the upstream icon path; WP7 switches it to the overlay.
     - `index.html` + `Trunk.toml`: copied from upstream `apps/<app>-web/`, with `dist` and paths retargeted to
       `$OFFICE_TARGET/dist/<d>-web`. The brand edits happen in WP7.
     - `domains/slides/.cargo/config.toml`: not needed, because env.sh sets `CRAFT_FONTS_DIR` (F9).
  5. **Order of proof** (cheapest first, so a design failure costs ≤1 h):
     - (a) sheets desktop in **debug** (`cargo build -p sheets-desktop`): proves `[[bin]] path` outside the package
       and workspace inheritance through path deps;
     - (b) slides desktop release: deckcraft's git `[patch]`;
     - (c) pdf desktop release: 8 vendored `[patch]` paths;
     - (d) docs desktop release;
     - (e) all four web builds with `trunk build --release` from `domains/<d>/apps/<d>-web`;
     - (f) all four CLIs.

     Use `scripts/build.sh <d> desktop|web|cli [--release]` (lock, RAM gate, log to
     `$OFFICE_TARGET/logs/build-<d>-<what>.log`, wall time to `$OFFICE_TARGET/logs/times.tsv`).
  6. Smoke tests:
     - CLI `--version`, plus the inventory smoke command per app (inventory §wordcraft/§gridcraft/…; e.g.
       `docs-cli run --template sample --cmd format.bold --save out.docx`, then `convert out.docx out.pdf`);
     - GUI: start the exe, check it is still alive after 15 s (`Start-Process`, `HasExited -eq $false`), then
       `Stop-Process`;
     - web: the dist contains `.wasm` and `.js`; brotli the wasm with the same tool used for budgets (WP6a; §6.4).
  7. Write `docs/phase1/domain-build.md`: per app, the build times, exe and wasm sizes and deltas against the
     inventory, the lockdiff output, and **the decision D-A (keep) or D-B (fallback)**, with evidence.
- **Files.** `domains/*/domain.toml`, the generated `domains/*/Cargo.toml` and `domains/*/apps/*/Cargo.toml`,
  `domains/*/Cargo.lock`, `domains/*/apps/*/{build.rs,index.html,Trunk.toml}`, `scripts/build.sh`,
  `xtask` (`gen-domain`, `lockdiff`), `docs/phase1/domain-build.md`.
- **Done-check.**
  - `scripts/build.sh all all --release` exits 0.
  - `cargo xt gen-domain --check all` → `4 domains up to date`.
  - `for d in docs sheets slides pdf; do cargo xt lockdiff domains/$d/Cargo.lock upstream/$(cargo xt domain-upstream $d)/Cargo.lock; done`
    → four lines `0 version changes, N removed (dev/xtask), 0 foreign additions`.
  - `ls $OFFICE_TARGET/dist/*-web/*.wasm | wc -l` → `4`.
  - Each web wasm brotli is within ±0.5% of the inventory HEAD value (9,441,597 / 4,731,289 / 9,300,367 /
    9,390,759 B), with the gridcraft queue delta reported separately. The comparison is only like for like if the
    `CRAFT_FONTS_DIR` setting matches the Phase 0 build; record which setting was used.
- **Estimate.** 10 h of engineering + ≈3 h machine (cold release builds sum to about 2 h at the inventory times;
  pdfcraft's numbers are upper bounds). Risk buffer +8 h.
- **Risks.**
  - (1) Cargo may refuse a `[[bin]] path` outside the package. Step 5(a) finds out in 30 min. If it does, switch to
    **D-B** at once.
  - (2) Upstream main.rs uses `env!("CARGO_MANIFEST_DIR")`: pdfcraft `apps/pdfcraft/src/main.rs:982,1057,1110`,
    which look like test-only paths. These resolve differently under D-A, so the domain builds bins only and never
    runs app tests (tests run in the upstream workspace; §6).
  - (3) Feature unification may differ from upstream's `-p` build. lockdiff and the size comparison catch it.
  - (4) pdfcraft's build time is unknown on an idle machine (2,636 s was inflated).
- **Model.** builder. Escalate to **architect** if (a) fails, or if any `[patch]` resolves differently.
- **Parallel / RAM.** Serial builds only (build lock). Write the xtask while nothing builds.

#### 2a. Domain build design: D-A (recommended) vs D-B (fallback)

| | **D-A: domain wraps the upstream app source** | D-B: build in the upstream workspace |
|---|---|---|
| How | The domain package's `[[bin]] path` points at `upstream/<app>/apps/<app>/src/main.rs`. The domain owns the manifest (generated), build.rs, index.html and Trunk.toml. | `cargo build --manifest-path upstream/<app>/Cargo.toml -p <app>`. The domain workspace holds only our crates. |
| Brand files (exe icon, VERSIONINFO, web `<title>`, favicon, loading text) | **Ours, no patch** | Carried patches (≈3 extra per app) |
| Linking our crates into a product (Phase 2 translit command, Phase 4 shared services) | A domain manifest edit; the upstream lock is untouched | Patch upstream `Cargo.toml` **and** `Cargo.lock`, which conflicts at every sync |
| Cost | Generator plus drift check; app-level `CARGO_MANIFEST_DIR` semantics change | None |
| ADR match | Exactly ADR §2.1.2 W2 ("path-depend on an unmodified upstream app") | A degraded W2 |

D-A is recommended because Phase 2 already needs our crate inside Docs. The deciding trade-off is that D-A adds
generator complexity, and in return it never patches an upstream lockfile. **If WP3 step 5(a) fails, use D-B for all
apps.** It changes only WP7's patch count (+12) and WP3's files. It is not a reason to reopen W1.

---

### WP4. craft-libs single-rev experiment
- **Goal.** Measure whether all four apps compile and pass the tests of their craft-libs consumers against **one**
  craft-libs rev (ADR §2.1.2 dedupe). The result is a recommendation: adopt one rev in the domains, or keep the five
  upstream revs (`d27646a`, `c348732`, `5c874d5`, `666c358`, `f58bc1a`).
- **Inputs / deps.** WP3 (domains build). Best run after WP5, so it is at the new BASE; the done-check states which
  BASE was used.
- **Steps.**
  1. Scratch copies only. Use a monorepo worktree at `$OFFICE_TARGET/wt/craftlibs` with its own target dir.
  2. Per domain, add `[patch."https://github.com/storytold/craft-libs"] craft-ui = { path =
     "../../upstream/craft-libs/crates/ui" }` and the same for craft-print, craft-quit and craft-pdf. For deckcraft's
     krilla, use `[patch.crates-io] krilla = { path = "../../upstream/craft-libs/vendor/krilla" }`. Run
     `cargo check -p <d>-desktop` and `--target wasm32-unknown-unknown -p <d>-web`.
  3. To test the consumers: in a scratch worktree of the upstream app (cache clone, never `upstream/`), apply the
     same `[patch]` to its root `Cargo.toml` and run `cargo nextest run -p <app>-ui-egui` (plus the app crate). Count
     compile errors and test deltas against the same suite without the patch.
  4. Write `docs/phase1/craftlibs-single-rev.md`: a matrix of app × {check native, check wasm, tests} with error
     counts and the API breaks (`file:line`), and a recommendation.
- **Files.** `docs/phase1/craftlibs-single-rev.md`. No committed manifest change unless the recommendation is
  "adopt" **and** all cells are green. In that case `domain.toml` gets `craft_libs = "vendored"`, and gen-domain
  emits the `[patch]`.
- **Done-check.** The report has 4×3 cells filled with PASS/FAIL and numbers. `git status --porcelain domains/` is
  empty unless the decision is "adopt".
- **Estimate.** 5 h + ≈3 h machine.
- **Risks.** pdfcraft is pinned to three revs, so API drift is likely there. Expect a "keep 5 revs" outcome for now
  (ESTIMATE); that is an acceptable result.
- **Model.** builder.
- **Parallel / RAM.** Night slot; serial.

### WP5. `scripts/sync-upstream`, the dry run, the rebase to a fresh BASE, and the rebase cost
- **Goal.**
  - (a) A working `scripts/sync-upstream.sh` with `--dry-run` and `--apply` (§5).
  - (b) A **dry run** to a fresh BASE that succeeds.
  - (c) An **applied** sync, so every later Phase 1 patch is written against the fresh BASE.
  - (d) The **rebase cost per patch**, which is risk 1 and heavier now that every patch is carried.
- **Inputs / deps.** WP2 and WP3 (so the gates can build).
- **Steps.**
  1. Implement the script per §5.1. Gates: `quick` = verify-queue + gen-domain + lockdiff + deny + `cargo check` of all
     domains; `full` = quick + release desktop and web builds + debug tests of changed apps.
  2. Choose the new BASE: each repo's `origin/HEAD` at the moment of the run, frozen for the rest of Phase 1. Record it
     in the report. Exception: craft-fonts stays at `3d9ab3a` unless it moved.
  3. Run `scripts/sync-upstream.sh --dry-run --gates quick all`. Conflicts are **resolved in scratch** (`--resolve`
     mode). wordcraft by builder; gridcraft by **architect**, because commit 0002 touches 27 files and `recalc.rs`
     is +337 lines.
     - Log every patch to `patches/rebase-log.tsv` (§5.4).
     - Timebox gridcraft to **2 working days**. If it is exceeded: stop. Keep 0001–0019 out of the product queue
       (park them as `patches/gridcraft/parked/` with the register status "parked: rebase cost X h"), keep 0020–0022
       (pdf-fonts), and report this as an owner decision (the B-6 revisit or track E).
  4. Rebuild at the new BASE: `--gates full`. Run the debug tests of all four apps with the quarantine (WP6b,
     provisional). Then `--apply`: commits P′ and Q′ per repo, plus regenerated domains and locks.
  5. Write `docs/phase1/rebase-cost.md`:
     - per patch: result, conflicted files, minutes, resolution lines from the `git range-diff` interdiff, and whether
       a retest was needed;
     - per repo: total minutes, upstream commits crossed, churn on queue-touched files (`git diff --stat OLD NEW --
       <files>`), and **minutes per 100 upstream commits**;
     - a projection of weekly cost = rate × measured weekly commits (701–993; D4).
  6. Write `docs/UPSTREAM.md` (how to sync, carry and rebase; §5) from the script as built.
- **Files.** `scripts/sync-upstream.sh`, `patches/rebase-log.tsv`, `docs/phase1/rebase-cost.md`,
  `docs/UPSTREAM.md`, updated `patches/*/BASE`, `patches/*/*.patch`, `REGISTER.md` (last-rebase column), the domain
  manifests and locks.
- **Done-check.**
  - `scripts/sync-upstream.sh --dry-run --gates quick all; echo $?` → `0`. The report ends with
    `DRY RUN OK: 5 repos, 27 patches (clean A, 3-way B, resolved C), gates quick PASS`, and `git status --porcelain`
    stays empty (a dry run touches nothing).
  - After `--apply`: `scripts/verify-queue.sh all` passes at the new SHAs, and `scripts/build.sh all all --release`
    exits 0.
  - `wc -l patches/rebase-log.tsv` → `28` (header + 27).
- **Estimate.** 27 h: script 8; wordcraft 3; gridcraft 10 (4–20, **this is the measurement**); pdf-fonts 2;
  rebuild/test 2 h of engineering + 6 h machine; docs 2.
- **Risks.**
  - The gridcraft series conflicts heavily (that is the timebox above).
  - The new BASE needs a newer wasm-bindgen. trunk downloads the matching version; check network access.
  - The new BASE bumps `rust-version` above 1.97.1. Check this in step 2 of §5.1; if it happens, install the toolchain
    and record it.
  - A new HEAD breaks a test: quarantine it with class `upstream-bug` and evidence.
- **Model.** builder (script, wordcraft, pdf-fonts); **architect** (the gridcraft rebase, the cost report).
- **Parallel / RAM.** Serial builds. While the gridcraft rebase is in progress, writing WP7a/WP9a designs and WP6a
  scripts can run.

### WP6a. CI skeleton: the local script, the GitHub workflow, cargo-deny ×5 and size budgets
- **Goal.** `scripts/ci.sh` with the jobs in §6.1, a `.github/workflows/ci.yml` mirror, deny ×5 green, and
  `web/budgets.toml`.
- **Inputs / deps.** WP2 (queue scripts); WP3 (sizes); the budgets get their final numbers after WP7–WP9.
- **Steps.**
  1. `scripts/ci.sh <job>|quick|full|nightly`. Every job writes `$OFFICE_TARGET/ci/<run>/<job>.json` and appends a
     markdown summary. Exit codes: 0 green, 1 a blocking failure, 2 an infrastructure error.
  2. `deny.toml` per §6.6. Run `cargo deny --manifest-path <ws>/Cargo.toml --config deny.toml check` for the root
     and the four domains. Add every exception with a reason comment.
  3. `cargo xt sizes`: brotli (`brotli` crate, quality 11, lgwin 24) and gzip −9 for each `dist/*-web/*.{wasm,js}`;
     compare with `web/budgets.toml`. **Re-measure the pristine BASE wasm with the same tool** (one pristine web build
     per app per BASE, in the nightly or sync job), so the ratchet compares like with like.
  4. `cargo xt brand-grep` (§3.4). `cargo xt i18n lint` is a stub until WP9.
  5. `.github/workflows/ci.yml`: the same jobs, each calling `scripts/ci.sh <job>`; triggered by
     `workflow_dispatch` only. Actions are pinned by commit SHA when the workflow is enabled; they are listed by
     version in a comment for now. Lint it with `actionlint`.
- **Files.** `scripts/ci.sh`, `deny.toml`, `web/budgets.toml`, `.github/workflows/ci.yml`, xtask (`sizes`,
  `brand-grep`), `ci/web-smoke/` (Playwright spec reusing `spikes/docserver/e2e` setup: serve dist, load, wait for
  canvas, fail on console "failed to start", screenshot).
- **Done-check.**
  - `scripts/ci.sh deny` → five `ok` lines, exit 0.
  - `scripts/ci.sh sizes` → a table of 4 apps × {raw, gzip, brotli, budget, headroom}, exit 0.
  - `actionlint .github/workflows/ci.yml` → no output.
  - `scripts/ci.sh queue lint gen-check` exits 0.
- **Estimate.** 12 h.
- **Risks.**
  - The advisory-DB fetch hits the revocation error. Set `[advisories] git-fetch-with-cli = true` and
    `db-path = "E:/office-cache/advisory-db"`.
  - Unknown licence exceptions in the wordcraft or deckcraft graphs. Add them one by one with a reason; never allow a
    copyleft licence.
- **Model.** builder.
- **Parallel / RAM.** Script writing runs in parallel with WP3–WP5. Builds go through the lock.

### WP6b. Test execution: debug tests, nextest, quarantine, the deckcraft crash
- **Goal.** Honest green tests for all four apps in the **upstream workspaces** (patched trees), in debug, with a
  justified allowlist. Also the first measurement of debug-profile test time (ADR §7: unmeasured).
- **Inputs / deps.** WP5 (new BASE). Rerun after WP7–WP9 land.
- **Steps.**
  1. `ci/nextest.toml`:
     - profile `ci` with `retries = 0`, `fail-fast = false`, `slow-timeout = { period = "120s", terminate-after = 5 }`;
     - profile `flaky`, which has retries but applies only to quarantine class `flaky`;
     - a test group `gpu-serial` (`max-threads = 1`) for the egui_kittest wgpu suites if needed.

     Command per app: `cargo nextest run --workspace --profile ci --config-file ci/nextest.toml
     --manifest-path upstream/<app>/Cargo.toml`, with target `$OFFICE_TARGET/upstream-<app>` and the overlay env set
     (§3.2). Doctests run separately: `cargo test --doc --workspace`.
  2. Measure the cold and warm debug test time per app, with and without sccache. Record it in STATUS and in
     `docs/phase1/ci-times.md`.
  3. `ci/quarantine.toml` (§6.5).
     - Seed it with the Phase 0 failures (inventory): wordcraft 5; deckcraft 1 plus the crash; pdfcraft 14.
     - **Re-verify each at the new BASE.** Entries that now pass are dropped.
     - Debug-only failures (overflow checks; F-note §6.5) are triaged individually.
  4. The deckcraft `ui-egui` lib crash. nextest runs each test in its own process, which removes the in-process
     concurrency the crash needs (1 of 8 runs multi-threaded, 0 of 4 single-threaded). Run the suite 10×.
     - 0 crashes: record "not reproduced under process-per-test", and move the `cargo test` reproduction to a
       diagnostic note.
     - Still crashing: identify the test with `--no-capture` plus the last-started test, quarantine that test only
       with class `flaky-crash`, and put the binary in `gpu-serial`.
  5. `cargo xt quarantine apply <app>` turns quarantine.toml into a nextest filterset
     (`-E 'not (test(=…) | …)'`) and runs the quarantined tests in a separate pass, so that a quarantined test that
     now passes is reported as **stale**.
- **Files.** `ci/nextest.toml`, `ci/quarantine.toml`, xtask `quarantine`, `docs/phase1/ci-times.md`.
- **Done-check.** `scripts/ci.sh test all` exits 0 and prints, per app, `passed P, failed 0 (non-quarantined),
  quarantined Q (still failing Q1, stale Q2), crashed 0`. Each quarantine entry has non-empty `reason` and
  `evidence`: `cargo xt quarantine check` → `OK, N entries`.
- **Estimate.** 10 h + ≈8 h machine (cold debug test builds of four upstream workspaces; unmeasured).
- **Risks.**
  - Debug builds enable overflow checks, so new panics may appear that release hides. Each one is triaged as an
    upstream bug, never silently quarantined.
  - Process-per-test multiplies wgpu start-up, which can slow the ui-egui suites. Measure it; fall back to
    `cargo test` for those binaries if it is more than 2× slower.
  - Locale-dependent tests (ru OEM code page, localised labels) differ between this PC and GitHub runners, so entries
    carry an `env` key.
- **Model.** builder. **architect** reviews quarantine.toml (1 h) for honesty: no entry may hide a regression that our
  patch causes.
- **Parallel / RAM.** Serial, heavy. Run in night slots.

### WP7. Branding: brand config, overlay hook patches, our own assets, chrome removal
- **Goal.** All four apps (desktop and web) show the working name from `brand/brand.toml`, our placeholder icons and
  colours, no ArtCraft names or logos, no Discord, Share or Zotero chrome, no upstream update check, and an About
  attribution line. Every upstream `LICENSE`, `NOTICE` and `ATTRIBUTION` file stays.
- **Inputs / deps.** WP5 (patches are written on the new BASE); WP7a design before the per-app work.
- **Steps.**
  - **WP7a (architect, 3 h, no builds).**
    - Freeze the overlay contract (§3.2): the `overlay.env` key list, the generated `office_overlay.rs` API and the
      `rebrand()` rules.
    - Write `brand/brand.toml` (§3.1).
    - Write the per-app site lists from §3.3, re-grepped at the new BASE (`rg -n '"[^"]*(WordCraft|ArtCraft)'` …).
  - **WP7b (builder, 5 h).**
    - Crate `crates/office-brand`.
    - `cargo xt overlay <app>` writes `$OFFICE_OVERLAY_ROOT/<app>/overlay.env`, `icon-256.png`, `icon.ico` and
      `favicon.ico`.
    - `cargo xt brand icons` draws the placeholder icons with tiny-skia: a rounded square in the accent colour with a
      white pictogram (lines, grid, slide, folded page).
    - The domain `build.rs` (resources from the overlay), `index.html` (title, favicon, loading text from the
      overlay; `build.rs` cannot write `index.html`, so `scripts/build.sh` renders `index.html` from
      `index.html.in` before running trunk) and `scripts/build.sh` (sets `OFFICE_OVERLAY_DIR`).
  - **WP7c–f (builder per app, 5 h each, can be written in parallel, built serially).** One patch per app,
    `0100-<app>-office-overlay-hook.patch`:
    - (i) adds `crates/ui-egui/build.rs` (or extends the existing one) and `crates/ui-egui/src/office_overlay.rs`;
    - (ii) replaces each brand site in §3.3 with an overlay constant, or gates it with an overlay flag;
    - (iii) adds `office_overlay_tests.rs`. These egui_kittest tests open the main window and the About dialog, take a
      snapshot to `$OFFICE_EVIDENCE_DIR` when it is set, and assert that no accesskit label contains `Craft`, `ArtCraft`
      or `discord` when the overlay is active.

    A second patch, `0101-<app>-community-chrome-off.patch`, handles Discord, Share, Zotero and the update check
    where the gating touches more than one-token sites. It is kept separate so it can be dropped on its own.
  - **WP7g (builder, 2 h).**
    - Root `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` (our layer; NOTICE names storytold's apps and licences).
    - The About attribution text in `brand.toml` (the owner reviews the wording: nominative trademark use).
    - `cargo xt brand-grep` allowlist `brand/allowlist.txt`, with a reason per line (tests, log lines, code
      identifiers, file-format extensions).
- **Files.** `brand/brand.toml`, `brand/allowlist.txt`, `crates/office-brand/`, xtask (`overlay`, `brand icons`,
  `brand-grep`), `domains/*/apps/*/{build.rs,index.html.in}`, `patches/<app>/0100-*.patch`, `0101-*.patch`, root
  licence files, `docs/phase1/evidence/brand-<app>-{desktop,web}.png`.
- **Done-check.**
  - `scripts/ci.sh brand` → `brand-grep: 0 unallowlisted hits; overlay tests: 4/4 apps PASS`.
  - `strings -n 6 $OFFICE_TARGET/domain-docs/release/<bin>.exe | grep -c 'discord.gg'` → `0`; the same for the other
    three exes and the four wasm files.
  - `ls docs/phase1/evidence/brand-*.png | wc -l` → `8`, each showing the working name in the title bar or About
    dialog. The web evidence comes from the Playwright smoke, using the page title and a screenshot.
- **Estimate.** 28 h (3 + 5 + 4×5 + 2, minus overlap) ≈ 28 h.
- **Risks.**
  - **Carry cost**: the brand sites sit in hot files (`dialogs.rs`, `lib.rs`, `chrome.rs`, `backstage.rs`).
    Mitigation: one-token edits only, all logic in the new `office_overlay.rs`; measured in the second dry run (WP14).
  - Upstream tests that assert brand text fail with the overlay active: quarantine class `brand-overlay`, each listed.
  - `xtask assets` (pdfcraft) or `xtask layers` rejects the new build.rs or module: run both in CI from WP6a on.
- **Model.** architect (WP7a), builder (WP7b–g).
- **Parallel / RAM.** Four builders write patches in parallel in separate cache worktrees; builds are serial.

### WP8. Fallback UI and document font (U+02BB, U+02BC, Cyrillic)
- **Goal.** The text `Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №` renders without tofu in all four apps,
  desktop and web, in the UI and in gridcraft's PDF export.
- **Inputs / deps.** WP7 (the overlay hook carries the font registration).
- **Steps.**
  1. Font choice: **Noto Sans** Regular + Bold and **Noto Sans Mono** Regular (OFL-1.1). Download them from the
     upstream Noto release with `curl --ssl-no-revoke`. Record the version, URL and SHA-256 in `SOURCE.md`. Read the
     OFL header: if a Reserved Font Name is declared, rename the subset family to `OfficeFallback` (OFL §3).
  2. Subset with `python -I -m fontTools.subset` to U+0020–024F, U+02B0–02FF, U+0300–036F, U+0400–052F,
     U+2000–206F, U+20AC, U+20BD, U+2116, U+2122. Write a cmap check script that asserts every codepoint of the test
     text is present.
  3. The overlay lists `FALLBACK_FONTS`. Each app's hook appends them **last** in the Proportional and Monospace
     families. Never put them first, so upstream metrics stay unchanged:
     - wordcraft `theme.rs:473-482` (`font_definitions_language`), `:423-431` (`add_fallback_faces`);
     - gridcraft `theme.rs:359,381,388`;
     - deckcraft `theme.rs:165-169`;
     - pdfcraft `theme.rs:284-297`. It is the single builder used by all `set_fonts` sites.

     All four are scout-reported.
  4. gridcraft PDF: register the regular face via `pdf_fonts::register_font`, which exists after queue patches
     0020–0022, as the last default fallback on native and web.
  5. Tests in `office_overlay_tests.rs`:
     - `ctx.fonts(|f| f.has_glyphs(&FontId::proportional(14.0), TEXT))` and the same for monospace. Verify the egui
       0.36 API name in WP8; if it differs, use the font's `glyph_info` per char;
     - gridcraft: export a sheet with TEXT to PDF and extract it with `pdfcraft-cli text`; it must contain TEXT
       (U+02BB included).
  6. Measure the wasm delta per app.
- **Files.** `assets/fonts/fallback/*`, `scripts/subset-fallback-font.sh`, the overlay entries, test additions inside
  `0100-*` patches (or `0102-<app>-fallback-font.patch` if the 0100 patch grows over about 150 lines).
- **Done-check.**
  - `scripts/ci.sh test all` includes `office_overlay_tests::fallback_font_covers_uzbek` PASS in 4 apps.
  - `cargo xt sizes` shows the per-app font delta, expected to be under 250 KiB brotli each (ESTIMATE).
  - The Playwright screenshot `docs/phase1/evidence/font-<app>-web.png` shows `Oʻzbekiston` in the language picker.
- **Estimate.** 6 h.
- **Risks.**
  - Inter, which upstream uses, may lack some Uzbek Cyrillic letters. The fallback covers them anyway.
  - Hinting or metric differences when the fallback is used mid-word (U+02BB inside an Inter word). This is visible
    but acceptable; show it to the owner in the evidence.
  - pdfcraft's `tests/fonts.rs:297` stays as is, because uz is not in its hard-coded list (F10). Our own test covers
    uz.
- **Model.** builder.
- **Parallel / RAM.** Light builds.

### WP9. i18n: catalogs uz-Latn, uz-Cyrl, ru and en in each app's own mechanism, plus the review pipeline
- **Goal.** wordcraft, gridcraft and pdfcraft switch to uz-Latn and uz-Cyrl through their own language pickers;
  ru and en keep working. deckcraft gets a design and a carried patch that adds a mechanism and covers a P1 string
  subset (B-13). Every Uzbek string has a recorded status, and the owner receives review batches.
- **Inputs / deps.** WP7 (the overlay hook, extended for catalogs); WP8 (font).
- **Steps.**
  - **WP9a (architect, 3 h).** Freeze the canonical catalog format, the status model, the export mapping per app
    (§4) and `i18n/DRAFTING.md`.
  - **WP9b (builder, 8 h).** Crate `crates/office-i18n` and the xtask commands:
    - `i18n extract <app>`: the key universe from upstream at BASE. wordcraft and pdfcraft: the union of
      `(context, English)` over all upstream TSVs. gridcraft: the union of the match arms in all `i18n/*.rs` plus the
      `JAPANESE` table (`i18n.rs:178`). deckcraft: the `t!("…")` sites in the patched tree.
    - `i18n lint`: placeholders such as `{}`, `{0}`, `%s` and pdfcraft `@id` must match the English; the apostrophe
      rule (uz-Latn must use U+02BB for oʻ/gʻ and U+02BC for the tutuq belgisi; ASCII `'`, U+2018 and U+2019 there
      are errors); ellipsis `…`; font coverage against the fallback plus the upstream UI fonts; a length-ratio warning
      above 1.6× for ribbon and tab labels.
    - `i18n coverage`: per app and locale, keys / draft / reviewed / %.
    - `i18n review-export --batch N`: CSV, UTF-8 with BOM for Excel.
    - `i18n review-import <csv>`.
    - `i18n export <app> --mode review|release`: native files into the overlay dir. `release` = reviewed only;
      `review` = reviewed + draft.
  - **WP9c (builder, 3–4 h per app).**
    - `0110-<app>-office-languages.patch`: registers `uz-Latn` and `uz-Cyrl` from the overlay (§4.3–§4.5) and maps
      system locales `uz`, `uz-Latn-*` → uz-Latn and `uz-Cyrl-*` → uz-Cyrl.
    - pdfcraft also needs a `DATE_LANGUAGES` entry (Uzbek month and day names, Latin and Cyrillic, drafted and
      reviewed like the strings; test `mod.rs:3349`) and the plural rule `plural_one_other` (`mod.rs:53`).
  - **WP9d (builder, 3 h design + 8 h patch).**
    - `docs/phase1/deckcraft-i18n.md` (§4.6).
    - `0110-deckcraft-i18n-mechanism.patch` + `0111-deckcraft-i18n-p1-strings.patch`.
  - **WP9e (builder, 8 h).** Drafting.
    - `i18n/glossary.tsv` (about 150 terms). It goes to the owner first as batch 0.
    - Then uz-Latn drafts for the P1 subset of each app (§4.2), then uz-Cyrl derived from uz-Latn. The uz-Cyrl draft is
      produced by a deterministic throwaway script (`scripts/i18n/latn2cyrl-draft.py`, marked "draft aid, not
      uz-translit") and then reviewed.
    - ru drafts are needed for deckcraft only.
    - Every draft row gets `status=draft`, `by=agent:<model>`, `date`.
  - **WP9f (builder, 2 h).** Review batches 0–3 written to `i18n/review/` and handed to the owner (§4.2). Import
    the returned files.
- **Files.** `i18n/*`, `crates/office-i18n/`, xtask `i18n`, `scripts/i18n/latn2cyrl-draft.py`,
  `patches/<app>/0110-*.patch`, `patches/deckcraft/0111-*.patch`, `docs/phase1/deckcraft-i18n.md`,
  `docs/phase1/evidence/i18n-<app>-{uz-Latn,uz-Cyrl}.png`.
- **Done-check.**
  - `cargo xt i18n lint` → `0 errors`.
  - `cargo xt i18n coverage` prints, for each of the four apps × {uz-Latn, uz-Cyrl}, `keys K, draft D, reviewed R`,
    with P1 drafted at 100%.
  - The kittest `office_overlay_tests::switches_to_uz_latn_and_uz_cyrl` passes in all 4 apps. It runs the app's own
    language command (wordcraft `ui.language`, gridcraft `app.language.set`, pdfcraft its pref path, deckcraft the new
    command), asserts that a known P1 label shows the uz text (e.g. `File` → `Fayl` / `Файл`), and snapshots.
  - Playwright with `locale: 'uz-Latn-UZ'` and `'uz-Cyrl-UZ'` screenshots each web app.
  - 16 evidence PNGs.
- **Estimate.** 43 h.
- **Risks.**
  - **Owner review latency**. Mitigation: glossary first, P1 subset only, batches of at most 400 rows.
  - Upstream tests that iterate every catalog fail on incomplete uz (pdfcraft `mod.rs:829,948`, `object_ui.rs:418`,
    `sign_ui.rs:1537`; wordcraft `i18n/tests.rs:136`). Mitigation: put those key sets **into P1**, so the tests pass
    honestly. Whatever remains goes to quarantine class `i18n-incomplete`, with a count that must shrink.
  - Hot-file conflicts in the language tables: the hook adds entries in one place only.
- **Model.** architect (WP9a); builder (WP9b–f). Drafts: sonnet (haiku's Uzbek is too weak). Key extraction counts:
  scout.
- **Parallel / RAM.** Drafting and tooling do not touch the build lock. Patches are built serially.

### WP10. Locale decimal-separator check
- **Goal.** Measure and size how numbers, dates and formula entry follow the uz and ru locales, against Excel, and
  decide whether a patch belongs in Phase 1 or in fidelity (Phase 8).
- **Inputs / deps.** WP5 (new BASE). Independent of the other WPs.
- **Steps.**
  1. A matrix over the Windows user locales ru-RU (this PC), uz-Latn-UZ and en-US (switch through `Set-Culture`,
     restore afterwards) × {Excel 16, gridcraft desktop, gridcraft web} × the cells `3500000.5` formatted
     `General`, `#,##0.0`, `0.00%`, the date `2026-10-12`, and typing `3,5` and `=SUM(1;2)` into a cell. Record the
     displayed text. Excel goes through COM like `spikes/xlsx-bench/excel_ref.ps1`.
  2. Code map: `crates/numfmt/src/render.rs:158-159,198` (hard-coded separators), `crates/engine/src/display.rs`,
     `ui-egui/src/formula_locale.rs` (function names only, En and Es). Also check wordcraft fields (`PAGE` and date
     fields) and deckcraft date placeholders for locale.
  3. Design options: (a) a `NumLocale { decimal, group, list_sep }` threaded from the UI language or system locale
     into numfmt render and the editor parse; (b) display-only. Estimate each in engineer-days. Recommend one.
- **Files.** `docs/phase1/locale-numbers.md`, plus a proposed patch outline in REGISTER as status "proposed".
- **Done-check.** The report has the full matrix (3 locales × 3 apps × 6 cases = 54 cells, measured) and an
  estimate with a recommendation. `Get-Culture` is back to `ru-RU`.
- **Estimate.** 5 h.
- **Risks.** Changing the culture affects other running tools; do it with nothing else running and restore it in a
  `finally`.
- **Model.** builder.
- **Parallel / RAM.** No heavy builds. Not during the WP6b test nights.

### WP11. Corpus run, LibreOffice and x2t references, Excel census (only if `corpus/` is delivered)
- **Goal.** The first real fidelity numbers (B-15 promised them for week 1) and the Excel census (track E step 0).
- **Inputs / deps.** `corpus/` from the owner (B-14). Release CLIs from WP3 (old BASE is fine; state which).
- **Steps.**
  1. Pull Docker images for LibreOffice headless and ONLYOFFICE Document Server (x2t). They are used **only as
     reference renderers**, never vendored or linked (licensing §5.2). Wire up `libreoffice_reference` and
     `x2t_reference` in `spikes/fidelity/run.py`.
  2. Run `python -I spikes/fidelity/run.py --input corpus/ --out E:/office-target/fidelity-corpus --refs office,libreoffice,x2t`.
     Outputs carry ids only.
  3. Census: a read-only script over the large workbooks (sizes, formula counts, external links, sheets) → a RAM
     forecast per file (ADR §2.3 step 0).
  4. The report `docs/phase1/fidelity-corpus.md` holds **aggregates only**: no file names, no content (CLAUDE.md).
- **Files.** `docs/phase1/fidelity-corpus.md`, `docs/phase1/excel-census.md`, the run.py additions.
- **Done-check.** `grep -c . docs/phase1/fidelity-corpus.md` > 0. A `git grep` for any corpus file name in the repo
  returns nothing.
- **Estimate.** 8 h + machine (it depends on corpus size).
- **Risks.** Docker images are large: keep them on E: (Docker data root). Personal data: aggregates only.
- **Model.** builder.
- **Parallel / RAM.** Docker uses 1–2 GB, so no release builds at the same time.

### WP12. Phase 2–10 sizing (B-15, week 1)
- **Goal.** Estimates per phase that the ADR did not give, delivered in week 1 and refined at the gate.
- **Inputs / deps.** The ADR, the Phase 0 reports, and the early Phase 1 unit costs (WP3 build times, WP2/WP5 rebase
  minutes where available).
- **Steps.**
  - Per phase: a work breakdown into items of 0.5–5 days.
  - Unit costs: Phase 0 spike productivity (for example the wordcraft 5 patches, +651/−18, in about one day; the
    gridcraft templates spike at 865 production lines), build and test machine time, and the measured rebase cost.
  - A range for each item and the assumptions behind it, with the items marked UNSIZED where no evidence exists.
  - Put the A-web compact layout and selection handles in an explicit "unknown until Phase 4 sizing check" band.
- **Files.** `docs/phase1/sizing.md`.
- **Done-check.** The file has a table with rows for Phases 2–10 and columns {low, likely, high (engineer-days),
  main assumptions, evidence}. The total row is filled.
- **Estimate.** 6 h. Draft by day 5; refine at the gate.
- **Risks.** It invites false precision. Label every number ESTIMATE.
- **Model.** architect.
- **Parallel / RAM.** No builds.

### WP13. Keep approach B's binding layer compiling (ADR §2.2.1 step 2)
- **Goal.** `spikes/mobile-b/bind` moves into `domains/docs/crates/docs-bind`, and its 2 tests run in CI.
- **Inputs / deps.** WP5 (new BASE).
- **Steps.** Copy the crate, adapt its path deps to `../../../../upstream/wordcraft/crates/*`, and add it to the docs
  domain members. Run `cargo test -p docs-bind` and `cargo build -p docs-bind --target wasm32-unknown-unknown`. Fix
  API drift at the new BASE (that cost also counts as carry cost). Leave the spike copy where it is.
- **Files.** `domains/docs/crates/docs-bind/`.
- **Done-check.** `scripts/ci.sh test-ours` → `docs-bind: 2 passed`.
- **Estimate.** 2 h.
- **Risks.** Engine API drift since `fd0bb2f`.
- **Model.** builder.
- **Parallel / RAM.** A light build.

### WP14. Clean-checkout proof, second sync dry run, docs, final review, gate report
- **Goal.** The evidence for all four gate items, plus an adversarial review.
- **Inputs / deps.** All the other WPs. The second dry run needs BASE to be **at least 7 days old**.
- **Steps.**
  1. Clean checkout:
     ```bash
     git clone C:/Users/uge226/Desktop/office E:/office-clean
     cd E:/office-clean && scripts/bootstrap.sh && OFFICE_TARGET=E:/office-target/clean scripts/ci.sh full
     ```
     Run it once with sccache disabled (cold) to record honest times, and keep the log.
  2. The second dry run: `scripts/sync-upstream.sh --dry-run --gates full all`, rebasing to the then-current HEAD,
     which is about one week of upstream. Its rebase cost includes the brand, i18n and font patches. Append the
     result to `rebase-cost.md`.
  3. Update `docs/UPSTREAM.md` and `docs/STATUS.md`. The quarantine count goes in the gate report.
  4. **architect** final adversarial review of the plan's claims against the evidence: each gate item, licensing
     (every upstream LICENSE/NOTICE/ATTRIBUTION present: a `git ls-files upstream | grep -E 'LICENSE|NOTICE|ATTRIBUTION'`
     count equal to BASE), honesty of the quarantine, and brand completeness (a scout runs the brand sweep).
  5. The gate report in Uzbek, using the CLAUDE.md structure.
- **Files.** Logs under `E:/office-target/clean/`, `docs/phase1/gate-evidence.md`, the STATUS update.
- **Done-check.**
  - The clean-checkout `scripts/ci.sh full` exits 0 and prints `GREEN: queue lint deny gen-check brand i18n test×4
    build-desktop×4 build-web×4 sizes web-smoke×4`.
  - The second dry run prints `DRY RUN OK`.
  - The review has no open blocker.
- **Estimate.** 12 h + ≈5 h machine.
- **Risks.** A cold clean build takes longer than one working day. Start it in the evening.
- **Model.** builder (steps 1–3); scout (the brand sweep); **architect** (step 4).
- **Parallel / RAM.** Exclusive machine use during step 1.

---

## 3. Branding approach

### 3.1 Where the working name lives: `brand/brand.toml` (the only place)

```toml
[product]
id = "suite"                  # stable machine id: settings dirs, AppUserModelID prefix. Changed ONCE when B-1 is decided, before any user install
name = "Suite"                # neutral working name (B-1 deferred). Display only.
version = "0.1.0"
vendor = "Suite contributors" # VERSIONINFO CompanyName / Manufacturer (Phase 4 MSI)
attribution = "Based on WordCraft, GridCraft, DeckCraft and PdfCraft by storytold (MIT OR Apache-2.0)."  # owner reviews wording

[apps]
docs   = { upstream = "wordcraft", bin = "suite-docs",   cli = "suite-docs-cli",   aumid = "Suite.Docs",   data_dir = "Suite/Docs" }
sheets = { upstream = "gridcraft", bin = "suite-sheets", cli = "suite-sheets-cli", aumid = "Suite.Sheets", data_dir = "Suite/Sheets" }
slides = { upstream = "deckcraft", bin = "suite-slides", cli = "suite-slides-cli", aumid = "Suite.Slides", data_dir = "Suite/Slides" }
pdf    = { upstream = "pdfcraft",  bin = "suite-pdf",    cli = "suite-pdf-cli",    aumid = "Suite.Pdf",    data_dir = "Suite/Pdf" }

[names]   # per-app display names; Uzbek forms are drafts until the owner reviews them
docs   = { en = "Suite Docs",   ru = "Suite Документы",   uz-Latn = "Suite Hujjatlar",   uz-Cyrl = "Suite Ҳужжатлар" }
sheets = { en = "Suite Sheets", ru = "Suite Таблицы",     uz-Latn = "Suite Jadvallar",   uz-Cyrl = "Suite Жадваллар" }
slides = { en = "Suite Slides", ru = "Suite Презентации", uz-Latn = "Suite Taqdimotlar", uz-Cyrl = "Suite Тақдимотлар" }
pdf    = { en = "Suite PDF",    ru = "Suite PDF",         uz-Latn = "Suite PDF",         uz-Cyrl = "Suite PDF" }

[colors]  # placeholders until the owner chooses (Appendix B, Phase 1)
accent_light = "#0F766E"
accent_dark = "#5EC4B6"

[chrome]  # everything upstream-community is off
community = false
share = false
zotero = false
update_check = false
upstream_links = false
```

Rules:
- No other file may contain the working name; `brand-grep` enforces this.
- The **id** is separate from the name, so that choosing the real name (B-1) changes only display strings. The id
  itself changes once, before Phase 4 installs.
- The HRM "craft" identifiers (`client_id craft-*`, `aud:"craft"`) are not touched in Phase 1. They belong to ADR
  §5.1 and Phase 5. Note them in the gate report as still open.

### 3.2 Mechanism: the overlay

`cargo xt overlay <app>` writes `$OFFICE_OVERLAY_ROOT/<app>/`, which holds:
- `overlay.env`: `key=value` lines with no dependency needed to parse them: `APP_NAME`, `PRODUCT_NAME`, `APP_ID`,
  `DATA_DIR`, `COMMUNITY`, `SHARE`, `ZOTERO`, `UPDATE_CHECK`, `ATTRIBUTION`, `ACCENT_LIGHT`, `ACCENT_DARK`,
  `UPSTREAM_NAMES=WordCraft,ArtCraft`, and the `CATALOG_*` and `FONT_*` file entries;
- the icons;
- the catalogs (WP9);
- the fonts (WP8).

`scripts/build.sh` exports `OFFICE_OVERLAY_DIR=$OFFICE_OVERLAY_ROOT/<app>` and also `OFFICE_DATA_DIR` for crates
below the UI layer (see below).

Each app carries **one hook patch**, `0100-<app>-office-overlay-hook`:
- `crates/ui-egui/build.rs` reads `OFFICE_OVERLAY_DIR` (`cargo::rerun-if-env-changed` plus `rerun-if-changed` per
  file) and generates `$OUT_DIR/office_overlay.rs`, which holds:
  - `pub const ACTIVE: bool`;
  - the string and colour constants;
  - `pub static ICON_PNG: &[u8]` via `include_bytes!(<absolute path>)`;
  - `pub static EXTRA_LANGS` and `pub static FALLBACK_FONTS`.

  **When the variable is unset, the generated values are upstream's own** (the upstream name, flags `true`, no extra
  languages or fonts). A pristine-like build therefore behaves exactly like upstream.
- `crates/ui-egui/src/office_overlay.rs` holds `include!(concat!(env!("OUT_DIR"), "/office_overlay.rs"))` and
  `pub fn rebrand(s: &str) -> Cow<str>`. The function replaces `WordCraft`→`APP_NAME`, removes
  "Part of the Crafting Apps from the ArtCraft team"-style lines, and leaves other strings untouched.
- **Call sites become one-token edits**: `"WordCraft"` → `office_overlay::APP_NAME`; `tl!("About WordCraft")` →
  `office_overlay::rebrand(&tl!("About WordCraft"))`. Catalog keys stay upstream's, so ru and other translations keep
  working and get rebranded by the same `rebrand`.
- Crates below the UI layer (`xtask layers` forbids them from depending on ui-egui), such as wordcraft
  `control-key`, use `option_env!("OFFICE_DATA_DIR")` with the upstream fallback (`match` in a const). rustc tracks
  `option_env!` for rebuilds.
- App crates (`apps/<app>/src/main.rs`: window title, icon, app id) use `<app>_ui_egui::office_overlay::*`; ui-egui is
  already their dependency.

Patch vs hook, in short. **No upstream hook exists (F4)**, so every item below is a carried patch routed through the
overlay ("overlay"). Exceptions: "ours" means a domain-owned file with no patch under D-A, and "keep" means a code
identifier that is not user-visible.

### 3.3 What changes, per app (scout-reported `file:line` at Phase 0 HEADs; re-grep at the new BASE in WP7a)

**wordcraft** (patches 0100/0101)

| Item | Where | Action |
|---|---|---|
| Window title | `apps/wordcraft/src/main.rs:374` `with_title("WordCraft")`; centred title `crates/ui-egui/src/chrome.rs:350` | overlay `APP_NAME` |
| Window icon | `apps/wordcraft/src/main.rs:260,262` `include_bytes!` | overlay `ICON_PNG` |
| Exe icon + VERSIONINFO | `apps/wordcraft/build.rs:17-22` | **ours** (domain build.rs) |
| About dialog | `dialogs.rs:1266` (title), `:2383-2395` (name, "Part of the Crafting Apps…", copyright line, website `:2390`, Discord `:2391`, GitHub `:2392`) | overlay: name and `ATTRIBUTION`; remove the website and Discord links; the upstream copyright line moves to NOTICE (licence kept) and About shows the attribution |
| Discord pill | `chrome.rs:296-307` (gated by the user pref `show_discord`, `lib.rs:259`, default `true` `lib.rs:354`) | overlay: `if COMMUNITY && app.ui.show_discord`; hide the option checkbox `options.rs:193` when `!COMMUNITY` |
| Discord in Help group / Backstage / URL | `ribbon.rs:2004-2006`, `backstage.rs:121`, `lib.rs:1496` | 0101 gate on `COMMUNITY` |
| Share button | `ribbon.rs:228-241` | 0101 gate on `SHARE` |
| Zotero tab and panel | tab list `crates/engine/src/ribbon.rs:19`; dispatch `ui-egui/src/ribbon.rs:371`; panel `zotero.rs:228` | 0101: filter the tab in ui-egui at render (no engine edit) when `!ZOTERO` |
| Web start error | `apps/wordcraft-web/src/web.rs:79` "WordCraft failed to start" | overlay `rebrand` |
| Web title, favicon, loading text | `apps/wordcraft-web/index.html:6,7,18` | **ours** (domain index.html.in) |
| Default-apps registration | `apps/wordcraft/src/default_apps.rs:87` `registeredAppMachine=WordCraft` | overlay `APP_ID` (the MSI must match in Phase 4) |
| Settings, key and log dir | `crates/control-key/src/lib.rs:7,17` (`%APPDATA%\WordCraft`) | `option_env!("OFFICE_DATA_DIR")` |
| Accent colour | `crates/ui-egui/src/theme.rs:299` (`APP_COLOR`), `:341` (dark) | overlay `ACCENT_*` |
| Packaging (wxs, Linux, macOS) | `packaging/windows/wordcraft.wxs:20-23,87-147` etc. | not built in Phase 1; **ours** in Phase 4 (suite MSI) |
| Crate names `wordcraft-*`, `.docx` handling, MCP ids | — | keep (code identifiers) |

**gridcraft**

| Item | Where | Action |
|---|---|---|
| Window title, app id | `apps/gridcraft/src/main.rs:312`, `:315` (`ai.storyteller.gridcraft`) | overlay `APP_NAME`, `APP_ID` |
| Window icon | `main.rs:225,227` | overlay `ICON_PNG` |
| About | `crates/ui-egui/src/dialogs.rs:677` (title), `:1486-1491` ("Part of ArtCraft.", getartcraft.com, Discord) | overlay name + attribution; links removed (0101) |
| Start dialog, message-box titles | `dialogs.rs:779,924`; `lib.rs:541,692,769,912,946,1078`; `backstage.rs:891-924`; `formula_bar.rs:130` | overlay `APP_NAME` |
| Help menu website + Discord | `apps/gridcraft/src/native_menu.rs:177-178` | 0101 gate on `UPSTREAM_LINKS`/`COMMUNITY` |
| Web | `apps/gridcraft-web/index.html:6,7,18` (ours); `web.rs:78` error text (overlay) | as wordcraft |
| Config, log, data dirs | `apps/gridcraft/src/main.rs:76-85,97,109-112` | overlay `DATA_DIR` |
| Accent | `crates/ui-egui/src/theme.rs:65,66,107` | overlay |
| "Share" | `ribbon.rs:132` is Export / save a copy | **keep** (not social chrome) |

**deckcraft**

| Item | Where | Action |
|---|---|---|
| Window title, app ids | `apps/deckcraft/src/main.rs:200,235`, `APP_ID` `:127` | overlay |
| Presenter view title | `crates/ui-egui/src/show.rs:1757` | overlay `rebrand` |
| About | `lib.rs:395` (command label), `menus.rs:292`, `backstage.rs:1333-1368` (`:1359` header), `dialogs.rs:50,2610-2620` (copyright `:2620`, website `:2617`, Discord `:2616`), drawn mark `dialogs.rs:2639` | overlay name and attribution; remove links and the drawn mark (0101) |
| Discord and links | `crates/engine/src/links.rs:3` (+ WEBSITE / APP_PAGE / GITHUB), used at `lib.rs:1053`, `backstage.rs:737,1374` | 0101 gate on `COMMUNITY`/`UPSTREAM_LINKS`. `crates/mcp/src/server.rs:141-143` is not UI: keep, register as a later item |
| Error and status strings | `lib.rs:194,747`, `dialogs.rs:3220,3235`, `backstage.rs:1043,1045,1351`, `main.rs:220` | overlay `rebrand` (about 31 literal lines) |
| Built-in theme named "DeckCraft" | `backstage.rs:540`, `dialogs.rs:277,285,468,485` | overlay `rebrand` of the display name only (the saved theme id stays) |
| Native format label | `backstage.rs:216`, `main.rs:106-107` "DeckCraft Presentation (*.deckcraft)" | overlay `rebrand` of the label; the `.deckcraft` extension stays (file format) |
| Web | `apps/deckcraft-web/index.html:6,8,18` | **ours** |
| Exe resources, icon | `apps/deckcraft/build.rs:17-22` (ours); `main.rs:165` (overlay) | |
| Settings dir | `main.rs:55-68` | overlay `DATA_DIR` |
| Accent | `crates/ui-egui/src/theme.rs:34,63,91` | overlay |

**pdfcraft**

| Item | Where | Action |
|---|---|---|
| Window title, per-document title, app id | `apps/pdfcraft/src/main.rs:310,249`, `APP_ID` `:49,315`; `crates/ui-egui/src/lib.rs:3090-3091` | overlay |
| **ArtCraft logo and mark (trademark)** | `widgets.rs:503` (`artcraft_logo`), `:514` (`artcraft_mark`); used in `dialogs.rs:1156,1174` and `home.rs:40-43` | 0101: remove the calls and do not compile the logo bytes (the source-level check in brand-grep forbids `docs/brand` in any `include_bytes!`) |
| Community card, Discord button | `home.rs:40-43`; `chrome.rs:302` (ungated) | 0101 gate on `COMMUNITY` |
| **Update check** | `apps/pdfcraft/src/updates.rs:7`; `crates/ui-egui/src/updates.rs:15` | 0101: disabled when `!UPDATE_CHECK`; no network call (an overlay test asserts it) |
| Links, Help menu | `crates/engine/src/links.rs:10-13`; `commands.rs:373,376` | 0101 gate |
| Other "PdfCraft" strings | about 20 ui-egui files (`lib.rs` 17 hits; `content_ui.rs:50-53`, …) | overlay `rebrand` at display sites |
| Storage dirs and keys | `eframe::storage_dir("PdfCraft")` `main.rs:65,221,249`; `recovery.rs:156`; storage key `lib.rs:2913`; web fallback `apps/pdfcraft-web/src/main.rs:27` | overlay `DATA_DIR` (keep the migration code paths) |
| Web | `apps/pdfcraft-web/index.html:6` (no favicon upstream) | **ours** (adds a favicon) |
| Exe resources | `apps/pdfcraft/build.rs:14-27` | **ours** |
| Accent | `theme.rs:89,113` | overlay |
| Signing script | `packaging/windows/sign.ps1:58` | not built in Phase 1 (Phase 10) |

### 3.4 Brand checks in CI
- `cargo xt brand-grep` scans the **patched** `upstream/*/crates/ui-egui/src`, `apps/*/src` and our domain files for
  string literals matching `(?i)(word|grid|deck|pdf|art)craft|storytold|getartcraft|discord\.gg`. Hits are allowed
  only when they pass through `office_overlay::` on the same line, or are listed in `brand/allowlist.txt` with a
  reason (tests, logs, MCP ids, file-format ids).
- It also fails on any `include_bytes!`/`include_str!` whose path contains `docs/brand/`.
- The runtime checks are the `office_overlay_tests` (kittest accesskit labels) and the evidence PNGs.

---

## 4. i18n approach

### 4.1 Principles
- **Each app keeps its own mechanism** (ADR, the brief). Our catalogs reach each app through the overlay, so catalog
  data is **never in the patch queue**. Only a small registration hook is carried.
- Locales: `uz-Latn`, `uz-Cyrl` (new); `ru` and `en` already exist upstream for wordcraft, gridcraft and pdfcraft.
  Our `i18n/<app>.tsv` keeps optional `ru` overrides, initially empty. deckcraft gets all four.
- **Fallback for a missing string = upstream behaviour: the English source** (wordcraft `i18n/mod.rs:375,403`
  `unwrap_or(s)`; gridcraft `i18n.rs:133`; pdfcraft `mod.rs:438-439`). uz never falls back to ru (mixed scripts would
  confuse). Whether to fall back to ru instead is an owner decision (§9).
- Release vs review builds: `i18n export --mode release` includes only `reviewed` rows; `--mode review` includes
  drafts. **Phase 1 gate evidence is produced from review builds and labelled "DRAFT Uzbek strings: N reviewed / M
  drafted".** The Uzbek strings are never presented as trusted.

### 4.2 Canonical catalogs, drafting and review

`i18n/<app>.tsv`: UTF-8, LF, no BOM, sorted by id. Columns:
```
id  context  source_en  uz_latn  uz_latn_status  uz_cyrl  uz_cyrl_status  ru  ru_status  priority  by  date  note
```
- `id`: the first 12 hex digits of SHA-1 over `context \x1f source_en`. It stays stable while the English stays.
- Status values: `todo | draft | reviewed | rejected | obsolete`.
- `priority`: `P1 | P2`.
- `by`: `agent:sonnet` / `owner`.

**P1 subset**, about 1,350 strings in total:
- ribbon tabs and groups, File/Backstage, the common dialogs (Open, Save, Print, Find, Options), the status bar,
  context menus and error messages;
- **plus the key sets that upstream "every catalog" tests require** (WP9 risks).

Target sizes: wordcraft ≈400 of ≈2,600; gridcraft ≈400 (all of its 398 ru arms); pdfcraft ≈400 of ≈2,000–2,600;
deckcraft ≈150.

Pipeline:
1. **Batch 0, the glossary** (about 150 terms, en → uz-Latn → uz-Cyrl → ru) goes to the owner first. Every draft
   must follow it, which is the largest lever for consistency.
2. Drafts are produced by a sonnet builder following `i18n/DRAFTING.md`:
   - follow the glossary;
   - official orthography: U+02BB in oʻ/gʻ and U+02BC for the tutuq belgisi;
   - keep placeholders and keytips;
   - stay within 1.6× the English length for labels;
   - copy no strings from Microsoft, ONLYOFFICE or LibreOffice UIs (clean room; general vocabulary is fine).

   Every draft is marked `draft` with `by=agent:…`. uz-Cyrl drafts come from uz-Latn through
   `scripts/i18n/latn2cyrl-draft.py`, a throwaway draft aid that is not the Phase 2 `uz-translit`.
3. Hand-off: `cargo xt i18n review-export --batch N` writes `i18n/review/batch-NN.csv` (UTF-8 BOM, for Excel). Columns:
   ```
   id, app, where (context + one source file:line), English, uz-Latn draft, uz-Cyrl draft, verdict (OK/FIX), corrected uz-Latn, corrected uz-Cyrl, comment
   ```
   Batches hold at most 400 rows. **Two passes**: meaning (uz-Latn), then spelling (uz-Cyrl). ESTIMATE for the owner:
   about 4–6 h for all P1 rows.
4. `review-import` sets `reviewed`/`rejected`, `by=owner` and the date. Rejected rows are re-drafted and appear in
   the next batch.
5. Coverage numbers go to STATUS and the gate report.

### 4.3 wordcraft (TSV)
- Mechanism: `LANGUAGES: [LangInfo; 23]` at `crates/ui-egui/src/i18n/mod.rs:55`, entries `:61-165` (`include_str!`).
  TSV is `context<TAB>English<TAB>translation` (header `ru.tsv:1-13`). Lookup is `tl!`/`t`/`tr`/`tc`/`fmt` with the
  English key.
- Hook (0110): extend the array to `[LangInfo; 25]`. The two new entries take `source` from the generated
  `office_overlay::catalog("uz-Latn")`, which is an empty catalog when no overlay is set. The system-locale mapping
  goes in `system_lang()` `mod.rs:322`.
- The switch is reached through File › Options › General › "Interface language" (`options.rs:616-630`, command
  `ui.language`).
- Tests to watch: `i18n/tests.rs:117` `bundled_catalogs_are_well_formed` (format), `:136`
  `every_tab_and_command_is_translated` (put those keys in P1). Add our own `uz_*_covers_p1` test.

### 4.4 gridcraft (Rust tables)
- Mechanism: `enum Language` at `crates/ui-egui/src/i18n.rs:24-36` (`ALL :39`, names `:44`, codes `:60`,
  `from_tag :81`, `system() :100`, `tr :119-131`, fallback `:133`, `saved_language :156`). Tables are
  `fn translate(&str) -> Option<&'static str>` with a `match` (ru 398 arms).
- Hook (0110): add variants `UzLatn` and `UzCyrl`, and codes `uz-Latn` and `uz-Cyrl`. Their `translate` comes from
  build.rs, which generates a `match` from the overlay's exported `uz-Latn.tsv`, so no Rust source is ever exported.
  Map the tags in `from_tag`; on web, map the browser language in `apps/gridcraft-web/src/web.rs:54-61`. Font
  selection per language (`theme.rs:381`) gets the fallback face.
- The switch is reached through the View tab › "Interface language" (`ribbon.rs:2222-2236`) and Backstage
  (`backstage.rs:953`); the command is `app.language.set` (`lib.rs:626`).
- Not touched: `formula_locale.rs` (function names En/Es only). Localised function names and argument separators are
  out of Phase 1; record them in WP10.

### 4.5 pdfcraft (`tl!` TSV)
- Mechanism: `tl!` at `crates/ui-egui/src/lib.rs:17-21` → `i18n::t`; `LANGUAGES: [LangInfo; 23]` at `mod.rs:150-212`;
  plural functions `mod.rs:53…`; `detect_system_lang` `:345-365`; fallback `:438-439`.
- Hook (0110):
  - two entries with `plural_one_other`;
  - a `DATE_LANGUAGES` entry (the test at `:3349` requires it to equal `LANGUAGES`); the Uzbek month and day names
    are catalog rows like any other (status-tracked);
  - the locale mapping in `candidates()` `:302`.
- In-tree catalogs would need `ATTRIBUTION.toml` sha256 entries (`docs/localization.md:83-89`). The overlay avoids
  that, because the catalogs are not in the tree.
- The switch is reached through Preferences › "Interface language" (`js_ui.rs:265-280`).
- Tests that iterate every catalog: `mod.rs:829,948`, `object_ui.rs:418`, `sign_ui.rs:1537`, `icons.rs:82` → their
  key sets go into P1.

### 4.6 deckcraft (no mechanism; B-13: a design plus a carried patch)

The design goes in `docs/phase1/deckcraft-i18n.md`.
- **Mechanism.** Copy wordcraft's model: English-key TSV and `t!("…")` returning `&'static str` from a `OnceLock`
  catalog. This lets our exporter and reviewers reuse one format.
- **Patch 0110** adds `crates/ui-egui/src/i18n.rs`:
  - the loader and the overlay catalogs (uz-Latn, uz-Cyrl, ru);
  - a language preference in the existing settings (`crates/ui-egui/src/settings.rs`);
  - a picker in Backstage › Options;
  - a command `app.language.set`.
- **CommandSpec labels** stay `&'static str` in the engine (`crates/engine/src/cmd/mod.rs:46,63`, macro arms
  `:149-156`). They are translated **at display sites** (the `lib.rs:395-414` table, `ribbon.rs:260` `tip(id,label)`,
  menus), so the engine is untouched.
- **Patch 0111** wraps the P1 literals (ribbon `big(app …)` ×49 in `ribbon.rs`, `menus.rs`, the main Backstage pages,
  About, common buttons; about 150 strings). The remaining ≈572 UI call lines stay English, which the owner accepts
  at the gate (B-13). Estimate full coverage in `sizing.md`.
- **Carry risk is high.** `ribbon.rs` and `backstage.rs` are hot, but every edit is a one-token wrap, so conflicts are
  trivial to resolve. Measure it in the second dry run.

### 4.7 The U+02BB font

See WP8: subset Noto Sans and Noto Sans Mono, append them last in every app's font families, register the regular
face in gridcraft PDF (`pdf_fonts::register_font`), and test coverage with `Fonts::has_glyphs` plus PDF text
extraction. `i18n lint` refuses any catalog character the fonts cannot draw.

---

## 5. Sync procedure

### 5.1 `scripts/sync-upstream.sh [--dry-run|--apply] [--resolve] [--to <sha>] [--gates quick|full] <repo…|all>`

| Step | What it does | Output |
|---|---|---|
| 0 Preflight | Monorepo clean (`git status --porcelain` empty); env; build lock; RAM gate 4 GB | abort with code 2 if not |
| 1 Fetch | `git -C $UPSTREAM_CACHE/<r> fetch --prune --tags origin`. NEW = `--to`, else the newest tag that is newer than BASE, else `origin/HEAD`. Count `rev-list --count BASE..NEW`. Churn on queue files: `git diff --stat BASE NEW -- $(files touched by the queue)` | report §1 |
| 2 Signals | `git diff BASE NEW --` over `docs/decisions.md docs/gaps.md docs/roadmap.md *parity*.md CLAUDE.md AGENTS.md Cargo.toml .cargo/config.toml rust-toolchain*`. Flags: `rust-version` change, `[patch]`/`[profile]`/`[workspace.lints]` change, wasm-bindgen or egui version bump, new apps or crates under `apps/*` | report §2 |
| 3 Rebase | `git -C cache worktree add --detach $OFFICE_TARGET/sync/<r>-<new7> NEW`; for each patch `git am -3 <p>`, timed. Clean / 3-way auto / conflict. A conflict: in `--dry-run` without `--resolve` → stop, report `CONFLICT at NNNN (files)`; with `--resolve` → wait for the resolver (an agent), `git am --continue`, record minutes. Regenerate with `git format-patch --zero-commit --binary --no-signature -o $scratch/patches NEW..HEAD`; `git range-diff BASE..OLDQ NEW..HEAD` → interdiff lines per patch | `rebase-log.tsv` rows; regenerated queue in scratch |
| 4 Materialise in scratch | A monorepo worktree at `$OFFICE_TARGET/sync/mono-<id>` (detached at HEAD). Import pristine NEW (commit P′), import the rebased tree (commit Q′), copy the regenerated patches and BASE, run `cargo xt gen-domain all`, re-seed `domains/*/Cargo.lock` from the new upstream locks, `cargo xt lockdiff`, `cargo deny ×5` | scratch branch `sync/<date>` |
| 5 Gates | **Separate target dirs** under `$OFFICE_TARGET/sync/target-*` (a Phase 0 lesson: two worktrees must not share a target dir). quick: verify-queue, gen-check, lockdiff, deny, `cargo check` per domain (native + wasm). full: + release desktop and web builds, sizes vs budgets, debug tests (nextest + quarantine) for every repo whose BASE moved, the web smoke; + (nightly) fidelity if the corpus exists, `xlsx-bench` w10/w40 peak RAM for gridcraft (deterministic to the MB) | report §5, PASS/FAIL per gate |
| 6 Apply / hold | `--dry-run`: delete the scratch worktrees; write the report to `$OFFICE_TARGET/sync/reports/<date>.md`; **the monorepo is untouched**. `--apply` (only if every gate passes): `git merge --ff-only sync/<date>`, update `REGISTER.md` (last rebase), the pristine size baselines in `web/budgets.toml` and STATUS; commit the report to `docs/phase1/sync-reports/`. If a gate fails, the pin stays and a STATUS issue line is written | — |

**Dry-run definition.** A dry run **succeeds** when steps 0–5 complete for every selected repo with the monorepo
untouched (`git status --porcelain` empty before and after, and HEAD unchanged), every patch applied (clean, 3-way,
or resolved in scratch with `--resolve`), and every selected gate passes. A dry run that stops at an unresolved
conflict is a **correct failure** of the tool, with a report; it is not a success. The Phase 1 gate needs one
successful dry run (WP5) and one more at least 7 days later (WP14).

### 5.2 `scripts/verify-queue.sh <repo|all>` (offline, seconds; CI job `queue`)

```bash
. <(grep -E '^(sha|tree)=' patches/$r/BASE)
git cat-file -e "$tree" || fail "BASE tree $tree not in repo (pristine commit missing)"
export GIT_INDEX_FILE=$(mktemp); git read-tree --prefix=upstream/$r/ "$tree"
for p in patches/$r/[0-9][0-9][0-9][0-9]-*.patch; do git apply --cached --directory=upstream/$r "$p" || fail "$p"; done
[ "$(git rev-parse "$(git write-tree):upstream/$r")" = "$(git rev-parse HEAD:upstream/$r)" ] || fail "tree != BASE+queue"
git diff --quiet HEAD -- upstream/$r && [ -z "$(git ls-files --others --exclude-standard upstream/$r)" ] || fail "hand-edited"
```

### 5.3 Patch register (`patches/REGISTER.md`)

One markdown table, with fixed columns, parsed by `cargo xt register check`:

| id | file | subject | files / +/− | category | why | measured effect | upstream PR | carried since | last rebase | drop when |
|---|---|---|---|---|---|---|---|---|---|---|
| wordcraft/0003 | `0003-ui-egui-handle-ImeEvent-DeleteSurrounding-so-Android.patch` | IME DeleteSurrounding | 1 / +37 | mobile | Gboard autocorrect appended the word | `teh `→`the ` (was `tehthe`) | not sent (B-6, 2026-10-17) | 2026-10-15 @ fd0bb2f | 2026-10-2x @ <new7>: clean, 2 min | upstream handles `ImeEvent::DeleteSurrounding` |

- Categories: `mobile | memory | pdf | brand | i18n | font | ci-fix | parked`.
- The "upstream PR" column always reads "not sent" while B-6 stands. The column stays so that a later B-6 revisit
  needs no reformatting.
- `register check` fails if: a file has no row, a row has no file, a cell is empty, or "last rebase" is older than
  the current BASE date.

### 5.4 How rebase cost is measured

`patches/rebase-log.tsv` is append-only, one row per patch per sync:
```
date  repo  from_sha  to_sha  upstream_commits  patch  result(clean|3way|conflict|parked)  conflicted_files  conflict_hunks  minutes  resolution_lines  rebuild_s  retest(pass|fail)  resolver(model)
```
- `minutes` is wall time between the conflict and `am --continue`, by the resolving agent. Agent time is not human
  time, so the report gives both the agent minutes and the resolution size.
- `resolution_lines` = the interdiff lines from `git range-diff`.
- Derived numbers, in `docs/phase1/rebase-cost.md`:
  - minutes per sync per repo;
  - **minutes per 100 upstream commits**;
  - the share of patches that are clean;
  - the churn on queue files per week;
  - the hot files (patches conflicting in two consecutive syncs).
- Data points in Phase 1:
  - WP5: 27 patches over about 4–7 days of upstream;
  - WP14: about 35 patches (with brand, i18n and font) over about 7 days.

  Two points are not a trend, so the gate report says so.

---

## 6. CI design

### 6.1 Jobs (`scripts/ci.sh`; the same names in `.github/workflows/ci.yml`)

| Job | What | Blocking | Time budget (this PC) |
|---|---|---|---|
| `queue` | verify-queue all; `cargo xt register check` | yes | < 1 min |
| `lint` | root workspace `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`; each upstream tree `cargo fmt --all --check` (our patches must be fmt-clean), `cargo xtask layers`, `cargo xtask assets` (where present); upstream `clippy -D warnings` **advisory**, as a count ratchet (a newer clippy than upstream's CI may add lints we did not cause) | yes, except upstream clippy | ≤ 10 min warm |
| `gen-check` | `cargo xt gen-domain --check all`; lockdiff ×4 | yes | < 1 min |
| `deny` | cargo-deny ×5 (root + 4 domains) | yes | ≤ 5 min |
| `brand` | brand-grep + logo-include check | yes | < 1 min |
| `i18n` | `i18n lint`; coverage report (no fail on coverage, only on errors) | yes (lint) | < 1 min |
| `test <app>` | upstream workspace debug tests, nextest + quarantine, overlay ON; + doctests | yes | budget set from the WP6b measurement; target ≤ 30 min warm per app |
| `test-ours` | root workspace tests + domain crates (docs-bind) | yes | ≤ 5 min |
| `check <domain>` | `cargo check` desktop + wasm32 web | yes | ≤ 10 min warm |
| `build <domain> desktop\|web\|cli` | release builds via `scripts/build.sh` | yes in `full` | cold 323–897 s per artefact (inventory), warm ≤ 5 min |
| `sizes` | brotli/gzip vs `web/budgets.toml` | yes | < 2 min |
| `web-smoke` | Playwright: each dist loads, no "failed to start", screenshots | yes in `full` | ≤ 5 min |
| `actionlint` | workflow lint | yes | < 10 s |
| `release-tests` (nightly) | `cargo test --release` per app with the quarantine (cached test binaries) | yes in `nightly` | cold ≈6.6 h total (PROJECTION from inventory), cached 205–465 s per app |
| `fidelity`, `xlsx-bench` (nightly) | corpus harness if present; gridcraft w10/w40 peak RAM vs the last BASE (±1%) | report | ≈1 h |
| `sync-dry-run` (weekly) | `sync-upstream --dry-run --gates quick all` | report | ≤ 30 min + rebase time |

Profiles:
- `quick` (per commit): queue, lint, gen-check, deny, brand, i18n, `check` and `test` for the domains affected by
  the diff. Mapping: `upstream/<app>/`, `patches/<app>/` or `domains/<d>/` → that app; `brand/`, `i18n/`, `assets/`,
  `crates/`, `xtask/` → all apps. Target ≤ 45 min warm.
- `full` (gate, daily): everything for all apps, plus builds, sizes and web-smoke. Target ≤ 4 h warm.
- `nightly`: full + release tests + fidelity + xlsx-bench. ≤ 8 h, run overnight.

All profiles run serially under the build lock.

### 6.2 Caching
- **sccache**: `E:/office-cache/sccache`, 40 GB, `CARGO_INCREMENTAL=0`. It caches rustc for library crates; links are
  never cached.
- **Persistent target dirs** per workspace and per profile: `E:/office-target/p1/{root,domain-<d>,upstream-<app>}`.
  Never shared between worktrees (a Phase 0 lesson).
- The cargo registry and git DB stay in `~/.cargo` (on C:, already populated).
- The advisory DB lives at `E:/office-cache/advisory-db`.
- On GitHub (when enabled): `mozilla-actions/sccache-action` with the GHA cache backend, plus a per-workspace target
  cache keyed on `hashFiles('<ws>/Cargo.lock')`. GH caches are capped at 10 GB per repo, so release builds of four
  apps will not all stay cached; size the runners or use a self-hosted runner then. The runner decision is made when
  CI is enabled.

### 6.3 GitHub Actions (ready to enable, not enabled)

`.github/workflows/ci.yml`:
- `on: workflow_dispatch` only. Enabling means adding `push`/`pull_request` once a remote is allowed.
- A matrix over `{app} × {windows-latest, ubuntu-latest}` for `test`/`check`/`build`. The windows runners give the
  **MSVC** build that shipping needs (ADR §2.5).
- `concurrency` per ref; `timeout-minutes` per job (test 120, build 90, nightly 360).
- Every step runs `scripts/ci.sh <job>`, so local and remote are one implementation.
- The workflow is untested until a remote exists; `actionlint` is the only check. Say so in the gate report.

### 6.4 Size budgets (`web/budgets.toml`)

Pristine HEAD sizes measured in Phase 0 (inventory, at the old BASE; brotli −q 11, bytes):

| App | wasm raw | wasm brotli | JS brotli | Known delta of our queue |
|---|---|---|---|---|
| wordcraft | 42,597,532 | 9,441,597 | 20,555 | +9,263 B brotli (patches 0001–0005) |
| gridcraft | 18,973,730 | 4,731,289 | 20,036 | pdf-fonts +8,487 B gzip; memory series unmeasured |
| deckcraft | 35,120,783 | 9,300,367 | 19,665 | — |
| pdfcraft | 39,076,254 | 9,390,759 | 20,487 | — |

Rule (the ADR ratchet on our delta):
```toml
[docs]
base_sha = "<BASE>"
base_br = 0            # pristine brotli at BASE, re-measured by `cargo xt sizes`
craft_fonts = false
delta_br_max = 0       # measured delta after WP7–WP9, +10%, rounded up to 16 KiB: hard fail
js_br_max = 24576
upstream_growth_warn = 0.05   # warn (not fail) when base_br grows >5% between syncs: upstream trend
```
- Before WP7–WP9 measure, `delta_br_max = 524288` (512 KiB) as a guard. Expected contributors: patches about
  10–20 KiB; the fallback fonts < 250 KiB (ESTIMATE); two uz catalogs about 40–80 KiB.
- Raising any `delta_br_max` above its measured value + 10% needs a STATUS line with the reason.
- **`craft_fonts` is part of the budget key**: the wasm embeds Inter and BIZ UDPGothic when `CRAFT_FONTS_DIR` is set
  (F9). Measure both settings in WP3 and recommend one to the owner. CJK in the web bundle is unlikely to be wanted
  for uz/ru users.
- Absolute budgets per editor (the shell's lazy-load budget) are Phase 4's.

### 6.5 Known upstream test failures: the quarantine (`ci/quarantine.toml`)

```toml
[[test]]
app = "wordcraft"
crate = "wordcraft-ui-egui"
binary = "lib"
name = "keys::tests::alt_numpad_code_types_its_character_without_keytips"
class = "env"        # env | upstream-bug | flaky | flaky-crash | brand-overlay | i18n-incomplete | debug-only
env = "win10-ru-oem866"   # where it fails; CI applies the entry only there
reason = "Alt+numpad decodes through the OEM code page; on this ru-locale PC 0233 gives й, not é. Product issue tracked for uz/ru Windows."
evidence = "docs/phase0/inventory.md#wordcraft"
since = "2026-10-15@fd0bb2f"
review = "every sync"
```

Seed entries, each to be re-verified at the new BASE (inventory):
- **wordcraft (5):**
  - `render` `tests_color_glyphs::emoji_are_drawn_in_colour` and `emoji_keep_their_colours_on_a_dark_page` (env:
    colour emoji font);
  - the Alt+numpad test above;
  - `ui-egui tests/breaks_menu` `page_break_from_the_menu_adds_a_page` and
    `section_break_from_the_menu_splits_sections` (env: labels localised to the system language).
- **deckcraft:** `replacefonts_tests::ribbon_button_opens_the_dialog_and_replaces` (env: localised label), plus the
  `ui-egui` lib crash (flaky-crash, WP6b).
- **pdfcraft (14):** signing password length (upstream-bug); zh-hant duplicate keys ×3 and Vietnamese missing
  keys ×3 (upstream-bug); the parity-manifest duplicate key (upstream-bug); JetBrainsMono lacks `ѝ` (env/asset);
  print dialog driver options (env: Windows); and 4 of unknown cause (`automation.rs:359`, `canvas.rs:5118`,
  `editing.rs:1781`, the view swipe test). Each unknown one needs a one-paragraph diagnosis before it may stay.

Policy, so that green is honest:
1. Green = no non-quarantined failure and no crash.
2. A quarantined test that **passes** is reported as `stale`. Two consecutive stale runs → CI fails until the entry
   is removed.
3. **No entry may cover a test that fails because of our own patch**, except classes `brand-overlay` and
   `i18n-incomplete`. Those are listed one by one, counted in every gate report, and expected to shrink.
4. `debug-only` (it panics in debug, for example on overflow checks, and passes in release) is an upstream bug. The
   evidence must show the release pass.
5. Every entry has `reason` and `evidence`. `cargo xt quarantine check` fails on empty fields or unknown classes.

### 6.6 cargo-deny configuration sources

`deny.toml` at the root, modelled on `upstream/pdfcraft/deny.toml` (65 lines: `[graph]`, `[licenses]`,
`[licenses.private]`, `[bans]`, `[sources]`, `[advisories]`, 2× `[[licenses.exceptions]]`) and ADR §5.2:
- **licences allowed:** MIT, MIT-0, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC,
  Zlib, Unicode-3.0, BSL-1.0, CC0-1.0;
- **exceptions:** `epaint_default_fonts` (OFL-1.1 + Ubuntu Font Licence) and `webpki-root-certs`
  (CDLA-Permissive-2.0);
- `[licenses.private] ignore = true` (our `publish = false` crates);
- **sources:** `unknown-registry = "deny"`, `unknown-git = "deny"`, `allow-git = ["https://github.com/storytold/craft-libs",
  "https://github.com/storytold/winprint.rs"]`;
- **bans:** `multiple-versions = "warn"` (32 duplicate names are known), `wildcards = "deny"`,
  `allow-wildcard-paths = true`;
- **advisories:** `version = 2`, `yanked = "warn"`, `git-fetch-with-cli = true`,
  `db-path = "E:/office-cache/advisory-db"`. Each `ignore` entry has a RUSTSEC id, a reason and a review date.

One config serves all 5 lockfiles. New exceptions found in the wordcraft or deckcraft graphs are added with a
`# why` comment. A GPL/AGPL/LGPL/MPL licence is never allowed: those tools are sidecars only.

---

## 7. Risks specific to Phase 1

| # | Risk | L | I | Mitigation | Trigger / owner |
|---|---|---|---|---|---|
| R1 | **D-A domain wrapper fails** (`[[bin]] path` outside the package, `CARGO_MANIFEST_DIR`, build.rs cwd) | M | M | WP3 step 5(a) tests it in 30 min; D-B fallback fully specified (§2a). Neither option reopens W1 | WP3 day 1 |
| R2 | **Copied `[patch]` blocks resolve differently** (pdfcraft 8 vendored paths, deckcraft git krilla) | M | H | lockdiff (0 version changes) + wasm/exe size ±0.5% vs the inventory | WP3 |
| R3 | **Gridcraft rebase cost is high** (0002 touches 27 files; all patches carried, B-6) | H | H | Architect resolves it; a 2-day timebox; park 0001–0019 if exceeded; the owner decides on carrying vs track E vs a B-6 revisit | WP5 |
| R4 | **CRLF / mode corruption of snapshots** (F2, F3) | H if ignored | H | `.gitattributes -text`, `autocrlf=false`, `read-tree` import, tree-hash equality in CI | WP1, CI `queue` |
| R5 | **RAM (11.8 GB) and serial builds stretch the calendar** | H | M | Build lock, RAM gate, night slots, sccache, `-j 4` release; agent work (scripts, catalogs, designs) in parallel with builds | all |
| R6 | **C: has about 34 GB free** | M | M | Every target, cache and Docker data root on E:; the repo grows about 35 MB + 5–15 MB/week | WP0 |
| R7 | **Owner review of Uzbek strings is slow or partial** | H | M | Glossary first; P1 only (≈1,350); batches of ≤400; review builds labelled DRAFT; the gate reports coverage honestly | WP9 |
| R8 | **Upstream tests break with the overlay or new catalogs** | H | M | Required key sets in P1; quarantine classes `brand-overlay`/`i18n-incomplete` listed and counted | WP6b, WP7, WP9 |
| R9 | **Debug-profile tests behave differently** (overflow panics, timing), and their duration is unmeasured | M | M | WP6b measures it; triage as `debug-only` with release-pass evidence | WP6b |
| R10 | **Brand patches in hot files raise the weekly carry** | H | M | One-token sites, logic in new files, a separate 0101 chrome patch; measured in the second dry run | WP14 |
| R11 | **Network quirks** (revocation for git, curl, cargo-deny, trunk's wasm-bindgen download, Docker pulls) | M | M | `http.schannelCheckRevoke=false` via `GIT_CONFIG_*`, `--ssl-no-revoke`, `git-fetch-with-cli`; tools pinned in `tools.lock` | WP0 |
| R12 | **New BASE needs a newer toolchain or wasm-bindgen** | M | M | Sync step 2 flags it; install, then record | WP5 |
| R13 | **No GitHub remote**: the workflow cannot be exercised; no MSVC, macOS, Linux or Win11 build here | certain | M | One implementation (`scripts/ci.sh`) + actionlint; the gate report states plainly that only windows-gnu on Win10 was verified | WP14 |
| R14 | **Trademark or upstream-link leaks** (pdfcraft logo bytes, update check, MCP server info strings) | M | H | brand-grep + logo-include check + `strings` check on artefacts + an overlay runtime test (no network call) | WP7 |
| R15 | **The corpus does not arrive** (B-14) | M | M | WP11 is conditional; the gate reports "corpus run not done", as in Phase 0 | owner |
| R16 | **Locale change (WP10) disturbs other jobs** | L | L | Run alone; restore `ru-RU` in a `finally` | WP10 |
| R17 | **The 2 data points of rebase cost are mistaken for a trend** | M | M | Label it as such; keep the weekly log in Phase 2 | WP14 |

---

## 8. Schedule

Working days from Monday 2026-10-19. "M" = machine-heavy; runs in the evening or at night under the build lock.

| Day | Main line (critical path in bold) | In parallel (agents, no build lock) |
|---|---|---|
| D1 | **WP0** tools; **WP1** clones → cache, pristine snapshots | WP12 reading |
| D2 | **WP2** queue + register; **WP3** step 5(a) sheets debug (D-A go/no-go) | WP6a scripts start |
| D3 | **WP3** slides, pdf, docs desktop releases (M) | WP7a, WP9a designs (architect) |
| D4 | **WP3** web + CLI builds (M), `domain-build.md`; WP11 if the corpus arrived | WP12 draft |
| D5 | **WP5** script; dry run starts; wordcraft and pdf-fonts rebase | **WP12 sizing delivered (B-15 week-1 promise)** |
| D6 | **WP5** gridcraft rebase (architect, timebox day 1) | WP6a deny/sizes; WP9b xtask |
| D7 | **WP5** gridcraft rebase (timebox day 2); `--gates full` (M) | WP9e glossary → owner (batch 0) |
| D8 | **WP5** apply sync; `rebase-cost.md`; `UPSTREAM.md` | WP7b overlay tooling; WP13 |
| D9 | WP6b debug-test measurement (M); quarantine at the new BASE | WP7c–f patches written (4 builders) |
| D10 | WP6b nextest + deckcraft crash (M) | WP10 locale check (not during M) |
| D11 | **WP7** per-app hook builds + overlay tests (M) | WP9e P1 drafts |
| D12 | **WP7** chrome patches; brand evidence; **WP8** font | WP4 craft-libs (night, M) |
| D13 | **WP9c** language hooks ×3 (M) | WP9f review batches 1–2 → owner |
| D14 | **WP9d** deckcraft mechanism + P1 patch (M) | WP9f batch 3 |
| D15 | WP6 final: budgets from measured deltas, quarantine review (architect) | import of returned review batches |
| D16 | `scripts/ci.sh full` green on the working tree (M) | evidence PNGs; STATUS |
| D17 | **WP14** second dry run (BASE ≥ 7 days old) + rebase cost 2 | |
| D18 | **WP14** clean checkout, cold `ci.sh full` (M, exclusive) | review import, final coverage |
| D19 | **WP14** adversarial review (architect) and fixes | |
| D20 | Gate report (Uzbek); stop and wait for "go" | |

Totals:
- **≈185 engineer-hours** (sum of the WP estimates) + **≈45 machine-hours**.
- **20 working days ≈ 4 calendar weeks** (ESTIMATE).
- **Realistic range: 17–27 days.** The main swing factors are the gridcraft rebase (R3, ±3 days), a D-A failure
  (R1, +1–2 days), the owner's review latency (R7: if review batches are not back by D18, the gate goes ahead with
  coverage stated honestly), and debug-test duration (R9).

**Critical path:** WP0 → WP1 → WP2 → WP3 (W2 proof) → WP5 (fresh BASE + sync script + rebase cost) → WP7 hook → WP9
language hooks → WP6 final budgets and quarantine → WP14 (second dry run, which needs BASE ≥ 7 days old, then the
clean-checkout `ci.sh full`). These items are sequential, and their machine time is serial.

---

## 9. Owner decisions this plan asks for during Phase 1

1. The working name `Suite` and the ids in `brand/brand.toml` as placeholders (B-1 stays deferred). Also the
   attribution wording in About and NOTICE (nominative trademark use).
2. Brand colours and icons (BRIEF Appendix B, Phase 1). Placeholders are used until then.
3. Uzbek review: the glossary first, then the P1 batches. Accept that the Phase 1 gate shows DRAFT strings where
   review is incomplete.
4. Fallback for missing Uzbek strings: English (recommended, upstream behaviour) or Russian.
5. B-13: deckcraft = mechanism + P1 strings in Phase 1; the rest English until a later phase.
6. Web bundles with or without craft-fonts embedded (affects the size budgets by megabytes; §6.4).
7. If the gridcraft rebase exceeds its timebox: carry the memory series anyway, park it, or revisit B-6 (PRs).
8. Still open from Phase 0: B-7 to B-14 (the corpus B-14 unblocks WP11).
