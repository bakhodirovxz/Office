import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The converter page against the real convert-api binary. The tools are the test stand-in (convert-api-fake-tool): it writes a
// stub PDF, `FAIL...` input makes it exit 1, `SLEEP...` input keeps it busy (cancel). Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.converter.config.ts
// UI_SHOTS=<dir> also writes the five-language screenshots.
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

test('no warning shows a placeholder that was never filled, in any language', async ({ page }) => {
  // cp1251 text (text_transcoded: {encoding}) and a text file under a .docx name (kind_mismatch: {kind})
  const cp1251 = Buffer.from([0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2, 0x2c, 0x20, 0xec, 0xe8, 0xf0, 0x21, 0x20, 0xdd, 0xf2, 0xee, 0x20, 0xee, 0xe1, 0xfb, 0xf7, 0xed, 0xfb, 0xe9, 0x0a]);
  for (const lang of ['en', 'ru', 'uz-Latn', 'uz-Latn-2026', 'uz-Cyrl']) {
    await page.goto(base + '/');
    await page.selectOption('#lang', lang);
    await expect(page.locator('#go')).toHaveText(txt(lang, 'ui.convert'));
    await pick(page, [file('old.txt', cp1251), file('memo.docx', GOOD)]);
    await page.selectOption('#o-to', 'pdf');
    await page.click('#go');
    await expect(page.locator('#results .warn')).toHaveCount(2);
    const shown = await page.locator('#results').innerText();
    expect(shown, lang).not.toMatch(/\{\w+\}/);
    expect(shown, lang).toMatch(/windows-1251|cp1251/i); // the encoding the engine named is in the sentence
  }
});

const pdfFile = (name: string, pages: number) => file(name, `%PDF-1.4\n%% PAGES=${pages}\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n`);

test('PDF operations are offered for PDFs only; combine keeps the order the user sets (buttons and keyboard)', async ({ page }) => {
  const problems = await open(page);
  await pick(page, [file('note.txt', GOOD)]);
  await expect(page.locator('#f-mode')).toBeHidden(); // not every file is a PDF
  await page.click('#clear');
  await pick(page, [pdfFile('b.pdf', 3), pdfFile('a.pdf', 2), pdfFile('c.pdf', 4)]);
  await expect(page.locator('#f-mode')).toBeVisible();
  await expect(page.locator('#o-mode option')).toHaveText([txt('en', 'ui.opt.mode.convert'), txt('en', 'ui.opt.mode.combine'), txt('en', 'ui.opt.mode.split'), txt('en', 'ui.opt.mode.extract')]);
  await expect(page.locator('#f-to')).toBeVisible();
  await page.selectOption('#o-mode', 'combine');
  await expect(page.locator('#f-to')).toBeHidden();
  const names = () => page.locator('#files th.name').allInnerTexts();
  expect(await names()).toEqual(['1. b.pdf', '2. a.pdf', '3. c.pdf']);
  await expect(page.locator('#order-hint')).toContainText(txt('en', 'ui.order.heading'));
  // the first row cannot go up, the last cannot go down
  await expect(page.getByRole('button', { name: txt('en', 'ui.order.up', { name: 'b.pdf' }) })).toBeDisabled();
  await expect(page.getByRole('button', { name: txt('en', 'ui.order.down', { name: 'c.pdf' }) })).toBeDisabled();
  // button: a.pdf up; focus stays on that row's control and the move is announced
  await page.getByRole('button', { name: txt('en', 'ui.order.up', { name: 'a.pdf' }) }).click();
  expect(await names()).toEqual(['1. a.pdf', '2. b.pdf', '3. c.pdf']);
  await expect(page.locator('#live')).toHaveText(txt('en', 'ui.order.moved', { name: 'a.pdf', pos: '1', total: '3' }));
  await expect(page.locator('#files tr').first().locator('[data-act="down"]')).toBeFocused(); // "up" is disabled at the top
  // keyboard: Alt+ArrowDown on a focused control of b.pdf's row moves b.pdf down (the buttons themselves are native)
  await page.getByRole('button', { name: txt('en', 'ui.order.down', { name: 'b.pdf' }) }).focus();
  await page.keyboard.press('Alt+ArrowDown');
  expect(await names()).toEqual(['1. a.pdf', '2. c.pdf', '3. b.pdf']);
  await page.click('#go');
  await expect(page.locator('#results li')).toHaveCount(1); // one result for the whole operation
  const card = page.locator('#results li').first();
  await expect(card.locator('.badge')).toHaveText(txt('en', 'state.done'));
  const done = txt('en', 'state.done');
  await expect(page.locator('#files tr td:nth-child(4)')).toHaveText([done, done, done]); // n2: the table follows the operation
  const link = card.locator('a.btn');
  await expect(link).toHaveCount(1);
  const [dl] = await Promise.all([page.waitForEvent('download'), link.click()]);
  expect(dl.suggestedFilename()).toBe('combined.pdf');
  // the page counts of the inputs, in the order the page showed them (a=2, c=4, b=3; the stand-in tool writes them down)
  expect(/ORDER=([\d,]+)/.exec(fs.readFileSync((await dl.path())!, 'latin1'))![1]).toBe('2,4,3');
  expect(problems).toEqual([]);
});

