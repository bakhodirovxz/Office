import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import { documentXml } from './docx-xml';

// The Docs plugin bridge, PLAN phase 7 WP4 (docs/plugins.md, "The web bridge"): register, call, actions over two rounds with a
// transferred text map, a stale `rev`, a timeout, and the input lock released after each. The host page here is a STUB: a few lines
// of JS that answer the rounds themselves (no wasm, no Worker; that is the page host of WP5). Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-plugins.config.ts
// Env: DOCS_DIST (default E:/office-target/p1/dist/docs-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p1/dist/docs-web');
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const NAME = path.basename(DOCX);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const MANIFEST = `id = "stub"
version = "1.0.0"
api = 1
apps = ["docs"]
publisher = "Test"
name.en = "Stub"

[permissions]
document = "write"

[[command]]
id = "go"
label.en = "Stub go"
reads = ["textmap"]
`;

// The stub page host. `__mode` picks what it does with a call: insert (one round), rounds (a read, then an insert), stale (echoes
// the wrong rev), silent (never answers), hold (answers when the test calls __release()).
const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<script>
  window.__msgs = []; window.__calls = []; window.__mode = 'insert'; window.__saved = null; window.__held = null;
  const ed = () => document.getElementById('ed').contentWindow;
  window.__toEditor = (msg, transfer) => ed().postMessage(msg, location.origin, transfer);
  const answer = (c, header, rev) => window.__toEditor({ type: 'wordcraft:pluginActions', call: c.call, rev: rev ?? c.rev, header, blobs: [] });
  const insert = (t) => ({ type: 'run', id: 'text.insert', params: { text: t } });
  addEventListener('message', (e) => {
    if (e.source !== ed()) return;
    const d = e.data || {};
    if (d.type === 'wordcraft:save') window.__saved = d.bytes;
    if (d.type === 'wordcraft:pluginCall') {
      window.__calls.push({ call: d.call, rev: d.rev, round: d.round, kind: d.header.kind, command: d.header.command, reads: d.header.reads,
        blobs: (d.blobs || []).map((b) => b.byteLength) });
      const m = window.__mode;
      if (m === 'insert') answer(d, { actions: [insert('PLG')] });
      else if (m === 'rounds') answer(d, d.round === 1 ? { actions: [], next: { reads: [{ what: 'textmap' }] } } : { actions: [insert('TWO')] });
      else if (m === 'stale') answer(d, { actions: [insert('BAD')] }, d.rev + 1);
      else if (m === 'hold') window.__held = d;
    }
    window.__msgs.push({ type: d.type, ok: d.ok, error: d.error, call: d.call, ids: d.ids, errors: d.errors });
  });
  window.__release = () => answer(window.__held, { actions: [insert('PLG')] });
</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
    const file = path.join(DIST, url.pathname === '/' ? 'index.html' : url.pathname);
    if (!file.startsWith(DIST) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) return void res.writeHead(404).end();
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' }).end(fs.readFileSync(file));
  });
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  await new Promise<void>((ok) => server.listen(port, '127.0.0.1', ok));
  base = `http://127.0.0.1:${port}`;
});
test.afterAll(() => { server?.close(); });

const msgs = (page: Page) => page.evaluate(() => (window as any).__msgs as any[]);
const count = async (page: Page, type: string) => (await msgs(page)).filter((m) => m.type === type).length;
const waitFor = (page: Page, type: string, n = 1, timeout = 90_000) => expect.poll(() => count(page, type), { timeout }).toBeGreaterThanOrEqual(n);
const last = async (page: Page, type: string) => (await msgs(page)).filter((m) => m.type === type).at(-1);
const calls = (page: Page) => page.evaluate(() => (window as any).__calls as any[]);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const mode = (page: Page, m: string) => page.evaluate((x) => { (window as any).__mode = x; }, m);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');
const run = (page: Page, id: string) => send(page, { type: 'wordcraft:run', id });

/** Save the document and return its word/document.xml. */
async function xml(page: Page): Promise<string> {
  await page.evaluate(() => { (window as any).__saved = null; });
  await send(page, { type: 'wordcraft:requestSave' });
  await expect.poll(() => page.evaluate(() => (window as any).__saved !== null), { timeout: 30_000 }).toBe(true);
  const bytes: number[] = await page.evaluate(() => Array.from(new Uint8Array((window as any).__saved)));
  return documentXml(Buffer.from(bytes));
}

async function openAndRegister(page: Page, extra: object = {}) {
  await page.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: n, bytes: buf }, [buf]);
  }, [NAME, Array.from(fs.readFileSync(DOCX))] as const);
  await waitFor(page, 'wordcraft:opened');
  await send(page, { type: 'wordcraft:plugins', plugins: [{ manifest: MANIFEST }], ...extra });
  await waitFor(page, 'wordcraft:pluginsSet');
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'wordcraft:ready');
});

