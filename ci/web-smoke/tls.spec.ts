import { test, expect, chromium, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

// Server mode end to end over TLS: suite-serve (auth = "jwt", built-in TLS, public_url, no trusted proxies, a redirect listener) and the
// converter page in Chromium. The CA and the server certificate are made here with openssl (EC P-256, in a temp dir that is deleted
// afterwards) and are never installed anywhere: Chromium gets --ignore-certificate-errors-spki-list=<pin of the server key>, so the
// handshake is real and pinned, and a second Chromium without the flag must refuse the same server (the negative control).
// Run with: npm exec --prefix ci/web-smoke -- playwright test --config ci/web-smoke/playwright.tls.config.ts
const ROOT = path.resolve(__dirname, '../..');
const SVC = process.env.CONVERT_SERVICES_DIR ?? 'E:/office-target/p1/services/release';
const BIN = process.env.SUITE_SERVE_BIN ?? path.join(SVC, 'hoshiya-serve.exe');
const FAKE = process.env.CONVERT_API_FAKE_TOOL ?? path.join(SVC, 'convert-api-fake-tool.exe');
const OPENSSL = process.env.OPENSSL ?? 'openssl';

const I18N: Record<string, Record<string, string>> = JSON.parse(fs.readFileSync(path.join(ROOT, 'services/convert-api/ui/i18n.json'), 'utf8'));
const txt = (key: string) => I18N.en[key];

const SECRET = crypto.randomBytes(48);
const b64u = (b: Buffer | string) => Buffer.from(b).toString('base64url');
/** An HS256 token the way the issuer makes it (the server's jwt.rs: scope "convert", exp required). */
// Long-lived on purpose (the server accepts up to 12 h): a run on a VM that is paused or whose clock steps must not outlive its tokens.
const TTL = 11 * 3600;
function mint(expIn: number): string {
  const head = b64u(JSON.stringify({ alg: 'HS256', typ: 'JWT' }));
  const body = b64u(JSON.stringify({ sub: 'tls-user', scope: 'convert', exp: Math.floor(Date.now() / 1000) + expIn }));
  return `${head}.${body}.${crypto.createHmac('sha256', SECRET).update(`${head}.${body}`).digest('base64url')}`;
}
const free = () => new Promise<number>((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => ok(p)); }); });

let dir: string, proc: ChildProcess, browser: Browser, strict: Browser;
let port: number, httpPort: number, base: string;
const GOOD = Buffer.from('Salom dunyo, bu oddiy matn.\n');
const file = (name: string, body: Buffer | string) => ({ name, mimeType: 'application/octet-stream', buffer: Buffer.from(body) });
const pdfFile = (name: string, pages: number) => file(name, `%PDF-1.4\n%% PAGES=${pages}\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n`);

