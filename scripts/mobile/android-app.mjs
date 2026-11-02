// WP8 evidence run on an emulator: the Android app opens a docx (from the Files app picker, or a VIEW / SEND intent), a Latin and a Cyrillic word are typed
// with Gboard, Save writes back, the file is pulled and its text checked. One screen recording per scenario.
// Usage: node scripts/mobile/android-app.mjs <picker|view|send|readonly|rotate> [--out dir]   (emulator HRM_Pixel_8 running, app installed)
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import * as L from './lib.mjs';

const args = process.argv.slice(2), scenario = args[0];
const S = L.serial('HRM_Pixel_8'), PKG = 'app.hoshiya.suite', ACT = `${PKG}/office.android.MainActivity`;
const runId = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
const out = args.includes('--out') ? args[args.indexOf('--out') + 1] : path.join(L.EVIDENCE_BIG, runId);
fs.mkdirSync(out, { recursive: true });
const FIX = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\//, '')), 'fixtures/phone-ux.docx');
let DOCOUT = '';
const DOC = '/sdcard/Download/phone-ux.docx', MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
const shot = (n) => fs.writeFileSync(path.join(out, `${scenario}-${n}.png`), L.screencap(S));
const am = (c) => L.sh(S, `am start ${c} -n ${ACT}`);
const log = (...a) => console.log(`[${scenario}]`, ...a);

async function docText() {
  fs.mkdirSync(path.join(out, 'pulled'), { recursive: true });
  const f = path.join(out, 'pulled', `${scenario}.docx`);
  L.adb(S, 'pull', DOCOUT || DOC, f);
  return execFileSync('python3', ['-X', 'utf8', '-I', '-c', 'import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read("word/document.xml").decode("utf8");print("".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>",x)))', f], { encoding: 'utf8' });
}
const mediaUri = async () => {
  for (let i = 0; i < 10; i++) {
    const m = /_id=(\d+), _display_name=phone-ux\.docx/.exec(L.sh(S, 'content query --uri content://media/external/downloads --projection _id:_display_name'));
    if (m) return `content://media/external/downloads/${m[1]}`;
    await L.sleep(1000);
  }
  throw new Error('phone-ux.docx is not in the media store');
};
async function typeAndSave() {
  L.tap(S, 966, 2220); await L.sleep(3000);          // the pencil: edit mode (Docs phone layout)
  L.tap(S, 780, 345); await L.sleep(4000);           // caret at the end of the first heading
  execFileSync('node', [path.resolve('scripts/mobile/android-type.mjs'), S, 'salom', 'привет'], { stdio: 'inherit' });
  await L.sleep(1500); shot('typed');
  L.tap(S, 1016, 197); await L.sleep(2500); shot('more');   // More -> Save
  L.tap(S, 184, 935); await L.sleep(4000); shot('saved');
}
const start = async () => { L.sh(S, 'am force-stop ' + PKG); L.sh(S, 'rm -f ' + DOC); L.adb(S, 'push', FIX, DOC); L.sh(S, `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://${DOC}`); await L.sleep(5000); };

await start();
const rec = L.startRecord(S);
const grants = '--grant-read-uri-permission --grant-write-uri-permission';
if (scenario === 'picker') {
  am(''); await L.sleep(7000);
  L.tap(S, 187, 1957); await L.sleep(4000); shot('files-app');
  L.tap(S, 73, 194); await L.sleep(1500); L.tapText(S, /^Downloads$/); await L.sleep(2500);   // Recent is empty until the media store has indexed the push
  for (let i = 0; i < 8 && !L.tapText(S, /phone-ux\.docx/); i++) await L.sleep(2500);
  await L.sleep(15000); shot('opened');
} else if (scenario === 'view' || scenario === 'readonly') {
  const u = await mediaUri();
  am(`-a android.intent.action.VIEW -d ${u} -t ${MIME} --grant-read-uri-permission ${scenario === 'view' ? '--grant-write-uri-permission' : ''}`); await L.sleep(20000); shot('opened');
} else if (scenario === 'send') {   // `am` cannot grant an EXTRA_STREAM (a real chooser share puts it in ClipData with the grant): the same uri also goes in the data so the grant covers it
  const u = await mediaUri();
  am(`-a android.intent.action.SEND -t ${MIME} -d ${u} --eu android.intent.extra.STREAM ${u} ${grants}`); await L.sleep(20000); shot('opened');
}
await typeAndSave();
if (scenario === 'readonly') {   // the write was refused: the system's Save as dialog is up; save under a new name next to the original
  shot('save-as-dialog');
  await L.sleep(1500);
  if (!L.tapText(S, /^Save$/i)) throw new Error('no Save button in the Save as dialog');
  await L.sleep(4000); shot('after-save-as');
  const files = L.sh(S, 'ls -t /sdcard/Download').split(/\r?\n/); log('Download:', files.join(', '));
  DOCOUT = '/sdcard/Download/' + files[0];
}
const txt = await docText();
const at = txt.indexOf('привет');
log('Latin word saved:', /salom привет/i.test(txt.slice(at - 7, at + 6)), ' Cyrillic word saved:', at >= 0, ' typed run in the file:', JSON.stringify(txt.slice(at - 12, at + 6)));
if (scenario === 'view') {   // reopen: the saved text is read back from the file; then rotation keeps the document
  L.sh(S, 'am force-stop ' + PKG);
  const u = await mediaUri();
  am(`-a android.intent.action.VIEW -d ${u} -t ${MIME} --grant-read-uri-permission --grant-write-uri-permission`); await L.sleep(20000);
  L.swipe(S, 540, 1500, 540, 700); await L.sleep(1500); shot('reopened');
  L.sh(S, 'settings put system accelerometer_rotation 0; settings put system user_rotation 1'); await L.sleep(5000); shot('landscape');
  L.sh(S, 'settings put system user_rotation 0'); await L.sleep(5000); shot('portrait-again');
  L.sh(S, 'settings put system accelerometer_rotation 1');
}
const r = await L.stopRecord(S, rec, path.join(out, `${scenario}.mp4`));
log(JSON.stringify(r));
