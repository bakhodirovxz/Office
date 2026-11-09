import { test, expect, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The Docs (wordcraft) host bridge, PLAN phase 5 WP6: read-only open, the save acknowledgement and the periodic autosave over the
// ?host=parent postMessage protocol, against the built docs-web dist, from a tiny same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-docs.config.ts
// Env: DOCS_DIST (default E:/office-target/p1/dist/docs-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.DOCS_DIST ?? 'E:/office-target/p1/dist/docs-web');
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
const NAME = path.basename(DOCX);
const QUIET_MS = 3_000; // autosave.rs QUIET_MS: the quiet time after the last edit, and the least gap between two autosaves
const MAX_MS = 30_000; // autosave.rs MAX_MS: the longest the first unsent edit waits during continuous editing
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<script>
  window.__msgs = [];
  addEventListener('message', (e) => {
    if (e.source !== document.getElementById('ed').contentWindow) return;
    const d = e.data || {};
    window.__msgs.push({ type: d.type, name: d.name, dirty: d.dirty, error: d.error, t: Date.now(),
      bytes: d.bytes instanceof ArrayBuffer ? new Uint8Array(d.bytes).slice(0, 2).join(',') + ':' + d.bytes.byteLength : undefined });
  });
  window.__toEditor = (msg, transfer) => document.getElementById('ed').contentWindow.postMessage(msg, location.origin, transfer);
</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
    const file = path.join(DIST, url.pathname === '/' ? 'index.html' : url.pathname);
    if (!file.startsWith(DIST) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) return void res.writeHead(404).end();
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
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'wordcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}, file = DOCX) {
  const n = await count(page, 'wordcraft:opened');
  await page.evaluate(([nm, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'wordcraft:open', name: nm, bytes: buf, ...(x as object) }, [buf]);
  }, [path.basename(file), Array.from(fs.readFileSync(file)), extra] as const);
  await waitFor(page, 'wordcraft:opened', n + 1);
}
async function type(page: Page) {
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await page.keyboard.type('zz');
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'wordcraft:ready');
});

test('1 control: an editable document becomes dirty when typed in', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('2 readOnly: typing is refused, the document never becomes dirty', async ({ page }) => {
  await open(page, { readOnly: true });
  await type(page);
  await page.keyboard.press('Control+v');
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'wordcraft:dirty' && m.dirty)).toHaveLength(0);
  // a save request is ignored
  await send(page, { type: 'wordcraft:requestSave' });
  await page.waitForTimeout(2000);
  expect(await count(page, 'wordcraft:save')).toBe(0);
});

