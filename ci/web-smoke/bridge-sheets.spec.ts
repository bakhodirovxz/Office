import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The Sheets (gridcraft) host bridge, docs/phase5/r6.md (patches gridcraft/0160, 0161-0163): open/opened, read-only open, the save
// acknowledgement and canPrint over the ?host=parent postMessage protocol, against the built sheets-web dist, from a tiny
// same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-sheets.config.ts
// Env: SHEETS_DIST (default E:/office-target/p1/dist/sheets-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.SHEETS_DIST ?? 'E:/office-target/p1/dist/sheets-web');
const XLSX = path.join(ROOT, 'domains/sheets/tests/translit-fixtures/xlsx/01-styles.xlsx');
const NAME = path.basename(XLSX);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<iframe id="sib" src="/sibling.html" style="width:10px;height:10px;border:0"></iframe>
<script>
  window.__msgs = [];
  addEventListener('message', (e) => {
    if (e.source !== document.getElementById('ed').contentWindow) return;
    const d = e.data || {};
    window.__msgs.push({ t: Date.now(), type: d.type, name: d.name, dirty: d.dirty, error: d.error, version: d.version,
      bytes: d.bytes instanceof ArrayBuffer ? new Uint8Array(d.bytes).slice(0, 2).join(',') + ':' + d.bytes.byteLength : undefined });
  });
  window.__toEditor = (msg, transfer) => document.getElementById('ed').contentWindow.postMessage(msg, location.origin, transfer);
