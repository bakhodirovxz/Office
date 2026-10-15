// Spike helper: evaluate JS in the HRM tab (or an inner frame) of Android Chrome over CDP.
// usage: node cdp-eval.js "<expr>" [frame-url-substring]
const { chromium } = require('@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const pages = b.contexts().flatMap((c) => c.pages());
  const page = [...pages].reverse().find((p) => p.url().includes('8790')) || pages[0];
  const target = process.argv[3] ? page.frames().find((f) => f.url().includes(process.argv[3])) : page;
  console.log(JSON.stringify(await target.evaluate(process.argv[2])));
  await b.close();
})().catch((e) => { console.error('ERR', e.message); process.exit(1); });
