// Phase 2 WP10 N3: the transliteration command on the 108-page document in the docs-web wasm build, headless Chromium (Playwright).
//   node scripts/translit/perf-web.mjs [--rounds 6] [--dist <dir>] [--out result.json] [--cdp <url> --port <p> --base <url> --row <y> --only latn|cyrl] [--shots <dir>]
// The dist is served at /wc/ next to a host page that embeds it (`?host=parent`, the 0001 postMessage bridge) and opens the
// .docx with `wordcraft:open`. The command is run from the palette (Alt+Q "Tell me", type the label, Enter or a click on the row). In the page:
//   t0       the keydown of the final Enter (capture listener, performance.now())
//   longtask PerformanceObserver 'longtask' entries after t0 (the frame that ran the command and its layout)
//   frame    the first requestAnimationFrame callback after the last long task ends = the next presented frame
// total = frame - t0. --cdp connects to an existing Chrome (Android emulator via adb forward) instead of launching one.
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const require = createRequire(path.resolve('spikes/docserver/e2e/') + '/');
const { chromium } = require('playwright');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const rounds = Number(opt('--rounds', 6));
const dist = path.resolve(opt('--dist', path.join(process.env.OFFICE_TARGET || 'E:/office-target/p1', 'dist', 'docs-web')));
const shots = opt('--shots', null);
const fx = path.resolve('domains/docs/crates/docs-translit/tests/fixtures');
// [file, command label, y of the palette row to click, or 0 for Enter: the search ranks "Latin to Cyrillic" first for both labels]
const cases = [['uz108-latn.docx', 'Latin to Cyrillic', 0], ['uz108-cyrl.docx', 'Cyrillic to Latin', 379]];
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.png': 'image/png', '.css': 'text/css' };
const HOST = `<!doctype html><meta charset=utf-8><body style="margin:0"><iframe id=wc src="/wc/index.html?${process.argv.includes("--webgl") ? "webgl&" : ""}host=parent" style="border:0;width:100vw;height:100vh"></iframe>
<script>
window.log = [];
window.addEventListener('message', (ev) => { if (ev.data && typeof ev.data.type === 'string') window.log.push(ev.data.type + (ev.data.error ? ':' + ev.data.error : '')); });
window.openDoc = (name, b64) => { const bin = atob(b64); const u = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
  document.getElementById('wc').contentWindow.postMessage({ type: 'wordcraft:open', name, bytes: u.buffer }, location.origin, [u.buffer]); };
</script>`;
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split('?')[0]);
  if (url === '/host.html') return res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
  const file = path.join(dist, url.replace(/^\/wc\//, '/'));
  if (!url.startsWith('/wc/') || !file.startsWith(dist) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) return res.writeHead(404).end();
  res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream' });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(Number(opt('--port', 0)), '127.0.0.1', r));
const base = opt('--base', `http://127.0.0.1:${server.address().port}`);
const cdp = opt('--cdp', null);
const browser = cdp ? await chromium.connectOverCDP(cdp) : await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
if (shots) fs.mkdirSync(shots, { recursive: true });
const results = [];
for (const [name, label, rowY0] of cases) {
  if (opt('--only', null) && !name.includes(opt('--only'))) continue;
  const rowY = rowY0 && opt('--row', null) ? Number(opt('--row')) : rowY0;
  const b64 = fs.readFileSync(path.join(fx, name)).toString('base64');
  const rows = [];
  for (let rnd = 0; rnd < rounds; rnd++) {
    const ctx = cdp ? browser.contexts()[0] : await browser.newContext({ viewport: { width: 1280, height: 800 }, locale: 'en-US' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(`${base}/host.html`);
    await page.waitForFunction(() => window.log.includes('wordcraft:ready'), null, { timeout: 240000 });
    await page.evaluate(([n, d]) => window.openDoc(n, d), [name, b64]);
    await page.waitForFunction(() => window.log.includes('wordcraft:opened'), null, { timeout: 240000 });
    await page.waitForTimeout(12000); // first layout of 110 pages and the first frames
    const frame = page.frames().find((f) => f.url().includes('/wc/index.html'));
    await frame.evaluate(() => {
      window.__lt = []; window.__raf = []; window.__t0 = 0;
      new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lt.push([e.startTime, e.duration]); }).observe({ entryTypes: ['longtask'] });
      const tick = () => { window.__raf.push(performance.now()); requestAnimationFrame(tick); }; requestAnimationFrame(tick);
      window.addEventListener('keydown', (e) => { if (e.key === 'Enter') window.__t0 = performance.now(); }, true);
      window.addEventListener('mousedown', () => { window.__t0 = performance.now(); }, true);
    });
    const [vw, vh] = await page.evaluate(() => [innerWidth, innerHeight]);
    await page.mouse.click(vw / 2, vh * 0.6); // focus the page canvas
    await page.keyboard.press('Alt+q');
    await page.waitForTimeout(1500);
    await page.keyboard.type(label, { delay: 40 });
    await page.waitForTimeout(1500);
    if (shots && rnd === 0) await page.screenshot({ path: path.join(shots, `palette-${name}.png`) });
    await frame.evaluate(() => { window.__lt.length = 0; });
    if (rowY) await page.mouse.click(vw / 2, rowY); else await page.keyboard.press('Enter');
    await page.waitForTimeout(15000);
    const m = await frame.evaluate(() => {
      const t0 = window.__t0; const lts = window.__lt.filter(([s]) => s >= t0 - 50);
      const end = lts.length ? Math.max(...lts.map(([s, d]) => s + d)) : t0;
      const next = window.__raf.find((t) => t >= end);
      const longest = lts.length ? Math.max(...lts.map(([, d]) => d)) : 0;
      return { t0, longtasks: lts.length, longest_ms: longest, task_end_ms: end - t0, frame_ms: next === undefined ? null : next - t0 };
    });
    if (shots && rnd === 0) await page.screenshot({ path: path.join(shots, `after-${name}.png`) });
    const row = { round: rnd, ...m, errors: errors.slice(0, 3) };
    rows.push(row);
    console.log(name, JSON.stringify(row));
    await page.close();
    if (!cdp) await ctx.close();
  }
  const timed = rows.slice(1).map((r) => r.frame_ms).filter((x) => x != null).sort((a, b) => a - b);
  const median = timed.length ? timed[Math.floor(timed.length / 2)] : null;
  const summary = { file: name, command: label, timed: timed.length, median_frame_ms: median, max_frame_ms: timed.at(-1) ?? null };
  console.log(JSON.stringify(summary));
  results.push({ summary, rows });
}
if (opt('--out', null)) fs.writeFileSync(opt('--out'), JSON.stringify(results, null, 1));
await browser.close();
server.close();
