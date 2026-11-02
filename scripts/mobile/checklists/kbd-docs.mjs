// WP7 keyboard matrix (PLAN phase 6 section 6): Docs on a phone-sized emulator, typing through Gboard. Synthetic fixture only.
import path from 'node:path';
export const app = 'kbd';
const DOCX = 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx';

export const items = [
  { id: 'open', run: async (c) => {
    await c.page.addInitScript(() => { window.__msgs = []; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
    await c.page.goto(c.base + '/', { waitUntil: 'load' });
    await c.activate();
    await c.page.waitForSelector('#cards li');
    await c.page.setInputFiles('#file', path.join(c.root, DOCX));
    await c.page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180_000 });
    await c.page.waitForTimeout(1500);
    c.shot('opened');
  } },
  { id: 'calibrate', run: async (c) => {
    c.L.tap(c.ser, 400, 700); await c.L.sleep(2000);
    c.shot('kbd-en');
  } },
];
