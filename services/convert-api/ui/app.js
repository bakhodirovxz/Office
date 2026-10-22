'use strict';
// The converter page (PLAN phase 3 section 6). No framework, no inline script, no third-party origin (CSP script-src 'self').
// All server text goes in through textContent, never innerHTML.
(function () {
  const $ = (id) => document.getElementById(id);
  // uz-Latn-2026: the 2026 Latin alphabet, generated from uz-Cyrl by `cargo xt i18n export` (WP16)
  const LANGS = ['en', 'ru', 'uz-Latn', 'uz-Latn-2026', 'uz-Cyrl'];
  const S = { lang: 'en', i18n: {}, fmts: null, health: null, items: [], sent: [], op: null, job: null, ctrl: null, timer: null, running: false, token: null };

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

  // The server's default Latin alphabet (uz-translit DEFAULT_LATIN_ALPHABET, owner decision O13); null until /formats answers.
  const defAlphabet = () => (S.fmts && S.fmts.defaults && S.fmts.defaults.alphabet) || null;

  // ---- language ------------------------------------------------------------------------------------------------------
  function guessLang() {
    const saved = store('localStorage', 'cv.lang');
    if (LANGS.includes(saved)) return saved;
    const n = (navigator.language || 'en').toLowerCase();
    if (n.startsWith('uz')) return /cyrl|-cy/.test(n) ? 'uz-Cyrl' : 'uz-Latn';
    return n.startsWith('ru') ? 'ru' : 'en';
  }
  // the number format follows the language until the user picks one
  const langLocale = (l) => (l === 'ru' ? 'ru' : l === 'uz-Cyrl' ? 'uz-Cyrl' : l.startsWith('uz') ? 'uz-Latn' : 'en-US');
  function setLang(l) {
    S.lang = l;
    if (!S.localeTouched) $('o-locale').value = langLocale(l);
    store('localStorage', 'cv.lang', l);
    document.documentElement.lang = l;
    $('lang').value = l;
    document.title = t('app.name');
    $('appname').textContent = t('app.name');
    for (const e of document.querySelectorAll('[data-i18n]')) e.textContent = t(e.dataset.i18n);
    for (const o of $('o-alphabet').options) if (o.value === defAlphabet()) o.textContent = t('ui.opt.alphabet.default', { alphabet: o.textContent });
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
    // `args` are the values the engine put into its English text ({encoding}, {missing}, {total}, {copy}, {kind}, {target})
    const text = has(key) ? t(key, Object.assign({ text: w.text }, w.args)) : w.text;
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
  // The click fetches the file in both modes: a token cannot ride on a plain link, and a file that has expired (404) is then
  // reported by the page instead of a browser error page.
  function dlLink(path, name, label, aria) {
    const a = el('a', { class: 'btn', href: path, download: name, text: label, role: 'button' });
    if (aria) a.setAttribute('aria-label', aria);
    a.addEventListener('click', (ev) => { ev.preventDefault(); download(path, name); });
    return a;
  }

  // ---- connect to server ---------------------------------------------------------------------------------------------
  // A token travels only over https or to this machine: on plain http across a network it would be readable by anyone on the path.
  const secureHost = (u) => u.protocol === 'https:' || (u.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(u.hostname));
  function readFragment() {
    const m = /^#token=([^&]+)/.exec(location.hash);
    if (!m) return;
    if (!secureHost(location)) { history.replaceState(null, '', location.pathname + location.search); return; }
    try { store('sessionStorage', 'cv.token', decodeURIComponent(m[1])); } catch (_) { /* ignore */ }
    history.replaceState(null, '', location.pathname + location.search); // the token never stays in the address bar
  }
  function connect() {
    clearError();
    const url = $('srv-url').value.trim(), tok = $('srv-token').value.trim();
    if (url) {
      let u;
      try { u = new URL(url); if (!secureHost(u)) throw new Error('scheme'); } catch (_) { showError(new ApiErr('x', t('ui.conn.bad_url'))); return; }
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
  // PDF operations are offered when every file is a PDF (by its name; the server checks the content)
  const allPdf = () => S.items.length > 0 && S.items.every((i) => i.ext === 'pdf');
  const mode = () => (allPdf() ? $('o-mode').value : 'convert');
  function move(i, d) {
    const j = i + d;
    if (S.running || j < 0 || j >= S.items.length) return;
    [S.items[i], S.items[j]] = [S.items[j], S.items[i]];
    render();
    const row = $('files').children[j];
    // keep the focus on the control that was used; at the end of the list the other arrow takes it
    const want = row.querySelector(d < 0 ? '[data-act="up"]' : '[data-act="down"]');
    (want && !want.disabled ? want : row.querySelector('[data-act="up"]:not(:disabled), [data-act="down"]:not(:disabled)')).focus();
    $('live').textContent = t('ui.order.moved', { name: S.items[j].file.name, pos: j + 1, total: S.items.length });
  }
  function render() {
    const lim = S.fmts && S.fmts.limits;
    $('drop-hint').textContent = lim ? t('ui.drop.hint', { files: lim.maxBatchFiles, mb: lim.maxBatchMb }) : '';
    $('conn-mode').textContent = S.token ? t('ui.conn.mode.server') : t('ui.conn.mode.local');
    $('files-none').hidden = S.items.length > 0;
    $('files-wrap').hidden = !S.items.length;
    $('clear').hidden = !S.items.length;
    $('clear').disabled = S.running;
    $('f-mode').hidden = !allPdf();
    const md = mode(), isOp = md !== 'convert', ordered = md === 'combine';
    $('order-hint').hidden = !ordered;
    const tbl = $('files-wrap').querySelector('table');
    if (ordered) tbl.setAttribute('aria-describedby', 'order-hint'); else tbl.removeAttribute('aria-describedby');
    const body = $('files'); body.replaceChildren();
    S.items.forEach((it, i) => {
      const st = it.state ? t('state.' + it.state) : '';
      const acts = el('td', { class: 'acts' });
      if (ordered) {
        acts.append(
          el('button', { type: 'button', class: 'btn ghost', 'data-act': 'up', 'aria-label': t('ui.order.up', { name: it.file.name }), text: '\u2191', onclick: () => move(i, -1) }),
          el('button', { type: 'button', class: 'btn ghost', 'data-act': 'down', 'aria-label': t('ui.order.down', { name: it.file.name }), text: '\u2193', onclick: () => move(i, 1) }));
        acts.querySelector('[data-act="up"]').disabled = i === 0;
        acts.querySelector('[data-act="down"]').disabled = i === S.items.length - 1;
      }
      acts.append(el('button', { type: 'button', class: 'btn ghost', 'data-act': 'remove', 'aria-label': t('ui.files.remove_named', { name: it.file.name }), text: t('ui.files.remove'), onclick: () => { S.items.splice(i, 1); render(); } }));
      const tr = el('tr', { 'data-i': String(i) },
        el('th', { scope: 'row', class: 'name', text: (ordered ? (i + 1) + '. ' : '') + it.file.name }),
        el('td', { text: (it.kind || it.ext || '—').toUpperCase() }),
        el('td', { text: size(it.file.size) }),
        el('td', { text: st }),
        acts);
      if (S.running) for (const b of acts.querySelectorAll('button')) b.disabled = true;
      body.append(tr);
    });
    for (const e of $('opt-h').parentElement.querySelectorAll('select, input')) e.disabled = S.running;
    // target list, keeping the current choice when it is still offered
    const tg = targets(), sel = $('o-to'), cur = sel.value;
    sel.replaceChildren(...tg.list.map((x) => el('option', { value: x, text: x.toUpperCase() })));
    if (tg.list.includes(cur)) sel.value = cur;
    $('o-to-none').hidden = !tg.none;
    sel.disabled = S.running || !tg.list.length;
    const to = sel.value;
    $('f-to').hidden = isOp;
    $('f-locale').hidden = isOp || !S.items.some((i) => ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'ods'].includes(i.ext));
    for (const f of document.querySelectorAll('[data-mode]')) f.hidden = f.dataset.mode !== md;
    for (const f of document.querySelectorAll('[data-opt]')) f.hidden = isOp || !(to && optionAvailable(f.dataset.opt, to));
    const tr = $('o-translit').value;
    document.querySelector('[data-opt="alphabet"]').hidden = isOp || !tr || document.querySelector('[data-opt="translit"]').hidden;
    // what the operation still needs, said next to the choice and not only by a disabled button
    const n = S.items.length;
    const need = md === 'combine' && n < 2 ? 'ui.pdfop.min_files' : (md === 'split' || md === 'extract') && n !== 1 ? 'ui.pdfop.one_file' : null;
    const filled = md === 'split' ? !!($('o-every').value.trim() || $('o-before').value.trim()) : md === 'extract' ? !!$('o-xpages').value.trim() : true;
    $('op-note').hidden = !need; $('op-note').textContent = need ? t(need) : '';
    const sc = $('o-sidecar'); sc.disabled = !!(S.health && S.health.sidecars && S.health.sidecars.pdf2docx !== true);
    if (sc.disabled && $('o-engine').value === 'sidecar') $('o-engine').value = 'native';
    $('go').disabled = S.running || !S.items.length || (isOp ? !!need || !filled : !to);
    $('stop').hidden = !S.running;
    renderResults();
  }
  function collect() {
    const md = mode();
    if (md !== 'convert') {
      const o = { op: md };
      if (md === 'split') {
        if ($('o-every').value.trim()) o.every = Number($('o-every').value);
        if ($('o-before').value.trim()) o.before = $('o-before').value.split(',').map((x) => Number(x.trim()));
      }
      if (md === 'extract') o.pages = $('o-xpages').value.trim();
      return o;
    }
    const o = { to: $('o-to').value, locale: $('o-locale').value }, vis = (n) => !document.querySelector('[data-opt="' + n + '"]').hidden;
    if (vis('translit') && $('o-translit').value) { o.translit = $('o-translit').value; if (vis('alphabet')) o.alphabet = $('o-alphabet').value; }
    if (vis('sheet')) o.sheet = $('o-sheet').value;
    if (vis('pages') && $('o-pages').value.trim()) o.pages = $('o-pages').value.trim();
    if (vis('dpi') && $('o-dpi').value) o.dpi = Number($('o-dpi').value);
    if (vis('pdf-engine')) o.pdfEngine = $('o-engine').value;
    return o;
  }

  // ---- results -------------------------------------------------------------------------------------------------------
  function renderResults() {
    // a PDF operation has one result for all its files
    const shown = S.op ? [S.op] : S.items.filter((i) => i.state);
    $('res').hidden = !shown.length && !S.running;
    const list = $('results'); list.replaceChildren();
    for (const it of shown) {
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
    const done = shown.filter((i) => i.state === 'done' || i.state === 'failed' || i.state === 'rejected').length, total = shown.length;
    $('bar').hidden = !S.running; $('bar').max = Math.max(total, 1); $('bar').value = done;
    $('prog').textContent = total ? t('ui.progress', { done, total }) : '';
    const zip = $('zip'), finished = S.job && !S.running && shown.some((i) => i.outputs.length);
    zip.hidden = !finished;
    if (finished) {
      const path = '/api/v1/jobs/' + S.job + '/zip';
      zip.setAttribute('href', path); zip.setAttribute('download', 'converted.zip');
      zip.onclick = (ev) => { ev.preventDefault(); download(path, 'converted.zip'); };
    }
  }
  const announce = (it) => { $('live').textContent = t('ui.live', { name: it.file.name, state: t('state.' + it.state) }); };

  // ---- convert, poll, cancel -----------------------------------------------------------------------------------------
  function setBusy(on, msg) { S.running = on; $('busy').hidden = !on || !msg; $('busy').textContent = msg ? t(msg) : ''; render(); }
  async function convert() {
    clearError();
    for (const it of S.items) Object.assign(it, { state: null, kind: null, code: null, message: '', warnings: [], outputs: [], n: null });
    S.job = null; S.op = null;
    const md = mode();
    // Files over the server's limit are refused here with their message instead of being uploaded (the server checks again).
    const lim = S.fmts && S.fmts.limits;
    S.sent = [];
    for (const it of S.items) {
      const max = lim ? (it.ext === 'xlsx' ? lim.maxSheetFileMb : lim.maxFileMb) * 1048576 : Infinity;
      if (it.file.size > max) { Object.assign(it, { state: 'rejected', code: 'file_too_large', message: size(it.file.size) + ' > ' + size(max) }); announce(it); } else S.sent.push(it);
    }
    if (md !== 'convert') {
      // the files are parts of one operation: if one is refused here, nothing is uploaded
      S.op = { file: { name: S.items.map((i) => i.file.name).join(', ') }, state: 'queued', code: null, message: '', warnings: [], outputs: [], n: 0 };
      const bad = S.items.find((i) => i.state === 'rejected');
      if (bad) { Object.assign(S.op, { state: 'rejected', code: bad.code, message: bad.file.name + ': ' + bad.message }); announce(S.op); render(); return; }
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
      if (f.error && !S.op) { it.state = 'rejected'; it.code = f.error.code; it.message = f.error.message; announce(it); } else it.state = 'queued';
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
        const it = S.op || S.sent[f.n]; if (!it) continue;
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
    S.op = null;
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
    if (defAlphabet()) $('o-alphabet').value = defAlphabet();
    setLang(S.lang);
  }
  async function init() {
    readFragment();
    S.token = store('sessionStorage', 'cv.token') || null;
    let i18nOk = true;
    try { const r = await fetch('/i18n.json'); if (!r.ok) throw new Error(String(r.status)); S.i18n = await r.json(); } catch (_) { S.i18n = {}; i18nOk = false; }
    S.lang = guessLang();
    $('lang').addEventListener('change', (e) => setLang(e.target.value));
    $('pick').addEventListener('change', (e) => { addFiles(e.target.files); e.target.value = ''; });
    $('clear').addEventListener('click', () => { S.items = []; S.job = null; render(); });
    $('go').addEventListener('click', convert);
    $('stop').addEventListener('click', cancel);
    $('srv-apply').addEventListener('click', connect);
    $('srv-clear').addEventListener('click', disconnect);
    for (const id of ['o-to', 'o-translit', 'o-mode']) $(id).addEventListener('change', render);
    $('o-locale').addEventListener('change', () => { S.localeTouched = true; });
    for (const id of ['o-every', 'o-before', 'o-xpages']) $(id).addEventListener('input', render);
    // Alt+Up / Alt+Down moves the file in focus when the files are being combined
    $('files').addEventListener('keydown', (e) => {
      const tr = e.target.closest && e.target.closest('tr');
      if (!e.altKey || !tr || mode() !== 'combine' || !['ArrowUp', 'ArrowDown'].includes(e.key)) return;
      e.preventDefault(); move(Number(tr.dataset.i), e.key === 'ArrowUp' ? -1 : 1);
    });
    const d = $('drop');
    for (const ev of ['dragenter', 'dragover']) d.addEventListener(ev, (e) => { e.preventDefault(); d.classList.add('over'); });
    for (const ev of ['dragleave', 'drop']) d.addEventListener(ev, (e) => { e.preventDefault(); d.classList.remove('over'); });
    d.addEventListener('drop', (e) => { if (!S.running) addFiles(e.dataTransfer.files); });
    if (S.token) $('conn').open = false;
    await start();
    // without the catalog every text would show as its key: say so in plain English (this one text cannot come from the catalog)
    if (!i18nOk) { $('alert').replaceChildren(el('p', { text: 'The page texts could not be loaded. Reload the page.' })); $('alert').hidden = false; }
  }
  init();
})();
