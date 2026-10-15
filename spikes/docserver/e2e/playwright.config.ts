import { defineConfig } from '@playwright/test';

// Servers are started by the caller (fake_hrm :8790, docserver :8780); see ../scripts/smoke.sh.
export default defineConfig({
  testDir: '.',
  testMatch: 'spike4.spec.ts',
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 240_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    launchOptions: {
      args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'],
    },
  },
});
