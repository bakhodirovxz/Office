import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The home shell (PLAN phase 4, WP7) against the real suite-serve binary: the home screen, lazy loading of the editors, the Docs
// round trip through the postMessage bridge, the converter reached from the shell, and the headers per route. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.shell.config.ts
// Env: SUITE_SERVE_BIN, CONVERT_API_FAKE_TOOL (default: the services release build), SHELL_DIST_ROOT (default: the build dists,
// with the assembled shell in <root>/shell: scripts/shell-dist.sh).
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'suite-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const DIST = process.env.SHELL_DIST_ROOT ?? 'E:/office-target/p1/dist';
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const PPTX = path.join(ROOT, 'domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx');

// The product name and the app names come from the repo's own files, never hard-coded here.
const I18N = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/shell/i18n.json'), 'utf8'));
const brandText = fs.readFileSync(path.join(ROOT, 'brand/brand.toml'), 'utf8');
const PRODUCT = /^\[product\][^[]*?\nname = "([^"]+)"/m.exec(brandText)![1];
const appName = (id: string, lang = 'en') =>
  new RegExp(`\\[apps\\.${id}\\][\\s\\S]*?names = \\{[^}]*?"?${lang}"? = "([^"]+)"`).exec(brandText)![1];
const txt = (lang: string, key: string) => I18N[lang][key];
const EDITORS = ['docs', 'sheets', 'slides', 'pdf'];
const have = (d: string) => fs.existsSync(path.join(DIST, `${d}-web`, 'index.html'));

let proc: ChildProcess;
let base: string;
let dir: string;

test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'shell-spec-'));
  const cfg = path.join(dir, 'api.toml');
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  fs.writeFileSync(cfg, `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(BIN, ['--config', cfg, '--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  base = await new Promise<string>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => { out += d; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) ok(m[1]); });
    proc.on('exit', (c) => bad(new Error(`suite-serve exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('suite-serve did not start')), 15000);
  });
});
test.afterAll(() => { proc?.kill(); fs.rmSync(dir, { recursive: true, force: true }); });

/** Records page errors, CSP violations and every request (frames included). */
function watch(page: Page) {
  const w = { problems: [] as string[], urls: [] as string[] };
  page.on('pageerror', (e) => w.problems.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy/i.test(m.text())) w.problems.push(m.text()); });
  page.on('request', (r) => w.urls.push(new URL(r.url()).pathname));
  return w;
}
const wasm = (urls: string[]) => urls.filter((u) => u.endsWith('.wasm'));
// the editor's wasm is instantiated and its loading banner is gone (the canvas element exists from the first byte)
const editorUp = async (page: Page, route: string) => {
  await page.waitForFunction((r) => !!document.querySelector(`iframe[src^="${r}"]`), route);
  const frame = page.frames().find((f) => f.url().includes(route))!;
  await frame.waitForFunction(() => !!(window as any).wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 120_000 });
};

