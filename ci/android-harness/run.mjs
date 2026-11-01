// WP4 run: the WebView harness APK against the local docserver + fake HRM on AVD HRM_Dev_33 (API 33, Android System WebView 109, Gboard 12.4).
// open -> type (Gboard taps, Latin then Cyrillic) -> save -> status-6 callback -> close; then the frame-ancestors / frame_origins cases and
// the WebGL probe. Usage: node ci/android-harness/run.mjs [--out <dir>]   (emulator must be up: P6_DIR=... node scripts/mobile/rig.mjs start HRM_Dev_33)
// Preconditions on the AVD: Gboard has Russian added (Gboard settings > Languages) and is on the English layout; run.mjs ends on English.
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import * as L from '../../scripts/mobile/lib.mjs';
import { connect } from './cdp.mjs';
import { px } from './png.mjs';
import { ROOT, startEnv } from './env.mjs';

const AVD = 'HRM_Dev_33', S = L.serial(AVD), PORT = 9333, PKG = 'test.harness.webview';
const outDir = path.resolve(process.argv.includes('--out') ? process.argv[process.argv.indexOf('--out') + 1] : path.join(ROOT, 'docs/phase6/evidence/webview'));
const work = `E:/office-target/p6-webview/run-${Date.now()}`;
fs.mkdirSync(outDir, { recursive: true });
const result = { avd: AVD, steps: {}, notes: [] };
const step = (k, v) => { result.steps[k] = v; console.log(k, JSON.stringify(v)); };
const logcat = () => spawnSync('adb', ['-s', S, 'logcat', '-d', '-s', 'HARNESS'], { encoding: 'utf8' }).stdout.replace(/\r/g, '');
const events = () => logcat().split('\n').map((l) => /HARNESS : event (.*)$/.exec(l)?.[1]).filter(Boolean).map((j) => JSON.parse(j));
const until = async (what, f, ms = 120000) => { const t = Date.now(); while (Date.now() - t < ms) { const v = await f(); if (v) return v; await L.sleep(1000); } throw new Error('timeout: ' + what); };
const launch = (env) => {
  L.sh(S, 'am force-stop ' + PKG); L.adb(S, 'logcat', '-c');
  L.sh(S, `am start -n ${PKG}/.MainActivity --es ds '${env.DS}' --es cfg '${env.HRM}/files/1/editor-config?lang=en&device=mobile'`);
};
const build = (env) => {
  execFileSync('bash', [path.join(ROOT, 'ci/android-harness/build.sh'), path.join(env.dir, 'ca.pem'), path.join(work, 'harness.apk')], { stdio: 'inherit', env: { ...process.env, MSYS_NO_PATHCONV: '1' } });
  try { L.adb(S, 'uninstall', PKG); } catch { /* not installed */ } // the debug keystore is per run dir
  L.adb(S, 'install', '-r', path.join(work, 'harness.apk'));
};

// Gboard key centres (px, 1080x2400 @ 420 dpi, calibrated on screenshots of this AVD): English from lib.mjs, Russian rows measured here
const RU = {};
[...'йцукенгшщзх'].forEach((c, i) => (RU[c] = [54 + 97.2 * i, 1716]));
[...'фывапролджэ'].forEach((c, i) => (RU[c] = [54 + 97.2 * i, 1870]));
[...'ячсмитьбю'].forEach((c, i) => (RU[c] = [172 + 91.8 * i, 2026]));
const GLOBE = [326, 2180], SPACE = [593, 2180];
// the globe key cycles English <-> Russian; the shift key is light blue on the English layout and a plain letter key (white) on the Russian one
const layout = () => (px(L.screencap(S), [[140, 2000]])[0][0] > 240 ? 'ru' : 'en');
const ensure = async (want) => { for (let i = 0; i < 3 && layout() !== want; i++) { L.tap(S, ...GLOBE); await L.sleep(2000); } if (layout() !== want) throw new Error('Gboard layout is not ' + want); };
const typeKeys = async (text, map) => { for (const c of text) { const [x, y] = c === ' ' ? SPACE : map[c]; L.tap(S, x, y); await L.sleep(300); } };

// the WebGL probe, run in every frame of the page (host document, wrapper, editor): context creation, renderer, and which API the editor's canvases use
const PROBE = `(() => {
  const gl = (t) => { try { const c = document.createElement('canvas'); const g = c.getContext(t); if (!g) return null; const e = g.getExtension('WEBGL_debug_renderer_info'); return { renderer: e ? g.getParameter(e.UNMASKED_RENDERER_WEBGL) : g.getParameter(g.RENDERER), version: g.getParameter(g.VERSION) }; } catch (e) { return String(e); } };
  const canv = [...document.querySelectorAll('canvas')].map((c) => c.width + 'x' + c.height);
  return { href: location.href, origin: location.origin, ancestors: [...(location.ancestorOrigins || [])], isSecureContext, webgl1: gl('webgl'), webgl2: gl('webgl2'), canvases: canv.length, sample: canv.slice(0, 3),
    canvasIs2d: [...document.querySelectorAll('canvas')].map((c) => { try { return c.getContext('2d') ? '2d' : (c.getContext('webgl2') ? 'webgl2' : c.getContext('webgl') ? 'webgl' : 'other'); } catch { return 'err'; } }) };
})()`;

