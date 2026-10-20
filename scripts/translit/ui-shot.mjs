// Phase 2 WP7 evidence: screenshots of a web app with the Transliterate entries, driven by a step list.
//   node scripts/translit/ui-shot.mjs <docs|sheets|slides> <outdir> --steps <steps.json> [--file <doc>] [--locale uz-Latn-UZ] [--dist <dir>] [--width W --height H]
// steps: ["click",x,y] ["move",x,y] ["key",K] ["type",text] ["wait",ms] ["insert",text] ["shot",name] ["rclick",x,y] ["drag",x1,y1,x2,y2] ["upload",path]
// docs opens --file through the 0001 host bridge (iframe host page); sheets and slides load their own start screen.
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
const require = createRequire(path.resolve('spikes/docserver/e2e/') + '/');
const { chromium } = require('playwright');
const args = process.argv.slice(2);
const app = args[0];
const outdir = path.resolve(args[1]);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const dist = path.resolve(opt('--dist', path.join(process.env.OFFICE_TARGET || 'E:/office-target/p1', 'dist', `${app}-web`)));
const steps = JSON.parse(fs.readFileSync(opt('--steps'), 'utf8'));
const file = opt('--file', null);
fs.mkdirSync(outdir, { recursive: true });
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.ico': 'image/x-icon', '.png': 'image/png', '.css': 'text/css' };
const HOST = `<!doctype html><meta charset=utf-8><body style="margin:0"><iframe id=wc src="/wc/index.html?host=parent" style="border:0;width:100vw;height:100vh"></iframe>
<script>window.log=[];window.addEventListener('message',(ev)=>{if(ev.data&&typeof ev.data.type==='string')window.log.push(ev.data.type)});
window.openDoc=(name,b64)=>{const bin=atob(b64);const u=new Uint8Array(bin.length);for(let i=0;i<bin.length;i++)u[i]=bin.charCodeAt(i);document.getElementById('wc').contentWindow.postMessage({type:'wordcraft:open',name,bytes:u.buffer},location.origin,[u.buffer]);};</script>`;
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split('?')[0]);
  if (app === 'docs' && url === '/host.html') return res.writeHead(200, { 'content-type': 'text/html' }).end(HOST);
  const rel = app === 'docs' ? url.replace(/^\/wc\//, '/') : url;
  const f = path.join(dist, rel === '/' ? 'index.html' : rel);
  if (!f.startsWith(dist) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) return res.writeHead(404).end();
  res.writeHead(200, { 'content-type': MIME[path.extname(f)] || 'application/octet-stream' });
  fs.createReadStream(f).pipe(res);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
const ctx = await browser.newContext({ viewport: { width: Number(opt('--width', 1280)), height: Number(opt('--height', 800)) }, locale: opt('--locale', 'uz-Latn-UZ') });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
if (app === 'docs') {
  await page.goto(`${base}/host.html`);
  await page.waitForFunction(() => window.log.includes('wordcraft:ready'), null, { timeout: 240000 });
  await page.evaluate(([n, d]) => window.openDoc(n, d), [path.basename(file), fs.readFileSync(file).toString('base64')]);
  await page.waitForFunction(() => window.log.includes('wordcraft:opened'), null, { timeout: 240000 });
} else {
  await page.goto(`${base}/`);
  await page.waitForFunction(() => !document.querySelector('[id$="_loading"]'), null, { timeout: 240000 }).catch(() => errors.push('start screen not reached'));
}
await page.waitForTimeout(4000);
for (const [op, a, b, c, d] of steps) {
  if (op === 'click') await page.mouse.click(a, b);
  else if (op === 'rclick') await page.mouse.click(a, b, { button: 'right' });
  else if (op === 'move') await page.mouse.move(a, b);
  else if (op === 'drag') { await page.mouse.move(a, b); await page.mouse.down(); await page.mouse.move(c, d, { steps: 10 }); await page.mouse.up(); }
  else if (op === 'key') await page.keyboard.press(a);
  else if (op === 'type') await page.keyboard.type(a, { delay: 30 });
  else if (op === 'insert') await page.keyboard.insertText(a);
  else if (op === 'wait') await page.waitForTimeout(a);
  else if (op === 'upload') { const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), page.mouse.click(b, c)]); await fc.setFiles(path.resolve(a)); }
  else if (op === 'shot') await page.screenshot({ path: path.join(outdir, `${a}.png`) });
}
console.log(JSON.stringify({ app, errors: errors.slice(0, 3) }));
await browser.close();
server.close();
