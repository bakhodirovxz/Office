//! Hand-rolled HS256 JWT, ported from spike 4 (`spikes/docserver/src/jwt.rs`) with these changes: `exp` is required (the spike
//! checked it only when present), `nbf` is honoured, both with a 60 s leeway, and `claims` adds the `sub`, `scope`, optional
//! `iss` and `aud` rules of PLAN 5.5 and a maximum lifetime (WP7 review m1).
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, PartialEq, Eq)]
pub enum JwtError {
    Malformed,
    BadAlgorithm,
    BadSignature,
    Expired,
    NoExpiry,
    NotYetValid,
    BadClaims(&'static str),
}

/// Clock drift between the issuer and this server that is tolerated on `exp` and `nbf`.
pub const LEEWAY_S: u64 = 60;

/// What `claims` demands beyond a valid signature.
#[derive(Clone, Debug)]
pub struct Rules {
    pub iss: Option<String>,
    pub aud: Option<String>,
    /// a token whose `exp` is further away than this is refused (there is no revocation, so a leaked token must run out)
    pub max_lifetime_s: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
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

/// Verify an HS256 token and return its payload. `alg` must be exactly `HS256` (so `none` and every other algorithm is
/// rejected), `exp` must be present and not past (60 s leeway) and `nbf`, when present, not in the future (60 s leeway).
pub fn verify(secret: &[u8], token: &str) -> Result<Value, JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::Malformed);
    }
    let header: Value = serde_json::from_slice(&B64.decode(parts[0]).map_err(|_| JwtError::Malformed)?).map_err(|_| JwtError::Malformed)?;
    if header.get("alg").and_then(Value::as_str) != Some("HS256") {
        return Err(JwtError::BadAlgorithm);
    }
    let sig = B64.decode(parts[2]).map_err(|_| JwtError::Malformed)?;
    let mut m = mac(secret);
    m.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    m.verify_slice(&sig).map_err(|_| JwtError::BadSignature)?; // constant time
    let payload: Value = serde_json::from_slice(&B64.decode(parts[1]).map_err(|_| JwtError::Malformed)?).map_err(|_| JwtError::Malformed)?;
    let t = now();
    match payload.get("exp").and_then(Value::as_u64) {
        None => Err(JwtError::NoExpiry),
        Some(exp) if exp + LEEWAY_S < t => Err(JwtError::Expired),
        Some(_) if payload.get("nbf").is_some_and(|n| n.as_u64().is_none_or(|n| n > t + LEEWAY_S)) => Err(JwtError::NotYetValid),
        Some(_) => Ok(payload),
    }
}

