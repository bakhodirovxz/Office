// The Plugins page of the shell (PLAN phase 7 WP8, section 8). Lazy: the shell fetches this file when the user first presses "Plugins" (a
// desktop-only button; the host does the work over WebView IPC: plugins_list, plugins_install, plugins_enable, plugins_grant; the answer is
// the 'suite-host' event `plugins`). Install, enable and the per-address network grants are the user's choice here; a machine policy can
// switch install and grants off (the answer carries `policy`). Plugin text reaches the DOM only as text nodes. Strings: ./i18n.json (plg.page.*).
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const S = { i18n: null, d: null, msg: '', bad: false, was: 'home', root: null, focus: '', tm: 0 };
  const lang = () => document.documentElement.lang;
  const t = (k, p = {}) => String(((S.i18n || {})[lang()] || {})[k] ?? ((S.i18n || {}).en || {})[k] ?? k).replace(/\{(\w+)\}/g, (m, n) => p[n] ?? m);
  const name = (n) => String((n && (n[lang()] || n.en)) || '');
  const send = (m) => { clearTimeout(S.tm); S.tm = setTimeout(() => { S.bad = true; S.msg = t('plg.page.silent'); render(); }, 20000); (window.ipc || { postMessage: (j) => SuiteHost.plugins(j) }).postMessage(JSON.stringify(m)); };

  function row(it, i, pol) {
    const id = (k, n = '') => `plg-${k}-${i}${n}`;
    const box = (bid, checked, disabled, label, change) => {
      const c = el('input', { type: 'checkbox', id: bid, checked, disabled, onchange: () => { S.focus = bid; change(c.checked); } });
      return el('p', {}, el('label', {}, c, ' ', label));
    };
    const doc = it.document === 'write' ? 'plg.page.doc.write' : it.document === 'read' ? 'plg.page.doc.read' : 'plg.page.doc.none';
    const net = it.network || [];
    return el('li', {},
      el('h3', {}, name(it.name), it.version ? el('span', { className: 'muted' }, ' ' + t('plg.page.version', { version: it.version })) : ''),
      it.problem ? el('p', { className: 'error', role: 'alert' }, t('plg.page.problem', { problem: it.problem })) : '',
      it.publisher ? el('p', { className: 'muted' }, t('plg.page.publisher', { publisher: it.publisher }), ' ', t(doc)) : '',
      it.machine ? el('p', { className: 'muted' }, t('plg.page.machine')) : '',
      it.id ? box(id('en'), !!it.enabled, !!it.problem, t('plg.page.enabled'), (on) => send({ type: 'plugins_enable', id: it.id, enabled: on })) : '',
      ...net.map((o, n) => box(id('net', n), (it.grant || []).includes(o), !it.id || pol.user_grants === false, t('plg.page.net', { origin: o }),
        (on) => send({ type: 'plugins_grant', id: it.id, origin: o, granted: on }))),
      net.length && pol.user_grants === false ? el('p', { className: 'muted' }, t('plg.page.net.blocked')) : '');
  }

  function render() {
    if (!S.root) return;
    const d = S.d || { items: [], policy: {} }, pol = d.policy || {};
    $('plg-title').textContent = t('plg.page.title');
    $('plg-back').textContent = t('plg.page.back');
    $('plg-main').replaceChildren(
      el('p', { id: 'plg-msg', className: S.bad ? 'error' : 'muted', role: S.bad ? 'alert' : 'status' }, S.msg),
      el('p', {}, el('button', { id: 'plg-install', type: 'button', className: 'btn', disabled: pol.user_install === false, onclick: () => { S.focus = 'plg-install'; send({ type: 'plugins_install' }); } }, t('plg.page.install')),
        pol.user_install === false ? el('span', { className: 'muted' }, ' ' + t('plg.page.blocked')) : ''),
      d.items.length ? el('ul', { className: 'list', id: 'plg-list' }, ...d.items.map((it, i) => row(it, i, pol))) : el('p', { className: 'muted' }, t('plg.page.none')));
    const f = $(S.focus);
    if (f && !f.disabled) f.focus();
  }

  function build() {
    S.root = el('div', { id: 'plugins-page', hidden: true },
      el('header', { className: 'bar' }, el('button', { id: 'plg-back', type: 'button', className: 'btn', onclick: close }),
        el('div', { id: 'plg-title', role: 'heading', tabIndex: -1 })),
      el('main', { id: 'plg-main' }));
    S.root.querySelector('#plg-title').setAttribute('aria-level', '1');
    S.root.setAttribute('role', 'region'); S.root.setAttribute('aria-labelledby', 'plg-title');
    S.root.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !document.querySelector('dialog[open]')) close(); });
    document.body.append(S.root);
    addEventListener('suite-host', (e) => {
      const d = e.detail || {};
      if (d.type !== 'plugins') return;
      clearTimeout(S.tm);
      if (d.cancelled) return;
      if (d.items) S.d = d;
      const made = d.installed && d.items && d.items.find((i) => i.id === d.installed);
      S.bad = d.ok === false;
      S.msg = S.bad ? t('plg.page.error', { error: String(d.error) }) : d.restart && !made ? t('plg.page.restart') : made ? t('plg.page.installed', { name: name(made.name) }) : '';
      render();
    });
    // the page language is set by shell.js after this script ran, and again when the user changes it
    new MutationObserver(() => { if (!S.root.hidden) render(); }).observe(document.documentElement, { attributes: true, attributeFilter: ['lang'] });
  }

  function close() { S.root.hidden = true; $(S.was).hidden = false; $('nav-plugins').focus(); }

  async function open() {
    if (!S.i18n) S.i18n = await (await fetch('shell/plugins/i18n.json')).json().catch(() => ({}));
    if (!S.root) build();
    if (!S.root.hidden) return;
    S.was = $('editor').hidden ? 'home' : 'editor';
    $(S.was).hidden = true; S.root.hidden = false;
    S.msg = ''; S.bad = false; S.focus = '';
    render();
    $('plg-title').focus();
    send({ type: 'plugins_list' });
  }

  window.PluginsPage = { open };
  open();
})();
