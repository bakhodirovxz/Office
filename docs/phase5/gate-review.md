# Phase 5 gate review (WP18): the document server as any integrator installs it

Date 2026-11-27, branch `phase-5` (gate run at `67d1a0c8`, plus the two fix commits of section 5) (HEAD includes the security fixes G1-G3, the quick start, the merged-image
verification and the wordcraft quarantine fix). Prepared by the agent for the owner's "go". Every `file:line` is on that commit;
logs named `E:/office-target/...` are outside the repo (synthetic data only, no corpus, no HRM data).

Gate (`docs/phase5/PLAN.md:36`, owner decision 2026-11-24): (1) a generic ONLYOFFICE-API host runs open, edit, save, reopen,
download for docx, xlsx, pptx and pdf, plus a view-only open, against a fresh Docker install configured only by env/config; not
tied to the HRM (the HRM pilot WP17 is optional); (2) the security checklist (PLAN §10) passes.

**Verdict: READY for "go" on the two criteria, with the open items of sections 7 and 8 stated.** Criterion (1) is measured 8 of 8
on the merged image. Criterion (2) is 23 PASS (two fixed during the gate run), 1 PARTIAL (S19, no front proxy and no real certificate here), 1 not applicable
by decision (S25). `ci.sh full`: the first run had 5 red or infra jobs; two were real and are fixed (stale THIRD-PARTY-NOTICES.txt, one rustfmt hunk in a carried patch), the others were fresh-target ordering, a network blip and flakes; after the reruns all jobs are ok except one known flaky Playwright case (section 5).

---

## 1. Gate table

| # | Criterion | Evidence | Verdict |
|---|---|---|---|
| 1 | Generic host: open, edit, save, reopen, download for docx, xlsx, pptx, pdf, and view-only, on a fresh Docker install configured by env only | `packaging/docserver/rehearse-generic.sh` + `ci/web-smoke/generic.spec.ts:61` (edit round trip) and `:99` (view-only) + host `ci/web-smoke/generic-host/host.mjs`. Merged image `hoshiya-docserver:p5-verify` (phase-5 `9af9a861`): **8 of 8** in 2.1 min, callbacks 1,6,2 for each format, 0 tokens in the server log (`E:/office-target/p5-verify/rg1.log`; `docs/phase5/gate-security.md` "Merged image"). Saved sizes docx 14122 to 10146 B, xlsx 9775 to 4891, pptx 41327 to 30068, pdf 31263 to 31564. Screenshots `docs/phase5/evidence/generic/` (16 files). Earlier run on the quick-start image also 8 of 8 (`docs/STATUS.md:1976-1985`). One xlsx run in four lost a typed marker before the spec typed with a 60 ms delay; the merged-image run had no flake | **met** (Chromium, one local Docker host, section 7) |
| 2 | Security checklist S1-S25 passes | `docs/phase5/gate-security.md` section 2: 23 PASS (two of them were FAIL on the gate commit, S21 and S22, fixed), S19 PARTIAL, S25 owner/HRM (no longer a gate condition). Attacker review: G1 (high), G2, G3 (medium) reproduced, fixed, each with a test that fails without the fix (section 3). Fail-closed probes on the merged image: 7 of 7 refuse with a named message (section 4 there). Tests on the merged tree: `convert-api` 78 passed, `docserver` 85 passed, 0 failed (`--no-fail-fast`; two timing flakes under load passed alone) | **met**, S19 partial |

Criterion (2) caveats: S19 is measured with real Traefik v3.6 in front of the image (HSTS, 308 to https, a direct request 403), not
behind a front proxy and not with a real host name and certificate. S25 is the HRM team's row and is outside the gate by the owner's
decision.

## 2. Findings of this review

No new code finding. The review re-read `gate-security.md`, `README.md` and the generic spec; I did not repeat the attacker review.

