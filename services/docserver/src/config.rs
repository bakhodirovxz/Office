//! The `[docserver]` section of the server config and the checks that make a bad combination a refusal to start (PLAN phase 5, 3.2,
//! 4 and S4). A message names the key and never prints a secret.
use convert_api::config::Cidr;
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

/// One `fetch_allow` / `callback_allow` entry: `"http://host:port"`, or `{ url = "http://host:port", internal = true }` when the
/// address may be a private one (a Docker network name, for example).
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum AllowEntry {
    Url(String),
    Table(AllowTable),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AllowTable {
    pub url: String,
    #[serde(default)]
    pub internal: bool,
}

/// One `[[docserver.providers]]` entry: configuration, never code.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub name: String,
    /// the `iss` the provider's config tokens carry
    pub issuer: String,
    /// the provider's shared HS256 secret (32 bytes or more); differs from `server_secret_file` and from every other provider's
    pub secret_file: Option<PathBuf>,
    /// where the provider's documents may be fetched from
    #[serde(default)]
    pub fetch_allow: Vec<AllowEntry>,
    /// where the saved-file callback may be posted
    #[serde(default)]
    pub callback_allow: Vec<AllowEntry>,
    /// `scheme://host[:port]` of the pages that may frame the editor and load `api.js`
    #[serde(default)]
    pub frame_origins: Vec<String>,
    /// refuse config tokens without `exp` (default true)
    #[serde(default = "yes")]
    pub require_exp: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// the key of our own session and file tokens: 32 bytes or more, different from every provider's secret
    pub server_secret_file: Option<PathBuf>,
    /// sessions and their versions live here (WP3)
    pub data_dir: Option<PathBuf>,
    pub max_document_mb: u64,
    pub callback_timeout_s: u64,
    /// `ip:port` of the internal listener that serves the saved-file route only (F4); needs `internal_allow`
    pub internal_listen: Option<String>,
    /// IPs or CIDRs that may use the internal listener
    pub internal_allow: Vec<String>,
    pub encrypt_at_rest: bool,
    pub providers: Vec<ProviderConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_secret_file: None,
            data_dir: None,
            max_document_mb: 50,
            callback_timeout_s: 60,
            internal_listen: None,
            internal_allow: vec![],
            encrypt_at_rest: false,
            providers: vec![],
        }
    }
}

/// An allowlist entry after the checks: `origin` is the exact `scheme://host:port`, lower case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allow {
    pub origin: String,
    pub internal: bool,
}

/// A provider after the checks and the secret read. No `Debug`: it holds a secret.
#[derive(Clone)]
pub struct Provider {
    pub name: String,
    pub issuer: String,
    pub secret: Vec<u8>,
    pub fetch_allow: Vec<Allow>,
    pub callback_allow: Vec<Allow>,
    pub frame_origins: Vec<String>,
    pub require_exp: bool,
}

#[derive(Clone, Debug)]
pub struct Internal {
    pub listen: SocketAddr,
    pub allow: Vec<Cidr>,
}

/// The checked configuration. No `Debug`: it holds secrets.
#[derive(Clone)]
pub struct Docserver {
    pub server_secret: Vec<u8>,
    pub data_dir: PathBuf,
    pub max_document_mb: u64,
    pub callback_timeout_s: u64,
    pub internal: Option<Internal>,
    pub encrypt_at_rest: bool,
    pub providers: Vec<Provider>,
}

fn read_secret(path: &Path, key: &str) -> Result<Vec<u8>, String> {
    let s = std::fs::read(path)
        .map_err(|e| format!("docserver: {key}: cannot read {}: {e}", path.display()))?;
    let s = s.trim_ascii_end().to_vec();
    if s.len() < 32 {
        return Err(format!("docserver: {key} must hold at least 32 bytes"));
    }
    Ok(s)
}