</script>`;
const SIBLING = `<!doctype html><script>window.__post = (msg) => parent.document.getElementById('ed').contentWindow.postMessage(msg, location.origin);</script>`;

let server: http.Server;
let base: string;

test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const url = new URL(req.url!, 'http://x');
    if (url.pathname === '/host.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
    if (url.pathname === '/sibling.html') return void res.writeHead(200, { 'content-type': 'text/html' }).end(SIBLING);
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
const waitFor = (page: Page, type: string, n = 1) => expect.poll(() => count(page, type), { timeout: 90_000 }).toBeGreaterThanOrEqual(n);
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'gridcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}, name = NAME, bytes = fs.readFileSync(XLSX)) {
  const n = await count(page, 'gridcraft:opened');
  await page.evaluate(([nm, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'gridcraft:open', name: nm, bytes: buf, ...(x as object) }, [buf]);
  }, [name, Array.from(bytes), extra] as const);
  if (name === NAME) await waitFor(page, 'gridcraft:opened', n + 1);
}
async function type(page: Page) {
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.type('zz');
  await page.keyboard.press('Enter');
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'gridcraft:ready');
});

test('1 ready once with a version, then open: shown, announced by name, editable', async ({ page }) => {
  const m = await msgs(page);
  expect(m.filter((x) => x.type === 'gridcraft:ready')).toHaveLength(1);
  expect(m.find((x) => x.type === 'gridcraft:ready').version).toMatch(/^\d+\.\d+\.\d+/);
  await open(page);
  expect((await msgs(page)).find((x) => x.type === 'gridcraft:opened').name).toBe(NAME);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  expect(await count(page, 'gridcraft:failed')).toBe(0);
});

test('2 save acknowledgement: unanswered and ok:false keep the dirty mark, ok:true clears it', async ({ page }) => {
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'gridcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^80,75:\d{4,}/); // "PK", a zip
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await send(page, { type: 'gridcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save', 2);
  await send(page, { type: 'gridcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('3 readOnly: an edit is taken back, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  await open(page, { readOnly: true });
  await type(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Control+s');
  await send(page, { type: 'gridcraft:requestSave' });
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'gridcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'gridcraft:save')).toBe(0);
});

test('4 a second open without readOnly replaces the first and is editable again', async ({ page }) => {
  await open(page, { readOnly: true });
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save');
});

// Print hands a PDF to a hidden iframe of the editor page (web.rs print_pdf); with canPrint:false nothing is handed over.
const printFrames = (page: Page) => page.frameLocator('#ed').locator('iframe').count();
test('5 canPrint default true: Ctrl+P prints; canPrint:false refuses it and Save still works', async ({ page }) => {
  await open(page);
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.press('Control+p');
  await expect.poll(() => printFrames(page), { timeout: 20_000 }).toBeGreaterThan(0);
  await page.goto(base + '/host.html');
  await waitFor(page, 'gridcraft:ready');
  await open(page, { canPrint: false, canDownload: false });
  await canvas(page).click({ position: { x: 420, y: 420 } });
  await page.keyboard.press('Control+p');
  await page.waitForTimeout(4000);
  expect(await printFrames(page)).toBe(0);
  await page.keyboard.press('Enter'); // the "not available" message box
  await type(page);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save'); // the host's own save is not a download
});

// (a plain text file named .xlsx opens as text, as in Excel's import; a damaged package does not open)
test('6 a damaged workbook: failed, no opened, and the editor keeps going', async ({ page }) => {
  await open(page, {}, 'broken.xlsx', Buffer.concat([Buffer.from([0x50, 0x4b, 3, 4]), Buffer.alloc(64, 7)]));
  await waitFor(page, 'gridcraft:failed');
  expect(await count(page, 'gridcraft:opened')).toBe(0);
  await page.keyboard.press('Enter');
  await open(page);
});

test('7 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'gridcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'gridcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(XLSX))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'gridcraft:opened')).toBe(0);
  expect(await count(page, 'gridcraft:save')).toBe(0);
});

// ---- autosave (patch gridcraft/0163): `gridcraft:autosave {name, bytes}` every 30 s while the workbook is dirty ----
const AUTOSAVE_MS = 30_000;
const lastOf = async (page: Page, type: string) => (await msgs(page)).filter((m) => m.type === type).at(-1);

test('8 autosave: nothing while clean; handed over while dirty after the period; no save, still dirty', async ({ page }) => {
  test.setTimeout(240_000);
  await open(page);
  await page.waitForTimeout(AUTOSAVE_MS + 5_000);
  expect(await count(page, 'gridcraft:autosave')).toBe(0);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await expect.poll(() => count(page, 'gridcraft:autosave'), { timeout: AUTOSAVE_MS + 20_000 }).toBeGreaterThanOrEqual(1);
  const a = await lastOf(page, 'gridcraft:autosave');
  expect(a.name).toBe(NAME);
  expect(a.bytes).toMatch(/^80,75:\d{4,}/); // "PK", a zip
  expect(await lastDirty(page)).toBe(true); // an autosave is not a save
  expect(await count(page, 'gridcraft:save')).toBe(0);
});

test('9 autosave: none in a read-only session', async ({ page }) => {
  test.setTimeout(240_000);
  await open(page, { readOnly: true });
  await type(page);
  await page.waitForTimeout(AUTOSAVE_MS + 15_000);
  expect(await count(page, 'gridcraft:autosave')).toBe(0);
  expect(await count(page, 'gridcraft:save')).toBe(0);
});

test('10 autosave: none while a save is unanswered; it resumes after saved ok:false', async ({ page }) => {
  test.setTimeout(300_000);
  await open(page);
  await type(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'gridcraft:requestSave' });
  await waitFor(page, 'gridcraft:save');
  await page.waitForTimeout(AUTOSAVE_MS + 10_000);
  expect(await count(page, 'gridcraft:autosave')).toBe(0);
  await send(page, { type: 'gridcraft:saved', ok: false });
  await expect.poll(() => count(page, 'gridcraft:autosave'), { timeout: AUTOSAVE_MS + 20_000 }).toBeGreaterThanOrEqual(1);
  expect(await lastDirty(page)).toBe(true);
});


async function editCell(page: Page, i: number) {
  await canvas(page).click({ position: { x: 420, y: 300 + (i % 6) * 18 } });
  await page.keyboard.type(`v${i}`);
  await page.keyboard.press('Enter');
}
const sheetsAutosaves = (page: Page) => count(page, 'gridcraft:autosave');
// hengines (patch gridcraft/0170): the recovery copy goes 3 s after the last edit, and at the latest 30 s after the first unsent edit
// while editing goes on; never more often than every 3 s; none for an unchanged document.
test('8b autosave: 3 s after the last edit, once per quiet period; continuous editing is saved within 30 s', async ({ page }) => {
  test.setTimeout(180_000);
  await open(page);
  await waitFor(page, 'gridcraft:opened');
  const n = async () => (await sheetsAutosaves(page));
  expect(await n()).toBe(0);
  await editCell(page, 0);
  await expect.poll(() => lastDirty(page)).toBe(true);
  const t0 = Date.now();
  await expect.poll(n, { timeout: 12_000 }).toBe(1);
  expect(Date.now() - t0, 'a copy about 3 s after the edit, not 30 s').toBeLessThan(12_000);
  await page.waitForTimeout(8_000);
  expect(await n(), 'an unchanged document is not sent again').toBe(1);
  // continuous editing (edits back to back, well under 3 s apart): nothing while the edits keep coming for 20 s, a copy by the 30 s ceiling
  const start = Date.now();
  let i = 1;
  while (Date.now() - start < 20_000) { await editCell(page, i++); await page.waitForTimeout(500); }
  expect(await n(), 'no copy while the edits are less than 3 s apart (before the 30 s ceiling)').toBe(1);
  while (Date.now() - start < 38_000) { await editCell(page, i++); await page.waitForTimeout(500); }
  const during = await n();
  expect(during, 'a copy by the 30 s ceiling although the edits never paused for 3 s').toBeGreaterThanOrEqual(2);
  const times = (await msgs(page)).filter((m) => m.type === 'gridcraft:autosave').map((m) => m.t as number);
  for (let k = 1; k < times.length; k++) expect(times[k] - times[k - 1], 'never more often than every 3 s').toBeGreaterThanOrEqual(2_800);
  // the edits stop: the last state goes 3 s later
  await expect.poll(n, { timeout: 12_000 }).toBeGreaterThan(during);
  await page.waitForTimeout(8_000);
  const after = await n();
  await page.waitForTimeout(5_000);
  expect(await n(), 'quiet again: nothing more').toBe(after);
  expect(await lastDirty(page)).toBe(true);
});

// ---- read-only: nothing can change the workbook, not even for one frame (patches gridcraft/0161, 0162) ----
// Canvas-relative points in the 1100x700 editor (01-styles.xlsx: C2 holds 1234,50).
const C2 = { x: 155, y: 163 };
const CLIP_ALL = { x: 0, y: 0, width: 1100, height: 560 }; // the toast sits below this
const CLIP_GRID = { x: 0, y: 120, width: 1100, height: 440 };
async function shot(page: Page, clip = CLIP_ALL) {
  const box = (await canvas(page).boundingBox())!;
  return page.screenshot({ clip: { x: box.x + clip.x, y: box.y + clip.y, width: clip.width, height: clip.height }, animations: 'disabled' });
}
async function mouse(page: Page, steps: Array<['down' | 'up' | 'move', number, number]>) {
  const box = (await canvas(page).boundingBox())!;
  for (const [k, x, y] of steps) {
    if (k === 'move') await page.mouse.move(box.x + x, box.y + y, { steps: 6 });
    else if (k === 'down') { await page.mouse.move(box.x + x, box.y + y); await page.mouse.down(); }
    else await page.mouse.up();
  }
}
const drag = (page: Page, x0: number, y0: number, x1: number, y1: number) =>
  mouse(page, [['down', x0, y0], ['move', (x0 + x1) / 2, (y0 + y1) / 2], ['move', x1, y1], ['up', x1, y1]]);
const clickC2 = (page: Page) => canvas(page).click({ position: C2 });

// `ro`: the session is read only. Enter after typing is only for the editable control: in a read-only one it would just move the selection.
// fx: a fixture under ci/web-smoke/ to open instead of 01-styles.xlsx.
type Path = { name: string; fx?: string; pixels?: boolean; act: (p: Page, ro: boolean) => Promise<void>; clip?: typeof CLIP_ALL; control?: boolean; prep?: (p: Page) => Promise<void> };
const PATHS: Path[] = [
  { name: 'typing into a cell', act: async (p, ro) => { await p.keyboard.type('zz'); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'F2 and typing', act: async (p, ro) => { await p.keyboard.press('F2'); await p.keyboard.type('zz'); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'double-click and typing', act: async (p, ro) => { await canvas(p).dblclick({ position: C2 }); await p.keyboard.type('zz'); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'the formula bar', act: async (p, ro) => { await canvas(p).click({ position: { x: 500, y: 102 } }); await p.keyboard.type('zz'); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'Delete', act: (p) => p.keyboard.press('Delete') },
  { name: 'Backspace', act: async (p, ro) => { await p.keyboard.press('Backspace'); await p.keyboard.type('q'); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'Ctrl+V', act: (p) => p.keyboard.press('Control+v'), prep: (p) => p.evaluate(() => navigator.clipboard.writeText('pasted')) },
  { name: 'Ctrl+X', act: async (p, ro) => { await p.keyboard.press('Control+x'); if (!ro) await p.keyboard.press('ArrowDown'); await p.keyboard.press('Control+v'); } },
  { name: 'Ctrl+Z and Ctrl+Y', act: async (p) => { await p.keyboard.press('Control+z'); await p.keyboard.press('Control+y'); }, control: false },
  { name: 'context menu: Clear Contents', act: async (p) => { await canvas(p).click({ position: C2, button: 'right' }); await canvas(p).click({ position: { x: 258, y: 347 } }); } },
  // the menu opens at the click: Sort Z to A is 103 px right and 273 px below it
  { name: 'context menu: Sort', act: async (p) => { await canvas(p).click({ position: C2, button: 'right' }); await canvas(p).click({ position: { x: 258, y: 436 } }); } },
  { name: 'ribbon: Bold', act: async (p) => { await canvas(p).click({ position: { x: 240, y: 62 } }); await p.mouse.move(700, 400); } },
  { name: 'ribbon: fill colour', act: async (p) => { await canvas(p).click({ position: { x: 375, y: 62 } }); await p.mouse.move(700, 400); }, clip: CLIP_GRID },
  { name: 'dragging the fill handle', act: (p) => drag(p, 181, 171, 181, 230), clip: CLIP_GRID },
  { name: 'dragging the selection by its border', act: (p) => drag(p, 130, 163, 330, 163), clip: CLIP_GRID },
  { name: 'dragging a column border', act: (p) => drag(p, 233, 128, 330, 128), clip: CLIP_GRID },
  { name: 'dragging a row border', act: (p) => drag(p, 14, 171, 14, 200), clip: CLIP_GRID },
  { name: 'double-click on a column border (AutoFit)', act: (p) => canvas(p).dblclick({ position: { x: 233, y: 128 } }), clip: CLIP_GRID },
  // Editors that are not the cell editor (gridcraft/0162 and 0164): the note/comment dialog, the header/footer editor, the text box editor.
  { name: 'the note dialog (Shift+F2) and typing in it', act: async (p, ro) => {
      await p.keyboard.press('Shift+F2');
      await p.waitForTimeout(700);
      if (ro) await p.keyboard.type('zz'); // no dialog opens: nothing to click, and a click on the grid would move the selection
      else { await canvas(p).click({ position: { x: 527, y: 192 } }); await p.keyboard.type('zz'); await canvas(p).click({ position: { x: 708, y: 374 } }); }
    } },
  { name: 'the header editor in Page Layout view', prep: async (p) => { await canvas(p).click({ position: { x: 839, y: 688 } }); await p.waitForTimeout(1500); },
    act: async (p, ro) => {
      await canvas(p).click({ position: { x: 380, y: 195 } });
      await p.keyboard.type('zz');
      if (!ro) await canvas(p).click({ position: { x: 600, y: 500 } }); // leaves the editor: the header is stored
    } },
  { name: 'the text box editor (double-click, typing)', fx: 'fixtures/sheets-text-box.xlsx', prep: async (p) => { await canvas(p).click({ position: { x: 80, y: 180 } }); await p.waitForTimeout(500); },
    act: async (p, ro) => {
      await canvas(p).dblclick({ position: { x: 80, y: 180 } });
      await p.keyboard.type('zz');
      if (!ro) await canvas(p).click({ position: { x: 600, y: 500 } });
    } },
  {
    name: 'dropping a file on the canvas', // a picture is no workbook: the editor answers with its own "can't open" box, so no pixel check
    pixels: false,
    act: async (p) => {
      await canvas(p).evaluate((c) => {
        const dt = new DataTransfer();
        dt.items.add(new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], 'dropped.png', { type: 'image/png' }));
        c.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, clientX: 420, clientY: 420 }));
      });
    },
    control: false,
  },
];

async function session(page: Page, readOnly: boolean, p: Path) {
  const logs: string[] = [];
  page.on('console', (m) => logs.push(m.text()));
  await open(page, readOnly ? { readOnly: true } : {}, NAME, p.fx ? fs.readFileSync(path.join(ROOT, 'ci/web-smoke', p.fx)) : fs.readFileSync(XLSX));
  await clickC2(page);
  await page.waitForTimeout(1200);
  if (p.prep) { await p.prep(page); await page.waitForTimeout(800); }
  return logs;
}

for (const [i, p] of PATHS.entries()) {
  test(`${11 + i} read-only: ${p.name} changes nothing, not even for a frame`, async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    const logs = await session(page, true, p);
    const clip = p.clip ?? CLIP_ALL;
    const base = await shot(page, clip);
    await p.act(page, true);
    const right = await shot(page, clip); // the frame right after the input
    await page.waitForTimeout(1200);
    const later = await shot(page, clip);
    if (p.pixels !== false) {
      expect(right.equals(base), 'a frame right after the input differs').toBe(true);
      expect(later.equals(base), 'the canvas differs a second after the input').toBe(true);
    }
    expect((await msgs(page)).filter((m) => m.type === 'gridcraft:dirty' && m.dirty)).toHaveLength(0);
    expect(logs.filter((l) => l.includes('read-only safety net'))).toHaveLength(0);
    expect(await count(page, 'gridcraft:save')).toBe(0);
    expect(await count(page, 'gridcraft:autosave')).toBe(0);
  });
}

// The control: the same input in an editable session does change the workbook (so the checks above are not vacuous).
for (const p of PATHS.filter((x) => x.control !== false)) {
  test(`control: ${p.name} in an editable session makes it dirty and changes the canvas`, async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await session(page, false, p);
    const clip = p.clip ?? CLIP_ALL;
    const base = await shot(page, clip);
    await p.act(page, false);
    await expect.poll(() => lastDirty(page), { timeout: 10_000 }).toBe(true);
    await page.waitForTimeout(800);
    expect((await shot(page, clip)).equals(base)).toBe(false);
  });
}

test('read-only: view-only actions still work (selection, copy)', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const logs = await session(page, true, { name: 'view', act: async () => {} });
  const base = await shot(page, CLIP_GRID);
  await page.keyboard.press('Control+c');
  await page.keyboard.press('ArrowDown');
  await page.waitForTimeout(800);
  expect((await shot(page, CLIP_GRID)).equals(base)).toBe(false); // the selection moved
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain('1234');
  expect((await msgs(page)).filter((m) => m.type === 'gridcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(logs.filter((l) => l.includes('read-only safety net'))).toHaveLength(0);
});