| # | Finding | Status |
|---|---|---|
| R1 | The server cannot run on plain http: `public_url` must be https (`tls = "builtin"` or a trusted proxy). A bare `docker run -p 8080:80` with only the JWT settings does not exist. ONLYOFFICE's default is plain http. The README says so first (`packaging/docserver/README.md:12-14`, `docs/STATUS.md:1983`) | by design, decision 2 in section 9 |
| R2 | L1-L5 of the attacker review are open (section 8 below) | open, decision 3 |
| R3 | The `cookie` 0.18.3 lock bump (RUSTSEC-2026-0594) is on phase-5 but not on `phase-4`'s services lock (`gate-security.md` section 6) | open: belongs on `phase-4`, then merge forward |
| R5 | Two real `ci.sh full` reds on phase-5 (stale `THIRD-PARTY-NOTICES.txt`, rustfmt hunk in `gridcraft/0189`), both fixed in their own commits (section 5) | fixed |
| R6 | In a fresh target dir `ci.sh full` runs `shell` and `docserver` before any `build` job, so they fail for missing dists; run `ci.sh build` first or reorder the profile | open, small |
| R4 | `--read-only` with no volume prints `mkdir: cannot create directory` shell errors before the server's own refusal; it still fails closed (exit 2) | cosmetic (= L5) |

## 3. Work packages WP0-WP18

Status: done / partial / not done / optional. Evidence is the STATUS section or file named.

| WP | Status | Evidence |
|---|---|---|
| WP0 baseline, merge-forward rule | done | `docs/STATUS.md:1231`; `ci.sh quick` 19/19 and services 213 passed at f89e7b6 |
| WP1 HRB contract harness (fake HRM) | done | `services/fake-hrm`, `services/docserver/tests/` (auth, outbound, dropin); `STATUS.md:1519` fake-hrm 14 tests |
| WP2 crate, config, mounting | done | `STATUS.md:1236`; `services/docserver/src/config.rs`; `suite-serve/tests/docserver.rs:134` |
| WP3 sessions and tokens | done | `docserver/tests/auth.rs` (S1, S3, S5, S11, S24); on-disk sessions `sessions.rs` |
| WP4 outbound, SSRF, callback | done | `docserver/tests/outbound.rs` (S6-S9, S23) |
| WP5 `api.js`, wrapper, framing | done | `ci/web-smoke/docserver.spec.ts` (S13 `:314`, S14 `:263`, `:287`); generic spec |
| WP6 Docs bridge read-only and saved ack | done | wordcraft 0127, 0128 (download/print flags), 0152; `STATUS.md:1260` |
| WP7 permissions | partial | edit and view rows, download/print flags (Docs only), server refusal of download; review and comment-only modes are not supported, opened as view (`onlyoffice-compat.md`, `README.md` "Differences") |
| WP8 audit log, rate limits | done | S15 `outbound.rs:1200`, S16 `outbound.rs:1073`, S17 `no_ids_in_logs.rs:41` |
| WP9 `_hrm_*` bookmarks, translit in embed | done (synthetic only) | `STATUS.md:1781`; the real HRM samples were never tested (corpus O9 not delivered) |
| WP10 deployment image, local rehearsal | done | `STATUS.md:1241`; image, compose rehearsal 3 of 3 (2026-10-25), drop-in rehearsal 54 of 54 (`dropin.md:266`) |
| WP11 server version store | done | `docserver/src/history.rs`, `tests/history.rs`; `suite-core` `history.rs:1278` `torn_index_is_repaired`; retention `retention.rs` (S18) |
| WP12 diff engines, compare | done for Docs and Sheets (Q7) | `crates/seq-diff`, `docs-diff`, `sheets-diff`; 9 + 9 + 6 golden tests; 108-page docx 65-106 ms, w40 30-31 s (`STATUS.md:1244`). Slides and PDF later by Q7 |
| WP13 SSO, desktop sign-in (`kind = "oidc"`) | not done (decided: after the gate, Q8) | `services/convert-api/src/config.rs:365` still refuses `kind = "oidc"` |
| WP14 XDFU on the server | done (off by default) | `docserver/src/xdfu.rs`, `tests/wp14.rs` (6); Q12 `STATUS.md:1276` |
| WP15 Phase 4 carry-forward | partial | F7, F11-F17, F20, F21 fixed on phase-4 and merged (`phase-4/gate-review.md` section 1); F14 `STATUS.md:1799`; F18/F19 `:1815`; F1 remainder `history.rs:1278`. Not done: m10 (PDF locale probe), the `GATE-FLOW` scripts into `ci/` (no match in `ci/`), F5 `_begin` design, F20 ICE60/61 warnings |
| WP16 CI, docs, compat file | done | `ci.sh docserver` (in `full`); `onlyoffice-compat.md`; `no_hrm_names` scan list. Field names not re-checked against the public DocsAPI pages (`STATUS.md:1255`) |
| WP17 HRM pilot on `<test-host>` | optional (owner decision 2026-11-24) | install on the test server done 2026-11-08 with owner OK (`STATUS.md:1894`); a real person's open/edit/save there was not run |
| WP18 gate: checklist, attacker review, report | done | `gate-security.md`; this file |

