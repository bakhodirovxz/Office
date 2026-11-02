// Serves web/ plus the built files and drives bench.html in headless Chromium (Playwright from another checkout).
// Usage: node web/run.mjs <dir with plugin.wasm, plugin-imp.wasm, input.bin, native.bin> <playwright module path>
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const [dataDir, pwPath] = process.argv.slice(2);
const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.bin': 'application/octet-stream' };
const srv = http.createServer((req, res) => {
  const name = req.url.slice(1).split('?')[0];
  const file = [path.join(here, name), path.join(dataDir, name)].find((f) => name && fs.existsSync(f));
  if (!file) { res.writeHead(404); return res.end(); }
  const h = { 'content-type': types[path.extname(file)] || 'application/octet-stream' };
  if (name === 'worker.js') h['content-security-policy'] = "default-src 'none'";
  res.writeHead(200, h);
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => srv.listen(0, '127.0.0.1', r));
const { chromium } = createRequire(import.meta.url)(pwPath);
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(`http://127.0.0.1:${srv.address().port}/bench.html`);
await page.waitForFunction(() => window.result, null, { timeout: 120000 });
console.log(JSON.stringify(await page.evaluate(() => window.result), null, 1));
console.log('chromium', browser.version());
await browser.close();
srv.close();
