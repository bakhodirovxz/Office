//! SSRF-guarded outbound HTTP: scheme + exact host:port allowlist, no redirects, size + time caps.
use std::time::Duration;
use url::Url;

#[derive(Debug)]
pub enum FetchError {
    Blocked(String),
    Upstream(String),
}

/// Scheme must be http(s) and `host:port` must be in the allowlist (exact match).
pub fn check_allowed(raw: &str, allowed: &[String]) -> Result<Url, FetchError> {
    let url = Url::parse(raw).map_err(|e| FetchError::Blocked(format!("bad url: {e}")))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(FetchError::Blocked(format!("scheme {} not allowed", url.scheme())));
    }
    let host = url.host_str().ok_or_else(|| FetchError::Blocked("no host".into()))?;
    let port = url.port_or_known_default().unwrap_or(0);
    let hp = format!("{host}:{port}");
    if !allowed.iter().any(|a| a == &hp) {
        return Err(FetchError::Blocked(format!("host not allowed: {hp}")));
    }
    Ok(url)
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

pub async fn fetch(client: &reqwest::Client, url: Url, max: usize) -> Result<Vec<u8>, FetchError> {
    let mut resp = client.get(url).send().await.map_err(|e| FetchError::Upstream(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        // includes 3xx: redirects are never followed
        return Err(FetchError::Upstream(format!("upstream status {status}")));
    }
    if resp.content_length().map(|l| l as usize > max).unwrap_or(false) {
        return Err(FetchError::Upstream("document too large".into()));
    }
    let mut out = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| FetchError::Upstream(e.to_string()))? {
        if out.len() + chunk.len() > max {
            return Err(FetchError::Upstream("document too large".into()));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allowlist() {
        let a = vec!["127.0.0.1:8790".to_string()];
        assert!(check_allowed("http://127.0.0.1:8790/files/x", &a).is_ok());
        assert!(check_allowed("http://169.254.169.254/latest", &a).is_err());
        assert!(check_allowed("http://127.0.0.1:9999/x", &a).is_err());
        assert!(check_allowed("file:///etc/passwd", &a).is_err());
        assert!(check_allowed("http://127.0.0.1.evil.com:8790/", &a).is_err());
    }
}
