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
    let chain = CertificateDer::pem_file_iter(cert).map_err(|e| format!("tls_cert: {e}"))?.collect::<Result<Vec<_>, _>>().map_err(|e| format!("tls_cert: {e}"))?;
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
        .fallback(move |uri: axum::http::Uri| async move { Redirect::permanent(&format!("{base}{}", uri.path_and_query().map_or("/", |p| p.as_str()))) })
        .layer(axum::middleware::map_response(|mut r: axum::response::Response| async move {
            crate::routes::baseline_headers(r.headers_mut(), false);
            r
        }))
}

/// Serve `router` over TLS on `listener` until `stop` completes. A handshake has 10 s, request headers have 10 s, and at most
/// 1024 connections are open (the rest are dropped), so an idle or slow client cannot hold the server.
pub async fn serve(listener: tokio::net::TcpListener, cfg: Arc<rustls::ServerConfig>, router: Router, stop: impl std::future::Future<Output = ()>) {
    use hyper_util::rt::{TokioIo, TokioTimer};
    use std::time::Duration;
    use tower::ServiceExt;
    let acceptor = tokio_rustls::TlsAcceptor::from(cfg);
    let slots = Arc::new(tokio::sync::Semaphore::new(1024));
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
        let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
        let (acceptor, router) = (acceptor.clone(), router.clone());
        tokio::spawn(async move {
            let _permit = permit;
            let Ok(Ok(tls)) = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(tcp)).await else { return };
            let svc = hyper::service::service_fn(move |mut req: axum::http::Request<hyper::body::Incoming>| {
                req.extensions_mut().insert(axum::extract::ConnectInfo(addr));
                router.clone().oneshot(req.map(axum::body::Body::new))
            });
            let _ = hyper::server::conn::http1::Builder::new().timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(10)).serve_connection(TokioIo::new(tls), svc).await;
        });
    }
}
