import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';

// The Slides (deckcraft) host bridge, PLAN phase 4 WP8: the ?host=parent postMessage protocol against the built slides-web dist,
// from a tiny same-origin host page. Run with:
//   npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.bridge-slides.config.ts
// Env: SLIDES_DIST (default E:/office-target/p1/dist/slides-web).
const ROOT = path.resolve(__dirname, '../..');
const DIST = path.resolve(process.env.SLIDES_DIST ?? 'E:/office-target/p1/dist/slides-web');
const PPTX = path.join(ROOT, 'domains/slides/tests/translit-fixtures/pptx/01-shapes-notes.pptx');
const NAME = path.basename(PPTX);
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon' };

// The host page keeps every message from the editor frame and lets the test post to it (as the frame's parent) or from a sibling frame.
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
const types = async (page: Page) => (await msgs(page)).map((m) => m.type);
const count = async (page: Page, type: string) => (await types(page)).filter((t) => t === type).length;
const waitFor = (page: Page, type: string, n = 1) => expect.poll(() => count(page, type), { timeout: 90_000 }).toBeGreaterThanOrEqual(n);

async function open(page: Page, name = NAME, bytes = fs.readFileSync(PPTX), extra: object = {}) {
  await page.evaluate(([n, b, x]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__toEditor({ type: 'deckcraft:open', name: n, bytes: buf, ...(x as object) }, [buf]);
  }, [name, Array.from(bytes), extra] as const);
}
const canvas = (page: Page) => page.frameLocator('#ed').locator('canvas');
// Everything the editor frame logs (the read-only safety net logs a warning when it has to take a change back).
const consoleOf = (page: Page) => { const logs: string[] = []; page.on('console', (m) => logs.push(m.text())); return logs; };
// eslint-disable-next-line @typescript-eslint/no-require-imports
const { PNG } = require('playwright-core/lib/utilsBundle');
// Without the status bar (a refusal shows its reason there) and the title bar.
const frameShot = (page: Page) => page.screenshot({ clip: { x: 8, y: 48, width: 1100, height: 620 } }); // the frame sits at 8,8
/** Pixels of the two screenshots that differ (Infinity: different sizes). */
const diffPixels = (a: Buffer, b: Buffer) => {
  const x = PNG.sync.read(a), y = PNG.sync.read(b);
  if (x.width !== y.width || x.height !== y.height) return Infinity;
  let n = 0;
  for (let i = 0; i < x.data.length; i += 4) if (Math.abs(x.data[i] - y.data[i]) + Math.abs(x.data[i + 1] - y.data[i + 1]) + Math.abs(x.data[i + 2] - y.data[i + 2]) > 24) n++;
  return n;
};
/** Wait out the font notice (it goes away by itself after 8 s), then until the frame has not changed for 3 s: a steady baseline. */
async function settle(page: Page) {
  let prev = await frameShot(page);
  let steady = 0;
  for (let i = 0; i < 50 && (i < 10 || steady < 3); i++) {
    await page.waitForTimeout(1000);
    const now = await frameShot(page);
    steady = diffPixels(prev, now) <= 60 ? steady + 1 : 0;
    prev = now;
  }
  return prev;
}

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => { throw e; });
  await page.goto(base + '/host.html');
  await waitFor(page, 'deckcraft:ready');
});

test('1 ready: announced once with a version, clean, nothing else sent', async ({ page }) => {
  const m = await msgs(page);
  expect(m.filter((x) => x.type === 'deckcraft:ready')).toHaveLength(1);
  expect(m.find((x) => x.type === 'deckcraft:ready').version).toMatch(/^\d+\.\d+\.\d+/);
  await waitFor(page, 'deckcraft:dirty');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:dirty').dirty).toBe(false);
  expect(await count(page, 'deckcraft:save')).toBe(0);
  expect(await count(page, 'deckcraft:failed')).toBe(0);
});

