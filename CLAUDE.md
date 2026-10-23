# Office suite on the Craft engines — agent working rules

**Current phase: Phase 4 — Unified shell (web + desktop)** (branch `phase-4`, from `phase-3`; Phase 3 gate passed 2026-10-22, owner "go"; plan: docs/phase4/PLAN.md).
The full brief (mission, constraints, phases, exit gates) is in [docs/BRIEF.md](docs/BRIEF.md).
Progress, measurements and risks live in [docs/STATUS.md](docs/STATUS.md) — keep it current.

## Gate decisions in force (ADR Appendix C)
- Our layer is licensed **MIT OR Apache-2.0**.
- **Product name: Hoshiya** (owner, 2026-10-23; B-1 closed). uz-Cyrl form: Ҳошия. The name lives only in the one brand
  config (`brand/brand.toml`). Never hard-code it.
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
- **Mobile:** A-web for v1, conditionally. **Android deferred** (owner, 2026-10-20): the converter and the UI translations go first.
- **Priority:** fidelity → mobile → co-editing.
- **Accounts are optional** (owner, 2026-10-21): fully open source, most users have no HRM. Local mode needs no login; connecting to a server is optional; the HRM is one configured token issuer or SSO provider, never required or hard-coded.

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
- Windows 10, 12 cores, **31.8 GB RAM** (upgraded from 11.8 GB, noticed 2026-10-20), Intel UHD 630 (no discrete GPU). No macOS → no iOS simulator.
- Rust: `~/.cargo/bin` (rustc 1.97.1, `stable-x86_64-pc-windows-gnu`, MSYS2 ucrt64 gcc).
  MSVC is also installed (2026-10-22, owner OK, VS Community terms): VS 2022 Build Tools at `E:\VS\BuildTools`, Windows 11 SDK
  (signtool in `C:\Program Files (x86)\Windows Kitsin.0.26100.0d`), toolchain `stable-x86_64-pc-windows-msvc`;
  `scripts/build.sh ... --msvc` builds into `E:/office-target/p1/msvc/`. gnu stays the default.
  Not on the Bash PATH by default: `export PATH="$HOME/.cargo/bin:$PATH"`.
- C: has ~34 GB free; build output goes to E: (`CARGO_TARGET_DIR=E:/office-target/<repo>`).
- Android SDK at `%LOCALAPPDATA%\Android\Sdk`, NDK 27.1, AVDs `HRM_Dev_33`, `HRM_Pixel_8`.
- cargo git deps (craft-libs) need `CARGO_NET_GIT_FETCH_WITH_CLI=true` and
  `CARGO_HTTP_CHECK_REVOKE=false` on this machine.
- Docker 29 available (for LibreOffice / ONLYOFFICE x2t reference renders and sidecars).
- Docker networking on this PC goes through a TLS-intercepting proxy: `pip`/`curl` inside a container fail with CERTIFICATE_VERIFY_FAILED. Fetch dependencies on the host with hashes and install offline (see `sidecars/pdf2docx/fetch-wheels.sh`). CI is not affected.
