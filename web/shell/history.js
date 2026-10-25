// The History panel of the shell (PLAN phase 4, 5.5, WP10). Route #history or #history=<urlencoded path>. Plain JS; the desktop host does
// the work over WebView IPC (history, history_versions, history_view, history_restore, history_copy, history_name, history_compare:
// phase 5 WP12, the domain diff tools); in the browser or
// connected to a server there is no host and the panel only says so. Strings: shell/i18n.json (keys hist.*); the language is the page's.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const desktop = !!window.ipc;
  const S = { i18n: null, docs: [], path: '', vers: [], was: 'home' };
  const t = (k, p = {}) => String(((S.i18n || {})[document.documentElement.lang] || {})[k] ?? ((S.i18n || {}).en || {})[k] ?? k).replace(/\{(\w+)\}/g, (m, n) => p[n] ?? m);
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const plain = (p) => String(p).replace(/^\\\\\?\\/, '');          // the store keeps \\?\C:\..., the user knows C:\...
  const same = (a, b) => plain(a).toLowerCase() === plain(b).toLowerCase();
  const base = (p) => plain(p).split(/[\\/]/).pop();
  const size = (n) => (n < 1048576 ? Math.max(1, Math.round(n / 1024)) + ' KB' : (n / 1048576).toFixed(1) + ' MB');
  const when = (s) => new Date(s * 1000).toLocaleString(document.documentElement.lang);
  const aria = (e, attrs) => { for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; };
  const say = (msg, bad) => { const e = $('hist-msg'); e.textContent = msg; e.className = bad ? 'error' : 'muted'; e.setAttribute('role', bad ? 'alert' : 'status'); };

  // One request to the host; its answer is the 'suite-host' event of that type. null (and a message) on refusal or silence.
  const host = (msg, type, wait = 20000) => new Promise((ok) => {
    const done = (e) => {
      const d = e.detail || {};
      // a version list for another document answers another request (copy and view replies carry their output path instead)
      if ((d.type === type && (type !== 'history_versions' || d.path === msg.path)) || d.type === 'rejected') { removeEventListener('suite-host', done); clearTimeout(tm); ok(d); }
    };
    const tm = setTimeout(() => { removeEventListener('suite-host', done); ok({ type: 'rejected', error: t('hist.silent') }); }, wait);
    addEventListener('suite-host', done);
    window.ipc.postMessage(JSON.stringify(msg));
  });
  // The store keeps these two messages in English (suite-core history.rs); they are shown in the page language, the stored data is unchanged.
  const note = (v) => {
    const m = /^Restored from version (\d+)$/.exec(v.message || '');
    if (m) return t('hist.restoredtag', { v: m[1] });
    return v.message === 'Before restore' ? t('hist.beforerestore') : v.message || t('hist.restoredtag', { v: v.restored_from });
  };
  const fails = (d, type) => (d.type !== type || d.ok === false ? (say(String(d.error || t('hist.silent')), true), true) : false);

  function renderDocs() {
    $('hist-docs').replaceChildren(...S.docs.map((d) => el('li', {},
      aria(el('button', { type: 'button', className: 'open', onclick: () => choose(d.path, true) }, base(d.path)), same(d.path, S.path) ? { 'aria-current': 'true' } : {}),
      el('span', { className: 'muted' }, t('hist.doc.meta', { n: d.versions, time: when(d.ts), size: size(d.size) })))));
    $('hist-none').hidden = S.docs.length > 0;
  }

  function renderVersions() {
    const { verified, problem } = S;
    const name = base(S.path);
    $('hist-ver-sec').hidden = false;
    $('h-vers').textContent = t('hist.vers', { name });
    const c = $('hist-chain');
    c.textContent = verified ? t('hist.chain.ok') : t('hist.chain.bad', { problem: problem || '' });
    c.className = verified ? 'ok' : 'error';
    const newest = Math.max(...S.vers.map((v) => v.v));
    // a repeated button text gets the version it belongs to, so a screen reader list tells the buttons apart
    const btn = (key, v, fn) => aria(el('button', { type: 'button', className: 'btn ghost', onclick: fn }, t(key)), { 'aria-label': `${t(key)}: ${t('hist.version', { v: v.v })}` });
    $('hist-vers').replaceChildren(...[...S.vers].reverse().map((v) => el('li', {},
      el('div', { className: 'vinfo' },
        el('strong', {}, t('hist.version', { v: v.v })),
        ...(v.v === newest ? [' ', el('span', { className: 'tag' }, t('hist.latest'))] : []),
        ...(v.name ? [' ', el('span', { className: 'tag name' }, v.name)] : []),
        el('div', { className: 'muted' }, t('hist.meta', { time: when(v.time), author: v.author, size: size(v.size), hash: String(v.sha256).slice(0, 8) })),
        ...(v.message || v.restored_from ? [el('div', { className: 'muted' }, note(v))] : [])),
      el('div', { className: 'acts' },
        btn('hist.view', v, () => view(v)), btn('hist.restore', v, () => restore(v)),
        btn('hist.copy', v, () => copy(v)), btn('hist.name', v, () => rename(v))))));
    // Compare any two: newest-first lists, the one before the latest against the latest by default (kept across redraws)
    const order = [...S.vers].sort((x, y) => y.v - x.v);
    $('hist-cmp-bar').hidden = order.length < 2;
    for (const [id, dflt] of [['hist-cmp-a', order[1]], ['hist-cmp-b', order[0]]]) {
      const sel = $(id);
      const keep = order.some((v) => String(v.v) === sel.value) ? sel.value : dflt && String(dflt.v);
      sel.replaceChildren(...order.map((v) => el('option', { value: String(v.v) }, t('hist.version', { v: v.v }) + (v.name ? ' \u00b7 ' + v.name : ''))));
      if (keep) sel.value = keep;
    }
  }

  // ---- Compare (phase 5 WP12): the JSON of docs-diff / sheets-diff (docs/phase5/PLAN.md 6.1, 6.2), drawn as text nodes only ----
  const MAX_ITEMS = 500;
  const colNum = (s) => [...s].reduce((n, c) => n * 26 + c.charCodeAt(0) - 64, 0);
  const colName = (n) => { let s = ''; for (; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s; return s; };
  // [[first, count], ...] as "4, 7\u20139" (rows numbered, columns lettered)
  const runList = (runs, col) => runs.map(([f, n]) => {
    if (n === 1) return String(f);
    return col ? `${f}\u2013${colName(colNum(f) + n - 1)}` : `${f}\u2013${f + n - 1}`;
  }).join(', ');
  const tag = (op, key) => el('span', { className: 'tag op-' + op }, t(key));
  const by = (c) => (c.authors && c.authors.length ? [' ', el('span', { className: 'muted' }, t('hist.cmp.by', { who: c.authors.join(', ') }))] : []);
  const text = (s) => (s ? s : t('hist.cmp.empty'));
  const OPS = { insert: 'hist.cmp.ins', delete: 'hist.cmp.del', modify: 'hist.cmp.mod', move: 'hist.cmp.move', format: 'hist.cmp.fmt' };
  const docItems = (diff) => (diff.changes || []).map((c) => {
    const body = c.op === 'modify'
      ? el('span', { className: 'diff' }, ...c.spans.map((s) => (s.ins != null ? el('ins', {}, s.ins) : s.del != null ? el('del', {}, s.del) : document.createTextNode(s.eq))))
      : el('span', { className: c.op === 'delete' ? 'diff gone' : 'diff' }, text(c.text));
    return el('li', {}, tag(c.op, OPS[c.op] || 'hist.cmp.mod'), ...(c.format && c.op !== 'format' ? [' ', tag('format', 'hist.cmp.fmt')] : []), ' ', body, ...by(c));
  });
  const cellText = (x) => (x == null ? '\u2014' : x.formula ? x.formula + (x.value ? ' = ' + x.value : '') : x.value);
  const sheetItems = (diff) => (diff.sheets || []).map((s) => {
    if (s.op === 'insert') return el('li', {}, tag('insert', 'hist.cmp.ins'), ' ', t('hist.cmp.sheet.ins', { name: s.name }));
    if (s.op === 'delete') return el('li', {}, tag('delete', 'hist.cmp.del'), ' ', t('hist.cmp.sheet.del', { name: s.name }));
    if (s.op === 'rename') return el('li', {}, tag('modify', 'hist.cmp.mod'), ' ', t('hist.cmp.sheet.ren', { from: s.from, name: s.name }));
    const lines = [['rows', 'inserted', 'hist.cmp.rows.ins', false], ['rows', 'deleted', 'hist.cmp.rows.del', false],
      ['cols', 'inserted', 'hist.cmp.cols.ins', true], ['cols', 'deleted', 'hist.cmp.cols.del', true]]
      .filter(([k, w]) => s[k] && s[k][w] && s[k][w].length)
      .map(([k, w, key, col]) => el('div', {}, t(key, { list: runList(s[k][w], col) })));
    const cells = (s.cells || []).slice(0, MAX_ITEMS).map((c) => el('tr', {},
      el('td', {}, c.a && c.b && c.a !== c.b ? `${c.a} \u2192 ${c.b}` : c.b || c.a || ''),
      el('td', {}, cellText(c.old)), el('td', {}, cellText(c.new) + (c.format ? ' ' + t('hist.cmp.fmtflag') : ''))));
    const more = (s.cells || []).length - cells.length + (s.truncated || 0);
    return el('li', { className: 'sheet' }, el('strong', {}, t('hist.cmp.sheet', { name: s.name })), ...lines,
      ...(cells.length ? [el('table', { className: 'cells' },
        el('thead', {}, el('tr', {}, ...['hist.cmp.cell', 'hist.cmp.before', 'hist.cmp.after'].map((k) => aria(el('th', {}, t(k)), { scope: 'col' })))),
        el('tbody', {}, ...cells))] : []),
      ...(more > 0 ? [el('div', { className: 'muted' }, t('hist.cmp.more', { n: more }))] : []));
  });
  function renderCompare() {
    const d = S.cmp;
    if (!d) return;
    $('hist-cmp-sec').hidden = false;
    $('h-cmp').textContent = t('hist.cmp.title', { a: d.a, b: d.b });
    const items = d.app === 'sheets' ? sheetItems(d.diff) : docItems(d.diff);
    $('hist-cmp-none').hidden = items.length > 0;
    $('hist-cmp').replaceChildren(...items.slice(0, MAX_ITEMS),
      ...(items.length > MAX_ITEMS ? [el('li', { className: 'muted' }, t('hist.cmp.more', { n: items.length - MAX_ITEMS }))] : []));
  }
  async function compare() {
    const [a, b] = [+$('hist-cmp-a').value, +$('hist-cmp-b').value];
    if (a === b) { say(t('hist.cmp.same'), true); return; }
    say(t('hist.cmp.busy'));
    // a large workbook takes about a minute to read twice
    const d = await host({ type: 'history_compare', path: S.path, a, b }, 'history_compare', 600000);
    if (fails(d, 'history_compare')) return;
    say('');
    S.cmp = d; renderCompare();
    $('h-cmp').focus();
  }
  function closeCompare() { S.cmp = null; $('hist-cmp-sec').hidden = true; }

  async function loadDocs() {
    const d = await host({ type: 'history' }, 'history');
    S.docs = fails(d, 'history') ? [] : d.items;
    renderDocs();
  }
  async function loadVersions(focus) {
    const p = S.path;
    const d = await host({ type: 'history_versions', path: p }, 'history_versions');
    if (p !== S.path) return;   // another document was chosen meanwhile: its own request renders
    if (d.type === 'rejected') { S.vers = []; $('hist-ver-sec').hidden = true; say(t('hist.nodoc', { name: base(S.path) })); return; }
    if (fails(d, 'history_versions')) return;
    S.vers = d.items; S.verified = d.verified; S.problem = d.problem; renderVersions();
    if (focus) $('h-vers').focus();
  }
  async function choose(path, focus) {
    S.path = path; say(''); closeCompare();
    renderDocs();
    await loadVersions(focus);
  }

  async function view(v) {
    const d = await host({ type: 'history_view', path: S.path, v: v.v }, 'history_view');
    if (!fails(d, 'history_view')) say(t('hist.viewed', { v: v.v, name: d.name }));
  }
  async function copy(v) {
    const d = await host({ type: 'history_copy', path: S.path, v: v.v }, 'history_copy');
    if (!fails(d, 'history_copy')) say(t('hist.copied', { path: d.path }));
  }
  // A native <dialog>: modal, focus trapped, Esc cancels, focus goes back to the button that opened it.
  const ask = (dlg) => new Promise((ok) => { dlg.onclose = () => ok(dlg.returnValue === 'ok'); dlg.returnValue = ''; dlg.showModal(); });
  async function restore(v) {
    $('hist-restore-title').textContent = t('hist.restore.title', { v: v.v });
    $('hist-restore-body').textContent = t('hist.restore.body', { v: v.v });
    if (!(await ask($('hist-restore')))) return;
    const d = await host({ type: 'history_restore', path: S.path, v: v.v }, 'history_restore');
    if (fails(d, 'history_restore')) return;
    say(t('hist.restored', { from: v.v, v: d.version.v }));
    await loadDocs(); await loadVersions(true);
  }
  async function rename(v) {
    $('hist-name-title').textContent = t('hist.name.title', { v: v.v });
    $('hist-name-input').value = v.name || '';
    if (!(await ask($('hist-name')))) return;
    const d = await host({ type: 'history_name', path: S.path, v: v.v, name: $('hist-name-input').value }, 'history_name');
    if (fails(d, 'history_name')) return;
    say(t('hist.named', { v: v.v }));
    await loadVersions(true);
  }

  // ---- the route ----
  const opened = () => !$('history').hidden;
  async function route() {
    const h = location.hash;
    if (!h.startsWith('#history')) {
      if (opened()) { $('history').hidden = true; $(S.was).hidden = false; }
      return;
    }
    if (!opened()) { S.was = $('editor').hidden ? 'home' : 'editor'; $(S.was).hidden = true; $('history').hidden = false; }
    $('hist-web').hidden = desktop; $('hist-body').hidden = !desktop;
    $('hist-title').focus();
    if (!desktop) return;
    if (!S.i18n) S.i18n = await (await fetch('shell/i18n.json')).json();
    let want = '';
    try { want = h.startsWith('#history=') ? decodeURIComponent(h.slice(9)) : ''; } catch (_) { /* a damaged fragment: no file */ }
    await loadDocs();
    if (want) {
      const known = S.docs.find((d) => same(d.path, want));
      await choose(known ? known.path : want);
    }
  }
  const leave = () => { history.replaceState(null, '', location.pathname + location.search); route(); };

  async function init() {
    S.i18n = await (await fetch('shell/i18n.json')).json().catch(() => null);
    $('nav-history').hidden = !desktop;
    $('nav-history').onclick = () => { location.hash = '#history'; };
    $('hist-back').onclick = leave;
    $('hist-cmp-go').onclick = compare;
    $('hist-cmp-close').onclick = () => { closeCompare(); $('hist-cmp-go').focus(); };
    addEventListener('hashchange', route);
    // the host asks for a file's panel (a second launch with --history FILE)
    addEventListener('suite-host', (e) => { const d = e.detail || {}; if (d.type === 'show_history' && typeof d.path === 'string') location.hash = '#history=' + encodeURIComponent(d.path); });
    // shell.js sets the page language after this script ran, and again when the user changes it: redraw the dynamic parts
    new MutationObserver(() => { if (opened()) { renderDocs(); if (S.vers.length) renderVersions(); renderCompare(); } }).observe(document.documentElement, { attributes: true, attributeFilter: ['lang'] });
    route();
  }
  init();
})();
