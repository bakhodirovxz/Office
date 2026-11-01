// Phase 4 WP16 M3 diagnosis: the 108-page docx opened from the shell at a phone viewport in desktop Chromium, stage by stage.
//   --cdp [url] [--serial emulator-5554]: drive the stock Chrome on a running emulator (scripts/mobile/rig.mjs start/forward) over CDP instead of desktop
//   Chromium: the page is served through `adb reverse`, the viewport/CPU are the device's own, and a logcat excerpt (renderer kills) is saved.
//   node scripts/mobile-m3.mjs [--file <docx>] [--cpu 1] [--width 360] [--height 800] [--heap 0] [--wasmcap 0] [--timeout 240] [--tag name] [--out dir]
// Stages (ms from the file hand-over to the shell): iframe created, wordcraft:ready, wordcraft:opened (parse done), first frame
// after the last long task (layout done and painted). Samples the wasm memory (page-captured WebAssembly.Memory) and the CDP JS
// heap every 500 ms. --wasmcap MB makes WebAssembly.Memory.grow throw past that size (a phone's per-tab wasm limit proxy);
// --heap MB passes --js-flags=--max-old-space-size (JS heap only, wasm memory is not on the JS heap).
import { createRequire } from 'node:module';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const pwDir = ['ci/web-smoke', process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.resolve(d, 'node_modules')));
const require = createRequire(path.resolve(pwDir) + '/');
const { chromium } = require('@playwright/test');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const FILE = path.resolve(opt('--file', 'domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx'));
const CPU = Number(opt('--cpu', 1)), W = Number(opt('--width', 360)), H = Number(opt('--height', 800));
const HEAP = Number(opt('--heap', 0)), CAP = Number(opt('--wasmcap', 0)), TMO = Number(opt('--timeout', 240)) * 1000;
const TAG = opt('--tag', `cpu${CPU}-${W}x${H}${HEAP ? `-heap${HEAP}` : ''}${CAP ? `-cap${CAP}` : ''}`);
const CDPI = args.indexOf('--cdp'), CDPURL = CDPI < 0 ? null : (args[CDPI + 1] && !args[CDPI + 1].startsWith('--') ? args[CDPI + 1] : 'http://127.0.0.1:9222');
const SER = opt('--serial', 'emulator-5554');
const OUT = path.resolve(opt('--out', 'docs/phase4/evidence/mobile'));
const SVC = 'E:/office-target/p1/services/release', DIST = opt('--dist', 'E:/office-target/p1/dist');
fs.mkdirSync(OUT, { recursive: true });

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mobile-m3-'));
const port = await new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
const proc = spawn(path.join(SVC, 'hoshiya-serve.exe'), ['--config', path.join(dir, 'api.toml'), '--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(process.cwd(), 'brand/brand.toml')]);
const base = await new Promise((ok, bad) => { let o = ''; proc.stdout.on('data', (d) => { o += d; const m = /listening on (http:\/\/\S+)/.exec(o); if (m) ok(CDPURL ? `http://localhost:${port}` : m[1]); }); setTimeout(() => bad(new Error('serve did not start')), 15000); });

const flags = ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'];
if (HEAP) flags.push(`--js-flags=--max-old-space-size=${HEAP}`);
let browser, ctx, page;
if (CDPURL) {
  spawnSync('adb', ['-s', SER, 'reverse', `tcp:${port}`, `tcp:${port}`]);
  spawnSync('adb', ['-s', SER, 'logcat', '-c']);
  browser = await chromium.connectOverCDP(CDPURL);
  ctx = browser.contexts()[0];
  page = await ctx.newPage();
} else {
  browser = await chromium.launch({ headless: true, args: flags });
  ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 3, hasTouch: true, isMobile: true });
  page = await ctx.newPage();
}
const cdp = await ctx.newCDPSession(page);
await cdp.send('Performance.enable');
if (CPU > 1 && !CDPURL) await cdp.send('Emulation.setCPUThrottlingRate', { rate: CPU });
await page.addInitScript((cap) => {
  window.__mems = []; window.__marks = {}; window.__lt = []; window.__grow = [];
  for (const k of ['instantiate', 'instantiateStreaming']) { const o = WebAssembly[k]; WebAssembly[k] = async (...a) => { const r = await o.apply(WebAssembly, a); const i = r.instance || r; if (i.exports && i.exports.memory) window.__mems.push(i.exports.memory); window.__marks.instantiated = performance.now(); return r; }; }
  if (cap) { const g = WebAssembly.Memory.prototype.grow; WebAssembly.Memory.prototype.grow = function (n) { if ((this.buffer.byteLength / 65536 + n) * 65536 > cap * 1048576) { window.__marks.capHit = performance.now(); throw new RangeError('wasm memory cap'); } return g.call(this, n); }; }
  if (window === top) addEventListener('message', (e) => { const t = e.data && e.data.type; if (typeof t === 'string') window.__marks[t] = window.__marks[t] ?? performance.now(); });
  {
    try { new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lt.push([e.startTime, e.duration]); }).observe({ entryTypes: ['longtask'] }); } catch {}
  }
}, CAP);
const errors = [];
page.on('pageerror', (e) => errors.push(String(e).slice(0, 200)));
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text().slice(0, 200)); });
await page.goto(base + '/', { waitUntil: 'load' });
if (CDPURL) { const t = (await (await fetch(`${CDPURL}/json/list`)).json()).find((x) => x.type === 'page' && x.url.startsWith(base)); if (t) await fetch(`${CDPURL}/json/activate/${t.id}`); } // the tab under test must be in the foreground
if (args.includes('--profile')) { await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 500 }); await cdp.send('Profiler.start'); }
const t0 = await page.evaluate(() => performance.now());
await page.setInputFiles('#file', FILE);
const samples = [];
const ed = () => page.frames().find((f) => f.url().includes('/docs/'));
const edState = () => ed()?.evaluate(() => ({ wasm: Math.max(0, ...window.__mems.map((x) => x.buffer.byteLength)), lt: window.__lt, inst: window.__marks.instantiated, now: performance.now() })).catch(() => null);
const start = Date.now();
let opened = false;
while (Date.now() - start < TMO) {
  await page.waitForTimeout(500);
  const [m, s] = await Promise.all([cdp.send('Performance.getMetrics'), page.evaluate(() => ({ marks: window.__marks })).then(async (r) => ({ ...r, wasm: (await edState())?.wasm ?? 0 }))]);
  const heap = m.metrics.find((x) => x.name === 'JSHeapUsedSize').value;
  samples.push([Date.now() - start, Math.round(heap / 1048576), s ? Math.round(s.wasm / 1048576) : null]);
  if (s && s.marks['wordcraft:opened'] && !opened) { opened = true; }
  if (opened && Date.now() - start > 0) {
    // settle: no long task for 3 s after opened
    const st = await edState(); const lt = st?.lt.at(-1);
    if (lt && st.now - (lt[0] + lt[1]) > 3000) break;
  }
}
const top0 = await page.evaluate(() => ({ marks: window.__marks, now: performance.now() }));
const es = await edState();
// the frame's clock starts with the frame; the shell's timeOrigin is the same page, so offset by the difference of the two clocks
const off = es ? top0.now - es.now : 0;
const fin = { marks: top0.marks, lt: (es?.lt ?? []).map(([a, b]) => [a + off, b]), mem: [es?.wasm ?? 0] };
fin.marks.instantiated = es?.inst != null ? es.inst + off : null;
const rel = (v) => (v == null ? null : Math.round(v - t0));
const lts = fin.lt.filter(([s]) => s >= t0);
const last = lts.length ? Math.max(...lts.map(([s, d]) => s + d)) : null;
const res = {
  tag: TAG, cpu: CPU, viewport: `${W}x${H}`, heap_cap_mb: HEAP || null, wasm_cap_mb: CAP || null,
  ms: { wasm_instantiated: rel(fin.marks.instantiated), ready: rel(fin.marks['wordcraft:ready']), opened: rel(fin.marks['wordcraft:opened']), last_longtask_end: rel(last), cap_hit: rel(fin.marks.capHit) },
  longtasks: lts.length, longest_ms: lts.length ? Math.round(Math.max(...lts.map(([, d]) => d))) : 0,
  busy_ms_by_10s: (() => { const b = {}; for (const [st, d] of lts) { const k = Math.floor((st - t0) / 10000) * 10; b[k] = (b[k] || 0) + Math.round(d); } return b; })(),
  top_longtasks: lts.slice().sort((a, b) => b[1] - a[1]).slice(0, 5).map(([s, d]) => [rel(s), Math.round(d)]),
  wasm_mb_max: Math.round(Math.max(0, ...fin.mem) / 1048576), heap_mb_max: Math.max(...samples.map((x) => x[1])),
  opened: !!fin.marks['wordcraft:opened'], elapsed_s: Math.round((Date.now() - start) / 100) / 10,
  cdp: CDPURL ? { serial: SER, viewport: await page.evaluate(() => `${innerWidth}x${innerHeight}@${devicePixelRatio}`), renderer_kill_lines: (() => { const l = spawnSync('adb', ['-s', SER, 'logcat', '-d'], { encoding: 'utf8', maxBuffer: 1 << 28 }).stdout.split('\n').filter((x) => /Process .*(died|killed)|lowmemorykiller|GPU process|Renderer .*crash|render process gone|SIGKILL/i.test(x)); fs.writeFileSync(path.join(OUT, `m3-${TAG}.logcat.txt`), l.join('\n')); return l.length; })() } : null,
  errors: errors.slice(0, 4), mem_series: samples.filter((_, i) => i % 4 === 0).slice(0, 40),
};
if (args.includes('--profile')) {
  const { profile } = await cdp.send('Profiler.stop'); const self = {}; const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const dt = profile.timeDeltas; profile.samples.forEach((id, i) => { const n = byId.get(id); const k = `${n.callFrame.functionName || '(anon)'} ${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber}`; self[k] = (self[k] || 0) + dt[i] / 1000; });
  console.log(JSON.stringify(Object.entries(self).sort((a, b) => b[1] - a[1]).slice(0, 25).map(([k, v]) => [k, Math.round(v)])));
}
await page.screenshot({ path: path.join(OUT, `m3-${TAG}.png`) });
console.log(JSON.stringify(res));
await browser.close(); proc.kill(); fs.rmSync(dir, { recursive: true, force: true });
