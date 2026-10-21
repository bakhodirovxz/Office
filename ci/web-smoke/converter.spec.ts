import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The converter page against the real convert-api binary. The tools are the test stand-in (convert-api-fake-tool): it writes a
// stub PDF, `FAIL...` input makes it exit 1, `SLEEP...` input keeps it busy (cancel). Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.converter.config.ts
// UI_SHOTS=<dir> also writes the four-language screenshots.
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const API_BIN = process.env.CONVERT_API_BIN ?? path.join(SVC, 'convert-api.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');

// The brand name and the strings come from the repo's own files, never hard-coded here.
const I18N: Record<string, Record<string, string>> = JSON.parse(fs.readFileSync(path.join(ROOT, 'services/convert-api/ui/i18n.json'), 'utf8'));
const brand = fs.readFileSync(path.join(ROOT, 'brand/brand.toml'), 'utf8').split('[apps.convert]')[1].split(/\n\[/)[0];
const BRAND_EN = /names\s*=\s*\{[^}]*\ben\s*=\s*"([^"]+)"/.exec(brand)![1];
const txt = (lang: string, key: string, p: Record<string, string> = {}) =>
  (I18N[lang]?.[key] ?? I18N.en[key]).replace(/\{(\w+)\}/g, (m, k) => p[k] ?? m);

let proc: ChildProcess;
let base: string;
let dir: string;

test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'convert-ui-'));
  const cfg = path.join(dir, 'api.toml');
  // the Host check needs the real port, so take a free one first
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  fs.writeFileSync(cfg, `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(API_BIN, ['--config', cfg], { env });
  base = await new Promise<string>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => {
      out += d;
      const m = /listening on (http:\/\/\S+)/.exec(out);
      if (m) ok(m[1]);
    });
    proc.on('exit', (c) => bad(new Error(`convert-api exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('convert-api did not start')), 15000);
  });
});
test.afterAll(() => {
  proc.kill();
  fs.rmSync(dir, { recursive: true, force: true });
});

const file = (name: string, body: string | Buffer) => ({ name, mimeType: 'application/octet-stream', buffer: Buffer.from(body) });
const GOOD = 'Salom dunyo, bu oddiy matn.\n';

async function open(page: Page, lang = 'en') {
  const problems: string[] = [];
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy/i.test(m.text())) problems.push(m.text()); });
  await page.addInitScript((l) => { if (!localStorage.getItem('cv.lang')) localStorage.setItem('cv.lang', l); }, lang);
  await page.goto(base + '/');
  await expect(page.locator('#o-to')).toBeVisible();
  return problems;
}
async function pick(page: Page, files: ReturnType<typeof file>[]) {
  await page.setInputFiles('#pick', files);
  await expect(page.locator('#files tr')).toHaveCount(files.length);
}
const row = (page: Page, name: string) => page.locator('#results li', { hasText: name });

test('a successful conversion: progress, download and zip, with no CSP violation', async ({ page }) => {
  const problems = await open(page);
  await expect(page).toHaveTitle(BRAND_EN);
  await expect(page.locator('#conn-mode')).toHaveText(txt('en', 'ui.conn.mode.local')); // local mode asks for nothing
  await pick(page, [file('note.txt', GOOD)]);
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  await expect(row(page, 'note.txt').locator('.badge')).toHaveText(txt('en', 'state.done'));
  await expect(page.locator('#prog')).toHaveText(txt('en', 'ui.progress', { done: '1', total: '1' }));
  const [dl] = await Promise.all([page.waitForEvent('download'), row(page, 'note.txt').locator('a.btn').click()]);
  expect(dl.suggestedFilename()).toMatch(/\.pdf$/);
  expect(fs.readFileSync((await dl.path())!).subarray(0, 5).toString()).toBe('%PDF-');
  const [zip] = await Promise.all([page.waitForEvent('download'), page.click('#zip')]);
  expect(zip.suggestedFilename()).toBe('converted.zip');
  expect(fs.readFileSync((await zip.path())!).subarray(0, 2).toString()).toBe('PK');
  expect(problems).toEqual([]);
});

