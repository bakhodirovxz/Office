//! The native `fetch` action (PLAN phase 7 WP3 "TODO from WP2 (fetch)"; docs/plugins.md sections 4 and 8).
//!
//! Plain `https` only, to origins the user or admin granted (byte for byte). The host name is resolved here, every address is
//! checked ([`blocked`]), and the connection goes to the checked address (the name is not resolved a second time). A redirect is
//! followed only inside the same origin, up to [`MAX_REDIRECTS`] hops, and every hop is resolved and checked again. The host sends
//! no cookies and no credentials of its own. TLS is rustls with the `ring` provider and the operating system's trust roots.
//!
//! shortcut: HTTP/1.1 with `Connection: close`, no proxy support, no keep-alive; upgrade when a plugin needs a corporate proxy.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plugin_api::action::url_origin;
use rustls::pki_types::ServerName;

pub const MAX_REDIRECTS: usize = 3;
/// The response body kept (the web host keeps the same 1 MiB), and the header block read.
pub const MAX_BODY: usize = 1 << 20;
const MAX_HEAD: usize = 64 * 1024;
/// One whole `fetch`, all hops: connect, send, read.
const TOTAL: Duration = Duration::from_secs(10);

/// What the plugin gets back: the status and the body as text (cut at [`MAX_BODY`]).
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

/// May the host connect to this address? `false` for everything that is not a public unicast address: loopback, private
/// (10/8, 172.16/12, 192.168/16), link-local (169.254/16, with the cloud metadata address), CGNAT (100.64/10), unspecified,
/// "this network", multicast, broadcast and reserved v4; v6 loopback, unspecified, multicast, ULA fc00::/7, link-local fe80::/10,
/// site-local fec0::/10, and every v6 form that carries a v4 address (mapped, compatible, NAT64 64:ff9b::/96, 6to4 2002::/16),
/// judged by the v4 address inside.
pub fn blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => blocked_v4(a),
        IpAddr::V6(a) => blocked_v6(a),
    }
}

fn blocked_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_loopback()
        || a.is_private()
        || a.is_link_local()
        || a.is_unspecified()
        || a.is_multicast()
        || a.is_broadcast()
        || o[0] == 0 // 0.0.0.0/8 "this network"
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
        || o[0] >= 240 // 240.0.0.0/4 reserved
}

fn blocked_v6(a: Ipv6Addr) -> bool {
    if let Some(v4) = a.to_ipv4() {
        // ::ffff:a.b.c.d (mapped) and ::a.b.c.d (compatible); this also covers :: and ::1 through 0.0.0.0 and 0.0.0.1
        return blocked_v4(v4) || a.is_unspecified() || a.is_loopback();
    }
    let s = a.segments();
    let b = a.octets();
    let embedded = |i: usize| blocked_v4(Ipv4Addr::new(b[i], b[i + 1], b[i + 2], b[i + 3]));
    a.is_unspecified()
        || a.is_loopback()
        || a.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
        || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        || (s[0] & 0xffc0) == 0xfec0 // fec0::/10 site-local (deprecated)
        || (s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0; 4] && embedded(12)) // NAT64
        || (s[0] == 0x2002 && embedded(2)) // 6to4
}

type Resolver = dyn Fn(&str, u16) -> std::io::Result<Vec<IpAddr>> + Send + Sync;

/// The host side of `fetch` for one plugin: the origins it was granted.
pub struct Net {
    origins: Vec<String>,
    resolve: Box<Resolver>,
}

impl Net {
    pub fn new(origins: Vec<String>) -> Net {
        Net::with_resolver(origins, |h, p| {
            Ok((h, p).to_socket_addrs()?.map(|a| a.ip()).collect())
        })
    }

    /// A resolver of your own (tests).
    pub fn with_resolver(
        origins: Vec<String>,
        resolve: impl Fn(&str, u16) -> std::io::Result<Vec<IpAddr>> + Send + Sync + 'static,
    ) -> Net {
        Net {
            origins,
            resolve: Box::new(resolve),
        }
    }

