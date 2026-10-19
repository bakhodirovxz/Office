import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as path from 'node:path';

const DIST_ROOT = process.env.DIST_ROOT;
if (!DIST_ROOT) throw new Error('DIST_ROOT is not set');
// The brand name comes from the one brand config, never hard-coded.
const BRAND = /^name\s*=\s*"([^"]+)"/m.exec(
  fs.readFileSync(path.resolve(__dirname, '../../brand/brand.toml'), 'utf8').split('[product]')[1] ?? '',
)?.[1];
if (!BRAND) throw new Error('product name not found in brand/brand.toml');

const MIME: Record<string, string> = {
  '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon',
  '.css': 'text/css', '.json': 'application/json',
};

let server: http.Server;
let base: string;
test.beforeAll(async () => {
  server = http.createServer((req, res) => {
    const rel = decodeURIComponent((req.url ?? '/').split('?')[0]);
    const file = path.join(DIST_ROOT, rel.endsWith('/') ? rel + 'index.html' : rel);
    if (!file.startsWith(path.resolve(DIST_ROOT)) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
  base = `http://127.0.0.1:${(server.address() as any).port}`;
});
test.afterAll(() => server.close());

for (const d of ['docs', 'sheets', 'slides', 'pdf']) {
  test(`${d}-web starts`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(`${base}/${d}-web/`);
    // Running = module initialised, loading banner (docs/sheets/slides) removed, canvas has a size.
    await page.waitForFunction(
      () => {
        const c = document.querySelector('canvas');
        return !!(window as any).wasmBindings && !document.querySelector('[id$="_loading"]') && !!c && c.width > 0 && c.height > 0;
      },
      undefined,
      { timeout: 90_000 },
    );
    expect(await page.locator('canvas').count()).toBe(1);
    expect(await page.locator('body').innerText()).not.toContain('failed to start');
    expect(await page.title()).toContain(BRAND);
    expect(errors).toEqual([]);
  });
}
