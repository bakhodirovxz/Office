// B2 done-check in desktop Chromium (mobile emulation): load, tap places the caret, typing works, Save posts a docx.
// usage: node desktop-check.js http://127.0.0.1:8781 OUTDIR
const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
const fs = require('fs');
(async () => {
  const base = process.argv[2], out = process.argv[3];
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 412, height: 915 }, hasTouch: true, isMobile: true, deviceScaleFactor: 2.625 });
  const page = await ctx.newPage();
  const logs = [];
  page.on('console', (m) => logs.push(m.type() + ': ' + m.text().slice(0, 300)));
  page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
  const t0 = Date.now();
  await page.goto(base + '/index.html?doc=sample.docx');
  await page.waitForFunction(() => window.__perf && window.__perf.ready, null, { timeout: 120000 });
  console.log('ready after', Date.now() - t0, 'ms', JSON.stringify(await page.evaluate(() => window.__perf.load)));
  await page.screenshot({ path: out + '/d1-loaded.png' });
  // tap in the text: first page, near the first line
  const box = await page.evaluate(() => { const r = document.querySelector('.page').getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  await page.touchscreen.tap(box.x + box.w * 0.3, box.y + box.h * 0.1);
  await page.waitForTimeout(300);
  console.log('focused ime:', await page.evaluate(() => document.activeElement && document.activeElement.id), 'tap_ms', await page.evaluate(() => window.__perf.tap_ms));
  const before = await page.evaluate(() => window.__wc.ed.plain_text());
  await page.keyboard.type('Hello Салом oʻgʻil', { delay: 20 });
  await page.waitForTimeout(500);
  const after = await page.evaluate(() => window.__wc.ed.plain_text());
  console.log('typed ok:', after.includes('Hello Салом oʻgʻil'), JSON.stringify(after.slice(0, 120)));
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Enter');
  await page.keyboard.type('Second');
  await page.waitForTimeout(300);
  console.log('after bs/enter:', JSON.stringify((await page.evaluate(() => window.__wc.ed.plain_text())).slice(0, 160)));
  await page.click('#bBold'); await page.click('#bUndo'); await page.click('#bZoom');
  await page.waitForTimeout(800);
  await page.screenshot({ path: out + '/d2-typed.png' });
  await page.click('#bSave');
  await page.waitForTimeout(800);
  console.log('save perf', JSON.stringify(await page.evaluate(() => window.__perf.save)));
  console.log('summary', JSON.stringify(await page.evaluate(() => window.__perf.summary())));
  console.log('edits', JSON.stringify(await page.evaluate(() => window.__perf.edits.slice(-5))));
  console.log('inputTypes', JSON.stringify(await page.evaluate(() => window.__perf.inputTypes.slice(-12))));
  fs.writeFileSync(out + '/d-console.txt', logs.join('\n'));
  console.log('console lines', logs.length, logs.slice(0, 5));
  await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