test.beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tls-spec-'));
  const ssl = (...a: string[]) => execFileSync(OPENSSL, a, { cwd: dir, stdio: 'pipe' });
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'ca.key');
  ssl('req', '-x509', '-new', '-key', 'ca.key', '-sha256', '-days', '2', '-subj', '/CN=test-ca', '-out', 'ca.pem');
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'leaf.key');
  ssl('pkcs8', '-topk8', '-nocrypt', '-in', 'leaf.key', '-out', 'server.key'); // the PKCS#8 form the server reads
  ssl('req', '-new', '-key', 'leaf.key', '-subj', '/CN=localhost', '-out', 'leaf.csr');
  fs.writeFileSync(path.join(dir, 'ext.cnf'), 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n');
  ssl('x509', '-req', '-in', 'leaf.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-days', '2', '-sha256', '-extfile', 'ext.cnf', '-out', 'server.pem');
  const pin = crypto.createHash('sha256').update(new crypto.X509Certificate(fs.readFileSync(path.join(dir, 'server.pem'))).publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
  fs.writeFileSync(path.join(dir, 'secret'), SECRET);
  for (const n of ['server-secret', 'provider-secret']) fs.writeFileSync(path.join(dir, n), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'audit-key'), crypto.randomBytes(48).toString('hex'));
  [port, httpPort] = [await free(), await free()];
  base = `https://localhost:${port}`;
  const f = (n: string) => path.join(dir, n).replace(/\\/g, '/');
  fs.writeFileSync(path.join(dir, 'api.toml'), [
    `listen = "127.0.0.1:${port}"`, 'auth = "jwt"', `secret_file = "${f('secret')}"`, `tls_cert = "${f('server.pem')}"`, `tls_key = "${f('server.key')}"`,
    `public_url = "${base}"`, `redirect_listen = "127.0.0.1:${httpPort}"`, 'trusted_proxies = []', `work_dir = "${f('work')}"`, `store_dir = "${f('store')}"`, 'max_file_mb = 1',
    // the document server on the same TLS listener (S19): only /ds/editor is fetched, no provider is ever contacted
    '[docserver]', `server_secret_file = "${f('server-secret')}"`, `data_dir = "${f('ds-data')}"`, `audit_key_file = "${f('audit-key')}"`,
    '[[docserver.providers]]', 'name = "fake"', 'issuer = "fake"', `secret_file = "${f('provider-secret')}"`,
    'fetch_allow = [{ url = "http://127.0.0.1:9", internal = true }]', 'callback_allow = [{ url = "http://127.0.0.1:9", internal = true }]', 'frame_origins = ["https://hr.example"]',
  ].join('\n'));
  fs.mkdirSync(path.join(dir, 'dist'));
  const env = { ...process.env } as Record<string, string>;
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) env[`CONVERT_${t}_CLI`] = FAKE;
  proc = spawn(BIN, ['--config', path.join(dir, 'api.toml'), '--dist', path.join(dir, 'dist'), '--brand', path.join(ROOT, 'brand/brand.toml')], { env });
  await new Promise<void>((ok, bad) => {
    let out = '';
    proc.stdout!.on('data', (d) => { out += d; if (/listening on https:/.test(out)) ok(); });
    proc.on('exit', (c) => bad(new Error(`suite-serve exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('suite-serve did not start')), 15000);
  });
  browser = await chromium.launch({ args: [`--ignore-certificate-errors-spki-list=${pin}`] });
  strict = await chromium.launch();
});
test.afterAll(async () => {
  await browser?.close(); await strict?.close();
  proc?.kill();
  fs.rmSync(dir, { recursive: true, force: true }); // the throwaway CA and keys
});

/** A fresh context (so sessionStorage is empty) with a log of every request: URL and Authorization header. */
async function open(): Promise<{ ctx: BrowserContext; page: Page; log: { url: string; auth?: string }[]; problems: string[] }> {
  const ctx = await browser.newContext({ viewport: { width: 1100, height: 900 } });
  const page = await ctx.newPage();
  const log: { url: string; auth?: string }[] = [], problems: string[] = [];
  page.on('request', (r) => log.push({ url: r.url(), auth: r.headers()['authorization'] }));
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error' || /Content Security Policy/i.test(m.text())) problems.push(m.text()); });
  await page.addInitScript(() => { if (!localStorage.getItem('cv.lang')) localStorage.setItem('cv.lang', 'en'); });
  return { ctx, page, log, problems };
}
const pick = async (page: Page, files: ReturnType<typeof file>[]) => { await page.setInputFiles('#pick', files); await expect(page.locator('#files tr')).toHaveCount(files.length); };

test('TLS is real: pinned Chromium connects (TLS 1.3, HSTS, CSP); Chromium without the pin refuses', async () => {
  const { ctx, page, problems } = await open();
  const res = await page.goto(`${base}/convert/`);
  expect(page.url().startsWith('https://')).toBe(true);
  expect((await res!.securityDetails())?.protocol).toBe('TLS 1.3');
  const h = res!.headers();
  expect(h['strict-transport-security']).toMatch(/^max-age=31536000/);
  expect(h['content-security-policy']).toContain("script-src 'self'");
  // no token: the page says the server needs one (the 401 of /formats)
  await expect(page.locator('#alert')).toContainText(txt('ui.conn.need_token'));
  expect(problems.filter((p) => !/401/.test(p))).toEqual([]);
  await ctx.close();
  const bare = await strict.newContext();
  await expect(bare.newPage().then((p) => p.goto(`${base}/healthz`))).rejects.toThrow(/ERR_CERT/);
  await bare.close();
});

test('plain HTTP only redirects to the https public_url', async () => {
  const get = (p: string) => new Promise<http.IncomingMessage>((ok, bad) => { http.get({ host: '127.0.0.1', port: httpPort, path: p, headers: { host: 'evil.example' } }, ok).on('error', bad); });
  const r = await get('/convert/?x=1');
  expect(r.statusCode).toBe(308);
  expect(r.headers.location).toBe(`${base}/convert/?x=1`);
  r.resume();
  // in the browser the redirect lands on the pinned https origin
  const { ctx, page } = await open();
  await page.goto(`http://127.0.0.1:${httpPort}/convert/`);
  expect(page.url()).toBe(`${base}/convert/`);
  await ctx.close();
});

test('S19: /ds/editor over built-in TLS carries HSTS, and plain HTTP to it is redirected', async () => {
  const { ctx, page } = await open();
  const res = await page.goto(`${base}/ds/editor`);
  expect(res!.status(), await res!.text()).toBe(200);
  expect(res!.headers()['strict-transport-security']).toMatch(/^max-age=31536000/);
  await ctx.close();
  const r = await new Promise<http.IncomingMessage>((ok, bad) => { http.get({ host: '127.0.0.1', port: httpPort, path: '/ds/editor?x=1', headers: { host: 'evil.example' } }, ok).on('error', bad); });
  expect([r.statusCode, r.headers.location]).toEqual([308, `${base}/ds/editor?x=1`]);
  r.resume();
});

test('the #token= flow: conversion, fetch+blob download, PDF combine, with the token in no URL', async () => {
  const token = mint(TTL);
  const { ctx, page, log, problems } = await open();
  await page.goto(`${base}/convert/#token=${token}`);
  await expect(page.locator('#conn-mode')).toHaveText(txt('ui.conn.mode.server'));
  expect(new URL(page.url()).hash).toBe(''); // out of the address bar
  expect(await page.evaluate(() => location.href.includes('token'))).toBe(false);
  await expect(page.locator('#o-to')).toBeVisible();
  // conversion
  await pick(page, [file('note.txt', GOOD)]);
  await page.selectOption('#o-to', 'pdf');
  await page.click('#go');
  const row = page.locator('#results li', { hasText: 'note.txt' });
  await expect(row.locator('.badge')).toHaveText(txt('state.done'));
  const [dl] = await Promise.all([page.waitForEvent('download'), row.locator('a.btn').click()]);
  expect(fs.readFileSync((await dl.path())!).subarray(0, 5).toString()).toBe('%PDF-');
  // combine two PDFs, in the order the page shows
  await page.click('#clear');
  await pick(page, [pdfFile('b.pdf', 3), pdfFile('a.pdf', 2)]);
  await page.selectOption('#o-mode', 'combine');
  await page.click('#go');
  const card = page.locator('#results li').first();
  await expect(card.locator('.badge')).toHaveText(txt('state.done'));
  const [cdl] = await Promise.all([page.waitForEvent('download'), card.locator('a.btn').click()]);
  expect(cdl.suggestedFilename()).toBe('combined.pdf');
  expect(/ORDER=([\d,]+)/.exec(fs.readFileSync((await cdl.path())!, 'latin1'))![1]).toBe('3,2');
  // the network log: the token is in no URL, and every API call (the downloads included) carried it in the header
  const jwtPart = token.split('.')[1];
  expect(log.filter((r) => r.url.includes(token) || r.url.includes(jwtPart) || /token=/i.test(r.url))).toEqual([]);
  const api = log.filter((r) => new URL(r.url).pathname.startsWith('/api/v1/'));
  expect(api.some((r) => /\/files\//.test(r.url))).toBe(true);
  expect(api.every((r) => r.auth === `Bearer ${token}`)).toBe(true);
  expect(log.every((r) => r.url.startsWith('https://') || r.url.startsWith('blob:'))).toBe(true);
  expect(problems).toEqual([]);
  await ctx.close();
});

test('the paste field connects, and Disconnect returns the page to local mode', async () => {
  const token = mint(TTL);
  const { ctx, page, log } = await open();
  await page.goto(`${base}/convert/`);
  await expect(page.locator('#alert')).toContainText(txt('ui.conn.need_token'));
  await page.fill('#srv-token', token);
  await page.click('#srv-apply');
  await expect(page.locator('#conn-mode')).toHaveText(txt('ui.conn.mode.server'));
  await expect(page.locator('#alert')).toBeHidden();
  await expect(page.locator('#srv-token')).toHaveValue('');
  await expect(page.locator('#o-to')).toBeVisible();
  await page.click('#srv-clear');
  await expect(page.locator('#conn-mode')).toHaveText(txt('ui.conn.mode.local'));
  expect(await page.evaluate(() => sessionStorage.getItem('cv.token'))).toBeNull();
  const n = log.length;
  await page.reload();
  await expect(page.locator('#conn-mode')).toHaveText(txt('ui.conn.mode.local'));
  expect(log.slice(n).filter((r) => r.auth)).toEqual([]); // nothing is sent after the disconnect
  await ctx.close();
});

test('an expired token and a token signed with another secret give a clear message', async () => {
  for (const token of [mint(-600), mint(TTL).replace(/.$/, (c) => (c === 'A' ? 'B' : 'A'))]) {
    const { ctx, page } = await open();
    await page.goto(`${base}/convert/#token=${token}`);
    await expect(page.locator('#alert')).toContainText(txt('ui.conn.token_rejected'));
    await expect(page.locator('#conn')).toHaveJSProperty('open', true); // the Connect panel is opened for a new token
    await ctx.close();
  }
});
