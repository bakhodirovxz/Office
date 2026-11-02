// WP10a PDF rows: scroll, pinch, long-press select, find-field typing on Gboard, in the survey.mjs session (PDF open, side panes closed).
//   node scripts/mobile/survey-pdf.mjs <scroll|pinch|select|type> <out-dir>      (RIG_SCALE=0.5; coordinates are 540x1200 screen pixels)
import fs from 'node:fs';
import path from 'node:path';
import * as L from './lib.mjs';
const [step, out] = process.argv.slice(2), S = L.serial('HRM_Pixel_8'), AV = L.AVDS.HRM_Pixel_8;
fs.mkdirSync(out, { recursive: true });
const shot = (n) => fs.writeFileSync(path.join(out, `pdf-${n}.png`), L.screencap(S));
const raw = ([x, y]) => [Math.round(x * 2 / 1080 * 32767), Math.round(y * 2 / 2400 * 32767)];   // screen px (540x1200) to the console's 0..32767
async function pinch(c, d0, d1, steps = 12) {
  const c0 = await L.consoleOpen(AV.port), ev = (t, k, v) => c0.cmd(`event send ${t}:${k}:${v}`);
  const pos = async (slot, id, [x, y]) => { const [rx, ry] = raw([x, y]); await ev(3, 47, slot); await ev(3, 57, id); await ev(3, 53, rx); await ev(3, 54, ry); await ev(3, 58, 100); };
  await pos(0, 2001, [c[0] - d0 / 2, c[1]]); await ev(1, 330, 1); await pos(1, 2002, [c[0] + d0 / 2, c[1]]); await ev(0, 0, 0); await L.sleep(150);
  for (let i = 1; i <= steps; i++) { const d = d0 + (d1 - d0) * i / steps; await pos(0, 2001, [c[0] - d / 2, c[1]]); await pos(1, 2002, [c[0] + d / 2, c[1]]); await ev(0, 0, 0); await L.sleep(40); }
  await ev(3, 47, 0); await ev(3, 57, -1); await ev(3, 47, 1); await ev(3, 57, -1); await ev(1, 330, 0); await ev(0, 0, 0);
  c0.close();
}
if (step === 'scroll') { shot('scroll-before'); L.swipe(S, 400, 1500, 400, 700, 400); await L.sleep(2000); shot('scroll-after'); }
else if (step === 'pinch') { shot('pinch-before'); await pinch([270, 700], 120, 380); await L.sleep(2500); shot('pinch-out'); await pinch([270, 700], 380, 120); await L.sleep(2500); shot('pinch-in'); }
else if (step === 'select') { L.tap(S, 560, 842); await L.sleep(200); await L.hold('HRM_Pixel_8', [560, 842], { holdMs: 1400, during: async () => shot('select-hold') }); await L.sleep(2000); shot('select-after'); }
else if (step === 'type') { L.tap(S, 420, 606); await L.sleep(3000); shot('type-focus'); for (const ch of 'papka') { L.tap(S, ...L.EN_LAYOUT[ch]); await L.sleep(450); } await L.sleep(2500); shot('type-after'); }
