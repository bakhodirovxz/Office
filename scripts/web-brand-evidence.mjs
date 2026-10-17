// Playwright evidence for a built web app (WP7/WP8): page title, a screenshot, and optionally the font test string.
//   node scripts/web-brand-evidence.mjs <docs|sheets|slides|pdf> <outdir> [--dist <dir>] [--font] [--tag <name>]
// The dist is served from $OFFICE_TARGET/dist/<d>-web (or --dist). Headless Chromium with SwiftShader (as the e2e spikes).
// Output: <outdir>/<tag>-<d>-web.png (the page after start) and, with --font, <outdir>/font-<d>-web.png with the test string typed
// into the app's own text field. Prints one JSON line with the title and the console errors.
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
const font = args.includes('--font');
const TEXT = 'Oʻzbekiston gʻoya maʼno Ўзбекистон қўғирчоқ ғоя ҳа ₽ №';
fs.mkdirSync(outdir, { recursive: true });

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
const port = server.address().port;

const browser = await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
const page = await browser.newPage({ viewport: { width: 1100, height: 680 } });
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
await page.goto(`http://127.0.0.1:${port}/`);
const loading = page.locator('[id$="_loading"]');
await page.waitForFunction(() => !document.querySelector('[id$="_loading"]'), null, { timeout: 120000 }).catch(() => errors.push('loading element still present'));
await page.waitForTimeout(4000);
const title = await page.title();
await page.screenshot({ path: path.join(outdir, `${tag}-${d}-web.png`) });
let fontShot = null;
if (font) {
  // Each app's own text field: the command palette (Ctrl+K), or the formula bar for the sheet.
  await page.mouse.click(550, 400);
  if (d === 'sheets') {
    await page.keyboard.press('F2');
  } else {
    await page.keyboard.press('Control+K');
  }
  await page.waitForTimeout(1500);
  await page.keyboard.insertText(TEXT);
  await page.waitForTimeout(2000);
  fontShot = path.join(outdir, `font-${d}-web.png`);
  await page.screenshot({ path: fontShot });
}
console.log(JSON.stringify({ app: d, dist, title, errors: errors.slice(0, 5), screenshot: `${tag}-${d}-web.png`, font: fontShot && path.basename(fontShot) }));
await browser.close();
server.close();
