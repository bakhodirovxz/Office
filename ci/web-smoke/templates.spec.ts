import { test, expect } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// Phase 4 integration: every starter template of web/shell/templates opened from the shell's home screen in its real editor
// (the editor's wasm, not a stand-in). Each must reach `<app>:opened` with no page error and no CSP violation; a screenshot of the
// shell with the editor goes to TEMPLATE_SHOTS for a person to look at. Env as shell.spec.ts, plus TEMPLATE_SHOTS.
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'hoshiya-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const DIST = process.env.SHELL_DIST_ROOT ?? 'E:/office-target/p1/dist';
const SHOTS = process.env.TEMPLATE_SHOTS ?? path.join(ROOT, 'docs/phase4/evidence/templates');
const catalog: { app: string; file: string }[] = JSON.parse(fs.readFileSync(path.join(ROOT, 'web/shell/templates.json'), 'utf8'));

let proc: ChildProcess;
let base: string;
let dir: string;

test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'templates-spec-'));
  const cfg = path.join(dir, 'api.toml');
  const port = await new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
  fs.writeFileSync(cfg, `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').split(path.sep).join('/')}"\nmax_file_mb = 1\n`);
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(BIN, ['--config', cfg, '--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  base = await new Promise<string>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => { out += d; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) ok(m[1]); });
    proc.on('exit', (c) => bad(new Error(`the server exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('the server did not start')), 15000);
  });
  fs.mkdirSync(SHOTS, { recursive: true });
});
test.afterAll(() => { proc?.kill(); fs.rmSync(dir, { recursive: true, force: true }); });

for (const [i, t] of catalog.entries()) {
  test(`template ${path.basename(t.file)} opens in ${t.app}`, async ({ page }) => {
    const problems: string[] = [];
    page.on('pageerror', (e) => problems.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy/i.test(m.text())) problems.push(m.text()); });
    await page.setViewportSize({ width: 1500, height: 900 });
    await page.addInitScript(() => {
      localStorage.setItem('sh.lang', 'uz-Latn');
      (window as any).__msgs = [];
      addEventListener('message', (e) => (window as any).__msgs.push(e.data?.type));
    });
    await page.goto(base + '/');
    await page.locator('#templates li').nth(i).locator('button').click();
    // gridcraft's bridge has no `opened` reply (ready, open, save, dirty, failed only): after `ready` the file is loaded and the sheet drawn
    const done = t.app === 'sheets' ? /:ready$/ : /:opened$/;
    await expect.poll(() => page.evaluate((re) => (window as any).__msgs.filter((m: string) => new RegExp(re).test(m)).length, done.source), { timeout: 180_000 }).toBe(1);
    expect(await page.evaluate(() => (window as any).__msgs.filter((m: string) => /:failed$/.test(m)))).toEqual([]);
    await page.waitForTimeout(t.app === 'sheets' ? 12_000 : 4000); // first paint of the document
    await page.screenshot({ path: path.join(SHOTS, `${path.basename(t.file, path.extname(t.file))}.png`) });
    expect(problems).toEqual([]);
  });
}
