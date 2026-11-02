// Phase 6 emulator rig (PLAN phase 6, section 10): AVD control, adb helpers, CDP forward, screenrecord, the glide injector.
// Our code; the Phase 0 harnesses (spikes/mobile-a/probe/scripts/glide.py, docs/phase0/mobile-a-kill.md) are reference only.
// Rules baked in: never `adb shell input keyevent` (flips Gboard into its hardware-keyboard bar); -gpu swiftshader_indirect
// -no-snapshot-save; only processes this rig started are killed (`adb -s <serial> emu kill`).
import { spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

export const SDK = process.env.ANDROID_SDK_ROOT ?? path.join(process.env.LOCALAPPDATA ?? '', 'Android/Sdk');
export const EMU = path.join(SDK, 'emulator/emulator.exe');
export const P6 = process.env.P6_DIR ?? 'E:/office-target/p6';
export const EVIDENCE_BIG = path.join(P6, 'evidence');
export const AVD_HOME_360 = path.join(P6, 'avd');
// Console ports: serial is emulator-<port>. dp = CSS width of the viewport.
export const AVDS = {
  HRM_Pixel_8: { port: 5554, dp: 412, size: '1080x2400', home: null },
  HRM_360: { port: 5556, dp: 360, size: '1080x2400', home: AVD_HOME_360 },
  HRM_Dev_33: { port: 5560, dp: 393, size: '1080x2400', home: null }, // WP4: API 33 google_apis image (WebView embed proof)
};
export const serial = (n) => `emulator-${AVDS[n].port}`;
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function adb(ser, ...a) {
  const r = spawnSync('adb', ['-s', ser, ...a], { encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`adb ${a.join(' ')}: ${r.stderr || r.stdout}`);
  return r.stdout;
}
export const screencap = (ser) => spawnSync('adb', ['-s', ser, 'exec-out', 'screencap', '-p'], { maxBuffer: 1 << 28 }).stdout;
// one string for the device shell (quoting survives adb's argument joining)
export const sh = (ser, cmd) => adb(ser, 'shell', cmd).replace(/\r/g, '').trim();
// Every coordinate the rig takes is in the 1080x2400 panel space; RIG_SCALE=0.5 maps them onto a `wm size 540x1200` / `wm density 240` display (same 360 dp,
// a quarter of the pixels: the software GL of the emulator starves the guest at full size, see docs/phase6/keyboard.md).
export const SCALE = Number(process.env.RIG_SCALE ?? 1);
export const tap = (ser, x, y) => sh(ser, `input tap ${Math.round(x * SCALE)} ${Math.round(y * SCALE)}`);
export const swipe = (ser, x1, y1, x2, y2, ms = 300) => sh(ser, `input swipe ${Math.round(x1 * SCALE)} ${Math.round(y1 * SCALE)} ${Math.round(x2 * SCALE)} ${Math.round(y2 * SCALE)} ${ms}`);

/** Tap the first UI node whose text/content-desc matches `re` (uiautomator dump). Returns false when absent. */
export function tapText(ser, re) {
  sh(ser, 'uiautomator dump /sdcard/ui.xml >/dev/null');
  const xml = sh(ser, 'cat /sdcard/ui.xml');
  for (const m of xml.matchAll(/<node [^>]*?(?:text|content-desc)="([^"]*)"[^>]*?bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/g)) {
    if (re.test(m[1])) { tap(ser, (+m[2] + +m[4]) / 2, (+m[3] + +m[5]) / 2); return true; }
  }
  return false;
}

export function isUp(ser) {
  const r = spawnSync('adb', ['-s', ser, 'shell', 'getprop', 'sys.boot_completed'], { encoding: 'utf8' });
  return r.status === 0 && r.stdout.trim() === '1';
}

export async function startAvd(name) {
  const a = AVDS[name]; if (!a) throw new Error(`unknown AVD ${name}`);
  const ser = serial(name);
  if (isUp(ser)) return ser;
  fs.mkdirSync(path.join(P6, 'logs'), { recursive: true });
  const log = fs.openSync(path.join(P6, 'logs', `emu-${name}.log`), 'w');
  const env = { ...process.env, ...(a.home ? { ANDROID_AVD_HOME: a.home } : {}) };
  const p = spawn(EMU, ['-avd', name, '-port', String(a.port), '-gpu', 'swiftshader_indirect', '-no-snapshot-save', '-no-boot-anim'],
    { detached: true, stdio: ['ignore', log, log], env });
  p.unref();
  for (let i = 0; i < 120; i++) { await sleep(3000); if (isUp(ser)) { await sleep(5000); return ser; } }
  throw new Error(`${name} did not boot in 6 minutes`);
}
export const stopAvd = (name) => { try { adb(serial(name), 'emu', 'kill'); } catch { /* not running */ } };

/** The 360 dp AVD: same image as HRM_Pixel_8, 1080x2400 at 480 dpi = 360 dp wide, under E: (no avdmanager in this SDK, so the files are written). */
export function create360() {
  const dir = path.join(AVD_HOME_360, 'HRM_360.avd');
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(AVD_HOME_360, 'HRM_360.ini'), `avd.ini.encoding=UTF-8\npath=${dir.replace(/\//g, '\\')}\ntarget=android-36\n`);
  fs.writeFileSync(path.join(dir, 'config.ini'), [
    'AvdId=HRM_360', 'avd.ini.displayname=HRM_360', 'PlayStore.enabled=yes', 'abi.type=x86_64', 'hw.cpu.arch=x86_64', 'hw.cpu.ncore=4',
    'hw.device.name=pixel_8', 'hw.device.manufacturer=Google', 'hw.lcd.width=1080', 'hw.lcd.height=2400', 'hw.lcd.density=480',
    'hw.ramSize=4096', 'hw.keyboard=no', 'hw.gpu.enabled=yes', 'hw.gpu.mode=auto', 'hw.initialOrientation=portrait', 'hw.mainKeys=no',
    'hw.sdCard=no', 'disk.dataPartition.size=6G', 'showDeviceFrame=no', 'tag.id=google_apis_playstore', 'tag.display=Google Play',
    'image.sysdir.1=system-images\\android-36\\google_apis_playstore\\x86_64\\', 'target=android-36', ''].join('\n'));
  return dir;
}

