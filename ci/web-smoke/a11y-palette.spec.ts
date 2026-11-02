import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';
import { bolds, documentXml } from './docx-xml';

// Phase 6 WP11: the keyboard way to the editor's commands in the home shell (the canvas has no DOM). Tab reaches the palette button, F1 or
// the button opens a combobox + listbox with named options, Enter runs Bold on the selected text (checked in the saved docx), Esc closes and
// returns to the document, F6 leaves the document. Env as shell.spec.ts (SUITE_SERVE_BIN, SHELL_DIST_ROOT with docs-web and shell).
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'hoshiya-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const DIST = process.env.SHELL_DIST_ROOT ?? 'E:/office-target/p1/dist';
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const I18N = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/shell/i18n.json'), 'utf8'));

let proc: ChildProcess, base: string, dir: string;
test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'a11y-spec-'));
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

async function openDocs(page: Page) {
  await page.addInitScript(() => { (window as any).__msgs = []; addEventListener('message', (e) => (window as any).__msgs.push(e.data?.type)); });
  await page.goto(base + '/');
  await page.setInputFiles('#file', DOCX);
  await expect.poll(() => page.evaluate(() => (window as any).__msgs), { timeout: 120_000 }).toContain('wordcraft:opened');
}

test('shell: Tab reaches the palette, Bold runs from it on the selection, Esc and F6 leave the canvas to the right places', async ({ page }) => {
  const problems: string[] = [];
  page.on('pageerror', (e) => problems.push(e.message));
  await openDocs(page);
  const before = bolds(documentXml(fs.readFileSync(DOCX)));
  const button = page.getByRole('button', { name: I18N.en['pal.open'] });
  // keyboard walk: "Home" holds the focus after opening (WP18), the next Tab is the palette button, shown only while focused
  await expect(page.locator('#back')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(button).toBeFocused();
  expect((await button.boundingBox())!.width).toBeGreaterThan(20);
  // select all in the document, then F1 from inside the canvas opens the palette
  const canvas = page.frameLocator('iframe[src^="/docs/"]').locator('canvas');
  await canvas.click({ position: { x: 300, y: 300 } });
  await page.keyboard.press('Control+a');
  await page.keyboard.press('F1');
  const dlg = page.getByRole('dialog', { name: I18N.en['pal.title'] });
  await expect(dlg).toBeVisible();
  const box = dlg.getByRole('combobox', { name: I18N.en['pal.search'] });
  await expect(box).toBeFocused();
  await expect(dlg.getByRole('option').first()).toBeVisible({ timeout: 30_000 });
  await box.fill('bold');
  await expect(dlg.getByRole('option', { name: /^Bold/ }).first()).toBeVisible();
  const snap = await dlg.ariaSnapshot();
  console.log(snap.split('\n').slice(0, 14).join('\n'));
  expect(snap).toMatch(/combobox/);
  expect(snap).toMatch(/- option "Bold/);
  await box.press('Enter');
  await expect(dlg).toBeHidden();
  // the document gets the focus back: save with Ctrl+S, the docx has more bold runs than the original
  const [dl] = await Promise.all([page.waitForEvent('download', { timeout: 60_000 }), page.keyboard.press('Control+s')]);
  const after = bolds(documentXml(fs.readFileSync((await dl.path())!)));
  expect(after).toBeGreaterThan(before);
  // Esc closes the palette and returns to the document: F1 again, Esc, then F6 leaves for "Home"
  await page.keyboard.press('F1');
  await expect(dlg).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dlg).toBeHidden();
  await page.keyboard.press('F6');
  await expect(page.locator('#back')).toBeFocused();
  // the button opens it too
  await button.focus();
  await page.keyboard.press('Enter');
  await expect(dlg).toBeVisible();
  await expect(dlg.getByRole('option').first()).toBeVisible({ timeout: 30_000 });
  expect(problems).toEqual([]);
});

for (const lang of ['uz-Latn', 'uz-Cyrl', 'ru']) {
  test(`shell: the palette speaks ${lang}`, async ({ page }) => {
    await page.addInitScript((l) => localStorage.setItem('sh.lang', l), lang);
    await openDocs(page);
    await page.getByRole('button', { name: I18N[lang]['pal.open'] }).focus();
    await page.keyboard.press('Enter');
    const dlg = page.getByRole('dialog', { name: I18N[lang]['pal.title'] });
    await expect(dlg).toBeVisible();
    await expect(dlg.getByRole('combobox', { name: I18N[lang]['pal.search'] })).toBeFocused();
    await expect(dlg.getByRole('option').first()).toBeVisible({ timeout: 30_000 });
  });
}
