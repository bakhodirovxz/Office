// Phase 7 re-check: the Plugins page "Install from file" on the real desktop host (hoshiya.exe, a debug build: the SUITE_* hooks), in the real WebView2
// window. The page's button asks the host (IPC `plugins_install`), the host opens the native Open dialog (rfd), and this script drives that dialog with
// UI Automation and Win32 messages (scripts/plugins/file-dialog.ps1). Two branches: Cancel (the page says nothing is wrong, nothing installed) and Open with a package.
//   node scripts/plugins/desktop-install-check.mjs <hoshiya.exe> <dist dir> <plugin.zip> <evidence-dir>      (needs ci/web-smoke/node_modules; an unlocked interactive desktop)
// Writes <evidence-dir>/desktop-install.json (no screenshots: a capture of a locked desktop shows the lock screen, and the dialog is proved by UI Automation instead).
// Starts one hoshiya.exe of its own and kills only that one.
import { createRequire } from 'node:module';
import { spawn, spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import * as net from 'node:net';

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '../..');
const { chromium } = createRequire(path.join(ROOT, 'ci/web-smoke/package.json'))('playwright');
const [EXE, DIST, ZIP, EV] = process.argv.slice(2);
const free = () => new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const out = { run: new Date().toISOString(), exe: EXE, steps: [] };
const step = (name, ok, details = {}) => { out.steps.push({ name, ok, ...details }); console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}  ${JSON.stringify(details).slice(0, 300)}`); };
fs.mkdirSync(EV, { recursive: true });

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'plg-desktop-'));
const data = path.join(dir, 'data'), app = path.join(dir, 'app');
fs.mkdirSync(app);
const port = await free();
fs.copyFileSync(EXE, path.join(app, 'hoshiya.exe'));
const find = (d) => fs.existsSync(d) ? fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? find(path.join(d, e.name)) : e.name === 'WebView2Loader.dll' && path.basename(d) === 'x64' ? [path.join(d, e.name)] : [])) : [];
const loader = fs.existsSync(path.join(path.dirname(EXE), 'WebView2Loader.dll')) ? path.join(path.dirname(EXE), 'WebView2Loader.dll') : find(path.join(path.dirname(EXE), 'build'))[0];
if (loader) fs.copyFileSync(loader, path.join(app, 'WebView2Loader.dll'));
// the machine policy must allow a user install: this run has none (no trusted machine dir entry), so the default applies
const host = spawn(path.join(app, 'hoshiya.exe'), ['--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml'), '--bin-dir', dir], {
  env: { ...process.env, SUITE_DATA_DIR: data, SUITE_INSTANCE: 'plgchk' + process.pid, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=*` },
  stdio: ['ignore', 'ignore', 'inherit'],
});
const dialog = (args) => { const r = spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(ROOT, 'scripts/plugins/file-dialog.ps1'), '-ProcessId', String(host.pid), ...args], { encoding: 'utf8' }); return { status: r.status, out: (r.stdout || '').trim(), err: (r.stderr || "").trim().slice(0, 4000) }; };
let browser;
try {
  for (let i = 0; i < 120 && !browser; i++) { try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); } catch { await sleep(500); } }
  if (!browser) throw new Error('no WebView2 debugging port');
  let page;
  for (let i = 0; i < 60 && !page; i++) { page = browser.contexts().flatMap((c) => c.pages()).find((p) => /^http:\/\/127\.0\.0\.1:\d+\//.test(p.url())); if (!page) await sleep(500); }
  if (!page) throw new Error('the shell page was not found');
  await page.waitForSelector('#cards li');
  // the window itself, through UI Automation: a top-level window of our process with a WebView2 (Chrome_WidgetWin) child
  const win = spawnSync('powershell', ['-NoProfile', '-Command', `Add-Type -AssemblyName UIAutomationClient; $c=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,${host.pid}); $ws=[System.Windows.Automation.AutomationElement]::RootElement.FindAll('Children',$c); ($ws | % { $_.Current.ClassName + '|' + $_.Current.Name + '|' + [int]$_.Current.BoundingRectangle.Width + 'x' + [int]$_.Current.BoundingRectangle.Height }) -join ';'`], { encoding: 'utf8' }).stdout.trim();
  step('the host shows its WebView2 window (UI Automation)', /Chrome_WidgetWin|WinitWindow|Window/i.test(win) && win.length > 0, { windows: win });
  step('the shell runs in the host (window.ipc present)', await page.evaluate(() => !!window.ipc));
  const open = async () => { for (let i = 0; i < 20 && !(await page.evaluate(() => !!document.querySelector('#plg-main li, #plg-install'))); i++) { await page.evaluate(() => document.getElementById('nav-plugins')?.click()); await sleep(1000); } };
  await open();
  const rows = () => page.evaluate(() => [...document.querySelectorAll('#plg-main li')].map((li) => li.querySelector('h3')?.textContent));
  const before = await rows();
  step('the Plugins page is up and Install from file is enabled', await page.evaluate(() => { const b = document.getElementById('plg-install'); return !!b && !b.disabled; }), { rows: before });

  await page.evaluate(() => { window.__r = []; addEventListener('suite-host', (e) => window.__r.push({ t: Date.now(), type: e.detail.type, ok: e.detail.ok, error: e.detail.error, cancelled: e.detail.cancelled, installed: e.detail.installed })); });
  // branch 1: the dialog opens and is cancelled
  await page.evaluate(() => document.getElementById('plg-install').click());
  await sleep(1500);
  const c = dialog(['-Cancel']);
  await sleep(2000);
  const msg1 = await page.evaluate(() => document.getElementById('plg-msg')?.textContent ?? '');
  step('Cancel in the native dialog: nothing installed, no error', c.status === 0 && JSON.stringify(await rows()) === JSON.stringify(before), { dialog: c.out, err: c.err, message: msg1 });

  // branch 2: Open with a package
  await page.evaluate(() => document.getElementById('plg-install').click());
  const o = dialog(['-Path', ZIP.replace(/\//g, '\\')]);
  for (let i = 0; i < 20 && (await rows()).length === before.length; i++) await sleep(1000);
  await sleep(1000);
  const after = await rows(), msg2 = await page.evaluate(() => document.getElementById('plg-msg')?.textContent ?? '');
  const id = path.basename(ZIP).replace(/-[^-]+\.zip$/, '');
  const recs = fs.existsSync(data) ? [...fs.readdirSync(data, { recursive: true })].filter((f) => String(f).endsWith('install.toml')).map(String) : [];
  const rec = recs.find((r) => r.includes(id));
  step('Open in the native dialog: the package is installed', o.status === 0 && after.length === before.length + 1 && !!rec, { dialog: o.out, err: o.err, rows_after: after, message: msg2, record: rec, data_files: fs.existsSync(data) ? [...fs.readdirSync(data, { recursive: true })].map(String).filter((f) => /plugin/i.test(f)).slice(0, 20) : [], dialogs_left: dialog(['-Cancel', '-Wait', '2']).out });
  if (rec) out.record = fs.readFileSync(path.join(data, rec), 'utf8');
  out.replies = await page.evaluate(() => window.__r);   // every reply the page got from the host: a Cancel is one `cancelled`, an Open one `installed`
} catch (e) { step('run', false, { error: String(e) }); }
finally {
  out.pass = out.steps.length > 0 && out.steps.every((s) => s.ok);
  fs.writeFileSync(path.join(EV, 'desktop-install.json'), JSON.stringify(out, null, 2));
  try { await browser?.close(); } catch { /* gone */ }
  host.kill();
  await sleep(1000);
  try { fs.rmSync(dir, { recursive: true, force: true }); } catch { /* still in use */ }
  process.exit(out.pass ? 0 : 1);
}
