import { test, expect, chromium } from '@playwright/test';
import * as crypto from 'node:crypto';

// WP10 rehearsal: open -> edit -> save -> reopen -> download against the containers of packaging/docserver/compose.yml
// (Traefik with a throwaway CA on https://127.0.0.1:18443, the fake HR backend on http://localhost:8790). Run by rehearse.sh, which passes
// the CA's public-key pin in PIN. The server's disk is in a container, so everything is checked from the backend's side.
const HOST = process.env.HOST_URL ?? 'http://localhost:8790';
const PIN = process.env.PIN!;
const sha = (b: Buffer) => crypto.createHash('sha256').update(b).digest('hex');
const state = async () => (await fetch(`${HOST}/__test/state`)).json() as Promise<{ received: { status: number }[]; rejected: string[]; objects: Record<string, string> }>;

test('open, edit, save, close, reopen, download', async () => {
  const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${PIN}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
  try {
    const open = async () => {
      const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
      const problems: string[] = [];
      page.on('pageerror', (e) => problems.push(e.message));
      await page.goto(`${HOST}/host?kind=files&id=1&lang=en`);
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 90_000 }).toMatch(/ready|error/);
      expect(await page.locator('body').getAttribute('data-state'), await page.locator('#detail').textContent() ?? '').toBe('ready');
      return { page, problems };
    };
    const s0 = await state();
    const key = Object.keys(s0.objects).find((k) => k.startsWith('files/1/'))!;

    const { page, problems } = await open();
    const editor = page.frames().find((f) => f.url().startsWith('https://127.0.0.1:18443/docs/'))!;
    await editor.locator('canvas').first().click({ position: { x: 300, y: 200 } });
    await page.keyboard.type('x');
    await expect.poll(() => page.evaluate(() => (window as any).__dirty), { timeout: 15_000 }).toBe(true);
    await page.evaluate(() => (window as any).__editor.requestSave());
    await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('saved');
    await page.evaluate(() => (window as any).__editor.close());
    await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('closed');
    expect(await page.locator('#detail').textContent()).toBe('closed status 2');
    const s1 = await state();
    expect(s1.received.slice(s0.received.length).map((r) => r.status)).toEqual([6, 2]);
    expect(s1.rejected).toEqual(s0.rejected);
    expect(s1.objects[key]).not.toBe(s0.objects[key]);

    // reopen: a new session fetches the saved version from the backend (through the internal route of its own save, then the file route)
    const again = await open();
    const cfg = await (await fetch(`${HOST}/files/1/editor-config`)).json() as { document: { url: string } };
    // download the stored document from the backend's file route (the container name rewritten to the published port)
    const url = new URL(cfg.document.url); url.host = new URL(HOST).host; url.protocol = 'http:';
    const bytes = Buffer.from(await (await fetch(url)).arrayBuffer());
    expect(bytes.subarray(0, 4).toString('latin1')).toBe('PK\x03\x04');
    expect(sha(bytes)).toBe(s1.objects[key]);

    // WP12: one more save in the reopened session, then History > Compare draws a real docs diff made by docs-diff in the image (not the 501)
    const ed2 = again.page.frames().find((f) => f.url().startsWith('https://127.0.0.1:18443/docs/'))!;
    await ed2.locator('canvas').first().click({ position: { x: 300, y: 200 } });
    await again.page.keyboard.type('y');
    await expect.poll(() => again.page.evaluate(() => (window as any).__dirty), { timeout: 15_000 }).toBe(true);
    await again.page.evaluate(() => (window as any).__editor.requestSave());
    await expect.poll(() => again.page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('saved');
    const w = again.page.frames().find((f) => f.url().includes('/ds/editor'))!;
    await w.getByRole('button', { name: 'History', exact: true }).click();
    await expect.poll(() => w.locator('#hist-vers > li').count(), { timeout: 15_000 }).toBeGreaterThanOrEqual(2);
    await w.getByRole('button', { name: 'Compare', exact: true }).click();
    await expect(w.locator('#hist-cmp-sec')).toBeVisible({ timeout: 60_000 });
    expect(await w.locator('#hist-msg').textContent()).not.toContain('not installed');
    await expect(w.locator('#hist-cmp > li').first()).toBeVisible();
    await expect(w.locator('#hist-cmp ins, #hist-cmp del').first()).toBeVisible();
    expect(problems.concat(again.problems)).toEqual([]);
    console.log(`download ${bytes.length} bytes sha256 ${sha(bytes).slice(0, 12)} (was ${s0.objects[key].slice(0, 12)})`);
  } finally {
    await browser.close();
  }
});

