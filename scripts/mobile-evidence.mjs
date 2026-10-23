// Phase 4 WP16: the shell and the four editors at phone viewports in desktop Chromium (touch, DPR, optional CPU throttling).
//   node scripts/mobile-evidence.mjs [--out docs/phase4/evidence/mobile] [--cpu 1] [--only docs,sheets] [--json result.json]
// Needs the release dists (E:/office-target/p1/dist) and hoshiya-serve (E:/office-target/p1/services/release). Measures per
// viewport and editor: load (card tap -> wasm up), CDP JS heap, wasm memory, horizontal overflow, a typed edit and Ctrl+S.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(path.resolve('ci/web-smoke') + '/');
const { chromium } = require('@playwright/test');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const OUT = path.resolve(opt('--out', 'docs/phase4/evidence/mobile'));
const CPU = Number(opt('--cpu', 1));
const ONLY = opt('--only', 'docs,sheets,slides,pdf').split(',');
const ROOT = process.cwd();
const SVC = 'E:/office-target/p1/services/release';
const DIST = 'E:/office-target/p1/dist';
const VIEWPORTS = [
  { name: '360x800', width: 360, height: 800, deviceScaleFactor: 3 },
  { name: '412x915', width: 412, height: 915, deviceScaleFactor: 2.625 },
];
const SURFACES = [['docs'], ['sheets'], ['slides'], ['pdf']];
fs.mkdirSync(OUT, { recursive: true });

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mobile-ev-'));
const port = await new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
const proc = spawn(path.join(SVC, 'hoshiya-serve.exe'), ['--config', path.join(dir, 'api.toml'), '--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')]);
const base = await new Promise((ok, bad) => {
  let o = ''; proc.stdout.on('data', (d) => { o += d; const m = /listening on (http:\/\/\S+)/.exec(o); if (m) ok(m[1]); });
  setTimeout(() => bad(new Error('serve did not start')), 15000);
});

const browser = await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
const results = [];
const metrics = async (cdp) => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));
const mb = (x) => Math.round((x / 1048576) * 10) / 10;

for (const vp of VIEWPORTS) {
  for (const [id] of [['shell']].concat(SURFACES.filter(([i]) => ONLY.includes(i)))) {
    const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor, hasTouch: true, isMobile: true, acceptDownloads: true });
    const page = await ctx.newPage();
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Performance.enable');
    if (CPU > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: CPU });
    await page.addInitScript(() => { // capture the wasm memory of every instance (the module namespace hides __wasm)
      window.__mems = [];
      for (const k of ['instantiate', 'instantiateStreaming']) { const o = WebAssembly[k]; WebAssembly[k] = async (...a) => { const r = await o.apply(WebAssembly, a); const i = r.instance || r; if (i.exports && i.exports.memory) window.__mems.push(i.exports.memory); return r; }; }
    });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e).slice(0, 200)));
    const row = { viewport: vp.name, surface: id, cpu: CPU };
    const t0 = Date.now();
    await page.goto(base + '/', { waitUntil: 'load' });
    row.shell_load_ms = Date.now() - t0;
    row.shell_overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    if (id === 'shell') {
      await page.screenshot({ path: path.join(OUT, `${vp.name}-shell.png`) });
      row.heap_mb = mb((await metrics(cdp)).JSHeapUsedSize);
    } else {
      const idx = SURFACES.findIndex(([i]) => i === id);
      const t1 = Date.now();
      if (id === 'pdf') await page.setInputFiles('#file', path.join(ROOT, 'spikes/workspace/src/pdfcraft/crates/ui-egui/tests/data/form.pdf'));
      else await page.locator('#cards li').nth(idx).getByRole('button').tap();
      await page.waitForFunction((r) => !!document.querySelector(`iframe[src^="${r}"]`), `/${id}/`);
      const frame = page.frames().find((f) => f.url().includes(`/${id}/`));
      try {
        await frame.waitForFunction(() => !!window.wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 180000 });
        row.load_ms = Date.now() - t1;
      } catch { row.load_ms = null; row.load_error = 'timeout 180 s'; }
      await page.waitForTimeout(3000);
      const m = await metrics(cdp);
      row.heap_mb = mb(m.JSHeapUsedSize); row.heap_total_mb = mb(m.JSHeapTotalSize);
      row.wasm_mb = await frame.evaluate(() => { try { return Math.round(Math.max(...window.__mems.map((m) => m.buffer.byteLength)) / 104857.6) / 10; } catch { return null; } });
      row.iframe = await page.evaluate(() => { const r = document.querySelector('iframe').getBoundingClientRect(); return [Math.round(r.width), Math.round(r.height)]; });
      row.overflow_x = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
      await page.screenshot({ path: path.join(OUT, `${vp.name}-${id}-open.png`) });
      // type and save: tap into the canvas, type, Ctrl+S -> the shell downloads the bytes
      if (id !== 'pdf') {
        const box = await page.locator(`iframe[src^="/${id}/"]`).boundingBox();
        const at = { docs: [0.5, 0.45], sheets: [0.3, 0.3], slides: [0.5, 0.35] }[id];
        const tx = box.x + box.width * at[0], ty = box.y + box.height * at[1];
        await page.touchscreen.tap(tx, ty);
        if (id === 'slides') { await page.touchscreen.tap(tx, ty); }
        await page.waitForTimeout(500);
        await page.keyboard.type('Salom Hoshiya', { delay: 30 });
        if (id === 'sheets') await page.keyboard.press('Enter');
        await page.waitForTimeout(800);
        await page.screenshot({ path: path.join(OUT, `${vp.name}-${id}-typed.png`) });
        try {
          const [dl] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), page.keyboard.press('Control+s')]);
          const b = fs.readFileSync(await dl.path());
          fs.writeFileSync(path.join(os.tmpdir(), `mev-${vp.name}-${id}.zip`), b);
          row.saved = { name: dl.suggestedFilename(), bytes: b.length, zip: b.subarray(0, 2).toString() === 'PK' };
        } catch { row.saved = null; }
      }
    }
    row.errors = errors.slice(0, 3);
    console.log(JSON.stringify(row));
    results.push(row);
    await ctx.close();
  }
}
await browser.close();
proc.kill();
fs.rmSync(dir, { recursive: true, force: true });
if (opt('--json', null)) fs.writeFileSync(opt('--json'), JSON.stringify(results, null, 1));
