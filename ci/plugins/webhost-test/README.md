# Page-host test plugin (Phase 7 WP5)

Fixtures for `ci/web-smoke/plugins-host.spec.ts`. Not a product plugin.

- `gen.mjs` writes `plugin.wat`, `bad-import.wat`, `bad-export.wat` (`node gen.mjs`). `plugin.wat` has the API v1 shape and picks its answer
  by the first letter of the command id (see the header of `gen.mjs`); the other two are refused at load (M1 WASI import, M2 exported memory).
- The `.wasm` files are committed. Rebuild with the `wat` crate (1.263.0, offline from the cargo registry is fine): a scratch package whose
  `main` is `std::fs::write(out, wat::parse_file(in)?)`, run once per file:
  `watc plugin.wat plugin.wasm && watc bad-import.wat bad-import.wasm && watc bad-export.wat bad-export.wasm`.
- `evil-fetch.js` is the test-only Worker script of M11: it calls `fetch` directly. The spec serves it under `/shell/plugins/`, where the
  Worker CSP of suite-serve applies, and expects the call to fail.
