// Phase 7 WP9 evidence run on an emulator (HRM_Pixel_8, `wm size 540x1200`, RIG_SCALE=0.5, the app installed): the plugins in the Android app.
//   translit  uz-translit (bundled) toCyrillic then toLatin on scripts/mobile/fixtures/phone-ux.docx, run from the editor (`wordcraft:run`), saved and pulled
//   install   Plugins page: Install from file (the system picker) of a test package, enable, restart; the SHA-256 record in the app's private files
//   disable   Plugins page: turn uz-translit off, restart: the host is no longer loaded
//   cols      (WP10) M3 as an abort, and M5 (deny) and R1 (many) from the editor, with the document compared before and after; needs the test packages built with the current make-test-packages.py
//   recheck   (Phase 7 re-check) M5 M8 M10 S1 S2 (deadline, fetch cap) F2 F5 F7a F7b with the extended fixture plugin; needs make-test-packages.py and make-protected.py output
//   rows      the section 5.4 rows that apply to this container (M1 M2 M3 M4 M6 M7 M9 M11 + no network at all) with the WP5 test plugin
// Usage: node scripts/mobile/android-plugins.mjs <translit|install|disable|rows|cols|recheck|all> [--pkgs <dir of make-test-packages.py output>] [--ev <evidence subdir, default p7-android>]
// Evidence: a JSON and screenshots per item in docs/phase7/evidence/android/, recordings in $P6_DIR/evidence/p7-android/ (SHA lines printed for SHA256SUMS).
import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), what = args[0] ?? 'all';
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const S = L.serial('HRM_Pixel_8'), PKG = 'app.hoshiya.suite', ACT = `${PKG}/office.android.MainActivity`;
const SMALL = path.join(ROOT, 'docs/phase7/evidence/android'), EV = args.includes('--ev') ? args[args.indexOf('--ev') + 1] : 'p7-android', BIG = path.join(L.EVIDENCE_BIG, EV);
const PKGS = path.resolve(opt('--pkgs', 'E:/office-target/tmp/p7-android/pkgs'));
fs.mkdirSync(SMALL, { recursive: true }); fs.mkdirSync(BIG, { recursive: true });
const DOC = '/sdcard/Download/phone-ux.docx', MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
const sums = [];
const log = (...a) => console.log(...a);
const shot = (n) => { fs.writeFileSync(path.join(SMALL, `${n}.png`), L.screencap(S)); return `${n}.png`; };
async function record(name, fn) {
  const rec = L.startRecord(S); await L.sleep(1500);
  try { return await fn(); } finally { const r = await L.stopRecord(S, rec, path.join(BIG, `${name}.mp4`)); sums.push(`${r.sha256}  ${EV}/${name}.mp4  (${new Date().toISOString().slice(0, 16)}, ${r.bytes} bytes)`); }
}
const item = (name, pass, details, shots = []) => {
  fs.writeFileSync(path.join(SMALL, `${name}.json`), JSON.stringify({ item: name, pass, device: 'HRM_Pixel_8 (emulator), 540x1200 rig', run: new Date().toISOString(), details, screenshots: shots }, null, 2));
  log(pass ? 'PASS' : 'FAIL', name, JSON.stringify(details).slice(0, 400));
  return pass;
};

// ---- CDP to the WebView (debuggable build) ----
async function ws(url) {
  const w = new WebSocket(url); let id = 0; const wait = new Map();
  await new Promise((r, j) => { w.onopen = r; w.onerror = j; });
  const events = [];
  w.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && wait.has(d.id)) { wait.get(d.id)(d); wait.delete(d.id); } else if (d.method) events.push(d); };
  const send = (method, params = {}, sessionId) => new Promise((r, j) => { const i = ++id; const tm = setTimeout(() => { wait.delete(i); j(new Error(`CDP ${method}: no answer in 30 s (the page is paused while another app is on top)`)); }, 30000); wait.set(i, (d) => { clearTimeout(tm); return d.error ? j(new Error(d.error.message)) : r(d.result); }); w.send(JSON.stringify({ id: i, method, params, ...(sessionId && { sessionId }) })); });
  return { send, events, close: () => w.close(), ev: async (expression, sessionId) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, sessionId)).result?.value };
}
async function webview() {
  let pid;
  for (let i = 0; i < 40 && !pid; i++) { pid = [...L.sh(S, 'cat /proc/net/unix').matchAll(/@webview_devtools_remote_(\d+)/g)].pop()?.[1]; if (!pid) await L.sleep(1000); }
  if (!pid) throw new Error('no WebView devtools socket (is the app running and debuggable?)');
  L.adb(S, 'forward', 'tcp:9333', `localabstract:webview_devtools_remote_${pid}`);
  let t;
  for (let i = 0; i < 40 && !t; i++) {
    try { t = (await (await fetch('http://127.0.0.1:9333/json/list')).json()).find((x) => x.type === 'page' && x.url.startsWith('https://appassets.androidplatform.net/')); } catch { /* not yet */ }
    if (!t) await L.sleep(1000);
  }
  if (!t) throw new Error('the app page did not appear');
  return ws(t.webSocketDebuggerUrl);
}
async function until(page, expr, ms = 60000) {
  for (let t = 0; t < ms; t += 1000) { if (await page.ev(expr).catch(() => false)) return true; await L.sleep(1000); }
  return false;
}
const launch = async (extra = '') => { L.sh(S, 'am force-stop ' + PKG); L.sh(S, `am start ${extra} -n ${ACT}`); const p = await webview(); await until(p, "!!document.getElementById('nav-plugins')", 60000); return p; };
const ready = (p) => until(p, '!!(window.PluginHost && PluginHost.last && PluginHost.last.plugins.some((x) => x.win))', 90000);

