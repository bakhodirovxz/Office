'use strict';
// The converter page (PLAN phase 3 section 6). No framework, no inline script, no third-party origin (CSP script-src 'self').
// All server text goes in through textContent, never innerHTML.
(function () {
  const $ = (id) => document.getElementById(id);
  const LANGS = ['en', 'ru', 'uz-Latn', 'uz-Cyrl'];
  const S = { lang: 'en', i18n: {}, fmts: null, health: null, items: [], sent: [], job: null, ctrl: null, timer: null, running: false, token: null };

  // ---- small helpers -------------------------------------------------------------------------------------------------
  // shortcut: storage may be blocked (private window); the page works without it.
  const store = (area, k, v) => { try { const s = window[area]; if (v === undefined) return s.getItem(k); if (v === null) s.removeItem(k); else s.setItem(k, v); } catch (_) { /* ignore */ } return null; };
  const has = (k) => k in (S.i18n[S.lang] || {}) || k in (S.i18n.en || {});
  function t(key, p) {
    const s = (S.i18n[S.lang] || {})[key] ?? (S.i18n.en || {})[key] ?? key;
    return p ? s.replace(/\{(\w+)\}/g, (m, k) => (k in p ? String(p[k]) : m)) : s;
  }
  function el(tag, props, ...kids) {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) { if (k === 'class') e.className = v; else if (k === 'text') e.textContent = v; else if (k.startsWith('on')) e.addEventListener(k.slice(2), v); else e.setAttribute(k, v); }
    for (const c of kids) if (c != null) e.append(c);
    return e;
  }
  const extOf = (n) => { const m = /\.([A-Za-z0-9]+)$/.exec(n); return m ? m[1].toLowerCase() : ''; };
  const size = (n) => (n < 1024 ? n + ' B' : n < 1048576 ? (n / 1024).toFixed(1) + ' KB' : (n / 1048576).toFixed(1) + ' MB');

  class ApiErr extends Error { constructor(code, message, status) { super(message); this.code = code; this.status = status || 0; } }

  // ---- language ------------------------------------------------------------------------------------------------------
  function guessLang() {
    const saved = store('localStorage', 'cv.lang');
    if (LANGS.includes(saved)) return saved;
    const n = (navigator.language || 'en').toLowerCase();
    if (n.startsWith('uz')) return /cyrl|-cy/.test(n) ? 'uz-Cyrl' : 'uz-Latn';
    return n.startsWith('ru') ? 'ru' : 'en';
  }
  function setLang(l) {
    S.lang = l;
    store('localStorage', 'cv.lang', l);
    document.documentElement.lang = l;
    $('lang').value = l;
    document.title = t('app.name');
    $('appname').textContent = t('app.name');
    for (const e of document.querySelectorAll('[data-i18n]')) e.textContent = t(e.dataset.i18n);
    const sc = $('o-sidecar'); if (sc && S.health && S.health.sidecars && S.health.sidecars.pdf2docx === false) sc.textContent = t('ui.opt.engine.sidecar') + ' (' + t('ui.opt.engine.unavailable') + ')';
    render();
  }

  // ---- messages ------------------------------------------------------------------------------------------------------
  const ERR_ALIAS = { too_large: 'file_too_large', bad_origin: 'bad_host', unknown_field: 'bad_options' };
  // A numeric code is a file's exit code; a string code is a request-level error. Both show the localised headline and the
  // engine's own English message under "Details", so nothing is lost in translation.
  function describe(code, message) {
    const key = typeof code === 'number' ? 'exit.' + code : 'err.' + (ERR_ALIAS[code] || code);
    const head = has(key) ? t(key) : t('err.unknown');
    return { head, details: message && message !== head ? t('ui.details', { text: message }) : '' };
  }
  function showError(e) {
    const a = $('alert');
    a.replaceChildren();
    if (e.status === 401) {
      a.append(el('p', { text: t('ui.conn.need_token') }));
      $('conn').open = true;
    } else {
      const d = e.code === 'network' ? { head: t('err.network', { text: e.message }), details: '' } : describe(e.code, e.message);
      a.append(el('p', { text: d.head }));
      if (d.details) a.append(el('p', { class: 'muted', text: d.details }));
    }
    a.hidden = false;
  }
  const clearError = () => { $('alert').hidden = true; $('alert').replaceChildren(); };
  function warningText(w) {
    const key = 'warning.' + w.code;
    let text;
    if (!has(key)) text = w.text;
    else if (w.code === 'kind_mismatch') text = t(key, { kind: (/content is \.(\w+)/.exec(w.text) || [])[1] || '' });
    else text = t(key, { text: w.text });
    return t('ui.warning', { text: w.count > 1 ? text + ' (×' + w.count + ')' : text });
  }

  // ---- api -----------------------------------------------------------------------------------------------------------
  async function api(path, init) {
    const o = Object.assign({}, init);
    if (S.token) o.headers = Object.assign({ Authorization: 'Bearer ' + S.token }, o.headers);
    let r;
    try { r = await fetch(path, o); } catch (e) { if (e.name === 'AbortError') throw e; throw new ApiErr('network', e.message); }
    if (!r.ok) {
      let b = null; try { b = await r.json(); } catch (_) { /* not json */ }
      const er = b && b.error ? b.error : {};
      throw new ApiErr(er.code || 'unknown', er.message || r.status + ' ' + r.statusText, r.status);
    }
    return r;
  }
  async function download(path, name) {
    try {
      const r = await api(path);
      const url = URL.createObjectURL(await r.blob());
      const a = el('a', { href: url, download: name });
      document.body.append(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
    } catch (e) { showError(Object.assign(e, { message: t('ui.download.failed', { text: e.message }) })); }
  }
  // Local mode: a plain link. With a token the browser cannot send the header on a link, so the click fetches the file.
  function dlLink(path, name, label, aria) {
    const a = el('a', { class: 'btn', href: path, download: name, text: label });
    if (aria) a.setAttribute('aria-label', aria);
    if (S.token) { a.setAttribute('href', '#'); a.setAttribute('role', 'button'); a.addEventListener('click', (ev) => { ev.preventDefault(); download(path, name); }); }
    return a;
  }

  // ---- connect to server ---------------------------------------------------------------------------------------------
  function readFragment() {
    const m = /^#token=([^&]+)/.exec(location.hash);
    if (!m) return;
    try { store('sessionStorage', 'cv.token', decodeURIComponent(m[1])); } catch (_) { /* ignore */ }
    history.replaceState(null, '', location.pathname + location.search); // the token never stays in the address bar
  }
  function connect() {
    clearError();
    const url = $('srv-url').value.trim(), tok = $('srv-token').value.trim();
    if (url) {
      let u;
      try { u = new URL(url); if (!/^https?:$/.test(u.protocol)) throw new Error('scheme'); } catch (_) { showError(new ApiErr('x', t('ui.conn.bad_url'))); return; }
      if (u.origin !== location.origin) { // CSP and CORS keep this page on its own origin: open the other server's own page instead
        $('conn-mode').textContent = t('ui.conn.redirecting');
        location.assign(u.origin + '/' + (tok ? '#token=' + encodeURIComponent(tok) : ''));
        return;
      }
    }
    if (tok) { S.token = tok; store('sessionStorage', 'cv.token', tok); }
    $('srv-token').value = '';
    start();
  }
  function disconnect() { S.token = null; store('sessionStorage', 'cv.token', null); $('srv-token').value = ''; $('srv-url').value = ''; clearError(); start(); }

  // ---- files and options ---------------------------------------------------------------------------------------------
  function addFiles(list) {
    for (const f of list) S.items.push({ file: f, ext: extOf(f.name), state: null, kind: null, code: null, message: '', warnings: [], outputs: [], n: null });
    render();
  }
  const rowsFor = (ext, to) => (S.fmts ? S.fmts.rows.filter((r) => r.from === ext && (!to || r.to === to)) : []);
  function targets() {
    if (!S.fmts) return { list: [], none: false };
    const known = S.items.filter((i) => S.fmts.inputs.includes(i.ext));
    if (!known.length) return { list: S.items.length ? S.fmts.targets : [], none: false }; // unknown extensions: the server decides by content
    const set = new Set(known.flatMap((i) => rowsFor(i.ext).map((r) => r.to)));
    return { list: S.fmts.targets.filter((x) => set.has(x)), none: set.size === 0 };
  }
  function optionAvailable(opt, to) {
    const known = S.items.filter((i) => S.fmts && S.fmts.inputs.includes(i.ext));
    return known.some((i) => rowsFor(i.ext, to).some((r) => r.options.includes(opt)));
  }
  function render() {
    const lim = S.fmts && S.fmts.limits;
    $('drop-hint').textContent = lim ? t('ui.drop.hint', { files: lim.maxBatchFiles, mb: lim.maxBatchMb }) : '';
    $('conn-mode').textContent = S.token ? t('ui.conn.mode.server') : t('ui.conn.mode.local');
    $('files-none').hidden = S.items.length > 0;
    $('files-wrap').hidden = !S.items.length;
    $('clear').hidden = !S.items.length;
    $('clear').disabled = S.running;
    const body = $('files'); body.replaceChildren();
    S.items.forEach((it, i) => {
      const st = it.state ? t('state.' + it.state) : '';
      body.append(el('tr', {},
        el('th', { scope: 'row', class: 'name', text: it.file.name }),
        el('td', { text: (it.kind || it.ext || '—').toUpperCase() }),
        el('td', { text: size(it.file.size) }),
        el('td', { text: st }),
        el('td', {}, el('button', { type: 'button', class: 'btn ghost', 'aria-label': t('ui.files.remove_named', { name: it.file.name }), text: t('ui.files.remove'), onclick: () => { S.items.splice(i, 1); render(); } }))));
      if (S.running) body.lastChild.querySelector('button').disabled = true;
    });
    for (const e of $('opt-h').parentElement.querySelectorAll('select, input')) e.disabled = S.running;
    // target list, keeping the current choice when it is still offered
    const tg = targets(), sel = $('o-to'), cur = sel.value;
    sel.replaceChildren(...tg.list.map((x) => el('option', { value: x, text: x.toUpperCase() })));
    if (tg.list.includes(cur)) sel.value = cur;
    $('o-to-none').hidden = !tg.none;
    sel.disabled = S.running || !tg.list.length;
    const to = sel.value;
    for (const f of document.querySelectorAll('[data-opt]')) f.hidden = !(to && optionAvailable(f.dataset.opt, to));
    const tr = $('o-translit').value;
    document.querySelector('[data-opt="alphabet"]').hidden = !tr || document.querySelector('[data-opt="translit"]').hidden;
    const sc = $('o-sidecar'); sc.disabled = !!(S.health && S.health.sidecars && S.health.sidecars.pdf2docx !== true);
    if (sc.disabled && $('o-engine').value === 'sidecar') $('o-engine').value = 'native';
    $('go').disabled = S.running || !S.items.length || !to;
    $('stop').hidden = !S.running;
    renderResults();
  }
  function collect() {
    const o = { to: $('o-to').value, locale: S.lang }, vis = (n) => !document.querySelector('[data-opt="' + n + '"]').hidden;
    if (vis('translit') && $('o-translit').value) { o.translit = $('o-translit').value; if (vis('alphabet')) o.alphabet = $('o-alphabet').value; }
    if (vis('sheet')) o.sheet = $('o-sheet').value;
    if (vis('pages') && $('o-pages').value.trim()) o.pages = $('o-pages').value.trim();
    if (vis('dpi') && $('o-dpi').value) o.dpi = Number($('o-dpi').value);
    if (vis('pdf-engine')) o.pdfEngine = $('o-engine').value;
    return o;
  }

  // ---- results -------------------------------------------------------------------------------------------------------
  function renderResults() {
    const any = S.items.some((i) => i.state);
    $('res').hidden = !any && !S.running;
    const list = $('results'); list.replaceChildren();
    for (const it of S.items) {
      if (!it.state) continue;
      const li = el('li', { class: 'res res-' + it.state }, el('div', { class: 'res-head' }, el('strong', { text: it.file.name }), el('span', { class: 'badge b-' + it.state, text: t('state.' + it.state) })));
      if (it.state === 'failed' || it.state === 'rejected') {
        const d = describe(it.code, it.message);
        li.append(el('p', { class: 'msg', text: d.head }));
        if (d.details) li.append(el('p', { class: 'muted', text: d.details }));
      }
      for (const w of it.warnings) li.append(el('p', { class: 'warn', text: warningText(w) }));
      if (it.outputs.length) {
        const row = el('div', { class: 'row' });
        for (const o of it.outputs) row.append(dlLink('/api/v1/jobs/' + S.job + '/files/' + it.n + '/' + o.id, o.name, t('ui.download.named', { name: o.name }), t('ui.download.named', { name: o.name })));
        li.append(row);
      }
      list.append(li);
    }
    const done = S.items.filter((i) => i.state === 'done' || i.state === 'failed' || i.state === 'rejected').length, total = S.items.filter((i) => i.state).length;
    $('bar').hidden = !S.running; $('bar').max = Math.max(total, 1); $('bar').value = done;
    $('prog').textContent = total ? t('ui.progress', { done, total }) : '';
    const zip = $('zip'), finished = S.job && !S.running && S.items.some((i) => i.outputs.length);
    zip.hidden = !finished;
    if (finished) {
      const path = '/api/v1/jobs/' + S.job + '/zip';
      zip.setAttribute('href', path); zip.setAttribute('download', 'converted.zip');
      zip.onclick = S.token ? (ev) => { ev.preventDefault(); download(path, 'converted.zip'); } : null;
    }
  }
  const announce = (it) => { $('live').textContent = t('ui.live', { name: it.file.name, state: t('state.' + it.state) }); };

  // ---- convert, poll, cancel -----------------------------------------------------------------------------------------
  function setBusy(on, msg) { S.running = on; $('busy').hidden = !on || !msg; $('busy').textContent = msg ? t(msg) : ''; render(); }
  async function convert() {
    clearError();
    for (const it of S.items) Object.assign(it, { state: null, kind: null, code: null, message: '', warnings: [], outputs: [], n: null });
    S.job = null;
    // Files over the server's limit are refused here with their message instead of being uploaded (the server checks again).
    const lim = S.fmts && S.fmts.limits;
    S.sent = [];
    for (const it of S.items) {
      const max = lim ? (it.ext === 'xlsx' ? lim.maxSheetFileMb : lim.maxFileMb) * 1048576 : Infinity;
      if (it.file.size > max) { Object.assign(it, { state: 'rejected', code: 'file_too_large', message: size(it.file.size) + ' > ' + size(max) }); announce(it); } else S.sent.push(it);
    }
    if (!S.sent.length) { render(); return; }
    const fd = new FormData();
    for (const it of S.sent) fd.append('files', it.file, it.file.name);
    fd.append('options', JSON.stringify(collect()));
    S.ctrl = new AbortController();
    setBusy(true, 'ui.uploading');
    let r;
    try { r = await (await api('/api/v1/jobs', { method: 'POST', body: fd, signal: S.ctrl.signal })).json(); }
    catch (e) { if (e.name !== 'AbortError') showError(e); setBusy(false); return; }
    S.job = r.job;
    r.files.forEach((f, i) => {
      const it = S.sent[i]; it.n = f.n; it.kind = f.kind || null;
      if (f.error) { it.state = 'rejected'; it.code = f.error.code; it.message = f.error.message; announce(it); } else it.state = 'queued';
    });
    setBusy(true, null);
    poll(0);
  }
  async function poll(fails) {
    const id = S.job;
    try {
      const v = await (await api('/api/v1/jobs/' + id)).json();
      if (id !== S.job) return;
      for (const f of v.files) {
        const it = S.sent[f.n]; if (!it) continue;
        const was = it.state;
        if (it.state !== 'rejected') it.state = f.state;
        Object.assign(it, { kind: f.kind || it.kind, warnings: f.warnings || [], outputs: f.outputs || [] });
        if (f.state === 'failed') { it.code = f.code; it.message = f.message || ''; }
        if (it.state !== was) announce(it);
      }
      if (v.state === 'done') { setBusy(false); return; }
      render();
      S.timer = setTimeout(() => poll(0), 1000);
    } catch (e) {
      if (id !== S.job) return;
      if (e.code === 'network' && fails < 2) { S.timer = setTimeout(() => poll(fails + 1), 1500); return; }
      showError(e); setBusy(false); // polling stops on any 4xx or lasting failure, and the error is shown
    }
  }
  async function cancel() {
    clearTimeout(S.timer);
    if (S.ctrl) S.ctrl.abort();
    const id = S.job; S.job = null;
    for (const it of S.items) { it.state = null; it.outputs = []; }
    setBusy(false);
    $('alert').replaceChildren(el('p', { text: t('ui.cancelled') })); $('alert').hidden = false;
    if (id) { try { await api('/api/v1/jobs/' + id, { method: 'DELETE' }); } catch (e) { if (e.status !== 404) showError(e); } }
  }

  // ---- start ---------------------------------------------------------------------------------------------------------
  async function start() {
    S.fmts = null; S.health = null;
    // api() adds the Authorization header in server mode: without a token the server answers only {ok:true} (no tool list, no sidecar state)
    try { S.health = await (await api('/healthz')).json(); } catch (_) { /* shown by the next call if the server is really down */ }
    try { S.fmts = await (await api('/api/v1/formats')).json(); } catch (e) { showError(e); }
    setLang(S.lang);
  }
  async function init() {
    readFragment();
    S.token = store('sessionStorage', 'cv.token') || null;
    try { S.i18n = await (await fetch('/i18n.json')).json(); } catch (_) { S.i18n = {}; }
    S.lang = guessLang();
    $('lang').addEventListener('change', (e) => setLang(e.target.value));
    $('pick').addEventListener('change', (e) => { addFiles(e.target.files); e.target.value = ''; });
    $('clear').addEventListener('click', () => { S.items = []; S.job = null; render(); });
    $('go').addEventListener('click', convert);
    $('stop').addEventListener('click', cancel);
    $('srv-apply').addEventListener('click', connect);
    $('srv-clear').addEventListener('click', disconnect);
    for (const id of ['o-to', 'o-translit']) $(id).addEventListener('change', render);
    const d = $('drop');
    for (const ev of ['dragenter', 'dragover']) d.addEventListener(ev, (e) => { e.preventDefault(); d.classList.add('over'); });
    for (const ev of ['dragleave', 'drop']) d.addEventListener(ev, (e) => { e.preventDefault(); d.classList.remove('over'); });
    d.addEventListener('drop', (e) => { if (!S.running) addFiles(e.dataTransfer.files); });
    if (S.token) $('conn').open = false;
    await start();
  }
  init();
})();
