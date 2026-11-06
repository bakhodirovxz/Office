# Page-host test plugin (Phase 7 WP5)

Fixtures for `ci/web-smoke/plugins-host.spec.ts`. Not a product plugin.

- `gen.mjs` writes `plugin.wat`, `bad-import.wat`, `bad-export.wat` (`node gen.mjs`). `plugin.wat` has the API v1 shape and picks its answer
  by the first letter of the command id (see the header of `gen.mjs`); the other two are refused at load (M1 WASI import, M2 exported memory).
- The `.wasm` files are committed. Rebuild with the `wat` crate (1.263.0, offline from the cargo registry is fine): a scratch package whose
  `main` is `std::fs::write(out, wat::parse_file(in)?)`, run once per file:
  `watc plugin.wat plugin.wasm && watc bad-import.wat bad-import.wasm && watc bad-export.wat bad-export.wasm`.
- `evil-fetch.js` is the test-only Worker script of M11: it calls `fetch` directly. The spec serves it under `/shell/plugins/`, where the
  Worker CSP of suite-serve applies, and expects the call to fail.

Phase 7 re-check additions (`gen.mjs` header lists them): `bad`, `link`, `chr`, `incl`, `tracked`, `vocab`, `accept`, `wall` (answers the allowlist refuses: F2, F7a, M8),
`overflow` (unbounded recursion, S1), `ping` and `pong` (rounds of 0.2 to a few seconds that resume forever: the 10 s invocation deadline, S2) and `fetches` (a fetch on every
round: the 8-fetch cap, S2). A manifest holds at most 16 commands, so a spec lists only the ones it needs. `scripts/mobile/fixtures/make-protected.py` makes the F7b documents.
