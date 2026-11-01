// Phase 6 emulator rig CLI. Usage: node scripts/mobile/rig.mjs <command> [args]
//   create360                      write the 360 dp AVD (HRM_360) under E:/office-target/p6/avd
//   start <avd> | stop <avd>       boot (swiftshader_indirect, -no-snapshot-save) / `emu kill` that one emulator
//   versions <avd>                 Chrome, Android System WebView, Gboard versions, display
//   gboard <avd> [--enable]        Gboard check (installed, enabled subtypes vs English / Russian / Uzbek lotin); --enable sets the IME subtypes
//   forward <avd>                  adb forward 9222 -> Chrome DevTools (CDP)
//   shot <avd> [file]              screencap PNG
//   glide <avd> <word>             glide-type a word on the on-screen keyboard (EN layout, 1080x2400 @ 420 dpi calibration)
//   run <checklist> --avd <avd> [--dist <dist>] [--out <dir>] [--cdp-port 9222]   run scripts/mobile/checklists/<checklist>.mjs: JSON + one recording per item
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [cmd, a1, a2] = process.argv.slice(2);
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes(n) ? args[args.indexOf(n) + 1] : d);
const out = (o) => console.log(JSON.stringify(o, null, 2));

if (cmd === 'create360') console.log(L.create360());
else if (cmd === 'start') console.log(await L.startAvd(a1));
else if (cmd === 'stop') L.stopAvd(a1);
else if (cmd === 'versions') out(L.versions(L.serial(a1)));
else if (cmd === 'gboard') out(args.includes('--enable') ? L.gboardEnable(L.serial(a1)) : L.gboardState(L.serial(a1)));
else if (cmd === 'forward') console.log(L.forwardCdp(L.serial(a1)));
else if (cmd === 'shot') fs.writeFileSync(a2 ?? 'shot.png', L.screencap(L.serial(a1)));
else if (cmd === 'glide') out(await L.glide(a1, a2));
else if (cmd === 'run') await run(a1);
else { console.error(fs.readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(0, 10).join('\n')); process.exit(2); }

async function run(listName) {
  const name = opt('--avd', 'HRM_Pixel_8'), ser = L.serial(name), dp = L.AVDS[name].dp;
  const dist = path.resolve(opt('--dist', 'E:/office-target/p5/dist'));
  const runId = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  const jsonDir = path.resolve(opt('--out', path.join(ROOT, 'docs/phase6/evidence/rig')));
  const list = await import(pathToFileURL(path.join(ROOT, 'scripts/mobile/checklists', `${listName}.mjs`)).href);
  // Playwright comes from ci/web-smoke/node_modules (this worktree's, else PLAYWRIGHT_DIR, else the phase-5 worktree's)
  const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
  const { chromium } = createRequire(pwDir + '/')('@playwright/test');

  await L.startAvd(name);
  L.gboardEnable(ser); await L.sleep(3000); L.gboardEnable(ser); // the first call can land before Gboard registers its subtypes on a fresh AVD
  const serve = await L.startServe({ root: ROOT, dist });
  L.reverse(ser, serve.port);
  const cdpPort = Number(opt('--cdp-port', 9222)); // another agent's emulator may hold 9222
  const cdpUrl = L.forwardCdp(ser, cdpPort);
  const shots = [];
  const result = { run: runId, avd: name, dp, dist, versions: L.versions(ser), gboard: L.gboardState(ser), items: [] };
  let browser;
  try {
    L.openInChrome(ser, 'about:blank'); await L.sleep(3000);
    L.chromeFirstRun(ser); await L.sleep(2000);
    L.forwardCdp(ser, cdpPort);
    // tabs left by earlier runs are closed through the DevTools HTTP endpoint (Playwright's page.close hangs on Android Chrome)
    const tabs = (await (await fetch(`${cdpUrl}/json/list`)).json()).filter((t) => t.type === 'page');
    for (const t of tabs.slice(1)) await fetch(`${cdpUrl}/json/close/${t.id}`).catch(() => {});
    browser = await chromium.connectOverCDP(cdpUrl);
    const bctx = browser.contexts()[0];
    const page = bctx.pages()[0];
    const ctx = {
      name, ser, dp, root: ROOT, base: serve.base, page, bctx, L,
      notes: {},
      shot: (tag) => { const f = path.join(jsonDir, `${runId}-${tag}.png`); fs.mkdirSync(jsonDir, { recursive: true }); fs.writeFileSync(f, L.screencap(ser)); shots.push(f); return f; },
      activate: () => L.activateTab(cdpUrl, serve.base),
      glide: (w, o) => L.glide(name, w, o),
    };
    for (const item of list.items) {
      const rec = L.startRecord(ser);
      const t0 = Date.now();
      const r = { id: item.id, app: list.app, status: 'pass', notes: {} };
      ctx.notes = r.notes;
      try { await item.run(ctx); } catch (e) { r.status = 'fail'; r.error = String(e.message ?? e).slice(0, 500); try { ctx.shot(`${list.app}-${item.id}-fail`); } catch { /* keep going */ } }
      r.ms = Date.now() - t0;
      r.recording = await L.stopRecord(ser, rec, path.join(L.EVIDENCE_BIG, `${list.app}-${item.id}-${dp}.mp4`));
      result.items.push(r);
      console.log(`${r.status} ${item.id} ${r.ms} ms`);
    }
  } finally {
    result.screenshots = shots;
    fs.mkdirSync(jsonDir, { recursive: true });
    fs.writeFileSync(path.join(jsonDir, `${runId}-${listName}-${dp}.json`), JSON.stringify(result, null, 2));
    // SHA-256 list of the big files stays in the repo (docs/phase6/evidence/SHA256SUMS)
    const sums = path.join(ROOT, 'docs/phase6/evidence/SHA256SUMS');
    fs.appendFileSync(sums, result.items.filter((i) => i.recording).map((i) => `${i.recording.sha256}  ${path.basename(i.recording.file)}  (${runId})\n`).join(''));
    await browser?.close().catch(() => {});
    serve.stop();
  }
  process.exitCode = result.items.every((i) => i.status === 'pass') ? 0 : 1;
}
