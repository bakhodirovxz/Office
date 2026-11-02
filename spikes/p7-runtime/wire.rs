// Shared by the plugin and the host (included with #[path]). Binary batch format of the spike:
// input  = u8 dir (0 to Cyrillic, 1 to Latin), u32 paragraphs, per paragraph u32 runs, per run u8 convert, u32 len, bytes
// output = u32 paragraphs, per paragraph u32 runs, per run u8 changed, then (if changed) u32 len, bytes
#![allow(dead_code)]
pub struct Rd<'a> {
    pub b: &'a [u8],
    pub i: usize,
}
impl<'a> Rd<'a> {
    pub fn u8(&mut self) -> u8 {
        let v = self.b[self.i];
        self.i += 1;
        v
    }
    pub fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.b[self.i..self.i + 4].try_into().unwrap());
        self.i += 4;
        v
    }
    pub fn str(&mut self) -> &'a str {
        let n = self.u32() as usize;
        let s = core::str::from_utf8(&self.b[self.i..self.i + n]).unwrap();
        self.i += n;
        s
    }
}
pub fn put_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
pub fn put_str(o: &mut Vec<u8>, s: &str) {
    put_u32(o, s.len() as u32);
    o.extend_from_slice(s.as_bytes());
}
