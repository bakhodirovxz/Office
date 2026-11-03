// WP12 lifecycle runs on the emulator (PLAN phase 6): edit a document in Docs, hit one lifecycle event, say what survived.
//   node scripts/mobile/lifecycle.mjs <chrome|app> <home|rotate|kill|crash|glloss> [--dist <dist>] [--wait <s>]
//   home    Home, wait, back to the app          rotate  landscape, wait, portrait
//   kill    Home, wait, `am kill <pkg>`, relaunch (app: process death + restore; chrome: all Chrome processes, renderers included)
//   crash   Page.crash over CDP in the foreground (the renderer dies, the browser/app process lives)
//   glloss  WEBGL_lose_context.loseContext() on the editor canvas over CDP
// --before <s>: wait that long after typing before the event. --edit-first: the build opens a document for editing (no pencil tap). After the event the shell's recovery prompt (WP12 recovery, shell.js
// rec.ask) is accepted (Chrome: CDP, app: the native dialog's OK), the kept copy is pulled (<tag>-recovery.docx: Chrome IndexedDB, app
// files/recover/docs) and the reopened page is shot (<tag>-recovered.png).
// Typing is Gboard (android-type.mjs); the page is watched over raw CDP (a recorder of every window message and visibility change,
// polled to the host so it survives a dead page). Synthetic fixtures only. Evidence: E:/office-target/p6/evidence/<runId>/.
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), [surface, kase] = args;
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const S = L.serial('HRM_Pixel_8'), PKG = surface === 'app' ? 'app.hoshiya.suite' : 'com.android.chrome';
const ACT = 'app.hoshiya.suite/office.android.MainActivity', CHROME = 'com.android.chrome/com.google.android.apps.chrome.Main';
const dist = path.resolve(opt('--dist', 'E:/office-target/p6-mobileux/dist'));
const runId = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
const out = path.join(L.EVIDENCE_BIG, runId);
fs.mkdirSync(out, { recursive: true });
const pids = () => { try { return L.sh(S, `pidof ${PKG}`); } catch { return ''; } };
const tag = `${surface}-${kase}`;
const log = (...a) => console.log(`[${tag}]`, ...a);
const shot = (n) => fs.writeFileSync(path.join(out, `${tag}-${n}.png`), L.screencap(S));
const FIX = path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'), DEV = '/sdcard/Download/phone-ux.docx';
const MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
const t0 = Date.now(), at = () => ((Date.now() - t0) / 1000).toFixed(1);

// ---- raw CDP over WebSocket (Node's global one)
async function cdp(wsUrl) {
  const ws = new WebSocket(wsUrl); let id = 0; const wait = new Map();
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && wait.has(d.id)) { wait.get(d.id)(d); wait.delete(d.id); } };
  const send = (method, params = {}) => new Promise((r, j) => { const i = ++id; wait.set(i, (d) => (d.error ? j(new Error(d.error.message)) : r(d.result))); ws.send(JSON.stringify({ id: i, method, params })); setTimeout(() => { if (wait.delete(i)) j(new Error('cdp timeout ' + method)); }, 240000); });
  const ev = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.value;
  return { send, ev, close: () => ws.close() };
}
async function target(port, match) {
  for (let i = 0; i < 40; i++) {
    try { const t = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((x) => x.type === 'page' && match(x)); if (t) return t; } catch { /* not yet */ }
    await L.sleep(1000);
  }
  throw new Error('no CDP target');
}
const RECORDER = `if(!window.__lc){window.__lc=[];addEventListener('message',e=>{const d=e.data;if(d&&d.type==='wordcraft:autosave'&&d.bytes){const u=new Uint8Array(d.bytes);let b='';for(let i=0;i<u.length;i+=8192)b+=String.fromCharCode.apply(null,u.subarray(i,i+8192));window.__auto=btoa(b)}if(d&&typeof d.type==='string')__lc.push({t:Date.now(),type:d.type,n:d.bytes&&d.bytes.byteLength,dirty:d.dirty,err:d.error})});document.addEventListener('visibilitychange',()=>__lc.push({t:Date.now(),type:'vis:'+document.visibilityState}))}1`;

const events = [];   // polled from the page, with the host's clock of the poll
let page = null, polling = true;
const poll = (async () => { while (polling) { try { if (page) for (const e of JSON.parse(await page.ev('JSON.stringify(window.__lc?window.__lc.splice(0):[])') || '[]')) events.push(e); } catch { /* page gone */ } await L.sleep(1000); } })();

