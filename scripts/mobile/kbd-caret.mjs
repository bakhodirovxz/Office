// WP7 item 3: the caret must stay above the keyboard while typing at the bottom of the visible area. Types a letter and Enter 45 times in a blank
// document (the caret walks down past the first screen), screenshots every 15 lines and at the end, one recording kbd-caret-<label>-<dp>.mp4.
//   RIG_SCALE=0.5 node scripts/mobile/kbd-caret.mjs <label> [--avd HRM_360] [--out <dir>]   (session.mjs running, blank.docx open, caret in it, EN or UZ layout up)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const label = args[0] ?? 'run';
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const name = opt('--avd', 'HRM_360'), ser = L.serial(name), dp = L.AVDS[name].dp;
const out = opt('--out', 'E:/office-target/p6-kbd/caret');
fs.mkdirSync(out, { recursive: true });
const shot = (tag) => fs.writeFileSync(path.join(out, `${label}-${tag}.png`), L.screencap(ser));
const rec = L.startRecord(ser); await L.sleep(1500);
L.tap(ser, 600, 1300); await L.sleep(4000);
for (let i = 1; i <= 45; i++) {
  L.tap(ser, ...L.UZ_LAYOUT.a); await L.sleep(350);
  L.tap(ser, ...L.KEY.enter); await L.sleep(350);
  if (i % 15 === 0) { await L.sleep(2500); shot(`line-${i}`); }
}
await L.sleep(2500); shot('end');
const file = path.join(L.EVIDENCE_BIG, `kbd-caret-${label}-${dp}.mp4`);
const rf = await L.stopRecord(ser, rec, file);
fs.appendFileSync(path.join(ROOT, 'docs/phase6/evidence/SHA256SUMS'), `${rf.sha256}  ${path.basename(file)}  (kbd-caret)\n`);
console.log('done', rf.bytes);
