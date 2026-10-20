import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'npm-translit.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [['list']],
  use: { headless: true, viewport: { width: 1000, height: 400 } },
});
