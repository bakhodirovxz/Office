import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The plugin page host (PLAN phase 7 WP5) in the home shell, against the real suite-serve binary and a STUB editor (WP4's bridge is not
// merged yet): the lazy rule, load, the panel, the grant prompt, and the malicious cases M3, M6, M7, M9, M11 (plus the import check, M1/M2).
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.plugins.config.ts
// Env: SUITE_SERVE_BIN, CONVERT_API_FAKE_TOOL, SHELL_DIST_ROOT (a dist with shell/ from scripts/shell-dist.sh; no editor dist is needed).
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'hoshiya-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const DIST = process.env.SHELL_DIST_ROOT ?? 'E:/office-target/p1/dist';
const FIX = path.join(ROOT, 'ci/plugins/webhost-test');
const I18N = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/plugins/i18n.json'), 'utf8')).en as Record<string, string>;
const fill = (s: string, p: Record<string, string>) => s.replace(/\{(\w+)\}/g, (_, n) => p[n]);
const WORKER_CSP = "default-src 'none'; script-src 'wasm-unsafe-eval'";

let proc: ChildProcess, base: string, dir: string, prefix: string;
test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'plugins-spec-'));
  // a copy of the assembled shell plus the test-only Worker script of M11
  fs.cpSync(path.join(DIST, 'shell'), path.join(dir, 'shell'), { recursive: true });
  fs.copyFileSync(path.join(FIX, 'evil-fetch.js'), path.join(dir, 'shell/plugins/evil-fetch.js'));
  fs.copyFileSync(path.join(FIX, 'evil-fetch.js'), path.join(dir, 'shell/evil-fetch-open.js'));   // the control: the same script outside plugins/ has only the page's policy
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(BIN, ['--config', path.join(dir, 'api.toml'), '--dist', path.join(dir, 'dist'), '--shell', path.join(dir, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  base = await new Promise<string>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => { out += d; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) ok(m[1]); });
    proc.on('exit', (c) => bad(new Error(`suite-serve exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('suite-serve did not start')), 15000);
  });
});
test.beforeAll(async () => { prefix = (await (await fetch(base + '/shell-config.json')).json()).apps.find((a: any) => a.id === 'docs').prefix; });
test.afterAll(() => { proc?.kill(); fs.rmSync(dir, { recursive: true, force: true }); });

const wasmOf = (f: string) => fs.readFileSync(path.join(FIX, f));
const sha = (b: Buffer) => crypto.createHash('sha256').update(b).digest('hex');
const COMMANDS = ['hello', 'net', 'spin', 'grow', 'xss', 'edit'];
/** A descriptor as the server will list it (WP8): the manifest as JSON, the wasm by URL and SHA-256. */
function plugin(id: string, o: { file?: string; network?: string[]; grant?: string[]; memory_mb?: number } = {}) {
  const wasm = wasmOf(o.file ?? 'plugin.wasm');
  return {
    id, version: '1.0.0', sha256: sha(wasm), wasm: `/test-plugins/${o.file ?? 'plugin.wasm'}`, ...(o.grant ? { grant: o.grant } : {}),
    manifest: {
      id, version: '1.0.0', api: 1, apps: ['docs'], publisher: 'Test publisher', name: { en: 'Test plugin', ru: 'Тестовый плагин' },
      permissions: { document: 'write', network: o.network ?? [], memory_mb: o.memory_mb ?? 2 },
      command: COMMANDS.map((c) => ({ id: c, label: { en: `Cmd ${c}` }, reads: ['none'], params: {} })),
      toolbar: [{ command: 'hello', icon: 'x', group: { en: 'Plugins' } }], menu: [{ command: 'xss', place: 'tools' }],
      panel: [{ id: 'p', title: { en: 'Panel' } }],
    },
  };
}

// The stub editor: says ready, answers pluginActions with one ok per action, and calls a plugin command on request.
const STUB = `<!doctype html><title>stub</title><script>
window.got = [];
addEventListener('message', (e) => {
  window.got.push(e.data);
  if (e.data.type === '__P__:pluginActions') parent.postMessage({ type: '__P__:pluginResults', call: e.data.call, results: (e.data.actions || []).map(() => ({ ok: true })), reads: [] }, location.origin);
});
window.callPlugin = (plugin, command) => parent.postMessage({ type: '__P__:pluginCall', call: 'c' + Math.random(), plugin, command, params: {}, rev: 1, reads: {} }, location.origin);
parent.postMessage({ type: '__P__:ready' }, location.origin);
</script>`;

/** Opens the shell with the given plugins in shell-config.json and the stub editor as Docs. Returns the request log. */
async function openShell(page: Page, plugins: unknown[] | undefined, extra: Record<string, unknown> = {}) {
  const urls: string[] = [];
  if (process.env.PLG_DEBUG) { page.on('pageerror', (e) => console.log('PAGEERROR', e.message)); page.on('console', (m) => console.log('CONSOLE', m.text())); }
  page.on('request', (r) => urls.push(new URL(r.url()).pathname));
  await page.route('**/shell-config.json', async (route) => {
    const cfg = await (await route.fetch()).json();
    if (plugins) Object.assign(cfg, { plugins, ...extra });
    await route.fulfill({ json: cfg });
  });
  await page.route('**/test-plugins/*', (route) => route.fulfill({ body: wasmOf(path.basename(new URL(route.request().url()).pathname)), contentType: 'application/wasm' }));
  await page.route((u) => u.pathname.startsWith('/docs/'), (route) => route.fulfill({ body: STUB.split('__P__').join(prefix), contentType: 'text/html' }));
  await page.goto(base + '/');
  await page.locator('#cards li').first().getByRole('button').click();
  await page.waitForSelector('iframe[src^="/docs/"]');
  return urls;
}
const stub = (page: Page) => page.frames().find((f) => f.url().includes('/docs/'))!;
const call = (page: Page, id: string, cmd: string) => stub(page).evaluate(([i, c]) => (window as any).callPlugin(i, c), [id, cmd]);
const status = (page: Page) => page.locator('#ed-status');
const audit = (page: Page) => page.evaluate(() => (window as any).PluginHost.last.audit);
const logs = (page: Page) => page.evaluate(() => (window as any).PluginHost.last.logs.map((l: any) => l.text) as string[]);
const got = (page: Page, type: string) => stub(page).evaluate((t) => (window as any).got.filter((m: any) => m.type === t), prefix + ':' + type);
const registered = (page: Page) => expect.poll(async () => (await got(page, 'plugins')).length).toBe(1);

test('no plugin: the page host is never requested', async ({ page }) => {
  const urls = await openShell(page, undefined);
  await page.waitForLoadState('networkidle');
  expect(urls.filter((u) => u.includes('/plugins/'))).toEqual([]);
  expect(await page.evaluate(() => typeof (window as any).PluginHost)).toBe('undefined');
  // an empty list is the same
  const p2 = await page.context().newPage();
  const urls2 = await openShell(p2, []);
  await p2.waitForLoadState('networkidle');
  expect(urls2.filter((u) => u.includes('/plugins/'))).toEqual([]);
});

test('a plugin loads lazily: host.js at start, the Worker at the first call; the editor gets the slots', async ({ page }) => {
  const urls = await openShell(page, [plugin('plg-test')]);
  await expect.poll(() => urls.includes('/shell/plugins/host.js')).toBe(true);
  expect(urls.includes('/shell/plugins/worker.js')).toBe(false);
  await registered(page);
  const reg = (await got(page, 'plugins'))[0];
  expect(reg.commands.map((c: any) => [c.slot, c.id, c.label, c.mutates])).toEqual(COMMANDS.map((c, i) => [i, `plugin.plg-test.${c}`, `Cmd ${c}`, true]));
  expect(reg.toolbar).toEqual([{ slot: 0, icon: 'x', group: 'Plugins' }]);
  expect(reg.menu).toEqual([{ slot: 4, place: 'tools', group: '' }]);
  await call(page, 'plg-test', 'hello');
  await expect(status(page)).toHaveText('hello done');
  expect(urls.includes('/shell/plugins/worker.js')).toBe(true);
  // the Worker script is served under its own CSP
  const csp = await page.evaluate(async () => (await fetch('/shell/plugins/worker.js')).headers.get('content-security-policy'));
  expect(csp).toBe(WORKER_CSP);
});

test('the panel: accessible names, buttons call the plugin, plain text only; a bottom sheet on a phone', async ({ page }) => {
  await openShell(page, [plugin('plg-test')]);
  await registered(page);
  await call(page, 'plg-test', 'hello');
  const panel = page.getByRole('region', { name: 'Test plugin: Panel' });
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('heading', { name: 'Hello panel' })).toBeVisible();
  await expect(panel.getByRole('button', { name: I18N['plg.close'] })).toBeVisible();
  // the side panel makes room: the editor frame is narrower
  const w = await page.evaluate(() => document.querySelector('#frames iframe')!.getBoundingClientRect().width);
  expect(w).toBeLessThan(1280 - 300);
  await panel.getByRole('button', { name: 'Go' }).click();
  await expect(status(page)).toHaveText('clicked');
  // Escape closes it
  await panel.press('Escape');
  await expect(panel).toHaveCount(0);
  // phone: the same panel is a sheet along the bottom edge
  await page.setViewportSize({ width: 390, height: 800 });
  await call(page, 'plg-test', 'hello');
  await expect(panel).toBeVisible();
  const box = (await panel.boundingBox())!;
  expect(Math.round(box.x)).toBe(0);
  expect(Math.round(box.width)).toBe(390);
  expect(Math.round(box.y + box.height)).toBe(800);
  expect(box.height).toBeLessThanOrEqual(0.6 * 800 + 1);
  const bw = await page.evaluate(() => document.querySelector('#frames iframe')!.getBoundingClientRect().width);
  expect(bw).toBe(390);
});

test('M9: markup and a 10,000-character text are shown as text and cut', async ({ page }) => {
  await openShell(page, [plugin('plg-test')]);
  await registered(page);
  await call(page, 'plg-test', 'xss');
  const panel = page.getByRole('region', { name: 'Test plugin: Panel' });
  await expect(panel).toBeVisible();
  expect(await panel.locator('img, script, b').count()).toBe(0);
  expect(await panel.locator('h3').textContent()).toBe('<img src=x onerror=window.__xss=1>');
  expect(await panel.getByRole('button', { name: '<b>bold</b>' }).count()).toBe(1);
  expect((await panel.locator('.plg-text').textContent())!.length).toBe(1024);
  expect(await page.evaluate(() => (window as any).__xss)).toBeUndefined();
});

test('M3: fetch to an origin the manifest does not list ends the invocation, audited, nothing is fetched', async ({ page }) => {
  const urls = await openShell(page, [plugin('plg-test')]);
  await registered(page);
  await call(page, 'plg-test', 'net');
  await expect(status(page)).toHaveText(fill(I18N['plg.failed'], { name: 'Test plugin', error: 'network not permitted' }));
  expect((await logs(page)).some((l) => l.includes('"kind":"resume"'))).toBe(false);   // the plugin is never called again
  expect(await audit(page)).toEqual([{ plugin: 'plg-test', kind: 'network.refused', origin: 'https://example.org' }]);
  expect(urls.some((u) => u.includes('example.org'))).toBe(false);
  expect(await page.locator('dialog.plg-dlg').count()).toBe(0);   // nothing to grant: the manifest never asked
});

test('grant prompt: asked once per origin in local mode', async ({ page }) => {
  const net = 'https://example.org';
  await openShell(page, [plugin('plg-net', { network: [net] })]);
  await registered(page);
  const dlg = page.getByRole('dialog');
  // deny
  await call(page, 'plg-net', 'net');
  await expect(dlg).toBeVisible();
  await expect(dlg.getByRole('heading')).toHaveText(fill(I18N['plg.grant.title'], { name: 'Test plugin', origin: 'example.org' }));
  await expect(dlg).toContainText(fill(I18N['plg.grant.body'], { publisher: 'Test publisher' }));
  await dlg.getByRole('button', { name: I18N['plg.grant.deny'] }).click();
  await expect(status(page)).toHaveText('resumed');
  expect((await audit(page)).map((a: any) => a.kind)).toEqual(['network.denied']);
  // allow: remembered, so the next call does not ask; the page's own CSP (connect-src 'self') still stops the request, which the plugin sees as a failure
  await call(page, 'plg-net', 'net');
  await dlg.getByRole('button', { name: I18N['plg.grant.allow'] }).click();
  await expect.poll(async () => (await audit(page)).length).toBe(2);
  expect(await page.evaluate(() => localStorage.getItem('plg.grants'))).toBe(JSON.stringify({ 'plg-net': [net] }));
  await expect.poll(async () => (await logs(page)).filter((l) => l.includes('"results":[{"error":"the request failed"}]')).length).toBe(1);
  await call(page, 'plg-net', 'net');
  await expect.poll(async () => (await audit(page)).length).toBe(3);
  expect(await dlg.count()).toBe(0);
});

test('server mode: only the descriptor grants count, no prompt', async ({ page }) => {
  await openShell(page, [plugin('plg-net', { network: ['https://example.org'], grant: [] })]);
  await registered(page);
  await call(page, 'plg-net', 'net');
  await expect(status(page)).toHaveText('resumed');
  expect((await audit(page)).map((a: any) => a.kind)).toEqual(['network.denied']);
  expect(await page.locator('dialog.plg-dlg').count()).toBe(0);
});

test('M6: a loop in call is stopped at the time budget; the page stays responsive; the plugin works again after', async ({ page }) => {
  await openShell(page, [plugin('plg-test')], { pluginBudgetMs: 500 });
  await registered(page);
  await call(page, 'plg-test', 'hello');
  await expect(status(page)).toHaveText('hello done');   // the Worker is up, the spin below measures the call only
  await call(page, 'plg-test', 'spin');
  const late = await page.evaluate(() => new Promise<number>((ok) => { const t = performance.now(); setTimeout(() => ok(performance.now() - t - 100), 100); }));
  expect(late).toBeLessThan(150);   // the main thread kept its timers
  await expect(status(page)).toHaveText(fill(I18N['plg.timeout'], { name: 'Test plugin' }));
  expect((await audit(page)).map((a: any) => a.kind)).toEqual(['stopped']);
  // the editor was told to put everything back
  expect((await got(page, 'pluginActions')).filter((m: any) => m.abort).length).toBe(1);
  await call(page, 'plg-test', 'hello');
  await expect(status(page)).toHaveText('hello done');
});

test('M7: growing memory stops at memory_mb; nothing crashes', async ({ page }) => {
  await openShell(page, [plugin('plg-test', { memory_mb: 2 })]);
  await registered(page);
  await call(page, 'plg-test', 'grow');
  await expect(status(page)).toHaveText('hog capped');
  const mem = await page.evaluate(() => (window as any).PluginHost.last.mem['plg-test']);
  expect(mem).toBe(2 * 1024 * 1024);   // exactly the maximum: the last grow was refused
});

test('M11: a Worker script that calls fetch or importScripts itself is blocked by the Worker CSP', async ({ page }) => {
  await openShell(page, undefined);
  const r = await page.evaluate(() => new Promise<Record<string, string>>((ok) => {
    const w = new Worker('/shell/plugins/evil-fetch.js');
    w.onmessage = (e) => ok(e.data);
    w.postMessage(1);
  }));
  expect(r.same).toMatch(/^blocked/);
  expect(r.other).toMatch(/^blocked/);
  expect(r.imp).toMatch(/^blocked/);
  // the control: the same script one directory up (the strict page policy only) reaches the server, so it is the Worker's header that stops it
  const open = await page.evaluate(() => new Promise<Record<string, string>>((ok) => {
    const w = new Worker('/shell/evil-fetch-open.js');
    w.onmessage = (e) => ok(e.data);
    w.postMessage(1);
  }));
  expect(open.same).toBe('reached 200');
  expect(open.other).toMatch(/^blocked/);   // the page policy (connect-src 'self') still stops the foreign origin
});

test('M1/M2: a module that imports WASI or exports its own memory is refused at load', async ({ page }) => {
  await openShell(page, [plugin('plg-imp', { file: 'bad-import.wasm' }), plugin('plg-exp', { file: 'bad-export.wasm' })]);
  await registered(page);
  await call(page, 'plg-imp', 'hello');
  await expect(status(page)).toContainText('imports not allowed: wasi_snapshot_preview1.fd_write');
  await call(page, 'plg-exp', 'hello');
  await expect(status(page)).toContainText('exports a memory');
});

test('a wasm file that does not match its SHA-256 is refused', async ({ page }) => {
  const p = plugin('plg-test');
  p.sha256 = '0'.repeat(64);
  await openShell(page, [p]);
  await registered(page);
  await call(page, 'plg-test', 'hello');
  await expect(status(page)).toContainText('checksum');
});

test('editor actions go to the editor and the plugin is resumed with the results', async ({ page }) => {
  await openShell(page, [plugin('plg-test')]);
  await registered(page);
  await call(page, 'plg-test', 'edit');
  await expect(status(page)).toHaveText('resumed');
  const msgs = await got(page, 'pluginActions');
  expect(msgs.map((m: any) => [m.actions.map((a: any) => a.id), m.more])).toEqual([[['text.insert'], true], [[], false]]);
  expect((await logs(page)).some((l) => l.includes('"results":[{"ok":true}]'))).toBe(true);
});
