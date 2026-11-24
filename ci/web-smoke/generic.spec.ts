import { test, expect, chromium, type Page, type Frame, type Browser } from '@playwright/test';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as zlib from 'node:zlib';

// Generic-host rehearsal (packaging/docserver/rehearse-generic.sh): ci/web-smoke/generic-host/host.mjs, a plain ONLYOFFICE-API host,
// against a document server container started with the quick-start settings only. Per format: open, edit, save (callbacks 1, 6, 2,
// new bytes), reopen (the host serves the saved bytes, the editor opens them), download from the host. Plus a view-only open per format.
const HOST = process.env.HOST_URL ?? 'http://localhost:3000';
const DS = process.env.DS_URL!;
const PIN = process.env.PIN!;
const SHOTS = process.env.SHOTS ?? path.resolve(__dirname, '../../docs/phase5/evidence/generic');
const MARK = 'QSTART7';
const sha = (b: Buffer) => crypto.createHash('sha256').update(b).digest('hex');
type State = { received: { doc: string; status: number; sha256: string }[]; rejected: string[]; sha: Record<string, string> };
const state = async () => (await fetch(`${HOST}/state`)).json() as Promise<State>;

// the text of every part of a zip (stored or deflated): enough to look for the typed marker and to count slides
function zipParts(b: Buffer): Record<string, string> {
  const out: Record<string, string> = {};
  const e = b.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  const n = b.readUInt16LE(e + 10);
  let p = b.readUInt32LE(e + 16);
  for (let i = 0; i < n; i++) {
    const method = b.readUInt16LE(p + 10), csize = b.readUInt32LE(p + 20), nlen = b.readUInt16LE(p + 28), xlen = b.readUInt16LE(p + 30), clen = b.readUInt16LE(p + 32), off = b.readUInt32LE(p + 42);
    const name = b.subarray(p + 46, p + 46 + nlen).toString();
    const data = b.subarray(off + 30 + b.readUInt16LE(off + 26) + b.readUInt16LE(off + 28)).subarray(0, csize);
    out[name] = (method === 8 ? zlib.inflateRawSync(data) : data).toString('utf8');
    p += 46 + nlen + xlen + clen;
  }
  return out;
}
const hasMark = (b: Buffer) => Object.values(zipParts(b)).some((t) => t.includes(MARK));
const slides = (b: Buffer) => Object.keys(zipParts(b)).filter((n) => /^ppt\/slides\/slide\d+\.xml$/.test(n)).length;

const FORMATS: { name: string; route: string; magic: string; edit: (page: Page, ed: Frame) => Promise<void>; changed: (before: Buffer, after: Buffer) => boolean }[] = [
  { name: 'sample.docx', route: 'docs', magic: 'PK', edit: async (page, ed) => { await ed.locator('canvas').first().click({ position: { x: 300, y: 200 } }); await page.keyboard.type(MARK, { delay: 60 }); }, changed: (_, a) => hasMark(a) },
  { name: 'sample.xlsx', route: 'sheets', magic: 'PK', edit: async (page, ed) => { await ed.locator('canvas').first().click({ position: { x: 420, y: 420 } }); await page.keyboard.type(MARK, { delay: 60 }); await page.keyboard.press('Enter'); }, changed: (_, a) => hasMark(a) },
  { name: 'sample.pptx', route: 'slides', magic: 'PK', edit: async (page, ed) => { await ed.locator('canvas').first().click({ position: { x: 500, y: 350 } }); await page.keyboard.press('Control+Shift+N'); }, changed: (b, a) => slides(a) === slides(b) + 1 },
  { name: 'sample.pdf', route: 'pdf', magic: '%PDF', edit: async (page, ed) => { await ed.locator('canvas').first().click({ position: { x: 550, y: 400 } }); await page.keyboard.press('Control+k'); await page.waitForTimeout(500); await page.keyboard.type('Rotate pages clockwise'); await page.waitForTimeout(300); await page.keyboard.press('Enter'); }, changed: (b, a) => !a.equals(b) },
];

