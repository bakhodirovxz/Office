//! TLS for server mode (PLAN 5.5): rustls with the ring provider, TLS 1.3 plus TLS 1.2 with AEAD suites only.
use axum::Router;
use axum::response::Redirect;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::path::Path;
use std::sync::Arc;

/// The provider with every non-AEAD suite removed. rustls implements no CBC or RC4 suite at all, so this filter is a guard
/// against a future provider change, and `tests/api.rs` checks the result.
pub fn provider() -> rustls::crypto::CryptoProvider {
    let mut p = rustls::crypto::ring::default_provider();
    p.cipher_suites.retain(|s| {
        let n = format!("{:?}", s.suite());
        n.contains("_GCM_") || n.contains("CHACHA20_POLY1305")
    });
    p
}

pub fn server_config(cert: &Path, key: &Path) -> Result<Arc<rustls::ServerConfig>, String> {
    let chain = CertificateDer::pem_file_iter(cert)
        .map_err(|e| format!("tls_cert: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("tls_cert: {e}"))?;
    let key = PrivateKeyDer::from_pem_file(key).map_err(|e| format!("tls_key: {e}"))?;
    let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(provider()))
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .map_err(|e| format!("tls_cert/tls_key: {e}"))?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

/// The plain-HTTP side of server mode: everything redirects to the configured public URL (never to the request's `Host`).
pub fn redirect_router(public_url: &str) -> Router {
    let base = public_url.trim_end_matches('/').to_string();
    Router::new()
        .fallback(move |uri: axum::http::Uri| async move {
            Redirect::permanent(&format!(
                "{base}{}",
                uri.path_and_query().map_or("/", |p| p.as_str())
            ))
        })
        .layer(axum::middleware::map_response(
            |mut r: axum::response::Response| async move {
                crate::routes::baseline_headers(r.headers_mut(), false);
                r
            },
        ))
}

/// Connection limits of every listener (TLS, the redirect listener and the local one): a total, a cap per client address (an IPv6
/// /64), and how long a connection lives. Peers listed in `exempt` (the trusted proxies) are not counted per address, because
/// every client of the proxy comes from it; the total still applies to them.
#[derive(Clone)]
pub struct ConnLimits {
    pub total: usize,
    pub per_ip: usize,
    pub exempt: Vec<crate::config::Cidr>,
    /// a connection is closed after this long, however busy; an upload (`upload_timeout_s`) must fit
    pub max_age: std::time::Duration,
    /// a connection is closed (`Connection: close`) after this many requests
    pub max_requests: usize,
    /// handshake, and the wait for the head of each request including the idle wait between keep-alive requests
    pub head_timeout: std::time::Duration,
}

impl ConnLimits {
    pub fn from_settings(r: &crate::config::Resolved) -> ConnLimits {
        ConnLimits {
            total: r.s.max_connections,
            per_ip: r.s.max_conn_per_ip,
            exempt: r.trusted.clone(),
            max_age: std::time::Duration::from_secs(r.s.upload_timeout_s.max(3600) + 600),
            max_requests: 100,
            head_timeout: std::time::Duration::from_secs(10),
        }
    }
}

/// Releases a client address's slot when its connection ends.
struct PerIp(
    Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
    Option<String>,
);

impl Drop for PerIp {
    fn drop(&mut self) {
        let Some(k) = self.1.take() else { return };
        let mut m = self.0.lock().unwrap();
        if let Some(n) = m.get_mut(&k) {
            *n -= 1;
            if *n == 0 {
                m.remove(&k);
            }
        }
    }
}

async fn run_connection<I>(io: I, addr: std::net::SocketAddr, router: Router, lim: ConnLimits)
where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use hyper_util::rt::{TokioIo, TokioTimer};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;
    let seen = Arc::new(AtomicUsize::new(0));
    let max = lim.max_requests;
    let svc =
        hyper::service::service_fn(move |mut req: axum::http::Request<hyper::body::Incoming>| {
            req.extensions_mut()
                .insert(axum::extract::ConnectInfo(addr));
            let (router, last) = (
                router.clone(),
                seen.fetch_add(1, Ordering::Relaxed) + 1 >= max,
            );
            async move {
                let mut res = router.oneshot(req.map(axum::body::Body::new)).await;
                if last && let Ok(r) = res.as_mut() {
                    r.headers_mut().insert(
                        axum::http::header::CONNECTION,
                        axum::http::HeaderValue::from_static("close"),
                    );
                }
                res
            }
        });
    let conn = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(lim.head_timeout)
        .serve_connection(TokioIo::new(io), svc);
    let _ = tokio::time::timeout(lim.max_age, conn).await;
}

/// Serve `router` on `listener` until `stop` completes: over TLS when `tls` is given, else plain HTTP. Handshake and request-head
/// timeouts, a connection cap per client address and in total, a maximum connection age and a maximum number of requests per
/// connection apply to every listener, so an idle or slow client cannot hold a slot for ever (WP7 review M3).
pub async fn serve(
    listener: tokio::net::TcpListener,
    tls: Option<Arc<rustls::ServerConfig>>,
    router: Router,
    lim: ConnLimits,
    stop: impl std::future::Future<Output = ()>,
) {
    use std::time::Duration;
    let acceptor = tls.map(tokio_rustls::TlsAcceptor::from);
    let slots = Arc::new(tokio::sync::Semaphore::new(lim.total));
    let per_ip: Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>> = Arc::default();
    tokio::pin!(stop);
    loop {
        let (tcp, addr) = tokio::select! {
            r = listener.accept() => match r {
                Ok(x) => x,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            () = &mut stop => break,
        };
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            continue;
        };
        let ip_guard = if lim.exempt.iter().any(|c| c.contains(addr.ip())) {
            PerIp(per_ip.clone(), None)
        } else {
            let key = crate::app::ip_key(addr.ip());
            let mut m = per_ip.lock().unwrap();
            let n = m.entry(key.clone()).or_insert(0);
            if *n >= lim.per_ip {
                continue;
            }
            *n += 1;
            PerIp(per_ip.clone(), Some(key))
        };
        let (acceptor, router, lim) = (acceptor.clone(), router.clone(), lim.clone());
        tokio::spawn(async move {
            let (_permit, _ip) = (permit, ip_guard);
            match acceptor {
                Some(a) => {
                    let Ok(Ok(io)) = tokio::time::timeout(lim.head_timeout, a.accept(tcp)).await
                    else {
                        return;
                    };
                    run_connection(io, addr, router, lim).await;
                }
                None => run_connection(tcp, addr, router, lim).await,
            }
        });
    }
}
