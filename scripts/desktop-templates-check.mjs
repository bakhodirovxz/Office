// Phase 4 integration: the 5 starter templates opened from the shell inside hoshiya.exe (debug build: the SUITE_* hooks), through the
// WebView2 remote debugging port. Each must reach the editor's open message with no page error; screenshots of the host window content go to OUT.
// Usage: node scripts/desktop-templates-check.mjs [hoshiya.exe] [dist dir] [out dir]      (needs ci/web-smoke/node_modules)
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import * as net from 'node:net';

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '..');
const { chromium } = createRequire(path.join(ROOT, 'ci/web-smoke/package.json'))('playwright');
const EXE = process.argv[2] ?? 'E:/office-target/p1/services/debug/hoshiya.exe';
const DIST = process.argv[3] ?? 'E:/office-target/p1/dist';
const OUT = process.argv[4] ?? path.join(ROOT, 'docs/phase4/evidence/templates/desktop');
fs.mkdirSync(OUT, { recursive: true });
const catalog = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/shell/templates.json'), 'utf8'));
const free = () => new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (name, ok, extra = '') => { results.push({ name, ok }); console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? '  ' + extra : ''}`); };

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hoshiya-templates-'));
const data = path.join(dir, 'data');
const port = await free();
// the windows-gnu build imports WebView2Loader.dll: it must sit next to the exe (the msvc build links it statically)
const app = path.join(dir, 'app');
fs.mkdirSync(app);
fs.copyFileSync(EXE, path.join(app, 'hoshiya.exe'));
const find = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? find(path.join(d, e.name)) : e.name === 'WebView2Loader.dll' && path.basename(d) === 'x64' ? [path.join(d, e.name)] : []));
const loader = fs.existsSync(path.join(path.dirname(EXE), 'WebView2Loader.dll')) ? path.join(path.dirname(EXE), 'WebView2Loader.dll') : find(path.join(path.dirname(EXE), 'build'))[0];
if (loader) fs.copyFileSync(loader, path.join(app, 'WebView2Loader.dll'));
const host = spawn(path.join(app, 'hoshiya.exe'), ['--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml'), '--bin-dir', dir], {
  env: { ...process.env, SUITE_DATA_DIR: data, SUITE_INSTANCE: 'tplchk' + process.pid, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=*` },
  stdio: ['ignore', 'ignore', 'inherit'],
});
let browser;
try {
  for (let i = 0; i < 120 && !browser; i++) {
    try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); } catch { await sleep(500); }
  }
  if (!browser) throw new Error('no WebView2 debugging port');
  let page;
  for (let i = 0; i < 60 && !page; i++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => /^http:\/\/127\.0\.0\.1:\d+\//.test(p.url()));
    if (!page) await sleep(500);
  }
  if (!page) throw new Error('the shell page was not found');
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy/i.test(m.text())) problems.push(m.text()); });
  await page.waitForSelector('#templates li');
  check('the shell runs in the host (window.ipc present)', await page.evaluate(() => !!window.ipc));
  await page.selectOption('#lang', 'uz-Latn');
  await page.evaluate(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
  check('five templates listed', (await page.locator('#templates li').count()) === catalog.length, String(catalog.length));
  for (const [i, t] of catalog.entries()) {
    const name = path.basename(t.file, path.extname(t.file));
    if (await page.locator('#editor').isVisible()) await page.click('#back');
    const before = await page.evaluate(() => window.__msgs.length);
    await page.locator('#templates li').nth(i).locator('button').click();
    await page.waitForFunction((r) => !!document.querySelector(`iframe[src^="${r}"]`), `/${t.app}/`);
    const done = t.app === 'sheets' ? ':ready' : ':opened'; // gridcraft's bridge has no opened reply
    let ok = false;
    // an editor that is already loaded sends no second `ready`, so Sheets looks at every message so far
    for (let w = 0; w < 360 && !ok; w++) { ok = (await page.evaluate((n) => window.__msgs.slice(n), t.app === 'sheets' ? 0 : before)).some((m) => String(m).endsWith(done)); if (!ok) await sleep(500); }
    await sleep(t.app === 'sheets' ? 12000 : 4000);
    const failed = (await page.evaluate((n) => window.__msgs.slice(n), before)).filter((m) => String(m).endsWith(':failed'));
    await page.screenshot({ path: path.join(OUT, `${name}.png`) });
    check(`${name} opens in ${t.app} inside hoshiya.exe`, ok && failed.length === 0, path.join(OUT, `${name}.png`));
  }
  check('no page error or CSP violation', problems.length === 0, problems.join(' | '));
} finally {
  host.kill();
  await sleep(500);
  try { fs.rmSync(dir, { recursive: true, force: true }); } catch { /* the host may still hold its exe for a moment */ }
}
const bad = results.filter((r) => !r.ok).length;
console.log(`${results.length - bad}/${results.length} checks passed`);
process.exit(bad ? 1 : 0);
