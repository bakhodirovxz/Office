const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const page = [...b.contexts().flatMap((c) => c.pages())].reverse().find((p) => p.url().includes('8781'));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Network.enable'); await cdp.send('Network.clearBrowserCache');
  await page.reload();
  await page.waitForFunction(() => window.__perf && window.__perf.ready, null, { timeout: 120000 });
  console.log(JSON.stringify(await page.evaluate(() => __perf.load)));
  await b.close();
})();