async function mediaUri(name = 'phone-ux.docx') {
  for (let i = 0; i < 10; i++) {
    const m = new RegExp(`_id=(\\d+), _display_name=${name.replace('.', '\\.')}`).exec(L.sh(S, 'content query --uri content://media/external/downloads --projection _id:_display_name'));
    if (m) return `content://media/external/downloads/${m[1]}`;
    await L.sleep(1000);
  }
  throw new Error(`${name} is not in the media store`);
}
async function openFixture() {
  L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'pm clear ' + PKG); L.sh(S, 'rm -f ' + DOC);   // pm clear: no recovery document to ask about leaving
  L.adb(S, 'push', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'), DOC);
  L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DOC}`); await L.sleep(5000);
  const u = await mediaUri();
  L.sh(S, `am start -a android.intent.action.VIEW -d ${u} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
  const p = await webview();
  if (!(await ready(p))) throw new Error('the editor did not become ready with the plugin');
  await L.sleep(8000);
  return p;
}
function docText(file, remote = DOC) {
  const f = path.join(BIG, file); L.adb(S, 'pull', remote, f);
  return execFileSync('python3', ['-X', 'utf8', '-I', '-c', 'import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read("word/document.xml").decode("utf8");print(" ".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>",x)))', f], { encoding: 'utf8' });
}
const runCmd = (p, id) => p.ev(`document.querySelector('#frames iframe:not([hidden])').contentWindow.postMessage({ type: 'wordcraft:run', id: '${id}' }, location.origin)`);
async function save(tag) {   // the top "..." menu, then Save (the phone layout, edit mode)
  L.tap(S, 1016, 197); await L.sleep(2500); shot(`${tag}-more`);
  L.tap(S, 184, 940);   // the sheet is drawn on the canvas, so no text node: the Save row of the More sheet
  await L.sleep(4000);
}

// ---- translit ----
async function translit() {
  await record('translit', async () => {
    const p = await openFixture();
    const shots = [shot('translit-0-opened')];
    const before = (await p.ev(`PluginHost.last.plugins.map((x) => x.d.id)`)) ?? [];
    await runCmd(p, 'plugin.uz-translit.toCyrillic'); await L.sleep(8000); shots.push(shot('translit-1-cyrillic'));
    const mem = await p.ev(`PluginHost.last.mem['uz-translit']`), logs = await p.ev(`JSON.stringify(PluginHost.last.audit)`);
    await save('translit-cyr'); shots.push(shot('translit-2-saved'));
    const cyr = docText('translit-cyrillic.docx');
    item('translit-toCyrillic', before.includes('uz-translit') && mem > 0 && /Салом дунё/.test(cyr) && !/Salom dunyo/.test(cyr),
      { plugins: before, plugin_memory_bytes: mem, audit: logs, text_after: cyr.slice(0, 400) }, shots);
    await runCmd(p, 'plugin.uz-translit.toLatin'); await L.sleep(8000); shots.push(shot('translit-3-latin'));
    await save('translit-lat'); shots.push(shot('translit-4-saved'));
    const lat = docText('translit-latin.docx');
    item('translit-toLatin', /Salom dunyo/.test(lat) && !/Салом дунё/.test(lat), { text_after: lat.slice(0, 400) }, shots.slice(3));
  });
}

// ---- the Plugins page ----
const pluginsPage = async (p) => {   // the nav button's handler is set after the shell's catalog loaded: click until the page is up
  for (let i = 0; i < 30 && !(await p.ev("!!document.querySelector('#plg-main li')")); i++) { await p.ev("document.getElementById('nav-plugins').click()"); await L.sleep(1500); }
  await L.sleep(1000);
};
const pageItems = (p) => p.ev("[...document.querySelectorAll('#plg-main li')].map((li) => ({ name: li.querySelector('h3').textContent, enabled: li.querySelector('input[type=checkbox]')?.checked, boxes: li.querySelectorAll('input[type=checkbox]').length }))");
function tapNode(re) { return L.tapText(S, re); }

