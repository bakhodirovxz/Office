// The accessible command palette (PLAN phase 6 WP11, docs/phase6/a11y-palette.md). A web editor draws on a canvas, so a screen reader sees no
// commands in it; the editor posts its command list over the host bridge (<prefix>:commands) and this dialog (ARIA 1.2 combobox + listbox) shows it.
// Shared by the shell and the docserver wrapper page. make({ t, post, back, say }): t translates, post(type, extra) sends <prefix>:<type> to the
// editor, back() puts focus on the editor again, say(msg) shows an error. Returns { button, open, onMessage(type, data) }.
(() => {
  'use strict';
  const MAX = 200;   // options drawn at once; the filter narrows the rest
  window.Palette = { make({ t, post, back, say }) {
    let dlg, input, list, count, cmds = [], shown = [], act = -1;
    const el = (tag, props = {}, ...kids) => { const e = Object.assign(document.createElement(tag), props); e.append(...kids); return e; };
    const button = el('button', { type: 'button', className: 'btn ghost pal-btn', textContent: t('pal.open'), onclick: () => open() });
    button.dataset.i18n = 'pal.open'; button.setAttribute('aria-haspopup', 'dialog');
    function mark(i) {
      act = i; input.setAttribute('aria-activedescendant', i < 0 ? '' : 'pal-o' + i);
      [...list.children].forEach((o, k) => { o.setAttribute('aria-selected', k === i); if (k === i) o.scrollIntoView({ block: 'nearest' }); });
    }
    function filter() {
      const q = input.value.trim().toLowerCase();
      shown = cmds.filter((c) => !q || (c.label + ' ' + c.shortcut).toLowerCase().includes(q)).sort((a, b) => b.enabled - a.enabled).slice(0, MAX);
      list.replaceChildren(...shown.map((c, i) => {
        const o = el('li', { id: 'pal-o' + i, onclick: () => run(c) }, c.label, ...(c.shortcut ? [' ', el('span', { className: 'muted' }, c.shortcut)] : []));
        o.setAttribute('role', 'option'); o.setAttribute('aria-selected', 'false'); if (!c.enabled) o.setAttribute('aria-disabled', 'true');
        return o;
      }));
      count.textContent = shown.length ? t('pal.count', { n: shown.length }) : t('pal.none');
      mark(shown.findIndex((c) => c.enabled));
    }
    function run(c) {
      if (!c || !c.enabled) return;
      post('run', { id: c.id }); dlg.close();   // the close event hands focus back to the editor
    }
    function make1() {
      input = el('input', { type: 'text', autocomplete: 'off', oninput: filter });
      input.setAttribute('role', 'combobox'); input.setAttribute('aria-expanded', 'true'); input.setAttribute('aria-controls', 'pal-list');
      input.setAttribute('aria-autocomplete', 'list'); input.setAttribute('aria-label', t('pal.search'));
      list = el('ul', { id: 'pal-list', className: 'pal-list' }); list.setAttribute('role', 'listbox'); list.setAttribute('aria-label', t('pal.title'));
      count = el('p', { className: 'muted', role: 'status' });
      dlg = el('dialog', { className: 'pal', onclose: () => back() }, el('h2', { id: 'pal-t' }, t('pal.title')), input, count, list, el('p', { className: 'muted' }, t('pal.hint')));
      dlg.setAttribute('aria-labelledby', 'pal-t');
      input.onkeydown = (e) => {
        const step = { ArrowDown: 1, ArrowUp: -1 }[e.key];
        if (step && shown.length) { e.preventDefault(); mark((act + step + shown.length) % shown.length); }
        else if (e.key === 'Enter') { e.preventDefault(); run(shown[act]); }
      };
      document.body.append(dlg);
    }
    function open() {
      if (!dlg) make1();
      if (dlg.open) return;
      cmds = []; input.value = ''; filter(); count.textContent = t('pal.loading');
      dlg.showModal(); input.focus(); post('commands');
    }
    function onMessage(type, d) {
      if (type === 'commands' && dlg && dlg.open && Array.isArray(d.commands)) {
        cmds = d.commands.map((c) => ({ id: String(c.id), label: String(c.label), shortcut: String(c.shortcut || ''), enabled: !!c.enabled }));
        filter();
      } else if (type === 'ran' && !d.ok) say(t('pal.failed', { error: String(d.error) }));
    }
    return { button, open, onMessage };
  } };
})();
