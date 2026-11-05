//! The native end for Sheets and Slides (Phase 7 WP13; `docs-office::plugin_native` for any app): the wasmtime host as a
//! [`crate::Guest`], the host-performed `fetch`, and the loader that adds the enabled, verified plugins of the suite-core store that
//! are for this app. Desktop only; the web build runs plugins in a Worker and never links wasmtime.
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use plugin_api::wire::Message;
use plugin_host::net::Net;
use serde_json::{Value, json};
use suite_core::plugins::Store;

use crate::{Fetch, Guest, GuestSession, Plugin};

/// The wall-clock budget of one round (the epoch deadline); the manifest sets only the memory.
const ROUND: Duration = Duration::from_secs(5);

struct Wasm(plugin_host::Plugin);
struct Sess(plugin_host::Session);

impl Guest for Wasm {
    fn start(&self) -> Result<Box<dyn GuestSession>, String> {
        self.0
            .start()
            .map(|s| Box::new(Sess(s)) as Box<dyn GuestSession>)
            .map_err(|e| e.to_string())
    }
}

impl GuestSession for Sess {
    fn round(&mut self, call: &Message) -> Result<Message, String> {
        self.0.round(call).map_err(|e| e.to_string())
    }
    fn limit_budget(&mut self, d: Duration) {
        self.0.limit_round_budget(d);
    }
}

struct Granted(Net);

impl Fetch for Granted {
    fn fetch(
        &self,
        url: &str,
        method: &str,
        headers: &BTreeMap<String, String>,
        body: Option<&str>,
    ) -> Result<Value, String> {
        self.0
            .fetch(url, method, headers, body)
            .map(|r| json!({"status": r.status, "body": r.body}))
    }
}

fn host() -> Result<&'static plugin_host::Host, String> {
    static HOST: OnceLock<Result<plugin_host::Host, String>> = OnceLock::new();
    HOST.get_or_init(plugin_host::Host::new)
        .as_ref()
        .map_err(Clone::clone)
}

/// A plugin module as a guest with the native limits (memory cap, 5 s round budget): what [`load`] gives each package, for tests
/// that load a module directly.
pub fn wasm_guest(wasm: &[u8], memory_mb: u32) -> Result<Arc<dyn Guest>, String> {
    let g = host()?
        .load(wasm, memory_mb, ROUND)
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(Wasm(g)))
}

/// Load every enabled, verified plugin of `store` that is for this app ([`crate::app`]) and add it, replacing what was added before.
/// A package the store or the host refuses is said on stderr and skipped. Returns the ids that were added; the caller binds and
/// registers their slots.
pub fn load(store: &Store) -> Vec<String> {
    let mut ids = Vec::new();
    let Ok(host) = host().inspect_err(|e| eprintln!("plugins: host did not start: {e}")) else {
        return ids;
    };
    let app = crate::app();
    let list: Vec<_> = store
        .enabled()
        .into_iter()
        .filter(|e| e.package.manifest.apps.iter().any(|a| a == app))
        .collect();
    for p in crate::plugins() {
        if !list.iter().any(|e| e.package.manifest.id == p.manifest.id) {
            crate::remove(&p.manifest.id);
        }
    }
    for e in list {
        let m = e.package.manifest.clone();
        let id = m.id.clone();
        let r = suite_core::plugins::read_wasm(&e.package)
            .and_then(|w| {
                host.load(&w, m.permissions.memory_mb, ROUND)
                    .map_err(|e| e.to_string())
            })
            .and_then(|g| {
                let mut p = Plugin::new(m, Arc::new(Wasm(g)));
                p.fetcher = Some(Arc::new(Granted(Net::new(e.grant.clone()))));
                p.origins = e.grant;
                crate::add(p)
            });
        match r {
            Ok(()) => ids.push(id),
            Err(why) => eprintln!("plugins: {id}: {why}"),
        }
    }
    ids
}

/// Once per process: load the user's plugins on a thread, then `after` (the engine crate's slot registration; the commands appear in
/// the toolbar on a later frame). Not when `no_prefs` names an environment variable that is set (the apps' isolated test runs).
pub fn init(no_prefs: &str, after: fn()) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if std::env::var_os(no_prefs).is_some() {
            return;
        }
        let Some(store) = Store::open() else { return };
        let _ = std::thread::Builder::new()
            .name("plugin-load".into())
            .spawn(move || {
                load(&store);
                after();
            });
    });
}
