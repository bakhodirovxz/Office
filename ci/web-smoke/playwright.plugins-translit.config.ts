import { defineConfig } from '@playwright/test';

// The uz-translit plugin against the built-in command in the built docs-web dist (see plugins-translit.spec.ts).
export default defineConfig({
  testDir: '.',
  testMatch: 'plugins-translit.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 30 * 60_000,
  reporter: [['list']],
  use: {
    headless: true,
    viewport: { width: 1280, height: 800 },
    launchOptions: { args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] },
  },
});
