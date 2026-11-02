// Spike worker: one plugin instance, no imports except (optionally) the host-made memory. Served with
// `Content-Security-Policy: default-src 'none'` so the worker itself cannot reach the network either.
let inst = null;
let mem = null;
onmessage = async (e) => {
  const m = e.data;
  if (m.type === 'load') {
    const imports = {};
    if (m.maxPages) { mem = new WebAssembly.Memory({ initial: 17, maximum: m.maxPages }); imports.env = { memory: mem }; }
    inst = await WebAssembly.instantiate(m.module, imports);
    mem = mem || inst.exports.memory;
    let net = 'not tried';
    try { await fetch('/ping'); net = 'reached'; } catch (err) { net = 'blocked: ' + err.name; }
    postMessage({ type: 'loaded', net });
  } else if (m.type === 'run') {
    const x = inst.exports;
    const input = new Uint8Array(m.input);
    const p = x.p7_alloc(input.length);
    new Uint8Array(mem.buffer, p, input.length).set(input);
    const r = x.p7_bin(p, input.length);
    const op = Number(r >> 32n), on = Number(r & 0xffffffffn);
    const out = new Uint8Array(on);
    out.set(new Uint8Array(mem.buffer, op, on));
    x.p7_free(op, on);
    postMessage({ type: 'ran', out: out.buffer }, [out.buffer]);
  } else if (m.type === 'spin') {
    postMessage({ type: 'spinning' });
    inst.exports.p7_spin();
  } else if (m.type === 'hog') {
    const pages = inst.exports.p7_hog();
    postMessage({ type: 'hogged', pages, bytes: mem.buffer.byteLength });
  }
};
