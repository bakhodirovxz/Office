// Done-check checklist for WP1: the home shell, then Docs (open, type, save), on a phone-sized emulator. Synthetic fixture only.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

export const app = 'docs';
const DOCX = 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx';

export const items = [
  { id: 'shell-home', run: async (c) => {
    await c.page.addInitScript(() => { window.__msgs = []; window.__blobs = []; const cu = URL.createObjectURL.bind(URL); URL.createObjectURL = (b) => { if (b instanceof Blob) window.__blobs.push(b); return cu(b); }; addEventListener('message', (e) => window.__msgs.push(e.data?.type)); });
    await c.page.goto(c.base + '/', { waitUntil: 'load' });
    await c.activate(); // the tab under test must be the foreground one
    await c.page.waitForSelector('#cards li');
    c.notes.cards = await c.page.locator('#cards li').count();
    c.notes.viewport = await c.page.evaluate(() => ({ w: innerWidth, h: innerHeight, dpr: devicePixelRatio }));
    c.shot('shell-home');
  } },
  { id: 'open', run: async (c) => {
    await c.page.setInputFiles('#file', path.join(c.root, DOCX));
    await c.page.waitForFunction(() => window.__msgs.includes('wordcraft:opened'), undefined, { timeout: 180_000 });
    await c.page.waitForTimeout(1500);
    c.shot('docs-opened');
  } },
  // Taps are device pixels on HRM_Pixel_8 (1080x2400 @ 420 dpi): the editor is a canvas, so it has no accessibility nodes to find by text (plan section 8).
  { id: 'type', run: async (c) => {
    c.L.tap(c.ser, 696, 1278); // the heading's text: puts the caret in the document and raises Gboard
    await c.L.sleep(1500);
    for (const ch of 'hello') { c.L.tap(c.ser, ...c.L.EN_LAYOUT[ch]); await c.L.sleep(250); }
    c.L.tap(c.ser, 592, 2179); await c.L.sleep(300); // space
    c.notes.glide = await c.glide('world');
    await c.L.sleep(1500);
    c.shot('docs-typed');
  } },
  { id: 'save', run: async (c) => {
    c.L.tap(c.ser, 60, 580); await c.L.sleep(1500);   // File
    c.L.tap(c.ser, 180, 1204);                          // Save
    await c.page.waitForFunction(() => /saved/i.test(document.querySelector('#ed-status')?.textContent ?? ''), undefined, { timeout: 60_000 });
    c.notes.status = await c.page.locator('#ed-status').textContent();
    await c.L.sleep(2000);
    c.shot('docs-saved');
    // Chrome's own download to /sdcard fails on this image ("1 download failed"), so the saved bytes are read from the blob the shell hands to the download
    const b64 = await c.page.evaluate(async () => { const b = window.__blobs.at(-1); if (!b) return null; const u = new Uint8Array(await b.arrayBuffer()); let s = ''; for (let i = 0; i < u.length; i += 8192) s += String.fromCharCode(...u.subarray(i, i + 8192)); return btoa(s); });
    if (!b64) throw new Error('the shell produced no blob on Save');
    const dest = path.join(c.L.P6, 'saved-docs.docx');
    fs.writeFileSync(dest, Buffer.from(b64, 'base64'));
    c.notes.saved_bytes = fs.statSync(dest).size;
    const py = spawnSync('python', ['-I', '-c', "import sys,zipfile,re;x=zipfile.ZipFile(sys.argv[1]).read('word/document.xml').decode();sys.stdout.buffer.write(''.join(re.findall(r'<w:t[^>]*>([^<]*)</w:t>',x)).encode())", dest], { encoding: 'utf8', env: { ...process.env, PYTHONIOENCODING: 'utf-8' } });
    c.notes.saved_text_has_hello = /hello/.test(py.stdout);
    c.notes.saved_text_has_world = /world/.test(py.stdout);
    if (!c.notes.saved_text_has_hello) throw new Error('saved docx lacks the typed "hello"');
  } },
];
