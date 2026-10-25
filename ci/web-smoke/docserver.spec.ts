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
// The server's fetch of document.url is WP4's. Until it lands, this spec stands in for it: it watches the session directory and writes
// version 1 itself, with the bytes it downloads from the fake backend's file route (the URL in a freshly made editor config).
const ROOT = path.resolve(__dirname, '../..');
const TARGET = process.env.OFFICE_TARGET ?? 'E:/office-target/p5-wp5';
const exe = (n: string) => path.join(TARGET, 'debug', n + (process.platform === 'win32' ? '.exe' : ''));
const DS_BIN = process.env.DS_BIN ?? exe('hoshiya-serve');
const HRM_BIN = process.env.FAKE_HRM_BIN ?? exe('fake-hrm');
const DIST = process.env.DIST ?? 'E:/office-target/p1/dist';
const OPENSSL = process.env.OPENSSL ?? 'openssl';
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');

const free = () => new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });
let dir: string, ds: ChildProcess, hrm: ChildProcess, browser: Browser, shim: NodeJS.Timeout, stray: http.Server;
let dsPort: number, hrmPort: number, strayPort: number;
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
  [dsPort, hrmPort, strayPort] = [await free(), await free(), await free()];
  DS = `https://127.0.0.1:${dsPort}`;
  HOST = `http://localhost:${hrmPort}`;

  // the fake backend first: it generates the secret the provider entry of the server must hold
  hrm = spawn(HRM_BIN, [DOCX], { env: { ...process.env, FAKE_HRM_ADDR: `127.0.0.1:${hrmPort}`, FAKE_HRM_DOCSERVER: DS, FAKE_HRM_INTERNAL_BASE: `http://127.0.0.1:${hrmPort}`, FAKE_HRM_EXP: '1' } });
  const secret = await new Promise<string>((ok, bad) => {
    let err = '';
    hrm.stderr!.on('data', (d) => { err += d; const m = /\(secret ([0-9a-f]+),/.exec(err); if (m) ok(m[1]); });
    hrm.on('exit', (c) => bad(new Error(`fake-hrm exited (${c}): ${err}`)));
    setTimeout(() => bad(new Error('fake-hrm did not start')), 15000);
  });
  fs.writeFileSync(path.join(dir, 'server-secret'), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'provider-secret'), secret);
  fs.writeFileSync(path.join(dir, 'converter-secret'), crypto.randomBytes(48));
  const f = (n: string) => path.join(dir, n).replace(/\\/g, '/');
  fs.writeFileSync(path.join(dir, 'api.toml'), [
    `listen = "127.0.0.1:${dsPort}"`, 'auth = "jwt"', `secret_file = "${f('converter-secret')}"`, `tls_cert = "${f('server.pem')}"`, `tls_key = "${f('server.key')}"`, `public_url = "${DS}"`,
    'trusted_proxies = []', `work_dir = "${f('work')}"`, `store_dir = "${f('store')}"`,
    '[docserver]', `server_secret_file = "${f('server-secret')}"`, `data_dir = "${f('data')}"`,
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
    ds.stderr!.on('data', (d) => { out += d; });
    ds.on('exit', (c) => bad(new Error(`server exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('server did not start: ' + out)), 20000);
  });

  // the stand-in for WP4's fetch
  let bytes: Buffer | null = null;
  const fetchDoc = async () => {
    const cfg = await (await fetch(`http://127.0.0.1:${hrmPort}/files/1/editor-config`)).json();
    return Buffer.from(await (await fetch(cfg.document.url)).arrayBuffer());
  };
  shim = setInterval(async () => {
    for (const s of sessions()) {
      try {
        const r = record(s);
        if (r.versions.length || fs.existsSync(path.join(s, 'v1.bin'))) continue;
        bytes ??= await fetchDoc();
        fs.writeFileSync(path.join(s, 'v1.bin'), bytes);
        r.versions = [{ n: 1, sha256: crypto.createHash('sha256').update(bytes).digest('hex'), size: bytes.length, at: Math.floor(Date.now() / 1000), by: 'fake' }];
        fs.writeFileSync(path.join(s, 'session.json'), JSON.stringify(r));
      } catch { /* the server is writing the file: next tick */ }
    }
  }, 25);

  // a page on a site that is NOT a frame origin of the provider, for the framing refusal (S13)
  stray = http.createServer((_, res) => res.writeHead(200, { 'content-type': 'text/html' }).end(`<!doctype html><iframe id="w" src="${DS}/ds/editor#o=${encodeURIComponent(`http://127.0.0.1:${strayPort}`)}"></iframe>`));
  await new Promise<void>((ok) => stray.listen(strayPort, '127.0.0.1', ok));

  browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${pin}`, '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
});
test.afterAll(async () => {
  clearInterval(shim);
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
  expect(rec.versions.length).toBe(1);
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

  // close: the server answers 2 (a save happened since open)
  await page.evaluate(() => (window as any).__editor.close());
  await expect.poll(() => state(page)).toBe('closed');
  expect(await page.locator('#detail').textContent()).toBe('closed status 2');
  expect(record(sdir).closed.status).toBe(2);
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
