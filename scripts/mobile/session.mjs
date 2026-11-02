// Interactive rig session (WP7): boots/uses an AVD, serves a dist and answers a few FIXED commands on http://127.0.0.1:<port>/ (no code
// evaluation) so keyboard rows can be driven one at a time with adb taps and recordings between calls.
//   node scripts/mobile/session.mjs --avd HRM_360 --dist <dist> [--port 9333]
//   GET /open?file=<repo-relative synthetic file>   open it in Docs through the shell
//   GET /blob?name=<n>                              the plain text of the last docx the shell was handed on Save (also written to <out>/<n>.docx)
//   GET /reload                                     reload the page (drops the document)
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const name = opt('--avd', 'HRM_360'), ser = L.serial(name);
const dist = path.resolve(opt('--dist', 'E:/office-target/p5/dist')), port = +opt('--port', 9333);
const outDir = opt('--out', 'E:/office-target/p6/session');
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
page.setDefaultTimeout(120_000);

const cmds = {
  open: async (q) => {
    const f = path.resolve(ROOT, q.get('file'));
    if (!f.startsWith(ROOT)) throw new Error('file outside the repo');
    await page.addInitScript(() => { window.__msgs = []; window.__blobs = []; const cu = URL.createObjectURL.bind(URL); URL.createObjectURL = (b) => { if (b instanceof Blob) window.__blobs.push(b); return cu(b); }; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
    await page.goto(serve.base + '/', { waitUntil: 'load', timeout: 240_000 });
    await L.activateTab(cdpUrl, serve.base);
    await page.waitForSelector('#cards li');
    await page.setInputFiles('#file', f);
    await page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 240_000 });
    await page.waitForTimeout(1500);
    return 'opened';
  },
  blob: async (q) => {
    const b64 = await page.evaluate(async () => { const b = window.__blobs.at(-1); if (!b) return null; const u = new Uint8Array(await b.arrayBuffer()); let s = ''; for (let i = 0; i < u.length; i += 8192) s += String.fromCharCode(...u.subarray(i, i + 8192)); return btoa(s); });
    if (!b64) return 'no blob';
    const dest = path.join(outDir, `${(q.get('name') ?? 'saved').replace(/[^\w.-]/g, '_')}.docx`);
    fs.writeFileSync(dest, Buffer.from(b64, 'base64'));
    return spawnSync('python', ['-I', '-c', "import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read('word/document.xml').decode();sys.stdout.buffer.write('\\n'.join(''.join(re.findall(r'<w:t[^>]*>([^<]*)</w:t>',p)) for p in re.findall(r'<w:p[ >].*?</w:p>',x,re.S)).encode())", dest], { encoding: 'utf8' }).stdout;
  },
  reload: async () => { await page.reload(); return 'reloaded'; },
};
http.createServer(async (req, res) => {
  const u = new URL(req.url, 'http://x');
  try { res.end(String(await cmds[u.pathname.slice(1)](u.searchParams))); } catch (e) { res.statusCode = 500; res.end(String(e.stack ?? e)); }
}).listen(port, '127.0.0.1', () => console.log(`session ready on ${port}`));