/// `scheme://host[:port]` and nothing else: http or https, a plain host name, an IPv4 address or a bracketed IPv6 address, no user
/// info, path, query, fragment, wildcard or separator that could change a CSP source list. Returns the lower-case form; with
/// `need_port` the port must be written out.
pub(crate) fn origin(s: &str, key: &str, need_port: bool) -> Result<String, String> {
    let bad = |why: &str| {
        format!("docserver: {key}: \"{s}\" is not a scheme://host[:port] origin ({why})")
    };
    let (scheme, rest) = s.split_once("://").ok_or_else(|| bad("no scheme"))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(bad("the scheme must be http or https"));
    }
    if !rest.is_ascii()
        || rest
            .chars()
            .any(|c| c.is_ascii_control() || " /@?#\\;,'\"*%".contains(c))
    {
        return Err(bad("no path, user info, wildcard or special character"));
    }
    let (host, port) = if let Some(r) = rest.strip_prefix('[') {
        let (h, p) = r.split_once(']').ok_or_else(|| bad("unclosed ["))?;
        h.parse::<std::net::Ipv6Addr>()
            .map_err(|_| bad("bad IPv6 address"))?;
        let port = match p.strip_prefix(':') {
            Some(p) => Some(p),
            None if p.is_empty() => None,
            None => return Err(bad("bad port")),
        };
        (format!("[{h}]"), port)
    } else {
        match rest.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (rest.to_string(), None),
        }
    };
    let host = host.to_ascii_lowercase();
    let plain = host.starts_with('[')
        || (!host.is_empty()
            && host.split('.').all(|l| {
                !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            }));
    if !plain {
        return Err(bad("bad host name"));
    }
    match port.map(|p| p.parse::<u16>().ok().filter(|p| *p != 0)) {
        Some(None) => Err(bad("bad port")),
        Some(Some(p)) => Ok(format!("{scheme}://{host}:{p}")),
        None if need_port => Err(bad("the port must be written out")),
        None => Ok(format!("{scheme}://{host}")),
    }
}

fn allow_list(v: &[AllowEntry], key: &str) -> Result<Vec<Allow>, String> {
    v.iter()
        .map(|e| {
            let (url, internal) = match e {
                AllowEntry::Url(u) => (u.as_str(), false),
                AllowEntry::Table(t) => (t.url.as_str(), t.internal),
            };
            Ok(Allow {
                origin: origin(url, key, true)?,
                internal,
            })
        })
        .collect()
}

impl Docserver {
    /// The `[docserver]` table of the server config, checked. `main_listen` is the public listener, which the internal one must
    /// not share.
    pub fn from_table(
        t: toml::Table,
        main_listen: Option<SocketAddr>,
    ) -> Result<Docserver, String> {
        t.try_into::<Config>()
            .map_err(|e| format!("docserver: {e}"))?
            .resolve(main_listen)
    }

    /// The `frame-ancestors` source list of the wrapper page and the editor frames: ourselves and every provider's frame origins.
    pub fn frame_ancestors(&self) -> String {
        let mut v = vec!["'self'".to_string()];
        for o in self.providers.iter().flat_map(|p| &p.frame_origins) {
            if !v.contains(o) {
                v.push(o.clone());
            }
        }
        v.join(" ")
    }

    /// Binds the internal listener (a refusal to start when the address is taken). `None` when none is configured.
    pub fn bind_internal(&self) -> Result<Option<std::net::TcpListener>, String> {
        let Some(i) = &self.internal else {
            return Ok(None);
        };
        let l = std::net::TcpListener::bind(i.listen)
            .map_err(|e| format!("docserver: cannot listen on {}: {e}", i.listen))?;
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Some(l))
    }

    pub fn internal_allows(&self, ip: IpAddr) -> bool {
        self.internal
            .as_ref()
            .is_some_and(|i| i.allow.iter().any(|c| c.contains(ip)))
    }
}

