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
  var HOST = null;
  try {
    var m = /^#o=(.+)$/.exec(location.hash);
    if (m) { var u = new URL(decodeURIComponent(m[1])); if (/^https?:$/.test(u.protocol)) HOST = u.origin; }
  } catch (e) { /* no host origin: nothing will be accepted */ }
  if (!HOST || window.parent === window) return;

  var S = { sid: null, token: null, timer: 0, prefix: null, frame: null, ready: false, started: false, begun: false, closed: false, i18n: {}, lang: 'en', title: 'document', fileType: '' };
  var status = document.createElement('p');
  status.style.cssText = 'margin:0;padding:1em;font:14px system-ui,sans-serif;color:#555';
  document.body.style.cssText = 'margin:0;height:100vh;overflow:hidden';
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
    try { e.code = (await r.json()).error; } catch (x) { /* not JSON */ }
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

  async function getDocument() {
    // the server may still be fetching the file from the host: a few short retries on "not loaded yet"
    for (var i = 0; ; i++) {
      try { return await (await api('/document')).arrayBuffer(); }
      catch (e) { if (e.code !== 'no_document' || i >= 20) throw e; await new Promise(function (ok) { setTimeout(ok, 250); }); }
    }
  }

  function toEditor(type, extra, transfer) {
    S.frame.contentWindow.postMessage(Object.assign({ type: S.prefix + ':' + type }, extra), location.origin, transfer || []);
  }

  async function onEditor(d) {
    switch (d.type.slice(S.prefix.length + 1)) {
      case 'ready':
        if (S.started) return;
        S.started = true;
        emit('onAppReady', {});
        try {
          var bytes = await getDocument();
          var ext = '.' + S.fileType;
          toEditor('open', { name: S.title.toLowerCase().endsWith(ext) ? S.title : S.title + ext, bytes: bytes }, [bytes]);
        } catch (e) { fail(codeOf(e), 'the document could not be loaded'); }
        break;
      case 'opened': status.style.display = 'none'; S.ready = true; emit('onDocumentReady', {}); break;
      case 'dirty': emit('onDocumentStateChange', !!d.dirty); break;
      case 'failed': fail(-1, String(d.error)); break;
      case 'save':
        if (!(d.bytes instanceof ArrayBuffer)) return;
        try {
          var r = await (await api('/save', { method: 'POST', body: d.bytes })).json();
          emit('onSaved', r);
          // "failed" comes with the host callback (WP4); "unknown" is a save the server holds but the host has not been told of
          if (r.callback === 'failed') emit('onWarning', { warningCode: 1, warningDescription: 'the host did not accept the saved document' });
        } catch (e) { fail(codeOf(e), 'the document could not be saved'); }
        break;
      case 'autosave':
        if (!(d.bytes instanceof ArrayBuffer)) return;
        try { await api('/autosave', { method: 'POST', body: d.bytes }); }
        catch (e) { emit('onWarning', { warningCode: 2, warningDescription: 'the recovery copy could not be stored' }); }
        break;
    }
  }

  async function closeSession() {
    if (S.closed) return;
    S.closed = true;
    clearTimeout(S.timer);
    // before the session exists there is nothing to end
    try { emit('onClosed', S.sid ? await (await api('/close', { method: 'POST' })).json() : { status: 4 }); }
    catch (e) { emit('onClosed', { status: 0 }); }
  }

  async function start(cfg, origin) {
    var get = function (p) { return fetch(p, { credentials: 'omit' }).then(function (r) { return r.ok ? r.json() : null; }).catch(function () { return null; }); };
    var res = await Promise.all([get('/shell-config.json'), get('/shell/i18n.json')]);
    var apps = (res[0] && res[0].apps) || [];
    S.i18n = res[1] || {};
    var lang = String((cfg.editorConfig && cfg.editorConfig.lang) || navigator.language || 'en').toLowerCase();
    S.lang = lang.startsWith('ru') ? 'ru' : lang.startsWith('uz') ? (lang.includes('cyrl') ? 'uz-Cyrl' : 'uz-Latn') : 'en';
    var s;
    try {
      var r = await fetch('/ds/session', { method: 'POST', credentials: 'omit', cache: 'no-store', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ origin: origin, config: cfg }) });
      if (!r.ok) throw await refusal(r);
      s = await r.json();
    } catch (e) { return fail(codeOf(e), e.code === 'bad_config' ? 'the configuration was refused' : 'the session was refused'); }
    S.sid = s.sid; S.token = s.token; S.title = s.title; S.fileType = String((cfg.document && cfg.document.fileType) || '').toLowerCase();
    renewSoon(s.expires);
    var app = apps.find(function (a) { return a.route === '/' + s.editor + '/'; });
    if (!app) return fail(-1, 'no editor for this document type');
    S.prefix = app.prefix;
    status.textContent = t('ed.loading', { app: app.names[S.lang] || app.names.en });
    S.frame = document.createElement('iframe');
    S.frame.src = app.route + '?host=parent';
    S.frame.title = s.title;
    S.frame.allow = 'clipboard-read; clipboard-write; fullscreen';
    S.frame.style.cssText = 'border:0;display:block;width:100%;height:100%';
    document.body.appendChild(S.frame);
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
