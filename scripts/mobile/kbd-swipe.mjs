// WP7 "keyboard reappears after a swipe" (PLAN phase 6 section 6, item 2): raise the keyboard, hide it with Gboard's chevron, swipe-scroll the
// document, read `mInputShown` from dumpsys after each step. One recording: kbd-swipe-<label>-<dp>.mp4.
// Precondition: session.mjs running with a document open (blank.docx plus some text) and the caret in it.
//   RIG_SCALE=0.5 node scripts/mobile/kbd-swipe.mjs <label> [--avd HRM_360] [--out <dir>]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const label = args[0] ?? 'run';
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const name = opt('--avd', 'HRM_360'), ser = L.serial(name), dp = L.AVDS[name].dp;
const out = opt('--out', 'E:/office-target/p6-kbd/swipe');
fs.mkdirSync(out, { recursive: true });
const shown = () => /mInputShown=true/.test(L.sh(ser, 'dumpsys input_method'));
const shot = (tag) => fs.writeFileSync(path.join(out, `${label}-${tag}.png`), L.screencap(ser));
const log = {};
const step = async (id, fn, wait = 3500) => { await fn(); await L.sleep(wait); log[id] = shown(); shot(id); console.log(id, 'keyboard shown:', log[id]); };

const rec = L.startRecord(ser); await L.sleep(1500);
await step('1-tap-in-text', async () => L.tap(ser, 600, 1300)); // panel space (1080x2400): the caret line of the document
await step('2-hide-with-chevron', async () => { for (let i = 0; i < 3 && shown(); i++) { L.tap(ser, 106, 2326); await L.sleep(3500); } }); // Gboard's down chevron = the Back button of the keyboard
await step('3-swipe-scroll-1', async () => L.swipe(ser, 600, 1500, 600, 700, 400)); // one finger drags the page up
await step('4-swipe-scroll-2', async () => L.swipe(ser, 600, 700, 600, 1500, 400)); // and back down
await step('5-tap-in-text-again', async () => L.tap(ser, 600, 1300)); // a tap must raise it again
const file = path.join(L.EVIDENCE_BIG, `kbd-swipe-${label}-${dp}.mp4`);
const rf = await L.stopRecord(ser, rec, file);
fs.appendFileSync(path.join(ROOT, 'docs/phase6/evidence/SHA256SUMS'), `${rf.sha256}  ${path.basename(file)}  (kbd-swipe)\n`);
fs.writeFileSync(path.join(out, `${label}.json`), JSON.stringify({ label, avd: name, dp, steps: log, recording: path.basename(file), sha256: rf.sha256 }, null, 2));
