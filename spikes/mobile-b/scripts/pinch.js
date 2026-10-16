const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const page = [...b.contexts().flatMap((c) => c.pages())].reverse().find((p) => p.url().includes('8781'));
  const cdp = await page.context().newCDPSession(page);
  const scale = +process.argv[2] || 2;
  await page.evaluate(() => { __perf.tiles.length = 0; __perf.events.length = 0; });
  const t0 = Date.now();
  await cdp.send('Input.synthesizePinchGesture', { x: +(process.argv[4]||206), y: +(process.argv[3]||400), scaleFactor: scale, relativeSpeed: 400, gestureSourceType: 'touch' });
  await page.waitForTimeout(1500);
  console.log(JSON.stringify(await page.evaluate(() => ({ vv: visualViewport.scale, pinch: __perf.events.filter(e => e[1] === 'pinch'), tiles: __perf.tiles.slice(-6), layoutW: innerWidth, cpp: __wc.cssPerPt }))));
  await b.close();
})();