test('2 open: the file is shown and announced by name', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:opened').name).toBe(NAME);
  expect(await count(page, 'deckcraft:failed')).toBe(0);
});

test('3 requestSave: a .pptx comes back as a transferred ArrayBuffer, named after the opened file', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  const s = (await msgs(page)).find((x) => x.type === 'deckcraft:save');
  expect(s.name).toBe(NAME);
  expect(s.bytes).toMatch(/^80,75:\d{4,}/); // "PK", a zip
});

const lastDirty = async (page: Page) => (await msgs(page)).filter((x) => x.type === 'deckcraft:dirty').map((x) => x.dirty).at(-1);
const newSlide = async (page: Page) => {
  await canvas(page).click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+Shift+N'); // New Slide
};

test('4 dirty: an edit reports true; a save reports false once the host acknowledged it (r6)', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true); // unanswered: still dirty
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:saved', ok: false }));
  await page.waitForTimeout(2000);
  expect(await lastDirty(page)).toBe(true);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save', 2);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:saved', ok: true }));
  await expect.poll(() => lastDirty(page)).toBe(false);
});

test('5 Ctrl+S in the editor sends a .pptx, never a .deckcraft download', async ({ page }) => {
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await canvas(page).click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+s');
  await waitFor(page, 'deckcraft:save');
  const saves = (await msgs(page)).filter((x) => x.type === 'deckcraft:save');
  expect(saves).toHaveLength(1);
  expect(saves[0].name).toBe(NAME);
});

test('6 a second open replaces the first', async ({ page }) => {
  await open(page, 'first.pptx');
  await waitFor(page, 'deckcraft:opened');
  await open(page, 'second.pptx');
  await waitFor(page, 'deckcraft:opened', 2);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  expect((await msgs(page)).find((x) => x.type === 'deckcraft:save').name).toBe('second.pptx');
});

test('7 only the parent is heard: a message from a sibling frame is ignored', async ({ page }) => {
  const sib = page.frames().find((f) => f.url().endsWith('/sibling.html'))!;
  await sib.evaluate(([n, b]) => {
    const buf = new Uint8Array(b as number[]).buffer;
    (window as any).__post({ type: 'deckcraft:open', name: n, bytes: buf });
    (window as any).__post({ type: 'deckcraft:requestSave' });
  }, [NAME, Array.from(fs.readFileSync(PPTX))] as const);
  await page.waitForTimeout(3000);
  expect(await count(page, 'deckcraft:opened')).toBe(0);
  expect(await count(page, 'deckcraft:save')).toBe(0);
});

test('8 a file that is not a presentation: failed, and the editor keeps going', async ({ page }) => {
  await open(page, 'broken.pptx', Buffer.from('this is not a zip file'));
  await waitFor(page, 'deckcraft:failed');
  expect(await count(page, 'deckcraft:opened')).toBe(0);
  await open(page);
  await waitFor(page, 'deckcraft:opened');
});

// docs/phase5/r6.md (patch deckcraft/0160): the read-only open and canPrint.
test('9 readOnly: an edit is refused, never dirty, and no save is sent (requestSave, Ctrl+S)', async ({ page }) => {
  const logs = consoleOf(page);
  await open(page, NAME, fs.readFileSync(PPTX), { readOnly: true });
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await page.waitForTimeout(2000);
  await page.keyboard.press('Control+s');
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await page.waitForTimeout(3000);
  expect((await msgs(page)).filter((m) => m.type === 'deckcraft:dirty' && m.dirty)).toHaveLength(0);
  expect(await count(page, 'deckcraft:save')).toBe(0);
  expect(logs.filter((l) => l.includes('read-only safety net'))).toEqual([]);
});

