//! The server configuration (a TOML file, `convert-api --config FILE`) and the checks that make a bad combination a refusal to
//! start (PLAN 5.5): `auth = "none"` only on a loopback address, `auth = "jwt"` only with TLS (built in, or terminated by a
//! trusted proxy), a secret and a public URL.
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

/// What a provider speaks (PLAN phase 4, 5.6). `Oidc` is the seam for Phase 5 SSO: it parses, and `resolve` refuses it for now.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Jwt,
    Oidc,
}

/// One `[[providers]]` entry: configuration, never code. `name` is for the operator's logs and the account page; `issuer` is the
/// `iss` the provider's tokens carry.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub name: String,
    pub issuer: String,
    pub kind: ProviderKind,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Settings {
    /// `ip:port`; an IP literal, so there is no name to resolve to something else
    pub listen: String,
    /// `"none"` (local mode, the default) or `"jwt"` (server mode)
    pub auth: String,
    /// server mode: the issuer's shared HS256 secret (at least 32 bytes). Use a secret that only this service shares with the issuer.
    pub secret_file: Option<PathBuf>,
    /// server mode: accept only tokens whose `iss` is this
    pub iss: Option<String>,
    /// server mode: accept only tokens whose `aud` is (or lists) this
    pub aud: Option<String>,
    /// server mode: identity providers. Phase 4 knows one `kind = "jwt"` entry, whose `issuer` is the required `iss` (the same as
    /// setting `iss`; both may be set when equal). `kind = "oidc"` is refused until Phase 5. None by default.
    pub providers: Vec<Provider>,
    /// The `[docserver]` section, opaque here: the `docserver` crate checks it, and `suite-serve` mounts it (PLAN phase 5, WP2).
    pub docserver: Option<toml::Table>,
    /// server mode: the longest remaining life a token may have when it arrives (it cannot be revoked), default 12 h
    pub max_token_lifetime_s: u64,
    /// Not used since the job keys stopped being written to disk (WP7 review M5). Accepted so that an older config still loads;
    /// when set it must still be 32 random bytes (raw, or base64 or hex of them), so a passphrase is caught early.
    pub key_file: Option<PathBuf>,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    /// `"builtin"` (default): this server speaks TLS itself. `"upstream"`: a trusted proxy terminates TLS and talks plain HTTP to
    /// this server; needs a non-empty `trusted_proxies`, and the server then answers only that proxy.
    pub tls: String,
    /// IPs or CIDRs of reverse proxies. Only a peer in this list may set `X-Forwarded-For` and `X-Forwarded-Proto`; for every other
    /// peer the headers are ignored. Empty by default.
    pub trusted_proxies: Vec<String>,
    /// server mode with `tls = "builtin"`: a plain-HTTP listener that only redirects to `public_url`
    pub redirect_listen: Option<String>,
    /// `https://host[:port]` the clients use (no path); the redirect target and the allowed `Host`. Required in server mode.
    pub public_url: Option<String>,
    /// scratch for the tools. Dedicated to this server: stale entries are swept at start. Server mode: put it on a RAM disk.
    pub work_dir: Option<PathBuf>,
    /// where the (encrypted) job store lives; defaults to `<work_dir>-store`
    pub store_dir: Option<PathBuf>,
    /// XDFU documents (PLAN 10): `"off"` (the default: nothing is detected, logged or marked), `"watermark"` or `"refuse"`
    pub xdfu: String,
    /// the hash-chained audit log of XDFU jobs. Required in server mode, and it must be on storage that survives a restart (not the
    /// RAM disk of `work_dir`); local mode defaults to the user's data directory.
    pub xdfu_audit_file: Option<PathBuf>,
    /// the operator's key of the audit log's hash chain (HMAC-SHA256): 32 or more random bytes, kept outside the log. Without it the
    /// chain only stops a naive edit and the server says so at start.
    pub xdfu_audit_key_file: Option<PathBuf>,
    /// leave file names out of the audit log
    pub xdfu_redact: bool,
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
    /// the store (inputs not yet converted, outputs, zips) never grows beyond this; a new job that could push it over is a 503
    pub store_max_mb: u64,
    /// a new job is refused while the volume of `store_dir` has less than this (plus the batch limit) free
    pub min_free_mb: u64,
    /// the most one `sub` may keep in the store at once; a new job past it is a 429
    pub user_store_mb: u64,
    /// how long a request waits for a CPU slot for its upload check before it answers 503
    pub slot_wait_s: u64,
    /// connections one client address (an IPv6 /64) may hold open, and all clients together
    pub max_conn_per_ip: usize,
    pub max_connections: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            listen: "127.0.0.1:8787".into(),
            auth: "none".into(),
            secret_file: None,
            iss: None,
            aud: None,
            providers: vec![],
            docserver: None,
            max_token_lifetime_s: 12 * 3600,
            key_file: None,
            tls_cert: None,
            tls_key: None,
            tls: "builtin".into(),
            trusted_proxies: vec![],
            redirect_listen: None,
            public_url: None,
            work_dir: None,
            store_dir: None,
            xdfu: "off".into(),
            xdfu_audit_file: None,
            xdfu_audit_key_file: None,
            xdfu_redact: false,
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
            store_max_mb: 20 * 1024,
            min_free_mb: 2048,
            user_store_mb: 2048,
            slot_wait_s: 30,
            max_conn_per_ip: 32,
            max_connections: 1024,
        }
    }
}

