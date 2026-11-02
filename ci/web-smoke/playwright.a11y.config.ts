import { defineConfig } from '@playwright/test';

// The accessible command palette of the shell (Phase 6 WP11) against the real suite-serve binary: see a11y-palette.spec.ts for the env.
export default defineConfig({
  testDir: '.',
  testMatch: 'a11y-palette.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 180_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    launchOptions: { args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] },
  },
});
