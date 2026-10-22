import { defineConfig } from '@playwright/test';

// The home shell against the real suite-serve binary and the built web dists (SUITE_SERVE_BIN, SHELL_DIST_ROOT: see shell.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'shell.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 180_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    // software GL: CI machines have no GPU (same flags as the dist smoke)
    launchOptions: { args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] },
  },
});