const ARGS = [`--ignore-certificate-errors-spki-list=${PIN}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'];
const open = async (browser: Browser, doc: string, mode = 'edit') => {
  const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
  const problems: string[] = [];
  page.on('pageerror', (e) => problems.push(e.message));
  await page.goto(`${HOST}/host?doc=${doc}&mode=${mode}`);
  await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 90_000 }).toMatch(/ready|error/);
  expect(`${await page.locator('body').getAttribute('data-state')}|${await page.locator('#detail').textContent()}`).toBe('ready|');
  return { page, problems };
};
const closeIt = async (page: Page) => {
  await page.evaluate(() => (window as any).__editor.close());
  await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('closed');
};

for (const f of FORMATS) {
  const ext = f.name.split('.').pop()!;
  test(`${ext}: open, edit, save, reopen, download`, async () => {
    fs.mkdirSync(SHOTS, { recursive: true });
    const browser = await chromium.launch({ args: ARGS });
    try {
      const s0 = await state();
      const v0 = Buffer.from(await (await fetch(`${HOST}/download?doc=${f.name}`)).arrayBuffer());
      const { page, problems } = await open(browser, f.name);
      await page.screenshot({ path: path.join(SHOTS, `${ext}-1-open.png`) });
      const ed = page.frames().find((fr) => fr.url().startsWith(`${DS}/${f.route}/`))!;
      expect(ed, 'the editor frame').toBeTruthy();
      await f.edit(page, ed);
      await expect.poll(() => page.evaluate(() => (window as any).__dirty), { timeout: 15_000 }).toBe(true);
      await page.screenshot({ path: path.join(SHOTS, `${ext}-2-edited.png`) });
      await page.evaluate(() => (window as any).__editor.requestSave());
      await expect.poll(() => page.locator('body').getAttribute('data-state'), { timeout: 30_000 }).toBe('saved');
      await closeIt(page);
      const s1 = await state();
      expect(s1.received.slice(s0.received.length).map((r) => [r.doc, r.status])).toEqual([[f.name, 1], [f.name, 6], [f.name, 2]]); // 1: first change, 6: the save, 2: closed
      expect(s1.rejected).toEqual(s0.rejected);
      expect(s1.sha[f.name]).not.toBe(s0.sha[f.name]);

      // reopen: the host now serves the saved bytes; the editor opens them
      const again = await open(browser, f.name);
      await again.page.screenshot({ path: path.join(SHOTS, `${ext}-3-reopened.png`) });
      await closeIt(again.page); // nothing edited: no new version
      const s2 = await state();
      expect(s2.sha[f.name]).toBe(s1.sha[f.name]);

      // download from the host: the saved document
      const v1 = Buffer.from(await (await fetch(`${HOST}/download?doc=${f.name}`)).arrayBuffer());
      expect(v1.subarray(0, f.magic.length).toString('latin1')).toBe(f.magic);
      expect(sha(v1)).toBe(s1.sha[f.name]);
      expect(f.changed(v0, v1), 'the saved bytes hold the edit').toBe(true);
      expect(problems.concat(again.problems)).toEqual([]);
      console.log(`${ext}: ${v0.length} -> ${v1.length} bytes, callbacks ${s1.received.slice(s0.received.length).map((r) => r.status)}`);
    } finally { await browser.close(); }
  });

  test(`${ext}: view-only open takes no edit and saves nothing`, async () => {
    const browser = await chromium.launch({ args: ARGS });
    try {
      const s0 = await state();
      const { page, problems } = await open(browser, f.name, 'view');
      const ed = page.frames().find((fr) => fr.url().startsWith(`${DS}/${f.route}/`))!;
      await f.edit(page, ed);
      await page.keyboard.press('Control+s');
      await page.waitForTimeout(3000);
      expect(await page.evaluate(() => (window as any).__dirty)).not.toBe(true);
      await page.evaluate(() => (window as any).__editor.requestSave());
      await page.waitForTimeout(2000);
      await page.screenshot({ path: path.join(SHOTS, `${ext}-4-viewonly.png`) });
      await closeIt(page);
      const s1 = await state();
      expect(s1.received.length).toBe(s0.received.length); // no callback at all for a view session
      expect(s1.sha[f.name]).toBe(s0.sha[f.name]);
      expect(problems).toEqual([]);
    } finally { await browser.close(); }
  });
}