test('3 a second open without readOnly is editable again', async ({ page }) => {
  await open(page, { readOnly: true });
  await open(page);
  await waitFor(page, 'wordcraft:opened', 2);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('4 a failed save keeps the dirty mark; ok:true clears it', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save');
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await send(page, { type: 'wordcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save', 2);
  await send(page, { type: 'wordcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('5 autosave: handed over 3 s after the last edit while dirty, once per change, nothing while clean', async ({ page }) => {
  test.setTimeout(120_000);
  await open(page);
  await page.waitForTimeout(QUIET_MS + 3000);
  expect(await count(page, 'wordcraft:autosave')).toBe(0);
  const t0 = Date.now();
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  expect(await count(page, 'wordcraft:autosave')).toBe(0); // not at once: the quiet time comes first
  await waitFor(page, 'wordcraft:autosave', 1, QUIET_MS + 8000);
  const a = (await msgs(page)).find((m) => m.type === 'wordcraft:autosave');
  expect(a.t - t0).toBeGreaterThanOrEqual(QUIET_MS - 500);
  expect(a.name).toBe(NAME);
  expect(a.bytes).toMatch(/^80,75:\d{4,}/);
  expect(await lastDirty(page)).toBe(true); // an autosave is not a save
  // Unchanged since: no second copy, however long the page stays dirty.
  await page.waitForTimeout(QUIET_MS * 2 + 2000);
  expect(await count(page, 'wordcraft:autosave')).toBe(1);
});

test('5b autosave: continuous editing hands over at least every 30 s, never twice within 3 s', async ({ page }) => {
  test.setTimeout(150_000);
  await open(page);
  await canvas(page).click({ position: { x: 400, y: 160 } });
  const t0 = Date.now();
  while (Date.now() - t0 < MAX_MS + 6000) {
    await page.keyboard.type('z');
    await page.waitForTimeout(1000);
  }
  const sent = (await msgs(page)).filter((m) => m.type === 'wordcraft:autosave');
  expect(sent.length).toBeGreaterThanOrEqual(1);
  expect(sent[0].t - t0).toBeGreaterThanOrEqual(MAX_MS - 3000);
  expect(sent[0].t - t0).toBeLessThanOrEqual(MAX_MS + 4000);
  for (let k = 1; k < sent.length; k++) expect(sent[k].t - sent[k - 1].t).toBeGreaterThanOrEqual(QUIET_MS - 200);
});

// Patch 0128: canDownload / canPrint withhold the editor's own Save As, exports and Print (default true). Probed with the
// keyboard: F12 is Save As (a download, so a wordcraft:save message), Ctrl+P opens the Print page (the whole frame repaints).
const lumaOf = async (page: Page) => {
  const png = (await page.locator('#ed').screenshot()).toString('base64');
  return page.evaluate(async (b64) => {
    const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
    const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
    const g = c.getContext('2d')!; g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data; let s = 0;
    for (let i = 0; i < d.length; i += 4) s += d[i] + d[i + 1] + d[i + 2];
    return s / (d.length / 4) / 3;
  }, png);
};
async function probe(page: Page) {
  await canvas(page).click({ position: { x: 400, y: 160 } });
  const before = await lumaOf(page);
  await page.keyboard.press('Control+p');
  await page.waitForTimeout(2500);
  const printShift = Math.abs((await lumaOf(page)) - before);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await page.keyboard.press('F12');
  await page.waitForTimeout(2500);
  return { printShift, saves: await count(page, 'wordcraft:save') };
}

test('6 canDownload/canPrint default true: Print opens and Save As reaches the host', async ({ page }) => {
  await open(page);
  const r = await probe(page);
  expect(r.printShift).toBeGreaterThan(5);
  expect(r.saves).toBeGreaterThan(0);
});

test('7 canDownload:false and canPrint:false: Print does not open, Save As sends nothing, Save still works', async ({ page }) => {
  await open(page, { canDownload: false, canPrint: false });
  const r = await probe(page);
  expect(r.printShift).toBeLessThan(1);
  expect(r.saves).toBe(0);
  await type(page);
  await send(page, { type: 'wordcraft:requestSave' });
  await waitFor(page, 'wordcraft:save'); // the host's own save is not a download
});

test('8 the flags are per open: a later open without them restores both', async ({ page }) => {
  await open(page, { canDownload: false, canPrint: false });
  await open(page);
  await waitFor(page, 'wordcraft:opened', 2);
  const r = await probe(page);
  expect(r.printShift).toBeGreaterThan(5);
  expect(r.saves).toBeGreaterThan(0);
});

// Patch 0152: a read-only document whose whole body is an exception everyone may edit (w:permStart edGrp="everyone", made from
// the 01-split-runs fixture). Restrict Editing alone lets such ranges be typed in; a host read-only open refuses them before they
// run, and Stop Protection and Undo too. Checked by the dirty messages and by the pixels of the frame above the status bar (caret
// blink is a few dozen).
const EXCEPTION = path.join(ROOT, 'ci/web-smoke/fixtures/docs-exception-everyone.docx');
const shot = async (page: Page) => (await page.locator('#ed').screenshot()).toString('base64');
const pixelsChanged = (page: Page, a: string, b: string) => page.evaluate(async ([x, y]) => {
  const px = async (b64: string) => {
    const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
    const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
    const g = c.getContext('2d')!; g.drawImage(img, 0, 0); return g.getImageData(0, 0, c.width, c.height).data;
  };
  // the page area only: the status bar's refusal message (the bottom 40 px) is no change of the document
  const [p, q] = [await px(x), await px(y)]; let n = 0;
  const end = Math.max(0, p.length - 40 * 1100 * 4);
  for (let i = 0; i < end; i += 4) if (Math.abs(p[i] - q[i]) + Math.abs(p[i + 1] - q[i + 1]) + Math.abs(p[i + 2] - q[i + 2]) > 48) n++;
  return n;
}, [a, b] as const);
async function editException(page: Page) {
  await canvas(page).click({ position: { x: 400, y: 160 } });
  await page.waitForTimeout(800);
  const before = await shot(page);
  await page.keyboard.type('zzzzzz');
  await page.keyboard.press('Enter');
  await page.keyboard.type('zzzzzz');
  await page.waitForTimeout(300);
  const soon = await pixelsChanged(page, before, await shot(page));
  await page.waitForTimeout(1000);
  return { soon, later: await pixelsChanged(page, before, await shot(page)) };
}

test('9 control: an editable document with an exception changes when typed in', async ({ page }) => {
  await open(page, {}, EXCEPTION);
  const r = await editException(page);
  expect(r.later).toBeGreaterThan(300);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('10 readOnly with an exception everyone may edit: typing, paste and undo are refused before they show (Stop Protection: tests 11 and 12)', async ({ page }) => {
  const logs: string[] = [];
  page.on('console', (m) => logs.push(m.text()));
  await open(page, { readOnly: true }, EXCEPTION);
  const r = await editException(page);
  expect(r.soon).toBeLessThan(150);
  expect(r.later).toBeLessThan(150);
  await page.keyboard.press('Control+v');
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+y');
  await page.waitForTimeout(2000);
  expect((await msgs(page)).filter((m) => m.type === 'wordcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'wordcraft:save')).toBe(0);
  expect(await count(page, 'wordcraft:autosave')).toBe(0);
  expect(logs.filter((l) => l.includes('read-only safety net'))).toHaveLength(0);
});

// Stop Protection from the Restrict Editing pane (patch 0152; the engine test is `host_read_only_refuses_the_exceptions_and_stop_protection`).
// The fixture is 01-split-runs with `w:documentProtection w:edit="readOnly" w:enforcement="1"` and no password, so an editable session may
// lift the protection. The pane opens from the command search (it shows "Stop Protection" while the protection is on).
const PROTECTED = path.join(ROOT, 'ci/web-smoke/fixtures/docs-protected-no-password.docx');
// The frame's page area (left of the pane) and the pane above its buttons: a refusal prints its reason lower in the pane and in the status bar.
const DOC_CLIP = { x: 48, y: 216, width: 780, height: 440 };
const PANE_TOP_CLIP = { x: 840, y: 180, width: 260, height: 100 };
const shotClip = async (page: Page, clip: typeof DOC_CLIP) => (await page.screenshot({ clip })).toString('base64');
async function stopProtection(page: Page) {
  await page.waitForTimeout(3000); // the first frames: fonts and the opened document
  await canvas(page).click({ position: { x: 912, y: 9 } }); // the command search
  await page.keyboard.type('Restrict', { delay: 40 });
  await page.waitForTimeout(500);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1500);
  const [before, docBefore, paneBefore] = [await shot(page), await shotClip(page, DOC_CLIP), await shotClip(page, PANE_TOP_CLIP)];
  await canvas(page).click({ position: { x: 896, y: 309 } }); // Stop Protection (English UI: the Russian one wraps the button text)
  await page.waitForTimeout(2300);
  return {
    frame: await pixelsChanged(page, before, await shot(page)),
    doc: await pixelsChanged(page, docBefore, await shotClip(page, DOC_CLIP)),
    pane: await pixelsChanged(page, paneBefore, await shotClip(page, PANE_TOP_CLIP)),
  };
}

test('11 control: Stop Protection in an editable session lifts the protection (the pane changes, the document is dirty)', async ({ page }) => {
  await open(page, {}, PROTECTED);
  const r = await stopProtection(page);
  expect(r.frame).toBeGreaterThan(300);
  await expect.poll(() => lastDirty(page)).toBe(true);
});

test('12 readOnly: Stop Protection is refused before it shows (the pane stays, never dirty)', async ({ page }) => {
  const logs: string[] = [];
  page.on('console', (m) => logs.push(m.text()));
  await open(page, { readOnly: true }, PROTECTED);
  const r = await stopProtection(page);
  expect(r.doc).toBeLessThan(150);
  expect(r.pane).toBeLessThan(50); // the pane still offers Stop Protection (a refusal only prints its reason below)
  // still protected: typing in the document is refused too
  await type(page);
  await page.waitForTimeout(1500);
  expect((await msgs(page)).filter((m) => m.type === 'wordcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'wordcraft:save')).toBe(0);
  expect(await count(page, 'wordcraft:autosave')).toBe(0);
  expect(logs.filter((l) => l.includes('read-only safety net'))).toHaveLength(0);
});

// window.__hostFinalCopy (patches/wordcraft/0164): the parent's synchronous pull as it closes the session (the document server's
// wrapper, in its pagehide). Null while clean or read only; the unsaved document once per change, which counts as a handover (the
// autosave does not send it again). It runs inside the parent's pagehide, so its cost is measured: a ~40 KB and two larger documents
// made from the uz108 fixture (whole top-level body elements, kept or repeated).
const UZ108 = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx');
const PYTHON = process.env.PYTHON ?? 'python';
const SYNTH = `import re,sys,zipfile
src,dst,keep,rep=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4])
zi=zipfile.ZipFile(src);zo=zipfile.ZipFile(dst,'w',zipfile.ZIP_DEFLATED)
for i in zi.infolist():
  b=zi.read(i.filename)
  if i.filename=='word/document.xml':
    s=b.decode('utf8');a=s.index('<w:body>')+8;e=s.rindex('<w:sectPr');d=0;start=a;kids=[]
    for m in re.finditer(r'<(/?)[A-Za-z][^>]*?(/?)>',s[a:e]):
      if m.group(1): d-=1
      elif not m.group(2): d+=1
      if d==0: kids.append(s[start:a+m.end()]);start=a+m.end()
    out=[];n=0
    for k in kids:
      if n>=keep: break
      out.append(k);n+=len(k)
    b=(s[:a]+''.join(out)*rep+s[e:]).encode('utf8')
  zo.writestr(i.filename,b)
zo.close()`;
const finalCopy = (page: Page) => page.frame({ url: /index\.html/ })!.evaluate(() => {
  const t = performance.now();
  const b = (window as any).__hostFinalCopy() as Uint8Array | null;
  return { ms: performance.now() - t, n: b ? b.length : null, pk: b ? b[0] === 0x50 && b[1] === 0x4b : null };
});

test('13 __hostFinalCopy: null while clean, the unsaved document once per change (the autosave does not resend it); its cost', async ({ page }) => {
  test.setTimeout(600_000);
  // read only: never
  await open(page, { readOnly: true });
  await type(page);
  await page.waitForTimeout(1500);
  expect((await finalCopy(page)).n).toBeNull();
  await open(page);
  expect((await finalCopy(page)).n).toBeNull(); // clean
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  let r = await finalCopy(page);
  expect([r.pk, r.n! > 1000]).toEqual([true, true]);
  expect((await finalCopy(page)).n).toBeNull(); // handed over: not twice
  await page.waitForTimeout(QUIET_MS + 2000);
  expect(await count(page, 'wordcraft:autosave')).toBe(0); // the pull was the handover of that state
  await page.keyboard.type('q');
  await expect.poll(async () => (r = await finalCopy(page)).n).not.toBeNull(); // a new change: once more
  // saved (acknowledged) before each next open, which a dirty document would hold back
  const save = async () => {
    const n = await count(page, 'wordcraft:save');
    await send(page, { type: 'wordcraft:requestSave' });
    await waitFor(page, 'wordcraft:save', n + 1);
    await send(page, { type: 'wordcraft:saved', ok: true });
    await expect.poll(() => lastDirty(page)).toBe(false);
  };
  await save();

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'final-copy-'));
  const files: [string, string][] = [['01-split-runs', DOCX]];
  for (const [label, keep, rep] of [['uz108 part', 185_000, 1], ['uz108', 1 << 30, 1], ['uz108 x4', 1 << 30, 4]] as const) {
    const f = path.join(dir, label.replace(/ /g, '-') + '.docx');
    execFileSync(PYTHON, ['-I', '-c', SYNTH, UZ108, f, String(keep), String(rep)]);
    files.push([label, f]);
  }
  for (const [label, f] of files) {
    await open(page, {}, f);
    const ms: number[] = [];
    let n = 0;
    for (let i = 0; i < 3; i++) {
      await type(page);
      await expect.poll(async () => (r = await finalCopy(page)).n, { timeout: 60_000 }).not.toBeNull();
      ms.push(r.ms); n = r.n!;
    }
    await save();
    ms.sort((a, b) => a - b);
    console.log(`MEASURED __hostFinalCopy ${label}: file ${fs.statSync(f).size} B, copy ${n} B, ${ms.map((m) => m.toFixed(1)).join(' / ')} ms (median ${ms[1].toFixed(1)})`);
  }
  fs.rmSync(dir, { recursive: true, force: true });
});
