// Spike 3 / approach B: canvas tiles + hidden-textarea IME bridge over wordcraft-bind (wasm).
// Pages are <canvas> tiles inside a native overflow:auto scroller: browser scroll, momentum and pinch apply.
(function () {
  'use strict';
  const q = new URLSearchParams(location.search);
  const TILE = 1024; // device pixels per tile side
  const $ = (id) => document.getElementById(id);
  const scroller = $('scroller'), pagesEl = $('pages'), hud = $('hud');
  const perf = (window.__perf = { load: {}, tiles: [], edits: [], frames: [], inputTypes: [], events: [], save: null });
  const log = (...a) => { perf.events.push([Math.round(performance.now()), ...a]); if (perf.events.length > 300) perf.events.shift(); };

  let ed = null, docName = q.get('doc') || 'long.docx';
  let dpr = window.devicePixelRatio || 1, pinch = 1, zoom = 1;
  const ZOOMS = [1, 1.5, 2, 0.75];
  let cssPerPt = 1, pages = [], rev = 0, queue = [], pumping = false, lastScroll = 0;
  let ime = null, composing = false, mirror = '', baseline = '', lastSel = [0, 0], syncing = false, suppressClick = false;

  // ---------------------------------------------------------------- pages and tiles
  function buildPages(force) {
    const n = ed.page_count();
    const fit = Math.max(100, scroller.clientWidth - 16);
    const p0 = ed.page_size_pt(0);
    const cpp = (fit / p0[0]) * zoom;
    const sizes = [];
    for (let i = 0; i < n; i++) sizes.push(Array.from(ed.page_size_pt(i)));
    const same = !force && n === pages.length && Math.abs(cpp - cssPerPt) < 1e-6 && sizes.every((s, i) => s[0] === pages[i].w && s[1] === pages[i].h);
    if (same) return false;
    for (const p of pages) p.el.remove();
    cssPerPt = cpp;
    pages = sizes.map((s, i) => {
      const el = document.createElement('div');
      el.className = 'page'; el.dataset.i = i;
      el.style.width = s[0] * cpp + 'px'; el.style.height = s[1] * cpp + 'px';
      const ov = document.createElement('div'); ov.className = 'ov'; el.appendChild(ov);
      pagesEl.appendChild(el);
      return { i, w: s[0], h: s[1], el, ov, tiles: new Map() };
    });
    if (ime) pagesEl.appendChild(ime);
    return true;
  }
  const rasterScale = () => Math.min(cssPerPt * dpr * pinch, 12);
  function visRect() {
    const sr = scroller.getBoundingClientRect(), vv = window.visualViewport;
    let l = sr.left, t = sr.top, r = sr.right, b = sr.bottom;
    if (vv) { l = Math.max(l, vv.offsetLeft); t = Math.max(t, vv.offsetTop); r = Math.min(r, vv.offsetLeft + vv.width); b = Math.min(b, vv.offsetTop + vv.height); }
    return { l, t, r, b };
  }
  function needTiles(margin) {
    const v = visRect(), rs = rasterScale(), tcss = TILE / (dpr * pinch), out = [];
    for (const p of pages) {
      const r = p.el.getBoundingClientRect();
      const x0 = Math.max(v.l - margin, r.left), x1 = Math.min(v.r + margin, r.right);
      const y0 = Math.max(v.t - margin, r.top), y1 = Math.min(v.b + margin, r.bottom);
      if (x1 <= x0 || y1 <= y0) continue;
      const wpx = Math.ceil(p.w * rs), hpx = Math.ceil(p.h * rs);
      const tx0 = Math.floor(((x0 - r.left) / tcss) * 1), tx1 = Math.floor(((x1 - r.left - 0.01) / tcss));
      const ty0 = Math.floor((y0 - r.top) / tcss), ty1 = Math.floor((y1 - r.top - 0.01) / tcss);
      for (let ty = Math.max(0, ty0); ty <= ty1 && ty * TILE < hpx; ty++)
        for (let tx = Math.max(0, tx0); tx <= tx1 && tx * TILE < wpx; tx++)
          out.push({ p, tx, ty, d: Math.abs((r.top + (ty + 0.5) * tcss) - (v.t + v.b) / 2) });
    }
    return out;
  }
  function updateTiles() {
    const rs = rasterScale();
    const want = needTiles(80);
    const keep = new Set(needTiles(900).map((t) => t.p.i + ':' + t.tx + ':' + t.ty));
    for (const p of pages) for (const [k, t] of p.tiles) if (!keep.has(p.i + ':' + k)) { t.canvas.remove(); p.tiles.delete(k); }
    want.sort((a, b) => a.d - b.d);
    for (const w of want) {
      const key = w.tx + ':' + w.ty, t = w.p.tiles.get(key);
      if (!t || t.rev !== rev || t.rs !== rs) enqueue(w.p, w.tx, w.ty);
    }
    pump();
  }
  function enqueue(p, tx, ty) {
    if (!queue.some((j) => j.p === p && j.tx === tx && j.ty === ty)) queue.push({ p, tx, ty });
  }
  function renderTile(p, tx, ty) {
    const rs = rasterScale();
    const wpx = Math.ceil(p.w * rs), hpx = Math.ceil(p.h * rs);
    const w = Math.min(TILE, wpx - tx * TILE), h = Math.min(TILE, hpx - ty * TILE);
    if (w <= 0 || h <= 0) return 0;
    const t0 = performance.now();
    const px = ed.render_tile(p.i, rs, tx * TILE, ty * TILE, w, h);
    const t1 = performance.now();
    const key = tx + ':' + ty;
    let t = p.tiles.get(key);
    if (!t) {
      const c = document.createElement('canvas');
      c.style.left = (tx * TILE) / (dpr * pinch) + 'px'; c.style.top = (ty * TILE) / (dpr * pinch) + 'px';
      p.el.insertBefore(c, p.ov);
      t = { canvas: c }; p.tiles.set(key, t);
    }
    t.canvas.width = w; t.canvas.height = h;
    t.canvas.style.width = w / (dpr * pinch) + 'px'; t.canvas.style.height = h / (dpr * pinch) + 'px';
    t.canvas.getContext('2d').putImageData(new ImageData(px, w, h), 0, 0);
    t.rev = rev; t.rs = rs;
    const t2 = performance.now();
    perf.tiles.push({ page: p.i, w, h, rs: +rs.toFixed(3), render_ms: +(t1 - t0).toFixed(1), put_ms: +(t2 - t1).toFixed(1) });
    if (perf.tiles.length > 400) perf.tiles.shift();
    return t2 - t0;
  }
  function pump() {
    if (pumping) return;
    pumping = true;
    const step = () => {
      const j = queue.shift();
      if (!j) { pumping = false; return; }
      renderTile(j.p, j.tx, j.ty);
      setTimeout(step, 0);
    };
    setTimeout(step, 0);
  }
  let rafUpd = 0;
  function scheduleUpdate() { if (!rafUpd) rafUpd = requestAnimationFrame(() => { rafUpd = 0; updateTiles(); }); }

  // ---------------------------------------------------------------- overlays (caret, selection, preedit)
  let preEl = null, preText = '';
  function updateOverlay() {
    for (const p of pages) p.ov.textContent = '';
    if (!ed) return;
    const s = ed.selection_rects();
    for (let i = 0; i + 4 < s.length; i += 5) {
      const p = pages[s[i]]; if (!p) continue;
      const d = document.createElement('div'); d.className = 'sel';
      d.style.cssText = `left:${s[i + 1] * cssPerPt}px;top:${s[i + 2] * cssPerPt}px;width:${s[i + 3] * cssPerPt}px;height:${s[i + 4] * cssPerPt}px`;
      p.ov.appendChild(d);
    }
    const c = ed.caret();
    if (c.length) {
      const p = pages[c[0]];
      if (p) {
        if (s.length === 0) {
          const d = document.createElement('div'); d.className = 'caret';
          d.style.cssText = `left:${c[1] * cssPerPt}px;top:${c[2] * cssPerPt}px;width:${Math.max(2, c[4] * cssPerPt)}px;height:${c[3] * cssPerPt}px`;
          p.ov.appendChild(d);
        }
        if (preText) {
          const d = document.createElement('div'); d.className = 'pre'; d.textContent = preText;
          d.style.cssText = `left:${c[1] * cssPerPt}px;top:${c[2] * cssPerPt}px;height:${c[3] * cssPerPt}px;line-height:${c[3] * cssPerPt}px;font-size:${c[3] * cssPerPt * 0.8}px`;
          p.ov.appendChild(d);
        }
        placeIme(p, c);
      }
    }
  }
  function placeIme(p, c) {
    if (!ime) return;
    if (!p) { c = ed.caret(); if (!c.length) return; p = pages[c[0]]; }
    const rp = p.el.getBoundingClientRect(), rc = pagesEl.getBoundingClientRect();
    ime.style.left = rp.left - rc.left + c[1] * cssPerPt + 'px';
    ime.style.top = rp.top - rc.top + c[2] * cssPerPt + 'px';
    ime.style.height = Math.max(16, c[3] * cssPerPt) + 'px';
  }
  function ensureCaretVisible() {
    const c = ed.caret(); if (!c.length || !pages[c[0]]) return;
    const rp = pages[c[0]].el.getBoundingClientRect(), sr = scroller.getBoundingClientRect();
    const top = rp.top + c[2] * cssPerPt, bot = top + c[3] * cssPerPt;
    if (bot > sr.bottom - 24) scroller.scrollTop += bot - sr.bottom + 60;
    else if (top < sr.top + 8) scroller.scrollTop -= sr.top - top + 60;
  }

  // ---------------------------------------------------------------- after an engine mutation
  function afterEdit(evTs, t0, engineMs) {
    rev++;
    const rebuilt = buildPages(false);
    updateOverlay();
    ensureCaretVisible();
    // first repaint: the tile(s) under the caret, synchronously; the rest of the visible tiles are queued.
    const c = ed.caret(), rs = rasterScale(), tcss = TILE / (dpr * pinch);
    let first = 0;
    if (c.length && pages[c[0]]) {
      const p = pages[c[0]];
      const tx = Math.floor((c[1] * cssPerPt) / tcss), ty = Math.floor(((c[2] + c[3] / 2) * cssPerPt) / tcss);
      first = renderTile(p, tx, ty) || 0;
    }
    const tFirst = performance.now() - t0;
    queue = [];
    updateTiles();
    requestAnimationFrame(() => {
      perf.edits.push({ engine_ms: +engineMs.toFixed(1), first_tile_ms: +first.toFixed(1), handler_to_first_paint_ms: +tFirst.toFixed(1), event_to_raf_ms: +(performance.now() - evTs).toFixed(1), rebuilt });
      if (perf.edits.length > 300) perf.edits.shift();
    });
  }
  function runCmd(id, params) {
    const ts = performance.now(), t0 = ts;
    try { ed.run(id, params || '{}'); } catch (e) { log('cmd-error', id, String(e)); }
    afterEdit(ts, t0, performance.now() - t0);
    syncIme(true);
    updateButtons();
  }

  // ---------------------------------------------------------------- IME bridge
  function syncIme(force) {
    if (!ime || composing) return;
    syncing = true;
    const c = ed.text_context(300, 300);
    const text = c.text, a = c.sel_start, b = c.sel_end;
    if (ime.value !== text) ime.value = text;
    if (ime.selectionStart !== a || ime.selectionEnd !== b) ime.setSelectionRange(a, b);
    mirror = text; lastSel = [a, b];
    syncing = false;
  }
  // The edit that turns `oldV` into `newV`, anchored at the new caret when the tail after it is unchanged.
  function diff(oldV, newV, caret) {
    const tail = newV.length - caret;
    let s = 0;
    if (tail >= 0 && tail <= oldV.length && oldV.endsWith(newV.slice(caret))) s = tail;
    else { while (s < Math.min(oldV.length, newV.length) && oldV[oldV.length - 1 - s] === newV[newV.length - 1 - s]) s++; }
    const max = Math.min(oldV.length, newV.length) - s;
    let p = 0;
    while (p < max && oldV[p] === newV[p]) p++;
    if (p > 0 && p < oldV.length && /[\ud800-\udbff]/.test(oldV[p - 1])) p--;
    return { from: p, to: oldV.length - s, text: newV.slice(p, newV.length - s) };
  }
  function commitDiff(e) {
    const nv = ime.value;
    if (nv === mirror) return;
    const evTs = e && e.timeStamp ? e.timeStamp : performance.now(), t0 = performance.now();
    const d = diff(mirror, nv, ime.selectionEnd);
    try { ed.replace_utf16(d.from, d.to, d.text); } catch (er) { log('replace-error', String(er)); }
    const engineMs = performance.now() - t0;
    log('commit', d.from, d.to, JSON.stringify(d.text));
    afterEdit(evTs, t0, engineMs);
    syncIme(true);
    updateButtons();
  }
  function setPreedit(text) { if (preText !== text) { preText = text; updateOverlay(); } }
  function setupIme() {
    ime = document.createElement('textarea');
    ime.id = 'ime'; ime.setAttribute('autocapitalize', 'sentences'); ime.setAttribute('autocomplete', 'off');
    ime.setAttribute('autocorrect', 'on'); ime.setAttribute('spellcheck', 'true'); ime.setAttribute('enterkeyhint', 'enter'); ime.setAttribute('wrap', 'off');
    ime.rows = 1;
    pagesEl.appendChild(ime);
    ime.addEventListener('beforeinput', (e) => { perf.inputTypes.push(e.inputType + (e.data ? ':' + e.data : '')); if (perf.inputTypes.length > 200) perf.inputTypes.shift(); });
    ime.addEventListener('compositionstart', () => { composing = true; baseline = mirror; log('compstart'); });
    ime.addEventListener('compositionupdate', (e) => { log('compupd', e.data); });
    ime.addEventListener('input', (e) => {
      if (composing) { const d = diff(baseline, ime.value, ime.selectionEnd); setPreedit(d.text); return; }
      commitDiff(e);
    });
    ime.addEventListener('compositionend', (e) => {
      composing = false; log('compend', e.data); setPreedit('');
      mirror = baseline; commitDiff(e);
    });
    ime.addEventListener('focus', () => { syncIme(true); updateOverlay(); });
    document.addEventListener('selectionchange', () => {
      if (document.activeElement !== ime || composing || syncing || ime.value !== mirror) return;
      const a = ime.selectionStart, b = ime.selectionEnd;
      if (a === lastSel[0] && b === lastSel[1]) return;
      lastSel = [a, b];
      try { ed.select_utf16(a, b); } catch (er) { log('sel-error', String(er)); }
      updateOverlay();
    });
    ime.addEventListener('keydown', (e) => {
      const k = e.key, mod = e.ctrlKey || e.metaKey;
      if (composing || e.isComposing) return;
      if (k === 'ArrowUp' || k === 'ArrowDown') { e.preventDefault(); runCmd(k === 'ArrowUp' ? 'caret.up' : 'caret.down', e.shiftKey ? '{"extend":true}' : '{}'); }
      else if (mod && k.toLowerCase() === 'z') { e.preventDefault(); runCmd('edit.undo'); }
      else if (mod && k.toLowerCase() === 'y') { e.preventDefault(); runCmd('edit.redo'); }
      else if (mod && k.toLowerCase() === 'b') { e.preventDefault(); runCmd('format.bold'); }
      else if (mod && k.toLowerCase() === 's') { e.preventDefault(); save(); }
    });
  }

  // ---------------------------------------------------------------- pointer
  function pageAt(e) {
    const el = e.target.closest && e.target.closest('.page');
    if (!el) return null;
    const r = el.getBoundingClientRect(), i = +el.dataset.i;
    return { i, x: (e.clientX - r.left) / cssPerPt, y: (e.clientY - r.top) / cssPerPt };
  }
  function focusIme() { ime.focus({ preventScroll: true }); }
  function setupPointer() {
    scroller.addEventListener('click', (e) => {
      if (suppressClick) { suppressClick = false; return; }
      const h = pageAt(e); if (!h) return;
      const t0 = performance.now();
      try { ed.tap(h.i, h.x, h.y, e.shiftKey); } catch (er) { log('tap-error', String(er)); return; }
      perf.tap_ms = +(performance.now() - t0).toFixed(1);
      focusIme(); syncIme(true); updateOverlay(); updateButtons();
    });
    let lp = 0, sx = 0, sy = 0;
    scroller.addEventListener('touchstart', (e) => {
      clearTimeout(lp);
      if (e.touches.length !== 1) return;
      const t = e.touches[0]; sx = t.clientX; sy = t.clientY;
      lp = setTimeout(() => {
        const h = pageAt({ target: document.elementFromPoint(sx, sy), clientX: sx, clientY: sy }); if (!h) return;
        try { ed.tap(h.i, h.x, h.y, false); ed.select_word(); } catch (er) { log('lp-error', String(er)); return; }
        suppressClick = true; log('longpress', ed.selected_text());
        focusIme(); syncIme(true); updateOverlay(); updateButtons();
      }, 480);
    }, { passive: true });
    const cancel = () => clearTimeout(lp);
    scroller.addEventListener('touchmove', (e) => { const t = e.touches[0]; if (Math.abs(t.clientX - sx) + Math.abs(t.clientY - sy) > 10) cancel(); }, { passive: true });
    scroller.addEventListener('touchend', cancel); scroller.addEventListener('touchcancel', cancel);
    scroller.addEventListener('contextmenu', (e) => e.preventDefault());
    scroller.addEventListener('scroll', () => { lastScroll = performance.now(); scheduleUpdate(); }, { passive: true });
    if (window.visualViewport) {
      let t = 0;
      const vv = window.visualViewport;
      vv.addEventListener('scroll', scheduleUpdate);
      vv.addEventListener('resize', () => {
        scheduleUpdate(); clearTimeout(t);
        t = setTimeout(() => {
          const s = Math.max(1, +vv.scale.toFixed(2));
          if (Math.abs(s - pinch) > 0.05) { pinch = s; log('pinch', s); for (const p of pages) { for (const [, tl] of p.tiles) tl.canvas.remove(); p.tiles.clear(); } queue = []; updateTiles(); }
        }, 300);
      });
    }
    let lastW = scroller.clientWidth;
    window.addEventListener('resize', () => {
      if (scroller.clientWidth !== lastW) { lastW = scroller.clientWidth; rebuildAll(); } else { scheduleUpdate(); ensureCaretVisible(); }
    });
  }

  // ---------------------------------------------------------------- toolbar
  function updateButtons() {
    $('bBold').classList.toggle('on', ed.bold_at_caret());
    $('bUndo').disabled = !ed.can_undo();
    $('bZoom').textContent = 'Zoom ' + Math.round(zoom * 100) + '%';
  }
  function rebuildAll() {
    const ratio = scroller.scrollHeight ? scroller.scrollTop / scroller.scrollHeight : 0;
    queue = []; buildPages(true); scroller.scrollTop = ratio * scroller.scrollHeight; updateOverlay(); updateTiles();
  }
  function save() {
    const t0 = performance.now();
    const bytes = ed.save(docName.replace(/\.[^.]+$/, '') + '-edited.docx');
    const ms = performance.now() - t0;
    const name = docName.replace(/\.[^.]+$/, '') + '-edited.docx';
    const blob = new Blob([bytes], { type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' });
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = name; document.body.appendChild(a); a.click(); a.remove();
    fetch('/__save?name=' + encodeURIComponent(name), { method: 'POST', body: bytes }).catch(() => {});
    perf.save = { bytes: bytes.length, ms: +ms.toFixed(1), name };
    log('save', bytes.length);
    updateButtons();
  }
  function setupToolbar() {
    for (const id of ['bBold', 'bUndo', 'bSave', 'bZoom']) $(id).addEventListener('mousedown', (e) => e.preventDefault()); // keep the keyboard
    $('bBold').addEventListener('click', () => runCmd('format.bold'));
    $('bUndo').addEventListener('click', () => runCmd('edit.undo'));
    $('bSave').addEventListener('click', save);
    $('bZoom').addEventListener('click', () => { zoom = ZOOMS[(ZOOMS.indexOf(zoom) + 1) % ZOOMS.length]; rebuildAll(); updateButtons(); });
  }

  // ---------------------------------------------------------------- frame sampler and hud
  function frames() {
    let last = performance.now();
    const f = (t) => {
      if (t - lastScroll < 200) { perf.frames.push(+(t - last).toFixed(1)); if (perf.frames.length > 3000) perf.frames.shift(); }
      last = t; requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
  }
  perf.summary = () => {
    const sorted = (a) => a.slice().sort((x, y) => x - y), pct = (a, p) => { const s = sorted(a); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * p))] : null; };
    const r = perf.tiles.map((t) => t.render_ms), pu = perf.tiles.map((t) => t.put_ms);
    return { tiles: perf.tiles.length, render_p50: pct(r, 0.5), render_p95: pct(r, 0.95), put_p50: pct(pu, 0.5),
      frames: perf.frames.length, frame_p50: pct(perf.frames, 0.5), frame_p95: pct(perf.frames, 0.95), frame_max: perf.frames.length ? Math.max(...perf.frames) : null,
      over50: perf.frames.filter((x) => x > 50).length, edits: perf.edits.length, edit_raf_p50: pct(perf.edits.map((e) => e.event_to_raf_ms), 0.5), edit_raf_p95: pct(perf.edits.map((e) => e.event_to_raf_ms), 0.95) };
  };
  setInterval(() => {
    if (!ed || q.get('hud') === '0') { hud.style.display = q.get('hud') === '0' ? 'none' : ''; return; }
    const s = perf.summary();
    hud.textContent = `p${pages.length} dpr${dpr} z${zoom} pv${pinch}\ntile ${s.render_p50 ?? '-'}ms (p95 ${s.render_p95 ?? '-'})\nframe p95 ${s.frame_p95 ?? '-'}ms\nwords ${ed.word_count()}`;
  }, 700);

  // ---------------------------------------------------------------- start
  async function start() {
    const T0 = performance.now();
    const b = window.wasmBindings;
    b.init();
    perf.load.wasm_ready_ms = +(performance.now() - T0).toFixed(0);
    const resp = await fetch(docName);
    const bytes = new Uint8Array(await resp.arrayBuffer());
    const t1 = performance.now();
    ed = b.WcEditor.open(docName, bytes);
    const n = ed.page_count();
    perf.load.open_layout_ms = +(performance.now() - t1).toFixed(0);
    perf.load.pages = n; perf.load.doc_bytes = bytes.length;
    window.__wc = { get ed() { return ed; }, runCmd, save, zoomTo: (z) => { zoom = z; rebuildAll(); }, get pages() { return pages; }, get cssPerPt() { return cssPerPt; } };
    setupIme(); setupPointer(); setupToolbar(); frames();
    buildPages(true);
    updateOverlay();
    const t2 = performance.now();
    updateTiles();
    // first paint = first tile done
    await new Promise((r) => { const w = () => (queue.length === 0 && !pumping ? r() : setTimeout(w, 10)); setTimeout(w, 20); });
    perf.load.first_tiles_ms = +(performance.now() - t2).toFixed(0);
    perf.load.total_since_script_ms = +(performance.now() - T0).toFixed(0);
    perf.load.nav_to_ready_ms = +performance.now().toFixed(0);
    perf.ready = true;
    updateButtons();
    log('ready');
  }
  window.addEventListener('TrunkApplicationStarted', () => { start().catch((e) => { hud.textContent = 'FAILED ' + e; log('start-error', String(e)); console.error(e); }); });
})();
