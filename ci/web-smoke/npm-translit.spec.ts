import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as path from 'node:path';

// PKG_DIR: the unpacked npm package (set by ci.sh translit-npm); SHOT: where to write the screenshot.
const PKG_DIR = process.env.PKG_DIR;
if (!PKG_DIR) throw new Error('PKG_DIR is not set');
const PAGE_DIR = path.resolve(__dirname, '../npm/smoke');
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

let server: http.Server;
test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const rel = decodeURIComponent((req.url ?? '/').split('?')[0]);
    const file = rel.startsWith('/pkg/') ? path.join(PKG_DIR, rel.slice(5)) : path.join(PAGE_DIR, rel === '/' ? 'index.html' : rel);
    if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404).end(); return; }
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
});
test.afterAll(() => server.close());

test('npm package converts text in Chromium', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${(server.address() as any).port}/`);
  await page.waitForSelector('body[data-ready="1"]');
  const out = await page.locator('.out').allTextContents();
  expect([out[0], out[1], out[2], out[4]]).toEqual([
    'Ўзбекистон Республикаси, Тошкент шаҳри',
    'Oʻzbekiston Respublikasi, Toshkent shahri',
    'Özbekiston, Toşkent şahri',
    'cyrl',
  ]);
  expect(out[3].replace(/[ |]/g, '')).toBe('шаҳар');
  if (process.env.SHOT) await page.screenshot({ path: process.env.SHOT });
  expect(errors).toEqual([]);
});
