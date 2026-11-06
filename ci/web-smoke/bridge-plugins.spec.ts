import { test, expect, type Page } from '@playwright/test';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import { documentXml } from './docx-xml';

// The Docs plugin bridge, PLAN phase 7 WP4 (docs/plugins.md sections 9 and 9a), against the built docs-web dist and a small same-origin
// host page. Two kinds of host: the REAL page host (`web/plugins/host.js` with the fixture plugin `ci/plugins/webhost-test`, a Worker
// and all) for the end-to-end cases, and a STUB (a few lines of JS that answer the rounds themselves, no wasm) for the cases the real
// host cannot produce on demand: a silent page, a held round, a lock that does not hold. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-plugins.config.ts
// Env: DOCS_DIST (default E:/office-target/p1/dist/docs-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p1/dist/docs-web');
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const FIX = path.join(ROOT, 'ci/plugins/webhost-test');
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.json': 'application/json', '.css': 'text/css' };

const wasm = fs.readFileSync(path.join(FIX, 'plugin.wasm'));
const MANIFEST = {
  id: 'plg-test', version: '1.0.0', api: 1, apps: ['docs'], events: ['selectionChanged'], publisher: 'Test publisher', name: { en: 'Test plugin' },
  permissions: { document: 'write', memory_mb: 2, network: ['https://example.org'] },
  command: [
    { id: 'hello', label: { en: 'Cmd hello' }, reads: ['none'], params: {} },
    { id: 'edit', label: { en: 'Cmd edit' }, reads: ['textmap'], params: {} },
    { id: 'deny', label: { en: 'Cmd deny' }, reads: ['none'], params: {} },
    { id: 'many', label: { en: 'Cmd many' }, reads: ['none'], params: {} },
    { id: 'bad', label: { en: 'Cmd bad' }, reads: ['none'], params: {} },
    { id: 'net', label: { en: 'Cmd net' }, reads: ['none'], params: {} },
    { id: 'spin', label: { en: 'Cmd spin' }, reads: ['none'], params: {} },
  ],
  panel: [{ id: 'p', title: { en: 'Panel' } }],
};
const DESCRIPTOR = { id: 'plg-test', version: '1.0.0', sha256: crypto.createHash('sha256').update(wasm).digest('hex'), wasm: '/test-plugins/plugin.wasm', grant: ['https://example.org'], manifest: MANIFEST };

