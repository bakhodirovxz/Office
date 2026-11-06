import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The Plugins page of the shell (PLAN phase 7 WP8, section 8) against the real suite-serve binary and a FAKE desktop host: `window.ipc` is a stub
// that answers like services/suite-desktop (plugins_list / plugins_install / plugins_enable / plugins_grant -> the `plugins` event). The Rust side of
// the messages is tested in the host's own tests. Checked here: the lazy rule (the page module is fetched on the first click only, and the button
// exists only with a desktop host), what the page shows, the exact messages it sends, the machine policy, a refused package, languages.
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.plugins-page.config.ts
// Env: SUITE_SERVE_BIN, CONVERT_API_FAKE_TOOL, SHELL_DIST_ROOT (a dist with shell/ from scripts/shell-dist.sh; no editor dist is needed).
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'hoshiya-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const DIST = process.env.SHELL_DIST_ROOT ?? 'E:/office-target/p1/dist';
const I18N = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/plugins/i18n.json'), 'utf8')) as Record<string, Record<string, string>>;
const en = I18N.en;
const fill = (s: string, p: Record<string, string>) => s.replace(/\{(\w+)\}/g, (_, n) => p[n]);

let proc: ChildProcess, base: string, dir: string;
test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'plugins-page-spec-'));
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(BIN, ['--config', path.join(dir, 'api.toml'), '--dist', path.join(dir, 'dist'), '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  base = await new Promise<string>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => { out += d; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) ok(m[1]); });
    proc.on('exit', (c) => bad(new Error(`suite-serve exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('suite-serve did not start')), 15000);
  });
});
test.afterAll(() => { proc?.kill(); fs.rmSync(dir, { recursive: true, force: true }); });

/** The fake host. `policy` and `items` are what plugins_list answers; install adds a package; every message is kept in window.__msgs. */
const FAKE_HOST = (policy: unknown) => `
window.__msgs = [];
window.__st = {
  policy: ${JSON.stringify(policy)},
  items: [
    { id: 'demo.plugin', version: '1.0.0', name: { en: 'Demo plugin', ru: 'Демо-плагин' }, publisher: 'Test publisher', document: 'write', network: ['https://api.example.org'], grant: [], enabled: false, machine: false, sha256: 'a'.repeat(64), problem: null },
    { id: 'it.plugin', version: '2.0.0', name: { en: 'IT plugin' }, publisher: 'IT', document: 'read', network: [], grant: [], enabled: false, machine: true, sha256: 'b'.repeat(64), problem: 'the package changed since it was installed' },
    { id: null, name: { en: '<img src=x onerror=window.__xss=1>' }, enabled: false, machine: false, problem: 'plugin.wasm is over 4 MiB' },
  ],
};
window.ipc = { postMessage(s) {
  const m = JSON.parse(s), st = window.__st;
  window.__msgs.push(m);
  const reply = (d) => setTimeout(() => dispatchEvent(new CustomEvent('suite-host', { detail: d })), 0);
  const list = (extra) => reply({ type: 'plugins', ok: true, items: JSON.parse(JSON.stringify(st.items)), policy: st.policy, ...extra });
  const it = () => st.items.find((i) => i.id === m.id);
  switch (m.type) {
    case 'settings_get': return reply({ type: 'settings', values: {} });
    case 'recent': return reply({ type: 'recent', items: [] });
    case 'plugins_list': return list();
    case 'plugins_enable': it().enabled = m.enabled; return list();
    case 'plugins_grant': { const g = it().grant.filter((o) => o !== m.origin); if (m.granted) g.push(m.origin); it().grant = g; return list(); }
    case 'plugins_install':
      if (window.__hold) { window.__answer = () => reply({ type: 'plugins', ok: false, error: 'cancelled', cancelled: true }); return; }   // the native dialog stays open
      if (!st.policy.user_install) return list({ ok: false, error: 'installing plugins is switched off by the administrator' });
      st.items.push({ id: 'new.plugin', version: '1.0.0', name: { en: 'New plugin' }, publisher: 'P', document: 'none', network: [], grant: [], enabled: false, machine: false, sha256: 'c'.repeat(64), problem: null });
      return list({ installed: 'new.plugin' });
  }
} };`;

async function open(page: Page, policy = { user_install: true, user_grants: true }) {
  const urls: string[] = [];
  page.on('request', (r) => urls.push(new URL(r.url()).pathname));
  await page.addInitScript(FAKE_HOST(policy));
  await page.goto(base + '/');
  return urls;
}
const msgs = (page: Page) => page.evaluate(() => (window as any).__msgs.filter((m: any) => m.type.startsWith('plugins_')) as any[]);
const nav = (page: Page) => page.getByRole('button', { name: 'Plugins', exact: true });

test('the page module is fetched on the first click only, and only with a desktop host', async ({ page, browser }) => {
  const urls = await open(page);
  await expect(nav(page)).toBeVisible();
  await page.waitForLoadState('networkidle');
  expect(urls.filter((u) => u.includes('/plugins/'))).toEqual([]);
  await nav(page).click();
  await expect(page.getByRole('heading', { name: en['plg.page.title'], level: 1 })).toBeVisible();
  expect(urls.includes('/shell/plugins/page.js')).toBe(true);
  // no desktop host (a plain browser): no button, no request
  const p2 = await (await browser.newContext()).newPage();
  const urls2: string[] = [];
  p2.on('request', (r) => urls2.push(new URL(r.url()).pathname));
  await p2.goto(base + '/');
  await p2.waitForLoadState('networkidle');
  await expect(nav(p2)).toBeHidden();
  expect(urls2.filter((u) => u.includes('/plugins/'))).toEqual([]);
});

test('the list: names, state, a refused package, plain text only', async ({ page }) => {
  await open(page);
  await nav(page).click();
  const list = page.locator('#plg-list');
  await expect(list.getByRole('heading', { name: /Demo plugin/ })).toBeVisible();
  await expect(list.getByText(fill(en['plg.page.publisher'], { publisher: 'Test publisher' }))).toBeVisible();
  await expect(list.getByText(en['plg.page.doc.write'])).toBeVisible();
  // a package of the administrator that no longer matches its record: shown with the reason, cannot be turned on
  await expect(list.getByText(en['plg.page.machine'])).toBeVisible();
  await expect(list.getByRole('alert').filter({ hasText: 'the package changed' })).toBeVisible();
  await expect(list.getByRole('checkbox', { name: en['plg.page.enabled'] })).toHaveCount(2);
  await expect(page.locator('#plg-en-1')).toBeDisabled();
  // the folder that did not load: its name is text
  await expect(list.getByRole('heading', { name: '<img src=x onerror=window.__xss=1>' })).toBeVisible();
  expect(await page.locator('#plugins-page img').count()).toBe(0);
  expect(await page.evaluate(() => (window as any).__xss)).toBeUndefined();
});

test('enable and the network grant send exact messages and keep the focus', async ({ page }) => {
  await open(page);
  await nav(page).click();
  const en0 = page.locator('#plg-en-0');
  await en0.check();
  await expect(en0).toBeChecked();
  await expect(en0).toBeFocused();
  const net = page.getByRole('checkbox', { name: fill(en['plg.page.net'], { origin: 'https://api.example.org' }) });
  await expect(net).not.toBeChecked();
  await net.check();
  await expect(net).toBeChecked();
  await net.uncheck();
  await expect(net).not.toBeChecked();
  expect(await msgs(page)).toEqual([
    { type: 'plugins_list' },
    { type: 'plugins_enable', id: 'demo.plugin', enabled: true },
    { type: 'plugins_grant', id: 'demo.plugin', origin: 'https://api.example.org', granted: true },
    { type: 'plugins_grant', id: 'demo.plugin', origin: 'https://api.example.org', granted: false },
  ]);
});

test('install from file: the host shows the dialog (no path from the page); the new package is listed, not enabled', async ({ page }) => {
  await open(page);
  await nav(page).click();
  await page.getByRole('button', { name: en['plg.page.install'] }).click();
  await expect(page.getByRole('status')).toHaveText(fill(en['plg.page.installed'], { name: 'New plugin' }));
  await expect(page.locator('#plg-list').getByRole('heading', { name: /New plugin/ })).toBeVisible();
  expect((await msgs(page)).pop()).toEqual({ type: 'plugins_install' });
  await expect(page.locator('#plg-en-3')).not.toBeChecked();
});

test('a machine policy: install and network grants are off, and the page says so', async ({ page }) => {
  await open(page, { user_install: false, user_grants: false });
  await nav(page).click();
  await expect(page.getByRole('button', { name: en['plg.page.install'] })).toBeDisabled();
  await expect(page.getByText(en['plg.page.blocked'])).toBeVisible();
  await expect(page.getByRole('checkbox', { name: /api\.example\.org/ })).toBeDisabled();
  await expect(page.getByText(en['plg.page.net.blocked'])).toBeVisible();
  expect((await msgs(page)).map((m) => m.type)).toEqual(['plugins_list']);
});

test('back, Escape and the language', async ({ page }) => {
  await open(page);
  await nav(page).click();
  await expect(page.locator('#home')).toBeHidden();
  await page.keyboard.press('Escape');
  await expect(page.locator('#plugins-page')).toBeHidden();
  await expect(page.locator('#home')).toBeVisible();
  await expect(nav(page)).toBeFocused();
  await page.locator('#lang').selectOption('ru');
  await page.getByRole('button', { name: I18N.ru['plg.page.title'], exact: true }).click();
  await expect(page.getByRole('heading', { name: I18N.ru['plg.page.title'], level: 1 })).toBeVisible();
  await expect(page.getByRole('button', { name: I18N.ru['plg.page.install'] })).toBeVisible();
  await expect(page.locator('#plg-list').getByRole('heading', { name: /Демо-плагин/ })).toBeVisible();
  await page.getByRole('button', { name: I18N.ru['plg.page.back'] }).click();
  await expect(page.locator('#home')).toBeVisible();
});

// Phase 7 re-check (found with the real native dialog): the 20 s "the application did not respond" timer must not run while the user has the file dialog open,
// or a Cancel after 20 s leaves that error on the page.
test('install from file: a dialog that stays open for more than 20 s does not turn into "did not respond"', async ({ page }) => {
  const urls = await open(page);
  void urls;
  await nav(page).click();
  await expect(page.locator('#plg-list')).toBeVisible();
  await page.evaluate(() => { (window as any).__hold = true; });
  await page.clock.install();   // from here on the page's timers are ours
  await page.getByRole('button', { name: en['plg.page.install'] }).click();
  await page.clock.runFor(25_000);
  await page.evaluate(() => (window as any).__answer());   // Cancel in the dialog
  await page.clock.runFor(100);
  await expect(page.locator('#plg-msg')).not.toContainText(en['plg.page.silent']);
  await expect(page.locator('#plg-msg')).toHaveText('');
});