## 4. Gate flows run here

- **Generic host (criterion 1):** section 1, row 1. Run 2026-11-25 on the merged image built by `packaging/docserver/build.sh`
  (`TAG=p5-verify`); Chromium on the host, host and image on one Docker network, a throwaway CA and secret, synthetic fixtures.
- **Fail-closed probes** on the same image (`gate-security.md` "Merged image"): no env (exit 64), `JWT_ENABLED=false` (64),
  `DOCSERVER_PUBLIC_URL` missing (64), `JWT_SECRET=secret` (2, "at least 16 bytes", no value printed), no certificate and no proxy
  (64), an unreadable certificate (64), `--read-only` with no volume (2).
- **Real proxy chain** (gate commit, Traefik v3.6 in front of the image): HSTS `max-age=31536000`, plain http 308 to the https
  `public_url`, a direct request 403, a stranger on the internal port 403 (`gate-security.md` section 4).
- **Docker rehearsals not re-run on the merged image:** the drop-in (54 of 54, 2026-11-08) and the compose rehearsal (3 of 3,
  2026-10-25) are earlier results.

## 5. `ci.sh full`

`scripts/ci.sh full`, run `20261125-140105`, started 2026-11-25 14:01 at `67d1a0c8`, finished by 2026-11-26; summary and one log per job in
`E:/office-target/p5-gate/ci/20261125-140105/` (`summary.md`). Fresh target dir `E:/office-target/p5-gate`. Reruns of the red jobs, after
two fix commits, are in `.../20261127-141106` (notices), `.../20261127-141428` (convert, shell, docserver) and `.../20261127-152619` (shell).
The full run itself exited with 5 red or infra jobs of 31 job lines; after the reruns and fixes below, **every job is ok except one known
flake** (the xlsx "Back 300 ms" Playwright case). E: free: 122 GB at the start, 66 GB at the end (lowest 62 GB; I deleted my own
`root`, `domain-*` and the finished `upstream-*` intermediates during the run, which cost recompiles, not results).

