// WP6 checklist: the Docs phone layout on a phone-sized emulator (top row, contextual bottom toolbar, sheets), one recording per item.
//   node scripts/mobile/rig.mjs run phone-docs --avd HRM_360 --dist <dist>      (RIG_SCALE=0.5 with `wm size 540x1200`, see docs/phase6/keyboard.md)
// Taps on the layout go through Chrome's DevTools touch input (the editor is a canvas: no DOM to find by text), so the editor sees real touch events;
// the soft keyboard is driven by adb taps on Gboard keys as in the keyboard checklists. Synthetic fixture only (a generated empty document).
import path from 'node:path';
export const app = 'phone';
const DOCX = 'scripts/mobile/fixtures/blank.docx';

// A finger through Chrome's DevTools: touch down, 60 ms, touch up, at editor-frame CSS px (negative y counts from the bottom). A sheet of n rows has row k
// centred at (frame height - 48 - (48 n + 8) + 48 k + 28), the bar is the last 48 px, the top row the first 48.
const frameTap = async (c, x, y, wait = 1300) => {
  const b = await c.page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y, h: r.height, w: r.width }; });
  c.cdp ??= await c.bctx.newCDPSession(c.page);
  const pt = { x: b.x + (x < 0 ? b.w + x : x), y: b.y + (y < 0 ? b.h + y : y), id: 1 };
  await c.cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pt] });
  await c.L.sleep(60);
  await c.cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await c.L.sleep(wait);
  return b;
};
// the sheet is at most 60 % of the (keyboard-shrunk) frame: re-read the frame at every tap
const sheetRow = (b, n, k) => b.h - 48 - Math.min(48 * n + 8, 0.6 * b.h) + 48 * k + 28;
const sheetTap = async (c, x, n, k) => frameTap(c, x, sheetRow(await frameBox(c), n, k));
const slotX = (b, i, n = 5) => (b.w / n) * (i + 0.5);
const frameBox = (c) => c.page.evaluate(() => { const r = document.querySelector('#frames iframe:not([hidden])').getBoundingClientRect(); return { x: r.x, y: r.y, h: r.height, w: r.width }; });
const view = (c) => c.page.evaluate(() => ({ w: innerWidth, h: innerHeight, dpr: devicePixelRatio, vv: visualViewport.height }));

export const items = [
  { id: 'open', run: async (c) => {
    await c.page.addInitScript(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
    await c.page.goto(c.base + '/', { waitUntil: 'load' });
    await c.activate();
    await c.page.waitForSelector('#cards li');
    await c.page.setInputFiles('#file', path.join(c.root, DOCX));
    await c.page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180_000 });
    await c.L.sleep(3000);
    c.notes.viewport = await view(c);
    c.notes.frame = await frameBox(c);
    c.shot('chrome-start');
  } },
  { id: 'type-and-format', run: async (c) => {
    const b = await frameBox(c);
    await frameTap(c, b.w / 2, 200);                     // a finger in the page: the caret, and the keyboard
    await c.L.sleep(1500);
    for (const ch of 'salom') { c.L.tap(c.ser, ...c.L.EN_LAYOUT[ch]); await c.L.sleep(250); }
    await c.L.sleep(800);
    c.shot('typed');
    await frameTap(c, slotX(b, 0), -24);                 // Format
    c.shot('sheet-format');
    await sheetTap(c, b.w * 0.125, 6, 0);                // Bold, the first of four in row 0
    c.shot('bold-on');
    await frameTap(c, slotX(b, 0), -24);                 // close
  } },
  { id: 'list-insert', run: async (c) => {
    const b = await frameBox(c);
    await frameTap(c, b.w / 2, 100);                     // a press outside the sheet closes it (the state of the last item is not assumed)
    await frameTap(c, slotX(b, 1), -24); c.shot('sheet-list'); await frameTap(c, slotX(b, 1), -24);
    await frameTap(c, slotX(b, 2), -24); c.shot('sheet-insert');
    await sheetTap(c, b.w / 2, 3, 0);                    // 3 x 3 (the middle of the first row)
    await c.L.sleep(1000);
    c.shot('table-set');                                 // the toolbar now offers Row, Column, Delete, Format, Table
  } },
  { id: 'table-sheet', run: async (c) => {
    const b = await frameBox(c);
    await frameTap(c, 60, 82);                           // a finger in the first cell: the table set
    c.shot('table-caret');
    await frameTap(c, slotX(b, 4), -24); c.shot('sheet-table');
    await frameTap(c, slotX(b, 4), -24);
    await frameTap(c, slotX(b, 2), -24); c.shot('sheet-delete');
    await frameTap(c, slotX(b, 2), -24);
  } },
  { id: 'menu', run: async (c) => {
    await frameTap(c, -24, 24);                          // the top row's More
    c.shot('menu');
    await frameTap(c, -24, 24);
  } },
];
