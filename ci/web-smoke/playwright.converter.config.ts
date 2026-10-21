import { defineConfig } from '@playwright/test';

// The converter page (services/convert-api/ui) against the real convert-api binary with a stand-in tool.
// CONVERT_API_BIN and CONVERT_API_FAKE_TOOL default to the services release build.
export default defineConfig({
  testDir: '.',
  testMatch: 'converter.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [['list']],
  use: { headless: true, viewport: { width: 1100, height: 900 } },
});
