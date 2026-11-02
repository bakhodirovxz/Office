// How long the editor's frames take on desktop Chromium with software GL: a tap whose release is processed more than 0.5 s after its press is a long press to egui, so the
// screenshots (phone-ux.mjs) must know the frame time. Prints the long tasks and the animation frame gaps around a tap on the page in editing.
//   node scripts/mobile/phone-frames.mjs --dist <dist> [--w 360 --h 640]
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-mobileux/dist'));
const [w, h] = [Number(opt('--w', 360)), Number(opt('--h', 640))];
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');
const serve = await L.startServe({ root: ROOT, dist });
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-unsafe-webgpu'] });
try {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, hasTouch: true, isMobile: true });
  await ctx.addInitScript(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
  const page = await ctx.newPage();
  await page.goto(serve.base + '/', { waitUntil: 'load' });
  await page.waitForSelector('#cards li');
  await page.setInputFiles('#file', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'));
  await page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180000 });
  await page.waitForTimeout(4000);
  const frame = page.frames().find((f) => f !== page.mainFrame());
  await frame.evaluate(() => {
    window.__t = [];
    let last = performance.now();
    const tick = (now) => { window.__t.push(Math.round(now - last)); last = now; requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  });
  const cdp = await ctx.newCDPSession(page);
  const b = await page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y }; });
  for (const [x, y] of [[w / 2, h / 2], [w - 44, h - 44], [330, 100], [330, 100]]) {
    await frame.evaluate(() => { window.__t.length = 0; });
    const pt = { x: b.x + x, y: b.y + y, id: 1 };
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pt] });
    await page.waitForTimeout(60);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await page.waitForTimeout(1000);
    console.log('tap', x, y, 'animation frame gaps (ms) after it:', JSON.stringify(await frame.evaluate(() => window.__t.filter((t) => t > 40).slice(0, 12))));
  }
  await ctx.close();
} finally {
  await browser.close();
  serve.stop();
}
