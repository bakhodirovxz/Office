// Playwright evidence for a built web app (WP7/WP8): page title, About dialog, the font test string, console errors.
//   node scripts/web-brand-evidence.mjs <docs|sheets|slides|pdf> <outdir> [--dist <dir>] [--tag <prefix>]
// The dist is served from $OFFICE_TARGET/dist/<d>-web (or --dist). Headless Chromium with SwiftShader (as the e2e spikes).
// Output in <outdir>: <prefix>-<d>-web.png (the page after start), <prefix>-<d>-web-about.png (About dialog) and
// font-<d>-web.png (the test string typed into the app's own field). Prints one JSON line with the page title.
// The step lists below are the click paths of each app (window 1100x680, locale en-US); they are evidence helpers, not tests.
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const require = createRequire(path.resolve('spikes/docserver/e2e/') + '/');
const { chromium } = require('playwright');

const args = process.argv.slice(2);
const d = args[0];
const outdir = path.resolve(args[1]);
const opt = (name, dflt) => (args.includes(name) ? args[args.indexOf(name) + 1] : dflt);
const dist = path.resolve(opt('--dist', path.join(process.env.OFFICE_TARGET || 'E:/office-target/p1', 'dist', `${d}-web`)));
const tag = opt('--tag', 'brand');
const TEXT = 'Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №';
fs.mkdirSync(outdir, { recursive: true });

// [op, a, b]: click x y | key K | insert (the test text) | text T | wait ms | shot name
const STEPS = {
  pdf: [
    ['wait', 500], ['shot', `${tag}-pdf-web`],
    ['key', 'Control+K'], ['wait', 1000], ['text', 'About'], ['wait', 1000], ['key', 'Enter'], ['wait', 2000], ['shot', `${tag}-pdf-web-about`],
    ['key', 'Escape'], ['wait', 500], ['key', 'Control+K'], ['wait', 1000], ['insert'], ['wait', 1500], ['shot', 'font-pdf-web'],
  ],
  docs: [
    ['wait', 500], ['shot', `${tag}-docs-web`],
    ['click', 315, 180], ['wait', 12000], ['click', 600, 300], ['insert'], ['wait', 6000], ['shot', 'font-docs-web'],
    ['click', 706, 53], ['wait', 2000], ['click', 45, 100], ['wait', 3000], ['shot', `${tag}-docs-web-about`],
  ],
  sheets: [
    ['wait', 500], ['shot', `${tag}-sheets-web`],
    ['click', 631, 53], ['wait', 1000], ['click', 272, 110], ['wait', 2000], ['shot', `${tag}-sheets-web-about`],
    ['key', 'Escape'], ['wait', 500], ['click', 700, 400], ['click', 300, 200], ['insert'], ['key', 'Enter'], ['wait', 1500], ['shot', 'font-sheets-web'],
  ],
  slides: [
    ['wait', 500], ['shot', `${tag}-slides-web`],
    ['click', 22, 18], ['wait', 2500], ['click', 73, 612], ['wait', 2500], ['click', 638, 262], ['wait', 3000], ['shot', `${tag}-slides-web-about`],
    ['key', 'Escape'], ['wait', 1000], ['click', 26, 62], ['wait', 2500], ['click', 668, 343], ['click', 668, 343], ['wait', 1000], ['key', 'Control+A'], ['insert'], ['wait', 4000], ['shot', 'font-slides-web'],
  ],
};
if (process.env.STEPS_FILE) STEPS[d] = JSON.parse(fs.readFileSync(process.env.STEPS_FILE, 'utf8'));

const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.png': 'image/png', '.css': 'text/css' };
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split('?')[0]);
  const file = path.join(dist, url === '/' ? 'index.html' : url);
  if (!file.startsWith(dist) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream' });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));

const browser = await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
const ctx = await browser.newContext({ viewport: { width: 1100, height: 680 }, locale: 'en-US' });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => !document.querySelector('[id$="_loading"]'), null, { timeout: Number(process.env.START_TIMEOUT || 240000) }).catch(() => errors.push('start screen not reached'));
await page.waitForTimeout(3000);
const title = await page.title();
const shots = [];
for (const [op, a, b] of STEPS[d]) {
  if (op === 'click') await page.mouse.click(a, b);
  else if (op === 'key') await page.keyboard.press(a);
  else if (op === 'insert') await page.keyboard.insertText(TEXT);
  else if (op === 'text') await page.keyboard.insertText(a);
  else if (op === 'wait') await page.waitForTimeout(a);
  else if (op === 'shot') { await page.screenshot({ path: path.join(outdir, `${a}.png`) }); shots.push(`${a}.png`); }
}
console.log(JSON.stringify({ app: d, dist, title, errors: errors.slice(0, 5), shots }));
await browser.close();
server.close();
