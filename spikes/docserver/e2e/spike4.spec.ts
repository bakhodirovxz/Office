import { test, expect, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

const HRM = 'http://127.0.0.1:8790';
// ADAPTER=shim (default) or bridge: how the wrapper talks to the editor (editor.html).
const ADAPTER = process.env.ADAPTER || 'shim';
const Q = ADAPTER === 'bridge' ? '?adapter=bridge' : '';
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

// Save: the shim has no host-initiated save (Ctrl+S in the editor); the bridge is asked to.
async function save(page: Page) {
  if (ADAPTER === 'bridge') await page.evaluate(() => (window as any).__editor.requestSave());
  else await page.keyboard.press('Control+S');
}

test.afterEach(async () => {
  await fetch(HRM + '/admin/callback-secret', { method: 'POST' }); // reset any override
});

for (const run of [1, 2, 3]) {
  test(`S6 happy path (${ADAPTER}) run ${run}`, async ({ page }) => {
    const wasm = trackWasm(page);
    const logs: string[] = [];
    page.on('console', (m) => logs.push(m.type() + ': ' + m.text()));
    page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
    const before = (await received()).length;

    const t0 = Date.now();
    await page.goto(HRM + '/' + Q);
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
    await save(page);
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
    record({ test: 'S6', adapter: ADAPTER, run, readyMs, saveToCallbackMs, wasmBytes: wasm.bytes, wasmVia: wasm.via, savedDocxBytes: size, fixtureBytes: fixSize, marker });
    console.log(JSON.stringify({ adapter: ADAPTER, run, readyMs, saveToCallbackMs, wasmBytes: wasm.bytes, wasmVia: wasm.via, size, fixSize }));
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
  await page.goto(HRM + '/' + Q);
  await page.waitForFunction(() => document.body.dataset.state === 'ready' || document.body.dataset.state === 'error', null, { timeout: 150_000 });
  expect(await page.evaluate(() => document.body.dataset.state)).toBe('ready');
  const ed = page.frameLocator('#ph iframe').frameLocator('#wc');
  await page.waitForTimeout(1500);
  await ed.locator('#wordcraft_canvas').click({ position: { x: 400, y: 300 } });
  await page.keyboard.press('Control+End');
  await page.keyboard.type(' NEG3-' + Date.now());
  await save(page);
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

// Bridge only: typing is reported as a dirty change, a save clears it, and closing the editor
// reports status 2 (changes saved) to the HRM.
test('S9 bridge: dirty events, requestSave, close -> status 2', async ({ page }) => {
  test.skip(ADAPTER !== 'bridge', 'bridge adapter only');
  const logs: string[] = [];
  page.on('console', (m) => logs.push(m.type() + ': ' + m.text()));
  page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
  const before = (await received()).length;
  const t0 = Date.now();
  await page.goto(HRM + '/?adapter=bridge');
  await page.waitForFunction(() => document.body.dataset.state === 'ready' || document.body.dataset.state === 'error', null, { timeout: 150_000 });
  const readyMs = Date.now() - t0;
  expect(await page.evaluate(() => document.body.dataset.state), logs.join('\n')).toBe('ready');
  const dirtyEvents = () => page.evaluate(() => ((window as any).__hrmLog as any[]).filter((e) => 'dirty' in e).map((e) => e.dirty));
  await page.waitForTimeout(1500);
  expect(await dirtyEvents(), 'no dirty change before typing').not.toContain(true);
  const ed = page.frameLocator('#ph iframe').frameLocator('#wc');
  await ed.locator('#wordcraft_canvas').click({ position: { x: 400, y: 300 } });
  await page.keyboard.press('Control+End');
  const marker = 'BRIDGE-' + Date.now();
  await page.keyboard.type(' ' + marker);
  await page.waitForFunction(() => ((window as any).__hrmLog as any[]).some((e) => e.dirty === true), null, { timeout: 10_000 });
  await page.waitForTimeout(800); // keys typed a moment ago reach the document before the save runs
  const tReq = Date.now();
  await page.evaluate(() => (window as any).__editor.requestSave());
  let line: any = null;
  while (Date.now() - tReq < 30_000) {
    const r = await received();
    if (r.length > before) { line = r[r.length - 1]; break; }
    await new Promise((r) => setTimeout(r, 50));
  }
  expect(line, logs.join('\n')).not.toBeNull();
  const requestSaveMs = Date.now() - tReq;
  expect(line.status).toBe(6);
  expect(execFileSync('python', ['-I', VERIFY, line.file, marker], { encoding: 'utf8' })).toContain('OK');
  await page.waitForFunction(() => {
    const l = (window as any).__hrmLog as any[];
    const d = l.filter((e) => 'dirty' in e);
    return d.length > 0 && d[d.length - 1].dirty === false;
  }, null, { timeout: 10_000 });
  // Type again (dirty), then close: the editor saves first, then the docserver reports status 2.
  await ed.locator('#wordcraft_canvas').click({ position: { x: 400, y: 300 } });
  await page.keyboard.press('Control+End');
  const marker2 = 'CLOSE-' + Date.now();
  await page.keyboard.type(' ' + marker2);
  await page.waitForTimeout(800);
  await page.waitForFunction(() => { const d = ((window as any).__hrmLog as any[]).filter((e) => 'dirty' in e); return d[d.length - 1].dirty === true; }, null, { timeout: 10_000 });
  const tClose = Date.now();
  await page.evaluate(() => (window as any).__editor.close());
  await page.waitForFunction(() => document.body.dataset.state === 'closed', null, { timeout: 30_000 });
  const closeMs = Date.now() - tClose;
  const all = await received();
  const closeLine = all[all.length - 1];
  expect(closeLine.status).toBe(2);
  expect(execFileSync('python', ['-I', VERIFY, closeLine.file, marker, marker2], { encoding: 'utf8' })).toContain('OK');
  const dirtySeq = await dirtyEvents();
  record({ test: 'S9', adapter: 'bridge', readyMs, requestSaveMs, closeMs, dirtySeq, statuses: all.slice(before).map((l: any) => l.status) });
  console.log(JSON.stringify({ test: 'S9', readyMs, requestSaveMs, closeMs, dirtySeq, statuses: all.slice(before).map((l: any) => l.status) }));
});

// Lifecycle (bridge only). A hidden tab hands the unsaved document to the host; a lost WebGL
// context makes the wrapper reload the editor on that copy and the work is still there.
const DATA = 'E:/office-target/docserver/data';
function newestAutosave(): string | null {
  const dirs = fs.readdirSync(DATA).map((d) => path.join(DATA, d)).filter((d) => fs.existsSync(path.join(d, 'autosave.docx')));
  dirs.sort((a, b) => fs.statSync(path.join(b, 'autosave.docx')).mtimeMs - fs.statSync(path.join(a, 'autosave.docx')).mtimeMs);
  return dirs.length ? path.join(dirs[0], 'autosave.docx') : null;
}

test('S9 lifecycle: hidden tab autosaves; lost WebGL context recovers', async ({ page }) => {
  test.skip(ADAPTER !== 'bridge', 'bridge adapter only');
  await page.goto(HRM + '/?adapter=bridge');
  await page.waitForFunction(() => document.body.dataset.state === 'ready', null, { timeout: 150_000 });
  const frame = page.frames().find((f) => f.url().includes('/wc/index.html'))!;
  const ed = page.frameLocator('#ph iframe').frameLocator('#wc');
  await page.waitForTimeout(1500);
  await ed.locator('#wordcraft_canvas').click({ position: { x: 400, y: 300 } });
  await page.keyboard.press('Control+End');
  const marker = 'LIFE-' + Date.now();
  await page.keyboard.type(' ' + marker);
  await page.waitForFunction(() => ((window as any).__hrmLog as any[]).some((e) => e.dirty === true), null, { timeout: 10_000 });
  await page.waitForTimeout(800); // the last keys reach the document (a hidden tab runs no input pass)
  // The tab goes to the background.
  const t0 = Date.now();
  await frame.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => true });
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await page.waitForFunction(() => ((window as any).__hrmLog as any[]).some((e) => e.autosave && e.autosave.stored), null, { timeout: 15_000 });
  const autosaveMs = Date.now() - t0;
  const file = newestAutosave()!;
  expect(execFileSync('python', ['-I', VERIFY, file, marker], { encoding: 'utf8' })).toContain('OK');
  const autosaveBytes = fs.statSync(file).size;
  await frame.evaluate(() => {
    delete (document as any).hidden; delete (document as any).visibilityState;
    document.dispatchEvent(new Event('visibilitychange'));
  });
  // The GPU context is lost.
  const lost = await frame.evaluate(() => {
    const c = document.getElementById('wordcraft_canvas') as HTMLCanvasElement;
    const gl = c.getContext('webgl2') as WebGL2RenderingContext | null;
    const ext = gl && gl.getExtension('WEBGL_lose_context');
    if (!ext) return false;
    ext.loseContext();
    return true;
  });
  expect(lost, 'WEBGL_lose_context available').toBe(true);
  await page.waitForFunction(() => ((window as any).__hrmLog as any[]).some((e) => e.recovered), null, { timeout: 150_000 });
  const recoverMs = Date.now() - t0;
  // The reloaded editor holds the work: saving it sends the marker to the HRM.
  const before = (await received()).length;
  await page.waitForTimeout(1500);
  await page.evaluate(() => (window as any).__editor.requestSave());
  let line: any = null;
  for (let i = 0; i < 300 && !line; i++) {
    const r = await received();
    if (r.length > before) line = r[r.length - 1];
    else await new Promise((r) => setTimeout(r, 100));
  }
  expect(line).not.toBeNull();
  expect(execFileSync('python', ['-I', VERIFY, line.file, marker], { encoding: 'utf8' })).toContain('OK');
  record({ test: 'S9-lifecycle', adapter: 'bridge', autosaveMs, autosaveBytes, recoverMs });
  console.log(JSON.stringify({ test: 'S9-lifecycle', autosaveMs, autosaveBytes, recoverMs }));
});
