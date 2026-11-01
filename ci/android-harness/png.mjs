// Pixel reader for the rig's screencaps (8-bit RGBA PNG): used to tell Gboard's English and Russian layouts apart. Our code, MIT OR Apache-2.0.
import zlib from 'node:zlib';
export function px(buf, pts) {
  let o = 8, w, h, idat = [];
  while (o < buf.length) { const n = buf.readUInt32BE(o), t = buf.toString('latin1', o + 4, o + 8); if (t === 'IHDR') { w = buf.readUInt32BE(o + 8); h = buf.readUInt32BE(o + 12); } if (t === 'IDAT') idat.push(buf.subarray(o + 8, o + 8 + n)); o += 12 + n; }
  const raw = zlib.inflateSync(Buffer.concat(idat)), bpp = 4, st = w * bpp, img = Buffer.alloc(h * st);
  for (let y = 0; y < h; y++) { const f = raw[y * (st + 1)]; for (let i = 0; i < st; i++) { const r = raw[y * (st + 1) + 1 + i], a = i >= bpp ? img[y * st + i - bpp] : 0, b = y ? img[(y - 1) * st + i] : 0, c = i >= bpp && y ? img[(y - 1) * st + i - bpp] : 0;
    img[y * st + i] = (r + (f === 0 ? 0 : f === 1 ? a : f === 2 ? b : f === 3 ? (a + b) >> 1 : (() => { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); return pa <= pb && pa <= pc ? a : pb <= pc ? b : c; })())) & 255; } }
  return pts.map(([x, y]) => [...img.subarray(y * st + x * 4, y * st + x * 4 + 3)]);
}
