//! The route table (PLAN phase 5, 3.1) with the security headers of the cross-site rules (F5), and the internal listener's router
//! (F4). The embed script and the wrapper page (WP5) are static files of this crate; the session handlers are in `api`.
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

const API_JS: &str = include_str!("../static/api.js");
const EDITOR_HTML: &str = include_str!("../static/editor.html");
const EDITOR_JS: &str = include_str!("../static/editor.js");
/// The one line of `editor.js` that `allow_opaque_origin` changes (D3b): the wrapper then accepts a `null` host origin.
const OPAQUE_OFF: &str = "var OPAQUE = false;";

async fn api_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        API_JS,
    )
}

async fn editor_html() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        EDITOR_HTML,
    )
}

/// `editor.js` as served: the file itself, or with `OPAQUE` set when `allow_opaque_origin` is on.
fn editor_js_text(opaque: bool) -> String {
    if opaque {
        EDITOR_JS.replacen(OPAQUE_OFF, "var OPAQUE = true;", 1)
    } else {
        EDITOR_JS.to_string()
    }
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such route")
}

impl Docserver {
    /// The strict baseline CSP with `frame-ancestors` widened to `frame_ancestors()`, or without it (`allow_opaque_origin`): for the
    /// wrapper page. Editor pages build theirs the same way in `suite-serve`.
    pub fn frame_csp(&self) -> String {
        match self.frame_ancestors() {
            Some(a) => CSP.replace("frame-ancestors 'none'", &format!("frame-ancestors {a}")),
            None => CSP.replace("frame-ancestors 'none'; ", ""),
        }
    }

    /// The public `/ds/` routes. Only `api.js` is cross-origin readable (`Cross-Origin-Resource-Policy`); only the wrapper page may be
    /// framed by the providers' origins. Every other route keeps the baseline headers.
    pub fn routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        let csp = HeaderValue::from_str(&self.frame_csp())
            .expect("frame origins are checked to be plain origins");
        let embed = Router::new()
            .route("/ds/api.js", get(api_js))
            .route("/web-apps/apps/api/documents/api.js", get(api_js))
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
        let frame =
            Router::new()
                .route("/ds/editor", get(editor_html))
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
            .route("/ds/editor.js", {
                let js: Arc<str> = editor_js_text(self.allow_opaque_origin).into();
                get(move || async move {
                    (
                        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                        js.to_string(),
                    )
                })
            })
            .merge(self.session_routes())
            .merge(self.history_routes())
    }

    /// The router of the internal listener (F4): the token-bound file route and nothing else, for the configured addresses only.
    pub fn internal_router(self: &Arc<Self>) -> Router {
        self.gated(self.file_route().fallback(not_found))
    }

    fn gated(self: &Arc<Self>, r: Router) -> Router {
        r.layer(middleware::from_fn_with_state(self.clone(), internal_gate))
            .layer(middleware::map_response(|mut r: Response| async move {
                baseline_headers(r.headers_mut(), false);
                r
            }))
    }

    /// The shared layout (`internal_listen` = the public `listen`, docs/phase5/dropin.md D2b): the file route, behind the internal
    /// peer gate, is merged into the finished public router, so the public guard (proxy only) does not wrap it and the gate wraps
    /// nothing else. Every other path, `/ds/files/<other shapes>` included, is the public router's, unchanged. Without the shared
    /// layout `public` is returned as it is.
    pub fn share_port(self: &Arc<Self>, public: Router) -> Router {
        if !self.internal.as_ref().is_some_and(|i| i.shared) {
            return public;
        }
        public.merge(self.gated(self.file_route()))
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
            fetch_timeout_s: 60,
            internal_url: None,
            lookup: crate::outbound::system_lookup(),
            watch: Arc::new(crate::audit::Watch::off()),
            internal: Some(Internal {
                listen: "127.0.0.1:9000".parse().unwrap(),
                allow: vec![Cidr::parse("172.18.0.0/16").unwrap()],
                names: Default::default(),
                shared: false,
            }),
            at_rest: None,
            xdfu: None,
            diff_bin_dir: None,
            diff_timeout_s: 120,
            keep_closed_s: 24 * 3600,
            keep_abandoned_s: 7 * 86_400,
            close_idle_s: 300,
            min_free_mb: 0,
            max_versions: 500,
            allow_opaque_origin: false,
            providers: vec![Provider {
                name: "p".into(),
                issuer: "i".into(),
                secret: vec![2; 32],
                fetch_allow: vec![allow("http://api:8000")],
                callback_allow: vec![],
                frame_origins: vec!["https://app.test".into()],
                require_exp: true,
                keep_history: false,
                view_history: false,
                edit_types: vec!["docx".into()],
                ui_lang: None,
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
        assert_eq!(
            get("::ffff:172.18.3.4").await,
            StatusCode::UNAUTHORIZED,
            "allowed: on to the token check"
        );
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
        // D3b: the switch drops the directive and changes nothing else
        let mut on = (*ds()).clone();
        on.allow_opaque_origin = true;
        let open = on.frame_csp();
        assert!(!open.contains("frame-ancestors"), "{open}");
        assert_eq!(
            open.replace(
                "object-src 'none'; ",
                "object-src 'none'; frame-ancestors 'none'; "
            ),
            CSP
        );
    }

    #[test]
    fn editor_js_changes_one_line_and_only_with_the_switch() {
        assert_eq!(EDITOR_JS.matches(OPAQUE_OFF).count(), 1);
        assert_eq!(editor_js_text(false), EDITOR_JS, "off: the file as it is");
        let on = editor_js_text(true);
        assert_eq!(on.replacen("var OPAQUE = true;", OPAQUE_OFF, 1), EDITOR_JS);
        assert!(!on.contains(OPAQUE_OFF));
    }
}
