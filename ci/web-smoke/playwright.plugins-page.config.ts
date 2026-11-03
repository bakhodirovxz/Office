import { defineConfig } from '@playwright/test';

// The Plugins page of the shell (Phase 7 WP8) against the real suite-serve binary (SUITE_SERVE_BIN, SHELL_DIST_ROOT: see plugins-page.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'plugins-page.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [['list']],
  use: { headless: true, viewport: { width: 1280, height: 800 } },
});
