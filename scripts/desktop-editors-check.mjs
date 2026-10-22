// Review m6 follow-up: with WebView2's browser accelerator keys and context menu off, do the web editors inside suite.exe still
// print, save and zoom on their own? Drives the real suite.exe (debug build: the SUITE_* hooks) over the WebView2 remote
// debugging port, for Docs, Sheets and Slides:
//   - Ctrl+P: the editor's own print UI opens (the canvas changes) and no new browser window appears;
//   - Ctrl+S: the editor hands its bytes to the shell, the host's download handler writes the file, history.json records it;
//   - zoom: Ctrl+Plus, Ctrl+Minus and Ctrl+wheel leave devicePixelRatio alone (the editor zooms its own canvas).
// Usage: node scripts/desktop-editors-check.mjs [suite.exe] [dist dir]      (needs ci/web-smoke/node_modules)
// It writes one file per editor into the downloads folder (the host's default) and deletes exactly the files history.json lists.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import * as net from 'node:net';

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '..');
const { chromium } = createRequire(path.join(ROOT, 'ci/web-smoke/package.json'))('playwright');
const EXE = process.argv[2] ?? 'E:/office-target/p1/services/debug/suite.exe';
const DIST = process.argv[3] ?? 'E:/office-target/p1/dist';
const free = () => new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (name, ok, extra = '') => { results.push({ name, ok }); console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? '  ' + extra : ''}`); };

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'suite-editors-'));
const data = path.join(dir, 'data');
const port = await free();
// the windows-gnu build imports WebView2Loader.dll: it must sit next to the exe (the msvc build links it statically)
const app = path.join(dir, 'app');
fs.mkdirSync(app);
fs.copyFileSync(EXE, path.join(app, 'suite.exe'));
const find = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? find(path.join(d, e.name)) : e.name === 'WebView2Loader.dll' && path.basename(d) === 'x64' ? [path.join(d, e.name)] : []));
const loader = fs.existsSync(path.join(path.dirname(EXE), 'WebView2Loader.dll')) ? path.join(path.dirname(EXE), 'WebView2Loader.dll') : find(path.join(path.dirname(EXE), 'build'))[0];
if (loader) fs.copyFileSync(loader, path.join(app, 'WebView2Loader.dll'));
const host = spawn(path.join(app, 'suite.exe'), ['--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml'), '--bin-dir', dir], {
  env: { ...process.env, SUITE_DATA_DIR: data, SUITE_INSTANCE: 'edchk' + process.pid, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=*` },
  stdio: ['ignore', 'ignore', 'inherit'],
});
let browser;
try {
  for (let i = 0; i < 120 && !browser; i++) {
    try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); } catch { await sleep(500); }
  }
  if (!browser) throw new Error('no WebView2 debugging port');
  let page;
  for (let i = 0; i < 60 && !page; i++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => /^http:\/\/127\.0\.0\.1:\d+\//.test(p.url()));
    if (!page) await sleep(500);
  }
  if (!page) throw new Error('the shell page was not found');
  await page.waitForSelector('#cards li');
  check('the shell runs in the host (window.ipc present)', await page.evaluate(() => !!window.ipc));

  const seen = new Set(); // history paths before each save
  for (const [i, app, route] of [[0, 'docs', '/docs/'], [1, 'sheets', '/sheets/'], [2, 'slides', '/slides/']]) {
    if (!fs.existsSync(path.join(DIST, `${app}-web`, 'index.html'))) { console.log(`SKIP  ${app}: dist not built`); continue; }
    if (await page.locator('#editor').isVisible()) await page.click('#back');
    await page.locator('#cards li').nth(i).getByRole('button').click();
    await page.waitForFunction((r) => !!document.querySelector(`iframe[src^="${r}"]`), route);
    const frame = page.frames().find((f) => f.url().includes(route));
    await frame.waitForFunction(() => !!window.wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 120000 });
    await sleep(1500);
    const canvas = frame.locator('canvas').first();
    const box = await canvas.boundingBox();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    const shot = () => canvas.screenshot().then((b) => b.toString('base64'));
    const pagesBefore = browser.contexts().flatMap((c) => c.pages()).length;

    // print: the editor's own UI answers Ctrl+P
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    const before = await shot();
    await page.keyboard.press('Control+p');
    await sleep(2500);
    const after = await shot();
    fs.writeFileSync(path.join(dir, `${app}-after-ctrl-p.png`), Buffer.from(after, 'base64'));
    check(`${app}: Ctrl+P opens the editor's own print UI (canvas changed)`, before !== after, `screenshot: ${path.join(dir, app + '-after-ctrl-p.png')}`);
    check(`${app}: Ctrl+P opened no browser window`, browser.contexts().flatMap((c) => c.pages()).length === pagesBefore);
    await page.keyboard.press('Escape'); await sleep(500);

    // save: Ctrl+S gives the bytes to the shell, the host writes the file and records it
    const hist = () => { try { return JSON.parse(fs.readFileSync(path.join(data, 'history.json'), 'utf8')).map((e) => e.path); } catch { return []; } };
    const known = new Set(hist());
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await page.keyboard.press('Escape'); await sleep(500); // leave any cell edit or dialog
    await page.keyboard.press('Control+s');
    let saved;
    for (let t = 0; t < 30 && !saved; t++) { saved = hist().find((p) => !known.has(p)); if (!saved) await sleep(500); }
    fs.writeFileSync(path.join(dir, `${app}-after-ctrl-s.png`), Buffer.from(await shot(), 'base64'));
    if (!saved && process.env.TRY_SAVE_ICON) {
      await page.mouse.click(box.x + 47, box.y + 17); // the editor's own Save icon, top left
      for (let t = 0; t < 20 && !saved; t++) { saved = hist().find((p) => !known.has(p)); if (!saved) await sleep(500); }
      console.log(`INFO  ${app}: the Save icon ${saved ? 'saved ' + saved : 'saved nothing either'}`);
      saved = undefined;
    }
    check(`${app}: Ctrl+S saved a file through the host's download handler`, !!saved && fs.existsSync(saved), saved ?? '(no file)');
    await sleep(3000);
    const all = hist().filter((p) => !known.has(p));
    check(`${app}: one Ctrl+S wrote exactly one file`, all.length <= 1, all.join(' | '));
    for (const p of all) seen.add(p);
    // zoom, last: these keys are the editors' own shortcuts too (Sheets: Ctrl+Minus is Delete cells), so a dialog may open
    const dpr = () => page.evaluate(() => window.devicePixelRatio);
    const d0 = await dpr();
    for (const k of ['Control+Equal', 'Control+Equal', 'Control+Minus', 'Control+0']) { await page.keyboard.press(k); await sleep(200); }
    await page.keyboard.down('Control'); await page.mouse.wheel(0, -400); await page.keyboard.up('Control'); await sleep(300);
    check(`${app}: browser zoom stays put (Ctrl+Plus/Minus/0/wheel)`, (await dpr()) === d0, `dpr ${d0} -> ${await dpr()}`);

    await page.keyboard.press('Escape'); await sleep(300);
    await page.keyboard.press('Escape');
  }

} catch (e) {
  check('the run completed', false, String(e));
} finally {
  await browser?.close().catch(() => {});
  host.kill();
  await sleep(1500);
  // only what the host recorded in this run's fresh data dir (a download can still finish while the host shuts down)
  try { for (const e of JSON.parse(fs.readFileSync(path.join(data, 'history.json'), 'utf8'))) { try { if (fs.existsSync(e.path)) fs.unlinkSync(e.path); } catch (x) { console.log('could not delete', e.path, String(x)); } } } catch {}
  console.log(`screenshots and data: ${dir}`);
}
const bad = results.filter((r) => !r.ok).length;
console.log(`${results.length - bad}/${results.length} checks passed`);
process.exit(bad ? 1 : 0);
