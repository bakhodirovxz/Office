// Phase 7 WP9 evidence run on an emulator (HRM_Pixel_8, `wm size 540x1200`, RIG_SCALE=0.5, the app installed): the plugins in the Android app.
//   translit  uz-translit (bundled) toCyrillic then toLatin on scripts/mobile/fixtures/phone-ux.docx, run from the editor (`wordcraft:run`), saved and pulled
//   install   Plugins page: Install from file (the system picker) of a test package, enable, restart; the SHA-256 record in the app's private files
//   disable   Plugins page: turn uz-translit off, restart: the host is no longer loaded
//   cols      (WP10) M3 as an abort, and M5 (deny) and R1 (many) from the editor, with the document compared before and after; needs the test packages built with the current make-test-packages.py
//   rows      the section 5.4 rows that apply to this container (M1 M2 M3 M4 M6 M7 M9 M11 + no network at all) with the WP5 test plugin
// Usage: node scripts/mobile/android-plugins.mjs <translit|install|disable|rows|cols|all> [--pkgs <dir of make-test-packages.py output>] [--ev <evidence subdir, default p7-android>]
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
  const send = (method, params = {}, sessionId) => new Promise((r, j) => { const i = ++id; wait.set(i, (d) => (d.error ? j(new Error(d.error.message)) : r(d.result))); w.send(JSON.stringify({ id: i, method, params, ...(sessionId && { sessionId }) })); });
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

async function mediaUri() {
  for (let i = 0; i < 10; i++) {
    const m = /_id=(\d+), _display_name=phone-ux\.docx/.exec(L.sh(S, 'content query --uri content://media/external/downloads --projection _id:_display_name'));
    if (m) return `content://media/external/downloads/${m[1]}`;
    await L.sleep(1000);
  }
  throw new Error('phone-ux.docx is not in the media store');
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
function docText(file) {
  const f = path.join(BIG, file); L.adb(S, 'pull', DOC, f);
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
  L.tap(S, 73, 194); await L.sleep(1500); tapNode(/^Downloads$/); await L.sleep(2500);
  let hit = false; for (let i = 0; i < 6 && !hit; i++) { hit = tapNode(new RegExp(file.replace(/\./g, '\\.'))); if (!hit) await L.sleep(2000); }
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
    const need = [['test.host', 'Test plugin'], ['test.bad-import', 'Bad import'], ['test.bad-export', 'Bad export'], ['test.nonet', 'No network']].filter(([id]) => !have0.includes(id));
    if (need.length) {   // setup through the same picker path as the install item
      await pluginsPage(p);
      for (const [id, name] of need) { await pickPackage(p, `${id}-1.0.0.zip`); await enableRow(p, name); await L.sleep(1500); }
      p = await launch(); await until(p, '!!(window.PluginHost && PluginHost.last)');
    }
    const H = 'PluginHost.last';
    await p.ev(`window.__tick = 0; setInterval(() => __tick++, 100)`);
    const say = () => p.ev("document.getElementById('ed-status').textContent");
    const inv = (id, cmd) => p.ev(`${H}.invoke('${id}', '${cmd}')`);
    const audit = async (n) => JSON.parse(await p.ev(`JSON.stringify(${H}.audit.slice(${n}))`));
    const have = await p.ev(`${H}.plugins.map((x) => x.d.id)`);
    // M1 / M2: the Android store accepts the file, the Worker refuses the load
    for (const [row, id, re] of [['M1', 'test.bad-import', /imports not allowed: wasi_snapshot_preview1/], ['M2', 'test.bad-export', /exports a memory/]]) {
      const n = (await audit(0)).length; await inv(id, 'hello'); await L.sleep(1500);
      const a = await audit(n), s = await say();
      item(`${row}-load-refused`, have.includes(id) && a.some((x) => x.kind === 'failed' && re.test(x.error)), { plugin: id, audit: a, status: s });
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
    await save('cols'); const after = docText('cols-after.docx');
    const same = after.trim() === orig.trim();
    item('M5-saveAs-refused', seen['M5-saveAs-refused'].ok && same, { ...seen['M5-saveAs-refused'], commands_in_manifest: cmds, document_unchanged_after_save: same, text: after.slice(0, 200) }, shots.slice(0, 2));
    item('R1-reads-cap', seen['R1-reads-cap'].ok && same, { ...seen['R1-reads-cap'], document_unchanged_after_save: same }, [shots[2]]);
  });
}

if (what === 'translit' || what === 'all') await translit();
if (what === 'install' || what === 'all') await install();
if (what === 'rows' || what === 'all') await rows();
if (what === 'disable' || what === 'all') await disable();
if (what === 'cols') await cols();
log('SHA lines:\n' + sums.join('\n'));
fs.appendFileSync(path.join(BIG, 'SHA256SUMS.part'), sums.join('\n') + '\n');
process.exit(0);   // the CDP sockets keep node alive