/** Android-level IME subtypes for Gboard. Gboard's own language list is private; whether it follows these is checked on screen (`rig.mjs shot`). */
const GBOARD = 'com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME';
export function gboardState(ser) {
  const dump = sh(ser, 'dumpsys input_method');
  const names = [...new Set([...dump.matchAll(/mImeName=Gboard mSubtypeName=([^{}]*?) mSubtypeIndex/g)].map((m) => m[1]))];
  return {
    installed: sh(ser, 'pm list packages com.google.android.inputmethod.latin').includes('latin'),
    default_ime: sh(ser, 'settings get secure default_input_method'),
    enabled_subtypes: names,
    want: { English: names.some((n) => /English/.test(n)), Russian: names.some((n) => /Russian/.test(n)), 'Uzbek (latin)': names.some((n) => /Uzbek/.test(n)) },
  };
}
// subtype hash codes from `dumpsys input_method` of the Gboard 15.1 image: en_US, ru_RU, uz (ascii-capable = Latin)
export function gboardEnable(ser) {
  sh(ser, `ime enable ${GBOARD}; ime set ${GBOARD}; settings put secure show_ime_with_hard_keyboard 1; settings put secure stylus_handwriting_enabled 0`);
  sh(ser, `settings put secure enabled_input_methods '${GBOARD};137935560;-988987337;1151084727:com.google.android.tts/com.google.android.apps.speech.tts.googletts.settings.asr.voiceime.VoiceInputMethodService'`);
  return gboardState(ser);
}

