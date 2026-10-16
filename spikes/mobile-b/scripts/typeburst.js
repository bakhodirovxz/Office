// keystroke -> repaint latency on the device, using CDP key events (no Gboard): types N chars into the focused editor.
const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const page = [...b.contexts().flatMap((c) => c.pages())].reverse().find((p) => p.url().includes('8781'));
  await page.evaluate(() => { __perf.edits.length = 0; });
  await page.keyboard.type('the quick brown fox jumps over the lazy dog', { delay: 120 });
  await page.waitForTimeout(800);
  const edits = await page.evaluate(() => __perf.edits.slice());
  const p = (k, q) => { const s = edits.map((x) => x[k]).sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(s.length * q))]; };
  const res = { n: edits.length };
  for (const k of ['engine_ms', 'first_tile_ms', 'handler_to_first_paint_ms', 'event_to_raf_ms']) res[k] = { p50: p(k, .5), p95: p(k, .95), max: p(k, .999) };
  console.log(JSON.stringify(res)); require('fs').writeFileSync(process.argv[2], JSON.stringify({ res, edits }));
  await b.close();
})();
