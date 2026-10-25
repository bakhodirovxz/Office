import { test, expect, chromium, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// The embed path across two sites (PLAN phase 5, WP5, S13, S14): the fake HR backend's host page on http://localhost:<p1> loads api.js
// from the document server at https://127.0.0.1:<p2> (real TLS, a throwaway CA pinned by public key) and frames its wrapper page,
// which frames the Docs editor. Two different site names, so third-party cookie and storage rules apply as in production.
// Run with: npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.docserver.config.ts
// Env: DS_BIN (hoshiya-serve), FAKE_HRM_BIN, DIST (trunk dists + shell/, default E:/office-target/p1/dist), OPENSSL.
//
// The server fetches version 1 itself from the fake backend's file route (WP4); the fake backend runs with its test-only
// /__test/ routes (FAKE_HRM_TEST_API=1), which show what it received and let a case break its callback verification.
const ROOT = path.resolve(__dirname, '../..');
const TARGET = process.env.OFFICE_TARGET ?? 'E:/office-target/p5/services';
const exe = (n: string) => path.join(TARGET, 'debug', n + (process.platform === 'win32' ? '.exe' : ''));
const DS_BIN = process.env.DS_BIN ?? exe('hoshiya-serve');
const HRM_BIN = process.env.FAKE_HRM_BIN ?? exe('fake-hrm');
const DIST = process.env.DIST ?? 'E:/office-target/p1/dist';
const OPENSSL = process.env.OPENSSL ?? 'openssl';
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');

const free = () => new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
let dir: string, ds: ChildProcess, hrm: ChildProcess, browser: Browser, dsLog = '', stray: http.Server;
let dsPort: number, hrmPort: number, strayPort: number, intPort: number;
let DS: string, HOST: string;

const sessions = () => {
  const d = path.join(dir, 'data/sessions');
  return fs.existsSync(d) ? fs.readdirSync(d).map((s) => path.join(d, s)) : [];
};
const record = (s: string) => JSON.parse(fs.readFileSync(path.join(s, 'session.json'), 'utf8'));

test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ds-spec-'));
  const ssl = (...a: string[]) => execFileSync(OPENSSL, a, { cwd: dir, stdio: 'pipe' });
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'ca.key');
  ssl('req', '-x509', '-new', '-key', 'ca.key', '-sha256', '-days', '2', '-subj', '/CN=test-ca', '-out', 'ca.pem');
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'leaf.key');
  ssl('pkcs8', '-topk8', '-nocrypt', '-in', 'leaf.key', '-out', 'server.key');
  ssl('req', '-new', '-key', 'leaf.key', '-subj', '/CN=localhost', '-out', 'leaf.csr');
  fs.writeFileSync(path.join(dir, 'ext.cnf'), 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n');
  ssl('x509', '-req', '-in', 'leaf.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-days', '2', '-sha256', '-extfile', 'ext.cnf', '-out', 'server.pem');
  const pin = crypto.createHash('sha256').update(new crypto.X509Certificate(fs.readFileSync(path.join(dir, 'server.pem'))).publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
  [dsPort, hrmPort, strayPort, intPort] = [await free(), await free(), await free(), await free()];
  DS = `https://127.0.0.1:${dsPort}`;
  HOST = `http://localhost:${hrmPort}`;

  // the fake backend first: it generates the secret the provider entry of the server must hold
  hrm = spawn(HRM_BIN, [DOCX], { env: { ...process.env, FAKE_HRM_ADDR: `127.0.0.1:${hrmPort}`, FAKE_HRM_DOCSERVER: DS, FAKE_HRM_INTERNAL_BASE: `http://127.0.0.1:${intPort}`, FAKE_HRM_EXP: '1', FAKE_HRM_TEST_API: '1' } });
  const secret = await new Promise<string>((ok, bad) => {
    let err = '';
    hrm.stderr!.on('data', (d) => { err += d; const m = /\(secret ([0-9a-f]+),/.exec(err); if (m) ok(m[1]); });
    hrm.on('exit', (c) => bad(new Error(`fake-hrm exited (${c}): ${err}`)));
    setTimeout(() => bad(new Error('fake-hrm did not start')), 15000);
  });
  fs.writeFileSync(path.join(dir, 'server-secret'), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'provider-secret'), secret);
  fs.writeFileSync(path.join(dir, 'converter-secret'), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'audit-key'), crypto.randomBytes(48).toString('hex'));
  const f = (n: string) => path.join(dir, n).replace(/\\/g, '/');
  fs.writeFileSync(path.join(dir, 'api.toml'), [
    `listen = "127.0.0.1:${dsPort}"`, 'auth = "jwt"', `secret_file = "${f('converter-secret')}"`, `tls_cert = "${f('server.pem')}"`, `tls_key = "${f('server.key')}"`, `public_url = "${DS}"`,
    'trusted_proxies = []', `work_dir = "${f('work')}"`, `store_dir = "${f('store')}"`,
    '[docserver]', `server_secret_file = "${f('server-secret')}"`, `data_dir = "${f('data')}"`, `audit_key_file = "${f('audit-key')}"`, `internal_listen = "127.0.0.1:${intPort}"`, 'internal_allow = ["127.0.0.1"]',
    '[[docserver.providers]]', 'name = "fake"', 'issuer = "fake"', `secret_file = "${f('provider-secret')}"`,
    `fetch_allow = [{ url = "http://127.0.0.1:${hrmPort}", internal = true }]`,
    `callback_allow = [{ url = "http://127.0.0.1:${hrmPort}", internal = true }]`, `frame_origins = ["${HOST}"]`,
  ].join('\n'));
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = 'unused';
  ds = spawn(DS_BIN, ['--config', path.join(dir, 'api.toml'), '--dist', DIST, '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  await new Promise<void>((ok, bad) => {
    let out = '';
    ds.stdout!.on('data', (d) => { out += d; if (/listening on https:/.test(out)) ok(); });
    ds.stderr!.on('data', (d) => { out += d; dsLog += d; });
    ds.on('exit', (c) => bad(new Error(`server exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('server did not start: ' + out)), 20000);
  });

  // a page on a site that is NOT a frame origin of the provider, for the framing refusal (S13)
  stray = http.createServer((_, res) => res.writeHead(200, { 'content-type': 'text/html' }).end(`<!doctype html><iframe id="w" src="${DS}/ds/editor#o=${encodeURIComponent(`http://127.0.0.1:${strayPort}`)}"></iframe>`));
  await new Promise<void>((ok) => stray.listen(strayPort, '127.0.0.1', ok));

  browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${pin}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
});
test.afterAll(async () => {
  await browser?.close();
  stray?.close();
  ds?.kill(); hrm?.kill();
  fs.rmSync(dir, { recursive: true, force: true });
});

test.beforeEach(() => { fs.rmSync(path.join(dir, 'data/sessions'), { recursive: true, force: true }); });

async function open(): Promise<{ ctx: BrowserContext; page: Page; problems: string[] }> {
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const page = await ctx.newPage();
  const problems: string[] = [];
  page.on('pageerror', (e) => problems.push('pageerror: ' + e.message));
  page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy|Refused to/i.test(m.text())) problems.push(m.text()); });
  return { ctx, page, problems };
}
const state = (page: Page) => page.locator('body').getAttribute('data-state');
const hrmState = async () => (await fetch(`http://127.0.0.1:${hrmPort}/__test/state`)).json() as Promise<{ received: { status: number; id: number; sha256: string; bytes: number }[]; rejected: string[]; objects: Record<string, string> }>;
const sha = (f: string) => crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex');
const log = (page: Page) => page.evaluate(() => (window as any).__hrmLog as any[]);

test('cross-site: api.js, session, the document is shown, save, close; no CSP violation', async () => {
  const { ctx, page, problems } = await open();
  const apiJs: { status: number; headers: Record<string, string> }[] = [];
  page.on('response', (r) => { if (r.url().endsWith('/web-apps/apps/api/documents/api.js')) apiJs.push({ status: r.status(), headers: r.headers() }); });
  const created: string[] = [];
  page.on('request', (r) => { if (r.url() === `${DS}/ds/session`) created.push(r.postData() ?? ''); });
  const t0 = Date.now();
  const nav = await page.goto(`${HOST}/host?kind=files&id=1`);
  // what the host page sends: the headers the real HRM page would have to be compatible with (COOP, COEP, CSP)
  const h = nav!.headers();
  console.log('host page headers:', JSON.stringify({ coop: h['cross-origin-opener-policy'] ?? null, coep: h['cross-origin-embedder-policy'] ?? null, csp: h['content-security-policy'] ?? null }));
  await expect.poll(() => state(page), { timeout: 90_000 }).toMatch(/ready|error/);
  expect(await state(page), await page.locator('#detail').textContent() ?? '').toBe('ready');
  const readyMs = Date.now() - t0;

  // the sites differ, the frames are nested host -> wrapper -> editor, and each is on the right origin
  expect(new URL(page.url()).hostname).not.toBe(new URL(DS).hostname);
  const wrapper = page.frames().find((f) => f.url().startsWith(`${DS}/ds/editor`))!;
  const editor = page.frames().find((f) => f.url().startsWith(`${DS}/docs/`))!;
  expect(wrapper && editor).toBeTruthy();
  expect(editor.parentFrame()).toBe(wrapper);
  expect(wrapper.parentFrame()).toBe(page.mainFrame());
  await expect(editor.locator('canvas')).toBeVisible();

  // api.js: a plain script, readable cross-origin by CORP only, cached only on 200, no CORS header
  expect(apiJs.map((a) => a.status)).toEqual([200]);
  const ah = apiJs[0].headers;
  expect(ah['cross-origin-resource-policy']).toBe('cross-origin');
  expect(ah['cache-control']).toBe('public, max-age=300');
  expect(ah['access-control-allow-origin']).toBeUndefined();

  // the session is the fake backend's document, and no token sits in any storage of either site
  const [sdir] = sessions();
  const rec = record(sdir);
  expect(rec.title).toBe('01-split-runs.docx');
  // version 1 is the server's own fetch from the backend's file route
  expect(rec.versions.length).toBe(1);
  expect(sha(path.join(sdir, 'v1.bin'))).toBe(sha(DOCX));
  const hrm0 = await hrmState();
  const object = 'files/1/01-split-runs.docx';

  // the first change of the session: the wrapper tells the server, the server sends callback status 1 (once)
  await editor.locator('canvas').first().click({ position: { x: 300, y: 200 } });
  await page.keyboard.type('x');
  await expect.poll(() => page.evaluate(() => (window as any).__dirty), { timeout: 15_000 }).toBe(true);
  await expect.poll(() => /callback status 1 to .*: ok/.test(dsLog), { timeout: 15_000 }).toBe(true);
  // (the editor frame shares the wrapper's origin and keeps its own settings in localStorage: only a token-shaped value would be wrong)
  const jwtLike = '^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*$';
  expect(await wrapper.evaluate((re) => [...Object.values(localStorage), ...Object.values(sessionStorage)].filter((v) => new RegExp(re).test(v) || /Bearer/.test(v)).length + document.cookie.length, jwtLike)).toBe(0);
  expect(await page.evaluate(() => localStorage.length + sessionStorage.length + document.cookie.length)).toBe(0);

  // save: the host asks, the editor hands the bytes over, the server stores version 2
  const l0 = (await log(page)).length;
  const tSave = Date.now();
  await page.evaluate(() => (window as any).__editor.requestSave());
  await expect.poll(() => state(page), { timeout: 30_000 }).toBe('saved');
  const saveMs = Date.now() - tSave;
  const after = record(sdir);
  expect(after.versions.map((v: any) => v.n)).toEqual([1, 2]);
  expect(fs.readFileSync(path.join(sdir, 'v2.bin')).subarray(0, 4).toString('latin1')).toBe('PK\x03\x04');
  expect((await log(page)).length).toBeGreaterThan(l0);
  // the backend really got it: a status 6 whose download from our internal file route succeeded, and its stored object changed
  const hrm1 = await hrmState();
  expect(hrm1.received.slice(hrm0.received.length).map((r) => r.status)).toEqual([6]);
  expect(hrm1.rejected).toEqual(hrm0.rejected);
  expect(hrm1.objects[object]).toBe(sha(path.join(sdir, 'v2.bin')));
  expect(hrm1.objects[object]).not.toBe(sha(DOCX));

  // close: the server answers 2 (a save happened since open)
  await page.evaluate(() => (window as any).__editor.close());
  await expect.poll(() => state(page)).toBe('closed');
  expect(await page.locator('#detail').textContent()).toBe('closed status 2');
  expect(record(sdir).closed.status).toBe(2);
  const hrm2 = await hrmState();
  expect(hrm2.received.slice(hrm0.received.length).map((r) => r.status)).toEqual([6, 2]);
  expect(hrm2.received.at(-1)!.sha256).toBe(sha(path.join(sdir, 'v2.bin')));
  expect(hrm2.rejected).toEqual(hrm0.rejected);

  // S16/S17: the audit log has the events of this run, and neither it nor the server's stderr holds a token, a secret, a session id
  // or a query string
  const audit = fs.readFileSync(path.join(dir, 'data/audit.jsonl'), 'utf8');
  const events = audit.trim().split(/\r?\n/).map((l) => JSON.parse(l).event);
  for (const e of ['session.create', 'document.fetch', 'callback', 'save', 'close']) expect(events).toContain(e);
  const secrets = ['provider-secret', 'server-secret', 'audit-key'].map((n) => fs.readFileSync(path.join(dir, n), 'latin1').trim());
  for (const [what, text] of [['audit log', audit], ['server stderr', dsLog]]) {
    for (const needle of [path.basename(sdir), ...secrets, 'Bearer', '?t=', '/ds/files']) expect(text, what).not.toContain(needle);
    expect(text, what).not.toMatch(/eyJ[A-Za-z0-9_-]{10,}/);
  }
  expect(audit).not.toContain('?');
  // the host's events stay on its page: the config the server got is the signed one, with the origin api.js saw
  expect(created.length).toBe(1);
  const sent = JSON.parse(created[0]);
  expect(sent.origin).toBe(HOST);
  expect(Object.keys(sent.config)).not.toContain('events');
  // unsupported methods of the ONLYOFFICE API fail loudly; destroyEditor puts the placeholder back (the session is already closed)
  expect(await page.evaluate(() => { try { (window as any).__editor.downloadAs(); return 'no error'; } catch (e) { return String(e); } })).toContain('not supported');
  await page.evaluate(() => (window as any).__editor.destroyEditor());
  await expect.poll(() => page.frames().length, { timeout: 5000 }).toBe(1);
  expect(await page.evaluate(() => document.getElementById('ph')!.tagName)).toBe('DIV');
  console.log(`MEASURED page load -> onDocumentReady ${readyMs} ms; requestSave -> onSaved ${saveMs} ms`);
  expect(problems).toEqual([]);
  await ctx.close();
});

test('S23: the host refuses the callback -> the page hears "not saved" and the document stays dirty', async () => {
  const { ctx, page } = await open();
  await page.goto(`${HOST}/host?kind=files&id=1`);
  await expect.poll(() => state(page), { timeout: 90_000 }).toBe('ready');
  const hrm0 = await hrmState();
  // the backend now verifies callbacks with another secret: it answers 401 {"error":1}
  await fetch(`http://127.0.0.1:${hrmPort}/__test/callback-secret`, { method: 'POST', body: 'not-the-provider-secret' });
  try {
    await page.evaluate(() => (window as any).__editor.requestSave());
    await expect.poll(() => state(page), { timeout: 30_000 }).toBe('warning');
    expect(await page.evaluate(() => (window as any).__warning)).toMatchObject({ warningCode: 1 });
    expect(await page.evaluate(() => (window as any).__dirty)).toBe(true);
    // no "saved" was ever shown, the server holds the version, and the backend accepted nothing
    expect((await log(page)).some((e) => e.state === 'saved')).toBe(false);
    const [sdir] = sessions();
    expect(record(sdir).versions.map((v: any) => v.n)).toEqual([1, 2]);
    const hrm1 = await hrmState();
    expect(hrm1.received.length).toBe(hrm0.received.length);
    // the refused save (6) and, since the failed save marks the document unsaved (`saved {ok:false}`), the first-change callback (1)
    expect(hrm1.rejected.length).toBe(hrm0.rejected.length + 2);
  } finally {
    await fetch(`http://127.0.0.1:${hrmPort}/__test/callback-secret`, { method: 'POST', body: '' });
  }
  await ctx.close();
});

test('S14: a foreign frame cannot feed the host page or the wrapper', async () => {
  const { ctx, page, problems } = await open();
  await page.goto(`${HOST}/host?kind=files&id=1`);
  await expect.poll(() => state(page), { timeout: 90_000 }).toBe('ready');
  const before = sessions().length;
  // a sibling frame of the host page (same origin as the host, so only the source window can tell it from the real one)
  await page.evaluate(() => new Promise<void>((ok) => {
    const f = document.createElement('iframe');
    f.srcdoc = '<script>' +
      'parent.postMessage({type:"docs:event",event:"onError",data:{errorCode:-1,errorDescription:"forged"}},"*");' +
      'parent.frames[0].postMessage({type:"docs:config",config:{token:"x"}},"*");' +
      'parent.frames[0].postMessage({type:"docs:close"},"*");' +
      'parent.postMessage({type:"docs:ready"},"*");<' + '/script>';
    f.onload = () => ok();
    document.body.appendChild(f);
  }));
  await page.waitForTimeout(1000);
  expect(await state(page)).toBe('ready'); // no forged onError
  expect(sessions().length).toBe(before);   // no second session from a forged config
  expect(record(sessions()[0]).closed).toBeNull(); // the forged close did nothing
  expect(problems).toEqual([]);
  await ctx.close();
});

test('S13: a page on a site that is not a frame origin cannot frame the wrapper or the editor', async () => {
  const { ctx, page } = await open();
  const refusals: string[] = [];
  page.on('console', (m) => { if (/frame-ancestors|Refused to frame/i.test(m.text())) refusals.push(m.text()); });
  await page.goto(`http://127.0.0.1:${strayPort}/`);
  await expect.poll(() => refusals.length, { timeout: 15_000 }).toBeGreaterThan(0);
  // nothing of ours rendered in the frame: it is an error page, and no session was opened for it
  const frame = page.frames().find((f) => f !== page.mainFrame())!;
  expect(frame.url()).not.toContain('/ds/editor');
  await ctx.close();
  // the editor route itself, framed from the unlisted site
  const b = await browser.newContext();
  const p = await b.newPage();
  const ref2: string[] = [];
  p.on('console', (m) => { if (/frame-ancestors|Refused to frame/i.test(m.text())) ref2.push(m.text()); });
  await p.goto(`http://127.0.0.1:${strayPort}/`);
  await p.evaluate((u) => { const f = document.createElement('iframe'); f.src = u; document.body.appendChild(f); }, `${DS}/docs/`);
  await expect.poll(() => ref2.length, { timeout: 15_000 }).toBeGreaterThan(0);
  await b.close();
});

test('a tampered config token ends in onError -20 and no editor', async () => {
  const { ctx, page } = await open();
  await page.goto(`${HOST}/host?kind=files&id=1&tamper=1`);
  await expect.poll(() => state(page), { timeout: 30_000 }).toBe('error');
  expect(await page.evaluate(() => (window as any).__error)).toMatchObject({ errorCode: -20 });
  expect(page.frames().some((f) => f.url().includes('/docs/'))).toBe(false);
  expect(sessions().length).toBe(0);
  await ctx.close();
});

// WP7: the view row. The host sends mode=view (no callbackUrl, edit:false); the wrapper opens the editor read-only, so typing is refused
// and neither a save nor an autosave nor a callback leaves the page. (The server refuses them too: auth.rs, 403.)
test('WP7 view session: read-only editor, typing refused, nothing is sent', async () => {
  const { ctx, page, problems } = await open();
  const posts: string[] = [];
  page.on('request', (r) => { if (r.method() === 'POST' && r.url().includes('/ds/session/')) posts.push(new URL(r.url()).pathname.split('/').pop()!); });
  await page.goto(`${HOST}/host?kind=files&id=1&mode=view`);
  await expect.poll(() => state(page), { timeout: 90_000 }).toBe('ready');
  const hrm0 = await hrmState();
  const editor = page.frames().find((f) => f.url().startsWith(`${DS}/docs/`))!;
  await editor.locator('canvas').first().click({ position: { x: 300, y: 200 } });
  await page.keyboard.type('xyz');
  await page.keyboard.press('Control+s');
  await page.evaluate(() => (window as any).__editor.requestSave());
  await page.waitForTimeout(2000);
  expect(await page.evaluate(() => (window as any).__dirty)).not.toBe(true);
  expect(posts.filter((p) => p !== 'close' && p !== 'renew')).toEqual([]);
  const [sdir] = sessions();
  const rec = record(sdir);
  expect([rec.mode, rec.perms.edit]).toEqual(['view', false]);
  expect(rec.versions.map((v: any) => v.n)).toEqual([1]);
  expect((await hrmState()).received.length).toBe(hrm0.received.length);
  expect(problems).toEqual([]);
  await ctx.close();
});
