//! Data at rest (PLAN 5.5, server mode): AES-256-GCM, a fresh key per job, streaming in 64 KiB frames.
//!
//! Format of one blob: `"CAPI1"`, a u32 file id, then frames `[u32 len | LAST<<31][ciphertext + 16-byte tag]`. The nonce is
//! `file id (4) | frame index (8)`, so it never repeats under a job key (ids come from a per-job counter). The AAD is the LAST
//! flag, which makes a truncated blob (cut at a frame border) fail instead of reading as a shorter file. A frame that is
//! moved or reordered fails too, because its index is in the nonce.
//!
//! The job key lives in memory (`JobKey`) and is zeroed on drop. A copy wrapped by the operator key is written next to the job
//! (`wrap`), so a crashed server's leftovers can be recovered by the operator and by nobody else; the sweeper deletes that file
//! before the data. `Codec::Plain` is local mode: the same API without encryption.
use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload, rand_core::RngCore};
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
}

impl Drop for JobKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl JobKey {
    pub fn generate() -> Arc<JobKey> {
        let mut b = [0u8; 32];
        OsRng.fill_bytes(&mut b);
        Self::from_bytes(b)
    }
    pub fn from_bytes(bytes: [u8; 32]) -> Arc<JobKey> {
        Arc::new(JobKey { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&bytes)), bytes, next_file: AtomicU32::new(1) })
    }
    /// `nonce | ciphertext` of the job key under the operator key; the job id is the AAD, so a wrapped key cannot be moved to another job.
    pub fn wrap(&self, operator: &[u8; 32], job_id: &str) -> Vec<u8> {
        let kek = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(operator));
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ct = kek.encrypt(Nonce::from_slice(&nonce), Payload { msg: &self.bytes, aad: job_id.as_bytes() }).expect("AES-GCM encrypts 32 bytes");
        [nonce.to_vec(), ct].concat()
    }
    pub fn unwrap(operator: &[u8; 32], job_id: &str, wrapped: &[u8]) -> Option<Arc<JobKey>> {
        let kek = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(operator));
        let (nonce, ct) = wrapped.split_at_checked(12)?;
        let mut pt = kek.decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: job_id.as_bytes() }).ok()?;
        let mut b = [0u8; 32];
        let ok = pt.len() == 32;
        if ok {
            b.copy_from_slice(&pt);
        }
        pt.zeroize();
        ok.then(|| Self::from_bytes(b))
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
    pub fn writer<W: Write>(&self, mut inner: W) -> io::Result<BlobWriter<W>> {
        let file = match self {
            Codec::Plain => 0,
            Codec::Aes(k) => {
                let id = k.next_file.fetch_add(1, Ordering::Relaxed);
                inner.write_all(MAGIC)?;
                inner.write_all(&id.to_be_bytes())?;
                id
            }
        };
        Ok(BlobWriter { inner, codec: self.clone(), file, frame: 0, buf: Vec::with_capacity(FRAME), written: 0 })
    }

    pub fn reader<R: Read>(&self, mut inner: R) -> io::Result<BlobReader<R>> {
        let file = match self {
            Codec::Plain => 0,
            Codec::Aes(_) => {
                let mut h = [0u8; 9];
                inner.read_exact(&mut h)?;
                if &h[..5] != MAGIC {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "not an encrypted blob"));
                }
                u32::from_be_bytes([h[5], h[6], h[7], h[8]])
            }
        };
        Ok(BlobReader { inner, codec: self.clone(), file, frame: 0, plain: Vec::new(), pos: 0, done: false })
    }

    pub fn open(&self, path: &std::path::Path) -> io::Result<BlobReader<std::fs::File>> {
        self.reader(std::fs::File::open(path)?)
    }

    /// Encrypt (or copy) a plaintext file into `dest` (which must not exist); returns the plaintext size.
    pub fn store_file(&self, src: &std::path::Path, dest: &std::path::Path) -> io::Result<u64> {
        let mut w = self.writer(std::fs::File::create_new(dest)?)?;
        io::copy(&mut std::fs::File::open(src)?, &mut w)?;
        w.finish()
    }
}

pub struct BlobWriter<W: Write> {
    inner: W,
    codec: Codec,
    file: u32,
    frame: u64,
    buf: Vec<u8>,
    written: u64,
}

