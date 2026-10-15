// Spike helper: print console messages (matching argv[2]) from the Android HRM tab and its frames for N seconds.
const { chromium } = require('@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const pages = b.contexts().flatMap((c) => c.pages());
  const page = [...pages].reverse().find((p) => p.url().includes('8790')) || pages[0];
  const re = new RegExp(process.argv[2] || '.');
  page.on('console', (m) => { if (re.test(m.text())) console.log(new Date().toISOString().slice(14, 23), m.text().replace(/^.*?DBG/, 'DBG').slice(0, 300)); });
  await new Promise((r) => setTimeout(r, Number(process.argv[3] || 15) * 1000));
  await b.close();
})();