// The recovery prompt is a confirm(): it blocks the page's JS, so it is answered before the recorder is put back.
async function acceptPrompt() {
  for (let i = 0; i < 25; i++) {
    if (surface === 'app') { try { L.tapText(S, /^OK$/); return true; } catch { /* not up yet */ } }
    else {
      try {
        L.forwardCdp(S, 9222);
        const t = (await (await fetch('http://127.0.0.1:9222/json/list')).json()).find((x) => x.type === 'page' && x.url.startsWith(serve.base));
        if (t) { const c = await cdp(t.webSocketDebuggerUrl); try { await c.send('Page.handleJavaScriptDialog', { accept: true }); return true; } catch { /* no dialog yet */ } finally { c.close(); } }
      } catch { /* not up yet */ }
    }
    await L.sleep(1000);
  }
  return false;
}
async function attach() {
  let t;
  if (surface === 'chrome') { L.forwardCdp(S, 9222); t = await target(9222, (x) => x.url.startsWith(serve.base)); }
  else {
    let pid = ''; for (let i = 0; i < 30 && !pid; i++) { pid = pids().trim().split(/\s+/)[0]; await L.sleep(500); }
    L.adb(S, 'forward', 'tcp:9223', `localabstract:webview_devtools_remote_${pid}`);
    t = await target(9223, (x) => x.url.includes('appassets'));
  }
  page = await cdp(t.webSocketDebuggerUrl);
  await page.ev(RECORDER);
}
const editFirst = args.includes('--edit-first');
const edit = async () => {
  if (surface === 'app') { if (!editFirst) { L.tap(S, 966, 2220); await L.sleep(3000); } L.tap(S, 780, 345); await L.sleep(4000); }
  else { if (!editFirst) { L.tap(S, ...CH.pencil); await L.sleep(3000); } L.tap(S, ...CH.caret); await L.sleep(4000); }
  execFileSync('node', [path.join(ROOT, 'scripts/mobile/android-type.mjs'), S, 'lifecycle', 'тест'], { stdio: 'inherit' });
  await L.sleep(1500);
};
const CH = { pencil: [966, 2220], caret: [780, 556] };   // Chrome has its toolbar: the same layout sits lower (calibrated on a screenshot)

