// Phase 6 WP2 step 2 done-check (plan section 7, M3): the 108-page synthetic docx opens from the shell on the emulator, page 1 shows,
// and a tap places the caret (a typed key then makes the document dirty). Synthetic fixture only, never corpus/.
import path from 'node:path';

export const app = 'docs';
const DOCX = 'domains/docs/crates/docs-translit/tests/fixtures/uz108-latn.docx';

export const items = [
  { id: 'shell-home', run: async (c) => {
    await c.page.addInitScript(() => { window.__msgs = []; window.__t = {}; addEventListener('message', (e) => { const t = e.data?.type; window.__msgs.push(t); window.__t[t] ??= performance.now(); }); });
    await c.page.goto(c.base + '/', { waitUntil: 'load' });
    await c.activate(); // the tab under test must be the foreground one
    await c.page.waitForSelector('#cards li');
  } },
  { id: 'open-108', run: async (c) => {
    const t0 = await c.page.evaluate(() => performance.now());
    await c.page.setInputFiles('#file', path.join(c.root, DOCX));
    await c.L.sleep(4000);
    c.notes.cover_while_opening = await c.page.locator('#ed-cover').isVisible() ? await c.page.locator('#ed-cover').textContent() : null;
    c.shot('m3-opening');
    await c.page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 300_000 });
    const t = await c.page.evaluate(() => window.__t);
    c.notes.ready_ms = Math.round(t['wordcraft:ready'] - t0);
    c.notes.opened_ms = Math.round(t['wordcraft:opened'] - t0);
    c.notes.cover_after_opened = await c.page.locator('#ed-cover').isVisible();
    await c.L.sleep(3000);
    c.shot('m3-page1');
  } },
  // Device pixels on HRM_Pixel_8 (1080x2400), read off m3-page1.png; the canvas has no accessibility nodes to find.
  { id: 'tap-caret', run: async (c) => {
    const dirty0 = (await c.page.evaluate(() => window.__msgs)).filter((m) => m === 'wordcraft:dirty').length;
    // an overloaded emulator host can raise Android's own "System UI isn't responding" dialog over the page: answer Wait
    c.notes.anr_dialog_dismissed = c.L.tapText(c.ser, /^Wait$/);
    if (c.notes.anr_dialog_dismissed) await c.L.sleep(1500);
    c.L.tap(c.ser, 700, 1265); // the title line, above where Gboard opens (a slow emulator can deliver the tap after Gboard is up)
    await c.L.sleep(2500);
    c.shot('m3-tapped');
    c.L.tap(c.ser, ...c.L.EN_LAYOUT.x); // Gboard is up only when the tap put the caret in the text
    await c.page.waitForFunction((n) => window.__msgs.filter((m) => m === 'wordcraft:dirty').length > n, dirty0, { timeout: 60_000 });
    c.notes.typed_made_dirty = true;
    await c.L.sleep(1500);
    c.shot('m3-typed');
  } },
];