const env = await startEnv(path.join(work, 'env'));
result.docserver = env.DS; result.fake_hrm = env.HRM; result.frame_origins = env.origins;
try {
  L.sh(S, 'settings put global hide_error_dialogs 1'); L.sh(S, 'settings put secure show_ime_with_hard_keyboard 1');
  result.versions = L.versions(S);
  build(env);
  const rec = L.startRecord(S);
  launch(env);
  await until('document ready', () => events().some((e) => e.ev === 'ready'));
  const boot = events().find((e) => e.ev === 'boot');
  step('open', { ready: true, boot });
  await L.sleep(2000);

  // origin / frame-ancestors evidence and WebGL, over the WebView's devtools socket
  L.adb(S, 'forward', `tcp:${PORT}`, `localabstract:webview_devtools_remote_${L.sh(S, 'pidof ' + PKG)}`);
  const cdp = await connect(PORT);
  const probe = await cdp.evalAll(PROBE);
  step('frames', probe);

  // type: tap into the empty last paragraph, wait for Gboard, Latin then Cyrillic (globe key switches the Russian layout in)
  L.tap(S, 700, 1750);
  await until('IME shown', () => /mInputShown=true/.test(L.sh(S, 'dumpsys input_method')), 20000);
  await L.sleep(1500);
  await ensure('en');
  await typeKeys('salom ', L.EN_LAYOUT);
  await ensure('ru');
  await typeKeys('привет', RU);
  await ensure('en'); // leave English for the next run
  fs.writeFileSync(path.join(outDir, 'typed.png'), L.screencap(S));
  step('type', { latin: 'salom', cyrillic: 'привет', dirty: events().filter((e) => e.ev === 'dirty').map((e) => e.v) });

  // save (the host asks, as the wrapper's requestSave in the HRM app's save button) -> callback status 6, then close
  const top = (expr) => cdp.send('Runtime.evaluate', { expression: expr, returnByValue: true });
  const before = (await env.hrmState()).received.length;
  await top('window.__editor.requestSave()');
  await until('saved event', () => events().some((e) => e.ev === 'saved'), 60000);
  const st = await env.hrmState();
  const got = st.received.slice(before);
  step('save', { saved: events().filter((e) => e.ev === 'saved'), hrm_received_since_open: got.map((r) => r.status), hrm_rejected: st.rejected });
  await top('window.__editor.close()');
  await until('closed event', () => events().some((e) => e.ev === 'closed'), 90000);
  step('close', { closed: events().filter((e) => e.ev === 'closed') });
  cdp.close();
  step('ds_callback_log', env.dsLog().split('\n').filter((l) => /callback status/.test(l)));
  result.recording = await L.stopRecord(S, rec, path.join(outDir, 'webview-embed.mp4'));

  // the saved docx: v2.bin of the session
  const [sdir] = env.sessions();
  const v2 = path.join(sdir, 'v2.bin');
  const text = execFileSync('python', ['-X', 'utf8', '-I', '-c', 'import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read("word/document.xml").decode("utf8");print("".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>",x)))', v2], { encoding: 'utf8' });
  step('docx', { has_latin: text.includes('salom'), has_cyrillic: text.includes('привет'), tail: text.slice(-60) });
  if (!text.includes('salom') || !text.includes('привет')) result.notes.push('FAIL: typed text missing from the saved docx');

  // frame_origins negative: the operator lists only another site -> session create is refused (403), the editor never opens
  const bad = await startEnv(path.join(work, 'env-bad'), { frameOrigins: ['https://hrm.example.test'] });
  try {
    build(bad);
    launch(bad);
    await until('error or ready', () => events().some((e) => e.ev === 'error' || e.ev === 'ready') || /not a frame origin/.test(bad.dsLog()), 60000).catch(() => {});
    await L.sleep(3000);
    step('frame_origins_negative', { frame_origins: bad.origins, ready: events().some((e) => e.ev === 'ready'), events: events().map((e) => e.ev), ds_log: bad.dsLog().split('\n').filter((l) => /frame origin|auth.fail/.test(l)) });
  } finally { bad.stop(); }
} finally {
  fs.writeFileSync(path.join(outDir, 'ds-log.txt'), env.dsLog().split(String.fromCharCode(10)).filter((l) => !/token|secret/i.test(l)).join(String.fromCharCode(10)));
  fs.writeFileSync(path.join(outDir, 'logcat-harness.txt'), logcat());
  env.stop();
  try { L.adb(S, 'forward', '--remove', `tcp:${PORT}`); } catch { /* none */ }
  L.sh(S, 'am force-stop ' + PKG);
}
fs.writeFileSync(path.join(outDir, 'webview-embed.json'), JSON.stringify(result, null, 2));
console.log('wrote', path.join(outDir, 'webview-embed.json'));
