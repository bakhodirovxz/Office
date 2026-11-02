// WP10a survey session: Sheets, Slides or PDF in Chrome on the emulator, driven one step at a time with adb taps (like session.mjs, but for any editor).
// Fixed HTTP commands only (no code evaluation), on http://127.0.0.1:<port>/ :
//   GET /open?file=<repo-relative synthetic file>   open it through the shell, wait for <prefix>:opened or :ready (+ settle)
//   GET /msgs                                       the window messages seen since the last call (type, dirty, error, byte count)
//   GET /save?name=<n>                              post <prefix>:requestSave to the editor, wait for <prefix>:save, write the bytes to <out>/<n>.<ext>
//   GET /reload                                     reload the page
//   node scripts/mobile/survey.mjs [--avd HRM_Pixel_8] [--dist <dist>] [--port 9334] [--out <dir>]
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const name = opt('--avd', 'HRM_Pixel_8'), ser = L.serial(name);
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-mobileux/dist')), port = +opt('--port', 9334);
const outDir = opt('--out', 'E:/office-target/p6-lifecycle/survey');
fs.mkdirSync(outDir, { recursive: true });
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');

await L.startAvd(name);
L.gboardEnable(ser);
const serve = await L.startServe({ root: ROOT, dist });
L.reverse(ser, serve.port);
const cdpUrl = L.forwardCdp(ser);
L.openInChrome(ser, 'about:blank'); await L.sleep(3000); L.chromeFirstRun(ser); await L.sleep(2000); L.forwardCdp(ser);
for (const t of (await (await fetch(`${cdpUrl}/json/list`)).json()).filter((t) => t.type === 'page').slice(1)) await fetch(`${cdpUrl}/json/close/${t.id}`).catch(() => {});
const browser = await chromium.connectOverCDP(cdpUrl);
const page = browser.contexts()[0].pages()[0];
page.setDefaultTimeout(240_000);
let prefix = '';
// Registered once. Test shim (localStorage __shim = 1, set by /open?shim=1): the emulator's WebGL2 reports MAX_TEXTURE_SIZE 4096, wgpu's default device descriptor
// requires 8192 and requestDevice fails (docs: patch wordcraft/0002; Sheets, Slides, PDF have no such patch). With the shim the page is told 8192 so the survey can go on.
await page.addInitScript(() => {
  window.__msgs = []; window.__saves = [];
  try { if (localStorage.getItem('__shim') === '1') { const gp = WebGL2RenderingContext.prototype.getParameter; WebGL2RenderingContext.prototype.getParameter = function (p) { const v = gp.call(this, p); return p === 0x0D33 && v < 8192 ? 8192 : v; }; } } catch { /* no storage */ }
  addEventListener('message', (e) => {
    const d = e.data; if (!d || typeof d.type !== 'string') return;
    window.__msgs.push({ type: d.type, dirty: d.dirty, error: d.error, bytes: d.bytes && d.bytes.byteLength });
    if (d.type.endsWith(':save') && d.bytes) { const u = new Uint8Array(d.bytes); let s = ''; for (let i = 0; i < u.length; i += 8192) s += String.fromCharCode(...u.subarray(i, i + 8192)); window.__saves.push({ name: d.name, b64: btoa(s) }); }
  });
});


const cmds = {
  open: async (q) => {
    const f = path.resolve(ROOT, q.get('file'));
    if (!f.startsWith(ROOT)) throw new Error('file outside the repo');
    await page.goto(serve.base + '/', { waitUntil: 'load', timeout: 240_000 });
    await page.evaluate((v) => localStorage.setItem('__shim', v), q.get('shim') ?? '0');
    await page.reload({ waitUntil: 'load' });
    await L.activateTab(cdpUrl, serve.base);
    await page.waitForSelector('#cards li');
    await page.setInputFiles('#file', f);
    await page.waitForFunction(() => window.__msgs.some((m) => m.type.endsWith(':opened') || m.type.endsWith(':ready') || m.type.endsWith(':failed')), undefined, { timeout: 240_000 });
    prefix = await page.evaluate(() => window.__msgs.find((m) => m.type.endsWith(':opened') || m.type.endsWith(':ready') || m.type.endsWith(':failed')).type.split(':')[0]);
    await page.waitForTimeout(8000);
    return `open, prefix ${prefix}`;
  },
  msgs: async () => JSON.stringify(await page.evaluate(() => window.__msgs.splice(0))),
  save: async (q) => {
    await page.evaluate((p) => { window.__saves.length = 0; document.querySelector('iframe:not([hidden])').contentWindow.postMessage({ type: p + ':requestSave' }, location.origin); }, prefix);
    await page.waitForFunction(() => window.__saves.length > 0, undefined, { timeout: 60_000 });
    const s = await page.evaluate(() => window.__saves.at(-1));
    const dest = path.join(outDir, `${(q.get('name') ?? 'saved').replace(/[^\w.-]/g, '_')}${path.extname(s.name)}`);
    fs.writeFileSync(dest, Buffer.from(s.b64, 'base64'));
    return `${dest} ${fs.statSync(dest).size} B`;
  },
  reload: async () => { await page.reload(); return 'reloaded'; },
};
http.createServer(async (req, res) => {
  const u = new URL(req.url, 'http://x');
  try { res.end(String(await cmds[u.pathname.slice(1)](u.searchParams))); } catch (e) { res.statusCode = 500; res.end(String(e.stack ?? e)); }
}).listen(port, '127.0.0.1', () => console.log(`survey session ready on ${port}`));
