//! The route table (PLAN phase 5, 3.1) with the security headers of the cross-site rules (F5), and the internal listener's router
//! (F4). The handlers are stubs that answer 501; WP3 and later replace them one by one. The header rules do not depend on them.
use crate::Docserver;
use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use convert_api::routes::{ApiError, CSP, NO_FRAME_OPTIONS, baseline_headers};
use std::net::SocketAddr;
use std::sync::Arc;

async fn stub() -> ApiError {
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "not_implemented",
        "this route is not available in this build yet",
    )
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such route")
}

impl Docserver {
    /// The strict baseline CSP with `frame-ancestors` widened to `frame_ancestors()`: for the wrapper page. Editor pages build theirs
    /// the same way in `suite-serve`.
    pub fn frame_csp(&self) -> String {
        CSP.replace(
            "frame-ancestors 'none'",
            &format!("frame-ancestors {}", self.frame_ancestors()),
        )
    }

    /// The public `/ds/` routes. Only `api.js` is cross-origin readable (`Cross-Origin-Resource-Policy`); only the wrapper page may be
    /// framed by the providers' origins. Every other route keeps the baseline headers.
    pub fn routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        let csp = HeaderValue::from_str(&self.frame_csp())
            .expect("frame origins are checked to be plain origins");
        let embed = Router::new()
            .route("/ds/api.js", get(stub))
            .route("/web-apps/apps/api/documents/api.js", get(stub))
            .layer(middleware::map_response(|mut r: Response| async move {
                r.headers_mut().insert(
                    "cross-origin-resource-policy",
                    HeaderValue::from_static("cross-origin"),
                );
                // only a good answer is cached; an error keeps the baseline no-store
                if matches!(r.status(), StatusCode::OK | StatusCode::NOT_MODIFIED) {
                    r.headers_mut().insert(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=300"),
                    );
                }
                r
            }));
        let frame = Router::new()
            .route("/ds/editor", get(stub))
            .layer(middleware::map_response(move |mut r: Response| {
                let csp = csp.clone();
                async move {
                    let h = r.headers_mut();
                    h.insert(header::CONTENT_SECURITY_POLICY, csp);
                    h.insert(header::X_FRAME_OPTIONS, NO_FRAME_OPTIONS);
                    r
                }
            }));
        Router::new()
            .merge(embed)
            .merge(frame)
            .merge(self.session_routes())
    }

    /// The router of the internal listener (F4): the token-bound file route and nothing else, for the configured addresses only.
    pub fn internal_router(self: &Arc<Self>) -> Router {
        self.file_route()
            .fallback(not_found)
            .layer(middleware::from_fn_with_state(self.clone(), internal_gate))
            .layer(middleware::map_response(|mut r: Response| async move {
                baseline_headers(r.headers_mut(), false);
                r
            }))
    }
}

async fn internal_gate(State(ds): State<Arc<Docserver>>, req: Request, next: Next) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    if peer.is_some_and(|ip| ds.internal_allows(ip)) {
        next.run(req).await
    } else {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "this address may not use the internal listener",
        )
        .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Allow, Internal, Provider};
    use axum::body::Body;
    use axum::http::Request;
    use convert_api::config::Cidr;
    use tower::ServiceExt;

    fn ds() -> Arc<Docserver> {
        let allow = |o: &str| Allow {
            origin: o.into(),
            internal: false,
        };
        Arc::new(Docserver {
            server_secret: vec![1; 32],
            data_dir: "d".into(),
            max_document_mb: 50,
            callback_timeout_s: 60,
            internal: Some(Internal {
                listen: "127.0.0.1:9000".parse().unwrap(),
                allow: vec![Cidr::parse("172.18.0.0/16").unwrap()],
            }),
            encrypt_at_rest: false,
            providers: vec![Provider {
                name: "p".into(),
                issuer: "i".into(),
                secret: vec![2; 32],
                fetch_allow: vec![allow("http://api:8000")],
                callback_allow: vec![],
                frame_origins: vec!["https://app.test".into()],
                require_exp: true,
            }],
        })
    }

    async fn call(r: Router, method: &str, uri: &str, peer: &str) -> (StatusCode, String) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(
            format!("{peer}:4000").parse::<SocketAddr>().unwrap(),
        ));
        let res = r.oneshot(req).await.unwrap();
        let h = res.headers();
        let csp = h
            .get("content-security-policy")
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        assert_eq!(h["x-frame-options"], "DENY", "{uri}");
        assert_eq!(h["cross-origin-resource-policy"], "same-origin", "{uri}");
        (res.status(), csp)
    }

    #[tokio::test]
    async fn internal_listener_serves_files_only() {
        let r = ds().internal_router();
        let ok = "172.18.3.4";
        assert_eq!(
            call(r.clone(), "GET", "/ds/files/s/1?t=x", ok).await.0,
            StatusCode::UNAUTHORIZED
        );
        // nothing else exists there: not the public routes, not the health check, not the editors
        for (m, uri) in [
            ("GET", "/healthz"),
            ("POST", "/ds/session"),
            ("GET", "/ds/api.js"),
            ("GET", "/ds/editor"),
            ("GET", "/docs/"),
            ("GET", "/api/v1/formats"),
            ("GET", "/ds/files/s"),
        ] {
            assert_eq!(
                call(r.clone(), m, uri, ok).await.0,
                StatusCode::NOT_FOUND,
                "{uri}"
            );
        }
        // an address outside internal_allow gets 403, even on the file route
        for peer in ["127.0.0.1", "172.19.0.1", "10.0.0.1", "8.8.8.8"] {
            assert_eq!(
                call(r.clone(), "GET", "/ds/files/s/1", peer).await.0,
                StatusCode::FORBIDDEN,
                "{peer}"
            );
        }
        // no peer address at all is refused too
        let req = Request::builder()
            .uri("/ds/files/s/1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            r.oneshot(req).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn mapped_ipv6_peers_are_judged_as_their_ipv4_address() {
        let r = ds().internal_router();
        let get = |peer: &str| {
            let r = r.clone();
            let peer: SocketAddr = format!("[{peer}]:4000").parse().unwrap();
            async move {
                let mut req = Request::builder()
                    .uri("/ds/files/s/1")
                    .body(Body::empty())
                    .unwrap();
                req.extensions_mut().insert(ConnectInfo(peer));
                r.oneshot(req).await.unwrap().status()
            }
        };
        assert_eq!(get("::ffff:172.18.3.4").await, StatusCode::UNAUTHORIZED, "allowed: on to the token check");
        assert_eq!(get("::ffff:8.8.8.8").await, StatusCode::FORBIDDEN);
        assert_eq!(get("::1").await, StatusCode::FORBIDDEN);
    }

    #[test]
    fn the_frame_csp_differs_from_the_baseline_in_frame_ancestors_only() {
        let framed = ds().frame_csp();
        assert!(
            framed.contains("frame-ancestors 'self' https://app.test;"),
            "{framed}"
        );
        assert_eq!(framed.replace("'self' https://app.test", "'none'"), CSP);
    }
}