// ---- setup
let serve = null;
L.gboardEnable(S);
L.sh(S, 'settings put system accelerometer_rotation 0; settings put system user_rotation 0');
L.sh(S, `am force-stop ${PKG}`);
L.sh(S, `rm -f ${DEV}`); L.adb(S, 'push', FIX, DEV);
L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DEV}`);
const rec = L.startRecord(S);
if (surface === 'chrome') {
  serve = await L.startServe({ root: ROOT, dist }); L.reverse(S, serve.port);
  L.openInChrome(S, 'about:blank'); await L.sleep(3000); L.chromeFirstRun(S); await L.sleep(2000);
  L.openInChrome(S, serve.base + '/'); await L.sleep(4000);
  await attach();
  await page.ev(`new Promise(r=>{const i=setInterval(()=>{if(document.querySelector('#cards li')){clearInterval(i);r(1)}},500)})`);
  // the file input gets the fixture through a DataTransfer (a CDP file chooser would need a device path and fires no change event here)
  await page.ev(`(()=>{const b=Uint8Array.from(atob('${fs.readFileSync(FIX).toString('base64')}'),c=>c.charCodeAt(0));const dt=new DataTransfer();dt.items.add(new File([b],'phone-ux.docx'));const i=document.querySelector('#file');i.files=dt.files;i.dispatchEvent(new Event('change',{bubbles:true}));return 1})()`);
  while (!events.some((e) => e.type === 'wordcraft:opened')) await L.sleep(500);   // the poller drains the page's recorder
  await L.sleep(3000);
} else {
  let uri = ''; for (let i = 0; i < 10 && !uri; i++) { const m = /_id=(\d+), _display_name=phone-ux\.docx/.exec(L.sh(S, 'content query --uri content://media/external/downloads --projection _id:_display_name')); if (m) uri = `content://media/external/downloads/${m[1]}`; else await L.sleep(1000); }
  L.sh(S, `am start -a android.intent.action.VIEW -d ${uri} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
  await L.sleep(8000); await attach();
  while (!events.some((e) => e.type === 'wordcraft:opened') && !(await page.ev("!!document.querySelector('iframe')"))) await L.sleep(500);
  await L.sleep(12000);
}
shot('opened');

// ---- edit, then the event
await edit(); shot('typed');
const tEdit = Date.now();
log(`typed at ${at()} s`);
const sinceEdit = () => ((Date.now() - tEdit) / 1000).toFixed(0);
const hold = Number(opt('--wait', 8)) * 1000;
// --before <s>: let the editor work that long after typing before the event (e.g. past its 30 s periodic autosave)
if (opt('--before')) { await L.sleep(Number(opt('--before')) * 1000); log(`event at ${sinceEdit()} s after typing`); }
if (kase === 'home') {
  L.sh(S, 'am start -a android.intent.action.MAIN -c android.intent.category.HOME'); await L.sleep(hold); shot('home');
  L.sh(S, surface === 'app' ? `am start -n ${ACT}` : `am start -n ${CHROME}`); await L.sleep(6000);
} else if (kase === 'rotate') {
  L.sh(S, 'settings put system user_rotation 1'); await L.sleep(hold); shot('landscape');
  L.sh(S, 'settings put system user_rotation 0'); await L.sleep(6000);
} else if (kase === 'kill') {
  L.sh(S, 'am start -a android.intent.action.MAIN -c android.intent.category.HOME'); await L.sleep(hold);
  log(`am kill ${PKG} (pids before: ${pids()})`);
  L.sh(S, `am kill ${PKG}`); await L.sleep(3000);
  log(`pids after: ${JSON.stringify(pids())}`);
  page = null;
  L.sh(S, surface === 'app' ? `am start -n ${ACT}` : `am start -n ${CHROME}`); await L.sleep(15000);
  if (surface === 'chrome') { L.chromeFirstRun(S); await L.sleep(3000); }   // a relaunched Chrome may show a promo page first
} else if (kase === 'crash') {
  try { await page.send('Page.crash'); } catch (e) { log('Page.crash:', e.message); }
  page = null; await L.sleep(10000); shot('after-crash');
  log(`pids: ${JSON.stringify(pids())}`);
  // Chrome shows "Aw, Snap!": the user reloads the page (the app recreates its WebView by itself, onRenderProcessGone)
  if (surface === 'chrome') { L.openInChrome(S, serve.base + '/'); await L.sleep(8000); }
} else if (kase === 'glloss') {
  log('lose context:', await page.ev(`(()=>{const f=document.querySelector('iframe');const c=f.contentDocument.querySelector('canvas');const g=c.getContext('webgl2')||c.getContext('webgl');const x=g&&g.getExtension('WEBGL_lose_context');if(!x)return 'no extension';x.loseContext();return 'lost'})()`));
  await L.sleep(hold + 4000);
} else throw new Error('unknown case ' + kase);
shot('after'); log(`after the event: ${sinceEdit()} s since typing`);

// a restarted page asks about the kept copy (WP12 recovery): accept it
if (kase !== 'glloss' && kase !== 'home' && kase !== 'rotate') { shot('prompt'); log('recovery prompt accepted:', await acceptPrompt()); await L.sleep(15000); shot('recovered'); }
else if (kase === 'glloss') shot('recovered');
// what is on the page now (a restarted page is reattached)
try { await attach(); log('page state:', await page.ev(`JSON.stringify({url:location.href,frames:document.querySelectorAll('iframe').length,home:!!document.querySelector('#cards li')&&!document.querySelector('iframe:not([hidden])')})`)); } catch (e) { log('no page to reattach:', e.message); }
await L.sleep(3000);
polling = false; await poll;
try { const a = await page.ev('window.__auto||null'); if (a) fs.writeFileSync(path.join(out, `${tag}-autosave.docx`), Buffer.from(a, 'base64')); } catch { /* page gone */ }
// the kept copy, as the next start would get it
try {
  let kept = null;
  if (surface === 'app') {
    const raw = spawnSync('adb', ['-s', S, 'exec-out', 'run-as', PKG, 'cat', 'files/recover/docs'], { maxBuffer: 1 << 28 }).stdout;
    const nl = raw.indexOf(10); if (raw.length && nl > 0) kept = raw.subarray(nl + 1);
  } else {
    const b = await page.ev(`new Promise(r=>{const o=indexedDB.open('shell');o.onsuccess=()=>{const q=o.result.transaction('blob').objectStore('blob').get('rec:docs');q.onsuccess=()=>{const v=q.result;if(!v)return r(null);const u=new Uint8Array(v.bytes);let s='';for(let i=0;i<u.length;i+=8192)s+=String.fromCharCode.apply(null,u.subarray(i,i+8192));r(btoa(s))}};o.onerror=()=>r(null)})`);
    if (b) kept = Buffer.from(b, 'base64');
  }
  if (kept) { fs.writeFileSync(path.join(out, `${tag}-recovery.docx`), kept); log(`kept copy: ${kept.length} B`); } else log('kept copy: none');
} catch (e) { log('kept copy: not read:', e.message); }
try { log('editor state:', await page.ev(`JSON.stringify({status:document.getElementById('ed-status').textContent,error:document.getElementById('error').textContent,frames:document.querySelectorAll('iframe').length})`)); } catch { /* page gone */ }
const t1 = events.find((e) => e.type === 'wordcraft:dirty' && e.dirty)?.t;
const timeline = events.map((e) => ({ s: t1 ? +((e.t - t1) / 1000).toFixed(1) : null, ...e, t: undefined }));
fs.writeFileSync(path.join(out, `${tag}.json`), JSON.stringify({ run: runId, surface, kase, dist, versions: L.versions(S), timeline }, null, 1));
log('timeline (s from first dirty):', timeline.filter((e) => !/wordcraft:(ready|opened)/.test(e.type)).map((e) => `${e.s} ${e.type}${e.n ? ` ${e.n}B` : ''}${e.err ? ` ${e.err}` : ''}`).join(' | '));
const r = await L.stopRecord(S, rec, path.join(out, `${tag}.mp4`));
fs.appendFileSync(path.join(ROOT, 'docs/phase6/evidence/SHA256SUMS'), `${r.sha256}  ${runId}/${tag}.mp4  (WP12)\n`);
log(JSON.stringify(r));
serve?.stop();
page?.close();
process.exit(0);