/// A verified token as the API needs it: the user (`sub`). Needs `scope == "convert"`, and `iss` when the operator set one.
pub fn claims(secret: &[u8], token: &str, rules: &Rules) -> Result<String, JwtError> {
    let p = verify(secret, token)?;
    if p.get("scope").and_then(Value::as_str) != Some("convert") {
        return Err(JwtError::BadClaims("scope"));
    }
    if rules.iss.as_deref().is_some_and(|i| p.get("iss").and_then(Value::as_str) != Some(i)) {
        return Err(JwtError::BadClaims("iss"));
    }
    if let Some(a) = rules.aud.as_deref() {
        let hit = match p.get("aud") {
            Some(Value::String(s)) => s == a,
            Some(Value::Array(v)) => v.iter().any(|x| x.as_str() == Some(a)),
            _ => false,
        };
        if !hit {
            return Err(JwtError::BadClaims("aud"));
        }
    }
    // `verify` made sure `exp` exists
    if p.get("exp").and_then(Value::as_u64).is_some_and(|e| e > now() + rules.max_lifetime_s) {
        return Err(JwtError::BadClaims("lifetime"));
    }
    match p.get("sub").and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.chars().count() <= 128 && !s.chars().any(char::is_control) => Ok(s.to_string()),
        _ => Err(JwtError::BadClaims("sub")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_config_and_payload_wrapper() {
        let cfg = json!({"document": {"key": "k1", "url": "http://x/y"}, "exp": now() + 60});
        assert_eq!(verify(b"s3", &sign(b"s3", &cfg)).unwrap(), cfg);
        let wrapped = json!({"payload": {"key": "k1", "status": 6}, "exp": now() + 60});
        assert_eq!(verify(b"s3", &sign(b"s3", &wrapped)).unwrap(), wrapped);
    }

    #[test]
    fn tampered_token_rejected() {
        let good = sign(b"s3", &json!({"a": 1, "exp": now() + 60}));
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
    fn wrong_secret_expired_and_missing_exp_rejected() {
        let t = sign(b"s3", &json!({"a": 1, "exp": now() + 60}));
        assert_eq!(verify(b"other", &t), Err(JwtError::BadSignature));
        let old = sign(b"s3", &json!({"exp": 1}));
        assert_eq!(verify(b"s3", &old), Err(JwtError::Expired));
        assert_eq!(verify(b"s3", &sign(b"s3", &json!({"a": 1}))), Err(JwtError::NoExpiry));
    }

    #[test]
    fn claims_need_scope_sub_and_the_issuer() {
        let rules = Rules { iss: Some("idp".into()), aud: None, max_lifetime_s: 3600 };
        let t = |p: Value| claims(b"s3", &sign(b"s3", &p), &rules);
        let ok = json!({"sub": "u1", "exp": now() + 60, "scope": "convert", "iss": "idp"});
        assert_eq!(t(ok.clone()).unwrap(), "u1");
        assert_eq!(t(json!({"sub": "u1", "exp": now() + 60, "scope": "admin", "iss": "idp"})), Err(JwtError::BadClaims("scope")));
        assert_eq!(t(json!({"sub": "u1", "exp": now() + 60, "scope": "convert", "iss": "other"})), Err(JwtError::BadClaims("iss")));
        assert_eq!(t(json!({"exp": now() + 60, "scope": "convert", "iss": "idp"})), Err(JwtError::BadClaims("sub")));
        assert_eq!(claims(b"s3", &sign(b"s3", &ok), &Rules { iss: None, ..rules }).unwrap(), "u1");
    }

    #[test]
    fn nbf_leeway_lifetime_and_audience() {
        let rules = Rules { iss: None, aud: Some("convert-api".into()), max_lifetime_s: 3600 };
        let base = |extra: Value| {
            let mut p = json!({"sub": "u1", "exp": now() + 600, "scope": "convert", "aud": "convert-api"});
            p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            claims(b"s3", &sign(b"s3", &p), &rules)
        };
        assert_eq!(base(json!({})).unwrap(), "u1");
        // not before: 30 s ahead is inside the leeway, 5 min ahead is not
        assert!(base(json!({"nbf": now() + 30})).is_ok());
        assert_eq!(base(json!({"nbf": now() + 300})), Err(JwtError::NotYetValid));
        // expired 30 s ago is inside the leeway, 5 min ago is not
        assert!(base(json!({"exp": now() - 30})).is_ok());
        assert_eq!(base(json!({"exp": now() - 300})), Err(JwtError::Expired));
        // a token valid for 100 years is refused, one for the maximum is not
        assert_eq!(base(json!({"exp": now() + 100 * 365 * 86400})), Err(JwtError::BadClaims("lifetime")));
        assert!(base(json!({"exp": now() + 3500})).is_ok());
        // audience: a string or a list, and it must be there when configured
        assert!(base(json!({"aud": ["other", "convert-api"]})).is_ok());
        assert_eq!(base(json!({"aud": "other"})), Err(JwtError::BadClaims("aud")));
        let no_aud = sign(b"s3", &json!({"sub": "u1", "exp": now() + 600, "scope": "convert"}));
        assert_eq!(claims(b"s3", &no_aud, &rules), Err(JwtError::BadClaims("aud")));
    }
}