// Drop-in rehearsal (packaging/docserver/rehearse-dropin.sh, docs/phase5/r6.md): with the installer's default edit types the backend's
// edit configs of an xlsx (files/2) and a pptx (files/3) edit: an edit, save (status 6), close (status 2), and the backend's object changes.
for (const [ext, id, route, edit] of [
  ['xlsx', 2, 'sheets', async (page: import('@playwright/test').Page) => { await page.keyboard.type('zz'); await page.keyboard.press('Enter'); }],
  ['pptx', 3, 'slides', async (page: import('@playwright/test').Page) => { await page.keyboard.press('Control+Shift+N'); }],
] as const) {
  test(`drop-in: an ${ext} edit config edits and saves in the ${route} editor`, async () => {
    test.skip(!process.env.DROPIN, 'drop-in rehearsal only');
    const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${PIN}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
    try {
      const s0 = await state();
      const key = Object.keys(s0.objects).find((k) => k.startsWith(`files/${id}/`))!;
      const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
      await page.goto(`${HOST}/host?kind=files&id=${id}&lang=en`);
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 90_000 }).toMatch(/ready|error/);
      expect(await page.locator('body').getAttribute('data-state'), await page.locator('#detail').textContent() ?? '').toBe('ready');
      const editor = page.frames().find((f) => f.url().startsWith(`https://127.0.0.1:18443/${route}/`))!;
      await editor.locator('canvas').first().click({ position: { x: 420, y: 420 } });
      await edit(page);
      await expect.poll(() => page.evaluate(() => (window as any).__dirty), { timeout: 15_000 }).toBe(true);
      await page.evaluate(() => (window as any).__editor.requestSave());
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('saved');
      await page.evaluate(() => (window as any).__editor.close());
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('closed');
      expect(await page.locator('#detail').textContent()).toBe('closed status 2');
      const s1 = await state();
      expect(s1.received.slice(s0.received.length).map((r) => r.status)).toEqual([6, 2]);
      expect(s1.rejected).toEqual(s0.rejected);
      expect(s1.objects[key]).not.toBe(s0.objects[key]);
      console.log(`drop-in ${ext}: stored ${s1.objects[key].slice(0, 12)} (was ${s0.objects[key].slice(0, 12)})`);
    } finally {
      await browser.close();
    }
  });
}

// Read-only through the whole chain (docs/phase5/r6.md section 5): the backend's editor-config with `mode=view` (permissions.edit false) opens each
// editor read only; typing is refused (the host's dirty flag never turns true) and nothing reaches the backend (no callback, the object is unchanged).
// The pixel-level proof (not even for a frame) is in the bridge-* specs; this is the same flag crossing the real wrapper, JWT and Traefik.
for (const [ext, id, route, edit] of [
  ['docx', 1, 'docs', async (page: import('@playwright/test').Page) => { await page.keyboard.type('zz'); }],
  ['xlsx', 2, 'sheets', async (page: import('@playwright/test').Page) => { await page.keyboard.type('zz'); await page.keyboard.press('Enter'); }],
  ['pptx', 3, 'slides', async (page: import('@playwright/test').Page) => { await page.keyboard.press('Control+Shift+N'); }],
] as const) {
  test(`drop-in: an ${ext} opened with mode=view takes no edit and sends nothing`, async () => {
    test.skip(!process.env.DROPIN, 'drop-in rehearsal only');
    const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${PIN}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
    try {
      const s0 = await state();
      const key = Object.keys(s0.objects).find((k) => k.startsWith(`files/${id}/`))!;
      const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
      await page.goto(`${HOST}/host?kind=files&id=${id}&lang=en&mode=view`);
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 90_000 }).toMatch(/ready|error/);
      expect(await page.locator('body').getAttribute('data-state'), await page.locator('#detail').textContent() ?? '').toBe('ready');
      const editor = page.frames().find((f) => f.url().startsWith(`https://127.0.0.1:18443/${route}/`))!;
      await editor.locator('canvas').first().click({ position: { x: 420, y: 420 } });
      await edit(page);
      await page.waitForTimeout(3000);
      expect(await page.evaluate(() => (window as any).__dirty)).not.toBe(true);
      await page.evaluate(() => (window as any).__editor.requestSave());
      await page.waitForTimeout(2000);
      await page.evaluate(() => (window as any).__editor.close());
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('closed');
      const s1 = await state();
      expect(s1.received.slice(s0.received.length).filter((r) => r.status === 6)).toEqual([]); // no save callback
      expect(s1.objects[key]).toBe(s0.objects[key]);
      console.log(`drop-in ${ext} view: object unchanged (${s1.objects[key].slice(0, 12)})`);
    } finally {
      await browser.close();
    }
  });
}
