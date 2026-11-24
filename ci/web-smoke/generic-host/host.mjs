// A minimal, generic ONLYOFFICE-API host application (no dependencies): the shape any integrator's backend has. It signs the editor
// config (HS256 over the whole config, no exp), serves the document bytes, takes the save callback (verifies its token, downloads
// `url` on status 2 and 6) and lets a test read what it received. Test tool for packaging/docserver/rehearse-generic.sh; synthetic
// documents only. Env: PORT, DS_URL (the document server as browsers see it), SELF_URL (this host as the document server sees it),
// JWT_SECRET (shared with the document server), DOCS_DIR (the files to serve, by name).
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

const { PORT = '3000', DS_URL, SELF_URL, JWT_SECRET, DOCS_DIR = '/docs' } = process.env;
if (!DS_URL || !SELF_URL || !JWT_SECRET) throw new Error('DS_URL, SELF_URL and JWT_SECRET are required');
const b64 = (b) => Buffer.from(b).toString('base64url');
const sign = (p) => { const h = b64(JSON.stringify({ alg: 'HS256', typ: 'JWT' })) + '.' + b64(JSON.stringify(p)); return h + '.' + crypto.createHmac('sha256', JWT_SECRET).update(h).digest('base64url'); };
const verify = (t) => {
  const [h, p, s] = String(t).split('.');
  if (!s || JSON.parse(Buffer.from(h, 'base64url')).alg !== 'HS256') throw new Error('bad token');
  const want = crypto.createHmac('sha256', JWT_SECRET).update(h + '.' + p).digest();
  if (!crypto.timingSafeEqual(want, Buffer.from(s, 'base64url'))) throw new Error('bad signature');
  return JSON.parse(Buffer.from(p, 'base64url'));
};
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const TYPES = { docx: 'word', xlsx: 'cell', pptx: 'slide', pdf: 'pdf' };
const docs = {}; // name -> { bytes, version }
for (const n of fs.readdirSync(DOCS_DIR)) docs[n] = { bytes: fs.readFileSync(path.join(DOCS_DIR, n)), version: 1 };
const received = [], rejected = [];

const page = `<!doctype html><meta charset=utf-8><title>Generic host</title>
<style>html,body{margin:0;height:100%}body{display:flex;flex-direction:column;font:14px system-ui}#ph{flex:1;min-height:0}</style>
<body data-state=loading><div style="padding:6px;background:#1f3a5f;color:#fff"><b>Generic host</b> <span id=banner>loading</span> <span id=detail></span></div>
<div id=ph></div>
<script src="${DS_URL}/web-apps/apps/api/documents/api.js"></script>
<script>
  const qs = new URLSearchParams(location.search);
  const set = (s, d) => { document.body.dataset.state = s; banner.textContent = s; detail.textContent = d || ''; };
  window.__dirty = null;
  (async () => {
    const cfg = await (await fetch('/config?' + qs)).json();
    cfg.events = {
      onDocumentReady: () => set('ready'),
      onDocumentStateChange: (e) => { window.__dirty = e.data; },
      onSaved: (e) => set('saved', 'v' + e.data.version),
      onClosed: (e) => set('closed', 'closed status ' + e.data.status),
      onError: (e) => set('error', (e.data.status || '') + ' ' + (e.data.message || '')),
    };
    window.__editor = new DocsAPI.DocEditor('ph', cfg);
  })().catch((e) => set('error', String(e)));
</script>`;

const send = (res, code, body, type = 'application/json') => { res.writeHead(code, { 'content-type': type, 'cache-control': 'no-store' }); res.end(body); };
const readBody = (req) => new Promise((ok) => { const c = []; req.on('data', (d) => c.push(d)); req.on('end', () => ok(Buffer.concat(c))); });

http.createServer(async (req, res) => {
  const u = new URL(req.url, SELF_URL);
  const name = u.searchParams.get('doc') ?? '';
  try {
    if (u.pathname === '/host') return send(res, 200, page, 'text/html; charset=utf-8');
    if (u.pathname === '/config') {
      const d = docs[name], ext = name.split('.').pop();
      if (!d) return send(res, 404, '{}');
      const edit = u.searchParams.get('mode') !== 'view';
      const cfg = {
        documentType: TYPES[ext],
        document: {
          fileType: ext, title: name, key: `${name.replace(/\W/g, '_')}_v${d.version}_${crypto.randomBytes(4).toString('hex')}`,
          url: `${SELF_URL}/file?doc=${name}&t=${sign({ doc: name, exp: Math.floor(Date.now() / 1000) + 3600 })}`,
          permissions: { edit, download: true, print: true, review: false, comment: edit },
        },
        editorConfig: {
          mode: edit ? 'edit' : 'view', lang: 'en', user: { id: 'u1', name: 'Test User' },
          ...(edit ? { callbackUrl: `${SELF_URL}/callback?doc=${name}`, customization: { forcesave: true } } : {}),
        },
      };
      return send(res, 200, JSON.stringify({ ...cfg, token: sign(cfg) }));
    }
    if (u.pathname === '/file') { // what document.url points at: the bytes as the host holds them now
      if (verify(u.searchParams.get('t')).doc !== name) throw new Error('wrong doc');
      return send(res, 200, docs[name].bytes, 'application/octet-stream');
    }
    if (u.pathname === '/download') return send(res, 200, docs[name].bytes, 'application/octet-stream'); // the user's "download" from the host
    if (u.pathname === '/state') return send(res, 200, JSON.stringify({ received, rejected, sha: Object.fromEntries(Object.entries(docs).map(([n, d]) => [n, sha(d.bytes)])) }));
    if (u.pathname === '/callback' && req.method === 'POST') {
      const body = JSON.parse((await readBody(req)).toString());
      let p;
      try { p = verify(body.token ?? (req.headers.authorization ?? '').replace(/^Bearer /, '')); } catch (e) { rejected.push(String(e.message)); return send(res, 401, '{"error":1}'); }
      const c = p.payload ?? p; // the token carries the body, as is or under `payload`
      if (c.key !== body.key || c.status !== body.status) { rejected.push('payload mismatch'); return send(res, 403, '{"error":1}'); }
      const rec = { doc: name, status: body.status, bytes: 0, sha256: '' };
      if (body.status === 2 || body.status === 6) {
        const r = await fetch(body.url);
        if (!r.ok) { rejected.push('download ' + r.status); return send(res, 500, '{"error":1}'); }
        const bytes = Buffer.from(await r.arrayBuffer());
        docs[name] = { bytes, version: docs[name].version + 1 };
        Object.assign(rec, { bytes: bytes.length, sha256: sha(bytes) });
      }
      received.push(rec);
      return send(res, 200, '{"error":0}');
    }
    send(res, 404, '{}');
  } catch (e) { send(res, 500, JSON.stringify({ error: String(e.message) })); }
}).listen(Number(PORT), '0.0.0.0');