| Job | Result | Seconds | Notes |
|---|---|---|---|
| queue | ok | 52 | verify-queue all, register check |
| lint | **red, fixed** | 1,901 | clippy upstream baselined (1/3/2/1 warnings); **fmt-gridcraft 1 hunk** in `upstream/gridcraft/crates/engine/tests/translit.rs:265`, from carried patch `gridcraft/0189` (merged from the phase-4 quarantine work). New, ours. Fixed in the commit "gridcraft/0189: rustfmt the one assert" (patch and tree together): `cargo fmt --check` ok, `verify-queue.sh gridcraft` 77 patches tree OK, `register check` 159 rows. The whole lint job was not rerun (30 min); only the fmt step was rechecked |
| gen-check, deny, i18n | ok | 22 / 36 / 6 | deny: root + 4 domains ok |
| brand | ok | 26 | the second `brand` (binaries) ok, 328 s |
| test-ours | ok | 3,230 | nextest 258 / 151 / 56 / 47 / 74 passed (1, 2, 0, 0, 4 skipped) |
| notices | **red, fixed** | 232 | `THIRD-PARTY-NOTICES.txt` stale against a fresh cargo-about run (the zip/url lock changes merged from phase-4 dropped the zopfli entries). New, ours. Regenerated (708 crates), commit "THIRD-PARTY-NOTICES.txt regenerated"; `ci.sh notices` rerun **ok** (194 s) |
| test wordcraft | ok | 4,317 | 3,500 passed (6 skipped); quarantine: 3 still failing, 0 stale (the colour-emoji-font tests, known) |
| test gridcraft | ok | 4,498 | 1,796 passed (4 skipped); quarantine 1 still failing, 0 stale |
| test deckcraft | ok | 2,353 | 1,776 passed (11 skipped); quarantine 4 (3 failing, 1 flaky-passed), 0 stale |
| test pdfcraft | ok | 4,731 | 3,807 passed (44 skipped); quarantine 28 (22 failing, 6 flaky-passed), 0 stale |
| check docs / sheets / slides / pdf | ok | 423 / 1,087 / 405 / 473 | desktop + wasm32 |
| bind | ok | 835 | |
| convert | **infra, rerun ok** | 602 | nextest 353 of 353 passed in the full run; `cargo deny` services could not fetch the advisory database (no DNS for github.com at that moment, `convert.log` "Could not resolve host"). Rerun 2026-11-27: ok, 353 of 353, deny ok on host and linux (224 s) |
| convert-e2e | ok | 2,064 | API e2e 2/2, `directions.sh` 43 PASS, converter Playwright 13 passed |
| shell | **red, rerun ok** | 1,001 | the full run's `templates` spec timed out 5 of 5 (180 s each): the editor dists did not exist yet, because `shell` runs before the `build` jobs in a fresh target dir (order effect, not a product fault). Rerun 1: templates 5/5 passed, but 2 of 28 `shell.spec.ts:106` ("docs/slides: the editor frame follows the shell language") failed on a race in the spec (`frame` looked up before the iframe navigated, `shell.spec.ts:112-113`). Rerun 2 (`shell` alone): **ok**, 547 s. Known flake class, not fixed |
| docserver | **infra, rerun: 1 flake** | 211 / 3,016 | the full run stopped at once: `dist/docs-web` missing (same ordering). Rerun after the builds: fmt, clippy, Rust tests all ok (every `test result` line `0 failed`, `docserver.log`), Playwright docserver 36 passed, **1 failed** (`docserver.spec.ts:1012` "HRM Back 300 ms after an edit, xlsx": a typed key lost, the expected marker `49994` missing from the saved xlsx), bridge-docs 16, bridge-sheets 55, bridge-slides 38, bridge-pdf 40 passed. The failing case alone: 1 fail in 4 runs (3 passed). Known: `gate-security.md` section 1 recorded it failing 3 of 9 and 2 of 8 on the unfixed binary; the test's own comment calls it an engine matter under load. Not caused by a gate change |
| build docs / sheets / slides / pdf | ok | 1,623 / 747 / 1,513 / 1,690 | desktop, web, cli |
| translit-npm, translit-preserve | ok | 35 / 128 | preserve 8 of 8 IDENTICAL |
| sizes | ok | 451 | below |
| web-smoke, actionlint | ok | 11 / 2 | |

Sizes (brotli bytes, budget, headroom): docs 9,848,652 / 9,904,416 / 55,764; sheets 4,965,570 / 5,032,307 / 66,737; slides 9,556,969 /
9,589,753 / 32,784; pdf 9,574,187 / 9,599,462 / 25,275; shell 60,988 / 65,536 / 4,548; npm 31,710 / 49,152 / 17,442. Every JS
chunk is under 24,576. The shell has the least headroom (4.5 KB, 7 %); it was 10.8 KB at the Phase 4 gate.

Not run in `full`: the Docker compose rehearsal (`ci.sh docserver rehearsal`; section 8), the on-demand `desktop`, `msi` and `msvc` jobs, `convert-ui`
(`tls.spec.ts`), `nightly` jobs.

