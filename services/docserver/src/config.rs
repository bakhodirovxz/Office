//! The `[docserver]` section of the server config and the checks that make a bad combination a refusal to start (PLAN phase 5, 3.2,
//! 4 and S4). A message names the key and never prints a secret.
use convert_api::config::{Cidr, NameSet};
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
    /// refuse config tokens without `exp` (default true; false with `compat = "docsapi"`)
    pub require_exp: Option<bool>,
    /// `"docsapi"`: a backend written for another DocsAPI document server, unchanged (docs/phase5/dropin.md D1, D4): config tokens
    /// without `exp`, and a secret of 16 bytes or more (a warning under 32). Every other check stays.
    pub compat: Option<String>,
    /// the file types whose sessions may edit (default: every served type); an edit config of another type opens as a view
    pub edit_types: Option<Vec<String>>,
    /// `"session"` (default): a document's history goes with the last session of its `document.key` (the retention sweep), right
    /// for a provider that sends a new key on every open. `"keep"`: the history stays under the store's own limits, for a provider
    /// with stable keys.
    #[serde(default = "session")]
    pub history: String,
    /// view sessions may list, download and compare the document's history too (default false: edit sessions only; gate review L4)
    #[serde(default)]
    pub view_history: bool,
    /// `uz-Latn`, `uz-Cyrl`, `ru` or `en`: the editors' language for this provider's sessions, over `editorConfig.lang` (a provider
    /// whose own UI speaks a language the DocsAPI does not name, and so always sends another). Unset: `editorConfig.lang` decides
    pub ui_lang: Option<String>,
}

