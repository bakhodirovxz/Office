//! Our own tokens (PLAN phase 5, 3.2): the session token (a bearer the wrapper page keeps in memory) and the file token (the
//! purpose-bound token in the saved-file URL). Both are HS256 over `convert_api::jwt`, each under its own subkey of the
//! `server_secret`, so a token of one kind never verifies as the other even if a `purpose` claim were forged. Verification here is
//! strict: no leeway on `exp`, and a lifetime longer than we ever mint is refused.
use crate::Docserver;
use crate::sessions::SessionRecord;
use convert_api::jwt;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

pub const SESSION_TTL_S: u64 = 15 * 60;
pub const FILE_TTL_S: u64 = 10 * 60;

pub struct SessionClaims {
    pub sid: String,
    pub sub: String,
    pub prov: String,
}

pub struct FileClaims {
    pub sid: String,
    pub ver: u64,
    pub sha256: String,
}

/// Constant-time equality for two byte strings (digests).
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |x, (p, q)| x | (p ^ q)) == 0
}

impl Docserver {
    /// `HMAC(server_secret, label)`: the key of one token kind.
    fn subkey(&self, label: &str) -> Vec<u8> {
        let mut m = Hmac::<Sha256>::new_from_slice(&self.server_secret).expect("any key length");
        m.update(label.as_bytes());
        m.finalize().into_bytes().to_vec()
    }

    /// A session token for `rec`, valid 15 minutes but never past the session's absolute end.
    pub fn mint_session_token(&self, rec: &SessionRecord, now: u64) -> String {
        let exp = (now + SESSION_TTL_S).min(rec.expires);
        jwt::sign(
            &self.subkey("session"),
            &json!({"purpose": "session", "sid": rec.sid, "sub": rec.user_id, "prov": rec.provider,
                    "perm": if rec.perms.edit { "edit" } else { "view" }, "iat": now, "exp": exp}),
        )
    }

    fn checked(&self, label: &str, token: &str, ttl: u64, now: u64) -> Option<Value> {
        let p = jwt::verify(&self.subkey(label), token).ok()?;
        let exp = p.get("exp")?.as_u64()?;
        (p.get("purpose")?.as_str()? == label && exp >= now && exp <= now + ttl + 5).then_some(p)
    }

    pub fn session_claims(&self, token: &str, now: u64) -> Option<SessionClaims> {
        let p = self.checked("session", token, SESSION_TTL_S, now)?;
        let s = |k: &str| p.get(k)?.as_str().map(str::to_string);
        Some(SessionClaims {
            sid: s("sid")?,
            sub: s("sub")?,
            prov: s("prov")?,
        })
    }

    /// A file token for version `ver` of session `sid`, bound to the version's digest, valid 10 minutes.
    pub fn mint_file_token(&self, sid: &str, ver: u64, sha256: &str, now: u64) -> String {
        jwt::sign(
            &self.subkey("file"),
            &json!({"purpose": "file", "sid": sid, "ver": ver, "sha256": sha256, "exp": now + FILE_TTL_S}),
        )
    }

    pub fn file_claims(&self, token: &str, now: u64) -> Option<FileClaims> {
        let p = self.checked("file", token, FILE_TTL_S, now)?;
        Some(FileClaims {
            sid: p.get("sid")?.as_str()?.to_string(),
            ver: p.get("ver")?.as_u64()?,
            sha256: p.get("sha256")?.as_str()?.to_string(),
        })
    }
}