## 6. Licences and security notes

- `ci.sh deny` (root + 4 domains) and the services deny (host and linux) are **ok** (section 5). The single licence exception is still
  MPL-2.0 symphonia (unmodified, in NOTICE); no new exception was added in Phase 5. RUSTSEC-2026-0594 (`cookie` 0.18.2) is closed by the
  lock bump to 0.18.3 (`gate-security.md` S22).
- Third-party notices: `THIRD-PARTY-NOTICES.txt` was stale on phase-5 (finding in section 5) and is regenerated, 708 crates; `ci.sh notices`
  ok.
- The Phase 5 crates (`docserver`, `fake-hrm`, `seq-diff`, `docs-diff`, `sheets-diff`) are MIT OR Apache-2.0 and in the services/domain
  deny runs. `no_hrm_names` (S21) scans `docserver` and the web-smoke specs and passes.
- Security: see `gate-security.md`; this review added no code finding.

## 7. Carry-forward from Phase 4 (gate-review sections 9 and 11)

| Item | State now |
|---|---|
| WP23 IT staff sessions (`usability.md` §7 table empty) | closed without being run (owner, 2026-11-23); still not run. Needed before any wider rollout |
| Desktop keyboard / Narrator walk (`a11y-baseline.md`) | not run; the web editors still fail the in-iframe keyboard walk, the desktop apps expose unnamed UIA nodes (as of the Phase 4 measurement; not re-measured) |
| "Always" by hand on this PC | not run |
| Windows 11 and domain-PC rows (GPO, AppLocker/WDAC) | not run (owner hardware) |
| Docs pointer report | closed, no fix (owner: the laptop's cursor settings) |
| B-3 handle design (§10 item 5) | re-homed to Phase 6, not on this branch |
| Corpus O9 | not delivered: `corpus/` does not exist in this tree; WP9 and R6 ran on synthetic files only |
| `C:\c` housekeeping | the folder still exists (`/c/c`); the owner can delete it |
| F3 third-party notices | fixed in Phase 4 (`bdb77d6`); `ci.sh notices` runs in `full` (section 5) |
| F7, F10-F17, F20 (partly), F21 | fixed (Phase 4 commits and `p5-f14`); F20 still has ICE60/ICE61 warnings and no install or upgrade test of the rebuilt MSI |
| XDFU set before it is switched on: F4, F5, F18, F19, m13 | F4, F18, F19 fixed; F5 partly (the lock is still held during produce); m13 rest not re-checked. XDFU stays off by default |
| m10 (PDF locale probe), `GATE-FLOW` scripts into `ci/` | not done |
| Torn history index repair (F1 remainder) | done (`crates/suite-core/src/history.rs:1278`) |

Not run because they need the owner or hardware this PC lacks: ARM64, real phones, macOS, Windows 11, a domain PC.

## 8. What was not verified

- A real domain and a real certificate: every TLS test uses a throwaway CA (`rehearse-generic.sh`) or Traefik's.
- A TLS proxy in front of the generic install: the generic run uses the server's own TLS; Traefik was measured on the gate image
  (section 4), not on the merged image, and no front proxy before Traefik (F4 of the review).
- A real third-party ONLYOFFICE connector (Nextcloud, a CMS plugin, a vendor SDK): only our 100-line host and the HRM-shaped
  fake were used. Field-name parity with the public DocsAPI pages is not re-checked (`STATUS.md:1255`).
- Browsers other than Chromium (Firefox, Safari, mobile WebViews other than the fake-opaque-origin test).
- Restart and upgrade behaviour of the generic install: the README says "not tested here: close sessions first"
  (`packaging/docserver/README.md` "Upgrading"). Restart is covered only by the docserver unit tests.
- The README compose example was not run; the `docker run` example is what `rehearse-generic.sh` mirrors.
- The merged-image run used web dists from an earlier build (`E:/office-target/p5/dist`, 2026-11-21/23), not rebuilt from `67d1a0c8`.
  Services code is the merged code.
- The "TLS certificate and no proxy keeps the per-address cap" check, the Traefik rows and the compose/drop-in rehearsals on the merged
  image (`gate-security.md` section 5).
- Low items L1-L5 (`gate-security.md` section 3), open: L1 an authenticated edit session can fill the disk (no per-session
  version cap or free-space floor); L2 `trusted_proxies` accepts `0.0.0.0/0`; L3 env mode keeps generated secrets on the data volume
  unless `/run/secrets` is mounted (the audit key sits beside its log); L4 stable `document.key` values expose old versions across
  users of that key; L5 `entrypoint.sh` says "generated" when it could not write (still fails closed).
- The editors' parsing of hostile documents (browser side), timing side channels (read only), co-editing (not offered).
- The HRM: no person-run flow on `<test-host>` (WP17 optional); Android and iOS apps; mobile.

## 9. Decisions for the owner

| # | Decision | Recommendation |
|---|---|---|
| 1 | **Accept the Phase 5 gate ("go")** on criteria (1) and (2) as met above, with sections 7 and 8 recorded as open | **Go.** Both criteria have a measurement on the merged image; the open items are environments, not code |
| 2 | Keep https mandatory (the server refuses plain http) against ONLYOFFICE's plain-http default | **Keep it.** Browsers refuse a cross-site editor frame without it, and a plain-http mode would drop HSTS and the proxy-only rules and invite tokens over cleartext. Cost: a new integrator needs a certificate or a TLS proxy, which the README states first. Add a "local trial with a self-signed certificate" snippet to the README instead |
| 3 | L1-L5: fix before the gate or in Phase 6 | **Phase 6**, except L2 (refuse a /0 `trusted_proxies`, one line) and the L3 `/run/secrets` note in the quick start, which are cheap and can go in now. L1 (disk floor, version cap) needs an authenticated session, so it is a hardening item, not a gate item |
| 4 | Put the `cookie` 0.18.3 bump on `phase-4` (then merge forward) | **Yes**, as the first merge-forward fix |
| 5 | Fix the `ci.sh full` ordering (R6) and make the xlsx "Back 300 ms" case less load-sensitive | **Yes, small**, Phase 6 |
| 6 | Whether WP13 (SSO, desktop sign-in) is Phase 5 remainder or its own later work | **Later** (Q8 already says after the pilot gate); keep `kind = "oidc"` refused until then |

## 10. Close-out (2026-11-27): owner "Go"

The owner closed the Phase 5 gate on 2026-11-27 ("Go"), on criteria (1) and (2) as measured above, with sections 7 and 8
recorded as open. Head at close: `phase-5` 42cadb0e (the gate review plus the L2 fix). Answers to section 9:

| # | Owner answer | What follows |
|---|---|---|
| 1 | Go | Current phase is 6 (CLAUDE.md, STATUS) |
| 2 | **Overruled: plain http allowed, as ONLYOFFICE does** ("Onlyoffice kabi http bilan ham ishlayversin") | Plain-http mode on `phase-5` (branch `p5-http`): an `http://` public URL with no TLS settings starts on http; https stays the production recommendation; the http-mode differences go into `gate-security.md` |
| 3 | Fix L1-L5 now ("Hozir tuzataver") | L2 done in 42cadb0e (`trusted_proxies` refuses a /0); L1, L3, L4, L5 on `p5-lows` |
| 4 | Done before the close (phase-4 412faf90) | - |
| 5 | Not answered; stays the recommendation | `ci.sh full` ordering (R6) and the xlsx "Back 300 ms" case, Phase 6 |
| 6 | Yes, later | WP13 after the gate; `kind = "oidc"` stays refused |

Not run at the close, carried to Phase 6 (so nobody reads them as passed): every row of section 8, the Phase 4 carry-over of
section 7 (staff sessions, the Narrator walk, Windows 11 and domain PCs), and the WP17 HRM pilot (optional). The two follow-up
branches are Phase 5 fixes: they land on `phase-5` and merge forward to 6 and 7.
