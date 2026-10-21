//! Data at rest (PLAN 5.5, server mode): AES-256-GCM, a fresh key per job, streaming in 64 KiB frames.
//!
//! Format of one blob: `"CAPI1"`, a u32 file id, then frames `[u32 len | LAST<<31][ciphertext + 16-byte tag]`. The nonce is
//! `file id (4) | frame index (8)`, so it never repeats under a job key (ids come from a per-job counter). The AAD is
//! `job id | role | LAST flag`: the LAST flag makes a truncated blob (cut at a frame border) fail instead of reading as a shorter
//! file, and the role (`in/3`, `out/1/0`, `zip`) makes a blob that was swapped for another one of the same job fail (WP7 review
//! m6). A frame that is moved or reordered fails too, because its index is in the nonce. Bytes after the LAST frame are an error.
//!
//! The job key lives in memory only (`JobKey`) and is zeroed on drop. It is not written anywhere, so what a crash leaves behind is
//! ciphertext with no key on the machine (WP7 review M5); the next start sweeps it. `Codec::Plain` is local mode: the same API
//! without encryption.
use aes_gcm::aead::{AeadInPlace, KeyInit, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use zeroize::Zeroize;

const MAGIC: &[u8; 5] = b"CAPI1";
const FRAME: usize = 64 * 1024;
const LAST: u32 = 1 << 31;

pub struct JobKey {
    bytes: [u8; 32],
    cipher: Aes256Gcm,
    next_file: AtomicU32,
    job_id: String,
}

impl Drop for JobKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl JobKey {
    pub fn generate(job_id: &str) -> Arc<JobKey> {
        let mut b = [0u8; 32];
        OsRng.fill_bytes(&mut b);
        Self::from_bytes(b, job_id)
    }
    pub fn from_bytes(bytes: [u8; 32], job_id: &str) -> Arc<JobKey> {
        Arc::new(JobKey { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&bytes)), bytes, next_file: AtomicU32::new(1), job_id: job_id.into() })
    }
    /// `job id | 0 | role | 0 | last`: the last byte is the LAST flag, set per frame.
    fn aad(&self, role: &str) -> Vec<u8> {
        [self.job_id.as_bytes(), &[0], role.as_bytes(), &[0, 0]].concat()
    }
}

fn nonce(file: u32, frame: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..4].copy_from_slice(&file.to_be_bytes());
    n[4..].copy_from_slice(&frame.to_be_bytes());
    n
}

#[derive(Clone)]
pub enum Codec {
    Plain,
    Aes(Arc<JobKey>),
}

impl Codec {
    /// `role` names what the blob is (`in/3`, `out/1/0`, `zip`); the reader must name the same one.
    pub fn writer<W: Write>(&self, mut inner: W, role: &str) -> io::Result<BlobWriter<W>> {
        let (file, aad) = match self {
            Codec::Plain => (0, vec![]),
            Codec::Aes(k) => {
                let id = k.next_file.fetch_add(1, Ordering::Relaxed);
                inner.write_all(MAGIC)?;
                inner.write_all(&id.to_be_bytes())?;
                (id, k.aad(role))
            }
        };
        // room for the tag, so the in-place encryption never reallocates (a realloc would leave a plaintext copy behind)
        Ok(BlobWriter { inner, codec: self.clone(), file, aad, frame: 0, buf: Vec::with_capacity(FRAME + 16), written: 0 })
    }

    pub fn reader<R: Read>(&self, mut inner: R, role: &str) -> io::Result<BlobReader<R>> {
        let (file, aad) = match self {
            Codec::Plain => (0, vec![]),
            Codec::Aes(k) => {
                let mut h = [0u8; 9];
                inner.read_exact(&mut h)?;
                if &h[..5] != MAGIC {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "not an encrypted blob"));
                }
                (u32::from_be_bytes([h[5], h[6], h[7], h[8]]), k.aad(role))
            }
        };
        Ok(BlobReader { inner, codec: self.clone(), file, aad, frame: 0, plain: Vec::with_capacity(FRAME + 16), pos: 0, done: false })
    }

    pub fn open(&self, path: &std::path::Path, role: &str) -> io::Result<BlobReader<std::fs::File>> {
        self.reader(std::fs::File::open(path)?, role)
    }

    /// Encrypt (or copy) a plaintext file into `dest` (which must not exist); returns the plaintext size.
    pub fn store_file(&self, src: &std::path::Path, dest: &std::path::Path, role: &str) -> io::Result<u64> {
        let mut w = self.writer(std::fs::File::create_new(dest)?, role)?;
        io::copy(&mut std::fs::File::open(src)?, &mut w)?;
        w.finish()
    }
}

pub struct BlobWriter<W: Write> {
    inner: W,
    codec: Codec,
    file: u32,
    aad: Vec<u8>,
    frame: u64,
    buf: Vec<u8>,
    written: u64,
}

impl<W: Write> Drop for BlobWriter<W> {
    fn drop(&mut self) {
        self.buf.zeroize();
    }
}

impl<W: Write> BlobWriter<W> {
    fn emit(&mut self, last: bool) -> io::Result<()> {
        let Codec::Aes(k) = &self.codec else { return Ok(()) };
        *self.aad.last_mut().expect("the aad ends with the last flag") = last as u8;
        k.cipher.encrypt_in_place(Nonce::from_slice(&nonce(self.file, self.frame)), &self.aad, &mut self.buf).map_err(|_| io::Error::other("encryption failed"))?;
        self.inner.write_all(&((self.buf.len() as u32) | if last { LAST } else { 0 }).to_be_bytes())?;
        self.inner.write_all(&self.buf)?;
        self.frame += 1;
        self.buf.zeroize();
        Ok(())
    }
    /// Finish the blob; returns the number of plaintext bytes.
    pub fn finish(mut self) -> io::Result<u64> {
        if matches!(self.codec, Codec::Aes(_)) {
            self.emit(true)?;
        }
        self.inner.flush()?;
        Ok(self.written)
    }
}