impl<W: Write> BlobWriter<W> {
    fn emit(&mut self, last: bool) -> io::Result<()> {
        let Codec::Aes(k) = &self.codec else { return Ok(()) };
        let flag = [last as u8];
        let ct = k.cipher.encrypt(Nonce::from_slice(&nonce(self.file, self.frame)), Payload { msg: &self.buf, aad: &flag }).map_err(|_| io::Error::other("encryption failed"))?;
        self.inner.write_all(&((ct.len() as u32) | if last { LAST } else { 0 }).to_be_bytes())?;
        self.inner.write_all(&ct)?;
        self.frame += 1;
        self.buf.zeroize();
        self.buf.clear();
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
    frame: u64,
    plain: Vec<u8>,
    pos: usize,
    done: bool,
}

impl<R: Read> Read for BlobReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let Codec::Aes(k) = &self.codec else { return self.inner.read(out) };
        while self.pos == self.plain.len() {
            if self.done {
                return Ok(0);
            }
            let mut len = [0u8; 4];
            self.inner.read_exact(&mut len).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "truncated blob"))?;
            let word = u32::from_be_bytes(len);
            let (last, n) = (word & LAST != 0, (word & !LAST) as usize);
            if !(16..=FRAME + 16).contains(&n) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "bad frame length"));
            }
            let mut ct = vec![0u8; n];
            self.inner.read_exact(&mut ct).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "truncated blob"))?;
            self.plain = k
                .cipher
                .decrypt(Nonce::from_slice(&nonce(self.file, self.frame)), Payload { msg: &ct, aad: &[last as u8] })
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "blob failed authentication"))?;
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

    fn seal(c: &Codec, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut w = c.writer(&mut out).unwrap();
        w.write_all(data).unwrap();
        assert_eq!(w.finish().unwrap(), data.len() as u64);
        out
    }

    #[test]
    fn round_trip_at_the_frame_borders_and_nothing_readable_on_disk() {
        let c = Codec::Aes(JobKey::generate());
        for n in [0usize, 1, FRAME - 1, FRAME, FRAME + 1, 3 * FRAME, 3 * FRAME + 7] {
            let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            let sealed = seal(&c, &data);
            if n > 64 {
                assert!(!sealed.windows(64).any(|w| w == &data[..64]), "plaintext visible at n={n}");
            }
            let mut back = Vec::new();
            c.reader(&sealed[..]).unwrap().read_to_end(&mut back).unwrap();
            assert_eq!(back, data, "n={n}");
        }
    }

    #[test]
    fn tamper_truncate_reorder_and_wrong_key_all_fail() {
        let c = Codec::Aes(JobKey::generate());
        let data = vec![7u8; 3 * FRAME];
        let sealed = seal(&c, &data);
        let read = |c: &Codec, b: &[u8]| c.reader(b).and_then(|mut r| r.read_to_end(&mut Vec::new()));
        let mut flipped = sealed.clone();
        flipped[40] ^= 1;
        assert!(read(&c, &flipped).is_err());
        // cut exactly after the first frame: the missing LAST frame must be noticed
        let f = 4 + FRAME + 16;
        assert!(read(&c, &sealed[..9 + f]).is_err());
        assert!(read(&c, &sealed[..sealed.len() - 1]).is_err());
        // swap frames 1 and 2
        let mut swapped = sealed[..9].to_vec();
        swapped.extend_from_slice(&sealed[9 + f..9 + 2 * f]);
        swapped.extend_from_slice(&sealed[9..9 + f]);
        swapped.extend_from_slice(&sealed[9 + 2 * f..]);
        assert!(read(&c, &swapped).is_err());
        assert!(read(&Codec::Aes(JobKey::generate()), &sealed).is_err());
    }

    #[test]
    fn wrapped_key_round_trips_only_for_its_job_and_operator() {
        let op = [9u8; 32];
        let k = JobKey::generate();
        let w = k.wrap(&op, "job-a");
        let k2 = JobKey::unwrap(&op, "job-a", &w).expect("unwraps");
        assert_eq!(k.bytes, k2.bytes);
        assert!(JobKey::unwrap(&op, "job-b", &w).is_none());
        assert!(JobKey::unwrap(&[1u8; 32], "job-a", &w).is_none());
    }
}
