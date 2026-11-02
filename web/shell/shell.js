// The home shell (PLAN phase 4, 3.2). Plain JS, no framework, no wasm. An editor is an iframe made the first time it is used
// (?host=parent) and driven by the postMessage bridge: <prefix>:ready / :open / :save / :dirty / :failed. Recent files, by mode
// (review M2): in the desktop host the host's own list over IPC (names and paths, no copies); connected to a server, or in the
// desktop host's browser fallback, names only, in this tab, cleared on Disconnect; in the plain web host, copies in this browser (IndexedDB, capped, oldest dropped first).
// No product name is written here: it comes from /shell-config.json.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const CAP = 200 * 1024 * 1024;          // bytes of Recent kept in IndexedDB
  const READY_MS = 60000;                 // an editor that has not said ready by then cannot take a file
  const EXT = { docs: 'docx doc odt rtf txt', sheets: 'xlsx xlsm xls csv ods', slides: 'pptx ppt odp', pdf: 'pdf' };
  const S = { cfg: null, i18n: {}, lang: 'en', theme: 'system', frames: {}, current: null, recent: [], db: null, templates: [], mode: 'web' };
  const NAMES_CAP = 50;                   // names mode: names kept in the tab

  const store = (k, v) => { try { return v === undefined ? localStorage.getItem(k) : localStorage.setItem(k, v); } catch (_) { return null; } };
  const t = (k, p = {}) => String((S.i18n[S.lang] || {})[k] ?? S.i18n.en[k] ?? k).replace(/\{(\w+)\}/g, (m, n) => p[n] ?? m);
  const appName = (a) => a.names[S.lang] || a.names.en;
  const baseName = (p) => String(p).split(/[\\/]/).pop();
  const appOf = (name) => { const e = name.split('.').pop().toLowerCase(); return S.cfg.apps.find((a) => EXT[a.id].split(' ').includes(e)); };
  const fmtSize = (n) => (n < 1048576 ? Math.max(1, Math.round(n / 1024)) + ' KB' : (n / 1048576).toFixed(1) + ' MB');
  // A repeated button text ("New", "Use") gets the card's name, so a screen reader list tells the buttons apart; the visible text stays first in it.
  const named = (btn, what) => { btn.setAttribute('aria-label', `${btn.textContent}: ${what}`); return btn; };
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const fail = (msg) => { const e = $('error'); e.textContent = msg; e.hidden = !msg; if (msg) $('ed-status').textContent = msg; };

  // A request to the desktop host (WebView IPC); the answer arrives as the 'suite-host' event of that type. null on refusal or silence.
  const host = (msg, type) => new Promise((ok) => {
    const done = (e) => {
      const d = e.detail || {};
      if (d.type === type || d.type === 'rejected') { removeEventListener('suite-host', done); clearTimeout(tm); ok(d.type === type ? d : null); }
    };
    const tm = setTimeout(() => { removeEventListener('suite-host', done); ok(null); }, 3000);
    addEventListener('suite-host', done);
    window.ipc.postMessage(JSON.stringify(msg));
  });
  const session = (k, v) => { try { return v === undefined ? sessionStorage.getItem(k) : v === null ? sessionStorage.removeItem(k) : sessionStorage.setItem(k, v); } catch (_) { return null; } };

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
    if (S.mode === 'desktop') {
      const r = await host({ type: 'recent' }, 'recent');
      S.recent = ((r && r.items) || []).map((i) => ({ app: i.app, path: i.path, name: baseName(i.path), ts: i.ts * 1000 }));
    } else if (S.mode === 'names') {
      try { S.recent = JSON.parse(session('sh.recent') || '[]'); } catch (_) { S.recent = []; }
    } else {
      try { S.recent = (await idb(['meta'], 'readonly', (m) => req(m.getAll()))) || []; } catch (_) { S.recent = []; }
    }
    S.recent.sort((a, b) => b.ts - a.ts);
  }
  async function remember(app, name, bytes) {
    if (S.mode === 'desktop') return;     // the host's download handler records where the file went
    if (S.mode === 'names') {            // the name only: no copy of a document stays in the browser
      const kept = S.recent.filter((r) => !(r.app === app && r.name === name));
      session('sh.recent', JSON.stringify([{ app, name, ts: Date.now() }, ...kept].slice(0, NAMES_CAP)));
      return loadRecent();
    }
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
  // Editors that post <prefix>:opened once the opened document is drawn: until then a cover names the file (phase 6 M3, a large
  // document on a phone takes seconds; the editor's frame is blank or frozen meanwhile). The others clear the status on ready.
  const SAYS_OPENED = ['docs', 'slides'];
  function cover() {
    const f = S.current && S.frames[S.current.id], name = f ? f.opening : '';
    $('ed-cover').hidden = !name; $('ed-cover').textContent = name ? t('ed.opening', { name }) : '';
  }
  function frameOf(a) {
    if (S.frames[a.id]) return S.frames[a.id];
    const f = { app: a, ready: false, dirty: false, pending: null, timer: 0, opening: '' };
    f.el = el('iframe', { src: a.route + '?host=parent&lang=' + encodeURIComponent(S.lang) + '&theme=' + S.theme, title: appName(a) });
    $('frames').append(f.el);
    return (S.frames[a.id] = f);
  }
  function post(f, file) {
    f.pending = null; clearTimeout(f.timer);
    f.el.contentWindow.postMessage({ type: f.app.prefix + ':open', name: file.name, bytes: file.bytes }, location.origin, [file.bytes]);
  }
  // Docs has a phone layout whose bottom toolbar must stay above the soft keyboard (PLAN phase 6 section 5.1/6, docs/phase6/phone-layout.md): the keyboard then
  // shrinks the layout viewport. The other editors keep Chrome's default (their desktop chrome would be squeezed to a sliver).
  const viewport = (on) => { const m = document.querySelector('meta[name=viewport]'); if (m) m.content = 'width=device-width, initial-scale=1' + (on ? ', interactive-widget=resizes-content' : ''); };
  function show(a, file) {
    fail('');
    let f = S.frames[a.id];
    // a file replaces the document in the editor; a second New starts from the editor's own blank page
    if (f && f.dirty && !confirm(t('ed.dirty'))) return;
    if (f && !file && f.ready) { f.ready = false; f.dirty = false; f.el.contentWindow.location.reload(); }
    f = frameOf(a);
    f.opening = file && SAYS_OPENED.includes(a.id) ? file.name : '';
    if (file) {
      f.pending = file; clearTimeout(f.timer);
      f.timer = setTimeout(() => { if (f.pending) { f.opening = ''; cover(); fail(t('err.bridge', { app: appName(a) })); } }, READY_MS);
      if (f.ready) post(f, file);
    }
    for (const x of Object.values(S.frames)) x.el.hidden = x !== f;
    S.current = a; $('ed-title').textContent = file ? file.name : appName(a);
    $('ed-status').textContent = f.opening ? t('ed.opening', { name: f.opening }) : f.ready ? '' : t('ed.loading', { app: appName(a) });
    cover();
    $('home').hidden = true; $('editor').hidden = false; document.title = appName(a);
    viewport(a.id === 'docs');
    $('back').focus(); // the focused card is hidden now: keep the keyboard user's place, one Tab from the editor
  }
  function home() {
    const prev = S.current;
    $('editor').hidden = true; $('home').hidden = false; S.current = null; render();
    viewport(false);
    if (prev) $('cards').querySelectorAll('button')[Math.max(S.cfg.apps.findIndex((x) => x.id === prev.id), 0)]?.focus(); // back where the user left
  }

  addEventListener('message', (ev) => {
    if (ev.origin !== location.origin) return;
    const f = Object.values(S.frames).find((x) => x.el.contentWindow === ev.source);
    const d = ev.data;
    if (!f || !d || typeof d.type !== 'string' || !d.type.startsWith(f.app.prefix + ':')) return;
    switch (d.type.slice(f.app.prefix.length + 1)) {
      case 'ready': f.ready = true; if (!f.opening) $('ed-status').textContent = ''; if (f.pending) post(f, f.pending); break;
      case 'opened': if (f.opening) { f.opening = ''; $('ed-status').textContent = ''; cover(); } break;
      case 'dirty': f.dirty = !!d.dirty; break;
      case 'failed': f.opening = ''; cover(); fail(t('err.failed', { error: String(d.error) })); break;
      case 'save': {
        // only this app's own file types, and no path parts: an editor fed a hostile document cannot make the shell save x.lnk or x.hta
        const name = baseName(d.name), bytes = d.bytes;
        if (!(bytes instanceof ArrayBuffer)) return;
        if (!name || !EXT[f.app.id].split(' ').includes(name.split('.').pop().toLowerCase())) return fail(t('err.type'));
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

  // ---- the theme: light or dark on <html data-theme>, the logo and the accent follow ----
  const darkOs = matchMedia('(prefers-color-scheme: dark)');
  function applyTheme() {
    const dark = S.theme === 'dark' || (S.theme !== 'light' && darkOs.matches);
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
    $('logo-dark').media = dark ? 'all' : 'not all';
    if (S.cfg) document.documentElement.style.setProperty('--accent', dark ? S.cfg.accent.dark : S.cfg.accent.light);
  }
  // before the first paint: the browser's saved choice (web) or the OS (desktop, until the host answers)
  S.theme = (!window.ipc && store('sh.theme')) || 'system';
  applyTheme();

  // ---- rendering ----
  function render() {
    document.documentElement.lang = S.lang;
    document.title = S.cfg.product;
    $('product').textContent = S.cfg.product;
    $('lang').value = S.lang;
    $('theme').value = S.theme;
    document.querySelectorAll('[data-i18n]').forEach((n) => { n.textContent = t(n.dataset.i18n); });
    document.querySelectorAll('[data-i18n-ph]').forEach((n) => { n.placeholder = t(n.dataset.i18nPh); });
    $('convert').textContent = S.cfg.convert.names[S.lang] || S.cfg.convert.names.en;
    $('cards').replaceChildren(...S.cfg.apps.map((a) => el('li', {}, el('span', { className: 'name' }, appName(a)),
      named(el('button', { type: 'button', className: 'btn', onclick: () => (a.id === 'pdf' ? pick('pdf') : show(a)) }, t(a.id === 'pdf' ? 'home.open.pdf' : 'home.new.doc')), appName(a)))));
    $('sec-templates').hidden = !S.templates.length;
    $('templates').replaceChildren(...S.templates.map((x) => el('li', {}, el('span', { className: 'name' }, x.name[S.lang] || x.name.en),
      named(el('button', { type: 'button', className: 'btn ghost', onclick: () => useTemplate(x) }, t('home.use')), x.name[S.lang] || x.name.en))));
    const q = $('search').value.trim().toLowerCase();
    const rows = S.recent.filter((r) => r.name.toLowerCase().includes(q));
    $('recent').replaceChildren(...rows.map((r) => el('li', {},
      el('button', { type: 'button', className: 'open', onclick: () => reopen(r) }, r.name),
      el('span', { className: 'muted' }, (r.size ? fmtSize(r.size) + ' · ' : '') + new Date(r.ts).toLocaleDateString(S.lang)),
      // the desktop list is the host's open log: the page cannot edit it
      ...(S.mode === 'desktop' ? [] : [el('button', { type: 'button', className: 'btn ghost', 'aria-label': t('recent.remove', { name: r.name }), onclick: () => forget(r) }, t('recent.remove.short'))]))));
    $('recent-note').textContent = t(S.mode === 'desktop' ? 'home.recent.note.desktop' : S.mode === 'names' ? 'home.recent.note.names' : 'home.recent.note');
    const none = $('recent-empty');
    none.hidden = rows.length > 0; none.textContent = t(S.recent.length ? 'home.search.none' : 'home.recent.empty');
  }
  async function reopen(r) {
    if (S.mode === 'desktop') return window.ipc.postMessage(JSON.stringify({ type: 'open', path: r.path }));
    if (S.mode === 'names') return pick(r.app);   // no copy is kept: choose the file again
    try {
      const bytes = await idb(['blob'], 'readonly', (b) => req(b.get(r.id)));
      show(S.cfg.apps.find((a) => a.id === r.app), { name: r.name, bytes });
    } catch (_) { fail(t('err.gone', { name: r.name })); }
  }
  async function forget(r) {
    if (S.mode === 'names') { session('sh.recent', JSON.stringify(S.recent.filter((x) => x !== r))); await loadRecent(); return render(); }
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

  // F18 (PLAN 6): what the host's pre-open guard found in a workbook. A workbook over the wall is refused (error), external links only
  // warn (notice); each offers Excel when the host found it. The text is ours (guard.*); the host sends the code, not the sentence.
  function guardNote(d) {
    const g = $('guard'), b = $('guard-excel');
    g.hidden = !d.guard;
    if (!d.guard) return;
    const sheets = S.cfg.apps.find((a) => a.id === 'sheets');
    $('guard-text').textContent = t('guard.' + d.guard, { app: sheets ? appName(sheets) : '' });
    g.className = d.ok ? 'muted' : 'error';
    b.hidden = !d.excel;
    b.onclick = () => host({ type: 'open_in_excel', path: d.path }, 'excel').then((r) => { if (r && !r.ok) fail(String(r.error)); });
  }

  addEventListener('suite-host', (e) => {
    const d = e.detail || {};
    if (d.type === 'opened') { guardNote(d); fail(d.ok || d.cancelled || d.guard ? '' : String(d.error)); if (d.ok || d.guard) loadRecent().then(render); }
    // the host's download handler: where a saved file went (or why it was refused)
    if (d.type === 'saved') { if (d.ok) $('ed-status').textContent = t('ed.saved', { name: d.name }); else fail(String(d.error)); loadRecent().then(render); }
  });

  async function main() {
    const get = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(u + ' ' + r.status); return r.json(); };
    [S.cfg, S.i18n] = await Promise.all([get('/shell-config.json'), get('shell/i18n.json')]);
    S.templates = await get('shell/templates.json').catch(() => []);
    const nav = (navigator.language || 'en').toLowerCase();
    S.mode = window.ipc ? 'desktop' : session('cv.token') || S.cfg.local ? 'names' : 'web';
    // the desktop keeps the language in the host's settings, not in the WebView's storage
    const settings = S.mode === 'desktop' ? ((await host({ type: 'settings_get' }, 'settings')) || { values: {} }).values : {};
    const saved = S.mode === 'desktop' ? settings.lang : store('sh.lang');
    // settings.toml `lang`: 'auto' (the default) follows the browser; 'uz-Latn-2026' has no shell catalog yet and shows uz-Latn
    S.lang = saved && saved !== 'auto' ? saved : (nav.startsWith('uz') ? (nav.includes('cyrl') ? 'uz-Cyrl' : 'uz-Latn') : nav.startsWith('ru') ? 'ru' : 'en');
    if (!S.i18n[S.lang]) S.lang = S.lang.startsWith('uz-Latn') ? 'uz-Latn' : 'en';
    S.theme = (S.mode === 'desktop' ? settings.theme : store('sh.theme')) || 'system';
    applyTheme();
    darkOs.addEventListener('change', applyTheme);
    // earlier versions kept document copies in the WebView's IndexedDB: drop that database, it is not used in these modes
    if (S.mode !== 'web') { try { indexedDB.deleteDatabase('shell'); } catch (_) { /* ignore */ } } else await openDb().catch(() => {});
    await loadRecent();
    $('lang').onchange = (e) => {
      S.lang = e.target.value; render();
      // editors read the language when they start: drop the idle ones so the next open follows the new language (an editor with unsaved work keeps its own until it is reopened)
      for (const [id, fr] of Object.entries(S.frames)) if (!fr.dirty) { fr.el.remove(); delete S.frames[id]; }
      if (S.mode === 'desktop') host({ type: 'settings_set', key: 'lang', value: S.lang }, 'settings'); else store('sh.lang', S.lang);
    };
    // settings.toml `theme` (system | light | dark): the apps read it when they start, the shell applies it now
    $('theme').onchange = (e) => {
      S.theme = e.target.value; applyTheme();
      // editors read the theme when they start: drop the idle ones, as for the language (one with unsaved work keeps its own until reopened)
      for (const [id, fr] of Object.entries(S.frames)) if (!fr.dirty) { fr.el.remove(); delete S.frames[id]; }
      if (S.mode === 'desktop') host({ type: 'settings_set', key: 'theme', value: S.theme }, 'settings'); else store('sh.theme', S.theme);
    };
    $('search').oninput = render;
    $('file').onchange = (e) => e.target.files[0] && openPicked(e.target.files[0], only);
    $('open-file').onclick = () => pick('');
    $('back').onclick = home;
    render();
  }
  main().catch((e) => fail(String(e.message || e)));
})();