impl<W: Write> Write for BlobWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.written += data.len() as u64;
        if matches!(self.codec, Codec::Plain) {
            self.inner.write_all(data)?;
            return Ok(data.len());
        }
        let mut rest = data;
        while !rest.is_empty() {
            // a full frame is written only when more data follows, so the last frame is always the one `finish` writes
            if self.buf.len() == FRAME {
                self.emit(false)?;
            }
            let n = (FRAME - self.buf.len()).min(rest.len());
            self.buf.extend_from_slice(&rest[..n]);
            rest = &rest[n..];
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct BlobReader<R: Read> {
    inner: R,
    codec: Codec,
    file: u32,
    aad: Vec<u8>,
    frame: u64,
    plain: Vec<u8>,
    pos: usize,
    done: bool,
}

impl<R: Read> Read for BlobReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let Codec::Aes(k) = &self.codec else { return self.inner.read(out) };
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        while self.pos == self.plain.len() {
            if self.done {
                return Ok(0);
            }
            let mut len = [0u8; 4];
            self.inner.read_exact(&mut len).map_err(|_| bad("truncated blob"))?;
            let word = u32::from_be_bytes(len);
            let (last, n) = (word & LAST != 0, (word & !LAST) as usize);
            if !(16..=FRAME + 16).contains(&n) {
                return Err(bad("bad frame length"));
            }
            // one buffer, decrypted in place: the previous frame is wiped before the next one is read into it
            self.plain.zeroize();
            self.plain.resize(n, 0);
            self.inner.read_exact(&mut self.plain).map_err(|_| bad("truncated blob"))?;
            *self.aad.last_mut().expect("the aad ends with the last flag") = last as u8;
            if k.cipher.decrypt_in_place(Nonce::from_slice(&nonce(self.file, self.frame)), &self.aad, &mut self.plain).is_err() {
                self.plain.zeroize();
                return Err(bad("blob failed authentication"));
            }
            if last && self.inner.read(&mut [0u8; 1]).map_err(|_| bad("blob unreadable"))? != 0 {
                self.plain.zeroize();
                return Err(bad("data after the last frame"));
            }
            self.pos = 0;
            self.frame += 1;
            self.done = last;
        }
        let n = out.len().min(self.plain.len() - self.pos);
        out[..n].copy_from_slice(&self.plain[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl<R: Read> Drop for BlobReader<R> {
    fn drop(&mut self) {
        self.plain.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seal(c: &Codec, data: &[u8], role: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut w = c.writer(&mut out, role).unwrap();
        w.write_all(data).unwrap();
        assert_eq!(w.finish().unwrap(), data.len() as u64);
        out
    }

    fn key() -> Codec {
        Codec::Aes(JobKey::generate("job-a"))
    }

    #[test]
    fn round_trip_at_the_frame_borders_and_nothing_readable_on_disk() {
        let c = key();
        for n in [0usize, 1, FRAME - 1, FRAME, FRAME + 1, 3 * FRAME, 3 * FRAME + 7] {
            let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            let sealed = seal(&c, &data, "in/0");
            if n > 64 {
                assert!(!sealed.windows(64).any(|w| w == &data[..64]), "plaintext visible at n={n}");
            }
            let mut back = Vec::new();
            c.reader(&sealed[..], "in/0").unwrap().read_to_end(&mut back).unwrap();
            assert_eq!(back, data, "n={n}");
        }
    }

    #[test]
    fn tamper_truncate_reorder_wrong_key_wrong_role_and_trailing_bytes_all_fail() {
        let c = key();
        let data = vec![7u8; 3 * FRAME];
        let sealed = seal(&c, &data, "in/0");
        let read = |c: &Codec, b: &[u8], role: &str| c.reader(b, role).and_then(|mut r| r.read_to_end(&mut Vec::new()));
        assert!(read(&c, &sealed, "in/0").is_ok());
        let mut flipped = sealed.clone();
        flipped[40] ^= 1;
        assert!(read(&c, &flipped, "in/0").is_err());
        // cut exactly after the first frame: the missing LAST frame must be noticed
        let f = 4 + FRAME + 16;
        assert!(read(&c, &sealed[..9 + f], "in/0").is_err());
        assert!(read(&c, &sealed[..sealed.len() - 1], "in/0").is_err());
        // swap frames 1 and 2
        let mut swapped = sealed[..9].to_vec();
        swapped.extend_from_slice(&sealed[9 + f..9 + 2 * f]);
        swapped.extend_from_slice(&sealed[9..9 + f]);
        swapped.extend_from_slice(&sealed[9 + 2 * f..]);
        assert!(read(&c, &swapped, "in/0").is_err());
        assert!(read(&Codec::Aes(JobKey::generate("job-a")), &sealed, "in/0").is_err());
        // a whole blob moved to another role of the same job, or to another job under the same key, no longer reads
        assert!(read(&c, &sealed, "in/1").is_err());
        let Codec::Aes(k) = &c else { unreachable!() };
        assert!(read(&Codec::Aes(JobKey::from_bytes(k.bytes, "job-b")), &sealed, "in/0").is_err());
        // bytes after the last frame
        let mut extra = sealed.clone();
        extra.push(0);
        assert!(read(&c, &extra, "in/0").is_err());
    }
}
