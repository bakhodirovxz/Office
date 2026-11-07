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

// Drop-in rehearsal (packaging/docserver/rehearse-dropin.sh, docs/phase5/dropin.md D5): the backend's edit config of an xlsx opens the
// Sheets editor as a view (edit_types = docx); nothing reaches the backend.
test('drop-in: an xlsx edit config opens view-only in the Sheets editor', async () => {
  test.skip(!process.env.DROPIN, 'drop-in rehearsal only');
  const browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${PIN}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
  try {
    const s0 = await state();
    const cfg = await (await fetch(`${HOST}/files/2/editor-config`)).json() as { editorConfig: { mode: string } };
    expect(cfg.editorConfig.mode).toBe('edit');
    const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
    await page.goto(`${HOST}/host?kind=files&id=2&lang=en`);
    await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 90_000 }).toMatch(/ready|error/);
    expect(await page.locator('body').getAttribute('data-state'), await page.locator('#detail').textContent() ?? '').toBe('ready');
    expect(page.frames().some((f) => f.url().startsWith('https://127.0.0.1:18443/sheets/'))).toBe(true);
    await page.evaluate(() => (window as any).__editor.close());
    await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('closed');
    const s1 = await state();
    expect(s1.received.length).toBe(s0.received.length);
  } finally {
    await browser.close();
  }
});
