// Spike: one-finger swipe in desktop Chromium with touch emulation (CDP Input.dispatchTouchEvent).
const { chromium } = require('@playwright/test');
const DIST = process.argv[2] || 'bridge';
(async () => {
  const b = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const ctx = await b.newContext({ viewport: { width: 412, height: 915 }, hasTouch: true, isMobile: true, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  page.on('console', (m) => { if (/DBG/.test(m.text())) console.log('console', m.type(), m.text().slice(0, 200)); });
  await page.goto('http://127.0.0.1:8790/?adapter=bridge');
  await page.waitForFunction(() => document.body.dataset.state === 'ready', null, { timeout: 150000 });
  await page.waitForTimeout(2000);
  const inner = page.frames().find((f) => f.url().includes('/wc/index.html'));
  await inner.evaluate(() => { window.__ev = {}; for (const n of ['touchstart','touchmove','touchend','touchcancel','pointerdown','pointermove','pointerup','pointercancel','mousedown']) document.addEventListener(n, (e) => { window.__ev[n] = (window.__ev[n]||0)+1; }, { capture: true, passive: true }); });
  const cdp = await ctx.newCDPSession(page);
  const t = (type, x, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x, y }] });
  const shot = async (n) => page.screenshot({ path: process.env.OUT + '/' + n + '.png' });
  await shot('d0-before');
  // slow swipe, 16 ms per event
  await t('touchStart', 300, 700);
  for (let i = 1; i <= 15; i++) { await t('touchMove', 300 - i * 12, 700 - i * 30); await page.waitForTimeout(16); }
  await t('touchEnd');
  await page.waitForTimeout(100);
  await shot('d1-just-after');
  await page.waitForTimeout(2500);
  await shot('d2-after-momentum');
  console.log(JSON.stringify(await inner.evaluate(() => window.__ev)));
  await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
