import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The PDF (pdfcraft) host bridge, docs/phase5/r6.md (patch pdfcraft/0122): open/opened, read-only open, the save
// acknowledgement and canPrint over the ?host=parent postMessage protocol, against the built pdf-web dist, from a tiny
// same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-pdf.config.ts
// Env: PDF_DIST (default E:/office-target/p1/dist/pdf-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.PDF_DIST ?? 'E:/office-target/p1/dist/pdf-web');
const PDF = path.join(ROOT, 'docs/phase4/wp23/fixtures/namuna-hisobot.pdf');
const NAME = path.basename(PDF);
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
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}, name = NAME) {
  const n = await count(page, 'pdfcraft:opened');
  await page.evaluate(([nm, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'pdfcraft:open', name: nm, bytes: buf, ...(x as object) }, [buf]);
  }, [name, Array.from(fs.readFileSync(PDF)), extra] as const);
  await waitFor(page, 'pdfcraft:opened', n + 1);
}
// an edit with the keyboard: the command palette (Ctrl+K) runs "Rotate pages clockwise"
async function edit(page: Page) {
  await canvas(page).click({ position: { x: 550, y: 400 } });
  await page.keyboard.press('Control+k');
  await page.waitForTimeout(500);
  await page.keyboard.type('Rotate pages clockwise');
  await page.waitForTimeout(300);
  await page.keyboard.press('Enter');
}
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

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'pdfcraft:ready');
});

test('1 open: shown and announced by name; an edit makes it dirty', async ({ page }) => {
  await open(page);
  expect((await msgs(page)).find((x) => x.type === 'pdfcraft:opened').name).toBe(NAME);
  await edit(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  expect(await count(page, 'pdfcraft:failed')).toBe(0);
});

test('2 save acknowledgement: unanswered and ok:false keep the dirty mark, ok:true clears it', async ({ page }) => {
  await open(page);
  await edit(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'pdfcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^37,80:\d{4,}/); // "%P"
  expect(Number(s.bytes.split(':')[1])).toBeGreaterThan(fs.statSync(PDF).size); // an incremental update appends
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save', 2);
  await send(page, { type: 'pdfcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('3 readOnly: an edit is reverted, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  await open(page, { readOnly: true });
  await edit(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Control+s');
  await send(page, { type: 'pdfcraft:requestSave' });
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'pdfcraft:save')).toBe(0);
});

test('4 canPrint default true opens Print; canPrint:false keeps it closed and Save still works', async ({ page }) => {
  const printShift = async () => {
    await canvas(page).click({ position: { x: 550, y: 400 } });
    await page.waitForTimeout(1000);
    const before = await lumaOf(page);
    await page.keyboard.press('Control+p');
    await page.waitForTimeout(6000); // past the "Not allowed" toast
    return Math.abs((await lumaOf(page)) - before);
  };
  await open(page);
  expect(await printShift()).toBeGreaterThan(3);
  await page.goto(base + '/host.html');
  await waitFor(page, 'pdfcraft:ready');
  await open(page, { canPrint: false, canDownload: false });
  expect(await printShift()).toBeLessThan(1);
  await edit(page);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save'); // the host's own save is not a download
});

test('5 a second open replaces the first document', async ({ page }) => {
  await open(page, {}, 'first.pdf');
  await open(page, {}, 'second.pdf');
  await edit(page);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  expect((await msgs(page)).filter((x) => x.type === 'pdfcraft:save').map((x) => x.name)).toEqual(['second.pdf']);
});

test('6 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'pdfcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'pdfcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(PDF))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'pdfcraft:opened')).toBe(0);
  expect(await count(page, 'pdfcraft:save')).toBe(0);
});
