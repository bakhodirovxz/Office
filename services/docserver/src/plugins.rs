//! Plugins for a provider's users (PLAN phase 7, section 8). The admin puts packages in `[docserver.plugins] dir` and lists them per
//! provider (`config`); this module is what the browser sees of that:
//!
//! - the session answer lists the plugins of the session's provider (`plugin_descriptors`), nothing else: the config token is not
//!   read for plugins at all, so ONLYOFFICE's `editorConfig.plugins` / `pluginsData` cannot add one (M12, `onlyoffice-compat.md`);
//! - `GET /ds/plugins/<sha256>?s=<sid>` serves the `plugin.wasm` of a plugin of that session's provider, read again and compared
//!   with the digest on every request (a changed package is refused). The session id is the credential, as in the file route. Any
//!   other plugin, or a session that is not live, gets the same 404;
//! - `POST /ds/session/{sid}/plugin-event` lets the wrapper page report what the page host did with the network grant, for the
//!   audit line `plugin.network` (the fetch itself runs in the page, so the server cannot see it any other way).
//!
//! Audit lines: `plugin.loaded`, `plugin.refused` (also at start, for an entry the folder does not match), `plugin.network`.
use crate::Docserver;
use crate::api::{Cl, authorize, blk, client, json_ok};
use crate::audit::sid_ref;
use crate::config::ProviderPlugin;
use crate::sessions::{SessionRecord, valid_sid};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use convert_api::jwt::now;
use convert_api::routes::ApiError;
use serde_json::{Value, json};
use std::sync::Arc;

type Ds = State<Arc<Docserver>>;

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such plugin")
}

impl Docserver {
    fn plugins_of(&self, provider: &str) -> &[ProviderPlugin] {
        self.provider(provider).map_or(&[], |p| &p.plugins)
    }

    /// The descriptors the page host gets (`docs/plugins.md` 9a): the provider's plugins, `grant` always present (the admin's list is
    /// final, nobody is asked), `wasm` a same-origin URL that carries the session id.
    pub(crate) fn plugin_descriptors(&self, rec: &SessionRecord) -> Vec<Value> {
        self.plugins_of(&rec.provider)
            .iter()
            .map(|pl| {
                suite_core::plugins::descriptor(
                    &pl.package,
                    &format!("/ds/plugins/{}?s={}", pl.package.sha256, rec.sid),
                    Some(&pl.grant),
                    pl.settings.as_ref(),
                )
            })
            .collect()
    }

    /// Every origin any provider grants a plugin: the wrapper page's `connect-src`.
    /// shortcut: one list for all providers, because the page's policy is fixed before its session (and so its provider) is known;
    /// the page host still lets a plugin reach only its own descriptor's `grant`. A per-provider page would need the provider in the URL.
    pub fn plugin_origins(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .providers
            .iter()
            .flat_map(|p| &p.plugins)
            .flat_map(|pl| pl.grant.iter().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// One line per session a minute: the sha is the client's choice, so it is not a reason to write a line each time.
    fn refused(&self, rec: &SessionRecord, sha: &str, reason: &str) {
        self.audit_once(
            &format!("plugin.refused/{}", rec.sid),
            "plugin.refused",
            json!({"session": sid_ref(&rec.sid), "provider": rec.provider, "sha256": sha, "reason": reason}),
        );
    }

    pub(crate) fn plugin_routes<S: Clone + Send + Sync + 'static>(self: &Arc<Self>) -> Router<S> {
        Router::new()
            .route("/ds/plugins/{sha}", get(file))
            .route(
                "/ds/session/{sid}/plugin-event",
                post(event).layer(DefaultBodyLimit::max(4096)),
            )
            .with_state(self.clone())
    }
}

async fn file(
    State(ds): Ds,
    Path(sha): Path<String>,
    req: Request,
) -> Result<axum::response::Response, ApiError> {
    // a sha256 in lower-case hex, nothing else: no unknown string reaches the log or the lookup
    if sha.len() != 64 || !sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(not_found());
    }
    let sid = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("s=")))
        .filter(|s| valid_sid(s))
        .ok_or_else(not_found)?
        .to_string();
    let bytes = blk(move || {
        let rec = ds.load_session(&sid).filter(|r| r.closed.is_none() && now() <= r.expires)?;
        let Some(pl) = ds.plugins_of(&rec.provider).iter().find(|p| p.package.sha256 == sha) else {
            ds.refused(&rec, &sha, "not a plugin of this provider");
            return None;
        };
        match suite_core::plugins::read_wasm(&pl.package) {
            Ok(b) => {
                ds.audit_once(
                    &format!("plugin.loaded/{sid}/{sha}"),
                    "plugin.loaded",
                    json!({"session": sid_ref(&sid), "provider": rec.provider, "plugin": pl.package.manifest.id,
                           "version": pl.package.manifest.version, "sha256": sha}),
                );
                Some(Ok(b))
            }
            Err(e) => {
                ds.refused(&rec, &sha, &e);
                Some(Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "plugin_changed",
                    "the plugin file changed on the server",
                )))
            }
        }
    })
    .await?
    .ok_or_else(not_found)??;
    Ok(([(header::CONTENT_TYPE, "application/wasm")], bytes).into_response())
}

/// `{plugin, kind, origin}` from the wrapper page: what the page host did with a `fetch` action. Only the plugins of the session's
/// provider, only three kinds, only a well-formed origin.
async fn event(
    State(ds): Ds,
    cl: Cl,
    Path(sid): Path<String>,
    h: HeaderMap,
    body: Bytes,
) -> Result<axum::response::Response, ApiError> {
    let rec = authorize(&ds, &h, &sid, false, "plugin-event", &client(cl))?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    let (plugin, kind, origin) = (
        v["plugin"].as_str().unwrap_or(""),
        v["kind"].as_str().unwrap_or(""),
        v["origin"].as_str(),
    );
    let known = ds
        .plugins_of(&rec.provider)
        .iter()
        .any(|p| p.package.manifest.id == plugin);
    let kind_ok = ["network", "network.refused", "network.denied"].contains(&kind);
    if !known || !kind_ok {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "unknown plugin or event",
        ));
    }
    let origin = origin.filter(|o| plugin_api::manifest::check_origin(o));
    // a `network` line says the page host let a fetch through, so its origin must be one the admin granted this plugin; the other
    // kinds name what was turned away
    if kind == "network"
        && !ds.plugins_of(&rec.provider).iter().any(|p| {
            p.package.manifest.id == plugin
                && origin.is_some_and(|o| p.grant.iter().any(|g| g == o))
        })
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "unknown plugin or event",
        ));
    }
    if let Err(secs) = ds.watch.auth_rl.hit(&format!("plugin-event/{sid}")) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many events for this session",
        )
        .retry(secs));
    }
    // the page reports it, so the line is a claim of the client, not an observation of the server
    ds.audit(
        "plugin.network",
        json!({"session": sid_ref(&sid), "provider": rec.provider, "plugin": plugin, "outcome": kind, "origin": origin, "source": "client"}),
    );
    json_ok(json!({"ok": true}))
}
