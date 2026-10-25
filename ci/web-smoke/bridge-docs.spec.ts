import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The Docs (wordcraft) host bridge, PLAN phase 5 WP6: read-only open, the save acknowledgement and the periodic autosave over the
// ?host=parent postMessage protocol, against the built docs-web dist, from a tiny same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-docs.config.ts
// Env: DOCS_DIST (default E:/office-target/p1/dist/docs-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p1/dist/docs-web');
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const NAME = path.basename(DOCX);
const AUTOSAVE_MS = 30_000; // host.rs AUTOSAVE_MS
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<script>
  window.__msgs = [];
  addEventListener('message', (e) => {
    if (e.source !== document.getElementById('ed').contentWindow) return;
    const d = e.data || {};
    window.__msgs.push({ type: d.type, name: d.name, dirty: d.dirty, error: d.error, t: Date.now(),
      bytes: d.bytes instanceof ArrayBuffer ? new Uint8Array(d.bytes).slice(0, 2).join(',') + ':' + d.bytes.byteLength : undefined });
  });
  window.__toEditor = (msg, transfer) => document.getElementById('ed').contentWindow.postMessage(msg, location.origin, transfer);
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
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'wordcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}) {
  await page.evaluate(([n, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: n, bytes: buf, ...(x as object) }, [buf]);
  }, [NAME, Array.from(fs.readFileSync(DOCX)), extra] as const);
  await waitFor(page, 'wordcraft:opened');
}
async function type(page: Page) {
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await page.keyboard.type('zz');
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'wordcraft:ready');
});

test('1 control: an editable document becomes dirty when typed in', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('2 readOnly: typing is refused, the document never becomes dirty', async ({ page }) => {
  await open(page, { readOnly: true });
  await type(page);
  await page.keyboard.press('Control+v');
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'wordcraft:dirty' && m.dirty)).toHaveLength(0);
  // a save request is ignored
  await send(page, { type: 'wordcraft:requestSave' });
  await page.waitForTimeout(2000);
  expect(await count(page, 'wordcraft:save')).toBe(0);
});

test('3 a second open without readOnly is editable again', async ({ page }) => {
  await open(page, { readOnly: true });
  await open(page);
  await waitFor(page, 'wordcraft:opened', 2);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('4 a failed save keeps the dirty mark; ok:true clears it', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save');
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await send(page, { type: 'wordcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save', 2);
  await send(page, { type: 'wordcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('5 autosave: handed over while dirty after the period, nothing while clean', async ({ page }) => {
  test.setTimeout(180_000);
  await open(page);
  await page.waitForTimeout(AUTOSAVE_MS + 5000);
  expect(await count(page, 'wordcraft:autosave')).toBe(0);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await waitFor(page, 'wordcraft:autosave', 1, AUTOSAVE_MS + 20_000);
  const a = (await msgs(page)).find((m) => m.type === 'wordcraft:autosave');
  expect(a.name).toBe(NAME);
  expect(a.bytes).toMatch(/^80,75:\d{4,}/);
  expect(await lastDirty(page)).toBe(true); // an autosave is not a save
});

// Patch 0128: canDownload / canPrint withhold the editor's own Save As, exports and Print (default true). Probed with the
// keyboard: F12 is Save As (a download, so a wordcraft:save message), Ctrl+P opens the Print page (the whole frame repaints).
const lumaOf = async (page: Page) => {
  const png = (await page.locator('#ed').screenshot()).toString('base64');
  return page.evaluate(async (b64) => {
    const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
    const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
    const g = c.getContext('2d')!; g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data; let s = 0;
    for (let i = 0; i < d.length; i += 4) s += d[i] + d[i + 1] + d[i + 2];
    return s / (d.length / 4) / 3;
  }, png);
};
async function probe(page: Page) {
  await canvas(page).click({ position: { x: 400, y: 160 } });
  const before = await lumaOf(page);
  await page.keyboard.press('Control+p');
  await page.waitForTimeout(2500);
  const printShift = Math.abs((await lumaOf(page)) - before);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await page.keyboard.press('F12');
  await page.waitForTimeout(2500);
  return { printShift, saves: await count(page, 'wordcraft:save') };
}

test('6 canDownload/canPrint default true: Print opens and Save As reaches the host', async ({ page }) => {
  await open(page);
  const r = await probe(page);
  expect(r.printShift).toBeGreaterThan(5);
  expect(r.saves).toBeGreaterThan(0);
});

test('7 canDownload:false and canPrint:false: Print does not open, Save As sends nothing, Save still works', async ({ page }) => {
  await open(page, { canDownload: false, canPrint: false });
  const r = await probe(page);
  expect(r.printShift).toBeLessThan(1);
  expect(r.saves).toBe(0);
  await type(page);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save'); // the host's own save is not a download
});

test('8 the flags are per open: a later open without them restores both', async ({ page }) => {
  await open(page, { canDownload: false, canPrint: false });
  await open(page);
  await waitFor(page, 'wordcraft:opened', 2);
  const r = await probe(page);
  expect(r.printShift).toBeGreaterThan(5);
  expect(r.saves).toBeGreaterThan(0);
});
