// WP13 touch driver for a running emulator (device pixels of the current `wm size`, default 540x1200 = the rig's half scale).
//   node touch.mjs drag x1 y1 x2 y2 [holdMs]      press, wait holdMs (default 250), slide in 12 steps, release (console `event send`, one finger)
//   node touch.mjs press x y holdMs               a long press (finger stays down holdMs)
//   node touch.mjs pinch cx cy r1 r2 [vertical]   two fingers move apart (r2 > r1) or together, centred at (cx, cy)
import * as L from './lib.mjs';
const [cmd, ...a] = process.argv.slice(2), n = a.map(Number);
const AVD = process.env.RIG_AVD ?? 'HRM_Pixel_8', [W, H] = L.AVDS[AVD].size.split('x').map(Number);
const SC = Number(process.env.RIG_SCALE ?? 0.5);               // device px -> panel px
const raw = (x, y) => [Math.round((x / SC) / W * 32767), Math.round((y / SC) / H * 32767)];
const c = await L.consoleOpen(L.AVDS[AVD].port), ev = (t, k, v) => c.cmd(`event send ${t}:${k}:${v}`);
const down = async (slot, id, x, y) => { const [rx, ry] = raw(x, y); await ev(3, 47, slot); await ev(3, 57, id); await ev(3, 53, rx); await ev(3, 54, ry); await ev(3, 58, 100); if (slot === 0) await ev(1, 330, 1); await ev(0, 0, 0); };
const move = async (slot, x, y) => { const [rx, ry] = raw(x, y); await ev(3, 47, slot); await ev(3, 53, rx); await ev(3, 54, ry); await ev(0, 0, 0); };
const up = async (slot) => { await ev(3, 47, slot); await ev(3, 57, -1); if (slot === 0) await ev(1, 330, 0); await ev(0, 0, 0); };
if (cmd === 'drag' || cmd === 'press') {
  const [x1, y1, x2, y2, hold] = cmd === 'drag' ? [...n, n[4] ?? 250] : [n[0], n[1], n[0], n[1], n[2]];
  await down(0, 1111, x1, y1); await L.sleep(hold ?? 250);
  if (cmd === 'drag') for (let k = 1; k <= 12; k++) { await move(0, x1 + (x2 - x1) * k / 12, y1 + (y2 - y1) * k / 12); await L.sleep(Number(process.env.RIG_STEP ?? 80)); }
  await L.sleep(Number(process.env.RIG_END ?? 200)); await up(0);
} else if (cmd === 'pinch') {
  const [cx, cy, r1, r2, vert] = n, p = (r, s) => (vert ? [cx, cy + s * r] : [cx + s * r, cy]);
  await down(0, 2001, ...p(r1, -1)); await L.sleep(Number(process.env.RIG_GAP ?? 0)); await down(1, 2002, ...p(r1, 1)); await L.sleep(200);
  for (let k = 1; k <= 14; k++) { const r = r1 + (r2 - r1) * k / 14; await move(0, ...p(r, -1)); await move(1, ...p(r, 1)); await L.sleep(70); }
  await L.sleep(200); await up(1); await up(0);
} else throw new Error('drag|press|pinch');
c.close();
