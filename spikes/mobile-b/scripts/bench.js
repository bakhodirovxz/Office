// B4: render_tile ms per page at a few scales, plus keystroke-path costs; run in the page via CDP. usage: node bench.js [port-url-substring]
const { chromium } = require('C:/Users/uge226/Desktop/office/spikes/docserver/e2e/node_modules/@playwright/test');
(async () => {
  const b = await chromium.connectOverCDP('http://localhost:9222');
  const page = [...b.contexts().flatMap((c) => c.pages())].reverse().find((p) => p.url().includes('8781'));
  const r = await page.evaluate(() => {
    const ed = __wc.ed, out = { dpr: devicePixelRatio, ua: navigator.userAgent.slice(0, 60), full: [], tile: [] };
    const med = (a) => { const s = a.slice().sort((x, y) => x - y); return +s[Math.floor(s.length / 2)].toFixed(1); };
    // scales: fit-width (what the page uses), and CSS 100% at DPR (1.333 css px per pt x DPR)
    const scales = { fit: 0.647 * devicePixelRatio, css100: (96 / 72) * devicePixelRatio };
    for (const [name, rs] of Object.entries(scales)) {
      for (const p of [0, 1, 3]) {
        const [w, h] = ed.page_size_pt(p), W = Math.ceil(w * rs), H = Math.ceil(h * rs), ts = [];
        for (let i = 0; i < 5; i++) { const t = performance.now(); ed.render_tile(p, rs, 0, 0, W, H); ts.push(performance.now() - t); }
        out.full.push({ scale: name, rs: +rs.toFixed(3), page: p, px: W + 'x' + H, mpx: +(W * H / 1e6).toFixed(2), first_ms: +ts[0].toFixed(1), median_ms: med(ts), max_ms: +Math.max(...ts).toFixed(1) });
      }
      const ts = [];
      for (let i = 0; i < 7; i++) { const t = performance.now(); ed.render_tile(1, rs, 0, 0, 1024, 1024); ts.push(performance.now() - t); }
      out.tile.push({ scale: name, tile: '1024x1024', median_ms: med(ts), max_ms: +Math.max(...ts).toFixed(1) });
    }
    const t = performance.now(); ed.text_context(300, 300); out.text_context_ms = +(performance.now() - t).toFixed(2);
    const t2 = performance.now(); const n = 200; for (let i = 0; i < n; i++) ed.caret(); out.caret_ms = +((performance.now() - t2) / n).toFixed(3);
    return out;
  });
  console.log(JSON.stringify(r, null, 1));
  await b.close();
})();