// Print on the web is a PDF download of the editor page (SlideApp::print_pdf, the Print button of File › Print, which Ctrl+P opens);
// canPrint:false refuses it.
test('10 canPrint default true: Print downloads the PDF to print; canPrint:false refuses it and Save still works', async ({ page }) => {
  const printed = async () => {
    await canvas(page).click({ position: { x: 500, y: 350 } });
    await page.keyboard.press('Control+p');
    await page.waitForTimeout(1500);
    const dl = page.waitForEvent('download', { timeout: 8000 }).then((d) => d.suggestedFilename()).catch(() => null);
    await canvas(page).click({ position: { x: 251, y: 150 } }); // the Print button
    return dl;
  };
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  expect(await printed()).toMatch(/\.pdf$/);
  expect(await count(page, 'deckcraft:save')).toBe(0); // a print is not a save
  await page.goto(base + '/host.html');
  await waitFor(page, 'deckcraft:ready');
  await open(page, NAME, fs.readFileSync(PPTX), { canPrint: false, canDownload: false });
  await waitFor(page, 'deckcraft:opened');
  expect(await printed()).toBeNull();
  await page.keyboard.press('Escape'); // back from File › Print
  await newSlide(page);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
});

// docs/phase5/r6.md: a read-only presentation cannot be changed at all, not even for a frame. Every input path is tried on a
// read-only session (no change in pixels beyond a text caret, no dirty, no save/autosave, the per-frame revert never fires), and the
// same input on an editable session changes the presentation (so the checks are not vacuous). Coordinates are page pixels (the
// editor frame sits at 8,8); the slide of 01-shapes-notes.pptx has a title text, a group (box + ellipse) and a table.
const GROUP = { x: 368, y: 311 };
const TITLE = { x: 340, y: 163 };
const selectGroup = async (page: Page) => { await page.mouse.click(GROUP.x, GROUP.y); await page.waitForTimeout(500); };
const editTitle = async (page: Page) => { await page.mouse.dblclick(TITLE.x, TITLE.y); await page.waitForTimeout(500); };
const drag = async (page: Page, from: [number, number], to: [number, number]) => {
  await page.mouse.move(...from);
  await page.mouse.down();
  for (let i = 1; i <= 8; i++) await page.mouse.move(from[0] + ((to[0] - from[0]) * i) / 8, from[1] + ((to[1] - from[1]) * i) / 8);
  await page.mouse.up();
};
const clip = async (page: Page, text: string) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.evaluate((t) => navigator.clipboard.writeText(t), text);
};
// The command search (the magnifier at the top right): type a command and run the first match.
const palette = async (page: Page, q: string) => { await page.mouse.click(1085, 27); await page.waitForTimeout(500); await page.keyboard.type(q, { delay: 40 }); await page.waitForTimeout(600); await page.keyboard.press('Enter'); await page.waitForTimeout(1000); };
const key = (k: string) => async (page: Page) => { await page.keyboard.press(k); };

