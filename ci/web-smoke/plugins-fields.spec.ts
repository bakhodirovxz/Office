import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import * as zlib from 'node:zlib';

// Phase 7 WP7, PLAN 7 (web): the real plugins/template-fields module in a Worker of the real page host, through the editor bridge.
// Panel with a filter box and an Insert button per field; Insert puts a content control tagged with the field name in the document (saved docx:
// w:sdt with w:tag, round trip through open/save); the caret in a field highlights it; "Check fields" lists missing and unknown tags.
// The field names are the synthetic defaults of the manifest.
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.plugins-fields.config.ts
// Env: DOCS_DIST (default E:/office-target/p7-savelock/dist/docs-web), TEMPLATE_FIELDS_PLUGIN_WASM (from `cargo xt plugin build template-fields`).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p7-savelock/dist/docs-web');
const WASM = process.env.TEMPLATE_FIELDS_PLUGIN_WASM ?? 'E:/office-target/p7-fields/wasm32-unknown-unknown/plugin/template_fields_plugin.wasm';
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.json': 'application/json', '.css': 'text/css' };

const HOST = (descriptor: string) => `<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="/plugins/host.css"><body>
<div id="root" style="position:relative"><iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe></div>
<script src="/plugins/host.js"></script>
<script>
  window.__msgs = []; window.__saved = null; window.__say = [];
  const ed = () => document.getElementById('ed').contentWindow;
  window.__toEditor = (msg, transfer) => ed().postMessage(msg, location.origin, transfer);
  const H = PluginHost.make({ cfg: { plugins: [${descriptor}] }, lang: 'en', frames: {} }, (m) => window.__say.push(m), document.getElementById('root'));
  addEventListener('message', (e) => {
    if (e.source !== ed()) return;
    const d = e.data || {};
    if (String(d.type).startsWith('wordcraft:')) H.onMessage({ app: { id: 'docs', prefix: 'wordcraft' }, el: { contentWindow: ed() } }, d);
    if (d.type === 'wordcraft:save') window.__saved = d.bytes;
    window.__msgs.push({ type: d.type, error: d.error, ok: d.ok, id: d.id });
  });
</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  const { descriptor } = await import(path.join(ROOT, 'scripts/plugins/manifest-json.mjs').replace(/\\/g, '/').replace(/^([A-Za-z]):/, 'file:///$1:'));
  const d = JSON.stringify(descriptor(path.join(ROOT, 'plugins/template-fields'), WASM, '/test-plugins/plugin.wasm'));
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST(d));
    const file = url.pathname.startsWith('/plugins/') ? path.join(ROOT, 'web', url.pathname)
      : url.pathname === '/test-plugins/plugin.wasm' ? WASM
      : path.join(DIST, url.pathname === '/' ? 'index.html' : url.pathname);
    if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) return void res.writeHead(404).end();
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' }).end(fs.readFileSync(file));
  });
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  await new Promise<void>((ok) => server.listen(port, '127.0.0.1', ok));
  base = `http://127.0.0.1:${port}`;
});
test.afterAll(() => { server?.close(); });

const count = (page: Page, type: string) => page.evaluate((t) => (window as any).__msgs.filter((m: any) => m.type === t).length, type);
const waitCount = (page: Page, type: string, n: number) => expect.poll(() => count(page, type), { timeout: 120_000 }).toBeGreaterThanOrEqual(n);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);

/** Every entry of a docx, inflated, by name. */
function parts(buf: Buffer): Map<string, Buffer> {
  const out = new Map<string, Buffer>();
  const end = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  let p = buf.readUInt32LE(end + 16);
  for (let n = buf.readUInt16LE(end + 10); n > 0; n--) {
    const method = buf.readUInt16LE(p + 10), size = buf.readUInt32LE(p + 20), nl = buf.readUInt16LE(p + 28), xl = buf.readUInt16LE(p + 30), cl = buf.readUInt16LE(p + 32);
    const off = buf.readUInt32LE(p + 42), name = buf.toString('utf8', p + 46, p + 46 + nl);
    const start = off + 30 + buf.readUInt16LE(off + 26) + buf.readUInt16LE(off + 28), raw = buf.subarray(start, start + size);
    out.set(name, method === 8 ? zlib.inflateRawSync(raw) : raw);
    p += 46 + nl + xl + cl;
  }
  return out;
}