fn session() -> String {
    "session".into()
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
    /// total time allowed for fetching a document (seconds)
    pub fetch_timeout_s: u64,
    /// `ip:port` of the internal listener that serves the saved-file route only (F4); needs `internal_allow`. The public `listen`
    /// address itself: the shared layout (docs/phase5/dropin.md D2b), one port whose saved-file route only `internal_allow` reaches
    pub internal_listen: Option<String>,
    /// IPs or CIDRs that may use the internal listener
    pub internal_allow: Vec<String>,
    /// `http://host:port` the provider uses to reach the internal listener; it goes into the saved-file URL of a callback. Default: the
    /// `internal_listen` address (an unspecified one as 127.0.0.1)
    pub internal_url: Option<String>,
    /// seal session files, versions, autosave and history objects under `data_dir` (WP14); needs `encrypt_key_file`
    pub encrypt_at_rest: bool,
    /// the at-rest key: 32 random bytes or more, kept outside `data_dir`, different from every other secret
    pub encrypt_key_file: Option<PathBuf>,
    /// `"off"` (default: nothing is registered or stamped) or `"watermark"`: every download or callback copy of a document that carries
    /// the grif gets the cl. 12 stamp (WP14). Needs `encrypt_at_rest` and `audit_key_file`. `"refuse"` is a converter-server option only.
    pub xdfu: String,
    /// the audit log (default `<data_dir>/audit.jsonl`); an existing log must verify or the server does not start
    pub audit_file: Option<PathBuf>,
    /// the operator's key of the audit chain (32 bytes or more, kept outside the data directory); without it the chain only stops a naive edit
    pub audit_key_file: Option<PathBuf>,
    /// the folder holding `docs-diff` and `sheets-diff` (the History compare route); unset: the route answers 501
    pub diff_bin_dir: Option<PathBuf>,
    /// leave document titles out of the audit log
    pub redact: bool,
    /// session create attempts per client address and minute
    pub rate_create_per_min: u32,
    /// failed authentications per client address and minute; over it the answer is 429
    pub rate_auth_fail_per_min: u32,
    /// a session folder is deleted this long after a close the provider acknowledged (PLAN phase 5, Q10)
    pub keep_closed_hours: u64,
    /// any other session folder (never closed, or its close not acknowledged) is deleted this long after its last activity (Q10)
    pub keep_abandoned_days: u64,
    /// an open edit session with no activity for this long (no ping, save or autosave: its page went away without a close) is closed
    /// by the server, with its recovery copy as the last version (default 5)
    pub close_idle_minutes: u64,
    /// a save, restore, autosave or fetch is refused (507) while the volume of `data_dir` would keep less than this free after it
    /// (gate review L1); 0 turns the floor off. Default 1024
    pub min_free_mb: u64,
    /// the most versions one session stores; a save or restore past it is refused (507) and the close still keeps the last edits
    /// (gate review L1). Default 500
    pub max_versions_per_session: u64,
    /// accept a host page with an opaque (`null`) origin (docs/phase5/dropin.md D3b): no `frame-ancestors` and no `X-Frame-Options` on
    /// the wrapper page and the editors, and `null` counts as a frame origin of every provider. Any page can make its origin opaque,
    /// so this turns the frame check off for everyone; the config token's signature stays the gate. Default false.
    pub allow_opaque_origin: bool,
    pub providers: Vec<ProviderConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_secret_file: None,
            data_dir: None,
            max_document_mb: 50,
            callback_timeout_s: 60,
            fetch_timeout_s: 60,
            internal_listen: None,
            internal_allow: vec![],
            internal_url: None,
            encrypt_at_rest: false,
            encrypt_key_file: None,
            xdfu: "off".into(),
            audit_file: None,
            audit_key_file: None,
            diff_bin_dir: None,
            redact: false,
            rate_create_per_min: 30,
            rate_auth_fail_per_min: 20,
            keep_closed_hours: 168, // 7 days, as abandoned sessions (owner, 2026-10-31)
            keep_abandoned_days: 7,
            close_idle_minutes: 5,
            min_free_mb: 1024,
            max_versions_per_session: 500,
            allow_opaque_origin: false,
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
    /// `history = "keep"`: the retention sweep leaves this provider's histories alone
    pub keep_history: bool,
    /// `view_history`: view sessions see the history too
    pub view_history: bool,
    /// the file types that may open in edit mode (lower case)
    pub edit_types: Vec<String>,
    /// `ui_lang`: the editors' language, over `editorConfig.lang`
    pub ui_lang: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Internal {
    pub listen: SocketAddr,
    pub allow: Vec<Cidr>,
    /// the container names of `internal_allow`, resolved again every 15 s (a backend recreated with a new address)
    pub names: NameSet,
    /// `listen` is the public listener's own address: no socket of its own, the file route is split off the public port
    pub shared: bool,
}

/// The checked configuration. No `Debug`: it holds secrets.
#[derive(Clone)]
pub struct Docserver {
    pub server_secret: Vec<u8>,
    pub data_dir: PathBuf,
    pub max_document_mb: u64,
    pub callback_timeout_s: u64,
    pub fetch_timeout_s: u64,
    pub internal: Option<Internal>,
    /// how the provider reaches the internal listener (no trailing slash); None without one
    pub internal_url: Option<String>,
    /// the at-rest key, when `encrypt_at_rest` is on
    pub at_rest: Option<std::sync::Arc<crate::atrest::AtRest>>,
    /// the XDFU stamping, when `xdfu` is not `off`
    pub xdfu: Option<std::sync::Arc<crate::xdfu::Xdfu>>,
    /// where `docs-diff` and `sheets-diff` are (`diff_bin_dir`)
    pub diff_bin_dir: Option<PathBuf>,
    /// how long one compare may run (seconds)
    pub diff_timeout_s: u64,
    /// the retention sweep's two periods, in seconds (`keep_closed_hours`, `keep_abandoned_days`)
    pub keep_closed_s: u64,
    pub keep_abandoned_s: u64,
    /// `close_idle_minutes`, in seconds
    pub close_idle_s: u64,
    /// `min_free_mb` and `max_versions_per_session` (gate review L1)
    pub min_free_mb: u64,
    pub max_versions: u64,
    /// `allow_opaque_origin`: no frame rule on our pages, `null` accepted as a frame origin
    pub allow_opaque_origin: bool,
    pub providers: Vec<Provider>,
    /// the audit log and the rate limits (WP8)
    pub watch: std::sync::Arc<crate::audit::Watch>,
    /// the name lookup of every outbound request; the system resolver unless a test replaces it
    pub lookup: crate::outbound::Lookup,
}

fn read_secret(path: &Path, key: &str) -> Result<Vec<u8>, String> {
    read_secret_min(path, key, 32)
}

fn read_secret_min(path: &Path, key: &str, min: usize) -> Result<Vec<u8>, String> {
    let s = std::fs::read(path)
        .map_err(|e| format!("docserver: {key}: cannot read {}: {e}", path.display()))?;
    let s = s.trim_ascii_end().to_vec();
    if s.len() < min {
        return Err(format!("docserver: {key} must hold at least {min} bytes"));
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
    /// None with `allow_opaque_origin`: no directive at all, since no source expression (`*` included) matches an opaque ancestor.
    pub fn frame_ancestors(&self) -> Option<String> {
        if self.allow_opaque_origin {
            return None;
        }
        let mut v = vec!["'self'".to_string()];
        for o in self.providers.iter().flat_map(|p| &p.frame_origins) {
            if !v.contains(o) {
                v.push(o.clone());
            }
        }
        Some(v.join(" "))
    }

    /// Binds the internal listener (a refusal to start when the address is taken). `None` when none is configured.
    pub fn bind_internal(&self) -> Result<Option<std::net::TcpListener>, String> {
        let Some(i) = self.internal.as_ref().filter(|i| !i.shared) else {
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
            .is_some_and(|i| i.allow.iter().any(|c| c.contains(ip)) || i.names.contains(ip))
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
        if !(1..=600).contains(&self.fetch_timeout_s) {
            return Err("docserver: fetch_timeout_s must be 1 to 600".into());
        }
        if !(1..=100_000).contains(&self.rate_create_per_min)
            || !(1..=100_000).contains(&self.rate_auth_fail_per_min)
        {
            return Err(
                "docserver: rate_create_per_min and rate_auth_fail_per_min must be 1 to 100000"
                    .into(),
            );
        }
        if !(1..=8760).contains(&self.keep_closed_hours) {
            return Err("docserver: keep_closed_hours must be 1 to 8760".into());
        }
        if !(1..=365).contains(&self.keep_abandoned_days) {
            return Err("docserver: keep_abandoned_days must be 1 to 365".into());
        }
        if !(1..=1440).contains(&self.close_idle_minutes) {
            return Err("docserver: close_idle_minutes must be 1 to 1440".into());
        }
        if self.min_free_mb > 1 << 20 {
            return Err("docserver: min_free_mb must be 0 to 1048576".into());
        }
        if !(2..=100_000).contains(&self.max_versions_per_session) {
            return Err("docserver: max_versions_per_session must be 2 to 100000".into());
        }
        let xdfu_mode =
            xdfu_core::Mode::parse(&self.xdfu).map_err(|e| format!("docserver: {e}"))?;
        if xdfu_mode == xdfu_core::Mode::Refuse {
            return Err("docserver: xdfu = \"refuse\" is an option of the converter server only; use \"watermark\"".into());
        }
        // a security option that does nothing is worse than a refusal
        if self.encrypt_key_file.is_some() && !self.encrypt_at_rest {
            return Err("docserver: encrypt_key_file is set but encrypt_at_rest is false".into());
        }
        if xdfu_mode != xdfu_core::Mode::Off && !self.encrypt_at_rest {
            return Err(
                "docserver: xdfu needs encrypt_at_rest = true (PLAN phase 5, section 4)".into(),
            );
        }
        if xdfu_mode != xdfu_core::Mode::Off && self.audit_key_file.is_none() {
            return Err(
                "docserver: xdfu needs audit_key_file: the copy numbers come from a keyed chain"
                    .into(),
            );
        }
        let at_rest_key = if self.encrypt_at_rest {
            let k = need(&self.encrypt_key_file, "encrypt_key_file")?;
            let k = read_secret(&k, "encrypt_key_file")?;
            if k == server_secret {
                return Err(
                    "docserver: encrypt_key_file holds the same secret as server_secret_file"
                        .into(),
                );
            }
            if let Some(a) = &self.audit_key_file
                && xdfu_core::audit::load_key(a).is_ok_and(|a| a == k)
            {
                return Err(
                    "docserver: encrypt_key_file holds the same secret as audit_key_file".into(),
                );
            }
            Some(k)
        } else {
            crate::atrest::refuse_encrypted_data(&data_dir)?;
            None
        };
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
                if listen.port() == 0 {
                    return Err("docserver: internal_listen needs a fixed port (not 0)".into());
                }
                let shared = Some(listen) == main_listen;
                if shared {
                    eprintln!(
                        "docserver: internal_listen {listen} is the public listener: its saved-file route answers internal_allow only"
                    );
                } else if main_listen.is_some_and(|m| {
                    m.port() == listen.port()
                        && (m.ip().is_unspecified() || listen.ip().is_unspecified())
                }) {
                    // the two binds would collide
                    return Err(format!(
                        "docserver: internal_listen {listen} has the public listener's port; for the shared layout write the same address as listen"
                    ));
                } else if listen.ip().is_unspecified() {
                    eprintln!(
                        "docserver: warning: internal_listen {listen} listens on every interface; bind it to the Docker network address and never publish it with -p"
                    );
                }
                let (names, cidrs): (Vec<&String>, Vec<&String>) = self
                    .internal_allow
                    .iter()
                    .partition(|c| NameSet::is_name(c));
                let allow = cidrs
                    .into_iter()
                    .map(|c| {
                        Cidr::parse(c)
                            .ok()
                            .filter(|c| c.prefix_len() > 0)
                            .ok_or(format!("docserver: internal_allow: \"{c}\" is not an IP address, a CIDR range or a container name (a /0 range is not accepted)"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let names = NameSet::start(names.into_iter().cloned().collect());
                Some(Internal {
                    listen,
                    allow,
                    names,
                    shared,
                })
            }
        };
        let internal_url = match (&internal, &self.internal_url) {
            (None, Some(_)) => return Err("docserver: internal_url needs internal_listen".into()),
            (None, None) => None,
            (Some(i), None) => {
                let ip = if i.listen.ip().is_unspecified() {
                    "127.0.0.1".parse().expect("literal")
                } else {
                    i.listen.ip()
                };
                Some(format!("http://{}", SocketAddr::new(ip, i.listen.port())))
            }
            (Some(_), Some(u)) => {
                let o = origin(u, "internal_url", true)?;
                if !o.starts_with("http://") {
                    return Err("docserver: internal_url must be a plain http://host:port (the listener has no TLS)".into());
                }
                Some(o)
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
            let compat = match p.compat.as_deref() {
                None => false,
                Some("docsapi") => true,
                Some(_) => {
                    return Err(format!(
                        "docserver: {} must be \"docsapi\" when set",
                        at("compat")
                    ));
                }
            };
            if compat && p.require_exp == Some(true) {
                return Err(format!(
                    "docserver: {}: compat = \"docsapi\" accepts tokens without exp; remove require_exp = true",
                    at("require_exp")
                ));
            }
            let secret = read_secret_min(
                secret_path,
                &at("secret_file"),
                if compat { 16 } else { 32 },
            )?;
            if secret.len() < 32 {
                eprintln!(
                    "docserver: WARNING: provider \"{}\": its secret is only {} bytes; a config token from any browser can be attacked offline. Rotate it to 32 random bytes or more",
                    p.name,
                    secret.len()
                );
            }
            const TYPES: [&str; 4] = ["docx", "xlsx", "pptx", "pdf"];
            let edit_types = match &p.edit_types {
                None => TYPES.map(String::from).to_vec(),
                Some(v) => {
                    let v: Vec<String> = v.iter().map(|t| t.to_ascii_lowercase()).collect();
                    if let Some(t) = v.iter().find(|t| !TYPES.contains(&t.as_str())) {
                        return Err(format!(
                            "docserver: {}: \"{t}\" is not docx, xlsx, pptx or pdf",
                            at("edit_types")
                        ));
                    }
                    v
                }
            };
            if let Some(l) = &p.ui_lang
                && !["uz-Latn", "uz-Cyrl", "ru", "en"].contains(&l.as_str())
            {
                return Err(format!(
                    "docserver: {} must be uz-Latn, uz-Cyrl, ru or en",
                    at("ui_lang")
                ));
            }
            if secret == server_secret {
                return Err(format!(
                    "docserver: {} holds the same secret as server_secret_file",
                    at("secret_file")
                ));
            }
            if at_rest_key.as_deref() == Some(&secret[..]) {
                return Err(format!(
                    "docserver: {} holds the same secret as encrypt_key_file",
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
            let keep_history = match p.history.as_str() {
                "session" => false,
                "keep" => true,
                _ => {
                    return Err(format!(
                        "docserver: {} must be \"session\" or \"keep\"",
                        at("history")
                    ));
                }
            };
            if p.frame_origins.is_empty() {
                return Err(format!(
                    "docserver: {} must list at least one origin",
                    at("frame_origins")
                ));
            }
            let mut frame_origins = vec![];
            for o in &p.frame_origins {
                let o = origin(o, &at("frame_origins"), false)?;
                // the browser's event.origin never writes the default port: a stored ":443" would refuse every session (create's check)
                let o = match (o.strip_suffix(":443"), o.strip_suffix(":80")) {
                    (Some(b), _) if b.starts_with("https://") => b.to_string(),
                    (_, Some(b)) if b.starts_with("http://") => b.to_string(),
                    _ => o,
                };
                // a page served over plain http could be rewritten in transit and then frame the editor with a live session
                if let Some(h) = o.strip_prefix("http://") {
                    let host = h
                        .rsplit_once(':')
                        .filter(|(_, p)| p.parse::<u16>().is_ok())
                        .map_or(h, |(h, _)| h);
                    if !["localhost", "127.0.0.1", "[::1]"].contains(&host) {
                        return Err(format!(
                            "docserver: {}: \"{o}\" is plain http; use https (http is accepted for localhost only)",
                            at("frame_origins")
                        ));
                    }
                }
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
                require_exp: p.require_exp.unwrap_or(!compat),
                keep_history,
                view_history: p.view_history,
                edit_types,
                ui_lang: p.ui_lang.clone(),
            });
        }
        for p in providers.iter().filter(|p| !p.require_exp) {
            eprintln!(
                "docserver: warning: provider \"{}\": require_exp = false accepts config tokens that never expire",
                p.name
            );
        }
        if self.allow_opaque_origin {
            eprintln!(
                "docserver: warning: allow_opaque_origin = true: any page may frame the editor (no frame-ancestors); the config token's signature is the only gate"
            );
        }
        let audit_path = self
            .audit_file
            .clone()
            .unwrap_or_else(|| data_dir.join("audit.jsonl"));
        let log = convert::audit::AuditLog::open_keyed(
            &audit_path,
            self.redact,
            self.audit_key_file.as_deref(),
        )
        .map_err(|e| format!("docserver: {e}"))?;
        if !log.is_keyed() {
            eprintln!(
                "docserver: warning: audit_key_file is not set: the audit chain only stops a naive edit, because anyone who can write {} can rewrite it and recompute the hashes",
                audit_path.display()
            );
        }
        let at_rest = at_rest_key
            .map(|k| {
                let a = crate::atrest::AtRest::new(&k);
                a.init(&data_dir).map(|()| std::sync::Arc::new(a))
            })
            .transpose()?;
        let xdfu = (xdfu_mode != xdfu_core::Mode::Off)
            .then(|| {
                crate::xdfu::Xdfu::open(&data_dir, self.audit_key_file.as_deref())
                    .map(std::sync::Arc::new)
            })
            .transpose()?;
        crate::history::sweep_compare_leftovers(&data_dir);
        let watch = std::sync::Arc::new(crate::audit::Watch::new(
            Some(log),
            self.rate_create_per_min,
            self.rate_auth_fail_per_min,
        ));
        Ok(Docserver {
            server_secret,
            data_dir,
            max_document_mb: self.max_document_mb,
            callback_timeout_s: self.callback_timeout_s,
            fetch_timeout_s: self.fetch_timeout_s,
            internal,
            internal_url,
            at_rest,
            xdfu,
            diff_bin_dir: self.diff_bin_dir.clone(),
            diff_timeout_s: 120,
            keep_closed_s: self.keep_closed_hours * 3600,
            keep_abandoned_s: self.keep_abandoned_days * 86_400,
            close_idle_s: self.close_idle_minutes * 60,
            min_free_mb: self.min_free_mb,
            max_versions: self.max_versions_per_session,
            allow_opaque_origin: self.allow_opaque_origin,
            providers,
            watch,
            lookup: crate::outbound::system_lookup(),
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
    fn the_audit_log_and_the_limits_are_checked_at_start() {
        let d = Dir::new("audit");
        // a damaged log is not extended: the server does not start
        std::fs::create_dir_all(d.path("data")).unwrap();
        std::fs::write(
            d.path("data/audit.jsonl"),
            "not json
",
        )
        .unwrap();
        refused(&d, &|_| {}, "damaged");
        std::fs::remove_file(d.path("data/audit.jsonl")).unwrap();
        // a key file that is too short, and limits out of range
        refused(
            &d,
            &|t| {
                t.insert("audit_key_file".into(), d.path("short").into());
            },
            "audit key",
        );
        std::fs::write(d.path("short"), "tiny").unwrap();
        refused(
            &d,
            &|t| {
                t.insert("audit_key_file".into(), d.path("short").into());
            },
            "at least 32",
        );
        refused(
            &d,
            &|t| {
                t.insert("rate_auth_fail_per_min".into(), 0.into());
            },
            "rate_create_per_min",
        );
        let ds = d.toml(&|_| {}).unwrap();
        assert!(ds.watch.auth_rl.hit("x").is_ok() && d.0.join("data").is_dir());
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
        assert_eq!(
            ds.frame_ancestors().as_deref(),
            Some("'self' https://app.test")
        );
        assert!(ds.internal.is_none());
        assert!(!ds.allow_opaque_origin, "off by default");
        // D3b: the switch removes the directive, whatever the frame origins are
        let on = d
            .toml(&|t| {
                t.insert("allow_opaque_origin".into(), true.into());
            })
            .unwrap();
        assert!(on.allow_opaque_origin);
        assert_eq!(on.frame_ancestors(), None);
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
            "http://app.test",
            "http://app.test:8080",
            "http://127.0.0.2:3000",
            "http://localhost.evil.test",
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
                        "http://127.0.0.1",
                        "http://[::1]:5173",
                        // the default port written out: event.origin never carries it, so the stored origin must not either
                        "https://app.test:443",
                        "http://localhost:80",
                    ]),
                );
            })
            .unwrap();
        assert_eq!(
            ds.frame_ancestors().as_deref(),
            Some(
                "'self' https://app.test http://localhost:3000 http://127.0.0.1 http://[::1]:5173 http://localhost"
            )
        );
    }

    #[test]
    fn internal_listen_port_and_mapped_ranges() {
        let d = Dir::new("internal2");
        let go = |listen: &'static str, allow: &'static str| {
            d.toml(&move |t| {
                t.insert("internal_listen".into(), listen.into());
                t.insert("internal_allow".into(), strings(&[allow]));
            })
        };
        assert!(
            go("127.0.0.1:0", "127.0.0.0/8")
                .err()
                .unwrap()
                .contains("not 0")
        );
        // a v4-mapped prefix cannot be written as a v6 range: refused with the key named, never half understood
        let e = go("127.0.0.1:9000", "::ffff:172.18.0.0/112").err().unwrap();
        assert!(e.contains("internal_allow"), "{e}");
        // a mapped single address is the v4 address
        let ds = go("127.0.0.1:9000", "::ffff:172.18.0.4").unwrap();
        assert!(ds.internal_allows("172.18.0.4".parse().unwrap()));
        assert!(!ds.internal_allows("172.18.0.5".parse().unwrap()));
        assert!(
            go("[::]:9000", "10.0.0.0/8").is_ok(),
            "warns, does not refuse"
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
        // the public address itself is the shared layout: nothing of its own to bind; the same port elsewhere is a refusal
        let ds = d
            .toml(&set(Some("127.0.0.1:8787"), &["10.0.0.0/8"]))
            .unwrap();
        assert!(ds.internal.as_ref().unwrap().shared && ds.bind_internal().unwrap().is_none());
        refused(
            &d,
            &set(Some("0.0.0.0:8787"), &["10.0.0.0/8"]),
            "the public listener's port",
        );
        assert!(
            !d.toml(&set(Some("127.0.0.1:9000"), &["10.0.0.0/8"]))
                .unwrap()
                .internal
                .unwrap()
                .shared
        );
        refused(
            &d,
            &set(Some("10.0.0.5:9000"), &["no such thing"]),
            "internal_allow",
        );
        // a container name is resolved (`localhost` stands in for one)
        let ds = d.toml(&set(Some("10.0.0.5:9000"), &["localhost"])).unwrap();
        assert!(
            ds.internal_allows("127.0.0.1".parse().unwrap())
                && !ds.internal_allows("10.0.0.6".parse().unwrap())
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
    fn encryption_and_xdfu_options_are_checked() {
        let d = Dir::new("encrypt");
        std::fs::write(d.path("enc"), [b'e'; 40]).unwrap();
        std::fs::write(d.path("short"), [b'e'; 10]).unwrap();
        std::fs::write(d.path("akey"), [b'k'; 40]).unwrap();
        let on = |t: &mut toml::Table, key: &str| {
            t.insert("encrypt_at_rest".into(), true.into());
            t.insert("encrypt_key_file".into(), d.path(key).into());
        };
        refused(
            &d,
            &|t| {
                t.insert("encrypt_at_rest".into(), true.into());
            },
            "encrypt_key_file is required",
        );
        refused(
            &d,
            &|t| {
                t.insert("encrypt_key_file".into(), d.path("enc").into());
            },
            "encrypt_key_file is set but encrypt_at_rest is false",
        );
        refused(
            &d,
            &|t| on(t, "short"),
            "encrypt_key_file must hold at least 32 bytes",
        );
        refused(
            &d,
            &|t| on(t, "server"),
            "same secret as server_secret_file",
        );
        refused(&d, &|t| on(t, "a"), "same secret as encrypt_key_file");
        refused(
            &d,
            &|t| {
                on(t, "enc");
                t.insert("audit_key_file".into(), d.path("enc").into());
            },
            "same secret as audit_key_file",
        );
        for (mode, needle) in [
            ("refuse", "converter server only"),
            ("on", "expected \"off\""),
            ("watermark", "xdfu needs encrypt_at_rest"),
        ] {
            refused(
                &d,
                &|t| {
                    t.insert("xdfu".into(), mode.into());
                },
                needle,
            );
        }
        refused(
            &d,
            &|t| {
                on(t, "enc");
                t.insert("xdfu".into(), "watermark".into());
            },
            "xdfu needs audit_key_file",
        );
        // the good combinations
        let ds = d.toml(&|t| on(t, "enc")).unwrap();
        assert!(ds.at_rest.is_some() && ds.xdfu.is_none());
        let ds = d
            .toml(&|t| {
                on(t, "enc");
                t.insert("xdfu".into(), "watermark".into());
                t.insert("audit_key_file".into(), d.path("akey").into());
            })
            .unwrap();
        assert!(ds.at_rest.is_some() && ds.xdfu.is_some());
        // the data folder now belongs to that key
        refused(
            &d,
            &|t| on(t, "akey"),
            "not the key this data_dir was encrypted with",
        );
        refused(&d, &|_| {}, "holds encrypted data");
    }

    #[test]
    fn limits_and_unknown_keys_are_checked() {
        let d = Dir::new("limits");
        for (k, v) in [
            ("max_document_mb", 0),
            ("max_document_mb", 501),
            ("callback_timeout_s", 0),
            ("callback_timeout_s", 601),
            ("keep_closed_hours", 0),
            ("keep_closed_hours", 8761),
            ("keep_abandoned_days", 0),
            ("keep_abandoned_days", 366),
            ("close_idle_minutes", 0),
            ("close_idle_minutes", 1441),
            ("min_free_mb", (1 << 20) + 1),
            ("max_versions_per_session", 1),
            ("max_versions_per_session", 100_001),
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
        refused(
            &d,
            &|t| {
                prov(t).insert("history".into(), "forever".into());
            },
            "providers[0].history must be",
        );
        let ds = d.toml(&|_| {}).unwrap();
        assert_eq!((ds.min_free_mb, ds.max_versions), (1024, 500));
        assert_eq!(
            (ds.keep_closed_s, ds.keep_abandoned_s),
            (7 * 86_400, 7 * 86_400)
        );
        assert!(
            !ds.providers[0].keep_history,
            "the default deletes history with its session"
        );
        let ds = d
            .toml(&|t| {
                t.insert("max_document_mb".into(), 500.into());
                prov(t).insert("require_exp".into(), false.into());
                prov(t).insert("history".into(), "keep".into());
            })
            .unwrap();
        assert!(!ds.providers[0].require_exp && ds.max_document_mb == 500);
        assert!(ds.providers[0].keep_history);
        // the idle close: 5 minutes unless set; ui_lang: none unless set, and one of the four
        assert!(ds.close_idle_s == 300 && ds.providers[0].ui_lang.is_none());
        let ds = d
            .toml(&|t| {
                t.insert("close_idle_minutes".into(), 2.into());
                prov(t).insert("ui_lang".into(), "uz-Latn".into());
            })
            .unwrap();
        assert_eq!(
            (ds.close_idle_s, ds.providers[0].ui_lang.as_deref()),
            (120, Some("uz-Latn"))
        );
        refused(
            &d,
            &|t| {
                prov(t).insert("ui_lang".into(), "uz".into());
            },
            "providers[0].ui_lang must be",
        );
    }

    #[test]
    fn the_docsapi_compat_mode() {
        let d = Dir::new("compat");
        std::fs::write(d.path("p20"), [b'p'; 20]).unwrap();
        std::fs::write(d.path("p10"), [b'q'; 10]).unwrap();
        let compat = |secret: &'static str| {
            let p = d.path(secret);
            move |t: &mut toml::Table| {
                prov(t).insert("compat".into(), "docsapi".into());
                prov(t).insert("secret_file".into(), p.clone().into());
            }
        };
        // no exp needed, a 20-byte secret passes (with a warning), every other check stays
        let ds = d.toml(&compat("p20")).unwrap();
        assert!(!ds.providers[0].require_exp && ds.providers[0].secret.len() == 20);
        assert_eq!(ds.providers[0].edit_types, ["docx", "xlsx", "pptx", "pdf"]);
        refused(&d, &compat("p10"), "at least 16 bytes");
        // without compat the 32-byte rule holds
        refused(
            &d,
            &|t| {
                prov(t).insert("secret_file".into(), d.path("p20").into());
            },
            "at least 32 bytes",
        );
        refused(
            &d,
            &|t| {
                compat("p20")(t);
                prov(t).insert("require_exp".into(), true.into());
            },
            "remove require_exp",
        );
        refused(
            &d,
            &|t| {
                prov(t).insert("compat".into(), "msoffice".into());
            },
            "providers[0].compat",
        );
        refused(
            &d,
            &|t| {
                compat("p20")(t);
                prov(t).remove("frame_origins");
            },
            "frame_origins must list",
        );
        let ds = d
            .toml(&|t| {
                prov(t).insert("edit_types".into(), strings(&["DOCX"]));
            })
            .unwrap();
        assert!(ds.providers[0].require_exp && ds.providers[0].edit_types == ["docx"]);
        refused(
            &d,
            &|t| {
                prov(t).insert("edit_types".into(), strings(&["doc"]));
            },
            "providers[0].edit_types",
        );
    }
}