#[derive(Clone)]
pub enum Auth {
    /// local mode
    None,
    Jwt {
        secret: Vec<u8>,
        rules: crate::jwt::Rules,
    },
}

/// An IP or a CIDR range (`10.0.0.0/8`, `fd00::/8`, `::1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    addr: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Result<Cidr, String> {
        let bad = || format!("trusted_proxies: '{s}' is not an IP address or a CIDR range");
        let (a, p) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let addr: IpAddr = a
            .trim()
            .parse::<IpAddr>()
            .map_err(|_| bad())?
            .to_canonical();
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match p {
            Some(p) => p
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= max)
                .ok_or_else(bad)?,
            None => max,
        };
        Ok(Cidr { addr, prefix })
    }

    pub fn prefix_len(&self) -> u8 {
        self.prefix
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let bits = |ip: IpAddr| -> (u128, u8) {
            match ip.to_canonical() {
                IpAddr::V4(v) => (u32::from(v) as u128, 32),
                IpAddr::V6(v) => (u128::from(v), 128),
            }
        };
        let ((a, an), (b, bn)) = (bits(self.addr), bits(ip));
        if an != bn {
            return false;
        }
        let shift = (an - self.prefix) as u32;
        shift >= 128 || (a >> shift) == (b >> shift)
    }
}

/// `Settings` after the checks and the file reads.
#[derive(Clone)]
pub struct Resolved {
    pub s: Settings,
    pub listen: SocketAddr,
    pub auth: Auth,
    pub work_dir: PathBuf,
    pub store_dir: PathBuf,
    pub cpu_slots: usize,
    pub trusted: Vec<Cidr>,
    /// `tls = "upstream"`: TLS ends at a trusted proxy
    pub upstream_tls: bool,
    pub xdfu: convert::xdfu::Mode,
}

impl Resolved {
    pub fn server_mode(&self) -> bool {
        matches!(self.auth, Auth::Jwt { .. })
    }
    /// Job files are encrypted at rest exactly in server mode.
    pub fn encrypt(&self) -> bool {
        self.server_mode()
    }
    pub fn is_trusted_proxy(&self, ip: IpAddr) -> bool {
        self.trusted.iter().any(|c| c.contains(ip))
    }
}

