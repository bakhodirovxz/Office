//! Hand-rolled HS256 (the spike's `jwt.rs`). `exp` is optional and checked when present, as the HR backend's tokens carry none
//! (hrm-current-state.md:265, hr-backend-prompt.md:829). `alg` is pinned, so `none` and every other algorithm is refused.
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

#[derive(Debug, PartialEq, Eq)]
pub enum JwtError {
    Malformed,
    BadAlgorithm,
    BadSignature,
    Expired,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mac(secret: &[u8]) -> Hmac<Sha256> {
    Hmac::<Sha256>::new_from_slice(secret).expect("hmac accepts any key length")
}

pub fn sign(secret: &[u8], payload: &Value) -> String {
    let input = format!(
        "{}.{}",
        B64.encode(json!({"alg": "HS256", "typ": "JWT"}).to_string()),
        B64.encode(payload.to_string())
    );
    let mut m = mac(secret);
    m.update(input.as_bytes());
    format!("{input}.{}", B64.encode(m.finalize().into_bytes()))
}

pub fn verify(secret: &[u8], token: &str) -> Result<Value, JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::Malformed);
    }
    let dec = |s: &str| -> Result<Value, JwtError> {
        serde_json::from_slice(&B64.decode(s).map_err(|_| JwtError::Malformed)?)
            .map_err(|_| JwtError::Malformed)
    };
    if dec(parts[0])?.get("alg").and_then(Value::as_str) != Some("HS256") {
        return Err(JwtError::BadAlgorithm);
    }
    let sig = B64.decode(parts[2]).map_err(|_| JwtError::Malformed)?;
    let mut m = mac(secret);
    m.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    m.verify_slice(&sig).map_err(|_| JwtError::BadSignature)?;
    let payload = dec(parts[1])?;
    if payload
        .get("exp")
        .and_then(Value::as_u64)
        .is_some_and(|e| e < now())
    {
        return Err(JwtError::Expired);
    }
    Ok(payload)
}