// fx: a fixture of this directory's translit-fixtures, or one under ci/web-smoke/ (with a slash). dirtyOnly: the editable control changes the
// presentation without a visible change on the slide (a number typed into a pane), so only its dirty mark is checked. limit: the most pixels
// a read-only run may change (default 80; a small field needs less to be seen).
type Path = { name: string; fx?: string; dirtyOnly?: boolean; park?: [number, number]; limit?: number; setup?: (p: Page) => Promise<void>; input: (p: Page, readOnly: boolean) => Promise<void> };
const PATHS: Path[] = [
  { name: 'typing in a text', setup: editTitle, input: async (p) => { await p.keyboard.type('Zqx', { delay: 60 }); } },
  { name: 'Backspace in a text', setup: async (p) => { await editTitle(p); await p.keyboard.press('End'); }, input: async (p) => { for (let i = 0; i < 3; i++) await p.keyboard.press('Backspace'); } },
  { name: 'Delete with a shape selected', setup: selectGroup, input: key('Delete') },
  { name: 'Ctrl+V into a text', setup: async (p) => { await clip(p, 'PASTED'); await editTitle(p); }, input: key('Control+v') },
  { name: 'Ctrl+V on the slide', setup: async (p) => { await clip(p, 'PASTED'); await p.mouse.click(700, 480); }, input: key('Control+v') },
  { name: 'Ctrl+X', setup: selectGroup, input: key('Control+x') },
  { name: 'Ctrl+D', setup: selectGroup, input: key('Control+d') },
  { name: 'Delete, Ctrl+Z, Ctrl+Y', setup: selectGroup, input: async (p) => { await p.keyboard.press('Delete'); await p.keyboard.press('Control+z'); await p.keyboard.press('Control+y'); } },
  { name: 'dragging a shape', setup: selectGroup, input: (p) => drag(p, [GROUP.x, GROUP.y], [GROUP.x + 120, GROUP.y + 110]) },
  { name: 'dragging a resize handle', setup: selectGroup, input: (p) => drag(p, [305, 285], [250, 230]) },
  { name: 'dragging the rotate handle', setup: selectGroup, input: (p) => drag(p, [445, 268], [540, 268]) },
  { name: 'ribbon New Slide', input: async (p) => { await p.mouse.click(31, 66); } },
  { name: 'ribbon Bold on the selected text', setup: async (p) => { await editTitle(p); await p.keyboard.press('Control+a'); }, input: async (p) => { await p.mouse.click(342, 66); } },
  { name: 'context menu Cut', setup: selectGroup, input: async (p) => { await p.mouse.click(GROUP.x, GROUP.y, { button: 'right' }); await p.waitForTimeout(500); await p.mouse.click(387, 93); } },
  { name: 'thumbnail context menu Duplicate Slide', input: async (p) => { await p.mouse.click(136, 155, { button: 'right' }); await p.waitForTimeout(500); await p.mouse.click(190, 287); } },
  { name: 'typing in the notes pane', input: async (p) => { await p.mouse.click(600, 619); await p.keyboard.type('Zqx', { delay: 60 }); await p.mouse.click(1095, 400); } }, // the click leaves the field: the notes are stored then
  {
    name: 'dropping an image on the slide',
    input: async (p) => {
      await p.frameLocator('#ed').locator('canvas').evaluate((c) => {
        const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='), (ch) => ch.charCodeAt(0));
        const dt = new DataTransfer();
        dt.items.add(new File([png], 'dropped.png', { type: 'image/png' }));
        const at = { clientX: 500, clientY: 450, bubbles: true, cancelable: true, dataTransfer: dt };
        c.dispatchEvent(new DragEvent('dragenter', at)); c.dispatchEvent(new DragEvent('dragover', at)); c.dispatchEvent(new DragEvent('drop', at));
      });
    },
  },
  { name: 'dragging a slide thumbnail (reorder)', fx: '02-languages.pptx', input: (p) => drag(p, [136, 155], [136, 330]) },
  // Text fields outside the slide: the outline view, the Comments pane and the Format Shape pane are inert in a read-only session (deckcraft/0163).
  { name: 'typing in the outline view', setup: async (p) => { await palette(p, 'Outline View'); }, limit: 15, input: async (p, ro) => { await p.mouse.click(108, 149); await p.keyboard.type('Zqx', { delay: 60 }); if (!ro) await p.mouse.click(700, 640); } }, // read-only: stay in the field, so any typed text would still show
  { name: 'a comment reply typed in the Comments pane', fx: 'fixtures/slides-smartart-comment.pptx', setup: async (p) => { await p.mouse.click(665, 694); await p.waitForTimeout(800); }, limit: 15, input: async (p, ro) => { await p.mouse.click(874, 268); await p.keyboard.type('Zqx', { delay: 60 }); if (!ro) await p.keyboard.press('Enter'); } },
  { name: 'a number typed in the Format Shape pane', dirtyOnly: true, park: [450, 540], limit: 15, // a number in a 60 px box is only a few dozen pixels
    fx: 'fixtures/slides-smartart-comment.pptx', setup: async (p) => { await p.mouse.click(440, 400); await p.mouse.click(440, 400, { button: 'right' }); await p.waitForTimeout(700); await p.mouse.click(490, 611); await p.mouse.move(450, 540); await p.waitForTimeout(1000); }, input: async (p, ro) => { await p.mouse.dblclick(890, 545); await p.keyboard.type('88888888', { delay: 40 }); if (!ro) await p.keyboard.press('Enter'); } }, // Enter would end the edit (and show the old value again) in a read-only run: the typed digits must not show while the field would still hold them
  // the text box tool is armed from the toolbar, then dragged on the slide: the shape is drawn (and shows) as the pointer drags
  { name: 'the text box tool dragged on the slide (drawing preview)', setup: async (p) => { await p.mouse.click(666, 65); await p.waitForTimeout(500); }, input: (p) => drag(p, [620, 470], [820, 540]) },
];

