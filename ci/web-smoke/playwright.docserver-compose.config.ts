import { defineConfig } from '@playwright/test';

// WP10: the rehearsal against the containers (see docserver-compose.spec.ts and packaging/docserver/rehearse.sh).
export default defineConfig({ testDir: '.', testMatch: 'docserver-compose.spec.ts', workers: 1, retries: 0, timeout: 180_000, reporter: [['list']] });