// The host page. ?real=1 runs the real page host; otherwise a stub answers by `window.__mode`:
//   insert (one round), rounds (a text map read, then an insert), more (an insert, then silence: the test aborts), silent, hold (until __release()).
const HOST = (real: boolean, two = false) => `<!doctype html><meta charset="utf-8"><body>
<div id="root" style="position:relative"><iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe></div>
${real ? '<script src="/plugins/host.js"></script>' : ''}
<script>
  window.__msgs = []; window.__saved = null; window.__mode = 'insert'; window.__timeout = undefined; window.__held = null; window.__say = []; window.__calls = [];
  const ed = () => document.getElementById('ed').contentWindow;
  const toEditor = (msg, transfer) => ed().postMessage(msg, location.origin, transfer);
  window.__toEditor = toEditor;
  const insert = (t) => ({ type: 'run', id: 'text.insert', params: { text: t } });
  const actions = (d, actions, extra) => toEditor({ type: 'wordcraft:pluginActions', call: d.call, plugin: d.plugin, actions, blobs: [], reads: [], more: false, ...extra });
  ${real
    ? `const H = PluginHost.make({ cfg: { plugins: [${JSON.stringify(DESCRIPTOR)}${two ? ',' + JSON.stringify({ ...DESCRIPTOR, id: 'plg-two', manifest: { ...MANIFEST, id: 'plg-two', name: { en: 'Second plugin' }, command: ['hello', 'link', 'chr', 'incl', 'tracked', 'vocab', 'accept', 'wall', 'overflow', 'ping', 'pong', 'fetches'].map((id) => ({ id, label: { en: `Cmd ${id}` }, reads: ['none'], params: {} })) } }) : ''}] }, lang: 'en', frames: {} }, (m) => window.__say.push(m), document.getElementById('root'));
       window.__host = H;
       addEventListener('message', (e) => { if (e.source === ed() && String((e.data || {}).type).startsWith('wordcraft:')) H.onMessage({ app: { id: 'docs', prefix: 'wordcraft' }, el: { contentWindow: ed() } }, e.data); });`
    : `window.__register = (manifests) => toEditor({ type: 'wordcraft:plugins', commands: [], toolbar: [], menu: [], plugins: manifests.map((m) => ({ manifest: m })), timeoutMs: window.__timeout });`}
  addEventListener('message', (e) => {
    if (e.source !== ed()) return;
    const d = e.data || {};
    if (d.type === 'wordcraft:save') window.__saved = d.bytes;
    if (d.type === 'wordcraft:pluginCall') window.__calls.push({ call: d.call, command: d.command, reads: Object.fromEntries(Object.entries(d.reads || {}).map(([k, v]) => [k, v.byteLength])) });
    if (d.type === 'wordcraft:pluginCall' && !${real}) {
      const m = window.__mode;
      if (m === 'insert') actions(d, [insert('PLG')]);
      else if (m === 'rounds') toEditor({ type: 'wordcraft:pluginActions', call: d.call, plugin: d.plugin, actions: [], blobs: [], reads: [{ what: 'textmap' }], more: true });
      else if (m === 'more') actions(d, [insert('PLG')], { more: true });
      else if (m === 'hold') window.__held = d;
    }
    if (d.type === 'wordcraft:pluginResults' && !${real} && window.__mode === 'rounds' && !d.second) { window.__mode = 'rounds2'; window.__reads2 = (d.reads || []).map((b) => b.byteLength); actions(d, [insert('TWO')]); }
    window.__msgs.push({ type: d.type, error: d.error, call: d.call, ids: d.ids, errors: d.errors, results: d.results, event: d.event, ok: d.ok });
  });
  window.__release = () => actions(window.__held, [insert('PLG')]);
</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST(url.searchParams.has('real'), url.searchParams.has('two')));
    let file: string;
    if (url.pathname.startsWith('/plugins/')) file = path.join(ROOT, 'web', url.pathname);
    else if (url.pathname.startsWith('/test-plugins/')) file = path.join(FIX, path.basename(url.pathname));
    else file = path.join(DIST, url.pathname === '/' ? 'index.html' : url.pathname);
    if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) return void res.writeHead(404).end();
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
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const mode = (page: Page, m: string) => page.evaluate((x) => { (window as any).__mode = x; }, m);
const calls = (page: Page) => page.evaluate(() => (window as any).__calls as any[]);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');
const run = (page: Page, id: string) => send(page, { type: 'wordcraft:run', id });
const textOf = (x: string) => x.replace(/<[^>]+>/g, '');
const xs = (x: string) => (textOf(x).match(/x/g) ?? []).length;

/** Save the document and return its word/document.xml. */
async function xml(page: Page): Promise<string> {
  await page.evaluate(() => { (window as any).__saved = null; });
  await send(page, { type: 'wordcraft:requestSave' });
  await expect.poll(() => page.evaluate(() => (window as any).__saved !== null), { timeout: 30_000 }).toBe(true);
  const bytes: number[] = await page.evaluate(() => Array.from(new Uint8Array((window as any).__saved)));
  return documentXml(Buffer.from(bytes));
}

async function openDoc(page: Page, file = DOCX) {
  const opened = await count(page, 'wordcraft:opened');
  await page.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: n, bytes: buf }, [buf]);
  }, [path.basename(file), Array.from(fs.readFileSync(file))] as const);
  await waitFor(page, 'wordcraft:opened', opened + 1);
}

/** The stub registers the plugin itself (the real host does it when the editor says ready). */
async function stubReady(page: Page, timeout?: number) {
  await page.evaluate((t) => { (window as any).__timeout = t; }, timeout);
  await openDoc(page);
  await page.evaluate((m) => (window as any).__register([m]), MANIFEST);
  await waitFor(page, 'wordcraft:pluginsSet');
}

async function start(page: Page, real: boolean, two = false) {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html' + (real ? '?real=1' + (two ? '&two=1' : '') : ''));
  await waitFor(page, 'wordcraft:ready');
}

test.describe('real page host (host.js, a Worker, the fixture plugin)', () => {
  test.beforeEach(async ({ page }) => { await start(page, true); await openDoc(page); });

  test('1 the editor registers the slot commands from the page', async ({ page }) => {
    // the slot is filled on a frame after `plugins`; running the command is the proof it is registered
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(1);
    await expect.poll(async () => { await run(page, 'plugin.plg-test.hello'); await page.waitForTimeout(400); return (await last(page, 'wordcraft:ran') as any)?.ok; }, { timeout: 30_000 }).toBe(true);
    await expect.poll(() => count(page, 'wordcraft:pluginResults'), { timeout: 60_000 }).toBe(1);
    expect((await last(page, 'wordcraft:pluginResults')).error).toBeUndefined();
  });

  test('2 a command with an editor action runs over two rounds, is one change, and the lock is gone', async ({ page }) => {
    await canvas(page).click({ position: { x: 400, y: 160 } });
    const before = xs(await xml(page));
    await run(page, 'plugin.plg-test.edit');
    // round 1 (run text.insert) and the resume (a status) end with a closing pluginResults each
    await expect.poll(() => count(page, 'wordcraft:pluginResults'), { timeout: 60_000 }).toBeGreaterThanOrEqual(2);
    await expect.poll(() => page.evaluate(() => (window as any).__say)).toContain('resumed');
    const logged: string[] = await page.evaluate(() => (window as any).__host.logs.map((l: any) => l.text));
    expect(logged.some((t) => t.includes('"kind":"resume"'))).toBe(true);
    expect((await calls(page))[0]).toMatchObject({ command: 'edit' });
    expect((await calls(page))[0].reads.textmap).toBeGreaterThan(16); // a transferred ArrayBuffer, not JSON
    await page.keyboard.type('RR');
    const text = textOf(await xml(page));
    expect((text.match(/x/g) ?? []).length).toBe(before + 1);
    expect(text).toContain('RR'); // the input lock is released
    expect(text).not.toContain('readOnly');
  });

  test('3 a command that only uses the page (panel, status) leaves the document and the lock alone', async ({ page }) => {
    const before = await xml(page);
    await run(page, 'plugin.plg-test.hello');
    await expect.poll(() => page.evaluate(() => (window as any).__say), { timeout: 60_000 }).toContain('hello done');
    await expect(page.getByRole('region', { name: 'Test plugin: Panel' })).toBeVisible();
    await expect.poll(() => count(page, 'wordcraft:pluginResults')).toBe(1);
    expect(textOf(await xml(page))).toBe(textOf(before));
  });

  test('9 M5 and the reads cap: a refused command and a 17-read round end the invocation and leave the document as it was', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(1);
    const before = textOf(await xml(page));
    for (const [cmd, why] of [['deny', 'refused'], ['many', 'bad next'], ['bad', 'encoded data']] as const) {
      await expect.poll(async () => { await run(page, `plugin.plg-test.${cmd}`); await page.waitForTimeout(500); return (await page.evaluate(() => (window as any).__say) as string[]).some((m) => m.includes('could not finish') && m.includes(why)); }, { timeout: 60_000 }).toBe(true);
      expect(textOf(await xml(page))).toBe(before);
    }
  });

  test('M4 redirect: a granted origin that redirects to another origin is not followed, and the document is unchanged', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(1);
    const before = textOf(await xml(page));
    const hits: string[] = [];
    await page.route('https://example.org/x', (r) => { hits.push('A'); return r.fulfill({ status: 302, headers: { location: 'https://other.example/y' } }); });
    await page.route('https://other.example/**', (r) => { hits.push('B'); return r.fulfill({ status: 200, body: 'leak' }); });
    await run(page, 'plugin.plg-test.net');
    await expect.poll(async () => (await page.evaluate(() => (window as any).__host.logs.map((l: any) => l.text)) as string[]).some((l) => l.includes('"results":[{"error":"a redirect was refused"}]')), { timeout: 60_000 }).toBe(true);
    expect(hits).toEqual(['A']);
    expect(await page.evaluate(() => (window as any).__host.audit)).toEqual([{ plugin: 'plg-test', kind: 'network', origin: 'https://example.org' }]);   // the first hop is audited, B never appears
    expect(textOf(await xml(page))).toBe(before);
  });
});

// Phase 7 re-check: the rows that had no run in a browser with the real editor. The extra fixture commands live in a second plugin (a manifest has at most 16 commands).
test.describe('real page host, second plugin with the extra commands', () => {
  test.beforeEach(async ({ page }) => { await start(page, true, true); await openDoc(page); });

  const said = (page: Page) => page.evaluate(() => (window as any).__say as string[]);
  const auditOf = (page: Page) => page.evaluate(() => (window as any).__host.audit as { plugin: string; kind: string; error?: string }[]);
  // after a refused command the editor still holds the previous invocation's lock for a moment: send it again until it was the plugin's own answer
  const refused = (page: Page, cmd: string, why: RegExp) => expect.poll(async () => { await run(page, `plugin.plg-two.${cmd}`); await page.waitForTimeout(600); return (await said(page)).some((m) => why.test(m)); }, { timeout: 60_000, message: cmd }).toBe(true);

  test('F2 and F7a: links, field codes, forged revisions, the dictionary, accept-all by loop and restrict are refused by the allowlist; the document is as it was', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    const before = textOf(await xml(page));
    for (const cmd of ['link', 'chr', 'incl', 'tracked', 'vocab', 'accept', 'wall']) {
      await refused(page, cmd, /could not finish/);
      expect(textOf(await xml(page)), cmd).toBe(before);
    }
    const a = await auditOf(page);
    expect(a.filter((x) => x.kind === 'failed').length).toBeGreaterThanOrEqual(7);
    expect(a.some((x) => x.kind !== 'failed')).toBe(false);
  });

  test('M10 in the real editor: an event answer that tries a document edit is refused, nothing reaches the document', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    await expect.poll(async () => { await run(page, 'plugin.plg-two.hello'); await page.waitForTimeout(500); return (await said(page)).includes('hello done'); }, { timeout: 60_000 }).toBe(true);
    const before = textOf(await xml(page));
    await canvas(page).click({ position: { x: 400, y: 160 } });
    await page.keyboard.press('Control+End');   // one input only: the event has to come from the idle page, after the 200 ms of stillness, without another input to draw a frame
    await expect.poll(async () => (await auditOf(page)).some((x) => x.kind === 'failed' && /event may only run ui actions/.test(x.error ?? '')), { timeout: 10_000 }).toBe(true);
    expect(textOf(await xml(page))).toBe(before);
  });

  test('S1: unbounded recursion in the plugin ends the invocation cleanly; the host and the editor go on', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    const before = textOf(await xml(page));
    await refused(page, 'overflow', /could not finish/);
    const a = await auditOf(page);
    expect(a.some((x) => x.kind === 'failed' && /stack|recursion/i.test(x.error ?? '')), JSON.stringify(a)).toBe(true);
    await expect.poll(async () => { await run(page, 'plugin.plg-two.hello'); await page.waitForTimeout(500); return (await said(page)).includes('hello done'); }, { timeout: 60_000 }).toBe(true);
    expect(textOf(await xml(page))).toBe(before);
  });

  test('S2: a chain of rounds is stopped by the 10 s invocation deadline, not the 5 s round budget', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    const before = textOf(await xml(page));
    const resumes = () => page.evaluate(() => (window as any).__host.logs.filter((l: any) => l.text.includes('"kind":"resume"')).length);
    // a round has to be shorter than the 5 s budget and longer than 10 s / 16 rounds, whatever the PC: ping does a little work per round, pong more; the first that fits counts
    const tried: string[] = [];
    let fits = false;
    for (const cmd of ['ping', 'pong']) {
      const n = (await auditOf(page)).length, r0 = await resumes(), t0 = Date.now();
      for (let i = 0; i < 25 && (await auditOf(page)).length === n; i++) { await run(page, `plugin.plg-two.${cmd}`); await page.waitForTimeout(1500); }
      await expect.poll(async () => (await auditOf(page)).length, { timeout: 60_000 }).toBeGreaterThan(n);
      const end = (await auditOf(page)).at(-1)!, took = Date.now() - t0, rounds = (await resumes()) - r0;
      tried.push(`${cmd}: ${end.kind} ${end.error ?? ''} after ${took} ms, ${rounds} resume rounds`);
      if (end.kind === 'stopped' && rounds >= 2 && took > 9000) { fits = true; break; }
      await page.waitForTimeout(2500);
    }
    console.log('S2 deadline: ' + tried.join(' | '));
    test.info().annotations.push({ type: 'S2', description: tried.join(' | ') });
    expect(fits, tried.join(' | ')).toBe(true);
    expect(textOf(await xml(page))).toBe(before);
  });

  test('S2: the 9th fetch of one invocation is refused (8 are allowed), each is audited', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    let hits = 0;
    await page.route('https://example.org/x', (r) => { hits++; return r.fulfill({ status: 200, body: 'ok' }); });
    // the editor may still hold the previous lock for a moment: send again only until the invocation has started (it audits its first fetch)
    for (let i = 0; i < 25 && (await auditOf(page)).length === 0; i++) { await run(page, 'plugin.plg-two.fetches'); await page.waitForTimeout(1500); }
    await expect.poll(async () => (await auditOf(page)).some((x) => x.kind === 'failed' && /more than 8 fetches/.test(x.error ?? '')), { timeout: 60_000 }).toBe(true);
    expect(hits).toBe(8);
    expect((await auditOf(page)).filter((x) => x.kind === 'network').length).toBe(8);
  });

  test('F7b: a document with its own protection is not started on (comments: the engine gate; tracked changes: the bridge)', async ({ page }) => {
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    for (const [file, mode, why] of [['protected.docx', 'comments', /only comments are allowed/], ['protected-tracked.docx', 'trackedChanges', /document is protected/]] as const) {
      await openDoc(page, path.join(ROOT, 'scripts/mobile/fixtures', file));
      const ran0 = (await msgs(page)).filter((m) => m.type === 'wordcraft:ran').length;
      await expect.poll(async () => { await run(page, 'plugin.plg-two.hello'); await page.waitForTimeout(700); const r = await last(page, 'wordcraft:ran') as any; return r?.ok === false && why.test(r.error); }, { timeout: 60_000, message: file }).toBe(true);
      expect((await msgs(page)).filter((m) => m.type === 'wordcraft:ran').length).toBeGreaterThan(ran0);
      expect((await calls(page)).filter((c) => c.command === 'hello'), 'the page was never asked to run the plugin').toHaveLength(0);
      const x = await xml(page);
      expect(textOf(x)).toContain('Protected fixture text');
      test.info().annotations.push({ type: 'F7b', description: `${mode}: ${(await last(page, 'wordcraft:ran') as any).error}` });
    }
  });
});

test.describe('two plugins in the real page host', () => {
  test('F5: a command of another plugin while one is pending says "another plugin is still running"; the document ends unlocked', async ({ page }) => {
    await start(page, true, true); await openDoc(page);
    await expect.poll(() => page.evaluate(() => (window as any).__host.plugins.length)).toBe(2);
    await expect.poll(async () => { await run(page, 'plugin.plg-test.hello'); await page.waitForTimeout(500); return (await page.evaluate(() => (window as any).__say) as string[]).includes('hello done'); }, { timeout: 60_000 }).toBe(true);
    const before = textOf(await xml(page));
    const n0 = await page.evaluate(() => (window as any).__host.audit.length);
    await run(page, 'plugin.plg-test.spin');
    await expect.poll(async () => (await calls(page)).some((c) => c.command === 'spin'), { timeout: 30_000 }).toBe(true);
    await page.waitForTimeout(300);
    const ran0 = (await msgs(page)).filter((m) => m.type === 'wordcraft:ran').length;
    await run(page, 'plugin.plg-two.hello');
    await expect.poll(async () => (await msgs(page)).filter((m) => m.type === 'wordcraft:ran').length, { timeout: 30_000 }).toBeGreaterThan(ran0);
    const r = await last(page, 'wordcraft:ran') as any;
    expect(r.ok).toBe(false);
    expect(r.error).toContain('another plugin is still running');
    expect(r.error).not.toContain('read only');
    // the first plugin's budget ends it; afterwards the second one runs and the input lock is gone
    await expect.poll(() => page.evaluate((n) => (window as any).__host.audit.slice(n).some((x: any) => x.kind === 'stopped'), n0), { timeout: 60_000 }).toBe(true);
    await expect.poll(async () => { await run(page, 'plugin.plg-two.hello'); await page.waitForTimeout(600); return (await last(page, 'wordcraft:ran') as any)?.ok === true; }, { timeout: 60_000 }).toBe(true);
    await canvas(page).click({ position: { x: 400, y: 160 } });
    await page.keyboard.type('ZZ');
    const text = textOf(await xml(page));
    expect(text).toContain('ZZ');
    expect(text.replace('ZZ', '')).toBe(before);
    expect(await xml(page)).not.toContain('readOnly');
  });
});

test.describe('stub page host', () => {
  test.beforeEach(async ({ page }) => { await start(page, false); });

  test('4 register: a bad manifest is refused, an empty list unregisters', async ({ page }) => {
    await stubReady(page);
    expect((await last(page, 'wordcraft:pluginsSet')).errors).toEqual([]);
    expect((await last(page, 'wordcraft:pluginsSet')).errors).toEqual([]);
    expect((await last(page, 'wordcraft:pluginsSet')).ids).toEqual(['plg-test']);
    await page.evaluate(() => (window as any).__register([{ id: 'x' }]));
    await expect.poll(() => count(page, 'wordcraft:pluginsSet')).toBe(2);
    const bad = await last(page, 'wordcraft:pluginsSet');
    expect(bad.ids).toEqual([]);
    expect(bad.errors).toHaveLength(1);
    await run(page, 'plugin.plg-test.edit');
    await waitFor(page, 'wordcraft:ran');
    expect((await last(page, 'wordcraft:ran') as any).ok).toBe(false);
    expect(await calls(page)).toHaveLength(0);
  });

  test('5 rounds: the text map goes out as a buffer and a requested read comes back as one', async ({ page }) => {
    await stubReady(page);
    await mode(page, 'rounds');
    await run(page, 'plugin.plg-test.edit');
    await expect.poll(() => page.evaluate(() => (window as any).__mode), { timeout: 30_000 }).toBe('rounds2');
    expect((await calls(page))[0].reads.textmap).toBeGreaterThan(16);
    expect(await page.evaluate(() => (window as any).__reads2)).toHaveLength(1);
    expect(await page.evaluate(() => (window as any).__reads2[0])).toBeGreaterThan(16);
    await expect.poll(() => count(page, 'wordcraft:pluginResults'), { timeout: 30_000 }).toBe(2);
    expect(await xml(page)).toContain('TWO');
  });

  test('6 timeout: a silent page restores the document and releases the lock; a late answer is told it ended', async ({ page }) => {
    await stubReady(page, 400);
    await mode(page, 'silent');
    await canvas(page).click({ position: { x: 400, y: 160 } });
    await run(page, 'plugin.plg-test.edit');
    await expect.poll(async () => (await calls(page)).length, { timeout: 30_000 }).toBe(1);
    await page.waitForTimeout(1500);
    await page.keyboard.type('KK');
    const text = await xml(page);
    expect(text).toContain('KK'); // the input lock is released
    expect(text).not.toContain('readOnly');
    const call = (await calls(page))[0].call;
    await send(page, { type: 'wordcraft:pluginActions', call, plugin: 'plg-test', actions: [{ type: 'run', id: 'text.insert', params: { text: 'LATE' } }], blobs: [], reads: [], more: false });
    await waitFor(page, 'wordcraft:pluginResults');
    expect((await last(page, 'wordcraft:pluginResults')).error).toContain('ended');
    expect(await xml(page)).not.toContain('LATE');
  });

  test('7 input lock: typing while the plugin is pending is refused; after the answer it works', async ({ page }) => {
    await stubReady(page);
    await mode(page, 'hold');
    await canvas(page).click({ position: { x: 400, y: 160 } });
    await run(page, 'plugin.plg-test.edit');
    await expect.poll(async () => (await calls(page)).length, { timeout: 30_000 }).toBe(1);
    await page.keyboard.type('QQ');
    await page.waitForTimeout(500);
    await page.evaluate(() => (window as any).__release());
    await waitFor(page, 'wordcraft:pluginResults');
    await page.keyboard.type('RR');
    const text = await xml(page);
    expect(text).toContain('PLG');
    expect(text).toContain('RR');
    expect(text).not.toContain('QQ');
    expect(text).not.toContain('readOnly');
  });

  test('8 abort and stale: the page gives up, or the document moved; either way the document is as it was', async ({ page }) => {
    await stubReady(page);
    await canvas(page).click({ position: { x: 400, y: 160 } });
    await page.keyboard.type('AA');
    const before = textOf(await xml(page));
    // abort after a round that changed the text
    await mode(page, 'more');
    await run(page, 'plugin.plg-test.edit');
    await waitFor(page, 'wordcraft:pluginResults');
    expect(textOf(await xml(page))).toContain('PLG');
    const call = (await calls(page))[0].call;
    await send(page, { type: 'wordcraft:pluginActions', call, plugin: 'plg-test', actions: [], blobs: [], reads: [], more: false, abort: true });
    await expect.poll(async () => textOf(await xml(page)), { timeout: 30_000 }).toBe(before);
    // stale: undo while the plugin is pending moves the revision (if the lock lets it through), the next answer is refused
    await mode(page, 'hold');
    await run(page, 'plugin.plg-test.edit');
    await expect.poll(async () => (await calls(page)).length).toBe(2);
    await run(page, 'edit.undo');
    await page.waitForTimeout(500);
    await page.evaluate(() => (window as any).__release());
    await expect.poll(() => count(page, 'wordcraft:pluginResults'), { timeout: 30_000 }).toBeGreaterThanOrEqual(2);
    const r = await last(page, 'wordcraft:pluginResults');
    if (r.error) expect(r.error).toContain('stale'); // undo got through the lock: the answer is refused (unit test: a_document_that_moved_under_the_plugin_is_stale_and_restored)
    expect(textOf(await xml(page))).not.toContain('PLG' + 'PLG');
  });
});
