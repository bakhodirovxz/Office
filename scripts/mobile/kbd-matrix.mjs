// WP7 keyboard matrix (PLAN phase 6 section 6, item 1): the A0 rows re-run on HEAD through Gboard on HRM_360. One recording per row
// (<avd dp>: kbd-<row>-360.mp4 under E:/office-target/p6/evidence, SHA-256 in docs/phase6/evidence/SHA256SUMS), one screenshot after it.
// Precondition: scripts/mobile/session.mjs is running, blank.docx is open, the caret is in it and the Gboard UZ layout is up (the cycle is
// EN > RU > UZ > EN). Rows are separated by Enter; the saved docx text is read afterwards (GET /blob) and compared by hand with `expect` (Gboard capitalises the first letter of a paragraph).
//   node scripts/mobile/kbd-matrix.mjs [--avd HRM_360] [--out <dir>]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as L from './lib.mjs';

const FP = JSON.parse(fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), 'gboard-fp.json'), 'utf8'));

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const name = opt('--avd', 'HRM_360'), ser = L.serial(name), dp = L.AVDS[name].dp;
const out = opt('--out', 'E:/office-target/p6-kbd/matrix');
fs.mkdirSync(out, { recursive: true });
const GAP = 450; // the emulator renders in software: keep taps apart so every key lands

const key = async (map, ch) => { L.tap(ser, ...map[ch]); await L.sleep(GAP); };
const keys = async (map, s) => { for (const ch of s) await key(map, ch); };
const press = async (k) => { L.tap(ser, ...L.KEY[k]); await L.sleep(GAP); };
const shot = (tag) => { const f = path.join(out, `${tag}.png`); fs.writeFileSync(f, L.screencap(ser)); return f; };
// Gboard orders its languages by recent use, so the globe key's next language is not fixed: press it until the key area looks like `want`
// (fingerprints in gboard-fp.json, taken from this AVD's EN / UZ / RU layouts).
const which = () => { const fp = L.kbdFingerprint(ser); return Object.entries(FP).map(([k, v]) => [k, L.fpDist(fp, v)]).sort((a, b) => a[1] - b[1])[0]; };
async function setLang(want) {
  for (let i = 0; i < 4; i++) { const [k, d] = which(); if (k === want && d < 4) return; await press('globe'); await L.sleep(3500); }
  throw new Error(`could not reach the ${want} layout (is the keyboard up?)`);
}
const sums = path.join(ROOT, 'docs/phase6/evidence/SHA256SUMS');

const rows = [
  { id: 'uz-apostrophes', lang: 'UZ', note: 'o\u02BBg\u02BBil with U+02BB from the o\u02BB and g\u02BB keys, then the apostrophe key (U+02BC)', expect: 'o\u02BBg\u02BBil\u02BC',
    run: async () => { const U = L.UZ_LAYOUT; await key(U, '\u00f2'); await key(U, '\u011f'); await keys(U, 'il'); await key(U, '\u02bc'); } },
  { id: 'latin', lang: 'EN', note: 'hello world on the EN layout', expect: 'hello world',
    run: async () => { await keys(L.EN_LAYOUT, 'hello'); await press('space'); await keys(L.EN_LAYOUT, 'world'); } },
  { id: 'autocorrect', note: 'wolrd + space (Gboard replaces the word: delete-surrounding + commit)', expect: 'world ',
    run: async () => { await keys(L.EN_LAYOUT, 'wolrd'); await press('space'); } },
  { id: 'glide', note: 'glide-typed "hello" (emulator console touch injection)', expect: 'hello',
    run: async () => { await L.glide(name, 'hello'); await L.sleep(1500); } },
  { id: 'backspace', note: 'xyz then Backspace once', expect: 'xy',
    run: async () => { await keys(L.EN_LAYOUT, 'xyz'); await press('backspace'); } },
  { id: 'cyrillic-yo', lang: 'RU', note: 'RU layout: \u0441\u0430\u043b\u043e\u043c \u0434\u0443\u043d\u0451 with \u0451 from the long-press popup of \u0435', expect: '\u0441\u0430\u043b\u043e\u043c \u0434\u0443\u043d\u0451',
    run: async () => {
      const R = L.RU_LAYOUT; await keys(R, '\u0441\u0430\u043b\u043e\u043c'); await press('space'); await keys(R, '\u0434\u0443\u043d');
      // hold е: the popup is [е́, 5, ё, ъ]; slide right to the third one (115 px right, 170 px up in panel space) and release
      const e = R['е'];
      await L.hold(name, e, { during: async () => shot('cyrillic-yo-popup'), to: [e[0] + 115, e[1] - 170] }); await L.sleep(GAP);
    } },
  { id: 'enter', lang: 'UZ', note: 'ok, Enter, no (Enter makes a paragraph although the agent is an <input>)', expect: 'ok\nok2',
    run: async () => { await keys(L.UZ_LAYOUT, 'ok'); await press('enter'); await keys(L.UZ_LAYOUT, 'no'); } },
];
// 'enter' row typed 'ok' + Enter + 'okz'; fix expectation below
rows.at(-1).expect = 'ok\nokz';

const result = [];
for (const r of rows) {
  if (r.lang) await setLang(r.lang);
  const rec = L.startRecord(ser); await L.sleep(1500);
  const t0 = Date.now();
  await r.run();
  await L.sleep(2500);
  const png = shot(r.id);
  const file = path.join(L.EVIDENCE_BIG, `kbd-${r.id}-${dp}.mp4`);
  const rf = await L.stopRecord(ser, rec, file);
  fs.appendFileSync(sums, `${rf.sha256}  ${path.basename(file)}  (kbd-matrix)\n`);
  result.push({ id: r.id, note: r.note, expect: r.expect, ms: Date.now() - t0, screenshot: png, recording: path.basename(file), sha256: rf.sha256 });
  console.log(r.id, 'recorded');
  if (r.id !== 'enter') { await press('enter'); await L.sleep(1500); } // row separator
}
fs.writeFileSync(path.join(out, 'matrix.json'), JSON.stringify(result, null, 2));
