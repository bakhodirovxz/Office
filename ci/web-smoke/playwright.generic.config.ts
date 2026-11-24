import { defineConfig } from '@playwright/test';

// Generic DocsAPI host against a fresh docker run of the document server: packaging/docserver/rehearse-generic.sh
export default defineConfig({ testDir: '.', testMatch: 'generic.spec.ts', workers: 1, retries: 0, timeout: 240_000, reporter: [['list']] });