test('1 register: the plugin and its command are listed; a bad manifest is refused; an empty list unregisters', async ({ page }) => {
  await openAndRegister(page);
  expect((await last(page, 'wordcraft:pluginsSet')).ids).toEqual(['stub']);
  await send(page, { type: 'wordcraft:commands' });
  // the command registry picks the slot up on the next frame
  await expect.poll(async () => ((await last(page, 'wordcraft:commands')) as any)?.type).toBe('wordcraft:commands');
  await send(page, { type: 'wordcraft:plugins', plugins: [{ manifest: 'id = "x"' }] });
  await expect.poll(() => count(page, 'wordcraft:pluginsSet')).toBe(2);
  const bad = await last(page, 'wordcraft:pluginsSet');
  expect(bad.ids).toEqual([]);
  expect(bad.errors).toHaveLength(1);
  await run(page, 'plugin.stub.go');
  await waitFor(page, 'wordcraft:ran');
  expect((await last(page, 'wordcraft:ran')).ok).toBe(false); // unregistered: unknown command
  expect(await calls(page)).toHaveLength(0);
});

test('2 call: a command reaches the stub with the text map as a transferred buffer and its actions are one change', async ({ page }) => {
  await openAndRegister(page);
  await run(page, 'plugin.stub.go');
  await expect.poll(() => count(page, 'wordcraft:pluginDone'), { timeout: 30_000 }).toBe(1);
  const done = await last(page, 'wordcraft:pluginDone');
  expect(done.ok).toBe(true);
  const c = (await calls(page))[0];
  expect(c).toMatchObject({ round: 1, kind: 'command', command: 'go' });
  expect(c.reads).toEqual({ textmap: 0 });
  expect(c.blobs[0]).toBeGreaterThan(16);
  expect(await xml(page)).toContain('PLG');
});

test('3 actions over two rounds: the second call is a resume that carries a fresh text map', async ({ page }) => {
  await openAndRegister(page);
  await mode(page, 'rounds');
  await run(page, 'plugin.stub.go');
  await expect.poll(() => count(page, 'wordcraft:pluginDone'), { timeout: 30_000 }).toBe(1);
  const cs = await calls(page);
  expect(cs.map((c) => [c.round, c.kind])).toEqual([[1, 'command'], [2, 'resume']]);
  expect(cs[1].call).toBe(cs[0].call);
  expect(cs[1].blobs[0]).toBeGreaterThan(16);
  expect(await xml(page)).toContain('TWO');
});

test('4 stale rev: an answer for another revision changes nothing and the lock is released', async ({ page }) => {
  await openAndRegister(page);
  await mode(page, 'stale');
  await run(page, 'plugin.stub.go');
  await expect.poll(() => count(page, 'wordcraft:pluginDone'), { timeout: 30_000 }).toBe(1);
  const done = await last(page, 'wordcraft:pluginDone');
  expect(done.ok).toBe(false);
  expect(done.error).toContain('stale');
  const text = await xml(page);
  expect(text).not.toContain('BAD');
  expect(text).not.toContain('readOnly');
});

test('5 timeout: no answer restores the document, reports it, and typing works again', async ({ page }) => {
  await openAndRegister(page, { timeoutMs: 400 });
  await mode(page, 'silent');
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await run(page, 'plugin.stub.go');
  await expect.poll(() => count(page, 'wordcraft:pluginDone'), { timeout: 30_000 }).toBe(1);
  const done = await last(page, 'wordcraft:pluginDone');
  expect(done.ok).toBe(false);
  expect(done.error).toContain('no answer');
  await page.keyboard.type('KK');
  const text = await xml(page);
  expect(text).toContain('KK'); // the input lock is released
  expect(text).not.toContain('readOnly');
  // a late answer for the finished call is ignored
  await send(page, { type: 'wordcraft:pluginActions', call: done.call, header: { actions: [{ type: 'run', id: 'text.insert', params: { text: 'LATE' } }] } });
  await page.waitForTimeout(1000);
  expect(await xml(page)).not.toContain('LATE');
});

test('6 input lock: typing while the plugin is pending is refused; after the answer it works', async ({ page }) => {
  await openAndRegister(page);
  await mode(page, 'hold');
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await run(page, 'plugin.stub.go');
  await expect.poll(async () => (await calls(page)).length, { timeout: 30_000 }).toBe(1);
  await page.keyboard.type('QQ');
  await page.waitForTimeout(500);
  await page.evaluate(() => (window as any).__release());
  await expect.poll(() => count(page, 'wordcraft:pluginDone'), { timeout: 30_000 }).toBe(1);
  await page.keyboard.type('RR');
  const text = await xml(page);
  expect(text).toContain('PLG');
  expect(text).toContain('RR');
  expect(text).not.toContain('QQ');
  expect(text).not.toContain('readOnly');
});
