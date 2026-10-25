import { defineConfig } from '@playwright/test';

// The Docs host bridge against the built docs-web dist (DOCS_DIST: see bridge-docs.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'bridge-docs.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 180_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    // software GL: CI machines have no GPU (same flags as spikes/docserver/e2e)
    launchOptions: { args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] },
  },
});
