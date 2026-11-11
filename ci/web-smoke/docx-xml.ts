import * as zlib from 'node:zlib';

/** word/document.xml of a docx (stored or deflated entry, found through the central directory). */
export const documentXml = (buf: Buffer): string => zipEntry(buf, 'word/document.xml');

/** The text of one entry of a zip (an xlsx, a pptx, a docx), or throws. */
export function zipEntry(buf: Buffer, entry: string): string {
  const end = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  let p = buf.readUInt32LE(end + 16);
  for (let n = buf.readUInt16LE(end + 10); n > 0; n--) {
    const method = buf.readUInt16LE(p + 10), size = buf.readUInt32LE(p + 20), nl = buf.readUInt16LE(p + 28), xl = buf.readUInt16LE(p + 30), cl = buf.readUInt16LE(p + 32);
    const off = buf.readUInt32LE(p + 42), name = buf.toString('utf8', p + 46, p + 46 + nl);
    if (name === entry) {
      const start = off + 30 + buf.readUInt16LE(off + 26) + buf.readUInt16LE(off + 28), raw = buf.subarray(start, start + size);
      return (method === 8 ? zlib.inflateRawSync(raw) : raw).toString('utf8');
    }
    p += 46 + nl + xl + cl;
  }
  throw new Error('no ' + entry);
}
export const bolds = (xml: string) => (xml.match(/<w:b(\/| w:val="(1|true|on)"\/)>/g) ?? []).length;
