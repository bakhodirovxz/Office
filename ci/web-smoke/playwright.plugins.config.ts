import { defineConfig } from '@playwright/test';

// The plugin page host (Phase 7 WP5) in the shell against the real suite-serve binary (SUITE_SERVE_BIN, SHELL_DIST_ROOT: see plugins-host.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'plugins-host.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [['list']],
  use: { headless: true, viewport: { width: 1280, height: 800 } },
});
