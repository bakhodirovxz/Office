// Test environment for the WebView harness (PLAN phase 6, WP4): throwaway CA + leaf for 10.0.2.2, the fake HRM and the document server
// on loopback (the emulator reaches the host as 10.0.2.2), config written like ci/web-smoke/docserver.spec.ts. Our code, MIT OR Apache-2.0.
// Env: DS_BIN (hoshiya-serve), FAKE_HRM_BIN, DIST (trunk dists + shell/), OPENSSL, FRAME_ORIGINS (comma list; default: the server's own origin).
import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const T = process.env.OFFICE_TARGET ?? 'E:/office-target/p5/services';
const exe = (n) => path.join(T, 'debug', n + '.exe');
const DS_BIN = process.env.DS_BIN ?? exe('hoshiya-serve');
const HRM_BIN = process.env.FAKE_HRM_BIN ?? 'E:/office-target/p6-webview/services/debug/fake-hrm.exe';
const DIST = process.env.DIST ?? 'E:/office-target/p5/dist';
const DOCX = path.join(ROOT, 'domains/docs/crates/docs-translit/tests/fixtures/word/01-split-runs.docx');
export const HOST_ALIAS = '10.0.2.2';

const free = () => new Promise((ok) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }); });

export async function startEnv(dir, { frameOrigins } = {}) {
  fs.mkdirSync(dir, { recursive: true });
  const ssl = (...a) => execFileSync(process.env.OPENSSL ?? 'openssl', a, { cwd: dir, stdio: 'pipe', env: { ...process.env, MSYS_NO_PATHCONV: '1' } });
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'ca.key');
  ssl('req', '-x509', '-new', '-key', 'ca.key', '-sha256', '-days', '2', '-subj', '/CN=test-ca', '-out', 'ca.pem');
  ssl('ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', 'leaf.key');
  ssl('pkcs8', '-topk8', '-nocrypt', '-in', 'leaf.key', '-out', 'server.key');
  ssl('req', '-new', '-key', 'leaf.key', '-subj', '/CN=localhost', '-out', 'leaf.csr');
  fs.writeFileSync(path.join(dir, 'ext.cnf'), `subjectAltName=DNS:localhost,IP:127.0.0.1,IP:${HOST_ALIAS}\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n`);
  ssl('x509', '-req', '-in', 'leaf.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-days', '2', '-sha256', '-extfile', 'ext.cnf', '-out', 'server.pem');
  const [dsPort, hrmPort, intPort] = [await free(), await free(), await free()];
  const DS = `https://${HOST_ALIAS}:${dsPort}`, HRM = `http://${HOST_ALIAS}:${hrmPort}`;

  const hrm = spawn(HRM_BIN, [DOCX], { env: { ...process.env, FAKE_HRM_ADDR: `127.0.0.1:${hrmPort}`, FAKE_HRM_DOCSERVER: DS, FAKE_HRM_INTERNAL_BASE: `http://127.0.0.1:${intPort}`, FAKE_HRM_EXP: '1', FAKE_HRM_TEST_API: '1', FAKE_HRM_MOBILE_EDIT: '1' } });
  const secret = await new Promise((ok, bad) => {
    let err = '';
    hrm.stderr.on('data', (d) => { err += d; const m = /\(secret ([0-9a-f]+),/.exec(err); if (m) ok(m[1]); });
    hrm.on('exit', (c) => bad(new Error(`fake-hrm exited (${c}): ${err}`)));
    setTimeout(() => bad(new Error('fake-hrm did not start')), 15000);
  });
  // test values only: the fake HRM generated its secret for this run, the server's own are random
  fs.writeFileSync(path.join(dir, 'server-secret'), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'provider-secret'), secret);
  fs.writeFileSync(path.join(dir, 'converter-secret'), crypto.randomBytes(48));
  fs.writeFileSync(path.join(dir, 'audit-key'), crypto.randomBytes(48).toString('hex'));
  const f = (n) => path.join(dir, n).replace(/\\/g, '/');
  const origins = frameOrigins ?? (process.env.FRAME_ORIGINS ?? DS).split(',');
  const toml = [
    `listen = "127.0.0.1:${dsPort}"`, 'auth = "jwt"', `secret_file = "${f('converter-secret')}"`, `tls_cert = "${f('server.pem')}"`, `tls_key = "${f('server.key')}"`, `public_url = "${DS}"`,
    'trusted_proxies = []', `work_dir = "${f('work')}"`, `store_dir = "${f('store')}"`,
    '[docserver]', `server_secret_file = "${f('server-secret')}"`, `data_dir = "${f('data')}"`, `audit_key_file = "${f('audit-key')}"`, `internal_listen = "127.0.0.1:${intPort}"`, 'internal_allow = ["127.0.0.1"]',
    '[[docserver.providers]]', 'name = "fake"', 'issuer = "fake"', `secret_file = "${f('provider-secret')}"`,
    `fetch_allow = [{ url = "http://127.0.0.1:${hrmPort}", internal = true }]`,
    `callback_allow = [{ url = "http://127.0.0.1:${hrmPort}", internal = true }]`, `frame_origins = [${origins.map((o) => `"${o}"`).join(', ')}]`,
  ].join('\n');
  fs.writeFileSync(path.join(dir, 'api.toml'), toml);
  const denv = { ...process.env };
  for (const t of ['DOCS', 'SHEETS', 'SLIDES', 'PDF']) denv[`CONVERT_${t}_CLI`] = 'unused';
  const ds = spawn(DS_BIN, ['--config', path.join(dir, 'api.toml'), '--dist', DIST, '--brand', path.join(ROOT, 'brand/brand.toml')], { env: denv });
  let dsLog = '';
  await new Promise((ok, bad) => {
    let out = '';
    ds.stdout.on('data', (d) => { out += d; if (/listening on https:/.test(out)) ok(); });
    ds.stderr.on('data', (d) => { out += d; dsLog += d; });
    ds.on('exit', (c) => bad(new Error(`server exited early (${c}): ${out}`)));
    setTimeout(() => bad(new Error('server did not start: ' + out)), 20000);
  });
  return {
    dir, DS, HRM, dsPort, hrmPort, origins, dsLog: () => dsLog,
    hrmState: async () => (await fetch(`http://127.0.0.1:${hrmPort}/__test/state`)).json(),
    sessions: () => { const d = path.join(dir, 'data/sessions'); return fs.existsSync(d) ? fs.readdirSync(d).map((s) => path.join(d, s)) : []; },
    stop: () => { ds.kill(); hrm.kill(); },
  };
}
