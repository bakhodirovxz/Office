// WP12 tap-through check (p6-tapfix): the 3-step sequence that deleted a picture on the emulator, in Chrome (HRM_Pixel_8, RIG_SCALE=0.5 at `wm size 540x1200`
// / `wm density 210`): open phone-ux.docx (edit mode), More > Done editing, tap the round Edit button (1), tap the first heading (2), type with Gboard (3),
// Home (the editor's autosave goes to the shell), then read the picture out of the autosave payload.
//   node scripts/mobile/tapfix.mjs --dist <dist> --tag <before|after> [--hold <s>]
// --hold: stop after opening and wait that long (calibrating coordinates by hand). Evidence: E:/office-target/p6/evidence/<runId>-tapfix-<tag>/.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const S = L.serial('HRM_Pixel_8'), tag = opt('--tag', 'run');
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-recover/dist'));
const out = path.join(L.EVIDENCE_BIG, `${new Date().toISOString().replace(/[:.]/g, '-').slice(0, 16)}-tapfix-${tag}`);
fs.mkdirSync(out, { recursive: true });
const shot = (n) => fs.writeFileSync(path.join(out, `${tag}-${n}.png`), L.screencap(S));
const log = (...a) => console.log(`[${tag}]`, ...a);
// panel-space coordinates (1080x2400, Chrome has its toolbar)
const P = { more: [1000, 400], done: [780, 560], pencil: [966, 2220], heading: [300, 556] };
for (const k of ['more', 'done', 'pencil', 'heading']) if (process.env['P_' + k]) P[k] = process.env['P_' + k].split(',').map(Number);

async function cdp(wsUrl) {
  const ws = new WebSocket(wsUrl); let id = 0; const wait = new Map();
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && wait.has(d.id)) { wait.get(d.id)(d); wait.delete(d.id); } };
  const send = (method, params = {}) => new Promise((r, j) => { const i = ++id; wait.set(i, (d) => (d.error ? j(new Error(d.error.message)) : r(d.result))); ws.send(JSON.stringify({ id: i, method, params })); });
  return { ev: async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.value };
}
// Every autosave payload's media parts (the docx is a zip; the stored names are enough) and size, kept in the page.
const RECORDER = `if(!window.__tf){window.__tf=[];addEventListener('message',e=>{const d=e.data;if(d&&d.type==='wordcraft:autosave'&&d.bytes){const u=new Uint8Array(d.bytes);let s='';for(let i=0;i<u.length;i++)s+=String.fromCharCode(u[i]);window.__tf.push({n:u.length,media:/word\/media\/image1\.png/.test(s),at:Date.now()});}});}`;

L.gboardEnable(S);
L.sh(S, 'settings put system accelerometer_rotation 0; settings put system user_rotation 0; am force-stop com.android.chrome');
const serve = await L.startServe({ root: ROOT, dist });
L.reverse(S, serve.port);
try {
  L.openInChrome(S, 'about:blank'); await L.sleep(3000); L.chromeFirstRun(S); await L.sleep(2000);
  L.openInChrome(S, serve.base + '/'); await L.sleep(4000);
  L.forwardCdp(S, 9222);
  let t;
  for (let i = 0; i < 40 && !t; i++) {
    try { t = (await (await fetch('http://127.0.0.1:9222/json/list')).json()).find((x) => x.type === 'page' && x.url.startsWith(serve.base)); } catch { /* not yet */ }
    if (!t) await L.sleep(1000);
  }
  const page = await cdp(t.webSocketDebuggerUrl);
  await page.ev(RECORDER);
  await page.ev(`new Promise(r=>{const i=setInterval(()=>{if(document.querySelector('#cards li')){clearInterval(i);r(1)}},500)})`);
  const b64 = fs.readFileSync(path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx')).toString('base64');
  await page.ev(`(()=>{const b=Uint8Array.from(atob('${b64}'),c=>c.charCodeAt(0));const dt=new DataTransfer();dt.items.add(new File([b],'phone-ux.docx'));const i=document.querySelector('#file');i.files=dt.files;i.dispatchEvent(new Event('change',{bubbles:true}));})()`);
  await L.sleep(15000);
  shot('01-opened');
  if (opt('--hold')) { log('holding', opt('--hold'), 's'); await L.sleep(Number(opt('--hold')) * 1000); }
  else {
    L.tap(S, ...P.more); await L.sleep(1500); shot('02-more');
    L.tap(S, ...P.done); await L.sleep(2500); shot('03-reading');
    L.tap(S, ...P.pencil); await L.sleep(2500); shot('04-after-pencil');
    L.tap(S, ...P.heading); await L.sleep(2500); shot('05-after-heading-tap');
    execFileSync('node', [path.join(ROOT, 'scripts/mobile/android-type.mjs'), S, 'tapfix', 'тест'], { stdio: 'inherit' });
    await L.sleep(1500); shot('06-typed');
    L.sh(S, 'am start -a android.intent.action.MAIN -c android.intent.category.HOME'); await L.sleep(6000);
    L.sh(S, 'am start -n com.android.chrome/com.google.android.apps.chrome.Main'); await L.sleep(4000);
    const got = JSON.parse((await page.ev('JSON.stringify(window.__tf)')) || '[]');
    log('autosave payloads (size, has word/media/image1.png):', JSON.stringify(got.map((g) => [g.n, g.media])));
    fs.writeFileSync(path.join(out, `${tag}-autosaves.json`), JSON.stringify(got));
  }
} finally { serve.stop(); }
log('evidence', out);
