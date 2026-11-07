import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The Sheets (gridcraft) host bridge, docs/phase5/r6.md (patch gridcraft/0160): open/opened, read-only open, the save
// acknowledgement and canPrint over the ?host=parent postMessage protocol, against the built sheets-web dist, from a tiny
// same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-sheets.config.ts
// Env: SHEETS_DIST (default E:/office-target/p1/dist/sheets-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.SHEETS_DIST ?? 'E:/office-target/p1/dist/sheets-web');
const XLSX = path.join(ROOT, 'domains/sheets/tests/translit-fixtures/xlsx/01-styles.xlsx');
const NAME = path.basename(XLSX);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<iframe id="sib" src="/sibling.html" style="width:10px;height:10px;border:0"></iframe>
<script>
  window.__msgs = [];
  addEventListener('message', (e) => {
    if (e.source !== document.getElementById('ed').contentWindow) return;
    const d = e.data || {};
    window.__msgs.push({ type: d.type, name: d.name, dirty: d.dirty, error: d.error, version: d.version,
      bytes: d.bytes instanceof ArrayBuffer ? new Uint8Array(d.bytes).slice(0, 2).join(',') + ':' + d.bytes.byteLength : undefined });
  });
  window.__toEditor = (msg, transfer) => document.getElementById('ed').contentWindow.postMessage(msg, location.origin, transfer);
</script>`;
const SIBLING = `<!doctype html><script>window.__post = (msg) => parent.document.getElementById('ed').contentWindow.postMessage(msg, location.origin);</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
    if (url.pathname === '/sibling.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(SIBLING);
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
const waitFor = (page: Page, type: string, n = 1) => expect.poll(() => count(page, type), { timeout: 90_000 }).toBeGreaterThanOrEqual(n);
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'gridcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}, name = NAME, bytes = fs.readFileSync(XLSX)) {
  const n = await count(page, 'gridcraft:opened');
  await page.evaluate(([nm, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'gridcraft:open', name: nm, bytes: buf, ...(x as object) }, [buf]);
  }, [name, Array.from(bytes), extra] as const);
  if (name === NAME) await waitFor(page, 'gridcraft:opened', n + 1);
}
async function type(page: Page) {
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.type('zz');
  await page.keyboard.press('Enter');
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'gridcraft:ready');
});

test('1 ready once with a version, then open: shown, announced by name, editable', async ({ page }) => {
  const m = await msgs(page);
  expect(m.filter((x) => x.type === 'gridcraft:ready')).toHaveLength(1);
  expect(m.find((x) => x.type === 'gridcraft:ready').version).toMatch(/^\d+\.\d+\.\d+/);
  await open(page);
  expect((await msgs(page)).find((x) => x.type === 'gridcraft:opened').name).toBe(NAME);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  expect(await count(page, 'gridcraft:failed')).toBe(0);
});

test('2 save acknowledgement: unanswered and ok:false keep the dirty mark, ok:true clears it', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'gridcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^80,75:\d{4,}/); // "PK", a zip
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await send(page, { type: 'gridcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save', 2);
  await send(page, { type: 'gridcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('3 readOnly: an edit is taken back, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  await open(page, { readOnly: true });
  await type(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Enter'); // the read-only message box
  await page.keyboard.press('Control+s');
  await send(page, { type: 'gridcraft:requestSave' });
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'gridcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'gridcraft:save')).toBe(0);
});

test('4 a second open without readOnly replaces the first and is editable again', async ({ page }) => {
  await open(page, { readOnly: true });
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save');
});

// Print hands a PDF to a hidden iframe of the editor page (web.rs print_pdf); with canPrint:false nothing is handed over.
const printFrames = (page: Page) => page.frameLocator('#ed').locator('iframe').count();
test('5 canPrint default true: Ctrl+P prints; canPrint:false refuses it and Save still works', async ({ page }) => {
  await open(page);
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.press('Control+p');
  await expect.poll(() => printFrames(page), { timeout: 20_000 }).toBeGreaterThan(0);
  await page.goto(base + '/host.html');
  await waitFor(page, 'gridcraft:ready');
  await open(page, { canPrint: false, canDownload: false });
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.press('Control+p');
  await page.waitForTimeout(4000);
  expect(await printFrames(page)).toBe(0);
  await page.keyboard.press('Enter'); // the "not available" message box
  await type(page);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save'); // the host's own save is not a download
});

// (a plain text file named .xlsx opens as text, as in Excel's import; a damaged package does not open)
test('6 a damaged workbook: failed, no opened, and the editor keeps going', async ({ page }) => {
  await open(page, {}, 'broken.xlsx', Buffer.concat([Buffer.from([0x50, 0x4b, 3, 4]), Buffer.alloc(64, 7)]));
  await waitFor(page, 'gridcraft:failed');
  expect(await count(page, 'gridcraft:opened')).toBe(0);
  await page.keyboard.press('Enter');
  await open(page);
});

test('7 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'gridcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'gridcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(XLSX))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'gridcraft:opened')).toBe(0);
  expect(await count(page, 'gridcraft:save')).toBe(0);
});
