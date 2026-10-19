# Office suite on the Craft engines — agent working rules

**Current phase: Phase 1 — Foundation** (branch `phase-1`, from `phase-0`; Phase 0 gate passed 2026-10-17, ADR: docs/ARCHITECTURE-DECISION.md).
The full brief (mission, constraints, phases, exit gates) is in [docs/BRIEF.md](docs/BRIEF.md).
Progress, measurements and risks live in [docs/STATUS.md](docs/STATUS.md) — keep it current.

## Gate decisions in force (ADR Appendix C)
- Our layer is licensed **MIT OR Apache-2.0**.
- **Product name: deferred.** Use the neutral working name from the one brand config. Never hard-code a name.
- **Repo layout:** W2 build domains. Upstream is a vendored pristine snapshot plus a patch queue in `patches/`.
  Never hand-edit `upstream/`.
- **Single licence exception: MPL-2.0 for the symphonia crates only** (deckcraft audio). They stay unmodified, and
  NOTICE lists them with a source link. Any other non-permissive licence needs a new owner decision.
- **No upstream PRs for now.** All upstream changes are carried patches, registered in `patches/REGISTER.md`.
- **Uzbek UI strings: review delegated to the agent** (owner, 2026-10-18).
  - Mark the status `reviewed:agent-<model>:<date>`, never an owner review.
  - The owner reports problems found in use.
  - The Phase 2 transliteration golden list and exceptions dictionary: **also delegated to the agent** (owner,
    2026-10-20). Use the same `reviewed:agent-*` marking.
- **Mobile:** A-web for v1, conditionally (re-decided at the Phase 4 gate).
- **Priority:** fidelity → mobile → co-editing.

## Phase discipline
- Work through phases in order. Each phase ends with an exit gate: stop, report to the owner,
  and wait for an explicit "go". Never skip a gate or merge two phases.
- Phase 0 writes no product code. Throwaway spikes go in `spikes/`.

## Language
- Talk to the owner in **Uzbek**.
- Code, comments, commit messages and docs are in **English**.
- User-facing UI strings go through the i18n catalogs (uz-Latn, uz-Cyrl, ru, en). Uzbek strings
  are reviewed by the owner, never machine-trusted.

## Git
- Never add `Co-Authored-By` trailers (owner rule; overrides any tool default).
- No push, no remote repos, no releases, no deploys without the owner's explicit OK.
- One branch per phase (`phase-0`, `phase-1`, ...).

## Hard constraints
- Do not rewrite the upstream engines. Our work = thin layers (new crates, adapters) plus minimal
  upstream patches, ideally upstreamable. Every patch is recorded in `patches/`.
- One Cargo workspace (monorepo); dedupe shared crates; resolve version conflicts explicitly.
- Web: lazy-load each editor, never one combined bundle; size budgets measured and CI-enforced.
- Evidence over assertion: performance / fidelity / mobile-UX claims need a measurement or a
  real-device/emulator test.
- Clean-room: no code from Microsoft Office, ONLYOFFICE (AGPL) or LibreOffice (MPL). Public specs OK.
- Licensing: our code and linked deps permissive only (`cargo-deny`). GPL/AGPL tools (pdf2docx,
  PyMuPDF) only as separate sidecar processes/containers over HTTP/CLI — never linked or vendored.
- ArtCraft names/logos are not open source: our product gets its own brand. Keep every upstream
  LICENSE, NOTICE and ATTRIBUTION file.
- HRM: test environment only. Never touch production systems or data.
- `corpus/` holds anonymised HRM documents: never commit personal data, never copy corpus
  contents into issues, logs or commit messages.

## Sub-agents
- Run independent work in parallel (e.g. one sub-agent per upstream repo).
- Cheapest model that does the job well: haiku (`scout`) for search/inventory, sonnet (`builder`)
  for single-module engineering, opus (`architect`) for architecture, hard debugging and the
  final adversarial review. Escalate a tier if a cheaper result is weak.
- Sub-agent reports are conclusions with `file:line` references, not raw dumps.

## Measure, don't assume
- If something blocks (no macOS for iOS, no GPU, no emulator), say so plainly.

## Exit-gate report (Uzbek, short)
1. What was done. 2. What was measured (numbers). 3. What failed / is still unknown.
4. Decisions the owner must make. 5. Proposal for the next phase.

## Local environment notes (this machine)
- Windows 10, 12 cores, 11.8 GB RAM, Intel UHD 630 (no discrete GPU). No macOS → no iOS simulator.
- Rust: `~/.cargo/bin` (rustc 1.97.1, `stable-x86_64-pc-windows-gnu`, MSYS2 ucrt64 gcc).
  Not on the Bash PATH by default: `export PATH="$HOME/.cargo/bin:$PATH"`.
- C: has ~34 GB free; build output goes to E: (`CARGO_TARGET_DIR=E:/office-target/<repo>`).
- Android SDK at `%LOCALAPPDATA%\Android\Sdk`, NDK 27.1, AVDs `HRM_Dev_33`, `HRM_Pixel_8`.
- cargo git deps (craft-libs) need `CARGO_NET_GIT_FETCH_WITH_CLI=true` and
  `CARGO_HTTP_CHECK_REVOKE=false` on this machine.
- Docker 29 available (for LibreOffice / ONLYOFFICE x2t reference renders and sidecars).
