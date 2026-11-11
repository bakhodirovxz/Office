import { test, expect, type Page } from '@playwright/test';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import { zipEntry } from './docx-xml';

// The plugin bridge inside the Sheets and Slides frames (PLAN phase 7 WP13, docs/plugins.md section 6a and 9): the REAL page host
// (`web/plugins/host.js`, a Worker, the fixture plugin `ci/plugins/webhost-test` and the real uz-translit module) against the built
// sheets-web and slides-web dists, through the editors' own `<prefix>:plugin*` bridge. The editors have no `run` message, so a command
// is started the way a user starts it: the toolbar's Plugins menu, three clicks on the canvas (button, plugin, command). The click
// positions are measured for the 1100 x 700 frame at device scale 1 (see MENU below) and every start is checked by what the page host
// received, never by a pixel. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-plugins-apps.config.ts
// Env: SHEETS_DIST, SLIDES_DIST (default E:/office-target/p7-left/dist/<app>-web), UZ_TRANSLIT_PLUGIN_WASM (cargo xt plugin build uz-translit).
const ROOT = path.resolve(__dirname, '../..');
const FIX = path.join(ROOT, 'ci/plugins/webhost-test');
const TRANSLIT = path.join(ROOT, 'plugins/uz-translit');
const WASM = process.env.UZ_TRANSLIT_PLUGIN_WASM ?? 'E:/office-target/p7-left/plugins/wasm32-unknown-unknown/plugin/uz_translit_plugin.wasm';
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.json': 'application/json', '.css': 'text/css' };

// canvas pixels: `more` (Sheets only: the toolbar's overflow button), the Plugins button or entry, the first plugin's row, the commands' x
type Menu = { more?: [number, number]; plugins: [number, number]; plugin: [number, number]; command: number };
const APPS: readonly {
  id: string; prefix: string; dist: string; file: string; menu: Menu; clickAt: { x: number; y: number }; entries: string[]; converted: RegExp;
}[] = [
  { id: 'sheets', prefix: 'gridcraft', dist: path.resolve(process.env.SHEETS_DIST ?? 'E:/office-target/p7-left/dist/sheets-web'), file: path.join(ROOT, 'domains/sheets/tests/translit-fixtures/xlsx/01-styles.xlsx'),
    menu: { more: [1008, 62], plugins: [817, 198], plugin: [704, 198], command: 830 }, clickAt: { x: 400, y: 420 }, entries: ['xl/sharedStrings.xml'], converted: /Тошкент/ },
  { id: 'slides', prefix: 'deckcraft', dist: path.resolve(process.env.SLIDES_DIST ?? 'E:/office-target/p7-left/dist/slides-web'), file: path.join(ROOT, 'domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx'),
    menu: { plugins: [934, 58], plugin: [922, 90], command: 830 }, clickAt: { x: 900, y: 400 }, entries: ['ppt/slides/slide1.xml'], converted: /[\u0400-\u04FF]/ },
];

const fixWasm = fs.readFileSync(path.join(FIX, 'plugin.wasm'));
const COMMANDS = ['hello', 'deny', 'many', 'bad', 'net', 'spin', 'grow', 'xss', 'overflow'];
const manifest = (app: string) => ({
  id: 'plg-test', version: '1.0.0', api: 1, apps: [app], events: [], publisher: 'Test publisher', name: { en: 'Test plugin' },
  permissions: { document: 'write', memory_mb: 2, network: ['https://example.org'] },
  command: COMMANDS.map((id) => ({ id, label: { en: `Cmd ${id}` }, reads: ['none'], params: {} })),
  panel: [{ id: 'p', title: { en: 'Panel' } }],
});
const descriptor = (app: string) => ({ id: 'plg-test', version: '1.0.0', sha256: crypto.createHash('sha256').update(fixWasm).digest('hex'), wasm: '/test-plugins/plugin.wasm', grant: ['https://example.org'], manifest: manifest(app) });

const HOST = (app: string, prefix: string, translit: string) => `<!doctype html><meta charset="utf-8"><body>
<div id="root" style="position:relative"><iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe></div>
<script src="/plugins/host.js"></script>
<script>
  window.__msgs = []; window.__saved = null; window.__say = []; window.__calls = [];
  const ed = () => document.getElementById('ed').contentWindow;
  window.__toEditor = (msg, transfer) => ed().postMessage(msg, location.origin, transfer);
  const H = PluginHost.make({ cfg: { plugins: [${JSON.stringify(descriptor(app))}, ${translit}] }, lang: 'en', frames: {} }, (m) => window.__say.push(m), document.getElementById('root'));
  window.__host = H;
  addEventListener('message', (e) => {
    if (e.source !== ed()) return;
    const d = e.data || {};
    if (String(d.type).startsWith('${prefix}:')) H.onMessage({ app: { id: '${app}', prefix: '${prefix}' }, el: { contentWindow: ed() } }, d);
    if (d.type === '${prefix}:save') window.__saved = d.bytes;
    if (d.type === '${prefix}:pluginCall') window.__calls.push({ call: d.call, command: d.command, reads: Object.fromEntries(Object.entries(d.reads || {}).map(([k, v]) => [k, v.byteLength])) });
    window.__msgs.push({ type: d.type, error: d.error, call: d.call, ids: d.ids, errors: d.errors, results: d.results, name: d.name, dirty: d.dirty });
  });
</script>`;

