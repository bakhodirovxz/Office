//! Data at rest (PLAN phase 5, section 4, WP14): AES-256-GCM over whole files under `data_dir`, with a key from `encrypt_key_file`.
//!
//! Format of one sealed file: `"DSAR1"`, a 12-byte random nonce, then ciphertext and the 16-byte tag. The nonce is fresh for every write
//! (96 random bits, no counter to lose across a restart). The authenticated header is the magic plus the file's NAME (its path under
//! `data_dir`, or `history/objects/<sha>`), so a file copied over another one fails, as does any changed byte. Whatever fails to open is
//! refused: there is no plaintext fallback.
//!
//! Threat model: this protects the data folder against theft of the disk, a copy of the volume or a backup. It does not protect against
//! someone who is root on the running host: the key is in the server's memory, and the key file must be readable by the server.
use crate::config::Docserver;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;
use suite_core::history::Sealer;

const MAGIC: &[u8; 5] = b"DSAR1";
const OVERHEAD: usize = MAGIC.len() + 12 + 16;
/// Sealed under the key at the first start; opening it again is how a wrong key is found.
const CHECK_FILE: &str = "at-rest.check";
const CHECK_TEXT: &[u8] = b"docserver at-rest v1";

pub struct AtRest(Aes256Gcm);

fn bad(m: &str) -> Error {
    Error::new(ErrorKind::InvalidData, m)
}

impl AtRest {
    pub fn new(key_material: &[u8]) -> AtRest {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key_material).expect("any key length");
        mac.update(b"docserver at-rest key v1");
        let k = mac.finalize().into_bytes();
        AtRest(Aes256Gcm::new_from_slice(&k).expect("32 bytes"))
    }

    pub fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        getrandom::getrandom(&mut nonce).map_err(|e| Error::other(e.to_string()))?;
        let aad = [MAGIC.as_slice(), name.as_bytes()].concat();
        let ct = self
            .0
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plain,
                    aad: &aad,
                },
            )
            .map_err(|_| bad("encryption failed"))?;
        Ok([MAGIC.as_slice(), &nonce, &ct].concat())
    }

    pub fn open(&self, name: &str, sealed: &[u8]) -> Result<Vec<u8>> {
        if sealed.len() < OVERHEAD || &sealed[..MAGIC.len()] != MAGIC {
            return Err(bad("not an encrypted file"));
        }
        let (nonce, ct) = sealed[MAGIC.len()..].split_at(12);
        let aad = [MAGIC.as_slice(), name.as_bytes()].concat();
        self.0
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: &aad })
            .map_err(|_| bad("wrong key or damaged file"))
    }

    /// The start-up check of `data_dir` against the key (see the module doc): a wrong key, or data written without encryption, stops the
    /// server instead of failing request by request.
    pub fn init(&self, data_dir: &Path) -> std::result::Result<(), String> {
        let check = data_dir.join(CHECK_FILE);
        if check.is_file() {
            let b = std::fs::read(&check).map_err(|e| format!("docserver: {CHECK_FILE}: {e}"))?;
            return match self.open(CHECK_FILE, &b) {
                Ok(t) if t == CHECK_TEXT => Ok(()),
                _ => Err(
                    "docserver: encrypt_key_file is not the key this data_dir was encrypted with"
                        .into(),
                ),
            };
        }
        let used =
            |d: &str| std::fs::read_dir(data_dir.join(d)).is_ok_and(|mut r| r.next().is_some());
        if used("sessions") || used("history") {
            return Err("docserver: data_dir already holds unencrypted sessions or history; use an empty data_dir for encrypt_at_rest = true".into());
        }
        std::fs::create_dir_all(data_dir).map_err(|e| format!("docserver: data_dir: {e}"))?;
        let sealed = self
            .seal(CHECK_FILE, CHECK_TEXT)
            .map_err(|e| e.to_string())?;
        std::fs::write(&check, sealed).map_err(|e| format!("docserver: {CHECK_FILE}: {e}"))
    }
}

/// With encryption off, a data folder that carries the check file is encrypted data: refuse to read it as plain.
pub fn refuse_encrypted_data(data_dir: &Path) -> std::result::Result<(), String> {
    if data_dir.join(CHECK_FILE).exists() {
        return Err("docserver: data_dir holds encrypted data: set encrypt_at_rest = true and encrypt_key_file".into());
    }
    Ok(())
}

impl Sealer for AtRest {
    fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>> {
        AtRest::seal(self, name, plain)
    }
    fn open(&self, name: &str, sealed: &[u8]) -> Result<Vec<u8>> {
        AtRest::open(self, name, sealed)
    }
    fn overhead(&self) -> usize {
        OVERHEAD
    }
}

impl Docserver {
    /// `path` relative to `data_dir`, with `/`: the name that is authenticated with the file.
    fn sealed_name(&self, path: &Path) -> String {
        path.strip_prefix(&self.data_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }

    /// The bytes of a stored file, opened when encryption is on. A file that does not open is an error, never plaintext.
    pub(crate) fn read_stored(&self, path: &Path) -> Result<Vec<u8>> {
        let b = std::fs::read(path)?;
        match &self.at_rest {
            None => Ok(b),
            Some(k) => k.open(&self.sealed_name(path), &b).inspect_err(|e| {
                eprintln!(
                    "docserver: refusing to serve {}: {e}",
                    self.sealed_name(path)
                );
            }),
        }
    }

    /// The bytes to put on disk for `path`: sealed when encryption is on.
    pub(crate) fn to_store(&self, path: &Path, bytes: &[u8]) -> Result<Vec<u8>> {
        match &self.at_rest {
            None => Ok(bytes.to_vec()),
            Some(k) => k.seal(&self.sealed_name(path), bytes),
        }
    }
}
