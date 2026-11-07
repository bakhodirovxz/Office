import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The Slides (deckcraft) host bridge, PLAN phase 4 WP8: the ?host=parent postMessage protocol against the built slides-web dist,
// from a tiny same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-slides.config.ts
// Env: SLIDES_DIST (default E:/office-target/p1/dist/slides-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.SLIDES_DIST ?? 'E:/office-target/p1/dist/slides-web');
const PPTX = path.join(ROOT, 'domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx');
const NAME = path.basename(PPTX);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

// The host page keeps every message from the editor frame and lets the test post to it (as the frame's parent) or from a sibling frame.
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
const types = async (page: Page) => (await msgs(page)).map((m) => m.type);
const count = async (page: Page, type: string) => (await types(page)).filter((t) => t === type).length;
const waitFor = (page: Page, type: string, n = 1) => expect.poll(() => count(page, type), { timeout: 90_000 }).toBeGreaterThanOrEqual(n);

async function open(page: Page, name = NAME, bytes = fs.readFileSync(PPTX), extra: object = {}) {
  await page.evaluate(([n, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'deckcraft:open', name: n, bytes: buf, ...(x as object) }, [buf]);
  }, [name, Array.from(bytes), extra] as const);
}
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'deckcraft:ready');
});

test('1 ready: announced once with a version, clean, nothing else sent', async ({ page }) => {
  const m = await msgs(page);
  expect(m.filter((x) => x.type === 'deckcraft:ready')).toHaveLength(1);
  expect(m.find((x) => x.type === 'deckcraft:ready').version).toMatch(/^\d+\.\d+\.\d+/);
  await waitFor(page, 'deckcraft:dirty');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:dirty').dirty).toBe(false);
  expect(await count(page, 'deckcraft:save')).toBe(0);
  expect(await count(page, 'deckcraft:failed')).toBe(0);
});

test('2 open: the file is shown and announced by name', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:opened').name).toBe(NAME);
  expect(await count(page, 'deckcraft:failed')).toBe(0);
});

test('3 requestSave: a .pptx comes back as a transferred ArrayBuffer, named after the opened file', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'deckcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^80,75:\d{4,}/); // "PK", a zip
});

const lastDirty = async (page: Page) => (await msgs(page)).filter((x) => x.type === 'deckcraft:dirty').map((x) => x.dirty).at(-1);
const newSlide = async (page: Page) => {
  await canvas(page).click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+Shift+N'); // New Slide
};

test('4 dirty: an edit reports true; a save reports false once the host acknowledged it (r6)', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:saved', ok: false }));
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save', 2);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:saved', ok: true }));
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('5 Ctrl+S in the editor sends a .pptx, never a .deckcraft download', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await canvas(page).click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+s');
  await waitFor(page, 'deckcraft:save');
  const saves = (await msgs(page)).filter((x) => x.type === 'deckcraft:save');
  expect(saves).toHaveLength(1);
  expect(saves[0].name).toBe(NAME);
});

test('6 a second open replaces the first', async ({ page }) => {
  await open(page, 'first.pptx');
  await waitFor(page, 'deckcraft:opened');
  await open(page, 'second.pptx');
  await waitFor(page, 'deckcraft:opened', 2);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:save').name).toBe('second.pptx');
});

test('7 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'deckcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'deckcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(PPTX))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'deckcraft:opened')).toBe(0);
  expect(await count(page, 'deckcraft:save')).toBe(0);
});

test('8 a file that is not a presentation: failed, and the editor keeps going', async ({ page }) => {
  await open(page, 'broken.pptx', Buffer.from('this is not a zip file'));
  await waitFor(page, 'deckcraft:failed');
  expect(await count(page, 'deckcraft:opened')).toBe(0);
  await open(page);
  await waitFor(page, 'deckcraft:opened');
});

// docs/phase5/r6.md (patch deckcraft/0160): the read-only open and canPrint.
test('9 readOnly: an edit is taken back, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  await open(page, NAME, fs.readFileSync(PPTX), { readOnly: true });
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Control+s');
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'deckcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'deckcraft:save')).toBe(0);
});

// Print on the web is a PDF download of the editor page (SlideApp::print_pdf, the Print button of File › Print, which Ctrl+P opens);
// canPrint:false refuses it.
test('10 canPrint default true: Print downloads the PDF to print; canPrint:false refuses it and Save still works', async ({ page }) => {
  const printed = async () => {
    await canvas(page).click({ position: { x: 500, y: 350 } });
    await page.keyboard.press('Control+p');
    await page.waitForTimeout(1500);
    const dl = page.waitForEvent('download', { timeout: 8000 }).then((d) => d.suggestedFilename()).catch(() => null);
    await canvas(page).click({ position: { x: 251, y: 150 } }); // the Print button
    return dl;
  };
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  expect(await printed()).toMatch(/\.pdf$/);
  expect(await count(page, 'deckcraft:save')).toBe(0); // a print is not a save
  await page.goto(base + '/host.html');
  await waitFor(page, 'deckcraft:ready');
  await open(page, NAME, fs.readFileSync(PPTX), { canPrint: false, canDownload: false });
  await waitFor(page, 'deckcraft:opened');
  expect(await printed()).toBeNull();
  await page.keyboard.press('Escape'); // back from File › Print
  await newSlide(page);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
});

test('not framed: ?host=parent alone starts the normal app, with the sample deck and no bridge', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(base + '/index.html?host=parent');
  await page.waitForFunction(() => !!(window as any).wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 90_000 });
  expect(errors).toEqual([]);
});
