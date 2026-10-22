import { defineConfig } from '@playwright/test';

// Server mode over real TLS (PLAN phase 4 WP19): suite-serve with auth = "jwt", its own TLS and a plain-HTTP redirect listener, driven
// by Chromium that trusts only the test certificate's public key (see tls.spec.ts). SUITE_SERVE_BIN and CONVERT_API_FAKE_TOOL default
// to the services release build.
export default defineConfig({
  testDir: '.',
  testMatch: 'tls.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [['list']],
});