// The system picker (DocumentsUI): Downloads, then the file. The Plugins page must be open.
async function pickPackage(p, file) {
  await p.ev("document.getElementById('plg-install').click()");   /* the page's button moves when a message line is shown, and WebView text is not in the uiautomator tree */ await L.sleep(4000);
  const want = new RegExp(file.replace(/\./g, '\\.'));
  let hit = tapNode(want);   // the picker opens where it was last (Downloads after the first install): the file may be on screen already, and the drawer would be toggled shut
  if (!hit) {   // the drawer: a slow picker may not be up yet, and the closed drawer's items are in the tree off screen, so check that the Downloads folder really opened
    for (let k = 0; k < 5; k++) {
      L.tap(S, 73, 194); await L.sleep(2000); tapNode(/^Downloads$/); await L.sleep(2500);
      L.sh(S, 'uiautomator dump /sdcard/ui.xml >/dev/null'); if (/Files in Downloads/.test(L.sh(S, 'cat /sdcard/ui.xml'))) break;
    }
  }
  for (let i = 0; i < 4 && !hit; i++) { L.swipe(S, 540, 900, 540, 1900); await L.sleep(800); }   // back to the top: the list may have been left scrolled
  hit = hit || tapNode(want);
  for (let i = 0; i < 8 && !hit; i++) { hit = tapNode(want); if (!hit) { L.swipe(S, 540, 1900, 540, 900); await L.sleep(2000); } }   // a longer list: scroll
  await L.sleep(4000);
  return hit;
}
const enableRow = (p, re) => p.ev(`(() => { const li = [...document.querySelectorAll('#plg-main li')].find((l) => /${re}/.test(l.textContent)); li.querySelector('input[type=checkbox]').click(); })()`);
const pushPackages = () => { L.sh(S, 'rm -f /sdcard/Download/test.*.zip'); for (const f of fs.readdirSync(PKGS)) if (f.endsWith('.zip')) { L.adb(S, 'push', path.join(PKGS, f), '/sdcard/Download/' + f); L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Download/${f}`); } };

async function install() {
  pushPackages(); await L.sleep(5000);
  await record('install', async () => {
    let p = await launch(); await pluginsPage(p);
    const shots = [shot('install-0-page')], before = await pageItems(p);
    await p.ev("document.getElementById('plg-install').click()");   /* the page's button moves when a message line is shown, and WebView text is not in the uiautomator tree */ await L.sleep(4000); shots.push(shot('install-1-picker'));
    L.tap(S, 73, 194); await L.sleep(1500); tapNode(/^Downloads$/); await L.sleep(2500);
    let hit = false; for (let i = 0; i < 6 && !hit; i++) { hit = tapNode(/test\.host-1\.0\.0\.zip/); if (!hit) await L.sleep(2000); }
    await L.sleep(4000); shots.push(shot('install-2-installed'));
    const after = await pageItems(p), msg = await p.ev("document.getElementById('plg-msg').textContent");
    const dir = L.sh(S, `run-as ${PKG} ls files/plugins/test.host`), rec = L.sh(S, `run-as ${PKG} cat files/plugins/test.host/install.toml`);
    const wasm = path.join(BIG, 'test-host.wasm'); fs.writeFileSync(wasm, execFileSync('adb', ['-s', S, 'exec-out', 'run-as', PKG, 'cat', 'files/plugins/test.host/plugin.wasm'], { maxBuffer: 1 << 26 }));
    const sha = crypto.createHash('sha256').update(fs.readFileSync(wasm)).digest('hex');
    const listed = hit && after.some((x) => /Test plugin/.test(x.name)), recOk = rec.includes(`sha256 = "${sha}"`);
    await enableRow(p, 'Test plugin');   // turn it on: the Enabled box of its row, then the app restarts to apply
    await L.sleep(2500); shots.push(shot('install-3-enabled'));
    const msg2 = await p.ev("document.getElementById('plg-msg').textContent");
    p = await launch();
    const ids = (await until(p, '!!(window.PluginHost && PluginHost.last)')) ? await p.ev('PluginHost.last.plugins.map((x) => x.d.id)') : [];
    shots.push(shot('install-4-restarted'));
    item('install-from-file', listed && recOk && ids.includes('test.host'), { picker_before: before, picker_after: after, message: msg, enabled_message: msg2, files: dir, record: rec, wasm_sha256: sha, plugins_after_restart: ids,
      note: 'the package came through the system document picker (ACTION_OPEN_DOCUMENT); the record is the same install.toml as crates/suite-core/src/plugins.rs' }, shots);
  });
}

const setRow = (p, re, want) => p.ev(`(() => { const c = [...document.querySelectorAll('#plg-main li')].find((l) => /${re}/.test(l.textContent)).querySelector('input[type=checkbox]'); if (c.checked !== ${want}) c.click(); return c.checked; })()`);

async function disable() {
  await record('disable', async () => {
    let p = await launch(); await pluginsPage(p);
    await setRow(p, 'Transliteration', true); await L.sleep(1500);   // a known start: on
    p = await launch(); await pluginsPage(p);
    const shots = [shot('disable-0-page')], before = await p.ev('PluginHost.last.plugins.map((x) => x.d.id)');
    const now = await setRow(p, 'Transliteration', false);
    await L.sleep(2500); shots.push(shot('disable-1-off'));
    const msg = await p.ev("document.getElementById('plg-msg').textContent");
    p = await launch(); await L.sleep(6000);
    const ids = await p.ev('(window.PluginHost && PluginHost.last) ? PluginHost.last.plugins.map((x) => x.d.id) : []');
    const cfg = JSON.parse(await p.ev("fetch('/shell-config.json').then((r) => r.text())")).plugins.map((x) => x.id);
    const served = await p.ev(`fetch('/plugins/' + '${(await p.ev("fetch('/shell-config.json').then((r) => r.json()).then((j) => j.plugins[0]?.sha256 || '')")) || '0'.repeat(64)}').then((r) => r.status)`);
    shots.push(shot('disable-2-restarted'));
    item('disable', before.includes('uz-translit') && now === false && !ids.includes('uz-translit') && !cfg.includes('uz-translit'),
      { before, checkbox_after_click: now, message: msg, plugins_after_restart: ids, shell_config_plugins: cfg, other_enabled_plugin_wasm_status: served }, shots);
    p = await launch(); await pluginsPage(p);   // back on, for the next items
    await setRow(p, 'Transliteration', true); await L.sleep(1500);
  });
}

// ---- section 5.4 rows ----
async function rows() {
  await record('rows', async () => {
    let p = await launch();
    const have0 = (await until(p, '!!(window.PluginHost && PluginHost.last)')) ? await p.ev('PluginHost.last.plugins.map((x) => x.d.id)') : [];
    const need = [['test.host', 'Test plugin'], ['test.bad-import', 'Bad import'], ['test.bad-export', 'Bad export'], ['test.nonet', 'No network'], ['test.own-memory', 'Own memory'], ['test.gc-type', 'GC type']].filter(([id]) => !have0.includes(id));
    if (need.length) {   // setup through the same picker path as the install item
      await pluginsPage(p);
      for (const [id, name] of need) { const hit = await pickPackage(p, `${id}-1.0.0.zip`); log('install', id, hit, await p.ev("document.getElementById('plg-msg').textContent")); await enableRow(p, name); await L.sleep(1500); }
      p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    }
    const H = 'PluginHost.last';
    await p.ev(`window.__tick = 0; setInterval(() => __tick++, 100)`);
    const say = () => p.ev("document.getElementById('ed-status').textContent");
    const inv = (id, cmd) => p.ev(`${H}.invoke('${id}', '${cmd}')`);
    const audit = async (n) => JSON.parse(await p.ev(`JSON.stringify(${H}.audit.slice(${n}))`));
    const have = await p.ev(`${H}.plugins.map((x) => x.d.id)`);
    // M1 / M2: the Android store accepts the file, the Worker refuses the load
    for (const [row, id, re] of [['M1', 'test.bad-import', /imports not allowed: wasi_snapshot_preview1/], ['M2', 'test.bad-export', /exports a memory|defines its own memory/], ['M2b-own-memory', 'test.own-memory', /the module defines its own memory/], ['M2b-gc-type', 'test.gc-type', /type form that is not allowed/]]) {
      const n = (await audit(0)).length; await inv(id, 'hello'); await L.sleep(1500);
      const a = await audit(n), s = await say();
      item(`${row}-load-refused`.replace('M2b-own-memory-load-refused', 'M2b-own-memory').replace('M2b-gc-type-load-refused', 'M2b-gc-type'), have.includes(id) && a.some((x) => x.kind === 'failed' && re.test(x.error)), { plugin: id, audit: a, status: s });
    }
    // M3: no network permission; M4: the manifest lists example.org but Android grants nothing
    let n = (await audit(0)).length; await inv('test.nonet', 'net'); await L.sleep(1500);
    const a3 = await audit(n); item('M3-network-not-in-manifest', a3.some((x) => x.kind === 'network.refused'), { audit: a3, status: await say() });
    n = (await audit(0)).length; await inv('test.host', 'net'); await L.sleep(1500);
    const a4 = await audit(n); item('M4-network-listed-but-not-granted', a4.some((x) => x.kind === 'network.denied') && !a4.some((x) => x.kind === 'network'), { audit: a4, status: await say(), note: 'no grant dialog appeared; the descriptor carries grant: []' });
    // M6: infinite loop; the page keeps running (a 100 ms timer keeps ticking) and the host says the plugin stopped
    const t0 = await p.ev('__tick'); const t1 = Date.now();
    n = (await audit(0)).length; await inv('test.host', 'spin');
    const took = Date.now() - t1, ticks = (await p.ev('__tick')) - t0, a6 = await audit(n);
    item('M6-time-budget', a6.some((x) => x.kind === 'stopped') && ticks > (took / 100) * 0.6, { ms: took, timer_ticks: ticks, expected_ticks: Math.round(took / 100), audit: a6, status: await say() });
    shot('M6-stopped');
    // M7: memory growth capped at memory_mb (2 MiB)
    await inv('test.host', 'grow'); await L.sleep(1000);
    const mem = await p.ev(`${H}.mem['test.host']`);
    item('M7-memory-cap', mem <= 2 * 1048576 && (await say()) === 'hog capped', { memory_bytes: mem, cap_bytes: 2097152, status: await say() });
    // M9: markup in a panel is text, the 10,000-character text is cut
    await inv('test.host', 'xss'); await L.sleep(1500);
    const x = await p.ev(`(() => { const b = document.querySelector('.plg-panel .plg-body'); return { imgs: b.querySelectorAll('img').length, h3: b.querySelector('h3').textContent, textLen: b.querySelector('.plg-text').textContent.length, xss: window.__xss === undefined }; })()`);
    item('M9-panel-is-text', x.imgs === 0 && x.h3.startsWith('<img') && x.textLen <= 1024 && x.xss, x, [shot('M9-panel')]);
    // M11: the Worker's own CSP: a direct fetch from inside the plugin Worker, through its devtools target
    const csp = await p.ev("fetch('/shell/plugins/worker.js').then((r) => r.headers.get('content-security-policy'))");
    // the page's own devtools session auto-attaches to its Workers; evaluate inside the plugin Worker
    await p.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true }); await L.sleep(2000);
    const targets = p.events.filter((e) => e.method === 'Target.attachedToTarget' && e.params.targetInfo.type === 'worker' && /plugins\/worker\.js/.test(e.params.targetInfo.url));
    let w = {};
    if (targets.length) {
      const sessionId = targets[0].params.sessionId;
      for (const [k, u] of [['same', '/shell-config.json'], ['other', 'https://example.org/']]) w[k] = await p.ev(`fetch('${u}').then((r) => 'reached ' + r.status, (e) => 'blocked ' + e.name)`, sessionId);
    }
    item('M11-worker-csp', csp === "default-src 'none'; script-src 'wasm-unsafe-eval'" && w.same?.startsWith('blocked') && w.other?.startsWith('blocked'), { header: csp, worker_targets: targets.length, fetch_from_worker: w });
    // the page itself reaches nothing else either, and the app has no INTERNET permission at all
    const net = await p.ev("fetch('https://example.org/').then((r) => 'reached ' + r.status, (e) => 'blocked ' + e.name)");
    const perms = L.sh(S, `dumpsys package ${PKG} | grep -c android.permission.INTERNET; true`);
    item('no-network', net.startsWith('blocked') || net === 'reached 404', { page_fetch_example_org: net, internet_permission_lines: perms, note: 'AndroidManifest.xml declares no uses-permission; shouldInterceptRequest answers 404 for any other host' });
    // M5 M8 M10 M12 need the WP10 evil plugin or a server: not run here
  });
}


// ---- WP10: the rows that need the editor (M5, R1) and M3 as an abort ----
async function cols() {
  pushPackages(); await L.sleep(5000);
  await record('columns', async () => {
    L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'pm clear ' + PKG);   // an older stored copy of the fixture has no deny or many
    let p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    const have = await p.ev('PluginHost.last.plugins.map((x) => x.d.id)');
    await pluginsPage(p);
    // the packages are (re)installed from the picker: the fixture gained the deny and many commands, and a stored copy of an older one has neither
    for (const [id, name] of [['test.host', 'Test plugin'], ['test.nonet', 'No network']]) { if (!have.includes(id)) { await pickPackage(p, `${id}-1.0.0.zip`); await enableRow(p, name); await L.sleep(1500); } }
    p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    const H = 'PluginHost.last', say = () => p.ev("document.getElementById('ed-status').textContent");
    const cmds = await p.ev(`${H}.plugins.find((x) => x.d.id === 'test.host').m.command.map((c) => c.id)`);
    // M3: a fetch outside the manifest's list ends the invocation (the plugin is not resumed), audited
    const n = (await p.ev(`${H}.audit.length`)), l0 = await p.ev(`${H}.logs.filter((l) => l.text.includes('"kind":"resume"')).length`);
    await p.ev(`${H}.invoke('test.nonet', 'net')`); await L.sleep(1500);
    const a3 = JSON.parse(await p.ev(`JSON.stringify(${H}.audit.slice(${n}))`)), l1 = await p.ev(`${H}.logs.filter((l) => l.text.includes('"kind":"resume"')).length`);
    item('M3-abort', a3.length === 1 && a3[0].kind === 'network.refused' && /network not permitted/.test(await say()) && l1 === l0, { audit: a3, status: await say(), resume_calls_before_after: [l0, l1], note: 'no INTERNET permission either; nothing leaves the device' });
    // M5 and R1 from the editor on the open document; the saved file must carry the text it had
    const orig = execFileSync('python3', ['-X', 'utf8', '-I', '-c', 'import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read("word/document.xml").decode("utf8");print(" ".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>",x)))', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx')], { encoding: 'utf8' });
    L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'rm -f ' + DOC);
    L.adb(S, 'push', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'), DOC);
    L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DOC}`); await L.sleep(5000);
    L.sh(S, `am start -a android.intent.action.VIEW -d ${await mediaUri()} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
    p = await webview();
    if (!(await ready(p))) throw new Error('the editor did not become ready with the plugin');
    await L.sleep(8000);
    const shots = [shot('M5-0-opened')];
    const seen = {};
    for (const [row, cmd, why, shotName] of [['M5-saveAs-refused', 'deny', 'refused', 'M5-1-refused'], ['R1-reads-cap', 'many', 'bad next', 'R1-1-refused']]) {
      let ok = false, s = '';
      for (let i = 0; i < 20 && !ok; i++) { await runCmd(p, `plugin.test.host.${cmd}`); await L.sleep(1500); s = await say(); ok = /could not finish/.test(s) && s.includes(why); }
      shots.push(shot(shotName));
      seen[row] = { ok, status: s, audit_tail: JSON.parse(await p.ev('JSON.stringify(PluginHost.last.audit.slice(-3))')) };
    }
    // F5: a second plugin while one is pending: the busy refusal, then the first one's timeout leaves the document unlocked
    const f5n = await p.ev('PluginHost.last.audit.length');
    await runCmd(p, 'plugin.test.host.spin'); await L.sleep(1500);
    await runCmd(p, 'plugin.test.nonet.hello'); await L.sleep(1500);
    const busy = await say(); shots.push(shot('F5-1-busy'));
    let stopped = false; for (let i = 0; i < 30 && !stopped; i++) { await L.sleep(1000); stopped = await p.ev(`PluginHost.last.audit.slice(${f5n}).some((x) => x.kind === 'stopped')`); }
    await L.sleep(1500);
    await runCmd(p, 'plugin.test.nonet.hello'); await L.sleep(2500);
    const afterBusy = await say();
    await save('cols'); const after = docText('cols-after.docx');
    const same = after.trim() === orig.trim();
    item('M5-saveAs-refused', seen['M5-saveAs-refused'].ok && same, { ...seen['M5-saveAs-refused'], commands_in_manifest: cmds, document_unchanged_after_save: same, text: after.slice(0, 200) }, shots.slice(0, 2));
    item('F5-busy', /Another plugin is still running|document is protected \(read only\)/.test(busy) && stopped && same, { note: 'a command is refused by the editor registry while the first plugin holds the document lock (the busy string plg.busy is the panel-click route)', status_during: busy, first_plugin_stopped: stopped, status_after_next_command: afterBusy, document_unchanged_after_save: same }, [shots[shots.length - 1]]);
    item('R1-reads-cap', seen['R1-reads-cap'].ok && same, { ...seen['R1-reads-cap'], document_unchanged_after_save: same }, [shots[2]]);
    // F6: reinstalling a plugin whose manifest changed turns it off; the same package again keeps it on
    p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)'); await pluginsPage(p);
    const f6 = [], isOn = async (re) => p.ev(`[...document.querySelectorAll('#plg-main li')].find((l) => /${re}/.test(l.textContent))?.querySelector('input[type=checkbox]')?.checked`);
    const on0 = await isOn('Test plugin');
    await pickPackage(p, 'test.host-1.0.0.zip'); await L.sleep(2000); const onSame = await isOn('Test plugin'); f6.push(shot('F6-1-same-package'));
    await pickPackage(p, 'test.host-changed-1.0.0.zip'); await L.sleep(2000); const onChanged = await isOn('Test plugin changed'); f6.push(shot('F6-2-changed-package'));
    const msg6 = await p.ev("document.getElementById('plg-msg').textContent");
    item('F6-reinstall-consent', on0 === true && onSame === true && onChanged === false, { enabled_before: on0, enabled_after_same_package: onSame, enabled_after_changed_manifest: onChanged, message: msg6, rows: await pageItems(p) }, f6);
  });
}

// ---- Phase 7 re-check: the Android cells that were gaps (M5 again, M8, M10, S1, S2, F2, F5 wording, F7a, F7b) ----
async function recheck() {
  pushPackages(); await L.sleep(5000);
  await record('recheck', async () => {
    L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'pm clear ' + PKG);
    let p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    await pluginsPage(p);
    for (const [id, name] of [['test.recheck', 'Recheck commands'], ['test.nonet', 'No network']]) {
      let hit = false;
      for (let t = 0; t < 3 && !hit; t++) { tapNode(/^Wait$/); hit = await pickPackage(p, `${id}-1.0.0.zip`); log('install', id, 'try', t, hit, await p.ev("document.getElementById('plg-msg').textContent").catch(() => 'n/a (picker still open)')); shot(`recheck-install-${id}-${t}`); }
      await enableRow(p, name); await L.sleep(1500);
    }
    p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    log('plugins loaded', await p.ev('PluginHost.last.plugins.map((x) => x.d.id)'));
    const cmds = await p.ev("PluginHost.last.plugins.find((x) => x.d.id === 'test.recheck').m.command.map((c) => c.id)");
    const orig = execFileSync('python3', ['-X', 'utf8', '-I', '-c', 'import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read("word/document.xml").decode("utf8");print(" ".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>",x)))', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx')], { encoding: 'utf8' });
    L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'rm -f ' + DOC);
    L.adb(S, 'push', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'), DOC);
    L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DOC}`); await L.sleep(5000);
    L.sh(S, `am start -a android.intent.action.VIEW -d ${await mediaUri()} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
    p = await webview();
    if (!(await ready(p))) throw new Error('the editor did not become ready with the plugin');
    await L.sleep(8000);
    const say = () => p.ev("document.getElementById('ed-status').textContent");
    const aud = async (n = 0) => JSON.parse(await p.ev(`JSON.stringify(PluginHost.last.audit.slice(${n}))`));
    const shots = [shot('recheck-0-opened')];
    // a command sent while the editor still holds the previous invocation's lock is refused ("protected (read only)"): send it again until the plugin's own answer shows
    const tryCmd = async (cmd, re, plugin = 'test.recheck') => {
      const n = (await aud()).length; let ok = false, s = '';
      for (let i = 0; i < 20 && !ok; i++) { await runCmd(p, `plugin.${plugin}.${cmd}`); await L.sleep(1500); s = await say(); ok = re.test(s); }
      return { ok, status: s, audit: await aud(n) };
    };
    const sameDoc = () => { const t = docText('recheck-now.docx'); return t.trim() === orig.trim(); };
    const results = {};
    // M5 (again), M8, F2, F7a, S1: each ends the invocation with "could not finish", the audit holds one `failed`
    for (const [row, plugin, cmd, re] of [['M5', 'test.recheck', 'deny', /could not finish.*refused/], ['M8', 'test.recheck', 'bad', /could not finish.*encoded data/], ['S1', 'test.recheck', 'overflow', /could not finish/],
      ['F2-link', 'test.recheck', 'link', /could not finish/], ['F2-chr', 'test.recheck', 'chr', /could not finish/], ['F2-incl', 'test.recheck', 'incl', /could not finish/], ['F2-tracked', 'test.recheck', 'tracked', /could not finish/],
      ['F7a-vocab', 'test.recheck', 'vocab', /could not finish/], ['F7a-accept', 'test.recheck', 'accept', /could not finish/], ['F7a-wall', 'test.recheck', 'wall', /could not finish/]]) {
      results[row] = await tryCmd(cmd, re, plugin); shots.push(shot(`recheck-${row}`));
    }
    // M10: a selection change reaches the plugin as an event, the answer is an edit; refused, nothing reaches the document
    const n10 = (await aud()).length; let ev10 = [];
    for (let i = 0; i < 12 && !ev10.some((a) => a.kind === 'failed' && /event may only run ui actions/.test(a.error || '')); i++) { L.tap(S, 300 + (i % 4) * 150, 700 + (i % 3) * 120); await L.sleep(1500); ev10 = await aud(n10); }
    shots.push(shot('recheck-M10'));
    // S2: the 8-fetch cap (Android grants no origin: each fetch is denied and counted), then the 10 s invocation deadline
    const nF = (await aud()).length;
    await runCmd(p, 'plugin.test.recheck.fetches');
    for (let i = 0; i < 40 && !(await aud(nF)).some((a) => /more than 8 fetches/.test(a.error || '')); i++) await L.sleep(1500);
    results['S2-fetches'] = { audit: await aud(nF) };
    // the 10 s invocation deadline: a round must be under the 5 s budget and over 10 s / 16 rounds, whatever the machine: ping, pong and pang do 0.6e9, 3e9 and 15e9 loop turns per round (smallest first); the first that fits counts
    const resumes = () => p.ev(`PluginHost.last.logs.filter((l) => l.text.includes('"kind":"resume"')).length`);
    const tried = []; let fits = false, took = 0, rounds = 0;
    for (const cmd of ['ping', 'pong', 'pang']) {
      const r0 = await resumes(), n2 = (await aud()).length, t0 = Date.now();
      await runCmd(p, `plugin.test.recheck.${cmd}`);
      let end; for (let i = 0; i < 90 && !end; i++) { await L.sleep(1000); end = (await aud(n2))[0]; }
      took = Date.now() - t0; rounds = (await resumes()) - r0;
      tried.push(`${cmd}: ${end ? end.kind + ' ' + (end.error || '') : 'nothing'} after ${took} ms, ${rounds} resume rounds`);
      if (end && end.kind === 'stopped' && rounds >= 2 && took > 9000) { fits = true; break; }
      await L.sleep(3000);
    }
    shots.push(shot('recheck-S2'));
    await L.sleep(2000);
    // F5: a command of another plugin while one is pending: the busy reason, then the first one's timeout leaves the document unlocked
    const f5n = (await aud()).length;
    await runCmd(p, 'plugin.test.recheck.spin'); await L.sleep(1500);
    await runCmd(p, 'plugin.test.nonet.hello'); await L.sleep(1500);
    const busy = await say(); shots.push(shot('recheck-F5-busy'));
    let f5stopped = false; for (let i = 0; i < 30 && !f5stopped; i++) { await L.sleep(1000); f5stopped = (await aud(f5n)).some((a) => a.kind === 'stopped'); }
    await L.sleep(1500);
    await runCmd(p, 'plugin.test.nonet.hello'); await L.sleep(2500);
    const afterBusy = await say();
    await save('recheck'); const same = sameDoc();
    for (const row of ['M5', 'M8', 'S1', 'F2-link', 'F2-chr', 'F2-incl', 'F2-tracked', 'F7a-vocab', 'F7a-accept', 'F7a-wall']) {
      const r = results[row];
      const name = { M5: 'M5-saveAs-refused', M8: 'M8-malformed-answer', S1: 'S1-recursion' }[row] ?? row;
      item(name, r.ok && r.audit.length === 1 && r.audit[0].kind === 'failed' && same && (row !== 'S1' || /stack|recursion/i.test(r.audit[0].error)), { ...r, commands_in_manifest: row === 'M5' ? cmds : undefined, document_unchanged_after_save: same }, [shots.find((x) => x.includes(row))].filter(Boolean));
    }
    item('M10-event-edit-refused', ev10.some((a) => a.kind === 'failed' && /event may only run ui actions/.test(a.error || '')) && same, { audit: ev10, document_unchanged_after_save: same, note: 'the event is selectionChanged from a tap in the document; the fixture answers it with text.insert' }, ['recheck-M10.png']);
    const f = results['S2-fetches'];
    item('S2-fetch-cap', f.audit.filter((a) => a.kind === 'network.denied').length === 8 && f.audit.some((a) => a.kind === 'failed' && /more than 8 fetches/.test(a.error || '')) && same, { audit: f.audit, note: 'Android grants no origin, so each of the 8 fetches is denied (and counted); the 9th ends the invocation' });
    item('S2-deadline', fits && same, { tried, stopped_after_ms: took, resume_rounds: rounds, note: 'a round is under the 5 s budget and over 10 s / 16 rounds; only the 10 s invocation deadline can stop the chain' }, ['recheck-S2.png']);
    item('F5-busy', /another plugin is still running/i.test(busy) && f5stopped && same, { status_during: busy, first_plugin_stopped: f5stopped, status_after_next_command: afterBusy, document_unchanged_after_save: same }, ['recheck-F5-busy.png']);

    // F7b: a document with its own protection: the plugin is not started (comments: the engine's own gate says so; tracked changes: the bridge does)
    for (const [file, mode, why] of [['protected.docx', 'comments', /only comments are allowed/], ['protected-tracked.docx', 'trackedChanges', /document is protected/i]]) {
      const remote = '/sdcard/Download/' + file;
      L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'rm -f ' + remote);
      L.adb(S, 'push', path.join(PKGS, file), remote);
      L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${remote}`); await L.sleep(5000);
      L.sh(S, `am start -a android.intent.action.VIEW -d ${await mediaUri(file)} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
      p = await webview();
      if (!(await ready(p))) throw new Error('the editor did not become ready with the protected document');
      await L.sleep(8000);
      const l0 = await p.ev('PluginHost.last.logs.length'), a0 = (await aud()).length;
      let st = '', refusedOk = false;
      for (let i = 0; i < 8 && !refusedOk; i++) { await runCmd(p, 'plugin.test.recheck.hello'); await L.sleep(1500); st = await say(); refusedOk = why.test(st); }
      const l1 = await p.ev('PluginHost.last.logs.length'), a1 = (await aud()).length;
      const shotP = shot(`recheck-F7b-${mode}`);
      const after = path.join(BIG, file); L.adb(S, 'pull', remote, after);
      const unchanged = fs.readFileSync(after).equals(fs.readFileSync(path.join(PKGS, file)));
      item(`F7b-${mode}`, refusedOk && l1 === l0 && a1 === a0 && unchanged, { status: st, plugin_log_entries_before_after: [l0, l1], audit_entries_before_after: [a0, a1], file_on_device_unchanged: unchanged, note: `the page was never asked to run the plugin; documentProtection edit="${mode}" (scripts/mobile/fixtures/make-protected.py)` }, [shotP]);
    }
  });
}

if (what === 'translit' || what === 'all') await translit();
if (what === 'install' || what === 'all') await install();
if (what === 'rows' || what === 'all') await rows();
if (what === 'disable' || what === 'all') await disable();
if (what === 'cols') await cols();
if (what === 'recheck') await recheck();
log('SHA lines:\n' + sums.join('\n'));
fs.appendFileSync(path.join(BIG, 'SHA256SUMS.part'), sums.join('\n') + '\n');
process.exit(0);   // the CDP sockets keep node alive