test('a failing file and a rejected file show their messages; the rest of the batch still runs', async ({ page }) => {
  const problems = await open(page);
  await pick(page, [file('good.txt', GOOD), file('bad.txt', 'FAIL this one'), file('junk.docx', Buffer.from([0, 1, 2, 3, 250, 251, 0, 7]))]);
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  await expect(row(page, 'good.txt').locator('.badge')).toHaveText(txt('en', 'state.done'));
  // failed at run time: localised headline (exit 1) plus the engine's own message
  const bad = row(page, 'bad.txt');
  await expect(bad.locator('.badge')).toHaveText(txt('en', 'state.failed'));
  await expect(bad.locator('.msg')).toHaveText(txt('en', 'exit.1'));
  await expect(bad).toContainText('fake tool: cannot read this file');
  // rejected before the run (unknown content): exit 3 headline plus the server's reason
  const junk = row(page, 'junk.docx');
  await expect(junk.locator('.badge')).toHaveText(txt('en', 'state.rejected'));
  await expect(junk.locator('.msg')).toHaveText(txt('en', 'exit.3'));
  await expect(junk).toContainText('unsupported input');
  await expect(page.locator('#go')).toBeEnabled(); // finished: the page is usable again
  expect(problems).toEqual([]);
});

test('an oversize upload is refused and the message shows (not silent)', async ({ page }) => {
  await open(page);
  await pick(page, [file('huge.txt', 'x'.repeat(2 * 1024 * 1024))]); // the test server allows 1 MB per file
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  const huge = row(page, 'huge.txt');
  await expect(huge.locator('.badge')).toHaveText(txt('en', 'state.rejected'));
  await expect(huge.locator('.msg')).toHaveText(txt('en', 'err.file_too_large'));
  await expect(huge).toContainText('2.0 MB');
  await expect(page.locator('#go')).toBeEnabled();
});

test('a warning shows in the user language', async ({ page }) => {
  await open(page, 'ru');
  await pick(page, [file('memo.docx', GOOD)]); // plain text under a .docx name: the kind-mismatch warning
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  await expect(row(page, 'memo.docx').locator('.warn')).toHaveText(txt('ru', 'ui.warning', { text: txt('ru', 'warning.kind_mismatch', { kind: 'txt' }) }));
});

test('cancel stops a running conversion and deletes the job', async ({ page }) => {
  await open(page);
  const created = page.waitForResponse((r) => r.url().endsWith('/api/v1/jobs') && r.request().method() === 'POST');
  await pick(page, [file('slow.txt', 'SLEEP for a long time')]);
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  const job = (await (await created).json()).job as string;
  await expect(page.locator('#stop')).toBeVisible();
  await expect(page.locator('#files tr td').nth(3)).toHaveText(/./); // a state is shown
  await page.click('#stop');
  await expect(page.locator('#alert')).toHaveText(txt('en', 'ui.cancelled'));
  await expect(page.locator('#stop')).toBeHidden();
  await expect(page.locator('#go')).toBeEnabled();
  await expect.poll(async () => (await page.request.get(`${base}/api/v1/jobs/${job}`)).status()).toBe(404);
});

