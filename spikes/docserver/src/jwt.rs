//! Hand-rolled HS256 JWT (hmac + sha2 + base64). Replaces `jsonwebtoken` to avoid its
//! `rsa` dependency (RUSTSEC-2023-0071); ~60 lines, as the spike plan suggests for Phase 5.
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum JwtError {
    Malformed,
    BadAlgorithm,
    BadSignature,
    Expired,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mac(secret: &[u8]) -> HmacSha256 {
    HmacSha256::new_from_slice(secret).expect("hmac accepts any key length")
}

pub fn sign(secret: &[u8], payload: &Value) -> String {
    let head = B64.encode(json!({"alg": "HS256", "typ": "JWT"}).to_string());
    let body = B64.encode(payload.to_string());
    let input = format!("{head}.{body}");
    let mut m = mac(secret);
    m.update(input.as_bytes());
    format!("{input}.{}", B64.encode(m.finalize().into_bytes()))
}

/// Verify an HS256 token and return its payload. `alg` must be exactly `HS256` (so `none`
/// and every other algorithm is rejected). `exp` is optional, and checked when present.
pub fn verify(secret: &[u8], token: &str) -> Result<Value, JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::Malformed);
    }
    let header: Value = serde_json::from_slice(&B64.decode(parts[0]).map_err(|_| JwtError::Malformed)?)
        .map_err(|_| JwtError::Malformed)?;
    if header.get("alg").and_then(Value::as_str) != Some("HS256") {
        return Err(JwtError::BadAlgorithm);
    }
    let sig = B64.decode(parts[2]).map_err(|_| JwtError::Malformed)?;
    let mut m = mac(secret);
    m.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    m.verify_slice(&sig).map_err(|_| JwtError::BadSignature)?; // constant time
    let payload: Value = serde_json::from_slice(&B64.decode(parts[1]).map_err(|_| JwtError::Malformed)?)
        .map_err(|_| JwtError::Malformed)?;
    if let Some(exp) = payload.get("exp").and_then(Value::as_u64) {
        if exp < now() {
            return Err(JwtError::Expired);
        }
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_config_and_payload_wrapper() {
        let cfg = json!({"document": {"key": "k1", "url": "http://x/y"}, "exp": now() + 60});
        assert_eq!(verify(b"s3", &sign(b"s3", &cfg)).unwrap(), cfg);
        let wrapped = json!({"payload": {"key": "k1", "status": 6}});
        assert_eq!(verify(b"s3", &sign(b"s3", &wrapped)).unwrap(), wrapped);
    }

    #[test]
    fn tampered_token_rejected() {
        let good = sign(b"s3", &json!({"a": 1}));
        let mut bytes = good.clone().into_bytes();
        let i = bytes.len() - 3; // inside the signature
        bytes[i] = if bytes[i] == b'A' { b'B' } else { b'A' };
        assert!(verify(b"s3", &String::from_utf8(bytes).unwrap()).is_err());
        // tampered payload with the original signature
        let parts: Vec<&str> = good.split('.').collect();
        let forged = format!("{}.{}.{}", parts[0], B64.encode(json!({"a": 2}).to_string()), parts[2]);
        assert_eq!(verify(b"s3", &forged), Err(JwtError::BadSignature));
    }

    #[test]
    fn alg_none_rejected() {
        let head = B64.encode(json!({"alg": "none", "typ": "JWT"}).to_string());
        let body = B64.encode(json!({"a": 1}).to_string());
        assert_eq!(verify(b"s3", &format!("{head}.{body}.")), Err(JwtError::BadAlgorithm));
        let head = B64.encode(json!({"alg": "HS512", "typ": "JWT"}).to_string());
        assert_eq!(verify(b"s3", &format!("{head}.{body}.AAAA")), Err(JwtError::BadAlgorithm));
    }

    #[test]
    fn wrong_secret_and_expired_rejected() {
        let t = sign(b"s3", &json!({"a": 1}));
        assert_eq!(verify(b"other", &t), Err(JwtError::BadSignature));
        let old = sign(b"s3", &json!({"exp": 1}));
        assert_eq!(verify(b"s3", &old), Err(JwtError::Expired));
    }
}
