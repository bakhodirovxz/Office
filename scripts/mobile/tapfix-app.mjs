// WP12 tap-through check (scripts/mobile/tapfix.mjs) in the Android APP instead of Chrome (Phase 7 WP9): open phone-ux.docx, More > Done editing,
// the round Edit button (1), the first heading (2), type with Gboard (3), Home; then read the app's recovery file (the editor's autosave, files/recover/docs)
// and look for the picture in it. Emulator HRM_Pixel_8 at `wm size 540x1200` with RIG_SCALE=0.5, the APK installed.
//   node scripts/mobile/tapfix-app.mjs --tag <before|after> [--hold <s>]       Evidence: docs/phase7/evidence/android/tapfix-<tag>-*.png and .json
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const S = L.serial('HRM_Pixel_8'), PKG = 'app.hoshiya.suite', ACT = `${PKG}/office.android.MainActivity`, tag = opt('--tag', 'run');
const OUT = path.join(ROOT, 'docs/phase7/evidence/android'), TMP = process.env.TMP_DIR ?? 'E:/office-target/tmp/p7-android';
const DOC = '/sdcard/Download/phone-ux.docx', MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
const shot = (n) => fs.writeFileSync(path.join(OUT, `tapfix-${tag}-${n}.png`), L.screencap(S));
const P = { more: [1016, 197], done: [250, 820], pencil: [966, 2220], heading: [320, 410] };   // rig coordinates (1080x2400 space)
for (const k of Object.keys(P)) if (process.env['P_' + k]) P[k] = process.env['P_' + k].split(',').map(Number);

L.gboardEnable(S);
L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'pm clear ' + PKG); L.sh(S, 'rm -f ' + DOC);
L.adb(S, 'push', path.join(ROOT, 'scripts/mobile/fixtures/phone-ux.docx'), DOC);
L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DOC}`); await L.sleep(5000);
let uri;
for (let i = 0; i < 10 && !uri; i++) { const m = /_id=(\d+), _display_name=phone-ux\.docx/.exec(L.sh(S, 'content query --uri content://media/external/downloads --projection _id:_display_name')); if (m) uri = `content://media/external/downloads/${m[1]}`; else await L.sleep(1000); }
L.sh(S, `am start -a android.intent.action.VIEW -d ${uri} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission -n ${ACT}`);
await L.sleep(45000); shot("01-opened");
if (opt('--hold')) { await L.sleep(Number(opt('--hold')) * 1000); process.exit(0); }
L.tap(S, ...P.more); await L.sleep(10000); shot('02-more');   // the tap raises the keyboard; the sheet settles after it (as in tapfix.mjs)
L.tap(S, ...P.done); await L.sleep(8000); shot('03-reading');
L.tap(S, ...P.pencil); await L.sleep(5000); shot('04-after-pencil');
L.tap(S, ...P.heading); await L.sleep(5000); shot('05-after-heading-tap');
execFileSync('node', [path.join(ROOT, 'scripts/mobile/android-type.mjs'), S, 'tapfix', 'тест'], { stdio: 'inherit' });
await L.sleep(1500); shot('06-typed');
L.sh(S, 'am start -a android.intent.action.MAIN -c android.intent.category.HOME'); await L.sleep(8000);
const f = path.join(TMP, `tapfix-${tag}-docs.recover`);
let got = { autosave: false };
try {
  fs.writeFileSync(f, execFileSync('adb', ['-s', S, 'exec-out', 'run-as', PKG, 'cat', 'files/recover/docs'], { maxBuffer: 1 << 27 }));
  const b = fs.readFileSync(f), nl = b.indexOf(10), docx = path.join(TMP, `tapfix-${tag}.docx`);
  fs.writeFileSync(docx, b.subarray(nl + 1));
  const names = execFileSync('python3', ['-I', '-c', 'import sys,zipfile;print(chr(10).join(zipfile.ZipFile(sys.argv[1]).namelist()))', docx], { encoding: 'utf8' }).split(/\r?\n/);
  got = { autosave: true, name: b.subarray(0, nl).toString(), bytes: b.length - nl - 1, picture_in_autosave: names.includes('word/media/image1.png'), media: names.filter((n) => n.startsWith('word/media/')) };
} catch (e) { got.error = String(e.message).slice(0, 200); }
fs.writeFileSync(path.join(OUT, `tapfix-${tag}-autosave.json`), JSON.stringify({ tag, device: 'HRM_Pixel_8 (emulator), app WebView', run: new Date().toISOString(), ...got }, null, 2));
console.log(`[${tag}]`, JSON.stringify(got));
process.exit(0);