    /// Run one request. `headers` and `body` were checked by `plugin_api::action::check_actions`; the origin is checked again here.
    pub fn fetch(
        &self,
        url: &str,
        method: &str,
        headers: &std::collections::BTreeMap<String, String>,
        body: Option<&str>,
    ) -> Result<Response, String> {
        let deadline = Instant::now() + TOTAL;
        let (mut url, mut method, mut body) = (url.to_string(), method.to_string(), body);
        let first = url_origin(&url).map(str::to_string);
        for _ in 0..=MAX_REDIRECTS {
            let target = Target::parse(&url, &self.origins)?;
            if first.as_deref() != Some(target.origin) {
                return Err("a redirect to another origin was refused".into());
            }
            let addr = self.pick(&target)?;
            let (head, resp) = self.once(addr, &target, &method, headers, body, deadline)?;
            if matches!(resp.status, 301 | 302 | 303 | 307 | 308)
                && let Some(loc) = head.header("location")
            {
                url = redirect_url(target.origin, loc).ok_or("a redirect target was refused")?;
                if resp.status == 303 || (matches!(resp.status, 301 | 302) && method == "POST") {
                    method = "GET".into();
                    body = None;
                }
                continue;
            }
            return Ok(resp);
        }
        Err("too many redirects".into())
    }

    /// Resolve and check: one blocked address among the answers refuses the name (a mixed answer is a rebinding attempt).
    fn pick(&self, t: &Target) -> Result<SocketAddr, String> {
        let ips = (self.resolve)(t.host, t.port)
            .map_err(|_| "the host name could not be resolved".to_string())?;
        if ips.is_empty() || ips.iter().any(|ip| blocked(*ip)) {
            return Err(format!(
                "{} resolves to an address the host does not connect to",
                t.host
            ));
        }
        Ok(SocketAddr::new(ips[0], t.port))
    }

    fn once(
        &self,
        addr: SocketAddr,
        t: &Target,
        method: &str,
        headers: &std::collections::BTreeMap<String, String>,
        body: Option<&str>,
        deadline: Instant,
    ) -> Result<(Head, Response), String> {
        let left = || {
            deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or("the request timed out")
        };
        let tcp = TcpStream::connect_timeout(&addr, left()?)
            .map_err(|e| format!("connect failed: {}", e.kind()))?;
        let t_left = left()?;
        tcp.set_read_timeout(Some(t_left))
            .and_then(|()| tcp.set_write_timeout(Some(t_left)))
            .map_err(|e| e.to_string())?;
        let name =
            ServerName::try_from(t.host.to_string()).map_err(|_| "bad host name".to_string())?;
        let conn = rustls::ClientConnection::new(tls_config()?, name).map_err(|e| e.to_string())?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        let mut req = format!(
            "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept-Encoding: identity\r\n",
            t.path,
            t.host_header()
        );
        for (k, v) in headers {
            if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
                return Err("a request header holds a line break".into());
            }
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = body {
            req.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        req.push_str("\r\n");
        let send = |tls: &mut rustls::StreamOwned<_, TcpStream>| -> std::io::Result<()> {
            tls.write_all(req.as_bytes())?;
            if let Some(b) = body {
                tls.write_all(b.as_bytes())?;
            }
            tls.flush()
        };
        send(&mut tls).map_err(|e| format!("send failed: {}", e.kind()))?;
        let mut raw = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        while raw.len() < MAX_HEAD + MAX_BODY + 64 * 1024 {
            left()?;
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&buf[..n]),
                // a server that closes without close_notify after a complete answer is common; the parser checks completeness
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Err("the request timed out".into());
                }
                Err(e) => return Err(format!("read failed: {}", e.kind())),
            }
        }
        parse_response(&raw)
    }
}

fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    static CFG: OnceLock<Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
        if roots.is_empty() {
            return Err("no trusted root certificates found".into());
        }
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Arc::new(cfg))
    })
    .clone()
}

/// A checked request URL split into the parts the request needs.
struct Target<'a> {
    origin: &'a str,
    host: &'a str,
    port: u16,
    path: String,
}

impl<'a> Target<'a> {
    fn parse(url: &'a str, granted: &[String]) -> Result<Target<'a>, String> {
        let origin = url_origin(url).ok_or("not a plain https URL")?;
        if !granted.iter().any(|g| g == origin) {
            return Err(format!("{origin} is not granted"));
        }
        let authority = &origin[8..];
        let (host, port) = match authority.split_once(':') {
            Some((h, p)) => (h, p.parse().map_err(|_| "bad port")?),
            None => (authority, 443),
        };
        let rest = url[origin.len()..].split('#').next().unwrap_or("");
        // an empty path, or a bare `?query`, is "/" in the request target
        let path = if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        };
        Ok(Target {
            origin,
            host,
            port,
            path,
        })
    }

