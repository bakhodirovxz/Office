// The home shell (PLAN phase 4, 3.2). Plain JS, no framework, no wasm. An editor is an iframe made the first time it is used
// (?host=parent) and driven by the postMessage bridge: <prefix>:ready / :open / :save / :dirty / :failed. Recent files are
// kept in this browser (IndexedDB, capped, oldest dropped first). No product name is written here: it comes from /shell-config.json.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const CAP = 200 * 1024 * 1024;          // bytes of Recent kept in IndexedDB
  const READY_MS = 60000;                 // an editor that has not said ready by then cannot take a file
  const EXT = { docs: 'docx doc odt rtf txt', sheets: 'xlsx xlsm xls csv ods', slides: 'pptx ppt odp', pdf: 'pdf' };
  const S = { cfg: null, i18n: {}, lang: 'en', frames: {}, current: null, recent: [], db: null, templates: [] };

  const store = (k, v) => { try { return v === undefined ? localStorage.getItem(k) : localStorage.setItem(k, v); } catch (_) { return null; } };
  const t = (k, p = {}) => String((S.i18n[S.lang] || {})[k] ?? S.i18n.en[k] ?? k).replace(/\{(\w+)\}/g, (m, n) => p[n] ?? m);
  const appName = (a) => a.names[S.lang] || a.names.en;
  const appOf = (name) => { const e = name.split('.').pop().toLowerCase(); return S.cfg.apps.find((a) => EXT[a.id].split(' ').includes(e)); };
  const fmtSize = (n) => (n < 1048576 ? Math.max(1, Math.round(n / 1024)) + ' KB' : (n / 1048576).toFixed(1) + ' MB');
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const fail = (msg) => { const e = $('error'); e.textContent = msg; e.hidden = !msg; if (msg) $('ed-status').textContent = msg; };

  // ---- IndexedDB: store "meta" {id, app, name, size, ts}, store "blob" {id -> ArrayBuffer} ----
  const idb = (stores, mode, fn) => new Promise((ok, bad) => {
    if (!S.db) return bad(new Error('no storage'));
    const tx = S.db.transaction(stores, mode); let out;
    tx.oncomplete = () => ok(out); tx.onerror = tx.onabort = () => bad(tx.error);
    out = fn(...stores.map((s) => tx.objectStore(s)));
  });
  const req = (r) => new Promise((ok, bad) => { r.onsuccess = () => ok(r.result); r.onerror = () => bad(r.error); });
  async function openDb() {
    S.db = await new Promise((ok, bad) => {
      const r = indexedDB.open('shell', 1);
      r.onupgradeneeded = () => { r.result.createObjectStore('meta', { keyPath: 'id', autoIncrement: true }); r.result.createObjectStore('blob'); };
      r.onsuccess = () => ok(r.result); r.onerror = () => bad(r.error);
    });
  }
  async function loadRecent() {
    try { S.recent = (await idb(['meta'], 'readonly', (m) => req(m.getAll()))) || []; } catch (_) { S.recent = []; }
    S.recent.sort((a, b) => b.ts - a.ts);
  }
  async function remember(app, name, bytes) {
    try {
      const old = S.recent.filter((r) => r.app === app && r.name === name).map((r) => r.id);
      let total = S.recent.filter((r) => !old.includes(r.id)).reduce((s, r) => s + r.size, 0) + bytes.byteLength;
      const drop = [...old];
      for (const r of [...S.recent].reverse()) { if (total <= CAP) break; if (!old.includes(r.id)) { drop.push(r.id); total -= r.size; } }
      await idb(['meta', 'blob'], 'readwrite', (m, b) => {
        drop.forEach((id) => { m.delete(id); b.delete(id); });
        req(m.add({ app, name, size: bytes.byteLength, ts: Date.now() })).then((id) => b.put(bytes, id));
      });
      await loadRecent();
    } catch (_) { /* no storage (private window): the download still happened */ }
  }

  // ---- editors ----
  function frameOf(a) {
    if (S.frames[a.id]) return S.frames[a.id];
    const f = { app: a, ready: false, dirty: false, pending: null, timer: 0 };
    f.el = el('iframe', { src: a.route + '?host=parent', title: appName(a) });
    $('frames').append(f.el);
    return (S.frames[a.id] = f);
  }
  function post(f, file) {
    f.pending = null; clearTimeout(f.timer);
    f.el.contentWindow.postMessage({ type: f.app.prefix + ':open', name: file.name, bytes: file.bytes }, location.origin, [file.bytes]);
  }
  function show(a, file) {
    fail('');
    let f = S.frames[a.id];
    // a file replaces the document in the editor; a second New starts from the editor's own blank page
    if (f && f.dirty && !confirm(t('ed.dirty'))) return;
    if (f && !file && f.ready) { f.ready = false; f.dirty = false; f.el.contentWindow.location.reload(); }
    f = frameOf(a);
    if (file) {
      f.pending = file; clearTimeout(f.timer);
      f.timer = setTimeout(() => { if (f.pending) fail(t('err.bridge', { app: appName(a) })); }, READY_MS);
      if (f.ready) post(f, file);
    }
    for (const x of Object.values(S.frames)) x.el.hidden = x !== f;
    S.current = a; $('ed-title').textContent = file ? file.name : appName(a);
    $('ed-status').textContent = f.ready ? '' : t('ed.loading', { app: appName(a) });
    $('home').hidden = true; $('editor').hidden = false; document.title = appName(a);
  }
  function home() { $('editor').hidden = true; $('home').hidden = false; S.current = null; render(); }

  addEventListener('message', (ev) => {
    if (ev.origin !== location.origin) return;
    const f = Object.values(S.frames).find((x) => x.el.contentWindow === ev.source);
    const d = ev.data;
    if (!f || !d || typeof d.type !== 'string' || !d.type.startsWith(f.app.prefix + ':')) return;
    switch (d.type.slice(f.app.prefix.length + 1)) {
      case 'ready': f.ready = true; $('ed-status').textContent = ''; if (f.pending) post(f, f.pending); break;
      case 'dirty': f.dirty = !!d.dirty; break;
      case 'failed': fail(t('err.failed', { error: String(d.error) })); break;
      case 'save': {
        const name = String(d.name), bytes = d.bytes;
        if (!(bytes instanceof ArrayBuffer)) return;
        const a = el('a', { href: URL.createObjectURL(new Blob([bytes])), download: name });
        document.body.append(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 10000);
        $('ed-status').textContent = t('ed.saved', { name }); f.dirty = false;
        remember(f.app.id, name, bytes.slice(0));
        break;
      }
    }
  });
  addEventListener('beforeunload', (e) => { if (Object.values(S.frames).some((f) => f.dirty)) { e.preventDefault(); e.returnValue = ''; } });

  async function openPicked(file, only) {
    fail('');
    const a = appOf(file.name);
    if (!a || (only && a.id !== only)) return fail(t('err.type'));
    show(a, { name: file.name, bytes: await file.arrayBuffer() });
  }

  // ---- rendering ----
  function render() {
    document.documentElement.lang = S.lang;
    document.title = S.cfg.product;
    $('product').textContent = S.cfg.product;
    $('lang').value = S.lang;
    document.querySelectorAll('[data-i18n]').forEach((n) => { n.textContent = t(n.dataset.i18n); });
    document.querySelectorAll('[data-i18n-ph]').forEach((n) => { n.placeholder = t(n.dataset.i18nPh); });
    $('convert').textContent = S.cfg.convert.names[S.lang] || S.cfg.convert.names.en;
    $('cards').replaceChildren(...S.cfg.apps.map((a) => el('li', {}, el('span', { className: 'name' }, appName(a)),
      el('button', { type: 'button', className: 'btn', onclick: () => (a.id === 'pdf' ? pick('pdf') : show(a)) }, t(a.id === 'pdf' ? 'home.open.pdf' : 'home.new.doc')))));
    $('sec-templates').hidden = !S.templates.length;
    $('templates').replaceChildren(...S.templates.map((x) => el('li', {}, el('span', { className: 'name' }, x.name[S.lang] || x.name.en),
      el('button', { type: 'button', className: 'btn ghost', onclick: () => useTemplate(x) }, t('home.use')))));
    const q = $('search').value.trim().toLowerCase();
    const rows = S.recent.filter((r) => r.name.toLowerCase().includes(q));
    $('recent').replaceChildren(...rows.map((r) => el('li', {},
      el('button', { type: 'button', className: 'open', onclick: () => reopen(r) }, r.name),
      el('span', { className: 'muted' }, fmtSize(r.size) + ' · ' + new Date(r.ts).toLocaleDateString(S.lang)),
      el('button', { type: 'button', className: 'btn ghost', 'aria-label': t('recent.remove', { name: r.name }), onclick: () => forget(r) }, t('recent.remove.short')))));
    const none = $('recent-empty');
    none.hidden = rows.length > 0; none.textContent = t(S.recent.length ? 'home.search.none' : 'home.recent.empty');
  }
  async function reopen(r) {
    try {
      const bytes = await idb(['blob'], 'readonly', (b) => req(b.get(r.id)));
      show(S.cfg.apps.find((a) => a.id === r.app), { name: r.name, bytes });
    } catch (_) { fail(t('err.gone', { name: r.name })); }
  }
  async function forget(r) {
    try { await idb(['meta', 'blob'], 'readwrite', (m, b) => { m.delete(r.id); b.delete(r.id); }); await loadRecent(); } catch (_) { /* ignore */ }
    render();
  }
  async function useTemplate(x) {
    try {
      const res = await fetch('shell/' + x.file); if (!res.ok) throw new Error(String(res.status));
      show(S.cfg.apps.find((a) => a.id === x.app), { name: x.file.split('/').pop(), bytes: await res.arrayBuffer() });
    } catch (e) { fail(t('err.gone', { name: x.file })); }
  }
  let only = '';   // the app the file picker is limited to ('' = any)
  function pick(app) {
    // desktop host (WebView2): the native dialog and the native editor, over WebView IPC (services/suite-desktop); the answer arrives as 'suite-host'
    if (window.ipc) { window.ipc.postMessage(JSON.stringify(app ? { type: 'pick', app } : { type: 'pick' })); return; }
    const f = $('file'); only = app; f.value = ''; f.accept = '.' + (app ? EXT[app] : Object.values(EXT).join(' ')).split(' ').join(',.');
    f.click();
  }

  addEventListener('suite-host', (e) => { const d = e.detail || {}; if (d.type === 'opened') fail(d.ok || d.cancelled ? '' : String(d.error)); });

  async function main() {
    const get = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(u + ' ' + r.status); return r.json(); };
    [S.cfg, S.i18n] = await Promise.all([get('/shell-config.json'), get('shell/i18n.json')]);
    S.templates = await get('shell/templates.json').catch(() => []);
    const nav = (navigator.language || 'en').toLowerCase();
    S.lang = store('sh.lang') || (nav.startsWith('uz') ? (nav.includes('cyrl') ? 'uz-Cyrl' : 'uz-Latn') : nav.startsWith('ru') ? 'ru' : 'en');
    if (!S.i18n[S.lang]) S.lang = 'en';
    document.documentElement.style.setProperty('--accent', matchMedia('(prefers-color-scheme: dark)').matches ? S.cfg.accent.dark : S.cfg.accent.light);
    await openDb().catch(() => {}); await loadRecent();
    $('lang').onchange = (e) => { S.lang = e.target.value; store('sh.lang', S.lang); render(); };
    $('search').oninput = render;
    $('file').onchange = (e) => e.target.files[0] && openPicked(e.target.files[0], only);
    $('open-file').onclick = () => pick('');
    $('back').onclick = home;
    render();
  }
  main().catch((e) => fail(String(e.message || e)));
})();
