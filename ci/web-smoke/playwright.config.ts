import { defineConfig } from '@playwright/test';

// DIST_ROOT (set by ci.sh) holds <domain>-web/ directories.
export default defineConfig({
  testDir: '.',
  testMatch: 'smoke.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 120_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    // software GL: CI machines have no GPU (same flags as spikes/docserver/e2e)
    launchOptions: { args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] },
  },
});
