//! The server configuration (a TOML file, `convert-api --config FILE`) and the checks that make a bad combination a refusal to
//! start (PLAN 5.5): `auth = "none"` only on a loopback address, `auth = "jwt"` only with TLS, a secret and a key file.
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Settings {
    /// `ip:port`; an IP literal, so there is no name to resolve to something else
    pub listen: String,
    /// `"none"` (local mode, the default) or `"jwt"` (server mode)
    pub auth: String,
    /// server mode: the issuer's shared HS256 secret (at least 32 bytes)
    pub secret_file: Option<PathBuf>,
    /// server mode: accept only tokens whose `iss` is this
    pub iss: Option<String>,
    /// server mode: the operator key that wraps every job key (32 raw bytes, or 64 hex characters)
    pub key_file: Option<PathBuf>,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    /// server mode: a plain-HTTP listener that only redirects to `public_url`
    pub redirect_listen: Option<String>,
    /// `https://host[:port]` the clients use; the redirect target and the allowed `Host`
    pub public_url: Option<String>,
    /// scratch for the tools and the job store. Dedicated to this server: stale entries are swept at start.
    pub work_dir: Option<PathBuf>,
    /// where the (encrypted) job store lives; defaults to `work_dir`
    pub store_dir: Option<PathBuf>,
    /// `"off"`; any other value is refused until WP15
    pub xdfu: String,
    pub cpu_slots: Option<usize>,
    pub mem_budget_mb: u64,
    pub queue_max: usize,
    pub per_user_active: usize,
    pub max_file_mb: u64,
    pub max_xlsx_mb: u64,
    pub max_batch_files: usize,
    pub max_batch_mb: u64,
    pub retention_s: u64,
    pub upload_timeout_s: u64,
    pub job_timeout_s: u64,
    pub rate_user_per_min: u32,
    pub rate_ip_per_min: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            listen: "127.0.0.1:8787".into(),
            auth: "none".into(),
            secret_file: None,
            iss: None,
            key_file: None,
            tls_cert: None,
            tls_key: None,
            redirect_listen: None,
            public_url: None,
            work_dir: None,
            store_dir: None,
            xdfu: "off".into(),
            cpu_slots: None,
            mem_budget_mb: 12 * 1024,
            queue_max: 100,
            per_user_active: 3,
            max_file_mb: 50,
            max_xlsx_mb: 100,
            max_batch_files: 30,
            max_batch_mb: 300,
            retention_s: 3600,
            upload_timeout_s: 900,
            job_timeout_s: 30 * 60,
            rate_user_per_min: 30,
            rate_ip_per_min: 120,
        }
    }
}

#[derive(Clone)]
pub enum Auth {
    /// local mode
    None,
    Jwt { secret: Vec<u8>, iss: Option<String> },
}

/// `Settings` after the checks and the file reads.
#[derive(Clone)]
pub struct Resolved {
    pub s: Settings,
    pub listen: SocketAddr,
    pub auth: Auth,
    /// the operator key (server mode)
    pub operator_key: Option<[u8; 32]>,
    pub work_dir: PathBuf,
    pub store_dir: PathBuf,
    pub cpu_slots: usize,
}

impl Resolved {
    pub fn server_mode(&self) -> bool {
        matches!(self.auth, Auth::Jwt { .. })
    }
}

fn read_key(path: &Path) -> Result<[u8; 32], String> {
    let raw = std::fs::read(path).map_err(|e| format!("cannot read key_file {}: {e}", path.display()))?;
    if let Ok(b) = <[u8; 32]>::try_from(raw.as_slice()) {
        return Ok(b);
    }
    let hex = String::from_utf8_lossy(&raw);
    let hex = hex.trim();
    if hex.len() == 64 && hex.is_ascii() {
        let mut b = [0u8; 32];
        for (i, o) in b.iter_mut().enumerate() {
            *o = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|_| "key_file is not valid hex".to_string())?;
        }
        return Ok(b);
    }
    Err("key_file must hold exactly 32 raw bytes or 64 hex characters".into())
}

impl Settings {
    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn resolve(self) -> Result<Resolved, String> {
        let listen: SocketAddr = self.listen.parse().map_err(|_| format!("listen = \"{}\" is not an ip:port", self.listen))?;
        if self.xdfu != "off" {
            return Err(format!("xdfu = \"{}\": not implemented yet (only \"off\")", self.xdfu));
        }
        let (auth, operator_key) = match self.auth.as_str() {
            "none" => {
                if !listen.ip().is_loopback() {
                    return Err(format!("auth = \"none\" is refused on {listen}: local mode is for a loopback address only (use auth = \"jwt\" for a network server)"));
                }
                if self.redirect_listen.is_some() || self.tls_cert.is_some() {
                    return Err("tls_cert and redirect_listen belong to server mode (auth = \"jwt\")".into());
                }
                (Auth::None, None)
            }
            "jwt" => {
                let need = |o: &Option<PathBuf>, k: &str| o.clone().ok_or(format!("auth = \"jwt\" needs {k}"));
                let secret = std::fs::read(need(&self.secret_file, "secret_file")?).map_err(|e| format!("cannot read secret_file: {e}"))?;
                let secret = secret.trim_ascii_end().to_vec();
                if secret.len() < 32 {
                    return Err("secret_file must hold at least 32 bytes".into());
                }
                need(&self.tls_cert, "tls_cert")?;
                need(&self.tls_key, "tls_key")?;
                let key = read_key(&need(&self.key_file, "key_file")?)?;
                if self.redirect_listen.is_some() && self.public_url.is_none() {
                    return Err("redirect_listen needs public_url".into());
                }
                (Auth::Jwt { secret, iss: self.iss.clone() }, Some(key))
            }
            other => return Err(format!("auth = \"{other}\": expected \"none\" or \"jwt\"")),
        };
        let work_dir = self.work_dir.clone().unwrap_or_else(|| std::env::temp_dir().join("convert-api-work"));
        let store_dir = self.store_dir.clone().unwrap_or_else(|| work_dir.clone());
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let cpu_slots = self.cpu_slots.unwrap_or((cores / 3).clamp(1, 4)).max(1);
        Ok(Resolved { listen, auth, operator_key, work_dir, store_dir, cpu_slots, s: self })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_none_is_refused_off_loopback_and_xdfu_is_off_only() {
        let s = |listen: &str| Settings { listen: listen.into(), ..Settings::default() };
        assert!(s("127.0.0.1:1").resolve().is_ok());
        assert!(s("[::1]:1").resolve().is_ok());
        for bad in ["0.0.0.0:8787", "192.168.1.5:8787", "[::]:8787"] {
            let e = s(bad).resolve().err().unwrap_or_default();
            assert!(e.contains("loopback"), "{bad}: {e}");
        }
        let e = Settings { xdfu: "on".into(), ..Settings::default() }.resolve().err().unwrap();
        assert!(e.contains("not implemented yet"), "{e}");
        let e = Settings { auth: "jwt".into(), ..Settings::default() }.resolve().err().unwrap();
        assert!(e.contains("secret_file"), "{e}");
    }
}
