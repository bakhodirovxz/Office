import { defineConfig } from '@playwright/test';

// The embed path across two sites (PLAN phase 5, WP5): the fake HR backend's host page, the document server over TLS and the Docs
// editor in a nested frame. Needs the debug builds of hoshiya-serve and fake-hrm (see docserver.spec.ts for the environment).
export default defineConfig({
  testDir: '.',
  testMatch: 'docserver.spec.ts',
  workers: 1,
  retries: 0,
  timeout: 180_000,
  reporter: [['list']],
});