    fn host_header(&self) -> String {
        if self.port == 443 {
            self.host.to_string()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// The absolute URL a `Location` header names, when it stays on `origin`: an absolute URL of that origin, or a path starting
/// with a single `/`. Anything else (another origin, a scheme-relative `//`, a relative path) is refused.
fn redirect_url(origin: &str, loc: &str) -> Option<String> {
    if loc
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    if loc.starts_with('/') && !loc.starts_with("//") {
        return Some(format!("{origin}{loc}"));
    }
    (url_origin(loc)? == origin).then(|| loc.to_string())
}

struct Head {
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn parse_response(raw: &[u8]) -> Result<(Head, Response), String> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the answer has no header block")?;
    if end > MAX_HEAD {
        return Err("the answer header is too large".into());
    }
    let head = std::str::from_utf8(&raw[..end]).map_err(|_| "the answer header is not text")?;
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .filter(|l| l.starts_with("HTTP/1."))
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("bad status line")?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let head = Head { headers };
    let data = &raw[end + 4..];
    let body = if head
        .header("transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    {
        dechunk(data)?
    } else {
        let len = head
            .header("content-length")
            .map(|v| v.parse::<usize>().map_err(|_| "bad content-length"))
            .transpose()?;
        match len {
            Some(n) if data.len() < n.min(MAX_BODY) => {
                return Err("the answer was cut short".into());
            }
            Some(n) => data[..n.min(data.len())].to_vec(),
            None => data.to_vec(),
        }
    };
    let body = String::from_utf8_lossy(&body[..body.len().min(MAX_BODY)]).into_owned();
    Ok((head, Response { status, body }))
}

fn dechunk(mut d: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = d
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("bad chunked body")?;
        let size = std::str::from_utf8(&d[..eol])
            .ok()
            .and_then(|s| usize::from_str_radix(s.split(';').next()?.trim(), 16).ok())
            .ok_or("bad chunk size")?;
        d = &d[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if d.len() < size {
            // truncated at the size cap: keep what came, but only when the cap is the reason
            if out.len() + d.len() >= MAX_BODY {
                out.extend_from_slice(d);
                return Ok(out);
            }
            return Err("the answer was cut short".into());
        }
        out.extend_from_slice(&d[..size]);
        d = d.get(size + 2..).ok_or("bad chunked body")?;
        if out.len() >= MAX_BODY {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn address_table() {
        // (address, refused)
        let table = [
            ("127.0.0.1", true),
            ("127.255.255.254", true),
            ("10.0.0.1", true),
            ("10.255.255.255", true),
            ("172.16.0.1", true),
            ("172.31.255.255", true),
            ("172.15.255.255", false),
            ("172.32.0.1", false),
            ("192.168.0.1", true),
            ("192.168.255.255", true),
            ("169.254.0.1", true),
            ("169.254.169.254", true),
            ("100.64.0.1", true),
            ("100.127.255.255", true),
            ("100.63.255.255", false),
            ("100.128.0.1", false),
            ("0.0.0.0", true),
            ("0.1.2.3", true),
            ("224.0.0.1", true),
            ("239.255.255.250", true),
            ("255.255.255.255", true),
            ("240.0.0.1", true),
            ("8.8.8.8", false),
            ("93.184.216.34", false),
            ("::", true),
            ("::1", true),
            ("ff02::1", true),
            ("ff0e::1", true),
            ("fc00::1", true),
            ("fd12:3456::1", true),
            ("fe80::1", true),
            ("febf::1", true),
            ("fec0::1", true),
            ("::ffff:127.0.0.1", true),
            ("::ffff:10.1.2.3", true),
            ("::ffff:172.16.0.1", true),
            ("::ffff:192.168.1.1", true),
            ("::ffff:169.254.169.254", true),
            ("::ffff:100.64.0.1", true),
            ("::ffff:0.0.0.0", true),
            ("::ffff:224.0.0.1", true),
            ("::ffff:8.8.8.8", false),
            ("::127.0.0.1", true),
            ("64:ff9b::7f00:1", true),
            ("64:ff9b::a9fe:a9fe", true),
            ("64:ff9b::808:808", false),
            ("2002:7f00:1::1", true),
            ("2002:c0a8:101::1", true),
            ("2002:808:808::1", false),
            ("2001:4860:4860::8888", false),
            ("2606:2800:220:1:248:1893:25c8:1946", false),
        ];
        for (a, refused) in table {
            assert_eq!(blocked(ip(a)), refused, "{a}");
        }
    }

    fn net(ips: &[&str]) -> Net {
        let ips: Vec<IpAddr> = ips.iter().map(|s| s.parse().unwrap()).collect();
        Net::with_resolver(vec!["https://api.example.org".into()], move |_, _| {
            Ok(ips.clone())
        })
    }

    fn get(n: &Net, url: &str) -> Result<Response, String> {
        n.fetch(url, "GET", &Default::default(), None)
    }

    #[test]
    fn a_public_name_that_resolves_to_a_private_address_is_refused() {
        for bad in [
            &["127.0.0.1"][..],
            &["10.0.0.5"],
            &["169.254.169.254"],
            &["::1"],
            &["::ffff:192.168.0.1"],
            &["8.8.8.8", "10.0.0.5"],
            &[],
        ] {
            let e = get(&net(bad), "https://api.example.org/x").unwrap_err();
            assert!(e.contains("resolves to an address"), "{bad:?}: {e}");
        }
    }

    #[test]
    fn the_origin_must_be_granted_and_plain_https() {
        let n = net(&["8.8.8.8"]);
        assert!(
            get(&n, "https://other.example.org/")
                .unwrap_err()
                .contains("not granted")
        );
        assert!(
            get(&n, "http://api.example.org/")
                .unwrap_err()
                .contains("plain https")
        );
        assert!(get(&n, "https://api.example.org@evil.example.org/").is_err());
        assert!(get(&n, "https://127.0.0.1/").is_err());
        assert!(
            get(&n, "https://api.example.org:8443/")
                .unwrap_err()
                .contains("not granted")
        );
    }

    #[test]
    fn redirects_stay_in_the_origin() {
        let o = "https://api.example.org";
        assert_eq!(
            redirect_url(o, "/a?b=1").as_deref(),
            Some("https://api.example.org/a?b=1")
        );
        assert_eq!(
            redirect_url(o, "https://api.example.org/z").as_deref(),
            Some("https://api.example.org/z")
        );
        for bad in [
            "https://other.example.org/",
            "//other.example.org/",
            "http://api.example.org/",
            "relative",
            "https://127.0.0.1/",
            "/a b",
            "https://api.example.org:8443/",
        ] {
            assert_eq!(redirect_url(o, bad), None, "{bad}");
        }
    }

    #[test]
    fn targets() {
        let g = vec![
            "https://api.example.org".to_string(),
            "https://api.example.org:8443".to_string(),
        ];
        let t = Target::parse("https://api.example.org/a/b?x=1#f", &g).unwrap();
        assert_eq!(
            (t.host, t.port, t.path.as_str(), t.host_header().as_str()),
            ("api.example.org", 443, "/a/b?x=1", "api.example.org")
        );
        let t = Target::parse("https://api.example.org:8443", &g).unwrap();
        assert_eq!(
            (t.port, t.path.as_str(), t.host_header().as_str()),
            (8443, "/", "api.example.org:8443")
        );
        let t = Target::parse("https://api.example.org?q=1", &g).unwrap();
        assert_eq!(t.path, "/?q=1");
    }

    #[test]
    fn responses() {
        let (h, r) =
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX: y\r\n\r\nhello").unwrap();
        assert_eq!(
            (r.status, r.body.as_str(), h.header("x")),
            (200, "hello", Some("y"))
        );
        let (_, r) = parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;e=1\r\nde\r\n0\r\n\r\n").unwrap();
        assert_eq!(r.body, "abcde");
        let (h, r) = parse_response(b"HTTP/1.1 302 Found\r\nLocation: /next\r\n\r\n").unwrap();
        assert_eq!((r.status, h.header("LOCATION")), (302, Some("/next")));
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nhello").is_err());
        assert!(parse_response(b"garbage").is_err());
        assert!(
            parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n").is_err()
        );
    }

    /// Live check against the public internet; run by hand: `cargo test -p plugin-host live -- --ignored`.
    #[test]
    #[ignore]
    fn live_https_get() {
        let n = Net::new(vec!["https://example.org".into()]);
        let r = get(&n, "https://example.org/").unwrap();
        assert_eq!(r.status, 200);
        assert!(r.body.contains("Example Domain"));
    }
}