test('combine needs two PDFs; split and extract send their fields', async ({ page }) => {
  await open(page);
  await pick(page, [pdfFile('x.pdf', 5)]);
  await page.selectOption('#o-mode', 'combine');
  await expect(page.locator('#op-note')).toHaveText(txt('en', 'ui.pdfop.min_files'));
  await expect(page.locator('#go')).toBeDisabled();
  await page.selectOption('#o-mode', 'split');
  await expect(page.locator('#go')).toBeDisabled(); // needs a number or the pages to start at
  await page.fill('#o-every', '2');
  await expect(page.locator('#go')).toBeEnabled();
  await page.click('#go');
  await expect(page.locator('#results li .badge')).toHaveText(txt('en', 'state.done'));
  await expect(page.locator('#results li a.btn')).toHaveCount(3); // 5 pages, 2 per part
  await page.selectOption('#o-mode', 'extract');
  await page.fill('#o-xpages', '2-9'); // beyond the last page: the failure is shown with the engine's words
  await page.click('#go');
  await expect(page.locator('#results li .badge')).toHaveText(txt('en', 'state.failed'));
  await expect(page.locator('#results li')).toContainText('beyond');
});

test('a Russian-format CSV in an English page: the Number format select removes the locale warning', async ({ page }) => {
  await open(page, 'en');
  await pick(page, [file('note.txt', GOOD)]);
  await expect(page.locator('#f-locale')).toBeHidden(); // no csv or spreadsheet among the files
  await page.click('#clear');
  const csv = 'Имя;Дата;Сумма\nАнна;01.03.2019;3500000,0\nОлег;15.04.2020;1200,5\n';
  await pick(page, [file('staff.csv', csv)]);
  await expect(page.locator('#f-locale')).toBeVisible();
  await expect(page.locator('#o-locale')).toHaveValue('en-US'); // follows the UI language
  await page.selectOption('#o-to', 'csv');
  await page.click('#go');
  await expect(page.locator('#results .warn')).toContainText(txt('en', 'warning.csv_locale_mismatch').slice(0, 30));
  await page.selectOption('#o-locale', 'ru');
  await page.click('#go');
  await expect(page.locator('#results .badge')).toHaveText(txt('en', 'state.done'));
  await expect(page.locator('#results .warn')).toHaveCount(0);
  // the choice survives a change of the page language, and the default follows the language until then
  await page.selectOption('#lang', 'uz-Latn-2026');
  await expect(page.locator('#o-locale')).toHaveValue('ru');
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
  for (const [lang, htmlLang] of [['ru', 'ru'], ['uz-Latn', 'uz-Latn'], ['uz-Latn-2026', 'uz-Latn-2026'], ['uz-Cyrl', 'uz-Cyrl'], ['en', 'en']] as const) {
    await page.selectOption('#lang', lang);
    await expect(page.locator('html')).toHaveAttribute('lang', htmlLang);
    await expect(page.locator('#files-h')).toHaveText(txt(lang, 'ui.files.heading'));
    await expect(page.locator('#go')).toHaveText(txt(lang, 'ui.convert'));
    await expect(page).toHaveTitle(I18N[lang]['app.name']);
  }
  expect(txt('uz-Cyrl', 'ui.convert')).not.toBe(txt('uz-Latn', 'ui.convert'));
  // the 2026 catalog is generated from uz-Cyrl: it has the new letters and none of the 1995 oʻ gʻ
  const all26 = Object.values(I18N['uz-Latn-2026']).join(' ');
  expect(all26).toMatch(/[şçöğ]/i);
  expect(all26).not.toMatch(/ʻ/);
  // the Latin alphabet menu preselects and marks the server's default (DEFAULT_LATIN_ALPHABET, owner decision O13)
  const def = (await (await page.request.get(`${base}/api/v1/formats`)).json()).defaults.alphabet;
  await expect(page.locator('#o-alphabet')).toHaveValue(def);
  await expect(page.locator(`#o-alphabet option[value="${def}"]`)).toHaveText(txt('en', 'ui.opt.alphabet.default', { alphabet: def }));
  await page.selectOption('#lang', 'uz-Cyrl');
  await page.reload();
  await expect(page.locator('#lang')).toHaveValue('uz-Cyrl'); // remembered
  await expect(page.locator('#go')).toHaveText(txt('uz-Cyrl', 'ui.convert'));
  // the self-hosted Noto Sans really loaded and has the Uzbek letters
  expect(await page.evaluate(async () => { await document.fonts.ready; return document.fonts.check('16px "Noto Sans Uz"', '\u02bb\u02bc\u045e\u049b\u0493\u04b3\u015e\u015f\u00c7\u00e7\u00d6\u00f6\u011e\u011f'); })).toBe(true);
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

test('screenshots in en, ru, uz-Latn, uz-Latn-2026 and uz-Cyrl', async ({ page }) => {
  test.skip(!process.env.UI_SHOTS, 'set UI_SHOTS=<dir> to write them');
  fs.mkdirSync(process.env.UI_SHOTS!, { recursive: true });
  for (const lang of ['en', 'ru', 'uz-Latn', 'uz-Latn-2026', 'uz-Cyrl']) {
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
