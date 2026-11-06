# Test signing keys (TEST ONLY)

`test-a.seed` and `test-b.seed` are Ed25519 private keys (32-byte seeds, hex) for the plugin signature tests (Phase 7 O4). They
are published in this repository, so anyone can sign with them: never put their public keys in `brand/brand.toml`, a
`policy.toml`, a server config or a user's `settings.toml` outside a test. The public keys are printed by
`cargo xt plugin pubkey <seed file>` (copies in `test-a.pub`, `test-b.pub`).

`pkg/` is a tiny package signed with `test-a` (`cargo xt plugin sign ci/plugins/signing/pkg ci/plugins/signing/test-a.seed`), used by
the Android verifier's check (`packaging/android/sigcheck.sh`) to prove it accepts what the Rust side signs.
