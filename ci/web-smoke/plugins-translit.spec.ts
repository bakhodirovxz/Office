import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import * as zlib from 'node:zlib';

// Phase 7 WP6, PLAN 6.3 (output, web): the real plugins/uz-translit module in a Worker of the real page host, through the editor bridge, against
// the built-in command in the same editor build. For every Word-written fixture x {tracked off, on} x {to Cyrillic, back to Latin} the docx saved
// through the bridge must have the same entries with the same bytes, once the wall-clock timestamps are blanked (the runs are seconds apart). Scope is the document (`wordcraft:run` carries no params, and the caret is collapsed: scope auto).
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.plugins-translit.config.ts
// Env: DOCS_DIST (default E:/office-target/p7-translit/dist/docs-web), UZ_TRANSLIT_PLUGIN_WASM (the module from `cargo xt plugin build uz-translit`).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p7-translit/dist/docs-web');
const WASM = process.env.UZ_TRANSLIT_PLUGIN_WASM ?? 'E:/office-target/p7-translit/wasm32-unknown-unknown/plugin/uz_translit_plugin.wasm';
const FIXTURES = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word');
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.json': 'application/json', '.css': 'text/css' };
const NAMES = fs.readdirSync(FIXTURES).filter((f) => f.endsWith('.docx')).sort();

const HOST = (descriptor: string) => `<!doctype html><meta charset="utf-8"><body>
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
  const d = JSON.stringify(descriptor(path.join(ROOT, 'plugins/uz-translit'), WASM, '/test-plugins/plugin.wasm'));
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

/** The two runs are seconds apart and the editor stamps wall-clock times (document properties, revisions): equal means the same entries with
 *  the same content once ISO timestamps are blanked. (Native, where both runs fall in one second, compares whole files.) */
function same(a: Buffer, b: Buffer): boolean {
  const [pa, pb] = [parts(a), parts(b)];
  const norm = (x: Buffer) => x.toString('latin1').replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g, 'T');
  return pa.size === pb.size && [...pa].every(([n, v]) => pb.has(n) && norm(v) === norm(pb.get(n)!));
}

/** Open `bytes`, optionally turn tracking on, run the command, and return the docx saved through the bridge. */
async function convert(page: Page, name: string, bytes: Buffer, id: string, tracked: boolean): Promise<Buffer> {
  const opened = await count(page, 'wordcraft:opened');
  await page.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: n, bytes: buf }, [buf]);
  }, [name, Array.from(bytes)] as const);
  await waitCount(page, 'wordcraft:opened', opened + 1);
  if (tracked) {
    const ran = await count(page, 'wordcraft:ran');
    await send(page, { type: 'wordcraft:run', id: 'review.trackChanges' });
    await waitCount(page, 'wordcraft:ran', ran + 1);
  }
  const ran = await count(page, 'wordcraft:ran');
  const results = await count(page, 'wordcraft:pluginResults');
  // a plugin command registers on a frame after `plugins`; retry the run until it is accepted
  await expect.poll(async () => {
    const n = await count(page, 'wordcraft:ran');
    await send(page, { type: 'wordcraft:run', id });
    await waitCount(page, 'wordcraft:ran', n + 1);
    return page.evaluate(() => (window as any).__msgs.filter((m: any) => m.type === 'wordcraft:ran').at(-1).ok);
  }, { timeout: 60_000 }).toBe(true);
  void ran;
  if (id.startsWith('plugin.')) await waitCount(page, 'wordcraft:pluginResults', results + 1);
  await page.waitForTimeout(300);
  await page.evaluate(() => { (window as any).__saved = null; });
  await send(page, { type: 'wordcraft:requestSave' });
  await expect.poll(() => page.evaluate(() => (window as any).__saved !== null), { timeout: 60_000 }).toBe(true);
  return Buffer.from(await page.evaluate(() => Array.from(new Uint8Array((window as any).__saved))));
}

test('the plugin saves the same docx as the built-in command (both directions, tracked off and on)', async ({ page }) => {
  test.setTimeout(30 * 60_000);
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitCount(page, 'wordcraft:ready', 1);
  const diffs: string[] = [];
  let ok = 0;
  for (const name of NAMES) {
    const input = fs.readFileSync(path.join(FIXTURES, name));
    let cyr: Buffer | null = null;
    for (const tracked of [false, true]) {
      const a = await convert(page, name, input, 'tools.translitCyrl', tracked);
      const b = await convert(page, name, input, 'plugin.uz-translit.toCyrillic', tracked);
      if (same(a, b)) ok++; else diffs.push(`${name} to Cyrillic tracked=${tracked}: ${a.length} vs ${b.length} bytes`);
      if (!tracked) cyr = a;
    }
    for (const tracked of [false, true]) {
      const a = await convert(page, 'cyr.docx', cyr!, 'tools.translitLatn', tracked);
      const b = await convert(page, 'cyr.docx', cyr!, 'plugin.uz-translit.toLatin', tracked);
      if (same(a, b)) ok++; else diffs.push(`${name} back to Latin tracked=${tracked}: ${a.length} vs ${b.length} bytes`);
    }
  }
  // the two synthetic 108-page files, one direction each, tracked off and on
  for (const [name, builtin, plugin] of [['uz108-latn.docx', 'tools.translitCyrl', 'plugin.uz-translit.toCyrillic'], ['uz108-cyrl.docx', 'tools.translitLatn', 'plugin.uz-translit.toLatin']]) {
    const input = fs.readFileSync(path.join(FIXTURES, '..', name));
    for (const tracked of [false, true]) {
      const a = await convert(page, name, input, builtin, tracked);
      const b = await convert(page, name, input, plugin, tracked);
      if (same(a, b)) ok++; else diffs.push(`${name} tracked=${tracked}: ${a.length} vs ${b.length} bytes`);
    }
  }
  console.log(`WEB OUTPUT MATRIX: identical ${ok}, differing ${diffs.length}`);
  for (const d of diffs) console.log(`DIFF ${d}`);
  expect(diffs).toEqual([]);
});
