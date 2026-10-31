/*
 * The wrapper page /ds/editor (PLAN phase 5, 3.1). It sits between the host page (api.js) and one editor, both in iframes of
 * different origins, and does what neither may: it takes the host's config, opens a session at /ds/session, keeps the bearer
 * session token in this page's memory only (no cookie, no URL, no storage), and carries the document bytes and the saves
 * between /ds/session/... and the editor's `?host=parent` postMessage bridge.
 *
 * postMessage rules (S14): the host's messages count only when they come from window.parent at the origin api.js named in the
 * URL hash; the editor's only from its own iframe at this origin. The server checks the host's origin again at /ds/session.
 */
(function () {
  'use strict';
  // the shared theme: ?theme=light|dark from the shell, else the OS (embedded in a host with no setting); the host's uiTheme replaces it in start()
  var th = new URLSearchParams(location.search).get('theme'), mq = window.matchMedia('(prefers-color-scheme: dark)');
  function applyTheme() { document.documentElement.dataset.theme = th === 'dark' || (th !== 'light' && mq.matches) ? 'dark' : 'light'; }
  applyTheme();
  mq.addEventListener('change', applyTheme);
  var HOST = null;
  try {
    var m = /^#o=(.+)$/.exec(location.hash);
    if (m) { var u = new URL(decodeURIComponent(m[1])); if (/^https?:$/.test(u.protocol)) HOST = u.origin; }
  } catch (e) { /* no host origin: nothing will be accepted */ }
  if (!HOST || window.parent === window) return;

  var S = { sid: null, token: null, timer: 0, prefix: null, frame: null, ready: false, started: false, begun: false, closed: false, closing: null, dirtySent: false, readOnly: false, i18n: {}, lang: 'en', title: 'document', fileType: '' };
  var status = document.createElement('p');
  status.style.cssText = 'margin:0;padding:1em;font:14px system-ui,sans-serif;color:var(--muted)';
  document.body.style.cssText = 'margin:0;height:100vh;overflow:hidden;display:flex;flex-direction:column';
  document.body.appendChild(status);
  var t = function (k, p) { return String((S.i18n[S.lang] || {})[k] || (S.i18n.en || {})[k] || k).replace(/\{(\w+)\}/g, function (a, n) { return (p || {})[n] == null ? a : p[n]; }); };

  function emit(event, data) { window.parent.postMessage({ type: 'docs:event', event: event, data: data }, HOST); }
  function fail(code, description) {
    status.textContent = t('err.failed', { error: description });
    status.style.display = 'block';
    emit('onError', { errorCode: code, errorDescription: description });
  }
  async function refusal(r) {
    var e = new Error('HTTP ' + r.status);
    e.status = r.status;
    try { var j = (await r.json()).error; e.code = j.code; e.message += ': ' + j.message; } catch (x) { /* not our JSON */ }
    return e;
  }
  // 401 and 403 -> -20 (the ONLYOFFICE code for a rejected token), no document -> -4, anything else -> -1
  function codeOf(e) { return e.status === 401 || e.status === 403 ? -20 : e.code === 'no_document' ? -4 : -1; }

  async function api(path, opt) {
    var headers = Object.assign({}, (opt || {}).headers, { Authorization: 'Bearer ' + S.token });
    var r = await fetch('/ds/session/' + S.sid + path, Object.assign({}, opt, { headers: headers, credentials: 'omit', cache: 'no-store' }));
    if (!r.ok) throw await refusal(r);
    return r;
  }

  function renewSoon(expires) {
    clearTimeout(S.timer);
    // session tokens live 15 min: renew at 10, sooner when the session itself ends sooner
    var wait = Math.max(5, Math.min(600, expires - Date.now() / 1000 - 60));
    S.timer = setTimeout(async function () {
      if (S.closed) return;
      try { var r = await (await api('/renew', { method: 'POST' })).json(); S.token = r.token; renewSoon(r.expires); }
      catch (e) { fail(codeOf(e), 'the session could not be renewed'); }
    }, wait * 1000);
  }

  // the first call of a session makes the server fetch the file from the host and answers when it is stored
  async function getDocument() { return (await api('/document')).arrayBuffer(); }

  function toEditor(type, extra, transfer) {
    S.frame.contentWindow.postMessage(Object.assign({ type: S.prefix + ':' + type }, extra), location.origin, transfer || []);
  }

  // sends the session's newest version to the editor: at start, and after a restore (else the editor's next save would overwrite it)
  async function openInEditor() {
    var bytes = await getDocument();
    var ext = '.' + S.fileType;
    toEditor('open', { name: S.title.toLowerCase().endsWith(ext) ? S.title : S.title + ext, bytes: bytes, readOnly: S.readOnly, canDownload: S.canDownload, canPrint: S.canPrint }, [bytes]);
  }

  async function onEditor(d) {
    switch (d.type.slice(S.prefix.length + 1)) {
      case 'ready':
        if (S.started) return;
        S.started = true;
        emit('onAppReady', {});
        try { await openInEditor(); } catch (e) { fail(codeOf(e), 'the document could not be loaded'); }
        break;
      case 'opened': status.style.display = 'none'; S.ready = true; emit('onDocumentReady', {}); break;
      case 'dirty':
        emit('onDocumentStateChange', !!d.dirty);
        // the first change of the session: the host learns of it once (callback status 1); best effort, the server sends it once
        if (d.dirty && !S.dirtySent) { S.dirtySent = true; api('/dirty', { method: 'POST' }).catch(function () { S.dirtySent = false; }); }
        break;
      case 'failed': fail(-1, String(d.error)); break;
      case 'save':
        // every save is answered with exactly one `saved`, in order (the editor keeps the document dirty until then)
        var ok = false;
        try {
          if (!(d.bytes instanceof ArrayBuffer)) throw new Error('no bytes');
          var r = await (await api('/save', { method: 'POST', body: d.bytes })).json();
          // saved means the host acknowledged it: "failed" and "unknown" (no answer in time) are not, and the document stays dirty
          ok = r.callback === 'ok' && !r.dirty;
          if (ok) emit('onSaved', r);
          else {
            emit('onWarning', { warningCode: 1, warningDescription: 'the host did not accept the saved document' });
            emit('onDocumentStateChange', true);
          }
        } catch (e) { fail(codeOf(e), 'the document could not be saved'); }
        toEditor('saved', { ok: ok });
        break;
      case 'autosave':
        if (!(d.bytes instanceof ArrayBuffer)) return;
        try { await api('/autosave', { method: 'POST', body: d.bytes }); }
        catch (e) { emit('onWarning', { warningCode: 2, warningDescription: 'the recovery copy could not be stored' }); }
        break;
    }
  }

  // ---- the History panel (PLAN 6.4): the list, download, restore, name and compare of this session's document, over /ds/session/{sid}/history.
  // The token is in this page only, so the panel lives here; the rows and the diff drawing are shared with the shell (/shell/history-ui.js, history.css).
  // Controls the session's permissions do not allow are not drawn (edit: restore, name; download: download, compare); the server refuses them anyway.
  var H = { panel: null, vers: [], opener: null, ui: null };
  var $h = function (id) { return document.getElementById(id); };
  function hsay(msg, bad) { var e = $h('hist-msg'); e.textContent = msg; e.className = bad ? 'error' : 'muted'; e.setAttribute('role', bad ? 'alert' : 'status'); }
  async function hfail(fn) { try { return await fn(); } catch (e) { if (H.panel) hsay(e.message, true); } }
  // A native <dialog>: modal, Esc cancels, focus returns to the button that opened it.
  function ask(dlg) { return new Promise(function (ok) { dlg.onclose = function () { ok(dlg.returnValue === 'ok'); }; dlg.returnValue = ''; dlg.showModal(); }); }
  function dialog(id, title, body, okKey) {
    var E = HistUI.el, d = E('dialog', { id: id });
    d.setAttribute('aria-labelledby', id + '-t');
    var form = E('form', { method: 'dialog' }, E('h2', { id: id + '-t' }, title));
    if (body) form.append(E('p', {}, body));
    form.append(E('p', {}, E('button', { type: 'submit', value: 'ok', className: 'btn' }, t(okKey)), ' ', E('button', { type: 'submit', value: '', className: 'btn ghost' }, t('hist.cancel'))));
    d.append(form);
    return d;
  }
  async function hload() {
    H.vers = (await (await api('/history')).json()).versions;
    var ui = H.ui, can = S.perms.download;
    $h('hist-vers').replaceChildren.apply($h('hist-vers'), ui.versionRows(H.vers, function (v) {
      return [].concat(
        can ? [ui.vbtn('hist.copy', v, function () { hfail(function () { return hdownload(v); }); })] : [],
        S.perms.edit ? [ui.vbtn('hist.restore', v, function () { hrestore(v); }), ui.vbtn('hist.name', v, function () { hname(v); })] : []);
    }));
    if (can) $h('hist-cmp-bar').hidden = !ui.fillSelects($h('hist-cmp-a'), $h('hist-cmp-b'), H.vers);
  }
  async function hdownload(v) {
    var blob = await (await api('/history/' + v.v)).blob();
    var a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = S.title.replace(/\.[^./\\]+$/, '') + '-v' + v.v + (S.fileType ? '.' + S.fileType : '');
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(function () { URL.revokeObjectURL(a.href); }, 10000);
  }
  async function hrestore(v) {
    var d = dialog('hist-restore', t('hist.restore.title', { v: v.v }), t('hist.restore.body', { v: v.v }), 'hist.restore.ok');
    H.panel.append(d);
    var yes = await ask(d); d.remove();
    if (!yes) return;
    hfail(async function () {
      var r = await (await api('/history/' + v.v + '/restore', { method: 'POST' })).json();
      await openInEditor();
      hsay(t('hist.restored', { from: v.v, v: r.version.v }));
      await hload();
    });
  }
  async function hname(v) {
    var d = dialog('hist-name', t('hist.name.title', { v: v.v }), '', 'hist.name.ok');
    var input = HistUI.el('input', { type: 'text', maxLength: 200, autocomplete: 'off', value: v.name || '' });
    d.querySelector('h2').after(HistUI.el('p', {}, HistUI.el('label', {}, t('hist.name.label'), HistUI.el('br'), input)));
    H.panel.append(d);
    var yes = await ask(d); d.remove();
    if (!yes) return;
    hfail(async function () {
      await api('/history/' + v.v + '/name', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: input.value }) });
      hsay(t('hist.named', { v: v.v }));
      await hload();
    });
  }
  // the compare route's error codes (history.rs) in the user's language; any other refusal keeps the server's text
  var CMP_ERR = { not_configured: 'hist.cmp.err.missing', not_supported: 'hist.cmp.err.kind', compare_timeout: 'hist.cmp.err.timeout',
    compare_too_large: 'hist.cmp.err.large', compare_failed: 'hist.cmp.err.failed' };
  async function hcompare() {
    var a = +$h('hist-cmp-a').value, b = +$h('hist-cmp-b').value;
    if (a === b) return hsay(t('hist.cmp.same'), true);
    hsay(t('hist.cmp.busy'));
    hfail(async function () {
      var r;
      try { r = await api('/history/' + a + '/compare/' + b); }
      catch (e) { if (CMP_ERR[e.code]) e.message = t(CMP_ERR[e.code]); throw e; }
      var d = await r.json();
      var items = H.ui.compareItems(d);
      hsay('');
      $h('hist-cmp-sec').hidden = false;
      $h('h-cmp').textContent = t('hist.cmp.title', { a: d.a, b: d.b });
      $h('hist-cmp-none').hidden = items.length > 0;
      $h('hist-cmp').replaceChildren.apply($h('hist-cmp'), items);
      $h('h-cmp').focus();
    });
  }
  function hclose() {
    if (!H.panel) return;
    H.panel.remove(); H.panel = null;
    H.opener.setAttribute('aria-expanded', 'false');
    H.opener.focus();
  }
  async function hopen() {
    if (H.panel) return hclose();
    var E = HistUI.el, ui = H.ui = HistUI.make(t), can = S.perms.download;
    var sel = function (id) { return E('select', { id: id }); };
    var label = function (key, s) { return E('label', {}, E('span', {}, t(key)), ' ', s); };
    var h1 = E('h2', { id: 'hist-h', tabIndex: -1 }, t('hist.title'));
    h1.style.cssText = 'margin:0;flex:1 1 auto;font-size:1.25rem;outline:none';
    var head = E('div', {}, h1, E('button', { type: 'button', className: 'btn ghost', onclick: hclose }, t('hist.close')));
    head.style.cssText = 'display:flex;align-items:center;gap:12px';
    var cmpSec = E('section', { id: 'hist-cmp-sec', hidden: true },
      E('h2', { id: 'h-cmp', tabIndex: -1 }), E('p', { id: 'hist-cmp-none', hidden: true, className: 'muted' }, t('hist.cmp.none')),
      E('ul', { id: 'hist-cmp', className: 'list cmp' }),
      E('p', {}, E('button', { type: 'button', className: 'btn ghost', onclick: function () { cmpSec.hidden = true; $h('hist-cmp-go').focus(); } }, t('hist.cmp.close'))));
    H.panel = E('aside', { id: 'hist-panel', onkeydown: function (e) { if (e.key === 'Escape' && !document.querySelector('dialog[open]')) { e.preventDefault(); hclose(); } } },
      head, E('p', { id: 'hist-msg', className: 'muted', role: 'status' }),
      ...(can ? [E('div', { id: 'hist-cmp-bar', className: 'cmpbar', hidden: true }, label('hist.cmp.from', sel('hist-cmp-a')), label('hist.cmp.to', sel('hist-cmp-b')),
        E('button', { id: 'hist-cmp-go', type: 'button', className: 'btn', onclick: hcompare }, t('hist.compare')))] : []),
      E('ul', { id: 'hist-vers', className: 'list vers' }), ...(can ? [cmpSec] : []));
    H.panel.setAttribute('role', 'dialog');
    H.panel.setAttribute('aria-labelledby', 'hist-h');
    H.panel.style.cssText = 'position:fixed;top:0;right:0;bottom:0;width:min(36rem,100%);overflow:auto;padding:12px 16px;background:var(--bg);color:var(--fg);border-left:1px solid var(--line);box-shadow:-4px 0 12px var(--shadow,rgba(0,0,0,.25));z-index:10';
    document.body.appendChild(H.panel);
    H.opener.setAttribute('aria-expanded', 'true');
    h1.focus();
    hfail(hload);
  }
  function addHistoryButton() {
    if (!window.HistUI) return;   // the shell files are not served: no panel
    var b = H.opener = document.createElement('button');
    b.type = 'button'; b.className = 'btn ghost'; b.textContent = t('hist.nav');
    b.setAttribute('aria-expanded', 'false'); b.setAttribute('aria-controls', 'hist-panel');
    b.style.cssText = 'padding:2px 10px;font-size:.85rem';
    b.onclick = hopen;
    var bar = document.createElement('div');
    bar.style.cssText = 'flex:0 0 auto;display:flex;justify-content:flex-end;padding:2px 8px;background:var(--bg);border-bottom:1px solid var(--line)';
    bar.appendChild(b);
    document.body.insertBefore(bar, S.frame);
  }

  async function closeSession() {
    // asked twice (close(), then destroyEditor()): the second asker gets the first answer
    if (S.closed) return S.closing.then(emit.bind(null, 'onClosed'));
    S.closed = true;
    clearTimeout(S.timer);
    // before the session exists there is nothing to end
    S.closing = (S.sid ? api('/close', { method: 'POST' }).then(function (r) { return r.json(); }) : Promise.resolve({ status: 4 })).catch(function () { return { status: 0 }; });
    var c = await S.closing;
    // a close the host did not acknowledge is queued by the server and retried; the host page is told it is not saved yet
    if (c.callback === 'failed' || c.callback === 'unknown' || c.dirty) emit('onWarning', { warningCode: 1, warningDescription: 'the host did not accept the saved document' });
    emit('onClosed', c);
  }

  async function start(cfg, origin) {
    var get = function (p) { return fetch(p, { credentials: 'omit' }).then(function (r) { return r.ok ? r.json() : null; }).catch(function () { return null; }); };
    var res = await Promise.all([get('/shell-config.json'), get('/shell/i18n.json')]);
    var apps = (res[0] && res[0].apps) || [];
    S.i18n = res[1] || {};
    var lang = String((cfg.editorConfig && cfg.editorConfig.lang) || navigator.language || 'en').toLowerCase();
    S.lang = lang.startsWith('ru') ? 'ru' : lang.startsWith('uz') ? (lang.includes('cyrl') ? 'uz-Cyrl' : 'uz-Latn') : 'en';
    document.documentElement.lang = S.lang;
    // editorConfig.customization.uiTheme, ONLYOFFICE names (theme-dark, theme-classic-light, default-dark, ...): wins over ?theme= and the OS; any other name keeps them
    // theme-white and theme-night are the public defaults for light and dark without the suffix
    var un = String((cfg.editorConfig && cfg.editorConfig.customization && cfg.editorConfig.customization.uiTheme) || '');
    var ui = un === 'theme-white' ? 'light' : un === 'theme-night' ? 'dark' : (/-(dark|light)$/.exec(un) || [])[1];
    if (ui) { th = ui; applyTheme(); }
    var s;
    try {
      var r = await fetch('/ds/session', { method: 'POST', credentials: 'omit', cache: 'no-store', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ origin: origin, config: cfg }) });
      if (!r.ok) throw await refusal(r);
      s = await r.json();
    } catch (e) { return fail(codeOf(e), (e.code === 'bad_config' ? 'the configuration was refused' : 'the session was refused') + ' (' + e.message + ')'); }
    S.perms = s.perms || {};
    S.readOnly = !S.perms.edit;
    // permissions.download / print hide the editor's own Save As, export and Print entries (as ONLYOFFICE hides its buttons); the browser's own print cannot be blocked.
    S.canDownload = !(s.perms && s.perms.download === false); S.canPrint = !(s.perms && s.perms.print === false); S.sid = s.sid; S.token = s.token; S.title = s.title; S.fileType = String((cfg.document && cfg.document.fileType) || '').toLowerCase();
    renewSoon(s.expires);
    var app = apps.find(function (a) { return a.route === '/' + s.editor + '/'; });
    if (!app) return fail(-1, 'no editor for this document type');
    S.prefix = app.prefix;
    document.documentElement.dataset.app = s.editor;   // history.css: the host app's look
    status.textContent = t('ed.loading', { app: app.names[S.lang] || app.names.en });
    S.frame = document.createElement('iframe');
    S.frame.src = app.route + '?host=parent&theme=' + (th === 'dark' || th === 'light' ? th : 'system');   // read once by the editor at start
    S.frame.title = s.title;
    S.frame.allow = 'clipboard-read; clipboard-write; fullscreen';
    S.frame.style.cssText = 'border:0;display:block;width:100%;flex:1 1 auto;min-height:0';
    document.body.appendChild(S.frame);
    addHistoryButton();
  }

  window.addEventListener('message', function (ev) {
    var d = ev.data;
    if (!d || typeof d.type !== 'string') return;
    if (ev.source === window.parent) {
      if (ev.origin !== HOST) return;
      if (d.type === 'docs:config' && !S.begun && d.config && typeof d.config === 'object') { S.begun = true; start(d.config, ev.origin); }
      else if (d.type === 'docs:requestSave' && S.ready) toEditor('requestSave', {});
      else if (d.type === 'docs:close') closeSession();
    } else if (S.frame && ev.source === S.frame.contentWindow && ev.origin === location.origin && d.type.indexOf(S.prefix + ':') === 0) {
      onEditor(d);
    }
  });
  window.parent.postMessage({ type: 'docs:ready' }, HOST);
})();
