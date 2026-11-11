import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The PDF (pdfcraft) host bridge, docs/phase5/r6.md (patches pdfcraft/0122, 0123-0124): open/opened, read-only open (view only:
// no input path changes the document), the 30 s autosave, the save acknowledgement and canPrint over the ?host=parent postMessage protocol, against the built pdf-web dist, from a tiny
// same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-pdf.config.ts
// Env: PDF_DIST (default E:/office-target/p1/dist/pdf-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.PDF_DIST ?? 'E:/office-target/p1/dist/pdf-web');
const PDF = path.join(ROOT, 'docs/phase4/wp23/fixtures/namuna-hisobot.pdf');
const FORM = path.join(ROOT, 'upstream/pdfcraft/crates/ui-egui/tests/data/form.pdf');
const NAME = path.basename(PDF);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

const HOST = `<!doctype html><meta charset="utf-8"><body>
<iframe id="ed" src="/index.html?host=parent" style="width:1100px;height:700px;border:0"></iframe>
<iframe id="sib" src="/sibling.html" style="width:10px;height:10px;border:0"></iframe>
<script>
  window.__msgs = [];
  addEventListener('message', (e) => {
    if (e.source !== document.getElementById('ed').contentWindow) return;
    const d = e.data || {};
    if (d.type === 'pdfcraft:save' && d.bytes instanceof ArrayBuffer) window.__lastSave = Array.from(new Uint8Array(d.bytes));
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
const lastDirty = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty').map((m) => m.dirty).at(-1);
const send = (page: Page, msg: object) => page.evaluate((m) => (window as any).__toEditor(m), msg);
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');

async function open(page: Page, extra: object = {}, name = NAME, file: string | number[] = PDF) {
  const n = await count(page, 'pdfcraft:opened');
  await page.evaluate(([nm, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'pdfcraft:open', name: nm, bytes: buf, ...(x as object) }, [buf]);
  }, [name, typeof file === 'string' ? Array.from(fs.readFileSync(file)) : file, extra] as const);
  await waitFor(page, 'pdfcraft:opened', n + 1);
}
// an edit with the keyboard: the command palette (Ctrl+K) runs "Rotate pages clockwise"
async function edit(page: Page) {
  await canvas(page).click({ position: { x: 550, y: 400 } });
  await page.keyboard.press('Control+k');
  await page.waitForTimeout(500);
  await page.keyboard.type('Rotate pages clockwise');
  await page.waitForTimeout(300);
  await page.keyboard.press('Enter');
}
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

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'pdfcraft:ready');
});

test('1 open: shown and announced by name; an edit makes it dirty', async ({ page }) => {
  await open(page);
  expect((await msgs(page)).find((x) => x.type === 'pdfcraft:opened').name).toBe(NAME);
  await edit(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  expect(await count(page, 'pdfcraft:failed')).toBe(0);
});

test('2 save acknowledgement: unanswered and ok:false keep the dirty mark, ok:true clears it', async ({ page }) => {
  await open(page);
  await edit(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'pdfcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^37,80:\d{4,}/); // "%P"
  expect(Number(s.bytes.split(':')[1])).toBeGreaterThan(fs.statSync(PDF).size); // an incremental update appends
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:saved', ok: false });
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save', 2);
  await send(page, { type: 'pdfcraft:saved', ok: true });
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('3 readOnly: an edit is reverted, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  await open(page, { readOnly: true });
  await edit(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Control+s');
  await send(page, { type: 'pdfcraft:requestSave' });
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'pdfcraft:save')).toBe(0);
});

test('4 canPrint default true opens Print; canPrint:false keeps it closed and Save still works', async ({ page }) => {
  const printShift = async () => {
    await canvas(page).click({ position: { x: 550, y: 400 } });
    await page.waitForTimeout(1000);
    const before = await lumaOf(page);
    await page.keyboard.press('Control+p');
    await page.waitForTimeout(6000); // past the "Not allowed" toast
    return Math.abs((await lumaOf(page)) - before);
  };
  await open(page);
  expect(await printShift()).toBeGreaterThan(3);
  await page.goto(base + '/host.html');
  await waitFor(page, 'pdfcraft:ready');
  await open(page, { canPrint: false, canDownload: false });
  expect(await printShift()).toBeLessThan(1);
  await edit(page);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save'); // the host's own save is not a download
});

test('5 a second open replaces the first document', async ({ page }) => {
  await open(page, {}, 'first.pdf');
  await open(page, {}, 'second.pdf');
  await edit(page);
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  expect((await msgs(page)).filter((x) => x.type === 'pdfcraft:save').map((x) => x.name)).toEqual(['second.pdf']);
});

test('6 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'pdfcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'pdfcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(PDF))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'pdfcraft:opened')).toBe(0);
  expect(await count(page, 'pdfcraft:save')).toBe(0);
});

// ---- read-only sessions: nothing can change the document, not even for one frame (patch pdfcraft/0123) ----
const AUTOSAVE_WAIT = 30_000;
const logs = new WeakMap<Page, string[]>();
test.use({ permissions: ['clipboard-read', 'clipboard-write'] });
test.beforeEach(({ page }) => {
  const l: string[] = [];
  logs.set(page, l);
  page.on('console', (m) => l.push(m.text()));
});

const PAGE_CLIP = { x: 356, y: 124, width: 299, height: 425 };
const THUMB_CLIP = { x: 347, y: 188, width: 128, height: 182 }; // the page thumbnail in Organize pages
const shotBuf = (page: Page, clip = PAGE_CLIP) => page.screenshot({ clip });
// the page raster only: a toast, a hover or a tool panel is not the document
// number of clearly different pixels between two PNGs
const diffOf = (page: Page, a: Buffer, b: Buffer) =>
  page.evaluate(async ([x, y]) => {
    const load = async (b64: string) => { const i = new Image(); i.src = 'data:image/png;base64,' + b64; await i.decode(); const c = document.createElement('canvas'); c.width = i.width; c.height = i.height; const g = c.getContext('2d')!; g.drawImage(i, 0, 0); return g.getImageData(0, 0, c.width, c.height).data; };
    const [p, q] = [await load(x), await load(y)]; let n = 0;
    for (let i = 0; i < p.length; i += 4) if (Math.abs(p[i] - q[i]) + Math.abs(p[i + 1] - q[i + 1]) + Math.abs(p[i + 2] - q[i + 2]) > 40) n++;
    return n;
  }, [a.toString('base64'), b.toString('base64')] as const);

const click = (page: Page, x: number, y: number) => page.mouse.click(x, y);
const drag = async (page: Page, x1: number, y1: number, x2: number, y2: number) => {
  await page.mouse.move(x1, y1); await page.mouse.down(); await page.mouse.move((x1 + x2) / 2, (y1 + y2) / 2, { steps: 5 }); await page.mouse.move(x2, y2, { steps: 5 }); await page.mouse.up();
};
const type = async (page: Page, text: string) => { await page.keyboard.type(text, { delay: 30 }); };
const dropFile = (page: Page, x: number, y: number) => canvas(page).evaluate((el, [px, py]) => {
  const dt = new DataTransfer();
  dt.items.add(new File([new Uint8Array([37, 80, 68, 70])], 'dropped.pdf', { type: 'application/pdf' }));
  const r = el.getBoundingClientRect();
  for (const t of ['dragenter', 'dragover', 'drop']) el.dispatchEvent(new DragEvent(t, { dataTransfer: dt, bubbles: true, cancelable: true, clientX: r.left + px, clientY: r.top + py }));
}, [x, y]);

type Case = { name: string; file?: string; transient?: boolean; thumb?: boolean; clip?: typeof PAGE_CLIP; noSettle?: boolean; setup?: (p: Page) => Promise<void>; input: (p: Page) => Promise<void> };
// coordinates are page coordinates of the 1280x800 viewport (the editor iframe starts at 8,8)
const TOOLS = { select: [321, 131], note: [321, 199], highlight: [321, 233], ink: [321, 267], shape: [321, 301] };
const toolsList = async (p: Page, label: 'organize' | 'edit' | 'fill' | 'comments') => {
  const y = { comments: 440, organize: 400, edit: 280, fill: 240 }[label];
  await click(p, 59, y); await p.waitForTimeout(800);
};
// a sticky note needs its text and Post
const postNote = async (p: Page) => { await click(p, 500, 400); await type(p, 'note'); await click(p, 761, 521); };
const CASES: Case[] = [
  { name: 'ribbon/palette command: Rotate pages clockwise', transient: true, input: edit },
  { name: 'sticky note tool, click on the page', setup: async (p) => { await toolsList(p, 'edit'); await click(p, TOOLS.note[0], TOOLS.note[1]); }, input: postNote },
  { name: 'highlight tool, drag over text', transient: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, TOOLS.highlight[0], TOOLS.highlight[1]); }, input: (p) => drag(p, 392, 174, 520, 176) },
  { name: 'ink tool, drag on the page', transient: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, TOOLS.ink[0], TOOLS.ink[1]); }, input: (p) => drag(p, 400, 350, 560, 420) },
  { name: 'shape tool, drag on the page', transient: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, TOOLS.shape[0], TOOLS.shape[1]); }, input: (p) => drag(p, 400, 350, 560, 420) },
  { name: 'typing, Backspace, Delete on the page', setup: (p) => click(p, 500, 400), input: async (p) => { await type(p, 'hello'); await p.keyboard.press('Backspace'); await p.keyboard.press('Delete'); } },
  { name: 'Edit text & images tool: click text and type', setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 231); }, input: async (p) => { await click(p, 420, 188); await type(p, 'xyz'); } },
  { name: 'Add Text tool: click the page and type', setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 502); }, input: async (p) => { await click(p, 500, 400); await type(p, 'xyz'); await click(p, 621, 414); } },
  { name: 'Ctrl+X on selected text', noSettle: true, setup: async (p) => { await drag(p, 392, 174, 520, 176); }, input: (p) => p.keyboard.press('Control+x') },
  { name: 'Ctrl+V with text on the clipboard', setup: async (p) => { await click(p, 500, 400); await canvas(p).evaluate(() => navigator.clipboard.writeText('pasted text')); }, input: (p) => p.keyboard.press('Control+v') },
  { name: 'Ctrl+Z then Ctrl+Y', setup: (p) => click(p, 500, 400), input: async (p) => { await p.keyboard.press('Control+z'); await p.keyboard.press('Control+y'); await p.keyboard.press('Control+Shift+z'); } },
  { name: 'Organize pages: rotate and delete buttons on the thumbnail', thumb: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 431); await p.waitForTimeout(800); await click(p, 410, 280); }, input: async (p) => { await click(p, 411, 424); await click(p, 440, 424); await click(p, 81, 291); await click(p, 121, 291); } },
  { name: 'Organize pages: context menu on a thumbnail', thumb: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 431); await p.waitForTimeout(800); }, input: async (p) => { await p.mouse.click(410, 280, { button: 'right' }); await p.waitForTimeout(300); await click(p, 440, 330); } },
  { name: 'Organize pages: drag a thumbnail', thumb: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 431); await p.waitForTimeout(800); }, input: (p) => drag(p, 410, 280, 600, 300) },
  { name: 'Delete key on the selected page thumbnail', thumb: true, setup: async (p) => { await toolsList(p, 'edit'); await click(p, 59, 431); await p.waitForTimeout(800); await click(p, 410, 280); }, input: (p) => p.keyboard.press('Delete') },
  { name: 'Delete pages / Rotate pages in the Edit list', setup: (p) => toolsList(p, 'edit'), input: async (p) => { await click(p, 59, 271); await p.waitForTimeout(500); await click(p, 59, 351); } },
  { name: 'form fields: type in a text field, tick the check box, pick a radio button', transient: true, /* the refusal toast sits over the page */ file: FORM, clip: { x: 476, y: 172, width: 394, height: 528 }, setup: async (p) => { await click(p, 959, 115); for (let i = 0; i < 3; i++) { await click(p, 1085, 683); await p.waitForTimeout(400); } await p.waitForTimeout(800); }, input: async (p) => { await click(p, 673, 222); await type(p, 'abc'); await p.keyboard.press('Enter'); await click(p, 510, 280); await click(p, 563, 333); await click(p, 673, 450); await type(p, 'q'); await p.keyboard.press('Enter'); } },
  { name: 'file dropped on the canvas', input: (p) => dropFile(p, 500, 400) },
  // side panels (patch 0123 holds `editable()` false: the panel offers no editor, or refuses with its own message)
  { name: 'Bookmarks panel: double-click a bookmark and type a new name', transient: true, clip: { x: 746, y: 203, width: 310, height: 40 }, input: async (p) => { await p.mouse.dblclick(830, 215); await p.waitForTimeout(400); await type(p, 'ZZZ'); await p.keyboard.press('Enter'); } },
  { name: 'Prepare a form: Text field tool, click the page and type', transient: true, setup: async (p) => { await click(p, 120, 631); await p.waitForTimeout(1200); }, input: async (p) => { await click(p, 120, 654); await p.waitForTimeout(500); await click(p, 500, 400); await p.waitForTimeout(500); await type(p, 'ZZ'); await p.keyboard.press('Escape'); } },
];

async function readOnlyAttempt(page: Page, c: Case) {
  await open(page, { readOnly: true }, path.basename(c.file ?? PDF), c.file ?? PDF);
  await page.waitForTimeout(1500);
  await c.setup?.(page);
  await page.waitForTimeout(500);
  const clip = c.clip ?? (c.thumb ? THUMB_CLIP : PAGE_CLIP);
  const base0 = await shotBuf(page, clip);
  await c.input(page);
  const now = await shotBuf(page, clip);
  await page.waitForTimeout(1200);
  // a view-only effect (text selection, a menu, the palette) may show; once dismissed the page must be as it was
  if (!c.noSettle) {
    await page.keyboard.press('Escape');
    await click(page, 690, 640);
  }
  await page.waitForTimeout(c.noSettle ? 1200 : 4500);
  const later = await shotBuf(page, clip);
  return { d0: await diffOf(page, base0, now), d1: await diffOf(page, base0, later), base0, later };
}

for (const [i, c] of CASES.entries()) {
  test(`7.${i + 1} read-only blocks: ${c.name}`, async ({ page }) => {
    const r = await readOnlyAttempt(page, c);
    console.log(`ro ${i + 1} ${c.name}: diff immediately ${r.d0}, after 1 s ${r.d1}`);
    const m = await msgs(page);
    expect(m.filter((x) => x.type === 'pdfcraft:dirty' && x.dirty), 'no dirty message').toHaveLength(0);
    expect(m.filter((x) => x.type === 'pdfcraft:save' || x.type === 'pdfcraft:autosave'), 'no save').toHaveLength(0);
    expect(logs.get(page)!.filter((t) => t.includes('read-only safety net')), 'the safety net never fired').toHaveLength(0);
    if (!c.transient) expect(r.d0, 'no pixel of the page changed at once').toBe(0);
    expect(r.d1, 'the page is as it was').toBe(0);
  });
}

// the same inputs in an editable session DO change the document (so the read-only checks above are not vacuous)
const CONTROL = [1, 2, 3, 4, 7, 8, 12, 17, 19, 20];
for (const n of CONTROL) {
  const c = CASES[n - 1];
  test(`8.${n} control, editable: ${c.name} makes the document dirty`, async ({ page }) => {
    await open(page, {}, path.basename(c.file ?? PDF), c.file ?? PDF);
    await page.waitForTimeout(1500);
    await c.setup?.(page);
    await page.waitForTimeout(500);
    await c.input(page);
    await expect.poll(() => lastDirty(page), { timeout: 15_000 }).toBe(true);
  });
}

test('9 Delete on a selected annotation: read-only keeps it (the annotation comes from an editable control session)', async ({ page }) => {
  // an editable session adds a sticky note and saves; that file opens read only
  await open(page);
  await toolsList(page, 'edit');
  await click(page, TOOLS.note[0], TOOLS.note[1]);
  await postNote(page);
  await expect.poll(() => lastDirty(page), { timeout: 15_000 }).toBe(true);
  await page.keyboard.press('Escape');
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  const noted: number[] = await page.evaluate(() => (window as any).__lastSave);
  expect(noted.length).toBeGreaterThan(fs.statSync(PDF).size);
  await page.goto(base + '/host.html');
  await waitFor(page, 'pdfcraft:ready');
  await open(page, { readOnly: true }, NAME, noted);
  await page.waitForTimeout(2000);
  await toolsList(page, 'edit'); // same layout as the editable session
  const before = await shotBuf(page);
  await click(page, 500, 400); // select the note
  await page.waitForTimeout(500);
  const selected = await shotBuf(page);
  await page.keyboard.press('Delete');
  await page.keyboard.press('Backspace');
  const after = await shotBuf(page);
  await page.waitForTimeout(1500);
  expect(await diffOf(page, before, selected), 'the note is on the page and selecting it shows').toBeGreaterThan(0);
  expect(await diffOf(page, selected, after), 'Delete/Backspace changed nothing at once').toBe(0);
  expect(await diffOf(page, selected, await shotBuf(page)), 'nor later').toBe(0);
  expect((await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(logs.get(page)!.filter((t) => t.includes('read-only safety net'))).toHaveLength(0);
});

// ---- the 30 s autosave (the Docs bridge's, wordcraft/0127): a recovery copy, not a save ----
test('10 autosave: none when clean; after an edit pdfcraft:autosave within 30 s; stays dirty; none while a save is pending', async ({ page }) => {
  test.setTimeout(300_000);
  await open(page);
  await page.waitForTimeout(35_000);
  expect(await count(page, 'pdfcraft:autosave'), 'a clean document is not autosaved').toBe(0);
  await edit(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await waitFor(page, 'pdfcraft:autosave');
  const a = (await msgs(page)).find((m) => m.type === 'pdfcraft:autosave');
  expect(a.name).toBe(NAME);
  expect(a.bytes).toMatch(/^37,80:\d{4,}/); // "%P"
  expect(await lastDirty(page), 'an autosave is not a save: still dirty').toBe(true);
  expect(await count(page, 'pdfcraft:save'), 'and not a save message').toBe(0);
  // a save waits for its answer: no autosave meanwhile
  await send(page, { type: 'pdfcraft:requestSave' });
  await waitFor(page, 'pdfcraft:save');
  const n = await count(page, 'pdfcraft:autosave');
  await page.waitForTimeout(AUTOSAVE_WAIT + 5000);
  expect(await count(page, 'pdfcraft:autosave'), 'none while the save is pending').toBe(n);
  await send(page, { type: 'pdfcraft:saved', ok: false });
  await waitFor(page, 'pdfcraft:autosave', n + 1); // it resumes
  expect(await lastDirty(page)).toBe(true);
});


const pdfEdit = async (page: Page, _i: number) => edit(page);
const pdfAutosaves = (page: Page) => count(page, 'pdfcraft:autosave');
// hengines (patch pdfcraft/0170): the recovery copy goes 3 s after the last edit, and at the latest 30 s after the first unsent edit
// while editing goes on; never more often than every 3 s; none for an unchanged document.
test('8b autosave: 3 s after the last edit, once per quiet period; continuous editing is saved within 30 s', async ({ page }) => {
  test.setTimeout(180_000);
  await open(page);
  await waitFor(page, 'pdfcraft:opened');
  const n = async () => (await pdfAutosaves(page));
  expect(await n()).toBe(0);
  await pdfEdit(page, 0);
  await expect.poll(() => lastDirty(page)).toBe(true);
  const t0 = Date.now();
  await expect.poll(n, { timeout: 12_000 }).toBe(1);
  expect(Date.now() - t0, 'a copy about 3 s after the edit, not 30 s').toBeLessThan(12_000);
  await page.waitForTimeout(8_000);
  expect(await n(), 'an unchanged document is not sent again').toBe(1);
  // continuous editing (edits back to back, well under 3 s apart): nothing while the edits keep coming for 20 s, a copy by the 30 s ceiling
  // The copy of a quiet period is allowed wherever two edits ended more than 3 s apart (a loaded machine stalls an edit now and then).
  const start = Date.now();
  const ends: number[] = [];
  let i = 1;
  const edits = async (until: number) => { while (Date.now() - start < until) { await pdfEdit(page, i++); ends.push(Date.now()); } };
  const stalls = () => ends.filter((e, k) => k > 0 && e - ends[k - 1] > 2_900).length;
  await edits(20_000);
  expect(await n(), 'no copy while the edits are less than 3 s apart (before the 30 s ceiling)').toBeLessThanOrEqual(1 + stalls());
  await edits(38_000);
  const during = await n();
  expect(during, 'a copy by the 30 s ceiling although the edits never paused for 3 s').toBeGreaterThanOrEqual(2);
  if (stalls() === 0) expect(during, 'exactly the ceiling copy').toBe(2);
  const times = (await msgs(page)).filter((m) => m.type === 'pdfcraft:autosave').map((m) => m.t as number);
  for (let k = 1; k < times.length; k++) expect(times[k] - times[k - 1], 'never more often than every 3 s').toBeGreaterThanOrEqual(2_800);
  // the edits stop: the last state goes 3 s later
  await expect.poll(n, { timeout: 12_000 }).toBeGreaterThan(during);
  await page.waitForTimeout(8_000);
  const after = await n();
  await page.waitForTimeout(5_000);
  expect(await n(), 'quiet again: nothing more').toBe(after);
  expect(await lastDirty(page)).toBe(true);
});

test('11 autosave: none in a read-only session', async ({ page }) => {
  test.setTimeout(120_000);
  await open(page, { readOnly: true });
  await edit(page);
  await page.waitForTimeout(AUTOSAVE_WAIT + 5000);
  expect(await count(page, 'pdfcraft:autosave')).toBe(0);
  expect((await msgs(page)).filter((m) => m.type === 'pdfcraft:dirty' && m.dirty)).toHaveLength(0);
});
