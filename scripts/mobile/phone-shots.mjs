// Phase 6 WP6: screenshots of the Docs phone layout on desktop Chromium with phone viewports and touch (a proxy: the emulator rig does the real recordings).
//   node scripts/mobile/phone-shots.mjs --dist <dist> [--out <dir>]
// Serves the dist with hoshiya-serve, opens the editor route directly (?phone=on is NOT used: the touch events must switch it, that is the trigger under test),
// and for 360 and 412 dp takes one shot per toolbar state and per sheet. Taps are CSS px (the canvas has no DOM). Synthetic text only.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-phone/dist'));
const out = path.resolve(opt('--out', path.join(ROOT, 'docs/phase6/evidence/phone-layout')));
fs.mkdirSync(out, { recursive: true });
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');

const serve = await L.startServe({ root: ROOT, dist });
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-unsafe-webgpu'] });
const log = [];
try {
  for (const [w, h, dpr] of [[360, 780, 1], [412, 915, 1]]) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: dpr, hasTouch: true, isMobile: true, locale: 'en-US' });
    const page = await ctx.newPage();
    page.on('console', (m) => { if (m.type() === 'error') log.push(`${w}: ${m.text().slice(0, 200)}`); });
    await page.addInitScript(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
    await page.goto(serve.base + '/', { waitUntil: 'load' });
    await page.waitForSelector('#cards li');
    await page.setInputFiles('#file', path.join(ROOT, 'scripts/mobile/fixtures/blank.docx'));
    await page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 120000 });
    await page.waitForTimeout(3000);
    const box = await page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; });
    const shot = async (tag) => { await page.waitForTimeout(700); await page.screenshot({ path: path.join(out, `${w}-${tag}.png`) }); };
    const cdp = await ctx.newCDPSession(page); // a finger: touch down, 60 ms, touch up (Playwright's own tap sends both at once and the page missed it)
    const tap = async (x, y) => { const pt = { x: box.x + x, y: box.y + y, id: 1 }; await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pt] }); await page.waitForTimeout(60); await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] }); await page.waitForTimeout(1200); };
    const H = box.h;
    await tap(w / 2, H / 2); // first touch: the trigger
    await shot('start');
    for (const [tag, x] of [['format', 0.1], ['list', 0.3], ['insert', 0.5]]) {
      await tap(w * x, H - 24); await shot(`sheet-${tag}`);
      await tap(w * x, H - 24); // closes it
    }
    await tap(w * 0.95, 24); await shot('menu');
    await tap(w * 0.95, 24);
    console.log(w, 'done');
    await ctx.close();
  }
} finally {
  await browser.close();
  serve.stop();
  if (log.length) console.log(log.join('\n'));
}