export function versions(ser) {
  const v = (p) => (sh(ser, `dumpsys package ${p} | grep versionName`).split('\n')[0] || '').replace(/.*versionName=/, '');
  return {
    avd_api: sh(ser, 'getprop ro.build.version.sdk'), size: sh(ser, 'wm size'), density: sh(ser, 'wm density'),
    chrome: v('com.android.chrome'), webview_package: v('com.google.android.webview'),
    webview_active: (sh(ser, 'dumpsys webviewupdate').match(/Current WebView package \(name, version\): \(([^)]*)\)/) ?? [])[1] ?? null,
    gboard: v('com.google.android.inputmethod.latin'),
  };
}

/** adb reverse for the host server (so the page is on http://localhost:<port>, a secure context) and the CDP forward for Chrome. */
export function reverse(ser, port) { adb(ser, 'reverse', `tcp:${port}`, `tcp:${port}`); }
export function forwardCdp(ser, port = 9222) { adb(ser, 'forward', `tcp:${port}`, 'localabstract:chrome_devtools_remote'); return `http://127.0.0.1:${port}`; }
export function openInChrome(ser, url) {
  sh(ser, `am start -n com.android.chrome/com.google.android.apps.chrome.Main -a android.intent.action.VIEW -d '${url}'`);
}
/** Bring the tab whose URL starts with `prefix` to the foreground (Playwright's bringToFront is not enough on Android Chrome). */
export async function activateTab(cdpUrl, prefix) {
  const t = (await (await fetch(`${cdpUrl}/json/list`)).json()).find((x) => x.type === 'page' && x.url.startsWith(prefix));
  if (!t) throw new Error(`no tab at ${prefix}`);
  await fetch(`${cdpUrl}/json/activate/${t.id}`);
}
/** Chrome's first-run screen (shown once per fresh data partition): "Use without an account", then dismiss follow-ups. */
export function chromeFirstRun(ser) {
  for (const re of [/^Use without an account$/, /^(No thanks|Got it|Skip|No, thanks)$/i]) tapText(ser, re);
}

// ---- screenrecord: one file per checklist item (3-minute cap per file, so long items are split by the caller)
export function startRecord(ser, remote = '/sdcard/rig.mp4') {
  sh(ser, `rm -f ${remote}`);
  const p = spawn('adb', ['-s', ser, 'shell', `screenrecord --size 540x1200 --bit-rate 3000000 --time-limit 180 ${remote}`], { stdio: 'ignore' });
  return { p, remote };
}
export async function stopRecord(ser, rec, dest) {
  sh(ser, 'pkill -2 screenrecord || true');
  await Promise.race([new Promise((r) => rec.p.once('exit', r)), sleep(8000)]);
  await sleep(1000);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  adb(ser, 'pull', rec.remote, dest);
  const sha = crypto.createHash('sha256').update(fs.readFileSync(dest)).digest('hex');
  return { file: dest, bytes: fs.statSync(dest).size, sha256: sha };
}