async function open(page: Page, name: string, bytes: Buffer) {
  const opened = await count(page, 'wordcraft:opened');
  await page.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: n, bytes: buf }, [buf]);
  }, [name, Array.from(bytes)] as const);
  await waitCount(page, 'wordcraft:opened', opened + 1);
}

/** Run a registered command by id; a plugin command registers on a frame after `plugins`, so retry until it is accepted. */
async function run(page: Page, id: string) {
  await expect.poll(async () => {
    const n = await count(page, 'wordcraft:ran');
    await send(page, { type: 'wordcraft:run', id });
    await waitCount(page, 'wordcraft:ran', n + 1);
    return page.evaluate(() => (window as any).__msgs.filter((m: any) => m.type === 'wordcraft:ran').at(-1).ok);
  }, { timeout: 60_000 }).toBe(true);
}

async function save(page: Page): Promise<Buffer> {
  await page.evaluate(() => { (window as any).__saved = null; });
  await send(page, { type: 'wordcraft:requestSave' });
  await expect.poll(() => page.evaluate(() => (window as any).__saved !== null), { timeout: 60_000 }).toBe(true);
  return Buffer.from(await page.evaluate(() => Array.from(new Uint8Array((window as any).__saved))));
}

const docXml = (b: Buffer) => parts(b).get('word/document.xml')!.toString('utf8');
const panel = (page: Page) => page.locator('.plg-panel');
const insertBtn = (page: Page, label: string) => panel(page).getByRole('button', { name: new RegExp(`Insert: ${label}$`) });

test('insert, highlight and check through the real page host', async ({ page }) => {
  test.setTimeout(10 * 60_000);
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitCount(page, 'wordcraft:ready', 1);
  // a blank document: the control is the whole paragraph, so Select All selects exactly it
  await run(page, 'file.new');

  // the panel: three fields, a filter box, a Check button
  await run(page, 'plugin.template-fields.open');
  await expect(panel(page)).toBeVisible();
  await expect(panel(page).locator('li button')).toHaveText(['Insert: Sample text', 'Insert: Sample date', 'Insert: Sample mark']);
  await panel(page).locator('input[type=text]').fill('MARK');
  await panel(page).locator('input[type=text]').blur();
  await expect(panel(page).locator('li button')).toHaveText(['Insert: Sample mark']);
  await panel(page).locator('input[type=text]').fill('');
  await panel(page).locator('input[type=text]').blur();
  await expect(panel(page).locator('li button')).toHaveCount(3);

  // Insert: w:sdt with w:tag = the name, the label as placeholder, delete-locked
  const results = await count(page, 'wordcraft:pluginResults');
  await insertBtn(page, 'Sample date').click();
  await waitCount(page, 'wordcraft:pluginResults', results + 1);
  await expect.poll(async () => docXml(await save(page)), { timeout: 30_000 }).toContain('<w:tag w:val="field_b"/>');
  const first = await save(page);
  const xml = docXml(first);
  expect(xml).toContain('<w:sdt>');
  expect(xml).toContain('Sample date');
  expect(xml).toContain('<w:lock w:val="sdtLocked"/>');

  // highlight: select the control (a collapsed caret carries no text, so an event cannot name its field); the open panel marks that field
  await run(page, 'select.all');
  await page.mouse.move(300, 300);   // egui repaints on input: the 200 ms debounce of the event needs a frame
  await page.mouse.move(320, 320);
  await expect(panel(page).getByText('Caret is in: Sample date')).toBeVisible({ timeout: 15_000 });
  await expect(insertBtn(page, 'Sample date')).toContainText('\u25BA');

  // Check fields: field_a and mark_a are missing, nothing is unknown
  await panel(page).getByRole('button', { name: 'Check fields' }).click();
  await expect(panel(page).getByText('Missing from the document: field_a, mark_a')).toBeVisible({ timeout: 15_000 });
  await expect(panel(page).getByText(/Not in the field list/)).toHaveCount(0);

  // the round trip: open the saved bytes and save again, the control is still there
  await open(page, 'saved.docx', first);
  const again = docXml(await save(page));
  expect(again).toContain('<w:tag w:val="field_b"/>');
  expect(again).toContain('<w:sdt>');
  console.log('WEB FIELDS: insert, highlight, check and round trip OK');
});