const msgs = (page: Page) => page.evaluate(() => (window as any).__msgs as any[]);
const count = async (page: Page, type: string) => (await msgs(page)).filter((m) => m.type === type).length;
const waitFor = (page: Page, type: string, n = 1, timeout = 120_000) => expect.poll(() => count(page, type), { timeout }).toBeGreaterThanOrEqual(n);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const say = (page: Page) => page.evaluate(() => (window as any).__say as string[]);
const calls = (page: Page) => page.evaluate(() => (window as any).__calls as any[]);
const audit = (page: Page) => page.evaluate(() => (window as any).__host.audit as { plugin: string; kind: string; error?: string }[]);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

for (const app of APPS) {
  test.describe(`${app.id} frame`, () => {
    let server: http.Server;
    let base: string;
    const P = app.prefix;

    test.beforeAll(async () => {
      const translit = JSON.stringify((await import(('file:///' + path.join(ROOT, 'scripts/plugins/manifest-json.mjs').replace(/\\/g, '/')))).descriptor(TRANSLIT, WASM, '/test-plugins/translit.wasm'));
      server = http.createServer((req, res) => {
        const url = new URL(req.url!, 'http://x');
        if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST(app.id, P, translit));
        let file: string;
        if (url.pathname.startsWith('/plugins/')) file = path.join(ROOT, 'web', url.pathname);
        else if (url.pathname === '/test-plugins/translit.wasm') file = WASM;
        else if (url.pathname.startsWith('/test-plugins/')) file = path.join(FIX, path.basename(url.pathname));
        else file = path.join(app.dist, url.pathname === '/' ? 'index.html' : url.pathname);
        if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) return void res.writeHead(404).end();
        res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' }).end(fs.readFileSync(file));
      });
      const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
      await new Promise<void>((ok) => server.listen(port, '127.0.0.1', ok));
      base = `http://127.0.0.1:${port}`;
    });
    test.afterAll(() => { server?.close(); });

    async function openFile(page: Page) {
      const n = await count(page, `${P}:opened`);
      await page.evaluate(([nm, b, pre]) => {
        const buf = new Uint8Array(b as number[]).buffer;
        (window as any).__toEditor({ type: `${pre}:open`, name: nm, bytes: buf }, [buf]);
      }, [path.basename(app.file), Array.from(fs.readFileSync(app.file)), P] as const);
      await waitFor(page, `${P}:opened`, n + 1);
    }

    test.beforeEach(async ({ page }) => {
      page.on('pageerror', (e) => { throw e; });
      await page.goto(base + '/host.html');
      await waitFor(page, `${P}:ready`);
      await openFile(page);
    });

    const said = async (page: Page, re: RegExp) => (await say(page)).some((m) => re.test(m));
    /** The Plugins menu in the toolbar's overflow: button, Plugins, the plugin, the command (positions in canvas pixels). */
    async function menuClick(page: Page, plugin: number, index: number) {
      const c = canvas(page), at = (x: number, y: number) => c.click({ position: { x, y } });
      if (app.menu.more) await at(...app.menu.more);
      await at(...app.menu.plugins);
      await at(app.menu.plugin[0], app.menu.plugin[1] + 26 * plugin);
      await at(app.menu.command, app.menu.plugin[1] + 26 * (plugin + index));
    }
    /** Start a command like a user (until the page host was asked for it: a menu click can miss while the frame is busy). */
    async function runCmd(page: Page, plugin: number, index: number, cmd: string) {
      const n = (await calls(page)).filter((c) => c.command === cmd).length;
      await expect.poll(async () => {
        if ((await calls(page)).filter((c) => c.command === cmd).length > n) return true;
        await menuClick(page, plugin, index);
        await page.waitForTimeout(800);
        return (await calls(page)).filter((c) => c.command === cmd).length > n;
      }, { timeout: 90_000, message: cmd }).toBe(true);
    }
    const runTest = (page: Page, cmd: string) => runCmd(page, 0, COMMANDS.indexOf(cmd), cmd);
    const runTranslit = (page: Page, which: 0 | 1 | 2) => runCmd(page, 1, which, ['toCyrillic', 'toLatin', 'auto'][which]);

    /** The entries a probe compares: the document's own text parts, saved through the bridge. */
    async function probe(page: Page): Promise<string> {
      await page.evaluate(() => { (window as any).__saved = null; });
      await send(page, { type: `${P}:requestSave` });
      await expect.poll(() => page.evaluate(() => (window as any).__saved !== null), { timeout: 60_000 }).toBe(true);
      const bytes = Buffer.from(await page.evaluate(() => Array.from(new Uint8Array((window as any).__saved))));
      return app.entries.map((e) => zipEntry(bytes, e)).join('\n');
    }
    // the invocation has ended when the page host has no pending call
    const settled = (page: Page, cmd: string) => expect.poll(() => page.evaluate(() => (window as any).__host.plugins.every((p: any) => !p.pending)), { timeout: 60_000, message: `${cmd} ended` }).toBe(true);

    test('1 the editor registers both plugins from the page host', async ({ page }) => {
      const set = (await msgs(page)).find((m) => m.type === `${P}:pluginsSet`);
      expect(set.ids).toEqual(['plg-test', 'uz-translit']);
      expect(set.errors).toEqual([]);
    });

    test('2 a command that only uses the page (panel, status) leaves the document alone', async ({ page }) => {
      const before = await probe(page);
      await runTest(page, 'hello');
      await expect.poll(() => said(page, /^hello done$/), { timeout: 60_000 }).toBe(true);
      await expect(page.getByRole('region', { name: 'Test plugin: Panel' })).toBeVisible();
      await expect.poll(() => count(page, `${P}:pluginResults`)).toBeGreaterThanOrEqual(1);
      expect((await msgs(page)).find((m) => m.type === `${P}:pluginResults`).error).toBeUndefined();
      expect(await probe(page)).toBe(before);
    });

    test('M5, R1, M8: a refused command, a 17-read round and a malformed answer end the invocation and leave the document as it was', async ({ page }) => {
      const before = await probe(page);
      for (const [cmd, why] of [['deny', 'refused'], ['many', 'bad next'], ['bad', 'encoded data']] as const) {
        await runTest(page, cmd);
        await expect.poll(() => said(page, new RegExp(`could not finish.*${why}`)), { timeout: 60_000, message: cmd }).toBe(true);
        await settled(page, cmd);
        expect(await probe(page), cmd).toBe(before);
      }
      expect((await audit(page)).filter((x) => x.kind === 'failed').length).toBeGreaterThanOrEqual(3);
    });

    test('M6, M7, S1, M9: a loop is stopped at the budget, memory growth is capped, deep recursion traps, markup and a 10,000-character text are shown as text and cut; the lock is gone after each', async ({ page }) => {
      const before = await probe(page);
      await runTest(page, 'grow');
      await expect.poll(() => said(page, /^hog capped$/), { timeout: 60_000 }).toBe(true);
      await runTest(page, 'overflow');
      await expect.poll(async () => (await audit(page)).some((x) => x.kind === 'failed' && /stack|recursion/i.test(x.error ?? '')), { timeout: 60_000 }).toBe(true);
      await settled(page, 'overflow');
      await runTest(page, 'spin');
      await expect.poll(async () => (await audit(page)).some((x) => x.kind === 'stopped'), { timeout: 60_000 }).toBe(true);
      await settled(page, 'spin');
      expect(await probe(page)).toBe(before);
      // the editor takes input again: the real plugin changes the document after all of the above (a stuck lock would refuse it)
      await runTranslit(page, 0);
      await expect.poll(async () => (await probe(page)) !== before, { timeout: 60_000 }).toBe(true);
      // last: the panel it opens covers the toolbar menu
      await runTest(page, 'xss');
      const panel = page.getByRole('region', { name: 'Test plugin: Panel' });
      await expect(panel).toBeVisible({ timeout: 60_000 });
      await expect(panel.locator('h3')).toHaveText('<img src=x onerror=window.__xss=1>');
      expect(await panel.locator('img, script, b').count()).toBe(0);
      expect((await panel.locator('.plg-text').textContent())!.length).toBe(1024);
      expect(await page.evaluate(() => (window as any).__xss)).toBeUndefined();
      await settled(page, 'xss');
    });

    test('M4: a granted origin that redirects to another origin is not followed, the document is unchanged', async ({ page }) => {
      const before = await probe(page);
      const hits: string[] = [];
      await page.route('https://example.org/x', (r) => { hits.push('A'); return r.fulfill({ status: 302, headers: { location: 'https://other.example/y' } }); });
      await page.route('https://other.example/**', (r) => { hits.push('B'); return r.fulfill({ status: 200, body: 'leak' }); });
      await runTest(page, 'net');
      await expect.poll(async () => (await page.evaluate(() => (window as any).__host.logs.map((l: any) => l.text)) as string[]).some((l) => l.includes('"results":[{"error":"a redirect was refused"}]')), { timeout: 60_000 }).toBe(true);
      expect(hits).toEqual(['A']);
      expect(await audit(page)).toEqual([{ plugin: 'plg-test', kind: 'network', origin: 'https://example.org' }]);
      expect(await probe(page)).toBe(before);
    });

    test('3 the real uz-translit module converts through the text map; the result is one undo step', async ({ page }) => {
      const before = await probe(page);
      await runTranslit(page, 0);
      await expect.poll(async () => app.converted.test(await probe(page)), { timeout: 90_000 }).toBe(true);
      expect((await probe(page))).not.toBe(before);
      expect((await calls(page)).find((c) => c.command === 'toCyrillic')!.reads.textmap).toBeGreaterThan(16);   // a transferred ArrayBuffer, not JSON
      await canvas(page).click({ position: app.clickAt });
      await page.keyboard.press('Control+z');
      await expect.poll(async () => (await probe(page)) === before, { timeout: 30_000 }).toBe(true);   // one undo restores the document
    });
  });
}