// ---- emulator console (needs ~/.emulator_console_auth_token) and the glide injector
export async function consoleOpen(port) {
  const tok = fs.readFileSync(path.join(os.homedir(), '.emulator_console_auth_token'), 'utf8').trim();
  const s = net.createConnection(port, '127.0.0.1');
  let buf = '', waiter = null;
  s.on('data', (d) => { buf += d; if (waiter && /(OK|KO[^\n]*)\r?\n/.test(buf)) { const w = waiter; waiter = null; buf = ''; w(); } });
  await new Promise((r, j) => { s.once('connect', r); s.once('error', j); });
  const cmd = (c) => new Promise((r) => { waiter = r; s.write(c + '\r\n'); setTimeout(() => { if (waiter === r) { waiter = null; r(); } }, 2000); });
  await sleep(300); buf = '';
  await cmd(`auth ${tok}`);
  return { cmd, close: () => s.destroy() };
}
// Gboard EN key centres for the 1080x2400 @ 420 dpi image (Phase 0 calibration). Another density or keyboard height needs its own
// map: pass `layout` ({char: [x, y]} in px) read off a screenshot (`rig.mjs shot`).
export const EN_LAYOUT = (() => {
  const m = {};
  [...'qwertyuiop'].forEach((c, i) => (m[c] = [58 + 107.1 * i, 1718]));
  [...'asdfghjkl'].forEach((c, i) => (m[c] = [111 + 107.1 * i, 1869]));
  [...'zxcvbnm'].forEach((c, i) => (m[c] = [218 + 107.1 * i, 2022]));
  return m;
})();
// Gboard 15.1 key centres read off `rig.mjs shot` on HRM_360 (1080x2400 @ 480 dpi, 360 dp). The keys are laid out by screen width in px, so EN is
// the same map as on the 412 dp AVD (checked on both). RU is ЙЦУКЕН (12/12/9 keys); UZ is O‘zbek (lotin) QWERTY with oʻ and gʻ keys (11/11/7).
export const KEY = { shift: [86, 2022], backspace: [994, 2022], sym: [86, 2174], comma: [216, 2174], globe: [324, 2174], space: [592, 2174], period: [816, 2174], enter: [994, 2174] };
export const RU_LAYOUT = (() => {
  const m = {};
  [...'йцукенгшщзх'].forEach((c, i) => (m[c] = [55 + 97 * i, 1720]));
  [...'фывапролджэ'].forEach((c, i) => (m[c] = [62 + 96.4 * i, 1870]));
  [...'ячсмитьбю'].forEach((c, i) => (m[c] = [173 + 91.8 * i, 2022]));
  return m;
})();
export const UZ_LAYOUT = (() => {
  const m = { ...EN_LAYOUT };
  [...'qwertyuiop'].forEach((c, i) => (m[c] = [55 + 96.8 * i, 1720]));
  m['ò'] = [1023, 1720];                       // the oʻ key (U+02BB after it)
  [...'asdfghjkl'].forEach((c, i) => (m[c] = [55 + 96.8 * i, 1870]));
  m['ğ'] = [926, 1870]; m['ʼ'] = [1026, 1870]; // the gʻ key, and the apostrophe key (U+02BC)
  return m;
})();
/** Coarse fingerprint of the keyboard's key area (36x14 grey cells of the raw screencap): tells the Gboard layouts apart (EN / RU / UZ), which the accessibility tree does not. */
export function kbdFingerprint(ser) {
  const r = spawnSync('adb', ['-s', ser, 'exec-out', 'screencap'], { maxBuffer: 1 << 28 }).stdout;
  const w = r.readUInt32LE(0), h = r.readUInt32LE(4), px = r.subarray(12);
  const y0 = Math.floor(h * 0.655), y1 = Math.floor(h * 0.86), fp = [];
  for (let gy = 0; gy < 14; gy++) for (let gx = 0; gx < 36; gx++) {
    let sum = 0, n = 0;
    for (let y = y0 + Math.floor((y1 - y0) * gy / 14); y < y0 + Math.floor((y1 - y0) * (gy + 1) / 14); y += 2)
      for (let x = Math.floor(w * gx / 36); x < Math.floor(w * (gx + 1) / 36); x += 2) { const i = (y * w + x) * 4; sum += px[i] + px[i + 1] + px[i + 2]; n++; }
    fp.push(sum / n / 3);
  }
  return fp;
}
export const fpDist = (a, b) => a.reduce((s, v, i) => s + Math.abs(v - b[i]), 0) / a.length;
/** Glide `word` over the on-screen keyboard through the emulator console (14 ms steps, ~18 px spacing, dwell on the last key). */
export async function glide(name, word, { layout = EN_LAYOUT, stepMs = 14 } = {}) {
  const [W, H] = AVDS[name].size.split('x').map(Number);
  const pts = [...word.toLowerCase()].map((c) => { if (!layout[c]) throw new Error(`no key for "${c}" in the layout`); return layout[c]; });
  const keys = [pts[0]]; for (const p of pts.slice(1)) if (p[0] !== keys.at(-1)[0] || p[1] !== keys.at(-1)[1]) keys.push(p);
  const dense = [];
  keys.slice(1).forEach((b, i) => { const a = keys[i]; const n = Math.max(6, Math.floor(Math.hypot(a[0] - b[0], a[1] - b[1]) / 18)); for (let k = 0; k < n; k++) dense.push([a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n]); });
  dense.push(keys.at(-1), ...Array(6).fill(keys.at(-1)));
  const raw = ([x, y]) => [Math.round(x / W * 32767), Math.round(y / H * 32767)];
  const c = await consoleOpen(AVDS[name].port);
  const ev = (t, k, v) => c.cmd(`event send ${t}:${k}:${v}`);
  const [x0, y0] = raw(dense[0]);
  await ev(3, 47, 0); await ev(3, 57, 1234); await ev(3, 53, x0); await ev(3, 54, y0); await ev(3, 58, 100); await ev(1, 330, 1); await ev(0, 0, 0);
  await sleep(100);
  for (const p of dense.slice(1)) { const [x, y] = raw(p); await ev(3, 53, x); await ev(3, 54, y); await ev(0, 0, 0); await sleep(stepMs); }
  await sleep(150);
  await ev(3, 57, -1); await ev(1, 330, 0); await ev(0, 0, 0);
  c.close();
  return { points: dense.length };
}

