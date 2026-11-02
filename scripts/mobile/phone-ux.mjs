// Phase 6 WP6 revision (docs/phase6/phone-layout.md, "Read first and one bar"): screenshots of the Docs phone layout on desktop Chromium with phone
// viewports, touch (CDP touches with a 60 ms hold) and DPR 1. One run is one viewport x theme x language:
//   node scripts/mobile/phone-ux.mjs --dist <dist> [--w 360 --h 640] [--theme light|dark] [--lang uz-Latn|ru|en] [--out <dir>] [--tag <prefix>]
// Shots: reading (with the Edit button, the shell's header hidden), edit (caret set), format-text, format-para, insert, menu, table, picture, desktop.
// Synthetic document only (fixtures/phone-ux.docx, made by make-phone-ux.py). Taps are CSS px of the editor frame (the canvas has no DOM).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-mobileux/dist'));
const out = path.resolve(opt('--out', path.join(ROOT, 'docs/phase6/evidence/phone-ux')));
const [w, h] = [Number(opt('--w', 360)), Number(opt('--h', 640))];
const theme = opt('--theme', 'light');
const lang = opt('--lang', 'uz-Latn');
const only = opt('--only', '').split(',').filter(Boolean);
const tag = opt('--tag', `${w}-${theme}-${lang}`);
fs.mkdirSync(out, { recursive: true });
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');

const serve = await L.startServe({ root: ROOT, dist });
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-unsafe-webgpu'] });
const log = [];
try {
  const desktop = args.includes('--desktop');   // a 1280 x 800 window with a mouse: the layout must be the desktop one
  const ctx = await browser.newContext({ viewport: { width: desktop ? 1280 : w, height: desktop ? 800 : h }, deviceScaleFactor: 1, hasTouch: !desktop, isMobile: !desktop, locale: lang === 'ru' ? 'ru-RU' : 'uz-UZ', colorScheme: theme });
  await ctx.addInitScript(([l, t]) => { try { localStorage.setItem('sh.lang', l); localStorage.setItem('sh.theme', t); } catch (_) { /* ignore */ } window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); }, [lang, theme]);
  const page = await ctx.newPage();
  page.on('console', (m) => { if (m.type() === 'error') log.push(`${tag}: ${m.text().slice(0, 200)}`); });
  await page.goto(serve.base + '/', { waitUntil: 'load' });
  await page.waitForSelector('#cards li');
  await page.setInputFiles('#file', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'));
  await page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180000 });
  await page.waitForTimeout(3000);
  if (desktop) { await page.screenshot({ path: path.join(out, `desktop-${theme}-${lang}.png`) }); console.log('desktop done'); await ctx.close(); process.exitCode = 0; throw new Error('desktop-only'); }
  const cdp = await ctx.newCDPSession(page);
  const box = () => page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; });
  // a finger: touch down, 60 ms, touch up (Playwright's own tap sends both at once and the page misses it)
  const tap = async (x, y, wait = 1000) => {
    const b = await box();
    const pt = { x: b.x + x, y: b.y + y, id: 1 };
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pt] });
    await page.waitForTimeout(60);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await page.waitForTimeout(wait);
  };
  const shot = async (name) => { if (only.length && !only.includes(name)) return; await page.waitForTimeout(500); await page.screenshot({ path: path.join(out, `${tag}-${name}.png`) }); };
  const wheel = async (dy) => { const b = await box(); await page.mouse.move(b.x + w / 2, b.y + b.h / 2); await page.mouse.wheel(0, dy); await page.waitForTimeout(800); };

  await tap(w / 2, h / 2);          // the first touch is the trigger (a tap on the page in reading does nothing else)
  const b = await box();
  const H = b.h;
  const slot = (i, n = 5) => w * (i + 0.5) / n;
  const bar = H - 24;
  const sheetTop = (rows, tabs) => H - 48 - Math.min((52 + (tabs ? 48 : 0) + rows * 48 + 8), H * 0.6);
  await shot('reading');
  await tap(w - 16 - 28, H - 16 - 28);   // the pencil
  await tap(w * 0.4, 120);               // the caret goes where the finger is
  await shot('edit');
  // Format sheet, both tabs
  await tap(slot(0), bar);
  await shot('format-text');
  const t = sheetTop(5, true);
  await tap(w * 0.75, t + 52 + 24);
  await shot('format-para');
  await tap(w - 24, t + 4 + 24);         // close
  await tap(slot(2), bar);
  await shot('insert');
  await tap(w - 24, sheetTop(3, false) + 4 + 24);
  await tap(w - 24, 24);                 // More
  await shot('menu');
  await tap(w - 24, 24);
  // the fixture's table and picture: scroll them into view and put the caret / selection there
  await wheel(Number(opt('--table-scroll', 330)));
  await tap(w * 0.3, Number(opt('--table-y', 150)));
  await shot('table');
  await wheel(Number(opt('--pic-scroll', 250)));
  await tap(w * 0.3, Number(opt('--pic-y', 250)));
  await shot('picture');
  console.log(tag, 'done');
  await ctx.close();
} catch (e) {
  if (e.message !== 'desktop-only') throw e;
} finally {
  await browser.close();
  serve.stop();
  if (log.length) console.log(log.join('\n'));
}
