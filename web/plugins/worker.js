// One plugin instance in a dedicated Worker (PLAN phase 7 sections 4.2 and 5.1). The server sends this file with
// `Content-Security-Policy: default-src 'none'; script-src 'wasm-unsafe-eval'`: no fetch, no importScripts, wasm compile only.
// Messages in: {type:'load', bytes, maxPages} and {type:'call', id, msg}. Out: loaded | answer {id, out, mem} | error {id?, error} | log {text}.
'use strict';
const MAX_OUT = 32 << 20;
const dec = new TextDecoder();
let inst = null, mem = null, logs = 0;

// The memory is made here with a maximum, never exported by the module: an exported memory had no cap in the spike (4.28 GB reached).
const memory = (initial, maximum) => new WebAssembly.Memory({ initial: Math.min(initial, maximum), maximum });

// Section headers (id byte, LEB128 size) are walked before compile: V8 allows several memories, and a memory the module defines itself has no cap
// here; wasm GC objects have none either. So: no memory section (5), no GC type form (0x4e 0x4f 0x50 0x5e 0x5f) in the type section (1).
// A header that does not parse is refused.
function screen(bytes) {
  const u = new Uint8Array(bytes), leb = (i) => { let v = 0, s = 0, b; do { if (i >= u.length || s > 28) throw new Error('malformed module'); b = u[i++]; v += (b & 127) * 2 ** s; s += 7; } while (b & 128); return [v, i]; };
  if (u.length < 8 || u[0] !== 0 || u[1] !== 0x61 || u[2] !== 0x73 || u[3] !== 0x6d) throw new Error('malformed module');
  for (let i = 8; i < u.length;) {
    const id = u[i++]; let n; [n, i] = leb(i);
    if (n > u.length - i) throw new Error('malformed module');
    if (id === 5) throw new Error('the module defines its own memory');
    if (id === 1) {
      let j = i, c; [c, j] = leb(j);
      // only plain function types (0x60, value types of one byte) are walked; a GC form or a typed reference (0x63, 0x64) ends the walk with a refusal
      const bad = () => { throw new Error('the module uses a type form that is not allowed'); }, vec = () => { let k; [k, j] = leb(j); for (; k > 0; k--) { if (j >= i + n || u[j] === 0x63 || u[j] === 0x64) bad(); j++; } };
      for (; c > 0; c--) { if (u[j] !== 0x60) bad(); j++; vec(); vec(); }
    }
    i += n;
  }
}

async function load({ bytes, maxPages }) {
  const max = Math.max(1, Math.min(maxPages | 0, 1024));   // 64 MiB is the host cap
  screen(bytes);
  const mod = await WebAssembly.compile(bytes);
  const imports = WebAssembly.Module.imports(mod);
  const bad = imports.filter((i) => !((i.module === 'env' && i.name === 'memory' && i.kind === 'memory') || (i.module === 'plugin_v1' && i.name === 'log' && i.kind === 'function')));
  if (bad.length) throw new Error('imports not allowed: ' + bad.map((i) => `${i.module}.${i.name}`).join(', '));
  const exports = WebAssembly.Module.exports(mod);
  if (exports.some((x) => x.kind === 'memory')) throw new Error('the module exports a memory');
  if (!imports.some((i) => i.module === 'env')) throw new Error('the module does not import env.memory');
  for (const n of ['api_version', 'alloc', 'free', 'call']) if (!exports.some((x) => x.name === n && x.kind === 'function')) throw new Error('missing export ' + n);
  const env = () => ({
    env: { memory: mem },
    // debug text only: rate-limited, cut, never shown to users
    plugin_v1: { log: (p, n) => { if (logs++ < 32 && (n >>> 0) <= 2048 && (p >>> 0) + (n >>> 0) <= mem.buffer.byteLength) postMessage({ type: 'log', text: dec.decode(new Uint8Array(mem.buffer, p >>> 0, n >>> 0)) }); } },
  });
  mem = memory(17, max);
  try { inst = await WebAssembly.instantiate(mod, env()); } catch (e) {
    if (!(e instanceof WebAssembly.LinkError)) throw e;
    mem = memory(max, max);   // the module asks for more than 17 pages at start
    inst = await WebAssembly.instantiate(mod, env());
  }
  if (inst.exports.api_version() !== 1) throw new Error('api_version is not 1');
}

function call(id, msg) {
  const x = inst.exports, input = new Uint8Array(msg);
  logs = 0;
  const p = x.alloc(input.length) >>> 0;
  if (p + input.length > mem.buffer.byteLength) throw new Error('alloc returned a pointer outside the memory');
  new Uint8Array(mem.buffer, p, input.length).set(input);
  const r = BigInt.asUintN(64, x.call(p, input.length));
  const op = Number(r >> 32n), on = Number(r & 0xffffffffn);
  if (on > MAX_OUT || op + on > mem.buffer.byteLength) throw new Error('the answer lies outside the plugin memory');
  const out = mem.buffer.slice(op, op + on);
  x.free(op, on); x.free(p, input.length);
  postMessage({ type: 'answer', id, out, mem: mem.buffer.byteLength }, [out]);
}

onmessage = async (e) => {
  const m = e.data;
  try {
    if (m.type === 'load') { await load(m); postMessage({ type: 'loaded' }); } else if (m.type === 'call' && inst) call(m.id, m.msg);
  } catch (err) { postMessage({ type: 'error', id: m.id, error: String((err && err.message) || err) }); }
};
