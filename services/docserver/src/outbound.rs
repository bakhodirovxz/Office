//! The outbound HTTP client (PLAN phase 5, D4, 4 "SSRF", S6-S9): the one way the docserver reaches a provider, for the document fetch
//! and the callbacks. A request goes out only for a URL the caller has already matched against the provider's allowlist; this module
//! adds the network half. The name is resolved once, by our own resolver, and EVERY address it returns is judged: if any is in a
//! denied range the request is refused, and the connection goes to those checked addresses (ureq asks the resolver and connects to
//! what it returned, so the name is never resolved a second time: no DNS rebinding). No redirects, no proxy, a size cap while the body
//! streams, a connect timeout and a total timeout.
//!
//! Plain `http` only: the workspace's ureq has no TLS (`default-features = false`, as the converter's). An `https` entry in an
//! allowlist is refused here with a clear error until a TLS feature is chosen (open question in PLAN 9).
//! `shortcut: ureq's resolver API is "unversioned" (may change in a minor release), and Cargo.lock pins 3.4.2; re-check on upgrade.`
use crate::Docserver;
use std::fmt;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

/// `host, port` to every address of the name. Tests replace it; production uses the system resolver.
pub type Lookup = Arc<dyn Fn(&str, u16) -> io::Result<Vec<IpAddr>> + Send + Sync>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn system_lookup() -> Lookup {
    Arc::new(|host, port| {
        // a blocking getaddrinfo cannot be interrupted: run it on a thread and stop waiting after the timeout
        let (tx, rx) = std::sync::mpsc::channel();
        let h = host.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(
                (h.as_str(), port)
                    .to_socket_addrs()
                    .map(|i| i.collect::<Vec<_>>()),
            );
        });
        match rx.recv_timeout(RESOLVE_TIMEOUT) {
            Ok(r) => Ok(r?.into_iter().map(|a| a.ip()).collect()),
            Err(_) => Err(io::Error::new(ErrorKind::TimedOut, "name lookup timed out")),
        }
    })
}

/// What an address is, for the deny rules.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Class {
    Public,
    /// loopback, private, link-local, CGNAT, unique-local: allowed only for an `internal = true` entry
    Internal,
    /// never allowed, `internal` or not
    Never(&'static str),
}

pub fn classify(ip: IpAddr) -> Class {
    match ip.to_canonical() {
        IpAddr::V4(a) => classify4(a),
        IpAddr::V6(a) => classify6(a),
    }
}

fn classify4(a: Ipv4Addr) -> Class {
    use Class::*;
    match a.octets() {
        [169, 254, 169, 254] => Never("metadata address"),
        [0, ..] => Never("unspecified (0.0.0.0/8)"),
        [127, ..] | [10, ..] | [192, 168, ..] | [169, 254, ..] => Internal,
        [172, 16..=31, ..] | [100, 64..=127, ..] => Internal,
        [224..=255, ..] => Never("multicast, reserved or broadcast"),
        [192, 0, 0 | 2, _] | [198, 18 | 19, ..] | [198, 51, 100, _] | [203, 0, 113, _] => {
            Never("reserved or documentation range")
        }
        _ => Public,
    }
}

fn classify6(a: Ipv6Addr) -> Class {
    use Class::*;
    let s = a.segments();
    if a.is_unspecified() {
        Never("unspecified (::)")
    } else if a.is_loopback() {
        Internal
    } else if s[..6] == [0; 6] {
        Never("IPv4-compatible (::/96)")
    } else if s[0] == 0xfd00 && s[1] == 0x0ec2 {
        Never("metadata address")
    } else if s[0] & 0xfe00 == 0xfc00 || s[0] & 0xffc0 == 0xfe80 {
        Internal
    } else if s[0] & 0xffc0 == 0xfec0 {
        Never("site-local (fec0::/10)")
    } else if s[0] & 0xff00 == 0xff00 {
        Never("multicast")
    } else if s[0] == 0x2002 {
        Never("6to4 (2002::/16)")
    } else if s[0] == 0x0064 && s[1] == 0xff9b {
        Never("NAT64 (64:ff9b::/32)")
    } else if s[0] == 0x2001 && (s[1] == 0 || s[1] == 0x0db8) {
        Never("Teredo or documentation range")
    } else {
        Public
    }
}

/// Resolves through `lookup`, then refuses the whole name if any answer is denied.
struct Guard {
    lookup: Lookup,
    internal: bool,
}

impl fmt::Debug for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Guard")
    }
}

impl Resolver for Guard {
    fn resolve(
        &self,
        uri: &Uri,
        _config: &Config,
        _timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let host = uri.host().ok_or(ureq::Error::HostNotFound)?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let port = uri.port_u16().unwrap_or(80);
        let ips = (self.lookup)(host, port)?;
        let mut out = self.empty();
        for ip in ips.into_iter().take(16) {
            match classify(ip) {
                Class::Public => {}
                Class::Internal if self.internal => {}
                Class::Internal => {
                    return Err(blocked("a private, loopback or link-local address"));
                }
                Class::Never(why) => return Err(blocked(why)),
            }
            out.push(SocketAddr::new(ip, port));
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(out)
    }
}

fn blocked(why: &str) -> ureq::Error {
    ureq::Error::Io(io::Error::new(
        ErrorKind::PermissionDenied,
        format!("blocked: {why}"),
    ))
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutErr {
    /// SSRF guard (scheme, address range)
    Blocked(String),
    Redirect,
    Status(u16),
    TooLarge,
    /// the total time ran out (for a callback: the provider may or may not have acted)
    Timeout,
    Unreachable(String),
}

impl fmt::Display for OutErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutErr::Blocked(w) => write!(f, "refused: {w}"),
            OutErr::Redirect => f.write_str("refused: the server answered with a redirect"),
            OutErr::Status(s) => write!(f, "status {s}"),
            OutErr::TooLarge => f.write_str("the body is over the size limit"),
            OutErr::Timeout => f.write_str("timed out"),
            OutErr::Unreachable(w) => write!(f, "unreachable: {w}"),
        }
    }
}