for (const c of PATHS) {
  test(`11 read-only: ${c.name} is refused before it shows; the same input edits an editable session`, async ({ page }) => {
    test.setTimeout(240_000);
    const bytes = fs.readFileSync(c.fx ? (c.fx.includes('/') ? path.join(ROOT, 'ci/web-smoke', c.fx) : path.join(path.dirname(PPTX), c.fx)) : PPTX);
    const logs = consoleOf(page);
    const run = async (readOnly: boolean) => {
      await open(page, NAME, bytes, readOnly ? { readOnly: true } : {});
      await waitFor(page, 'deckcraft:opened');
      await settle(page);
      await c.setup?.(page);
      const base = await settle(page);
      await c.input(page, readOnly);
      await page.mouse.move(...(c.park ?? [1095, 560])); // off the thumbnails and buttons, so no hover highlight differs (park: a case with a pane on the right)
      await page.waitForTimeout(150);
      const first = await frameShot(page);
      await page.waitForTimeout(1000);
      const later = await frameShot(page);
      return { first: diffPixels(base, first), later: diffPixels(base, later) };
    };
    const ro = await run(true);
    expect((await msgs(page)).filter((m) => m.type === 'deckcraft:dirty' && m.dirty), 'dirty in a read-only session').toHaveLength(0);
    expect(await count(page, 'deckcraft:save')).toBe(0);
    expect(await count(page, 'deckcraft:autosave')).toBe(0);
    expect(logs.filter((l) => l.includes('read-only safety net')), 'the per-frame revert fired').toEqual([]);
    expect(ro.first, 'pixels changed right after the input').toBeLessThanOrEqual(c.limit ?? 80);
    expect(ro.later, 'pixels changed a second after the input').toBeLessThanOrEqual(c.limit ?? 80);
    // The control: the same input in an editable session.
    await page.goto(base + '/host.html');
    await waitFor(page, 'deckcraft:ready');
    const rw = await run(false);
    await expect.poll(() => lastDirty(page)).toBe(true);
    if (!c.dirtyOnly) expect(Math.max(rw.first, rw.later), 'the control input changed nothing, the check would be vacuous').toBeGreaterThan(80);
  });
}

// The 30 s autosave (docs/phase5/PLAN.md 3.3): a recovery copy while the presentation is unsaved, not a save.
const autosaves = async (page: Page) => (await msgs(page)).filter((m) => m.type === 'deckcraft:autosave');

test('12 autosave: none when clean; after an edit one arrives within a period, as a .pptx, and the presentation stays dirty', async ({ page }) => {
  test.setTimeout(240_000);
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await page.waitForTimeout(35_000);
  expect(await autosaves(page)).toHaveLength(0);
  await newSlide(page);
  await expect.poll(() => lastDirty(page)).toBe(true);
  await expect.poll(async () => (await autosaves(page)).length, { timeout: 50_000 }).toBeGreaterThanOrEqual(1);
  const a = (await autosaves(page))[0];
  expect(a.name).toBe(NAME);
  expect(a.bytes).toMatch(/^80,75:\d{4,}/);
  expect(await lastDirty(page)).toBe(true);
  expect(await count(page, 'deckcraft:save')).toBe(0);
});


