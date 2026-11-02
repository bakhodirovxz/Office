// Phase 6 WP6: every dialog the Docs phone chrome can open, at 360 and 412 dp (desktop Chromium, touch emulation; docs/phase6/phone-layout.md section 4).
//   node scripts/mobile/phone-dialogs.mjs --dist <dist> [--out <dir>]
// DPR 1 on purpose: in headless Chromium at DPR above 1 the web runner maps touch positions at the wrong scale (found in WP6, docs/phase6/phone-layout.md section 6); the emulator is right at 1.5 and 2.625.
// Taps are CDP touches with a 60 ms hold (what a finger does), in editor-frame CSS px; the sheet rows are 48 dp, bottom-anchored above the 48 dp bar.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-phone/dist-full'));
const out = path.resolve(opt('--out', path.join(ROOT, 'docs/phase6/evidence/phone-layout')));
fs.mkdirSync(out, { recursive: true });
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');

const serve = await L.startServe({ root: ROOT, dist });
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-unsafe-webgpu'] });
const errors = [];
try {
  for (const [w, h, dpr] of [[360, 780, 1], [412, 915, 1]]) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: dpr, hasTouch: true, isMobile: true, locale: 'en-US' });
    const open = async () => {
      const page = await ctx.newPage();
      page.on('console', (m) => { if (m.type() === 'error') errors.push(`${w}: ${m.text().slice(0, 200)}`); });
      page.on('pageerror', (e) => errors.push(`${w}: ${String(e).slice(0, 200)}`));
      await page.addInitScript(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
      await page.goto(serve.base + '/', { waitUntil: 'load' });
      await page.waitForSelector('#cards li');
      await page.setInputFiles('#file', path.join(ROOT, 'scripts/mobile/fixtures/blank.docx'));
      await page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 120000 });
      await page.waitForTimeout(3000);
      const box = await page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; });
      const cdp = await ctx.newCDPSession(page);
      const tap = async (x, y, wait = 1500) => {
        const pt = { x: box.x + x, y: box.y + (y < 0 ? box.h + y : y), id: 1 };
        await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pt] });
        await page.waitForTimeout(60);
        await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
        await page.waitForTimeout(wait);
      };
      await tap(box.w / 2, 140); // first finger
      return { page, box, tap, shot: (tag) => page.screenshot({ path: path.join(out, `${w}-dlg-${tag}.png`) }), barSlot: (i, n = 5) => (box.w / n) * (i + 0.5), row: (n, k) => box.h - 48 - (48 * n + 8) + 48 * k + 28 };
    };
    // 1. every menu row that is not a plain command: Save As, Find (menu: Save, Save As, Find, View, Desktop layout; Print is not offered, it opens the Backstage)
    for (const [tag, k] of [['saveas', 1], ['find', 2]]) {
      const p = await open(); await p.tap(p.box.w - 24, 24); await p.tap(p.box.w / 2, p.row(5, k), 2500); await p.shot(tag); await p.page.close();
    }
    // 2. a table: Insert > 3 x 3, then the Table sheet and its Split Cells
    { const p = await open(); await p.tap(p.barSlot(2), -24); await p.tap(p.box.w / 2, p.row(3, 0), 2500);
      await p.tap(60, 82, 1500); // a finger in the first cell: the table set
      await p.shot('table-set'); await p.page.waitForTimeout(3000); await p.shot('table-set-later'); await p.tap(p.barSlot(0), -24); await p.shot('row-added'); await p.tap(p.barSlot(4), -24); await p.shot('table-sheet'); await p.tap(p.box.w * 0.75, p.row(3, 2), 2500); await p.shot('split'); await p.page.close(); }
    // 3. a picture: Insert > Picture (a file chooser), then Size
    { const p = await open(); await p.tap(p.barSlot(2), -24);
      const chooser = p.page.waitForEvent('filechooser', { timeout: 8000 }).catch(() => null);
      await p.tap(p.box.w / 2, p.row(3, 1), 1000);
      const fc = await chooser;
      if (fc) { await fc.setFiles({ name: 'p.png', mimeType: 'image/png', buffer: PNG }); await p.page.waitForTimeout(3000); await p.shot('picture-set'); await p.tap(p.barSlot(1), -24, 2500); await p.shot('picture-size'); }
      else errors.push(`${w}: no file chooser for Insert > Picture`);
      await p.page.close(); }
    console.log(w, 'done');
    await ctx.close();
  }
} finally {
  await browser.close();
  serve.stop();
  if (errors.length) console.log(errors.join('\n'));
}
