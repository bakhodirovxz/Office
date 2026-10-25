// Shared by the shell's History panel (history.js, desktop host over IPC) and the document server's wrapper page (services/docserver/static/editor.js,
// server sessions over HTTP): the version rows, the compare selectors and the renderers of the docs-diff / sheets-diff JSON (docs/phase5/PLAN.md 6.1, 6.2).
// Plain JS, text nodes only. HistUI.make(t) -> the helpers; t(key, params) is the caller's string lookup (hist.* keys), the locale of dates is the page's lang.
(() => {
  'use strict';
  const MAX_ITEMS = 500;
  const aria = (e, attrs) => { for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; };
  const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
  const size = (n) => (n < 1048576 ? Math.max(1, Math.round(n / 1024)) + ' KB' : (n / 1048576).toFixed(1) + ' MB');
  const when = (s) => new Date(s * 1000).toLocaleString(document.documentElement.lang);
  const colNum = (s) => [...s].reduce((n, c) => n * 26 + c.charCodeAt(0) - 64, 0);
  const colName = (n) => { let s = ''; for (; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s; return s; };
  // [[first, count], ...] as "4, 7–9" (rows numbered, columns lettered)
  const runList = (runs, col) => runs.map(([f, n]) => {
    if (n === 1) return String(f);
    return col ? `${f}–${colName(colNum(f) + n - 1)}` : `${f}–${f + n - 1}`;
  }).join(', ');
  const OPS = { insert: 'hist.cmp.ins', delete: 'hist.cmp.del', modify: 'hist.cmp.mod', move: 'hist.cmp.move', format: 'hist.cmp.fmt' };

  function make(t) {
    // The store keeps these two messages in English (suite-core history.rs); they are shown in the page language, the stored data is unchanged.
    const note = (v) => {
      const m = /^Restored from version (\d+)$/.exec(v.message || '');
      if (m) return t('hist.restoredtag', { v: m[1] });
      return v.message === 'Before restore' ? t('hist.beforerestore') : v.message || t('hist.restoredtag', { v: v.restored_from });
    };
    // a repeated button text gets the version it belongs to, so a screen reader list tells the buttons apart
    const vbtn = (key, v, fn) => aria(el('button', { type: 'button', className: 'btn ghost', onclick: fn }, t(key)), { 'aria-label': `${t(key)}: ${t('hist.version', { v: v.v })}` });
    // newest first; acts(v) gives the buttons of one row
    const versionRows = (vers, acts) => {
      const newest = Math.max(...vers.map((v) => v.v));
      return [...vers].reverse().map((v) => el('li', {},
        el('div', { className: 'vinfo' },
          el('strong', {}, t('hist.version', { v: v.v })),
          ...(v.v === newest ? [' ', el('span', { className: 'tag' }, t('hist.latest'))] : []),
          ...(v.name ? [' ', el('span', { className: 'tag name' }, v.name)] : []),
          el('div', { className: 'muted' }, t('hist.meta', { time: when(v.time), author: v.author, size: size(v.size), hash: String(v.sha256).slice(0, 8) })),
          ...(v.message || v.restored_from ? [el('div', { className: 'muted' }, note(v))] : [])),
        el('div', { className: 'acts' }, ...acts(v))));
    };
    // Compare any two: newest-first lists, the one before the latest against the latest by default (kept across redraws). True when there are two to compare.
    const fillSelects = (selA, selB, vers) => {
      const order = [...vers].sort((x, y) => y.v - x.v);
      for (const [sel, dflt] of [[selA, order[1]], [selB, order[0]]]) {
        const keep = order.some((v) => String(v.v) === sel.value) ? sel.value : dflt && String(dflt.v);
        sel.replaceChildren(...order.map((v) => el('option', { value: String(v.v) }, t('hist.version', { v: v.v }) + (v.name ? ' · ' + v.name : ''))));
        if (keep) sel.value = keep;
      }
      return order.length >= 2;
    };

    const tag = (op, key) => el('span', { className: 'tag op-' + op }, t(key));
    const by = (c) => (c.authors && c.authors.length ? [' ', el('span', { className: 'muted' }, t('hist.cmp.by', { who: c.authors.join(', ') }))] : []);
    const text = (s) => (s ? s : t('hist.cmp.empty'));
    const docItems = (diff) => (diff.changes || []).map((c) => {
      const body = c.op === 'modify'
        ? el('span', { className: 'diff' }, ...c.spans.map((s) => (s.ins != null ? el('ins', {}, s.ins) : s.del != null ? el('del', {}, s.del) : document.createTextNode(s.eq))))
        : el('span', { className: c.op === 'delete' ? 'diff gone' : 'diff' }, text(c.text));
      return el('li', {}, tag(c.op, OPS[c.op] || 'hist.cmp.mod'), ...(c.format && c.op !== 'format' ? [' ', tag('format', 'hist.cmp.fmt')] : []), ' ', body, ...by(c));
    });
    const cellText = (x) => (x == null ? '—' : x.formula ? x.formula + (x.value ? ' = ' + x.value : '') : x.value);
    const sheetItems = (diff) => (diff.sheets || []).map((s) => {
      if (s.op === 'insert') return el('li', {}, tag('insert', 'hist.cmp.ins'), ' ', t('hist.cmp.sheet.ins', { name: s.name }));
      if (s.op === 'delete') return el('li', {}, tag('delete', 'hist.cmp.del'), ' ', t('hist.cmp.sheet.del', { name: s.name }));
      if (s.op === 'rename') return el('li', {}, tag('modify', 'hist.cmp.mod'), ' ', t('hist.cmp.sheet.ren', { from: s.from, name: s.name }));
      const lines = [['rows', 'inserted', 'hist.cmp.rows.ins', false], ['rows', 'deleted', 'hist.cmp.rows.del', false],
        ['cols', 'inserted', 'hist.cmp.cols.ins', true], ['cols', 'deleted', 'hist.cmp.cols.del', true]]
        .filter(([k, w]) => s[k] && s[k][w] && s[k][w].length)
        .map(([k, w, key, col]) => el('div', {}, t(key, { list: runList(s[k][w], col) })));
      const cells = (s.cells || []).slice(0, MAX_ITEMS).map((c) => el('tr', {},
        el('td', {}, c.a && c.b && c.a !== c.b ? `${c.a} → ${c.b}` : c.b || c.a || ''),
        el('td', {}, cellText(c.old)), el('td', {}, cellText(c.new) + (c.format ? ' ' + t('hist.cmp.fmtflag') : ''))));
      const more = (s.cells || []).length - cells.length + (s.truncated || 0);
      return el('li', { className: 'sheet' }, el('strong', {}, t('hist.cmp.sheet', { name: s.name })), ...lines,
        ...(cells.length ? [el('table', { className: 'cells' },
          el('thead', {}, el('tr', {}, ...['hist.cmp.cell', 'hist.cmp.before', 'hist.cmp.after'].map((k) => aria(el('th', {}, t(k)), { scope: 'col' })))),
          el('tbody', {}, ...cells))] : []),
        ...(more > 0 ? [el('div', { className: 'muted' }, t('hist.cmp.more', { n: more }))] : []));
    });
    // d = {a, b, app, diff}: the list items (at most MAX_ITEMS and a "more" line); an empty array means no differences
    const compareItems = (d) => {
      const items = d.app === 'sheets' ? sheetItems(d.diff) : docItems(d.diff);
      return [...items.slice(0, MAX_ITEMS), ...(items.length > MAX_ITEMS ? [el('li', { className: 'muted' }, t('hist.cmp.more', { n: items.length - MAX_ITEMS }))] : [])];
    };
    return { versionRows, vbtn, fillSelects, compareItems };
  }
  window.HistUI = { make, el, aria, size, when };
})();