// the first call selects the canvas, the rest are key presses only: edits must come well under 3 s apart even on a loaded machine
const slideEdit = async (page: Page, i: number) => {
  if (i === 0) await canvas(page).click({ position: { x: 500, y: 350 } });
  await page.keyboard.press('Control+Shift+N');
};
const slidesAutosaves = async (page: Page) => (await autosaves(page)).length;
// hengines (patch deckcraft/0170): the recovery copy goes 3 s after the last edit, and at the latest 30 s after the first unsent edit
// while editing goes on; never more often than every 3 s; none for an unchanged document.
test('8b autosave: 3 s after the last edit, once per quiet period; continuous editing is saved within 30 s', async ({ page }) => {
  test.setTimeout(180_000);
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  const n = async () => (await slidesAutosaves(page));
  expect(await n()).toBe(0);
  await slideEdit(page, 0);
  await expect.poll(() => lastDirty(page)).toBe(true);
  const t0 = Date.now();
  await expect.poll(n, { timeout: 12_000 }).toBe(1);
  expect(Date.now() - t0, 'a copy about 3 s after the edit, not 30 s').toBeLessThan(12_000);
  await page.waitForTimeout(8_000);
  expect(await n(), 'an unchanged document is not sent again').toBe(1);
  // continuous editing (edits back to back, well under 3 s apart): nothing while the edits keep coming for 20 s, a copy by the 30 s ceiling
  const start = Date.now();
  let i = 1;
  while (Date.now() - start < 20_000) { await slideEdit(page, i++); await page.waitForTimeout(300); }
  expect(await n(), 'no copy while the edits are less than 3 s apart (before the 30 s ceiling)').toBe(1);
  while (Date.now() - start < 38_000) { await slideEdit(page, i++); await page.waitForTimeout(300); }
  const during = await n();
  expect(during, 'a copy by the 30 s ceiling although the edits never paused for 3 s').toBeGreaterThanOrEqual(2);
  const times = (await msgs(page)).filter((m) => m.type === 'deckcraft:autosave').map((m) => m.t as number);
  for (let k = 1; k < times.length; k++) expect(times[k] - times[k - 1], 'never more often than every 3 s').toBeGreaterThanOrEqual(2_800);
  // the edits stop: the last state goes 3 s later
  await expect.poll(n, { timeout: 12_000 }).toBeGreaterThan(during);
  await page.waitForTimeout(8_000);
  const after = await n();
  await page.waitForTimeout(5_000);
  expect(await n(), 'quiet again: nothing more').toBe(after);
  expect(await lastDirty(page)).toBe(true);
});

test('13 autosave: none in a read-only session', async ({ page }) => {
  test.setTimeout(240_000);
  await open(page, NAME, fs.readFileSync(PPTX), { readOnly: true });
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await page.waitForTimeout(40_000);
  expect(await autosaves(page)).toHaveLength(0);
});

test('14 autosave: none while a save is unanswered; it resumes after the answer (saved ok:false)', async ({ page }) => {
  test.setTimeout(240_000);
  await open(page);
  await waitFor(page, 'deckcraft:opened');
  await newSlide(page);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:requestSave' }));
  await waitFor(page, 'deckcraft:save');
  await newSlide(page); // dirty again, the save still unanswered
  await expect.poll(() => lastDirty(page)).toBe(true);
  await page.waitForTimeout(40_000);
  expect(await autosaves(page)).toHaveLength(0);
  await page.evaluate(() => (window as any).__toEditor({ type: 'deckcraft:saved', ok: false }));
  await expect.poll(async () => (await autosaves(page)).length, { timeout: 50_000 }).toBeGreaterThanOrEqual(1);
});

test('not framed: ?host=parent alone starts the normal app, with the sample deck and no bridge', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(base + '/index.html?host=parent');
  await page.waitForFunction(() => !!(window as any).wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 90_000 });
  expect(errors).toEqual([]);
});
