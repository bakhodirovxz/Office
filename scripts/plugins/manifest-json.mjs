// plugin.toml -> the JSON manifest the page host takes in a descriptor (docs/plugins.md section 9a). Handles the subset the manifest uses:
// `key = "text" | number | true | [..] | { k = v }`, dotted keys (`name.en`), `[table]` and `[[array]]` headers. WP8 builds descriptors with plugin-api.
import fs from 'node:fs';
import crypto from 'node:crypto';

const value = (s) => {
  s = s.trim();
  if (s.startsWith('"')) return JSON.parse(s);
  if (s === 'true' || s === 'false') return s === 'true';
  if (/^-?\d+(\.\d+)?$/.test(s)) return Number(s);
  if (s.startsWith('[')) { try { return JSON.parse(s); } catch { return split(s.slice(1, -1)).map(value); } }   // an array of inline tables is not JSON
  if (s.startsWith('{')) return Object.fromEntries(split(s.slice(1, -1)).map((kv) => { const i = kv.indexOf('='); return [kv.slice(0, i).trim(), value(kv.slice(i + 1))]; }));
  throw new Error(`unsupported TOML value: ${s}`);
};
// split at top-level commas (outside strings and brackets)
const split = (s) => {
  const out = []; let depth = 0, q = false, cur = '';
  for (const c of s) {
    if (c === '"') q = !q;
    if (!q && (c === '[' || c === '{')) depth++;
    if (!q && (c === ']' || c === '}')) depth--;
    if (!q && depth === 0 && c === ',') { out.push(cur); cur = ''; } else cur += c;
  }
  return cur.trim() ? [...out, cur] : out;
};
const put = (o, key, v) => { const p = key.split('.'); const last = p.pop(); for (const k of p) o = o[k] ??= {}; o[last] = v; };

export function manifestJson(text) {
  const root = {}; let cur = root;
  const lines = text.split(/\r?\n/);
  for (let n = 0; n < lines.length; n++) {
    let line = lines[n].replace(/^\s+|\s+$/g, '');
    if (!line || line.startsWith('#')) continue;
    // a multi-line array: join the lines until the brackets balance (the strings of these manifests hold no brackets)
    const depth = (t) => (t.match(/[[{]/g) || []).length - (t.match(/[\]}]/g) || []).length;
    while (/^[\w.-]+\s*=/.test(line) && depth(line) > 0 && n + 1 < lines.length) line += ' ' + lines[++n].trim();
    let m;
    if ((m = line.match(/^\[\[([\w.-]+)\]\]$/))) { cur = {}; (root[m[1]] ??= []).push(cur); }
    else if ((m = line.match(/^\[([\w.-]+)\]$/))) cur = root[m[1]] ??= {};
    else { const i = line.indexOf('='); put(cur, line.slice(0, i).trim(), value(line.slice(i + 1))); }
  }
  return root;
}

/** The descriptor of a plugin directory: `wasmUrl` is where the page serves plugin.wasm. */
export function descriptor(dir, wasmFile, wasmUrl) {
  const manifest = manifestJson(fs.readFileSync(`${dir}/plugin.toml`, 'utf8'));
  const wasm = fs.readFileSync(wasmFile);
  return { id: manifest.id, version: manifest.version, sha256: crypto.createHash('sha256').update(wasm).digest('hex'), wasm: wasmUrl, manifest };
}
