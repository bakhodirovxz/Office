import { test, expect, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

const HRM = 'http://127.0.0.1:8790';
const SHOTS = path.resolve(__dirname, '../../../docs/phase0/docserver');
const RESULTS = path.resolve(__dirname, 'results.jsonl');
const FIXTURE = path.resolve(__dirname, '../fixtures/sample.docx');
const VERIFY = path.resolve(__dirname, 'verify_docx.py');
fs.mkdirSync(SHOTS, { recursive: true });

function record(o: object) {
  fs.appendFileSync(RESULTS, JSON.stringify({ at: new Date().toISOString(), ...o }) + '\n');
}

async function received(): Promise<any[]> {
  const t = await (await fetch(HRM + '/received')).text();
  return t.split('\n').filter(Boolean).map((l) => JSON.parse(l));
}
async function rejected(): Promise<any[]> {
  const t = await (await fetch(HRM + '/rejected')).text();
  return t.split('\n').filter(Boolean).map((l) => JSON.parse(l));
}

function trackWasm(page: Page) {
  const out = { bytes: 0, via: 'none' };
  page.on('response', (r) => {
    if (r.url().endsWith('.wasm')) {
      out.bytes = Number(r.headers()['content-length'] || 0);
      out.via = 'content-length';
      r.body().then((b) => { out.bytes = b.length; out.via = 'body().length'; }).catch(() => { /* keep header value */ });
    }
  });
  return out;
}

test.afterEach(async () => {
  await fetch(HRM + '/admin/callback-secret', { method: 'POST' }); // reset any override
});

for (const run of [1, 2, 3]) {
  test(`S6 happy path (shim) run ${run}`, async ({ page }) => {
    const wasm = trackWasm(page);
    const logs: string[] = [];
    page.on('console', (m) => logs.push(m.type() + ': ' + m.text()));
    page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
    const before = (await received()).length;

    const t0 = Date.now();
    await page.goto(HRM + '/');
    await page.waitForFunction(() => document.body.dataset.state === 'ready' || document.body.dataset.state === 'error', null, { timeout: 150_000 });
    const readyMs = Date.now() - t0;
    expect(await page.evaluate(() => document.body.dataset.state), logs.join('\n')).toBe('ready');

    const ed = page.frameLocator('#ph iframe').frameLocator('#wc');
    const canvas = ed.locator('#wordcraft_canvas');
    await page.waitForTimeout(1500); // let the first frames paint
    if (run === 1) await page.screenshot({ path: path.join(SHOTS, 's6-1-before-edit.png') });
    await canvas.click({ position: { x: 400, y: 300 } });
    await page.keyboard.press('Control+End');
    const marker = 'SPIKE4-' + Date.now();
    await page.keyboard.type(' ' + marker + ' Салом');
    await page.waitForTimeout(800);
    if (run === 1) await page.screenshot({ path: path.join(SHOTS, 's6-2-after-edit.png') });

    const tSave = Date.now();
    await page.keyboard.press('Control+S');
    let line: any = null;
    let tRecv = 0;
    while (Date.now() - tSave < 30_000) {
      const r = await received();
      if (r.length > before) { line = r[r.length - 1]; tRecv = Date.now(); break; }
      await new Promise((r) => setTimeout(r, 50));
    }
    expect(line, 'no callback received; console: ' + logs.join('\n')).not.toBeNull();
    expect(line.status).toBe(6);
    const saveToCallbackMs = tRecv - tSave;
    await page.waitForFunction(() => document.body.dataset.state === 'saved', null, { timeout: 15_000 });
    if (run === 1) await page.screenshot({ path: path.join(SHOTS, 's6-3-saved-banner.png') });

    const out = execFileSync('python', ['-I', VERIFY, line.file, marker, 'Салом'], { encoding: 'utf8' });
    expect(out).toContain('OK');
    const size = fs.statSync(line.file).size;
    const fixSize = fs.statSync(FIXTURE).size;
    record({ test: 'S6', run, readyMs, saveToCallbackMs, wasmBytes: wasm.bytes, wasmVia: wasm.via, savedDocxBytes: size, fixtureBytes: fixSize, marker });
    console.log(JSON.stringify({ run, readyMs, saveToCallbackMs, wasmBytes: wasm.bytes, wasmVia: wasm.via, size, fixSize }));
  });
}

test('S7 (i) tampered token -> 401 + onError', async ({ page }) => {
  let status = 0;
  page.on('response', (r) => { if (r.url().endsWith('/api/session') && r.request().method() === 'POST') status = r.status(); });
  await page.goto(HRM + '/?tamper=1');
  await page.waitForFunction(() => document.body.dataset.state === 'error', null, { timeout: 30_000 });
  const err = await page.evaluate(() => (window as any).__error);
  await page.screenshot({ path: path.join(SHOTS, 's7-1-tampered-token.png') });
  record({ test: 'S7i', httpStatus: status, onError: err });
  expect(status).toBe(401);
  expect(err.status).toBe(401);
});

test('S7 (ii) SSRF document.url (signed) -> 403', async ({ page }) => {
  let status = 0;
  page.on('response', (r) => { if (r.url().endsWith('/api/session') && r.request().method() === 'POST') status = r.status(); });
  await page.goto(HRM + '/?url=' + encodeURIComponent('http://169.254.169.254/latest'));
  await page.waitForFunction(() => document.body.dataset.state === 'error', null, { timeout: 30_000 });
  const err = await page.evaluate(() => (window as any).__error);
  await page.screenshot({ path: path.join(SHOTS, 's7-2-ssrf.png') });
  record({ test: 'S7ii', httpStatus: status, onError: err });
  expect(status).toBe(403);
  expect(err.message).toContain('host not allowed');
});

test('S7 (iii) wrong callback secret -> HRM 401, callback failed, onError', async ({ page }) => {
  await fetch(HRM + '/admin/callback-secret?value=not-the-shared-secret', { method: 'POST' });
  const recBefore = (await received()).length;
  const rejBefore = (await rejected()).length;
  let saveBody: any = null;
  page.on('response', async (r) => {
    if (r.url().includes('/api/session/') && r.url().endsWith('/save')) { try { saveBody = { status: r.status(), body: await r.json() }; } catch { /* ignore */ } }
  });
  await page.goto(HRM + '/');
  await page.waitForFunction(() => document.body.dataset.state === 'ready' || document.body.dataset.state === 'error', null, { timeout: 150_000 });
  expect(await page.evaluate(() => document.body.dataset.state)).toBe('ready');
  const ed = page.frameLocator('#ph iframe').frameLocator('#wc');
  await page.waitForTimeout(1500);
  await ed.locator('#wordcraft_canvas').click({ position: { x: 400, y: 300 } });
  await page.keyboard.press('Control+End');
  await page.keyboard.type(' NEG3-' + Date.now());
  await page.keyboard.press('Control+S');
  await page.waitForFunction(() => document.body.dataset.state === 'error', null, { timeout: 30_000 });
  const err = await page.evaluate(() => (window as any).__error);
  await page.screenshot({ path: path.join(SHOTS, 's7-3-wrong-callback-secret.png') });
  const rej = await rejected();
  record({ test: 'S7iii', saveResponse: saveBody, onError: err, hrmRejected: rej.slice(rejBefore) });
  expect(saveBody.body.callback).toBe('failed');
  expect(saveBody.body.detail).toContain('401');
  expect((await received()).length).toBe(recBefore);
  expect(rej.length).toBeGreaterThan(rejBefore);
});