test('the language switch changes the page, the name and the document language, and is remembered', async ({ page }) => {
  const problems = await open(page);
  for (const [lang, htmlLang] of [['ru', 'ru'], ['uz-Latn', 'uz-Latn'], ['uz-Cyrl', 'uz-Cyrl'], ['en', 'en']] as const) {
    await page.selectOption('#lang', lang);
    await expect(page.locator('html')).toHaveAttribute('lang', htmlLang);
    await expect(page.locator('#files-h')).toHaveText(txt(lang, 'ui.files.heading'));
    await expect(page.locator('#go')).toHaveText(txt(lang, 'ui.convert'));
    await expect(page).toHaveTitle(I18N[lang]['app.name']);
  }
  expect(txt('uz-Cyrl', 'ui.convert')).not.toBe(txt('uz-Latn', 'ui.convert'));
  await page.selectOption('#lang', 'uz-Cyrl');
  await page.reload();
  await expect(page.locator('#lang')).toHaveValue('uz-Cyrl'); // remembered
  await expect(page.locator('#go')).toHaveText(txt('uz-Cyrl', 'ui.convert'));
  // the self-hosted Noto Sans really loaded and has the Uzbek letters
  expect(await page.evaluate(async () => { await document.fonts.ready; return document.fonts.check('16px "Noto Sans Uz"', '\u02bb\u02bc\u045e\u049b\u0493\u04b3'); })).toBe(true);
  expect(problems).toEqual([]);
});

test('connect to server: the #token= fragment is read and removed; disconnect returns to local mode', async ({ page }) => {
  await page.goto(base + '/#token=abc.def.ghi');
  await expect(page.locator('#conn-mode')).toHaveText(txt('en', 'ui.conn.mode.server'));
  expect(new URL(page.url()).hash).toBe(''); // stripped from the address bar
  await page.locator('#conn summary').click();
  await page.click('#srv-clear');
  await expect(page.locator('#conn-mode')).toHaveText(txt('en', 'ui.conn.mode.local'));
  // a pasted token works too
  await page.fill('#srv-token', 'pasted-token');
  await page.click('#srv-apply');
  await expect(page.locator('#conn-mode')).toHaveText(txt('en', 'ui.conn.mode.server'));
  await expect(page.locator('#srv-token')).toHaveValue(''); // not left in the field
});

test('server mode: the healthz probe carries the token in the Authorization header, never in the URL; local mode sends none', async ({ page }) => {
  const probes: { url: string; auth: string | undefined }[] = [];
  page.on('request', (r) => { if (new URL(r.url()).pathname === '/healthz') probes.push({ url: r.url(), auth: r.headers()['authorization'] }); });
  await page.goto(base + '/#token=abc.def.ghi');
  await expect(page.locator('#conn-mode')).toHaveText(txt('en', 'ui.conn.mode.server'));
  await expect.poll(() => probes.length).toBeGreaterThan(0);
  expect(probes.at(-1)).toEqual({ url: base + '/healthz', auth: 'Bearer abc.def.ghi' });
  expect(probes.every((p) => !p.url.includes('abc.def.ghi'))).toBe(true);
  // disconnecting probes again, now without the header
  const n = probes.length;
  await page.locator('#conn summary').click();
  await page.click('#srv-clear');
  await expect.poll(() => probes.length).toBeGreaterThan(n);
  expect(probes.at(-1)!.auth).toBeUndefined();
});

test('screenshots in en, ru, uz-Latn and uz-Cyrl', async ({ page }) => {
  test.skip(!process.env.UI_SHOTS, 'set UI_SHOTS=<dir> to write them');
  fs.mkdirSync(process.env.UI_SHOTS!, { recursive: true });
  for (const lang of ['en', 'ru', 'uz-Latn', 'uz-Cyrl']) {
    await page.goto(base + '/');
    await page.selectOption('#lang', lang);
    await expect(page.locator('#go')).toHaveText(txt(lang, 'ui.convert'));
    await pick(page, [file('hisobot.txt', GOOD), file('memo.docx', GOOD), file('bad.txt', 'FAIL x'), file('junk.docx', Buffer.from([0, 1, 2, 250]))]);
    await page.selectOption('#o-to', 'pdf');
    await page.click('#go');
    await expect(row(page, 'junk.docx').locator('.badge')).toBeVisible();
    await expect(row(page, 'bad.txt').locator('.badge')).toHaveText(txt(lang, 'state.failed'));
    await expect(row(page, 'hisobot.txt').locator('.badge')).toHaveText(txt(lang, 'state.done'));
    await page.screenshot({ path: path.join(process.env.UI_SHOTS!, `converter-${lang}.png`), fullPage: true });
  }
});