test('the home screen loads with no editor bundle', async ({ page }) => {
  const w = watch(page);
  await page.goto(base + '/');
  await expect(page).toHaveTitle(PRODUCT);
  await expect(page.locator('h1')).toHaveText(PRODUCT);
  const cards = page.locator('#cards li');
  await expect(cards).toHaveCount(4);
  for (const [i, id] of EDITORS.entries()) await expect(cards.nth(i).locator('.name')).toHaveText(appName(id));
  await expect(page.locator('#recent-empty')).toHaveText(txt('en', 'home.recent.empty'));
  await expect(page.locator('#convert')).toHaveText(appName('convert'));
  await page.waitForLoadState('networkidle');
  await expect(page.locator('iframe')).toHaveCount(0);
  expect(wasm(w.urls)).toEqual([]);
  expect(w.urls.filter((u) => EDITORS.some((e) => u.startsWith(`/${e}/`)))).toEqual([]);
  expect(w.problems).toEqual([]);
  // no sideways scroll on a phone
  await page.setViewportSize({ width: 360, height: 740 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('language switch: ru and uz-Cyrl texts, and the choice is kept', async ({ page }) => {
  await page.goto(base + '/');
  for (const lang of ['ru', 'uz-Cyrl', 'uz-Latn']) {
    await page.selectOption('#lang', lang);
    await expect(page.locator('#h-recent')).toHaveText(txt(lang, 'home.recent'));
    await expect(page.locator('#cards li').first().locator('.name')).toHaveText(appName('docs', lang));
    await expect(page.locator('html')).toHaveAttribute('lang', lang);
  }
  await page.reload();
  await expect(page.locator('#h-recent')).toHaveText(txt('uz-Latn', 'home.recent'));
});

test('headers per route: strict on the shell and the converter, wasm only on an editor', async ({ page }) => {
  const csp = async (p: string) => (await page.request.get(base + p)).headers()['content-security-policy'];
  const strict = await csp('/');
  expect(strict).toContain("script-src 'self';");
  expect(strict).not.toContain('wasm-unsafe-eval');
  expect(strict).toContain("frame-ancestors 'none'");
  expect(await csp('/convert/')).toBe(strict);
  for (const e of EDITORS.filter(have)) {
    const c = await csp(`/${e}/`);
    expect(c).toMatch(/script-src 'self' 'wasm-unsafe-eval' 'sha256-[A-Za-z0-9+/=]+'/);
    expect(c).toContain("frame-ancestors 'self'");
    expect(c).not.toContain('unsafe-inline');
  }
});

test('opening Docs fetches only the Docs bundle, under its own CSP', async ({ page }) => {
  test.skip(!have('docs'), 'docs-web is not built');
  const w = watch(page);
  await page.goto(base + '/');
  await page.locator('#cards li').first().getByRole('button').click();
  await expect(page.locator('#editor')).toBeVisible();
  await editorUp(page, '/docs/');
  const w2 = wasm(w.urls);
  expect(w2).toHaveLength(1);
  expect(w2[0]).toMatch(/^\/docs\/docs-web-[0-9a-f]+_bg\.wasm$/);
  expect(w.urls.filter((u) => /^\/(sheets|slides|pdf)\//.test(u))).toEqual([]);
  expect(w.problems).toEqual([]);
  // back to the home screen keeps the editor; a second New restarts it from the HTTP cache, still only the Docs wasm
  await page.click('#back');
  await expect(page.locator('#home')).toBeVisible();
  await page.locator('#cards li').first().getByRole('button').click();
  await editorUp(page, '/docs/');
  expect(wasm(w.urls).every((u) => u === w2[0])).toBe(true);
});

test('Docs round trip: open a file from the shell, save it, find it in Recent, reopen it', async ({ page }) => {
  test.skip(!have('docs'), 'docs-web is not built');
  const w = watch(page);
  await page.addInitScript(() => {
    (window as any).__msgs = [];
    addEventListener('message', (e) => (window as any).__msgs.push(e.data?.type));
  });
  await page.goto(base + '/');
  await page.setInputFiles('#file', DOCX);
  const name = path.basename(DOCX);
  await expect(page.locator('#ed-title')).toHaveText(name);
  await editorUp(page, '/docs/');
  await expect.poll(() => page.evaluate(() => (window as any).__msgs), { timeout: 60_000 }).toContain('wordcraft:opened');
  // Ctrl+S in the editor hands the bytes to the shell, which downloads them and keeps a copy
  const frame = page.frameLocator('iframe[src^="/docs/"]').locator('canvas');
  await frame.click({ position: { x: 300, y: 300 } });
  const [dl] = await Promise.all([page.waitForEvent('download', { timeout: 60_000 }), page.keyboard.press('Control+s')]);
  expect(dl.suggestedFilename()).toBe(name);
  expect(fs.readFileSync((await dl.path())!).subarray(0, 2).toString()).toBe('PK');
  await expect(page.locator('#ed-status')).toHaveText(txt('en', 'ed.saved').replace('{name}', name));
  await page.click('#back');
  await expect(page.locator('#recent li')).toHaveCount(1);
  await expect(page.locator('#recent li .open')).toHaveText(name);
  // search narrows the list
  await page.fill('#search', 'zzz-no-such');
  await expect(page.locator('#recent-empty')).toHaveText(txt('en', 'home.search.none'));
  await page.fill('#search', name.slice(0, 4));
  await expect(page.locator('#recent li')).toHaveCount(1);
  // reopen from Recent: the editor is already loaded, so no second wasm
  const before = (await page.evaluate(() => (window as any).__msgs)).filter((m: string) => m === 'wordcraft:opened').length;
  await page.locator('#recent li .open').click();
  await expect(page.locator('#ed-title')).toHaveText(name);
  await expect.poll(async () => (await page.evaluate(() => (window as any).__msgs)).filter((m: string) => m === 'wordcraft:opened').length, { timeout: 60_000 }).toBe(before + 1);
  expect(wasm(w.urls)).toHaveLength(1);
  expect(w.problems).toEqual([]);
});

test('Slides round trip: open a pptx from the shell, edit it, save it, find it in Recent, reopen it', async ({ page }) => {
  test.skip(!have('slides'), 'slides-web is not built');
  const w = watch(page);
  await page.addInitScript(() => {
    (window as any).__msgs = [];
    addEventListener('message', (e) => (window as any).__msgs.push(e.data?.type + (e.data?.type === 'deckcraft:dirty' ? ':' + e.data.dirty : '')));
  });
  await page.goto(base + '/');
  await page.setInputFiles('#file', PPTX);
  const name = path.basename(PPTX);
  await expect(page.locator('#ed-title')).toHaveText(name);
  await editorUp(page, '/slides/');
  const msgs = () => page.evaluate(() => (window as any).__msgs as string[]);
  await expect.poll(msgs, { timeout: 60_000 }).toContain('deckcraft:opened');
  // an edit (New Slide) makes the editor dirty and the shell hears it
  const canvas = page.frameLocator('iframe[src^="/slides/"]').locator('canvas');
  await canvas.click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+Shift+N');
  await expect.poll(async () => (await msgs()).filter((m) => m.startsWith('deckcraft:dirty')).at(-1), { timeout: 60_000 }).toBe('deckcraft:dirty:true');
  // Ctrl+S hands a .pptx to the shell, which downloads it and keeps a copy
  const [dl] = await Promise.all([page.waitForEvent('download', { timeout: 60_000 }), page.keyboard.press('Control+s')]);
  expect(dl.suggestedFilename()).toBe(name);
  expect(fs.readFileSync((await dl.path())!).subarray(0, 2).toString()).toBe('PK');
  await expect(page.locator('#ed-status')).toHaveText(txt('en', 'ed.saved').replace('{name}', name));
  await expect.poll(async () => (await msgs()).filter((m) => m.startsWith('deckcraft:dirty')).at(-1), { timeout: 60_000 }).toBe('deckcraft:dirty:false');
  await page.click('#back');
  await expect(page.locator('#recent li')).toHaveCount(1);
  await expect(page.locator('#recent li .open')).toHaveText(name);
  // reopen from Recent: the editor is already loaded, so no second wasm
  const before = (await msgs()).filter((m) => m === 'deckcraft:opened').length;
  await page.locator('#recent li .open').click();
  await expect(page.locator('#ed-title')).toHaveText(name);
  await expect.poll(async () => (await msgs()).filter((m) => m === 'deckcraft:opened').length, { timeout: 60_000 }).toBe(before + 1);
  expect(wasm(w.urls)).toHaveLength(1);
  expect(w.problems).toEqual([]);
});

test('a file the editors cannot open points to the converter', async ({ page }) => {
  await page.goto(base + '/');
  await page.setInputFiles('#file', { name: 'x.zip', mimeType: 'application/zip', buffer: Buffer.from('PK') });
  await expect(page.locator('#error')).toHaveText(txt('en', 'err.type'));
});

test('the converter is reached from the shell and converts', async ({ page }) => {
  const w = watch(page);
  await page.goto(base + '/');
  await page.click('#convert');
  await expect(page).toHaveURL(base + '/convert/');
  await expect(page.locator('#o-to')).toBeVisible();
  await page.setInputFiles('#pick', { name: 'note.txt', mimeType: 'text/plain', buffer: Buffer.from('Salom dunyo, bu oddiy matn.\n') });
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  await expect(page.locator('#results li', { hasText: 'note.txt' }).locator('.badge')).toHaveText(/./);
  await expect(page.locator('#results li', { hasText: 'note.txt' }).locator('a.btn')).toBeVisible();
  expect(wasm(w.urls)).toEqual([]);
  expect(w.problems).toEqual([]);
});

for (const e of EDITORS) {
  test(`${e}: the editor starts under its own CSP, with no violation`, async ({ page }) => {
    test.skip(!have(e), `${e}-web is not built`);
    const w = watch(page);
    await page.goto(`${base}/${e}/?host=parent`);
    await page.waitForFunction(() => !!(window as any).wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 120_000 });
    expect(w.problems).toEqual([]);
  });
}

// Review M2. The desktop host's IPC is stood in for by a script: the shell must take Recent and the language from the host and
// keep nothing of its own in the browser.
test('desktop mode: Recent and the language come from the host, nothing is stored in the browser', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    w.__sent = [];
    w.ipc = {
      postMessage(m: string) {
        const d = JSON.parse(m);
        w.__sent.push(d);
        const reply = d.type === 'recent'
          ? { type: 'recent', items: [{ path: 'C:/Users/u/Docs/Buyruq 1.docx', app: 'docs', ts: 1760000000 }] }
          : d.type === 'settings_get' ? { type: 'settings', values: { lang: 'ru' } }
          : d.type === 'settings_set' ? { type: 'settings', values: { lang: d.value } } : null;
        if (reply) setTimeout(() => dispatchEvent(new CustomEvent('suite-host', { detail: reply })), 0);
      },
    };
  });
  await page.goto(base + '/');
  await expect(page.locator('#recent li .open')).toHaveText('Buyruq 1.docx');
  await expect(page.locator('#lang')).toHaveValue('ru');
  await expect(page.locator('#recent-note')).toHaveText(txt('ru', 'home.recent.note.desktop'));
  await expect(page.locator('#recent li button.ghost')).toHaveCount(0); // the host's log cannot be edited from the page
  await page.locator('#recent li .open').click();
  expect((await page.evaluate(() => (window as any).__sent)).at(-1)).toEqual({ type: 'open', path: 'C:/Users/u/Docs/Buyruq 1.docx' });
  await page.selectOption('#lang', 'en');
  await expect.poll(() => page.evaluate(() => (window as any).__sent.at(-1))).toEqual({ type: 'settings_set', key: 'lang', value: 'en' });
  expect(await page.evaluate(() => localStorage.getItem('sh.lang'))).toBeNull();
  expect(await page.evaluate(async () => (await indexedDB.databases()).map((d) => d.name))).not.toContain('shell');
});

// WP5: the five Uzbek starters are listed on the home screen and every file is served from the shell route.
test('the template cards list the five starters and every file is served', async ({ page, request }) => {
  await page.goto(base + '/');
  await expect(page.locator('#templates li')).toHaveCount(5);
  const catalog = await (await request.get(`${base}/shell/templates.json`)).json();
  expect(catalog.map((t: any) => t.app)).toEqual(['docs', 'docs', 'sheets', 'sheets', 'slides']);
  for (const t of catalog) {
    const r = await request.get(`${base}/shell/${t.file}`);
    expect(r.status(), t.file).toBe(200);
    expect((await r.body()).subarray(0, 2).toString(), t.file).toBe('PK'); // an OOXML package is a zip
  }
});

// settings.toml `lang` (suite-core): 'auto' follows the browser, 'uz-Latn-2026' has no shell catalog yet and shows uz-Latn.
for (const [saved, shown] of [['auto', 'en'], ['uz-Latn-2026', 'uz-Latn']]) {
  test(`desktop mode: lang "${saved}" from settings.toml shows ${shown}`, async ({ page }) => {
    await page.addInitScript((lang) => {
      (window as any).ipc = {
        postMessage(m: string) {
          const d = JSON.parse(m);
          const reply = d.type === 'recent' ? { type: 'recent', items: [] } : d.type === 'settings_get' ? { type: 'settings', values: { lang, toolbar: 'simple', latin_alphabet: '1995' } } : null;
          if (reply) setTimeout(() => dispatchEvent(new CustomEvent('suite-host', { detail: reply })), 0);
        },
      };
    }, saved);
    await page.goto(base + '/');
    await expect(page.locator('#lang')).toHaveValue(shown);
    await expect(page.locator('html')).toHaveAttribute('lang', shown);
  });
}

test('connected to a server: Recent keeps names only, no IndexedDB, and asks for the file again', async ({ page }) => {
  await page.addInitScript(() => {
    sessionStorage.setItem('cv.token', 'abc.def.ghi');
    if (!sessionStorage.getItem('sh.recent')) sessionStorage.setItem('sh.recent', JSON.stringify([{ app: 'docs', name: 'Hisobot.docx', ts: 1760000000000 }]));
  });
  await page.goto(base + '/');
  await expect(page.locator('#recent li .open')).toHaveText('Hisobot.docx');
  await expect(page.locator('#recent-note')).toHaveText(txt('en', 'home.recent.note.names'));
  expect(await page.evaluate(async () => (await indexedDB.databases()).map((d) => d.name))).not.toContain('shell');
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.locator('#recent li .open').click()]);
  expect(chooser).toBeTruthy(); // no copy to reopen: the file picker opens
  await page.locator('#recent li button.ghost').click();
  await expect(page.locator('#recent li')).toHaveCount(0);
});