impl Config {
    pub fn resolve(self, main_listen: Option<SocketAddr>) -> Result<Docserver, String> {
        let need =
            |o: &Option<PathBuf>, k: &str| o.clone().ok_or(format!("docserver: {k} is required"));
        let server_secret = read_secret(
            &need(&self.server_secret_file, "server_secret_file")?,
            "server_secret_file",
        )?;
        let data_dir = need(&self.data_dir, "data_dir")?;
        if data_dir.as_os_str().is_empty() {
            return Err("docserver: data_dir must not be empty".into());
        }
        if !(1..=500).contains(&self.max_document_mb) {
            return Err("docserver: max_document_mb must be 1 to 500".into());
        }
        if !(1..=600).contains(&self.callback_timeout_s) {
            return Err("docserver: callback_timeout_s must be 1 to 600".into());
        }
        let internal = match (&self.internal_listen, self.internal_allow.is_empty()) {
            (None, true) => None,
            (None, false) => return Err("docserver: internal_allow needs internal_listen".into()),
            (Some(_), true) => {
                return Err("docserver: internal_listen needs internal_allow: the internal listener answers listed addresses only".into());
            }
            (Some(l), false) => {
                let listen: SocketAddr = l.parse().map_err(|_| {
                    format!("docserver: internal_listen = \"{l}\" is not an ip:port")
                })?;
                if Some(listen) == main_listen {
                    return Err("docserver: internal_listen is the public listen address".into());
                }
                let allow = self
                    .internal_allow
                    .iter()
                    .map(|c| {
                        Cidr::parse(c)
                            .ok()
                            .filter(|c| c.prefix_len() > 0)
                            .ok_or(format!("docserver: internal_allow: \"{c}\" is not an IP address or a CIDR range (a /0 range is not accepted)"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Some(Internal { listen, allow })
            }
        };
        if self.providers.is_empty() {
            return Err("docserver: at least one [[docserver.providers]] entry is required".into());
        }
        let mut providers: Vec<Provider> = vec![];
        for (i, p) in self.providers.iter().enumerate() {
            let at = |k: &str| format!("providers[{i}].{k}");
            if p.name.trim().is_empty() || p.issuer.trim().is_empty() {
                return Err(format!(
                    "docserver: providers[{i}]: name and issuer must not be empty"
                ));
            }
            let secret_path = p
                .secret_file
                .as_deref()
                .ok_or(format!("docserver: {} is required", at("secret_file")))?;
            let secret = read_secret(secret_path, &at("secret_file"))?;
            if secret == server_secret {
                return Err(format!(
                    "docserver: {} holds the same secret as server_secret_file",
                    at("secret_file")
                ));
            }
            if let Some(o) = providers
                .iter()
                .find(|o| o.name == p.name || o.issuer == p.issuer || o.secret == secret)
            {
                let what = if o.secret == secret {
                    "secret_file holds the same secret as"
                } else {
                    "name or issuer repeats"
                };
                return Err(format!(
                    "docserver: providers[{i}]: {what} provider \"{}\"",
                    o.name
                ));
            }
            let fetch_allow = allow_list(&p.fetch_allow, &at("fetch_allow"))?;
            if fetch_allow.is_empty() {
                return Err(format!(
                    "docserver: {} must list at least one scheme://host:port",
                    at("fetch_allow")
                ));
            }
            let callback_allow = allow_list(&p.callback_allow, &at("callback_allow"))?;
            if p.frame_origins.is_empty() {
                return Err(format!(
                    "docserver: {} must list at least one origin",
                    at("frame_origins")
                ));
            }
            let mut frame_origins = vec![];
            for o in &p.frame_origins {
                let o = origin(o, &at("frame_origins"), false)?;
                if !frame_origins.contains(&o) {
                    frame_origins.push(o);
                }
            }
            providers.push(Provider {
                name: p.name.clone(),
                issuer: p.issuer.clone(),
                secret,
                fetch_allow,
                callback_allow,
                frame_origins,
                require_exp: p.require_exp,
            });
        }
        for p in providers.iter().filter(|p| !p.require_exp) {
            eprintln!(
                "docserver: warning: provider \"{}\": require_exp = false accepts config tokens that never expire",
                p.name
            );
        }
        Ok(Docserver {
            server_secret,
            data_dir,
            max_document_mb: self.max_document_mb,
            callback_timeout_s: self.callback_timeout_s,
            internal,
            encrypt_at_rest: self.encrypt_at_rest,
            providers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Dir {
            let d =
                std::env::temp_dir().join(format!("docserver-cfg-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&d).unwrap();
            for (f, b) in [("server", b's'), ("a", b'a'), ("b", b'b')] {
                std::fs::write(d.join(f), [b; 40]).unwrap();
            }
            Dir(d)
        }
        fn path(&self, f: &str) -> String {
            self.0.join(f).to_string_lossy().replace('\\', "/")
        }
        fn toml(&self, edit: &dyn Fn(&mut toml::Table)) -> Result<Docserver, String> {
            let mut t: toml::Table = format!(
                r#"server_secret_file = "{}"
data_dir = "{}"
[[providers]]
name = "one"
issuer = "https://issuer.test"
secret_file = "{}"
fetch_allow = ["http://api:8000"]
callback_allow = [{{ url = "http://api:8000", internal = true }}]
frame_origins = ["https://app.test"]
"#,
                self.path("server"),
                self.path("data"),
                self.path("a")
            )
            .parse()
            .unwrap();
            edit(&mut t);
            Docserver::from_table(t, Some("127.0.0.1:8787".parse().unwrap()))
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn prov(t: &mut toml::Table) -> &mut toml::Table {
        t.get_mut("providers")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .first_mut()
            .unwrap()
            .as_table_mut()
            .unwrap()
    }
    fn strings(v: &[&str]) -> toml::Value {
        v.iter().map(|s| s.to_string()).collect::<Vec<_>>().into()
    }
    fn refused(d: &Dir, edit: &dyn Fn(&mut toml::Table), needle: &str) {
        let e = d
            .toml(edit)
            .err()
            .unwrap_or_else(|| panic!("accepted, expected '{needle}'"));
        assert!(e.contains(needle), "{needle}: {e}");
        assert!(
            !e.contains(&"s".repeat(32)) && !e.contains(&"a".repeat(32)),
            "a secret leaked: {e}"
        );
    }

    #[test]
    fn a_good_config_resolves() {
        let d = Dir::new("good");
        let ds = d.toml(&|_| {}).unwrap();
        assert_eq!(
            ds.providers[0].fetch_allow,
            [Allow {
                origin: "http://api:8000".into(),
                internal: false
            }]
        );
        assert!(ds.providers[0].callback_allow[0].internal && ds.providers[0].require_exp);
        assert_eq!((ds.max_document_mb, ds.callback_timeout_s), (50, 60));
        assert_eq!(ds.frame_ancestors(), "'self' https://app.test");
        assert!(ds.internal.is_none());
    }

    #[test]
    fn refuses_without_a_server_secret_or_with_a_short_one() {
        let d = Dir::new("srv");
        refused(
            &d,
            &|t| {
                t.remove("server_secret_file");
            },
            "server_secret_file is required",
        );
        refused(
            &d,
            &|t| {
                t.insert("server_secret_file".into(), "/nonexistent/x".into());
            },
            "server_secret_file: cannot read",
        );
        std::fs::write(d.0.join("short"), b"too short").unwrap();
        let short = d.path("short");
        refused(
            &d,
            &|t| {
                t.insert("server_secret_file".into(), short.clone().into());
            },
            "server_secret_file must hold at least 32 bytes",
        );
        refused(
            &d,
            &|t| {
                t.remove("data_dir");
            },
            "data_dir is required",
        );
    }

    #[test]
    fn refuses_no_providers_and_bad_provider_fields() {
        let d = Dir::new("prov");
        refused(
            &d,
            &|t| {
                t.remove("providers");
            },
            "at least one [[docserver.providers]]",
        );
        refused(
            &d,
            &|t| {
                prov(t).remove("secret_file");
            },
            "providers[0].secret_file is required",
        );
        std::fs::write(d.0.join("short"), b"x").unwrap();
        let short = d.path("short");
        refused(
            &d,
            &|t| {
                prov(t).insert("secret_file".into(), short.clone().into());
            },
            "providers[0].secret_file must hold at least 32 bytes",
        );
        refused(
            &d,
            &|t| {
                prov(t).insert("name".into(), " ".into());
            },
            "name and issuer must not be empty",
        );
        refused(
            &d,
            &|t| {
                prov(t).insert("frobnicate".into(), true.into());
            },
            "frobnicate",
        );
        refused(
            &d,
            &|t| {
                prov(t).remove("fetch_allow");
            },
            "fetch_allow must list",
        );
        refused(
            &d,
            &|t| {
                prov(t).remove("frame_origins");
            },
            "frame_origins must list",
        );
    }

    #[test]
    fn secrets_must_differ() {
        let d = Dir::new("sec");
        let server = d.path("server");
        refused(
            &d,
            &|t| {
                prov(t).insert("secret_file".into(), server.clone().into());
            },
            "same secret as server_secret_file",
        );
        std::fs::write(d.0.join("a2"), [b'a'; 40]).unwrap();
        let two = |secret: String, name: &'static str, issuer: &'static str| {
            move |t: &mut toml::Table| {
                let mut p = prov(t).clone();
                p.insert("secret_file".into(), secret.clone().into());
                p.insert("name".into(), name.into());
                p.insert("issuer".into(), issuer.into());
                t.get_mut("providers")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .push(p.into());
            }
        };
        refused(
            &d,
            &two(d.path("a2"), "two", "https://two.test"),
            "same secret as provider \"one\"",
        );
        refused(
            &d,
            &two(d.path("b"), "one", "https://two.test"),
            "repeats provider \"one\"",
        );
        refused(
            &d,
            &two(d.path("b"), "two", "https://issuer.test"),
            "repeats provider \"one\"",
        );
        d.toml(&two(d.path("b"), "two", "https://two.test"))
            .unwrap();
    }

    #[test]
    fn allowlists_are_exact_origins_with_a_port() {
        let d = Dir::new("allow");
        for bad in [
            "api:8000",
            "http://api",
            "ftp://api:8000",
            "http://user@api:8000",
            "http://api:8000/x",
            "http://api:8000/",
            "http://*.test:80",
            "http://api:0",
            "http://api:99999",
            "http://a pi:80",
            "http://:80",
            "https://api:443?x=1",
            "http://[::1:80",
            "http://api:80;x",
        ] {
            for key in ["fetch_allow", "callback_allow"] {
                refused(
                    &d,
                    &|t| {
                        prov(t).insert(key.into(), strings(&[bad]));
                    },
                    &format!("providers[0].{key}"),
                );
            }
        }
        let ds = d
            .toml(&|t| {
                prov(t).insert(
                    "fetch_allow".into(),
                    strings(&["HTTPS://Files.Test:08443", "http://[::1]:80"]),
                );
            })
            .unwrap();
        assert_eq!(
            ds.providers[0].fetch_allow[0].origin,
            "https://files.test:8443"
        );
        assert_eq!(ds.providers[0].fetch_allow[1].origin, "http://[::1]:80");
        // a misspelt key of the table form is not silently dropped
        refused(
            &d,
            &|t| {
                let e: toml::Table = [
                    ("url".to_string(), "http://a:1".into()),
                    ("intern".to_string(), true.into()),
                ]
                .into_iter()
                .collect();
                prov(t).insert("callback_allow".into(), vec![toml::Value::Table(e)].into());
            },
            "docserver",
        );
    }

    #[test]
    fn frame_origins_are_origins_only() {
        let d = Dir::new("frame");
        for bad in [
            "*",
            "https://*.test",
            "https://a.test/",
            "https://a.test/x",
            "a.test",
            "https://a.test; script-src *",
            "https://a.test 'unsafe-inline'",
            "ws://a.test",
            "'self'",
            "https://a.test,https://b.test",
        ] {
            refused(
                &d,
                &|t| {
                    prov(t).insert("frame_origins".into(), strings(&[bad]));
                },
                "providers[0].frame_origins",
            );
        }
        let ds = d
            .toml(&|t| {
                prov(t).insert(
                    "frame_origins".into(),
                    strings(&[
                        "https://App.Test",
                        "http://localhost:3000",
                        "https://app.test",
                    ]),
                );
            })
            .unwrap();
        assert_eq!(
            ds.frame_ancestors(),
            "'self' https://app.test http://localhost:3000"
        );
    }

    #[test]
    fn the_internal_listener_needs_a_listen_address_and_a_cidr_list() {
        let d = Dir::new("internal");
        let set = |listen: Option<&'static str>, allow: &'static [&'static str]| {
            move |t: &mut toml::Table| {
                if let Some(l) = listen {
                    t.insert("internal_listen".into(), l.into());
                }
                t.insert("internal_allow".into(), strings(allow));
            }
        };
        refused(
            &d,
            &set(None, &["10.0.0.0/8"]),
            "internal_allow needs internal_listen",
        );
        refused(
            &d,
            &set(Some("10.0.0.5:9000"), &[]),
            "internal_listen needs internal_allow",
        );
        refused(
            &d,
            &set(Some("api:9000"), &["10.0.0.0/8"]),
            "not an ip:port",
        );
        refused(
            &d,
            &set(Some("127.0.0.1:8787"), &["10.0.0.0/8"]),
            "public listen address",
        );
        refused(
            &d,
            &set(Some("10.0.0.5:9000"), &["nonsense"]),
            "internal_allow",
        );
        refused(&d, &set(Some("10.0.0.5:9000"), &["0.0.0.0/0"]), "/0");
        let ds = d
            .toml(&set(Some("10.0.0.5:9000"), &["172.18.0.0/16", "fd00::/8"]))
            .unwrap();
        assert!(
            ds.internal_allows("172.18.0.7".parse().unwrap())
                && ds.internal_allows("fd12::1".parse().unwrap())
        );
        assert!(
            !ds.internal_allows("172.19.0.7".parse().unwrap())
                && !ds.internal_allows("127.0.0.1".parse().unwrap())
        );
    }

    #[test]
    fn limits_and_unknown_keys_are_checked() {
        let d = Dir::new("limits");
        for (k, v) in [
            ("max_document_mb", 0),
            ("max_document_mb", 501),
            ("callback_timeout_s", 0),
            ("callback_timeout_s", 601),
        ] {
            refused(
                &d,
                &|t| {
                    t.insert(k.into(), v.into());
                },
                k,
            );
        }
        refused(
            &d,
            &|t| {
                t.insert("server_secret".into(), "x".into());
            },
            "server_secret",
        );
        let ds = d
            .toml(&|t| {
                t.insert("max_document_mb".into(), 500.into());
                prov(t).insert("require_exp".into(), false.into());
            })
            .unwrap();
        assert!(!ds.providers[0].require_exp && ds.max_document_mb == 500);
    }
}
