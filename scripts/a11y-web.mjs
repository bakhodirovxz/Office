// Phase 4 WP18 web baseline: keyboard walk (Tab order, focus ring, accessible names, Escape) of the shell and the converter page,
// the editor frame's accessibility tree, and the WCAG contrast of the shell and converter colour tokens in light and dark.
//   node scripts/a11y-web.mjs [--json out.json]        (needs the release dists, ci/web-smoke/node_modules)
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(path.resolve('ci/web-smoke') + '/');
const { chromium } = require('@playwright/test');
const ROOT = process.cwd();
const SVC = 'E:/office-target/p1/services/release', DIST = 'E:/office-target/p1/dist';
const args = process.argv.slice(2);

// ---- contrast ---------------------------------------------------------------------------------------------------------------
const lum = (h) => { const c = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)); return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]; };
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((m, n) => n - m); return Math.round(((x + 0.05) / (y + 0.05)) * 100) / 100; };
function tokens(file) {
  const css = fs.readFileSync(file, 'utf8');
  const get = (blk) => Object.fromEntries([...blk.matchAll(/--([a-z-]+):\s*(#[0-9a-fA-F]{3,6})/g)].map((m) => [m[1], m[2].length === 4 ? '#' + [...m[2].slice(1)].map((c) => c + c).join('') : m[2]]));
  const light = get(css.slice(0, css.search(/@media \(prefers-color-scheme: dark\)/)));
  const dark = { ...light, ...get(css.slice(css.search(/@media \(prefers-color-scheme: dark\)/))) };
  return { light, dark };
}
const contrast = [];
for (const [surface, file, pairs] of [
  ['shell', 'web/shell/shell.css', [['fg', 'bg'], ['muted', 'bg'], ['muted', 'card'], ['fg', 'card'], ['on-accent', 'accent'], ['accent', 'bg'], ['accent', 'card'], ['err', 'bg'], ['line', 'bg']]],
  ['converter', 'services/convert-api/ui/app.css', [['fg', 'bg'], ['fg', 'card'], ['muted', 'bg'], ['muted', 'card'], ['on-accent', 'accent'], ['accent', 'card'], ['focus', 'card'], ['focus', 'bg'], ['warn-fg', 'warn-bg'], ['err-fg', 'err-bg'], ['ok-fg', 'card'], ['line', 'card']]],
]) {
  const t = tokens(file);
  for (const mode of ['light', 'dark']) for (const [f, b] of pairs) if (t[mode][f] && t[mode][b]) contrast.push({ surface, mode, pair: `${f} on ${b}`, fg: t[mode][f], bg: t[mode][b], ratio: ratio(t[mode][f], t[mode][b]) });
}
// the brand accent against white and the dark surfaces (brand.toml palette)
console.log('CONTRAST'); for (const c of contrast) console.log(`${c.ratio >= 4.5 ? 'ok  ' : c.ratio >= 3 ? 'ui3 ' : 'LOW '} ${c.surface} ${c.mode} ${c.pair} ${c.fg}/${c.bg} ${c.ratio}`);

// ---- serve ------------------------------------------------------------------------------------------------------------------
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'a11y-'));
const port = await new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });
fs.writeFileSync(path.join(dir, 'api.toml'), `listen = "127.0.0.1:${port}"\nwork_dir = "${path.join(dir, 'work').replace(/\\/g, '/')}"\nmax_file_mb = 1\n`);
const env = { ...process.env }; for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = path.join(SVC, 'convert-api-fake-tool.exe');
const proc = spawn(path.join(SVC, 'hoshiya-serve.exe'), ['--config', path.join(dir, 'api.toml'), '--dist', DIST, '--shell', path.join(DIST, 'shell'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
const base = await new Promise((ok, bad) => { let o = ''; proc.stdout.on('data', (d) => { o += d; const m = /listening on (http:\/\/\S+)/.exec(o); if (m) ok(m[1]); }); setTimeout(() => bad(new Error('serve did not start')), 20000); });

const browser = await chromium.launch({ headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
const walk = async (page, url, max = 60) => {
  await page.goto(base + url, { waitUntil: 'load' });
  await page.waitForTimeout(1500);
  const rows = [];
  for (let i = 0; i < max; i++) {
    await page.keyboard.press('Tab');
    const r = await page.evaluate(() => {
      const e = document.activeElement; if (!e || e === document.body) return null;
      const cs = getComputedStyle(e), r = e.getBoundingClientRect();
      const lab = e.getAttribute('aria-label') || (e.labels && e.labels[0] && e.labels[0].textContent.trim()) || (e.getAttribute('aria-labelledby') && document.getElementById(e.getAttribute('aria-labelledby'))?.textContent.trim()) || e.textContent.trim() || e.getAttribute('title') || e.getAttribute('placeholder') || '';
      const ring = cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) >= 2;
      return { id: e.id || '', tag: e.tagName.toLowerCase() + (e.type ? `[${e.type}]` : ''), name: lab.replace(/\s+/g, ' ').slice(0, 60), ring, ringPx: cs.outlineWidth + ' ' + cs.outlineStyle, w: Math.round(r.width), h: Math.round(r.height), visible: r.width > 0 && r.height > 0 };
    });
    if (!r) { rows.push({ id: '', tag: '(none: focus left the page or sits on body)', name: '', ring: true, w: 99, h: 99 }); if (rows.filter((x) => x.tag.startsWith('(none')).length > 2) break; continue; }
    if (rows.length && rows[0].id && r.id === rows[0].id && r.tag === rows[0].tag && i > 1) break;
    rows.push(r);
  }
  return rows;
};
const out = { contrast, walks: {} };
for (const scheme of ['light', 'dark']) {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, colorScheme: scheme });
  const page = await ctx.newPage();
  out.walks[`shell-${scheme}`] = await walk(page, '/');
  out.walks[`converter-${scheme}`] = await walk(page, '/convert/');
  if (scheme === 'light') {
    // a skip link or landmark check, and the language of the page
    out.landmarks = await page.evaluate(() => ({ lang: document.documentElement.lang, main: !!document.querySelector('main'), h1: !!document.querySelector('h1'), title: document.title }));
    // shell: Open Docs, then Tab from the Back button into the editor, Escape inside the editor
    await page.goto(base + '/');
    await page.locator('#cards li').first().getByRole('button').click();
    await page.waitForFunction(() => !!document.querySelector('iframe[src^="/docs/"]'));
    const frame = page.frames().find((f) => f.url().includes('/docs/'));
    await frame.waitForFunction(() => !!window.wasmBindings && !document.querySelector('[id$="_loading"]'), undefined, { timeout: 120000 });
    await page.waitForTimeout(1500);
    out.editorFrame = await frame.evaluate(() => ({ ariaNodes: document.querySelectorAll('[role],[aria-label]').length, bodyChildren: [...document.body.children].map((c) => c.tagName.toLowerCase() + (c.id ? '#' + c.id : '')), canvas: document.querySelectorAll('canvas').length, tabbable: [...document.querySelectorAll('canvas,[tabindex]')].map((c) => c.tagName.toLowerCase() + ':' + c.getAttribute('tabindex')) }));
    out.editorAria = await frame.locator('body').ariaSnapshot().catch((e) => 'error ' + e.message.slice(0, 80));
    const seq = [];
    await page.locator('#back').focus();
    for (let i = 0; i < 30; i++) { await page.keyboard.press('Tab'); seq.push(await page.evaluate(() => { const e = document.activeElement; return e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') + (e.tagName === 'IFRAME' ? ':' + e.getAttribute('src') : ''); })); }
    out.tabIntoEditor = [...new Set(seq)];
    await page.keyboard.press('Escape');
    out.afterEscape = await page.evaluate(() => ({ editorVisible: !document.getElementById('editor').hidden, active: document.activeElement.tagName.toLowerCase() + (document.activeElement.id ? '#' + document.activeElement.id : '') }));
    // Shift+Tab out of the editor back to the shell
    await page.keyboard.press('Shift+Tab');
    out.afterShiftTab = await page.evaluate(() => { const e = document.activeElement; return e.tagName.toLowerCase() + (e.id ? '#' + e.id : ''); });
  }
  await ctx.close();
}
await browser.close(); proc.kill(); fs.rmSync(dir, { recursive: true, force: true });
for (const [k, rows] of Object.entries(out.walks)) {
  console.log(`\nWALK ${k}: ${rows.length} stops, no ring: ${rows.filter((r) => !r.ring).length}, no name: ${rows.filter((r) => !r.name).length}, small (<24px): ${rows.filter((r) => r.w < 24 || r.h < 24).length}`);
  if (k.endsWith('light')) for (const r of rows) console.log(`  ${r.tag}#${r.id} "${r.name}" ring=${r.ring} ${r.w}x${r.h}`);
}
console.log(JSON.stringify({ landmarks: out.landmarks, editorFrame: out.editorFrame, editorAria: out.editorAria, tabIntoEditor: out.tabIntoEditor, afterEscape: out.afterEscape, afterShiftTab: out.afterShiftTab }, null, 1));
if (args.includes('--json')) fs.writeFileSync(args[args.indexOf('--json') + 1], JSON.stringify(out, null, 1));
