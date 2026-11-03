// The plugin page host (PLAN phase 7 WP5, docs/plugins.md). Lazy: the shell and the docserver wrapper load this file only when a plugin
// is enabled. One Worker per plugin (worker.js, own CSP `default-src 'none'`); this page does what the Worker cannot: the wire format, the
// time budget (`terminate`), the grant check and the fetch, the DOM panel, and the bridge to the editor (`<prefix>:plugins`, `pluginCall`,
// `pluginActions`, `pluginResults`, `pluginEvent`, PLAN 4.6). Plugin text only ever reaches the DOM through textContent.
//
//   PluginHost.make(S, say, root) -> { onMessage(f, d), invoke(id, command, params) }
//     S    { cfg: {plugins: [descriptor], pluginBudgetMs?}, lang, app?, onAudit?(x) } (the shell's state object; the wrapper passes the same shape)
//     say  (text) shows one status line;  root: the element the panel goes into (position: relative)
//     descriptor { id, version, sha256, wasm: same-origin URL, manifest: plugin.toml as JSON, grant?: [origins] (server mode: admin grants, no prompt),
//                  settings? }
//     f    { app: {id, prefix}, el: {contentWindow} }: the editor frame a message came from
(() => {
  'use strict';
  const BASE = document.currentScript.src.replace(/[^/]*$/, '');
  const MAX_MSG = 32 << 20, MAX_HEADER = 1 << 20, MAX_BLOBS = 16, MAX_ROUNDS = 16, SLOTS = 64;
  const TEXT = 1024, WIDGETS = 64, ITEMS = 256, STATUS = 256, FETCH_BODY = 1 << 20;
  const EDITOR_MS = 10000, LOAD_MS = 10000, FETCH_MS = 10000;
  const enc = new TextEncoder(), dec = new TextDecoder('utf-8', { fatal: true });
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const store = (k, v) => { try { return v === undefined ? localStorage.getItem(k) : localStorage.setItem(k, v); } catch (_) { return null; } };
  // control, invisible and bidi-override characters never reach the screen (the same set as plugin-api's `spoof`)
  const SPOOF = new RegExp('[\u0000-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2066-\u2069\ufeff]', 'g');
  const SPOOF_NL = new RegExp('[\u0000-\u0009\u000b-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2066-\u2069\ufeff]', 'g');
  const clean = (s, nl) => [...String(s ?? '').replace(nl ? SPOOF_NL : SPOOF, '')].slice(0, TEXT).join('');
  const loc = (m, lang) => clean((m && (m[lang] || m.en)) || '');
  const hex = (b) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
  const u8 = (v) => (v instanceof Uint8Array ? v : v instanceof ArrayBuffer ? new Uint8Array(v) : enc.encode(JSON.stringify(v ?? null)));

  // ---- the wire format (crates/plugin-api/src/wire.rs): u32 header_len, header (UTF-8 JSON), u32 blob_count, per blob u32 len + bytes; little endian ----
  function encode(header, blobs) {
    const h = enc.encode(JSON.stringify(header));
    if (h.length > MAX_HEADER || blobs.length > MAX_BLOBS) throw new Error('the message is too large');
    const total = 8 + h.length + blobs.reduce((s, b) => s + 4 + b.length, 0);
    if (total > MAX_MSG) throw new Error('the message is over 32 MiB');
    const out = new Uint8Array(total), dv = new DataView(out.buffer);
    let i = 0;
    dv.setUint32(i, h.length, true); out.set(h, i += 4); i += h.length;
    dv.setUint32(i, blobs.length, true); i += 4;
    for (const b of blobs) { dv.setUint32(i, b.length, true); out.set(b, i += 4); i += b.length; }
    return out.buffer;
  }
  function decode(buf) {
    const u = new Uint8Array(buf), dv = new DataView(buf);
    let i = 0;
    const need = (n) => { if (n > u.length - i) throw new Error('the answer is truncated'); };
    if (u.length > MAX_MSG) throw new Error('the answer is over 32 MiB');
    need(4); const hl = dv.getUint32(i, true); i += 4;
    if (hl > MAX_HEADER) throw new Error('the answer header is over 1 MiB');
    need(hl); const header = JSON.parse(dec.decode(u.subarray(i, i + hl))); i += hl;   // the fatal decoder refuses invalid UTF-8
    need(4); const n = dv.getUint32(i, true); i += 4;
    if (n > MAX_BLOBS) throw new Error('the answer has more than 16 blobs');
    const blobs = [];
    for (let k = 0; k < n; k++) { need(4); const l = dv.getUint32(i, true); i += 4; need(l); blobs.push(u.slice(i, i + l)); i += l; }
    if (i !== u.length) throw new Error('bytes after the end of the answer');
    return { header, blobs };
  }

  // ---- the answer (strict, like plugin-api's Answer: an unknown action or key refuses the whole answer) ----
  const KEYS = { run: ['id', 'params'], 'textmap.apply': ['blob'], read: ['what', 'scope', 'index'], fetch: ['url', 'method', 'headers', 'body'], 'ui.panel': ['panel', 'widgets'], 'ui.status': ['text'] };
  const WKEYS = { heading: ['text'], text: ['text'], list: ['items'], button: ['id', 'label'], textField: ['id', 'value', 'placeholder'], select: ['id', 'options', 'value'] };
  const isObj = (o) => o && typeof o === 'object' && !Array.isArray(o);
  const only = (o, keys, what) => { for (const k of Object.keys(o)) if (k !== 'type' && !keys.includes(k)) throw new Error(`unknown key ${k} in ${what}`); };
  const wid = (s) => { if (typeof s !== 'string' || !/^[A-Za-z0-9_-]{1,48}$/.test(s)) throw new Error('bad widget id'); return s; };
  function checkAnswer(a, P, fromEvent) {
    if (!isObj(a)) throw new Error('the answer is not an object');
    only(a, ['actions', 'next'], 'the answer');
    const acts = a.actions ?? [];
    if (!Array.isArray(acts) || acts.length > 256) throw new Error('bad actions');
    const doc = P.m.permissions?.document || 'none';
    for (const x of acts) {
      if (!isObj(x) || !KEYS[x.type] || (x.type === 'run' && typeof x.id !== 'string')) throw new Error('unknown action ' + (isObj(x) ? x.type : ''));
      only(x, KEYS[x.type], x.type);
      if (fromEvent && !x.type.startsWith('ui.')) throw new Error('an event may only run ui actions');
      const needs = x.type === 'run' ? (/^document\./.test(x.id) ? 'read' : 'write') : x.type === 'textmap.apply' ? 'write' : x.type === 'read' ? 'read' : '';
      if (needs && (doc === 'none' || (needs === 'write' && doc !== 'write'))) throw new Error(`${x.type} needs document access`);
      if (x.type === 'ui.panel' && !(P.m.panel || []).some((p) => p.id === x.panel)) throw new Error('unknown panel');
      if (x.type === 'ui.panel') { if (!Array.isArray(x.widgets)) throw new Error('bad widgets'); x.widgets.forEach((w) => { if (!isObj(w) || !WKEYS[w.type]) throw new Error('unknown widget'); only(w, WKEYS[w.type], 'widget ' + w.type); }); }
    }
    if (a.next !== undefined) { only(a.next, ['reads'], 'next'); if (!Array.isArray(a.next.reads) || a.next.reads.length > MAX_BLOBS) throw new Error('bad next'); }
    return { actions: acts, next: a.next };
  }

  // ---- fetch (PLAN 5.3): the origin is exactly one the manifest lists and a grant covers; the page does it, credentials omitted, no redirects ----
  const FORBIDDEN = ['host', 'cookie', 'content-length', 'connection', 'transfer-encoding', 'origin', 'referer', 'upgrade', 'te', 'expect'];
  function originOf(url) {
    if (typeof url !== 'string' || url.length > 2048 || new RegExp(SPOOF.source).test(url) || /[\s\\]/.test(url) || !url.startsWith('https://')) return null;
    const rest = url.slice(8), end = rest.search(/[/?#]/), host = end < 0 ? rest : rest.slice(0, end);
    return host.includes('@') || !host ? null : 'https://' + host;
  }
  function checkFetch(a) {
    const method = a.method ?? 'GET', h = a.headers ?? {};
    if (!['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].includes(method)) throw new Error('method not allowed');
    if (!isObj(h) || Object.keys(h).length > 16 || (a.body != null && (typeof a.body !== 'string' || a.body.length > FETCH_BODY))) throw new Error('bad headers or body');
    for (const [k, v] of Object.entries(h)) {
      const l = k.toLowerCase();
      if (!/^[A-Za-z0-9-]+$/.test(k) || FORBIDDEN.includes(l) || l.startsWith('proxy-') || l.startsWith('sec-') || typeof v !== 'string' || v.length > 4096 || /[\u0000-\u001f\u007f]/.test(v)) throw new Error('a header is not allowed');
    }
    return { method, headers: h, body: a.body ?? undefined };
  }

  function make(S, say, root) {
    // what the host did about the network and failures: kept here, and handed to S.onAudit (the docserver wrapper reports it to the server's audit log)
    const note = (x) => { H.audit.push(x); try { if (S.onAudit) S.onAudit(x); } catch (_) { /* a broken sink never stops a plugin */ } };
    const H = { logs: [], audit: [], mem: {}, plugins: [], onMessage, invoke: (id, cmd, params) => start(H.plugins.find((p) => p.m.id === id), { kind: 'command', command: cmd, params: params || {} }) };
    PluginHost.last = H;
    const budget = () => Math.min(5000, Number(S.cfg.pluginBudgetMs) || 5000);   // an admin may lower it, never raise it
    const lang = () => S.lang || 'en';
    let i18n = null, seq = 0, slots = 0, panel = null, shown = null;
    const t = (k, p = {}) => String((i18n && ((i18n[lang()] || {})[k] ?? (i18n.en || {})[k])) ?? k).replace(/\{(\w+)\}/g, (m, n) => p[n] ?? m);
    const ready = fetch(BASE + 'i18n.json', { credentials: 'omit' }).then((r) => r.json()).then((j) => { i18n = j; }, () => {});
    document.head.append(el('link', { rel: 'stylesheet', href: BASE + 'host.css' }));
    const app = (f) => f.app.id;

    for (const d of S.cfg.plugins || []) {
      try {
        const m = d.manifest;
        if (!isObj(m) || typeof d.id !== 'string' || m.id !== d.id || m.api !== 1 || !/^[0-9a-f]{64}$/.test(d.sha256) || new URL(d.wasm, location.href).origin !== location.origin) throw new Error('bad descriptor');
        if (!Array.isArray(m.apps) || (m.command || []).length > 16) throw new Error('bad manifest');
        const P = { d, m, name: loc(m.name, 'en') || d.id, bytes: null, w: null, pending: null, q: Promise.resolve(), cmds: [], win: null, settings: d.settings ?? m.settings ?? {} };
        for (const c of m.command || []) { if (slots >= SLOTS) break; P.cmds.push({ slot: slots++, c }); }
        H.plugins.push(P);
      } catch (e) { say(t('plg.refused', { name: String(d && d.id), error: e.message })); }
    }

    // ---- the Worker ----
    const stop = (P, why) => {
      if (P.w) { P.w.terminate(); P.w = null; }
      const p = P.pending; P.pending = null;
      if (p) clearTimeout(p.tm);
      if (p) p.bad(Object.assign(new Error(why || 'stopped'), { stopped: true }));
    };
    async function ensure(P) {
      if (P.w) return P.w;
      if (!P.bytes) {
        const r = await fetch(P.d.wasm, { credentials: 'omit' });
        if (!r.ok) throw new Error('the plugin file could not be fetched: ' + r.status);
        const b = await r.arrayBuffer();
        if (b.byteLength > 4 << 20) throw new Error('the plugin file is over 4 MiB');
        if (!crypto.subtle || hex(await crypto.subtle.digest('SHA-256', b)) !== P.d.sha256) throw new Error('the plugin file does not match its checksum');
        P.bytes = b;
      }
      const w = new Worker(BASE + 'worker.js');
      P.w = w;
      w.onmessage = (e) => {
        const m = e.data, p = P.pending;
        if (m.type === 'log') { H.logs.push({ plugin: P.d.id, text: String(m.text).slice(0, 2048) }); if (H.logs.length > 200) H.logs.shift(); return; }
        if (!p || (m.id !== undefined && m.id !== p.id)) return;
        P.pending = null; clearTimeout(p.tm);
        if (m.type === 'error') p.bad(new Error(m.error));
        else if (m.type === 'loaded') p.ok();
        else { H.mem[P.d.id] = m.mem; try { p.ok(decode(m.out)); } catch (err) { p.bad(err); } }
      };
      w.onerror = (e) => { e.preventDefault(); const p = P.pending; P.pending = null; if (p) { clearTimeout(p.tm); p.bad(new Error(e.message || 'the plugin failed')); } };
      const copy = P.bytes.slice(0);
      await new Promise((ok, bad) => {
        const tm = setTimeout(() => stop(P, 'load timed out'), LOAD_MS);
        P.pending = { id: undefined, ok, bad, tm };
        w.postMessage({ type: 'load', bytes: copy, maxPages: (Number(P.m.permissions?.memory_mb) || 16) * 16 }, [copy]);
      }).catch((e) => { stop(P); throw e; });
      return w;
    }
    // one round: the time budget runs from the post until the answer; when it is over the Worker is terminated and the next call starts a new one
    async function round(P, header, blobs) {
      const w = await ensure(P), msg = encode(header, blobs), id = ++seq;
      return new Promise((ok, bad) => {
        const tm = setTimeout(() => stop(P, 'time budget'), budget());
        P.pending = { id, ok, bad, tm };
        w.postMessage({ type: 'call', id, msg }, [msg]);
      });
    }

    // ---- the editor bridge ----
    const waits = new Map();
    function toEditor(P, type, body, transfer) { P.win.postMessage({ type: P.pre + ':' + type, ...body }, location.origin, transfer || []); }
    function ask(P, call, body) {
      if (!P.win) return Promise.reject(new Error('no editor to run the actions'));
      return new Promise((ok, bad) => {
        const tm = setTimeout(() => { waits.delete(call); bad(new Error('the editor did not answer')); }, EDITOR_MS);
        waits.set(call, (d) => { clearTimeout(tm); waits.delete(call); d.error ? bad(new Error(String(d.error))) : ok(d); });
        toEditor(P, 'pluginActions', { call, plugin: P.d.id, ...body }, body.blobs);
      });
    }

    // ---- grants (local mode: ask once per origin, remember it in this browser; server mode: only the descriptor's list) ----
    const grants = () => { try { return JSON.parse(store('plg.grants') || '{}'); } catch (_) { return {}; } };
    const granted = (P, o) => (P.d.grant ? P.d.grant.includes(o) : (grants()[P.d.id] || []).includes(o));
    async function grant(P, o) {
      if (granted(P, o)) return true;
      if (P.d.grant) return false;   // server mode: the admin's list is final
      await ready;
      const dlg = el('dialog', { className: 'plg-dlg' });
      const ok = await new Promise((done) => {
        dlg.setAttribute('aria-labelledby', 'plg-gt');
        dlg.append(el('h2', { id: 'plg-gt' }, t('plg.grant.title', { name: P.name, origin: o.slice(8) })), el('p', {}, t('plg.grant.body', { publisher: clean(P.m.publisher) })),
          el('p', {}, el('button', { type: 'button', className: 'btn', onclick: () => { dlg.returnValue = 'ok'; dlg.close(); } }, t('plg.grant.allow')),
            ' ', el('button', { type: 'button', className: 'btn ghost', onclick: () => dlg.close() }, t('plg.grant.deny'))));
        dlg.onclose = () => done(dlg.returnValue === 'ok');
        document.body.append(dlg); dlg.showModal();
      });
      dlg.remove();
      if (ok) { const g = grants(); (g[P.d.id] ||= []).push(o); store('plg.grants', JSON.stringify(g)); }
      return ok;
    }
    async function doFetch(P, a) {
      const o = originOf(a.url);
      if (!o || !(P.m.permissions?.network || []).includes(o)) { note({ plugin: P.d.id, kind: 'network.refused', origin: o }); return { error: 'network not permitted' }; }
      let req;
      try { req = checkFetch(a); } catch (e) { return { error: e.message }; }
      if (!(await grant(P, o))) { note({ plugin: P.d.id, kind: 'network.denied', origin: o }); return { error: 'network not permitted' }; }
      note({ plugin: P.d.id, kind: 'network', origin: o });
      try {
        const r = await fetch(a.url, { ...req, credentials: 'omit', redirect: 'manual', cache: 'no-store', referrerPolicy: 'no-referrer', signal: AbortSignal.timeout(FETCH_MS) });
        if (r.type === 'opaqueredirect') return { error: 'a redirect was refused' };
        const buf = await r.arrayBuffer();
        return { status: r.status, body: new TextDecoder().decode(buf.slice(0, FETCH_BODY)) };
      } catch (e) { return { error: 'the request failed' }; }
    }

    // ---- the panel (DOM, plain text only; a side panel, a bottom sheet on a phone) ----
    function close() { if (panel) { panel.remove(); panel = null; shown = null; root.classList.remove('plg-on'); } }
    function render(P, id, widgets) {
      const first = !panel || shown !== P.d.id + '/' + id;
      if (first) {
        close();
        const title = loc((P.m.panel || []).find((p) => p.id === id)?.title, lang()) || id, hid = 'plg-h-' + (++seq);
        panel = el('aside', { className: 'plg-panel' });
        panel.setAttribute('role', 'region'); panel.setAttribute('aria-labelledby', hid);
        panel.append(el('div', { className: 'plg-head' }, el('h2', { id: hid, tabIndex: -1 }, `${P.name}: ${title}`),
          el('button', { type: 'button', className: 'btn ghost', onclick: close }, t('plg.close'))), el('div', { className: 'plg-body' }));
        panel.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
        root.append(panel); root.classList.add('plg-on'); shown = P.d.id + '/' + id;
        panel.querySelector('h2').focus();
      }
      const ui = (widget, value) => start(P, { kind: 'ui', widget, params: value === undefined ? {} : { value } });
      const kids = widgets.slice(0, WIDGETS).map((w) => {
        switch (w.type) {
          case 'heading': return el('h3', {}, clean(w.text));
          case 'text': return el('p', { className: 'plg-text' }, clean(w.text, true));
          case 'list': return el('ul', {}, ...(Array.isArray(w.items) ? w.items : []).slice(0, ITEMS).map((i) => el('li', {}, i.id ? el('button', { type: 'button', className: 'btn ghost', onclick: () => ui(wid(i.id)) }, clean(i.text)) : clean(i.text))));
          case 'button': return el('button', { type: 'button', className: 'btn', onclick: () => ui(wid(w.id)) }, clean(w.label));
          case 'textField': { const i = el('input', { type: 'text', value: clean(w.value), maxLength: TEXT, onchange: () => ui(wid(w.id), i.value) }); i.setAttribute('aria-label', clean(w.placeholder) || w.id); i.placeholder = clean(w.placeholder); return i; }
          case 'select': { const s = el('select', { onchange: () => ui(wid(w.id), s.value) }, ...(Array.isArray(w.options) ? w.options : []).slice(0, ITEMS).map((o) => el('option', { value: clean(o.value), selected: o.value === w.value }, clean(o.label)))); s.setAttribute('aria-label', w.id); return s; }
        }
      });
      panel.querySelector('.plg-body').replaceChildren(...kids);
    }

    // ---- one invocation: rounds of (plugin call, actions) until no `next` or 16 rounds; all editor actions of it are one undo step on the editor side ----
    async function invokeNow(P, head, reads, call) {
      const fromEvent = head.kind === 'event';
      let blobs = [], idx = {}, results = [], sent = false;
      const add = (name, v) => { idx[name] = blobs.push(u8(v)) - 1; };
      for (const [k, v] of Object.entries(reads || {})) add(k, v);
      try {
        for (let r = 0; ; r++) {
          if (r >= MAX_ROUNDS) throw new Error('more than 16 rounds');
          const h = { kind: r ? 'resume' : head.kind, api: 1, api_minor: 0, app: P.app, lang: lang(), ...(head.command && { command: head.command }), ...(head.widget && { widget: head.widget }),
            ...(head.event && { event: head.event }), params: head.params || {}, settings: P.settings, ...(results.length && { results }), ...(Object.keys(idx).length && { reads: idx }) };
          const ans = await round(P, h, blobs);
          const a = checkAnswer(ans.header, P, fromEvent);
          results = [];
          const fwd = [];
          for (const x of a.actions) {
            if (x.type === 'ui.status') { say(clean(x.text).slice(0, STATUS)); results.push({ ok: true }); }
            else if (x.type === 'ui.panel') { await ready; render(P, x.panel, x.widgets); results.push({ ok: true }); }
            else if (x.type === 'fetch') results.push(await doFetch(P, x));
            else { fwd.push(results.length); results.push(null); }
          }
          const wantReads = a.next ? a.next.reads : [];
          blobs = []; idx = {};
          if (fwd.length || wantReads.length || (sent && !a.next) || (!a.next && call.fromEditor)) {
            // the editor runs `run`, `textmap.apply`, `read`; textmap.apply names a blob of this answer by its index
            const out = await ask(P, call.id, { actions: fwd.map((i) => a.actions[i]), blobs: ans.blobs.map((b) => b.buffer), reads: wantReads, more: !!a.next });
            sent = true;
            fwd.forEach((i, k) => { results[i] = (out.results || [])[k] ?? { error: 'no result' }; });
            wantReads.forEach((q, k) => { const v = (out.reads || [])[k]; if (v !== undefined && v !== null) add(q.what, v); });
          }
          if (!a.next) return;
        }
      } catch (e) {
        if (sent || call.fromEditor) { try { toEditor(P, 'pluginActions', { call: call.id, actions: [], more: false, abort: true }); } catch (_) { /* no editor */ } }
        await ready;
        say(e.stopped ? t('plg.timeout', { name: P.name }) : t('plg.failed', { name: P.name, error: clean(e.message) }));
        note({ plugin: P.d.id, kind: e.stopped ? 'stopped' : 'failed', error: String(e.message) });
      }
    }
    // invocations of one plugin never overlap
    function start(P, head, reads, editorCall) {
      if (!P) return Promise.resolve();
      const call = { id: editorCall ?? 'h' + (++seq), fromEditor: editorCall !== undefined };
      return (P.q = P.q.then(() => invokeNow(P, head, reads, call)));
    }

    // ---- messages from an editor frame ----
    function register(f) {
      const P = H.plugins.filter((p) => p.m.apps.includes(app(f)));
      if (!P.length) return;
      const L = lang(), commands = [], toolbar = [], menu = [];
      for (const p of P) {
        p.win = f.el.contentWindow; p.app = app(f); p.pre = f.app.prefix;
        for (const { slot, c } of p.cmds) {
          const id = `plugin.${p.m.id}.${c.id}`;
          commands.push({ slot, id, label: loc(c.label, L), mutates: p.m.permissions?.document === 'write', reads: c.reads || [], params: c.params || {} });
          for (const b of (p.m.toolbar || []).filter((x) => x.command === c.id)) toolbar.push({ slot, icon: b.icon || '', group: loc(b.group, L) });
          for (const b of (p.m.menu || []).filter((x) => x.command === c.id)) menu.push({ slot, place: b.place, group: loc(b.group, L) });
        }
      }
      toEditor(P[0], 'plugins', { commands, toolbar, menu, plugins: P.map((p) => ({ manifest: p.m, settings: p.settings })) });   // the editor builds its slot commands from the manifests
    }
    function onMessage(f, d) {
      const kind = d.type.slice(f.app.prefix.length + 1);
      if (kind === 'ready') return register(f);
      if (kind === 'pluginCall') {
        const P = H.plugins.find((p) => p.m.id === d.plugin && p.m.apps.includes(app(f)));
        const c = P && P.cmds.find((x) => x.c.id === d.command);
        if (!c) return;
        P.win = f.el.contentWindow; P.app = app(f); P.pre = f.app.prefix;
        start(P, { kind: 'command', command: d.command, params: isObj(d.params) ? d.params : c.c.params || {} }, d.reads, d.call);
      } else if (kind === 'pluginResults') {
        const w = waits.get(d.call);
        if (w) w(d);
      } else if (kind === 'pluginEvent' && typeof d.event === 'string') {
        for (const P of H.plugins) if (P.win === f.el.contentWindow && (P.m.events || []).includes(d.event)) start(P, { kind: 'event', event: d.event, params: isObj(d.data) ? d.data : {} });
      }
    }
    // frames that said ready before this file loaded
    for (const f of Object.values(S.frames || {})) if (f.ready) register(f);
    return H;
  }
  window.PluginHost = { make };
})();