fn map_err(e: ureq::Error) -> OutErr {
    match e {
        ureq::Error::Io(io) if io.kind() == ErrorKind::PermissionDenied => {
            OutErr::Blocked(io.to_string().trim_start_matches("blocked: ").into())
        }
        ureq::Error::Timeout(_) => OutErr::Timeout,
        ureq::Error::BodyExceedsLimit(_) => OutErr::TooLarge,
        ureq::Error::TooManyRedirects => OutErr::Redirect,
        // a stalled body surfaces as an I/O timeout
        ureq::Error::Io(io) if matches!(io.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
            OutErr::Timeout
        }
        e => OutErr::Unreachable(e.to_string()),
    }
}

/// `scheme://host:port` + path of a URL, with the query and fragment cut off: the only form that goes into a log.
pub fn host_path(u: &str) -> String {
    u.split(['?', '#']).next().unwrap_or("").to_string()
}

/// Only `scheme://host:port`: for a URL whose path carries a credential (the provider's own file token).
pub fn host_only(u: &str) -> String {
    let (s, rest) = u.split_once("://").unwrap_or(("", u));
    format!("{s}://{}", rest.split(['/', '?', '#']).next().unwrap_or(""))
}

impl Docserver {
    fn agent(&self, internal: bool, total: Duration) -> ureq::Agent {
        let config = Config::builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .proxy(None)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(total))
            .build();
        ureq::Agent::with_parts(
            config,
            DefaultConnector::new(),
            Guard {
                lookup: self.lookup.clone(),
                internal,
            },
        )
    }

    fn plain_http(url: &str) -> Result<(), OutErr> {
        if url
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
        {
            Ok(())
        } else {
            Err(OutErr::Blocked(
                "only http:// is supported for now (no TLS in the outbound client)".into(),
            ))
        }
    }

    /// GET `url` (already matched to an allowlist entry, `internal` from that entry): status 200 and a body of at most `max_bytes`.
    pub fn fetch_bytes(
        &self,
        url: &str,
        internal: bool,
        max_bytes: u64,
    ) -> Result<Vec<u8>, OutErr> {
        Self::plain_http(url)?;
        let agent = self.agent(internal, Duration::from_secs(self.fetch_timeout_s));
        let mut r = agent.get(url).call().map_err(map_err)?;
        let s = r.status().as_u16();
        if (300..400).contains(&s) {
            return Err(OutErr::Redirect);
        }
        if s != 200 {
            return Err(OutErr::Status(s));
        }
        // the header first (cheap refusal), then the count while streaming (the header may lie or be absent)
        if r.body().content_length().is_some_and(|n| n > max_bytes) {
            return Err(OutErr::TooLarge);
        }
        r.body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(map_err)
    }

    /// POST a JSON `body` with an `Authorization: Bearer` token; returns the status and at most 64 KB of the answer.
    pub fn post_json(
        &self,
        url: &str,
        internal: bool,
        bearer: &str,
        body: &[u8],
    ) -> Result<(u16, Vec<u8>), OutErr> {
        Self::plain_http(url)?;
        let agent = self.agent(internal, Duration::from_secs(self.callback_timeout_s));
        let mut r = agent
            .post(url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {bearer}"))
            .send(body)
            .map_err(map_err)?;
        let s = r.status().as_u16();
        if (300..400).contains(&s) {
            return Err(OutErr::Redirect);
        }
        let b = r
            .body_mut()
            .with_config()
            .limit(64 << 10)
            .read_to_vec()
            .map_err(map_err)?;
        Ok((s, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Class::*;

    fn c(s: &str) -> Class {
        classify(s.parse().unwrap())
    }

    #[test]
    fn every_deny_range() {
        for ip in [
            "127.0.0.1",
            "127.255.255.254",
            "10.0.0.5",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "100.127.255.255",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.1.2.3",
            "::ffff:7f00:1",
        ] {
            assert_eq!(c(ip), Internal, "{ip}");
        }
        for ip in [
            "0.0.0.0",
            "0.1.2.3",
            "224.0.0.1",
            "239.255.255.255",
            "240.0.0.1",
            "255.255.255.255",
            "169.254.169.254",
            "::ffff:169.254.169.254",
            "fd00:ec2::254",
            "::",
            "::ffff:0.0.0.0",
            "::2",
            "ff02::1",
            "fec0::1",
            "2002:7f00:1::1",
            "2002:0a00:0001::",
            "64:ff9b::7f00:1",
            "64:ff9b::a9fe:a9fe",
            "2001::1",
            "2001:db8::1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.7",
            "203.0.113.9",
        ] {
            assert!(matches!(c(ip), Never(_)), "{ip}: {:?}", c(ip));
        }
        for ip in [
            "8.8.8.8",
            "93.184.216.34",
            "172.32.0.1",
            "100.128.0.1",
            "2606:4700::1111",
        ] {
            assert_eq!(c(ip), Public, "{ip}");
        }
    }
}
