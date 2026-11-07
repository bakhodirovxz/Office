import { defineConfig } from '@playwright/test';

// The PDF host bridge against the built pdf-web dist (PDF_DIST: see bridge-pdf.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'bridge-pdf.spec.ts',
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
