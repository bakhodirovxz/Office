// usage: node cdp-eval.js "<expr>"  -- evaluate in the 8781 tab of Android Chrome over CDP (adb forward tcp:9222 localabstract:chrome_devtools_remote)
const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const pages = b.contexts().flatMap((c) => c.pages());
  const page = [...pages].reverse().find((p) => p.url().includes(process.env.URLPART||'8781')) || pages[0];
  const r = await page.evaluate(process.argv[2]);
  console.log(typeof r === 'string' ? r : JSON.stringify(r));
  await b.close();
})().catch((e) => { console.error('ERR', e.message); process.exit(1); });