/** Touch down at (x, y) (1080x2400 panel space), hold `holdMs` while `during()` runs (a screenshot of the long-press popup), optionally slide to (x2, y2), release. */
export async function hold(name, [x, y], { holdMs = 1200, during = async () => {}, to = null } = {}) {
  const [W, H] = AVDS[name].size.split('x').map(Number);
  const raw = (px, py) => [Math.round(px / W * 32767), Math.round(py / H * 32767)];
  const c = await consoleOpen(AVDS[name].port);
  const ev = (t, k, v) => c.cmd(`event send ${t}:${k}:${v}`);
  const [rx, ry] = raw(x, y);
  await ev(3, 47, 0); await ev(3, 57, 4321); await ev(3, 53, rx); await ev(3, 54, ry); await ev(3, 58, 100); await ev(1, 330, 1); await ev(0, 0, 0);
  await sleep(holdMs); await during();
  if (to) { for (let k = 1; k <= 8; k++) { const [qx, qy] = raw(x + (to[0] - x) * k / 8, y + (to[1] - y) * k / 8); await ev(3, 53, qx); await ev(3, 54, qy); await ev(0, 0, 0); await sleep(60); } await sleep(300); }
  await ev(3, 57, -1); await ev(1, 330, 0); await ev(0, 0, 0);
  c.close();
}

/** hoshiya-serve on a free port from a built dist (same invocation as ci/web-smoke/shell.spec.ts). */
export async function startServe({ root, dist, svc = 'E:/office-target/p1/services/release' }) {
  const dir = fs.mkdtempSync(path.join(process.env.TEMP ?? os.tmpdir(), 'rig-serve-'));
  const port = await new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
  fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
  const proc = spawn(path.join(svc, 'hoshiya-serve.exe'), ['--config', path.join(dir, 'api.toml'), '--dist', dist, '--shell', path.join(dist, 'shell'), '--brand', path.join(root, 'brand/brand.toml')]);
  await new Promise((ok, bad) => { let o = ''; proc.stdout.on('data', (d) => { o += d; if (/listening on http/.test(o)) ok(); }); proc.on('exit', (c) => bad(new Error(`serve exited ${c}`))); setTimeout(() => bad(new Error('serve did not start')), 15000); });
  return { port, base: `http://localhost:${port}`, stop: () => { proc.kill(); fs.rmSync(dir, { recursive: true, force: true }); } };
}
