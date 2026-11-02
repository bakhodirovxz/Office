// Phase 6 (docs/phase6/phone-layout.md section 9, owner decision 2026-11-03): a document opens in edit mode on a phone, the soft keyboard does not
// come up by itself, Done editing reads, and "Saved" shows as a toast. Desktop Chromium with touch emulation (CDP touches, 60 ms hold), DPR 1.
//   node scripts/mobile/phone-editfirst.mjs --dist <dist> [--w 360 --h 640] [--theme light] [--lang uz-Latn] [--out <dir>]
// Shots: <tag>-opened, <tag>-reading, <tag>-saved. Also prints the focused element of the editor frame before and after the first tap on the page, and
// whether a desktop window gets the toast (it must not).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-editfirst/ot/dist'));
const out = path.resolve(opt('--out', path.join(ROOT, 'docs/phase6/evidence/phone-ux')));
const [w, h] = [Number(opt('--w', 360)), Number(opt('--h', 640))];
const theme = opt('--theme', 'light');
const lang = opt('--lang', 'uz-Latn');
const tag = opt('--tag', `${w}-${theme}-${lang}-editfirst`);
fs.mkdirSync(out, { recursive: true });
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');
const serve = await L.startServe({ root: ROOT, dist });
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-unsafe-webgpu'] });
const fixture = path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx');
const opened = (page) => page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180000 });
const initScript = ([l, t]) => { try { localStorage.setItem('sh.lang', l); localStorage.setItem('sh.theme', t); } catch (_) { /* ignore */ } window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); };
const focused = (page) => page.evaluate(() => { const d = document.querySelector('#frames iframe:not([hidden])').contentDocument; const a = d.activeElement; return a ? `${a.tagName}${a.id ? '#' + a.id : ''}` : 'none'; });
try {
  // ---- phone
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, hasTouch: true, isMobile: true, locale: lang === 'ru' ? 'ru-RU' : 'uz-UZ', colorScheme: theme, acceptDownloads: true });
  await ctx.addInitScript(initScript, [lang, theme]);
  const page = await ctx.newPage();
  await page.goto(serve.base + '/', { waitUntil: 'load' });
  await page.waitForSelector('#cards li');
  await page.setInputFiles('#file', fixture);
  await opened(page);
  await page.waitForTimeout(4000);
  console.log('phone, just opened: frame focus =', await focused(page), '| shell header hidden =', await page.evaluate(() => getComputedStyle(document.querySelector('#editor > .bar')).display));
  await page.screenshot({ path: path.join(out, `${tag}-opened.png`) });
  const cdp = await ctx.newCDPSession(page);
  const box = () => page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y }; });
  const tap = async (x, y, wait = 1500) => {
    const b = await box();
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: b.x + x, y: b.y + y, id: 1 }] });
    await page.waitForTimeout(60);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await page.waitForTimeout(wait);
  };
  const sheetTop = (rows) => h - 48 - Math.min(52 + rows * 48 + 8, h * 0.6);
  await tap(150, 300, 2500);
  console.log('phone, after the first tap on the page: frame focus =', await focused(page));
  // More > Save: the toast
  await tap(w - 24, 24);
  const dl = page.waitForEvent('download', { timeout: 20000 }).catch(() => null);
  await tap(w / 2, sheetTop(5) + 52 + 48 + 24, 300);
  console.log('phone, Save: download =', (await dl)?.suggestedFilename() ?? 'none');
  await page.waitForTimeout(400);
  console.log('phone, toast text =', JSON.stringify(await page.locator('#toast').textContent()), 'class =', await page.locator('#toast').getAttribute('class'), 'role =', await page.locator('#toast').getAttribute('role'));
  await page.screenshot({ path: path.join(out, `${tag}-saved.png`) });
  await page.waitForTimeout(3200);
  console.log('phone, 3.6 s later: toast text =', JSON.stringify(await page.locator('#toast').textContent()), 'opacity =', await page.evaluate(() => getComputedStyle(document.getElementById('toast')).opacity));
  // More > Done editing: reading with the pencil
  await tap(w - 24, 24);
  await tap(w / 2, sheetTop(5) + 52 + 24, 2500);
  await page.screenshot({ path: path.join(out, `${tag}-reading.png`) });
  await ctx.close();
  // ---- desktop: the same save message must not make a toast
  const dctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, locale: 'en-US', acceptDownloads: true });
  await dctx.addInitScript(initScript, ['en', 'light']);
  const dp = await dctx.newPage();
  await dp.goto(serve.base + '/', { waitUntil: 'load' });
  await dp.waitForSelector('#cards li');
  await dp.setInputFiles('#file', fixture);
  await opened(dp);
  await dp.waitForTimeout(3000);
  await dp.frames().find((f) => f !== dp.mainFrame()).evaluate(() => parent.postMessage({ type: 'wordcraft:save', name: 'x.docx', bytes: new ArrayBuffer(8) }, location.origin));   // from inside the editor frame, as the editor does
  await dp.waitForTimeout(500);
  console.log('desktop, after a save: status line =', JSON.stringify(await dp.locator('#ed-status').textContent()), '| toast text =', JSON.stringify(await dp.locator('#toast').textContent()), '| toast opacity =', await dp.evaluate(() => getComputedStyle(document.getElementById('toast')).opacity));
  await dctx.close();
} finally {
  await browser.close();
  serve.stop();
}