/// 32 random bytes: raw, or base64 (either alphabet, padded or not), or 64 hex characters. A passphrase is not one of those
/// even when it happens to have 32 characters, so printable-only raw bytes and keys with few distinct bytes are refused.
fn read_key(path: &Path) -> Result<[u8; 32], String> {
    use base64::Engine;
    let raw =
        std::fs::read(path).map_err(|e| format!("cannot read key_file {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&raw);
    let text = text.trim();
    let hex = (text.len() == 64 && text.is_ascii())
        .then(|| {
            (0..32)
                .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()
        })
        .flatten();
    let b64 = [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|e| e.decode(text).ok().filter(|b| b.len() == 32));
    let printable = |b: &[u8]| b.iter().all(|c| (0x20..0x7f).contains(c));
    let key: Vec<u8> = match (raw.len() == 32 && !printable(&raw)).then(|| raw.clone()).or(hex).or(b64) {
        Some(k) => k,
        None => return Err("key_file must hold 32 random bytes (raw, base64 or 64 hex characters), not a passphrase".into()),
    };
    let distinct = key.iter().collect::<std::collections::BTreeSet<_>>().len();
    if distinct < 16 {
        return Err(
            "key_file does not look random (fewer than 16 distinct byte values in 32)".into(),
        );
    }
    Ok(key.try_into().expect("32 bytes"))
}

/// `https://host[:port]`, nothing else: no path, query, fragment or user info.
fn check_public_url(u: &str) -> Result<(), String> {
    let bad = || {
        format!(
            "public_url = \"{u}\" must look like https://host[:port] (no path, query or user info)"
        )
    };
    let rest = u
        .strip_prefix("https://")
        .ok_or_else(bad)?
        .trim_end_matches('/');
    if rest.is_empty() || rest.contains(['/', '@', '?', '#', ' ', '\\']) || rest.starts_with(':') {
        return Err(bad());
    }
    Ok(())
}

impl Settings {
    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The `iss` rule after the providers: the single jwt provider's issuer, or `iss`; refuses what Phase 4 cannot honour.
    fn provider_issuer(&self) -> Result<Option<String>, String> {
        if let Some(p) = self.providers.iter().find(|p| p.kind == ProviderKind::Oidc) {
            return Err(format!(
                "providers: \"{}\": kind = \"oidc\" is available in a later version",
                p.name
            ));
        }
        if self
            .providers
            .iter()
            .any(|p| p.name.trim().is_empty() || p.issuer.trim().is_empty())
        {
            return Err("providers: name and issuer must not be empty".into());
        }
        if self.providers.len() > 1 {
            return Err("providers: this version takes one provider".into());
        }
        if !self.providers.is_empty() && self.auth != "jwt" {
            return Err("providers belong to server mode (auth = \"jwt\")".into());
        }
        match (self.providers.first(), &self.iss) {
            (Some(p), Some(i)) if p.issuer != *i => Err(format!(
                "providers: issuer \"{}\" differs from iss = \"{i}\"",
                p.issuer
            )),
            (Some(p), _) => Ok(Some(p.issuer.clone())),
            (None, i) => Ok(i.clone()),
        }
    }

    pub fn resolve(self) -> Result<Resolved, String> {
        let listen: SocketAddr = self
            .listen
            .parse()
            .map_err(|_| format!("listen = \"{}\" is not an ip:port", self.listen))?;
        let xdfu = convert::xdfu::Mode::parse(&self.xdfu)?;
        let trusted = self
            .trusted_proxies
            .iter()
            .map(|s| Cidr::parse(s))
            .collect::<Result<Vec<_>, _>>()?;
        if !matches!(self.tls.as_str(), "builtin" | "upstream") {
            return Err(format!(
                "tls = \"{}\": expected \"builtin\" or \"upstream\"",
                self.tls
            ));
        }
        let upstream_tls = self.tls == "upstream";
        let iss = self.provider_issuer()?;
        let auth = match self.auth.as_str() {
            "none" => {
                if !listen.ip().is_loopback() {
                    return Err(format!(
                        "auth = \"none\" is refused on {listen}: local mode is for a loopback address only (use auth = \"jwt\" for a network server)"
                    ));
                }
                if self.redirect_listen.is_some()
                    || self.tls_cert.is_some()
                    || self.tls_key.is_some()
                {
                    return Err("tls_cert, tls_key and redirect_listen belong to server mode (auth = \"jwt\")".into());
                }
                if self.public_url.is_some() {
                    return Err("public_url belongs to server mode (auth = \"jwt\"); local mode serves the loopback names only".into());
                }
                if upstream_tls || !trusted.is_empty() {
                    return Err("tls = \"upstream\" and trusted_proxies belong to server mode (auth = \"jwt\")".into());
                }
                Auth::None
            }
            "jwt" => {
                let need = |o: &Option<PathBuf>, k: &str| {
                    o.clone().ok_or(format!("auth = \"jwt\" needs {k}"))
                };
                let secret = std::fs::read(need(&self.secret_file, "secret_file")?)
                    .map_err(|e| format!("cannot read secret_file: {e}"))?;
                let secret = secret.trim_ascii_end().to_vec();
                if secret.len() < 32 {
                    return Err("secret_file must hold at least 32 bytes".into());
                }
                let url = self.public_url.as_deref().ok_or(
                    "auth = \"jwt\" needs public_url (the Host check and the redirect target)",
                )?;
                check_public_url(url)?;
                if upstream_tls {
                    if trusted.is_empty() {
                        return Err("tls = \"upstream\" needs trusted_proxies: only a listed proxy may speak for TLS".into());
                    }
                    if self.tls_cert.is_some()
                        || self.tls_key.is_some()
                        || self.redirect_listen.is_some()
                    {
                        return Err(
                            "tls_cert, tls_key and redirect_listen belong to tls = \"builtin\""
                                .into(),
                        );
                    }
                } else {
                    need(&self.tls_cert, "tls_cert")?;
                    need(&self.tls_key, "tls_key")?;
                    if self.redirect_listen.is_some() && self.public_url.is_none() {
                        return Err("redirect_listen needs public_url".into());
                    }
                }
                if let Some(k) = &self.key_file {
                    read_key(k)?;
                }
                if xdfu != convert::xdfu::Mode::Off && self.xdfu_audit_file.is_none() {
                    return Err("xdfu needs xdfu_audit_file in server mode: the audit log must live on storage that survives a restart".into());
                }
                if self.max_token_lifetime_s == 0 {
                    return Err("max_token_lifetime_s must be positive".into());
                }
                Auth::Jwt {
                    secret,
                    rules: crate::jwt::Rules {
                        iss,
                        aud: self.aud.clone(),
                        max_lifetime_s: self.max_token_lifetime_s,
                    },
                }
            }
            other => return Err(format!("auth = \"{other}\": expected \"none\" or \"jwt\"")),
        };
        if let Some(k) = &self.xdfu_audit_key_file {
            check_audit_key(k)?;
        }
        let work_dir = self
            .work_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("convert-api-work"));
        for (name, d) in [
            ("work_dir", Some(&work_dir)),
            ("store_dir", self.store_dir.as_ref()),
        ] {
            if let Some(d) = d.filter(|d| cfg!(windows) && msys_path(&d.to_string_lossy())) {
                return Err(format!(
                    "{name} = \"{}\" is an MSYS path (/c/Users/...); on Windows it would land in C:\\c\\...; write C:/Users/... instead",
                    d.display()
                ));
            }
        }
        let store_dir = self.store_dir.clone().unwrap_or_else(|| {
            let name = work_dir.file_name().map_or("convert-api-work".into(), |n| {
                n.to_string_lossy().into_owned()
            });
            work_dir.with_file_name(format!("{name}-store"))
        });
        if work_dir == store_dir {
            eprintln!(
                "convert-api: warning: work_dir and store_dir are the same directory; they should be separate so each can be sized and placed on its own volume"
            );
        }
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let cpu_slots = self.cpu_slots.unwrap_or((cores / 3).clamp(1, 4)).max(1);
        Ok(Resolved {
            listen,
            auth,
            work_dir,
            store_dir,
            cpu_slots,
            trusted,
            upstream_tls,
            xdfu,
            s: self,
        })
    }
}

/// The audit key: the 32 random bytes of `read_key` in any of its spellings, or longer raw random bytes. A passphrase is refused.
fn check_audit_key(path: &Path) -> Result<(), String> {
    let raw = std::fs::read(path)
        .map_err(|e| format!("cannot read xdfu_audit_key_file {}: {e}", path.display()))?;
    if read_key(path).is_ok() {
        return Ok(());
    }
    let distinct = raw.iter().collect::<std::collections::BTreeSet<_>>().len();
    let printable = raw
        .iter()
        .all(|c| (0x20..0x7f).contains(c) || matches!(c, b'\r' | b'\n'));
    if raw.len() >= 32 && !printable && distinct >= 16 {
        return Ok(());
    }
    Err("xdfu_audit_key_file must hold 32 or more random bytes (raw, or 64 hex characters, or base64 of 32 bytes), not a passphrase".into())
}

/// `/c/Users/me/x` or `\c\Users\me`: a Git-Bash path written into a config file. Windows reads it as `C:\c\Users\...` on the current
/// drive, so a job store would silently appear in a stray folder.
fn msys_path(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3
        && matches!(b[0], b'/' | b'\\')
        && b[1].is_ascii_alphabetic()
        && matches!(b[2], b'/' | b'\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_are_a_seam_one_jwt_provider_sets_iss_and_oidc_waits() {
        let toml = |t: &str| toml::from_str::<Settings>(t).unwrap();
        let p = |kind: &str| {
            format!(
                "[[providers]]
name = \"idp\"
issuer = \"https://id.test\"
kind = \"{kind}\"
"
            )
        };
        // local mode takes none
        let e = toml(&p("jwt")).resolve().err().unwrap();
        assert!(e.contains("server mode"), "{e}");
        let e = toml(&p("oidc")).resolve().err().unwrap();
        assert!(e.contains("later version"), "{e}");
        assert!(toml::from_str::<Settings>(&p("saml")).is_err());
        assert!(toml("").providers.is_empty());
        let mut s = toml(&p("jwt"));
        s.auth = "jwt".into();
        assert_eq!(
            s.provider_issuer().unwrap().as_deref(),
            Some("https://id.test")
        );
        s.iss = Some("https://other".into());
        assert!(s.provider_issuer().unwrap_err().contains("differs"));
        s.providers.push(s.providers[0].clone());
        s.iss = None;
        assert!(s.provider_issuer().unwrap_err().contains("one provider"));
    }

    #[test]
    fn auth_none_is_refused_off_loopback_and_xdfu_is_off_only() {
        let s = |listen: &str| Settings {
            listen: listen.into(),
            ..Settings::default()
        };
        assert!(s("127.0.0.1:1").resolve().is_ok());
        assert!(s("[::1]:1").resolve().is_ok());
        // any 127.0.0.0/8 address is loopback; an IPv4-mapped IPv6 address is not (`Ipv6Addr::is_loopback` is `::1` only)
        assert!(s("127.0.0.2:1").resolve().is_ok());
        for bad in [
            "0.0.0.0:8787",
            "192.168.1.5:8787",
            "[::]:8787",
            "[::ffff:127.0.0.1]:8787",
            "[::ffff:192.168.1.5]:8787",
            "localhost:8787",
        ] {
            let e = s(bad).resolve().err().unwrap_or_default();
            assert!(
                e.contains("loopback") || e.contains("not an ip:port"),
                "{bad}: {e}"
            );
        }
        let e = Settings {
            xdfu: "on".into(),
            ..Settings::default()
        }
        .resolve()
        .err()
        .unwrap();
        assert!(e.contains("watermark") && e.contains("refuse"), "{e}");
        let e = Settings {
            auth: "jwt".into(),
            ..Settings::default()
        }
        .resolve()
        .err()
        .unwrap();
        assert!(e.contains("secret_file"), "{e}");
    }

    #[test]
    fn local_mode_takes_no_public_url_and_server_mode_needs_a_good_one() {
        let e = Settings {
            public_url: Some("https://x.test".into()),
            ..Settings::default()
        }
        .resolve()
        .err()
        .unwrap();
        assert!(e.contains("public_url"), "{e}");
        let dir = std::env::temp_dir().join(format!("capi-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("secret"), [b's'; 40]).unwrap();
        let server = |f: &dyn Fn(&mut Settings)| {
            let mut s = Settings {
                auth: "jwt".into(),
                secret_file: Some(dir.join("secret")),
                tls_cert: Some("c.pem".into()),
                tls_key: Some("k.pem".into()),
                public_url: Some("https://convert.test".into()),
                ..Settings::default()
            };
            f(&mut s);
            s.resolve().err()
        };
        assert!(server(&|_| {}).is_none());
        assert!(
            server(&|s| s.public_url = None)
                .unwrap()
                .contains("needs public_url")
        );
        for bad in [
            "http://x.test",
            "https://x.test/app",
            "https://u@x.test",
            "https://",
            "x.test",
            "https://x.test?a=1",
        ] {
            assert!(
                server(&|s| s.public_url = Some(bad.into()))
                    .unwrap()
                    .contains("public_url"),
                "{bad}"
            );
        }
        assert!(server(&|s| s.public_url = Some("https://x.test:8443/".into())).is_none());
        // the proxy settings
        let e = server(&|s| s.tls = "upstream".into()).unwrap();
        assert!(e.contains("trusted_proxies"), "{e}");
        let up = |s: &mut Settings| {
            s.tls = "upstream".into();
            s.trusted_proxies = vec!["10.1.0.0/16".into()];
            s.tls_cert = None;
            s.tls_key = None;
        };
        assert!(server(&up).is_none());
        assert!(
            server(&|s| {
                up(s);
                s.tls_cert = Some("c.pem".into());
            })
            .unwrap()
            .contains("builtin")
        );
        assert!(
            server(&|s| s.trusted_proxies = vec!["not-an-ip".into()])
                .unwrap()
                .contains("trusted_proxies")
        );
        assert!(
            server(&|s| s.tls = "plain".into())
                .unwrap()
                .contains("builtin")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cidr_matching_and_the_store_default() {
        let c = |s: &str| Cidr::parse(s).unwrap();
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(c("10.0.0.0/8").contains(ip("10.9.8.7")));
        assert!(!c("10.0.0.0/8").contains(ip("11.0.0.1")));
        assert!(c("192.168.1.5").contains(ip("192.168.1.5")));
        assert!(!c("192.168.1.5").contains(ip("192.168.1.6")));
        assert!(c("fd00::/8").contains(ip("fd12::1")));
        assert!(!c("fd00::/8").contains(ip("fe80::1")));
        assert!(
            c("10.0.0.0/8").contains(ip("::ffff:10.1.1.1")),
            "a mapped address is its IPv4 address"
        );
        assert!(!c("10.0.0.0/8").contains(ip("fd00::1")));
        assert!(c("0.0.0.0/0").contains(ip("1.2.3.4")));
        assert!(Cidr::parse("10.0.0.0/33").is_err() && Cidr::parse("x").is_err());
        let r = Settings {
            work_dir: Some("<server-path>".into()),
            ..Settings::default()
        }
        .resolve()
        .unwrap();
        assert_eq!(r.store_dir, PathBuf::from("<server-path>"));
        assert_ne!(
            Settings::default().resolve().unwrap().store_dir,
            Settings::default().resolve().unwrap().work_dir
        );
    }

    #[test]
    fn an_msys_path_is_told_apart_from_a_windows_one() {
        for bad in ["/c/Users/x/work", "\\c\\Users\\x"] {
            assert!(msys_path(bad), "{bad}");
        }
        for ok in [
            "C:/Users/x",
            "C:\\Users",
            "<server-path>",
            "/s",
            "//host/share/x",
            "relative/c/x",
        ] {
            assert!(!msys_path(ok), "{ok}");
        }
    }

    #[test]
    fn the_audit_key_file_is_checked_like_key_file() {
        let dir = std::env::temp_dir().join(format!("capi-akey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let with = |name: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).unwrap();
            Settings {
                xdfu_audit_key_file: Some(dir.join(name)),
                ..Settings::default()
            }
            .resolve()
            .map(|_| ())
        };
        let random: Vec<u8> = (0..48u8)
            .map(|i| i.wrapping_mul(37).wrapping_add(11) | 0x80)
            .collect();
        assert!(with("raw48", &random).is_ok());
        assert!(with("raw32", &random[..32]).is_ok());
        assert!(
            with("pass", b"correct horse battery staple, long enough")
                .unwrap_err()
                .contains("passphrase")
        );
        assert!(with("short", &random[..20]).is_err());
        assert!(with("same", &[7u8; 40]).is_err());
        assert!(
            Settings {
                xdfu_audit_key_file: Some(dir.join("missing")),
                ..Settings::default()
            }
            .resolve()
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_file_must_be_random_bytes_not_a_passphrase() {
        use base64::Engine;
        let dir = std::env::temp_dir().join(format!("capi-key-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let try_key = |name: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).unwrap();
            read_key(&dir.join(name))
        };
        let random: Vec<u8> = (0..32u8)
            .map(|i| i.wrapping_mul(37).wrapping_add(11) | 0x80)
            .collect();
        assert_eq!(try_key("raw", &random).unwrap().to_vec(), random);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&random);
        assert_eq!(try_key("b64", b64.as_bytes()).unwrap().to_vec(), random);
        let hex: String = random.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            try_key("hex", format!("{hex}\n").as_bytes())
                .unwrap()
                .to_vec(),
            random
        );
        // a 32-character passphrase is 32 bytes too, and so is a key of one repeated byte
        assert!(
            try_key("pass", b"correct horse battery staple 12")
                .unwrap_err()
                .contains("passphrase")
        );
        assert!(try_key("short", b"tooshort").is_err());
        assert!(try_key("same", &[7u8; 32]).unwrap_err().contains("random"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
